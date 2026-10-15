//! Report of the real-time terrain material inputs per area (no window, no GPU): which layer
//! fills each splat slot (original or improved, source size, tiling, cliff/triplanar), how long
//! the CPU build of the layer arrays takes, and a cliff-face camera near each area's safe start
//! (used by the before/after screenshots of `--terrain bake|splat`).
//!
//! Usage: terrain_splat_report --bin-root BIN [--areas 0,1,2,3,4,5] [--layer-size 1024] [--json OUT]
//! Run from the repository root (reads data/field_spawn_table.tsv for the start landmarks).
use std::path::PathBuf;

use bevy::math::Vec2;
use pla::assets::navmesh::AreaNavigation;
use pla::assets::terrain::TerrainArea;
use pla::assets::terrain_albedo::layer_weights;
use pla::game::spawn_point::{choose_safe_spawn, components, landmarks_for_area, SpawnConfig};
use pla::game::terrain_splat::{build_splat_data, default_override_dir, DEFAULT_LAYER_SIZE};

/// Ring around the start searched for a cliff face (metres).
const CLIFF_MIN_M: f32 = 20.0;
const CLIFF_MAX_M: f32 = 160.0;
/// Minimum slope of the face (degrees) and minimum weight of the cliff slots there.
const CLIFF_MIN_SLOPE_DEG: f32 = 50.0;
const CLIFF_MIN_WEIGHT: f32 = 0.3;
/// Camera distance from the face (metres).
const CAMERA_DISTANCE: f32 = 14.0;
/// Camera pitch above the focus (degrees).
const CAMERA_PITCH_DEG: f32 = 5.0;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let get = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1).cloned());
    let bin = PathBuf::from(get("--bin-root").expect("--bin-root"));
    let areas: Vec<u8> = get("--areas")
        .unwrap_or_else(|| "0,1,2,3,4,5".into())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let size: u32 = get("--layer-size").and_then(|s| s.parse().ok()).unwrap_or(DEFAULT_LAYER_SIZE);
    let spawn_tsv = std::fs::read_to_string("data/field_spawn_table.tsv").unwrap_or_default();
    let place_tsv = std::fs::read_to_string("data/field_object_placements.tsv").unwrap_or_default();
    let overrides = default_override_dir();
    println!("improved layers: {}", overrides.as_ref().map_or("<none>".into(), |d| d.display().to_string()));
    let mut json = Vec::new();

    for area in areas {
        let t = match TerrainArea::load(&bin, area) {
            Ok(t) => t,
            Err(e) => {
                println!("area {area:02}: not loaded ({e})");
                continue;
            }
        };
        let build = match build_splat_data(&t, overrides.as_deref(), size) {
            Ok(b) => b,
            Err(e) => {
                println!("area {area:02}: splat build failed ({e})");
                continue;
            }
        };
        println!(
            "area {area:02}: build {:.0} ms, albedo {} px x {} layers ({} mips), normals {}",
            build.build_ms,
            build.albedo.size,
            build.albedo.layers,
            build.albedo.mip_levels,
            build.normals.as_ref().map_or("none".into(), |n| format!("{} px", n.size))
        );
        for s in build.slots.iter().flatten() {
            println!(
                "  slot {} {:<28} {:>4}x{:<4} {:<8} tiling {:>4} -> {:.3}/m (tile {:>5.1} m) cliff {} coverage {:>5.1} % mean_lum {:.3}",
                s.slot,
                s.texture_name,
                s.source_size.0,
                s.source_size.1,
                if s.improved.is_some() { "improved" } else { "original" },
                s.tiling,
                pla::game::terrain_splat::repeats_per_metre(s.tiling),
                1.0 / pla::game::terrain_splat::repeats_per_metre(s.tiling),
                s.cliff,
                s.coverage * 100.0,
                s.mean_luminance
            );
        }

        // Safe start, as the viewer chooses it.
        let nav = AreaNavigation::load(&bin, area).ok();
        let start: Option<Vec2> = nav.as_ref().and_then(|n| {
            let m = n.primary();
            let c = components(m);
            let marks = landmarks_for_area(area, &spawn_tsv, &place_tsv);
            choose_safe_spawn(m, &c, &marks, &SpawnConfig::default()).map(|s| s.xz)
        });
        let Some(start) = start else {
            println!("  no safe start");
            continue;
        };
        let cliff_slots: Vec<usize> = build.slots.iter().flatten().filter(|s| s.cliff).map(|s| s.slot).collect();
        let (w, sw, sh) = layer_weights(&t).expect("weights");
        let extent = t.world_extent[0];
        let h = |x: f32, z: f32| t.sample_world_height(x, z).unwrap_or(0.0);
        let mut best: Option<(f32, [f32; 3], f32, f32)> = None;
        let step = 2.0;
        let mut z = (start.y - CLIFF_MAX_M).max(1.0);
        while z < (start.y + CLIFF_MAX_M).min(extent - 1.0) {
            let mut x = (start.x - CLIFF_MAX_M).max(1.0);
            while x < (start.x + CLIFF_MAX_M).min(extent - 1.0) {
                let d = Vec2::new(x, z).distance(start);
                if (CLIFF_MIN_M..CLIFF_MAX_M).contains(&d) {
                    let gx = (h(x + 1.0, z) - h(x - 1.0, z)) / 2.0;
                    let gz = (h(x, z + 1.0) - h(x, z - 1.0)) / 2.0;
                    let g = (gx * gx + gz * gz).sqrt();
                    let slope = g.atan().to_degrees();
                    let tx = ((x / extent * sw as f32) as usize).min(sw as usize - 1);
                    let ty = ((z / extent * sh as f32) as usize).min(sh as usize - 1);
                    let cw: f32 = cliff_slots.iter().map(|k| w[ty * sw as usize + tx][*k]).sum();
                    if slope > CLIFF_MIN_SLOPE_DEG && cw > CLIFF_MIN_WEIGHT && g > 0.0 {
                        // Downhill direction = where the camera stands.
                        let dir = Vec2::new(-gx, -gz) / g;
                        let cam = Vec2::new(x, z) + dir * CAMERA_DISTANCE;
                        let relief = h(x, z) - h(cam.x, cam.y);
                        if relief > 4.0 {
                            let score = relief.min(25.0) - d * 0.05;
                            if best.is_none_or(|b| score > b.0) {
                                let yaw = dir.x.atan2(dir.y).to_degrees();
                                // Look at the middle of the face from slightly above: the focus is
                                // half way between the face point and the ground under the camera.
                                let focus_y = (h(x, z) + h(cam.x, cam.y)) * 0.5 + 1.0;
                                best = Some((score, [x, focus_y, z], yaw, CAMERA_PITCH_DEG));
                            }
                        }
                    }
                }
                x += step;
            }
            z += step;
        }
        match best {
            Some((_, f, yaw, pitch)) => {
                println!("  start ({:.1}, {:.1}); cliff face at ({:.1}, {:.1}, {:.1}) yaw {:.0} pitch {:.1}", start.x, start.y, f[0], f[1], f[2], yaw, pitch);
                json.push(format!(
                    "{{\"area\":{area},\"start\":[{:.2},{:.2}],\"cliff\":{{\"focus\":[{:.2},{:.2},{:.2}],\"yaw\":{:.1},\"pitch\":{:.1},\"distance\":{CAMERA_DISTANCE}}},\"build_ms\":{:.0}}}",
                    start.x, start.y, f[0], f[1], f[2], yaw, pitch, build.build_ms
                ));
            }
            None => {
                println!("  start ({:.1}, {:.1}); no cliff face within {CLIFF_MAX_M} m", start.x, start.y);
                json.push(format!(
                    "{{\"area\":{area},\"start\":[{:.2},{:.2}],\"cliff\":null,\"build_ms\":{:.0}}}",
                    start.x, start.y, build.build_ms
                ));
            }
        }
    }
    if let Some(out) = get("--json") {
        std::fs::write(&out, format!("[\n{}\n]\n", json.join(",\n"))).expect("write json");
        println!("wrote {out}");
    }
}
