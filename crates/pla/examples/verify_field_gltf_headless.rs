//! Headless offscreen glTF verification for 10 diverse field models using Bevy.
//!
//! Loads exported glTF files through Bevy's asset system and renders offscreen
//! verification captures in a single process without opening any visible window.

use std::path::PathBuf;
use std::process::ExitCode;

use bevy::gltf::{Gltf, GltfMaterial, GltfMesh};
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::WindowResolution;

#[derive(Resource)]
struct VerifyQueue {
    models: Vec<(String, PathBuf, PathBuf, f32)>, // (label, gltf_rel_path, out_png, cam_dist)
    current_idx: usize,
    frame_in_step: u32,
    active_gltf: Option<Handle<Gltf>>,
    spawned_entities: Vec<Entity>,
}

#[derive(Component)]
struct MainCamera;

fn setup(mut commands: Commands) {
    // 3D camera pointing at model origin
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.0, 6.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        MainCamera,
    ));

    // Directional light 1
    commands.spawn((
        DirectionalLight {
            illuminance: 14000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(5.0, 10.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Directional fill light 2
    commands.spawn((
        DirectionalLight {
            illuminance: 7000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-5.0, 4.0, -4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn process_queue(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    gltf_materials: Res<Assets<GltfMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut queue: ResMut<VerifyQueue>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
    mut exit: MessageWriter<AppExit>,
) {
    queue.frame_in_step += 1;

    // Step 0: Request load of current model
    if queue.frame_in_step == 1 {
        if queue.current_idx >= queue.models.len() {
            println!("All {} field models verified successfully.", queue.models.len());
            exit.write(AppExit::Success);
            return;
        }

        let (label, gltf_rel, _, cam_dist) = queue.models[queue.current_idx].clone();
        println!(
            "[{}/{}] Loading glTF asset '{}' ({:?})",
            queue.current_idx + 1,
            queue.models.len(),
            label,
            gltf_rel
        );

        if let Ok(mut cam_tx) = camera_query.single_mut() {
            *cam_tx = Transform::from_xyz(0.0, cam_dist * 0.35, cam_dist)
                .looking_at(Vec3::new(0.0, cam_dist * 0.15, 0.0), Vec3::Y);
        }

        let asset_path = format!("cache/gltf_field/{}", gltf_rel.to_string_lossy());
        let handle: Handle<Gltf> = asset_server.load(asset_path);
        queue.active_gltf = Some(handle);
    }

    // Step 1: Wait for Gltf asset to load, then spawn primitives
    if queue.spawned_entities.is_empty() {
        if let Some(ref handle) = queue.active_gltf {
            if let Some(gltf) = gltf_assets.get(handle) {
                let mut spawned = Vec::new();
                for mesh_handle in &gltf.meshes {
                    if let Some(gltf_mesh) = gltf_meshes.get(mesh_handle) {
                        for prim in &gltf_mesh.primitives {
                            let std_mat = if let Some(ref h) = prim.material {
                                if let Some(gm) = gltf_materials.get(h) {
                                    StandardMaterial {
                                        base_color: gm.base_color,
                                        base_color_texture: gm.base_color_texture.clone(),
                                        perceptual_roughness: gm.perceptual_roughness,
                                        metallic: gm.metallic,
                                        ..default()
                                    }
                                } else {
                                    StandardMaterial {
                                        base_color: Color::srgb(0.7, 0.7, 0.7),
                                        metallic: 0.1,
                                        perceptual_roughness: 0.8,
                                        ..default()
                                    }
                                }
                            } else {
                                StandardMaterial {
                                    base_color: Color::srgb(0.7, 0.7, 0.7),
                                    metallic: 0.1,
                                    perceptual_roughness: 0.8,
                                    ..default()
                                }
                            };
                            let mat_handle = standard_materials.add(std_mat);

                            let e = commands
                                .spawn((
                                    Mesh3d(prim.mesh.clone()),
                                    MeshMaterial3d(mat_handle),
                                    Transform::from_xyz(0.0, 0.0, 0.0),
                                ))
                                .id();
                            spawned.push(e);
                        }
                    }
                }

                if !spawned.is_empty() {
                    println!("  Spawned {} primitives for rendering", spawned.len());
                    queue.spawned_entities = spawned;
                    queue.frame_in_step = 2;
                }
            }
        }
    }

    // Step 2: Render warm-up and screenshot capture
    if !queue.spawned_entities.is_empty() && queue.frame_in_step == 20 {
        let (_, _, out_png, _) = queue.models[queue.current_idx].clone();
        println!("  Capturing screenshot to {:?}", out_png);
        if let Some(parent) = out_png.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(out_png.to_string_lossy().into_owned()));
    }

    // Step 3: Despawn and advance
    if !queue.spawned_entities.is_empty() && queue.frame_in_step >= 30 {
        for e in queue.spawned_entities.drain(..) {
            commands.entity(e).despawn();
        }
        queue.active_gltf = None;
        queue.current_idx += 1;
        queue.frame_in_step = 0;
    }
}

fn main() -> ExitCode {
    let candidate_models = [
        // 1. Tree
        ("plants_tree01", "ha_area00_terrain/plants_tree01.gltf", 8.0),
        // 2. Tree (area02)
        ("plants_area02_tree01", "ha_area00_terrain/plants_area02_tree01.gltf", 9.0),
        // 3. Shrub
        ("plants_shrub02", "ha_area00_terrain/plants_shrub02.gltf", 4.0),
        // 4. Grass
        ("plants_area00_grass01_alb", "ha_area00_terrain/plants_area00_grass01_alb.gltf", 3.0),
        // 5. Flower
        ("plants_flower04", "ha_area00_terrain/plants_flower04.gltf", 3.0),
        // 6. Rock 1
        ("ground_rock03_gray", "ha_area00/ground_rock03_gray.gltf", 6.0),
        // 7. Rock 2
        ("ground_rock10_gray", "ha_area00/ground_rock10_gray.gltf", 7.0),
        // 8. Building (house)
        ("objects_area00_house02", "ha_area00/objects_area00_house02.gltf", 12.0),
        // 9. Building (const)
        ("objects_area00_const01", "ha_area00/objects_area00_const01.gltf", 8.0),
        // 10. Prop (fallen tree / log)
        ("ground_area00_fallentree01", "ha_area00/ground_area00_fallentree01.gltf", 7.0),
    ];

    let gltf_field_root = PathBuf::from("work/cache/gltf_field");
    let out_root = PathBuf::from("work/cache/gltf_field_verify");
    let _ = std::fs::create_dir_all(&out_root);

    let mut models = Vec::new();
    for (name, rel_path, dist) in candidate_models {
        let gltf_path = gltf_field_root.join(rel_path);
        if gltf_path.exists() {
            let out_png = out_root.join(format!("{}.png", name));
            models.push((name.to_string(), PathBuf::from(rel_path), out_png, dist));
        } else {
            eprintln!("Warning: candidate model not found: {:?}", gltf_path);
        }
    }

    if models.len() != 10 {
        eprintln!("Expected 10 verified models, got {}", models.len());
        return ExitCode::FAILURE;
    }

    let work_root = PathBuf::from("work");
    let work_abs = work_root.canonicalize().expect("canonicalize work_root");
    println!("Asset root: {}", work_abs.display());
    println!("Starting headless glTF verification for {} models...", models.len());

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "PLA port — Field glTF headless verifier".into(),
                        resolution: WindowResolution::new(512, 512),
                        visible: false, // OFFSCREEN / NO WINDOW
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: work_abs.to_string_lossy().to_string(),
                    unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                    ..default()
                }),
        )
        .insert_resource(VerifyQueue {
            models,
            current_idx: 0,
            frame_in_step: 0,
            active_gltf: None,
            spawned_entities: Vec::new(),
        })
        .insert_resource(ClearColor(Color::srgb(0.08, 0.08, 0.12)))
        .add_systems(Startup, setup)
        .add_systems(Update, process_queue)
        .run();

    ExitCode::SUCCESS
}
