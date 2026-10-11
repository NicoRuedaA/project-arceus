//! Bevy adapters for the demonstrated TR skeleton, skin, and animation lanes.
//!
//! Animation decoding is evidence-backed; interpolation and packed-rotation
//! playback here use Bevy's reference-oriented sampling policy, not a claim of
//! native-game playback parity.

use bevy::animation::animated_field;
use bevy::animation::prelude::AnimatedField;
use bevy::animation::prelude::{AnimatableCurve, AnimatableKeyframeCurve, AnimationClip};
use bevy::animation::AnimationTargetId;
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::mesh::{Mesh, VertexAttributeValues};
use bevy::prelude::{Mat4, Name, Quat, Transform, Vec3};

use super::{
    reference_unpack_rotation, TrAnmChannel, TrAnmClip, TrMbf, TrSkl, TrSklBindRecord, TrSklMatrix,
};

/// Checked skeleton data in Bevy's node and bind-record order.
#[derive(Debug, Clone)]
pub struct BevyTrSkeleton {
    /// Local rest transforms in TR transform-node order.
    pub local_transforms: Vec<Transform>,
    /// Parent transform-node indices, suitable for constructing the hierarchy.
    pub parents: Vec<Option<usize>>,
    /// Transform-node index for each bind/rig joint.
    pub rig_to_node: Vec<usize>,
    /// Validated inverse bind poses in rig order.
    pub inverse_bindposes: Vec<Mat4>,
}

impl BevyTrSkeleton {
    /// Convert a skeleton only when its stored bind matrices match the validated
    /// Blender-style reference rest pose within `epsilon`.
    pub fn from_tr(
        skeleton: &TrSkl,
        bind_records: &[TrSklBindRecord],
        epsilon: f64,
    ) -> Result<Self, String> {
        skeleton.validate_bind_rest_pose(bind_records, epsilon)?;
        let mut local_transforms = Vec::with_capacity(skeleton.nodes.len());
        let mut parents = Vec::with_capacity(skeleton.nodes.len());
        for node in &skeleton.nodes {
            if node
                .scale_pivot
                .iter()
                .chain(node.rotate_pivot.iter())
                .any(|v| *v != 0.0)
            {
                return Err("Bevy skeleton adapter does not support nonzero pivots".into());
            }
            let srt = &node.local;
            local_transforms.push(Transform {
                translation: Vec3::from_array(srt.translation),
                rotation: Quat::from_euler(
                    bevy::math::EulerRot::XYZ,
                    srt.rotation[0],
                    srt.rotation[1],
                    srt.rotation[2],
                ),
                scale: Vec3::from_array(srt.scale),
            });
            parents.push(node.parent);
        }
        let mut rig_to_node = vec![usize::MAX; bind_records.len()];
        for (node_index, node) in skeleton.nodes.iter().enumerate() {
            if let Some(rig_index) = node.rig_index {
                let slot = rig_to_node
                    .get_mut(rig_index)
                    .ok_or("skeleton rig index outside bind records")?;
                if *slot != usize::MAX {
                    return Err("duplicate rig-to-node mapping".into());
                }
                *slot = node_index;
            }
        }
        if rig_to_node.iter().any(|&node| node == usize::MAX) {
            return Err("unresolved rig-to-node mapping".into());
        }
        let inverse_bindposes = bind_records
            .iter()
            .map(|record| matrix_to_bevy(&record.matrix))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            local_transforms,
            parents,
            rig_to_node,
            inverse_bindposes,
        })
    }

    pub fn inverse_bindposes_asset(&self) -> SkinnedMeshInverseBindposes {
        self.inverse_bindposes.clone().into()
    }
}

/// Read validated skin lanes and insert Bevy's JOINT_INDEX/JOINT_WEIGHT data.
/// Joint indices refer to the rig-ordered `SkinnedMesh::joints` vector.
pub fn insert_skin_attributes(
    mesh_asset: &mut Mesh,
    mesh: &TrMbf,
    bytes: &[u8],
    skeleton: &TrSkl,
) -> Result<(), String> {
    let mut joint_indices = Vec::<[u16; 4]>::with_capacity(mesh.vertex_count);
    let mut joint_weights = Vec::<[f32; 4]>::with_capacity(mesh.vertex_count);
    for vertex in 0..mesh.vertex_count {
        let influences = mesh.skin_influences(bytes, vertex, skeleton)?;
        let mut indices = [0u16; 4];
        for lane in 0..4 {
            if let Some(node_index) = influences.node_indices[lane] {
                let rig_index = skeleton
                    .nodes
                    .get(node_index)
                    .and_then(|node| node.rig_index)
                    .ok_or("active skin lane has no rig index")?;
                indices[lane] = u16::try_from(rig_index)
                    .map_err(|_| "rig index exceeds Bevy's Uint16x4 joint attribute")?;
            }
        }
        joint_indices.push(indices);
        joint_weights.push(influences.unorm_weights());
    }
    mesh_asset.insert_attribute(
        Mesh::ATTRIBUTE_JOINT_INDEX,
        VertexAttributeValues::Uint16x4(joint_indices),
    );
    mesh_asset.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, joint_weights);
    Ok(())
}

/// Build a Bevy clip with translation, rotation, and scale curves at the TR
/// clip's frame rate. Duplicate frame records use the first occurrence, as in
/// the independent importer reference. Quaternion decoding follows that
/// reference; Bevy curve interpolation is an explicit port policy.
pub fn animation_clip(skeleton: &TrSkl, clip: &TrAnmClip) -> Result<AnimationClip, String> {
    if clip.frame_rate == 0 || clip.frame_count == 0 {
        return Err("animation clip has invalid timing".into());
    }
    let nodes = clip.skeleton_nodes(skeleton)?;
    let mut result = AnimationClip::default();
    let duration = clip.frame_count as f32 / clip.frame_rate as f32;
    for (track, &node_index) in clip.tracks.iter().zip(&nodes) {
        let target = animation_target_id(skeleton, node_index)?;
        let seconds = |frame: u32| frame as f32 / clip.frame_rate as f32;
        let translations = vector_samples(
            &track.translation,
            clip.frame_count,
            Vec3::from_array,
            seconds,
            clip.frame_count as f32 / clip.frame_rate as f32,
        )?;
        let scales = vector_samples(
            &track.scale,
            clip.frame_count,
            Vec3::from_array,
            seconds,
            clip.frame_count as f32 / clip.frame_rate as f32,
        )?;
        let rotations = rotation_samples(
            &track.rotation,
            clip.frame_count,
            seconds,
            clip.frame_count as f32 / clip.frame_rate as f32,
        )?;
        if !translations.is_empty() {
            result.add_curve_to_target(
                target,
                AnimatableCurve::new(
                    animated_field!(Transform::translation),
                    AnimatableKeyframeCurve::new(translations)
                        .map_err(|e| format!("translation curve: {e:?}"))?,
                ),
            );
        }
        if !rotations.is_empty() {
            result.add_curve_to_target(
                target,
                AnimatableCurve::new(
                    animated_field!(Transform::rotation),
                    AnimatableKeyframeCurve::new(rotations)
                        .map_err(|e| format!("rotation curve: {e:?}"))?,
                ),
            );
        }
        if !scales.is_empty() {
            result.add_curve_to_target(
                target,
                AnimatableCurve::new(
                    animated_field!(Transform::scale),
                    AnimatableKeyframeCurve::new(scales)
                        .map_err(|e| format!("scale curve: {e:?}"))?,
                ),
            );
        }
    }
    result.set_duration(duration);
    Ok(result)
}

fn vector_samples<T: Copy>(
    channel: &TrAnmChannel<[f32; 3]>,
    frame_count: u32,
    map: impl Fn([f32; 3]) -> T,
    seconds: impl Fn(u32) -> f32,
    duration: f32,
) -> Result<Vec<(f32, T)>, String> {
    channel.reference_records(frame_count)?;
    let mut samples = channel
        .keys
        .iter()
        .fold(Vec::new(), |mut out, key| {
            if out
                .last()
                .is_none_or(|(frame, _): &(u32, T)| *frame != key.frame)
            {
                out.push((key.frame, map(key.value)));
            }
            out
        })
        .into_iter()
        .map(|(frame, value)| (seconds(frame), value))
        .collect::<Vec<_>>();
    if channel.encoding == super::TrAnmEncoding::Fixed && samples.len() == 1 {
        samples.push((duration, samples[0].1));
    } else if samples.len() == 1 {
        return Err("single-sample non-fixed vector channel is unsupported".into());
    }
    Ok(samples)
}

fn rotation_samples(
    channel: &TrAnmChannel<[u16; 3]>,
    frame_count: u32,
    seconds: impl Fn(u32) -> f32,
    duration: f32,
) -> Result<Vec<(f32, Quat)>, String> {
    channel.reference_records(frame_count)?;
    let mut samples = channel
        .keys
        .iter()
        .fold(Vec::new(), |mut out, key| {
            if out
                .last()
                .is_none_or(|(frame, _): &(u32, Quat)| *frame != key.frame)
            {
                out.push((
                    key.frame,
                    Quat::from_array(reference_unpack_rotation(key.value)),
                ));
            }
            out
        })
        .into_iter()
        .map(|(frame, value)| (seconds(frame), value))
        .collect::<Vec<_>>();
    if channel.encoding == super::TrAnmEncoding::Fixed && samples.len() == 1 {
        samples.push((duration, samples[0].1));
    } else if samples.len() == 1 {
        return Err("single-sample non-fixed rotation channel is unsupported".into());
    }
    Ok(samples)
}

pub fn animation_target_id(
    skeleton: &TrSkl,
    node_index: usize,
) -> Result<AnimationTargetId, String> {
    let mut chain = Vec::new();
    let mut cursor = Some(node_index);
    while let Some(index) = cursor {
        let node = skeleton
            .nodes
            .get(index)
            .ok_or("animation target node out of range")?;
        chain.push(Name::new(node.name.clone()));
        cursor = node.parent;
    }
    chain.reverse();
    Ok(AnimationTargetId::from_names(chain.iter()))
}

fn matrix_to_bevy(matrix: &TrSklMatrix) -> Result<Mat4, String> {
    let mut columns = [0.0f32; 16];
    for row in 0..4 {
        for column in 0..4 {
            let value = matrix.rows[row][column];
            if !value.is_finite() || value.abs() > f32::MAX as f64 {
                return Err("bind matrix cannot be represented by Bevy Mat4".into());
            }
            columns[column * 4 + row] = value as f32;
        }
    }
    Ok(Mat4::from_cols_array(&columns))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::tr::{TrAnmBoneTrack, TrAnmEncoding, TrAnmKey, TrSklNode, TrSklTransform};

    fn skeleton() -> TrSkl {
        TrSkl {
            bones: vec!["root".into()],
            offsets: vec![0],
            nodes: vec![TrSklNode {
                name: "root".into(),
                local: TrSklTransform {
                    scale: [1.0; 3],
                    rotation: [0.0; 3],
                    translation: [0.0; 3],
                },
                scale_pivot: [0.0; 3],
                rotate_pivot: [0.0; 3],
                parent: None,
                rig_index: Some(0),
            }],
            bind_count: 1,
            root_flag: 0,
        }
    }

    fn channel<T>(encoding: TrAnmEncoding, values: &[(u32, T)]) -> TrAnmChannel<T>
    where
        T: Clone,
    {
        TrAnmChannel {
            encoding,
            keys: values
                .iter()
                .map(|(frame, value)| TrAnmKey {
                    frame: *frame,
                    value: value.clone(),
                })
                .collect(),
        }
    }

    #[test]
    fn converts_verified_rest_bind_and_preserves_reference_key_samples() {
        let skeleton = skeleton();
        let bind = [TrSklBindRecord {
            flags: [1, 1],
            matrix: TrSklMatrix::IDENTITY,
        }];
        let bevy_skeleton = BevyTrSkeleton::from_tr(&skeleton, &bind, 1e-6).unwrap();
        assert_eq!(bevy_skeleton.rig_to_node, [0]);
        assert_eq!(bevy_skeleton.inverse_bindposes, [Mat4::IDENTITY]);

        let translation = [[0.0, 0.0, 0.0], [2.0, 1.0, -1.0]];
        let rotation = [[0u16; 3], [1, 2, 3]];
        let clip = TrAnmClip {
            loops: false,
            frame_count: 3,
            frame_rate: 30,
            tracks: vec![TrAnmBoneTrack {
                name: "root".into(),
                scale: channel(TrAnmEncoding::Fixed, &[(0, [1.0; 3])]),
                rotation: channel(
                    TrAnmEncoding::Framed16,
                    &[(0, rotation[0]), (2, rotation[1])],
                ),
                translation: channel(
                    TrAnmEncoding::Framed16,
                    &[(0, translation[0]), (2, translation[1])],
                ),
            }],
        };
        let bevy_clip = animation_clip(&skeleton, &clip).unwrap();
        let target = animation_target_id(&skeleton, 0).unwrap();
        for time in [
            0.0,
            1.0 / clip.frame_rate as f32,
            clip.frame_count as f32 / clip.frame_rate as f32,
        ] {
            assert_eq!(
                bevy_clip.sample_clamped(animated_field!(Transform::scale), target, time),
                Some(Vec3::ONE)
            );
        }
        for frame in [0u32, 2] {
            let time = frame as f32 / clip.frame_rate as f32;
            let expected_translation = clip.tracks[0]
                .translation
                .reference_records(clip.frame_count)
                .unwrap()
                .record_at(frame)
                .unwrap()
                .copied()
                .unwrap();
            assert_eq!(
                bevy_clip.sample_clamped(animated_field!(Transform::translation), target, time),
                Some(Vec3::from_array(expected_translation))
            );
            let expected_rotation = reference_unpack_rotation(
                *clip.tracks[0]
                    .rotation
                    .reference_records(clip.frame_count)
                    .unwrap()
                    .record_at(frame)
                    .unwrap()
                    .unwrap(),
            );
            assert_eq!(
                bevy_clip.sample_clamped(animated_field!(Transform::rotation), target, time),
                Some(Quat::from_array(expected_rotation))
            );
        }
    }
}
