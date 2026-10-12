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

use std::path::PathBuf;
use std::process::ExitCode;

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::WindowResolution;

use pla::battle::BattleData;
use pla::pml::{calculate_hp, PmlDatabase};

#[derive(Component)]
struct PlayerAvatar;

#[derive(Component)]
struct DirectionIndicator;

#[derive(Component)]
struct PokemonSpawn {
    #[allow(dead_code)]
    national_dex: u16,
}

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
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut screenshot_path = None;
    let mut max_ticks = None;
    let mut initial_mode = CameraMode::OrbitDefault;
    let mut initial_dex: Option<u16> = None;
    let mut auto_throw_tick: Option<u32> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--screenshot" => {
                if let Some(path) = args.next() {
                    screenshot_path = Some(PathBuf::from(path));
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

    if screenshot_path.is_some() && max_ticks.is_none() {
        max_ticks = Some(70);
    }

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

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Pokémon Legends: Arceus — Bevy Port Interactive Viewer".into(),
                    resolution: WindowResolution::new(1280, 720),
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin::default()),
    );

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
    app.insert_resource(AutomationOptions {
        screenshot_path,
        max_ticks,
        current_ticks: 0,
        auto_throw_tick,
    });

    app.add_systems(Startup, setup_scene);
    app.add_systems(
        Update,
        (
            player_locomotion,
            camera_controls,
            update_camera_transform,
            ball_selection_controls,
            ball_throw_input,
            ball_ballistics_and_physics,
            capture_resolution_system,
            pokemon_selection_controls,
            update_hud_and_telemetry,
            automated_tick_driver,
        )
            .chain(),
    );

    app.run();
    ExitCode::SUCCESS
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    pml: Res<LoadedPml>,
    pkmn_state: Res<ActivePokemonState>,
) {
    // Ground plane (grid checkered field).
    let ground_size = 40.0;
    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Plane3d::default().mesh().size(ground_size, ground_size)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.28, 0.42, 0.22), // Hisui meadow green
            perceptual_roughness: 0.9,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // Field markers / waypoints for spatial orientation.
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

    // Player character avatar (capsule representation with facing indicator).
    let _player_body = commands
        .spawn((
            PlayerAvatar,
            Transform::from_xyz(0.0, 0.9, 0.0),
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

    // Spawn wild Pokémon in front of the player (in -Z).
    let active_dex = pkmn_state.dex_list[pkmn_state.current_index];
    let pkmn_color = get_species_color(active_dex);

    commands
        .spawn((
            PokemonSpawn {
                national_dex: active_dex,
            },
            Transform::from_xyz(0.0, 0.45, -3.8),
            Visibility::default(),
        ))
        .with_children(|parent| {
            // Main body
            parent.spawn((
                Mesh3d(meshes.add(Mesh::from(Sphere::new(0.5)))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: pkmn_color,
                    perceptual_roughness: 0.5,
                    ..default()
                })),
            ));
            // Ears (distinctive Pikachu/creature ears)
            for x_offset in [-0.25, 0.25] {
                parent.spawn((
                    Mesh3d(meshes.add(Mesh::from(Cone::new(0.1, 0.4)))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.95, 0.85, 0.15),
                        ..default()
                    })),
                    Transform::from_xyz(x_offset, 0.55, 0.0)
                        .with_rotation(Quat::from_rotation_z(if x_offset > 0.0 { -0.3 } else { 0.3 })),
                ));
            }
            // Cheeks
            for x_offset in [-0.35, 0.35] {
                parent.spawn((
                    Mesh3d(meshes.add(Mesh::from(Sphere::new(0.1)))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.95, 0.15, 0.15),
                        ..default()
                    })),
                    Transform::from_xyz(x_offset, 0.05, 0.35),
                ));
            }
            // Facing snout towards player (+Z)
            parent.spawn((
                Mesh3d(meshes.add(Mesh::from(Cone::new(0.08, 0.15)))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.2, 0.2, 0.2),
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.05, 0.48).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            ));
        });

    // Third-person Orbit Camera with centered aiming crosshair ("X").
    commands
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 3.0, 5.0).looking_at(Vec3::new(0.0, 1.2, 0.0), Vec3::Y),
        ))
        .with_children(|cam| {
            // Screen-centered aiming reticle with prominent "X" crosshair,
            // center dot, and circular frame positioned in camera local space.
            cam.spawn((
                Reticle,
                Transform::from_xyz(0.0, 0.0, -1.8),
                Visibility::Visible,
            ))
            .with_children(|reticle| {
                let x_bar_mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 1.0, 1.0), // Crisp white
                    unlit: true,
                    ..default()
                });
                let center_dot_mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.2, 0.2), // Bright red center pip
                    unlit: true,
                    ..default()
                });

                // Diagonal bar 1 of "X": +45 degrees in XY plane
                reticle.spawn((
                    Mesh3d(meshes.add(Mesh::from(Cylinder::new(0.003, 0.10)))),
                    MeshMaterial3d(x_bar_mat.clone()),
                    Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
                ));

                // Diagonal bar 2 of "X": -45 degrees in XY plane
                reticle.spawn((
                    Mesh3d(meshes.add(Mesh::from(Cylinder::new(0.003, 0.10)))),
                    MeshMaterial3d(x_bar_mat),
                    Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_4)),
                ));

                // Center dot / pip marking exact pixel center
                reticle.spawn((
                    Mesh3d(meshes.add(Mesh::from(Sphere::new(0.005)))),
                    MeshMaterial3d(center_dot_mat),
                    Transform::from_xyz(0.0, 0.0, 0.002),
                ));
            });
        });

    // Sun directional light & ambient illumination.
    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(8.0, 16.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    println!("============================================================");
    println!("Pokémon Legends: Arceus — Interactive Bevy Port Viewer");
    println!("Controls:");
    println!("  WASD / Arrows : Walk / Run in world");
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

fn get_species_color(dex: u16) -> Color {
    match dex {
        25 => Color::srgb(0.95, 0.85, 0.15),  // Pikachu (Electric Yellow)
        501 => Color::srgb(0.25, 0.65, 0.95), // Oshawott (Water Blue)
        722 => Color::srgb(0.55, 0.82, 0.35), // Rowlet (Grass Green)
        155 => Color::srgb(0.95, 0.45, 0.15), // Cyndaquil (Fire Orange)
        448 => Color::srgb(0.25, 0.45, 0.85), // Lucario (Aura Blue)
        _ => Color::srgb(0.8, 0.8, 0.8),
    }
}

/// Player locomotion system: translates input along camera relative vectors.
fn player_locomotion(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    camera_ctrl: Res<CameraController>,
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
        transform.translation += move_vec * speed * dt;

        // Smoothly orient player avatar towards movement direction.
        let target_rot = Quat::from_rotation_y(move_vec.x.atan2(move_vec.z));
        transform.rotation = transform.rotation.slerp(target_rot, (15.0 * dt).min(1.0));
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

                // Ground plane collision (Y <= 0.12)
                if ball_xf.translation.y <= 0.12 {
                    ball_xf.translation.y = 0.12;
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

    let capture_tag = inventory
        .last_capture_status
        .as_deref()
        .unwrap_or("Ready");

    window.title = format!(
        "PLA Port | Mode: {} | Pos: ({:.1}, {:.1}) | Ball: {} (thrown: {}) | Status: {} | Target: {}",
        mode_str,
        player_xf.translation.x,
        player_xf.translation.z,
        inventory.selected.display_name(),
        inventory.balls_thrown,
        capture_tag,
        pml_info
    );
}

/// Automated headless tick driver and screenshot capturer for test verification.
fn automated_tick_driver(
    mut commands: Commands,
    mut automation: ResMut<AutomationOptions>,
    mut exit: MessageWriter<AppExit>,
) {
    automation.current_ticks += 1;

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
