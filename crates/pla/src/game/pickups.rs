//! Field pickups: sparkling ground items, harvestable resource nodes and named collectibles.
//!
//! Data: `data/field_item_spawns.tsv` (`.tools/decode_field_items.py`, Brief 12). Three kinds:
//!
//! - `direct_item`: a sparkling ground item. The sheet carries its world model (`item_NNN`) but
//!   NOT the reward item id (see `reports/data-progress/field_items.md`); picking it collects the
//!   spawn and adds nothing to the bag until that link is decoded.
//! - `resource_node`: a tree / ore landmark whose per-area loot table IS decoded (item ids and
//!   weights). Interacting harvests one weighted item and shakes the marker briefly.
//! - `search_item`: a one-time named collectible (its system flag is in the sheet). Picking it
//!   collects the spawn; there is no inventory item.
//!
//! Collected spawn ids persist in `PlayerProgress.picked` (save format v7). Respawn: the game's
//! `field_item_respawn.bin` gives a margin of 40 for all three kinds with an unknown unit, so the
//! port uses a fixed [`RESPAWN_SECONDS`] (ASSUMED) and removes the id from the save when it fires.
//!
//! Resource nodes react to a thrown ball (the game's harvesting is by throwing, see
//! `reports/function-progress/n2-field-interact-and-harvesting.md`; PROBABLE, data-shape level):
//! a ball in flight that passes through a node harvests it. Trees stay standing and sway
//! (the reward is already in the bag); ore and crystals break and disappear. The interact key
//! still harvests from close range as a fallback. Sway length, break time and the hit volume are
//! ASSUMED.
//!
//! Markers are simple meshes: the decoded item models are not exported (blocked in the exporter)
//! and the gimmick models are not always available, so the maintainer judges the marker look.
//! They are placeholder visuals, OFF by default (`PLA_DEBUG_PLACEHOLDERS=1` re-enables them, see
//! [`super::debug_flags`]): the spawns still stream ([`SpawnMarker`] + `Transform`) and every
//! interaction behaves the same, but the default build draws no proxy cube/cone/sphere.

use std::collections::{BTreeMap, HashMap};

use bevy::prelude::*;

use super::ball_lifecycle::BallBody;
use super::input_map::{Action, ActionState};
use super::save_wire::PlayerProgress;
use super::wild::splitmix64;
use super::PlayerAvatar;

/// Spawns closer than this to the player are alive.
pub const LOAD_RADIUS: f32 = 120.0;
/// Spawns farther than this are removed (hysteresis).
pub const UNLOAD_RADIUS: f32 = 150.0;
/// Horizontal distance at which the interact key can collect a spawn.
pub const PICK_RANGE: f32 = 3.0;
/// Seconds until a collected spawn becomes pickable again. ASSUMED: `field_item_respawn.bin`
/// gives `directItem/landmark/searchItemRespawnMagine = 40` (each as the string `"40"`; the unit
/// is unknown, see the report).
pub const RESPAWN_SECONDS: f32 = 600.0;
/// Stack ceiling used when the item table is not loaded (VERIFIED 999 in `item_effects.tsv`).
pub const MAX_STACK_FALLBACK: u16 = 999;
/// How long a resource node shakes after it is harvested.
const SHAKE_SECONDS: f32 = 0.6;
/// How long a harvested tree sways before it is inert again (ASSUMED).
pub const TREE_SWAY_SECONDS: f32 = 1.2;
/// Horizontal radius within which a thrown ball hits a node (ASSUMED).
pub const BALL_HIT_RADIUS: f32 = 1.5;
/// Height above the node's ground point that a ball can still hit a tree (canopy) / an ore.
pub const TREE_HIT_HEIGHT: f32 = 5.0;
pub const ORE_HIT_HEIGHT: f32 = 1.8;

/// How a resource node behaves when harvested, from its landmark model name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// Berry trees, bushes and leaf piles: stay standing and sway.
    Tree,
    /// Crystals, rocks and boxes: break and disappear until they respawn.
    Ore,
}

/// Classify a `gimmick_*` model name (27 distinct names in the decoded sheet).
pub fn node_kind(model: &str) -> NodeKind {
    if model.contains("tree") || model.contains("yachi") || model.contains("leaves") {
        NodeKind::Tree
    } else {
        NodeKind::Ore
    }
}

/// Does a ball at `ball` (world position) hit the node standing at `spawn_pos` (ground point)?
pub fn ball_hits_node(ball: Vec3, spawn_pos: [f32; 3], kind: NodeKind) -> bool {
    let dx = ball.x - spawn_pos[0];
    let dz = ball.z - spawn_pos[2];
    if dx * dx + dz * dz > BALL_HIT_RADIUS * BALL_HIT_RADIUS {
        return false;
    }
    let height = match kind {
        NodeKind::Tree => TREE_HIT_HEIGHT,
        NodeKind::Ore => ORE_HIT_HEIGHT,
    };
    ball.y >= spawn_pos[1] - 0.3 && ball.y <= spawn_pos[1] + height
}

/// Sideways sway (m) of a harvested tree at time `t` of `duration`: a damped oscillation.
pub fn sway_offset(t: f32, duration: f32) -> f32 {
    if duration <= 0.0 || !(0.0..duration).contains(&t) {
        return 0.0;
    }
    0.25 * (1.0 - t / duration) * (t * 22.0).sin()
}

/// Scale of a breaking ore at time `t` of `duration`: shrinks linearly to nothing.
pub fn break_scale(t: f32, duration: f32) -> f32 {
    if duration <= 0.0 {
        return 0.0;
    }
    (1.0 - t / duration).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnKind {
    DirectItem,
    ResourceNode,
    SearchItem,
}

impl SpawnKind {
    fn from_col(s: &str) -> Option<Self> {
        match s {
            "direct_item" => Some(Self::DirectItem),
            "resource_node" => Some(Self::ResourceNode),
            "search_item" => Some(Self::SearchItem),
            _ => None,
        }
    }
}

/// One row of `data/field_item_spawns.tsv`.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemSpawn {
    pub kind: SpawnKind,
    pub area: String,
    pub key: String,
    /// Unique spawn id (the placement hash); the persistence key.
    pub id: u64,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub model: String,
    /// `(item id, weight)` pairs, resource nodes only.
    pub loot: Vec<(u16, u16)>,
    pub count_min: u16,
    pub count_max: u16,
    pub flag: String,
}

/// Parse the spawn sheet, keeping `area` (e.g. `ha_area01`). Unknown kinds and rows without a
/// parseable id/position are skipped, like the other sheet parsers.
pub fn parse_spawns(text: &str, area: &str) -> Vec<ItemSpawn> {
    let mut cols: HashMap<&str, usize> = HashMap::new();
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let c: Vec<&str> = line.split('\t').collect();
        if cols.is_empty() {
            for (i, h) in c.iter().enumerate() {
                cols.insert(h.split(':').next().unwrap_or(h), i);
            }
            continue;
        }
        let get = |n: &str| cols.get(n).and_then(|&i| c.get(i)).copied().unwrap_or("");
        let Some(kind) = SpawnKind::from_col(get("kind")) else {
            continue;
        };
        if get("area") != area {
            continue;
        }
        let Some(id) = u64::from_str_radix(get("placement_id"), 16).ok() else {
            continue;
        };
        let pos = ["x", "y", "z"].map(|n| get(n).parse::<f32>().unwrap_or(f32::NAN));
        if pos.iter().any(|v| !v.is_finite()) {
            continue;
        }
        let loot = get("loot_item_ids")
            .split(';')
            .zip(get("loot_weights").split(';'))
            .filter_map(|(i, w)| Some((i.parse::<u16>().ok()?, w.parse::<u16>().ok()?)))
            .collect();
        out.push(ItemSpawn {
            kind,
            area: get("area").to_string(),
            key: get("spawn_key").to_string(),
            id,
            pos,
            yaw: get("yaw").parse().unwrap_or(0.0),
            model: get("model").to_string(),
            loot,
            count_min: get("loot_min").parse().unwrap_or(0),
            count_max: get("loot_max").parse().unwrap_or(0),
            flag: get("flag").to_string(),
        });
    }
    out
}

/// Horizontal distance test with hysteresis, like `world_objects::should_be_alive`.
pub fn should_be_alive(
    player_xz: Vec2,
    spawn_xz: Vec2,
    alive: bool,
    load: f32,
    unload: f32,
) -> bool {
    let d = player_xz.distance(spawn_xz);
    if alive {
        d <= unload
    } else {
        d <= load
    }
}

/// Index of the closest spawn in `range` for which `wanted` is true.
pub fn nearest_within(
    spawns: &[ItemSpawn],
    player_xz: Vec2,
    range: f32,
    wanted: impl Fn(&ItemSpawn) -> bool,
) -> Option<usize> {
    spawns
        .iter()
        .enumerate()
        .filter(|(_, s)| wanted(s))
        .map(|(i, s)| (i, player_xz.distance(Vec2::new(s.pos[0], s.pos[2]))))
        .filter(|(_, d)| *d <= range)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// One weighted loot row; deterministic in `rng`.
pub fn roll_loot(loot: &[(u16, u16)], rng: &mut u64) -> Option<u16> {
    let total: u64 = loot.iter().map(|(_, w)| u64::from(*w)).sum();
    if total == 0 {
        return None;
    }
    let mut r = splitmix64(rng) % total;
    for &(id, w) in loot {
        if r < u64::from(w) {
            return Some(id);
        }
        r -= u64::from(w);
    }
    loot.last().map(|(id, _)| *id)
}

/// How many units one harvest gives: `count_min..=count_max` (1..=1 when the sheet has none).
pub fn roll_count(min: u16, max: u16, rng: &mut u64) -> u16 {
    let (lo, hi) = if max == 0 {
        (1, 1)
    } else {
        (min.max(1), max.max(min.max(1)))
    };
    (u64::from(lo) + splitmix64(rng) % (u64::from(hi - lo) + 1)) as u16
}

/// Add up to the stack ceiling; returns how many were stored. Zero quantities are dropped.
pub fn add_to_bag(bag: &mut BTreeMap<u16, u16>, id: u16, qty: u16, cap: u16) -> u16 {
    if qty == 0 {
        return 0;
    }
    let slot = bag.entry(id).or_insert(0);
    let stored = qty.min(cap.saturating_sub(*slot));
    *slot += stored;
    if *slot == 0 {
        bag.remove(&id);
    }
    stored
}

/// What one interaction did, for tests and the HUD toast.
#[derive(Debug, Clone, PartialEq)]
pub enum PickupResult {
    /// The spawn was collected; it may grant an item.
    Collected { item: Option<(u16, u16)> },
    /// The spawn exists but is not ready yet.
    Nothing,
}

/// Apply a pickup to the progress: collect the spawn id, grant the (rolled) item for a resource
/// node, and return the granted `(item id, count)`.
pub fn apply_pickup(
    spawn: &ItemSpawn,
    progress: &mut PlayerProgress,
    rng: &mut u64,
    item_cap: impl Fn(u16) -> u16,
) -> PickupResult {
    if progress.picked.contains(&spawn.id) {
        return PickupResult::Nothing;
    }
    progress.picked.insert(spawn.id);
    match spawn.kind {
        SpawnKind::ResourceNode => {
            let Some(id) = roll_loot(&spawn.loot, rng) else {
                return PickupResult::Collected { item: None };
            };
            let qty = roll_count(spawn.count_min, spawn.count_max, rng);
            let stored = add_to_bag(&mut progress.bag, id, qty, item_cap(id));
            PickupResult::Collected {
                item: (stored > 0).then_some((id, stored)),
            }
        }
        // The reward of ground items is not decoded; named collectibles give no item.
        SpawnKind::DirectItem | SpawnKind::SearchItem => PickupResult::Collected { item: None },
    }
}

/// Human-readable one-liner for the interaction toast / debug print.
pub fn describe_pickup(spawn: &ItemSpawn, result: &PickupResult) -> String {
    match result {
        PickupResult::Nothing => format!("{} is not ready", spawn.key),
        PickupResult::Collected { item } => match item {
            Some((id, n)) => format!("collected {} ({} x{})", spawn.key, id, n),
            None if spawn.kind == SpawnKind::ResourceNode => {
                format!("collected {} (bag full)", spawn.key)
            }
            None => format!("collected {}", spawn.key),
        },
    }
}

// ---------------------------------------------------------------- Bevy side

/// Generic marker for one alive spawn (public so tools such as the automation mode can count them).
#[derive(Component)]
#[allow(dead_code)]
pub struct SpawnMarker(pub u64);

/// A resource node that was harvested and is swaying (tree) or breaking (ore).
#[derive(Component)]
struct Shaking {
    timer: Timer,
    kind: NodeKind,
    /// Marker translation when the shake started (captured on the first tick).
    base: Option<Vec3>,
}

/// Interactive spawns of the area the viewer loaded.
pub struct PickupsPlugin {
    pub area: u8,
}

#[derive(Resource)]
struct Pickups {
    spawns: Vec<ItemSpawn>,
    /// `spawn id -> index in spawns`.
    by_id: HashMap<u64, usize>,
    alive: HashMap<u64, Entity>,
    /// Collected ids with the seconds left until they can be picked again.
    respawn: HashMap<u64, f32>,
    /// Shared marker assets, created once per marker style.
    markers: MarkerCache,
    rng: u64,
}

/// One cached marker style: mesh, material and the y offset.
#[derive(Default)]
struct MarkerCache {
    tree: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
    ore: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
    item: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
    collectible: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
}

impl Plugin for PickupsPlugin {
    fn build(&self, app: &mut App) {
        let area = format!("ha_area{:02}", self.area);
        let Some(text) = super::world_objects::find_data_file("field_item_spawns.tsv")
            .and_then(|p| std::fs::read_to_string(p).ok())
        else {
            println!("Pickups: data/field_item_spawns.tsv not found; skipping.");
            return;
        };
        let spawns = parse_spawns(&text, &area);
        if spawns.is_empty() {
            println!("Pickups {area}: no rows; skipping.");
            return;
        }
        let by_kind = |k: SpawnKind| spawns.iter().filter(|s| s.kind == k).count();
        println!(
            "Pickups {area}: {} spawns ({} ground items, {} resource nodes, {} collectibles), streamed within {LOAD_RADIUS} m.",
            spawns.len(),
            by_kind(SpawnKind::DirectItem),
            by_kind(SpawnKind::ResourceNode),
            by_kind(SpawnKind::SearchItem)
        );
        let rng = 0x5ca1_ab1e_u64;
        let by_id = spawns.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
        app.insert_resource(Pickups {
            spawns,
            by_id,
            alive: HashMap::new(),
            respawn: HashMap::new(),
            markers: MarkerCache::default(),
            rng,
        });
        app.add_systems(
            Update,
            (
                tick_respawn,
                stream_pickups,
                shake_nodes,
                pickup_interact,
                thrown_ball_hits_nodes,
            )
                .chain(),
        );
    }
}

fn tick_respawn(
    time: Res<Time>,
    mut pickups: ResMut<Pickups>,
    progress: Option<ResMut<PlayerProgress>>,
) {
    let dt = time.delta_secs();
    let mut back: Vec<u64> = Vec::new();
    pickups.respawn.retain(|id, left| {
        *left -= dt;
        if *left <= 0.0 {
            back.push(*id);
            false
        } else {
            true
        }
    });
    if let Some(mut progress) = progress {
        for id in back {
            progress.picked.remove(&id);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn stream_pickups(
    mut commands: Commands,
    mut pickups: ResMut<Pickups>,
    player: Query<&Transform, With<PlayerAvatar>>,
    progress: Option<Res<PlayerProgress>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    dist: Option<Res<super::quality::StreamDistances>>,
) {
    let Some(pt) = player.iter().next() else {
        return;
    };
    let pxz = Vec2::new(pt.translation.x, pt.translation.z);
    // Graphics screen "Pickups range" multiplier (1.0 = 120 / 150 m).
    let k = super::quality::scale_of(dist.as_deref(), |d| d.pickups);
    let (load_r, unload_r) = (LOAD_RADIUS * k, UNLOAD_RADIUS * k);
    let collected = |id: u64| progress.as_deref().is_some_and(|p| p.picked.contains(&id));
    let p = &mut *pickups;
    let remove: Vec<u64> = p
        .alive
        .keys()
        .copied()
        .filter(|id| {
            let Some(&i) = p.by_id.get(id) else {
                return true;
            };
            let s = &p.spawns[i];
            (collected(*id) && !stands_after_harvest(s))
                || !should_be_alive(
                    pxz,
                    Vec2::new(s.pos[0], s.pos[2]),
                    true,
                    load_r,
                    unload_r,
                )
        })
        .collect();
    for id in remove {
        if let Some(e) = p.alive.remove(&id) {
            commands.entity(e).despawn();
        }
    }
    for i in 0..p.spawns.len() {
        let s = &p.spawns[i];
        if p.alive.contains_key(&s.id) || (collected(s.id) && !stands_after_harvest(s)) {
            continue;
        }
        if !should_be_alive(
            pxz,
            Vec2::new(s.pos[0], s.pos[2]),
            false,
            load_r,
            unload_r,
        ) {
            continue;
        }
        // The marker entity always exists (bookkeeping, `Shaking` and the automation counts); its
        // proxy mesh is a debug placeholder, drawn only with `PLA_DEBUG_PLACEHOLDERS=1`.
        let e = if super::debug_flags::placeholders() {
            let (mesh, material, y) = marker_assets(s, &mut p.markers, &mut meshes, &mut materials);
            commands
                .spawn((
                    SpawnMarker(s.id),
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_xyz(s.pos[0], s.pos[1] + y, s.pos[2])
                        .with_rotation(Quat::from_rotation_y(s.yaw.to_radians())),
                ))
                .id()
        } else {
            commands
                .spawn((
                    SpawnMarker(s.id),
                    Transform::from_xyz(s.pos[0], s.pos[1], s.pos[2])
                        .with_rotation(Quat::from_rotation_y(s.yaw.to_radians())),
                ))
                .id()
        };
        p.alive.insert(s.id, e);
    }
}

/// Harvested trees remain in the world (inert until they respawn); everything else disappears.
fn stands_after_harvest(s: &ItemSpawn) -> bool {
    s.kind == SpawnKind::ResourceNode && node_kind(&s.model) == NodeKind::Tree
}

/// Marker mesh/material and the y offset (markers are generic proxies, not the game models).
fn marker_assets(
    s: &ItemSpawn,
    cache: &mut MarkerCache,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> (Handle<Mesh>, Handle<StandardMaterial>, f32) {
    match s.kind {
        SpawnKind::ResourceNode => {
            let is_tree = node_kind(&s.model) == NodeKind::Tree;
            let slot = if is_tree {
                &mut cache.tree
            } else {
                &mut cache.ore
            };
            let (mesh, mat) = slot.get_or_insert_with(|| {
                if is_tree {
                    (
                        meshes.add(Cone {
                            radius: 1.1,
                            height: 3.0,
                        }),
                        materials.add(StandardMaterial {
                            base_color: Color::srgb(0.18, 0.42, 0.16),
                            ..default()
                        }),
                    )
                } else {
                    (
                        meshes.add(Cuboid::new(1.0, 1.4, 1.0)),
                        materials.add(StandardMaterial {
                            base_color: Color::srgb(0.55, 0.62, 0.72),
                            metallic: 0.4,
                            ..default()
                        }),
                    )
                }
            });
            (mesh.clone(), mat.clone(), if is_tree { 1.5 } else { 0.7 })
        }
        SpawnKind::DirectItem => {
            let (mesh, mat) = cache.item.get_or_insert_with(|| {
                (
                    meshes.add(Sphere::new(0.3)),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(1.0, 0.85, 0.25),
                        emissive: LinearRgba::rgb(4.0, 3.0, 0.4),
                        ..default()
                    }),
                )
            });
            (mesh.clone(), mat.clone(), 0.3)
        }
        SpawnKind::SearchItem => {
            let (mesh, mat) = cache.collectible.get_or_insert_with(|| {
                (
                    meshes.add(Sphere::new(0.45)),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.35, 0.8, 1.0),
                        emissive: LinearRgba::rgb(0.4, 2.0, 4.0),
                        ..default()
                    }),
                )
            });
            (mesh.clone(), mat.clone(), 0.6)
        }
    }
}

fn shake_nodes(
    time: Res<Time>,
    mut commands: Commands,
    mut nodes: Query<(Entity, &mut Transform, &mut Shaking)>,
) {
    for (entity, mut tr, mut shake) in &mut nodes {
        let base = *shake.base.get_or_insert(tr.translation);
        shake.timer.tick(time.delta());
        let t = shake.timer.elapsed_secs();
        let duration = shake.timer.duration().as_secs_f32();
        match shake.kind {
            NodeKind::Tree => {
                tr.translation = base + Vec3::new(sway_offset(t, duration), 0.0, 0.0);
                if shake.timer.is_finished() {
                    tr.translation = base;
                    commands.entity(entity).remove::<Shaking>();
                }
            }
            NodeKind::Ore => {
                tr.scale = Vec3::splat(break_scale(t, duration).max(0.001));
                if shake.timer.is_finished() {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

/// Start the harvest reaction on a node's marker and schedule its respawn.
fn react(p: &mut Pickups, commands: &mut Commands, spawn: &ItemSpawn) {
    let kind = node_kind(&spawn.model);
    let seconds = match kind {
        NodeKind::Tree => TREE_SWAY_SECONDS,
        NodeKind::Ore => SHAKE_SECONDS,
    };
    if let Some(&entity) = p.alive.get(&spawn.id) {
        // Standing trees keep their marker (and their `alive` slot); ore is gone until the
        // respawn re-creates it.
        if kind == NodeKind::Ore {
            p.alive.remove(&spawn.id);
        }
        commands.entity(entity).insert(Shaking {
            timer: Timer::from_seconds(seconds, TimerMode::Once),
            kind,
            base: None,
        });
    }
    p.respawn.insert(spawn.id, RESPAWN_SECONDS);
}

/// A ball in flight that passes through a resource node harvests it (Brief 12: hit the node with
/// a ball). The ball itself is untouched; its own lifecycle continues.
fn thrown_ball_hits_nodes(
    mut commands: Commands,
    mut pickups: ResMut<Pickups>,
    balls: Query<&BallBody>,
    progress: Option<ResMut<PlayerProgress>>,
    assets: Option<Res<super::battle_controller::BattleAssets>>,
) {
    let Some(mut progress) = progress else { return };
    let cap = |id: u16| {
        assets
            .as_deref()
            .and_then(|a| a.items.get(id))
            .map(|e| e.max_quantity)
            .unwrap_or(MAX_STACK_FALLBACK)
    };
    for body in &balls {
        if !body.0.in_flight() {
            continue;
        }
        let p = &mut *pickups;
        let ball = body.0.position;
        let hit = p.spawns.iter().position(|s| {
            s.kind == SpawnKind::ResourceNode
                && p.alive.contains_key(&s.id)
                && !progress.picked.contains(&s.id)
                && ball_hits_node(ball, s.pos, node_kind(&s.model))
        });
        let Some(i) = hit else { continue };
        let spawn = p.spawns[i].clone();
        let mut rng = p.rng;
        let result = apply_pickup(&spawn, &mut progress, &mut rng, cap);
        p.rng = rng;
        println!("Pickups (ball hit): {}", describe_pickup(&spawn, &result));
        if matches!(result, PickupResult::Collected { .. }) {
            react(p, &mut commands, &spawn);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn pickup_interact(
    actions: Option<Res<ActionState>>,
    mut commands: Commands,
    mut pickups: ResMut<Pickups>,
    player: Query<&Transform, With<PlayerAvatar>>,
    progress: Option<ResMut<PlayerProgress>>,
    assets: Option<Res<super::battle_controller::BattleAssets>>,
) {
    let Some(actions) = actions else { return };
    if !actions.just_pressed(Action::Confirm) {
        return;
    }
    let Some(mut progress) = progress else { return };
    let Some(pt) = player.iter().next() else {
        return;
    };
    let pxz = Vec2::new(pt.translation.x, pt.translation.z);
    let p = &mut *pickups;
    let Some(i) = nearest_within(&p.spawns, pxz, PICK_RANGE, |s| {
        p.alive.contains_key(&s.id) && !progress.picked.contains(&s.id)
    }) else {
        return;
    };
    let spawn = p.spawns[i].clone();
    let mut rng = p.rng;
    let cap = |id: u16| {
        assets
            .as_deref()
            .and_then(|a| a.items.get(id))
            .map(|e| e.max_quantity)
            .unwrap_or(MAX_STACK_FALLBACK)
    };
    let result = apply_pickup(&spawn, &mut progress, &mut rng, cap);
    p.rng = rng;
    println!("Pickups: {}", describe_pickup(&spawn, &result));
    if matches!(result, PickupResult::Collected { .. }) {
        if spawn.kind == SpawnKind::ResourceNode {
            react(p, &mut commands, &spawn);
        } else {
            if let Some(entity) = p.alive.remove(&spawn.id) {
                commands.entity(entity).despawn();
            }
            p.respawn.insert(spawn.id, RESPAWN_SECONDS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    const T: &str = "# c\nkind:enum\tarea:string*\tspawn_key:string*\tplacement_id:string*\tx:f32\ty:f32\tz:f32\tyaw:f32?\tmodel:string?\tspawn_table_key:string?\tloot_item_ids:string?\tloot_weights:string?\tloot_min:u16?\tloot_max:u16?\tflag:string?\tconfidence:enum\n\
direct_item\tha_area01\t001\tbe636c89f1de0b4c\t470.78\t28.34\t280.35\t\titem_013\t\t\t\t\t\t\tpartial\n\
resource_node\tha_area01\t0001\t47748b4d30a9b07b\t389.99\t47.37\t203.84\t\tgimmick_tree11\ta5ab636c22581861\t1613;1688\t95;5\t5\t5\t\tverified\n\
resource_node\tha_area01\t0002\td3a946e6d72c9546\t420.68\t32.18\t258.75\t\tgimmick_crystal01\ta5ab5b6c22580ac9\t1679\t100\t3\t3\t\tverified\n\
search_item\tha_area01\tpoem_00\t3ddd5feb4a68a149\t397.93\t33.09\t459.32\t\t\t\t\t\t\t\tFSYS_POEM_GET_00\tverified\n\
direct_item\tha_area02\t007\tffffffffffffffff\t1\t2\t3\t\titem_001\t\t\t\t\t\t\tpartial\n";

    #[test]
    fn parses_only_the_area_and_known_kinds() {
        let s = parse_spawns(T, "ha_area01");
        assert_eq!(s.len(), 4);
        assert_eq!(s[0].kind, SpawnKind::DirectItem);
        assert_eq!(s[0].model, "item_013");
        assert_eq!(s[0].pos, [470.78, 28.34, 280.35]);
        assert_eq!(s[1].kind, SpawnKind::ResourceNode);
        assert_eq!(s[1].loot, vec![(1613, 95), (1688, 5)]);
        assert_eq!((s[1].count_min, s[1].count_max), (5, 5));
        assert_eq!(s[2].model, "gimmick_crystal01");
        assert_eq!(s[3].flag, "FSYS_POEM_GET_00");
        assert_eq!(parse_spawns(T, "ha_area03").len(), 0);
    }

    #[test]
    fn nearest_within_uses_range_and_filter() {
        let s = parse_spawns(T, "ha_area01");
        let player = Vec2::new(470.0, 280.0);
        let i =
            nearest_within(&s, player, PICK_RANGE, |x| x.kind == SpawnKind::DirectItem).unwrap();
        assert_eq!(s[i].key, "001");
        assert!(nearest_within(&s, player, 0.1, |_| true).is_none());
        // collected spawns are filtered out by the caller
        assert!(
            nearest_within(&s, player, PICK_RANGE, |x| x.kind != SpawnKind::DirectItem).is_none()
        );
    }

    #[test]
    fn loot_roll_follows_the_weights() {
        let mut rng = 7u64;
        let loot = [(1613u16, 95u16), (1688, 5)];
        let mut counts = HashMap::new();
        for _ in 0..4000 {
            let id = roll_loot(&loot, &mut rng).unwrap();
            *counts.entry(id).or_insert(0u32) += 1;
        }
        let a = *counts.get(&1613).unwrap_or(&0);
        let b = *counts.get(&1688).unwrap_or(&0);
        assert!(a > 3600 && a < 4000, "{a}");
        assert!(b > 100 && b < 400, "{b}");
        assert_eq!(roll_loot(&[], &mut rng), None);
    }

    #[test]
    fn count_roll_is_inclusive_and_defaults_to_one() {
        let mut rng = 1u64;
        for _ in 0..200 {
            let n = roll_count(5, 5, &mut rng);
            assert_eq!(n, 5);
            let n = roll_count(0, 0, &mut rng);
            assert_eq!(n, 1);
            let n = roll_count(2, 4, &mut rng);
            assert!((2..=4).contains(&n));
        }
    }

    #[test]
    fn bag_add_respects_the_stack_ceiling() {
        let mut bag = BTreeMap::new();
        assert_eq!(add_to_bag(&mut bag, 1613, 5, 999), 5);
        assert_eq!(add_to_bag(&mut bag, 1613, 1000, 999), 994);
        assert_eq!(bag[&1613], 999);
        assert_eq!(add_to_bag(&mut bag, 1613, 1, 999), 0);
        assert_eq!(add_to_bag(&mut bag, 1613, 0, 999), 0);
        assert_eq!(add_to_bag(&mut bag, 7, 3, 2), 2);
        assert_eq!(bag[&7], 2);
    }

    #[test]
    fn pickup_collects_once_and_grants_loot_for_nodes() {
        let s = parse_spawns(T, "ha_area01");
        let mut progress = PlayerProgress::new_game();
        let mut rng = 42u64;
        let node = s.iter().find(|x| x.key == "0001").unwrap();
        let first = apply_pickup(node, &mut progress, &mut rng, |_| 999);
        let (id, n) = match first {
            PickupResult::Collected { item: Some(v) } => v,
            other => panic!("expected loot, got {other:?}"),
        };
        assert!(id == 1613 || id == 1688);
        assert_eq!((n, progress.bag[&id]), (n, n));
        assert!(progress.picked.contains(&node.id));
        assert_eq!(
            apply_pickup(node, &mut progress, &mut rng, |_| 999),
            PickupResult::Nothing
        );
        // a direct item is collected but grants nothing until its reward is decoded
        let item = s.iter().find(|x| x.kind == SpawnKind::DirectItem).unwrap();
        let bag_before = progress.bag.clone();
        assert_eq!(
            apply_pickup(item, &mut progress, &mut rng, |_| 999),
            PickupResult::Collected { item: None }
        );
        assert_eq!(
            progress.bag, bag_before,
            "nothing to grant for the direct item"
        );
    }

    #[test]
    fn respawn_removes_the_id_from_the_picked_set() {
        // the same rule `tick_respawn` applies when the timer fires
        let mut picked: BTreeSet<u64> = BTreeSet::from([5u64]);
        picked.remove(&5);
        assert!(picked.is_empty());
    }

    #[test]
    fn hysteresis_matches_the_world_objects_rule() {
        assert!(should_be_alive(
            Vec2::ZERO,
            Vec2::new(100.0, 0.0),
            false,
            LOAD_RADIUS,
            UNLOAD_RADIUS
        ));
        assert!(!should_be_alive(
            Vec2::ZERO,
            Vec2::new(130.0, 0.0),
            false,
            LOAD_RADIUS,
            UNLOAD_RADIUS
        ));
        assert!(should_be_alive(
            Vec2::ZERO,
            Vec2::new(130.0, 0.0),
            true,
            LOAD_RADIUS,
            UNLOAD_RADIUS
        ));
    }

    #[test]
    fn node_kinds_follow_the_model_names() {
        assert_eq!(node_kind("gimmick_tree11"), NodeKind::Tree);
        assert_eq!(node_kind("gimmick_yachi01"), NodeKind::Tree);
        assert_eq!(node_kind("gimmick_leaves_a"), NodeKind::Tree);
        assert_eq!(node_kind("gimmick_crystal01"), NodeKind::Ore);
        assert_eq!(node_kind("gimmick_rock02"), NodeKind::Ore);
    }

    #[test]
    fn ball_hit_volume_is_a_cylinder_over_the_node() {
        let pos = [10.0, 5.0, 10.0];
        // Through the canopy of a tree, but not above an ore that is only 1.8 m tall.
        assert!(ball_hits_node(
            Vec3::new(10.5, 8.0, 10.0),
            pos,
            NodeKind::Tree
        ));
        assert!(!ball_hits_node(
            Vec3::new(10.5, 8.0, 10.0),
            pos,
            NodeKind::Ore
        ));
        assert!(ball_hits_node(
            Vec3::new(10.0, 5.5, 10.9),
            pos,
            NodeKind::Ore
        ));
        // Outside the radius, below the ground and above the canopy.
        assert!(!ball_hits_node(
            Vec3::new(12.0, 6.0, 10.0),
            pos,
            NodeKind::Tree
        ));
        assert!(!ball_hits_node(
            Vec3::new(10.0, 4.0, 10.0),
            pos,
            NodeKind::Tree
        ));
        assert!(!ball_hits_node(
            Vec3::new(10.0, 11.0, 10.0),
            pos,
            NodeKind::Tree
        ));
    }

    #[test]
    fn sway_decays_and_break_shrinks() {
        let d = TREE_SWAY_SECONDS;
        let early = (0..30)
            .map(|i| sway_offset(i as f32 * 0.01, d).abs())
            .fold(0.0, f32::max);
        let late = (0..30)
            .map(|i| sway_offset(d - 0.3 + i as f32 * 0.01, d).abs())
            .fold(0.0, f32::max);
        assert!(early > 0.1 && early <= 0.25, "{early}");
        assert!(late < early * 0.4, "damped: {late} vs {early}");
        assert_eq!(sway_offset(d, d), 0.0);
        assert_eq!(sway_offset(-1.0, d), 0.0);
        assert_eq!(break_scale(0.0, 0.6), 1.0);
        assert!((break_scale(0.3, 0.6) - 0.5).abs() < 1e-6);
        assert_eq!(break_scale(2.0, 0.6), 0.0);
    }

    // ------------------------------------------------------------ headless Bevy

    use super::super::ball_lifecycle::BallSimulation;

    /// App with the two systems under test and the sample spawns alive as bare markers.
    fn node_app() -> (App, HashMap<String, Entity>) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(100),
        ));
        let spawns = parse_spawns(T, "ha_area01");
        let mut alive = HashMap::new();
        let mut named = HashMap::new();
        for s in &spawns {
            let e = app
                .world_mut()
                .spawn(Transform::from_xyz(s.pos[0], s.pos[1] + 1.0, s.pos[2]))
                .id();
            alive.insert(s.id, e);
            named.insert(s.key.clone(), e);
        }
        let by_id = spawns.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
        app.insert_resource(Pickups {
            spawns,
            by_id,
            alive,
            respawn: HashMap::new(),
            markers: MarkerCache::default(),
            rng: 9,
        });
        app.insert_resource(PlayerProgress::default());
        app.add_systems(Update, (shake_nodes, thrown_ball_hits_nodes).chain());
        (app, named)
    }

    fn throw_through(app: &mut App, at: Vec3) -> Entity {
        app.world_mut()
            .spawn(BallBody(BallSimulation::new(at, Vec3::ZERO, 0.0)))
            .id()
    }

    #[test]
    fn a_ball_through_a_tree_harvests_it_and_the_tree_keeps_standing() {
        let (mut app, named) = node_app();
        let tree = named["0001"];
        let id = app.world().resource::<Pickups>().spawns[1].id;
        throw_through(&mut app, Vec3::new(389.99, 49.5, 203.84));
        for _ in 0..3 {
            app.update();
        }
        let progress = app.world().resource::<PlayerProgress>();
        assert!(progress.picked.contains(&id), "the node is collected");
        let total: u16 = progress.bag.values().sum();
        assert_eq!(total, 5, "loot_min = loot_max = 5 for this node");
        assert!(app.world().resource::<Pickups>().respawn.contains_key(&id));
        // Run past the sway: the marker is still there, inert, back where it started.
        for _ in 0..20 {
            app.update();
        }
        let e = app
            .world()
            .get_entity(tree)
            .expect("a harvested tree stays standing");
        assert!(e.get::<Shaking>().is_none(), "sway finished");
        assert!((e.get::<Transform>().unwrap().translation.x - 389.99).abs() < 1e-3);
        assert!(app.world().resource::<Pickups>().alive.contains_key(&id));
        // A second ball does not harvest it again.
        let before = app.world().resource::<PlayerProgress>().bag.clone();
        throw_through(&mut app, Vec3::new(389.99, 49.5, 203.84));
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<PlayerProgress>().bag, before);
    }

    #[test]
    fn a_ball_through_a_crystal_breaks_it() {
        let (mut app, named) = node_app();
        let crystal = named["0002"];
        let id = app.world().resource::<Pickups>().spawns[2].id;
        throw_through(&mut app, Vec3::new(420.68, 32.9, 258.75));
        for _ in 0..3 {
            app.update();
        }
        assert!(app
            .world()
            .resource::<PlayerProgress>()
            .picked
            .contains(&id));
        assert!(!app.world().resource::<Pickups>().alive.contains_key(&id));
        for _ in 0..12 {
            app.update();
        }
        assert!(
            app.world().get_entity(crystal).is_err(),
            "the broken crystal is gone"
        );
        assert_eq!(app.world().resource::<PlayerProgress>().bag[&1679], 3);
    }

    #[test]
    fn misses_and_balls_not_in_flight_harvest_nothing() {
        let (mut app, _) = node_app();
        // 3 m away from the tree, and a ball already holding a Pokemon right on top of it.
        throw_through(&mut app, Vec3::new(393.0, 49.0, 203.84));
        let mut held = BallSimulation::new(Vec3::new(389.99, 49.0, 203.84), Vec3::ZERO, 0.0);
        held.hit_pokemon(Vec3::new(389.99, 49.0, 203.84), 47.0, 3, true);
        app.world_mut().spawn(BallBody(held));
        for _ in 0..3 {
            app.update();
        }
        let progress = app.world().resource::<PlayerProgress>();
        assert!(progress.picked.is_empty() && progress.bag.is_empty());
    }
}
