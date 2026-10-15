//! Derive climbable cliff surfaces of the main areas from the real terrain heightmaps and the
//! game's decoded climb-angle rule, and write `data/field_climb_surfaces.tsv`.
//!
//! Why this is the real data: the original game has no climbable *objects* (no ladder/rope model
//! exists in any field pack, see `reports/data-progress/ropes_ladders_unknown.md`). Climbing is
//! the ride-Sneasler wall climb: the game checks the wall angle against the values in
//! `ride_nyuura_config` — `climbing_angle_lower_limit_deg` 45, `climbing_angle_start_border_deg`
//! 50, `climbing_angle_upper_limit_deg` 150 (`data/ride_params.tsv`, values VERIFIED). The
//! surfaces themselves are the terrain cliffs, so they are derived here from the decoded
//! heightmaps with that decoded rule.
//!
//! Method (headless, no window):
//! 1. For every main area (00..=06) load `TerrainArea::load` and scan a 1 m grid.
//! 2. A scan cell is *climb-startable* when its slope from horizontal is within
//!    `[50, 150]` degrees (the start border is the PROBABLE reading of that parameter; the
//!    upper limit can never bind on a heightfield — terrain has no overhangs, slope <= 90).
//! 3. Climb-startable cells are grouped into connected patches (4-neighbourhood) with a cell
//!    budget so a kilometres-long cliff band becomes several climb surfaces.
//! 4. Patches keep their foot (`base_x`, `base_z`, lowest scan cell), their `top_y` and the
//!    horizontal grab radius; rows with a height below `--min-height` (default 2 m) or fewer
//!    than `--min-cells` (default 6) are dropped as noise.
//!
//! Usage: `cargo run --release -p pla --example climb_surface_report -- [OUT_TSV]`
//! (`PLA_BIN_ROOT` overrides the game root; `--step`, `--min-height`, `--min-cells`,
//! `--max-cells` tune the scan).

use std::collections::VecDeque;
use std::path::PathBuf;

use pla::assets::terrain::TerrainArea;

fn bin_root() -> PathBuf {
    std::env::var_os("PLA_BIN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into())
}

/// The decoded ride-Sneasler climb angles, degrees from horizontal.
pub const CLIMB_ANGLE_LOWER_LIMIT_DEG: f32 = 45.0;
pub const CLIMB_ANGLE_START_BORDER_DEG: f32 = 50.0;
pub const CLIMB_ANGLE_UPPER_LIMIT_DEG: f32 = 150.0;

/// One derived climb surface.
#[derive(Debug, Clone, PartialEq)]
pub struct ClimbPatch {
    pub cells: u32,
    pub area_m2: f32,
    pub base: [f32; 2],
    pub bottom_y: f32,
    /// Horizontal position of the highest scan cell (the crest where the avatar steps off).
    pub top: [f32; 2],
    pub top_y: f32,
    pub radius_m: f32,
    pub slope_min_deg: f32,
    pub slope_max_deg: f32,
}

impl ClimbPatch {
    pub fn height_m(&self) -> f32 {
        self.top_y - self.bottom_y
    }
}

/// Scan one area's terrain; returns the kept patches and scan statistics.
pub fn scan_area(
    area: &TerrainArea,
    step: f32,
    min_height: f32,
    min_cells: u32,
    max_cells: u32,
) -> (Vec<ClimbPatch>, ScanStats) {
    let (ex, ez) = (area.world_extent[0], area.world_extent[1]);
    let nx = (ex / step).floor() as usize + 1;
    let nz = (ez / step).floor() as usize + 1;
    let h = |i: usize, j: usize| -> Option<f32> {
        area.sample_world_height(i as f32 * step, j as f32 * step)
    };
    let mut slope = vec![f32::NAN; nx * nz];
    let mut stats = ScanStats::default();
    for j in 0..nz {
        for i in 0..nx {
            stats.cells += 1;
            let (Some(west), Some(east), Some(north), Some(south)) = (
                if i > 0 { h(i - 1, j) } else { None },
                if i + 1 < nx { h(i + 1, j) } else { None },
                if j > 0 { h(i, j - 1) } else { None },
                if j + 1 < nz { h(i, j + 1) } else { None },
            ) else {
                continue;
            };
            let gx = (east - west) / (2.0 * step);
            let gz = (south - north) / (2.0 * step);
            let s = (gx * gx + gz * gz).sqrt().atan().to_degrees();
            if (CLIMB_ANGLE_START_BORDER_DEG..=CLIMB_ANGLE_UPPER_LIMIT_DEG).contains(&s) {
                stats.climbable_cells += 1;
            }
            slope[j * nx + i] = s;
        }
    }
    // Connected patches with a cell budget: one long cliff band becomes several surfaces.
    let mut visited = vec![false; nx * nz];
    let mut patches = Vec::new();
    for seed in 0..nx * nz {
        if visited[seed] {
            continue;
        }
        let s0 = slope[seed];
        if !(CLIMB_ANGLE_START_BORDER_DEG..=CLIMB_ANGLE_UPPER_LIMIT_DEG).contains(&s0) {
            continue;
        }
        stats.patches_raw += 1;
        let mut queue = VecDeque::from([seed]);
        visited[seed] = true;
        let mut cells: Vec<usize> = Vec::new();
        while let Some(c) = queue.pop_front() {
            cells.push(c);
            if cells.len() as u32 >= max_cells {
                continue;
            }
            let (ci, cj) = (c % nx, c / nx);
            let mut push = |i: usize, j: usize, queue: &mut VecDeque<usize>| {
                let k = j * nx + i;
                if !visited[k]
                    && (CLIMB_ANGLE_START_BORDER_DEG..=CLIMB_ANGLE_UPPER_LIMIT_DEG)
                        .contains(&slope[k])
                {
                    visited[k] = true;
                    queue.push_back(k);
                }
            };
            if ci > 0 {
                push(ci - 1, cj, &mut queue);
            }
            if ci + 1 < nx {
                push(ci + 1, cj, &mut queue);
            }
            if cj > 0 {
                push(ci, cj - 1, &mut queue);
            }
            if cj + 1 < nz {
                push(ci, cj + 1, &mut queue);
            }
        }
        // Patch stats.
        let mut bottom = f32::INFINITY;
        let mut top = f32::NEG_INFINITY;
        let (mut bx, mut bz) = (0usize, 0usize);
        let (mut tx, mut tz) = (0usize, 0usize);
        let (mut smin, mut smax) = (f32::INFINITY, f32::NEG_INFINITY);
        for &c in &cells {
            let (i, j) = (c % nx, c / nx);
            let Some(y) = h(i, j) else { continue };
            if y < bottom {
                bottom = y;
                (bx, bz) = (i, j);
            }
            // Highest cell wins; ties keep the one closest to the base (smallest step-off).
            if y > top
                || (y == top
                    && (i as f32 - bx as f32).powi(2) + (j as f32 - bz as f32).powi(2)
                        < (tx as f32 - bx as f32).powi(2) + (tz as f32 - bz as f32).powi(2))
            {
                top = y;
                (tx, tz) = (i, j);
            }
            smin = smin.min(slope[c]);
            smax = smax.max(slope[c]);
        }
        if !bottom.is_finite() || !top.is_finite() {
            continue;
        }
        let base = [bx as f32 * step, bz as f32 * step];
        let crest = [tx as f32 * step, tz as f32 * step];
        let mut reach: f32 = 0.0;
        for &c in &cells {
            let (i, j) = (c % nx, c / nx);
            let d = ((i as f32 * step - base[0]).powi(2) + (j as f32 * step - base[1]).powi(2))
                .sqrt();
            reach = reach.max(d);
        }
        let patch = ClimbPatch {
            cells: cells.len() as u32,
            area_m2: cells.len() as f32 * step * step,
            base,
            bottom_y: bottom,
            top: crest,
            top_y: top,
            radius_m: reach.clamp(2.0, 6.0),
            slope_min_deg: smin,
            slope_max_deg: smax,
        };
        if patch.height_m() >= min_height && patch.cells >= min_cells {
            patches.push(patch);
        } else {
            stats.patches_dropped += 1;
        }
    }
    patches.sort_by(|a, b| b.height_m().total_cmp(&a.height_m()));
    (patches, stats)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ScanStats {
    pub cells: u64,
    pub climbable_cells: u64,
    pub patches_raw: u64,
    pub patches_dropped: u64,
}

fn main() {
    let mut out = PathBuf::from("data/field_climb_surfaces.tsv");
    let mut step = 1.0f32;
    let mut min_height = 2.0f32;
    let mut min_cells = 6u32;
    let mut max_cells = 120u32;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--step" => {
                step = args[i + 1].parse().expect("step");
                i += 2;
            }
            "--min-height" => {
                min_height = args[i + 1].parse().expect("min-height");
                i += 2;
            }
            "--min-cells" => {
                min_cells = args[i + 1].parse().expect("min-cells");
                i += 2;
            }
            "--max-cells" => {
                max_cells = args[i + 1].parse().expect("max-cells");
                i += 2;
            }
            other => {
                out = PathBuf::from(other);
                i += 1;
            }
        }
    }
    let bin = bin_root();
    let mut lines: Vec<String> = Vec::new();
    let mut total_kept = 0u64;
    for area_idx in 0..=6u8 {
        let area = match TerrainArea::load(&bin, area_idx) {
            Ok(a) => a,
            Err(e) => {
                println!("area {area_idx:02}: terrain not loaded ({e}); skipped");
                continue;
            }
        };
        let (patches, stats) = scan_area(&area, step, min_height, min_cells, max_cells);
        total_kept += patches.len() as u64;
        let tallest = patches.first().map(|p| p.height_m()).unwrap_or(0.0);
        println!(
            "area {area_idx:02}: {} cells, {} climbable ({:.2}%), {} raw patches, {} kept, {} dropped, tallest {:.1} m",
            stats.cells,
            stats.climbable_cells,
            stats.climbable_cells as f64 * 100.0 / stats.cells.max(1) as f64,
            stats.patches_raw,
            patches.len(),
            stats.patches_dropped,
            tallest,
        );
        let name = format!("ha_area{area_idx:02}");
        for (k, p) in patches.iter().enumerate() {
            lines.push(format!(
                "{}\t{}\t{}\t{:.1}\t{:.1}\t{:.1}\t{:.2}\t{:.1}\t{:.1}\t{:.2}\t{:.2}\t{:.1}\t{:.1}\t{:.1}",
                name,
                k as u32,
                p.cells,
                p.area_m2,
                p.base[0],
                p.base[1],
                p.bottom_y,
                p.top[0],
                p.top[1],
                p.top_y,
                p.height_m(),
                p.radius_m,
                p.slope_min_deg,
                p.slope_max_deg,
            ));
        }
    }
    let header = "# sheet: data/field_climb_surfaces\n\
# version: 2\n\
# generator: crates/pla/examples/climb_surface_report.rs\n\
# target: Pokémon Legends: Arceus v262144 climbable cliff surfaces (main areas 00..=06)\n\
# note: derived from the real terrain heightmaps (archive/field/terrain/ha_areaNN.gfpak) with the decoded\n\
# note: ride-Sneasler rule (data/ride_params.tsv ride_nyuura_config, values VERIFIED): lower limit 45 deg,\n\
# note: start border 50 deg, upper limit 150 deg. A 1 m scan cell is climb-startable at slope in [50, 150] deg\n\
# note: from horizontal (the start-border reading is PROBABLE; heights fields cannot exceed 90 deg).\n\
# note: patches are budget-limited connected groups (default <= 120 cells); base_* is the lowest scan cell\n\
# note: (the foot of the cliff); top_x/top_z is the highest scan cell (the crest). The avatar holds
# note: base_xz while climbing and steps off at top_xz (port mapping of a slope into a climb, ASSUMED).\n\
# note: radius_m is the horizontal grab reach from the base (2..6 m). Filter: height >= 2 m, cells >= 6.\n\
area:string*\tpatch:u32\tcells:u32\tarea_m2:f32\tbase_x:f32\tbase_z:f32\tbottom_y:f32\ttop_x:f32\ttop_z:f32\ttop_y:f32\theight_m:f32\tradius_m:f32\tslope_min_deg:f32\tslope_max_deg:f32\n";
    std::fs::write(&out, format!("{header}{}", lines.join("\n") + "\n")).expect("write sheet");
    println!("{} climb surfaces -> {}", total_kept, out.display());
}
