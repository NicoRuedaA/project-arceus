//! Headless report (no window): navmesh walkable components per area, where the OLD default
//! start lands, and the navmesh-based safe start of the REPORT helpers
//! (`pla::game::nav_report`; since 2026-10-17 the game's own movement and spawn use the
//! terrain rule of `pla::game::terrain_collision`, not the navmesh).
//!
//! Usage: spawn_report   (PLA_BIN_ROOT overrides the game root; run from the repo root for data/)

use std::path::PathBuf;

use bevy::math::Vec2;
use pla::assets::navmesh::AreaNavigation;
use pla::game::nav_report::*;
use pla::game::spawn_point::{landmarks_for_area, SpawnConfig};

fn describe(mesh: &pla::assets::navmesh::NavMesh, c: &Components, p: Vec2) -> String {
    match classify_point(mesh, c, p.x, p.y) {
        PointStatus::NotWalkable => "NOT walkable".into(),
        PointStatus::Walkable { component, face, y } => format!(
            "walkable, component #{component} ({} faces, {:.0} m2 of {} faces total), y={y:.1}, slope={:.1} deg",
            c.face_count[component as usize],
            c.area_m2[component as usize],
            mesh.faces.len(),
            face_slope_deg(mesh, face)
        ),
    }
}

fn main() {
    let root = std::env::var_os("PLA_BIN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into());
    let spawn_tsv = std::fs::read_to_string("data/field_spawn_table.tsv").expect("data/field_spawn_table.tsv");
    let place_tsv = std::fs::read_to_string("data/field_object_placements.tsv").expect("placements");
    let cfg = SpawnConfig::default();
    for area in 0u8..=5 {
        let nav = match AreaNavigation::load(&root, area) {
            Ok(n) => n,
            Err(e) => {
                println!("area {area}: skip ({e})");
                continue;
            }
        };
        let m = nav.primary();
        let c = components(m);
        println!("== area {area:02}: {} faces, {} components", m.faces.len(), c.len());
        for i in 0..c.len().min(5) {
            println!("   component #{i}: {} faces, {:.0} m2", c.face_count[i], c.area_m2[i]);
        }
        // OLD default as the viewer computed it: fixed point, snapped to nearest face centre.
        let old = if area == 0 { Vec2::new(448.0, 652.0) } else { Vec2::new(512.5, 512.5) };
        let mut old_eff = old;
        let mut note = String::new();
        if !m.is_walkable(old.x, old.y) {
            if let Some(h) = m.nearest(old.x, old.y, 400.0) {
                let cen = face_centroid(m, h.face);
                old_eff = Vec2::new(cen[0], cen[2]);
                note = format!(" (fixed point {old} not walkable; viewer snapped to nearest face centre)");
            }
        }
        println!("   OLD start {old_eff}{note}: {}", describe(m, &c, old_eff));
        let marks = landmarks_for_area(area, &spawn_tsv, &place_tsv);
        let usable = usable_landmarks(m, &c, &marks);
        println!("   landmarks: {} total, {} usable (in/within 15 m of the largest component, first 24 kept)", marks.len(), usable.len());
        match choose_safe_spawn(m, &c, &marks, &cfg) {
            None => println!("   NEW start: NONE FOUND"),
            Some(s) => {
                println!("   NEW start {}: {}", s.xz, describe(m, &c, s.xz));
                if !usable.is_empty() {
                    let cen = usable.iter().copied().sum::<Vec2>() / usable.len() as f32;
                    let proj = m.nearest(cen.x, cen.y, 60.0).map(|h| {
                        let f = face_centroid(m, h.face);
                        (h.face, Vec2::new(f[0], f[2]))
                    });
                    println!("   landmark centroid {cen}: {}; distance from new start {:.1} m", describe(m, &c, cen), cen.distance(s.xz));
                    if let Some((face, p)) = proj {
                        let reach = reachable_faces(m, s.face);
                        println!("   centroid projected to nearest face {p}: reachable by BFS from new start = {}", reach[face as usize]);
                    }
                }
                // Reachability by explicit BFS over face adjacency from the start face to every landmark face.
                let reach = reachable_faces(m, s.face);
                let ok = marks
                    .iter()
                    .filter(|p| matches!(classify_point(m, &c, p.x, p.y), PointStatus::Walkable { face, .. } if reach[face as usize]))
                    .count();
                let ok_snapped = usable
                    .iter()
                    .filter(|p| matches!(classify_point(m, &c, p.x, p.y), PointStatus::Walkable { face, .. } if reach[face as usize]))
                    .count();
                println!(
                    "   landmarks reachable by BFS from the new start: {ok_snapped} of {} usable (snapped); {ok} of {} raw positions are themselves on walkable ground",
                    usable.len(),
                    marks.len()
                );
                // Clearance + slope audit.
                let clear = (0..16).all(|k| {
                    let a = k as f32 * std::f32::consts::TAU / 16.0;
                    (1..=10).all(|r| matches!(classify_point(m, &c, s.xz.x + r as f32 * a.cos(), s.xz.y + r as f32 * a.sin()), PointStatus::Walkable { component: 0, .. }))
                });
                println!("   audit: slope {:.1} deg (<= {}), 10 m radial walkable & in largest component: {clear}", s.slope_deg, cfg.max_slope_deg);
            }
        }
    }
}

fn reachable_faces(m: &pla::assets::navmesh::NavMesh, start: u32) -> Vec<bool> {
    let mut seen = vec![false; m.faces.len()];
    let mut stack = vec![start];
    seen[start as usize] = true;
    while let Some(f) = stack.pop() {
        let fc = m.faces[f as usize];
        for e in fc.start_edge..fc.start_edge + fc.num_edges as u32 {
            let o = m.edges[e as usize].opposite_face;
            if (o as usize) < seen.len() && !seen[o as usize] {
                seen[o as usize] = true;
                stack.push(o);
            }
        }
    }
    seen
}
