//! Cross-check object placements (data/field_object_placements.tsv) against the terrain heightmaps.
//!
//! Usage: `cargo run --release -p pla --example placement_terrain_check -- <romfs_patched/bin> [tsv]`
//! For every data area (top-level `ha_areaNN` separate from its `_sMM` subareas) prints how many placements lie
//! inside the matching terrain extent and how far placement y is from `sample_world_height`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pla::assets::terrain::TerrainArea;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bin_root = PathBuf::from(args.get(1).expect("bin root"));
    let tsv = args.get(2).map(String::as_str).unwrap_or("data/field_object_placements.tsv");
    // (parent area, is_subarea) -> placements (x, y, z, model_status)
    let mut rows: BTreeMap<(String, bool), Vec<[f32; 3]>> = BTreeMap::new();
    for line in std::fs::read_to_string(tsv).expect("tsv").lines() {
        if line.starts_with('#') || line.starts_with("area:") {
            continue;
        }
        let c: Vec<&str> = line.split('\t').collect();
        let p = [c[6], c[7], c[8]].map(|v| v.parse::<f32>().unwrap());
        rows.entry((c[0][..9].to_owned(), c[0].len() > 9)).or_default().push(p);
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
        let own = format!("ha_area{t:02}");
        for ((pa, sub), pts) in &rows {
            if *pa != own {
                continue;
            }
            let (mut inside, mut sum, mut within3, mut counted) = (0, 0.0f64, 0, 0);
            for p in pts {
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
                "{pa}{} placements {:4} inside {:4} mean|dy| {:7.2} within3 {:4} ({:.1}%)",
                if *sub { " subareas" } else { " main    " },
                pts.len(),
                inside,
                sum / counted.max(1) as f64,
                within3,
                100.0 * within3 as f64 / counted.max(1) as f64
            );
        }
    }
}
