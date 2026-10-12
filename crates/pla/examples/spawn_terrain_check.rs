//! Cross-check wild spawner positions (data/field_spawn_table.tsv) against terrain heightmaps.
//!
//! Usage: `cargo run --release -p pla --example spawn_terrain_check -- <romfs_patched/bin> [data/field_spawn_table.tsv]`
//! Prints, for every (spawn area x terrain area) pair, how many distinct spawners lie inside
//! the terrain extent and how far spawner y is from `sample_world_height`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pla::assets::terrain::TerrainArea;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bin_root = PathBuf::from(args.get(1).expect("bin root"));
    let tsv = args.get(2).map(String::as_str).unwrap_or("data/field_spawn_table.tsv");
    // distinct spawners per top-level data area (subareas folded into their parent ha_areaNN)
    let mut spawners: BTreeMap<String, BTreeMap<String, [f32; 3]>> = BTreeMap::new();
    for line in std::fs::read_to_string(tsv).expect("tsv").lines() {
        if line.starts_with('#') || line.starts_with("area:") {
            continue;
        }
        let c: Vec<&str> = line.split('\t').collect();
        let p = [c[2], c[3], c[4]].map(|v| v.parse::<f32>().unwrap());
        spawners
            .entry(c[0][..9].to_owned())
            .or_default()
            .insert(format!("{}/{}", c[0], c[1]), p);
    }
    for t in 0..=5u8 {
        let area = match TerrainArea::load(&bin_root, t) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("terrain {t}: {e}");
                continue;
            }
        };
        let (ex, ez) = (area.world_extent[0], area.world_extent[1]);
        let (mut sx, mut sz, mut n) = (0.0f64, 0.0f64, 0usize);
        for i in &area.instances {
            sx += i.matrix[3][0] as f64;
            sz += i.matrix[3][2] as f64;
            n += 1;
        }
        println!(
            "terrain ha_area{t:02}: extent {ex}x{ez} height_scale {} instances {n} centroid ({:.0},{:.0})",
            area.world_height,
            sx / n.max(1) as f64,
            sz / n.max(1) as f64
        );
        for (sa, map) in &spawners {
            let (mut inside, mut sum, mut within3, mut counted) = (0, 0.0f64, 0, 0);
            for p in map.values() {
                if p[0] >= 0.0 && p[0] <= ex && p[2] >= 0.0 && p[2] <= ez {
                    inside += 1;
                }
                if let Some(h) = area.sample_world_height(p[0], p[2]) {
                    let d = (h - p[1]).abs() as f64;
                    sum += d;
                    counted += 1;
                    if d <= 3.0 {
                        within3 += 1;
                    }
                }
            }
            println!(
                "  spawn {sa}: spawners {:4} inside {:4} mean|dy| {:7.2} within3 {:4}",
                map.len(),
                inside,
                sum / counted.max(1) as f64,
                within3
            );
        }
    }
}
