//! Windowless vegetation report: detail-map statistics, scatter counts per chunk and area, the
//! estimated draw calls / triangles / memory of the streaming window, the correlation of every
//! mask channel with the ground-colour layers, and a top-down PNG of the scatter density.
//!
//! No window, no GPU. Usage:
//! `vegetation_report [--bin-root DIR] [--cache-root DIR] [--areas 0,1,5] [--quality low|high]
//!                    [--png-dir DIR] [--splat] [--orientation]`

use std::path::{Path, PathBuf};

use pla::assets::terrain::TerrainArea;
use pla::assets::terrain_albedo::layer_weights;
use pla::assets::vegetation::*;

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned()
}

/// (vertices, triangles) of a glTF file, summed over all primitives.
fn gltf_stats(path: &Path) -> Option<(u32, u32)> {
    let j: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    let acc = j.get("accessors")?.as_array()?;
    let (mut v, mut t) = (0u32, 0u32);
    for m in j.get("meshes")?.as_array()? {
        for p in m.get("primitives")?.as_array()? {
            v += acc[p["attributes"]["POSITION"].as_u64()? as usize]["count"].as_u64()? as u32;
            t += acc[p["indices"].as_u64()? as usize]["count"].as_u64()? as u32 / 3;
        }
    }
    Some((v, t))
}

fn find_model(cache: &Path, area: u8, name: &str) -> Option<PathBuf> {
    let mut dirs = vec![format!("ha_area{area:02}_terrain"), format!("ha_area{area:02}")];
    if let Ok(rd) = std::fs::read_dir(cache.join("gltf_field")) {
        let mut others: Vec<String> = rd
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with("_terrain") && !dirs.contains(n))
            .collect();
        others.sort();
        dirs.extend(others);
    }
    dirs.iter()
        .map(|d| cache.join("gltf_field").join(d).join(format!("{name}.gltf")))
        .find(|p| p.exists())
}

struct Plant {
    spec_model: String,
    used_model: Option<String>,
    verts: u32,
    tris: u32,
    density: f32,
    radius: f32,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bin_root = PathBuf::from(arg(&args, "--bin-root").unwrap_or_else(|| {
        "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into()
    }));
    let cache = PathBuf::from(
        arg(&args, "--cache-root")
            .or_else(|| std::env::var("PLA_CACHE_ROOT").ok())
            .unwrap_or_else(|| "/home/nico/work/decompilacion/work/cache".into()),
    );
    let areas: Vec<u8> = arg(&args, "--areas")
        .unwrap_or_else(|| "0,1,5".into())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let quality = arg(&args, "--quality").and_then(|q| GrassQuality::parse(&q)).unwrap_or(GrassQuality::Low);
    let png_dir = arg(&args, "--png-dir").map(PathBuf::from);
    let splat = args.iter().any(|a| a == "--splat");
    let orient = args.iter().any(|a| a == "--orientation");
    println!("quality {quality:?}, radii (cards, other) = {:?}, caps live {} / per frame {}", quality.radii(), quality.max_live_instances(), quality.max_instances_per_frame());

    for &area in &areas {
        let t0 = std::time::Instant::now();
        let veg = match load_vegetation_area(&bin_root, area) {
            Ok(v) => v,
            Err(e) => {
                println!("[skip] area {area:02}: {e}");
                continue;
            }
        };
        println!("\n=== area {area:02}: {} plants, {} detail maps, load {:.1}s, extent {:?} H {}", veg.descriptor.plants.len(), veg.masks.len(), t0.elapsed().as_secs_f32(), veg.descriptor.extent, veg.descriptor.height);
        let texel = veg.texel_size();

        // Resolve models (with substitutes for blocked ones) and their size.
        let available: Vec<String> = veg
            .descriptor
            .plants
            .iter()
            .filter(|p| find_model(&cache, area, &p.model).is_some())
            .map(|p| p.model.clone())
            .collect();
        let avail_refs: Vec<&str> = available.iter().map(String::as_str).collect();
        let mut plants = Vec::new();
        for spec in &veg.descriptor.plants {
            let used = if find_model(&cache, area, &spec.model).is_some() {
                Some(spec.model.clone())
            } else {
                substitute_model(&spec.model, &avail_refs)
            };
            let (verts, tris) = used
                .as_ref()
                .and_then(|m| find_model(&cache, area, m))
                .and_then(|p| gltf_stats(&p))
                .unwrap_or((0, 0));
            plants.push(Plant {
                spec_model: spec.model.clone(),
                used_model: used,
                verts,
                tris,
                density: density_per_texel(tris, texel, quality),
                radius: radius_for(tris, quality),
            });
        }

        // Splat weights for the enrichment table.
        let splat_data = if splat {
            TerrainArea::load(&bin_root, area).ok().and_then(|ta| {
                let names: Vec<String> = ta
                    .material
                    .as_ref()
                    .map(|m| {
                        let mut v = vec![String::new(); 8];
                        for (f, r) in &m.slots.textures {
                            if let Some(k) = r.strip_prefix("SplatColor").and_then(|d| d.parse::<usize>().ok()) {
                                if k < 8 {
                                    v[k] = f.trim_end_matches("_alb.bntx").trim_start_matches("ha_").to_owned();
                                }
                            }
                        }
                        v
                    })
                    .unwrap_or_default();
                layer_weights(&ta).ok().map(|(w, sw, sh)| (w, sw, sh, names))
            })
        } else {
            None
        };

        println!("{:>2} {:>3} {:<34} {:<28} {:>5} {:>5} {:>7} {:>7} {:>9} {:>8}", "i", "m.c", "plant", "model used", "tris", "dens", "set %", "set m2", "instances", "radius");
        let mut all_instances: Vec<Vec<PlantInstance>> = Vec::new();
        let mut coverage: Vec<(u32, u32, Vec<u32>)> = Vec::new();
        for (i, (spec, pl)) in veg.descriptor.plants.iter().zip(&plants).enumerate() {
            let Some((mask, ch)) = veg.mask_for(i) else {
                println!("{i:>2} (no mask for this plant)");
                all_instances.push(Vec::new());
                coverage.push((0, 0, Vec::new()));
                continue;
            };
            let (cw, chh, counts) = chunk_coverage(mask, ch);
            let set: u64 = counts.iter().map(|&c| c as u64).sum();
            let mut inst = Vec::new();
            if pl.used_model.is_some() {
                let p = ScatterParams { seed: 0x5EED, area, plant: i as u16, density: pl.density };
                scatter_range(mask, ch, veg.descriptor.extent, (0, mask.width), (0, mask.height), &p, spec, &mut inst);
            }
            println!(
                "{:>2} {}.{} {:<34} {:<28} {:>5} {:>5.2} {:>7.3} {:>7.0} {:>9} {:>8.0}",
                i,
                i / 4,
                i % 4,
                pl.spec_model,
                pl.used_model.as_deref().map(|m| if m == pl.spec_model { "(same)" } else { m }).unwrap_or("-- none --"),
                pl.tris,
                pl.density,
                100.0 * set as f64 / (mask.width as f64 * mask.height as f64),
                set as f64 * (texel * texel) as f64,
                inst.len(),
                pl.radius
            );
            all_instances.push(inst);
            coverage.push((cw, chh, counts));
        }
        let total: usize = all_instances.iter().map(Vec::len).sum();
        let total_tris: u64 = all_instances.iter().zip(&plants).map(|(v, p)| v.len() as u64 * p.tris as u64).sum();
        println!("area total (whole map, every chunk): {total} instances, {total_tris} triangles");

        // Per chunk statistics (all plants together).
        let cw = veg.masks[0].width.div_ceil(CHUNK_TEXELS);
        let chh = veg.masks[0].height.div_ceil(CHUNK_TEXELS);
        let mut per_chunk = vec![0usize; (cw * chh) as usize];
        let mut per_chunk_tris = vec![0u64; (cw * chh) as usize];
        let chunk_m = CHUNK_TEXELS as f32 * texel;
        for (pi, inst) in all_instances.iter().enumerate() {
            for it in inst {
                let c = ((it.z / chunk_m) as u32).min(chh - 1) * cw + ((it.x / chunk_m) as u32).min(cw - 1);
                per_chunk[c as usize] += 1;
                per_chunk_tris[c as usize] += plants[pi].tris as u64;
            }
        }
        let mut nz: Vec<usize> = per_chunk.iter().copied().filter(|&n| n > 0).collect();
        nz.sort_unstable();
        if !nz.is_empty() {
            println!(
                "chunks of {chunk_m:.1} m: {} of {} have instances; instances per non-empty chunk: min {} median {} mean {:.0} p95 {} max {}",
                nz.len(),
                per_chunk.len(),
                nz[0],
                nz[nz.len() / 2],
                nz.iter().sum::<usize>() as f64 / nz.len() as f64,
                nz[nz.len() * 95 / 100],
                nz[nz.len() - 1]
            );
        }

        // Streaming window: slide the player over a 32 m grid; count chunk-plant entries (draw calls
        // = entries x primitives, primitives unknown here so entries), instances and triangles.
        let mut best = (0usize, 0usize, 0u64, (0.0f32, 0.0f32));
        let mut best_verts = 0u64;
        let mut samples: Vec<(usize, usize, u64)> = Vec::new();
        let step = 32.0;
        let n = (veg.descriptor.extent[0] / step) as i32;
        for gz in 0..n {
            for gx in 0..n {
                let (px, pz) = ((gx as f32 + 0.5) * step, (gz as f32 + 0.5) * step);
                let (mut dc, mut ins, mut tr, mut vt) = (0usize, 0usize, 0u64, 0u64);
                for (pi, pl) in plants.iter().enumerate() {
                    let (cwp, chp, counts) = &coverage[pi];
                    if counts.is_empty() || pl.used_model.is_none() {
                        continue;
                    }
                    let r = pl.radius;
                    let (c0, c1) = (((px - r) / chunk_m).floor().max(0.0) as u32, ((px + r) / chunk_m).floor().min(*cwp as f32 - 1.0) as u32);
                    let (r0, r1) = (((pz - r) / chunk_m).floor().max(0.0) as u32, ((pz + r) / chunk_m).floor().min(*chp as f32 - 1.0) as u32);
                    for cz in r0..=r1 {
                        for cx in c0..=c1 {
                            let cnt = counts[(cz * cwp + cx) as usize];
                            if cnt == 0 {
                                continue;
                            }
                            // distance from the player to the chunk rectangle
                            let (x0, z0) = (cx as f32 * chunk_m, cz as f32 * chunk_m);
                            let dx = (x0 - px).max(0.0).max(px - (x0 + chunk_m));
                            let dz = (z0 - pz).max(0.0).max(pz - (z0 + chunk_m));
                            if dx * dx + dz * dz <= r * r {
                                dc += 1;
                                let k = (cnt as f32 * pl.density) as usize;
                                ins += k;
                                tr += k as u64 * pl.tris as u64;
                                vt += k as u64 * pl.verts as u64;
                            }
                        }
                    }
                }
                if ins > 0 {
                    samples.push((dc, ins, tr));
                }
                if ins > best.1 {
                    best = (dc, ins, tr, (px, pz));
                    best_verts = vt;
                }
            }
        }
        if !samples.is_empty() {
            let mut d: Vec<usize> = samples.iter().map(|s| s.0).collect();
            let mut t: Vec<u64> = samples.iter().map(|s| s.2).collect();
            let mut ii: Vec<usize> = samples.iter().map(|s| s.1).collect();
            d.sort_unstable();
            t.sort_unstable();
            ii.sort_unstable();
            let m = samples.len();
                        println!(
                "streaming window ({quality:?}) over {m} player positions with vegetation: draw calls (chunk x plant) median {} p95 {} max {}; instances median {} p95 {} max {}; triangles median {} p95 {} max {}",
                d[m / 2], d[m * 95 / 100], d[m - 1], ii[m / 2], ii[m * 95 / 100], ii[m - 1], t[m / 2], t[m * 95 / 100], t[m - 1]
            );
            println!(
                "worst window at ({:.0},{:.0}): {} draw calls, {} instances, {} tris; merged-mesh memory there ~{:.1} MiB ({} vertices x 52 B: pos+normal+uv+colour, + 4 B/index)",
                best.3 .0, best.3 .1, best.0, best.1, best.2, (best_verts as f64 * 52.0 + best.2 as f64 * 12.0) / 1048576.0, best_verts
            );
        }

        // Correlation of every mask channel with the ground layers (splat weights).
        if let Some((w, sw, sh, names)) = &splat_data {
            let mut mean = [0.0f64; 8];
            for v in w {
                for k in 0..8 {
                    mean[k] += v[k] as f64;
                }
            }
            mean.iter_mut().for_each(|m| *m /= w.len() as f64);
            println!("splat layers: {:?}", names);
            println!("mean layer weight (whole area): {:?}", mean.iter().map(|m| (m * 100.0).round() / 100.0).collect::<Vec<_>>());
            println!("per plant: mean layer weight under set texels (x / area mean = enrichment) top 3:");
            for (i, pl) in plants.iter().enumerate() {
                let Some((mask, ch)) = veg.mask_for(i) else { continue };
                let mut acc = [0.0f64; 8];
                let mut n = 0u64;
                for row in 0..mask.height {
                    for col in 0..mask.width {
                        if mask.get(col, row, ch) != 0 {
                            let (sx, sy) = ((col * sw / mask.width) as usize, (row * sh / mask.height) as usize);
                            let v = &w[sy * *sw as usize + sx];
                            for k in 0..8 {
                                acc[k] += v[k] as f64;
                            }
                            n += 1;
                        }
                    }
                }
                if n == 0 {
                    println!("{i:>2} {:<34} (empty)", pl.spec_model);
                    continue;
                }
                let mut e: Vec<(usize, f64, f64)> = (0..8).map(|k| (k, acc[k] / n as f64, (acc[k] / n as f64) / mean[k].max(1e-6))).collect();
                e.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                let s: Vec<String> = e.iter().take(3).map(|(k, m, r)| format!("{}={:.2} (x{:.1})", names.get(*k).map_or("?", |s| s.as_str()), m, r)).collect();
                println!("{i:>2} {:<34} n={n:<8} {}", pl.spec_model, s.join(", "));
            }
            if orient {
                // all 8 flips/transposes of the (first) mask channel against the grass layers' weight
                let grass: Vec<usize> = names.iter().enumerate().filter(|(_, n)| n.contains("grass")).map(|(k, _)| k).collect();
                println!("orientation test, grass layers {grass:?}: mean grass weight under set texels of channels 0.0 / 0.1");
                for (name, tf) in [("id", 0), ("flipx", 1), ("flipz", 2), ("both", 3), ("T", 4), ("T-flipx", 5), ("T-flipz", 6), ("T-both", 7)] {
                    let mut line = format!("  {name:<8}");
                    for (mi, ch) in [(0usize, 0usize), (0, 1)] {
                        let Some(mask) = veg.masks.get(mi) else { continue };
                        let (mut acc, mut n) = (0.0f64, 0u64);
                        for row in 0..mask.height {
                            for col in 0..mask.width {
                                if mask.get(col, row, ch) == 0 {
                                    continue;
                                }
                                let (mut c, mut r) = (col, row);
                                if tf >= 4 {
                                    std::mem::swap(&mut c, &mut r);
                                }
                                if tf & 1 == 1 {
                                    c = mask.width - 1 - c;
                                }
                                if tf & 2 == 2 {
                                    r = mask.height - 1 - r;
                                }
                                let (sx, sy) = ((c * sw / mask.width) as usize, (r * sh / mask.height) as usize);
                                acc += grass.iter().map(|&k| w[sy * *sw as usize + sx][k] as f64).sum::<f64>();
                                n += 1;
                            }
                        }
                        line += &format!("  {:.3}", acc / n.max(1) as f64);
                    }
                    println!("{line}");
                }
            }
        }

        // Top-down density PNG: 1 px per metre; R = flowers/details, G = grass cards, B = shrubs/stones.
        if let Some(dir) = &png_dir {
            std::fs::create_dir_all(dir).ok();
            let side = veg.descriptor.extent[0].ceil() as usize;
            let mut img = vec![0u8; side * side * 3];
            // base: height shading
            for z in 0..side {
                for x in 0..side {
                    let h = veg.ground_y(x as f32, z as f32) / veg.descriptor.height.max(1.0);
                    let g = (30.0 + 70.0 * h.clamp(0.0, 1.0)) as u8;
                    let o = (z * side + x) * 3;
                    img[o] = g / 2;
                    img[o + 1] = g / 2;
                    img[o + 2] = g / 2 + 8;
                }
            }
            let mut acc = vec![[0.0f32; 3]; side * side];
            for (pi, inst) in all_instances.iter().enumerate() {
                let name = plants[pi].spec_model.as_str();
                let cls = if name.contains("grass") || name.contains("gimmick") || name.contains("detail") {
                    1
                } else if name.contains("flower") {
                    0
                } else {
                    2
                };
                for it in inst {
                    let (x, z) = ((it.x as usize).min(side - 1), (it.z as usize).min(side - 1));
                    acc[z * side + x][cls] += 1.0;
                }
            }
            for (i, a) in acc.iter().enumerate() {
                for c in 0..3 {
                    if a[c] > 0.0 {
                        let v = (60.0 + 40.0 * a[c].ln_1p()).min(255.0) as u8;
                        img[i * 3 + c] = img[i * 3 + c].max(v);
                        if c != 1 {
                            img[i * 3 + 1] = img[i * 3 + 1].max(v / 2);
                        }
                    }
                }
            }
            let path = dir.join(format!("scatter_area{area:02}.png"));
            let f = std::fs::File::create(&path).expect("png file");
            let mut enc = png::Encoder::new(std::io::BufWriter::new(f), side as u32, side as u32);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(&img).unwrap();
            println!("wrote {}", path.display());
        }
    }
}
