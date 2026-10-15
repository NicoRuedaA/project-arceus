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

use std::collections::{HashMap, HashSet};

use bevy::asset::AssetId;
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;

/// Share of the dome normal in the final normal for faces, hair and clothes. ASSUMED, picked by eye.
pub const FACE_BLEND: f32 = 0.92;
pub const HAIR_BLEND: f32 = 0.85;
pub const CLOTH_BLEND: f32 = 0.75;
/// Shared head-dome centre at the base of the skull in character model space for multi-part hair
/// (`hair_*`, `hide_*`) and internal mouth meshes so adjacent pieces share one continuous dome.
pub const HAIR_DOME_CENTRE: Vec3 = Vec3::new(0.0, 1.18, -0.02);

/// Whether a mesh is a character part whose normals should be softened (`<part>_mesh_shape#<material>`
/// glTF name, with or without the `.material-name` suffix added by `bevy_gltf` on `Mesh3d` child
/// entities). Eyes, eyebrows and eyelashes keep their sharp normals.
pub fn blend_for_mesh_name(name: &str) -> Option<f32> {
    let mesh_name = name.split('.').next().unwrap_or(name);
    if matches!(
        mesh_name,
        "face_mesh_shape#face_skin"
            | "mouth_mesh_shape#face_skin"
            | "tongue_mesh_shape#face_skin"
            | "tooth_mesh_shape#face"
    ) {
        Some(FACE_BLEND)
    } else if mesh_name.ends_with("#hairstyle") {
        Some(HAIR_BLEND)
    } else if mesh_name.ends_with("#body")
        || mesh_name.ends_with("#body_skin")
        || mesh_name.ends_with("#shoes")
        || mesh_name.ends_with("#baggage")
    {
        Some(CLOTH_BLEND)
    } else {
        None
    }
}

/// Welds split UV-seam normals at identical positions and runs `passes` rounds of triangle-neighbour
/// Laplacian normal averaging so sharp 1-triangle cloth wrinkles do not speckle the toon threshold.
pub fn smooth_mesh_normals(positions: &[[f32; 3]], normals: &mut [[f32; 3]], indices: &[u32], passes: usize) {
    if positions.is_empty() || positions.len() != normals.len() {
        return;
    }
    let quant = |p: [f32; 3]| [(p[0] * 10_000.0).round() as i32, (p[1] * 10_000.0).round() as i32, (p[2] * 10_000.0).round() as i32];
    let mut groups: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
    for (i, p) in positions.iter().enumerate() {
        groups.entry(quant(*p)).or_default().push(i);
    }
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); positions.len()];
    for tri in indices.chunks_exact(3) {
        let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        if a < positions.len() && b < positions.len() && c < positions.len() {
            adj[a].extend([b, c]);
            adj[b].extend([a, c]);
            adj[c].extend([a, b]);
        }
    }
    for _ in 0..passes {
        // Weld co-located seam vertices first.
        for verts in groups.values() {
            if verts.len() > 1 {
                let sum = verts.iter().fold(Vec3::ZERO, |acc, &i| acc + Vec3::from(normals[i]));
                if let Some(avg) = sum.try_normalize() {
                    let arr = avg.to_array();
                    for &i in verts {
                        normals[i] = arr;
                    }
                }
            }
        }
        let prev: Vec<Vec3> = normals.iter().map(|n| Vec3::from(*n)).collect();
        for (i, nbrs) in adj.iter().enumerate() {
            if nbrs.is_empty() {
                continue;
            }
            let sum = nbrs.iter().fold(prev[i] * 2.0, |acc, &j| acc + prev[j]);
            if let Some(avg) = sum.try_normalize() {
                normals[i] = avg.to_array();
            }
        }
    }
}

/// Blends `normals` towards a dome direction from `centre` by `blend` (0..1). Vertices below
/// `centre.y` are lifted to a gentle upward tilt so an overhead sun does not cast a dark lower-hemisphere
/// "beard" across the cheeks, mouth, chin or sleeves. Vertices at the centre keep their normal.
pub fn spherize_normals(positions: &[[f32; 3]], normals: &mut [[f32; 3]], centre: Vec3, blend: f32) {
    let blend = blend.clamp(0.0, 1.0);
    for (p, n) in positions.iter().zip(normals.iter_mut()) {
        let mut radial = Vec3::from(*p) - centre;
        if radial.y < 0.0 {
            let horiz = Vec2::new(radial.x, radial.z).length();
            radial.y = horiz * 0.35;
        }
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

fn soften_mesh(mesh: &mut Mesh, name: &str, blend: f32) -> bool {
    let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        return false;
    };
    let positions = positions.clone();
    let indices: Vec<u32> = match mesh.indices() {
        Some(Indices::U32(v)) => v.clone(),
        Some(Indices::U16(v)) => v.iter().map(|&i| u32::from(i)).collect(),
        None => Vec::new(),
    };
    let mesh_name = name.split('.').next().unwrap_or(name);
    let is_head_shared = mesh_name.ends_with("#hairstyle")
        || matches!(
            mesh_name,
            "mouth_mesh_shape#face_skin" | "tongue_mesh_shape#face_skin" | "tooth_mesh_shape#face"
        );
    let centre = if is_head_shared {
        HAIR_DOME_CENTRE
    } else {
        let Some(c) = head_centre(&positions) else { return false };
        c
    };
    let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL) else {
        return false;
    };
    if normals.len() != positions.len() {
        return false;
    }
    if !indices.is_empty() {
        smooth_mesh_normals(&positions, normals, &indices, 4);
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
            soften_mesh(&mut mesh, name.as_str(), blend);
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
    fn character_face_hair_and_cloth_meshes_are_softened() {
        assert_eq!(blend_for_mesh_name("face_mesh_shape#face_skin"), Some(FACE_BLEND));
        assert_eq!(
            blend_for_mesh_name("face_mesh_shape#face_skin.face_mesh_shape#face_skin-material"),
            Some(FACE_BLEND)
        );
        assert_eq!(blend_for_mesh_name("mouth_mesh_shape#face_skin"), Some(FACE_BLEND));
        assert_eq!(blend_for_mesh_name("tooth_mesh_shape#face"), Some(FACE_BLEND));
        assert_eq!(blend_for_mesh_name("hair_mesh_shape#hairstyle"), Some(HAIR_BLEND));
        assert_eq!(
            blend_for_mesh_name("hair_mesh_shape#hairstyle.hair_mesh_shape#hairstyle-material"),
            Some(HAIR_BLEND)
        );
        assert_eq!(blend_for_mesh_name("hide_b_mesh_shape#hairstyle"), Some(HAIR_BLEND));
        assert_eq!(blend_for_mesh_name("uppercloth_mesh_shape#body"), Some(CLOTH_BLEND));
        assert_eq!(blend_for_mesh_name("face_mesh_shape#eye"), None);
        assert_eq!(blend_for_mesh_name("eyelash_mesh_shape#face"), None);
        assert_eq!(blend_for_mesh_name("eyebrow_mesh_shape#eyebrow"), None);
        assert_eq!(blend_for_mesh_name("eye_mesh_shape#eye"), None);
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

        // Co-located seam vertices are welded and smoothed across triangle neighbours.
        let seam_pos = [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let mut seam_nrm = [[0.6, 0.0, 0.8], [-0.6, 0.0, 0.8], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]];
        smooth_mesh_normals(&seam_pos, &mut seam_nrm, &[0, 2, 3, 1, 2, 3], 2);
        assert!((seam_nrm[0][0] - seam_nrm[1][0]).abs() < 1e-5, "co-located seam welded");
        assert!(seam_nrm[0][2] > 0.95);
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
