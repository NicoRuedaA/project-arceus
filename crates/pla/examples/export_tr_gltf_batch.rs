//! Export skeleton-less TR mesh buffers from the checked-in Task B index.
//!
//! Usage: cargo run -p pla --example export_tr_gltf_batch --offline -- <bin-root> <asset-index.tsv> <gltf-root> <png-root> <index.tsv>

use pla::assets::gltf_export::{primitive_from_tr, GltfExport, GltfMaterial};
use pla::assets::tr::{TrMbf, TrMtr};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

fn main() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 5 {
        return Err(
            "expected <bin-root> <asset-index.tsv> <gltf-root> <png-root> <index.tsv>".into(),
        );
    }
    let bin_root = PathBuf::from(&args[0]);
    let input_index = File::open(&args[1]).map_err(|e| format!("open asset index: {e}"))?;
    let output_root = PathBuf::from(&args[2]);
    let png_root = PathBuf::from(&args[3]);
    if let Some(parent) = Path::new(&args[4]).parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create index directory: {e}"))?;
    }
    let mut output_index =
        BufWriter::new(File::create(&args[4]).map_err(|e| format!("create export index: {e}"))?);
    writeln!(output_index, "source\toutput\tstatus\treason").map_err(|e| e.to_string())?;
    let mut totals = [0usize; 4];
    for line in BufReader::new(input_index).lines() {
        let line = line.map_err(|e| format!("read asset index: {e}"))?;
        if line.starts_with('#') || line.starts_with("source:string") || line.trim().is_empty() {
            continue;
        }
        let Some(source) = line.split('\t').next() else {
            continue;
        };
        if !source.ends_with(".trmbf") {
            continue;
        }
        totals[0] += 1;
        let relative = Path::new(source);
        let input = bin_root.join(relative);
        let result = export_one(&input, relative, &output_root, &png_root, &bin_root);
        match result {
            Ok(output) => {
                totals[1] += 1;
                writeln!(output_index, "{}\t{}\tok\t", source, output.display())
                    .map_err(|e| e.to_string())?;
            }
            Err(reason) => {
                totals[2] += 1;
                let reason = reason.replace(['\t', '\n', '\r'], " ");
                writeln!(output_index, "{}\t\tblocked\t{}", source, reason)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    output_index.flush().map_err(|e| e.to_string())?;
    totals[3] = totals[0] - totals[1] - totals[2];
    println!(
        "models={} exported={} blocked={} other={}",
        totals[0], totals[1], totals[2], totals[3]
    );
    Ok(())
}

fn export_one(
    input: &Path,
    source_relative: &Path,
    output_root: &Path,
    png_root: &Path,
    bin_root: &Path,
) -> Result<PathBuf, String> {
    let skeleton_path = input.with_extension("trskl");
    if skeleton_path.exists() {
        return Err("skinned model deferred to the B3 recognizability gate".into());
    }
    let layout_path = input.with_extension("trmsh");
    let bytes = read_capped(input)?;
    let layout = read_capped(&layout_path)?;
    let mesh = TrMbf::parse_with_layout(&bytes, &layout)?;
    let mut export = GltfExport::default();
    let mut material_index = None;
    let material_path = input.with_extension("trmtr");
    if material_path.exists() {
        let material_bytes = read_capped(&material_path)?;
        if let Ok(material) = TrMtr::parse(&material_bytes) {
            let base_color_uri = texture_uri(
                material.base_color(),
                source_relative,
                output_root,
                png_root,
                bin_root,
            )?;
            let normal_uri = texture_uri(
                material.texture("NormalMap"),
                source_relative,
                output_root,
                png_root,
                bin_root,
            )?;
            if base_color_uri.is_some() || normal_uri.is_some() {
                export.materials.push(GltfMaterial {
                    name: material_path
                        .file_stem()
                        .and_then(|v| v.to_str())
                        .unwrap_or("material")
                        .to_owned(),
                    base_color_uri,
                    normal_uri,
                });
                material_index = Some(0);
            }
        }
    }
    export.primitives.push(primitive_from_tr(
        input.file_stem().and_then(|v| v.to_str()).unwrap_or("mesh"),
        &mesh,
        &bytes,
        None,
        material_index,
    )?);
    let output = output_root.join(source_relative).with_extension("gltf");
    export.write(&output)?;
    Ok(output)
}

fn texture_uri(
    texture_name: Option<&str>,
    source_relative: &Path,
    output_root: &Path,
    png_root: &Path,
    bin_root: &Path,
) -> Result<Option<String>, String> {
    let Some(texture_name) = texture_name else {
        return Ok(None);
    };
    let texture_relative = source_relative
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(texture_name);
    let bntx = bin_root.join(&texture_relative);
    if !bntx.is_file() {
        return Err(format!(
            "material texture not found beside model: {texture_name}"
        ));
    }
    let png_relative = texture_relative.with_extension("png");
    let png = png_root.join(&png_relative);
    if !png.is_file() {
        return Err(format!(
            "exported PNG missing for material texture: {}",
            png.display()
        ));
    }
    let gltf = output_root.join(source_relative).with_extension("gltf");
    Ok(Some(relative_uri(
        gltf.parent().unwrap_or(output_root),
        &png,
    )?))
}

fn read_capped(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path).map_err(|e| format!("stat {}: {e}", path.display()))?;
    if metadata.len() > 512 * 1024 * 1024 {
        return Err(format!("input exceeds 512 MB cap: {}", path.display()));
    }
    std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))
}

fn relative_uri(from: &Path, to: &Path) -> Result<String, String> {
    let from = from.components().collect::<Vec<_>>();
    let to = to.components().collect::<Vec<_>>();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts = vec!["..".to_owned(); from.len().saturating_sub(common)];
    parts.extend(
        to.iter()
            .skip(common)
            .map(|part| part.as_os_str().to_string_lossy().into_owned()),
    );
    if parts.is_empty() {
        return Err("texture URI resolves to its containing directory".into());
    }
    Ok(parts.join("/"))
}
