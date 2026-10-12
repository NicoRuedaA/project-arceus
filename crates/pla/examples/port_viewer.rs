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

use pla::pml::{calculate_hp, PmlDatabase};

#[derive(Component)]
struct PlayerAvatar;

#[derive(Component)]
struct DirectionIndicator;

#[derive(Component)]
#[allow(dead_code)]
struct PokemonSpawn {
    national_dex: u16,
}

#[derive(Component)]
struct Reticle;

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
struct AutomationOptions {
    screenshot_path: Option<PathBuf>,
    max_ticks: Option<u32>,
    current_ticks: u32,
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut screenshot_path = None;
    let mut max_ticks = None;
    let mut initial_mode = CameraMode::OrbitDefault;
    let mut initial_dex: Option<u16> = None;

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
            "--help" | "-h" => {
                println!("Usage: cargo run -p pla --example port_viewer -- [OPTIONS]");
                println!("Options:");
                println!("  --screenshot <PATH>   Capture window screenshot and exit");
                println!("  --ticks <COUNT>       Run for COUNT frames and exit");
                println!("  --mode <orbit|aim>    Initial camera mode (default: orbit)");
                println!("  --species <DEX>       National dex species ID to spawn (default: 25 Pikachu)");
                return ExitCode::SUCCESS;
            }
            _ => {}
        }
    }

    if screenshot_path.is_some() && max_ticks.is_none() {
        max_ticks = Some(70);
    }

    // Attempt to locate PML data directory.
    let pml_dir = [
        PathBuf::from("data"),
        PathBuf::from("../../data"),
        PathBuf::from("/home/nico/work/decompilacion/data"),
    ]
    .into_iter()
    .find(|p| p.join("pml_personal.tsv").exists());

    let pml_db = pml_dir.and_then(|dir| PmlDatabase::load_from_dir(dir).ok());

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
    app.insert_resource(AutomationOptions {
        screenshot_path,
        max_ticks,
        current_ticks: 0,
    });

    app.add_systems(Startup, setup_scene);
    app.add_systems(
        Update,
        (
            player_locomotion,
            camera_controls,
            update_camera_transform,
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
    camera_ctrl: Res<CameraController>,
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

    // Aiming Reticle (attached to camera space or positioned in front).
    commands.spawn((
        Reticle,
        Mesh3d(meshes.add(Mesh::from(Torus::new(0.04, 0.005)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.2, 0.2),
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, -2.0),
        if camera_ctrl.mode == CameraMode::AimOverShoulder {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
    ));

    // Third-person Orbit Camera.
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 3.0, 5.0).looking_at(Vec3::new(0.0, 1.2, 0.0), Vec3::Y),
    ));

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
        *ret_vis = match camera_ctrl.mode {
            CameraMode::OrbitDefault => Visibility::Hidden,
            CameraMode::AimOverShoulder => Visibility::Visible,
        };
    }
}

/// Transform solver using canonical PLA spherical coordinates:
/// ΔX = dist * cos(pitch) * sin(yaw)
/// ΔY = dist * sin(pitch)
/// ΔZ = dist * cos(pitch) * cos(yaw)
/// cameraPos = lookAtPos + Δ
fn update_camera_transform(
    camera_ctrl: Res<CameraController>,
    player_query: Query<&Transform, (With<PlayerAvatar>, Without<Camera3d>, Without<Reticle>)>,
    mut camera_query: Query<&mut Transform, (With<Camera3d>, Without<PlayerAvatar>, Without<Reticle>)>,
    mut reticle_query: Query<&mut Transform, (With<Reticle>, Without<Camera3d>, Without<PlayerAvatar>)>,
) {
    let Ok(player_xf) = player_query.single() else {
        return;
    };
    let Ok(mut cam_xf) = camera_query.single_mut() else {
        return;
    };

    let mut focal_target = player_xf.translation + Vec3::new(0.0, camera_ctrl.focal_offset_y, 0.0);

    // If in over-the-shoulder Aim mode, shift focal point to right shoulder.
    if camera_ctrl.mode == CameraMode::AimOverShoulder {
        let right = Vec3::new(camera_ctrl.yaw.cos(), 0.0, -camera_ctrl.yaw.sin());
        focal_target += right * 0.45 + Vec3::new(0.0, 0.15, 0.0);
    }

    let pitch = camera_ctrl.pitch;
    let yaw = camera_ctrl.yaw;
    let dist = camera_ctrl.distance;

    let delta_x = dist * pitch.cos() * yaw.sin();
    let delta_y = dist * pitch.sin();
    let delta_z = dist * pitch.cos() * yaw.cos();

    let eye_pos = focal_target + Vec3::new(delta_x, delta_y, delta_z);

    cam_xf.translation = eye_pos;
    cam_xf.look_at(focal_target, Vec3::Y);

    // Position aiming reticle in front of camera along forward ray.
    if let Ok(mut ret_xf) = reticle_query.single_mut() {
        let forward = (focal_target - eye_pos).normalize();
        ret_xf.translation = eye_pos + forward * 2.5;
        ret_xf.look_at(eye_pos, Vec3::Y);
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

/// Updates window title and telemetry with real PML statistics.
fn update_hud_and_telemetry(
    mut windows: Query<&mut Window>,
    player_query: Query<&Transform, With<PlayerAvatar>>,
    camera_ctrl: Res<CameraController>,
    pkmn_state: Res<ActivePokemonState>,
    pml: Res<LoadedPml>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let Ok(player_xf) = player_query.single() else {
        return;
    };

    let dex = pkmn_state.dex_list[pkmn_state.current_index];
    let mode_str = match camera_ctrl.mode {
        CameraMode::OrbitDefault => "Orbit (Default 3rd Person)",
        CameraMode::AimOverShoulder => "Aim (ZL Over-the-Shoulder)",
    };

    let pml_info = if let Some(db) = &pml.0 {
        if let Some(entry) = db.personal.iter().find(|p| p.national_dex == dex) {
            let base_hp = entry.base_stats[0];
            let hp = calculate_hp(base_hp, pkmn_state.level, pkmn_state.ganba, Some(false))
                .unwrap_or(0);
            format!(
                "Pkmn: #{} {} [Type: {}/{}, Base BST: {}, HP@Lv{}(G{}): {}]",
                entry.national_dex,
                entry.species_name,
                entry.type1_name,
                if entry.type2_name.is_empty() { "None" } else { &entry.type2_name },
                entry.base_stat_total,
                pkmn_state.level,
                pkmn_state.ganba,
                hp
            )
        } else {
            format!("Pkmn: #{} (not found in table)", dex)
        }
    } else {
        format!("Pkmn: #{} (PML fallback)", dex)
    };

    window.title = format!(
        "PLA Port | Mode: {} | Pos: ({:.1}, {:.1}, {:.1}) | Dist: {:.1}m | {}",
        mode_str,
        player_xf.translation.x,
        player_xf.translation.y,
        player_xf.translation.z,
        camera_ctrl.distance,
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
