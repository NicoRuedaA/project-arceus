//! Lists the UV transform (`UVScaleOffset`, `UVScaleOffsetNormal`, `UVScaleOffset1`) and the
//! rotation scalars (and the texture slots) of every material block of Pokémon packs.
//!
//! Usage: pokemon_uv_census <pm....gfpak>...   (TSV on stdout: pack, material entry, block name,
//! shader, the three vectors, `UVRotation`, `UVRotationNormal`)

use pla::assets::pokemon_material::parse_blocks;
use pla::assets::pokemon_model::assemble_pack;
use std::env;

fn vec4(v: Option<&[f32; 4]>) -> String {
    v.map(|v| format!("{},{},{},{}", v[0], v[1], v[2], v[3]))
        .unwrap_or_else(|| "-".into())
}

fn main() -> Result<(), String> {
    println!("pack\tentry\tsource\tblock\tshader\tuvso\tuvso_normal\tuvso1\trot\trot_normal\ttextures");
    for path in env::args().skip(1) {
        let stem = std::path::Path::new(&path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let model = match assemble_pack(&path) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("{stem}: {e}");
                continue;
            }
        };
        for material in &model.materials {
            for block in parse_blocks(&material.raw_bytes) {
                let rot = |name: &str| {
                    block
                        .scalars
                        .get(name)
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into())
                };
                println!(
                    "{stem}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    material.entry_index,
                    material.source_name,
                    block.name,
                    block.shader,
                    vec4(block.vec4.get("UVScaleOffset")),
                    vec4(block.vec4.get("UVScaleOffsetNormal")),
                    vec4(block.vec4.get("UVScaleOffset1")),
                    rot("UVRotation"),
                    rot("UVRotationNormal"),
                    block
                        .textures
                        .iter()
                        .map(|(file, slot)| format!("{slot}={file}"))
                        .collect::<Vec<_>>()
                        .join(","),
                );
            }
        }
    }
    Ok(())
}
