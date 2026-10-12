//! Parse every terrain navmesh pack, print counts, check vertices against the
//! terrain heightmap, and export OBJ/TSV per main area.
//!
//! Usage: navmesh_report [OUT_DIR]   (PLA_BIN_ROOT overrides the game root)

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use pla::assets::navmesh::{load_pack_navmeshes, NavMesh};
use pla::assets::terrain::TerrainArea;

fn bilinear(samples: &[f32], w: usize, h: usize, extent: f32, x: f32, z: f32) -> f32 {
    let fx = (x * (w - 1) as f32 / extent).clamp(0.0, w as f32 - 1.001);
    let fz = (z * (h - 1) as f32 / extent).clamp(0.0, h as f32 - 1.001);
    let (x0, z0) = (fx as usize, fz as usize);
    let (tx, tz) = (fx - x0 as f32, fz - z0 as f32);
    let s = |x: usize, z: usize| samples[z * w + x];
    let a = s(x0, z0) * (1.0 - tx) + s(x0 + 1, z0) * tx;
    let b = s(x0, z0 + 1) * (1.0 - tx) + s(x0 + 1, z0 + 1) * tx;
    a * (1.0 - tz) + b * tz
}

fn pct(v: &mut Vec<f32>, p: f32) -> f32 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f32 * p) as usize]
}

fn main() {
    let root = std::env::var_os("PLA_BIN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into());
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "work/cache/navmesh".into()));
    std::fs::create_dir_all(&out).unwrap();
    let dir = root.join("archive/field/terrain");
    let mut packs: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(Result::ok).map(|e| e.path()).collect();
    packs.sort();
    let mut summary = String::from("pack\tmeshes\tfaces\tedges\tvertices\tmin_x\tmin_y\tmin_z\tmax_x\tmax_y\tmax_z\n");
    let (mut total_packs, mut total_meshes, mut total_faces) = (0, 0usize, 0usize);
    let mut flag_hist: BTreeMap<u8, usize> = BTreeMap::new();
    let mut face_data_hist: BTreeMap<i32, usize> = BTreeMap::new();
    let mut edge_data_hist: BTreeMap<i32, usize> = BTreeMap::new();
    let mut main_meshes: BTreeMap<u8, Vec<NavMesh>> = BTreeMap::new();
    for p in &packs {
        let name = p.file_stem().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("ha_area") {
            continue;
        }
        let meshes = match load_pack_navmeshes(p) {
            Ok(m) => m,
            Err(e) => {
                println!("FAIL {name}: {e}");
                continue;
            }
        };
        total_packs += 1;
        for (mi, m) in meshes.iter().enumerate() {
            total_meshes += 1;
            total_faces += m.faces.len();
            for e in &m.edges {
                *flag_hist.entry(e.flags).or_default() += 1;
            }
            for v in m.face_data.iter().take(2_000_000) {
                *face_data_hist.entry(*v).or_default() += 1;
            }
            for v in m.edge_data.iter() {
                *edge_data_hist.entry(*v).or_default() += 1;
            }
            let (lo, hi) = m.bounds().unwrap_or(([0.0; 3], [0.0; 3]));
            writeln!(summary, "{name}#{mi}\t{}\t{}\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}",
                meshes.len(), m.faces.len(), m.edges.len(), m.vertices.len(),
                lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]).unwrap();
        }
        if name.len() == 9 {
            main_meshes.insert(name[7..9].parse().unwrap(), meshes);
        }
    }
    std::fs::write(out.join("navmesh_summary.tsv"), &summary).unwrap();
    print!("{summary}");
    println!("packs={total_packs} meshes={total_meshes} faces={total_faces}");
    println!("edge flags: {flag_hist:?}");
    println!("face_data (first values): {:?}", face_data_hist.iter().take(12).collect::<Vec<_>>());
    println!("edge_data (first values): {:?}", edge_data_hist.iter().take(12).collect::<Vec<_>>());

    for (area, meshes) in &main_meshes {
        for (mi, m) in meshes.iter().enumerate() {
            let mut obj = String::new();
            for v in &m.vertices {
                writeln!(obj, "v {} {} {}", v[0], v[1], v[2]).unwrap();
            }
            for f in 0..m.faces.len() as u32 {
                let fc = m.faces[f as usize];
                let idx: Vec<String> = (fc.start_edge..fc.start_edge + fc.num_edges as u32)
                    .map(|e| (m.edges[e as usize].a + 1).to_string())
                    .collect();
                writeln!(obj, "f {}", idx.join(" ")).unwrap();
            }
            std::fs::write(out.join(format!("ha_area{area:02}_nav{mi}.obj")), obj).unwrap();
        }
        match TerrainArea::load(&root, *area) {
            Err(e) => println!("area {area}: heightmap unavailable: {e}"),
            Ok(t) => {
                for p in &packs {
                    let n = p.file_stem().unwrap().to_string_lossy().into_owned();
                    if !n.starts_with(&format!("ha_area{area:02}_s")) { continue; }
                    let Ok(ms) = load_pack_navmeshes(p) else { continue };
                    let hm = &t.heightmap;
                    for m in ms.iter().take(1) {
                        let d: Vec<f32> = m.vertices.iter().filter(|v| (0.0..=1025.0).contains(&v[0]) && (0.0..=1025.0).contains(&v[2]))
                            .map(|v| (v[1] - bilinear(&hm.samples, hm.width as usize, hm.height as usize, t.world_extent[0], v[0], v[2]) * t.world_height).abs()).collect();
                        let ok = d.iter().filter(|x| **x <= 2.0).count();
                        if m.vertices.len() > 100 { println!("  sub {n}: {}/{} vertices within 2 m of the area heightmap", ok, d.len()); }
                    }
                }
                let hm = &t.heightmap;
                let (w, h) = (hm.width as usize, hm.height as usize);
                write_preview(&out.join(format!("ha_area{area:02}_preview.png")), &t, meshes);
                for (mi, m) in meshes.iter().enumerate() {
                    let mut d: Vec<f32> = m
                        .vertices
                        .iter()
                        .filter(|v| (0.0..=1025.0).contains(&v[0]) && (0.0..=1025.0).contains(&v[2]))
                        .map(|v| (v[1] - bilinear(&hm.samples, w, h, t.world_extent[0], v[0], v[2]) * t.world_height).abs())
                        .collect();
                    let within2 = d.iter().filter(|x| **x <= 2.0).count() as f32 / d.len() as f32;
                    let max = d.iter().cloned().fold(0.0, f32::max);
                    let signed: f32 = m.vertices.iter().filter(|v| (0.0..=1025.0).contains(&v[0]) && (0.0..=1025.0).contains(&v[2]))
                        .map(|v| v[1] - bilinear(&hm.samples, w, h, t.world_extent[0], v[0], v[2]) * t.world_height).sum::<f32>() / d.len() as f32;
                    let mut d3: Vec<f32> = Vec::new();
                    for (fi, f) in m.faces.iter().enumerate() {
                        if m.face_data.get(fi * m.face_data_stride.max(1) as usize) == Some(&3) {
                            for v in m.face_vertices(fi as u32) {
                                d3.push(v[1] - bilinear(&hm.samples, w, h, t.world_extent[0], v[0].clamp(0.0, 1025.0), v[2].clamp(0.0, 1025.0)) * t.world_height);
                            }
                            let _ = f;
                        }
                    }
                    if !d3.is_empty() {
                        println!("  faces with data=3: {} corners, median signed dy={:.3} p5={:.3} p95={:.3}", d3.len(), pct(&mut d3.clone(), 0.5), pct(&mut d3.clone(), 0.05), pct(&mut d3, 0.95));
                    }
                    println!("  mean signed dy={signed:.3}");
                    println!("area {area} nav{mi}: H={} n={} median|dy|={:.3} p95={:.3} max={:.2} within2m={:.4}",
                        t.world_height, d.len(), pct(&mut d.clone(), 0.5), pct(&mut d, 0.95), max, within2);
                }
            }
        }
    }
}

/// Grey heightmap with the primary navmesh in green and the second one in orange.
fn write_preview(path: &std::path::Path, t: &TerrainArea, meshes: &[NavMesh]) {
    const N: usize = 1025;
    let hm = &t.heightmap;
    let mut rgb = vec![0u8; N * N * 3];
    for pz in 0..N {
        for px in 0..N {
            let (x, z) = (px as f32, pz as f32);
            let g = (bilinear(&hm.samples, hm.width as usize, hm.height as usize, t.world_extent[0], x, z).clamp(0.0, 1.0) * 255.0) as u8;
            let mut c = [g, g, g];
            for (mi, m) in meshes.iter().enumerate().take(2) {
                if m.is_walkable(x, z) {
                    let tint = if mi == 0 { [0.0, 255.0, 60.0] } else { [255.0, 140.0, 0.0] };
                    for k in 0..3 {
                        c[k] = (c[k] as f32 * 0.45 + tint[k] * 0.55) as u8;
                    }
                }
            }
            rgb[(pz * N + px) * 3..(pz * N + px) * 3 + 3].copy_from_slice(&c);
        }
    }
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), N as u32, N as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&rgb).unwrap();
}
