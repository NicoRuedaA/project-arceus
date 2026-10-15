//! Wild creature spawner and definitions for overworld exploration.
//!
//! Loads real field spawn tables from `data/field_spawn_table.tsv`
//! and manages deterministic, distance-based streaming around the player.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use bevy::prelude::*;

/// Component attached to spawned overworld wild creatures.
#[derive(Component, Debug, Clone)]
pub struct WildCreature {
    pub species_name: String,
    pub national_dex: u16,
    pub level: u8,
    pub spawn_pos: Vec3,
    pub aggro_range: f32,
    pub in_battle: bool,
    pub defeated: bool,
    /// Spawner ID from `field_spawn_table.tsv` that created this entity.
    pub spawner_id: String,
}

/// How a battle with a wild creature ended, from the wild creature's side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WildFate {
    /// The wild creature fainted: it leaves the world.
    Defeated,
    /// The wild creature was caught: it leaves the world.
    Captured,
    /// The player ran away (or lost): the wild creature stays where it was.
    Fled,
}

/// Classifies the end of a battle from the wild combatant's final state.
pub fn wild_fate(wild_hp: u32, wild_caught: bool) -> WildFate {
    if wild_caught {
        WildFate::Captured
    } else if wild_hp == 0 {
        WildFate::Defeated
    } else {
        WildFate::Fled
    }
}

impl WildFate {
    /// True when the creature must be removed from the world.
    pub fn removes_creature(self) -> bool {
        !matches!(self, WildFate::Fled)
    }
}

/// How long (in game seconds) a defeated / captured spawner stays empty before it may
/// spawn a new creature again. Leaving and re-entering the zone also clears the set
/// (see [`ConsumedSpawners::clear`]).
pub const RESPAWN_AFTER_S: f64 = 600.0;

/// Seconds a creature that the player fled from will not start another encounter.
pub const FLEE_COOLDOWN_S: f32 = 6.0;

/// Spawner ids whose creature was defeated or captured, with the time they may respawn.
#[derive(Resource, Debug, Default, Clone)]
pub struct ConsumedSpawners {
    respawn_at: HashMap<String, f64>,
}

impl ConsumedSpawners {
    /// Marks `spawner_id` as consumed at game time `now_s`.
    pub fn consume(&mut self, spawner_id: &str, now_s: f64) {
        self.respawn_at
            .insert(spawner_id.to_string(), now_s + RESPAWN_AFTER_S);
    }

    /// True while the spawner must stay empty.
    pub fn is_consumed(&self, spawner_id: &str, now_s: f64) -> bool {
        self.respawn_at.get(spawner_id).is_some_and(|t| now_s < *t)
    }

    /// Forgets spawners whose waiting time is over.
    pub fn purge_expired(&mut self, now_s: f64) {
        self.respawn_at.retain(|_, t| now_s < *t);
    }

    /// Forgets everything (zone reload).
    pub fn clear(&mut self) {
        self.respawn_at.clear();
    }

    pub fn len(&self) -> usize {
        self.respawn_at.len()
    }

    pub fn is_empty(&self) -> bool {
        self.respawn_at.is_empty()
    }
}

/// Encounters are suppressed while this component is present (fled battle).
#[derive(Component, Debug, Clone, Copy)]
pub struct FleeCooldown(pub f32);

/// One cooldown step: remaining seconds after `dt`, or `None` when it has expired.
pub fn flee_cooldown_step(remaining: f32, dt: f32) -> Option<f32> {
    let left = remaining - dt;
    (left > 0.0).then_some(left)
}

fn tick_flee_cooldown(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut FleeCooldown)>,
) {
    for (e, mut c) in &mut q {
        match flee_cooldown_step(c.0, time.delta_secs()) {
            Some(left) => c.0 = left,
            None => {
                commands.entity(e).try_remove::<FleeCooldown>();
            }
        }
    }
}

fn purge_consumed(time: Res<Time>, mut consumed: ResMut<ConsumedSpawners>) {
    if !consumed.is_empty() {
        consumed.purge_expired(time.elapsed_secs_f64());
    }
}

/// Registers the bookkeeping of defeated / captured / fled wild creatures.
pub struct WildLifecyclePlugin;

impl Plugin for WildLifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EncounterSafetyPlugin)
            .init_resource::<ConsumedSpawners>()
            .add_systems(Update, (tick_flee_cooldown, purge_consumed));
    }
}

// ---------------------------------------------------------------------------------------------
// Encounter safety (SUPUESTO values: the native encounter radii and any arrival grace time are
// not decoded; these numbers are playtest-driven guesses, not game data).
// ---------------------------------------------------------------------------------------------

/// SUPUESTO: seconds after spawning, a zone change or a load during which no encounter can start
/// and no creature chases the player.
pub const ENCOUNTER_GRACE_S: f32 = 5.0;
/// SUPUESTO: wild creatures are not spawned within this many metres of the arrival point.
pub const SPAWN_EXCLUSION_RADIUS_M: f32 = 15.0;
/// SUPUESTO: an automatic encounter (walking into an aggressive creature) needs this distance.
pub const AUTO_ENCOUNTER_DISTANCE_M: f32 = 1.5;
/// SUPUESTO: the manual encounter (E / Enter) reach.
pub const MANUAL_ENCOUNTER_DISTANCE_M: f32 = 4.0;
/// A single-frame player displacement above this many metres is a teleport (zone change, load,
/// "back to safe spawn"): it counts as a new arrival.
pub const ARRIVAL_JUMP_M: f32 = 30.0;

/// Grace period and arrival point of the player (see the constants above).
#[derive(Resource, Debug, Clone, Default)]
pub struct EncounterSafety {
    /// Seconds of grace left (0 = encounters allowed).
    pub grace_remaining_s: f32,
    /// Arrival point (x, z) around which no creature is spawned.
    pub anchor: Option<Vec2>,
    last_player: Option<Vec2>,
}

impl EncounterSafety {
    /// True while encounters and aggressive chases are suppressed.
    pub fn grace_active(&self) -> bool {
        self.grace_remaining_s > 0.0
    }

    /// Starts a new arrival at `pos`: grace period and spawn exclusion restart.
    pub fn begin_arrival(&mut self, pos: Vec2) {
        self.grace_remaining_s = ENCOUNTER_GRACE_S;
        self.anchor = Some(pos);
        self.last_player = Some(pos);
    }

    /// Ends the grace period at `pos` without treating the move as an arrival (test tooling:
    /// the automation `start_battle nearest` command walks the player up to a creature).
    pub fn clear_grace_at(&mut self, pos: Vec2) {
        self.grace_remaining_s = 0.0;
        self.last_player = Some(pos);
    }

    /// Forgets the last position so the next [`EncounterSafety::tick`] is treated as an arrival
    /// (called when a zone is replaced or a save is loaded).
    pub fn request_arrival(&mut self) {
        self.last_player = None;
    }

    /// One frame: detects the first frame and teleports as arrivals, then counts the grace down.
    pub fn tick(&mut self, dt: f32, player: Vec2) {
        match self.last_player {
            None => self.begin_arrival(player),
            Some(prev) if prev.distance(player) > ARRIVAL_JUMP_M => self.begin_arrival(player),
            _ => {}
        }
        self.last_player = Some(player);
        self.grace_remaining_s = (self.grace_remaining_s - dt.max(0.0)).max(0.0);
    }

    /// True when a spawner at `spawn_xz` is too close to the arrival point.
    pub fn spawn_excluded(&self, spawn_xz: Vec2) -> bool {
        self.anchor
            .is_some_and(|a| a.distance(spawn_xz) < SPAWN_EXCLUSION_RADIUS_M)
    }
}

/// Whether the viewer may start a battle by itself. `aggressive_engaged` is true when the
/// creature is in an aggressive Chase/Attack reaction.
pub fn auto_encounter_allowed(dist: f32, aggressive_engaged: bool, grace_active: bool) -> bool {
    !grace_active && aggressive_engaged && dist <= AUTO_ENCOUNTER_DISTANCE_M
}

/// Whether the E / Enter key starts a battle (never during the grace period).
pub fn manual_encounter_allowed(dist: f32, interact_pressed: bool, grace_active: bool) -> bool {
    !grace_active && interact_pressed && dist <= MANUAL_ENCOUNTER_DISTANCE_M
}

fn encounter_safety_system(
    time: Res<Time>,
    mut safety: ResMut<EncounterSafety>,
    player: Query<&Transform, With<crate::game::PlayerAvatar>>,
) {
    if let Ok(t) = player.single() {
        safety.tick(time.delta_secs(), Vec2::new(t.translation.x, t.translation.z));
    }
}

/// Registers [`EncounterSafety`] and its per-frame tick (runs before every `Update` system).
pub struct EncounterSafetyPlugin;

impl Plugin for EncounterSafetyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EncounterSafety>()
            .add_systems(PreUpdate, encounter_safety_system);
    }
}

/// Decoded facts of one encounter slot (`data/field_spawn_conditions.tsv`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SlotFacts {
    pub form: u16,
    pub alpha: bool,
    pub unique_alpha: bool,
    pub weight: u16,
    /// The slot's decoded `condition` string (`flag_condition?:equal:FSYS_NS_0N_CLEARED` on the
    /// random-alpha slots).
    pub condition: String,
}

impl SlotFacts {
    /// True when the slot is gated by the area's "Noble is calmed" flag (`FSYS_NS_0N_CLEARED`).
    /// Every random-alpha slot of an area carries it (1,655 of 1,870 alpha slots; VERIFIED read
    /// of `field_spawn_conditions.tsv`).
    pub fn gated_by_area_clear(&self) -> bool {
        self.condition.contains("FSYS_NS_0") && self.condition.contains("_CLEARED")
    }
}

/// The random-alpha slots of an area carry the decoded gate `FSYS_NS_0N_CLEARED` ("the area's
/// Noble is calmed", LIKELY). The port has no persisted Noble-clear state yet; while it does not,
/// this constant decides whether the gate is enforced (`false` = the port spawns random alphas
/// from the start, an ASSUMED simplification; flip it to `true` when the flag lands).
pub const ALPHA_REQUIRE_AREA_CLEARED_FLAG: bool = false;

/// Field scale of an alpha creature (`LL_field`; raw value verified in `pokemon_variants.tsv`).
pub const ALPHA_FIELD_SCALE: f32 = 2.5;

/// Field scale of a non-alpha creature from a uniform roll.
/// SUPUESTO: the data holds the category scales (S 0.85, M 1.0, L 1.8) but no
/// draw probabilities; a third of creatures at 1.8x looked gigantic, so the
/// port keeps almost all at M and makes S/L uncommon.
pub fn normal_size_scale(roll: u64) -> f32 {
    match roll % 100 {
        0..=9 => 0.85,  // S_field
        10..=94 => 1.0, // M_field
        _ => 1.8,       // L_field
    }
}

/// A possible species entry within an encounter group.
#[derive(Debug, Clone)]
pub struct SpawnSpeciesEntry {
    pub species_id: u16,
    pub species_name: String,
    /// Encounter form from `field_spawn_conditions.tsv`; zero is the explicit
    /// fallback when the companion evidence row is absent.
    pub form: u16,
    /// Alpha flags are decoded slot facts, not probabilities.
    pub alpha: bool,
    pub unique_alpha: bool,
    pub min_level: u8,
    pub max_level: u8,
    pub weight: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpawnVariant {
    pub form: u16,
    pub alpha: bool,
    pub size_scale: f32,
}

/// Raw spawner definition parsed from `data/field_spawn_table.tsv`.
#[derive(Debug, Clone)]
pub struct SpawnerDefinition {
    pub spawner_id: String,
    pub area: String,
    pub pos: Vec3,
    pub radius: f32,
    pub entries: Vec<SpawnSpeciesEntry>,
}

/// Resource holding all parsed spawner definitions for the active area,
/// as well as the active spawned entities mapped by spawner ID.
#[derive(Resource)]
pub struct ActiveAreaSpawners {
    pub spawners: Vec<SpawnerDefinition>,
    pub spawned_entities: HashMap<String, Entity>,
    pub streaming_radius: f32,
    pub seed: u64,
}

impl Default for ActiveAreaSpawners {
    fn default() -> Self {
        Self {
            spawners: Vec::new(),
            spawned_entities: HashMap::new(),
            streaming_radius: 120.0,
            seed: 0x5e1f_5eed_1234_5678,
        }
    }
}

/// Deterministic 64-bit LCG / SplitMix64 pseudo-random number generator.
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// Computes a deterministic 64-bit hash from a string identifier.
pub fn hash_str(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Loads spawner definitions for a given area index from `data/field_spawn_table.tsv`.
pub fn load_spawners_for_area(
    table_path: impl AsRef<Path>,
    area_idx: u8,
) -> Result<Vec<SpawnerDefinition>, String> {
    let path = table_path.as_ref();
    let file = File::open(path)
        .map_err(|e| format!("failed to open spawn table {}: {e}", path.display()))?;
    let reader = BufReader::new(file);

    let target_area = format!("ha_area{area_idx:02}");
    let condition_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("field_spawn_conditions.tsv");
    // A species can own several slots in one encounter group: a heavy normal slot (weight 100)
    // and one or two light alpha slots (weight 1-2, `alpha` bit) and, rarely, a fixed alpha
    // (weight 100, `unique_alpha` bit). Keep them ALL: the game rolls the slot by weight, so the
    // alpha chance of a table is `alpha weight / total weight` (~1 %, decoded 0.97 %..3.7 %).
    // The previous merge kept one slot per species and silently dropped the alpha slots.
    let mut conditions = HashMap::<(String, String, u16), Vec<SlotFacts>>::new();
    if let Ok(file) = File::open(&condition_path) {
        let reader = BufReader::new(file);
        for line_res in reader.lines().skip(9) {
            let Ok(line) = line_res else { continue };
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 10 || !fields[0].starts_with(&target_area) {
                continue;
            }
            let Ok(species_id) = fields[4].parse::<u16>() else {
                continue;
            };
            let incoming = SlotFacts {
                form: fields[6].parse::<u16>().unwrap_or(0),
                alpha: fields[7] == "1",
                unique_alpha: fields[8] == "1",
                weight: fields.get(9).and_then(|w| w.parse::<u16>().ok()).unwrap_or(0),
                condition: fields.last().copied().unwrap_or("").to_string(),
            };
            if ALPHA_REQUIRE_AREA_CLEARED_FLAG && incoming.alpha && incoming.gated_by_area_clear()
            {
                continue;
            }
            let slots = conditions
                .entry((fields[1].to_owned(), fields[2].to_owned(), species_id))
                .or_default();
            // The sheet repeats a (species, flag) pair when a spawner owns the group twice;
            // keep the heaviest row of each flag.
            if let Some(prev) = slots
                .iter_mut()
                .find(|s| s.alpha == incoming.alpha && s.unique_alpha == incoming.unique_alpha)
            {
                if incoming.weight > prev.weight {
                    *prev = incoming;
                }
            } else {
                slots.push(incoming);
            }
        }
    }
    let mut spawner_map: HashMap<String, SpawnerDefinition> = HashMap::new();

    for line_res in reader.lines() {
        let line = line_res.map_err(|e| format!("read error: {e}"))?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = trimmed.split('\t').collect();
        if parts.len() < 16 {
            continue;
        }

        let area_str = parts[0];
        // Match exact area ha_areaNN or subareas ha_areaNN_sMM
        if !area_str.starts_with(&target_area) {
            continue;
        }

        let spawner_id = parts[1].to_string();
        let encount_key = parts[8];
        let x: f32 = match parts[2].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let y: f32 = match parts[3].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let z: f32 = match parts[4].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let radius: f32 = parts[5].parse().unwrap_or(8.0);

        let species_id: u16 = match parts[9].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let species_name = parts[10].to_lowercase();
        let min_lvl: u8 = parts[12].parse().unwrap_or(5);
        let max_lvl: u8 = parts[13].parse().unwrap_or(min_lvl);
        let weight: u16 = parts[15].parse().unwrap_or(100).max(1);
        let facts = conditions.get(&(spawner_id.clone(), encount_key.to_owned(), species_id));
        let make = |form: u16, alpha: bool, unique_alpha: bool, weight: u16| SpawnSpeciesEntry {
            species_id,
            species_name: species_name.clone(),
            form,
            alpha,
            unique_alpha,
            min_level: min_lvl,
            max_level: max_lvl,
            weight: weight.max(1),
        };
        let mut entries = Vec::new();
        match facts {
            Some(slots) => {
                let normal = slots.iter().find(|s| !s.alpha);
                let alpha = slots.iter().find(|s| s.alpha);
                // Normal slot: its decoded weight, or the spawn-table weight when the condition
                // sheet has only the alpha slot of this species (the fixed-alpha spawners).
                if let Some(s) = normal {
                    entries.push(make(s.form, false, false, s.weight));
                } else if alpha.is_none() {
                    entries.push(make(0, false, false, weight));
                }
                if let Some(s) = alpha {
                    entries.push(make(s.form, true, s.unique_alpha, s.weight));
                }
            }
            None => entries.push(make(0, false, false, weight)),
        }

        let spawner = spawner_map
            .entry(spawner_id.clone())
            .or_insert_with(|| SpawnerDefinition {
                spawner_id,
                area: area_str.to_string(),
                pos: Vec3::new(x, y, z),
                radius,
                entries: Vec::new(),
            });

        // The port rolls one slot out of every entry of the spawner, so a (species, form, alpha)
        // owned twice (two encounter tables) must not double its weight: keep the heaviest row.
        for entry in entries {
            match spawner.entries.iter_mut().find(|p| {
                p.species_id == entry.species_id && p.form == entry.form && p.alpha == entry.alpha
            }) {
                Some(prev) => {
                    if entry.weight > prev.weight {
                        *prev = entry;
                    }
                }
                None => spawner.entries.push(entry),
            }
        }
    }

    let mut result: Vec<SpawnerDefinition> = spawner_map.into_values().collect();
    // Sort deterministically by spawner_id
    result.sort_by(|a, b| a.spawner_id.cmp(&b.spawner_id));
    Ok(result)
}

/// Chooses species and level deterministically for a spawner using seed + spawner_id.
pub fn roll_spawner(spawner: &SpawnerDefinition, base_seed: u64) -> Option<(u16, String, u8)> {
    roll_spawner_variant(spawner, base_seed).map(|(dex, name, level, _)| (dex, name, level))
}

/// Selects one encounter and a deterministic port-side appearance variant.
/// The alpha draw is now data-driven: the slot pool of a spawner keeps the decoded normal and
/// alpha rows with their own weights, so the alpha chance of a roll is the decoded
/// `alpha weight / total weight` (~1 %, see `SlotFacts`). Normal size categories are a stable
/// hash choice over the decoded S/M/L scales (SUPUESTO, see `normal_size_scale`).
pub fn roll_spawner_variant(
    spawner: &SpawnerDefinition,
    base_seed: u64,
) -> Option<(u16, String, u8, SpawnVariant)> {
    if spawner.entries.is_empty() {
        return None;
    }

    let spawner_seed = base_seed ^ hash_str(&spawner.spawner_id);
    let mut rng_state = spawner_seed;

    let total_weight: u32 = spawner.entries.iter().map(|e| e.weight as u32).sum();
    let roll_weight = if total_weight > 0 {
        (splitmix64(&mut rng_state) % (total_weight as u64)) as u32
    } else {
        0
    };

    let mut accumulated = 0;
    let mut chosen_entry = &spawner.entries[0];
    for entry in &spawner.entries {
        accumulated += entry.weight as u32;
        if roll_weight < accumulated {
            chosen_entry = entry;
            break;
        }
    }

    let min_lvl = chosen_entry.min_level;
    let max_lvl = chosen_entry.max_level;
    let lvl = if max_lvl > min_lvl {
        let span = (max_lvl - min_lvl + 1) as u64;
        min_lvl + (splitmix64(&mut rng_state) % span) as u8
    } else {
        min_lvl
    };

    let alpha = chosen_entry.alpha || chosen_entry.unique_alpha;
    let size_scale = if alpha {
        ALPHA_FIELD_SCALE
    } else {
        normal_size_scale(splitmix64(&mut rng_state))
    };
    Some((
        chosen_entry.species_id,
        chosen_entry.species_name.clone(),
        lvl,
        SpawnVariant {
            form: chosen_entry.form,
            alpha,
            size_scale,
        },
    ))
}

/// Backward compatibility helper for legacy calls.
pub fn get_area00_wild_spawns() -> Vec<WildCreature> {
    get_wild_spawns_for_area(0, Vec2::new(448.0, 652.0))
}

/// Helper that evaluates spawners for an area near player_spawn, falling back to real table.
pub fn get_wild_spawns_for_area(area: u8, player_spawn: Vec2) -> Vec<WildCreature> {
    let table_path = "data/field_spawn_table.tsv";
    if let Ok(spawners) = load_spawners_for_area(table_path, area) {
        if !spawners.is_empty() {
            let mut result = Vec::new();
            let base_seed = 0x5e1f_5eed_1234_5678;
            for spawner in &spawners {
                let dist_sq = (spawner.pos.x - player_spawn.x).powi(2)
                    + (spawner.pos.z - player_spawn.y).powi(2);
                if dist_sq <= 120.0 * 120.0 {
                    if let Some((dex, name, lvl)) = roll_spawner(spawner, base_seed) {
                        result.push(WildCreature {
                            species_name: name,
                            national_dex: dex,
                            level: lvl,
                            spawn_pos: spawner.pos,
                            aggro_range: 2.8,
                            in_battle: false,
                            defeated: false,
                            spawner_id: spawner.spawner_id.clone(),
                        });
                    }
                }
            }
            if !result.is_empty() {
                return result;
            }
        }
    }

    // Default fallback if table missing
    vec![WildCreature {
        species_name: "pikachu".into(),
        national_dex: 25,
        level: 5,
        spawn_pos: Vec3::new(player_spawn.x + 6.0, 0.0, player_spawn.y - 5.0),
        aggro_range: 2.8,
        in_battle: false,
        defeated: false,
        spawner_id: "fallback_pika".into(),
    }]
}

#[cfg(test)]
mod alpha_slot_tests {
    use super::*;

    fn spawn_table_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/field_spawn_table.tsv")
    }

    /// The random-alpha slots are no longer dropped: a spawner with a decoded alpha slot keeps
    /// both rows (normal and alpha) with their own weights (Brief 17 remainder).
    #[test]
    fn alpha_slots_survive_the_load() {
        let spawners = match load_spawners_for_area(&spawn_table_path(), 1) {
            Ok(s) if !s.is_empty() => s,
            _ => {
                eprintln!("[skip] spawn sheets not found");
                return;
            }
        };
        let with_alpha = spawners
            .iter()
            .filter(|s| s.entries.iter().any(|e| e.alpha))
            .count();
        let with_normal_and_alpha = spawners
            .iter()
            .filter(|s| {
                s.entries.iter().filter(|e| e.alpha).any(|a| {
                    s.entries
                        .iter()
                        .any(|n| n.species_id == a.species_id && !n.alpha)
                })
            })
            .count();
        assert!(
            with_alpha > 100,
            "area 01 must keep its hundreds of decoded alpha slots, got {with_alpha}"
        );
        assert!(
            with_normal_and_alpha > 100,
            "normal and alpha rows of the same species must coexist, got {with_normal_and_alpha}"
        );
        // The alpha rows carry the decoded light weight (1-2) next to the 100-weight normals.
        let alpha_weights: Vec<u16> = spawners
            .iter()
            .flat_map(|s| s.entries.iter().filter(|e| e.alpha).map(|e| e.weight))
            .collect();
        assert!(alpha_weights.iter().filter(|w| **w <= 2).count() > 100);
        assert!(alpha_weights.iter().any(|w| *w == 100), "fixed alphas stay 100");
    }

    /// The decoded alpha share of a table (`alpha weight / total weight`) sits in the range the
    /// external player notes describe (0.2 %-2.0 %; the decoded data says 0.97 %-1.96 % for the
    /// light slots), and never above a few percent.
    #[test]
    fn alpha_share_of_the_decoded_tables_is_about_one_percent() {
        let spawners = match load_spawners_for_area(&spawn_table_path(), 1) {
            Ok(s) if !s.is_empty() => s,
            _ => {
                eprintln!("[skip] spawn sheets not found");
                return;
            }
        };
        let mut shares = Vec::new();
        for s in &spawners {
            // Fixed-alpha spawners (decoded `unique_alpha` bit) and spawners whose every slot
            // is an alpha are 100 % alpha by design: they carry no normal slot to share with.
            if s.entries.iter().any(|e| e.unique_alpha) || !s.entries.iter().any(|e| !e.alpha) {
                continue;
            }
            let total: u32 = s.entries.iter().map(|e| u32::from(e.weight)).sum();
            let alpha: u32 = s
                .entries
                .iter()
                .filter(|e| e.alpha && !e.unique_alpha)
                .map(|e| u32::from(e.weight))
                .sum();
            if alpha > 0 && total > 0 {
                shares.push(alpha as f32 / total as f32);
            }
        }
        assert!(shares.len() > 100, "many spawners carry a random alpha slot");
        let min = shares.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = shares.iter().cloned().fold(0.0_f32, f32::max);
        assert!(min >= 0.008, "alpha share below the decoded minimum: {min}");
        assert!(max <= 0.05, "alpha share above a few percent: {max}");
    }

    /// With the fixed seed, some spawner eventually rolls its alpha slot, and the roll stays
    /// deterministic (same seed and spawner answer the same every time).
    #[test]
    fn alpha_rolls_are_deterministic_and_do_fire() {
        let spawners = match load_spawners_for_area(&spawn_table_path(), 1) {
            Ok(s) if !s.is_empty() => s,
            _ => {
                eprintln!("[skip] spawn sheets not found");
                return;
            }
        };
        let mut fired = 0;
        for s in &spawners {
            if !s.entries.iter().any(|e| e.alpha) {
                continue;
            }
            for seed in 0..2_000u64 {
                let a = roll_spawner_variant(s, seed).map(|(_, _, _, v)| v.alpha);
                let b = roll_spawner_variant(s, seed).map(|(_, _, _, v)| v.alpha);
                assert_eq!(a, b, "determinism");
                if a == Some(true) {
                    fired += 1;
                }
            }
        }
        assert!(fired > 0, "some spawner must roll an alpha over 2,000 seeds");
    }

    /// A real fixed alpha (decoded `unique_alpha` bit) always comes out as an alpha.
    #[test]
    fn fixed_alphas_are_always_alpha() {
        let spawners = match load_spawners_for_area(&spawn_table_path(), 1) {
            Ok(s) if !s.is_empty() => s,
            _ => {
                eprintln!("[skip] spawn sheets not found");
                return;
            }
        };
        let fixed: Vec<_> = spawners
            .iter()
            .filter(|s| s.entries.iter().any(|e| e.unique_alpha))
            .collect();
        assert!(!fixed.is_empty(), "area 01 has decoded fixed alphas");
        for s in fixed {
            for seed in [0u64, 1, 7, 99] {
                let v = roll_spawner_variant(s, seed).expect("entries").3;
                assert!(v.alpha, "fixed-alpha spawner {} rolled normal", s.spawner_id);
            }
        }
    }

    /// The decoded gate of the random-alpha slots is read, not lost.
    #[test]
    fn random_alpha_slots_carry_the_area_clear_gate() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/field_spawn_conditions.tsv");
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("[skip] field_spawn_conditions.tsv not found");
            return;
        };
        let mut gated_alpha = 0;
        let mut ungated_alpha = 0;
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.starts_with("area")) {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 10 || f[7] != "1" || f[8] == "1" {
                continue;
            }
            let cond = f.last().copied().unwrap_or("");
            if cond.contains("FSYS_NS_0") && cond.contains("_CLEARED") {
                gated_alpha += 1;
            } else {
                ungated_alpha += 1;
            }
        }
        assert!(gated_alpha > 3_000, "most random alpha slots are gated, got {gated_alpha}");
        // The rest carry another gate (event flags) or no readable condition.
        assert!(ungated_alpha < 400, "few are not, got {ungated_alpha}");
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[test]
    fn fate_from_final_state() {
        assert_eq!(wild_fate(0, false), WildFate::Defeated);
        assert_eq!(wild_fate(0, true), WildFate::Captured);
        assert_eq!(wild_fate(7, true), WildFate::Captured);
        assert_eq!(wild_fate(7, false), WildFate::Fled);
        assert!(WildFate::Defeated.removes_creature());
        assert!(WildFate::Captured.removes_creature());
        assert!(!WildFate::Fled.removes_creature());
    }

    #[test]
    fn consumed_spawner_blocks_until_respawn_time() {
        let mut c = ConsumedSpawners::default();
        assert!(!c.is_consumed("a", 0.0));
        c.consume("a", 100.0);
        assert!(c.is_consumed("a", 100.0));
        assert!(c.is_consumed("a", 100.0 + RESPAWN_AFTER_S - 1.0));
        assert!(!c.is_consumed("a", 100.0 + RESPAWN_AFTER_S));
        assert!(!c.is_consumed("b", 100.0), "other spawners are unaffected");
    }

    #[test]
    fn purge_and_clear_forget_spawners() {
        let mut c = ConsumedSpawners::default();
        c.consume("a", 0.0);
        c.consume("b", 500.0);
        c.purge_expired(RESPAWN_AFTER_S + 1.0);
        assert_eq!(c.len(), 1);
        assert!(c.is_consumed("b", RESPAWN_AFTER_S + 1.0));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn first_frame_and_teleports_start_the_grace_period() {
        let mut s = EncounterSafety::default();
        assert!(!s.grace_active());
        s.tick(1.0 / 60.0, Vec2::new(10.0, 10.0));
        assert!(s.grace_active(), "first frame is an arrival");
        // 5 s later the grace is over.
        for _ in 0..(ENCOUNTER_GRACE_S * 60.0) as usize + 2 {
            s.tick(1.0 / 60.0, Vec2::new(10.0, 10.0));
        }
        assert!(!s.grace_active());
        // Walking normally does not restart it.
        s.tick(1.0 / 60.0, Vec2::new(10.1, 10.0));
        assert!(!s.grace_active());
        // A teleport (zone change, load) does.
        s.tick(1.0 / 60.0, Vec2::new(500.0, 10.0));
        assert!(s.grace_active());
        assert_eq!(s.anchor, Some(Vec2::new(500.0, 10.0)));
        // An explicit request (zone replaced) does too, even for a short hop.
        for _ in 0..400 {
            s.tick(1.0 / 60.0, Vec2::new(500.0, 10.0));
        }
        assert!(!s.grace_active());
        s.request_arrival();
        s.tick(1.0 / 60.0, Vec2::new(502.0, 10.0));
        assert!(s.grace_active());
    }

    #[test]
    fn grace_blocks_both_automatic_and_manual_encounters() {
        // Auto: only an engaged aggressive creature, within 1.5 m.
        assert!(auto_encounter_allowed(1.0, true, false));
        assert!(!auto_encounter_allowed(2.77, true, false), "the old 2.8 m trigger is gone");
        assert!(!auto_encounter_allowed(1.0, false, false), "passive creatures never auto-start");
        assert!(!auto_encounter_allowed(1.0, true, true), "grace period");
        // Manual: 4 m with E / Enter.
        assert!(manual_encounter_allowed(3.9, true, false));
        assert!(!manual_encounter_allowed(4.1, true, false));
        assert!(!manual_encounter_allowed(1.0, false, false));
        assert!(!manual_encounter_allowed(1.0, true, true), "grace period");
        // The reported case: spawn point 2.77 m from a creature, very first frame.
        let mut s = EncounterSafety::default();
        s.tick(1.0 / 60.0, Vec2::ZERO);
        assert!(!auto_encounter_allowed(2.77, true, s.grace_active()));
    }

    #[test]
    fn spawn_exclusion_radius_around_the_arrival_point() {
        let mut s = EncounterSafety::default();
        assert!(!s.spawn_excluded(Vec2::new(1.0, 1.0)), "no arrival yet: nothing excluded");
        s.begin_arrival(Vec2::new(100.0, 100.0));
        assert!(s.spawn_excluded(Vec2::new(102.0, 100.0)));
        assert!(s.spawn_excluded(Vec2::new(100.0, 100.0 + SPAWN_EXCLUSION_RADIUS_M - 0.1)));
        assert!(!s.spawn_excluded(Vec2::new(100.0, 100.0 + SPAWN_EXCLUSION_RADIUS_M + 0.1)));
    }

    /// Real area-01 data: the safe start the viewer picks (496.53, 353.68, read back from the
    /// automation `state`) had a Zubat/Kricketot spawner 2.75 m away, inside the old 2.8 m
    /// trigger, so a battle began on the first frame.
    #[test]
    fn real_area01_start_has_no_spawner_within_the_exclusion_radius() {
        let table = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/field_spawn_table.tsv");
        let spawners = load_spawners_for_area(&table, 1).expect("area 01 spawn table");
        let start = Vec2::new(496.53, 353.68);
        let near = |s: &SpawnerDefinition| Vec2::new(s.pos.x, s.pos.z).distance(start);
        let closest = spawners.iter().map(near).fold(f32::INFINITY, f32::min);
        assert!(closest < 2.8, "the regression data changed: closest spawner {closest} m");
        let mut safety = EncounterSafety::default();
        safety.begin_arrival(start);
        let kept: Vec<_> = spawners
            .iter()
            .filter(|s| !safety.spawn_excluded(Vec2::new(s.pos.x, s.pos.z)))
            .collect();
        assert!(kept.len() < spawners.len(), "the near spawner must be excluded");
        assert!(kept.len() + 5 > spawners.len(), "only the few spawners near the start are dropped");
        for s in kept {
            assert!(near(s) >= SPAWN_EXCLUSION_RADIUS_M);
        }
    }

    #[test]
    fn flee_cooldown_counts_down_then_ends() {
        assert_eq!(flee_cooldown_step(1.0, 0.25), Some(0.75));
        assert_eq!(flee_cooldown_step(0.2, 0.25), None);
        assert_eq!(
            flee_cooldown_step(FLEE_COOLDOWN_S, 0.0),
            Some(FLEE_COOLDOWN_S)
        );
    }
}
