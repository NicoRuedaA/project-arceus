//! Streaming vegetation (grass cards, flowers, shrubs, small stones) around the player.
//!
//! Data: the terrain descriptor's plant table + the `*_detailmap_K.trtbf` RGBA masks (see
//! `assets/vegetation.rs` and `reports/data-progress/vegetation.md`). The plant -> mask channel
//! mapping, instance density, scale ranges and tints are ASSUMPTIONS documented there.
//!
//! Rendering: the world is cut into 64-texel (~32 m) chunks. For every (chunk, plant) inside the
//! plant's streaming radius all instances are merged on the CPU into ONE mesh (one draw call per
//! chunk x plant x glTF primitive, with per-instance tint baked in vertex colours). Materials are
//! double-sided alpha-mask `StandardMaterial`s and the meshes cast no shadows. No wind (a vertex
//! animation would need a custom shader; not done).
//!
//! Budgets: `GrassQuality` sets radii, a hard cap on live instances and a cap on instances generated
//! per frame. `--grass off|low|high` (default low).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::gltf::{Gltf, GltfMaterial, GltfMesh};
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy::tasks::{futures_lite::future, AsyncComputeTaskPool, Task};

use super::{ActiveZoneInfo, PlayerAvatar};
use crate::assets::vegetation::*;

/// Hysteresis (m) added to a plant's radius before its chunk is released.
pub const UNLOAD_MARGIN: f32 = 12.0;
const SEED: u64 = 0x5EED;

pub struct VegetationPlugin {
    pub area: u8,
    pub bin_root: PathBuf,
    pub cache_root: PathBuf,
    pub quality: GrassQuality,
}

/// Live numbers for HUDs and tests.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct VegetationStats {
    pub chunks: usize,
    pub draw_calls: usize,
    pub instances: usize,
    pub triangles: u64,
    pub built_this_frame: usize,
}

/// Vegetation settings. `quality` can be changed at run time (graphics screen): the streaming
/// state is rebuilt on the next frame by [`veg_requality`].
#[derive(Resource)]
pub struct VegetationConfig {
    area: u8,
    bin_root: PathBuf,
    cache_root: PathBuf,
    pub quality: GrassQuality,
    /// Quality the live chunks and densities were built with.
    applied: GrassQuality,
}

fn grass_on(cfg: Res<VegetationConfig>) -> bool {
    cfg.quality != GrassQuality::Off
}

/// Quality changed at run time: drop every chunk and the per-plant densities, so the area is
/// prepared again with the new budgets (or stays empty for `Off`).
fn veg_requality(
    mut commands: Commands,
    mut cfg: ResMut<VegetationConfig>,
    mut st: ResMut<VegetationState>,
    mut stats: ResMut<VegetationStats>,
) {
    if cfg.applied == cfg.quality {
        return;
    }
    cfg.applied = cfg.quality;
    for (_, chunk) in st.alive.drain() {
        for e in chunk.entities {
            commands.entity(e).despawn();
        }
    }
    *st = VegetationState::default();
    *stats = VegetationStats::default();
}

/// Everything computed off-thread when an area is loaded.
struct Prepared {
    data: VegetationArea,
    coverage: Vec<(u32, u32, Vec<u32>)>,
}

struct PrimData {
    mesh: Mesh,
    material: Handle<StandardMaterial>,
}

#[derive(Default)]
struct PlantRuntime {
    model: Option<String>,
    gltf: Option<Handle<Gltf>>,
    prims: Vec<PrimData>,
    tris: u32,
    density: f32,
    radius: f32,
    done: bool,
}

#[derive(Resource, Default)]
struct VegetationState {
    area: Option<u8>,
    loading: Option<Task<Result<Prepared, String>>>,
    data: Option<Arc<VegetationArea>>,
    coverage: Vec<(u32, u32, Vec<u32>)>,
    plants: Vec<PlantRuntime>,
    alive: HashMap<(u32, u32, u16), AliveChunk>,
    live_instances: usize,
}

struct AliveChunk {
    entities: Vec<Entity>,
    instances: usize,
    tris: u64,
}

#[derive(Component)]
pub struct VegetationChunk;

impl Plugin for VegetationPlugin {
    fn build(&self, app: &mut App) {
        if self.quality == GrassQuality::Off {
            println!("Vegetation: grass off, not streaming plants (can be switched on in the Graphics screen).");
        }
        println!(
            "Vegetation: quality {:?}, radii (grass cards, other) = {:?} m, live cap {} instances.",
            self.quality,
            self.quality.radii(),
            self.quality.max_live_instances()
        );
        app.insert_resource(VegetationConfig {
            area: self.area,
            bin_root: self.bin_root.clone(),
            cache_root: self.cache_root.clone(),
            quality: self.quality,
            applied: self.quality,
        })
        .init_resource::<VegetationState>()
        .init_resource::<VegetationStats>()
        .add_systems(
            Update,
            (
                veg_requality,
                (veg_track_area, veg_poll_load, veg_poll_models, veg_stream)
                    .chain()
                    .run_if(grass_on),
            )
                .chain(),
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Pure helpers (unit tested)
// ---------------------------------------------------------------------------------------------

/// Distance from `p` to the axis-aligned square `[x0, x0+size] x [z0, z0+size]`.
pub fn dist_to_square(p: Vec2, x0: f32, z0: f32, size: f32) -> f32 {
    let dx = (x0 - p.x).max(0.0).max(p.x - (x0 + size));
    let dz = (z0 - p.y).max(0.0).max(p.y - (z0 + size));
    (dx * dx + dz * dz).sqrt()
}

/// Chunk-plant keys `(cx, cz, plant)` with vegetation inside the plant's radius around `player`,
/// nearest first. `coverage[plant] = (chunks_x, chunks_z, set texels per chunk)`.
pub fn wanted_chunks(
    player: Vec2,
    chunk_m: f32,
    coverage: &[(u32, u32, Vec<u32>)],
    radius: &[f32],
) -> Vec<((u32, u32, u16), f32)> {
    let mut out = Vec::new();
    for (pi, (cw, ch, counts)) in coverage.iter().enumerate() {
        let r = radius.get(pi).copied().unwrap_or(0.0);
        if r <= 0.0 || counts.is_empty() {
            continue;
        }
        let c0 = ((player.x - r) / chunk_m).floor().max(0.0) as u32;
        let c1 = (((player.x + r) / chunk_m).floor().max(0.0) as u32).min(cw.saturating_sub(1));
        let r0 = ((player.y - r) / chunk_m).floor().max(0.0) as u32;
        let r1 = (((player.y + r) / chunk_m).floor().max(0.0) as u32).min(ch.saturating_sub(1));
        if player.x + r < 0.0 || player.y + r < 0.0 {
            continue;
        }
        for cz in r0..=r1 {
            for cx in c0..=c1 {
                if counts[(cz * cw + cx) as usize] == 0 {
                    continue;
                }
                let d = dist_to_square(player, cx as f32 * chunk_m, cz as f32 * chunk_m, chunk_m);
                if d <= r {
                    out.push(((cx, cz, pi as u16), d));
                }
            }
        }
    }
    out.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    out
}

/// Merge `instances` of the model `src` into one mesh in area coordinates. `ground_y(x, z)` gives
/// the terrain height; tints are blended per instance and stored as vertex colours.
pub fn merge_instances(
    src: &Mesh,
    instances: &[PlantInstance],
    ground_y: impl Fn(f32, f32) -> f32,
    tint_a: [f32; 4],
    tint_b: [f32; 4],
) -> Option<Mesh> {
    let Some(VertexAttributeValues::Float32x3(pos)) = src.attribute(Mesh::ATTRIBUTE_POSITION) else {
        return None;
    };
    let normals = match src.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(n)) if n.len() == pos.len() => Some(n),
        _ => None,
    };
    let uvs = match src.attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(u)) if u.len() == pos.len() => Some(u),
        _ => None,
    };
    let idx: Vec<u32> = match src.indices()? {
        Indices::U16(v) => v.iter().map(|&i| i as u32).collect(),
        Indices::U32(v) => v.clone(),
    };
    if pos.is_empty() || idx.is_empty() || instances.is_empty() {
        return None;
    }
    let n = instances.len();
    let mut p_out = Vec::with_capacity(n * pos.len());
    let mut n_out = Vec::with_capacity(n * pos.len());
    let mut u_out = Vec::with_capacity(n * pos.len());
    let mut c_out = Vec::with_capacity(n * pos.len());
    let mut i_out = Vec::with_capacity(n * idx.len());
    for inst in instances {
        let base = p_out.len() as u32;
        let rot = Quat::from_rotation_y(inst.yaw);
        let scale = Vec3::new(inst.scale_h, inst.scale_v, inst.scale_h);
        let origin = Vec3::new(inst.x, ground_y(inst.x, inst.z), inst.z);
        let t = inst.tint;
        let color = [
            tint_a[0] + (tint_b[0] - tint_a[0]) * t,
            tint_a[1] + (tint_b[1] - tint_a[1]) * t,
            tint_a[2] + (tint_b[2] - tint_a[2]) * t,
            1.0,
        ];
        for (k, p) in pos.iter().enumerate() {
            p_out.push((origin + rot * (Vec3::from(*p) * scale)).to_array());
            let nrm = normals.map_or(Vec3::Y, |n| Vec3::from(n[k]));
            // inverse-transpose of a (yaw x non-uniform scale) matrix
            let nrm = (rot * (nrm / scale)).normalize_or(Vec3::Y);
            n_out.push(nrm.to_array());
            // The exported glTF keeps the game's bottom-left UV origin (grass cards: v = 0 at the
            // base, blades drawn at the BOTTOM of the png); glTF/Bevy expect top-left, so flip v.
            u_out.push(uvs.map_or([0.0, 0.0], |u| [u[k][0], 1.0 - u[k][1]]));
            c_out.push(color);
        }
        i_out.extend(idx.iter().map(|i| base + i));
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, p_out);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n_out);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, u_out);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, c_out);
    mesh.insert_indices(Indices::U32(i_out));
    Some(mesh)
}

fn mesh_triangles(m: &Mesh) -> u32 {
    m.indices().map_or(0, |i| i.len() as u32 / 3)
}

fn model_asset_path(cache_root: &std::path::Path, area: u8, model: &str) -> Option<String> {
    let mut dirs = vec![format!("ha_area{area:02}_terrain"), format!("ha_area{area:02}")];
    if let Ok(rd) = std::fs::read_dir(cache_root.join("gltf_field")) {
        let mut others: Vec<String> = rd
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with("_terrain") && !dirs.contains(n))
            .collect();
        others.sort();
        dirs.extend(others);
    }
    dirs.into_iter().find_map(|d| {
        let rel = format!("gltf_field/{d}/{model}.gltf");
        cache_root.join(&rel).exists().then(|| format!("cache/{rel}"))
    })
}

// ---------------------------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------------------------

fn veg_track_area(
    mut commands: Commands,
    cfg: Res<VegetationConfig>,
    zone: Option<Res<ActiveZoneInfo>>,
    mut st: ResMut<VegetationState>,
    mut stats: ResMut<VegetationStats>,
) {
    let want = zone.and_then(|z| z.area_idx()).unwrap_or(cfg.area);
    if st.area == Some(want) {
        return;
    }
    // Area changed (or first frame): drop everything and start loading in the background.
    for (_, chunk) in st.alive.drain() {
        for e in chunk.entities {
            commands.entity(e).despawn();
        }
    }
    *st = VegetationState { area: Some(want), ..default() };
    *stats = VegetationStats::default();
    if want > 5 {
        println!("Vegetation: area {want:02} has no detail maps in the main-area set; skipped.");
        return;
    }
    let bin_root = cfg.bin_root.clone();
    st.loading = Some(AsyncComputeTaskPool::get_or_init(Default::default).spawn(async move {
        let data = load_vegetation_area(&bin_root, want)?;
        let mut coverage = Vec::new();
        for i in 0..data.descriptor.plants.len() {
            coverage.push(match data.mask_for(i) {
                Some((m, ch)) => chunk_coverage(m, ch),
                None => (0, 0, Vec::new()),
            });
        }
        Ok(Prepared { data, coverage })
    }));
}

fn veg_poll_load(
    cfg: Res<VegetationConfig>,
    asset_server: Res<AssetServer>,
    mut st: ResMut<VegetationState>,
) {
    let Some(task) = st.loading.as_mut() else { return };
    let Some(result) = future::block_on(future::poll_once(task)) else { return };
    st.loading = None;
    let area = st.area.unwrap_or(0);
    let prepared = match result {
        Ok(p) => p,
        Err(e) => {
            println!("Vegetation: area {area:02} failed to load: {e}");
            return;
        }
    };
    // Resolve models; blocked (skinned) ones borrow a model of the same family.
    let names: Vec<String> = prepared.data.descriptor.plants.iter().map(|p| p.model.clone()).collect();
    let have: Vec<&str> = names
        .iter()
        .filter(|n| model_asset_path(&cfg.cache_root, area, n).is_some())
        .map(String::as_str)
        .collect();
    let mut plants = Vec::new();
    for n in &names {
        let used = if have.contains(&n.as_str()) { Some(n.clone()) } else { substitute_model(n, &have) };
        let mut rt = PlantRuntime::default();
        if let Some(path) = used.as_ref().and_then(|m| model_asset_path(&cfg.cache_root, area, m)) {
            rt.gltf = Some(asset_server.load(path));
            rt.model = used;
        } else {
            rt.done = true; // nothing to draw for this plant
        }
        plants.push(rt);
    }
    println!(
        "Vegetation area {area:02}: {} plants ({} with a model), {} detail maps.",
        plants.len(),
        plants.iter().filter(|p| p.gltf.is_some()).count(),
        prepared.data.masks.len()
    );
    st.plants = plants;
    st.coverage = prepared.coverage;
    st.data = Some(Arc::new(prepared.data));
}

fn veg_poll_models(
    cfg: Res<VegetationConfig>,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    gltf_materials: Res<Assets<GltfMaterial>>,
    meshes: Res<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut st: ResMut<VegetationState>,
) {
    let Some(data) = st.data.clone() else { return };
    let texel = data.texel_size();
    for rt in st.plants.iter_mut().filter(|p| !p.done) {
        let Some(h) = rt.gltf.clone() else { continue };
        if asset_server.load_state(&h).is_failed() {
            rt.done = true;
            continue;
        }
        let Some(gltf) = gltfs.get(&h) else { continue };
        let mut ready = true;
        let mut prims = Vec::new();
        for mh in &gltf.meshes {
            let Some(gm) = gltf_meshes.get(mh) else {
                ready = false;
                break;
            };
            for prim in &gm.primitives {
                let mat = match &prim.material {
                    Some(m) => match gltf_materials.get(m) {
                        Some(g) => Some(g),
                        None => {
                            ready = false;
                            None
                        }
                    },
                    None => None,
                };
                let Some(mesh) = meshes.get(&prim.mesh) else {
                    ready = false;
                    continue;
                };
                let sm = StandardMaterial {
                    base_color: Color::WHITE, // tint comes from vertex colours
                    base_color_texture: mat.and_then(|g| g.base_color_texture.clone()),
                    perceptual_roughness: 0.95,
                    metallic: 0.0,
                    reflectance: 0.1,
                    cull_mode: None,
                    double_sided: true,
                    alpha_mode: AlphaMode::Mask(0.5),
                    ..default()
                };
                prims.push(PrimData { mesh: mesh.clone(), material: materials.add(sm) });
            }
        }
        if !ready {
            continue;
        }
        rt.tris = prims.iter().map(|p| mesh_triangles(&p.mesh)).sum();
        rt.density = density_per_texel(rt.tris, texel, cfg.quality);
        rt.radius = radius_for(rt.tris, cfg.quality);
        rt.prims = prims;
        rt.done = true;
        rt.gltf = None; // meshes were copied; release the glTF scene
    }
}

fn veg_stream(
    mut commands: Commands,
    cfg: Res<VegetationConfig>,
    player: Query<&Transform, With<PlayerAvatar>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut st: ResMut<VegetationState>,
    mut stats: ResMut<VegetationStats>,
) {
    let Some(data) = st.data.clone() else { return };
    let Some(pt) = player.iter().next() else { return };
    if st.plants.iter().any(|p| !p.done) {
        return; // wait until every model resolved so chunks are built once
    }
    let player_xz = Vec2::new(pt.translation.x, pt.translation.z);
    let chunk_m = CHUNK_TEXELS as f32 * data.texel_size();
    let radius: Vec<f32> =
        st.plants.iter().map(|p| if p.prims.is_empty() { 0.0 } else { p.radius }).collect();

    // Release chunks that left their radius (+ hysteresis).
    let stale: Vec<_> = st
        .alive
        .keys()
        .copied()
        .filter(|&(cx, cz, pi)| {
            let r = radius.get(pi as usize).copied().unwrap_or(0.0) + UNLOAD_MARGIN;
            dist_to_square(player_xz, cx as f32 * chunk_m, cz as f32 * chunk_m, chunk_m) > r
        })
        .collect();
    for key in stale {
        if let Some(c) = st.alive.remove(&key) {
            st.live_instances -= c.instances;
            for e in c.entities {
                commands.entity(e).despawn();
            }
        }
    }

    // Build new chunks nearest-first within the frame and live budgets.
    let wanted = wanted_chunks(player_xz, chunk_m, &st.coverage, &radius);
    let live_cap = cfg.quality.max_live_instances();
    let frame_cap = cfg.quality.max_instances_per_frame();
    let mut built = 0usize;
    let mut seen: HashSet<(u32, u32, u16)> = HashSet::new();
    for (key, _) in wanted {
        seen.insert(key);
        if st.alive.contains_key(&key) {
            continue;
        }
        let (cx, cz, pi) = key;
        let pi_us = pi as usize;
        let estimate = (st.coverage[pi_us].2[(cz * st.coverage[pi_us].0 + cx) as usize] as f32
            * st.plants[pi_us].density)
            .ceil() as usize;
        if built > 0 && built + estimate > frame_cap {
            break;
        }
        if st.live_instances + estimate > live_cap {
            break;
        }
        let Some((mask, ch)) = data.mask_for(pi_us) else { continue };
        let spec = &data.descriptor.plants[pi_us];
        let params = ScatterParams { seed: SEED, area: data.area, plant: pi, density: st.plants[pi_us].density };
        let inst = chunk_instances(mask, ch, data.descriptor.extent, (cx, cz), &params, spec);
        let mut entities = Vec::new();
        let mut tris = 0u64;
        for prim in &st.plants[pi_us].prims {
            if let Some(mesh) =
                merge_instances(&prim.mesh, &inst, |x, z| data.ground_y(x, z), spec.tint_a, spec.tint_b)
            {
                tris += mesh_triangles(&prim.mesh) as u64 * inst.len() as u64;
                entities.push(
                    commands
                        .spawn((
                            VegetationChunk,
                            Mesh3d(meshes.add(mesh)),
                            MeshMaterial3d(prim.material.clone()),
                            Transform::IDENTITY,
                            Visibility::default(),
                            NotShadowCaster,
                        ))
                        .id(),
                );
            }
        }
        built += inst.len();
        st.live_instances += inst.len();
        st.alive.insert(key, AliveChunk { entities, instances: inst.len(), tris });
    }
    stats.chunks = st.alive.len();
    stats.draw_calls = st.alive.values().map(|c| c.entities.len()).sum();
    stats.instances = st.live_instances;
    stats.triangles = st.alive.values().map(|c| c.tris).sum();
    stats.built_this_frame = built;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad() -> Mesh {
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        m.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[-0.25f32, 0.0, 0.0], [0.25, 0.0, 0.0], [0.25, 0.25, 0.0], [-0.25, 0.25, 0.0]],
        );
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; 4]);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
        m.insert_indices(Indices::U16(vec![0, 1, 2, 0, 2, 3]));
        m
    }

    fn inst(x: f32, z: f32, yaw: f32) -> PlantInstance {
        PlantInstance { x, z, yaw, scale_h: 1.0, scale_v: 2.0, tint: 0.5 }
    }

    #[test]
    fn merge_counts_and_places_instances() {
        let insts = [inst(10.0, 20.0, 0.0), inst(30.0, 40.0, 1.0), inst(50.0, 60.0, 2.0)];
        let m = merge_instances(&quad(), &insts, |_, _| 5.0, [1.0; 4], [0.0, 1.0, 0.0, 1.0]).unwrap();
        assert_eq!(m.count_vertices(), 12);
        assert_eq!(m.indices().unwrap().len(), 18);
        let Some(VertexAttributeValues::Float32x3(p)) = m.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
        // first instance, yaw 0: vertex 2 is (0.25, 0.25*2, 0) + origin (10, 5, 20)
        assert!((Vec3::from(p[2]) - Vec3::new(10.25, 5.5, 20.0)).length() < 1e-5);
        assert!(p.iter().all(|v| v.iter().all(|c| c.is_finite())));
        let Some(VertexAttributeValues::Float32x2(uv)) = m.attribute(Mesh::ATTRIBUTE_UV_0) else { panic!() };
        assert_eq!(uv[0], [0.0, 0.0]); // source v = 1 -> flipped to 0
        let Some(VertexAttributeValues::Float32x4(c)) = m.attribute(Mesh::ATTRIBUTE_COLOR) else { panic!() };
        assert_eq!(c[0], [0.5, 1.0, 0.5, 1.0]);
        // indices of the second instance are offset by 4 vertices
        let Indices::U32(i) = m.indices().unwrap() else { panic!() };
        assert_eq!(&i[6..12], &[4, 5, 6, 4, 6, 7]);
    }

    #[test]
    fn merge_rejects_empty_and_unindexed() {
        assert!(merge_instances(&quad(), &[], |_, _| 0.0, [1.0; 4], [1.0; 4]).is_none());
        let bare = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        assert!(merge_instances(&bare, &[inst(0.0, 0.0, 0.0)], |_, _| 0.0, [1.0; 4], [1.0; 4]).is_none());
    }

    #[test]
    fn square_distance() {
        let p = Vec2::new(50.0, 50.0);
        assert_eq!(dist_to_square(p, 32.0, 32.0, 32.0), 0.0); // inside
        assert!((dist_to_square(p, 100.0, 40.0, 32.0) - 50.0).abs() < 1e-5);
        assert!((dist_to_square(p, 100.0, 100.0, 10.0) - (50.0f32 * 2.0f32.sqrt())).abs() < 1e-4);
    }

    #[test]
    fn wanted_chunks_are_sorted_filtered_and_bounded() {
        // 10x10 chunk grid, plant 0 set everywhere, plant 1 only chunk (5,5), plant 2 radius 0.
        let full = (10u32, 10u32, vec![100u32; 100]);
        let mut one = vec![0u32; 100];
        one[55] = 9;
        let cov = vec![full.clone(), (10, 10, one), full];
        let w = wanted_chunks(Vec2::new(160.0, 160.0), 32.0, &cov, &[40.0, 200.0, 0.0]);
        assert!(w.windows(2).all(|p| p[0].1 <= p[1].1), "nearest first");
        assert!(w.iter().all(|(k, _)| k.2 != 2), "zero radius is skipped");
        assert!(w.iter().any(|(k, _)| *k == (5, 5, 1)));
        // plant 0 within 40 m of the centre of chunk (5,5): bounded neighbourhood, not the whole grid
        let n0 = w.iter().filter(|(k, _)| k.2 == 0).count();
        assert!(n0 > 0 && n0 <= 16, "{n0}");
        assert!(w.iter().filter(|(k, _)| k.2 == 0).all(|(_, d)| *d <= 40.0));
        // player outside the map: nothing within radius
        assert!(wanted_chunks(Vec2::new(-500.0, -500.0), 32.0, &cov, &[40.0, 40.0, 40.0]).is_empty());
    }

    #[test]
    fn model_path_prefers_the_area_terrain_directory() {
        let root = std::env::temp_dir().join(format!("veg_test_{}", std::process::id()));
        let d = root.join("gltf_field/ha_area01_terrain");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("plants_x.gltf"), "{}").unwrap();
        assert_eq!(
            model_asset_path(&root, 1, "plants_x").as_deref(),
            Some("cache/gltf_field/ha_area01_terrain/plants_x.gltf")
        );
        assert_eq!(model_asset_path(&root, 1, "plants_missing"), None);
        // found in another area's terrain dir as a fallback
        assert!(model_asset_path(&root, 0, "plants_x").is_some());
        std::fs::remove_dir_all(&root).ok();
    }
}
