//! Windowless report: how much interaction reach every decoded door needs.
//!
//! For each `data/warp_links.tsv` door row, sample the walkable points of its own area's navmesh
//! within 5 m of the door centre (0.1 m grid) and compute, per point, the extra margin `pad` that
//! makes `in_trigger(door, point, pad)` true. The minimum over the walkable points is the reach
//! that door needs; the maximum over all doors is the smallest reach that keeps every door
//! reachable on foot.
//!
//! `cargo run -p pla --example door_reach_report --release`

use pla::assets::navmesh::AreaNavigation;
use pla::game::zone::{door_local_offset, ZoneLinks};

fn bin_root() -> std::path::PathBuf {
    std::env::var_os("PLA_BIN_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
        })
}

fn main() {
    let bin = bin_root();
    let links = ZoneLinks::load_default();
    if links.is_empty() {
        eprintln!("[skip] data/warp_links.tsv not found");
        return;
    }
    let mut cache: std::collections::HashMap<String, Option<AreaNavigation>> = Default::default();
    let mut worst: Vec<(f32, String, [f32; 2])> = Vec::new();
    let mut unreachable = 0;
    for l in links.all() {
        let area = l.from_area.clone();
        cache.entry(area.clone()).or_insert_with(|| {
            AreaNavigation::load_named(&bin, &area, 255).ok().or_else(|| {
                area.strip_prefix("ha_area")
                    .and_then(|n| n.parse::<u8>().ok())
                    .and_then(|idx| AreaNavigation::load(&bin, idx).ok())
            })
        });
        let Some(nav) = cache.get(&area).and_then(|n| n.as_ref()) else {
            eprintln!("note: {} ({area}): navmesh not loadable", l.name);
            unreachable += 1;
            continue;
        };
        // Minimum reach over the walkable samples around the door (0.1 m grid, 5 m box).
        let mut need_best = f32::INFINITY;
        let mut best_at = [0.0f32; 2];
        let mut steps = 0i32;
        while steps <= 50 {
            let off = steps as f32 * 0.1;
            for (dx, dz) in [(-off, 0.0), (off, 0.0), (0.0, -off), (0.0, off), (-off, -off), (off, off), (-off, off), (off, -off)] {
                let (x, z) = (l.pos.x + dx, l.pos.z + dz);
                if !nav.is_walkable(x, z) {
                    continue;
                }
                let local = door_local_offset(l, x, z);
                let need = (local.x.abs() - l.trigger.x * 0.5)
                    .max(local.y.abs() - l.trigger.y * 0.5)
                    .max(0.0);
                if need < need_best {
                    need_best = need;
                    best_at = [x, z];
                }
            }
            steps += 1;
        }
        if !need_best.is_finite() {
            // No walkable sample on the axis cross: fall back to the nearest face.
            if let Some(hit) = nav.nearest(l.pos.x, l.pos.z, 400.0) {
                let c = pla::game::nav_report::face_centroid(nav.primary(), hit.face);
                let local = door_local_offset(l, c[0], c[2]);
                let need = (local.x.abs() - l.trigger.x * 0.5)
                    .max(local.y.abs() - l.trigger.y * 0.5)
                    .max(0.0);
                need_best = need;
                best_at = [c[0], c[2]];
            } else {
                eprintln!("note: {} ({area}): no walkable sample and no face", l.name);
                unreachable += 1;
                continue;
            }
        }
        worst.push((need_best, l.name.clone(), best_at));
    }
    worst.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    println!("doors measured: {}", worst.len());
    println!("navmesh not loadable / no face: {unreachable}");
    println!("largest required pads (the smallest safe reach is the first value):");
    for (need, name, p) in worst.iter().take(12) {
        println!("  pad>= {need:.3}  {name}  nearest walkable ({:.2}, {:.2})", p[0], p[1]);
    }
    println!(
        "smallest reach covering every door: {:.3} m",
        worst.first().map(|w| w.0).unwrap_or(0.0)
    );
}
