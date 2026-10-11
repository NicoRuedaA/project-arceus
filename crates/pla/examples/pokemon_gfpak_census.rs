//! Inspect Oodle-framed Pokémon GFLXPACK entries without writing game data.
//!
//! Run with `cargo run -p pla --example pokemon_gfpak_census -- <pokemon-dir>`.

use std::{env, fs, path::Path};

use pla::assets::{
    bntx::Bntx,
    gfpak::{is_oodle_frame, GfPak},
    tr::{TrMbf, TrMshLayout, TrSkl},
};

fn collect_packs(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_packs(&path, out)?;
        } else if path.file_name().is_some_and(|name| {
            let name = name.to_string_lossy();
            name.starts_with("pm") && name.ends_with(".gfpak")
        }) {
            out.push(path);
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = env::args()
        .nth(1)
        .ok_or("expected Pokémon archive directory")?;
    let mut packs = Vec::new();
    collect_packs(Path::new(&root), &mut packs)?;
    packs.sort();

    let mut framed_entries = 0usize;
    let mut compressed_entries = 0usize;
    let mut decoded_entries = 0usize;
    let mut decode_failures = 0usize;
    let mut entry_bounds_failures = 0usize;
    let mut trmbf_ok = 0usize;
    let mut skin_shapes_ok = 0usize;
    let mut packs_with_skin = 0usize;
    let mut trmsh_ok = 0usize;
    let mut trskl_ok = 0usize;
    let mut bntx_ok = 0usize;
    let mut packs_with_mesh = 0usize;
    let mut packs_with_skeleton = 0usize;
    let mut packs_with_both = 0usize;

    for path in &packs {
        let bytes = fs::read(path)?;
        let pak = GfPak::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        if pak.kind != 4 {
            continue;
        }
        let mut pack_mesh = false;
        let mut pack_skeleton = false;
        let mut pack_skin = false;
        let mut mesh_candidates = Vec::new();
        let mut layout_candidates = Vec::new();
        for entry in &pak.entries {
            let raw = match pak.data(&bytes, entry) {
                Ok(raw) => raw,
                Err(_) => {
                    entry_bounds_failures += 1;
                    continue;
                }
            };
            if !is_oodle_frame(raw) {
                continue;
            }
            framed_entries += 1;
            compressed_entries += usize::from(raw[0] & 0x40 == 0);
            match pak.decoded_data(&bytes, entry) {
                Ok(data) => {
                    decoded_entries += 1;
                    if TrMbf::parse(&data).is_ok() {
                        trmbf_ok += 1;
                        pack_mesh = true;
                        mesh_candidates.push(data.clone());
                    }
                    if TrMshLayout::parse(&data).is_ok() {
                        trmsh_ok += 1;
                        layout_candidates.push(data.clone());
                    }
                    if TrSkl::parse(&data).is_ok() {
                        trskl_ok += 1;
                        pack_skeleton = true;
                    }
                    if Bntx::parse(&data).is_ok() {
                        bntx_ok += 1;
                    }
                }
                Err(_) => decode_failures += 1,
            }
        }
        for mesh in &mesh_candidates {
            for layout in &layout_candidates {
                for shape_index in 0..64 {
                    match TrMbf::parse_skin_shape(mesh, layout, shape_index) {
                        Ok(_) => {
                            skin_shapes_ok += 1;
                            pack_skin = true;
                        }
                        Err(error) if error == "skin shape index out of range" => break,
                        Err(_) => {}
                    }
                }
            }
        }
        packs_with_mesh += usize::from(pack_mesh);
        packs_with_skeleton += usize::from(pack_skeleton);
        packs_with_both += usize::from(pack_mesh && pack_skeleton);
        packs_with_skin += usize::from(pack_skin);
    }

    println!("packs_scanned={}", packs.len());
    println!("oodle_framed_entries={framed_entries}");
    println!("kraken_compressed_entries={compressed_entries}");
    println!("decoded_entries={decoded_entries}");
    println!("decode_failures={decode_failures}");
    println!("entry_bounds_failures={entry_bounds_failures}");
    println!("TrMbf_parse_ok={trmbf_ok} packs_with_mesh={packs_with_mesh}");
    println!("TrMshLayout_parse_ok={trmsh_ok}");
    println!("parse_skin_shape_ok={skin_shapes_ok} packs_with_skin={packs_with_skin}");
    println!("TrSkl_parse_ok={trskl_ok} packs_with_skeleton={packs_with_skeleton} packs_with_both={packs_with_both}");
    println!("Bntx_parse_ok={bntx_ok}");
    Ok(())
}
