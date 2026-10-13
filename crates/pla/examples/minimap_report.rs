//! Generate the minimap of real areas without opening a window and report measured
//! size/time (Brief 15).
//!
//! Usage: `minimap_report [--areas 0,1,5] [--size 1024] [--bin-root BIN] [--game-map]
//! [--write WORK_ROOT] [--skip-if-unchanged]`
//!
//! With `--write` the map and its sidecar are stored under `WORK_ROOT/minimap/` (default
//! `PLA_CACHE_ROOT`, else the main checkout's `work/cache`), the same layout the viewer
//! reads. Without it nothing is written.

use std::path::PathBuf;

use pla::assets::terrain::TerrainArea;
use pla::assets::terrain_albedo::{albedo_cache_path, load_baked_albedo};
use pla::game::minimap::{
    bake_map_to_cache, build_area_map, candidate_cache_roots, load_game_map, minimap_cache_path,
    DEFAULT_CACHE_ROOT, DEFAULT_MAP_SIZE,
};

fn bin_root_arg(args: &[String]) -> Option<PathBuf> {
    let get = |key: &str| {
        args.iter()
            .position(|a| a == key)
            .map(|i| args[i + 1].clone())
    };
    get("--bin-root")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("PLA_BIN_ROOT").map(PathBuf::from))
        .or_else(|| {
            let default =
                PathBuf::from("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin");
            default
                .join("archive/field/terrain/ha_area00.gfpak")
                .is_file()
                .then_some(default)
        })
}

fn cached_albedo(area: u8) -> Option<(PathBuf, pla::assets::bntx::TextureData)> {
    for root in candidate_cache_roots() {
        for path in [
            root.join("terrain_albedo")
                .join(format!("area{area:02}.png")),
            albedo_cache_path(&root, area),
        ] {
            if path.is_file() {
                if let Ok(tex) = load_baked_albedo(&path) {
                    return Some((path, tex));
                }
            }
        }
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |key: &str| {
        args.iter()
            .position(|a| a == key)
            .map(|i| args[i + 1].clone())
    };
    let Some(bin_root) = bin_root_arg(&args) else {
        eprintln!("[skip] game bin root not found (pass --bin-root or set PLA_BIN_ROOT)");
        std::process::exit(0);
    };
    let areas: Vec<u8> = get("--areas")
        .map(|v| v.split(',').filter_map(|a| a.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![0, 1, 5]);
    let size: u32 = get("--size")
        .map(|v| v.parse().unwrap())
        .unwrap_or(DEFAULT_MAP_SIZE);
    let write_root = args
        .iter()
        .position(|a| a == "--write")
        .and_then(|i| args.get(i + 1).cloned())
        .map(PathBuf::from)
        .or_else(|| std::env::var("PLA_CACHE_ROOT").ok().map(PathBuf::from));
    let game_map = args.iter().any(|a| a == "--game-map");

    let mut total_ms = 0.0f64;
    let mut built = 0usize;
    for area in areas {
        let Ok(area_data) = TerrainArea::load(&bin_root, area) else {
            eprintln!("[skip] area {area:02}: terrain not loadable");
            continue;
        };
        let albedo = cached_albedo(area);
        if albedo.is_none() {
            eprintln!("[skip] area {area:02}: no baked albedo in the cache (heightmap-only map)");
        }
        let started = std::time::Instant::now();
        let map = build_area_map(&area_data, albedo.as_ref().map(|(_, t)| t), size)
            .expect("minimap build");
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        total_ms += ms;
        built += 1;
        let pixels = (map.width * map.height) as f64;
        let mut water = 0usize;
        let mut sum = 0.0f64;
        for p in map.rgba.as_chunks::<4>().0 {
            if p[2] as u32 > p[0] as u32 + 20 && p[2] as u32 > p[1] as u32 + 20 {
                water += 1;
            }
            sum += p[0] as f64 + p[1] as f64 + p[2] as f64;
        }
        println!(
            "area{area:02}: {}x{} extent {:?} H={:.0}m albedo={} water={:.1}% level={:?}m mean={:.0} time={:.0} ms",
            map.width,
            map.height,
            area_data.world_extent,
            area_data.world_height,
            albedo.is_some(),
            water as f64 / pixels * 100.0,
            pla::game::minimap::estimate_water_level_m(&area_data.heightmap, area_data.world_height),
            sum / (pixels * 3.0),
            ms,
        );
        if game_map {
            match load_game_map(&bin_root, area) {
                Ok(image) => println!(
                    "  game map map_lmap_pic_{area:02}: {}x{}",
                    image.width, image.height
                ),
                Err(e) => println!("  game map area {area:02}: [skip] {e}"),
            }
        }
        if let Some(work_root) = &write_root {
            let skip = args.iter().any(|a| a == "--skip-if-unchanged");
            match bake_map_to_cache(
                &area_data,
                albedo.as_ref().map(|(_, t)| t),
                work_root,
                size,
                skip,
            ) {
                Ok((outcome, ms)) => println!(
                    "  wrote {} ({:?}, {:.0} ms)",
                    minimap_cache_path(work_root, area).display(),
                    outcome,
                    ms
                ),
                Err(e) => eprintln!("  write area {area:02} failed: {e}"),
            }
        }
    }
    println!(
        "minimap_report: {built} area maps of {size}x{size}, total generation {total_ms:.0} ms (default cache root {DEFAULT_CACHE_ROOT})"
    );
}
