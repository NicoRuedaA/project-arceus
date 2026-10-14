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

// The start-up guard at the end builds this file as a test, where `main` is unused.
#![cfg_attr(test, allow(dead_code, unused_imports))]

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

use pla::assets::navmesh::AreaNavigation;
use pla::assets::terrain::{build_mesh_chunks, TerrainArea, TerrainScaleOverride};
use pla::battle::BattleData;
use pla::game::ball_lifecycle::{
    step_ball, BallBody, BallEvent, BallLifecycleState,
};
use pla::battle_engine::{CaptureModifiers, XorShift32};
use pla::game::capture_world::{
    bait_in_range, bait_multiplier, build_caught_member, is_backstrike, roll_capture, spawn_bait,
    store_caught, suitable_range, BaitObject, BaitTable, CatchStorage, WorldCaptureInput,
    BAIT_FEED_RADIUS,
};
use pla::game::catch_rate::CAPTURE_CATCH_RATE_PLACEHOLDER;
use pla::game::input_map::{Action, ActionState};
use pla::game::save_wire::{note_encounter, PlayerProgress};
use pla::game::throwing::{
    consume_ball, consume_item, launch_velocity, pick_aim_point, spawn_ball, BallInventory,
    BallKind, BallVisuals, PokeBall, ThrowParams, ThrowableTable,
};
use pla::game::wild::ConsumedSpawners;
use pla::game::main_menu::MainMenuPlugin;
use pla::game::pause_menu::PauseMenuPlugin;
use pla::game::settings::UserSettings;
use pla::game::state::{not_paused, Paused};
use pla::game::water::{PlayerSwim, SwimState, WaterBodies, SWIM_ENTER_DEPTH, SWIM_SPEED};
use pla::game::zone::{TerrainChunkMarker, TerrainResource};
use pla::game::{
    battle_input_system, load_spawners_for_area, roll_spawner_variant, setup_game_ui,
    sync_ui_visibility, update_battle_ui_system, update_world_hud_system, ActiveAreaSpawners,
    ActiveBattleSession, BattleAssets, GameNavmeshResource, GameState, PlayerAvatar, WildCreature,
};
use pla::pml::{calculate_hp, PmlDatabase};

#[derive(Resource, Clone)]
struct BinRootResource(PathBuf);

#[derive(Resource)]
struct TreeCatalogResource {
    handles: HashMap<String, Handle<Gltf>>,
    /// Main area whose `.trins` instances are spawned; trees reload when it changes (Brief 8b).
    spawned_area: Option<u8>,
}

/// One spawned `.trins` tree entity (one per glTF primitive); removed when the area changes.
#[derive(Component)]
struct TrinsTreeInstance;

#[derive(Resource)]
struct PokemonCatalogResource {
    handles: HashMap<PokemonAssetKey, Handle<Gltf>>,
    spawned: HashSet<Entity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PokemonAssetKey {
    dex: u16,
    form: u16,
    rare: bool,
}

/// Resolves a model path without silently substituting another form or palette.
/// Spawn evidence currently falls back to form 0 and rare=false when absent.
fn resolve_pokemon_gltf_rel(dex: u16, form: u16, rare: bool) -> Option<String> {
    let stem = format!("pm{dex:04}_{form:02}_00");
    let variants: &[&str] = if rare {
        &["rare_idle", "rare_static"]
    } else {
        &["idle", "static"]
    };
    let roots = [
        std::env::var("PLA_CACHE_DIR").map(PathBuf::from).ok(),
        Some(PathBuf::from("work/cache")),
        Some(PathBuf::from("/home/nico/work/decompilacion/work/cache")),
    ];
    for root in roots.into_iter().flatten() {
        for v in variants {
            let rel = format!("cache/pokemon_batch/{stem}/{stem}_{v}.gltf");
            let disk_candidate = root.join(format!("pokemon_batch/{stem}/{stem}_{v}.gltf"));
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
    national_dex: u16,
    form: u16,
    rare: bool,
    /// Retained for the spawn record even though alpha does not select a palette.
    #[allow(dead_code)]
    alpha: bool,
    /// The applied transform scale is kept here for inspection/debug tooling.
    #[allow(dead_code)]
    size_scale: f32,
}

#[derive(Component)]
struct BattleArenaMarker;

#[derive(Component)]
struct Reticle;

/// Screen-space hint shown only while aiming.
#[derive(Component)]
struct AimHint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CameraMode {
    OrbitDefault = 0,
    AimOverShoulder = 2,
}

/// Real player glTF (asset path under the work root), or `None` for the capsule avatar.
#[derive(Resource, Default)]
struct PlayerModelConfig {
    asset_rel: Option<String>,
}

#[derive(Resource)]
struct CameraController {
    pitch: f32,
    yaw: f32,
    distance: f32,
    mode: CameraMode,
    target_distance: f32,
    focal_offset_y: f32,
    /// Height of the aim-mode shoulder camera above the avatar root.
    shoulder_y: f32,
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
            shoulder_y: 1.35,
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
    controls_demo_dir: Option<PathBuf>,
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
    let mut controls_demo_dir: Option<PathBuf> = None;
    let mut player_variant = String::from("p1");
    let mut debug_zone_links = false;
    let mut main_menu = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--menu" => {
                main_menu = true;
            }
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
                        if let (Ok(x), Ok(z)) = (
                            parts[0].trim().parse::<f32>(),
                            parts[1].trim().parse::<f32>(),
                        ) {
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
            "--script-controls" => {
                // Hidden-window demo: world, pause menu open, aim mode with reticle (3 PNGs).
                headless = true;
                controls_demo_dir = Some(PathBuf::from(
                    args.next()
                        .unwrap_or_else(|| "work/cache/controls_demo".into()),
                ));
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
            "--player-variant" => {
                if let Some(v) = args.next() {
                    player_variant = v.to_lowercase();
                }
            }
            "--debug-zone-links" => {
                debug_zone_links = true;
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
                println!(
                    "  --headless            Run offscreen with hidden window (no display window)"
                );
                println!("  --auto-demo           Run deterministic game loop test offscreen (Walk -> Battle -> Return)");
                println!(
                    "  --script <DIR>        Run deterministic test and write screenshots to DIR"
                );
                println!("  --script-controls <DIR>  Hidden-window demo: world / pause menu / aim reticle PNGs in DIR");
                println!("  --verify-views        Verify 3 distinct viewpoints offscreen in a single process");
                println!("  --verify-views-dir D  Verify 3 viewpoints and save screenshots to directory D");
                println!("  --screenshot <PATH>   Capture window screenshot and exit");
                println!("  --ticks <COUNT>       Run for COUNT frames and exit");
                println!("  --player-variant <p1|p2>  Player glTF from $PLA_CACHE_ROOT/player (default p1; capsule if missing)");
                println!("  --debug-zone-links    P prints decoded door links near the player; T fast-travels to the next main area");
                println!("  --mode <orbit|aim>    Initial camera mode (default: orbit)");
                println!("  --species <DEX>       National dex species ID to spawn (default: 25 Pikachu)");
                println!("  --throw               Automatically throw Poké Ball on tick 20");
                println!("  --new-game            Ignore the existing save and start fresh");
                println!("  --menu                Start in the main menu (Continue / New game / Options / Exit)");
                println!("  --save-dir <PATH>     Save directory (sets PLA_SAVE_DIR); F5 saves");
                println!(
                    "  --no-audio            No sound and no audio device (same as PLA_NO_AUDIO=1)"
                );
                println!("  --effects <off|low|high>  Battle move effects (default low; PLA_EFFECTS, settings effects=)");
                println!("  --story <N> --event-time-scale <F> --distortion-now --outbreak <ID|off> --no-world-events  World events (Brief 39)");
                return ExitCode::SUCCESS;
            }
            _ => {}
        }
    }

    // Player save: --new-game / --save-dir; a loaded save supplies zone and position unless given.
    let cli_args: Vec<String> = std::env::args().collect();
    let wire_cli = pla::game::save_wire::WireCli::from_args(&cli_args);
    wire_cli.apply_env();
    let mut save_game = pla::game::save_game::SaveGame::from_env();
    let mut wire_start = pla::game::save_wire::load_startup(&mut save_game, &wire_cli);
    if wire_cli.has_time_slot {
        wire_start.progress.time_minutes = pla::game::clock::slot_centre_minutes(
            pla::game::lighting::time_slot_from_args(&cli_args),
        );
    }
    // Scripted runs have no save to shop with: stock the demo bag so --throw and the
    // controls demo still exercise the real bag counters (Brief 18).
    if auto_demo || controls_demo_dir.is_some() || auto_throw_tick.is_some() {
        for id in [1710u16, 1711, 1712, 1713, 1746, 1747, 1748, 1749, 1750] {
            *wire_start.progress.bag.entry(id).or_insert(0) += 30;
        }
        for id in [149u16, 164] {
            *wire_start.progress.bag.entry(id).or_insert(0) += 10;
        }
    }
    if !wire_cli.has_area {
        if let Some(a) = wire_start.area {
            area_idx = a;
        }
    }
    let pos_from_cli = wire_cli.has_player_pos || initial_player_pos.is_some();
    if !wire_cli.has_player_pos && initial_player_pos.is_none() {
        initial_player_pos = wire_start.position.map(|p| Vec2::new(p[0], p[2]));
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

    let terrain_res = load_terrain(&bin_root, area_idx, overrides);

    // Locate PML and Battle data directory.
    let (data_dir, pml_db, battle_db, battle_assets) = load_game_data();

    let work_root = PathBuf::from("work");
    let work_abs = work_root.canonicalize().unwrap_or(work_root);

    let navmesh_res = load_navmesh(&bin_root, area_idx);

    // Safe start (largest walkable island, gentle slope, clearance; see game/spawn_point.rs):
    // `--player-pos` is honoured, a valid saved position too, otherwise the safe start is used.
    let (player_spawn_pos, safe_spawn_xz, spawn_notice) = choose_player_spawn(
        navmesh_res.as_ref(),
        area_idx,
        initial_player_pos,
        pos_from_cli,
        player_spawn_pos,
    );

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

    // Real player model from the exporter cache; capsule avatar if the file is missing.
    let cache_root = std::env::var_os("PLA_CACHE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(pla::game::world_objects::DEFAULT_CACHE_ROOT));
    let player_model_file =
        pla::game::player_anim::resolve_player_model(&cache_root, &player_variant)
            .map(|f| f.canonicalize().unwrap_or(f));
    match &player_model_file {
        Some(f) => println!("Player model: {}", f.display()),
        None => println!(
            "Player model '{player_variant}' not found under {}/player; using the capsule avatar",
            cache_root.display()
        ),
    }
    let settings_dir = UserSettings::default_dir();
    configure_viewer(
        &mut app,
        ViewerSetup {
            area_idx,
            bin_root: bin_root.clone(),
            cache_root,
            main_menu,
            player_model: PlayerModelConfig {
                asset_rel: player_model_file
                    .map(|f| pla::game::player_anim::asset_path(&f, &work_abs)),
            },
            initial_mode,
            user_settings: UserSettings::load_from_dir(&settings_dir),
            settings_dir,
            initial_dex,
            data_dir,
            pml_db,
            battle_db,
            battle_assets,
            player_spawn_pos,
            terrain_res,
            navmesh_res,
            safe_spawn_xz,
            spawn_notice,
            static_wild: auto_demo || controls_demo_dir.is_some() || verify_views_dir.is_some(),
            debug_zone_links,
            automation: AutomationOptions {
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
                controls_demo_dir,
            },
            save_game,
            wire_start,
            wire_cli,
            art_style: pla::game::art_style::ArtStylePlugin::from_env_and_args(),
            trainers: pla::game::trainer_encounter::TrainerEncounterPlugin::from_env_and_args(),
            audio: pla::game::audio::GameAudioPlugin::from_env_and_args(),
            effects: pla::game::effects::EffectsPlugin::from_env_and_args(),
            bag: pla::game::bag_screen::BagScreenPlugin::from_env_and_args(),
            minimap: pla::game::minimap::MinimapPlugin::from_env_and_args(bin_root.clone()),
            cli_args,
        },
    );

    app.run();
    ExitCode::SUCCESS
}

/// Real terrain of `area_idx` from the game pack (`None`: fallback ground plane).
fn load_terrain(
    bin_root: &std::path::Path,
    area_idx: u8,
    overrides: TerrainScaleOverride,
) -> Option<TerrainResource> {
    match TerrainArea::load(&bin_root, area_idx) {
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
                        println!(
                            "Built {} terrain mesh chunks (cell size {:.3}m)",
                            chunks.len(),
                            settings.horizontal_cell_size
                        );
                        let blended_albedo =
                            pla::game::zone::resolve_terrain_albedo(area_idx, &area_data);
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
            println!(
                "Terrain pack not loaded from {}: {e}. Running with fallback ground plane.",
                bin_root.display()
            );
            None
        }
    }
}

/// The decoded data sheets directory (`data/`), PML database, battle data and battle assets.
fn load_game_data() -> (
    Option<PathBuf>,
    Option<PmlDatabase>,
    Option<BattleData>,
    Option<BattleAssets>,
) {
    let data_dir = [
        PathBuf::from("data"),
        PathBuf::from("../../data"),
        PathBuf::from("/home/nico/work/decompilacion/data"),
    ]
    .into_iter()
    .find(|p| p.join("pml_personal.tsv").exists());

    let pml_db = data_dir
        .as_ref()
        .and_then(|dir| PmlDatabase::load_from_dir(dir).ok());
    let battle_db = data_dir.as_ref().and_then(|dir| BattleData::load(dir).ok());
    let battle_assets = data_dir
        .as_ref()
        .and_then(|dir| match BattleAssets::load(dir) {
            Ok(a) => Some(a),
            Err(e) => {
                eprintln!("Battle assets (move effects / items / catch rates) not loaded: {e}");
                None
            }
        });
    (data_dir, pml_db, battle_db, battle_assets)
}

/// Real navmesh of `area_idx` from the terrain pack.
fn load_navmesh(bin_root: &std::path::Path, area_idx: u8) -> Option<GameNavmeshResource> {
    match AreaNavigation::load(&bin_root, area_idx) {
        Ok(nav) => {
            println!(
                "Loaded real navmesh ha_area{:02}.gfpak: {} faces, {} edges, {} vertices",
                area_idx,
                nav.primary().faces.len(),
                nav.primary().edges.len(),
                nav.primary().vertices.len()
            );
            Some(GameNavmeshResource { nav: Arc::new(nav) })
        }
        Err(e) => {
            println!("Navmesh not loaded from {}: {e}", bin_root.display());
            None
        }
    }
}

/// Start position: `--player-pos` is honoured, a valid saved position too, otherwise the safe
/// start. Returns the position, the safe start (if any) and a notice for the player.
fn choose_player_spawn(
    navmesh_res: Option<&GameNavmeshResource>,
    area_idx: u8,
    initial_player_pos: Option<Vec2>,
    pos_from_cli: bool,
    player_spawn_pos: Vec2,
) -> (Vec2, Option<Vec2>, Option<&'static str>) {
    use pla::game::spawn_point as sp;
    let mut safe_spawn_xz: Option<Vec2> = None;
    let mut spawn_notice: Option<&str> = None;
    let player_spawn_pos = if let Some(nr) = navmesh_res {
        let nav = nr.nav.primary();
        let comps = sp::components(nav);
        let rd = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
        let marks = sp::landmarks_for_area(
            area_idx,
            &rd("data/field_spawn_table.tsv"),
            &rd("data/field_object_placements.tsv"),
        );
        let safe = sp::choose_safe_spawn(nav, &comps, &marks, &sp::SpawnConfig::default());
        safe_spawn_xz = safe.map(|s| s.xz);
        let (explicit, saved) = if pos_from_cli {
            (initial_player_pos, None)
        } else {
            (None, initial_player_pos)
        };
        let snap = |p: Vec2| {
            if nav.is_walkable(p.x, p.y) {
                p
            } else if let Some(hit) = nav.nearest(p.x, p.y, 400.0) {
                let c = sp::face_centroid(nav, hit.face);
                println!(
                    "Adjusted initial spawn position to walkable navmesh face: ({:.1}, {:.1})",
                    c[0], c[2]
                );
                Vec2::new(c[0], c[2])
            } else {
                p
            }
        };
        match sp::choose_start(nav, &comps, explicit, saved, safe) {
            sp::StartChoice::Explicit(p) | sp::StartChoice::Saved(p) => snap(p),
            sp::StartChoice::Safe(p) => p,
            sp::StartChoice::SafeReplacingSaved(p) => {
                spawn_notice = Some(
                    "Saved position was not on the main walkable area: moved to the safe start.",
                );
                p
            }
            sp::StartChoice::NoSafe => snap(player_spawn_pos),
        }
    } else {
        player_spawn_pos
    };
    (player_spawn_pos, safe_spawn_xz, spawn_notice)
}

/// Everything `main` resolves from the command line, the environment and the game files before
/// the app is built. [`configure_viewer`] turns it into states, resources, plugins and systems;
/// the start-up schedule guard at the end of this file builds the same app headless from a setup
/// made in the test, so the viewer's registrations exist only once.
struct ViewerSetup {
    /// The full process arguments (`std::env::args()`), read by the `from_args` plugin builders.
    cli_args: Vec<String>,
    area_idx: u8,
    bin_root: PathBuf,
    /// Exporter cache (`$PLA_CACHE_ROOT`, else the default cache).
    cache_root: PathBuf,
    /// `--menu`: start in the main menu instead of the world.
    main_menu: bool,
    player_model: PlayerModelConfig,
    initial_mode: CameraMode,
    user_settings: UserSettings,
    settings_dir: PathBuf,
    initial_dex: Option<u16>,
    data_dir: Option<PathBuf>,
    pml_db: Option<PmlDatabase>,
    battle_db: Option<BattleData>,
    battle_assets: Option<BattleAssets>,
    player_spawn_pos: Vec2,
    terrain_res: Option<TerrainResource>,
    navmesh_res: Option<GameNavmeshResource>,
    safe_spawn_xz: Option<Vec2>,
    spawn_notice: Option<&'static str>,
    /// Scripted / automated runs need static wild positions (deterministic frames).
    static_wild: bool,
    debug_zone_links: bool,
    automation: AutomationOptions,
    save_game: pla::game::save_game::SaveGame,
    wire_start: pla::game::save_wire::StartupState,
    wire_cli: pla::game::save_wire::WireCli,
    art_style: pla::game::art_style::ArtStylePlugin,
    trainers: pla::game::trainer_encounter::TrainerEncounterPlugin,
    audio: pla::game::audio::GameAudioPlugin,
    effects: pla::game::effects::EffectsPlugin,
    bag: pla::game::bag_screen::BagScreenPlugin,
    minimap: pla::game::minimap::MinimapPlugin,
}

/// Everything the viewer registers after `DefaultPlugins`: states, resources, game plugins and
/// the viewer's own systems. Shared by `main` and the start-up schedule guard.
fn configure_viewer(app: &mut App, setup: ViewerSetup) {
    let ViewerSetup {
        cli_args,
        area_idx,
        bin_root,
        cache_root,
        main_menu,
        player_model,
        initial_mode,
        user_settings,
        settings_dir,
        initial_dex,
        data_dir,
        pml_db,
        battle_db,
        battle_assets,
        player_spawn_pos,
        terrain_res,
        navmesh_res,
        safe_spawn_xz,
        spawn_notice,
        static_wild,
        debug_zone_links,
        automation,
        save_game,
        wire_start,
        wire_cli,
        art_style,
        trainers,
        audio,
        effects,
        bag,
        minimap,
    } = setup;
    use pla::game::spawn_point as sp;

    app.init_state::<GameState>();
    // The main menu is opt-in (`--menu`) so the direct start and the automated runs
    // (`--auto-demo`, `--script`, ...) keep booting into GameState::World.
    if main_menu {
        app.insert_state(GameState::MainMenu);
    }

    let has_player_model = player_model.asset_rel.is_some();
    app.insert_resource(player_model);

    let mut camera_ctrl = CameraController::default();
    if has_player_model {
        let (focal, shoulder) =
            pla::game::player_anim::camera_offsets(pla::game::player_anim::MODEL_HEIGHT);
        camera_ctrl.focal_offset_y = focal;
        camera_ctrl.shoulder_y = shoulder;
    }
    camera_ctrl.mode = initial_mode;
    if initial_mode == CameraMode::AimOverShoulder {
        camera_ctrl.distance = 2.2;
        camera_ctrl.target_distance = 2.2;
    }
    app.insert_resource(camera_ctrl);
    PauseMenuPlugin::install(app, user_settings, settings_dir.clone());

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
    // Decoded throwables / baits (Brief 18); empty tables fall back to the hardcoded values.
    app.insert_resource(
        data_dir
            .as_ref()
            .and_then(|dir| ThrowableTable::load(dir).ok())
            .unwrap_or_default(),
    );
    app.insert_resource(
        data_dir
            .as_ref()
            .and_then(|dir| BaitTable::load(dir).ok())
            .unwrap_or_default(),
    );
    app.insert_resource(BallInventory::default());
    app.insert_resource(PlayerSpawnConfig {
        pos: player_spawn_pos,
        area_idx,
    });
    app.insert_resource(pla::game::ActiveZoneInfo {
        area: format!("ha_area{area_idx:02}"),
        name: pla::game::zone::display_name_for_index(area_idx),
    });
    app.insert_resource(TreeCatalogResource {
        handles: HashMap::new(),
        spawned_area: None,
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
        let tree_models: HashSet<String> = terrain_res
            .as_ref()
            .map(|t| {
                t.area
                    .instances
                    .iter()
                    .map(|i| i.model_name.clone())
                    .collect()
            })
            .unwrap_or_default();
        app.add_plugins(pla::game::vegetation::VegetationPlugin {
            area: area_idx,
            bin_root: bin_root.clone(),
            cache_root: cache_root.clone(),
            quality: pla::assets::vegetation::GrassQuality::from_args(&cli_args),
        });
        app.add_plugins((
            pla::game::pickups::PickupsPlugin { area: area_idx },
            pla::game::world_objects::WorldObjectsPlugin {
                area: area_idx,
                cache_root: cache_root.clone(),
                exclude_models: tree_models,
                rot_mode: pla::game::world_objects::rot_mode_from_args(&cli_args),
            },
            pla::game::world_collision::WorldCollisionPlugin {
                area: area_idx,
                bin_root: bin_root.clone(),
                rot_mode: pla::game::world_objects::rot_mode_from_args(&cli_args),
            },
            pla::game::lighting::ZoneLightingPlugin {
                area: area_idx,
                slot: match wire_start.time_minutes {
                    Some(m) if !wire_cli.has_time_slot => pla::game::clock::slot_of_minutes(m),
                    _ => pla::game::lighting::time_slot_from_args(&cli_args),
                },
            },
            // Weather lottery + rain/snow particles (game::weather): follows the game clock.
            pla::game::weather::WeatherPlugin::from_args(area_idx, &cli_args),
        ));
    }
    app.add_plugins(pla::game::pokemon_anim::PokemonAnimPlugin);
    app.add_plugins(pla::game::battle_anim::BattleAnimPlugin);
    app.add_plugins(pla::game::evolution_fx::EvolutionFxPlugin);
    app.add_plugins(pla::game::player_anim::PlayerAnimPlugin);
    app.add_plugins(art_style);
    app.add_plugins(pla::game::water::WaterPlugin);
    app.add_plugins(pla::game::wild::WildLifecyclePlugin);
    app.add_plugins(pla::game::world_events::WorldEventsPlugin::from_args(
        &cli_args,
    ));
    app.add_plugins(pla::game::wild_wander::WildWanderPlugin);
    // Wild reactions (alert / flee / chase from data/wild_behavior.tsv policy); `--wild-behavior
    // off` keeps the plain wander loop.
    app.insert_resource(pla::game::wild_wander::WildBehaviorSettings::from_args(&cli_args));
    app.add_plugins(pla::game::follower::FollowerPlugin);
    app.add_plugins(pla::game::capture_world::CaptureWorldPlugin);
    app.add_plugins(trainers);
    app.add_plugins(pla::game::npc::NpcPlugin {
        data_dir: None,
        cache_root,
    });
    app.add_plugins(pla::game::battle_visibility::BattleVisibilityPlugin);
    app.add_plugins(audio);
    app.add_plugins(effects);
    // Keyboard + gamepad (bevy_gilrs) to ActionState; gameplay systems below read only that.
    app.add_plugins(pla::game::input_map::InputMapPlugin);
    app.add_plugins(pla::game::player_motion::PlayerMotionPlugin);
    app.add_plugins(pla::game::player_skills::PlayerSkillsPlugin);
    app.add_plugins(pla::game::blink::BlinkPlugin);
    app.add_plugins(pla::game::interact::InteractPlugin);
    // Installed after InputMapPlugin: MainMenuPlugin adds that plugin itself when it is
    // missing, and Bevy panics when the same plugin is added twice.
    if main_menu {
        // Continue is enabled only when a valid save was loaded at start-up.
        MainMenuPlugin::install(app, wire_start.loaded, settings_dir);
    }
    app.add_plugins(bag);
    if static_wild {
        app.insert_resource(pla::game::wild_wander::WanderConfig { enabled: false });
    }
    if let Some(tr) = terrain_res {
        app.insert_resource(tr);
    }
    if let Some(nr) = navmesh_res {
        app.insert_resource(nr);
    }
    if let Some(assets) = battle_assets {
        app.insert_resource(assets);
    }
    app.insert_resource(BinRootResource(bin_root.clone()));
    // Zone switching (Brief 8): decoded door / fast-travel links, fade and scene swap.
    app.insert_resource(pla::game::zone::ZoneBinRoot(bin_root));
    app.add_plugins(pla::game::zone::ZonePlugin {
        links_path: None,
        debug_links: debug_zone_links,
    });
    app.add_plugins(pla::game::door_pairs::DoorPairsPlugin); // door hint (Brief 13)
    // Minimap, compass and full-screen map (Brief 15); `--no-minimap` / `--game-map`.
    app.add_plugins(minimap);
    app.insert_resource(automation);

    pla::game::play_time::PlayTimePlugin::install(app, wire_start.play_seconds);
    // Day/night clock: --time-slot pins it, --time-scale speeds it, the save supplies the start.
    pla::game::clock::GameClockPlugin::install(app, &cli_args, wire_start.time_minutes);
    pla::game::save_wire::SaveWirePlugin::install(app, save_game, wire_start, &wire_cli);
    // Fullscreen screens opened from the pause menu (Esc): party, boxes, Pokédex and crafting.
    // Installed after SaveWirePlugin so they read the live `PlayerProgress`.
    app.add_plugins((
        pla::game::party_screen::PartyScreenPlugin,
        pla::game::box_screen::BoxScreenPlugin,
        pla::game::dex_screen::DexScreenPlugin::default(),
        crafting_plugin(),
    ));
    if let Some(p) = safe_spawn_xz {
        app.insert_resource(sp::SafeSpawnPoint(p));
    }
    if let Some(msg) = spawn_notice {
        app.world_mut()
            .resource_mut::<pla::game::save_wire::SaveToast>()
            .show(msg);
    }
    add_viewer_systems(app);
}

/// The viewer's own systems (the game plugins are added by [`configure_viewer`]).
fn add_viewer_systems(app: &mut App) {
    app.add_systems(Update, apply_safe_spawn_request);
    app.add_systems(Startup, (setup_scene, setup_game_ui, spawn_aim_hint));
    app.add_systems(
        Update,
        (
            spawn_tree_instances,
            stream_wild_spawners
                .run_if(in_state(GameState::World))
                .run_if(not_paused),
            spawn_pokemon_models,
            player_locomotion
                .run_if(in_state(GameState::World))
                .run_if(not_paused),
            player_model_fallback,
            creature_encounter_system
                .run_if(in_state(GameState::World))
                .run_if(not_paused),
            battle_input_system
                .run_if(in_state(GameState::Battle))
                .run_if(not_paused),
            camera_controls,
            update_camera_transform,
            ball_selection_controls
                .run_if(in_state(GameState::World))
                .run_if(not_paused),
            ball_throw_input
                .run_if(in_state(GameState::World))
                .run_if(not_paused),
            ball_ballistics_and_physics
                .run_if(in_state(GameState::World))
                .run_if(not_paused),
            pokemon_selection_controls
                .run_if(in_state(GameState::World))
                .run_if(not_paused),
            update_aim_hud,
            sync_ui_visibility,
            update_world_hud_system.run_if(in_state(GameState::World)),
            update_battle_ui_system.run_if(in_state(GameState::Battle)),
            update_hud_and_telemetry,
            automated_tick_driver,
            controls_demo_driver,
        )
            .chain()
            // Crouch/dodge state is final before the movement reads it; jump/fall/climb
            // integrate after the horizontal locomotion of the frame.
            .after(pla::game::player_skills::PlayerSkillsSet)
            .before(pla::game::player_motion::PlayerMotionSet),
    );
    // Separate registration: the world tuple is already at Bevy's 20-system limit.
    app.add_systems(
        Update,
        bait_throw_input.run_if(in_state(GameState::World)).run_if(not_paused),
    );
    app.add_systems(OnEnter(GameState::Battle), setup_battle_arena);
    app.add_systems(OnExit(GameState::Battle), cleanup_battle_arena);
    app.add_systems(
        Update,
        sync_arena_player_model.run_if(in_state(GameState::Battle)),
    );
}

/// Capsule stand-in for the player (also the fallback when the glTF is missing or fails to load).
fn spawn_capsule_avatar(
    parent: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
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
        Transform::from_xyz(0.0, 0.75, -0.3)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));
}

/// If the player glTF fails to load, fall back to the capsule avatar and its camera heights.
fn player_model_fallback(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut camera_ctrl: ResMut<CameraController>,
    q: Query<
        (Entity, &pla::game::player_anim::PlayerModelSource),
        (
            With<PlayerAvatar>,
            Without<pla::game::player_anim::PlayerSceneSpawned>,
        ),
    >,
) {
    for (e, src) in &q {
        if matches!(
            asset_server.load_state(&src.gltf),
            bevy::asset::LoadState::Failed(_)
        ) {
            eprintln!("Player model failed to load; using the capsule avatar");
            commands
                .entity(e)
                .remove::<pla::game::player_anim::PlayerModelSource>();
            commands.entity(e).with_children(|parent| {
                spawn_capsule_avatar(parent, &mut meshes, &mut materials);
            });
            let d = CameraController::default();
            camera_ctrl.focal_offset_y = d.focal_offset_y;
            camera_ctrl.shoulder_y = d.shoulder_y;
        }
    }
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    pml: Res<LoadedPml>,
    _pkmn_state: Res<ActivePokemonState>,
    spawn_cfg: Res<PlayerSpawnConfig>,
    player_model: Res<PlayerModelConfig>,
    terrain_res: Option<Res<TerrainResource>>,
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
        let key = PokemonAssetKey {
            dex,
            form: 0,
            rare: false,
        };
        if let Some(rel_path) = resolve_pokemon_gltf_rel(dex, key.form, key.rare) {
            let handle: Handle<Gltf> = asset_server.load(rel_path);
            pokemon_catalog.handles.insert(key, handle);
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
            normal_map_texture: pla::game::zone::resolve_terrain_normal_handle(
                terrain.area.area,
                &terrain.area,
                &mut images,
            ),
            perceptual_roughness: 0.95,
            ..default()
        });

        for chunk in &terrain.chunks {
            commands.spawn((
                TerrainChunkMarker,
                Mesh3d(meshes.add(chunk.mesh.clone())),
                MeshMaterial3d(terrain_material.clone()),
                Transform::IDENTITY,
            ));
        }
        // The `.trins` tree models are registered and spawned per area by `spawn_tree_instances`
        // (it reloads them on a zone change, Brief 8b).
    } else {
        // Fallback: ground plane and field markers
        let ground_size = 40.0;
        commands.spawn((
            Mesh3d(meshes.add(Mesh::from(
                Plane3d::default().mesh().size(ground_size, ground_size),
            ))),
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

    // Player avatar: the real exported glTF when available, otherwise the capsule.
    let player_body = commands
        .spawn((
            PlayerAvatar,
            PlayerSwim::default(),
            Transform::from_xyz(spawn_x, player_y, spawn_z),
            Visibility::default(),
        ))
        .id();
    match &player_model.asset_rel {
        Some(rel) => {
            commands
                .entity(player_body)
                .insert(pla::game::player_anim::PlayerModelSource {
                    gltf: asset_server.load(rel.clone()),
                });
        }
        None => {
            commands.entity(player_body).with_children(|parent| {
                spawn_capsule_avatar(parent, &mut meshes, &mut materials);
            });
        }
    }

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
    println!("  Esc           : Pause menu (Bag, Party, Pokémon boxes, Pokédex, Crafting, Shop, ...)");
    println!("============================================================");

    if let Some(db) = &pml.0 {
        println!(
            "PML Database loaded successfully: {} species registered.",
            db.personal.len()
        );
    } else {
        println!("Note: PML Database not found at data/ (running with fallback species).");
    }
}

/// Crafting screen fed by the tracked `data/crafting_recipes.tsv` (empty when it is missing).
fn crafting_plugin() -> pla::game::crafting_ui::CraftingPlugin {
    use pla::game::crafting::{Bag, TsvRecipeProvider};
    let provider = pla::game::bag_screen::find_item_data_dir()
        .ok_or_else(|| "data directory not found".to_string())
        .and_then(TsvRecipeProvider::load)
        .unwrap_or_else(|e| {
            eprintln!("[crafting] recipe sheet unavailable ({e}); the screen lists no recipes");
            TsvRecipeProvider::default()
        });
    println!("[crafting] {} recipes loaded", provider.0.len());
    pla::game::crafting_ui::CraftingPlugin::with_provider(&provider, Bag::default())
}

/// Registers the `.trins` models of `terrain`'s area for asynchronous glTF loading (the models
/// live in the area's `_terrain` export directory, then the plain area directory).
fn register_tree_models(
    terrain: &pla::assets::terrain::TerrainArea,
    catalog: &mut TreeCatalogResource,
    asset_server: &AssetServer,
) {
    let unique_models: HashSet<String> = terrain.instances.iter().map(|i| i.model_name.clone()).collect();
    for model_name in unique_models {
        let possible_paths = [
            format!("cache/gltf_field/ha_area{:02}_terrain/{}.gltf", terrain.area, model_name),
            format!("cache/gltf_field/ha_area{:02}/{}.gltf", terrain.area, model_name),
        ];
        for rel_path in &possible_paths {
            let full_path = PathBuf::from("work").join(rel_path);
            if full_path.exists() {
                let handle: Handle<Gltf> = asset_server.load(rel_path.clone());
                catalog.handles.insert(model_name.clone(), handle);
                break;
            }
        }
    }
}

/// Spawns tree instances from real .trins transforms once their glTF models are loaded.
///
/// Tracks the active area: when `TerrainResource` changes (zone change, Brief 8b) the previous
/// area's tree entities are despawned, its model handles dropped and the new area's streamed.
#[allow(clippy::too_many_arguments)]
fn spawn_tree_instances(
    mut commands: Commands,
    mut catalog: ResMut<TreeCatalogResource>,
    terrain_res: Option<Res<TerrainResource>>,
    asset_server: Res<AssetServer>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    gltf_materials: Res<Assets<GltfMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    trees: Query<Entity, With<TrinsTreeInstance>>,
) {
    let Some(ref terrain) = terrain_res else {
        return;
    };
    let area = terrain.area.area;
    match pla::game::zone::trins_reload_step(catalog.spawned_area, area) {
        pla::game::zone::TrinsReloadStep::Skip => return,
        pla::game::zone::TrinsReloadStep::Reload => {
            for e in &trees {
                commands.entity(e).despawn();
            }
        }
        pla::game::zone::TrinsReloadStep::Initial => {}
    }
    catalog.handles.clear();
    register_tree_models(&terrain.area, &mut catalog, &asset_server);
    if catalog.handles.is_empty() {
        // No exported tree model for this area: mark it so the next change retries.
        println!("Trees: no exported .trins models for ha_area{area:02}");
        catalog.spawned_area = Some(area);
        return;
    }

    // Check if all requested model handles are ready
    let all_ready = catalog.handles.values().all(|h| gltf_assets.contains(h));
    if !all_ready {
        return;
    }

    let mut model_prims: HashMap<String, Vec<(Handle<Mesh>, Handle<StandardMaterial>)>> =
        HashMap::new();
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
            let transform = Transform::from_matrix(Mat4::from_cols_array(&flat));
            for (mesh_h, mat_h) in prims {
                commands.spawn((
                    TrinsTreeInstance,
                    Mesh3d(mesh_h.clone()),
                    MeshMaterial3d(mat_h.clone()),
                    transform,
                ));
            }
            count += 1;
        }
    }
    println!(
        "Spawned {} tree instances from real .trins matrices across {} unique models (ha_area{area:02}).",
        count,
        model_prims.len()
    );
    catalog.spawned_area = Some(area);
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
    consumed: Res<pla::game::wild::ConsumedSpawners>,
    time: Res<Time>,
) {
    let Ok(p_xf) = player_query.single() else {
        return;
    };
    let now_s = time.elapsed_secs_f64();

    let p_pos = Vec2::new(p_xf.translation.x, p_xf.translation.z);
    let radius = active_spawners.streaming_radius;
    let radius_sq = radius * radius;
    let unload_radius_sq = (radius + 25.0) * (radius + 25.0);

    // 1. Unload entities that moved out of radius
    let mut to_despawn = Vec::new();
    for (spawner_id, entity) in &active_spawners.spawned_entities {
        if let Some(spawner) = active_spawners
            .spawners
            .iter()
            .find(|s| &s.spawner_id == spawner_id)
        {
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
        if active_spawners
            .spawned_entities
            .contains_key(&spawner.spawner_id)
        {
            continue;
        }
        // Defeated / captured creature: keep the spawner empty until its respawn time.
        if consumed.is_consumed(&spawner.spawner_id, now_s) {
            continue;
        }

        let dist_sq = (spawner.pos.x - p_pos.x).powi(2) + (spawner.pos.z - p_pos.y).powi(2);
        if dist_sq <= radius_sq {
            if let Some((dex, name, lvl, variant)) = roll_spawner_variant(spawner, seed) {
                to_spawn.push((spawner.clone(), dex, name, lvl, variant));
            }
        }
    }

    for (spawner, dex, name, lvl, variant) in to_spawn {
        let spawn_x = spawner.pos.x;
        let spawn_z = spawner.pos.z;
        let surface_y = terrain_res
            .as_ref()
            .and_then(|t| t.area.sample_world_height(spawn_x, spawn_z))
            .unwrap_or(spawner.pos.y);
        let spawn_y = surface_y + 0.45;

        // Ensure glTF is loading
        let asset_key = PokemonAssetKey {
            dex,
            form: variant.form,
            rare: false,
        };
        if !pokemon_catalog.handles.contains_key(&asset_key) {
            if let Some(rel_path) = resolve_pokemon_gltf_rel(dex, asset_key.form, asset_key.rare) {
                let handle: Handle<Gltf> = asset_server.load(rel_path);
                pokemon_catalog.handles.insert(asset_key, handle);
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
                PokemonSpawn {
                    national_dex: dex,
                    form: variant.form,
                    // No native shiny probability is decoded; alpha does not imply rare palette.
                    rare: false,
                    alpha: variant.alpha,
                    size_scale: variant.size_scale,
                },
                Transform::from_xyz(spawn_x, spawn_y, spawn_z)
                    .with_scale(Vec3::splat(variant.size_scale)),
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

        active_spawners
            .spawned_entities
            .insert(spawner.spawner_id.clone(), entity);
    }
}

fn spawn_pokemon_models(
    mut commands: Commands,
    mut catalog: ResMut<PokemonCatalogResource>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    gltf_materials: Res<Assets<GltfMaterial>>,
    meshes: Res<Assets<Mesh>>,
    pkmn_query: Query<(
        Entity,
        &PokemonSpawn,
        &Children,
        Option<&pla::game::pokemon_anim::PokemonBattleSlot>,
    )>,
    asset_server: Res<AssetServer>,
) {
    for (entity, pkmn_spawn, children, battle_slot) in &pkmn_query {
        if catalog.spawned.contains(&entity) {
            continue;
        }
        let key = PokemonAssetKey {
            dex: pkmn_spawn.national_dex,
            form: pkmn_spawn.form,
            rare: pkmn_spawn.rare,
        };
        let Some(handle) = catalog.handles.get(&key) else {
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
                        && prim
                            .material
                            .as_ref()
                            .map_or(true, |mat_h| gltf_materials.contains(mat_h))
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
        if let Some(idle_rel) = handle
            .path()
            .map(|pth| pth.path().to_string_lossy().into_owned())
        {
            // Arena creatures also load the battle clips (attack / hit / faint).
            let source = if battle_slot.is_some() {
                pla::game::pokemon_anim::PokemonAnimSource::from_idle_path_with_battle(
                    &asset_server,
                    &idle_rel,
                )
            } else {
                pla::game::pokemon_anim::PokemonAnimSource::from_idle_path(&asset_server, &idle_rel)
            };
            commands.entity(entity).insert(source);
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
    mut player_vis_query: Query<&mut Visibility, With<PlayerAvatar>>,
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
    for mut vis in &mut player_vis_query {
        *vis = Visibility::Hidden;
    }
    let center_x = player_xf.translation.x;
    let center_z = player_xf.translation.z;

    let sample_y = |x: f32, z: f32| -> f32 {
        terrain_res
            .as_ref()
            .and_then(|t| t.area.sample_world_height(x, z))
            .unwrap_or(player_xf.translation.y - 0.9)
    };

    let wild_dex = battle_session.as_ref().map_or(397, |s| s.wild_dex);
    // The party member that is out (the saved party is installed before this runs).
    let player_dex: u16 = battle_session
        .as_ref()
        .map_or(25, |s| s.battle.combatants[s.active_player()].species.dex);

    // Ensure glTF models are requested
    for dex in [player_dex, wild_dex] {
        let key = PokemonAssetKey {
            dex,
            form: 0,
            rare: false,
        };
        if !pokemon_catalog.handles.contains_key(&key) {
            if let Some(rel) = resolve_pokemon_gltf_rel(dex, key.form, key.rare) {
                let handle: Handle<Gltf> = asset_server.load(rel);
                pokemon_catalog.handles.insert(key, handle);
            }
        }
    }

    // Positions in arena:
    // Place combatants facing each other across ~3.0m span
    let p_pkmn_x = center_x - 0.7;
    let p_pkmn_z = center_z - 1.8;
    let p_pkmn_y = sample_y(p_pkmn_x, p_pkmn_z);

    let w_pkmn_x = center_x + 0.9;
    let w_pkmn_z = center_z - 4.4;
    let w_pkmn_y = sample_y(w_pkmn_x, w_pkmn_z);

    // Face each other on the XZ plane:
    let p_to_w = Vec2::new(w_pkmn_x - p_pkmn_x, w_pkmn_z - p_pkmn_z).normalize();
    let p_rot = Quat::from_rotation_y(p_to_w.x.atan2(p_to_w.y));
    let w_rot = Quat::from_rotation_y((-p_to_w.x).atan2(-p_to_w.y));

    // Spawn Player Pokemon entity
    let _p_entity = commands
        .spawn((
            BattleArenaMarker,
            PokemonSpawn {
                national_dex: player_dex,
                form: 0,
                rare: false,
                alpha: false,
                size_scale: 1.0,
            },
            pla::game::pokemon_anim::PokemonBattleSlot(0),
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
            PokemonSpawn {
                national_dex: wild_dex,
                form: 0,
                rare: false,
                alpha: false,
                size_scale: 1.0,
            },
            pla::game::pokemon_anim::PokemonBattleSlot(1),
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
    mut player_vis_query: Query<&mut Visibility, With<PlayerAvatar>>,
) {
    for entity in &arena_query {
        catalog.spawned.remove(&entity);
        commands.entity(entity).despawn();
    }
    for mut vis in &mut player_vis_query {
        *vis = Visibility::Visible;
    }
}

fn get_species_color(dex: u16) -> Color {
    match dex {
        25 => Color::srgb(0.95, 0.85, 0.15), // Pikachu (Electric Yellow)
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

/// Pause-menu "Back to safe spawn": teleport the player to the area's safe start.
fn apply_safe_spawn_request(
    mut request: ResMut<pla::game::pause_menu::SafeSpawnRequested>,
    safe: Option<Res<pla::game::spawn_point::SafeSpawnPoint>>,
    navmesh_res: Option<Res<GameNavmeshResource>>,
    mut toast: ResMut<pla::game::save_wire::SaveToast>,
    mut player_query: Query<&mut Transform, With<PlayerAvatar>>,
) {
    if !std::mem::take(&mut request.0) {
        return;
    }
    let (Some(safe), Ok(mut t)) = (safe, player_query.single_mut()) else {
        toast.show("No safe spawn available for this area.");
        return;
    };
    t.translation.x = safe.0.x;
    t.translation.z = safe.0.y;
    if let Some(y) = navmesh_res
        .as_ref()
        .and_then(|n| n.height_at(safe.0.x, safe.0.y))
    {
        t.translation.y = y + 0.9;
    }
    toast.show("Back at the safe spawn.");
}

/// Player locomotion system: translates input along camera relative vectors,
/// uses navmesh collision to block cliffs/water with smooth sliding, and samples surface height.
/// Movement comes from [`ActionState`] (WASD/arrows or the gamepad left stick); the stick keeps
/// its deflected magnitude so a half-pushed stick walks slower.
fn player_locomotion(
    time: Res<Time>,
    input: Res<ActionState>,
    camera_ctrl: Res<CameraController>,
    terrain_res: Option<Res<TerrainResource>>,
    navmesh_res: Option<Res<GameNavmeshResource>>,
    wall_collision: Option<Res<pla::game::world_collision::WorldCollision>>,
    water_bodies: Option<Res<WaterBodies>>,
    safe: Option<Res<pla::game::spawn_point::SafeSpawnPoint>>,
    mut stuck: Local<pla::game::spawn_point::StuckTracker>,
    mut toast: ResMut<pla::game::save_wire::SaveToast>,
    motion_query: Query<&pla::game::player_motion::PlayerMotionState, With<PlayerAvatar>>,
    mut skills_query: Query<&mut pla::game::player_skills::PlayerSkillState, With<PlayerAvatar>>,
    mut player_query: Query<&mut Transform, With<PlayerAvatar>>,
    mut swim_query: Query<&mut PlayerSwim, With<PlayerAvatar>>,
) {
    use pla::game::player_motion::MotionMode;
    let Ok(mut transform) = player_query.single_mut() else {
        return;
    };
    // The state written at the end of the previous frame drives the speed and the clip today.
    let was_swimming = swim_query
        .single()
        .map(|s| s.state == SwimState::Swimming)
        .unwrap_or(false);
    let motion_mode = motion_query.single().map(|m| m.mode).unwrap_or_default();

    let dt = time.delta_secs();
    let input_dir = input.move_axis();

    // Camera yaw defines the forward direction on the XZ plane.
    let yaw = camera_ctrl.yaw;
    let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());

    // Dodge roll (game::player_skills): starts on a press (or a press buffered while airborne),
    // towards the held direction or the facing; the skills module keeps the timer and the state.
    if let Ok(mut sk) = skills_query.single_mut() {
        if (input.just_pressed(Action::Dodge) || sk.dodge_buffer > 0.0) && !sk.dodging() {
            let n = if input_dir.length() > 1.0 { input_dir.normalize() } else { input_dir };
            let want = forward * n.y + right * n.x;
            let facing3 = transform.rotation * Vec3::Z;
            let can_roll = motion_mode == MotionMode::Grounded && !was_swimming;
            let started = sk.begin_dodge(
                can_roll,
                Vec2::new(want.x, want.z),
                Vec2::new(facing3.x, facing3.z),
            );
            if started == Some(pla::game::player_skills::DodgeDir::Forward) {
                if let Some(d) = sk.dodge {
                    transform.rotation = Quat::from_rotation_y(d.world_dir.x.atan2(d.world_dir.y));
                }
            }
        }
    }
    let dodge = skills_query.single().ok().and_then(|s| s.dodge);
    let crouching = skills_query.single().is_ok_and(|s| s.crouching());

    if input_dir.length_squared() > 0.0 || dodge.is_some() {
        // Keyboard is digital (normalising a diagonal keeps full speed); the stick stays analog.
        let input_norm = if input_dir.length() > 1.0 {
            input_dir.normalize()
        } else {
            input_dir
        };
        let is_sprinting = input.pressed(Action::Sprint);
        // Sprinting is not allowed in water; swimming is slower than walking. In the air the
        // ground speed is scaled by the decoded control_airMoveRate (game::player_motion);
        // while climbing the vertical system holds the avatar in place horizontally.
        let ground_speed = if is_sprinting { 8.0 } else { 4.5 };
        let ground_speed = if crouching {
            pla::game::player_skills::crouch_speed(ground_speed)
        } else {
            ground_speed
        };
        let speed = if dodge.is_some() {
            pla::game::player_skills::roll_speed()
        } else if was_swimming {
            SWIM_SPEED
        } else if motion_mode == MotionMode::Airborne {
            pla::game::player_motion::air_speed(ground_speed)
        } else if motion_mode == MotionMode::Climbing {
            0.0
        } else {
            ground_speed
        };

        // Orthonormal basis: the vector magnitude is the input deflection (1 for keys). A dodge
        // roll ignores the stick and travels along its own direction.
        let move_vec = match dodge {
            Some(d) => Vec3::new(d.world_dir.x, 0.0, d.world_dir.y),
            None => forward * input_norm.y + right * input_norm.x,
        };
        let delta_xz = Vec2::new(move_vec.x * speed * dt, move_vec.z * speed * dt);
        let current_xz = Vec2::new(transform.translation.x, transform.translation.z);
        let target_xz = current_xz + delta_xz;

        // Stepping into water deep enough to swim bypasses the navmesh: the river edge is not a
        // walkable face, so a walker could otherwise never reach the water at all (ASSUMED: the
        // game's own rule for entering water is not decoded). The surface must sit on the ground
        // (see water::SWIM_MAX_GROUND_OFFSET): the placed water planes are flat sheets and some
        // float far above the terrain.
        let water_entry = water_bodies
            .as_ref()
            .and_then(|w| w.level_near(target_xz, 0.5))
            .is_some_and(|level| {
                terrain_res
                    .as_ref()
                    .and_then(|t| t.area.sample_world_height(target_xz.x, target_xz.y))
                    .is_some_and(|ground| {
                        (level - ground).abs() <= pla::game::water::SWIM_MAX_GROUND_OFFSET
                            && level - ground > SWIM_ENTER_DEPTH
                    })
            });

        // Apply navmesh collision sliding if real navmesh is loaded
        let final_xz = if water_entry {
            target_xz
        } else if let Some(ref nav) = navmesh_res {
            use pla::game::spawn_point as sp;
            let walk = |p: Vec2| nav.is_walkable(p.x, p.y);
            // Off the navmesh: walk toward the nearest walkable point instead of rejecting every move.
            let escape = sp::escape_step(current_xz, delta_xz, walk, |p| {
                nav.nearest(p.x, p.y, 60.0).map(|h| {
                    let c = sp::face_centroid(nav.nav.primary(), h.face);
                    Vec2::new(c[0], c[2])
                })
            });
            // Boxed in on all 8 sides for > 1 s with input held: back to the safe spawn.
            let boxed = escape.is_none() && sp::boxed_in(current_xz, 0.3, walk);
            match (stuck.update(dt, true, boxed), safe.as_ref()) {
                (true, Some(s)) => {
                    toast.show("Stuck: moved back to the safe spawn.");
                    s.0
                }
                _ => escape.unwrap_or_else(|| nav.try_move_with_slide(current_xz, delta_xz)),
            }
        } else {
            current_xz + delta_xz
        };
        // Wall collision of placed objects (houses, fences, rocks) after the navmesh slide.
        let final_xz = wall_collision.as_ref().map_or(final_xz, |w| {
            w.push_player_out(final_xz, transform.translation.y)
        });

        transform.translation.x = final_xz.x;
        transform.translation.z = final_xz.y;

        // Smoothly orient player avatar towards movement direction (a side or back roll keeps
        // the facing, the roll clip supplies the sidestep).
        if dodge.is_none_or(|d| d.dir == pla::game::player_skills::DodgeDir::Forward) {
            transform.rotation = pla::game::player_anim::step_facing(
                transform.rotation,
                Vec2::new(move_vec.x, move_vec.z),
                dt,
            );
        }
    } else {
        stuck.blocked_secs = 0.0;
    }

    // Keep player avatar on the real terrain or navmesh surface
    let nav_h = navmesh_res
        .as_ref()
        .and_then(|n| n.height_at(transform.translation.x, transform.translation.z));
    let terrain_h = terrain_res.as_ref().and_then(|t| {
        t.area
            .sample_world_height(transform.translation.x, transform.translation.z)
    });

    // Ground snapping (and the water probe) only run while grounded: in the air or on a
    // climbable the vertical system (game::player_motion) owns Y, and a jump over water
    // must not snap the avatar down to the surface mid-flight.
    if motion_mode == MotionMode::Grounded {
        if let Some(surface_y) = nav_h.or(terrain_h) {
            // Swimming: the chest-height rule with hysteresis lives in game::water (unit-tested there).
            let level = water_bodies.as_ref().and_then(|w| {
                pla::game::water::grounded_level_at(
                    w,
                    Vec2::new(transform.translation.x, transform.translation.z),
                    surface_y,
                    pla::game::water::SWIM_MAX_GROUND_OFFSET,
                )
            });
            let state = pla::game::water::step_swim(
                swim_query.single().map(|s| s.state).unwrap_or_default(),
                level,
                surface_y,
            );
            if let Ok(mut swim) = swim_query.single_mut() {
                swim.state = state;
            }
            transform.translation.y = if state == SwimState::Swimming {
                pla::game::water::swim_root_y(level.unwrap_or(surface_y))
            } else if pla::game::player_motion::is_ledge_drop(transform.translation.y, surface_y + 0.9)
            {
                // A drop longer than a step is a ledge: game::player_motion makes the avatar fall.
                transform.translation.y
            } else {
                surface_y + 0.9
            };
        }
    }
}

/// Camera input and state updates (implements PLA dec319 kinematics).
/// While paused it only drains the mouse events, so nothing accumulates behind the menu.
/// Keyboard/gamepad look and zoom come from [`ActionState`]; the mouse delta stays raw.
fn camera_controls(
    time: Res<Time>,
    input: Res<ActionState>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut mouse_wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut camera_ctrl: ResMut<CameraController>,
    settings: Res<UserSettings>,
    paused: Res<Paused>,
) {
    if paused.0 {
        mouse_motion.clear();
        mouse_wheel.clear();
        return;
    }
    let dt = time.delta_secs();

    // Toggle camera mode between Orbit (0) and Aim (2): Z/Tab, or gamepad ZL.
    if input.just_pressed(Action::Aim) {
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

    // Keyboard (Q/E) and gamepad right stick orbit, at the old 120 px/s full-deflection rate.
    // Stick up looks up (negative mouse-delta-y), matching mouse up.
    let look = input.look_axis();
    mouse_delta += Vec2::new(look.x, -look.y) * 120.0 * dt;

    // Settings: invert the horizontal (yaw: mouse X, Q/E) and vertical (pitch) axes separately.
    let (yaw_input, pitch_input) = settings.apply_invert(mouse_delta.x, mouse_delta.y);
    let sensitivity = 0.0035;
    camera_ctrl.yaw += yaw_input * sensitivity;
    camera_ctrl.pitch = (camera_ctrl.pitch + pitch_input * sensitivity).clamp(-0.25, 1.35);

    // Zoom adjustments.
    for ev in mouse_wheel.read() {
        camera_ctrl.target_distance = (camera_ctrl.target_distance - ev.y * 0.6).clamp(1.5, 12.0);
    }
    // Zoom axis: +1 closer (R), −1 farther (F).
    let zoom = input.zoom();
    if zoom != 0.0 {
        camera_ctrl.target_distance =
            (camera_ctrl.target_distance - zoom * 4.0 * dt).clamp(1.5, 12.0);
    }

    // In Aim mode, camera distance contracts and reticle becomes visible.
    let desired_dist = match camera_ctrl.mode {
        CameraMode::OrbitDefault => camera_ctrl.target_distance,
        CameraMode::AimOverShoulder => 2.2,
    };
    camera_ctrl.distance += (desired_dist - camera_ctrl.distance) * (10.0 * dt).min(1.0);
}

/// Reticle and "AIM" hint are visible only while aiming in the World (not paused).
fn update_aim_hud(
    camera_ctrl: Res<CameraController>,
    paused: Res<Paused>,
    game_state: Res<State<GameState>>,
    mut reticle_query: Query<&mut Visibility, (With<Reticle>, Without<AimHint>)>,
    mut hint_query: Query<&mut Visibility, (With<AimHint>, Without<Reticle>)>,
) {
    let show = camera_ctrl.mode == CameraMode::AimOverShoulder
        && !paused.0
        && *game_state.get() == GameState::World;
    let want = if show {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut v in reticle_query.iter_mut().chain(hint_query.iter_mut()) {
        if *v != want {
            *v = want;
        }
    }
}

fn spawn_aim_hint(mut commands: Commands) {
    commands.spawn((
        AimHint,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            bottom: Val::Px(90.0),
            margin: UiRect::left(Val::Px(-150.0)),
            width: Val::Px(300.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.8)),
        Visibility::Hidden,
        children![(
            Text::new("AIM: left click to throw"),
            TextFont::from_font_size(18.0),
            TextColor(Color::srgb(1.0, 0.9, 0.5)),
        )],
    ));
}

/// Transform solver using canonical PLA spherical coordinates:
/// ΔX = dist * cos(pitch) * sin(yaw)
/// ΔY = dist * sin(pitch)
/// ΔZ = dist * cos(pitch) * cos(yaw)
/// cameraPos = lookAtPos + Δ
fn update_camera_transform(
    game_state: Res<State<GameState>>,
    camera_ctrl: Res<CameraController>,
    player_query: Query<&Transform, (With<PlayerAvatar>, Without<Camera3d>)>,
    arena_query: Query<
        (&PokemonSpawn, &Transform),
        (
            With<BattleArenaMarker>,
            Without<Camera3d>,
            Without<PlayerAvatar>,
        ),
    >,
    mut camera_query: Query<&mut Transform, (With<Camera3d>, Without<PlayerAvatar>)>,
) {
    let Ok(player_xf) = player_query.single() else {
        return;
    };
    let Ok(mut cam_xf) = camera_query.single_mut() else {
        return;
    };

    // Dedicated combat camera framing when in battle
    if *game_state.get() == GameState::Battle {
        let mut p_pos = None;
        let mut w_pos = None;
        for (spawn, xf) in &arena_query {
            if spawn.national_dex == 25 {
                p_pos = Some(xf.translation);
            } else {
                w_pos = Some(xf.translation);
            }
        }
        if let (Some(p), Some(w)) = (p_pos, w_pos) {
            let to_wild = (w - p).normalize();
            let right_side = Vec3::new(to_wild.z, 0.0, -to_wild.x).normalize();
            // Camera placed 2.3m behind Pikachu, offset 1.0m to the side, elevated 1.35m
            let cam_pos = p - to_wild * 2.3 - right_side * 1.0 + Vec3::new(0.0, 1.35, 0.0);
            let look_target = (p + w) * 0.5 + Vec3::new(0.0, 0.35, 0.0) + right_side * 0.2;
            cam_xf.translation = cam_pos;
            cam_xf.look_at(look_target, Vec3::Y);
            return;
        }
    }

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
    let forward_dir = Vec3::new(
        -pitch.cos() * yaw.sin(),
        -pitch.sin(),
        -pitch.cos() * yaw.cos(),
    )
    .normalize();

    let (eye_pos, look_target) = match camera_ctrl.mode {
        CameraMode::OrbitDefault => {
            let eye = focal_target + Vec3::new(delta_x, delta_y, delta_z);
            (eye, focal_target)
        }
        CameraMode::AimOverShoulder => {
            let shoulder_pos =
                player_xf.translation + right * 0.45 + Vec3::new(0.0, camera_ctrl.shoulder_y, 0.0);
            let eye = shoulder_pos + Vec3::new(delta_x, delta_y, delta_z);
            let target = eye + forward_dir * 10.0;
            (eye, target)
        }
    };

    cam_xf.translation = eye_pos;
    cam_xf.look_at(look_target, Vec3::Y);
}

/// Selects the active throwable with keyboard shortcuts:
/// B: Poké Ball | G: Great | U: Ultra | F: Feather | N: Wing | J: Jet | L: Leaden | T: Gigaton
/// | V: cycle the bait (bag items from `bait_parameters.tsv`); X throws it while aiming.
fn ball_selection_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut inventory: ResMut<BallInventory>,
    baits: Res<BaitTable>,
    progress: Res<PlayerProgress>,
) {
    let pick = if keys.just_pressed(KeyCode::KeyB) {
        Some(BallKind::PokeBall)
    } else if keys.just_pressed(KeyCode::KeyG) {
        Some(BallKind::GreatBall)
    } else if keys.just_pressed(KeyCode::KeyU) {
        Some(BallKind::UltraBall)
    } else if keys.just_pressed(KeyCode::KeyF) {
        Some(BallKind::FeatherBall)
    } else if keys.just_pressed(KeyCode::KeyN) {
        Some(BallKind::WingBall)
    } else if keys.just_pressed(KeyCode::KeyJ) {
        Some(BallKind::JetBall)
    } else if keys.just_pressed(KeyCode::KeyL) {
        Some(BallKind::LeadenBall)
    } else if keys.just_pressed(KeyCode::KeyT) {
        Some(BallKind::GigatonBall)
    } else {
        None
    };
    if let Some(kind) = pick {
        inventory.selected = kind;
        println!("Selected Ball: {}", kind.display_name());
    }
    if keys.just_pressed(KeyCode::KeyV) {
        let available = baits.present_in(&progress.bag);
        match inventory.cycle_bait(&available) {
            Some(id) => println!(
                "Selected bait: item #{id} ({} left) — X throws it while aiming",
                progress.bag.get(&id).copied().unwrap_or(0)
            ),
            None => println!("Bait selection cleared"),
        }
    }
}

/// Spawns a thrown Poké Ball along the aim vector towards the reticle focal point.
/// Left click (or Space) throws ONLY while the camera is in `AimOverShoulder`; the scripted
/// `--throw` demo keeps its explicit path and works in any mode.
#[allow(clippy::too_many_arguments)]
fn ball_throw_input(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut visuals: Local<Option<BallVisuals>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut inventory: ResMut<BallInventory>,
    mut progress: ResMut<PlayerProgress>,
    assets: Option<Res<BattleAssets>>,
    throwables: Res<ThrowableTable>,
    player_query: Query<&Transform, With<PlayerAvatar>>,
    camera_query: Query<&Transform, (With<Camera3d>, Without<PlayerAvatar>)>,
    pokemon_query: Query<
        &Transform,
        (With<PokemonSpawn>, Without<PlayerAvatar>, Without<Camera3d>),
    >,
    camera_ctrl: Res<CameraController>,
    automation: Res<AutomationOptions>,
) {
    let aiming = camera_ctrl.mode == CameraMode::AimOverShoulder;
    let mut should_throw = aiming
        && (mouse_buttons.just_pressed(MouseButton::Left) || keys.just_pressed(KeyCode::Space));

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

    let ball_kind = inventory.selected;
    // Bag counters (Brief 18): consume one ball of that category when the item table is loaded.
    if let Some(a) = assets.as_deref() {
        if !consume_ball(&mut progress, &a.items, ball_kind) {
            let msg = format!("No {} left in the bag", ball_kind.display_name());
            println!("{msg}");
            inventory.last_capture_status = Some(msg);
            return;
        }
    }
    inventory.balls_thrown += 1;

    let yaw = camera_ctrl.yaw;
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let forward_horiz = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());

    // Spawn origin: right hand of player
    let spawn_pos = player_xf.translation + right * 0.4 + Vec3::new(0.0, 0.9, 0.0);

    // Aim convergence point: camera ray lock / ground intersection (pure, game::throwing)
    let targets: Vec<Vec3> = pokemon_query.iter().map(|t| t.translation).collect();
    let target_aim_point = pick_aim_point(
        cam_xf.translation,
        *cam_xf.forward(),
        player_xf.translation,
        forward_horiz,
        &targets,
    );

    let params = ThrowParams::for_ball(ball_kind, Some(&*throwables));
    let velocity = launch_velocity(spawn_pos, target_aim_point, params.speed, params.gravity);

    println!(
        "Throwing {} #{}: Speed: {:.1} m/s, Gravity: {:.1} m/s²",
        ball_kind.display_name(),
        inventory.balls_thrown,
        params.speed,
        params.gravity
    );

    let vis = visuals.get_or_insert_with(|| BallVisuals::new(&mut meshes, &mut materials));
    spawn_ball(
        &mut commands,
        vis,
        ball_kind,
        spawn_pos,
        velocity,
        params.gravity,
    );
}

/// Throws the selected bait (V cycles the selection) towards the aim point while aiming.
fn bait_throw_input(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut inventory: ResMut<BallInventory>,
    mut progress: ResMut<PlayerProgress>,
    baits: Res<BaitTable>,
    throwables: Res<ThrowableTable>,
    player_query: Query<&Transform, With<PlayerAvatar>>,
    camera_query: Query<&Transform, (With<Camera3d>, Without<PlayerAvatar>)>,
    pokemon_query: Query<&Transform, (With<PokemonSpawn>, Without<PlayerAvatar>, Without<Camera3d>)>,
    camera_ctrl: Res<CameraController>,
) {
    if camera_ctrl.mode != CameraMode::AimOverShoulder || !keys.just_pressed(KeyCode::KeyX) {
        return;
    }
    let Some(item_id) = inventory.selected_bait else {
        println!("No bait selected: V cycles the bait items in the bag");
        return;
    };
    let Ok(player_xf) = player_query.single() else {
        return;
    };
    let Ok(cam_xf) = camera_query.single() else {
        return;
    };
    if !consume_item(&mut progress, item_id) {
        println!("No bait item #{item_id} left in the bag");
        inventory.selected_bait = None;
        return;
    }
    let yaw = camera_ctrl.yaw;
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let forward_horiz = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let spawn_pos = player_xf.translation + right * 0.4 + Vec3::new(0.0, 0.9, 0.0);
    let targets: Vec<Vec3> = pokemon_query.iter().map(|t| t.translation).collect();
    let target_aim_point = pick_aim_point(
        cam_xf.translation,
        *cam_xf.forward(),
        player_xf.translation,
        forward_horiz,
        &targets,
    );
    let params = ThrowParams::for_throwable(item_id, Some(&*throwables));
    let velocity = launch_velocity(spawn_pos, target_aim_point, params.speed, params.gravity);
    let Some(row) = baits.get(item_id) else {
        println!("Bait item #{item_id} has no decoded row; bait not thrown");
        return;
    };
    spawn_bait(
        &mut commands,
        &mut meshes,
        &mut materials,
        row,
        spawn_pos,
        velocity,
        params.gravity,
    );
    println!("Threw bait item #{item_id} ({} left)", progress.bag.get(&item_id).copied().unwrap_or(0));
}


/// Steps every thrown ball through its lifecycle (`pla::game::ball_lifecycle`): flight, ground
/// roll, shaking, capture result. Every end of the lifecycle despawns the ball entity.
#[allow(clippy::too_many_arguments)]
fn ball_ballistics_and_physics(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut aura: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut ball_query: Query<(Entity, &mut Transform, &mut BallBody, &mut PokeBall), Without<PokemonSpawn>>,
    mut pkmn_query: Query<
        (Entity, &Transform, &mut Visibility, Option<&WildCreature>),
        (With<PokemonSpawn>, Without<PokeBall>),
    >,
    bait_query: Query<&Transform, (With<BaitObject>, Without<PokeBall>)>,
    player_query: Query<&Transform, (With<PlayerAvatar>, Without<PokeBall>, Without<PokemonSpawn>)>,
    battle: Res<LoadedBattle>,
    assets: Option<Res<BattleAssets>>,
    mut progress: ResMut<PlayerProgress>,
    mut consumed: Option<ResMut<ConsumedSpawners>>,
    mut active_spawners: Option<ResMut<ActiveAreaSpawners>>,
    terrain_res: Option<Res<TerrainResource>>,
    mut inventory: ResMut<BallInventory>,
) {
    let dt = time.delta_secs();
    let player_pos = player_query
        .single()
        .map(|t| t.translation)
        .unwrap_or(Vec3::ZERO);

    for (ball_entity, mut ball_xf, mut body, mut ball) in &mut ball_query {
        let was_in_flight = body.0.in_flight();
        let prev_pos = body.0.position;
        let ground_y = terrain_res
            .as_ref()
            .and_then(|t| t.area.sample_world_height(prev_pos.x, prev_pos.z))
            .unwrap_or(0.0);

        let out = step_ball(
            &mut commands,
            ball_entity,
            &mut ball_xf,
            &mut body,
            dt,
            ground_y,
            player_pos,
        );

        if let Some(reason) = out.despawn {
            println!("Ball despawned: {reason:?}");
        }
        match out.event {
            Some(BallEvent::Absorbed) => {
                println!("Ball absorbed the Pokémon (energy draw)...");
            }
            Some(BallEvent::Dropped) => {
                println!("Ball fell to the ground; starting the shake sequence");
            }
            Some(BallEvent::Shake(n)) => println!("Ball Shake {n}..."),
            Some(BallEvent::Captured) => {
                println!("*** CAPTURED! The wild Pokémon was caught! ***");
                inventory.last_capture_status = Some("Captured! ⭐".into());
                // Victory aura ring above the ball (child: it despawns with the ball)
                let (mesh, mat) = aura
                    .get_or_insert_with(|| {
                        (
                            meshes.add(Mesh::from(Torus::new(0.3, 0.02))),
                            materials.add(StandardMaterial {
                                base_color: Color::srgb(1.0, 0.85, 0.2),
                                unlit: true,
                                ..default()
                            }),
                        )
                    })
                    .clone();
                commands.entity(ball_entity).with_children(|p| {
                    p.spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(mat),
                        Transform::from_xyz(0.0, 0.35, 0.0),
                    ));
                });
                // Record the catch (party -> box -> recorded only) and empty the spawner.
                if let Some(target) = ball.target.take() {
                    if let Ok((_, _, _, Some(wild))) = pkmn_query.get_mut(target) {
                        let (species_name, level, dex, spawner_id) = (
                            wild.species_name.clone(),
                            wild.level,
                            wild.national_dex,
                            wild.spawner_id.clone(),
                        );
                        if let Some(db) = battle.0.as_ref() {
                            match build_caught_member(db, &species_name, level) {
                                Ok(member) => {
                                    let storage = store_caught(&mut progress, member);
                                    match storage {
                                        CatchStorage::Party => {
                                            println!("  #{dex} {species_name} stored in the party")
                                        }
                                        CatchStorage::Box => println!(
                                            "  #{dex} {species_name} sent to the box (party full)"
                                        ),
                                        CatchStorage::Full => println!(
                                            "  #{dex} {species_name} recorded as caught (party and box full)"
                                        ),
                                    }
                                }
                                Err(e) => eprintln!("Caught member not built: {e}"),
                            }
                        }
                        if let Some(c) = consumed.as_deref_mut() {
                            c.consume(&spawner_id, time.elapsed_secs_f64());
                        }
                        if let Some(s) = active_spawners.as_deref_mut() {
                            s.spawned_entities.remove(&spawner_id);
                        }
                        commands.entity(target).despawn();
                    }
                }
            }
            Some(BallEvent::BrokeFree) => {
                println!("*** BREAKOUT! The Pokémon broke free! ***");
                inventory.last_capture_status = Some("Broke out!".into());
                if let Some(target) = ball.target.take() {
                    if let Ok((_, _, mut vis, _)) = pkmn_query.get_mut(target) {
                        *vis = Visibility::Visible;
                    }
                }
            }
            None => {}
        }

        if body.0.in_flight() {
            // Subtle spin in flight
            ball_xf.rotate_local_x(12.0 * dt);
        } else if let BallLifecycleState::Shaking { timer, .. } = body.0.state {
            // Wiggle animation while the capture result is pending
            ball_xf.rotation = Quat::from_rotation_z((timer * 14.0).sin() * 0.22);
        }

        if !was_in_flight || out.despawn.is_some() {
            continue;
        }

        // Continuous collision detection against wild Pokémon along this frame's segment
        let next_pos = body.0.position;
        let seg = next_pos - prev_pos;
        let seg_len_sq = seg.length_squared();
        for (pkmn_entity, pkmn_xf, mut pkmn_vis, wild_opt) in &mut pkmn_query {
            let to_pkmn = pkmn_xf.translation - prev_pos;
            let t = if seg_len_sq > 1e-6 {
                (to_pkmn.dot(seg) / seg_len_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let closest = prev_pos + seg * t;
            let hit_dist = closest.distance(pkmn_xf.translation);
            if hit_dist >= 0.95 {
                continue;
            }
            println!(
                "Target Hit! Poke Ball connected with Pokemon (CCD dist: {:.2}m)",
                hit_dist
            );

            // Contain Pokémon inside ball
            *pkmn_vis = Visibility::Hidden;
            ball.target = Some(pkmn_entity);

            // Overworld capture roll: the engine tables, the ball category and the save RNG
            // seed (Brief 18). Ranks and a full-HP target are ASSUMED overworld inputs.
            let category = ball.kind.category();
            let (species, level, dex) = match wild_opt {
                Some(w) => (Some(w.species_name.as_str()), w.level, w.national_dex),
                None => (None, 5, 0),
            };
            if dex != 0 {
                note_encounter(&mut progress, dex);
            }
            let catch_rate = match (&assets, species) {
                (Some(a), Some(name)) => a.catch_rates.lookup(name).0,
                _ => CAPTURE_CATCH_RATE_PLACEHOLDER,
            };
            let throw_distance = pkmn_xf.translation.distance(player_pos);
            let backstrike = is_backstrike(*pkmn_xf.forward(), body.0.velocity);
            let bait = bait_multiplier(
                bait_in_range(
                    (pkmn_xf.translation.x, pkmn_xf.translation.z),
                    bait_query.iter().map(|t| (t.translation.x, t.translation.z)),
                    BAIT_FEED_RADIUS,
                ),
                battle.0.as_ref(),
            );
            let input = WorldCaptureInput {
                ball_category: category,
                suitable_range: suitable_range(ball.kind, throw_distance, backstrike),
                backstrike,
                catch_rate,
                hp_ratio: 1.0,
                player_rank: 1,
                pokemon_rank: 1,
                modifiers: CaptureModifiers { bait, ..CaptureModifiers::default() },
            };
            let mut rng = XorShift32(progress.rng_seed);
            let (shakes, will_capture) = match roll_capture(battle.0.as_ref(), input, &mut rng) {
                Ok(roll) => {
                    println!(
                        "Capture Roll: Category={category}, Rate={catch_rate}, SuitableRange={}, Backstrike={backstrike}, Bait={bait:.2}, Odds={:.1}%, Outcome={}",
                        input.suitable_range,
                        (roll.probability * 100.0).min(100.0),
                        if roll.caught { "SUCCESS" } else { "BREAKOUT" }
                    );
                    (roll.shakes, roll.caught)
                }
                Err(e) => {
                    eprintln!("Capture roll unavailable ({e}); the ball breaks out");
                    (1, false)
                }
            };
            progress.rng_seed = rng.0;
            if level > 0 && dex != 0 {
                println!("Target: #{dex} {species:?} Lv{level}");
            }

            // Impact point: the creature; the ball then drops to the terrain height there.
            let ground_at_hit = terrain_res
                .as_ref()
                .and_then(|t| t.area.sample_world_height(pkmn_xf.translation.x, pkmn_xf.translation.z))
                .unwrap_or(0.0);
            body.0
                .hit_pokemon(pkmn_xf.translation, ground_at_hit, shakes, will_capture);
            ball_xf.translation = body.0.position;
            break;
        }
    }
}

/// Creature encounter system: triggers turn-based Battle when player approaches wild Pokémon (or presses KeyCode::Enter/KeyE).
fn creature_encounter_system(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    player_query: Query<&Transform, With<PlayerAvatar>>,
    mut wild_query: Query<
        (Entity, &Transform, &mut WildCreature),
        (
            Without<PlayerAvatar>,
            Without<pla::game::wild::FleeCooldown>,
        ),
    >,
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
                    println!(
                        "Initialized ActiveBattleSession (Player Lv{} {} vs Wild Lv{} {})",
                        pkmn_state.level, player_species, wild.level, wild.species_name
                    );
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
            let hp =
                calculate_hp(base_hp, pkmn_state.level, pkmn_state.ganba, Some(false)).unwrap_or(0);
            format!(
                "#{} {} [HP@Lv{}: {}]",
                entry.national_dex, entry.species_name, pkmn_state.level, hp
            )
        } else {
            format!("#{}", dex)
        }
    } else {
        format!("#{}", dex)
    };

    let state_str = match game_state.get() {
        GameState::World => "WORLD",
        GameState::Loading => "LOADING",
        GameState::MainMenu => "MAIN MENU",
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
        let capture_tag = inventory.last_capture_status.as_deref().unwrap_or("Ready");

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

/// Thin wrapper kept for the auto-demo: the real implementation lives in `game::zone`.
#[allow(clippy::too_many_arguments)]
fn switch_terrain_area(
    area_idx: u8,
    bin_root: &std::path::Path,
    commands: &mut Commands,
    terrain_res: &mut ResMut<TerrainResource>,
    chunk_query: &Query<Entity, With<TerrainChunkMarker>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    images: &mut ResMut<Assets<Image>>,
    zone_info: &mut ResMut<pla::game::ActiveZoneInfo>,
) -> bool {
    pla::game::zone::switch_terrain_area(
        area_idx,
        bin_root,
        commands,
        terrain_res,
        chunk_query,
        meshes,
        materials,
        images,
        Some(&mut **zone_info),
    )
}

/// Automated headless tick driver and screenshot capturer for test verification.
fn automated_tick_driver(
    mut commands: Commands,
    mut automation: ResMut<AutomationOptions>,
    mut terrain_res: Option<ResMut<TerrainResource>>,
    chunk_query: Query<Entity, With<TerrainChunkMarker>>,
    mut camera_ctrl: ResMut<CameraController>,
    mut player_query: Query<&mut Transform, (With<PlayerAvatar>, Without<PokemonSpawn>)>,
    mut pkmn_query: Query<&mut Transform, (With<PokemonSpawn>, Without<PlayerAvatar>)>,
    mut wild_query: Query<&mut WildCreature>,
    mut next_state: ResMut<NextState<GameState>>,
    mut session: Option<ResMut<ActiveBattleSession>>,
    battle_db: Res<LoadedBattle>,
    battle_assets: Option<Res<BattleAssets>>,
    bin_root_res: Option<Res<BinRootResource>>,
    mut zone_info: Option<ResMut<pla::game::ActiveZoneInfo>>,
    (mut meshes, mut materials, mut images): (
        ResMut<Assets<Mesh>>,
        ResMut<Assets<StandardMaterial>>,
        ResMut<Assets<Image>>,
    ),
    mut exit: MessageWriter<AppExit>,
) {
    automation.current_ticks += 1;

    // Mode 0: Auto-demo playthrough mode (Walk -> Encounter -> Move Menu -> Attack -> Capture -> Return -> Multi-area)
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
                    write_demo_log(
                        "Step 0: Player exploring World with active World HUD and real spawns.",
                    );
                    camera_ctrl.yaw = 0.0;
                    camera_ctrl.pitch = 0.22;
                    camera_ctrl.distance = 5.2;
                }
                // Move player forward towards nearby spawner
                if let Ok(mut p_xf) = player_query.single_mut() {
                    p_xf.translation.x += 0.08;
                    p_xf.translation.z -= 0.04;
                    if let Some(ref terrain) = terrain_res {
                        if let Some(sy) = terrain
                            .area
                            .sample_world_height(p_xf.translation.x, p_xf.translation.z)
                        {
                            p_xf.translation.y = sy + 0.9;
                        }
                    }
                }
                if automation.demo_step_tick == 15 {
                    let shot = demo_dir.join("00_world_exploration.png");
                    write_demo_log(&format!(
                        "Capturing 00_world_exploration.png to {}",
                        shot.display()
                    ));
                    commands
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk(shot));
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
                }
                if automation.demo_step_tick == 20 {
                    let shot = demo_dir.join("01_battle_encounter.png");
                    write_demo_log(&format!(
                        "Capturing 01_battle_encounter.png to {}",
                        shot.display()
                    ));
                    commands
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk(shot));
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
                    if let Some(ref mut sess) = session {
                        for line in pla::game::battle_demo::run_beat(sess, beat) {
                            write_demo_log(&format!("  log: {line}"));
                        }
                    }
                }
                if automation.demo_step_tick == 15 {
                    let shot = demo_dir.join(beat.frame);
                    write_demo_log(&format!("Capturing {} to {}", beat.frame, shot.display()));
                    commands
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk(shot));
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
                        if let Some(sy) = terrain
                            .area
                            .sample_world_height(p_xf.translation.x, p_xf.translation.z)
                        {
                            p_xf.translation.y = sy + 0.9;
                        }
                    }
                }
                if automation.demo_step_tick >= 25 {
                    automation.demo_step = 7;
                    automation.demo_step_tick = 0;
                }
            }
            7 => {
                // Step 7: Area 00 (Jubilife Village) verification (gravel streets, sand coast, grass)
                if automation.demo_step_tick == 1 {
                    write_demo_log("Step 7: Switching to Area 00 (Jubilife Village) for terrain albedo verification.");
                    let bin_root =
                        bin_root_res
                            .as_ref()
                            .map(|b| b.0.clone())
                            .unwrap_or_else(|| {
                                PathBuf::from(
                                    "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin",
                                )
                            });
                    if let (Some(ref mut tr), Some(ref mut zi)) =
                        (terrain_res.as_mut(), zone_info.as_mut())
                    {
                        switch_terrain_area(
                            0,
                            &bin_root,
                            &mut commands,
                            tr,
                            &chunk_query,
                            &mut meshes,
                            &mut materials,
                            &mut images,
                            zi,
                        );
                    }
                    if let Ok(mut p_xf) = player_query.single_mut() {
                        let sy = terrain_res
                            .as_ref()
                            .and_then(|t| t.area.sample_world_height(445.0, 650.0))
                            .unwrap_or(60.8);
                        p_xf.translation = Vec3::new(445.0, sy + 0.9, 650.0);
                    }
                    camera_ctrl.yaw = 0.85;
                    camera_ctrl.pitch = 0.18;
                    camera_ctrl.distance = 7.0;
                    camera_ctrl.focal_offset_y = 0.0;
                }
                if automation.demo_step_tick == 8 {
                    let shot = demo_dir.join("06_area00_gravel_streets.png");
                    write_demo_log(&format!(
                        "Capturing 06_area00_gravel_streets.png to {}",
                        shot.display()
                    ));
                    commands
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk(shot));
                }
                if automation.demo_step_tick == 9 {
                    // Reposition to Area 00 sandy coast / shore
                    if let Ok(mut p_xf) = player_query.single_mut() {
                        let sy = terrain_res
                            .as_ref()
                            .and_then(|t| t.area.sample_world_height(320.0, 780.0))
                            .unwrap_or(10.0);
                        p_xf.translation = Vec3::new(320.0, sy + 0.9, 780.0);
                    }
                    camera_ctrl.yaw = 2.3;
                    camera_ctrl.pitch = 0.20;
                    camera_ctrl.distance = 8.0;
                }
                if automation.demo_step_tick == 16 {
                    let shot = demo_dir.join("07_area00_sand_coast.png");
                    write_demo_log(&format!(
                        "Capturing 07_area00_sand_coast.png to {}",
                        shot.display()
                    ));
                    commands
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk(shot));
                }
                if automation.demo_step_tick == 17 {
                    // Reposition to Area 00 green meadows / grass
                    if let Ok(mut p_xf) = player_query.single_mut() {
                        let sy = terrain_res
                            .as_ref()
                            .and_then(|t| t.area.sample_world_height(550.0, 520.0))
                            .unwrap_or(30.0);
                        p_xf.translation = Vec3::new(550.0, sy + 0.9, 520.0);
                    }
                    camera_ctrl.yaw = 0.40;
                    camera_ctrl.pitch = 0.20;
                    camera_ctrl.distance = 8.0;
                }
                if automation.demo_step_tick == 24 {
                    let shot = demo_dir.join("08_area00_grass.png");
                    write_demo_log(&format!(
                        "Capturing 08_area00_grass.png to {}",
                        shot.display()
                    ));
                    commands
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk(shot));
                }
                if automation.demo_step_tick >= 25 {
                    automation.demo_step = 8;
                    automation.demo_step_tick = 0;
                }
            }
            8 => {
                // Step 8: Area 05 (Alabaster Icelands) verification (snow and ice)
                if automation.demo_step_tick == 1 {
                    write_demo_log("Step 8: Switching to Area 05 (Alabaster Icelands) for snow albedo verification.");
                    let bin_root =
                        bin_root_res
                            .as_ref()
                            .map(|b| b.0.clone())
                            .unwrap_or_else(|| {
                                PathBuf::from(
                                    "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin",
                                )
                            });
                    if let (Some(ref mut tr), Some(ref mut zi)) =
                        (terrain_res.as_mut(), zone_info.as_mut())
                    {
                        switch_terrain_area(
                            5,
                            &bin_root,
                            &mut commands,
                            tr,
                            &chunk_query,
                            &mut meshes,
                            &mut materials,
                            &mut images,
                            zi,
                        );
                    }
                    if let Ok(mut p_xf) = player_query.single_mut() {
                        let sy = terrain_res
                            .as_ref()
                            .and_then(|t| t.area.sample_world_height(512.0, 512.0))
                            .unwrap_or(50.0);
                        p_xf.translation = Vec3::new(512.0, sy + 0.9, 512.0);
                    }
                    camera_ctrl.yaw = 1.10;
                    camera_ctrl.pitch = 0.22;
                    camera_ctrl.distance = 9.0;
                    camera_ctrl.focal_offset_y = 0.0;
                }
                if automation.demo_step_tick == 10 {
                    let shot = demo_dir.join("09_area05_snow.png");
                    write_demo_log(&format!(
                        "Capturing 09_area05_snow.png to {}",
                        shot.display()
                    ));
                    commands
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk(shot));
                }
                if automation.demo_step_tick >= 15 {
                    write_demo_log("Auto-demo playthrough and multi-area terrain verification finished successfully!");
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
                println!(
                    "  Capturing screenshot for '{}' to: {}",
                    label,
                    shot_path.display()
                );
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
            println!(
                "All {} viewpoints verified and captured cleanly in a single process. Exiting.",
                views.len()
            );
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
            println!(
                "Triggered screenshot request at tick {} to: {}",
                shot_tick,
                shot_path.display()
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(shot_path.clone()));
        }
    }

    if let Some(max) = automation.max_ticks {
        if automation.current_ticks >= max {
            println!(
                "Automated execution reached tick target ({}). Exiting cleanly.",
                max
            );
            exit.write(AppExit::Success);
        }
    }
}

/// `--script-controls <DIR>`: world, pause menu open and aim mode with reticle, in one process.
/// Combine with `--throw` to also prove that the thrown ball is frozen by the pause and gone
/// from the world afterwards.
fn controls_demo_driver(
    mut commands: Commands,
    automation: Res<AutomationOptions>,
    mut paused: ResMut<Paused>,
    mut camera_ctrl: ResMut<CameraController>,
    balls: Query<&Transform, With<PokeBall>>,
    mut frozen_at: Local<Option<Vec3>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(dir) = automation.controls_demo_dir.clone() else {
        return;
    };
    let t = automation.current_ticks;
    let shot = |commands: &mut Commands, name: &str| {
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(name);
        println!("[CONTROLS-DEMO] tick {t}: capturing {}", path.display());
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    };
    match t {
        25 => shot(&mut commands, "00_world.png"),
        30 => {
            println!(
                "[CONTROLS-DEMO] tick 30: Esc -> paused, balls in world: {}",
                balls.iter().count()
            );
            paused.0 = true;
            *frozen_at = balls.iter().next().map(|b| b.translation);
        }
        40 => {
            shot(&mut commands, "01_pause_menu.png");
            let now = balls.iter().next().map(|b| b.translation);
            println!("[CONTROLS-DEMO] ball position while paused: before={:?} after={:?} (equal = frozen)", *frozen_at, now);
        }
        45 => {
            println!("[CONTROLS-DEMO] tick 45: resumed, switching to aim mode");
            paused.0 = false;
            camera_ctrl.mode = CameraMode::AimOverShoulder;
        }
        70 => shot(&mut commands, "02_aim_reticle.png"),
        _ => {}
    }
    if t > 70 && t % 30 == 0 {
        println!(
            "[CONTROLS-DEMO] tick {t}: balls in world: {}",
            balls.iter().count()
        );
    }
    if t >= 100 && balls.iter().count() == 0 || t >= 1800 {
        println!(
            "[CONTROLS-DEMO] done at tick {t}: balls in world: {}",
            balls.iter().count()
        );
        exit.write(AppExit::Success);
    }
}

/// Keep the player's arena model on the species that is out: a switch or an evolution changes
/// it, so slot 0 is respawned with the new species (same pose); `spawn_pokemon_models` then
/// loads its glTF like any arena model. Slot 1 follows the opponent the same way (a trainer
/// sends out its next Pokemon).
fn sync_arena_player_model(
    mut commands: Commands,
    session: Option<Res<ActiveBattleSession>>,
    slots: Query<
        (
            Entity,
            &PokemonSpawn,
            &Transform,
            &pla::game::pokemon_anim::PokemonBattleSlot,
        ),
        With<BattleArenaMarker>,
    >,
    mut catalog: ResMut<PokemonCatalogResource>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(sess) = session else { return };
    for (entity, spawn, xf, slot) in &slots {
        // slot 0: the player's active Pokemon; slot 1: the opponent's (a trainer switches).
        let who = if slot.0 == 0 {
            sess.active_player()
        } else {
            sess.opponent()
        };
        let Some(c) = sess.battle.combatants.get(who) else {
            continue;
        };
        let want = c.species.dex;
        if slot.0 > 1 || spawn.national_dex == want {
            continue;
        }
        let key = PokemonAssetKey {
            dex: want,
            form: 0,
            rare: false,
        };
        if !catalog.handles.contains_key(&key) {
            if let Some(rel) = resolve_pokemon_gltf_rel(want, key.form, key.rare) {
                let handle: Handle<Gltf> = asset_server.load(rel);
                catalog.handles.insert(key, handle);
            }
        }
        catalog.spawned.remove(&entity);
        commands.entity(entity).despawn();
        commands
            .spawn((
                BattleArenaMarker,
                PokemonSpawn {
                    national_dex: want,
                    form: 0,
                    rare: false,
                    alpha: false,
                    size_scale: 1.0,
                },
                pla::game::pokemon_anim::PokemonBattleSlot(slot.0),
                Transform::from_translation(xf.translation).with_rotation(xf.rotation),
                Visibility::default(),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(meshes.add(Mesh::from(Sphere::new(0.35)))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: get_species_color(want),
                        ..default()
                    })),
                ));
            });
        println!("Arena slot {} now shows #{want}", slot.0);
    }
}

/// Start-up guard (`cargo test -p pla --example port_viewer`, also part of `cargo test -p pla`).
///
/// Builds the viewer's app through the same [`configure_viewer`] as `main`, but with no window,
/// no GPU, no gamepad and no audio device, then:
///
/// 1. initialises every system and run condition of every schedule (Startup, Update, PostUpdate,
///    FixedUpdate, OnEnter/OnExit of every state, ...) — where Bevy rejects conflicting system
///    parameters (B0001 / B0002), the panics that otherwise only show on the viewer's first frame;
/// 2. runs a few frames, so a system that panics on its first run (for example a required
///    resource nobody inserts) fails here too.
///
/// Known gaps: the render sub-app is not created (no GPU), so Bevy's own render-world systems are
/// not checked; `bevy_audio` (device mode), winit and gilrs are not added; the run starts a new
/// game in a scratch save directory instead of the player's save; and anything that only happens
/// later (state changes such as entering a battle, input, streaming far away) is initialised in
/// step 1 but never run.
#[cfg(test)]
mod startup_schedule_guard {
    use super::*;
    use bevy::render::settings::WgpuSettings;
    use bevy::render::RenderPlugin;
    use bevy::winit::WinitPlugin;

    /// Frames run after the initialisation check (Startup + OnEnter run in the first one).
    const FRAMES: usize = 5;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// The setup `main` would resolve for `args`, without touching the player's save or settings.
    fn setup_for(args: &[&str], scratch: &std::path::Path) -> ViewerSetup {
        let cli_args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let has = |f: &str| cli_args.iter().any(|a| a == f);
        let area_idx: u8 = cli_args
            .iter()
            .position(|a| a == "--area")
            .and_then(|i| cli_args.get(i + 1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let bin_root = std::env::var_os("PLA_BIN_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| repo_root().join("work/pla/pk2/romfs_patched/bin"));
        let cache_root = pla::game::audio::default_cache_root();
        let (data_dir, pml_db, battle_db, battle_assets) = load_game_data();
        let terrain_res = load_terrain(&bin_root, area_idx, TerrainScaleOverride::default());
        let navmesh_res = load_navmesh(&bin_root, area_idx);
        let fallback = if area_idx == 0 { Vec2::new(448.0, 652.0) } else { Vec2::new(512.5, 512.5) };
        let (player_spawn_pos, safe_spawn_xz, spawn_notice) =
            choose_player_spawn(navmesh_res.as_ref(), area_idx, None, false, fallback);

        // Always a new game in a scratch directory: the player's save is never read or written.
        let mut wire_cli = pla::game::save_wire::WireCli::from_args(&cli_args);
        wire_cli.new_game = true;
        wire_cli.save_dir = Some(scratch.to_path_buf());
        let mut save_game =
            pla::game::save_game::SaveGame::new(pla::game::save_game::SaveFile::in_dir(scratch));
        let wire_start = pla::game::save_wire::load_startup(&mut save_game, &wire_cli);

        let mut art_style = pla::game::art_style::ArtStylePlugin::from_env_and_args();
        art_style.settings = pla::game::art_style::StyleSettings::from_args(&cli_args);
        ViewerSetup {
            area_idx,
            bin_root: bin_root.clone(),
            cache_root,
            main_menu: has("--menu"),
            player_model: PlayerModelConfig { asset_rel: None },
            initial_mode: CameraMode::OrbitDefault,
            user_settings: UserSettings::default(),
            settings_dir: scratch.to_path_buf(),
            initial_dex: None,
            data_dir,
            pml_db,
            battle_db,
            battle_assets,
            player_spawn_pos,
            terrain_res,
            navmesh_res,
            safe_spawn_xz,
            spawn_notice,
            static_wild: false,
            debug_zone_links: has("--debug-zone-links"),
            automation: AutomationOptions {
                screenshot_path: None,
                max_ticks: None,
                current_ticks: 0,
                auto_throw_tick: None,
                verify_views_dir: None,
                verify_step: 0,
                verify_step_tick: 0,
                auto_demo: false,
                demo_output_dir: None,
                demo_step: 0,
                demo_step_tick: 0,
                controls_demo_dir: None,
            },
            save_game,
            wire_start,
            wire_cli,
            art_style,
            trainers: pla::game::trainer_encounter::TrainerEncounterPlugin {
                markers: !has("--no-trainers"),
                forced: None,
                data_dir: None,
            },
            // Null: the whole game audio pipeline is registered but no device is opened.
            audio: pla::game::audio::GameAudioPlugin::with_mode(pla::game::audio::AudioMode::Null),
            effects: pla::game::effects::EffectsPlugin::default(),
            bag: pla::game::bag_screen::BagScreenPlugin { debug_shop: has("--debug-shop"), data_dir: None },
            minimap: pla::game::minimap::MinimapPlugin {
                enabled: !has("--no-minimap"),
                bin_root: Some(bin_root),
                game_map: has("--game-map"),
                size: pla::game::minimap::DEFAULT_MAP_SIZE,
            },
            cli_args,
        }
    }

    /// `DefaultPlugins` as in `main`, minus everything that touches the user's machine: no
    /// window, no event loop, no GPU, no gamepad.
    fn headless_app() -> App {
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: repo_root().join("work").to_string_lossy().to_string(),
                    unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: WgpuSettings { backends: None, ..default() }.into(),
                    ..default()
                })
                .disable::<WinitPlugin>()
                .disable::<bevy::gilrs::GilrsPlugin>(),
        );
        // Stub: without a GPU the render plugin skips its entity-sync bookkeeping, whose
        // resource the component hooks of renderable components still need. With no render
        // world nothing drains it, which is harmless for a few frames.
        if !app.is_plugin_added::<bevy::render::sync_world::SyncWorldPlugin>() {
            app.add_plugins(bevy::render::sync_world::SyncWorldPlugin);
        }
        app
    }

    fn check_viewer_start(name: &str, args: &[&str]) {
        let scratch =
            std::env::temp_dir().join(format!("pla-viewer-schedule-guard-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&scratch).expect("scratch dir");
        let mut app = headless_app();
        configure_viewer(&mut app, setup_for(args, &scratch));
        app.finish();
        app.cleanup();
        let counts = pla::game::schedule_check::system_counts(&app);
        for (label, n) in &counts {
            println!("[{name}] schedule {label}: {n} systems");
        }
        let update = counts.iter().find(|(l, _)| l == "Update").map_or(0, |(_, n)| *n);
        assert!(update > 50, "[{name}] only {update} Update systems: the viewer was not built");

        let problems = pla::game::schedule_check::initialize_all_schedules(&mut app);
        assert!(
            problems.is_empty(),
            "[{name}] {} start-up schedule problem(s):\n{}",
            problems.len(),
            problems.join("\n")
        );
        for _ in 0..FRAMES {
            app.update();
        }
        let _ = std::fs::remove_dir_all(&scratch);
    }

    /// Exactly what `./play.sh --area 1` asks for (direct start into the world).
    #[test]
    fn play_sh_area_1_starts() {
        check_viewer_start("play-area-1", &["port_viewer", "--area", "1"]);
    }

    /// Every optional registration on: main menu, original art style, debug zone links and shop.
    #[test]
    fn menu_and_debug_options_start() {
        check_viewer_start(
            "menu-debug",
            &[
                "port_viewer",
                "--area",
                "1",
                "--menu",
                "--style",
                "original",
                "--debug-zone-links",
                "--debug-shop",
            ],
        );
    }
}
