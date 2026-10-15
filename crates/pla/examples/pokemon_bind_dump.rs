//! Dump skeleton reference-pose vs authored-bind residuals for Pokémon packs.
//!
//! Diagnostic tool for the packs the batch exporter reports as "static only"
//! because `validate_bind_rest_pose` rejects them. For every rig-mapped node it
//! prints `global * bind` and `bind * global` deviations from the identity; the
//! worst nodes also print their local SRT, their matrices and rotation
//! determinants. It also tests the six possible Euler composition orders for
//! the whole rest pose and reports the resulting maximum residual per order.
//!
//! Usage: pokemon_bind_dump <pm....gfpak>... [--threshold <f64>] [--all]

use pla::assets::pokemon_model::assemble_pack;
use pla::assets::tr::{TrSkl, TrSklMatrix};
use std::env;

type Mat3 = [[f64; 3]; 3];

fn euler(axis: usize, angle: f64) -> Mat3 {
    let (s, c) = angle.sin_cos();
    match axis {
        0 => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        1 => [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        _ => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
    }
}

fn mul3(a: &Mat3, b: &Mat3) -> Mat3 {
    std::array::from_fn(|r| {
        std::array::from_fn(|c| (0..3).map(|i| a[r][i] * b[i][c]).sum())
    })
}

/// R = E(order[0]) * E(order[1]) * E(order[2]) picking each angle by axis.
fn compose(order: [usize; 3], rotation: [f32; 3]) -> Mat3 {
    let e: Vec<Mat3> = order
        .iter()
        .map(|&axis| euler(axis, f64::from(rotation[axis])))
        .collect();
    mul3(&mul3(&e[0], &e[1]), &e[2])
}

fn rotation_det(rotation: &Mat3) -> f64 {
    rotation[0][0] * (rotation[1][1] * rotation[2][2] - rotation[1][2] * rotation[2][1])
        - rotation[0][1] * (rotation[1][0] * rotation[2][2] - rotation[1][2] * rotation[2][0])
        + rotation[0][2] * (rotation[1][0] * rotation[2][1] - rotation[1][1] * rotation[2][0])
}

fn orthonormality_error(rotation: &Mat3) -> f64 {
    let mut worst: f64 = 0.0;
    for a in 0..3 {
        for b in 0..3 {
            let dot: f64 = (0..3).map(|i| rotation[i][a] * rotation[i][b]).sum();
            let expected = if a == b { 1.0 } else { 0.0 };
            worst = worst.max((dot - expected).abs());
        }
    }
    worst
}

fn matrix_from_parts(rotation: &Mat3, scale: [f32; 3], translation: [f32; 3]) -> TrSklMatrix {
    let mut rows = TrSklMatrix::IDENTITY.rows;
    for row in 0..3 {
        for column in 0..3 {
            rows[row][column] = rotation[row][column] * f64::from(scale[column]);
        }
        rows[row][3] = f64::from(translation[row]);
    }
    TrSklMatrix { rows }
}

/// Inverse of an affine matrix (rotation/scale block plus translation). The
/// 3x3 block is inverted by its adjugate; near-singular blocks return the
/// original matrix so callers cannot silently read garbage.
fn invert_affine(matrix: &TrSklMatrix) -> TrSklMatrix {
    let m: Mat3 = std::array::from_fn(|r| std::array::from_fn(|c| matrix.rows[r][c]));
    let det = rotation_det(&m);
    if det.abs() < 1e-12 {
        return matrix.clone();
    }
    let inv_det = 1.0 / det;
    let adj: Mat3 = [
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
    let t = [matrix.rows[0][3], matrix.rows[1][3], matrix.rows[2][3]];
    let inv_t: [f64; 3] = std::array::from_fn(|r| -(0..3).map(|c| adj[r][c] * t[c]).sum::<f64>());
    let mut rows = TrSklMatrix::IDENTITY.rows;
    for r in 0..3 {
        rows[r][..3].copy_from_slice(&adj[r]);
        rows[r][3] = inv_t[r];
    }
    TrSklMatrix { rows }
}

/// Chain of local matrices under one Euler composition order, mirroring the
/// library's `reference_pose` but convention-parametric.
fn pose_with_order(
    skeleton: &TrSkl,
    order: [usize; 3],
) -> Result<Vec<TrSklMatrix>, String> {
    let mut global: Vec<TrSklMatrix> = Vec::with_capacity(skeleton.nodes.len());
    for (i, node) in skeleton.nodes.iter().enumerate() {
        let rotation = compose(order, node.local.rotation);
        let local = matrix_from_parts(&rotation, node.local.scale, node.local.translation);
        let world = match node.parent {
            None => local,
            Some(parent) if parent < i => global[parent].multiply(&local)?,
            _ => return Err("unsupported reference parent order or cycle".into()),
        };
        global.push(world);
    }
    Ok(global)
}

fn print_matrix(label: &str, matrix: &TrSklMatrix) {
    println!("    {label}:");
    for row in &matrix.rows {
        println!(
            "      [{:>11.6} {:>11.6} {:>11.6} {:>11.6}]",
            row[0], row[1], row[2], row[3]
        );
    }
}

fn print_deviation(label: &str, product: &TrSklMatrix) {
    let mut worst = (0.0_f64, 0, 0);
    for (r, row) in product.rows.iter().enumerate() {
        for (c, value) in row.iter().enumerate() {
            let expected = if r == c { 1.0 } else { 0.0 };
            let delta = (value - expected).abs();
            if delta > worst.0 {
                worst = (delta, r, c);
            }
        }
    }
    println!(
        "    {label} worst dev {:.9} at [{},{}]",
        worst.0, worst.1, worst.2
    );
}

fn dump_pack(path: &str, threshold: f64, all: bool, summary: bool) -> Result<(), String> {
    println!("== {path}");
    let model = assemble_pack(path)?;
    let Some(skeleton) = &model.skeleton else {
        println!(
            "  no skeleton: {}",
            model
                .skeleton_parse_error
                .clone()
                .unwrap_or_else(|| "unknown".into())
        );
        return Ok(());
    };
    println!(
        "  nodes={} bind_records={} node_types={:?}",
        skeleton.nodes.len(),
        model.bind_records.len(),
        model.skeleton_node_type_counts
    );

    // Convention probe: maximum residual across every rig-mapped node.
    let orders: [[usize; 3]; 6] = [
        [2, 1, 0],
        [0, 1, 2],
        [1, 2, 0],
        [2, 0, 1],
        [0, 2, 1],
        [1, 0, 2],
    ];
    for order in orders {
        match pose_with_order(skeleton, order) {
            Ok(globals) => {
                let mut worst: f64 = 0.0;
                let mut worst_node = 0;
                for (i, node) in skeleton.nodes.iter().enumerate() {
                    let Some(rig) = node.rig_index else { continue };
                    let Some(record) = model.bind_records.get(rig) else {
                        continue;
                    };
                    let product = globals[i]
                        .multiply(&record.matrix)
                        .map_err(|e| format!("multiply: {e}"))?;
                    let mut dev: f64 = 0.0;
                    for (r, row) in product.rows.iter().enumerate() {
                        for (c, value) in row.iter().enumerate() {
                            let expected = if r == c { 1.0 } else { 0.0 };
                            dev = dev.max((value - expected).abs());
                        }
                    }
                    if dev > worst {
                        worst = dev;
                        worst_node = i;
                    }
                }
                let label = match order {
                    [2, 1, 0] => "z*y*x (current)",
                    [0, 1, 2] => "x*y*z",
                    [1, 2, 0] => "y*z*x",
                    [2, 0, 1] => "z*x*y",
                    [0, 2, 1] => "x*z*y",
                    _ => "y*x*z",
                };
                println!(
                    "  convention {label}: max residual {worst:.9} (node {worst_node})"
                );
            }
            Err(error) => println!("  convention {order:?}: {error}"),
        }
    }

    let pose = skeleton.reference_pose()?;
    if summary {
        for (i, node) in skeleton.nodes.iter().enumerate() {
            let Some(rig) = node.rig_index else { continue };
            let Some(record) = model.bind_records.get(rig) else {
                continue;
            };
            let rotation = compose([2, 1, 0], node.local.rotation);
            let local_matrix =
                matrix_from_parts(&rotation, node.local.scale, node.local.translation);
            let inv_global = invert_affine(&pose.global[i]);
            let inv_local = invert_affine(&local_matrix);
            println!(
                "  node {:>3} rig {:>3} '{}' |b-invG|={:.3e} |b-invL|={:.3e}",
                i,
                rig,
                node.name,
                inv_global.max_abs_difference(&record.matrix)?,
                inv_local.max_abs_difference(&record.matrix)?,
            );
        }
        return Ok(());
    }
    let residuals = skeleton.bind_rest_residuals(&model.bind_records)?;
    let mut above = 0;
    for residual in &residuals {
        let error = residual
            .global_times_bind_error
            .max(residual.bind_times_global_error);
        if error <= threshold && !all {
            continue;
        }
        above += 1;
        let node = &skeleton.nodes[residual.node_index];
        println!(
            "  node {} '{}' parent={:?} rig={} g*b={:.9} b*g={:.9}",
            residual.node_index,
            node.name,
            node.parent,
            residual.rig_index,
            residual.global_times_bind_error,
            residual.bind_times_global_error,
        );
        let t = &node.local;
        println!(
            "    local s={:?} r={:?} t={:?}",
            t.scale, t.rotation, t.translation
        );
        let global = &pose.global[residual.node_index];
        let bind = &model.bind_records[residual.rig_index].matrix;
        let g_rot: Mat3 = std::array::from_fn(|r| std::array::from_fn(|c| global.rows[r][c]));
        let b_rot: Mat3 = std::array::from_fn(|r| std::array::from_fn(|c| bind.rows[r][c]));
        println!(
            "    det(global rot)={:.6} ortho_err={:.2e} | det(bind rot)={:.6} ortho_err={:.2e}",
            rotation_det(&g_rot),
            orthonormality_error(&g_rot),
            rotation_det(&b_rot),
            orthonormality_error(&b_rot),
        );
        let gb = global.multiply(bind)?;
        let bg = bind.multiply(global)?;
        print_deviation("global*bind", &gb);
        print_deviation("bind*global", &bg);
        print_matrix("global", global);
        print_matrix("bind", bind);
        print_matrix("global*bind", &gb);
        // Candidate semantics for the serialized record, closest first.
        let rotation = compose([2, 1, 0], node.local.rotation);
        let local_matrix =
            matrix_from_parts(&rotation, node.local.scale, node.local.translation);
        let mut candidates: Vec<(&str, TrSklMatrix)> = vec![
            ("inverse(global)", invert_affine(global)),
            ("inverse(local)", invert_affine(&local_matrix)),
            ("local", local_matrix),
            ("global", global.clone()),
        ];
        candidates.sort_by(|a, b| {
            a.1.max_abs_difference(bind)
                .unwrap_or(f64::INFINITY)
                .partial_cmp(&b.1.max_abs_difference(bind).unwrap_or(f64::INFINITY))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (label, candidate) in &candidates {
            println!(
                "    |bind - {label}| = {:.9}",
                candidate.max_abs_difference(bind)?
            );
        }
        // Per-order local-inverse probe: is the record a different Euler order of
        // this node's own local transform?
        let order_names = [
            ([2, 1, 0], "zyx"),
            ([0, 1, 2], "xyz"),
            ([1, 2, 0], "yzx"),
            ([2, 0, 1], "zxy"),
            ([0, 2, 1], "xzy"),
            ([1, 0, 2], "yxz"),
        ];
        for (order, name) in order_names {
            let rotation = compose(order, node.local.rotation);
            let local_matrix =
                matrix_from_parts(&rotation, node.local.scale, node.local.translation);
            println!(
                "    |bind - inverse(local {name})| = {:.9}",
                invert_affine(&local_matrix).max_abs_difference(bind)?
            );
        }
        // Does this record match the inverse-global of ANY node (mapping shift)?
        let mut best: Vec<(f64, usize)> = Vec::new();
        for (m, _other) in skeleton.nodes.iter().enumerate() {
            let inv = invert_affine(&pose.global[m]);
            best.push((inv.max_abs_difference(bind)?, m));
        }
        best.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        for (distance, m) in best.iter().take(2) {
            println!(
                "    best |bind - inverse(global of node {m} '{}')| = {distance:.9}",
                skeleton.nodes[*m].name
            );
        }
    }
    println!(
        "  {above} of {} rig-mapped nodes above {threshold}",
        residuals.len()
    );
    Ok(())
}

fn main() -> Result<(), String> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let mut threshold = 1.0e-4_f64;
    let mut all = false;
    let mut summary = false;
    if let Some(i) = args.iter().position(|a| a == "--summary") {
        summary = true;
        args.remove(i);
    }
    if let Some(i) = args.iter().position(|a| a == "--threshold") {
        if i + 1 < args.len() {
            let value = args.remove(i + 1);
            threshold = value.parse().map_err(|e| format!("bad threshold: {e}"))?;
        }
        args.remove(i);
    }
    if let Some(i) = args.iter().position(|a| a == "--all") {
        all = true;
        args.remove(i);
    }
    if args.is_empty() {
        return Err(
            "usage: pokemon_bind_dump <pm....gfpak>... [--threshold <f64>] [--all]".into(),
        );
    }
    let mut failed = Vec::new();
    for path in &args {
        if let Err(error) = dump_pack(path, threshold, all, summary) {
            println!("  ERROR: {error}");
            failed.push(path.clone());
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("{} packs failed: {failed:?}", failed.len()))
    }
}
