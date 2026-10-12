//! CPU terrain previews and opt-in interactive Bevy terrain viewer.
//!
//! Usage: `cargo run -p pla --example terrain_viewer -- --bin-root <romfs_patched/bin> [--area 0] [--cell-size 1] [--vertical-scale 100]`
//! The scale defaults are provisional debug units, not measured game units.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::render::mesh::Mesh;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::WindowResolution;

use pla::assets::bntx::TextureData;
use pla::assets::terrain::{build_mesh_chunks, TerrainArea, TerrainMeshSettings};

#[derive(Resource)]
struct ViewerScene {
    area: TerrainArea,
    chunks: Vec<pla::assets::terrain::TerrainMeshChunk>,
    blended_albedo: Option<TextureData>,
    settings: TerrainMeshSettings,
    camera_view: usize,
}

#[derive(Component)]
struct TerrainCamera;

#[derive(Resource)]
struct CameraLook {
    yaw: f32,
    pitch: f32,
}

fn main() -> ExitCode {
    let (bin_root, area, settings, screenshot_dir, capture, camera_view, interactive) =
        match parse_args() {
            Ok(Some(parsed)) => parsed,
            Ok(None) => return ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(2);
            }
        };
    if let Some(directory) = screenshot_dir {
        if capture.is_some() {
            eprintln!("--capture and --screenshot-dir cannot be combined");
            return ExitCode::from(2);
        }
        for area_number in 0..=6 {
            if let Err(error) = cpu_preview(&bin_root, area_number, settings, &directory, 3) {
                eprintln!("CPU preview area {area_number:02} failed: {error}");
                return ExitCode::FAILURE;
            }
        }
        return ExitCode::SUCCESS;
    }
    if let Some(output) = capture {
        let parent = output.parent().unwrap_or(Path::new("."));
        if let Err(error) = cpu_preview(&bin_root, area, settings, parent, 1) {
            eprintln!("CPU preview failed: {error}");
            return ExitCode::FAILURE;
        }
        let generated = parent.join(format!("area{area:02}_cpu_preview_light0.png"));
        return match std::fs::rename(&generated, &output) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!(
                    "rename {} to {}: {error}",
                    generated.display(),
                    output.display()
                );
                ExitCode::FAILURE
            }
        };
    }
    if !interactive {
        eprintln!("No window is opened by default. Pass --interactive to opt into the Bevy viewer, or --screenshot-dir DIR for CPU-only previews.");
        return ExitCode::from(2);
    }
    run_area(&bin_root, area, settings, camera_view)
}

fn run_area(
    bin_root: &Path,
    area: u8,
    settings: TerrainMeshSettings,
    camera_view: usize,
) -> ExitCode {
    let area_data = match TerrainArea::load(bin_root, area) {
        Ok(area) => area,
        Err(error) => {
            eprintln!("terrain load failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let height_min = area_data
        .heightmap
        .samples
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min);
    let height_max = area_data
        .heightmap
        .samples
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    println!(
        "Area {:02}: {}x{} heights [{height_min:.4}, {height_max:.4}] normalized samples; provisional scale cell={} vertical={}",
        area,
        area_data.heightmap.width,
        area_data.heightmap.height,
        settings.horizontal_cell_size,
        settings.vertical_scale
    );
    println!(
        "Normal map {}x{}; splat {}x{}; {} mask candidate(s)",
        area_data.normal_map.width,
        area_data.normal_map.height,
        area_data.splat_alpha.width,
        area_data.splat_alpha.height,
        area_data.masks.len()
    );
    if let Some(material) = &area_data.material {
        for (file, role) in &material.slots.textures {
            println!("Material slot {role}: {file}");
        }
        for parameter in &material.parameter_candidates {
            println!(
                "Observed f32-before-name candidate {} = {} (role unverified)",
                parameter.name, parameter.value
            );
        }
        if !area_data.unresolved_ground_textures.is_empty() {
            println!(
                "Unresolved material textures: {:?}",
                area_data.unresolved_ground_textures
            );
        }
    } else {
        println!("No unique TRMTR candidate; using a neutral fallback material.");
    }
    let blended_albedo = match area_data.blend_splat_by_slot_order_hypothesis() {
        Ok(texture) => {
            println!("Rendering splat channels R/G/B/A against material slot order (HYPOTHESIS). No channel binding is verified.");
            Some(texture)
        }
        Err(error) => {
            println!("Splat blend unavailable ({error}); rendering a neutral fallback material.");
            None
        }
    };
    let chunks = match build_mesh_chunks(&area_data.heightmap, settings) {
        Ok(chunks) => chunks,
        Err(error) => {
            eprintln!("terrain mesh build failed: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: format!("PLA terrain viewer — area {area:02} (provisional scale)"),
            resolution: WindowResolution::new(1280, 800),
            ..default()
        }),
        ..default()
    }));
    app.insert_resource(ViewerScene {
        area: area_data,
        chunks,
        blended_albedo,
        settings,
        camera_view,
    });
    app.add_systems(Startup, setup_scene);
    app.add_systems(Update, fly_camera);
    app.run();
    ExitCode::SUCCESS
}

fn parse_args() -> Result<
    Option<(
        PathBuf,
        u8,
        TerrainMeshSettings,
        Option<PathBuf>,
        Option<PathBuf>,
        usize,
        bool,
    )>,
    String,
> {
    let mut args = std::env::args().skip(1);
    let mut bin_root = std::env::var_os("PLA_BIN_ROOT").map(PathBuf::from);
    let mut area = 0u8;
    let mut settings = TerrainMeshSettings::default();
    let mut screenshot_dir = None;
    let mut capture = None;
    let mut camera_view = 0usize;
    let mut interactive = false;
    while let Some(arg) = args.next() {
        let value = match arg.as_str() {
            "--help" | "-h" => {
                println!("Usage: terrain_viewer --bin-root <romfs_patched/bin> [--area 0..6] [--cell-size N] [--vertical-scale N] [--screenshot-dir DIR] [--capture FILE.png] [--interactive]\n--screenshot-dir writes CPU shaded-relief + splat previews for all seven areas (three light directions each), without opening a window. --interactive is required to open the Bevy viewer. Defaults: cell-size=1 and vertical-scale=100 are provisional debug units.");
                return Ok(None);
            }
            "--interactive" => {
                interactive = true;
                continue;
            }
            "--bin-root" => args.next().map(|v| bin_root = Some(PathBuf::from(v))),
            "--area" => args
                .next()
                .and_then(|v| v.parse::<u8>().ok())
                .map(|v| area = v),
            "--cell-size" => args
                .next()
                .and_then(|v| v.parse::<f32>().ok())
                .map(|v| settings.horizontal_cell_size = v),
            "--vertical-scale" => args
                .next()
                .and_then(|v| v.parse::<f32>().ok())
                .map(|v| settings.vertical_scale = v),
            "--screenshot-dir" => args.next().map(|v| screenshot_dir = Some(PathBuf::from(v))),
            "--capture" => args.next().map(|v| capture = Some(PathBuf::from(v))),
            "--camera-view" => args
                .next()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|view| *view < 3)
                .map(|view| camera_view = view),
            other => {
                return Err(format!("unknown argument: {other}"));
            }
        };
        if value.is_none() {
            return Err(format!("invalid or missing value for {arg}"));
        }
    }
    let Some(bin_root) = bin_root else {
        return Err("provide --bin-root or set PLA_BIN_ROOT".into());
    };
    if area > 6 {
        return Err("--area must be in 0..=6".into());
    }
    Ok(Some((
        bin_root,
        area,
        settings,
        screenshot_dir,
        capture,
        camera_view,
        interactive,
    )))
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    scene: Res<ViewerScene>,
) {
    let albedo = scene.blended_albedo.as_ref().map(|texture| {
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
    let material = materials.add(StandardMaterial {
        base_color: if albedo.is_some() {
            Color::WHITE
        } else {
            Color::srgb(0.27, 0.38, 0.25)
        },
        base_color_texture: albedo,
        perceptual_roughness: 0.95,
        ..default()
    });
    for chunk in &scene.chunks {
        commands.spawn((
            Mesh3d(meshes.add(chunk.mesh.clone())),
            MeshMaterial3d(material.clone()),
        ));
    }

    let span_x = (scene.area.heightmap.width - 1) as f32 * scene.settings.horizontal_cell_size;
    let span_z = (scene.area.heightmap.height - 1) as f32 * scene.settings.horizontal_cell_size;
    let camera_pos = match scene.camera_view {
        0 => Vec3::new(
            span_x * 0.35,
            scene.settings.vertical_scale * 1.5,
            span_z * 0.45,
        ),
        1 => Vec3::new(span_x * 0.8, span_z * 0.7, span_z * 0.8),
        _ => Vec3::new(-span_x * 0.45, span_z * 0.8, span_z * 0.45),
    };
    let look_direction = (Vec3::ZERO - camera_pos).normalize();
    commands.insert_resource(CameraLook {
        yaw: (-look_direction.x).atan2(-look_direction.z),
        pitch: look_direction.y.asin(),
    });
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(camera_pos).looking_at(Vec3::ZERO, Vec3::Y),
        TerrainCamera,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            ..default()
        },
        Transform::from_xyz(-0.4, 1.0, 0.4).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn cpu_preview(
    bin_root: &Path,
    area: u8,
    settings: TerrainMeshSettings,
    output_dir: &Path,
    light_count: usize,
) -> Result<(), String> {
    const SIDE: u32 = 1024;
    if !settings.horizontal_cell_size.is_finite()
        || settings.horizontal_cell_size <= 0.0
        || !settings.vertical_scale.is_finite()
        || settings.vertical_scale <= 0.0
    {
        return Err("cell size and vertical scale must be finite and positive".into());
    }
    let area_data = TerrainArea::load(bin_root, area)?;
    let width = area_data.heightmap.width;
    let height = area_data.heightmap.height;
    let albedo = area_data.blend_splat_by_slot_order_hypothesis().ok();
    let lights = [
        Vec3::new(-0.45, 0.78, 0.43).normalize(),
        Vec3::new(0.62, 0.72, -0.31).normalize(),
        Vec3::new(-0.1, 0.95, -0.7).normalize(),
    ];
    std::fs::create_dir_all(output_dir)
        .map_err(|error| format!("create {}: {error}", output_dir.display()))?;
    for (view, light) in lights.iter().take(light_count).enumerate() {
        let mut rgb = vec![0u8; (SIDE * SIDE * 3) as usize];
        for y in 0..SIDE {
            let source_y = y as usize * (height as usize - 1) / (SIDE as usize - 1);
            for x in 0..SIDE {
                let source_x = x as usize * (width as usize - 1) / (SIDE as usize - 1);
                let left = source_y * width as usize + source_x.saturating_sub(1);
                let right = source_y * width as usize + (source_x + 1).min(width as usize - 1);
                let up = source_y.saturating_sub(1) * width as usize + source_x;
                let down = (source_y + 1).min(height as usize - 1) * width as usize + source_x;
                let dx = (area_data.heightmap.samples[right] - area_data.heightmap.samples[left])
                    * settings.vertical_scale
                    / (2.0 * settings.horizontal_cell_size);
                let dz = (area_data.heightmap.samples[down] - area_data.heightmap.samples[up])
                    * settings.vertical_scale
                    / (2.0 * settings.horizontal_cell_size);
                let normal = Vec3::new(-dx, 1.0, -dz).normalize_or_zero();
                let shade = 0.32 + 0.68 * normal.dot(*light).max(0.0);
                let color = albedo
                    .as_ref()
                    .map(|texture| {
                        let tx = source_x * texture.width as usize / width as usize;
                        let ty = source_y * texture.height as usize / height as usize;
                        let offset = (ty.min(texture.height as usize - 1) * texture.width as usize
                            + tx.min(texture.width as usize - 1))
                            * 4;
                        [
                            texture.rgba[offset],
                            texture.rgba[offset + 1],
                            texture.rgba[offset + 2],
                        ]
                    })
                    .unwrap_or([112, 126, 103]);
                let offset = (y * SIDE + x) as usize * 3;
                for channel in 0..3 {
                    rgb[offset + channel] = (color[channel] as f32 * shade).clamp(0.0, 255.0) as u8;
                }
            }
        }
        let path = output_dir.join(format!("area{area:02}_cpu_preview_light{view}.png"));
        let file = std::fs::File::create(&path)
            .map_err(|error| format!("create {}: {error}", path.display()))?;
        let mut encoder = png::Encoder::new(file, SIDE, SIDE);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(&rgb))
            .map_err(|error| format!("write {}: {error}", path.display()))?;
        println!(
            "CPU preview (shaded relief + {}): {}",
            if albedo.is_some() {
                "splat hypothesis"
            } else {
                "neutral color; material unresolved"
            },
            path.display()
        );
    }
    Ok(())
}

fn fly_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut look: ResMut<CameraLook>,
    mut cameras: Query<&mut Transform, With<TerrainCamera>>,
) {
    let Ok(mut transform) = cameras.single_mut() else {
        return;
    };
    let mut mouse_delta = Vec2::ZERO;
    for motion in mouse_motion.read() {
        if mouse_buttons.pressed(MouseButton::Right) {
            mouse_delta += motion.delta;
        }
    }
    look.yaw -= mouse_delta.x * 0.0035;
    look.pitch = (look.pitch - mouse_delta.y * 0.0035).clamp(-1.45, 1.45);
    if keys.pressed(KeyCode::KeyQ) {
        look.yaw += 1.5 * time.delta_secs();
    }
    if keys.pressed(KeyCode::KeyE) {
        look.yaw -= 1.5 * time.delta_secs();
    }
    transform.rotation = Quat::from_euler(EulerRot::YXZ, look.yaw, look.pitch, 0.0);

    let mut local = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        local.z -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        local.z += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        local.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        local.x += 1.0;
    }
    if keys.pressed(KeyCode::Space) {
        local.y += 1.0;
    }
    if keys.pressed(KeyCode::ControlLeft) {
        local.y -= 1.0;
    }
    if local != Vec3::ZERO {
        let direction = transform.rotation * local.normalize();
        transform.translation += direction * 500.0 * time.delta_secs();
    }
}
