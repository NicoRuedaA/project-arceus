//! Probe: export a Pokémon pack as a skinned glTF with an explicit bind/rest
//! residual tolerance, to compare visually against the shipped static fallback.
//!
//! Diagnostic only. The batch exporter keeps its own validated tolerance; this
//! tool exists so a rejected skeleton can be rendered offscreen and inspected.
//!
//! Usage: pokemon_probe_export <pack.gfpak> <out.gltf> [--epsilon F] [--clip SUBSTR]
//!
//! Writes every LOD0 shape with plain materials (no textures) and one clip.

use pla::assets::gltf_export::{
    animation_tracks_from_tr, primitive_from_tr, skeleton_from_tr_with_epsilon, GltfExport, GltfNode,
};
use pla::assets::pokemon_model::assemble_pack;
use pla::assets::tr::{TrSklBindRecord, TrSklMatrix};
use std::env;

/// Inverse of an affine matrix: 3x3 adjugate block plus `-Rinv * t`.
fn invert_affine(rows: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let m = *rows;
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-12 {
        return m;
    }
    let inv_det = 1.0 / det;
    let adj = [
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * inv_det,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv_det,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv_det,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * inv_det,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv_det,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv_det,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * inv_det,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv_det,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv_det,
        ],
    ];
    let t = [m[0][3], m[1][3], m[2][3]];
    let inv_t = [
        -(adj[0][0] * t[0] + adj[0][1] * t[1] + adj[0][2] * t[2]),
        -(adj[1][0] * t[0] + adj[1][1] * t[1] + adj[1][2] * t[2]),
        -(adj[2][0] * t[0] + adj[2][1] * t[1] + adj[2][2] * t[2]),
    ];
    let mut out = [[0.0_f64; 4]; 4];
    for r in 0..3 {
        out[r][..3].copy_from_slice(&adj[r]);
        out[r][3] = inv_t[r];
    }
    out[3] = [0.0, 0.0, 0.0, 1.0];
    out
}

fn main() -> Result<(), String> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let mut epsilon = 1.0e-4_f64;
    let mut needle = "defaultwait".to_string();
    let mut rest_inverse = false;
    let take = |args: &mut Vec<String>, flag: &str| -> Option<String> {
        args.iter().position(|a| a == flag).map(|i| {
            args.remove(i);
            args.remove(i)
        })
    };
    if let Some(v) = take(&mut args, "--epsilon") {
        epsilon = v.parse().map_err(|e| format!("bad --epsilon: {e}"))?;
    }
    if let Some(v) = take(&mut args, "--clip") {
        needle = v;
    }
    if args.iter().any(|a| a == "--rest-inverse") {
        rest_inverse = true;
        args.retain(|a| a != "--rest-inverse");
    }
    if args.len() != 2 {
        return Err("usage: pokemon_probe_export <pack.gfpak> <out.gltf> [--epsilon F] [--clip SUBSTR]".into());
    }
    let pack = &args[0];
    let out = &args[1];
    let model = assemble_pack(pack)?;
    let skeleton = model
        .skeleton
        .as_ref()
        .ok_or_else(|| format!("no skeleton: {:?}", model.skeleton_parse_error))?;
    if model.bind_records.is_empty() {
        return Err("no bind records".into());
    }
    let clip = model
        .clips
        .iter()
        .filter(|c| c.parsed.is_ok() && c.name.contains(&needle))
        .min_by_key(|c| (!c.name.contains("01_loop"), c.name.clone()))
        .ok_or_else(|| {
            let errors: Vec<String> = model
                .clips
                .iter()
                .filter(|c| c.name.contains(&needle))
                .map(|c| match &c.parsed {
                    Ok(_) => format!("{}: parsed", c.name),
                    Err(e) => format!("{}: {e}", c.name),
                })
                .collect();
            format!("no parsed clip matching '{needle}'; candidates: {errors:?}")
        })?;
    let clip_parsed = clip.parsed.as_ref().map_err(|e| e.clone())?;
    let mut export = GltfExport::default();
    let mut shapes = 0;
    for mesh in model.meshes.iter().filter(|mesh| mesh.lod == Some(0)) {
        export.primitives.push(primitive_from_tr(
            &mesh.name,
            &mesh.mesh,
            &mesh.buffer_bytes,
            Some(skeleton),
            None,
        )?);
        shapes += 1;
    }
    if shapes == 0 {
        return Err("no LOD0 shapes".into());
    }
    let (nodes, skin) = if rest_inverse {
        // Diagnostic substitution: inverse binds rebuilt from the reference rest
        // pose (skin = identity at rest). Never used by the shipped exporters.
        let pose = skeleton.reference_pose()?;
        let mut rig_to_node = vec![usize::MAX; model.bind_records.len()];
        for (i, node) in skeleton.nodes.iter().enumerate() {
            if let Some(rig) = node.rig_index {
                rig_to_node[rig] = i;
            }
        }
        if rig_to_node.iter().any(|&n| n == usize::MAX) {
            return Err("unresolved rig mapping".into());
        }
        let records: Vec<TrSklBindRecord> = rig_to_node
            .iter()
            .map(|&node_index| TrSklBindRecord {
                flags: [1, 1],
                matrix: TrSklMatrix {
                    rows: invert_affine(&pose.global[node_index].rows),
                },
            })
            .collect();
        skeleton_from_tr_with_epsilon(skeleton, &records, 0, 0, epsilon)?
    } else {
        skeleton_from_tr_with_epsilon(skeleton, &model.bind_records, 0, 0, epsilon)?
    };
    export.nodes = nodes;
    export.skins.push(skin);
    export.animations.push((
        clip.name.clone(),
        animation_tracks_from_tr(skeleton, clip_parsed)?,
    ));
    let root = export.skins.first().and_then(|skin| skin.skeleton);
    for mesh in 0..export.primitives.len() {
        let node_index = export.nodes.len();
        export.nodes.push(GltfNode {
            name: export.primitives[mesh].name.clone(),
            children: Vec::new(),
            mesh: Some(mesh),
            skin: Some(0),
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        });
        if let Some(parent) = root {
            export
                .nodes
                .get_mut(parent)
                .ok_or("mesh parent node is outside node table")?
                .children
                .push(node_index);
        }
    }
    export.write(out)?;
    println!(
        "wrote {out}: {} shapes, {} nodes, clip '{}', epsilon {epsilon}",
        shapes,
        export.nodes.len(),
        clip.name
    );
    Ok(())
}
