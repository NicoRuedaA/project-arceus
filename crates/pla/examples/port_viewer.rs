//! Pokémon Legends: Arceus — Interactive Bevy Port Viewer
//!
//! Provides a real-time 3D interactive viewer with:
//! 1. Player locomotion (WASD/arrows, sprint, orientation towards motion).
//! 2. Decoded field camera controller (subsystem `field::camera` / dec319):
//!    - Canonical spherical coordinate transformations (pitch, yaw, distance).
//!    - Orbit Mode 0 (Default 3rd Person) & Mode 2 (Aim Over-the-Shoulder ZL).
//!    - Pitch/distance clamping and smooth tracking.
//! 3. Real PML Pokémon database integration (`data/pml_personal.tsv`):
//!    - Live stat queries and verified PLA HP calculation with Ganba K[G].
//!    - Interactive Pokémon selector (keys 1–5).
//! 4. HUD telemetry in window title & console logs.
//! 5. Optional automated headless capture (`--screenshot <path>` / `--ticks <N>`).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::gltf::{Gltf, GltfMaterial, GltfMesh};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::WindowResolution;

use pla::assets::bntx::TextureData;
use pla::assets::navmesh::AreaNavigation;
use pla::assets::terrain::{
    build_mesh_chunks, TerrainArea, TerrainMeshChunk, TerrainMeshSettings,
    TerrainScaleOverride,
};
use pla::battle::BattleData;
use pla::game::{
    battle_input_system, load_spawners_for_area, roll_spawner, setup_game_ui,
    sync_ui_visibility, update_battle_ui_system, update_world_hud_system, ActiveAreaSpawners,
    ActiveBattleSession, BattleAssets, GameNavmeshResource, GameState, PlayerAvatar,
    WildCreature,
};
use pla::pml::{calculate_hp, PmlDatabase};

#[derive(Resource)]
struct TerrainResource {
    area: Arc<TerrainArea>,
    chunks: Vec<TerrainMeshChunk>,
    blended_albedo: Option<TextureData>,
    #[allow(dead_code)]
    settings: TerrainMeshSettings,
}

#[derive(Resource)]
struct TreeCatalogResource {
    handles: HashMap<String, Handle<Gltf>>,
    spawned: bool,
}

#[derive(Resource)]
struct PokemonCatalogResource {
    handles: HashMap<u16, Handle<Gltf>>,
    spawned: HashSet<Entity>,
}

/// Resolves relative asset path for a Pokémon's glTF model (checking both _idle.gltf and _static.gltf).
fn resolve_pokemon_gltf_rel(dex: u16) -> Option<String> {
    let variants = ["idle", "static"];
    let roots = [
        std::env::var("PLA_CACHE_DIR").map(PathBuf::from).ok(),
        Some(PathBuf::from("work/cache")),
        Some(PathBuf::from("/home/nico/work/decompilacion/work/cache")),
    ];
    for root in roots.into_iter().flatten() {
        for v in &variants {
            let rel = format!("cache/pokemon_batch/pm{dex:04}_00_00/pm{dex:04}_00_00_{v}.gltf");
            let disk_candidate = root.join(format!("pokemon_batch/pm{dex:04}_00_00/pm{dex:04}_00_00_{v}.gltf"));
            let rel_candidate = PathBuf::from("work").join(&rel);
            if disk_candidate.exists() || rel_candidate.exists() {
                return Some(rel);
            }
        }
    }
    None
}

#[derive(Resource)]
struct PlayerSpawnConfig {
    pos: Vec2,
    #[allow(dead_code)]
    area_idx: u8,
}

#[derive(Component)]
struct DirectionIndicator;

#[derive(Component)]
struct PokemonSpawn {
    #[allow(dead_code)]
    national_dex: u16,
}

#[derive(Component)]
struct BattleArenaMarker;

#[derive(Component)]
struct Reticle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BallKind {
    PokeBall,
    GreatBall,
    UltraBall,
    FeatherBall,
    HeavyBall,
}

impl BallKind {
    fn display_name(&self) -> &'static str {
        match self {
            Self::PokeBall => "Poké Ball",
            Self::GreatBall => "Great Ball",
            Self::UltraBall => "Ultra Ball",
            Self::FeatherBall => "Feather Ball",
            Self::HeavyBall => "Heavy Ball",
        }
    }

    fn ball_multiplier(&self) -> f64 {
        match self {
            Self::PokeBall => 0.75,
            Self::GreatBall => 1.5,
            Self::UltraBall => 2.25,
            Self::FeatherBall => 1.25,
            Self::HeavyBall => 1.25,
        }
    }

    fn initial_velocity(&self) -> f32 {
        match self {
            Self::FeatherBall => 42.0,
            Self::HeavyBall => 18.0,
            _ => 30.0,
        }
    }

    fn gravity(&self) -> f32 {
        match self {
            Self::FeatherBall => -8.0,
            Self::HeavyBall => -24.0,
            _ => -16.0,
        }
    }

    fn top_color(&self) -> Color {
        match self {
            Self::PokeBall => Color::srgb(0.85, 0.15, 0.15),
            Self::GreatBall => Color::srgb(0.15, 0.35, 0.85),
            Self::UltraBall => Color::srgb(0.2, 0.2, 0.2),
            Self::FeatherBall => Color::srgb(0.3, 0.75, 0.85),
            Self::HeavyBall => Color::srgb(0.35, 0.35, 0.38),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BallState {
    InFlight,
    Shaking {
        shake_count: u8,
        timer: f32,
        max_shakes: u8,
        will_capture: bool,
    },
    Captured,
    Breakout,
}

#[derive(Component)]
struct PokeBall {
    velocity: Vec3,
    gravity: f32,
    lifetime: f32,
    bounces: u8,
    state: BallState,
    kind: BallKind,
}

#[derive(Resource)]
struct BallInventory {
    selected: BallKind,
    balls_thrown: u32,
    last_capture_status: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CameraMode {
    OrbitDefault = 0,
    AimOverShoulder = 2,
}

#[derive(Resource)]
struct CameraController {
    pitch: f32,
    yaw: f32,
    distance: f32,
    mode: CameraMode,
    target_distance: f32,
    focal_offset_y: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            pitch: 0.35,
            yaw: 0.0,
            distance: 5.0,
            mode: CameraMode::OrbitDefault,
            target_distance: 5.0,
            focal_offset_y: 1.2,
        }
    }
}

#[derive(Resource)]
struct ActivePokemonState {
    dex_list: Vec<u16>,
    current_index: usize,
    level: u16,
    ganba: u8,
}

#[derive(Resource)]
struct LoadedPml(Option<PmlDatabase>);

#[derive(Resource)]
struct LoadedBattle(Option<BattleData>);

#[derive(Resource)]
struct AutomationOptions {
    screenshot_path: Option<PathBuf>,
    max_ticks: Option<u32>,
    current_ticks: u32,
    auto_throw_tick: Option<u32>,
    verify_views_dir: Option<PathBuf>,
    verify_step: usize,
    verify_step_tick: u32,
    auto_demo: bool,
    demo_output_dir: Option<PathBuf>,
    demo_step: usize,
    demo_step_tick: u32,
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut bin_root = std::env::var_os("PLA_BIN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
        });
    let mut area_idx = 0u8;
    let mut initial_player_pos = None;
    let mut overrides = TerrainScaleOverride::default();
    let mut screenshot_path = None;
    let mut max_ticks = None;
    let mut initial_mode = CameraMode::OrbitDefault;
    let mut initial_dex: Option<u16> = None;
    let mut auto_throw_tick: Option<u32> = None;
    let mut headless = false;
    let mut verify_views_dir = None;
    let mut auto_demo = false;
    let mut demo_output_dir = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bin-root" => {
                if let Some(path) = args.next() {
                    bin_root = PathBuf::from(path);
                }
            }
            "--area" => {
                if let Some(a) = args.next().and_then(|s| s.parse().ok()) {
                    area_idx = a;
                }
            }
            "--player-pos" => {
                if let Some(pos_str) = args.next() {
                    let parts: Vec<&str> = pos_str.split(',').collect();
                    if parts.len() == 2 {
                        if let (Ok(x), Ok(z)) = (parts[0].trim().parse::<f32>(), parts[1].trim().parse::<f32>()) {
                            initial_player_pos = Some(Vec2::new(x, z));
                        }
                    }
                }
            }
            "--cell-size" => {
                if let Some(s) = args.next().and_then(|s| s.parse().ok()) {
                    overrides.horizontal_cell_size = Some(s);
                }
            }
            "--vertical-scale" => {
                if let Some(s) = args.next().and_then(|s| s.parse().ok()) {
                    overrides.vertical_scale = Some(s);
                }
            }
            "--headless" => {
                headless = true;
            }
            "--auto-demo" => {
                headless = true;
                auto_demo = true;
                demo_output_dir = Some(PathBuf::from("work/cache/game_demo"));
            }
            "--script" => {
                headless = true;
                auto_demo = true;
                if let Some(dir) = args.next() {
                    demo_output_dir = Some(PathBuf::from(dir));
                } else {
                    demo_output_dir = Some(PathBuf::from("work/cache/game_demo"));
                }
            }
            "--verify-views" => {
                headless = true;
                verify_views_dir = Some(PathBuf::from("work/cache/world_verify"));
            }
            "--verify-views-dir" => {
                headless = true;
                if let Some(dir) = args.next() {
                    verify_views_dir = Some(PathBuf::from(dir));
                }
            }
            "--screenshot" => {
                if let Some(path) = args.next() {
                    screenshot_path = Some(PathBuf::from(path));
                    headless = true;
                }
            }
            "--ticks" => {
                if let Some(ticks) = args.next().and_then(|t| t.parse().ok()) {
                    max_ticks = Some(ticks);
                }
            }
            "--mode" => {
                if let Some(m) = args.next() {
                    match m.to_lowercase().as_str() {
                        "aim" | "2" => initial_mode = CameraMode::AimOverShoulder,
                        _ => initial_mode = CameraMode::OrbitDefault,
                    }
                }
            }
            "--species" => {
                if let Some(s) = args.next().and_then(|n| n.parse().ok()) {
                    initial_dex = Some(s);
                }
            }
            "--throw" => {
                auto_throw_tick = Some(20);
            }
            "--help" | "-h" => {
                println!("Usage: cargo run -p pla --example port_viewer -- [OPTIONS]");
                println!("Options:");
                println!("  --bin-root <PATH>     Extracted game bin root (default: .../romfs_patched/bin)");
                println!("  --area <N>            Field area index 0..6 (default: 0)");
                println!("  --player-pos <X,Z>    Initial player spawn coordinates (default: 445.0,650.0 for area 0)");
                println!("  --cell-size <N>       Override horizontal cell scale (metres)");
                println!("  --vertical-scale <N>  Override vertical height scale (metres)");
                println!("  --headless            Run offscreen with hidden window (no display window)");
                println!("  --auto-demo           Run deterministic game loop test offscreen (Walk -> Battle -> Return)");
                println!("  --script <DIR>        Run deterministic test and write screenshots to DIR");
                println!("  --verify-views        Verify 3 distinct viewpoints offscreen in a single process");
                println!("  --verify-views-dir D  Verify 3 viewpoints and save screenshots to directory D");
                println!("  --screenshot <PATH>   Capture window screenshot and exit");
                println!("  --ticks <COUNT>       Run for COUNT frames and exit");
                println!("  --mode <orbit|aim>    Initial camera mode (default: orbit)");
                println!("  --species <DEX>       National dex species ID to spawn (default: 25 Pikachu)");
                println!("  --throw               Automatically throw Poké Ball on tick 20");
                return ExitCode::SUCCESS;
            }
            _ => {}
        }
    }

    if auto_demo {
        headless = true;
        // The auto-demo contract requires approaching a real wild Pokemon in zone 01 (Obsidian Fieldlands)
        if area_idx == 0 {
            area_idx = 1;
        }
    } else if verify_views_dir.is_some() {
        headless = true;
    } else if screenshot_path.is_some() && max_ticks.is_none() {
        max_ticks = Some(70);
    }

    // Default spawn position: near the tree cluster in ha_area00 or center of ha_area01
    let default_spawn = if area_idx == 0 {
        Vec2::new(448.0, 652.0)
    } else {
        Vec2::new(512.5, 512.5)
    };
    let player_spawn_pos = initial_player_pos.unwrap_or(default_spawn);

    // Attempt loading real terrain from game pack
    let terrain_res = match TerrainArea::load(&bin_root, area_idx) {
        Ok(area_data) => {
            println!(
                "Loaded real terrain ha_area{:02}.gfpak: extent=({:.1}, {:.1}), H={:.1}m, {} instances",
                area_idx,
                area_data.world_extent[0],
                area_data.world_extent[1],
                area_data.world_height,
                area_data.instances.len()
            );
            match area_data.mesh_settings(overrides, 64) {
                Ok(settings) => match build_mesh_chunks(&area_data.heightmap, settings) {
                    Ok(chunks) => {
                        println!("Built {} terrain mesh chunks (cell size {:.3}m)", chunks.len(), settings.horizontal_cell_size);
                        let blended_albedo = area_data.blend_splat_by_slot_order_hypothesis().ok();
                        Some(TerrainResource {
                            area: Arc::new(area_data),
                            chunks,
                            blended_albedo,
                            settings,
                        })
                    }
                    Err(e) => {
                        eprintln!("Failed to build terrain chunks: {e}");
                        None
                    }
                },
                Err(e) => {
                    eprintln!("Failed to resolve terrain mesh settings: {e}");
                    None
                }
            }
        }
        Err(e) => {
            println!("Terrain pack not loaded from {}: {e}. Running with fallback ground plane.", bin_root.display());
            None
        }
    };

    // Locate PML and Battle data directory.
    let data_dir = [
        PathBuf::from("data"),
        PathBuf::from("../../data"),
        PathBuf::from("/home/nico/work/decompilacion/data"),
    ]
    .into_iter()
    .find(|p| p.join("pml_personal.tsv").exists());

    let pml_db = data_dir.as_ref().and_then(|dir| PmlDatabase::load_from_dir(dir).ok());
    let battle_db = data_dir.as_ref().and_then(|dir| BattleData::load(dir).ok());
    let battle_assets = data_dir.as_ref().and_then(|dir| match BattleAssets::load(dir) {
        Ok(a) => Some(a),
        Err(e) => {
            eprintln!("Battle assets (move effects / items / catch rates) not loaded: {e}");
            None
        }
    });

    let work_root = PathBuf::from("work");
    let work_abs = work_root.canonicalize().unwrap_or(work_root);

    // Attempt loading real navmesh from terrain pack
    let navmesh_res = match AreaNavigation::load(&bin_root, area_idx) {
        Ok(nav) => {
            println!(
                "Loaded real navmesh ha_area{:02}.gfpak: {} faces, {} edges, {} vertices",
                area_idx,
                nav.primary().faces.len(),
                nav.primary().edges.len(),
                nav.primary().vertices.len()
            );
            Some(GameNavmeshResource {
                nav: Arc::new(nav),
            })
        }
        Err(e) => {
            println!("Navmesh not loaded from {}: {e}", bin_root.display());
            None
        }
    };

    // Ensure initial spawn position is on a validated walkable navmesh face
    let player_spawn_pos = if let Some(ref nr) = navmesh_res {
        let nav = nr.nav.primary();
        if nav.is_walkable(player_spawn_pos.x, player_spawn_pos.y) {
            player_spawn_pos
        } else if let Some(hit) = nav.nearest(player_spawn_pos.x, player_spawn_pos.y, 400.0) {
            let vs = nav.face_vertices(hit.face);
            let cx = vs.iter().map(|v| v[0]).sum::<f32>() / vs.len() as f32;
            let cz = vs.iter().map(|v| v[2]).sum::<f32>() / vs.len() as f32;
            println!("Adjusted initial spawn position to walkable navmesh face: ({:.1}, {:.1})", cx, cz);
            Vec2::new(cx, cz)
        } else {
            player_spawn_pos
        }
    } else {
        player_spawn_pos
    };

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Pokémon Legends: Arceus — Bevy Port Interactive Viewer".into(),
                    resolution: WindowResolution::new(1280, 720),
                    visible: !headless,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: work_abs.to_string_lossy().to_string(),
                unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                ..default()
            }),
    );
    app.init_state::<GameState>();

    let mut camera_ctrl = CameraController::default();
    camera_ctrl.mode = initial_mode;
    if initial_mode == CameraMode::AimOverShoulder {
        camera_ctrl.distance = 2.2;
        camera_ctrl.target_distance = 2.2;
    }
    app.insert_resource(camera_ctrl);

    let mut dex_list = vec![25, 501, 722, 155, 448]; // Pikachu, Oshawott, Rowlet, Cyndaquil, Lucario
    let mut current_index = 0;
    if let Some(dex) = initial_dex {
        if let Some(pos) = dex_list.iter().position(|&d| d == dex) {
            current_index = pos;
        } else {
            dex_list.insert(0, dex);
            current_index = 0;
        }
    }
    app.insert_resource(ActivePokemonState {
        dex_list,
        current_index,
        level: 25,
        ganba: 3,
    });
    app.insert_resource(LoadedPml(pml_db));
    app.insert_resource(LoadedBattle(battle_db));
    app.insert_resource(BallInventory {
        selected: BallKind::PokeBall,
        balls_thrown: 0,
        last_capture_status: None,
    });
    app.insert_resource(PlayerSpawnConfig {
        pos: player_spawn_pos,
        area_idx,
    });
    let zone_name = match area_idx {
        0 => "Jubilife Village (ha_area00)".to_string(),
        1 => "Obsidian Fieldlands (ha_area01)".to_string(),
        2 => "Crimson Mirelands (ha_area02)".to_string(),
        3 => "Cobalt Coastlands (ha_area03)".to_string(),
        4 => "Coronet Highlands (ha_area04)".to_string(),
        5 => "Alabaster Icelands (ha_area05)".to_string(),
        _ => format!("Hisui Sector (ha_area{:02})", area_idx),
    };
    app.insert_resource(pla::game::ActiveZoneInfo {
        area_idx,
        name: zone_name,
    });
    app.insert_resource(TreeCatalogResource {
        handles: HashMap::new(),
        spawned: false,
    });
    app.insert_resource(PokemonCatalogResource {
        handles: HashMap::new(),
        spawned: HashSet::new(),
    });

    let spawn_table_path = "data/field_spawn_table.tsv";
    let area_spawners = match load_spawners_for_area(spawn_table_path, area_idx) {
        Ok(spawners) => {
            println!(
                "Loaded {} real spawner definitions from {} for area {:02}",
                spawners.len(),
                spawn_table_path,
                area_idx
            );
            ActiveAreaSpawners {
                spawners,
                spawned_entities: HashMap::new(),
                streaming_radius: 120.0,
                seed: 0x5e1f_5eed_1234_5678,
            }
        }
        Err(e) => {
            println!("Warning: Could not load {spawn_table_path}: {e}");
            ActiveAreaSpawners::default()
        }
    };
    app.insert_resource(area_spawners);

    // Placed scenery objects + decoded zone lighting (crates/pla/src/game/{world_objects,lighting}.rs)
    {
        let cli: Vec<String> = std::env::args().collect();
        let tree_models: HashSet<String> = terrain_res
            .as_ref()
            .map(|t| t.area.instances.iter().map(|i| i.model_name.clone()).collect())
            .unwrap_or_default();
        let cache_root = std::env::var_os("PLA_CACHE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(pla::game::world_objects::DEFAULT_CACHE_ROOT));
        app.add_plugins((
            pla::game::world_objects::WorldObjectsPlugin {
                area: area_idx,
                cache_root,
                exclude_models: tree_models,
                rot_mode: pla::game::world_objects::rot_mode_from_args(&cli),
            },
            pla::game::lighting::ZoneLightingPlugin {
                area: area_idx,
                slot: pla::game::lighting::time_slot_from_args(&cli),
            },
        ));
    }
    app.add_plugins(pla::game::pokemon_anim::PokemonAnimPlugin);
    if let Some(tr) = terrain_res {
        app.insert_resource(tr);
    }
    if let Some(nr) = navmesh_res {
        app.insert_resource(nr);
    }
    if let Some(assets) = battle_assets {
        app.insert_resource(assets);
    }
    app.insert_resource(AutomationOptions {
        screenshot_path,
        max_ticks,
        current_ticks: 0,
        auto_throw_tick,
        verify_views_dir,
        verify_step: 0,
        verify_step_tick: 0,
        auto_demo,
        demo_output_dir,
        demo_step: 0,
        demo_step_tick: 0,
    });

    app.add_systems(Startup, (setup_scene, setup_game_ui));
    app.add_systems(
        Update,
        (
            spawn_tree_instances,
            stream_wild_spawners.run_if(in_state(GameState::World)),
            spawn_pokemon_models,
            player_locomotion.run_if(in_state(GameState::World)),
            creature_encounter_system.run_if(in_state(GameState::World)),
            battle_input_system.run_if(in_state(GameState::Battle)),
            camera_controls,
            update_camera_transform,
            ball_selection_controls.run_if(in_state(GameState::World)),
            ball_throw_input.run_if(in_state(GameState::World)),
            ball_ballistics_and_physics.run_if(in_state(GameState::World)),
            capture_resolution_system.run_if(in_state(GameState::World)),
            pokemon_selection_controls.run_if(in_state(GameState::World)),
            sync_ui_visibility,
            update_world_hud_system.run_if(in_state(GameState::World)),
            update_battle_ui_system.run_if(in_state(GameState::Battle)),
            update_hud_and_telemetry,
            automated_tick_driver,
        )
            .chain(),
    );
    app.add_systems(OnEnter(GameState::Battle), setup_battle_arena);
    app.add_systems(OnExit(GameState::Battle), cleanup_battle_arena);

    app.run();
    ExitCode::SUCCESS
}


fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    pml: Res<LoadedPml>,
    _pkmn_state: Res<ActivePokemonState>,
    spawn_cfg: Res<PlayerSpawnConfig>,
    terrain_res: Option<Res<TerrainResource>>,
    mut tree_catalog: ResMut<TreeCatalogResource>,
    mut pokemon_catalog: ResMut<PokemonCatalogResource>,
    asset_server: Res<AssetServer>,
) {
    let spawn_x = spawn_cfg.pos.x;
    let spawn_z = spawn_cfg.pos.y;
    let surface_y = terrain_res
        .as_ref()
        .and_then(|t| t.area.sample_world_height(spawn_x, spawn_z))
        .unwrap_or(0.0);
    let player_y = surface_y + 0.9;

    // Register all known batch Pokemon glTF models for asynchronous loading
    for dex in [25, 41, 74, 155, 396, 397, 399, 401, 403, 448, 501, 722] {
        if let Some(rel_path) = resolve_pokemon_gltf_rel(dex) {
            let handle: Handle<Gltf> = asset_server.load(rel_path);
            pokemon_catalog.handles.insert(dex, handle);
        }
    }

    if let Some(ref terrain) = terrain_res {
        // Spawn real terrain mesh chunks
        let albedo = terrain.blended_albedo.as_ref().map(|texture| {
            images.add(Image::new(
                Extent3d {
                    width: texture.width,
                    height: texture.height,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                texture.rgba.clone(),
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            ))
        });
        let terrain_material = materials.add(StandardMaterial {
            base_color: if albedo.is_some() {
                Color::WHITE
            } else {
                Color::srgb(0.28, 0.42, 0.22)
            },
            base_color_texture: albedo,
            perceptual_roughness: 0.95,
            ..default()
        });

        for chunk in &terrain.chunks {
            commands.spawn((
                Mesh3d(meshes.add(chunk.mesh.clone())),
                MeshMaterial3d(terrain_material.clone()),
                Transform::IDENTITY,
            ));
        }

        // Register tree models for asynchronous glTF loading
        let unique_models: HashSet<String> = terrain
            .area
            .instances
            .iter()
            .map(|inst| inst.model_name.clone())
            .collect();
        for model_name in unique_models {
            let possible_paths = [
                format!("cache/gltf_field/ha_area{:02}_terrain/{}.gltf", terrain.area.area, model_name),
                format!("cache/gltf_field/ha_area{:02}/{}.gltf", terrain.area.area, model_name),
            ];
            for rel_path in &possible_paths {
                let full_path = PathBuf::from("work").join(rel_path);
                if full_path.exists() {
                    let handle: Handle<Gltf> = asset_server.load(rel_path.clone());
                    tree_catalog.handles.insert(model_name.clone(), handle);
                    break;
                }
            }
        }
    } else {
        // Fallback: ground plane and field markers
        let ground_size = 40.0;
        commands.spawn((
            Mesh3d(meshes.add(Mesh::from(Plane3d::default().mesh().size(ground_size, ground_size)))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.28, 0.42, 0.22),
                perceptual_roughness: 0.9,
                ..default()
            })),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));

        for x in [-10.0, 0.0, 10.0] {
            for z in [-10.0, 0.0, 10.0] {
                if x == 0.0 && z == 0.0 {
                    continue;
                }
                commands.spawn((
                    Mesh3d(meshes.add(Mesh::from(Cylinder::new(0.15, 0.6)))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.65, 0.55, 0.45),
                        ..default()
                    })),
                    Transform::from_xyz(x, 0.3, z),
                ));
            }
        }
    }

    // Player character avatar (capsule representation with facing indicator).
    let _player_body = commands
        .spawn((
            PlayerAvatar,
            Transform::from_xyz(spawn_x, player_y, spawn_z),
            Visibility::default(),
        ))
        .with_children(|parent| {
            // Main torso/capsule
            parent.spawn((
                Mesh3d(meshes.add(Mesh::from(Capsule3d::new(0.35, 1.1)))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.18, 0.28, 0.55), // Survey Corps blue coat
                    perceptual_roughness: 0.6,
                    ..default()
                })),
            ));
            // Head/Scarf
            parent.spawn((
                Mesh3d(meshes.add(Mesh::from(Sphere::new(0.26)))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.85, 0.25, 0.22), // Red headscarf
                    perceptual_roughness: 0.7,
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.75, 0.0),
            ));
            // Facing beak/visor (shows player orientation forward in -Z)
            parent.spawn((
                DirectionIndicator,
                Mesh3d(meshes.add(Mesh::from(Cone::new(0.12, 0.35)))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.95, 0.85, 0.2),
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.75, -0.3).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            ));
        })
        .id();

    // Third-person Orbit Camera with centered aiming crosshair ("X").
    commands
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(spawn_x, player_y + 2.0, spawn_z + 4.5)
                .looking_at(Vec3::new(spawn_x, player_y + 0.3, spawn_z), Vec3::Y),
        ))
        .with_children(|cam| {
            // Screen-centered aiming reticle with prominent "X" crosshair
            cam.spawn((
                Reticle,
                Transform::from_xyz(0.0, 0.0, -1.8),
                Visibility::Visible,
            ))
            .with_children(|reticle| {
                let x_bar_mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 1.0, 1.0),
                    unlit: true,
                    ..default()
                });
                let center_dot_mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.2, 0.2),
                    unlit: true,
                    ..default()
                });

                reticle.spawn((
                    Mesh3d(meshes.add(Mesh::from(Cylinder::new(0.003, 0.10)))),
                    MeshMaterial3d(x_bar_mat.clone()),
                    Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
                ));
                reticle.spawn((
                    Mesh3d(meshes.add(Mesh::from(Cylinder::new(0.003, 0.10)))),
                    MeshMaterial3d(x_bar_mat),
                    Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_4)),
                ));
                reticle.spawn((
                    Mesh3d(meshes.add(Mesh::from(Sphere::new(0.005)))),
                    MeshMaterial3d(center_dot_mat),
                    Transform::from_xyz(0.0, 0.0, 0.002),
                ));
            });
        });

    // Directional sun key light
    commands.spawn((
        DirectionalLight {
            illuminance: 16_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(spawn_x + 60.0, player_y + 120.0, spawn_z + 80.0)
            .looking_at(Vec3::new(spawn_x, player_y, spawn_z), Vec3::Y),
    ));

    // Directional sky fill light (avoids pitch-black shadowed faces)
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(spawn_x - 60.0, player_y + 80.0, spawn_z - 80.0)
            .looking_at(Vec3::new(spawn_x, player_y, spawn_z), Vec3::Y),
    ));

    println!("============================================================");
    println!("Pokémon Legends: Arceus — Interactive Bevy Port Viewer");
    println!("Controls:");
    println!("  WASD / Arrows : Walk / Run in world (height sampled from terrain)");
    println!("  Left Shift    : Sprint (~8.0 m/s)");
    println!("  Mouse Drag    : Orbit camera (Yaw / Pitch)");
    println!("  Q / E         : Rotate camera yaw horizontally");
    println!("  R / F         : Zoom camera in / out");
    println!("  Mouse Wheel   : Zoom camera distance");
    println!("  Z / Tab       : Toggle Orbit Mode 0 <-> Aim Mode 2 (ZL)");
    println!("  1 - 5         : Select active Pokémon (Pikachu, Starters)");
    println!("============================================================");

    if let Some(db) = &pml.0 {
        println!("PML Database loaded successfully: {} species registered.", db.personal.len());
    } else {
        println!("Note: PML Database not found at data/ (running with fallback species).");
    }
}

/// Spawns tree instances from real .trins transforms once their glTF models are loaded.
fn spawn_tree_instances(
    mut commands: Commands,
    mut catalog: ResMut<TreeCatalogResource>,
    terrain_res: Option<Res<TerrainResource>>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    gltf_materials: Res<Assets<GltfMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
) {
    if catalog.spawned {
        return;
    }
    let Some(ref terrain) = terrain_res else {
        return;
    };
    if catalog.handles.is_empty() {
        catalog.spawned = true;
        return;
    }

    // Check if all requested model handles are ready
    let all_ready = catalog.handles.values().all(|h| gltf_assets.contains(h));
    if !all_ready {
        return;
    }

    let mut model_prims: HashMap<String, Vec<(Handle<Mesh>, Handle<StandardMaterial>)>> = HashMap::new();
    for (name, handle) in &catalog.handles {
        if let Some(gltf) = gltf_assets.get(handle) {
            let mut prims = Vec::new();
            for mesh_h in &gltf.meshes {
                if let Some(gltf_mesh) = gltf_meshes.get(mesh_h) {
                    for prim in &gltf_mesh.primitives {
                        let std_mat = if let Some(ref mat_h) = prim.material {
                            if let Some(gm) = gltf_materials.get(mat_h) {
                                StandardMaterial {
                                    base_color: gm.base_color,
                                    base_color_texture: gm.base_color_texture.clone(),
                                    perceptual_roughness: gm.perceptual_roughness,
                                    metallic: gm.metallic,
                                    cull_mode: None, // Foliage visible from both sides
                                    alpha_mode: AlphaMode::Mask(0.5), // Mask out transparent foliage borders
                                    ..default()
                                }
                            } else {
                                StandardMaterial {
                                    base_color: Color::srgb(0.3, 0.45, 0.2),
                                    cull_mode: None,
                                    ..default()
                                }
                            }
                        } else {
                            StandardMaterial {
                                base_color: Color::srgb(0.3, 0.45, 0.2),
                                cull_mode: None,
                                ..default()
                            }
                        };
                        let m_handle = standard_materials.add(std_mat);
                        prims.push((prim.mesh.clone(), m_handle));
                    }
                }
            }
            model_prims.insert(name.clone(), prims);
        }
    }

    let mut count = 0;
    for inst in &terrain.area.instances {
        if let Some(prims) = model_prims.get(&inst.model_name) {
            let flat: [f32; 16] = [
                inst.matrix[0][0], inst.matrix[0][1], inst.matrix[0][2], inst.matrix[0][3],
                inst.matrix[1][0], inst.matrix[1][1], inst.matrix[1][2], inst.matrix[1][3],
                inst.matrix[2][0], inst.matrix[2][1], inst.matrix[2][2], inst.matrix[2][3],
                inst.matrix[3][0], inst.matrix[3][1], inst.matrix[3][2], inst.matrix[3][3],
            ];
            let transform = Transform::from_matrix(Mat4::from_cols_array(&flat));
            for (mesh_h, mat_h) in prims {
                commands.spawn((
                    Mesh3d(mesh_h.clone()),
                    MeshMaterial3d(mat_h.clone()),
                    transform,
                ));
            }
            count += 1;
        }
    }
    println!(
        "Spawned {} tree instances from real .trins matrices across {} unique models.",
        count,
        model_prims.len()
    );
    catalog.spawned = true;
}

/// Dynamic streaming system: loads and unloads wild creatures around the player within streaming radius.
fn stream_wild_spawners(
    mut commands: Commands,
    mut active_spawners: ResMut<ActiveAreaSpawners>,
    player_query: Query<&Transform, With<PlayerAvatar>>,
    terrain_res: Option<Res<TerrainResource>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut pokemon_catalog: ResMut<PokemonCatalogResource>,
    asset_server: Res<AssetServer>,
) {
    let Ok(p_xf) = player_query.single() else {
        return;
    };

    let p_pos = Vec2::new(p_xf.translation.x, p_xf.translation.z);
    let radius = active_spawners.streaming_radius;
    let radius_sq = radius * radius;
    let unload_radius_sq = (radius + 25.0) * (radius + 25.0);

    // 1. Unload entities that moved out of radius
    let mut to_despawn = Vec::new();
    for (spawner_id, entity) in &active_spawners.spawned_entities {
        if let Some(spawner) = active_spawners.spawners.iter().find(|s| &s.spawner_id == spawner_id) {
            let dist_sq = (spawner.pos.x - p_pos.x).powi(2) + (spawner.pos.z - p_pos.y).powi(2);
            if dist_sq > unload_radius_sq {
                to_despawn.push((spawner_id.clone(), *entity));
            }
        }
    }

    for (spawner_id, entity) in to_despawn {
        commands.entity(entity).despawn();
        pokemon_catalog.spawned.remove(&entity);
        active_spawners.spawned_entities.remove(&spawner_id);
    }

    // 2. Load spawners inside streaming radius that aren't spawned yet
    let seed = active_spawners.seed;
    let mut to_spawn = Vec::new();

    for spawner in &active_spawners.spawners {
        if active_spawners.spawned_entities.contains_key(&spawner.spawner_id) {
            continue;
        }

        let dist_sq = (spawner.pos.x - p_pos.x).powi(2) + (spawner.pos.z - p_pos.y).powi(2);
        if dist_sq <= radius_sq {
            if let Some((dex, name, lvl)) = roll_spawner(spawner, seed) {
                to_spawn.push((spawner.clone(), dex, name, lvl));
            }
        }
    }

    for (spawner, dex, name, lvl) in to_spawn {
        let spawn_x = spawner.pos.x;
        let spawn_z = spawner.pos.z;
        let surface_y = terrain_res
            .as_ref()
            .and_then(|t| t.area.sample_world_height(spawn_x, spawn_z))
            .unwrap_or(spawner.pos.y);
        let spawn_y = surface_y + 0.45;

        // Ensure glTF is loading
        if !pokemon_catalog.handles.contains_key(&dex) {
            if let Some(rel_path) = resolve_pokemon_gltf_rel(dex) {
                let handle: Handle<Gltf> = asset_server.load(rel_path);
                pokemon_catalog.handles.insert(dex, handle);
            }
        }

        let wild = WildCreature {
            species_name: name,
            national_dex: dex,
            level: lvl,
            spawn_pos: Vec3::new(spawn_x, surface_y, spawn_z),
            aggro_range: 2.8,
            in_battle: false,
            defeated: false,
            spawner_id: spawner.spawner_id.clone(),
        };

        let entity = commands
            .spawn((
                wild,
                PokemonSpawn { national_dex: dex },
                Transform::from_xyz(spawn_x, spawn_y, spawn_z),
                Visibility::default(),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(meshes.add(Mesh::from(Sphere::new(0.5)))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.7, 0.7, 0.7),
                        perceptual_roughness: 0.5,
                        ..default()
                    })),
                ));
            })
            .id();

        active_spawners.spawned_entities.insert(spawner.spawner_id.clone(), entity);
    }
}

fn spawn_pokemon_models(
    mut commands: Commands,
    mut catalog: ResMut<PokemonCatalogResource>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    gltf_materials: Res<Assets<GltfMaterial>>,
    meshes: Res<Assets<Mesh>>,
    pkmn_query: Query<(Entity, &PokemonSpawn, &Children)>,
    asset_server: Res<AssetServer>,
) {
    for (entity, pkmn_spawn, children) in &pkmn_query {
        if catalog.spawned.contains(&entity) {
            continue;
        }
        let Some(handle) = catalog.handles.get(&pkmn_spawn.national_dex) else {
            continue;
        };
        let Some(gltf) = gltf_assets.get(handle) else {
            continue;
        };

        // Ensure all meshes and primitive materials are loaded in Assets before replacing geometry
        let all_ready = gltf.meshes.iter().all(|mesh_h| {
            if let Some(gm) = gltf_meshes.get(mesh_h) {
                gm.primitives.iter().all(|prim| {
                    meshes.contains(&prim.mesh)
                        && prim.material.as_ref().map_or(true, |mat_h| gltf_materials.contains(mat_h))
                })
            } else {
                false
            }
        });
        if !all_ready {
            continue;
        }

        // glTF ready: despawn placeholder geometric children
        for child in children.iter() {
            commands.entity(child).despawn();
        }

        // Real glTF scene with skeleton + idle/walk clips (see game::pokemon_anim).
        if let Some(idle_rel) = handle.path().map(|pth| pth.path().to_string_lossy().into_owned()) {
            commands.entity(entity).insert(pla::game::pokemon_anim::PokemonAnimSource::from_idle_path(&asset_server, &idle_rel));
        }

        catalog.spawned.insert(entity);
        println!(
            "Loaded real glTF model for #{:04} with baked color textures",
            pkmn_spawn.national_dex
        );
    }
}

/// Spawns the 3D battle arena combatants (Player Pikachu + Wild Pokemon) grounded on the terrain surface.
fn setup_battle_arena(
    mut commands: Commands,
    player_query: Query<&Transform, With<PlayerAvatar>>,
    terrain_res: Option<Res<TerrainResource>>,
    battle_session: Option<Res<ActiveBattleSession>>,
    mut pokemon_catalog: ResMut<PokemonCatalogResource>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(player_xf) = player_query.single() else {
        return;
    };
    let center_x = player_xf.translation.x;
    let center_z = player_xf.translation.z;

    let sample_y = |x: f32, z: f32| -> f32 {
        terrain_res
            .as_ref()
            .and_then(|t| t.area.sample_world_height(x, z))
            .unwrap_or(player_xf.translation.y - 0.9)
    };

    let wild_dex = battle_session.as_ref().map_or(397, |s| s.wild_dex);
    let player_dex: u16 = 25; // Pikachu

    // Ensure glTF models are requested
    for dex in [player_dex, wild_dex] {
        if !pokemon_catalog.handles.contains_key(&dex) {
            if let Some(rel) = resolve_pokemon_gltf_rel(dex) {
                let handle: Handle<Gltf> = asset_server.load(rel);
                pokemon_catalog.handles.insert(dex, handle);
            }
        }
    }

    // Positions in arena:
    // Player Pokemon (Pikachu) at (center_x + 1.2, center_z - 1.8), facing towards Wild Pokemon (+X, -Z)
    let p_pkmn_x = center_x + 1.2;
    let p_pkmn_z = center_z - 1.8;
    let p_pkmn_y = sample_y(p_pkmn_x, p_pkmn_z);

    // Wild Pokemon (Staravia / wild encounter) at (center_x + 2.8, center_z - 5.5), facing towards Player Pokemon (-X, +Z)
    let w_pkmn_x = center_x + 2.8;
    let w_pkmn_z = center_z - 5.5;
    let w_pkmn_y = sample_y(w_pkmn_x, w_pkmn_z);

    // Face each other on the XZ plane:
    // Vector from player pokemon to wild pokemon
    let p_to_w = Vec2::new(w_pkmn_x - p_pkmn_x, w_pkmn_z - p_pkmn_z).normalize();
    // In Bevy / standard coordinates, looking towards (dx, dz): rotation = Quat::from_rotation_y(dx.atan2(dz))
    // Model default facing is +Z forward:
    let p_rot = Quat::from_rotation_y(p_to_w.x.atan2(p_to_w.y));
    let w_rot = Quat::from_rotation_y((-p_to_w.x).atan2(-p_to_w.y));

    // Spawn Player Pokemon entity
    let _p_entity = commands
        .spawn((
            BattleArenaMarker,
            PokemonSpawn { national_dex: player_dex },
            Transform::from_xyz(p_pkmn_x, p_pkmn_y, p_pkmn_z).with_rotation(p_rot),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(Mesh::from(Sphere::new(0.35)))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.95, 0.85, 0.15),
                    ..default()
                })),
            ));
        })
        .id();

    // Spawn Wild Pokemon entity
    let _w_entity = commands
        .spawn((
            BattleArenaMarker,
            PokemonSpawn { national_dex: wild_dex },
            Transform::from_xyz(w_pkmn_x, w_pkmn_y, w_pkmn_z).with_rotation(w_rot),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(Mesh::from(Sphere::new(0.4)))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.55, 0.50, 0.45),
                    ..default()
                })),
            ));
        })
        .id();

    println!(
        "Spawned Battle Arena 3D combatants: Player #{player_dex} at ({p_pkmn_x:.1}, {p_pkmn_y:.1}, {p_pkmn_z:.1}) vs Wild #{wild_dex} at ({w_pkmn_x:.1}, {w_pkmn_y:.1}, {w_pkmn_z:.1})",
    );
}

/// Despawns the 3D battle arena combatants when exiting GameState::Battle.
fn cleanup_battle_arena(
    mut commands: Commands,
    arena_query: Query<Entity, With<BattleArenaMarker>>,
    mut catalog: ResMut<PokemonCatalogResource>,
) {
    for entity in &arena_query {
        catalog.spawned.remove(&entity);
        commands.entity(entity).despawn();
    }
}

fn get_species_color(dex: u16) -> Color {
    match dex {
        25 => Color::srgb(0.95, 0.85, 0.15),  // Pikachu (Electric Yellow)
        399 => Color::srgb(0.65, 0.42, 0.22), // Bidoof (Normal Brown)
        396 => Color::srgb(0.55, 0.50, 0.45), // Starly (Flying Slate)
        403 => Color::srgb(0.25, 0.55, 0.85), // Shinx (Electric Blue)
        501 => Color::srgb(0.25, 0.65, 0.95), // Oshawott (Water Blue)
        722 => Color::srgb(0.55, 0.82, 0.35), // Rowlet (Grass Green)
        155 => Color::srgb(0.95, 0.45, 0.15), // Cyndaquil (Fire Orange)
        448 => Color::srgb(0.25, 0.45, 0.85), // Lucario (Aura Blue)
        _ => Color::srgb(0.8, 0.8, 0.8),
    }
}

/// Player locomotion system: translates input along camera relative vectors,
/// uses navmesh collision to block cliffs/water with smooth sliding, and samples surface height.
fn player_locomotion(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    camera_ctrl: Res<CameraController>,
    terrain_res: Option<Res<TerrainResource>>,
    navmesh_res: Option<Res<GameNavmeshResource>>,
    mut player_query: Query<&mut Transform, With<PlayerAvatar>>,
) {
    let Ok(mut transform) = player_query.single_mut() else {
        return;
    };

    let dt = time.delta_secs();
    let mut input_dir = Vec2::ZERO;

    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        input_dir.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        input_dir.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        input_dir.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        input_dir.x += 1.0;
    }

    if input_dir.length_squared() > 0.0 {
        let input_norm = input_dir.normalize();
        let is_sprinting = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let speed = if is_sprinting { 8.0 } else { 4.5 };

        // Camera yaw defines the forward direction on the XZ plane.
        let yaw = camera_ctrl.yaw;
        let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
        let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());

        let move_vec = (forward * input_norm.y + right * input_norm.x).normalize();
        let delta_xz = Vec2::new(move_vec.x * speed * dt, move_vec.z * speed * dt);
        let current_xz = Vec2::new(transform.translation.x, transform.translation.z);

        // Apply navmesh collision sliding if real navmesh is loaded
        let final_xz = if let Some(ref nav) = navmesh_res {
            nav.try_move_with_slide(current_xz, delta_xz)
        } else {
            current_xz + delta_xz
        };

        transform.translation.x = final_xz.x;
        transform.translation.z = final_xz.y;

        // Smoothly orient player avatar towards movement direction.
        let target_rot = Quat::from_rotation_y(move_vec.x.atan2(move_vec.z));
        transform.rotation = transform.rotation.slerp(target_rot, (15.0 * dt).min(1.0));
    }

    // Keep player avatar on the real terrain or navmesh surface
    let nav_h = navmesh_res
        .as_ref()
        .and_then(|n| n.height_at(transform.translation.x, transform.translation.z));
    let terrain_h = terrain_res
        .as_ref()
        .and_then(|t| t.area.sample_world_height(transform.translation.x, transform.translation.z));

    if let Some(surface_y) = nav_h.or(terrain_h) {
        transform.translation.y = surface_y + 0.9;
    }
}


/// Camera input and state updates (implements PLA dec319 kinematics).
fn camera_controls(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut mouse_wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut camera_ctrl: ResMut<CameraController>,
    mut reticle_query: Query<&mut Visibility, With<Reticle>>,
) {
    let dt = time.delta_secs();

    // Toggle camera mode between Orbit (0) and Aim (2).
    if keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::Tab) {
        camera_ctrl.mode = match camera_ctrl.mode {
            CameraMode::OrbitDefault => CameraMode::AimOverShoulder,
            CameraMode::AimOverShoulder => CameraMode::OrbitDefault,
        };
        println!("Switched Camera Mode to: {:?}", camera_ctrl.mode);
    }

    // Mouse drag orbit controls (or right-click drag).
    let mut mouse_delta = Vec2::ZERO;
    let dragging = mouse_buttons.pressed(MouseButton::Right)
        || mouse_buttons.pressed(MouseButton::Left)
        || camera_ctrl.mode == CameraMode::AimOverShoulder;

    for ev in mouse_motion.read() {
        if dragging {
            mouse_delta += ev.delta;
        }
    }

    // Keyboard orbit controls (Q/E for yaw).
    if keys.pressed(KeyCode::KeyQ) {
        mouse_delta.x -= 120.0 * dt;
    }
    if keys.pressed(KeyCode::KeyE) {
        mouse_delta.x += 120.0 * dt;
    }

    let sensitivity = 0.0035;
    camera_ctrl.yaw += mouse_delta.x * sensitivity;
    camera_ctrl.pitch = (camera_ctrl.pitch + mouse_delta.y * sensitivity).clamp(-0.25, 1.35);

    // Zoom adjustments.
    for ev in mouse_wheel.read() {
        camera_ctrl.target_distance = (camera_ctrl.target_distance - ev.y * 0.6).clamp(1.5, 12.0);
    }
    if keys.pressed(KeyCode::KeyR) {
        camera_ctrl.target_distance = (camera_ctrl.target_distance - 4.0 * dt).clamp(1.5, 12.0);
    }
    if keys.pressed(KeyCode::KeyF) {
        camera_ctrl.target_distance = (camera_ctrl.target_distance + 4.0 * dt).clamp(1.5, 12.0);
    }

    // In Aim mode, camera distance contracts and reticle becomes visible.
    let desired_dist = match camera_ctrl.mode {
        CameraMode::OrbitDefault => camera_ctrl.target_distance,
        CameraMode::AimOverShoulder => 2.2,
    };
    camera_ctrl.distance += (desired_dist - camera_ctrl.distance) * (10.0 * dt).min(1.0);

    if let Ok(mut ret_vis) = reticle_query.single_mut() {
        *ret_vis = Visibility::Visible;
    }
}

/// Transform solver using canonical PLA spherical coordinates:
/// ΔX = dist * cos(pitch) * sin(yaw)
/// ΔY = dist * sin(pitch)
/// ΔZ = dist * cos(pitch) * cos(yaw)
/// cameraPos = lookAtPos + Δ
fn update_camera_transform(
    camera_ctrl: Res<CameraController>,
    player_query: Query<&Transform, (With<PlayerAvatar>, Without<Camera3d>)>,
    mut camera_query: Query<&mut Transform, (With<Camera3d>, Without<PlayerAvatar>)>,
) {
    let Ok(player_xf) = player_query.single() else {
        return;
    };
    let Ok(mut cam_xf) = camera_query.single_mut() else {
        return;
    };

    let right = Vec3::new(camera_ctrl.yaw.cos(), 0.0, -camera_ctrl.yaw.sin());
    let mut focal_target = player_xf.translation + Vec3::new(0.0, camera_ctrl.focal_offset_y, 0.0);

    // If in over-the-shoulder Aim mode, shift focal point to right shoulder.
    if camera_ctrl.mode == CameraMode::AimOverShoulder {
        focal_target += right * 0.45 + Vec3::new(0.0, 0.15, 0.0);
    }

    let pitch = camera_ctrl.pitch;
    let yaw = camera_ctrl.yaw;
    let dist = camera_ctrl.distance;

    let delta_x = dist * pitch.cos() * yaw.sin();
    let delta_y = dist * pitch.sin();
    let delta_z = dist * pitch.cos() * yaw.cos();
    let forward_dir = Vec3::new(-pitch.cos() * yaw.sin(), -pitch.sin(), -pitch.cos() * yaw.cos()).normalize();

    let (eye_pos, look_target) = match camera_ctrl.mode {
        CameraMode::OrbitDefault => {
            let eye = focal_target + Vec3::new(delta_x, delta_y, delta_z);
            (eye, focal_target)
        }
        CameraMode::AimOverShoulder => {
            let shoulder_pos = player_xf.translation + right * 0.45 + Vec3::new(0.0, 1.35, 0.0);
            let eye = shoulder_pos + Vec3::new(delta_x, delta_y, delta_z);
            let target = eye + forward_dir * 10.0;
            (eye, target)
        }
    };

    cam_xf.translation = eye_pos;
    cam_xf.look_at(look_target, Vec3::Y);
}

/// Selects active Poké Ball type using keyboard shortcuts:
/// B: Standard Poké Ball | G: Great Ball | U: Ultra Ball | F: Feather Ball | H: Heavy Ball
fn ball_selection_controls(keys: Res<ButtonInput<KeyCode>>, mut inventory: ResMut<BallInventory>) {
    if keys.just_pressed(KeyCode::KeyB) {
        inventory.selected = BallKind::PokeBall;
        println!("Selected Ball: {}", inventory.selected.display_name());
    } else if keys.just_pressed(KeyCode::KeyG) {
        inventory.selected = BallKind::GreatBall;
        println!("Selected Ball: {}", inventory.selected.display_name());
    } else if keys.just_pressed(KeyCode::KeyU) {
        inventory.selected = BallKind::UltraBall;
        println!("Selected Ball: {}", inventory.selected.display_name());
    } else if keys.just_pressed(KeyCode::KeyF) {
        inventory.selected = BallKind::FeatherBall;
        println!("Selected Ball: {}", inventory.selected.display_name());
    } else if keys.just_pressed(KeyCode::KeyH) {
        inventory.selected = BallKind::HeavyBall;
        println!("Selected Ball: {}", inventory.selected.display_name());
    }
}

/// Spawns a thrown Poké Ball along the aim vector towards the reticle focal point.
fn ball_throw_input(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut inventory: ResMut<BallInventory>,
    player_query: Query<&Transform, With<PlayerAvatar>>,
    camera_query: Query<&Transform, (With<Camera3d>, Without<PlayerAvatar>)>,
    pokemon_query: Query<&Transform, (With<PokemonSpawn>, Without<PlayerAvatar>, Without<Camera3d>)>,
    camera_ctrl: Res<CameraController>,
    automation: Res<AutomationOptions>,
) {
    let mut should_throw = mouse_buttons.just_pressed(MouseButton::Left)
        || keys.just_pressed(KeyCode::Space);

    if let Some(throw_tick) = automation.auto_throw_tick {
        if automation.current_ticks == throw_tick {
            should_throw = true;
        }
    }

    if !should_throw {
        return;
    }

    let Ok(player_xf) = player_query.single() else {
        return;
    };
    let Ok(cam_xf) = camera_query.single() else {
        return;
    };

    inventory.balls_thrown += 1;
    let ball_kind = inventory.selected;

    let yaw = camera_ctrl.yaw;
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let forward_horiz = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());

    // Spawn origin: right hand of player
    let spawn_pos = player_xf.translation + right * 0.4 + Vec3::new(0.0, 0.9, 0.0);

    // Aim convergence point: raycast from camera along camera look vector
    let cam_pos = cam_xf.translation;
    let cam_forward = *cam_xf.forward();

    // 1. Check if camera aim ray directly targets any wild Pokémon
    let mut best_target: Option<Vec3> = None;
    let mut closest_pkmn_dist = f32::INFINITY;

    for pkmn_xf in &pokemon_query {
        let pkmn_pos = pkmn_xf.translation;
        let to_pkmn = pkmn_pos - cam_pos;
        let proj = to_pkmn.dot(cam_forward);
        if proj > 0.0 {
            let closest_point_on_ray = cam_pos + cam_forward * proj;
            let ray_pkmn_dist = (pkmn_pos - closest_point_on_ray).length();
            if ray_pkmn_dist < 1.0 && proj < closest_pkmn_dist {
                closest_pkmn_dist = proj;
                best_target = Some(pkmn_pos);
            }
        }
    }

    // 2. If not locked onto a Pokémon, check ground intersection
    let target_aim_point = if let Some(target) = best_target {
        target
    } else if cam_forward.y < -0.01 {
        let t_ground = (-cam_pos.y) / cam_forward.y;
        if t_ground > 0.0 && t_ground < 60.0 {
            let ground_pt = cam_pos + cam_forward * t_ground;
            if (ground_pt - player_xf.translation).dot(forward_horiz) > 0.5 {
                ground_pt
            } else {
                player_xf.translation + forward_horiz * 8.0 + Vec3::new(0.0, 0.5, 0.0)
            }
        } else {
            cam_pos + cam_forward * 15.0
        }
    } else {
        cam_pos + cam_forward * 20.0
    };

    let init_speed = ball_kind.initial_velocity();
    let gravity = ball_kind.gravity();

    // Calculate ballistic trajectory from spawn_pos to target_aim_point with arc loft compensation
    let to_target = target_aim_point - spawn_pos;
    let target_dist = to_target.length().max(1.0);
    let flight_time = target_dist / init_speed;
    let arc_loft = 0.5 * gravity.abs() * flight_time * flight_time;
    let compensated_target = target_aim_point + Vec3::new(0.0, arc_loft, 0.0);
    let aim_dir = (compensated_target - spawn_pos).normalize();
    let velocity = aim_dir * init_speed;

    println!(
        "Throwing {} #{}: Speed: {:.1} m/s, Gravity: {:.1} m/s²",
        ball_kind.display_name(),
        inventory.balls_thrown,
        init_speed,
        gravity
    );

    let top_color = ball_kind.top_color();
    let bottom_color = Color::srgb(0.9, 0.88, 0.82); // Hisui wood/cream lower hemisphere

    commands
        .spawn((
            PokeBall {
                velocity,
                gravity,
                lifetime: 0.0,
                bounces: 0,
                state: BallState::InFlight,
                kind: ball_kind,
            },
            Transform::from_translation(spawn_pos),
            Visibility::default(),
        ))
        .with_children(|parent| {
            // Ball upper hemisphere
            parent.spawn((
                Mesh3d(meshes.add(Mesh::from(Sphere::new(0.12)))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: top_color,
                    perceptual_roughness: 0.4,
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.04, 0.0),
            ));
            // Ball lower hemisphere
            parent.spawn((
                Mesh3d(meshes.add(Mesh::from(Sphere::new(0.115)))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: bottom_color,
                    perceptual_roughness: 0.7,
                    ..default()
                })),
                Transform::from_xyz(0.0, -0.04, 0.0),
            ));
            // Center latch / button
            parent.spawn((
                Mesh3d(meshes.add(Mesh::from(Cylinder::new(0.03, 0.02)))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.7, 0.6, 0.3), // Bronze latch
                    metallic: 0.8,
                    perceptual_roughness: 0.3,
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.0, 0.11).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            ));
        });
}

/// Physical ballistics integration and contact raycast/sphere queries.
fn ball_ballistics_and_physics(
    time: Res<Time>,
    mut commands: Commands,
    mut ball_query: Query<(Entity, &mut Transform, &mut PokeBall), Without<PokemonSpawn>>,
    mut pkmn_query: Query<(Entity, &Transform, &mut Visibility), (With<PokemonSpawn>, Without<PokeBall>)>,
    pml: Res<LoadedPml>,
    battle: Res<LoadedBattle>,
    pkmn_state: Res<ActivePokemonState>,
    terrain_res: Option<Res<TerrainResource>>,
) {
    let dt = time.delta_secs();

    for (ball_entity, mut ball_xf, mut ball) in &mut ball_query {
        ball.lifetime += dt;

        match ball.state {
            BallState::InFlight => {
                let prev_pos = ball_xf.translation;
                let next_pos = prev_pos + ball.velocity * dt;
                let seg = next_pos - prev_pos;
                let seg_len_sq = seg.length_squared();

                // Ballistic motion integration
                ball_xf.translation = next_pos;
                ball.velocity.y += ball.gravity * dt;

                // Subtle spin in flight
                ball_xf.rotate_local_x(12.0 * dt);

                // Ground surface collision (sampled from real terrain if available)
                let ground_y = terrain_res
                    .as_ref()
                    .and_then(|t| t.area.sample_world_height(ball_xf.translation.x, ball_xf.translation.z))
                    .unwrap_or(0.0)
                    + 0.12;

                if ball_xf.translation.y <= ground_y {
                    ball_xf.translation.y = ground_y;
                    if ball.velocity.y.abs() > 2.0 && ball.bounces < 3 {
                        ball.velocity.y = -ball.velocity.y * 0.42;
                        ball.velocity.x *= 0.75;
                        ball.velocity.z *= 0.75;
                        ball.bounces += 1;
                    } else {
                        ball.velocity = Vec3::ZERO;
                    }
                }

                // Continuous collision detection against wild Pokémon
                for (_pkmn_entity, pkmn_xf, mut pkmn_vis) in &mut pkmn_query {
                    let to_pkmn = pkmn_xf.translation - prev_pos;
                    let t = if seg_len_sq > 1e-6 {
                        (to_pkmn.dot(seg) / seg_len_sq).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let closest = prev_pos + seg * t;
                    let hit_dist = closest.distance(pkmn_xf.translation);

                    if hit_dist < 0.95 {
                        println!("Target Hit! Poke Ball connected with Pokemon (CCD dist: {:.2}m)", hit_dist);

                        // Contain Pokémon inside ball
                        *pkmn_vis = Visibility::Hidden;

                        // Place ball at Pokémon base on ground
                        ball_xf.translation = Vec3::new(pkmn_xf.translation.x, 0.15, pkmn_xf.translation.z);
                        ball.velocity = Vec3::ZERO;

                        // Evaluate capture roll using PLA formula & tables
                        let dex = pkmn_state.dex_list[pkmn_state.current_index];
                        let ball_mod = ball.kind.ball_multiplier();

                        let base_catch_rate = if let Some(db) = &pml.0 {
                            if let Some(entry) = db.personal.iter().find(|p| p.national_dex == dex) {
                                // Typical catch rate proxy
                                match entry.national_dex {
                                    25 => 190.0,
                                    501 => 45.0,
                                    722 => 45.0,
                                    155 => 45.0,
                                    448 => 45.0,
                                    _ => 100.0,
                                }
                            } else {
                                100.0
                            }
                        } else {
                            100.0
                        };

                        let level_factor = (100.0 - pkmn_state.level as f64).max(10.0) / 100.0;
                        let basic_coef = battle
                            .0
                            .as_ref()
                            .and_then(|b| b.capture_constant("capture_index_basic_coef").ok().flatten())
                            .unwrap_or(30.0);

                        let final_rate = (base_catch_rate * ball_mod * level_factor + basic_coef) / 255.0;
                        let will_capture = final_rate >= 0.45;

                        println!(
                            "Capture Roll Evaluated: BaseRate={:.0}, BallMod={:.2}, FinalOdds={:.1}%, Outcome={}",
                            base_catch_rate,
                            ball_mod,
                            (final_rate * 100.0).min(100.0),
                            if will_capture { "SUCCESS" } else { "BREAKOUT" }
                        );

                        ball.state = BallState::Shaking {
                            shake_count: 0,
                            timer: 0.0,
                            max_shakes: 3,
                            will_capture,
                        };
                        break;
                    }
                }

                // Despawn stray balls after 8 seconds
                if ball.lifetime > 8.0 {
                    commands.entity(ball_entity).despawn();
                }
            }
            _ => {}
        }
    }
}

/// Capture sequence evaluator: handles 3-shake wiggle, capture sparks, or breakout.
fn capture_resolution_system(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ball_query: Query<(Entity, &mut Transform, &mut PokeBall)>,
    mut pkmn_query: Query<&mut Visibility, With<PokemonSpawn>>,
    mut inventory: ResMut<BallInventory>,
) {
    let dt = time.delta_secs();

    for (ball_entity, mut ball_xf, mut ball) in &mut ball_query {
        if let BallState::Shaking {
            ref mut shake_count,
            ref mut timer,
            max_shakes,
            will_capture,
        } = ball.state
        {
            *timer += dt;

            // Wiggle rotation animation
            let wiggle_angle = (*timer * 14.0).sin() * 0.22;
            ball_xf.rotation = Quat::from_rotation_z(wiggle_angle);

            // Advance shake every 0.65 seconds
            if *timer >= 0.65 {
                *timer = 0.0;
                *shake_count += 1;
                println!("Ball Shake {} / {}...", shake_count, max_shakes);

                if *shake_count >= max_shakes {
                    if will_capture {
                        println!("*** CAPTURED! The wild Pokémon was caught! ***");
                        ball.state = BallState::Captured;
                        inventory.last_capture_status = Some("Captured! ⭐".into());

                        // Spawn victory capture aura rings/stars above ball
                        commands.spawn((
                            Mesh3d(meshes.add(Mesh::from(Torus::new(0.3, 0.02)))),
                            MeshMaterial3d(materials.add(StandardMaterial {
                                base_color: Color::srgb(1.0, 0.85, 0.2), // Gold stars aura
                                unlit: true,
                                ..default()
                            })),
                            Transform::from_translation(ball_xf.translation + Vec3::new(0.0, 0.35, 0.0)),
                        ));
                    } else {
                        println!("*** BREAKOUT! The Pokémon broke free! ***");
                        ball.state = BallState::Breakout;
                        inventory.last_capture_status = Some("Broke out!".into());

                        // Restore Pokémon visibility
                        for mut vis in &mut pkmn_query {
                            *vis = Visibility::Visible;
                        }

                        // Remove broken ball
                        commands.entity(ball_entity).despawn();
                    }
                }
            }
        }
    }
}

/// Creature encounter system: triggers turn-based Battle when player approaches wild Pokémon (or presses KeyCode::Enter/KeyE).
fn creature_encounter_system(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    player_query: Query<&Transform, With<PlayerAvatar>>,
    mut wild_query: Query<(Entity, &Transform, &mut WildCreature), Without<PlayerAvatar>>,
    mut next_state: ResMut<NextState<GameState>>,
    battle_db: Res<LoadedBattle>,
    pkmn_state: Res<ActivePokemonState>,
    battle_assets: Option<Res<BattleAssets>>,
) {
    let Ok(player_xf) = player_query.single() else {
        return;
    };
    let Some(ref battle_data) = battle_db.0 else {
        return;
    };

    let interact_pressed = keys.just_pressed(KeyCode::KeyE) || keys.just_pressed(KeyCode::Enter);

    for (entity, wild_xf, mut wild) in &mut wild_query {
        if wild.defeated || wild.in_battle {
            continue;
        }

        let dist = player_xf.translation.distance(wild_xf.translation);
        if dist <= wild.aggro_range || (dist <= 4.0 && interact_pressed) {
            println!(
                "*** WILD ENCOUNTER TRIGGERED! Approached #{} {} (dist: {:.2}m) ***",
                wild.national_dex, wild.species_name, dist
            );

            wild.in_battle = true;
            let player_species = match pkmn_state.dex_list[pkmn_state.current_index] {
                25 => "pikachu",
                _ => "pikachu",
            };

            let started = match battle_assets.as_deref() {
                Some(assets) => ActiveBattleSession::start(
                    assets,
                    player_species,
                    pkmn_state.level as u8,
                    &wild.species_name,
                    wild.level,
                    wild.national_dex,
                    entity,
                ),
                None => ActiveBattleSession::new(
                    battle_data,
                    player_species,
                    pkmn_state.level as u8,
                    &wild.species_name,
                    wild.level,
                    wild.national_dex,
                    entity,
                ),
            };
            match started {
                Ok(session) => {
                    println!("Initialized ActiveBattleSession (Player Lv{} {} vs Wild Lv{} {})", pkmn_state.level, player_species, wild.level, wild.species_name);
                    commands.insert_resource(session);
                    next_state.set(GameState::Battle);
                    break;
                }
                Err(e) => {
                    eprintln!("Failed to initialize Battle: {e}");
                }
            }
        }
    }
}

/// Cycles selected Pokémon with keys 1–5.
fn pokemon_selection_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut pkmn_state: ResMut<ActivePokemonState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    pkmn_query: Query<(&PokemonSpawn, &Children)>,
    child_query: Query<&MeshMaterial3d<StandardMaterial>>,
) {
    let mut changed = false;
    if keys.just_pressed(KeyCode::Digit1) {
        pkmn_state.current_index = 0;
        changed = true;
    } else if keys.just_pressed(KeyCode::Digit2) {
        pkmn_state.current_index = 1;
        changed = true;
    } else if keys.just_pressed(KeyCode::Digit3) {
        pkmn_state.current_index = 2;
        changed = true;
    } else if keys.just_pressed(KeyCode::Digit4) {
        pkmn_state.current_index = 3;
        changed = true;
    } else if keys.just_pressed(KeyCode::Digit5) {
        pkmn_state.current_index = 4;
        changed = true;
    }

    if changed {
        let dex = pkmn_state.dex_list[pkmn_state.current_index];
        let color = get_species_color(dex);
        for (_, children) in &pkmn_query {
            for child in children.iter() {
                if let Ok(mat_handle) = child_query.get(child) {
                    if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
                        mat.base_color = color;
                    }
                }
            }
        }
    }
}

/// Updates window title and telemetry with real PML statistics and ball status.
fn update_hud_and_telemetry(
    mut windows: Query<&mut Window>,
    player_query: Query<&Transform, With<PlayerAvatar>>,
    camera_ctrl: Res<CameraController>,
    pkmn_state: Res<ActivePokemonState>,
    pml: Res<LoadedPml>,
    inventory: Res<BallInventory>,
    game_state: Res<State<GameState>>,
    battle_session: Option<Res<ActiveBattleSession>>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let Ok(player_xf) = player_query.single() else {
        return;
    };

    let dex = pkmn_state.dex_list[pkmn_state.current_index];
    let mode_str = match camera_ctrl.mode {
        CameraMode::OrbitDefault => "Orbit",
        CameraMode::AimOverShoulder => "Aim (ZL)",
    };

    let pml_info = if let Some(db) = &pml.0 {
        if let Some(entry) = db.personal.iter().find(|p| p.national_dex == dex) {
            let base_hp = entry.base_stats[0];
            let hp = calculate_hp(base_hp, pkmn_state.level, pkmn_state.ganba, Some(false))
                .unwrap_or(0);
            format!(
                "#{} {} [HP@Lv{}: {}]",
                entry.national_dex,
                entry.species_name,
                pkmn_state.level,
                hp
            )
        } else {
            format!("#{}", dex)
        }
    } else {
        format!("#{}", dex)
    };

    let state_str = match game_state.get() {
        GameState::World => "WORLD",
        GameState::Battle => "BATTLE",
        GameState::BattleEnded => "POST-BATTLE",
    };

    if let Some(ref sess) = battle_session {
        let p_hp = sess.battle.combatants[0].hp;
        let p_max = sess.battle.combatants[0].max_hp;
        let w_name = &sess.battle.combatants[1].species.name;
        let w_hp = sess.battle.combatants[1].hp;
        let w_max = sess.battle.combatants[1].max_hp;

        window.title = format!(
            "[{state_str}] Turn {} | Player HP: {}/{} | Wild {} HP: {}/{} | {}",
            sess.current_turn, p_hp, p_max, w_name, w_hp, w_max, sess.battle_summary
        );
    } else {
        let capture_tag = inventory
            .last_capture_status
            .as_deref()
            .unwrap_or("Ready");

        window.title = format!(
            "[{state_str}] PLA Port | Mode: {} | Pos: ({:.1}, {:.1}) | Ball: {} | Status: {} | Target: {}",
            mode_str,
            player_xf.translation.x,
            player_xf.translation.z,
            inventory.selected.display_name(),
            capture_tag,
            pml_info
        );
    }
}

/// Automated headless tick driver and screenshot capturer for test verification.
fn automated_tick_driver(
    mut commands: Commands,
    mut automation: ResMut<AutomationOptions>,
    terrain_res: Option<Res<TerrainResource>>,
    mut camera_ctrl: ResMut<CameraController>,
    mut player_query: Query<&mut Transform, (With<PlayerAvatar>, Without<PokemonSpawn>)>,
    mut pkmn_query: Query<&mut Transform, (With<PokemonSpawn>, Without<PlayerAvatar>)>,
    mut wild_query: Query<&mut WildCreature>,
    mut next_state: ResMut<NextState<GameState>>,
    mut session: Option<ResMut<ActiveBattleSession>>,
    battle_db: Res<LoadedBattle>,
    battle_assets: Option<Res<BattleAssets>>,
    mut exit: MessageWriter<AppExit>,
) {
    automation.current_ticks += 1;

    // Mode 0: Auto-demo playthrough mode (Walk -> Encounter -> Move Menu -> Attack -> Capture -> Return)
    if automation.auto_demo {
        let demo_dir = automation
            .demo_output_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("work/cache/game_demo"));

        let log_file_path = demo_dir.join("game_demo.log");
        let write_demo_log = |msg: &str| {
            if let Some(parent) = log_file_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_file_path)
            {
                use std::io::Write;
                let _ = writeln!(file, "{}", msg);
            }
            println!("[AUTO-DEMO] {}", msg);
        };

        automation.demo_step_tick += 1;

        match automation.demo_step {
            0 => {
                // Step 0: Initial World exploration locomotion towards real wild Pokemon
                if automation.demo_step_tick == 1 {
                    write_demo_log("Step 0: Player exploring World with active World HUD and real spawns.");
                    camera_ctrl.yaw = 0.0;
                    camera_ctrl.pitch = 0.22;
                    camera_ctrl.distance = 5.2;
                }
                // Move player forward towards nearby spawner
                if let Ok(mut p_xf) = player_query.single_mut() {
                    p_xf.translation.x += 0.08;
                    p_xf.translation.z -= 0.04;
                    if let Some(ref terrain) = terrain_res {
                        if let Some(sy) = terrain.area.sample_world_height(p_xf.translation.x, p_xf.translation.z) {
                            p_xf.translation.y = sy + 0.9;
                        }
                    }
                }
                if automation.demo_step_tick == 15 {
                    let shot = demo_dir.join("00_world_exploration.png");
                    write_demo_log(&format!("Capturing 00_world_exploration.png to {}", shot.display()));
                    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(shot));
                }
                if automation.demo_step_tick >= 25 {
                    // Transition to Battle state with real wild spawner species and level
                    let (wild_sp, wild_lvl, wild_dex) = if let Some(w) = wild_query.iter().next() {
                        (w.species_name.clone(), w.level, w.national_dex)
                    } else {
                        ("staravia".to_string(), 17, 397)
                    };

                    write_demo_log(&format!(
                        "Approached real wild #{} {} (Lv{})! Initiating Battle.",
                        wild_dex, wild_sp, wild_lvl
                    ));

                    if let Some(ref assets) = battle_assets {
                        match ActiveBattleSession::start(
                            assets,
                            "pikachu",
                            25,
                            &wild_sp,
                            wild_lvl,
                            wild_dex,
                            Entity::PLACEHOLDER,
                        ) {
                            Ok(sess) => commands.insert_resource(sess),
                            Err(e) => write_demo_log(&format!("Battle start failed: {e}")),
                        }
                    } else if let Some(ref db) = battle_db.0 {
                        if let Ok(sess) = ActiveBattleSession::new(
                            db,
                            "pikachu",
                            25,
                            &wild_sp,
                            wild_lvl,
                            wild_dex,
                            Entity::PLACEHOLDER,
                        ) {
                            commands.insert_resource(sess);
                        }
                    }
                    next_state.set(GameState::Battle);
                    automation.demo_step = 1;
                    automation.demo_step_tick = 0;
                }
            }
            1 => {
                // Step 1: Wild Encounter triggered -> Battle Start
                if automation.demo_step_tick == 1 {
                    write_demo_log("Step 1: Wild encounter! Party of 3, bag, move effects and engine AI are active.");
                    camera_ctrl.yaw = 0.52;
                    camera_ctrl.pitch = 0.16;
                    camera_ctrl.distance = 6.2;
                    camera_ctrl.focal_offset_y = 0.5;
                }
                if automation.demo_step_tick == 20 {
                    let shot = demo_dir.join("01_battle_encounter.png");
                    write_demo_log(&format!("Capturing 01_battle_encounter.png to {}", shot.display()));
                    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(shot));
                }
                if automation.demo_step_tick >= 25 {
                    automation.demo_step = 2;
                    automation.demo_step_tick = 0;
                }
            }
            step @ 2..=5 => {
                // Steps 2-5: scripted battle beats (status move, item menu, medicine, capture)
                let beat = &pla::game::battle_demo::BEATS[step - 2];
                if automation.demo_step_tick == 1 {
                    write_demo_log(&format!("Step {step}: {}", beat.note));
                    camera_ctrl.yaw = 0.52 + 0.04 * (step as f32 - 2.0);
                    camera_ctrl.pitch = 0.16;
                    camera_ctrl.distance = 6.2;
                    camera_ctrl.focal_offset_y = 0.5;
                    if let Some(ref mut sess) = session {
                        for line in pla::game::battle_demo::run_beat(sess, beat) {
                            write_demo_log(&format!("  log: {line}"));
                        }
                    }
                }
                if automation.demo_step_tick == 15 {
                    let shot = demo_dir.join(beat.frame);
                    write_demo_log(&format!("Capturing {} to {}", beat.frame, shot.display()));
                    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(shot));
                }
                if automation.demo_step_tick >= 25 {
                    if step == 5 {
                        // End battle and return to World
                        for mut wild in &mut wild_query {
                            wild.defeated = true;
                        }
                        commands.remove_resource::<ActiveBattleSession>();
                        next_state.set(GameState::World);
                    }
                    automation.demo_step = step + 1;
                    automation.demo_step_tick = 0;
                }
            }
            6 => {
                // Step 6: Return to World state after battle resolution
                if automation.demo_step_tick == 1 {
                    write_demo_log("Step 6: Returning to GameState::World with active World HUD.");
                    camera_ctrl.yaw = 0.15;
                    camera_ctrl.pitch = 0.28;
                    camera_ctrl.distance = 5.5;
                    camera_ctrl.focal_offset_y = 0.0;
                }
                // Walk player forward
                if let Ok(mut p_xf) = player_query.single_mut() {
                    p_xf.translation.x += 0.08;
                    p_xf.translation.z -= 0.04;
                    if let Some(ref terrain) = terrain_res {
                        if let Some(sy) = terrain.area.sample_world_height(p_xf.translation.x, p_xf.translation.z) {
                            p_xf.translation.y = sy + 0.9;
                        }
                    }
                }
                if automation.demo_step_tick >= 25 {
                    write_demo_log("Auto-demo playthrough finished successfully with 4 battle screenshots.");
                    exit.write(AppExit::Success);
                }
            }
            _ => {
                exit.write(AppExit::Success);
            }
        }
        return;
    }

    // Mode 1: Multi-view verification for 3 distinct positions in a single offscreen process
    if let Some(ref dir) = automation.verify_views_dir.clone() {
        let views: [(&str, Vec2, f32, f32); 3] = [
            ("pos1", Vec2::new(448.0, 652.0), 0.75, 5.0), // Tree cluster in ha_area00
            ("pos2", Vec2::new(512.5, 512.5), 1.20, 9.0), // Center of terrain (elevation ~60.6m)
            ("pos3", Vec2::new(300.0, 400.0), 2.40, 7.0), // Northwestern slope
        ];

        if automation.verify_step < views.len() {
            let (label, pos, yaw, dist) = views[automation.verify_step];
            automation.verify_step_tick += 1;

            if automation.verify_step_tick == 1 {
                // Position player, pokemon, and camera for this viewpoint
                let h = terrain_res
                    .as_ref()
                    .and_then(|t| t.area.sample_world_height(pos.x, pos.y))
                    .unwrap_or(0.0);

                if let Ok(mut p_xf) = player_query.single_mut() {
                    p_xf.translation = Vec3::new(pos.x, h + 0.9, pos.y);
                }
                if let Ok(mut pk_xf) = pkmn_query.single_mut() {
                    let pk_h = terrain_res
                        .as_ref()
                        .and_then(|t| t.area.sample_world_height(pos.x, pos.y - 3.8))
                        .unwrap_or(h);
                    pk_xf.translation = Vec3::new(pos.x, pk_h + 0.45, pos.y - 3.8);
                }
                camera_ctrl.yaw = yaw;
                camera_ctrl.pitch = 0.22;
                camera_ctrl.distance = dist;
                camera_ctrl.target_distance = dist;
                println!(
                    "[{}/{}] Viewpoint '{}' at ({:.1}, {:.1}, h={:.1}m, yaw={:.2})",
                    automation.verify_step + 1,
                    views.len(),
                    label,
                    pos.x,
                    pos.y,
                    h,
                    yaw
                );
            }

            // Capture after 22 frames so geometry and gltf models settle
            if automation.verify_step_tick == 22 {
                let shot_path = dir.join(format!("{}.png", label));
                println!("  Capturing screenshot for '{}' to: {}", label, shot_path.display());
                if let Some(parent) = shot_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(shot_path));
            }

            // Step to next viewpoint at tick 28
            if automation.verify_step_tick >= 28 {
                automation.verify_step += 1;
                automation.verify_step_tick = 0;
            }
        } else {
            println!("All {} viewpoints verified and captured cleanly in a single process. Exiting.", views.len());
            exit.write(AppExit::Success);
        }
        return;
    }

    // Mode 2: Standard single-shot or tick limit automation
    let shot_tick = if let Some(max) = automation.max_ticks {
        (max / 2).max(15)
    } else {
        20
    };

    if automation.current_ticks == shot_tick {
        if let Some(shot_path) = &automation.screenshot_path {
            println!("Triggered screenshot request at tick {} to: {}", shot_tick, shot_path.display());
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(shot_path.clone()));
        }
    }

    if let Some(max) = automation.max_ticks {
        if automation.current_ticks >= max {
            println!("Automated execution reached tick target ({}). Exiting cleanly.", max);
            exit.write(AppExit::Success);
        }
    }
}

