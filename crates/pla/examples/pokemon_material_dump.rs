//! Dump raw TRMTR bytes and decoded textures of one Pokémon pack for material research.
//! Usage: pokemon_material_dump <pack.gfpak> <out dir>
use pla::assets::pokemon_model::assemble_pack;
use std::{env, fs};

fn main() -> Result<(), String> {
    let a: Vec<String> = env::args().skip(1).collect();
    let model = assemble_pack(&a[0])?;
    let out = std::path::Path::new(&a[1]);
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    for m in &model.materials {
        fs::write(out.join(format!("mat{}.trmtr", m.entry_index)), &m.raw_bytes).map_err(|e| e.to_string())?;
    }
    for t in &model.textures {
        if let Ok(img) = &t.image {
            img.save_png(&out.join(format!("{}.png", t.internal_name))).map_err(|e| e.to_string())?;
        }
    }
    // UV-space position-coded maps for LOD0 meshes, one per binding.
    let (w, h) = (512usize, 512usize);
    let mut maps: std::collections::BTreeMap<String, Vec<u8>> = Default::default();
    for m in model.meshes.iter().filter(|m| m.lod == Some(0)) {
        let img = maps.entry(m.material_binding.clone()).or_insert_with(|| vec![0u8; w * h * 4]);
        let v = |i: usize| -> Option<([f32; 3], [f32; 2])> {
            Some((m.mesh.position(&m.buffer_bytes, i)?, m.mesh.uv(&m.buffer_bytes, i)?))
        };
        for t in m.mesh.indices.chunks_exact(3) {
            let (Some(a), Some(b), Some(c)) = (v(t[0] as usize), v(t[1] as usize), v(t[2] as usize)) else { continue };
            let px = |u: [f32; 2]| [u[0] * w as f32, u[1] * h as f32];
            let (pa, pb, pc) = (px(a.1), px(b.1), px(c.1));
            let minx = pa[0].min(pb[0]).min(pc[0]).floor().max(0.0) as usize;
            let maxx = (pa[0].max(pb[0]).max(pc[0]).ceil() as usize).min(w - 1);
            let miny = pa[1].min(pb[1]).min(pc[1]).floor().max(0.0) as usize;
            let maxy = (pa[1].max(pb[1]).max(pc[1]).ceil() as usize).min(h - 1);
            let den = (pb[1] - pc[1]) * (pa[0] - pc[0]) + (pc[0] - pb[0]) * (pa[1] - pc[1]);
            if den.abs() < 1e-9 { continue; }
            for y in miny..=maxy { for x in minx..=maxx {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let l0 = ((pb[1] - pc[1]) * (fx - pc[0]) + (pc[0] - pb[0]) * (fy - pc[1])) / den;
                let l1 = ((pc[1] - pa[1]) * (fx - pc[0]) + (pa[0] - pc[0]) * (fy - pc[1])) / den;
                let l2 = 1.0 - l0 - l1;
                if l0 < 0.0 || l1 < 0.0 || l2 < 0.0 { continue; }
                let p = [0, 1, 2].map(|k| l0 * a.0[k] + l1 * b.0[k] + l2 * c.0[k]);
                let o = (y * w + x) * 4;
                img[o] = ((p[0] * 2.0 + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
                img[o + 1] = (p[1] * 2.0).clamp(0.0, 1.0).mul_add(255.0, 0.0) as u8;
                img[o + 2] = ((p[2] * 2.0 + 0.6).clamp(0.0, 1.0) * 255.0) as u8;
                img[o + 3] = 255;
            }}
        }
    }
    for (name, rgba) in maps {
        pla::assets::bntx::TextureData { width: w as u32, height: h as u32, rgba }
            .save_png(&out.join(format!("uvpos_{name}.png"))).map_err(|e| e.to_string())?;
    }
    for m in model.meshes.iter().filter(|m| m.lod == Some(0)) {
        let mut tops: Vec<(f32, [f32; 3], [f32; 2])> = (0..m.mesh.vertex_count)
            .filter_map(|i| Some((m.mesh.position(&m.buffer_bytes, i)?[1], m.mesh.position(&m.buffer_bytes, i)?, m.mesh.uv(&m.buffer_bytes, i)?)))
            .collect();
        tops.sort_by(|a, b| b.0.total_cmp(&a.0));
        for t in tops.iter().step_by(7).take(6) { println!("top {} {:?} uv {:?}", m.name, t.1, t.2); }
        let stride = m.mesh.vertex_stride;
        let verts = m.mesh.vertices(&m.buffer_bytes);
        let mut hist: std::collections::BTreeMap<[u8; 4], (usize, [f32; 3])> = Default::default();
        for i in 0..m.mesh.vertex_count {
            let c: [u8; 4] = verts[i * stride + 36..i * stride + 40].try_into().unwrap();
            let e = hist.entry(c).or_insert((0, m.mesh.position(&m.buffer_bytes, i).unwrap_or([0.0; 3])));
            e.0 += 1;
        }
        let mut v: Vec<_> = hist.into_iter().collect();
        v.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
        println!("vcol {} {}: {} distinct, top {:?}", m.name, m.material_binding, v.len(), &v[..v.len().min(4)]);
    }
    for m in &model.meshes {
        if m.lod == Some(0) { println!("  layout {:?} stride {}", m.mesh.layout, m.mesh.vertex_stride); }
        println!("mesh {} lod {:?} binding {} mat {:?}", m.name, m.lod, m.material_binding, m.material_entry_index);
    }
    Ok(())
}
