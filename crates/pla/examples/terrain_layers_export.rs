//! Export every ground layer texture (albedo + normal) of the terrain areas as PNG with unique
//! names, write the typed index `data/terrain_layer_index.tsv` and print the ranking of which
//! layers are worth improving first. No window.
//!
//! Usage: terrain_layers_export --bin-root BIN --work WORK_ROOT [--index data/terrain_layer_index.tsv]
//!   PNGs go to WORK_ROOT/cache/terrain_layers/<unique_name>.png (same name+content = one file;
//!   same name with different content in another area gets the suffix `__aNN`).
//!   Run from the repository root (reads data/field_spawn_table.tsv for the start landmarks).
use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy::math::Vec2;
use pla::assets::bntx::{Bntx, TextureData};
use pla::assets::gfpak::GfPak;
use pla::assets::navmesh::AreaNavigation;
use pla::assets::terrain::TerrainArea;
use pla::assets::terrain_albedo::{layer_weights, texture_hash, texture_name_of};
use pla::game::nav_report::{choose_safe_spawn, components};
use pla::game::spawn_point::{landmarks_for_area, SpawnConfig};

const AREA_NAMES: [&str; 7] = [
    "Aldea Joya",
    "Campos Obsidiana",
    "Pantanal Carmesí",
    "Costa Cobalto",
    "Cordillera Corona",
    "Tierras Heladas Alabastro",
    "zona 06",
];
/// Radius (m) around the safe start used for the "near the player" weight.
const NEAR_START_M: f32 = 60.0;
/// Navmesh sampling grid (cells per side) for the walkable ("land") part.
const GRID: usize = 256;

#[derive(Default)]
struct Row {
    texture_name: String,
    kind: &'static str,
    width: u32,
    height: u32,
    format: String,
    /// area -> (slots joined by `+` (a texture can fill several slots), first tiling, coverage %, whole-area %, near-start %)
    areas: BTreeMap<u8, (String, f32, f32, f32, f32)>,
    m2: f32,
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let get = |k: &str| a.iter().position(|x| x == k).map(|i| a[i + 1].clone());
    let bin = PathBuf::from(get("--bin-root").expect("--bin-root"));
    let work = PathBuf::from(get("--work").expect("--work"));
    let index_path = get("--index").unwrap_or_else(|| "data/terrain_layer_index.tsv".into());
    let out_dir = work.join("cache/terrain_layers");
    std::fs::create_dir_all(&out_dir).unwrap();
    let spawn_tsv = std::fs::read_to_string("data/field_spawn_table.tsv").unwrap_or_default();
    let place_tsv = std::fs::read_to_string("data/field_object_placements.tsv").unwrap_or_default();

    // unique-name bookkeeping: texture name -> list of (content hash, unique name)
    let mut seen: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    let mut starts = Vec::new();

    for area in 0u8..=6 {
        let t = TerrainArea::load(&bin, area).expect("load area");
        let extent = t.world_extent[0];
        let (w, sw, sh) = layer_weights(&t).expect("weights");
        // land mask: walkable navmesh cells (areas 00-05); area 06 has no navmesh -> everything
        let nav = if area <= 5 { AreaNavigation::load(&bin, area).ok() } else { None };
        let mut land = vec![false; GRID * GRID];
        for gy in 0..GRID {
            for gx in 0..GRID {
                let (x, z) = ((gx as f32 + 0.5) * extent / GRID as f32, (gy as f32 + 0.5) * extent / GRID as f32);
                land[gy * GRID + gx] = nav.as_ref().map_or(true, |n| n.is_walkable(x, z));
            }
        }
        let land_cells = land.iter().filter(|v| **v).count().max(1);
        let cell_m2 = (extent / GRID as f32).powi(2);
        let land_m2 = land_cells as f32 * cell_m2;
        let texel = |x: f32, z: f32| -> [f32; 8] {
            let tx = ((x / extent * sw as f32) as usize).min(sw as usize - 1);
            let ty = ((z / extent * sh as f32) as usize).min(sh as usize - 1);
            w[ty * sw as usize + tx]
        };
        let mut land_sum = [0.0f32; 8];
        for gy in 0..GRID {
            for gx in 0..GRID {
                if land[gy * GRID + gx] {
                    let p = texel((gx as f32 + 0.5) * extent / GRID as f32, (gy as f32 + 0.5) * extent / GRID as f32);
                    (0..8).for_each(|k| land_sum[k] += p[k]);
                }
            }
        }
        let mut all_sum = [0.0f32; 8];
        let step = 4usize;
        let mut all_n = 0usize;
        for ty in (0..sh as usize).step_by(step) {
            for tx in (0..sw as usize).step_by(step) {
                let p = w[ty * sw as usize + tx];
                (0..8).for_each(|k| all_sum[k] += p[k]);
                all_n += 1;
            }
        }
        // safe start (areas 00-05)
        let start: Option<Vec2> = nav.as_ref().and_then(|n| {
            let m = n.primary();
            let c = components(m);
            let marks = landmarks_for_area(area, &spawn_tsv, &place_tsv);
            choose_safe_spawn(m, &c, &marks, &SpawnConfig::default()).map(|s| s.xz)
        });
        if let Some(s) = start {
            starts.push((area, s));
        }
        let mut near_sum = [0.0f32; 8];
        let mut near_n = 0usize;
        if let Some(s) = start {
            for ty in 0..sh as usize {
                for tx in 0..sw as usize {
                    let (x, z) = ((tx as f32 + 0.5) * extent / sw as f32, (ty as f32 + 0.5) * extent / sh as f32);
                    if Vec2::new(x, z).distance(s) <= NEAR_START_M {
                        let p = w[ty * sw as usize + tx];
                        (0..8).for_each(|k| near_sum[k] += p[k]);
                        near_n += 1;
                    }
                }
            }
        }
        let pct = |sum: f32, n: usize| if n == 0 { 0.0 } else { (100.0 * sum / n as f32).max(0.0) };

        // formats straight from the pack's BNTX headers
        let pack_bytes = std::fs::read(&t.source_path).unwrap();
        let pack = GfPak::parse(&pack_bytes).unwrap();
        let prefix = format!("bin/field/terrain/ha_area{area:02}/terrain/");
        let format_of = |file: &str| -> String {
            let stem = texture_name_of(file);
            let Ok(Some(i)) = pack.entry_index_by_path(&format!("{prefix}{stem}.bntx")) else { return "unknown".into() };
            let d = pack.decoded_data(&pack_bytes, &pack.entries[i]).unwrap();
            let b = Bntx::parse(&d).unwrap();
            b.textures.iter().find(|x| x.name == stem).map_or("unknown".into(), |x| x.format.name().to_string())
        };

        let material = t.material.as_ref().expect("material");
        let mut items: Vec<(String, &'static str, usize, TextureData)> = Vec::new();
        for (file, role) in &material.slots.textures {
            if let Some(k) = role.strip_prefix("SplatColor").and_then(|d| d.parse::<usize>().ok()) {
                if let Some(tex) = t.ground_textures.get(file) {
                    items.push((file.clone(), "alb", k, tex.clone()));
                }
            }
        }
        for (file, role, tex) in t.load_layer_normals().expect("normals") {
            let k = role.strip_prefix("SplatNormal").and_then(|d| d.parse::<usize>().ok()).unwrap_or(0);
            items.push((file, "nrm", k, tex));
        }
        for (file, kind, k, tex) in items {
            let name = texture_name_of(&file);
            let hash = texture_hash(&tex);
            let list = seen.entry(name.clone()).or_default();
            let unique = match list.iter().find(|(h, _)| *h == hash) {
                Some((_, u)) => u.clone(),
                None => {
                    let u = if list.is_empty() { name.clone() } else { format!("{name}__a{area:02}") };
                    tex.save_png(&out_dir.join(format!("{u}.png"))).unwrap();
                    list.push((hash.clone(), u.clone()));
                    u
                }
            };
            let row = rows.entry(unique.clone()).or_insert_with(|| Row {
                texture_name: name.clone(),
                kind,
                width: tex.width,
                height: tex.height,
                format: format_of(&file),
                ..Default::default()
            });
            let tiling = material.splat_tiling.get(k).map_or(0.0, |s| s[0]);
            let cov = pct(land_sum[k.min(7)], land_cells);
            let all = pct(all_sum[k.min(7)], all_n);
            let near = pct(near_sum[k.min(7)], near_n);
            if kind == "alb" && area <= 5 {
                row.m2 += cov / 100.0 * land_m2;
            }
            let e = row.areas.entry(area).or_insert((String::new(), tiling, 0.0, 0.0, 0.0));
            if !e.0.is_empty() {
                e.0.push('+');
            }
            e.0 += &k.to_string();
            e.2 += cov;
            e.3 += all;
            e.4 += near;
        }
        eprintln!("area{area:02}: land {:.0} m2, start {:?}", land_m2, start);
    }

    // ranking over areas 00-05, albedo rows: half total land coverage, half weight near the starts
    let alb: Vec<&String> = rows.iter().filter(|(_, r)| r.kind == "alb").map(|(k, _)| k).collect();
    let total_m2: f32 = alb.iter().map(|k| rows[*k].m2).sum::<f32>().max(1.0);
    let n_start = starts.len().max(1) as f32;
    let near_total: f32 =
        alb.iter().map(|k| rows[*k].areas.iter().filter(|(a, _)| **a <= 5).map(|(_, v)| v.4).sum::<f32>()).sum::<f32>().max(1.0);
    let _ = n_start;
    let mut score: Vec<(f32, &String)> = alb
        .iter()
        .map(|k| {
            let r = &rows[*k];
            let near: f32 = r.areas.iter().filter(|(a, _)| **a <= 5).map(|(_, v)| v.4).sum();
            (0.5 * r.m2 / total_m2 + 0.5 * near / near_total, *k)
        })
        .collect();
    score.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap().then(x.1.cmp(y.1)));
    let rank: BTreeMap<&String, usize> = score.iter().enumerate().map(|(i, (_, k))| (*k, i + 1)).collect();

    let fmt_map = |r: &Row, f: &dyn Fn(&(String, f32, f32, f32, f32)) -> String| -> String {
        r.areas.iter().map(|(a, v)| format!("{a:02}:{}", f(v))).collect::<Vec<_>>().join(";")
    };
    let mut tsv = String::new();
    tsv += "# sheet: data/terrain_layer_index\n# version: 1\n# generator: crates/pla/examples/terrain_layers_export.rs\n";
    tsv += "# target: Pokémon Legends: Arceus update v262144 terrain ground layer textures (ha_areaNN.gfpak SplatColor/SplatNormal slots)\n";
    tsv += "# note: one row per unique texture (same name + same decoded content = one row; same name with other content gets `__aNN`).\n";
    tsv += "# note: per-area fields are `AA:value` joined by `;`. coverage_pct = mean normalised splat weight over the walkable (navmesh) part of\n";
    tsv += "# note: the area (area 06 has no navmesh: whole area); all_pct = whole area; near_start_pct = within 60 m of the safe start (areas 00-05).\n";
    tsv += "# note: coverage_m2_total = sum over areas 00-05 of coverage x walkable m2 (albedo only). priority_rank (albedo only) = 0.5 share of\n";
    tsv += "# note: total m2 + 0.5 share of near-start weight. Normal rows reuse the albedo slot of the same index. tiling = SplatST x (period 32/tiling m is a guess).\n";
    tsv += "unique_name:string*\ttexture_name:string\tkind:string\twidth:u32\theight:u32\tformat:string\tused_in_areas:string\tslots:string\ttiling:string\tcoverage_pct:string\tall_pct:string\tnear_start_pct:string\tcoverage_m2_total:f32\tpriority_rank:u32?\tplain_name:string\tpng_rel:string\n";
    for (u, r) in &rows {
        let areas = r.areas.keys().map(|a| format!("{a:02}")).collect::<Vec<_>>().join(";");
        tsv += &format!(
            "{u}\t{}\t{}\t{}\t{}\t{}\t{areas}\t{}\t{}\t{}\t{}\t{}\t{:.0}\t{}\t{}\twork/cache/terrain_layers/{u}.png\n",
            r.texture_name,
            r.kind,
            r.width,
            r.height,
            r.format,
            fmt_map(r, &|v| v.0.clone()),
            fmt_map(r, &|v| format!("{}", v.1)),
            fmt_map(r, &|v| format!("{:.1}", v.2)),
            fmt_map(r, &|v| format!("{:.1}", v.3)),
            fmt_map(r, &|v| format!("{:.1}", v.4)),
            if r.kind == "alb" { r.m2 } else { 0.0 },
            rank.get(u).map_or(String::new(), |n| n.to_string()),
            plain_name(&r.texture_name, r.kind),
        );
    }
    std::fs::write(&index_path, tsv).unwrap();
    eprintln!("wrote {index_path} ({} unique textures) and PNGs in {}", rows.len(), out_dir.display());

    println!("start points: {}", starts.iter().map(|(a, s)| format!("{a:02}=({:.0},{:.0})", s.x, s.y)).collect::<Vec<_>>().join(" "));
    println!("rank  score  m2_total  near%(sum)  size   unique_name  ->  plain name   (areas)");
    for (i, (sc, k)) in score.iter().enumerate() {
        let r = &rows[*k];
        let near: f32 = r.areas.iter().filter(|(a, _)| **a <= 5).map(|(_, v)| v.4).sum();
        println!(
            "{:>3}  {:.3}  {:>8.0}  {:>7.1}  {}x{}  {k} -> {}  [{}]",
            i + 1,
            sc,
            r.m2,
            near,
            r.width,
            r.height,
            plain_name(&r.texture_name, r.kind),
            r.areas.keys().map(|a| format!("{a:02}")).collect::<Vec<_>>().join(",")
        );
    }
}

/// Plain Spanish description derived from a texture name such as `ha_area01_grass01_alb`.
fn plain_name(texture: &str, kind: &str) -> String {
    let mut area: Option<usize> = None;
    let mut words: Vec<String> = Vec::new();
    for tok in texture.split('_') {
        if tok == "ha" || tok == "c" || tok == "alb" || tok == "nrm" {
            continue;
        }
        if let Some(n) = tok.strip_prefix("area").and_then(|d| d.parse::<usize>().ok()) {
            area = Some(n);
            continue;
        }
        let split = tok.find(|c: char| c.is_ascii_digit()).unwrap_or(tok.len());
        let (base, num) = tok.split_at(split);
        let es = match base {
            "grass" | "glass" => "hierba",
            "mountain" => "roca de montaña",
            "ground" => "suelo",
            "beach" => "playa",
            "sand" => "arena",
            "gravel" => "grava",
            "cliff" => "roca de acantilado",
            "rock" => "roca",
            "snow" => "nieve",
            "ice" => "hielo",
            "soil" | "dirt" => "tierra",
            "leaves" | "leaf" => "hojas",
            "mud" => "barro",
            "moss" => "musgo",
            "clay" => "arcilla",
            "stone" => "piedra",
            "road" | "path" => "camino",
            "swamp" | "marsh" => "pantano",
            "wet" | "water" => "terreno húmedo",
            "lava" => "lava",
            other => other,
        };
        words.push(if num.is_empty() { es.to_string() } else { format!("{es} {num}") });
    }
    let what = if words.is_empty() { texture.to_string() } else { words.join(" ") };
    let place = area.map_or("compartida".to_string(), |n| format!("de {}", AREA_NAMES.get(n).unwrap_or(&"?")));
    format!("{what} {place}{}", if kind == "nrm" { " (relieve)" } else { "" })
}
