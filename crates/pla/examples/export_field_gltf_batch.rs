//! Batch export of field scenery models (ha_area*.gfpak and plants_* from terrain) to glTF 2.0.
//!
//! Usage: cargo run -p pla --example export_field_gltf_batch --offline -- <romfs-root> <output-gltf-dir> <output-tsv>

use pla::assets::bntx::Bntx;
use pla::assets::gfpak::GfPak;
use pla::assets::gfpak_paths::GfPakHashIndex;
use pla::assets::gltf_export::{primitive_from_tr, skeleton_from_tr, GltfExport, GltfMaterial};
use pla::assets::tr::{TrMbf, TrMtr, TrSkl};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

struct PackContext<'a> {
    pack_name: &'a str,
    prefix: &'a str,
    bytes: &'a [u8],
    gfpak: &'a GfPak,
    index: &'a GfPakHashIndex,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        eprintln!("Usage: export_field_gltf_batch <romfs-root> <output-gltf-dir> <output-tsv>");
        std::process::exit(1);
    }

    let romfs_root = PathBuf::from(&args[0]);
    let out_dir = PathBuf::from(&args[1]);
    let out_tsv = PathBuf::from(&args[2]);

    if let Some(parent) = out_tsv.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut writer = BufWriter::new(File::create(&out_tsv)?);
    writeln!(writer, "# sheet: data/field_model_export_index")?;
    writeln!(writer, "# version: 1")?;
    writeln!(writer, "# generator: crates/pla/examples/export_field_gltf_batch.rs")?;
    writeln!(writer, "# target: Pokémon Legends: Arceus v262144 field scenery models")?;
    writeln!(writer, "source_pack:string\tmodel_name:string*\toutput:string?\tstatus:enum\treason:string?")?;

    let mut total_count = 0;
    let mut ok_count = 0;
    let mut blocked_count = 0;

    // 1. Process Field Model Packs
    let model_pack_dir = romfs_root.join("bin/archive/field/model/pack");
    let mut model_packs = Vec::new();
    if model_pack_dir.exists() {
        for entry in std::fs::read_dir(&model_pack_dir)? {
            let entry = entry?;
            let path = entry.path();
            let fname = path.file_name().unwrap().to_string_lossy();
            if fname.ends_with(".gfpak")
                && !fname.contains("collision")
                && !fname.contains("animation")
            {
                model_packs.push(path);
            }
        }
    }
    model_packs.sort();

    println!("Found {} field model packs", model_packs.len());

    for pack_path in &model_packs {
        let pack_name = pack_path.file_name().unwrap().to_str().unwrap();
        println!("Processing model pack: {}", pack_name);
        let bytes = std::fs::read(pack_path)?;
        let gfpak = match GfPak::parse(&bytes) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("  Failed to parse {}: {}", pack_name, e);
                continue;
            }
        };
        let index = match GfPakHashIndex::from_bytes(&bytes, gfpak.entry_count as usize) {
            Ok(idx) => idx,
            Err(e) => {
                eprintln!("  Failed to build hash index {}: {}", pack_name, e);
                continue;
            }
        };

        let prefix = "bin/field/model/pack/";
        let ctx = PackContext {
            pack_name,
            prefix,
            bytes: &bytes,
            gfpak: &gfpak,
            index: &index,
        };

        // Discover models in pack
        let models = discover_models(&bytes, prefix, &index);
        println!("  Found {} .trmdl candidates", models.len());

        for model in models {
            total_count += 1;
            let pack_stem = pack_path.file_stem().unwrap().to_str().unwrap();
            let out_pack_dir = out_dir.join(pack_stem);
            match export_model(&ctx, &model, &out_pack_dir) {
                Ok(rel_out) => {
                    ok_count += 1;
                    writeln!(
                        writer,
                        "{}\t{}\t{}\tok\t",
                        pack_name,
                        model,
                        rel_out.display()
                    )?;
                }
                Err(reason) => {
                    blocked_count += 1;
                    let clean_reason = reason.replace(['\t', '\n', '\r'], " ");
                    writeln!(
                        writer,
                        "{}\t{}\t\tblocked\t{}",
                        pack_name,
                        model,
                        clean_reason
                    )?;
                }
            }
        }
    }

    // 2. Process Terrain Packs (plants_* models only)
    let terrain_dir = romfs_root.join("bin/archive/field/terrain");
    let mut terrain_packs = Vec::new();
    if terrain_dir.exists() {
        for entry in std::fs::read_dir(&terrain_dir)? {
            let entry = entry?;
            let path = entry.path();
            let fname = path.file_name().unwrap().to_string_lossy();
            if fname.starts_with("ha_area") && fname.ends_with(".gfpak") {
                terrain_packs.push(path);
            }
        }
    }
    terrain_packs.sort();

    println!("Found {} terrain packs", terrain_packs.len());

    for pack_path in &terrain_packs {
        let pack_name = pack_path.file_name().unwrap().to_str().unwrap();
        let area_stem = pack_path.file_stem().unwrap().to_str().unwrap();
        let prefix = format!("bin/field/terrain/{}/terrain/", area_stem);

        println!("Processing terrain pack: {} (prefix: {})", pack_name, prefix);
        let bytes = std::fs::read(pack_path)?;
        let gfpak = match GfPak::parse(&bytes) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("  Failed to parse {}: {}", pack_name, e);
                continue;
            }
        };
        let index = match GfPakHashIndex::from_bytes(&bytes, gfpak.entry_count as usize) {
            Ok(idx) => idx,
            Err(e) => {
                eprintln!("  Failed to build hash index {}: {}", pack_name, e);
                continue;
            }
        };

        let ctx = PackContext {
            pack_name,
            prefix: &prefix,
            bytes: &bytes,
            gfpak: &gfpak,
            index: &index,
        };

        let all_models = discover_models(&bytes, &prefix, &index);
        let plant_models: Vec<String> = all_models
            .into_iter()
            .filter(|m| m.starts_with("plants_"))
            .collect();
        println!("  Found {} plants_* candidates", plant_models.len());

        for model in plant_models {
            total_count += 1;
            let out_pack_dir = out_dir.join(format!("{}_terrain", area_stem));
            match export_model(&ctx, &model, &out_pack_dir) {
                Ok(rel_out) => {
                    ok_count += 1;
                    writeln!(
                        writer,
                        "{}\t{}\t{}\tok\t",
                        pack_name,
                        model,
                        rel_out.display()
                    )?;
                }
                Err(reason) => {
                    blocked_count += 1;
                    let clean_reason = reason.replace(['\t', '\n', '\r'], " ");
                    writeln!(
                        writer,
                        "{}\t{}\t\tblocked\t{}",
                        pack_name,
                        model,
                        clean_reason
                    )?;
                }
            }
        }
    }

    writer.flush()?;
    println!(
        "\nTotal models: {}, Exported OK: {}, Blocked: {}",
        total_count, ok_count, blocked_count
    );

    Ok(())
}

fn discover_models(bytes: &[u8], prefix: &str, index: &GfPakHashIndex) -> Vec<String> {
    let mut names = HashSet::new();
    let mut pos = 0;
    while pos + 6 <= bytes.len() {
        if &bytes[pos..pos + 3] == b".tr" {
            let mut start = pos;
            while start > 0 {
                let c = bytes[start - 1];
                if (c >= b'a' && c <= b'z')
                    || (c >= b'A' && c <= b'Z')
                    || (c >= b'0' && c <= b'9')
                    || c == b'_'
                {
                    start -= 1;
                } else {
                    break;
                }
            }
            if pos - start >= 3 {
                if let Ok(s) = std::str::from_utf8(&bytes[start..pos]) {
                    names.insert(s.to_string());
                }
            }
        }
        pos += 1;
    }

    let mut found_models = Vec::new();
    for name in names {
        let mdl_path = format!("{}{}.trmdl", prefix, name);
        if index.find_path(&mdl_path).is_some() {
            found_models.push(name);
        }
    }
    found_models.sort();
    found_models
}

fn export_model(
    ctx: &PackContext,
    model_name: &str,
    out_pack_dir: &Path,
) -> Result<PathBuf, String> {
    let trmsh_path = format!("{}{}.trmsh", ctx.prefix, model_name);
    let trmbf_path = format!("{}{}.trmbf", ctx.prefix, model_name);
    let trmtr_path = format!("{}{}.trmtr", ctx.prefix, model_name);
    let trskl_path = format!("{}{}.trskl", ctx.prefix, model_name);

    let trmsh_idx = ctx
        .index
        .find_path(&trmsh_path)
        .ok_or_else(|| format!("missing {}", trmsh_path))?;
    let trmbf_idx = ctx
        .index
        .find_path(&trmbf_path)
        .ok_or_else(|| format!("missing {}", trmbf_path))?;

    let trmsh_bytes = ctx.gfpak.decoded_data(ctx.bytes, &ctx.gfpak.entries[trmsh_idx])?;
    let trmbf_bytes = ctx.gfpak.decoded_data(ctx.bytes, &ctx.gfpak.entries[trmbf_idx])?;

    let mut export = GltfExport::default();

    // Skeleton if present
    let skeleton = if let Some(trskl_idx) = ctx.index.find_path(&trskl_path) {
        if let Ok(skl_bytes) = ctx.gfpak.decoded_data(ctx.bytes, &ctx.gfpak.entries[trskl_idx]) {
            if let Ok(skl) = TrSkl::parse(&skl_bytes) {
                if let Ok(binds) = TrSkl::read_bind_records(&skl_bytes) {
                    if let Ok((nodes, skin)) = skeleton_from_tr(&skl, &binds, 0, 0) {
                        export.nodes = nodes;
                        export.skins.push(skin);
                        Some(skl)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Mesh
    let mesh = TrMbf::parse_with_layout(&trmbf_bytes, &trmsh_bytes)?;

    // Material & Textures
    let mut material_index = None;
    if let Some(trmtr_idx) = ctx.index.find_path(&trmtr_path) {
        if let Ok(mtr_bytes) = ctx.gfpak.decoded_data(ctx.bytes, &ctx.gfpak.entries[trmtr_idx]) {
            if let Ok(material) = TrMtr::parse(&mtr_bytes) {
                let mut base_color_uri = None;
                if let Some(tex_name) = material.base_color() {
                    let tex_path = format!("{}{}", ctx.prefix, tex_name);
                    if let Some(tex_idx) = ctx.index.find_path(&tex_path) {
                        if let Ok(bntx_bytes) =
                            ctx.gfpak.decoded_data(ctx.bytes, &ctx.gfpak.entries[tex_idx])
                        {
                            if let Ok(bntx) = Bntx::parse(&bntx_bytes) {
                                if !bntx.textures.is_empty() {
                                    if let Ok(tex_data) = bntx.textures[0].decode(&bntx_bytes) {
                                        let png_filename = format!("{}.png", tex_name);
                                        let png_file = out_pack_dir.join(&png_filename);
                                        let _ = tex_data.save_png(&png_file);
                                        base_color_uri = Some(png_filename);
                                    }
                                }
                            }
                        }
                    }
                }

                if base_color_uri.is_some() {
                    export.materials.push(GltfMaterial {
                        name: format!("{}_mat", model_name),
                        base_color_uri,
                        normal_uri: None, // Flat baseColor shading; normal map skipped as unproven shading
                    });
                    material_index = Some(0);
                }
            }
        }
    }

    export.primitives.push(primitive_from_tr(
        model_name,
        &mesh,
        &trmbf_bytes,
        skeleton.as_ref(),
        material_index,
    )?);

    let out_gltf = out_pack_dir.join(format!("{}.gltf", model_name));
    export.write(&out_gltf)?;

    Ok(out_gltf)
}
