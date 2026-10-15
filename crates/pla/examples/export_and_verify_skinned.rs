use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pla::assets::gltf_export::{
    animation_tracks_from_tr, primitive_from_tr, skeleton_from_tr, GltfExport, GltfMaterial,
};
use pla::assets::tr::{TrAnm, TrMbf, TrMtr, TrSkl};

use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::WindowResolution;

fn export_skinned_item_201() -> Result<PathBuf, String> {
    let base_bin = Path::new("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin");
    let model_dir = base_bin.join("chara/data/item/item_201");
    let anm_path = base_bin.join("demo/graphic/ev/d020/item_224/anm/d020_item_224_c002.tranm");

    let mbf_bytes = std::fs::read(model_dir.join("item_201.trmbf")).map_err(|e| e.to_string())?;
    let msh_bytes = std::fs::read(model_dir.join("item_201.trmsh")).map_err(|e| e.to_string())?;
    let skl_bytes = std::fs::read(model_dir.join("item_201.trskl")).map_err(|e| e.to_string())?;
    let mtr_bytes = std::fs::read(model_dir.join("item_201.trmtr")).map_err(|e| e.to_string())?;
    let anm_bytes = std::fs::read(&anm_path).map_err(|e| e.to_string())?;

    let skl = TrSkl::parse(&skl_bytes)?;
    let binds = TrSkl::read_bind_records(&skl_bytes)?;
    let (nodes, skin) = skeleton_from_tr(&skl, &binds, 0, 0)?;

    let mbf = TrMbf::parse_with_layout(&mbf_bytes, &msh_bytes)?;

    let mut materials = Vec::new();
    let mut material_idx = None;
    if let Ok(mtr) = TrMtr::parse(&mtr_bytes) {
        if let Some(base_col) = mtr.base_color() {
            // Relative URI from work/cache/gltf/chara/data/item/item_201 to work/cache/png/chara/data/item/item_201
            let rel_uri = format!("../../../../../png/chara/data/item/item_201/{}.png", base_col.trim_end_matches(".bntx"));
            materials.push(GltfMaterial {
                name: "item_201_material".into(),
                base_color_uri: Some(rel_uri),
                normal_uri: None,
                base_color_factor: None,
            });
            material_idx = Some(0);
        }
    }

    let primitive = primitive_from_tr("item_201_mesh", &mbf, &mbf_bytes, Some(&skl), material_idx)?;
    let clip = TrAnm::parse_tracks(&anm_bytes)?;
    let tracks = animation_tracks_from_tr(&skl, &clip)?;

    let export = GltfExport {
        primitives: vec![primitive],
        materials,
        nodes,
        skins: vec![skin],
        animations: vec![("d020_c002".into(), tracks)],
    };

    let out_dir = Path::new("work/cache/gltf/chara/data/item/item_201");
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let out_gltf = out_dir.join("item_201.gltf");
    export.write(&out_gltf)?;
    println!("Exported skinned glTF to: {}", out_gltf.display());
    Ok(out_gltf)
}

#[derive(Resource)]
struct SkinnedVerifyState {
    gltf_handle: Option<Handle<Gltf>>,
    spawned_scene: bool,
    frame_counter: u32,
    captures_taken: u32,
}

fn setup(mut commands: Commands) {
    // Camera framing the item (approx size 0.2m)
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.15, 0.35).looking_at(Vec3::new(0.0, 0.05, 0.0), Vec3::Y),
    ));

    // Directional light
    commands.spawn((
        DirectionalLight {
            illuminance: 12000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(1.0, 2.0, 1.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 6000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-1.0, 1.0, -1.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn process_skinned_verify(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<bevy::gltf::GltfMesh>>,
    gltf_materials: Res<Assets<bevy::gltf::GltfMaterial>>,
    gltf_nodes: Res<Assets<bevy::gltf::GltfNode>>,
    gltf_skins: Res<Assets<bevy::gltf::GltfSkin>>,
    anim_clips: Res<Assets<AnimationClip>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<SkinnedVerifyState>,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame_counter += 1;

    if state.gltf_handle.is_none() {
        let handle: Handle<Gltf> = asset_server.load("gltf/chara/data/item/item_201/item_201.gltf");
        state.gltf_handle = Some(handle);
        println!("Requested asset load for item_201.gltf");
        return;
    }

    let handle = state.gltf_handle.as_ref().unwrap();
    if !state.spawned_scene {
        if let Some(gltf) = gltf_assets.get(handle) {
            println!("Gltf loaded successfully!");
            println!("  Meshes: {}", gltf.meshes.len());
            println!("  Skins: {}", gltf.skins.len());
            println!("  Animations: {}", gltf.animations.len());

            let mut joint_entities = Vec::new();
            if let Some(skin_handle) = gltf.skins.first() {
                if let Some(skin) = gltf_skins.get(skin_handle) {
                    println!("    Skin '{}': {} joints", skin.name, skin.joints.len());
                    for joint_handle in &skin.joints {
                        let tf = if let Some(node) = gltf_nodes.get(joint_handle) {
                            node.transform
                        } else {
                            Transform::IDENTITY
                        };
                        let entity = commands.spawn((
                            tf,
                            Visibility::Inherited,
                        )).id();
                        joint_entities.push(entity);
                    }
                }
            }
            for anim_handle in &gltf.animations {
                if let Some(clip) = anim_clips.get(anim_handle) {
                    println!("    Animation clip duration: {:.3}s, curves: {}", clip.duration(), clip.curves().len());
                }
            }

            let default_mat = standard_materials.add(StandardMaterial {
                base_color: Color::srgb(0.8, 0.2, 0.2), // Red ball top
                metallic: 0.1,
                perceptual_roughness: 0.5,
                ..default()
            });

            for mesh_handle in &gltf.meshes {
                if let Some(gltf_mesh) = gltf_meshes.get(mesh_handle) {
                    for prim in &gltf_mesh.primitives {
                        let std_mat = if let Some(ref h) = prim.material {
                            if let Some(mat) = gltf_materials.get(h) {
                                standard_materials.add(StandardMaterial {
                                    base_color: mat.base_color,
                                    base_color_texture: mat.base_color_texture.clone(),
                                    metallic: mat.metallic,
                                    perceptual_roughness: mat.perceptual_roughness,
                                    normal_map_texture: mat.normal_map_texture.clone(),
                                    ..default()
                                })
                            } else {
                                default_mat.clone()
                            }
                        } else {
                            default_mat.clone()
                        };

                        let mut cmd = commands.spawn((
                            Mesh3d(prim.mesh.clone()),
                            MeshMaterial3d(std_mat),
                            Transform::from_xyz(0.0, 0.0, 0.0),
                        ));

                        if let Some(skin_handle) = gltf.skins.first() {
                            if let Some(skin) = gltf_skins.get(skin_handle) {
                                cmd.insert(bevy::mesh::skinning::SkinnedMesh {
                                    inverse_bindposes: skin.inverse_bind_matrices.clone(),
                                    joints: joint_entities.clone(),
                                });
                            }
                        }
                    }
                }
            }
            state.spawned_scene = true;
            println!("Spawned skinned mesh entity for item_201");
        }
        return;
    }

    // Wait 5 frames after spawning to take frame capture
    if state.captures_taken == 0 && state.frame_counter >= 10 {
        let out_dir = Path::new("work/cache/gltf_verify");
        let _ = std::fs::create_dir_all(out_dir);
        let out_path = out_dir.join("skinned_item_201.png");
        println!("Capturing screenshot: {}", out_path.display());
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(out_path));
        state.captures_taken += 1;
    }

    if state.captures_taken >= 1 && state.frame_counter >= 15 {
        println!("Skinned model verification render complete.");
        exit.write(AppExit::Success);
    }
}

fn main() -> ExitCode {
    if let Err(e) = export_skinned_item_201() {
        eprintln!("Failed to export skinned item_201: {e}");
        return ExitCode::FAILURE;
    }

    let work_cache = std::fs::canonicalize("work/cache").unwrap_or_else(|_| PathBuf::from("work/cache"));

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: work_cache.to_string_lossy().to_string(),
                    unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Headless Skinned glTF Verification".into(),
                        resolution: WindowResolution::new(512, 512),
                        visible: false,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .insert_resource(SkinnedVerifyState {
            gltf_handle: None,
            spawned_scene: false,
            frame_counter: 0,
            captures_taken: 0,
        })
        .register_type::<Transform>()
        .register_type::<GlobalTransform>()
        .add_systems(Startup, setup)
        .add_systems(Update, process_skinned_verify)
        .run();

    ExitCode::SUCCESS
}
