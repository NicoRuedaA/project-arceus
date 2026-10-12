//! Windowed TR skeletal-animation prototype.
//!
//! Usage: `cargo run -p pla --example anim_spike -- <model.trskl> <model.trmbf>
//! <model.trmsh> <clip.tranm> [work/anim-sequence]`
//!
//! The example is deliberately strict: it rejects unknown mesh layouts,
//! unresolved skin joints, and bind matrices that disagree with the currently
//! validated reference rest-pose hypothesis. Frames are written as PNGs.

use bevy::animation::animated_field;
use bevy::animation::prelude::{AnimatedField, AnimationClip};
use bevy::animation::AnimationTargetId;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use pla::assets::tr::{
    animation_target_id, bevy_animation_clip, insert_skin_attributes, BevyTrSkeleton, TrAnm, TrMbf,
    TrSkl,
};
use std::path::PathBuf;

#[derive(Component)]
struct JointNode;

struct TrackTarget {
    entity: Entity,
    target: AnimationTargetId,
}

#[derive(Resource)]
struct Playback {
    clip: AnimationClip,
    frame_rate: u32,
    frame_count: u32,
    tracks: Vec<TrackTarget>,
    frame: u32,
    output_dir: PathBuf,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut inverse_bindposes_assets: ResMut<Assets<SkinnedMeshInverseBindposes>>,
) {
    let mut args = std::env::args().skip(1);
    let skeleton_path = PathBuf::from(args.next().expect("missing .trskl path"));
    let mesh_path = PathBuf::from(args.next().expect("missing .trmbf path"));
    let layout_path = PathBuf::from(args.next().expect("missing .trmsh path"));
    let animation_path = PathBuf::from(args.next().expect("missing .tranm path"));
    let output_dir = PathBuf::from(args.next().unwrap_or_else(|| "work/anim-sequence".into()));
    std::fs::create_dir_all(&output_dir).expect("create screenshot directory");

    let skeleton_bytes = std::fs::read(&skeleton_path).expect("read .trskl");
    let bind_records = TrSkl::read_bind_records(&skeleton_bytes).expect("decode bind matrices");
    let skeleton = TrSkl::parse(&skeleton_bytes).expect("decode skeleton hierarchy");
    let bevy_skeleton = BevyTrSkeleton::from_tr(&skeleton, &bind_records, 1e-4)
        .expect("bind matrices must validate against the reference rest pose");
    let mesh_bytes = std::fs::read(&mesh_path).expect("read .trmbf");
    let layout_bytes = std::fs::read(&layout_path).expect("read .trmsh");
    let mut mesh_handles = Vec::new();
    let mut all_positions = Vec::new();
    for shape_index in 0..64 {
        let mesh_data = match TrMbf::parse_skin_shape(&mesh_bytes, &layout_bytes, shape_index) {
            Ok(mesh_data) => mesh_data,
            Err(error) if shape_index == 0 => TrMbf::parse_with_layout(&mesh_bytes, &layout_bytes)
                .unwrap_or_else(|fallback| {
                    panic!("decode a declared skinned mesh shape: {error}; {fallback}")
                }),
            Err(error) if error == "skin shape index out of range" => break,
            Err(error) => panic!("decode skinned mesh shape {shape_index}: {error}"),
        };
        let positions = (0..mesh_data.vertex_count)
            .map(|i| {
                mesh_data
                    .position(&mesh_bytes, i)
                    .ok_or("missing declared position")
            })
            .collect::<Result<Vec<_>, _>>()
            .expect("read positions");
        let uvs = (0..mesh_data.vertex_count)
            .map(|i| mesh_data.uv(&mesh_bytes, i).unwrap_or([0.0, 0.0]))
            .collect::<Vec<_>>();
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_indices(Indices::U32(
            mesh_data.indices.iter().map(|&i| u32::from(i)).collect(),
        ));
        mesh.compute_normals();
        insert_skin_attributes(&mut mesh, &mesh_data, &mesh_bytes, &skeleton)
            .expect("decode demonstrated skin influences");
        mesh_handles.push(meshes.add(mesh));
        all_positions.extend(positions);
    }
    assert!(!mesh_handles.is_empty(), "model has no supported mesh shapes");

    let mut node_entities = Vec::with_capacity(skeleton.nodes.len());
    for (node, transform) in skeleton.nodes.iter().zip(&bevy_skeleton.local_transforms) {
        node_entities.push(
            commands
                .spawn((
                    Name::new(node.name.clone()),
                    *transform,
                    GlobalTransform::default(),
                    JointNode,
                ))
                .id(),
        );
    }
    for (index, parent) in bevy_skeleton.parents.iter().enumerate() {
        if let Some(parent) = parent {
            commands
                .entity(node_entities[*parent])
                .add_child(node_entities[index]);
        }
    }
    let joints = bevy_skeleton
        .rig_to_node
        .iter()
        .map(|&node| node_entities[node])
        .collect::<Vec<_>>();
    let inverse_bindposes = inverse_bindposes_assets.add(bevy_skeleton.inverse_bindposes_asset());
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.78, 0.92),
        perceptual_roughness: 0.8,
        ..default()
    });
    for mesh_handle in mesh_handles {
        commands.spawn((
            Mesh3d(mesh_handle),
            MeshMaterial3d(material.clone()),
            SkinnedMesh {
                inverse_bindposes: inverse_bindposes.clone(),
                joints: joints.clone(),
            },
        ));
    }

    let animation_bytes = std::fs::read(&animation_path).expect("read .tranm");
    let clip_data = TrAnm::parse_tracks(&animation_bytes).expect("decode skeletal animation");
    let clip = bevy_animation_clip(&skeleton, &clip_data).expect("build Bevy animation clip");
    let mapped_nodes = clip_data
        .skeleton_nodes(&skeleton)
        .expect("map named tracks to joints");
    let tracks = clip_data
        .tracks
        .iter()
        .zip(mapped_nodes)
        .map(|(_, node)| {
            Ok(TrackTarget {
                entity: *node_entities.get(node).ok_or("track node out of range")?,
                target: animation_target_id(&skeleton, node)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()
        .expect("prepare animation targets");
    println!(
        "TR animation: {} tracks, {} frames at {} fps, bind-rest validated; saving frames to {}",
        tracks.len(),
        clip_data.frame_count,
        clip_data.frame_rate,
        output_dir.display()
    );
    commands.insert_resource(Playback {
        clip,
        frame_rate: clip_data.frame_rate,
        frame_count: clip_data.frame_count,
        tracks,
        frame: 0,
        output_dir,
    });

    let (minimum, maximum) = bounds(&all_positions);
    let centre = (minimum + maximum) * 0.5;
    let radius = all_positions
        .iter()
        .map(|p| Vec3::from_array(*p).distance(centre))
        .fold(0.0f32, f32::max)
        .max(0.1);
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(centre + Vec3::new(1.0, 0.7, 1.2).normalize() * radius * 3.0)
            .looking_at(centre, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(2.0, 4.0, 3.0).looking_at(centre, Vec3::Y),
    ));
}

fn bounds(points: &[[f32; 3]]) -> (Vec3, Vec3) {
    points.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(min, max), p| {
            let p = Vec3::from_array(*p);
            (min.min(p), max.max(p))
        },
    )
}

fn animate_and_capture(
    mut commands: Commands,
    mut playback: ResMut<Playback>,
    mut joints: Query<&mut Transform, With<JointNode>>,
    mut exit: MessageWriter<AppExit>,
) {
    if playback.frame >= playback.frame_count {
        if playback.frame == playback.frame_count + 2 {
            exit.write(AppExit::Success);
        }
        playback.frame += 1;
        return;
    }
    let time = playback.frame as f32 / playback.frame_rate as f32;
    for target in &playback.tracks {
        if let Ok(mut transform) = joints.get_mut(target.entity) {
            if let Some(value) = playback.clip.sample_clamped(
                animated_field!(Transform::translation),
                target.target,
                time,
            ) {
                transform.translation = value;
            }
            if let Some(value) = playback.clip.sample_clamped(
                animated_field!(Transform::rotation),
                target.target,
                time,
            ) {
                transform.rotation = value;
            }
            if let Some(value) =
                playback
                    .clip
                    .sample_clamped(animated_field!(Transform::scale), target.target, time)
            {
                transform.scale = value;
            }
        }
    }
    let path = playback
        .output_dir
        .join(format!("frame-{:04}.png", playback.frame));
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
    playback.frame += 1;
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "PLA TR animation spike".into(),
                resolution: (960, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.08)))
        .add_systems(Startup, setup)
        .add_systems(Update, animate_and_capture)
        .run();
}
