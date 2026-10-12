//! Bounded metadata inspection of one Pokémon GFLXPACK.
//!
//! Usage: `cargo run -p pla --example pokemon_spike -- <pokemon.gfpak>`.
//! The tool decodes one entry at a time, keeps only validated mesh candidates,
//! then tests them against every entry as a possible FlatBuffer skin descriptor.

use std::{env, fs};

use pla::assets::{
    bntx::Bntx,
    gfpak::GfPak,
    tr::{TrMbf, TrMshLayout, TrSkl},
};

struct Candidate {
    entry_index: usize,
    bytes: Vec<u8>,
}

fn printable_runs(bytes: &[u8]) -> Vec<String> {
    let mut runs = Vec::new();
    let mut start = None;
    for (index, byte) in bytes.iter().copied().chain([0]).enumerate() {
        let printable = byte.is_ascii_alphanumeric() || b"_./-".contains(&byte);
        if printable {
            start.get_or_insert(index);
        } else if let Some(begin) = start.take() {
            if index - begin >= 6 && runs.len() < 16 {
                runs.push(String::from_utf8_lossy(&bytes[begin..index]).into_owned());
            }
        }
    }
    runs
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("expected one Pokémon .gfpak path")?;
    let bytes = fs::read(&path)?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("pack exceeds the 64 MiB input cap".into());
    }
    let pack = GfPak::parse(&bytes)?;
    if pack.entry_count > 10_000 {
        return Err("pack entry count exceeds the 10,000-entry cap".into());
    }

    let mut mesh_candidates = Vec::new();
    let mut decoded_entries = 0usize;
    let mut layout_candidates = 0usize;
    let mut skeleton_candidates = 0usize;
    let mut bntx_candidates = 0usize;
    let mut mesh_candidate_bytes = 0usize;
    for (entry_index, entry) in pack.entries.iter().enumerate() {
        let decoded = pack.decoded_data(&bytes, entry)?;
        decoded_entries += 1;
        if (150..pack.entries.len().min(180)).contains(&entry_index) {
            let mut roles = Vec::new();
            if TrMbf::parse(&decoded).is_ok() {
                roles.push("TrMbf");
            }
            if TrMshLayout::parse(&decoded).is_ok() {
                roles.push("TrMshLayout");
            }
            if TrSkl::parse(&decoded).is_ok() {
                roles.push("TrSkl");
            }
            if Bntx::parse(&decoded).is_ok() {
                roles.push("BNTX");
            }
            println!(
                "entry={} decoded_bytes={} parser_roles={:?} strings={:?}",
                entry_index,
                decoded.len(),
                roles,
                printable_runs(&decoded)
            );
            if entry_index <= 151 {
                println!(
                    "entry={} trskl_parse_error={:?}",
                    entry_index,
                    TrSkl::parse(&decoded).err()
                );
            }
        }
        if TrMbf::parse(&decoded).is_ok() {
            if mesh_candidates.len() == 16 {
                return Err("mesh candidate count exceeds the 16-entry cap".into());
            }
            mesh_candidate_bytes = mesh_candidate_bytes
                .checked_add(decoded.len())
                .ok_or("mesh candidate size overflow")?;
            if mesh_candidate_bytes > 512 * 1024 * 1024 {
                return Err("aggregate mesh candidates exceed the 512 MiB cap".into());
            }
            mesh_candidates.push(Candidate {
                entry_index,
                bytes: decoded.clone(),
            });
        }
        if TrMshLayout::parse(&decoded).is_ok() {
            layout_candidates += 1;
        }
        if TrSkl::parse(&decoded).is_ok() {
            skeleton_candidates += 1;
        }
        if let Ok(bntx) = Bntx::parse(&decoded) {
            bntx_candidates += 1;
            println!("bntx_entry={entry_index} textures={}", bntx.textures.len());
            for texture in &bntx.textures {
                println!(
                    "  texture={} size={}x{} format={:?}",
                    texture.name, texture.width, texture.height, texture.format
                );
            }
        }
    }

    println!(
        "pack={} entries={} decoded={} TrMbf={} TrMshLayout={} TrSkl={} BNTX={}",
        path,
        pack.entries.len(),
        decoded_entries,
        mesh_candidates.len(),
        layout_candidates,
        skeleton_candidates,
        bntx_candidates,
    );
    for candidate in &mesh_candidates {
        let parsed = TrMbf::parse(&candidate.bytes)?;
        println!(
            "mesh_entry={} decoded_bytes={} vertices={} stride={} indices={}",
            candidate.entry_index,
            candidate.bytes.len(),
            parsed.vertex_count,
            parsed.vertex_stride,
            parsed.indices.len()
        );
    }

    let mut pair_count = 0usize;
    for (descriptor_index, entry) in pack.entries.iter().enumerate() {
        let descriptor = pack.decoded_data(&bytes, entry)?;
        for mesh in &mesh_candidates {
            let mut shapes = 0usize;
            for shape_index in 0..64 {
                match TrMbf::parse_skin_shape(&mesh.bytes, &descriptor, shape_index) {
                    Ok(_) => shapes += 1,
                    Err(error) if error == "skin shape index out of range" => break,
                    Err(_) => {}
                }
            }
            if shapes > 0 {
                pair_count += 1;
                println!(
                    "skin_pair mesh_entry={} descriptor_entry={} shapes={shapes}",
                    mesh.entry_index, descriptor_index
                );
            }
        }
    }
    println!("skin_pair_count={pair_count}");
    Ok(())
}
