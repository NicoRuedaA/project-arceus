//! Placed scenery objects (houses, props, fences...) from `data/field_object_placements.tsv`,
//! streamed in and out by distance from the player.
//!
//! Only main-terrain areas (00..05) are used; sub-area frames are unknown and ignored. Rotation is
//! interpreted as degrees, yaw about Y taken from `rot_y` (UNVERIFIED, see world_viewer.md). Model
//! glTF scenes are shared (one handle per model) and released when no instance of a model is alive.
//!
//! Water planes (`ground_*water*`, the game's undecoded `Water` shader) go through
//! `game::water`: they get the `WaterMaterial` and their world footprint/level is recorded in
//! `WaterBodies` so the locomotion can decide swimming.
//!
//! Brief 8b: [`reload_world_objects`] swaps the table to the new area on a zone change
//! (`game::zone::apply_zone_change`): the plugin parses the start area's placements once, so
//! without it the new area would stream the old area's houses and props.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use bevy::camera::primitives::MeshAabb;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use bevy::gltf::{Gltf, GltfMaterial, GltfMesh};

use super::PlayerAvatar;
use super::water::{
    is_water_model, water_body_from_mesh, water_material, WaterBodies, WaterExt, WaterMaterial,
    WaterMesh,
};

/// Objects closer than this to the player are alive.
pub const LOAD_RADIUS: f32 = 200.0;
/// Objects farther than this are removed (hysteresis avoids flicker at the border).
pub const UNLOAD_RADIUS: f32 = 230.0;
/// Default cache root (the main checkout; worktrees do not hold `work/`).
pub const DEFAULT_CACHE_ROOT: &str = "/home/nico/work/decompilacion/work/cache";

#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub model: String,
    pub pos: [f32; 3],
    pub rot_deg: [f32; 3],
    pub scale: [f32; 3],
}

/// Parse the placement sheet for the main area `area` (`ha_area00`), keeping rows whose glTF export
/// is `ok` in the sheet's own `model_status` snapshot (collision and report callers). The spawner
/// uses [`parse_placements_for_spawn`], where the export index is authoritative instead.
pub fn parse_placements(text: &str, area: &str) -> Vec<Placement> {
    parse_placements_impl(text, area, true)
}

/// Spawn variant of [`parse_placements`]: every row of the main area with a resolved model name,
/// regardless of the sheet's `model_status`.
///
/// `field_object_placements.tsv` copies the export status known when it was generated and lags
/// `field_model_export_index.tsv`: 378 main-area placements are `blocked` there while the index
/// exports the same model `ok` (e.g. `objects_area00_house01_a`, the Jubilife houses). For
/// spawning, a valid export is authoritative, so the streamability filter is
/// [`keep_streamable_placements`] (ok index row + file on disk), not this snapshot.
///
/// Rows that remain skipped after that filter: the 7 main-area models the index also blocks
/// (`wall_area00_02`, `wall_area01_water01_asase`, `wall_area01_kingtree01`, `wall_area02_floor01`,
/// `objects_oldtextplate07`, `wall_area03_water01_asase`, `objects_oldtextplate06`), all with
/// "no valid bounded .trmsh vertex-layout section; shape layout: no declared shapes", plus any
/// `ok` index row whose file is missing on disk.
pub fn parse_placements_for_spawn(text: &str, area: &str) -> Vec<Placement> {
    parse_placements_impl(text, area, false)
}

fn parse_placements_impl(text: &str, area: &str, require_sheet_ok: bool) -> Vec<Placement> {
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
        if get("area") != area || get("model_name").is_empty() {
            continue;
        }
        if require_sheet_ok && get("model_status") != "ok" {
            continue;
        }
        let f = |n: &str, d: f32| get(n).parse::<f32>().unwrap_or(d);
        out.push(Placement {
            model: get("model_name").to_string(),
            pos: [f("x", 0.0), f("y", 0.0), f("z", 0.0)],
            rot_deg: [f("rot_x", 0.0), f("rot_y", 0.0), f("rot_z", 0.0)],
            scale: [f("scale_x", 1.0), f("scale_y", 1.0), f("scale_z", 1.0)],
        });
    }
    dedupe_variants(out)
}

/// Strip a trailing `_a`..`_d` state-variant suffix.
fn base_model(name: &str) -> &str {
    match name.rsplit_once('_') {
        Some((b, s)) if s.len() == 1 && s.as_bytes()[0].is_ascii_lowercase() && s.as_bytes()[0] <= b'd' => b,
        _ => name,
    }
}

/// Drop placements at the origin (0,0,0: unplaced template rows) and keep a single entry per
/// (base model, position within 0.5 m): the sheet lists village-state variants (`_a`/`_b`) and exact
/// duplicates on the same spot; which variant is the default is unknown, the first listed is kept.
pub fn dedupe_variants(rows: Vec<Placement>) -> Vec<Placement> {
    let mut seen: HashSet<(String, i32, i32, i32)> = HashSet::new();
    let q = |v: f32| (v * 2.0).round() as i32;
    rows.into_iter()
        .filter(|p| p.pos != [0.0, 0.0, 0.0])
        .filter(|p| seen.insert((base_model(&p.model).to_string(), q(p.pos[0]), q(p.pos[1]), q(p.pos[2]))))
        .collect()
}

/// Placements that can be streamed: model not already placed by the `.trins` trees
/// (`exclude_models`) and with an exported glTF on disk (`paths`). Shared by the plugin build and
/// the zone-change reload so both filter the sheet the same way.
pub fn keep_streamable_placements(
    rows: Vec<Placement>,
    exclude_models: &HashSet<String>,
    paths: &HashMap<String, (PathBuf, String)>,
) -> Vec<Placement> {
    rows.into_iter()
        .filter(|p| !exclude_models.contains(&p.model) && paths.get(&p.model).is_some_and(|(f, _)| f.exists()))
        .collect()
}

/// Which rotation interpretation to use (kept switchable for visual verification).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotMode {
    /// Yaw about Y = rot_y degrees (counter-clockwise seen from +Y).
    YawY,
    /// Yaw about Y = -rot_y degrees.
    YawYNeg,
    /// Yaw about Y = rot_y + 180 degrees.
    YawYFlip,
}

pub fn placement_rotation(p: &Placement, mode: RotMode) -> Quat {
    let y = p.rot_deg[1].to_radians();
    let y = match mode {
        RotMode::YawY => y,
        RotMode::YawYNeg => -y,
        RotMode::YawYFlip => y + std::f32::consts::PI,
    };
    Quat::from_rotation_y(y)
}

/// Squared horizontal distance test with hysteresis: `alive` objects stay until `unload`, others need `load`.
pub fn should_be_alive(player_xz: Vec2, obj_xz: Vec2, alive: bool, load: f32, unload: f32) -> bool {
    let d = player_xz.distance(obj_xz);
    if alive { d <= unload } else { d <= load }
}

/// Value following `flag` in args, if any.
fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

pub fn rot_mode_from_args(args: &[String]) -> RotMode {
    match arg_value(args, "--obj-rot").as_deref() {
        Some("neg") => RotMode::YawYNeg,
        Some("flip") => RotMode::YawYFlip,
        _ => RotMode::YawY,
    }
}

pub fn find_data_file(name: &str) -> Option<PathBuf> {
    [PathBuf::from("data"), PathBuf::from("../../data"), PathBuf::from("/home/nico/work/decompilacion/data")]
        .into_iter()
        .map(|d| d.join(name))
        .find(|p| p.exists())
}

/// The `work/cache/`-relative tail of one `output` column of `data/field_model_export_index.tsv`.
///
/// The exporter writes absolute paths inside this repo's `work/cache`
/// (`/home/nico/work/decompilacion/work/cache/gltf_field/...`); older rows may hold a legacy
/// `work/cache/...`-relative one. Both spellings yield the same tail (e.g.
/// `gltf_field/ha_area00/objects_area00_house01_a.gltf`); a stored path that does not sit under a
/// `work/cache/` directory yields `None`.
pub fn cache_relative_export_path(stored: &str) -> Option<&str> {
    const MARKER: &str = "work/cache/";
    let rel = match stored.strip_prefix(MARKER) {
        Some(rel) => rel,
        None => {
            // Absolute row: strip through the LAST `work/cache/` so a path pointing at the main
            // checkout re-roots onto `cache_root` (a worktree's `work/` resolves to the same tree).
            let at = stored.rfind(MARKER)? + MARKER.len();
            &stored[at..]
        }
    };
    (!rel.is_empty()).then_some(rel)
}

/// Resolve one `output` column of `data/field_model_export_index.tsv` against `cache_root`:
/// `(cache_root.join(rel), "cache/<rel>")` — the absolute file path plus the asset-server path
/// (the asset root is `work/`). `None` when the stored path has no `work/cache/` tail; the caller
/// skips the row.
pub fn normalize_export_path(cache_root: &Path, stored: &str) -> Option<(PathBuf, String)> {
    let rel = cache_relative_export_path(stored)?;
    Some((cache_root.join(rel), format!("cache/{rel}")))
}

/// Map model name -> (filesystem PathBuf, relative asset path String) using the export index:
/// every `ok` row resolved onto `cache_root` (see [`normalize_export_path`]). This is the table
/// [`keep_streamable_placements`] checks; the placed-object collision shares it so both agree on
/// which models exist.
pub fn model_paths(cache_root: &Path) -> HashMap<String, (PathBuf, String)> {
    let mut m = HashMap::new();
    if let Some(text) = find_data_file("field_model_export_index.tsv").and_then(|p| std::fs::read_to_string(p).ok()) {
        for line in text.lines().filter(|l| !l.starts_with('#')) {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() >= 4 && c[3] == "ok" {
                if let Some(pair) = normalize_export_path(cache_root, c[2]) {
                    m.insert(c[1].to_string(), pair);
                }
            }
        }
    }
    m
}

/// Plugin streaming placed objects of main area `area` (0..=5) around the player.
pub struct WorldObjectsPlugin {
    pub area: u8,
    pub cache_root: PathBuf,
    /// Model names already placed from `.trins` (skipped here to avoid duplicates).
    pub exclude_models: HashSet<String>,
    pub rot_mode: RotMode,
}

/// Active placed-object state (crate-visible: the zone change driver swaps its table).
#[derive(Resource)]
pub(crate) struct WorldObjects {
    placements: Vec<Placement>,
    paths: HashMap<String, (PathBuf, String)>,
    alive: HashMap<usize, Entity>,
    handles: HashMap<String, Handle<Gltf>>,
    prims: HashMap<String, Prims>,
    /// Local geometry of the water models (bounds + XZ triangles), for the world water bodies.
    water_meshes: HashMap<String, WaterMesh>,
    /// Models whose skinned primitive had to be skipped (one log line each).
    skip_warned: HashSet<String>,
    rot_mode: RotMode,
}

/// One primitive of a model and the material it must be drawn with.
#[derive(Clone)]
enum PrimMaterial {
    Standard(Handle<StandardMaterial>),
    Water(Handle<WaterMaterial>),
}

type Prims = Vec<(Handle<Mesh>, PrimMaterial)>;

impl Plugin for WorldObjectsPlugin {
    fn build(&self, app: &mut App) {
        if self.area > 5 {
            return;
        }
        let area = format!("ha_area{:02}", self.area);
        let Some(text) = find_data_file("field_object_placements.tsv").and_then(|p| std::fs::read_to_string(p).ok()) else {
            println!("World objects: data/field_object_placements.tsv not found; skipping.");
            return;
        };
        let all = parse_placements_for_spawn(&text, &area);
        let paths = model_paths(&self.cache_root);
        let total = all.len();
        let overlap = all.iter().filter(|p| self.exclude_models.contains(&p.model)).count();
        let placements = keep_streamable_placements(all, &self.exclude_models, &paths);
        println!(
            "World objects {area}: {total} sheet placements, {overlap} overlap .trins models (skipped), {} with an exported glTF on disk.",
            placements.len()
        );
        app.insert_resource(WorldObjects {
            placements,
            paths,
            alive: HashMap::new(),
            handles: HashMap::new(),
            prims: HashMap::new(),
            water_meshes: HashMap::new(),
            skip_warned: HashSet::new(),
            rot_mode: self.rot_mode,
        });
        app.add_systems(Update, stream_objects);
    }
}

/// Swap the streamed placed objects to main area `area_idx` after a zone change (Brief 8b).
///
/// Despawns every live object (the primitive children go with it), forgets the old area's water
/// bodies, and installs the new area's table filtered with `exclude_models` (the new area's
/// `.trins` models) exactly like `WorldObjectsPlugin::build`. Areas outside 00..=05 get an empty
/// table, the same set the plugin parses at startup. Returns the number of placements installed,
/// or `None` when the plugin is not installed in `world`.
pub fn reload_world_objects(
    world: &mut World,
    area_idx: u8,
    exclude_models: &HashSet<String>,
) -> Option<usize> {
    let old: Vec<Entity> = {
        let mut wo = world.get_resource_mut::<WorldObjects>()?;
        wo.handles.clear();
        wo.prims.clear();
        wo.water_meshes.clear();
        wo.alive.drain().map(|(_, e)| e).collect()
    };
    for entity in old {
        // Despawns the primitive children as well (ChildOf linked-despawn).
        world.despawn(entity);
    }
    // The recorded surfaces belong to the old area's frame: keep them and a new-area player would
    // "swim" on dry land until the new area's water streams in.
    if let Some(mut bodies) = world.get_resource_mut::<WaterBodies>() {
        bodies.bodies.clear();
        bodies.dropped = 0;
    }
    let placements = if area_idx <= 5 {
        let area = format!("ha_area{area_idx:02}");
        let text = find_data_file("field_object_placements.tsv")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_else(|| {
                eprintln!("World objects: placement sheet not found on a zone change; {area} stays empty");
                String::new()
            });
        let wo = world.resource::<WorldObjects>();
        keep_streamable_placements(parse_placements_for_spawn(&text, &area), exclude_models, &wo.paths)
    } else {
        Vec::new()
    };
    let count = placements.len();
    world.resource_mut::<WorldObjects>().placements = placements;
    println!("World objects ha_area{area_idx:02}: {count} placements installed for the zone change");
    Some(count)
}

/// Number of placements of the currently installed area (diagnostics and tests); `None` when the
/// plugin is not installed.
pub fn installed_placement_count(world: &World) -> Option<usize> {
    world.get_resource::<WorldObjects>().map(|w| w.placements.len())
}

#[cfg(test)]
impl WorldObjects {
    /// Table with `placements` but no export paths and nothing alive: a zone-change test can then
    /// see the reload clear it (every placement is filtered out without `paths`).
    pub fn for_tests(placements: Vec<Placement>, rot_mode: RotMode) -> Self {
        Self {
            placements,
            paths: HashMap::new(),
            alive: HashMap::new(),
            handles: HashMap::new(),
            prims: HashMap::new(),
            water_meshes: HashMap::new(),
            skip_warned: HashSet::new(),
            rot_mode,
        }
    }
}

/// Local XZ triangles of a mesh (the y is dropped: water surfaces are horizontal planes).
fn mesh_xz_triangles(mesh: &Mesh) -> Vec<[Vec2; 3]> {
    let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        return Vec::new();
    };
    let Some(indices) = mesh.indices() else { return Vec::new() };
    let indices: Vec<usize> = indices.iter().collect();
    let mut out = Vec::with_capacity(indices.len() / 3);
    for tri in indices.chunks(3) {
        if let [i0, i1, i2] = tri {
            let p = |i: usize| positions.get(i).map(|v| Vec2::new(v[0], v[2]));
            if let (Some(a), Some(b), Some(c)) = (p(*i0), p(*i1), p(*i2)) {
                out.push([a, b, c]);
            }
        }
    }
    out
}

/// The mesh handle a static placed-object primitive is drawn with.
///
/// Bevy picks the **skinned** mesh pipeline and bind group from the mesh layout alone
/// (`is_skinned`: `JOINT_INDEX` + `JOINT_WEIGHT` attributes). A placed object is spawned as a
/// plain `Mesh3d` without `SkinnedMesh` or skin uniforms, so a skinned glTF primitive (the area 00
/// flag props `objects_area00_house04_b_flag` / `_house11_flag`, the only 2 of the 393 streamable
/// placed models) bound the model-only bind group against the skinned layout and the render
/// pipeline failed wgpu validation, quitting the viewer — latent until the placed objects started
/// spawning at all. The props do not animate in the port, so a copy without the joint attributes
/// is correct and keeps them visible; `handle` itself is returned when the mesh is already static
/// and `None` when its data cannot be read (a `RENDER_WORLD`-only asset), which makes the caller
/// skip the primitive with one log line.
fn static_placed_mesh(meshes: &mut Assets<Mesh>, handle: &Handle<Mesh>) -> Option<Handle<Mesh>> {
    let stripped = {
        let mesh = meshes.get(handle)?;
        let skinned = [Mesh::ATTRIBUTE_JOINT_INDEX, Mesh::ATTRIBUTE_JOINT_WEIGHT]
            .into_iter()
            .any(|attribute| mesh.try_contains_attribute(attribute).unwrap_or(false));
        if !skinned {
            return Some(handle.clone());
        }
        let mut copy = mesh.clone();
        for attribute in [Mesh::ATTRIBUTE_JOINT_INDEX, Mesh::ATTRIBUTE_JOINT_WEIGHT] {
            if copy.try_contains_attribute(attribute).unwrap_or(false)
                && copy.try_remove_attribute(attribute).is_err()
            {
                return None;
            }
        }
        copy
    };
    Some(meshes.add(stripped))
}

/// A live placed object; `0` indexes the area's placement set (the same order the collision
/// grid uses, see `WorldCollision::segment_blocked_placements`). The battle camera hides the
/// objects that block its view through this index.
#[derive(Component)]
pub struct PlacedObject(pub usize);

#[allow(clippy::too_many_arguments)]
fn stream_objects(
    mut commands: Commands,
    mut wo: ResMut<WorldObjects>,
    player: Query<&Transform, With<PlayerAvatar>>,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    gltf_materials: Res<Assets<GltfMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut water_materials: ResMut<Assets<WaterMaterial>>,
    mut water_bodies: Option<ResMut<WaterBodies>>,
    dist: Option<Res<super::quality::StreamDistances>>,
) {
    let Some(pt) = player.iter().next() else { return };
    let pxz = Vec2::new(pt.translation.x, pt.translation.z);
    // Graphics screen "Objects range" multiplier (1.0 = 200 / 230 m).
    let k = super::quality::scale_of(dist.as_deref(), |d| d.objects);
    let (load_r, unload_r) = (LOAD_RADIUS * k, UNLOAD_RADIUS * k);
    let wo = &mut *wo;
    let remove: Vec<usize> = wo
        .alive
        .keys()
        .copied()
        .filter(|&i| {
            let p = &wo.placements[i];
            !should_be_alive(pxz, Vec2::new(p.pos[0], p.pos[2]), true, load_r, unload_r)
        })
        .collect();
    for i in remove {
        if let Some(e) = wo.alive.remove(&i) {
            commands.entity(e).despawn();
        }
    }
    let mut pending: HashSet<String> = HashSet::new();
    for i in 0..wo.placements.len() {
        if wo.alive.contains_key(&i) {
            continue;
        }
        let p = wo.placements[i].clone();
        if !should_be_alive(pxz, Vec2::new(p.pos[0], p.pos[2]), false, load_r, unload_r) {
            continue;
        }
        let Some((_, rel_path)) = wo.paths.get(&p.model).cloned() else { continue };
        let handle = wo
            .handles
            .entry(p.model.clone())
            .or_insert_with(|| asset_server.load(rel_path))
            .clone();
        // Build the shared mesh/material list once the glTF and all child materials have finished loading.
        if !wo.prims.contains_key(&p.model) {
            pending.insert(p.model.clone());
            let Some(gltf) = gltfs.get(&handle) else { continue };

            // Ensure all primitive materials are ready before caching to avoid transient grey fallback.
            let all_mats_ready = gltf.meshes.iter().all(|mh| {
                if let Some(gm) = gltf_meshes.get(mh) {
                    gm.primitives.iter().all(|prim| {
                        if let Some(ref mh) = prim.material {
                            gltf_materials.contains(mh)
                        } else {
                            true
                        }
                    })
                } else {
                    false
                }
            });
            if !all_mats_ready {
                continue;
            }

            let mut prims = Vec::new();
            let is_water = is_water_model(&p.model);
            let mut water_mesh = WaterMesh::default();
            let mut has_water_geometry = false;
            for mh in &gltf.meshes {
                let Some(gm) = gltf_meshes.get(mh) else { continue };
                for prim in &gm.primitives {
                    let is_plant = p.model.starts_with("plants_");
                    let pm = if is_water {
                        // Water planes use the water shader (game::water) instead of a flat colour.
                        if let Some(m) = meshes.get(&prim.mesh) {
                            if let Some(a) = m.compute_aabb() {
                                let (lo, hi) = (Vec3::from(a.min()), Vec3::from(a.max()));
                                water_mesh.local_min = if has_water_geometry { water_mesh.local_min.min(lo) } else { lo };
                                water_mesh.local_max = if has_water_geometry { water_mesh.local_max.max(hi) } else { hi };
                                has_water_geometry = true;
                            }
                            water_mesh.triangles.extend(mesh_xz_triangles(m));
                        }
                        PrimMaterial::Water(water_materials.add(water_material(WaterExt::default())))
                    } else {
                        let mat = prim.material.as_ref().and_then(|h| gltf_materials.get(h));
                        let sm = match mat {
                            Some(m) => StandardMaterial {
                                base_color: m.base_color,
                                base_color_texture: m.base_color_texture.clone(),
                                perceptual_roughness: m.perceptual_roughness,
                                metallic: m.metallic,
                                cull_mode: None,
                                alpha_mode: if is_plant {
                                    AlphaMode::Mask(0.5)
                                } else {
                                    AlphaMode::Opaque
                                },
                                ..default()
                            },
                            None => StandardMaterial {
                                base_color: Color::srgb(0.6, 0.6, 0.6),
                                cull_mode: None,
                                ..default()
                            },
                        };
                        PrimMaterial::Standard(standard_materials.add(sm))
                    };
                    // A skinned primitive cannot be drawn as a plain static `Mesh3d` (see the
                    // helper); the props do not animate, so a stripped copy is drawn instead.
                    let Some(mesh) = static_placed_mesh(&mut meshes, &prim.mesh) else {
                        if wo.skip_warned.insert(p.model.clone()) {
                            eprintln!(
                                "World objects: primitive '{}' of {} skipped (mesh data not readable as a static mesh; cannot guarantee a non-skinned layout)",
                                gm.name, p.model
                            );
                        }
                        continue;
                    };
                    prims.push((mesh, pm));
                }
            }
            if has_water_geometry {
                wo.water_meshes.insert(p.model.clone(), water_mesh);
            }
            wo.prims.insert(p.model.clone(), prims);
        }
        let prims = wo.prims[&p.model].clone();
        let transform = Transform {
            translation: Vec3::from(p.pos),
            rotation: placement_rotation(&p, wo.rot_mode),
            scale: Vec3::from(p.scale),
        };
        // Record where the water surface is so the locomotion can start swimming there.
        if let Some(mesh) = wo.water_meshes.get(&p.model) {
            if let Some(bodies) = water_bodies.as_deref_mut() {
                let body = water_body_from_mesh(mesh, &transform);
                if !bodies.insert(body) {
                    println!(
                        "Water: body of {} not recorded ({} bodies is the cap, {} dropped)",
                        p.model,
                        WaterBodies::MAX_BODIES,
                        bodies.dropped
                    );
                }
            }
        }
        let cliff = super::terrain_splat::is_cliff_model(&p.model);
        let e = commands
            .spawn((PlacedObject(i), transform, Visibility::default()))
            .with_children(|c| {
                for (m, mat) in prims {
                    match mat {
                        PrimMaterial::Standard(mat) => {
                            let mut child = c.spawn((Mesh3d(m), MeshMaterial3d(mat), Transform::IDENTITY));
                            if cliff {
                                child.insert(super::terrain_splat::CliffSurface);
                            }
                        }
                        PrimMaterial::Water(mat) => {
                            c.spawn((Mesh3d(m), MeshMaterial3d(mat), Transform::IDENTITY));
                        }
                    }
                }
            })
            .id();
        wo.alive.insert(i, e);
    }
    // Release shared handles/materials of models with no live instance.
    let live: HashSet<String> = wo.alive.keys().map(|&i| wo.placements[i].model.clone()).collect();
    wo.handles.retain(|m, _| live.contains(m) || pending.contains(m));
    wo.prims.retain(|m, _| live.contains(m));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    const T: &str = "# c\narea:string*\tinstance_id:string*\tobject_key:string\tmodel_hash:string\tmodel_name:string?\tmodel_status:enum\tx:f32\ty:f32\tz:f32\trot_x:f32?\trot_y:f32?\trot_z:f32?\tscale_x:f32?\tscale_y:f32?\tscale_z:f32?\tgate_flags:string?\n\
ha_area00\ta\tk\th\tobjects_a\tok\t1\t2\t3\t\t90\t\t1.2\t1.2\t1.2\t\n\
ha_area00\tb\tk\th\tobjects_b\tblocked\t1\t2\t3\t\t\t\t\t\t\t\n\
ha_area00_s01\tc\tk\th\tobjects_c\tok\t1\t2\t3\t\t\t\t\t\t\t\n\
ha_area01\td\tk\th\tobjects_d\tok\t1\t2\t3\t\t\t\t\t\t\t\n";

    #[test]
    fn parses_only_ok_main_area() {
        let p = parse_placements(T, "ha_area00");
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].model, "objects_a");
        assert_eq!(p[0].pos, [1.0, 2.0, 3.0]);
        assert_eq!(p[0].rot_deg, [0.0, 90.0, 0.0]);
        assert_eq!(p[0].scale, [1.2, 1.2, 1.2]);
    }

    #[test]
    fn missing_scale_defaults_to_one() {
        let p = parse_placements(T, "ha_area01");
        assert_eq!(p[0].scale, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn load_radius_with_hysteresis() {
        let p = Vec2::ZERO;
        assert!(should_be_alive(p, Vec2::new(150.0, 0.0), false, 200.0, 230.0));
        assert!(!should_be_alive(p, Vec2::new(215.0, 0.0), false, 200.0, 230.0));
        assert!(should_be_alive(p, Vec2::new(215.0, 0.0), true, 200.0, 230.0));
        assert!(!should_be_alive(p, Vec2::new(240.0, 0.0), true, 200.0, 230.0));
    }

    #[test]
    fn dedupes_variants_and_drops_origin() {
        let mk = |m: &str, x: f32| Placement { model: m.into(), pos: [x, 1.0, 1.0], rot_deg: [0.0; 3], scale: [1.0; 3] };
        let v = dedupe_variants(vec![mk("h_a", 5.0), mk("h_b", 5.2), mk("h_a", 5.0), mk("h_a", 0.0), mk("h_a", 50.0)]);
        assert_eq!(v.len(), 3); // x=5 (one of three), x=0 (y,z != 0 so not the origin), x=50
        let o = Placement { model: "o".into(), pos: [0.0; 3], rot_deg: [0.0; 3], scale: [1.0; 3] };
        assert!(dedupe_variants(vec![o]).is_empty());
    }

    #[test]
    fn yaw_modes() {
        let p = Placement { model: "m".into(), pos: [0.0; 3], rot_deg: [0.0, 90.0, 0.0], scale: [1.0; 3] };
        let a = placement_rotation(&p, RotMode::YawY) * Vec3::X;
        let b = placement_rotation(&p, RotMode::YawYNeg) * Vec3::X;
        assert!((a + b).length() < 1e-5);
    }

    #[test]
    fn keep_streamable_filters_by_exclusion_and_disk() {
        let rows = parse_placements(T, "ha_area00");
        assert_eq!(rows.len(), 1, "only the ok row of the area is parsed");
        let dir = std::env::temp_dir().join(format!("wo_stream_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("objects_a.gltf");
        std::fs::write(&file, "{}").unwrap();
        let mut paths = HashMap::new();
        paths.insert("objects_a".to_string(), (file, "cache/objects_a.gltf".to_string()));
        assert_eq!(keep_streamable_placements(rows.clone(), &HashSet::new(), &paths).len(), 1);
        let mut excluded = HashSet::new();
        excluded.insert("objects_a".to_string());
        assert!(
            keep_streamable_placements(rows.clone(), &excluded, &paths).is_empty(),
            "a model the .trins trees place is skipped"
        );
        let mut missing = HashMap::new();
        missing.insert(
            "objects_a".to_string(),
            (dir.join("absent.gltf"), "cache/absent.gltf".to_string()),
        );
        assert!(
            keep_streamable_placements(rows, &HashSet::new(), &missing).is_empty(),
            "no exported glTF on disk: not streamable"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The export index stores absolute paths (current exporter) or legacy `work/cache/`-relative
    /// ones; both must resolve onto the cache root, and an absolute row must land on the exact
    /// glTF the index points at with the asset-server path the viewer loads.
    #[test]
    fn normalizes_absolute_and_relative_export_rows() {
        let root = Path::new("/tmp/some-cache-root");
        let abs_row =
            "/home/nico/work/decompilacion/work/cache/gltf_field/ha_area00/objects_area00_house01_a.gltf";
        let expected = Some((
            root.join("gltf_field/ha_area00/objects_area00_house01_a.gltf"),
            "cache/gltf_field/ha_area00/objects_area00_house01_a.gltf".to_string(),
        ));
        assert_eq!(normalize_export_path(root, abs_row), expected);
        assert_eq!(
            normalize_export_path(
                root,
                "work/cache/gltf_field/ha_area00/objects_area00_house01_a.gltf"
            ),
            expected,
            "the legacy relative spelling resolves to the same file and asset path"
        );
        assert!(normalize_export_path(root, "/other/place/model.gltf").is_none());
        assert!(normalize_export_path(root, "").is_none());
    }

    /// Regression for the absolute-path bug: with the real committed index and the git-ignored
    /// cache present (developer machine), the Jubilife house placements, `blocked` in the stale
    /// placement sheet but exported `ok`, must resolve to files on disk and become streamable.
    /// Returns early where the cache is absent so a data-less checkout stays green.
    #[test]
    fn real_index_resolves_the_jubilife_houses() {
        let cache_root = Path::new(DEFAULT_CACHE_ROOT);
        let (Some(index), Some(sheet)) = (
            find_data_file("field_model_export_index.tsv"),
            find_data_file("field_object_placements.tsv"),
        ) else {
            return;
        };
        if !cache_root.is_dir() {
            return;
        }
        let index_text = std::fs::read_to_string(index).unwrap();
        let paths: HashMap<String, (PathBuf, String)> = index_text
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                (c.len() >= 4 && c[3] == "ok")
                    .then(|| normalize_export_path(cache_root, c[2]).map(|p| (c[1].to_string(), p)))
                    .flatten()
            })
            .collect();
        let sheet_text = std::fs::read_to_string(sheet).unwrap();
        let rows = parse_placements_for_spawn(&sheet_text, "ha_area00");
        assert!(
            rows.iter().any(|p| p.model == "objects_area00_house01_a"),
            "the spawn parse keeps the sheet-blocked house row"
        );
        let streamable = keep_streamable_placements(rows, &HashSet::new(), &paths);
        let houses = streamable
            .iter()
            .filter(|p| p.model.starts_with("objects_area00_house"))
            .count();
        assert!(
            houses > 0,
            "the exported house models must resolve on disk ({houses} kept)"
        );
        let (path, asset) = normalize_export_path(
            cache_root,
            "/home/nico/work/decompilacion/work/cache/gltf_field/ha_area00/objects_area00_house01_a.gltf",
        )
        .unwrap();
        assert!(path.is_file(), "{} missing", path.display());
        assert_eq!(
            asset,
            "cache/gltf_field/ha_area00/objects_area00_house01_a.gltf"
        );
    }

    /// A skinned placed-object primitive must be drawn as a static mesh: Bevy picks the skinned
    /// pipeline and bind group from `JOINTS_0`/`WEIGHTS_0` alone, and a placed object is spawned
    /// without `SkinnedMesh` or skin uniforms, so the model-only bind group failed wgpu validation
    /// and quit the viewer (the area 00 flag props; latent until the placed objects spawned at
    /// all). The stripped copy keeps every other attribute and the original asset is untouched.
    #[test]
    fn skinned_primitives_are_stripped_for_static_placement() {
        let mut meshes = Assets::<Mesh>::default();
        let source = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; 3])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; 3])
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_JOINT_INDEX,
            VertexAttributeValues::Uint16x4(vec![[0u16, 0, 0, 0]; 3]),
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_JOINT_WEIGHT,
            VertexAttributeValues::Float32x4(vec![[1.0f32, 0.0, 0.0, 0.0]; 3]),
        )
        .with_inserted_indices(Indices::U32(vec![0, 1, 2]));
        let handle = meshes.add(source);
        let static_handle = static_placed_mesh(&mut meshes, &handle).expect("stripped copy");
        assert_ne!(static_handle, handle, "the shared glTF mesh is left alone");
        let stripped = meshes.get(&static_handle).expect("copy in the assets");
        assert!(!stripped.contains_attribute(Mesh::ATTRIBUTE_JOINT_INDEX));
        assert!(!stripped.contains_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT));
        assert!(stripped.contains_attribute(Mesh::ATTRIBUTE_POSITION));
        assert!(stripped.contains_attribute(Mesh::ATTRIBUTE_NORMAL));
        assert!(stripped.contains_attribute(Mesh::ATTRIBUTE_UV_0));
        assert_eq!(stripped.indices(), Some(&Indices::U32(vec![0, 1, 2])));
        assert!(meshes
            .get(&handle)
            .unwrap()
            .contains_attribute(Mesh::ATTRIBUTE_JOINT_INDEX));
        // A static mesh keeps its handle; a missing asset is refused so the caller can skip it.
        let plain = meshes.add(Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        ));
        assert_eq!(static_placed_mesh(&mut meshes, &plain), Some(plain));
        assert!(static_placed_mesh(&mut meshes, &Handle::<Mesh>::default()).is_none());
    }

    /// Real-data scan of the skinned placed props: of the streamable models across all areas,
    /// exactly the two area 00 flag props carry `JOINTS_0`/`WEIGHTS_0` (and declare `skins`), and
    /// every skinned primitive keeps POSITION/NORMAL/UV, so stripping the joints leaves a complete
    /// static mesh (see `skinned_primitives_are_stripped_for_static_placement`). Skips without the
    /// git-ignored cache.
    #[test]
    fn real_skinned_placed_models_are_strip_safe() {
        let cache_root = Path::new(DEFAULT_CACHE_ROOT);
        let Some(sheet) = find_data_file("field_object_placements.tsv") else {
            return;
        };
        if find_data_file("field_model_export_index.tsv").is_none() || !cache_root.is_dir() {
            return;
        }
        let paths = model_paths(cache_root);
        let sheet_text = std::fs::read_to_string(sheet).unwrap();
        let mut models: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for area in 0..=5u8 {
            let name = format!("ha_area{area:02}");
            for p in keep_streamable_placements(
                parse_placements_for_spawn(&sheet_text, &name),
                &HashSet::new(),
                &paths,
            ) {
                models.insert(p.model);
            }
        }
        let mut skinned = Vec::new();
        for model in &models {
            let Some((path, _)) = paths.get(model) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let has_skins = json
                .get("skins")
                .and_then(|s| s.as_array())
                .is_some_and(|s| !s.is_empty());
            for mesh in json["meshes"].as_array().into_iter().flatten() {
                for prim in mesh["primitives"].as_array().into_iter().flatten() {
                    let attrs = &prim["attributes"];
                    if attrs.get("JOINTS_0").is_none() && attrs.get("WEIGHTS_0").is_none() {
                        continue;
                    }
                    skinned.push((
                        model.clone(),
                        has_skins,
                        attrs.get("POSITION").is_some(),
                        attrs.get("NORMAL").is_some(),
                        attrs.get("TEXCOORD_0").is_some(),
                    ));
                }
            }
        }
        assert!(
            skinned
                .iter()
                .any(|(m, ..)| m == "objects_area00_house04_b_flag")
                && skinned
                    .iter()
                    .any(|(m, ..)| m == "objects_area00_house11_flag"),
            "the two skinned flag props must be in the streamable set: {skinned:?}"
        );
        for (model, has_skins, position, normal, uv) in &skinned {
            assert!(
                *has_skins,
                "{model}: JOINTS_0/WEIGHTS_0 without a skin declaration"
            );
            assert!(
                *position && *normal && *uv,
                "{model}: a skinned primitive lacks POSITION/NORMAL/TEXCOORD_0"
            );
        }
    }

    /// A `blocked` sheet row whose model the index exports `ok` must spawn; a model blocked in the
    /// index too (no path entry) stays out.
    #[test]
    fn spawn_parse_treats_a_valid_export_as_authoritative() {
        const S: &str = "# c\narea:string*\tinstance_id:string*\tobject_key:string\tmodel_hash:string\tmodel_name:string?\tmodel_status:enum\tx:f32\ty:f32\tz:f32\trot_x:f32?\trot_y:f32?\trot_z:f32?\tscale_x:f32?\tscale_y:f32?\tscale_z:f32?\tgate_flags:string?\n\
ha_area00\ta\tk\th\tobjects_stale\tblocked\t1\t2\t3\t\t\t\t\t\t\t\n\
ha_area00\tb\tk\th\tobjects_blocked\tblocked\t4\t2\t5\t\t\t\t\t\t\t\n";
        let dir = std::env::temp_dir().join(format!("wo_spawn_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("objects_stale.gltf");
        std::fs::write(&file, "{}").unwrap();
        let mut paths = HashMap::new();
        paths.insert(
            "objects_stale".to_string(),
            (file, "cache/objects_stale.gltf".to_string()),
        );
        assert!(
            parse_placements(S, "ha_area00").is_empty(),
            "the sheet's own snapshot drops both blocked rows"
        );
        let rows = parse_placements_for_spawn(S, "ha_area00");
        assert_eq!(rows.len(), 2, "the spawn parse keeps blocked rows");
        let streamable = keep_streamable_placements(rows, &HashSet::new(), &paths);
        assert_eq!(streamable.len(), 1);
        assert_eq!(
            streamable[0].model, "objects_stale",
            "the ok export is authoritative; the index-blocked model stays out"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reload_despawns_the_old_area_and_installs_the_new_table() {
        let mut world = World::new();
        let live = world.spawn_empty().id();
        let child = world.spawn(ChildOf(live)).id();
        let unrelated = world.spawn_empty().id();
        let stale = Placement {
            model: "objects_old".into(),
            pos: [1.0, 2.0, 3.0],
            rot_deg: [0.0; 3],
            scale: [1.0; 3],
        };
        let mut wo = WorldObjects::for_tests(vec![stale], RotMode::YawY);
        wo.alive.insert(0, live);
        world.insert_resource(wo);
        let mut bodies = WaterBodies::default();
        bodies.insert(crate::game::water::WaterBody {
            min: Vec2::ZERO,
            max: Vec2::new(10.0, 10.0),
            level: 1.0,
            triangles: vec![],
        });
        world.insert_resource(bodies);

        let count = reload_world_objects(&mut world, 1, &HashSet::new()).expect("plugin installed");
        assert_eq!(count, 0, "the test table has no export paths: nothing is streamable");
        assert!(world.get_entity(live).is_err(), "old placed object despawned");
        assert!(world.get_entity(child).is_err(), "its primitives went with it");
        assert!(world.get_entity(unrelated).is_ok());
        assert!(world.resource::<WorldObjects>().alive.is_empty());
        assert_eq!(installed_placement_count(&world), Some(0));
        assert!(
            world.resource::<WaterBodies>().bodies.is_empty(),
            "the old area's water bodies are forgotten"
        );
    }

    #[test]
    fn reload_without_the_plugin_is_a_no_op() {
        let mut world = World::new();
        assert!(reload_world_objects(&mut world, 1, &HashSet::new()).is_none());
        assert!(installed_placement_count(&world).is_none());
    }
}
