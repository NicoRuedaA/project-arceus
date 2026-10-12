//! Headless offscreen glTF verification using Bevy's glTF loader.
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
    models: Vec<(String, PathBuf, PathBuf)>, // (label, gltf_rel_path, out_png)
    current_idx: usize,
    frame_in_step: u32,
    active_gltf: Option<Handle<Gltf>>,
    spawned_entities: Vec<Entity>,
}

fn setup(mut commands: Commands) {
    // 3D camera pointing at model origin
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.4, 1.2).looking_at(Vec3::new(0.0, 0.05, 0.0), Vec3::Y),
    ));

    // Directional light 1
    commands.spawn((
        DirectionalLight {
            illuminance: 12000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(3.0, 6.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Directional fill light 2
    commands.spawn((
        DirectionalLight {
            illuminance: 6000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-3.0, 2.0, -2.0).looking_at(Vec3::ZERO, Vec3::Y),
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
    mut exit: MessageWriter<AppExit>,
) {
    queue.frame_in_step += 1;

    // Step 0: Request load of current model
    if queue.frame_in_step == 1 {
        if queue.current_idx >= queue.models.len() {
            println!("All {} models verified successfully.", queue.models.len());
            exit.write(AppExit::Success);
            return;
        }

        let (label, gltf_rel, _) = queue.models[queue.current_idx].clone();
        println!(
            "[{}/{}] Loading glTF asset '{}' ({:?})",
            queue.current_idx + 1,
            queue.models.len(),
            label,
            gltf_rel
        );

        let asset_path = format!("gltf/{}", gltf_rel.to_string_lossy());
        let handle: Handle<Gltf> = asset_server.load(asset_path);
        queue.active_gltf = Some(handle);
    }

    // Step 1: Wait for Gltf asset to load, then spawn primitives
    if queue.spawned_entities.is_empty() {
        if let Some(ref handle) = queue.active_gltf {
            if let Some(gltf) = gltf_assets.get(handle) {
                let default_mat = standard_materials.add(StandardMaterial {
                    base_color: Color::srgb(0.7, 0.7, 0.7),
                    metallic: 0.1,
                    perceptual_roughness: 0.8,
                    ..default()
                });

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
                                        normal_map_texture: gm.normal_map_texture.clone(),
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
                    // Reset frame counter to allow render warm-up
                    queue.frame_in_step = 2;
                }
            }
        }
    }

    // Step 2: Render warm-up and screenshot capture
    if !queue.spawned_entities.is_empty() && queue.frame_in_step == 25 {
        let (_, _, out_png) = queue.models[queue.current_idx].clone();
        println!("  Capturing screenshot to {:?}", out_png);
        if let Some(parent) = out_png.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(out_png.to_string_lossy().into_owned()));
    }

    // Step 3: Despawn and advance to next model
    if !queue.spawned_entities.is_empty() && queue.frame_in_step >= 35 {
        for e in queue.spawned_entities.drain(..) {
            commands.entity(e).despawn();
        }
        queue.active_gltf = None;
        queue.current_idx += 1;
        queue.frame_in_step = 0;
    }
}

fn main() -> ExitCode {
    let mut models = Vec::new();
    // Default 10 verified static models
    let candidate_models = [
        ("item_003", "chara/data/item/item_003/item_003.gltf"),
        ("item_004", "chara/data/item/item_004/item_004.gltf"),
        ("item_005", "chara/data/item/item_005/item_005.gltf"),
        ("item_006", "chara/data/item/item_006/item_006.gltf"),
        ("item_007", "chara/data/item/item_007/item_007.gltf"),
        ("item_008", "chara/data/item/item_008/item_008.gltf"),
        ("item_009", "chara/data/item/item_009/item_009.gltf"),
        ("item_010", "chara/data/item/item_010/item_010.gltf"),
        ("item_012", "chara/data/item/item_012/item_012.gltf"),
        ("item_015", "chara/data/item/item_015/item_015.gltf"),
    ];

    let gltf_root = PathBuf::from("work/cache/gltf");
    let out_root = PathBuf::from("work/cache/gltf_verify");
    let _ = std::fs::create_dir_all(&out_root);

    for (name, rel_path) in candidate_models {
        let gltf_path = gltf_root.join(rel_path);
        if gltf_path.exists() {
            let out_png = out_root.join(format!("{}.png", name));
            models.push((name.to_string(), PathBuf::from(rel_path), out_png));
        }
    }

    if models.is_empty() {
        eprintln!("No candidate models found in {:?}", gltf_root);
        return ExitCode::FAILURE;
    }

    let cache_root = PathBuf::from("work/cache");
    let cache_abs = cache_root.canonicalize().expect("canonicalize cache_root");
    println!("Asset root: {}", cache_abs.display());
    println!("Starting headless glTF verification for {} models...", models.len());

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "PLA port — glTF headless verifier".into(),
                        resolution: WindowResolution::new(512, 512),
                        visible: false, // INVISIBLE WINDOW: offscreen / no window shown
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: cache_abs.to_string_lossy().to_string(),
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
