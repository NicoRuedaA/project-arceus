//! Shared real-model support for NPCs and trainer markers.
//!
//! The exporter (`examples/npc_export.rs`) writes one skinned glTF per character id to
//! `<cache>/npc/<id>/npc.gltf`. This module answers "is there a model for this id and how tall is
//! it", spawns the glTF scene lazily (one asset handle per character id, shared by every
//! instance), grounds it from the glTF position bounds and plays the exported `idle` clip through
//! one animation graph per glTF. A missing or unreadable export falls back to the caller's
//! capsule. Clothing, hair and skin colours are SUPPOSED/unknown: the textures' own colours are
//! used and nothing is tinted.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle, AnimationNodeIndex};
use bevy::animation::transition::AnimationTransitions;
use bevy::animation::AnimationPlayer;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;

/// The entity root is the capsule centre; the exported model has its feet at the glTF origin.
pub const MODEL_ROOT_ABOVE_GROUND: f32 = 0.9;
/// Model scenes instantiated per frame at most (spreads the spawn cost of a crowded square).
pub const SCENES_PER_FRAME: usize = 4;
/// Heights outside this range (m) are not a standing person: the export is treated as unusable.
pub const PLAUSIBLE_HEIGHT_M: std::ops::RangeInclusive<f32> = 0.4..=3.5;

/// Vertical extent of an exported model, measured from the glTF POSITION accessor bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelBounds {
    pub min_y: f32,
    pub max_y: f32,
}

impl ModelBounds {
    pub fn height(&self) -> f32 {
        self.max_y - self.min_y
    }
}

/// Union of the POSITION accessor `min`/`max` of every mesh primitive in a `.gltf` JSON document.
pub fn parse_gltf_bounds(json: &[u8]) -> Option<ModelBounds> {
    let root: serde_json::Value = serde_json::from_slice(json).ok()?;
    let accessors = root.get("accessors")?.as_array()?;
    let mut bounds: Option<ModelBounds> = None;
    for mesh in root.get("meshes")?.as_array()? {
        for primitive in mesh.get("primitives")?.as_array()? {
            let index = primitive.get("attributes")?.get("POSITION")?.as_u64()? as usize;
            let accessor = accessors.get(index)?;
            let lo = accessor.get("min")?.as_array()?.get(1)?.as_f64()? as f32;
            let hi = accessor.get("max")?.as_array()?.get(1)?.as_f64()? as f32;
            if !lo.is_finite() || !hi.is_finite() {
                return None;
            }
            bounds = Some(match bounds {
                Some(b) => ModelBounds { min_y: b.min_y.min(lo), max_y: b.max_y.max(hi) },
                None => ModelBounds { min_y: lo, max_y: hi },
            });
        }
    }
    bounds
}

/// Local Y of the scene root so the lowest vertex touches the ground under an entity root that
/// sits `MODEL_ROOT_ABOVE_GROUND` above it.
pub fn scene_offset_y(bounds: ModelBounds, scale: f32) -> f32 {
    -MODEL_ROOT_ABOVE_GROUND - bounds.min_y * scale
}

/// One usable export: its asset path (relative to `work/`) and measured bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelInfo {
    pub asset_path: String,
    pub bounds: ModelBounds,
}

/// Where the export of `id` lives, when it exists.
pub fn model_gltf_path(cache_root: &Path, id: &str) -> Option<PathBuf> {
    if id.is_empty() || id.contains(['/', '\\']) || id == ".." {
        return None;
    }
    let file = cache_root.join("npc").join(id).join("npc.gltf");
    file.is_file().then_some(file)
}

/// Check the export of `id` and measure it (None: no file, unreadable or implausible height).
pub fn load_model_info(cache_root: &Path, work_root: &Path, id: &str) -> Option<ModelInfo> {
    let file = model_gltf_path(cache_root, id)?;
    let bounds = parse_gltf_bounds(&std::fs::read(&file).ok()?)?;
    if !PLAUSIBLE_HEIGHT_M.contains(&bounds.height()) {
        return None;
    }
    // `work_root` is the canonical `work/`; a relative cache root (`work/cache`) is also accepted.
    let rel = file.strip_prefix(work_root).or_else(|_| file.strip_prefix("work")).unwrap_or(&file);
    Some(ModelInfo { asset_path: rel.to_string_lossy().replace('\\', "/"), bounds })
}

/// Memoised per-id answers: the file system and the glTF JSON are read once per character id.
#[derive(Resource, Debug)]
pub struct NpcModelCache {
    entries: HashMap<String, Option<ModelInfo>>,
    /// `PLA_NPC_MODELS=0` forces the capsules (before/after measurements, broken exports).
    disabled: bool,
}

impl FromWorld for NpcModelCache {
    fn from_world(_: &mut World) -> Self {
        Self { entries: HashMap::new(), disabled: std::env::var("PLA_NPC_MODELS").is_ok_and(|v| v == "0") }
    }
}

impl NpcModelCache {
    pub fn info(&mut self, cache_root: &Path, id: &str) -> Option<ModelInfo> {
        if self.disabled {
            return None;
        }
        self.entries
            .entry(id.to_owned())
            .or_insert_with(|| {
                let work_root = PathBuf::from("work").canonicalize().unwrap_or_else(|_| PathBuf::from("work"));
                load_model_info(cache_root, &work_root, id)
            })
            .clone()
    }

    pub fn known(&self) -> usize {
        self.entries.len()
    }

    pub fn with_model(&self) -> usize {
        self.entries.values().filter(|v| v.is_some()).count()
    }
}

/// Request: render this exported glTF under the entity (NPC or trainer marker).
#[derive(Component, Debug, Clone)]
pub struct NpcModelSource {
    pub gltf: Handle<Gltf>,
    pub bounds: ModelBounds,
    pub scale: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct NpcSceneSpawned;

/// Marks that the capsule fallback replaced a model whose load failed.
#[derive(Component, Debug, Clone, Copy)]
pub struct NpcFallbackSpawned;

#[derive(Component, Debug, Clone)]
#[allow(dead_code)]
pub struct NpcAnimState {
    pub player: Entity,
    pub idle_node: AnimationNodeIndex,
}

/// Ask the asset server for the model (shared handle per path).
pub fn model_source(asset_server: &AssetServer, info: &ModelInfo, scale: f32) -> NpcModelSource {
    NpcModelSource { gltf: asset_server.load(info.asset_path.clone()), bounds: info.bounds, scale }
}

/// Neutral capsule child used when no export exists (or it failed to load).
pub fn spawn_capsule(
    entity: &mut EntityCommands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    color: Color,
) {
    entity.with_children(|parent| {
        parent.spawn((
            Mesh3d(meshes.add(Capsule3d::new(0.3, 1.0))),
            MeshMaterial3d(materials.add(StandardMaterial { base_color: color, perceptual_roughness: 0.8, ..default() })),
            Transform::from_xyz(0.0, -MODEL_ROOT_ABOVE_GROUND, 0.0),
        ));
    });
}

/// One animation graph (idle clip) per glTF, shared by every instance.
#[derive(Resource, Default)]
struct IdleGraphs(HashMap<AssetId<Gltf>, Option<(Handle<AnimationGraph>, AnimationNodeIndex)>>);

fn spawn_model_scenes(
    mut commands: Commands,
    gltfs: Option<Res<Assets<Gltf>>>,
    q: Query<(Entity, &NpcModelSource), Without<NpcSceneSpawned>>,
) {
    let Some(gltfs) = gltfs else { return };
    let mut spawned = 0;
    for (entity, source) in &q {
        if spawned >= SCENES_PER_FRAME {
            break;
        }
        let Some(gltf) = gltfs.get(&source.gltf) else { continue };
        let Some(scene) = gltf.default_scene.clone().or_else(|| gltf.scenes.first().cloned()) else { continue };
        spawned += 1;
        commands.entity(entity).insert(NpcSceneSpawned).with_children(|parent| {
            parent.spawn((
                WorldAssetRoot(scene),
                Transform::from_xyz(0.0, scene_offset_y(source.bounds, source.scale), 0.0)
                    .with_scale(Vec3::splat(source.scale)),
                Visibility::default(),
            ));
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn wire_model_animation(
    mut commands: Commands,
    gltfs: Option<Res<Assets<Gltf>>>,
    graphs: Option<ResMut<Assets<AnimationGraph>>>,
    mut shared: ResMut<IdleGraphs>,
    q: Query<(Entity, &NpcModelSource), (With<NpcSceneSpawned>, Without<NpcAnimState>)>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let (Some(gltfs), Some(mut graphs)) = (gltfs, graphs) else { return };
    for (entity, source) in &q {
        let Some(gltf) = gltfs.get(&source.gltf) else { continue };
        let Some(player_entity) = children.iter_descendants(entity).find(|d| players.get(*d).is_ok()) else {
            continue;
        };
        let entry = shared.0.entry(source.gltf.id()).or_insert_with(|| {
            gltf.named_animations.get("idle").cloned().map(|idle| {
                let (graph, node) = AnimationGraph::from_clip(idle);
                (graphs.add(graph), node)
            })
        });
        let Some((graph_handle, idle_node)) = entry.clone() else {
            // No idle clip in this export: mark done so the query stops matching.
            commands.entity(entity).try_insert(NpcAnimState { player: player_entity, idle_node: AnimationNodeIndex::default() });
            continue;
        };
        if let Ok(mut player) = players.get_mut(player_entity) {
            let mut transitions = AnimationTransitions::new();
            transitions.play(&mut player, idle_node, std::time::Duration::ZERO).repeat();
            commands.entity(player_entity).insert((AnimationGraphHandle(graph_handle), transitions));
            commands.entity(entity).try_insert(NpcAnimState { player: player_entity, idle_node });
        }
    }
}

/// A glTF that fails to load is replaced by the grey capsule.
fn fallback_failed_models(
    mut commands: Commands,
    asset_server: Option<Res<AssetServer>>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    q: Query<(Entity, &NpcModelSource), (Without<NpcSceneSpawned>, Without<NpcFallbackSpawned>)>,
) {
    let (Some(asset_server), Some(mut meshes), Some(mut materials)) = (asset_server, meshes, materials) else {
        return;
    };
    for (entity, source) in &q {
        if !matches!(asset_server.load_state(&source.gltf), bevy::asset::LoadState::Failed(_)) {
            continue;
        }
        commands.entity(entity).try_remove::<NpcModelSource>().try_insert(NpcFallbackSpawned);
        let mut entity_commands = commands.entity(entity);
        spawn_capsule(&mut entity_commands, &mut meshes, &mut materials, Color::srgb(0.35, 0.5, 0.58));
    }
}

/// Registers the shared scene / animation / fallback systems (idempotent per app).
pub struct NpcModelPlugin;

impl Plugin for NpcModelPlugin {
    fn is_unique(&self) -> bool {
        false
    }

    fn build(&self, app: &mut App) {
        if app.world().contains_resource::<NpcModelCache>() {
            return;
        }
        app.init_resource::<NpcModelCache>()
            .init_resource::<IdleGraphs>()
            .add_systems(Update, (spawn_model_scenes, wire_model_animation, fallback_failed_models).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLTF: &str = r#"{"accessors":[
        {"type":"VEC3","min":[-0.1,0.006,-0.1],"max":[0.1,0.35,0.2]},
        {"type":"VEC3","min":[-0.5,0.9,-0.1],"max":[0.5,1.62,0.2]},
        {"type":"VEC3","min":[0,0,0],"max":[1,1,1]}],
        "meshes":[{"primitives":[{"attributes":{"POSITION":0}},{"attributes":{"POSITION":1}}]}]}"#;

    #[test]
    fn bounds_are_the_union_of_position_accessors_of_mesh_primitives() {
        let b = parse_gltf_bounds(GLTF.as_bytes()).unwrap();
        assert!((b.min_y - 0.006).abs() < 1e-6 && (b.max_y - 1.62).abs() < 1e-6);
        assert!((b.height() - 1.614).abs() < 1e-5);
    }

    #[test]
    fn bounds_reject_documents_without_positions() {
        assert!(parse_gltf_bounds(b"{}").is_none());
        assert!(parse_gltf_bounds(b"not json").is_none());
        assert!(parse_gltf_bounds(br#"{"accessors":[{"type":"VEC3"}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}]}"#).is_none());
    }

    #[test]
    fn grounding_puts_the_lowest_vertex_on_the_ground() {
        let b = ModelBounds { min_y: 0.2, max_y: 1.8 };
        // entity root is 0.9 above the ground: feet = 0.9 + offset + min_y * scale = 0
        assert!((MODEL_ROOT_ABOVE_GROUND + scene_offset_y(b, 1.0) + b.min_y).abs() < 1e-6);
        let sunk = ModelBounds { min_y: -0.3, max_y: 1.3 };
        assert!((MODEL_ROOT_ABOVE_GROUND + scene_offset_y(sunk, 0.8) + sunk.min_y * 0.8).abs() < 1e-6);
    }

    #[test]
    fn lookup_falls_back_when_the_export_is_missing_bad_or_implausible() {
        let dir = std::env::temp_dir().join(format!("npc_model_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let make = |id: &str, text: &str| {
            let d = dir.join("npc").join(id);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("npc.gltf"), text).unwrap();
        };
        make("good", GLTF);
        make("broken", "{");
        make("tiny", r#"{"accessors":[{"min":[0,0,0],"max":[1,0.1,1]}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}]}"#);
        let work = dir.clone();
        let good = load_model_info(&dir, &work, "good").unwrap();
        assert_eq!(good.asset_path, "npc/good/npc.gltf");
        assert!(load_model_info(&dir, &work, "broken").is_none());
        assert!(load_model_info(&dir, &work, "tiny").is_none());
        assert!(load_model_info(&dir, &work, "absent").is_none());
        assert!(load_model_info(&dir, &work, "../good").is_none());
        let mut cache = NpcModelCache { entries: HashMap::new(), disabled: false };
        assert!(cache.info(&dir, "good").is_some());
        assert!(cache.info(&dir, "absent").is_none());
        assert_eq!((cache.known(), cache.with_model()), (2, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
