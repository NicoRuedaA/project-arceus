//! Windowless performance measurement (Brief 20).
//!
//! Two phases, both without any window:
//!
//! 1. CPU data phase: for areas 00..05 measures `TerrainArea::load`,
//!    `build_mesh_chunks`, the navmesh load, the cached albedo load (and optionally one
//!    in-memory albedo bake), plus content counts (vertices, triangles, `.trins` instances,
//!    placements, spawners) and process memory from `/proc/self/status`.
//! 2. Offscreen render phase (one process): builds a real scene (terrain mesh + baked albedo +
//!    `ZoneLightingPlugin` + streamed trees / placed objects / vegetation) and renders N frames to
//!    an image target with `FrameTimeDiagnosticsPlugin`. No window is created (`WinitPlugin`
//!    disabled), so nothing appears on the desktop. The frame loop times are CPU-side wall-clock
//!    numbers; the GPU frame time of the real windowed viewer is NOT measurable this way and is
//!    reported as UNKNOWN.
//!
//! The report is written to `reports/data-progress/performance.md` (override with `--out`).
//! A raw TSV (`--raw work/perf/baseline.tsv`, git-ignored) can be written for a later
//! `--compare` run that adds a before/after table with the same measurements.
//!
//! Usage:
//!   perf_report [--areas 0,1,2,3,4,5] [--out reports/data-progress/performance.md]
//!               [--raw work/perf/baseline.tsv] [--compare work/perf/baseline.tsv]
//!               [--no-render] [--render-area 0] [--render-pos X,Z] [--render-yaw DEG]
//!               [--render-warmup 600] [--render-frames 600] [--no-trees]
//!               [--tree-radius 150] [--bake-area N] [--bin-root PATH]
//!               [--grass off|low|high] [--no-shadows] [--look classic|vivid] ...
//!
//! `--grass` and `--look`/`--no-shadows` are the viewer's own flags, passed through to the
//! render phase. The example skips (exit 0) with a `[skip]` line when the game packs are absent.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::gltf::{Gltf, GltfMaterial, GltfMesh};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::time::Real;
use bevy::window::{ExitCondition, WindowPlugin};
use bevy::winit::WinitPlugin;

use pla::assets::navmesh::AreaNavigation;
use pla::assets::terrain::{build_mesh_chunks, TerrainArea, TerrainScaleOverride};
use pla::assets::terrain_albedo::{
    albedo_cache_path, bake_albedo_with, load_baked_albedo, BakeOptions,
};
use pla::assets::vegetation::GrassQuality;
use pla::game::lighting::{time_slot_from_args, ZoneLightingPlugin};
use pla::game::navigation::GameNavmeshResource;
use pla::game::render_look::LookSettings;
use pla::game::vegetation::{VegetationPlugin, VegetationStats};
use pla::game::wild::load_spawners_for_area;
use pla::game::world_objects::{
    find_data_file, parse_placements, WorldObjectsPlugin, DEFAULT_CACHE_ROOT,
};
use pla::game::PlayerAvatar;

const DEFAULT_BIN_ROOT: &str = "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin";

// ---------------------------------------------------------------------------------------------
// CLI helpers
// ---------------------------------------------------------------------------------------------

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn flag(args: &[String], key: &str) -> bool {
    args.iter().any(|a| a == key)
}

// ---------------------------------------------------------------------------------------------
// CPU data phase
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Default)]
struct AreaMetrics {
    area: u8,
    error: Option<String>,
    skip: Option<String>,
    pack_mib: f64,
    load_ms: f64,
    heightmap: Option<(u32, u32)>,
    extent_m: Option<f32>,
    world_height: Option<f32>,
    chunks_ms: f64,
    chunk_count: usize,
    vertices: usize,
    triangles: usize,
    mesh_mib: f64,
    nav_ms: f64,
    nav_faces: usize,
    nav_edges: usize,
    nav_vertices: usize,
    nav_meshes: usize,
    nav_error: Option<String>,
    albedo_ms: Option<f64>,
    albedo_dims: Option<(u32, u32)>,
    albedo_note: String,
    trins_instances: usize,
    trins_models: usize,
    masks: usize,
    placements_ok: usize,
    placements_with_gltf: usize,
    placement_ms: f64,
    spawners: usize,
    spawner_ms: f64,
    rss_before_mib: f64,
    rss_after_load_mib: f64,
    rss_delta_mib: f64,
    peak_mib: f64,
}

fn read_proc_status_kib(key: &str) -> Option<f64> {
    let text = std::fs::read_to_string("/proc/self/status").ok()?;
    text.lines().find_map(|line| {
        let mut it = line.split_whitespace();
        if it.next()? != key {
            return None;
        }
        it.next()?.parse::<f64>().ok()
    })
}

fn rss_mib() -> f64 {
    read_proc_status_kib("VmRSS:")
        .map(|kb| kb / 1024.0)
        .unwrap_or(f64::NAN)
}

fn peak_rss_mib() -> f64 {
    read_proc_status_kib("VmHWM:")
        .map(|kb| kb / 1024.0)
        .unwrap_or(f64::NAN)
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// `model -> relative exported glTF path under the cache root`, "ok" rows only. The stored
/// `output` may be absolute or legacy `work/cache/`-relative; both normalize to the cache-relative
/// tail this map keys on.
fn exported_gltf_index() -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    let Some(text) = find_data_file("field_model_export_index.tsv")
        .and_then(|p| std::fs::read_to_string(p).ok())
    else {
        return map;
    };
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() >= 4 && c[3] == "ok" {
            if let Some(rel) = pla::game::world_objects::cache_relative_export_path(c[2]) {
                map.insert(c[1].to_string(), PathBuf::from(rel));
            }
        }
    }
    map
}

fn measure_area(area: u8, bin_root: &Path, cache_root: &Path, work_root: &Path) -> AreaMetrics {
    let mut m = AreaMetrics {
        area,
        ..Default::default()
    };
    m.rss_before_mib = rss_mib();

    let pack_path = bin_root
        .join("archive/field/terrain")
        .join(format!("ha_area{area:02}.gfpak"));
    m.pack_mib = std::fs::metadata(&pack_path)
        .map(|md| md.len() as f64 / (1024.0 * 1024.0))
        .unwrap_or(f64::NAN);
    if !pack_path.exists() {
        m.skip = Some(format!("{pack_path:?} not found"));
        return m;
    }

    let t = Instant::now();
    let terrain = match TerrainArea::load(bin_root, area) {
        Ok(t) => t,
        Err(e) => {
            m.error = Some(e);
            return m;
        }
    };
    m.load_ms = ms(t);
    m.rss_after_load_mib = rss_mib();
    m.rss_delta_mib = (m.rss_after_load_mib - m.rss_before_mib).max(0.0);
    m.peak_mib = peak_rss_mib();

    m.heightmap = Some((terrain.heightmap.width, terrain.heightmap.height));
    m.extent_m = Some(terrain.world_extent[0]);
    m.world_height = Some(terrain.world_height);
    m.masks = terrain.masks.len();
    m.trins_instances = terrain.instances.len();
    m.trins_models = terrain
        .instances
        .iter()
        .map(|i| i.model_name.as_str())
        .collect::<BTreeSet<_>>()
        .len();

    // Terrain mesh chunks, exactly as the viewer builds them (64 cells per chunk).
    let t = Instant::now();
    match terrain.mesh_settings(TerrainScaleOverride::default(), 64) {
        Ok(settings) => match build_mesh_chunks(&terrain.heightmap, settings) {
            Ok(chunks) => {
                m.chunks_ms = ms(t);
                m.chunk_count = chunks.len();
                let mut bytes = 0usize;
                for c in &chunks {
                    let verts = c.mesh.count_vertices();
                    m.vertices += verts;
                    if let Some(indices) = c.mesh.indices() {
                        m.triangles += indices.len() / 3;
                        bytes += indices.len() * 4;
                    }
                    // position 3*f32 + normal 3*f32 + uv 2*f32 + tangent 4*f32
                    bytes += verts * (12 + 12 + 8 + 16);
                }
                m.mesh_mib = bytes as f64 / (1024.0 * 1024.0);
            }
            Err(e) => m.error = Some(format!("build_mesh_chunks: {e}")),
        },
        Err(e) => m.error = Some(format!("mesh_settings: {e}")),
    }

    // Navmesh (what `GameNavmeshResource::load` does at zone switch).
    let t = Instant::now();
    match AreaNavigation::load(bin_root, area) {
        Ok(nav) => {
            m.nav_ms = ms(t);
            m.nav_meshes = nav.meshes.len();
            if let Some(primary) = nav.meshes.first() {
                m.nav_faces = primary.faces.len();
                m.nav_edges = primary.edges.len();
                m.nav_vertices = primary.vertices.len();
            }
        }
        Err(e) => m.nav_error = Some(e),
    }

    // Baked albedo cache (the normal viewer path is load, not bake).
    let albedo_path = albedo_cache_path(work_root, area);
    if albedo_path.exists() {
        let t = Instant::now();
        match load_baked_albedo(&albedo_path) {
            Ok(tex) => {
                m.albedo_ms = Some(ms(t));
                m.albedo_dims = Some((tex.width, tex.height));
                m.albedo_note = format!(
                    "cached png {:.1} MiB",
                    std::fs::metadata(&albedo_path)
                        .map(|md| md.len() as f64 / (1024.0 * 1024.0))
                        .unwrap_or(f64::NAN)
                );
            }
            Err(e) => m.albedo_note = format!("[skip] cached png unreadable: {e}"),
        }
    } else {
        m.albedo_note = "[skip] no cached png".into();
    }

    // Placed objects (what WorldObjectsPlugin parses at build time).
    let t = Instant::now();
    if let Some(text) =
        find_data_file("field_object_placements.tsv").and_then(|p| std::fs::read_to_string(p).ok())
    {
        let rows = parse_placements(&text, &format!("ha_area{area:02}"));
        let index = exported_gltf_index();
        m.placements_ok = rows.len();
        m.placements_with_gltf = rows
            .iter()
            .filter(|p| {
                index
                    .get(&p.model)
                    .map(|rel| cache_root.join(rel).exists())
                    .unwrap_or(false)
            })
            .count();
    }
    m.placement_ms = ms(t);

    // Wild spawners for the area.
    let t = Instant::now();
    match find_data_file("field_spawn_table.tsv") {
        Some(path) => match load_spawners_for_area(&path, area) {
            Ok(spawners) => m.spawners = spawners.len(),
            Err(e) => m.error = Some(format!("spawners: {e}")),
        },
        None => m.error = Some("data/field_spawn_table.tsv not found".into()),
    }
    m.spawner_ms = ms(t);

    m.rss_after_load_mib = rss_mib();
    m.peak_mib = peak_rss_mib();
    drop(terrain);
    m
}

fn measure_bake(area: u8, bin_root: &Path) -> Option<(u8, f64)> {
    let terrain = TerrainArea::load(bin_root, area).ok()?;
    let t = Instant::now();
    match bake_albedo_with(&terrain, &BakeOptions::default()) {
        Ok(result) => {
            let elapsed = ms(t);
            println!(
                "[bake] area {area:02}: {elapsed:.1} ms ({}^2, in-memory, detail, 3-thread cap)",
                result.image.width,
            );
            Some((area, elapsed))
        }
        Err(e) => {
            eprintln!("[bake] area {area:02} failed: {e}");
            None
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Offscreen render phase (no window)
// ---------------------------------------------------------------------------------------------

#[derive(Resource)]
struct RenderCfg {
    warmup: u32,
    frames: u32,
    max_frames: u32,
    trees: bool,
    tree_radius: f32,
    anchor: Vec2,
    yaw: f32,
}

#[derive(Resource)]
struct RenderTerrain(TerrainArea);

#[derive(Resource, Default)]
struct RenderState {
    frame: u32,
    area: u8,
    samples: Vec<f64>,
    stable_frames: u32,
    last_entities: usize,
    chunks_ms: f64,
    nav_ms: f64,
    albedo_ms: f64,
    nav_error: Option<String>,
    anchor: [f32; 2],
    anchor_y: f32,
    exit_reason: String,
}

#[derive(Resource, Default, Clone)]
struct RenderSummary {
    done: bool,
    area: u8,
    measured: usize,
    mean_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    min_ms: f64,
    max_ms: f64,
    diag_frame_ms: Option<f64>,
    fps: Option<f64>,
    entities: usize,
    mesh_entities: usize,
    tree_instances: usize,
    tree_models: usize,
    tree_requested: usize,
    grass: Option<(usize, usize, u64, usize)>,
    exit_reason: String,
    anchor: [f32; 2],
    chunks_ms: f64,
    nav_ms: f64,
    albedo_ms: f64,
}

#[derive(Resource, Clone)]
struct SummaryCell(Arc<Mutex<Option<RenderSummary>>>);

/// One glTF primitive: mesh handle + the material this harness assigned to it.
type Prims = Vec<(Handle<Mesh>, Handle<StandardMaterial>)>;

#[derive(Resource)]
struct TreeStream {
    handles: HashMap<String, Handle<Gltf>>,
    instances: HashMap<String, Vec<Mat4>>,
    prims: HashMap<String, Prims>,
    spawned: bool,
    spawned_instances: usize,
    requested_models: usize,
}

fn render_area_default_pos(area: u8) -> Vec2 {
    if area == 0 {
        Vec2::new(448.0, 652.0)
    } else {
        Vec2::new(512.5, 512.5)
    }
}

fn run_render(
    args: &[String],
    area: u8,
    bin_root: &Path,
    cache_root: &Path,
    work_root: &Path,
) -> Option<RenderSummary> {
    let settings = LookSettings::from_args(args);
    let grass = GrassQuality::from_args(args);
    let trees = !flag(args, "--no-trees");
    let tree_radius: f32 = arg(args, "--tree-radius")
        .and_then(|s| s.parse().ok())
        .unwrap_or(150.0);
    let warmup: u32 = arg(args, "--render-warmup")
        .and_then(|s| s.parse().ok())
        .unwrap_or(600);
    let frames: u32 = arg(args, "--render-frames")
        .and_then(|s| s.parse().ok())
        .unwrap_or(600);
    let mut anchor = arg(args, "--render-pos")
        .and_then(|s| {
            let (a, b) = s.split_once(',')?;
            Some(Vec2::new(a.trim().parse().ok()?, b.trim().parse().ok()?))
        })
        .unwrap_or_else(|| render_area_default_pos(area));
    if !anchor.is_finite() {
        eprintln!("[render] --render-pos is not finite; using the area default");
        anchor = render_area_default_pos(area);
    }
    let yaw: f32 = arg(args, "--render-yaw")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);

    // Load the terrain once for the app setup, and derive the `.trins` model exclusion set for
    // the placed-object streamer (same division of work as the viewer).
    let terrain = match TerrainArea::load(bin_root, area) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[render] area {area:02} terrain not available: {e}");
            return None;
        }
    };
    let exclude_models: HashSet<String> = terrain
        .instances
        .iter()
        .map(|i| i.model_name.clone())
        .collect();
    let summary_cell: Arc<Mutex<Option<RenderSummary>>> = Arc::new(Mutex::new(None));

    let mut app = App::new();
    app.insert_resource(settings)
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    close_when_requested: false,
                    ..default()
                })
                .set(bevy::asset::AssetPlugin {
                    file_path: work_root.to_string_lossy().to_string(),
                    unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                    ..default()
                })
                .disable::<WinitPlugin>(),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::ZERO))
        .add_plugins(FrameTimeDiagnosticsPlugin::new(100_000))
        .add_plugins(ZoneLightingPlugin {
            area,
            slot: time_slot_from_args(args),
        });

    if grass != GrassQuality::Off {
        app.add_plugins(VegetationPlugin {
            area,
            bin_root: bin_root.to_path_buf(),
            cache_root: cache_root.to_path_buf(),
            quality: grass,
            grass_range: 1.0,
            plants_range: 1.0,
        });
    }
    if area <= 5 {
        app.add_plugins(WorldObjectsPlugin {
            area,
            cache_root: cache_root.to_path_buf(),
            exclude_models,
            rot_mode: pla::game::world_objects::RotMode::YawY,
        });
    }

    app.insert_resource(RenderCfg {
        warmup,
        frames,
        max_frames: warmup + frames + 6000,
        trees,
        tree_radius,
        anchor,
        yaw,
    })
    .insert_resource(RenderTerrain(terrain))
    .init_resource::<RenderState>()
    .init_resource::<RenderSummary>()
    .insert_resource(SummaryCell(summary_cell.clone()))
    .add_systems(Startup, setup_render_scene)
    .add_systems(Update, (stream_trees, sample_frames, finish).chain())
    .run();

    // `App::run` moves the world into the runner, so the summary is returned through the shared
    // cell instead of by querying the (now empty) `app`.
    let summary = summary_cell.lock().unwrap().clone();
    summary
}

fn setup_render_scene(
    mut commands: Commands,
    cfg: Res<RenderCfg>,
    terrain: Res<RenderTerrain>,
    mut state: ResMut<RenderState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let area = terrain.0.area;
    let setup_start = Instant::now();
    state.area = area;

    // Terrain chunks.
    let t = Instant::now();
    let settings = terrain
        .0
        .mesh_settings(TerrainScaleOverride::default(), 64)
        .expect("terrain mesh settings");
    let chunks = build_mesh_chunks(&terrain.0.heightmap, settings).expect("terrain chunks");
    state.chunks_ms = ms(t);

    // Baked albedo (viewer path).
    let t = Instant::now();
    let work_root = cfg_work_root();
    let albedo = load_baked_albedo(albedo_cache_path(&work_root, area))
        .ok()
        .map(|tex| {
            images.add(Image::new(
                Extent3d {
                    width: tex.width,
                    height: tex.height,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                tex.rgba,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            ))
        });
    state.albedo_ms = ms(t);
    let mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: albedo,
        perceptual_roughness: 0.95,
        ..default()
    });
    for c in &chunks {
        commands.spawn((
            Mesh3d(meshes.add(c.mesh.clone())),
            MeshMaterial3d(mat.clone()),
        ));
    }

    // Anchor: snap to the navmesh when the default point is not walkable.
    let mut anchor = cfg.anchor;
    let t = Instant::now();
    match GameNavmeshResource::load(cfg_bin_root(), area) {
        Ok(nav) => {
            if !nav.is_walkable(anchor.x, anchor.y) {
                if let Some(hit) = nav.nearest(anchor.x, anchor.y, 200.0) {
                    let centroid =
                        pla::game::nav_report::face_centroid(nav.nav.primary(), hit.face);
                    anchor = Vec2::new(centroid[0], centroid[2]);
                }
            }
        }
        Err(e) => state.nav_error = Some(e),
    }
    state.nav_ms = ms(t);
    let y = terrain
        .0
        .sample_world_height(anchor.x, anchor.y)
        .unwrap_or(0.0);
    state.anchor = [anchor.x, anchor.y];
    state.anchor_y = y;
    commands.spawn((
        PlayerAvatar,
        Transform::from_xyz(anchor.x, y + 0.9, anchor.y),
    ));

    // Offscreen render target + camera (same shape as the viewer's starting view).
    let target = images.add(Image::new_target_texture(
        1280,
        720,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let rot = Quat::from_rotation_y(cfg.yaw.to_radians()) * Quat::from_rotation_x(-0.12);
    commands.spawn((
        Camera3d::default(),
        RenderTarget::Image(target.clone().into()),
        Transform::from_xyz(anchor.x, y + 1.8, anchor.y).with_rotation(rot),
    ));

    // Streamed `.trins` trees inside the radius (same spawning shape as the viewer).
    if cfg.trees {
        let mut instances: HashMap<String, Vec<Mat4>> = HashMap::new();
        for inst in &terrain.0.instances {
            let flat: [f32; 16] = [
                inst.matrix[0][0],
                inst.matrix[0][1],
                inst.matrix[0][2],
                inst.matrix[0][3],
                inst.matrix[1][0],
                inst.matrix[1][1],
                inst.matrix[1][2],
                inst.matrix[1][3],
                inst.matrix[2][0],
                inst.matrix[2][1],
                inst.matrix[2][2],
                inst.matrix[2][3],
                inst.matrix[3][0],
                inst.matrix[3][1],
                inst.matrix[3][2],
                inst.matrix[3][3],
            ];
            let transform = Mat4::from_cols_array(&flat);
            let d = Vec2::new(transform.w_axis.x, transform.w_axis.z).distance(anchor);
            if d > cfg.tree_radius {
                continue;
            }
            instances
                .entry(inst.model_name.clone())
                .or_default()
                .push(transform);
        }
        let index = exported_gltf_index();
        let mut handles = HashMap::new();
        for model in instances.keys() {
            if let Some(rel) = index.get(model) {
                let asset_path = format!("cache/{}", rel.to_string_lossy());
                if Path::new(&cfg_work_root()).join(&asset_path).exists() {
                    handles.insert(model.clone(), asset_server.load(asset_path));
                }
            }
        }
        instances.retain(|m, _| handles.contains_key(m));
        let requested_models = handles.len();
        commands.insert_resource(TreeStream {
            handles,
            instances,
            prims: HashMap::new(),
            spawned: false,
            spawned_instances: 0,
            requested_models,
        });
    }

    println!(
        "[render] area {area:02} anchor ({:.1},{:.1}) y={y:.1}: {} chunks, albedo {:.1} ms, nav {:.1} ms, setup {:.1} ms",
        anchor.x,
        anchor.y,
        chunks.len(),
        state.albedo_ms,
        state.nav_ms,
        ms(setup_start)
    );
}

/// Work root (parent of the cache directory) and game bin root, resolved from the environment at
/// start-up (the heavy commands export `PLA_CACHE_ROOT`; `PLA_BIN_ROOT` optional).
fn cfg_work_root() -> PathBuf {
    let cache = std::env::var("PLA_CACHE_ROOT").unwrap_or_else(|_| DEFAULT_CACHE_ROOT.into());
    Path::new(&cache)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(cache))
}

fn cfg_bin_root() -> PathBuf {
    PathBuf::from(std::env::var("PLA_BIN_ROOT").unwrap_or_else(|_| DEFAULT_BIN_ROOT.into()))
}

fn stream_trees(
    mut commands: Commands,
    cfg: Res<RenderCfg>,
    mut ts: Option<ResMut<TreeStream>>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    gltf_materials: Res<Assets<GltfMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(ts) = ts.as_mut() else { return };
    if ts.spawned {
        return;
    }
    if !cfg.trees || ts.instances.is_empty() {
        ts.spawned = true;
        return;
    }
    if !ts.handles.values().all(|h| gltfs.contains(h)) {
        return;
    }

    let handles: Vec<(String, Handle<Gltf>)> = ts
        .handles
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (name, handle) in &handles {
        if ts.prims.contains_key(name) {
            continue;
        }
        let Some(gltf) = gltfs.get(handle) else {
            continue;
        };
        let mut prims = Vec::new();
        for mesh_h in &gltf.meshes {
            let Some(gltf_mesh) = gltf_meshes.get(mesh_h) else {
                continue;
            };
            for prim in &gltf_mesh.primitives {
                let sm = prim
                    .material
                    .as_ref()
                    .and_then(|h| gltf_materials.get(h))
                    .map(|gm| StandardMaterial {
                        base_color: gm.base_color,
                        base_color_texture: gm.base_color_texture.clone(),
                        perceptual_roughness: gm.perceptual_roughness,
                        metallic: gm.metallic,
                        cull_mode: None,
                        alpha_mode: AlphaMode::Mask(0.5),
                        ..default()
                    })
                    .unwrap_or(StandardMaterial {
                        base_color: Color::srgb(0.3, 0.45, 0.2),
                        cull_mode: None,
                        ..default()
                    });
                prims.push((prim.mesh.clone(), standard_materials.add(sm)));
            }
        }
        ts.prims.insert(name.clone(), prims);
    }

    let mut count = 0;
    for (name, transforms) in &ts.instances {
        let Some(prims) = ts.prims.get(name) else {
            continue;
        };
        for transform in transforms {
            for (mesh, mat) in prims {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(mat.clone()),
                    Transform::from_matrix(*transform),
                ));
            }
            count += 1;
        }
    }
    ts.spawned_instances = count;
    ts.spawned = true;
    println!(
        "[render] spawned {} tree instances from {} models",
        count,
        ts.instances.len()
    );
}

fn sample_frames(
    cfg: Res<RenderCfg>,
    mut state: ResMut<RenderState>,
    time: Res<Time<Real>>,
    ts: Option<Res<TreeStream>>,
    veg: Option<Res<VegetationStats>>,
    q: Query<Entity>,
    mut summary: ResMut<RenderSummary>,
    mut exit: MessageWriter<AppExit>,
) {
    if summary.done {
        return;
    }
    state.frame += 1;

    if state.frame >= cfg.max_frames {
        state.exit_reason = format!("did not settle before {} frames", cfg.max_frames);
        summary.done = true;
        exit.write(AppExit::Success);
        return;
    }
    if state.frame <= cfg.warmup {
        return;
    }
    if let Some(ts) = &ts {
        if cfg.trees && !ts.spawned && !ts.instances.is_empty() {
            return; // still loading tree glTFs
        }
    }
    // Settle: entity count stable for 60 frames (asset streaming finished) and, when grass is on,
    // vegetation also stopped building chunks this frame.
    if let Some(v) = &veg {
        if v.chunks == 0 || v.built_this_frame > 0 {
            state.stable_frames = 0;
            return;
        }
    }
    let entities = q.iter().count();
    if entities == state.last_entities {
        state.stable_frames += 1;
    } else {
        state.stable_frames = 0;
        state.last_entities = entities;
    }
    if state.stable_frames < 60 {
        return;
    }

    state.samples.push(time.delta_secs_f64() * 1000.0);
    if state.samples.len() >= cfg.frames as usize {
        state.exit_reason = "ok".into();
        summary.done = true;
        exit.write(AppExit::Success);
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn finish(
    state: Res<RenderState>,
    mut summary: ResMut<RenderSummary>,
    cell: Res<SummaryCell>,
    diagnostics: Res<DiagnosticsStore>,
    q_all: Query<Entity>,
    q_mesh: Query<Entity, With<Mesh3d>>,
    ts: Option<Res<TreeStream>>,
    veg: Option<Res<VegetationStats>>,
) {
    if !summary.done || summary.measured > 0 {
        return;
    }
    let mut s = state.samples.clone();
    s.sort_by(f64::total_cmp);
    let mean = if s.is_empty() {
        f64::NAN
    } else {
        s.iter().sum::<f64>() / s.len() as f64
    };
    let grass = veg
        .as_ref()
        .map(|v| (v.chunks, v.draw_calls, v.triangles, v.instances));
    *summary = RenderSummary {
        done: true,
        area: state.area,
        measured: s.len(),
        mean_ms: mean,
        p50_ms: percentile(&s, 0.50),
        p95_ms: percentile(&s, 0.95),
        p99_ms: percentile(&s, 0.99),
        min_ms: s.first().copied().unwrap_or(f64::NAN),
        max_ms: s.last().copied().unwrap_or(f64::NAN),
        diag_frame_ms: diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .and_then(|d| d.value()),
        fps: diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|d| d.value()),
        entities: q_all.iter().count(),
        mesh_entities: q_mesh.iter().count(),
        tree_instances: ts.as_ref().map(|t| t.spawned_instances).unwrap_or(0),
        tree_models: ts.as_ref().map(|t| t.instances.len()).unwrap_or(0),
        tree_requested: ts.as_ref().map(|t| t.requested_models).unwrap_or(0),
        grass,
        exit_reason: state.exit_reason.clone(),
        anchor: state.anchor,
        chunks_ms: state.chunks_ms,
        nav_ms: state.nav_ms,
        albedo_ms: state.albedo_ms,
    };
    *cell.0.lock().unwrap() = Some(summary.clone());
}

// ---------------------------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------------------------

fn cpu_label() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|t| {
            t.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into())
}

struct Env {
    cpu: String,
    bin_root: String,
    cache_root: String,
    profile: String,
    command: String,
}

fn fmt_opt_ms(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.1}"))
        .unwrap_or_else(|| "[skip]".into())
}

fn write_raw(
    path: &Path,
    metrics: &[AreaMetrics],
    bake: &Option<(u8, f64)>,
    renders: &[RenderSummary],
) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut out = String::new();
    let _ = writeln!(out, "# perf_report raw v1");
    let _ = writeln!(out, "# cpu\t{}", cpu_label());
    if let Some((area, ms)) = bake {
        let _ = writeln!(out, "# bake_area\t{area}\t{ms:.1}");
    }
    for r in renders {
        let _ = writeln!(
            out,
            "# render\t{}\t{:.3}\tp50\t{:.3}\tp95\t{:.3}\tp99\t{:.3}\tmeasured\t{}",
            r.area, r.mean_ms, r.p50_ms, r.p95_ms, r.p99_ms, r.measured
        );
    }
    let _ = writeln!(
        out,
        "area\tload_ms\tchunks_ms\tnav_ms\talbedo_ms\trss_delta_mib\tvertices\ttriangles"
    );
    for m in metrics {
        let _ = writeln!(
            out,
            "{}\t{:.1}\t{:.1}\t{:.1}\t{}\t{:.1}\t{}\t{}",
            m.area,
            m.load_ms,
            m.chunks_ms,
            m.nav_ms,
            fmt_opt_ms(m.albedo_ms),
            m.rss_delta_mib,
            m.vertices,
            m.triangles
        );
    }
    let _ = std::fs::write(path, out);
    println!("[raw] wrote {path:?}");
}

struct Compare {
    title: String,
    area: HashMap<u8, HashMap<String, f64>>,
    render_mean: HashMap<u8, f64>,
}

fn parse_compare(text: &str, title: String) -> Compare {
    let mut area: HashMap<u8, HashMap<String, f64>> = HashMap::new();
    let mut render_mean = HashMap::new();
    let mut header: Vec<String> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# render\t") {
            let mut it = rest.split('\t');
            if let (Some(a), Some(m)) = (
                it.next().and_then(|v| v.parse::<u8>().ok()),
                it.next().and_then(|v| v.parse::<f64>().ok()),
            ) {
                render_mean.insert(a, m);
            }
            continue;
        }
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.first() == Some(&"area") {
            header = cols.iter().map(|s| s.to_string()).collect();
            continue;
        }
        if let Ok(n) = cols[0].parse::<u8>() {
            let mut row = HashMap::new();
            for (i, name) in header.iter().enumerate().skip(1) {
                if let Some(v) = cols.get(i).and_then(|v| v.parse::<f64>().ok()) {
                    row.insert(name.clone(), v);
                }
            }
            area.insert(n, row);
        }
    }
    Compare {
        title,
        area,
        render_mean,
    }
}

fn delta(before: f64, after: f64) -> String {
    if before <= 0.0 || !before.is_finite() || !after.is_finite() {
        return "[skip]".into();
    }
    format!("{:+.1} %", (after - before) / before * 100.0)
}

fn write_report(
    path: &Path,
    env: &Env,
    metrics: &[AreaMetrics],
    bake: &Option<(u8, f64)>,
    render_requested: bool,
    renders: &[RenderSummary],
    compare: &Option<Compare>,
) {
    let mut out = String::new();
    out.push_str("# Performance baseline (Brief 20)\n\n");
    out.push_str(
        "Generated by [`crates/pla/examples/perf_report.rs`](../../crates/pla/examples/perf_report.rs).\n\
         **No window is created anywhere**: `WinitPlugin` is disabled and the camera renders to an\n\
         offscreen image, so the measurement never touches the desktop. The numbers below are\n\
         CPU-side wall-clock costs on the measurement machine. **The GPU frame time of the real\n\
         windowed viewer is UNKNOWN**: it cannot be measured without a window and is not faked here.\n\n",
    );
    let _ = writeln!(out, "- Command: `{}`", env.command);
    let _ = writeln!(
        out,
        "- Profile: {} (`play.sh` runs the viewer with `cargo run --release`)",
        env.profile
    );
    let _ = writeln!(out, "- CPU: {}", env.cpu);
    let _ = writeln!(out, "- Area packs: `{}`", env.bin_root);
    let _ = writeln!(out, "- Cache root: `{}`", env.cache_root);
    out.push('\n');

    out.push_str("## Zone load and content budget (CPU, no window)\n\n");
    out.push_str(
        "`load` = `TerrainArea::load` (pack parse + texture decode + `.trins` scan); `chunks` = \n\
         `build_mesh_chunks(.., 64 cells)`; `nav` = `AreaNavigation::load` (what\n\
         `GameNavmeshResource::load` calls); `albedo` = reading the cached 4096² bake from PNG\n\
         (`[skip]` when the cache is absent; the bake itself is a separate optional measurement\n\
         below). `mesh MiB` is the decoded attribute+index bytes handed to Bevy, not GPU memory.\n\
         `RSS Δ` is `VmRSS` after `TerrainArea::load` minus before (allocator-dependent, ±).\n\n",
    );
    out.push_str("| area | pack MiB | load ms | chunks ms | nav ms | albedo ms | chunks | vertices | triangles | mesh MiB | trins inst | placements (gltf) | spawners | RSS Δ MiB |\n");
    out.push_str("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for m in metrics {
        if m.skip.is_some() {
            let _ = writeln!(out, "| {:02} | [skip] | | | | | | | | | | | | |", m.area);
            continue;
        }
        let _ = writeln!(
            out,
            "| {:02} | {:.1} | {:.1} | {:.1} | {:.1} | {} | {} | {} | {} | {:.1} | {} | {} ({}) | {} | {:.0} |",
            m.area,
            m.pack_mib,
            m.load_ms,
            m.chunks_ms,
            m.nav_ms,
            fmt_opt_ms(m.albedo_ms),
            m.chunk_count,
            m.vertices,
            m.triangles,
            m.mesh_mib,
            m.trins_instances,
            m.placements_ok,
            m.placements_with_gltf,
            m.spawners,
            m.rss_delta_mib
        );
    }
    out.push('\n');
    for m in metrics {
        let mut notes = Vec::new();
        if let Some(e) = &m.error {
            notes.push(format!("ERROR: {e}"));
        }
        if let Some(e) = &m.nav_error {
            notes.push(format!("navmesh unavailable: {e}"));
        }
        if !m.albedo_note.is_empty() {
            notes.push(format!("albedo: {}", m.albedo_note));
        }
        if let Some((w, h)) = m.heightmap {
            let _ = writeln!(
                out,
                "- area {:02}: heightmap {w}×{h}, extent {} m, `world_height` {}, {} `.trins` models, {} masks{}",
                m.area,
                m.extent_m.map(|e| format!("{e:.1}")).unwrap_or("?".into()),
                m.world_height.map(|h| format!("{h:.1}")).unwrap_or("?".into()),
                m.trins_models,
                m.masks,
                if notes.is_empty() { String::new() } else { format!(" — {}", notes.join("; ")) }
            );
        }
    }
    let peak = metrics.iter().map(|m| m.peak_mib).fold(0.0f64, f64::max);
    if peak.is_finite() && peak > 0.0 {
        let _ = writeln!(
            out,
            "\nProcess `VmHWM` peak after the data phase: {peak:.0} MiB."
        );
    }
    out.push('\n');

    if let Some((area, bake_ms)) = bake {
        out.push_str("## Albedo bake (in-memory, one area)\n\n");
        let _ = writeln!(
            out,
            "- area {area:02}: **{bake_ms:.0} ms** `bake_albedo_with` (4096², detail, up to 3 threads; result not written to disk)."
        );
        out.push('\n');
    }

    out.push_str("## Offscreen frame loop (one process per area, no window)\n\n");
    if !render_requested {
        out.push_str("`[skip]` render phase disabled with `--no-render`.\n\n");
    } else if renders.is_empty() {
        out.push_str("`[skip]` render phase did not run (terrain for the render area unavailable; see the data table).\n\n");
    } else {
        out.push_str("| area | frames | mean ms | p50 ms | p95 ms | p99 ms | min ms | max ms | entities | mesh entities |\n");
        out.push_str("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
        for r in renders {
            if r.measured == 0 {
                let _ = writeln!(
                    out,
                    "| {:02} | 0 | `[skip]` no frames sampled ({}) | | | | | | | |",
                    r.area, r.exit_reason
                );
                continue;
            }
            let _ = writeln!(
                out,
                "| {:02} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {} | {} |",
                r.area,
                r.measured,
                r.mean_ms,
                r.p50_ms,
                r.p95_ms,
                r.p99_ms,
                r.min_ms,
                r.max_ms,
                r.entities,
                r.mesh_entities
            );
        }
        out.push('\n');
        for r in renders {
            if r.measured == 0 {
                continue;
            }
            let _ = writeln!(
                out,
                "- area {:02}: terrain mesh + baked albedo + `ZoneLightingPlugin` + streamed `.trins` trees{}{}; {} frames sampled after warm-up + settle (entity count stable for 60 frames). Setup inside the app: chunks {:.1} ms, navmesh {:.1} ms (anchor snap), albedo {:.1} ms; anchor ({:.0},{:.0}).",
                r.area,
                if r.tree_requested > 0 { format!(", {} tree instances from {} of {} models loaded", r.tree_instances, r.tree_models, r.tree_requested) } else { String::new() },
                match &r.grass {
                    Some((chunks, draw, tris, inst)) => format!(", vegetation {chunks} chunks / {draw} draw calls / {tris} tris / {inst} instances"),
                    None => ", vegetation off".to_string(),
                },
                r.measured,
                r.chunks_ms,
                r.nav_ms,
                r.albedo_ms,
                r.anchor[0],
                r.anchor[1]
            );
            if let Some(v) = r.diag_frame_ms {
                let fps = r.fps.map(|f| format!(", {f:.1} fps")).unwrap_or_default();
                let _ = writeln!(out, "  - `FrameTimeDiagnosticsPlugin` smoothed frame time at exit: **{v:.2} ms**{fps}.");
            }
            if r.exit_reason != "ok" {
                let _ = writeln!(
                    out,
                    "  - **Exit reason: {}** — numbers may cover a partially settled scene.",
                    r.exit_reason
                );
            }
        }
        out.push_str("\nThis offscreen render measures the CPU loop (extract + render graph submission).\n\
                     With pipelined rendering the GPU work is asynchronous, so these numbers do NOT bound\n\
                     the GPU frame time; the real viewer's GPU frame time is **UNKNOWN** here.\n\n");
    }

    if let Some(cmp) = compare {
        out.push_str("## Before/after (same measurement, same machine)\n\n");
        let _ = writeln!(out, "Baseline: {}.", cmp.title);
        out.push_str(
            "Sub-second phases (load, navmesh, cached-albedo read) vary between runs with IO and\n\
             allocator state; the targeted phase is the one with the large, reproducible delta.\n\n",
        );
        let mut any = false;
        for m in metrics {
            let Some(before) = cmp.area.get(&m.area) else {
                continue;
            };
            any = true;
            let row = |name: &str, after: f64, unit: &str| -> String {
                match before.get(name).copied() {
                    Some(b) if b.is_finite() && after.is_finite() => {
                        format!("{b:.1} → {after:.1} {unit} ({})", delta(b, after))
                    }
                    _ => format!("{after:.1} {unit} (no baseline)"),
                }
            };
            let _ = writeln!(
                out,
                "- area {:02}: load {}, chunks {}, nav {}, albedo {}",
                m.area,
                row("load_ms", m.load_ms, "ms"),
                row("chunks_ms", m.chunks_ms, "ms"),
                row("nav_ms", m.nav_ms, "ms"),
                row("albedo_ms", m.albedo_ms.unwrap_or(f64::NAN), "ms"),
            );
        }
        for r in renders {
            if r.measured == 0 {
                continue;
            }
            if let Some(before) = cmp.render_mean.get(&r.area) {
                any = true;
                let _ = writeln!(
                    out,
                    "- area {:02} render frame loop mean: {before:.2} → {:.2} ms ({})",
                    r.area,
                    r.mean_ms,
                    delta(*before, r.mean_ms)
                );
            }
        }
        if !any {
            out.push_str("(no matching area rows in the baseline file)\n");
        }
        out.push('\n');
    }

    out.push_str("## Verified / Assumed / Unknown\n\n");
    out.push_str(
        "- **Verified by this measurement**: per-area load/chunk/navmesh/albedo wall-clock times, content counts, decoded mesh bytes, process RSS; the offscreen loop's CPU frame times are measured values of this scene on this machine.\n\
         - **Assumed**: the offscreen scene (one camera pose per area, radius-limited trees) is representative of the viewer's views; `RSS Δ` attribution ignores allocator reuse; the 60-fps budget below is a proposal, not a decoder fact.\n\
         - **Unknown**: GPU frame time of the windowed viewer; GPU memory; frame times with the player moving / streaming continuously (this run is static); behaviour on other hardware; whether the maintainer's improved terrain textures are active (`play.sh` overlays `work/texturas_mejoradas`, this run uses the plain cache).\n\n",
    );
    out.push_str("## Proposed budget (ASSUMED)\n\n");
    out.push_str(
        "60 fps = 16.6 ms per frame is proposed as the target until the maintainer defines one.\n\
         The offscreen CPU loop above stays under that figure only for the scenes measured here;\n\
         without GPU numbers this is not a verdict on the viewer's frame rate.\n",
    );

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(path, out) {
        Ok(()) => println!("[report] wrote {path:?}"),
        Err(e) => eprintln!("[report] cannot write {path:?}: {e}"),
    }
}

// ---------------------------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bin_root =
        PathBuf::from(std::env::var("PLA_BIN_ROOT").unwrap_or_else(|_| {
            arg(&args, "--bin-root").unwrap_or_else(|| DEFAULT_BIN_ROOT.into())
        }));
    let cache_root = PathBuf::from(
        std::env::var("PLA_CACHE_ROOT").unwrap_or_else(|_| DEFAULT_CACHE_ROOT.into()),
    );
    let work_root = cache_root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| cache_root.clone());

    let areas: Vec<u8> = arg(&args, "--areas")
        .map(|s| {
            s.split(',')
                .filter_map(|t| t.trim().parse::<u8>().ok())
                .collect()
        })
        .unwrap_or_else(|| (0..=5).collect());
    let out = arg(&args, "--out").unwrap_or_else(|| "reports/data-progress/performance.md".into());
    let raw_path = arg(&args, "--raw").map(PathBuf::from);
    let compare_path = arg(&args, "--compare").map(PathBuf::from);
    let no_render = flag(&args, "--no-render");
    let profile = if cfg!(debug_assertions) {
        "debug (NOT representative; the viewer runs release)"
    } else {
        "release"
    };

    if !bin_root
        .join("archive/field/terrain/ha_area00.gfpak")
        .exists()
    {
        println!("[skip] game packs not found under {bin_root:?}; nothing measured");
        return;
    }

    let mut metrics = Vec::new();
    for &area in &areas {
        eprintln!("[measure] area {area:02}...");
        let m = measure_area(area, &bin_root, &cache_root, &work_root);
        if let Some(e) = &m.error {
            eprintln!("[measure] area {area:02} error: {e}");
        }
        metrics.push(m);
    }

    let bake = arg(&args, "--bake-area")
        .and_then(|s| s.parse::<u8>().ok())
        .and_then(|a| measure_bake(a, &bin_root));

    let render_areas: Vec<u8> = arg(&args, "--render-areas")
        .or_else(|| arg(&args, "--render-area"))
        .map(|s| {
            s.split(',')
                .filter_map(|t| t.trim().parse::<u8>().ok())
                .collect()
        })
        .unwrap_or_else(|| vec![0]);
    let mut renders: Vec<RenderSummary> = Vec::new();
    if !no_render {
        println!("[render] starting windowless render measurement (no window will appear)");
        for area in render_areas {
            match run_render(&args, area, &bin_root, &cache_root, &work_root) {
                Some(r) => {
                    println!(
                        "[render] area {area:02}: mean {:.2} ms, p50 {:.2}, p95 {:.2}, p99 {:.2} over {} frames (entities {}, mesh entities {})",
                        r.mean_ms, r.p50_ms, r.p95_ms, r.p99_ms, r.measured, r.entities, r.mesh_entities
                    );
                    renders.push(r);
                }
                None => eprintln!("[render] area {area:02} skipped"),
            }
        }
    }

    if let Some(path) = &raw_path {
        write_raw(path, &metrics, &bake, &renders);
    }
    let compare = compare_path.and_then(|p| {
        std::fs::read_to_string(&p)
            .ok()
            .map(|text| parse_compare(&text, format!("`{}`", p.display())))
    });

    let env = Env {
        cpu: cpu_label(),
        bin_root: bin_root.display().to_string(),
        cache_root: cache_root.display().to_string(),
        profile: profile.into(),
        command: std::env::args().collect::<Vec<_>>().join(" "),
    };
    write_report(
        Path::new(&out),
        &env,
        &metrics,
        &bake,
        !no_render,
        &renders,
        &compare,
    );
}
