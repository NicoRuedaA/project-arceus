//! Census of the materials of the exported field models: which textures each TRMTR block
//! references, whether they are found/decoded, albedo mean colour, layer parameters.
//! Usage: field_material_census <romfs-root> <index.tsv> <out.tsv>
use pla::assets::bntx::Bntx;
use pla::assets::field_material::trmsh_submeshes;
use pla::assets::gfpak::GfPak;
use pla::assets::pokemon_material::parse_blocks;
use std::collections::HashMap;
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let romfs = std::path::PathBuf::from(&a[0]);
    let index = std::fs::read_to_string(&a[1])?;
    let mut out = std::io::BufWriter::new(std::fs::File::create(&a[2])?);
    writeln!(out, "pack\tmodel\tblocks\tsubmeshes\tblock\tshader\tbase_map\tbase_state\tmean_rgb\tmask_map\tmask_state\thas_layers\tlayer_mean_rgb\tused_by_submesh\ttextures\tlayout\tsub_sum\tidx_len")?;
    let mut packs: HashMap<String, (Vec<u8>, GfPak)> = HashMap::new();
    for line in index.lines().filter(|l| !l.starts_with('#')).skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.get(3) != Some(&"ok") { continue; }
        let (pack, model, outp) = (f[0], f[1], f[2]);
        let terrain = outp.contains("_terrain/");
        let (path, prefix) = if terrain {
            let stem = pack.trim_end_matches(".gfpak");
            (romfs.join("bin/archive/field/terrain").join(pack), format!("bin/field/terrain/{stem}/terrain/"))
        } else {
            (romfs.join("bin/archive/field/model/pack").join(pack), "bin/field/model/pack/".to_string())
        };
        let key = path.to_string_lossy().to_string();
        if !packs.contains_key(&key) {
            let bytes = std::fs::read(&path)?;
            let g = GfPak::parse(&bytes)?;
            packs.insert(key.clone(), (bytes, g));
        }
        let (bytes, g) = &packs[&key];
        let get = |name: &str| -> Option<Vec<u8>> {
            let i = g.entry_index_by_path(&format!("{prefix}{name}")).ok().flatten()?;
            g.decoded_data(bytes, &g.entries[i]).ok()
        };
        let sub = get(&format!("{model}.trmsh")).map(|b| trmsh_submeshes(&b)).unwrap_or_default();
        let layout = get(&format!("{model}.trmsh"))
            .and_then(|sh| pla::assets::tr::TrMshLayout::parse(&sh).ok())
            .map(|l| format!("s{}:{}", l.stride, l.attributes.iter().map(|a| format!("{}@{}/{}", a.id, a.offset, a.format_code)).collect::<Vec<_>>().join(",")))
            .unwrap_or_default();
        let sub_sum: usize = sub.iter().map(|s| s.index_count).sum();
        let idx_len = match (get(&format!("{model}.trmbf")), get(&format!("{model}.trmsh"))) {
            (Some(b), Some(sh)) => pla::assets::tr::TrMbf::parse_with_layout(&b, &sh).map(|m| m.indices.len()).unwrap_or(0),
            _ => 0,
        };
        let Some(mtr) = get(&format!("{model}.trmtr")) else {
            writeln!(out, "{pack}\t{model}\t0\t{}\t-\t-\t-\tno_trmtr\t\t\t\t\t\t\t", sub.len())?;
            continue;
        };
        let blocks = parse_blocks(&mtr);
        if blocks.is_empty() {
            writeln!(out, "{pack}\t{model}\t0\t{}\t-\t-\t-\tno_blocks\t\t\t\t\t\t\t", sub.len())?;
            continue;
        }
        let state = |file: &str| -> (String, String) {
            match get(file) {
                None => ("missing_in_pack".into(), String::new()),
                Some(b) => match Bntx::parse(&b) {
                    Err(e) => (format!("bntx_err:{}", e.chars().take(40).collect::<String>()), String::new()),
                    Ok(bn) if bn.textures.is_empty() => ("no_textures".into(), String::new()),
                    Ok(bn) => match bn.textures[0].decode(&b) {
                        Err(e) => (format!("decode_err:{}", e.chars().take(40).collect::<String>()), String::new()),
                        Ok(t) => {
                            let n = (t.rgba.len() / 4).max(1);
                            let mut s = [0u64; 3];
                            for p in t.rgba.chunks_exact(4) { for c in 0..3 { s[c] += p[c] as u64; } }
                            ("ok".into(), format!("{:.0},{:.0},{:.0}", s[0] as f64 / n as f64, s[1] as f64 / n as f64, s[2] as f64 / n as f64))
                        }
                    },
                },
            }
        };
        for bl in &blocks {
            let base = bl.texture("BaseColorMap").unwrap_or("-").to_string();
            let (bs, bm) = if base == "-" { ("none".into(), String::new()) } else { state(&base) };
            let mask = bl.texture("LayerMaskMap").unwrap_or("-").to_string();
            let (ms, _) = if mask == "-" { ("none".into(), String::new()) } else { state(&mask) };
            let layers = bl.base_color_layers();
            let lm = layers.map(|l| format!("{:.2},{:.2},{:.2}", l.iter().map(|x| x[0]).sum::<f32>() / 4.0, l.iter().map(|x| x[1]).sum::<f32>() / 4.0, l.iter().map(|x| x[2]).sum::<f32>() / 4.0)).unwrap_or_default();
            let used = sub.iter().filter(|s| s.material.eq_ignore_ascii_case(&bl.name)).count();
            let tex: Vec<String> = bl.textures.iter().map(|(f, s)| format!("{s}={f}")).collect();
            writeln!(out, "{pack}\t{model}\t{}\t{}\t{}\t{}\t{base}\t{bs}\t{bm}\t{mask}\t{ms}\t{}\t{lm}\t{used}\t{}\t{}\t{}\t{}", blocks.len(), sub.len(), bl.name, bl.shader, layers.is_some(), tex.join(";"), layout, sub_sum, idx_len)?;
        }
    }
    Ok(())
}
