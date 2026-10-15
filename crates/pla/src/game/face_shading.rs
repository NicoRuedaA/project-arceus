//! Soft shading for character faces and hair.
//!
//! The exported faces are almost flat discs and their vertex normals follow every dent of the
//! mesh (nose, mouth, brow). With the two-tone sun shading of `art_style` (and with plain PBR
//! too) that makes hard dark patches appear on the face (a "beard" under the nose, a grey band
//! under the eyes) and ragged streaks in the hair, because each patch is just a part of the
//! surface that faces slightly away from the sun. Cel-shaded games avoid it by shading a face as
//! if it were a smooth ball. This module does the same: for the face, eyebrow and eyelash meshes
//! (and, less strongly, the hair) it blends each vertex normal towards the direction from the
//! centre of the head to that vertex. Positions, UVs and skinning are untouched.
//!
//! ASSUMED: the original game does something similar; the exact method is not decoded.

use std::collections::HashSet;

use bevy::asset::AssetId;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;

/// Share of the sphere normal in the final normal for faces. ASSUMED, picked by eye.
pub const FACE_BLEND: f32 = 0.9;

/// Whether a mesh is the face skin (`<part>_mesh_shape#<material>` glTF name). Only this mesh is
/// softened: it is one piece, so its own bounding box gives a sensible ball centre. Eyebrows,
/// eyelashes, eyes and the inside of the mouth are small separate pieces and keep their normals;
/// hair keeps its normals too (its patches come from the normal map, see `normal_fix`).
pub fn blend_for_mesh_name(name: &str) -> Option<f32> {
    (name == "face_mesh_shape#face_skin").then_some(FACE_BLEND)
}

/// Blends `normals` towards `normalize(position - centre)` by `blend` (0..1). Vertices at the
/// centre keep their normal. Result vectors are unit length.
pub fn spherize_normals(positions: &[[f32; 3]], normals: &mut [[f32; 3]], centre: Vec3, blend: f32) {
    let blend = blend.clamp(0.0, 1.0);
    for (p, n) in positions.iter().zip(normals.iter_mut()) {
        let radial = Vec3::from(*p) - centre;
        let Some(radial) = radial.try_normalize() else { continue };
        let old = Vec3::from(*n).try_normalize().unwrap_or(radial);
        *n = old.lerp(radial, blend).try_normalize().unwrap_or(old).to_array();
    }
}

/// Centre of the sphere the face is shaded as: the middle of the mesh box, moved back so a
/// disc-like face is treated as the front of a ball instead of a flat plate.
pub fn head_centre(positions: &[[f32; 3]]) -> Option<Vec3> {
    let first = positions.first()?;
    let (mut lo, mut hi) = (Vec3::from(*first), Vec3::from(*first));
    for p in positions {
        lo = lo.min(Vec3::from(*p));
        hi = hi.max(Vec3::from(*p));
    }
    let mid = (lo + hi) * 0.5;
    // The model looks towards +Z; a head is about as deep as it is wide.
    let width = (hi.x - lo.x).max(hi.y - lo.y);
    Some(Vec3::new(mid.x, mid.y, hi.z - width * 0.5))
}

fn soften_mesh(mesh: &mut Mesh, blend: f32) -> bool {
    let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        return false;
    };
    let positions = positions.clone();
    let Some(centre) = head_centre(&positions) else { return false };
    let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL) else {
        return false;
    };
    if normals.len() != positions.len() {
        return false;
    }
    spherize_normals(&positions, normals, centre, blend);
    true
}

fn soften_character_normals(
    named: Query<(&Name, &Mesh3d), Added<Mesh3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut done: Local<HashSet<AssetId<Mesh>>>,
) {
    for (name, mesh3d) in &named {
        let Some(blend) = blend_for_mesh_name(name.as_str()) else { continue };
        if !done.insert(mesh3d.0.id()) {
            continue;
        }
        if let Some(mut mesh) = meshes.get_mut(&mesh3d.0) {
            soften_mesh(&mut mesh, blend);
        }
    }
}

/// Registered by `NormalMapFixPlugin` (one plugin for all character mesh repairs).
pub(crate) fn add_systems(app: &mut App) {
    app.add_systems(Update, soften_character_normals);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_face_skin_mesh_is_softened() {
        assert_eq!(blend_for_mesh_name("face_mesh_shape#face_skin"), Some(FACE_BLEND));
        assert_eq!(blend_for_mesh_name("face_mesh_shape#eye"), None);
        assert_eq!(blend_for_mesh_name("eyelash_mesh_shape#face"), None);
        assert_eq!(blend_for_mesh_name("eyebrow_mesh_shape#eyebrow"), None);
        assert_eq!(blend_for_mesh_name("hair_mesh_shape#hairstyle"), None);
        assert_eq!(blend_for_mesh_name("mouth_mesh_shape#face_skin"), None);
        assert_eq!(blend_for_mesh_name("eye_mesh_shape#eye"), None);
        assert_eq!(blend_for_mesh_name("uppercloth_mesh_shape#body"), None);
        assert_eq!(blend_for_mesh_name("no_hash"), None);
    }

    #[test]
    fn spherize_moves_a_dent_normal_towards_the_radial_direction() {
        let positions = [[0.0, 0.0, 1.0]];
        let mut normals = [[1.0, 0.0, 0.0]]; // a dent normal pointing sideways
        spherize_normals(&positions, &mut normals, Vec3::ZERO, 0.9);
        let n = Vec3::from(normals[0]);
        assert!((n.length() - 1.0).abs() < 1e-5);
        assert!(n.z > 0.9, "now mostly radial: {n:?}");
    }

    #[test]
    fn zero_blend_keeps_normals_and_centre_vertex_is_skipped() {
        let positions = [[0.0, 0.0, 1.0], [0.0, 0.0, 0.0]];
        let mut normals = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        spherize_normals(&positions, &mut normals, Vec3::ZERO, 0.0);
        assert_eq!(normals[0], [1.0, 0.0, 0.0]);
        spherize_normals(&positions, &mut normals, Vec3::ZERO, 1.0);
        assert_eq!(normals[1], [0.0, 1.0, 0.0], "vertex at the centre untouched");
    }

    #[test]
    fn head_centre_sits_behind_the_front_of_a_disc() {
        let disc = [[-0.1, 1.2, -0.05], [0.1, 1.4, 0.11]];
        let c = head_centre(&disc).unwrap();
        assert!((c.x).abs() < 1e-6 && (c.y - 1.3).abs() < 1e-6);
        assert!(c.z < 0.11 - 0.05, "behind the front: {c:?}");
        assert!(head_centre(&[]).is_none());
    }
}
