//! Bake the terrain tangent-space normal map per area (headless, no window).
//!
//! Usage:
//!   terrain_normals_bake --bin-root BIN --area N|all --bake WORK_ROOT
//!                        [--size 2048] [--mean] [--skip-if-unchanged]
//!
//! Writes `WORK_ROOT/cache/terrain_normals/areaNN.png` (RGB8, tangent space) plus
//! the invalidation sidecar `areaNN.normals.json`, and prints per-area timing and
//! normal statistics (mean/min/max reconstructed z, tilted share).
use pla::assets::terrain::TerrainArea;
use pla::assets::terrain_normals::{
    bake_normals_to_cache, load_baked_normals, normal_stats, normals_cache_path, BakeOutcome,
    NormalBakeOptions, DEFAULT_BAKE_SIZE,
};
use std::{env, path::Path};

fn main() {
    let a: Vec<String> = env::args().collect();
    let get = |k: &str| a.iter().position(|x| x == k).map(|i| a[i + 1].clone());
    let bin = get("--bin-root").expect("--bin-root");
    let Some(work) = get("--bake") else {
        eprintln!(
            "usage: terrain_normals_bake --bin-root BIN --area N|all --bake WORK_ROOT \
             [--size 2048] [--mean] [--skip-if-unchanged]"
        );
        return;
    };
    let size: u32 = get("--size").map(|v| v.parse().unwrap()).unwrap_or(DEFAULT_BAKE_SIZE);
    let areas: Vec<u8> = if a.iter().any(|x| x == "all") { (0..=6).collect() } else { vec![area_arg(&a)] };
    let opts = NormalBakeOptions { size, detail: !a.iter().any(|x| x == "--mean"), threads: 0 };
    let skip = a.iter().any(|x| x == "--skip-if-unchanged");
    for area in areas {
        let Ok(terrain) = TerrainArea::load(&bin, area) else {
            eprintln!("area{area:02}: [skip] terrain pack not loaded");
            continue;
        };
        let started = std::time::Instant::now();
        match bake_normals_to_cache(&terrain, Path::new(&work), &opts, skip) {
            Ok(BakeOutcome::Skipped) => {
                eprintln!("area{area:02}: unchanged, skipped ({:.2}s)", started.elapsed().as_secs_f32())
            }
            Ok(BakeOutcome::Baked) => {
                let secs = started.elapsed().as_secs_f32();
                match load_baked_normals(normals_cache_path(&work, area)) {
                    Ok(image) => {
                        let s = normal_stats(&image);
                        eprintln!(
                            "area{area:02}: wrote {} ({size}px, {secs:.1}s) \
                             mean_z={:.4} min_z={:.4} max_z={:.4} tilted={:.1}%",
                            normals_cache_path(&work, area).display(),
                            s.mean_z,
                            s.min_z,
                            s.max_z,
                            s.tilted_pct
                        );
                    }
                    Err(e) => eprintln!("area{area:02}: wrote the PNG but stats failed: {e}"),
                }
            }
            Err(e) => eprintln!("area{area:02}: [skip] {e}"),
        }
    }
}

fn area_arg(a: &[String]) -> u8 {
    a.iter().position(|x| x == "--area").map(|i| a[i + 1].parse().unwrap()).expect("--area")
}
