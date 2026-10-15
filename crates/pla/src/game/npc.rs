//! Brief 31 runtime NPCs: data-bounded placement, distance instancing and idle models.
//!
//! The sheets are the source of truth. Story/quest gates remain unresolved, so selection only
//! applies the decoded time-of-day predicates and otherwise chooses a stable positioned fallback.
//! No NPC route is consumed: `data/npc_paths.tsv` contains wild-Pokémon paths, not NPC routes.
//!
//! Appearance colours (clothing, hair and skin) are SUPPOSED/unknown. The runtime deliberately
//! does not fabricate a palette. Models are optional exports from `npc_export`; a neutral capsule
//! is used only when a model is not available locally. The model scene and animation wiring follow
//! the same Bevy glTF/animation path as the player, without changing the player exporter.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use bevy::prelude::*;

use super::clock::GameClock;
use super::navigation::GameNavmeshResource;
use super::player_motion::ground_surface_y;
use super::zone::TerrainResource;
use super::npc_model::{model_source, spawn_capsule, NpcModelCache, NpcModelPlugin, MODEL_ROOT_ABOVE_GROUND};
pub use super::npc_model::NpcModelSource;
use super::state::GameState;
use super::trainer_encounter::TrainerWorld;
use super::ui::ActiveZoneInfo;
use super::world_objects::{
    find_data_file, should_be_alive, DEFAULT_CACHE_ROOT, LOAD_RADIUS, UNLOAD_RADIUS,
};
use super::PlayerAvatar;

/// The NPC root is placed at the ground point and the exported model is lowered by this amount.
pub const NPC_ROOT_ABOVE_GROUND: f32 = MODEL_ROOT_ABOVE_GROUND;
/// NPC entities created per frame at most (nearest first).
pub const NPC_SPAWNS_PER_FRAME: usize = 8;
/// The source sheet has no NPC routes. This is an explicit runtime contract, not an inferred path.
pub const NPC_ROUTE_CONFIDENCE: &str = "UNKNOWN";

/// One decoded NPC placement variant. Conditions are retained verbatim because story semantics are
/// not decoded.
#[derive(Debug, Clone, PartialEq)]
pub struct NpcPlacement {
    pub area: String,
    pub key: String,
    pub variant: u16,
    pub variants: u16,
    pub chara_hash: String,
    pub chara_id: Option<String>,
    pub position: [f32; 3],
    pub yaw_deg: f32,
    pub cond_role: String,
    pub cond_name: String,
    pub cond_value: String,
    pub cond_value2: String,
}

impl NpcPlacement {
    pub fn positioned(&self) -> bool {
        self.position.iter().all(|v| v.is_finite()) && self.position != [0.0, 0.0, 0.0]
    }
}

/// Player landings never sit inside an NPC (planar metres): a little above the follower's own
/// [`super::follower::MIN_SEPARATION`] (1.4 m), the existing "two models never overlap" floor.
pub const MIN_NPC_SEPARATION: f32 = 1.6;

/// Move a landing point out of the nearest NPC spot(s).
///
/// `spots` are the NPC placements the streamer can draw (every variant of a key shares one spot).
/// While the closest spot is nearer than [`MIN_NPC_SEPARATION`], the point moves along the away
/// vector to exactly that distance; a perfect overlap falls back to +X (a fixed convention, there
/// is no "away" direction then). At most 4 passes: a spot can push the point into another. `None`
/// when the landing is already clear. Deterministic: spots are visited in the given order.
pub fn landing_clear_of_npcs(landing: Vec2, spots: impl IntoIterator<Item = Vec2>) -> Option<Vec2> {
    let spots: Vec<Vec2> = spots.into_iter().collect();
    let mut p = landing;
    let mut moved = false;
    for _ in 0..4 {
        let mut nearest: Option<(Vec2, f32)> = None;
        for spot in &spots {
            let d = (p - *spot).length();
            if nearest.is_none_or(|(_, best)| d < best) {
                nearest = Some((*spot, d));
            }
        }
        let Some((spot, d)) = nearest else { break };
        if d >= MIN_NPC_SEPARATION {
            break;
        }
        let away = if d > 1e-3 { (p - spot) / d } else { Vec2::X };
        p = spot + away * MIN_NPC_SEPARATION;
        moved = true;
    }
    moved.then_some(p)
}

impl NpcWorld {
    /// The NPC spots of one area (`ha_area00`), every variant: the arrival nudge keeps the player
    /// out of these. Placements without a position are skipped (the streamer cannot draw them).
    pub fn spots_in_area<'a>(&'a self, area: &'a str) -> impl Iterator<Item = Vec2> + 'a {
        self.placements
            .iter()
            .filter(move |p| p.area == area && p.positioned())
            .map(|p| Vec2::new(p.position[0], p.position[2]))
    }
}

/// One decoded character recipe row, keyed by the placement hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcAppearance {
    pub hash: String,
    pub name: Option<String>,
    pub kind: String,
    pub base_model: Option<String>,
    pub dress_model: Option<String>,
    pub face_model: Option<String>,
    pub motion_set: Option<String>,
    pub pack: Option<String>,
    pub controller_clips: Vec<String>,
}

/// A decoded animation row. The runtime uses this census to choose an idle deterministically;
/// the exported NPC glTF exposes the selected body idle as the `idle` clip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcAnimationClip {
    pub clip: String,
    pub motion_dir: String,
    pub class: String,
    pub looped: bool,
}

/// Values used by the bounded root-yaw look-at fallback. The values themselves are decoded config;
/// their use here is a port policy and is therefore not native-behaviour proof.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NpcTuning {
    pub forward_range_m: f32,
    pub forward_angle_deg: f32,
    pub close_range_m: f32,
    pub close_angle_deg: f32,
    pub look_behind_angle_deg: f32,
    pub head_rotation_speed_deg_s: f32,
}

impl Default for NpcTuning {
    fn default() -> Self {
        Self {
            forward_range_m: 3.5,
            forward_angle_deg: 75.0,
            close_range_m: 0.95,
            close_angle_deg: 360.0,
            look_behind_angle_deg: 135.0,
            head_rotation_speed_deg_s: 60.0,
        }
    }
}

/// Runtime NPC data and distance-instancing state.
#[derive(Resource, Debug)]
pub struct NpcWorld {
    pub placements: Vec<NpcPlacement>,
    pub appearances: BTreeMap<String, NpcAppearance>,
    pub animations: Vec<NpcAnimationClip>,
    pub tuning: NpcTuning,
    pub active: Vec<NpcPlacement>,
    pub cache_root: PathBuf,
    spawned: HashMap<String, Entity>,
    last_area: Option<String>,
    last_slot: Option<usize>,
}

/// Runtime marker for one selected NPC placement. Routes remain explicitly UNKNOWN.
#[derive(Component, Debug, Clone)]
pub struct NpcInstance {
    pub key: String,
    pub chara_hash: String,
    pub chara_id: Option<String>,
    pub idle_clip: Option<String>,
    pub route_confidence: &'static str,
}

/// The root-yaw look-at state. Head/torso bone rotation is not fabricated because the existing
/// player/model API does not expose a stable NPC rig contract; root yaw is bounded and deterministic.
#[derive(Component, Debug, Clone, Copy)]
struct NpcLookState {
    base_yaw: f32,
    yaw: f32,
}

/// Runtime plugin. It loads only the already-decoded Brief 31 sheets.
#[derive(Debug, Clone)]
pub struct NpcPlugin {
    pub data_dir: Option<PathBuf>,
    pub cache_root: PathBuf,
}

impl Default for NpcPlugin {
    fn default() -> Self {
        Self {
            data_dir: None,
            cache_root: PathBuf::from(DEFAULT_CACHE_ROOT),
        }
    }
}

impl NpcPlugin {
    pub fn load_world(&self) -> Option<NpcWorld> {
        let dir = self.data_dir.clone().or_else(|| {
            find_data_file("npc_placements.tsv").and_then(|p| p.parent().map(PathBuf::from))
        })?;
        let read = |name: &str| -> Result<String, String> {
            std::fs::read_to_string(dir.join(name))
                .map_err(|e| format!("{}: {e}", dir.join(name).display()))
        };
        let placements = match read("npc_placements.tsv") {
            Ok(text) => parse_npc_placements(&text),
            Err(e) => {
                eprintln!("NPC sheets not loaded: {e}");
                return None;
            }
        };
        let appearances = match read("npc_appearance.tsv") {
            Ok(text) => parse_npc_appearances(&text),
            Err(e) => {
                eprintln!("NPC appearance sheet not loaded: {e}");
                return None;
            }
        };
        let animations = read("npc_animations.tsv")
            .map(|text| parse_npc_animations(&text))
            .unwrap_or_default();
        let tuning = read("npc_config.tsv")
            .map(|text| parse_npc_config(&text))
            .unwrap_or_default();
        println!(
            "NPCs: {} placement variants, {} appearance recipes, {} animation rows; routes UNKNOWN; clothing/hair/skin colours SUPPOSED",
            placements.len(),
            appearances.len(),
            animations.len()
        );
        Some(NpcWorld {
            placements,
            appearances,
            animations,
            tuning,
            active: Vec::new(),
            cache_root: self.cache_root.clone(),
            spawned: HashMap::new(),
            last_area: None,
            last_slot: None,
        })
    }
}

impl Plugin for NpcPlugin {
    fn build(&self, app: &mut App) {
        let Some(world) = self.load_world() else {
            return;
        };
        app.add_plugins(NpcModelPlugin)
            .insert_resource(world)
            .add_systems(Update, (refresh_npc_selection, stream_npcs, npc_look_at).chain());
    }
}

fn header_map(line: &str) -> HashMap<&str, usize> {
    line.split('\t')
        .enumerate()
        .map(|(i, h)| (h.split(':').next().unwrap_or(h), i))
        .collect()
}

fn field<'a>(row: &'a [&'a str], headers: &HashMap<&str, usize>, name: &str) -> &'a str {
    headers
        .get(name)
        .and_then(|i| row.get(*i))
        .copied()
        .unwrap_or("")
}

fn parse_f32(value: &str) -> f32 {
    value.parse::<f32>().unwrap_or(0.0)
}

/// Parse `data/npc_placements.tsv`, retaining only human NPC (`kind=npc`) variants.
pub fn parse_npc_placements(text: &str) -> Vec<NpcPlacement> {
    let mut headers = None;
    let mut out = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        if headers.is_none() {
            headers = Some(header_map(line));
            continue;
        }
        let h = headers.as_ref().expect("placement header");
        let row: Vec<&str> = line.split('\t').collect();
        if field(&row, h, "kind") != "npc" {
            continue;
        }
        let chara_id = match field(&row, h, "chara_id") {
            "" => None,
            value => Some(value.to_string()),
        };
        out.push(NpcPlacement {
            area: field(&row, h, "area").to_string(),
            key: field(&row, h, "key").to_string(),
            variant: field(&row, h, "variant").parse().unwrap_or(0),
            variants: field(&row, h, "variants").parse().unwrap_or(0),
            chara_hash: field(&row, h, "chara_hash").to_string(),
            chara_id,
            position: [
                parse_f32(field(&row, h, "x")),
                parse_f32(field(&row, h, "y")),
                parse_f32(field(&row, h, "z")),
            ],
            yaw_deg: parse_f32(field(&row, h, "rot_y")),
            cond_role: field(&row, h, "cond_role").to_string(),
            cond_name: field(&row, h, "cond_name").to_string(),
            cond_value: field(&row, h, "cond_value").to_string(),
            cond_value2: field(&row, h, "cond_value2").to_string(),
        });
    }
    out
}

/// Parse `data/npc_appearance.tsv`, keyed by the exact placement hash.
pub fn parse_npc_appearances(text: &str) -> BTreeMap<String, NpcAppearance> {
    let mut headers = None;
    let mut out = BTreeMap::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        if headers.is_none() {
            headers = Some(header_map(line));
            continue;
        }
        let h = headers.as_ref().expect("appearance header");
        let row: Vec<&str> = line.split('\t').collect();
        let hash = field(&row, h, "hash");
        if hash.is_empty() {
            continue;
        }
        let optional = |name: &str| {
            (!field(&row, h, name).is_empty()).then(|| field(&row, h, name).to_string())
        };
        let controller_clips = field(&row, h, "controller_clips")
            .split(',')
            .filter(|clip| !clip.is_empty())
            .map(str::to_string)
            .collect();
        out.insert(
            hash.to_string(),
            NpcAppearance {
                hash: hash.to_string(),
                name: optional("name"),
                kind: field(&row, h, "kind").to_string(),
                base_model: optional("base_model"),
                dress_model: optional("dress_model"),
                face_model: optional("face_model"),
                motion_set: optional("motion_set"),
                pack: optional("pack"),
                controller_clips,
            },
        );
    }
    out
}

/// Parse the animation census. This is intentionally a data census, not a route or state-machine
/// decoder.
pub fn parse_npc_animations(text: &str) -> Vec<NpcAnimationClip> {
    let mut headers = None;
    let mut out = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        if headers.is_none() {
            headers = Some(header_map(line));
            continue;
        }
        let h = headers.as_ref().expect("animation header");
        let row: Vec<&str> = line.split('\t').collect();
        let clip = field(&row, h, "clip");
        if clip.is_empty() {
            continue;
        }
        out.push(NpcAnimationClip {
            clip: clip.to_string(),
            motion_dir: field(&row, h, "motion_dir").to_string(),
            class: field(&row, h, "class").to_string(),
            looped: field(&row, h, "loop") == "1",
        });
    }
    out
}

/// Parse the verified config values used by the bounded runtime policy.
pub fn parse_npc_config(text: &str) -> NpcTuning {
    let mut values = HashMap::new();
    let mut headers = None;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        if headers.is_none() {
            headers = Some(header_map(line));
            continue;
        }
        let h = headers.as_ref().expect("config header");
        let row: Vec<&str> = line.split('\t').collect();
        let param = field(&row, h, "param").to_string();
        if !param.is_empty() {
            values.insert(param, parse_f32(field(&row, h, "value")));
        }
    }
    let d = NpcTuning::default();
    NpcTuning {
        forward_range_m: values
            .get("Outdoor_ForwardReactionRange_Distance")
            .copied()
            .unwrap_or(d.forward_range_m),
        forward_angle_deg: values
            .get("ForwardReactionRange_Angle")
            .copied()
            .unwrap_or(d.forward_angle_deg),
        close_range_m: values
            .get("CloseReactionRange_Distance")
            .copied()
            .unwrap_or(d.close_range_m),
        close_angle_deg: values
            .get("CloseReactionRange_Angle")
            .copied()
            .unwrap_or(d.close_angle_deg),
        look_behind_angle_deg: values
            .get("look_behind_angle_deg")
            .copied()
            .unwrap_or(d.look_behind_angle_deg),
        head_rotation_speed_deg_s: values
            .get("npc_head_rotation_speed_per_sec")
            .copied()
            .unwrap_or(d.head_rotation_speed_deg_s),
    }
}

fn parsed_slot(value: &str) -> Option<usize> {
    value.parse::<usize>().ok().filter(|slot| *slot < 4)
}

fn time_matches(row: &NpcPlacement, time_slot: usize) -> bool {
    match row.cond_role.as_str() {
        "time_zone" => [
            row.cond_name.as_str(),
            row.cond_value.as_str(),
            row.cond_value2.as_str(),
        ]
        .into_iter()
        .filter_map(parsed_slot)
        .any(|slot| slot == time_slot),
        "time_zone_afternoon" => time_slot == 2,
        "time_zone_night" => time_slot == 3,
        _ => false,
    }
}

fn selection_rank(row: &NpcPlacement, time_slot: usize) -> u8 {
    if time_matches(row, time_slot) {
        3
    } else if row.cond_role == "default" || row.cond_role == "none" {
        2
    } else if row.positioned() {
        1
    } else {
        0
    }
}

/// Select one positioned variant per placement key. Time predicates win; unresolved story/flag
/// predicates are retained as a stable positioned fallback, never interpreted as completed.
pub fn select_active_placements(rows: &[NpcPlacement], time_slot: usize) -> Vec<NpcPlacement> {
    let mut groups: BTreeMap<&str, Vec<&NpcPlacement>> = BTreeMap::new();
    for row in rows {
        groups.entry(&row.key).or_default().push(row);
    }
    groups
        .into_values()
        .filter_map(|mut variants| {
            variants.retain(|row| row.positioned());
            variants.sort_by_key(|row| {
                (
                    std::cmp::Reverse(selection_rank(row, time_slot)),
                    row.variant,
                )
            });
            variants.first().map(|row| (*row).clone())
        })
        .collect()
}

/// Deterministically choose a body idle from a controller's listed clips. The body exporter maps
/// the choice to its `idle` glTF name; this function does not invent a missing state-machine clip.
pub fn select_idle_clip(controller_clips: &[String], seed: u64) -> Option<String> {
    let mut candidates: Vec<&String> = controller_clips
        .iter()
        .filter(|clip| clip.contains("wait") && clip.contains("_loop"))
        .collect();
    candidates.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    if candidates.is_empty() {
        return None;
    }
    if let Some(preferred) = candidates
        .iter()
        .find(|clip| clip.contains("02001_wait01_loop"))
    {
        return Some((*preferred).clone());
    }
    Some(
        candidates[(seed as usize) % candidates.len()]
            .clone()
            .clone(),
    )
}

fn angle_diff(from: f32, to: f32) -> f32 {
    (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// Target yaw relative to the NPC's original placement yaw. The target is bounded to the decoded
/// behind angle, so a player behind the NPC cannot cause an unrestricted snap.
pub fn bounded_look_target(base_yaw: f32, player_yaw: f32, max_angle_deg: f32) -> f32 {
    let delta = angle_diff(base_yaw, player_yaw);
    base_yaw + delta.clamp(-max_angle_deg.to_radians(), max_angle_deg.to_radians())
}

/// One deterministic bounded-yaw step. No head/torso bones are touched because their runtime rig
/// contract is not exposed by the existing player/model APIs.
pub fn step_bounded_look(current_yaw: f32, target_yaw: f32, rate_deg_s: f32, dt: f32) -> f32 {
    let delta = angle_diff(current_yaw, target_yaw);
    let max_step = rate_deg_s.max(0.0).to_radians() * dt.max(0.0);
    if delta.abs() <= max_step {
        target_yaw
    } else {
        current_yaw + max_step * delta.signum()
    }
}

/// Standing height for a placement: the surface the player walks on (navmesh, then terrain heightmap) when it is
/// within 3 m of the sheet's Y; otherwise the sheet's Y (interiors, roofs and platforms are not on that surface).
pub fn snap_ground_y(position: [f32; 3], surface: Option<f32>) -> f32 {
    surface.filter(|y| y.is_finite() && (y - position[1]).abs() <= 3.0).unwrap_or(position[1])
}

pub fn should_instance(player_xz: Vec2, npc_xz: Vec2, alive: bool) -> bool {
    should_instance_scaled(player_xz, npc_xz, alive, 1.0)
}

/// [`should_instance`] with the Graphics screen "NPC range" multiplier (1.0 = 200 / 230 m).
pub fn should_instance_scaled(player_xz: Vec2, npc_xz: Vec2, alive: bool, scale: f32) -> bool {
    should_be_alive(player_xz, npc_xz, alive, LOAD_RADIUS * scale, UNLOAD_RADIUS * scale)
}

fn active_for_state(state: Option<Res<State<GameState>>>) -> bool {
    state.map_or(true, |state| *state.get() == GameState::World)
}

fn refresh_npc_selection(
    mut commands: Commands,
    mut world: ResMut<NpcWorld>,
    zone: Option<Res<ActiveZoneInfo>>,
    clock: Option<Res<GameClock>>,
    state: Option<Res<State<GameState>>>,
    trainers: Option<Res<TrainerWorld>>,
) {
    if !active_for_state(state) {
        return;
    }
    let Some(zone) = zone else { return };
    let slot = clock.map_or(1, |clock| clock.slot());
    if world.last_area.as_deref() == Some(zone.area.as_str()) && world.last_slot == Some(slot) {
        return;
    }
    for entity in world.spawned.values() {
        commands.entity(*entity).despawn();
    }
    world.spawned.clear();
    world.active = select_active_placements(
        &world
            .placements
            .iter()
            .filter(|row| row.area == zone.area)
            // A placed trainer is drawn by its marker (trainer_encounter), not twice.
            .filter(|row| !trainers.as_deref().is_some_and(|t| t.has_enabled_marker_key(&row.key)))
            .cloned()
            .collect::<Vec<_>>(),
        slot,
    );
    world.last_area = Some(zone.area.clone());
    world.last_slot = Some(slot);
    println!(
        "NPCs: {} active positioned placements in {} at time slot {}",
        world.active.len(),
        zone.area,
        slot
    );
}

#[allow(clippy::too_many_arguments)]
fn stream_npcs(
    mut commands: Commands,
    mut world: ResMut<NpcWorld>,
    player: Query<&Transform, With<PlayerAvatar>>,
    nav: Option<Res<GameNavmeshResource>>,
    terrain: Option<Res<TerrainResource>>,
    trainers: Option<Res<TrainerWorld>>,
    asset_server: Option<Res<AssetServer>>,
    mut models: ResMut<NpcModelCache>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    dist: Option<Res<super::quality::StreamDistances>>,
) {
    let Ok(player) = player.single() else { return };
    let player_xz = Vec2::new(player.translation.x, player.translation.z);
    let k = super::quality::scale_of(dist.as_deref(), |d| d.npcs);
    let active_keys: HashMap<String, ()> =
        world
            .active
            .iter()
            .filter(|p| !trainers.as_deref().is_some_and(|t| t.has_enabled_marker_key(&p.key)))
            .map(|p| (p.key.clone(), ()))
            .collect();
    let to_remove: Vec<(String, Entity)> = world
        .spawned
        .iter()
        .filter(|(key, _)| !active_keys.contains_key(*key))
        .map(|(key, entity)| (key.clone(), *entity))
        .chain(
            world
                .spawned
                .iter()
                .filter(|(key, _)| {
                    !trainers
                        .as_deref()
                        .is_some_and(|t| t.has_enabled_marker_key((*key).as_str()))
                })
                .filter_map(|(key, entity)| {
                    let placement = world.active.iter().find(|p| p.key == *key)?;
                    let alive = should_instance_scaled(
                        player_xz,
                        Vec2::new(placement.position[0], placement.position[2]),
                        true,
                        k,
                    );
                    (!alive).then_some((key.clone(), *entity))
                }),
        )
        .collect();
    for (key, entity) in to_remove {
        commands.entity(entity).despawn();
        world.spawned.remove(&key);
    }

    let mut to_spawn: Vec<NpcPlacement> = world
        .active
        .iter()
        .filter(|placement| !world.spawned.contains_key(&placement.key))
        .filter(|placement| {
            !trainers
                .as_deref()
                .is_some_and(|t| t.has_enabled_marker_key(&placement.key))
        })
        .filter(|placement| {
            should_instance_scaled(
                player_xz,
                Vec2::new(placement.position[0], placement.position[2]),
                false,
                k,
            )
        })
        .cloned()
        .collect();
    // Nearest first, a few per frame: a crowded square fills in over several frames, no hitch.
    to_spawn.sort_by(|a, b| {
        let da = Vec2::new(a.position[0], a.position[2]).distance_squared(player_xz);
        let db = Vec2::new(b.position[0], b.position[2]).distance_squared(player_xz);
        da.total_cmp(&db)
    });
    to_spawn.truncate(NPC_SPAWNS_PER_FRAME);
    for placement in to_spawn {
        let entity = spawn_npc(
            &mut commands,
            &world,
            &placement,
            nav.as_deref(),
            terrain.as_deref(),
            asset_server.as_deref(),
            &mut models,
            meshes.as_deref_mut(),
            materials.as_deref_mut(),
        );
        world.spawned.insert(placement.key, entity);
    }
}

fn spawn_npc(
    commands: &mut Commands,
    world: &NpcWorld,
    placement: &NpcPlacement,
    nav: Option<&GameNavmeshResource>,
    terrain: Option<&TerrainResource>,
    asset_server: Option<&AssetServer>,
    models: &mut NpcModelCache,
    mut meshes: Option<&mut Assets<Mesh>>,
    mut materials: Option<&mut Assets<StandardMaterial>>,
) -> Entity {
    let ground_y = snap_ground_y(placement.position, ground_surface_y(terrain, nav, placement.position[0], placement.position[2]));
    let base_yaw = placement.yaw_deg.to_radians();
    let appearance = world.appearances.get(&placement.chara_hash);
    let idle_clip = appearance.and_then(|appearance| {
        select_idle_clip(&appearance.controller_clips, stable_seed(&placement.key))
    });
    let mut entity = commands.spawn((
        NpcInstance {
            key: placement.key.clone(),
            chara_hash: placement.chara_hash.clone(),
            chara_id: placement.chara_id.clone(),
            idle_clip,
            route_confidence: NPC_ROUTE_CONFIDENCE,
        },
        NpcLookState {
            base_yaw,
            yaw: base_yaw,
        },
        Transform::from_xyz(
            placement.position[0],
            ground_y + NPC_ROOT_ABOVE_GROUND,
            placement.position[2],
        )
        .with_rotation(Quat::from_rotation_y(base_yaw)),
        Visibility::default(),
        Name::new(format!("npc {}", placement.key)),
    ));
    let model_id = appearance
        .and_then(|appearance| appearance.name.as_deref())
        .or(placement.chara_id.as_deref())
        .unwrap_or(placement.chara_hash.as_str());
    let info = models.info(&world.cache_root, model_id);
    if let (Some(asset_server), Some(info)) = (asset_server, info) {
        entity.insert(model_source(asset_server, &info, 1.0));
    } else if let (Some(meshes), Some(materials)) = (meshes.as_deref_mut(), materials.as_deref_mut()) {
        spawn_capsule(&mut entity, meshes, materials, Color::srgb(0.35, 0.5, 0.58));
    }
    entity.id()
}

fn stable_seed(value: &str) -> u64 {
    value.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

fn npc_look_at(
    time: Res<Time>,
    tuning: Option<Res<NpcWorld>>,
    player: Query<&Transform, With<PlayerAvatar>>,
    mut npcs: Query<
        (&mut Transform, &mut NpcLookState),
        (With<NpcInstance>, Without<PlayerAvatar>),
    >,
    state: Option<Res<State<GameState>>>,
) {
    if !active_for_state(state) {
        return;
    }
    let Ok(player) = player.single() else { return };
    let tuning = tuning.map_or_else(NpcTuning::default, |world| world.tuning);
    for (mut transform, mut look) in &mut npcs {
        let dx = player.translation.x - transform.translation.x;
        let dz = player.translation.z - transform.translation.z;
        let distance = (dx * dx + dz * dz).sqrt();
        let target = if distance <= tuning.close_range_m {
            Some(look_target_from_vector(
                look.base_yaw,
                dx,
                dz,
                tuning.close_angle_deg.min(tuning.look_behind_angle_deg),
            ))
        } else if distance <= tuning.forward_range_m {
            let player_yaw = dx.atan2(dz);
            let offset = angle_diff(look.base_yaw, player_yaw).abs().to_degrees();
            (offset <= tuning.forward_angle_deg).then(|| {
                bounded_look_target(look.base_yaw, player_yaw, tuning.look_behind_angle_deg)
            })
        } else {
            None
        };
        let target = target.unwrap_or(look.base_yaw);
        look.yaw = step_bounded_look(
            look.yaw,
            target,
            tuning.head_rotation_speed_deg_s,
            time.delta_secs(),
        );
        transform.rotation = Quat::from_rotation_y(look.yaw);
    }
}

fn look_target_from_vector(base_yaw: f32, dx: f32, dz: f32, max_angle_deg: f32) -> f32 {
    if dx.abs() + dz.abs() <= f32::EPSILON {
        base_yaw
    } else {
        bounded_look_target(base_yaw, dx.atan2(dz), max_angle_deg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLACEMENT_HEADER: &str = "area:string*\tkind:enum\tkey:string*\tvariant:u16*\tvariants:u16\tchara_hash:string\tchara_id:string?\tchara_kind:enum\tx:f32\ty:f32\tz:f32\trot_x:f32\trot_y:f32\trot_z:f32\tscale:f32\tcond_kind:string?\tcond_role:enum\tcond_op:string?\tcond_name:string?\tcond_value:string?\tcond_value2:string?";

    #[test]
    fn placement_parser_filters_pokemon_and_selects_time_variant() {
        let text = format!(
            "{PLACEMENT_HEADER}\nha_area00\tnpc\tvillager\t0\t3\thash\tcc_a\tcc\t10\t2\t20\t0\t0\t0\t1\t\tdefault\t\t\t\t\nha_area00\tnpc\tvillager\t1\t3\thash\tcc_a\tcc\t11\t2\t21\t0\t90\t0\t1\t\ttime_zone\t\t2\t\t\nha_area00\tnpc_pokemon\tpika\t0\t1\tpika_hash\t\tpm\t12\t2\t22\t0\t0\t0\t1\t\tdefault\t\t\t\t"
        );
        let rows = parse_npc_placements(&text);
        assert_eq!(rows.len(), 2);
        let selected = select_active_placements(&rows, 2);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].variant, 1);
        assert_eq!(selected[0].position, [11.0, 2.0, 21.0]);
    }

    #[test]
    fn distance_instancing_uses_load_and_unload_hysteresis() {
        let player = Vec2::ZERO;
        let npc = Vec2::new(210.0, 0.0);
        assert!(!should_instance(player, npc, false));
        assert!(should_instance(player, npc, true));
        assert!(should_instance(player, Vec2::new(199.9, 0.0), false));
        assert!(!should_instance(player, Vec2::new(230.1, 0.0), true));
    }

    #[test]
    fn idle_selection_is_stable_and_prefers_primary_wait() {
        let clips = vec![
            "tr_02022_wait03_loop".to_string(),
            "tr_02001_wait01_loop".to_string(),
            "tr_02012_wait02_loop".to_string(),
        ];
        assert_eq!(
            select_idle_clip(&clips, 0).as_deref(),
            Some("tr_02001_wait01_loop")
        );
        let without_primary = vec!["b_wait02_loop".to_string(), "a_wait01_loop".to_string()];
        assert_eq!(
            select_idle_clip(&without_primary, 0),
            select_idle_clip(&without_primary, 0)
        );
        assert_ne!(select_idle_clip(&without_primary, 0), None);
    }

    #[test]
    fn landing_never_stays_inside_an_npc() {
        // The capture case: the player landed 0.05 m from `area00_npc_re_leader`, with the senior
        // (0.7 m) and the doctor (1.3 m) around it (data/npc_placements.tsv, ha_area00).
        let leader = Vec2::new(485.748, 449.004);
        let senior = Vec2::new(485.102, 448.897);
        let doctor = Vec2::new(484.518, 448.799);
        let landing = Vec2::new(485.8, 449.0);
        let clear = landing_clear_of_npcs(landing, [leader, senior, doctor]).expect("moved");
        for spot in [leader, senior, doctor] {
            assert!(
                clear.distance(spot) >= MIN_NPC_SEPARATION - 1e-4,
                "{clear:?} is still inside the NPC at {spot:?}"
            );
        }
        // Far away (or no spots): nothing moves.
        assert!(landing_clear_of_npcs(Vec2::ZERO, [Vec2::new(5.0, 0.0)]).is_none());
        assert!(landing_clear_of_npcs(Vec2::ZERO, []).is_none());
        // Exactly on the spot: the fixed +X convention has no away direction to use.
        assert_eq!(
            landing_clear_of_npcs(Vec2::ZERO, [Vec2::ZERO]),
            Some(Vec2::new(MIN_NPC_SEPARATION, 0.0))
        );
        // Deterministic for the same input.
        assert_eq!(
            landing_clear_of_npcs(landing, [leader, senior, doctor]),
            Some(clear)
        );
    }

    #[test]
    fn look_at_angle_is_clamped_and_deterministic() {
        let back = std::f32::consts::PI;
        let a = bounded_look_target(0.0, back, 135.0);
        let b = bounded_look_target(0.0, back, 135.0);
        assert_eq!(a, b);
        assert!((a.to_degrees().abs() - 135.0).abs() < 1e-5);
        let step = step_bounded_look(0.0, a, 60.0, 0.5);
        assert!((step.to_degrees().abs() - 30.0).abs() < 1e-5);
    }

    #[test]
    fn config_parser_uses_decoded_reaction_and_look_values() {
        let text = "file:string*\tparam:string*\tvalue:string\n npc_ai_config\tOutdoor_ForwardReactionRange_Distance\t3.5\n character_look_at_config\tnpc_head_rotation_speed_per_sec\t60\n";
        let tuning = parse_npc_config(text);
        assert_eq!(tuning.forward_range_m, 3.5);
        assert_eq!(tuning.head_rotation_speed_deg_s, 60.0);
    }

    #[test]
    fn real_brief31_sheets_match_the_reported_counts() {
        let data = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let placements = parse_npc_placements(
            &std::fs::read_to_string(data.join("npc_placements.tsv")).unwrap(),
        );
        let appearances = parse_npc_appearances(
            &std::fs::read_to_string(data.join("npc_appearance.tsv")).unwrap(),
        );
        let animations = parse_npc_animations(
            &std::fs::read_to_string(data.join("npc_animations.tsv")).unwrap(),
        );
        assert_eq!(placements.len(), 1_325);
        assert_eq!(appearances.len(), 239);
        assert_eq!(animations.len(), 3_932);
        assert_eq!(placements.iter().filter(|p| p.positioned()).count(), 884);
    }
}
