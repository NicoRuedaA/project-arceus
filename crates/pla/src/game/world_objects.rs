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

/// Parse the placement sheet for the main area `area` (`ha_area00`), keeping rows whose glTF export is `ok`.
pub fn parse_placements(text: &str, area: &str) -> Vec<Placement> {
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
        if get("area") != area || get("model_status") != "ok" || get("model_name").is_empty() {
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

/// Map model name -> (filesystem PathBuf, relative asset path String) using the export index.
fn model_paths(cache_root: &Path, area: u8) -> HashMap<String, (PathBuf, String)> {
    let mut m = HashMap::new();
    if let Some(text) = find_data_file("field_model_export_index.tsv").and_then(|p| std::fs::read_to_string(p).ok()) {
        for line in text.lines().filter(|l| !l.starts_with('#')) {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() >= 4 && c[3] == "ok" {
                if let Some(rel) = c[2].strip_prefix("work/cache/") {
                    m.insert(c[1].to_string(), (cache_root.join(rel), format!("cache/{rel}")));
                }
            }
        }
    }
    let _ = area;
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

#[derive(Resource)]
struct WorldObjects {
    placements: Vec<Placement>,
    paths: HashMap<String, (PathBuf, String)>,
    alive: HashMap<usize, Entity>,
    handles: HashMap<String, Handle<Gltf>>,
    prims: HashMap<String, Prims>,
    /// Local geometry of the water models (bounds + XZ triangles), for the world water bodies.
    water_meshes: HashMap<String, WaterMesh>,
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
        let all = parse_placements(&text, &area);
        let paths = model_paths(&self.cache_root, self.area);
        let total = all.len();
        let overlap = all.iter().filter(|p| self.exclude_models.contains(&p.model)).count();
        let placements: Vec<Placement> = all
            .into_iter()
            .filter(|p| !self.exclude_models.contains(&p.model) && paths.get(&p.model).is_some_and(|(f, _)| f.exists()))
            .collect();
        println!(
            "World objects {area}: {total} ok placements, {overlap} overlap .trins models (skipped), {} with an exported glTF on disk.",
            placements.len()
        );
        app.insert_resource(WorldObjects {
            placements,
            paths,
            alive: HashMap::new(),
            handles: HashMap::new(),
            prims: HashMap::new(),
            water_meshes: HashMap::new(),
            rot_mode: self.rot_mode,
        });
        app.add_systems(Update, stream_objects);
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

#[derive(Component)]
#[allow(dead_code)]
struct PlacedObject(usize);

#[allow(clippy::too_many_arguments)]
fn stream_objects(
    mut commands: Commands,
    mut wo: ResMut<WorldObjects>,
    player: Query<&Transform, With<PlayerAvatar>>,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    gltf_materials: Res<Assets<GltfMaterial>>,
    meshes: Res<Assets<Mesh>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut water_materials: ResMut<Assets<WaterMaterial>>,
    mut water_bodies: Option<ResMut<WaterBodies>>,
) {
    let Some(pt) = player.iter().next() else { return };
    let pxz = Vec2::new(pt.translation.x, pt.translation.z);
    let wo = &mut *wo;
    let remove: Vec<usize> = wo
        .alive
        .keys()
        .copied()
        .filter(|&i| {
            let p = &wo.placements[i];
            !should_be_alive(pxz, Vec2::new(p.pos[0], p.pos[2]), true, LOAD_RADIUS, UNLOAD_RADIUS)
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
        if !should_be_alive(pxz, Vec2::new(p.pos[0], p.pos[2]), false, LOAD_RADIUS, UNLOAD_RADIUS) {
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
                    prims.push((prim.mesh.clone(), pm));
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
        let e = commands
            .spawn((PlacedObject(i), transform, Visibility::default()))
            .with_children(|c| {
                for (m, mat) in prims {
                    match mat {
                        PrimMaterial::Standard(mat) => {
                            c.spawn((Mesh3d(m), MeshMaterial3d(mat), Transform::IDENTITY));
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
}
