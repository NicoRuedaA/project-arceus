//! Overworld capture: capture roll, capture animation machine glue and baits (Brief 18, part b).
//!
//! The roll calls the same engine functions `Battle::attempt_capture` uses (`capture_index`,
//! `capture_probability` and the two-check draw from `two_shake_capture`), with the overworld
//! inputs: full HP, no status, player/pokemon rank 1 (ASSUMED, the port has no rank system).
//! The two draws are re-implemented in [`draw_capture_checks`] — same order and bound as
//! `two_shake_capture` — because the shake count needs how many checks passed. A test proves the
//! result and the final RNG state match `two_shake_capture` for the same seed.
//!
//! The RNG seed comes from `PlayerProgress::rng_seed`, the same source battles use
//! (`save_wire`), and every roll writes the advanced state back.
//!
//! Baits: `data/capture/bait_parameters.tsv` (item id, type, taste, colours, special effect).
//! A bait is a projectile; once it lands it attracts nearby wild creatures by re-homing their
//! `WanderHome` (reusing `wild_wander`), and feeding creatures get the bait factor from
//! `capture_context_modifiers.tsv`. Attraction radius, lifetime and the "favourite bait" rule
//! are ASSUMED (not decoded).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use bevy::prelude::*;

use crate::battle::BattleData;
use crate::battle_engine::{
    capture_index, capture_probability, BattleRng, CaptureModifiers, Combatant, XorShift32,
};

use super::ball_lifecycle::{BallBody, BallSimulation};
use super::save_game::{PartyMember, MAX_PARTY};
use super::save_wire::PlayerProgress;
use super::state::{not_paused, GameState};
use super::throwing::BallKind;
use super::wild::WildCreature;
use super::wild_wander::{WanderHome, WanderParams, WanderState};

/// Capture checks per throw (`evaluation_num` in `capture_constants.tsv`).
pub const CAPTURE_CHECKS: u8 = 2;
/// Random bound of the two checks (matches `two_shake_capture`).
pub const RANDOM_VALUE_MAX: u32 = 65_536;
/// Shakes shown when the capture succeeds (the recovered 3-shake sequence, FUN_018065e8).
pub const SHAKES_ON_CAPTURE: u8 = 3;

/// ASSUMED: winged balls count as "suitable range" from this throw distance on.
pub const LONG_RANGE_M: f32 = 12.0;
/// ASSUMED: dot(creature forward, throw velocity) above this counts as a backstrike.
pub const BACKSTRIKE_DOT: f32 = 0.3;
/// ASSUMED: creatures inside this radius walk towards an armed bait.
pub const BAIT_ATTRACT_RADIUS: f32 = 20.0;
/// ASSUMED: attraction re-homes the creature inside this radius around the bait.
pub const BAIT_TARGET_RADIUS: f32 = 2.0;
/// ASSUMED: feeding distance for the bait capture factor.
pub const BAIT_FEED_RADIUS: f32 = 3.0;
/// ASSUMED: how long an armed bait stays in the world.
pub const BAIT_LIFETIME_S: f32 = 20.0;

/// Inputs of one overworld capture roll.
#[derive(Debug, Clone, Copy)]
pub struct WorldCaptureInput<'a> {
    /// Capture-table category (see [`BallKind::category`]).
    pub ball_category: &'a str,
    /// Whether the throw counts as suitable range for the ball line.
    pub suitable_range: bool,
    /// Throw from behind the creature (unaware strike).
    pub backstrike: bool,
    /// Species catch rate (`catch_rate.rs`).
    pub catch_rate: u8,
    /// Current HP / max HP. The overworld has no damage, so callers pass 1.0.
    pub hp_ratio: f64,
    pub player_rank: u8,
    pub pokemon_rank: u8,
    pub modifiers: CaptureModifiers,
}

/// Outcome of one overworld capture roll.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldCaptureRoll {
    /// `capture_probability(index)`, 0..=1.
    pub probability: f64,
    /// Checks passed in order (0, 1 or 2).
    pub checks_passed: u8,
    /// Shakes to play before the result (`SHAKES_ON_CAPTURE` when caught).
    pub shakes: u8,
    pub caught: bool,
}

/// Shakes played for a result: the full 3-shake sequence on a catch, `checks_passed + 1`
/// otherwise (a failed first check breaks out after one shake).
pub fn shake_count(caught: bool, checks_passed: u8) -> u8 {
    if caught {
        SHAKES_ON_CAPTURE
    } else {
        (checks_passed + 1).min(SHAKES_ON_CAPTURE)
    }
}

/// The two capture draws, in the same order and with the same bound as
/// `battle_engine::two_shake_capture`, returning how many checks passed.
pub fn draw_capture_checks(probability: f64, rng: &mut XorShift32) -> (u8, bool) {
    let failure = (1.0 - probability.clamp(0.0, 1.0)).sqrt();
    let fail_1 = f64::from(rng.below(RANDOM_VALUE_MAX)) / f64::from(RANDOM_VALUE_MAX) < failure;
    let fail_2 = f64::from(rng.below(RANDOM_VALUE_MAX)) / f64::from(RANDOM_VALUE_MAX) < failure;
    let checks_passed = match (fail_1, fail_2) {
        (false, false) => 2,
        (false, true) => 1,
        _ => 0,
    };
    (checks_passed, !fail_1 && !fail_2)
}

/// Probability of one overworld capture. Uses the decoded tables when loaded; otherwise the
/// hardcoded sheet values as fallback (values decoded, rank/context handling ASSUMED).
pub fn capture_probability_for(
    data: Option<&BattleData>,
    input: &WorldCaptureInput<'_>,
) -> Result<f64, String> {
    let (sick, ball, context, rank) = match data {
        Some(d) => (
            d.capture_status_multiplier("None")?,
            d.capture_ball_multiplier(input.ball_category, input.suitable_range)?,
            d.capture_factor(if input.backstrike {
                "capture_index_backstrike_success_factor"
            } else {
                "capture_index_backstrike_failure_factor"
            })?
            .unwrap_or(1.0),
            d.capture_level_rank_adjustment(input.player_rank, input.pokemon_rank)?,
        ),
        None => (
            1.0,
            fallback_ball_multiplier(input.ball_category, input.suitable_range),
            if input.backstrike { 1.75 } else { 1.0 },
            0,
        ),
    };
    let index = capture_index(
        input.catch_rate,
        input.hp_ratio,
        rank,
        sick,
        ball,
        context,
        input.modifiers,
    );
    Ok(capture_probability(index))
}

/// Hardcoded copy of `capture_ball_multipliers.tsv` (used only when `BattleData` is absent).
pub fn fallback_ball_multiplier(category: &str, suitable_range: bool) -> f64 {
    match (category, suitable_range) {
        ("Regular", _) => 0.75,
        ("Great", _) => 1.5,
        ("Ultra", _) => 2.25,
        ("Feather", false) => 0.5,
        ("Feather", true) => 1.25,
        ("Wing", false) => 1.25,
        ("Wing", true) => 2.0,
        ("Jet", false) => 2.0,
        ("Jet", true) => 2.75,
        ("Heavy", false) => 1.0,
        ("Heavy", true) => 1.25,
        ("Leaden", false) => 1.75,
        ("Leaden", true) => 2.0,
        ("Gigaton", false) => 2.5,
        ("Gigaton", true) => 2.75,
        _ => 1.0,
    }
}

/// One overworld capture roll: probability from the tables, then the two engine draws.
/// `rng` is advanced exactly like `attempt_capture` advances the battle RNG (two draws).
pub fn roll_capture(
    data: Option<&BattleData>,
    input: WorldCaptureInput<'_>,
    rng: &mut XorShift32,
) -> Result<WorldCaptureRoll, String> {
    let probability = capture_probability_for(data, &input)?;
    let (checks_passed, caught) = draw_capture_checks(probability, rng);
    Ok(WorldCaptureRoll {
        probability,
        checks_passed,
        shakes: shake_count(caught, checks_passed),
        caught,
    })
}

/// Whether the throw counts as "suitable range" for the ball line. ASSUMED conditions:
/// winged balls at long distance, heavy balls on a backstrike, standard line always.
pub fn suitable_range(kind: BallKind, throw_distance: f32, backstrike: bool) -> bool {
    if kind.prefers_long_range() {
        return throw_distance >= LONG_RANGE_M;
    }
    if kind.prefers_backstrike() {
        return backstrike;
    }
    true
}

/// Unaware strike: the throw travels the way the creature faces (it never saw it). ASSUMED
/// threshold; the creature's facing comes from the wander yaw.
pub fn is_backstrike(creature_forward: Vec3, throw_velocity: Vec3) -> bool {
    let f = Vec3::new(creature_forward.x, 0.0, creature_forward.z).normalize_or_zero();
    let v = Vec3::new(throw_velocity.x, 0.0, throw_velocity.z).normalize_or_zero();
    v.dot(f) > BACKSTRIKE_DOT
}

/// One row of `data/capture/bait_parameters.tsv`.
#[derive(Debug, Clone, PartialEq)]
pub struct BaitRow {
    pub item_id: u16,
    pub bait_type: String,
    pub taste: String,
    pub category: String,
    pub color_primary: Color,
    pub color_secondary: Color,
    pub special_effect: String,
}

/// Decoded bait / berry rows.
#[derive(Debug, Clone, Default, Resource)]
pub struct BaitTable {
    by_id: std::collections::HashMap<u16, BaitRow>,
    order: Vec<u16>,
}

impl BaitTable {
    /// Load `<data_root>/capture/bait_parameters.tsv`.
    pub fn load(data_root: impl AsRef<Path>) -> Result<Self, String> {
        let path = data_root.as_ref().join("capture/bait_parameters.tsv");
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_tsv(&text)
    }

    pub fn from_tsv(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'));
        let header: Vec<&str> = lines
            .next()
            .ok_or("bait parameters: missing header")?
            .split('\t')
            .map(|h| h.split(':').next().unwrap_or(h))
            .collect();
        let col = |name: &str| {
            header
                .iter()
                .position(|h| *h == name)
                .ok_or_else(|| format!("bait parameters: missing column {name}"))
        };
        let (c_id, c_type, c_taste, c_cat, c_p, c_s, c_fx) = (
            col("item_id")?,
            col("bait_type")?,
            col("taste")?,
            col("category")?,
            col("color_hex_primary")?,
            col("color_hex_secondary")?,
            col("special_effect")?,
        );
        let mut table = Self::default();
        for (n, line) in lines.enumerate() {
            let v: Vec<&str> = line.split('\t').collect();
            if v.len() != header.len() {
                return Err(format!("bait row {}: column count", n + 2));
            }
            let item_id: u16 = v[c_id]
                .parse()
                .map_err(|_| format!("bait row {}: bad item id", n + 2))?;
            let row = BaitRow {
                item_id,
                bait_type: v[c_type].to_owned(),
                taste: v[c_taste].to_owned(),
                category: v[c_cat].to_owned(),
                color_primary: parse_hex_color(v[c_p]).unwrap_or(Color::srgb(0.8, 0.8, 0.8)),
                color_secondary: parse_hex_color(v[c_s]).unwrap_or(Color::srgb(0.2, 0.2, 0.2)),
                special_effect: v[c_fx].to_owned(),
            };
            if table.by_id.insert(item_id, row).is_none() {
                table.order.push(item_id);
            }
        }
        Ok(table)
    }

    pub fn get(&self, item_id: u16) -> Option<&BaitRow> {
        self.by_id.get(&item_id)
    }

    /// Item ids in file order.
    pub fn ids(&self) -> &[u16] {
        &self.order
    }

    /// Bait ids the bag actually holds (quantity > 0), in bag order.
    pub fn present_in(&self, bag: &BTreeMap<u16, u16>) -> Vec<u16> {
        bag.iter()
            .filter(|(id, n)| **n > 0 && self.by_id.contains_key(id))
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// `RRGGBB` (as in the sheet) to a colour; `None` on malformed input.
pub fn parse_hex_color(hex: &str) -> Option<Color> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some(Color::srgb_u8(r, g, b))
}

/// Bait capture factor: `capture_context_modifiers.tsv` values, hardcoded fallback.
/// ASSUMED: any feeding state maps to the "available" factor (favourite-bait detection is
/// not decoded).
pub fn bait_multiplier(available: bool, data: Option<&BattleData>) -> f64 {
    let key = if available {
        "capture_index_baiteffect_available_factor"
    } else {
        "capture_index_baiteffect_not_available_factor"
    };
    data.and_then(|d| d.capture_factor(key).ok().flatten())
        .unwrap_or(if available { 1.3 } else { 1.0 })
}

/// True when any bait position is within `radius` (planar) of the creature.
pub fn bait_in_range(
    creature_pos: (f32, f32),
    bait_positions: impl IntoIterator<Item = (f32, f32)>,
    radius: f32,
) -> bool {
    bait_positions
        .into_iter()
        .any(|b| ((b.0 - creature_pos.0).powi(2) + (b.1 - creature_pos.1).powi(2)).sqrt() <= radius)
}

/// Where a world catch was stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatchStorage {
    Party,
    /// Party full: the Pokemon went to the box list.
    Box,
    /// Party and box full: recorded as caught, not stored (same extreme as `save_wire`).
    Full,
}

/// Record a world catch in the progress: caught set, counter and party/box storage.
pub fn store_caught(progress: &mut PlayerProgress, member: PartyMember) -> CatchStorage {
    progress.caught.insert(member.species);
    progress.caught_count = progress.caught_count.saturating_add(1);
    if progress.party.len() < MAX_PARTY {
        progress.party.push(member);
        CatchStorage::Party
    } else if progress.deposit(member).is_ok() {
        CatchStorage::Box
    } else {
        CatchStorage::Full
    }
}

/// Build the caught member for a species from the battle data (tackle fallback like
/// `battle_controller::build_combatant`).
pub fn build_caught_member(
    data: &BattleData,
    species: &str,
    level: u8,
) -> Result<PartyMember, String> {
    let spec = data
        .species_row(species)?
        .ok_or_else(|| format!("unknown species {species}"))?;
    let mut moves = Vec::new();
    if let Some(m) = data.move_row("tackle")? {
        moves.push(m);
    }
    Ok(PartyMember::from_combatant(&Combatant::from_base(
        spec, level, moves,
    )))
}

// ------------------------------------------------------------------ bevy glue

/// A thrown bait in the world. While `expires_at` is `None` it is still a projectile.
#[derive(Component)]
pub struct BaitObject {
    pub item_id: u16,
    /// When the armed bait disappears (`Time::elapsed_secs` scale).
    pub expires_at: Option<f32>,
}

/// Marker: the bait is still in flight (has a [`BallBody`] to step).
#[derive(Component)]
pub struct BaitProjectile;

/// Creature currently walking towards a bait, with the wander home to restore.
#[derive(Component)]
pub struct AttractedTo {
    pub bait: Entity,
    pub original: WanderParams,
}

/// Spawn one bait projectile (logical components + a coloured sphere).
#[allow(clippy::too_many_arguments)]
pub fn spawn_bait(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    row: &BaitRow,
    position: Vec3,
    velocity: Vec3,
    gravity: f32,
) -> Entity {
    commands
        .spawn((
            BaitObject {
                item_id: row.item_id,
                expires_at: None,
            },
            BaitProjectile,
            BallBody(BallSimulation::new(position, velocity, gravity)),
            Mesh3d(meshes.add(Mesh::from(Sphere::new(0.1)))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: row.color_primary,
                perceptual_roughness: 0.6,
                ..default()
            })),
            Transform::from_translation(position),
        ))
        .id()
}

/// Step flying baits; when one touches the ground it arms (starts attracting) and stops.
fn step_bait_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    terrain: Option<Res<super::zone::TerrainResource>>,
    mut q: Query<(Entity, &mut Transform, &mut BallBody, &mut BaitObject), With<BaitProjectile>>,
) {
    let dt = time.delta_secs();
    for (entity, mut xf, mut body, mut bait) in &mut q {
        let prev = body.0.position;
        let ground_y = terrain
            .as_ref()
            .and_then(|t| t.area.sample_world_height(prev.x, prev.z))
            .unwrap_or(0.0);
        // `prev` as the player position: baits are never discarded for "too far".
        let out = body.0.tick_events(dt, ground_y, prev);
        xf.translation = body.0.position;
        if out.despawn.is_some() {
            commands.entity(entity).despawn();
            continue;
        }
        if !body.0.in_flight() {
            bait.expires_at = Some(time.elapsed_secs() + BAIT_LIFETIME_S);
            commands
                .entity(entity)
                .remove::<BallBody>()
                .remove::<BaitProjectile>();
        }
    }
}

/// Despawn armed baits past their lifetime.
fn bait_tick(time: Res<Time>, mut commands: Commands, q: Query<(Entity, &BaitObject)>) {
    let now = time.elapsed_secs();
    for (entity, bait) in &q {
        if bait.expires_at.is_some_and(|t| now >= t) {
            commands.entity(entity).despawn();
        }
    }
}

/// Re-home wild creatures near an armed bait (reuses `wild_wander`); restore when it is gone.
fn bait_attract_wild(
    mut commands: Commands,
    baits: Query<(Entity, &Transform, &BaitObject)>,
    mut wild: Query<(
        Entity,
        &mut WanderHome,
        &WanderState,
        &WildCreature,
        Option<&AttractedTo>,
    )>,
) {
    for (entity, mut home, state, wild, attracted) in &mut wild {
        if wild.in_battle || wild.defeated {
            continue;
        }
        let mut nearest: Option<(Entity, (f32, f32))> = None;
        let mut nearest_dist = f32::INFINITY;
        for (bait_entity, bait_xf, bait) in &baits {
            if bait.expires_at.is_none() {
                continue; // still in flight
            }
            let d = ((bait_xf.translation.x - state.pos.0).powi(2)
                + (bait_xf.translation.z - state.pos.1).powi(2))
            .sqrt();
            if d <= BAIT_ATTRACT_RADIUS && d < nearest_dist {
                nearest_dist = d;
                nearest = Some((bait_entity, (bait_xf.translation.x, bait_xf.translation.z)));
            }
        }
        match nearest {
            Some((bait_entity, pos)) => {
                // Capture the pre-attraction params before overwriting the home.
                let original = attracted.map_or(home.params, |a| a.original);
                home.params.home = pos;
                home.params.radius = BAIT_TARGET_RADIUS;
                if !attracted.is_some_and(|a| a.bait == bait_entity) {
                    commands.entity(entity).try_insert(AttractedTo {
                        bait: bait_entity,
                        original,
                    });
                }
            }
            None => {
                if let Some(a) = attracted {
                    home.params.home = a.original.home;
                    home.params.radius = a.original.radius;
                    commands.entity(entity).try_remove::<AttractedTo>();
                }
            }
        }
    }
}

/// Registers the bait projectile, attraction and expiry systems (overworld, unpaused).
pub struct CaptureWorldPlugin;

impl Plugin for CaptureWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (step_bait_projectiles, bait_tick, bait_attract_wild)
                .chain()
                .run_if(in_state(GameState::World))
                .run_if(not_paused),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle_engine::two_shake_capture;
    use crate::game::catch_rate::CAPTURE_CATCH_RATE_PLACEHOLDER;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    const BAITS_TSV: &str = "\
item_id:string*\tbait_type:string\ttaste:string\tcategory:string\tcolor_hex_primary:string\tcolor_hex_secondary:string\tspecial_effect:string
1724\textract\tother\tyosedama\tFF573B\t763D17\tnone
164\tberries\tother\tnuts\tFD6122\t612307\tcapture_rate_up
149\tberries\tother\tnuts\t7B2B01\t590B06\tnone
";

    #[test]
    fn draws_match_two_shake_capture_and_advance_the_same_rng() {
        for probability in [0.0, 0.1, 0.5, 0.78, 0.99, 1.0] {
            for seed in [1u32, 0x5ca1_e001, 42, 0xdead_beef, 7] {
                let mut engine_rng = XorShift32(seed);
                let engine_caught = two_shake_capture(probability, &mut engine_rng);
                let mut world_rng = XorShift32(seed);
                let (checks, caught) = draw_capture_checks(probability, &mut world_rng);
                assert_eq!(caught, engine_caught, "p={probability} seed={seed}");
                assert_eq!(engine_rng.0, world_rng.0, "rng state diverged");
                if caught {
                    assert_eq!(checks, 2);
                } else {
                    assert!(checks < 2);
                }
            }
        }
    }

    #[test]
    fn shake_count_is_monotonic_in_the_checks() {
        assert_eq!(shake_count(true, 2), 3);
        assert_eq!(
            shake_count(false, 2),
            3,
            "caught=false can only happen with <2 checks"
        );
        assert_eq!(shake_count(false, 1), 2);
        assert_eq!(shake_count(false, 0), 1);
    }

    #[test]
    fn roll_uses_the_tables_and_consumes_two_draws() {
        let data = BattleData::load(root()).unwrap();
        let input = WorldCaptureInput {
            ball_category: "Regular",
            suitable_range: true,
            backstrike: false,
            catch_rate: 255,
            hp_ratio: 1.0,
            player_rank: 1,
            pokemon_rank: 1,
            modifiers: CaptureModifiers::default(),
        };
        let mut rng = XorShift32(0x5ca1_e001);
        let mut reference = XorShift32(0x5ca1_e001);
        let roll = roll_capture(Some(&data), input, &mut rng).unwrap();
        let p = capture_probability_for(Some(&data), &input).unwrap();
        assert!((roll.probability - p).abs() < 1e-12);
        let expected = two_shake_capture(p, &mut reference);
        assert_eq!(roll.caught, expected);
        assert_eq!(rng.0, reference.0, "two draws consumed");
        assert!(p > 0.0 && p <= 1.0);
    }

    #[test]
    fn fallback_without_data_still_rolls() {
        let input = WorldCaptureInput {
            ball_category: "Feather",
            suitable_range: true,
            backstrike: false,
            catch_rate: CAPTURE_CATCH_RATE_PLACEHOLDER,
            hp_ratio: 1.0,
            player_rank: 1,
            pokemon_rank: 1,
            modifiers: CaptureModifiers::default(),
        };
        let mut rng = XorShift32(9);
        let roll = roll_capture(None, input, &mut rng).unwrap();
        assert!(roll.probability > 0.0);
        let long = fallback_ball_multiplier("Feather", true);
        let short = fallback_ball_multiplier("Feather", false);
        assert!(long > short);
    }

    #[test]
    fn fallback_multipliers_match_the_decoded_sheet() {
        let data = BattleData::load(root()).unwrap();
        for category in [
            "Regular", "Great", "Ultra", "Feather", "Wing", "Jet", "Heavy", "Leaden", "Gigaton",
        ] {
            for suitable in [false, true] {
                assert_eq!(
                    fallback_ball_multiplier(category, suitable),
                    data.capture_ball_multiplier(category, suitable).unwrap(),
                    "{category} suitable={suitable}"
                );
            }
        }
    }

    #[test]
    fn suitable_range_and_backstrike_rules() {
        assert!(suitable_range(BallKind::PokeBall, 3.0, false));
        assert!(!suitable_range(BallKind::JetBall, 5.0, false));
        assert!(suitable_range(BallKind::JetBall, 15.0, false));
        assert!(!suitable_range(BallKind::HeavyBall, 15.0, false));
        assert!(suitable_range(BallKind::HeavyBall, 15.0, true));
        // Facing +Z, thrown towards +Z: hit from behind.
        let forward = Vec3::new(0.0, 0.0, 1.0);
        assert!(is_backstrike(forward, Vec3::new(0.0, 1.0, 5.0)));
        assert!(!is_backstrike(forward, Vec3::new(0.0, 1.0, -5.0)));
        assert!(
            !is_backstrike(forward, Vec3::new(5.0, 1.0, 0.0)),
            "perpendicular"
        );
    }

    #[test]
    fn bait_table_parses_rows_and_colours() {
        let t = BaitTable::from_tsv(BAITS_TSV).unwrap();
        assert_eq!(t.len(), 3);
        assert_eq!(t.ids(), &[1724, 164, 149]);
        let bean = t.get(1724).unwrap();
        assert_eq!(bean.bait_type, "extract");
        assert_eq!(bean.special_effect, "none");
        assert_eq!(bean.color_primary, Color::srgb_u8(0xFF, 0x57, 0x3B));
        assert_eq!(parse_hex_color("zzzzzz"), None);
        assert_eq!(parse_hex_color("FFF"), None);
        let bag: BTreeMap<u16, u16> = [(149, 3), (164, 0), (999, 1)].into_iter().collect();
        assert_eq!(t.present_in(&bag), vec![149], "only stocked bait ids");
    }

    #[test]
    fn bait_factor_comes_from_the_context_table() {
        let data = BattleData::load(root()).unwrap();
        assert!((bait_multiplier(true, Some(&data)) - 1.3).abs() < 1e-9);
        assert!((bait_multiplier(false, Some(&data)) - 1.0).abs() < 1e-9);
        assert!((bait_multiplier(true, None) - 1.3).abs() < 1e-9, "fallback");
        let pos = (1.0, 1.0);
        assert!(bait_in_range(pos, [(2.5, 1.0)], BAIT_FEED_RADIUS));
        assert!(!bait_in_range(pos, [(5.0, 1.0)], BAIT_FEED_RADIUS));
        assert!(!bait_in_range(pos, [], BAIT_FEED_RADIUS));
    }

    #[test]
    fn caught_member_builds_and_stores_into_party_then_box() {
        let data = BattleData::load(root()).unwrap();
        let member = build_caught_member(&data, "bidoof", 7).unwrap();
        assert_eq!(member.species, 399);
        assert_eq!(member.level, 7);
        assert!(!member.moves.is_empty(), "tackle fallback");
        let mut p = PlayerProgress::new_game();
        assert_eq!(store_caught(&mut p, member.clone()), CatchStorage::Party);
        assert!(p.caught.contains(&399));
        assert_eq!(p.caught_count, 1);
        // Fill the party: the next catch goes to the box.
        while p.party.len() < MAX_PARTY {
            p.party.push(member.clone());
        }
        assert_eq!(store_caught(&mut p, member.clone()), CatchStorage::Box);
        assert_eq!(p.caught_count, 2);
    }

    /// Headless: a creature near an armed bait re-homes, and gets its home back when the bait
    /// expires.
    #[test]
    fn bait_attraction_rehomes_and_restores() {
        use crate::game::wild::WildCreature;
        use bevy::state::app::StatesPlugin;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.add_plugins(CaptureWorldPlugin);
        let original = WanderParams::new((100.0, 100.0), 8.0, 1.0);
        let creature = app
            .world_mut()
            .spawn((
                Transform::from_xyz(100.0, 0.0, 100.0),
                WanderState::new(7, (100.0, 100.0)),
                WanderHome {
                    params: original,
                    y_offset: None,
                },
                WildCreature {
                    species_name: "bidoof".into(),
                    national_dex: 399,
                    level: 5,
                    spawn_pos: Vec3::new(100.0, 0.0, 100.0),
                    aggro_range: 2.8,
                    in_battle: false,
                    defeated: false,
                    spawner_id: "s1".into(),
                },
            ))
            .id();
        // Armed bait 5 m away.
        let bait = app
            .world_mut()
            .spawn((
                BaitObject {
                    item_id: 149,
                    expires_at: Some(10.0),
                },
                Transform::from_xyz(105.0, 0.2, 100.0),
            ))
            .id();
        app.update();
        let params = app.world().get::<WanderHome>(creature).unwrap().params;
        assert_eq!(params.home, (105.0, 100.0), "re-homed onto the bait");
        assert!((params.radius - BAIT_TARGET_RADIUS).abs() < 1e-6);
        assert!(app.world().get::<AttractedTo>(creature).is_some());
        // Bait expires: attraction is released and the original home restored.
        app.world_mut()
            .get_mut::<BaitObject>(bait)
            .unwrap()
            .expires_at = Some(0.0);
        app.update();
        app.update();
        let params = app.world().get::<WanderHome>(creature).unwrap().params;
        assert_eq!(params.home, (100.0, 100.0));
        assert!((params.radius - original.radius).abs() < 1e-6);
        assert!(app.world().get::<AttractedTo>(creature).is_none());
    }
}
