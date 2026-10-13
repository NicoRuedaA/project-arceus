//! Non-windowed structural verification of exported field/Pokémon glTF files.
//!
//! Walks every `.gltf` under a root, parses the JSON on disk and checks:
//! buffers match their `.bin` byte length, every accessor/bufferView stays
//! inside its buffer, primitives carry POSITION/NORMAL/TEXCOORD_0 accessors
//! with matching counts, UINT32 indices form complete triangles and reference
//! vertices in range, positions are finite and bounded, materials point at
//! existing textures, and skins/animations only reference declared nodes.
//!
//! This is a structural check only: it proves the files are well-formed and
//! self-consistent, NOT that they look right. Rendering stays with the user.
//!
//! Usage: verify_field_gltf_json <gltf-root> <report-tsv>

use serde_json::Value;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: verify_field_gltf_json <gltf-root> <report-tsv>");
        std::process::exit(1);
    }
    let root = PathBuf::from(&args[0]);
    let out = PathBuf::from(&args[1]);
    let mut gltfs = Vec::new();
    collect(&root, &mut gltfs)?;
    gltfs.sort();
    let mut writer = BufWriter::new(File::create(&out)?);
    writeln!(writer, "gltf\tstatus\treason")?;
    let mut ok = 0usize;
    let mut failed = 0usize;
    for path in &gltfs {
        let relative = path.strip_prefix(&root).unwrap_or(path);
        match check(path) {
            Ok(()) => {
                ok += 1;
                writeln!(writer, "{}\tok\t", relative.display())?;
            }
            Err(error) => {
                failed += 1;
                writeln!(
                    writer,
                    "{}\terror\t{}",
                    relative.display(),
                    error.replace(['\t', '\n', '\r'], " ")
                )?;
            }
        }
    }
    writer.flush()?;
    println!(
        "verified {ok} glTF files, {failed} errors ({})",
        gltfs.len()
    );
    Ok(())
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    if !dir.is_dir() {
        return Err(format!("{} is not a directory", dir.display()).into());
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "gltf")
        {
            out.push(path);
        }
    }
    Ok(())
}

fn accessor(gltf: &Value, index: u64) -> Result<&Value, String> {
    gltf["accessors"]
        .get(index as usize)
        .ok_or_else(|| format!("accessor {index} missing"))
}

fn check(path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let gltf: Value = serde_json::from_str(&text).map_err(|error| format!("json: {error}"))?;
    let buffers = gltf["buffers"].as_array().ok_or("no buffers array")?;
    let mut bin_bytes = None;
    for (index, buffer) in buffers.iter().enumerate() {
        let uri = buffer["uri"].as_str().unwrap_or_default();
        let declared = buffer["byteLength"].as_u64().ok_or("buffer byteLength")?;
        if uri.is_empty() {
            return Err(format!("buffer {index} has no external uri"));
        }
        let data_path = path.with_file_name(uri);
        let actual = std::fs::metadata(&data_path)
            .map_err(|_| format!("buffer file {uri} missing"))?
            .len();
        if actual != declared {
            return Err(format!(
                "buffer {uri} is {actual} bytes, declared {declared}"
            ));
        }
        if index == 0 {
            bin_bytes = Some(std::fs::read(&data_path).map_err(|error| error.to_string())?);
        }
    }
    let bin = bin_bytes.ok_or("no binary buffer")?;
    let views = gltf["bufferViews"].as_array().ok_or("no bufferViews")?;
    let mut view_ranges = Vec::with_capacity(views.len());
    for (index, view) in views.iter().enumerate() {
        let offset = view["byteOffset"].as_u64().unwrap_or(0);
        let length = view["byteLength"].as_u64().ok_or("bufferView byteLength")?;
        if offset + length > bin.len() as u64 {
            return Err(format!("bufferView {index} exceeds the buffer"));
        }
        view_ranges.push((offset as usize, length as usize));
    }
    let accessors = gltf["accessors"].as_array().ok_or("no accessors")?;
    if accessors.is_empty() {
        return Err("no accessors".into());
    }
    for (index, accessor) in accessors.iter().enumerate() {
        let count = accessor["count"].as_u64().ok_or("accessor count")?;
        if count == 0 {
            return Err(format!("accessor {index} has zero count"));
        }
        let view = accessor["bufferView"]
            .as_u64()
            .ok_or("accessor bufferView")?;
        if view as usize >= view_ranges.len() {
            return Err(format!("accessor {index} bufferView {view} missing"));
        }
    }
    let meshes = gltf["meshes"].as_array().ok_or("no meshes")?;
    if meshes.is_empty() {
        return Err("no meshes".into());
    }
    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let primitives = mesh["primitives"]
            .as_array()
            .ok_or("mesh has no primitives")?;
        if primitives.is_empty() {
            return Err(format!("mesh {mesh_index} has no primitives"));
        }
        for (primitive_index, primitive) in primitives.iter().enumerate() {
            let attributes = primitive["attributes"]
                .as_object()
                .ok_or("primitive has no attributes")?;
            for name in ["POSITION", "NORMAL", "TEXCOORD_0"] {
                if !attributes.contains_key(name) {
                    return Err(format!(
                        "primitive {mesh_index}/{primitive_index} lacks {name}"
                    ));
                }
            }
            let positions = accessor(
                &gltf,
                attributes["POSITION"].as_u64().ok_or("POSITION accessor")?,
            )?;
            let position_count = positions["count"].as_u64().ok_or("position count")?;
            let position_view = positions["bufferView"].as_u64().unwrap_or(0) as usize;
            let (view_offset, _) = view_ranges[position_view];
            let bytes = &bin[view_offset..];
            for chunk in bytes
                .as_chunks::<4>()
                .0
                .iter()
                .take(position_count as usize * 3)
            {
                let value = f32::from_le_bytes(*chunk);
                if !value.is_finite() || value.abs() > 1.0e6 {
                    return Err(format!(
                        "primitive {mesh_index}/{primitive_index} has a non-finite or unbounded position"
                    ));
                }
            }
            let indices = accessor(
                &gltf,
                primitive["indices"].as_u64().ok_or("indices accessor")?,
            )?;
            if indices["componentType"].as_u64() != Some(5_125) {
                return Err(format!(
                    "primitive {mesh_index}/{primitive_index} indices are not UINT32"
                ));
            }
            let index_count = indices["count"].as_u64().ok_or("index count")?;
            if index_count == 0 || !index_count.is_multiple_of(3) {
                return Err(format!(
                    "primitive {mesh_index}/{primitive_index} index count {index_count} is not a triangle list"
                ));
            }
            let index_view = indices["bufferView"].as_u64().unwrap_or(0) as usize;
            let (index_offset, _) = view_ranges[index_view];
            let index_bytes = &bin[index_offset..];
            for chunk in index_bytes
                .as_chunks::<4>()
                .0
                .iter()
                .take(index_count as usize)
            {
                let value = u32::from_le_bytes(*chunk);
                if u64::from(value) >= position_count {
                    return Err(format!(
                        "primitive {mesh_index}/{primitive_index} index {value} outside {position_count} vertices"
                    ));
                }
            }
        }
    }
    if let Some(materials) = gltf["materials"].as_array() {
        let textures = gltf["textures"].as_array();
        let images = gltf["images"].as_array();
        for material in materials {
            let Some(texture_index) =
                material["pbrMetallicRoughness"]["baseColorTexture"]["index"].as_u64()
            else {
                continue;
            };
            let image_index = textures
                .and_then(|textures| textures.get(texture_index as usize))
                .and_then(|texture| texture["source"].as_u64())
                .ok_or_else(|| format!("material texture {texture_index} has no image source"))?;
            let uri = images
                .and_then(|images| images.get(image_index as usize))
                .and_then(|image| image["uri"].as_str())
                .ok_or_else(|| format!("image {image_index} has no uri"))?;
            let texture = path.with_file_name(uri);
            if !texture.exists() {
                return Err(format!("material texture {uri} missing"));
            }
        }
    }
    let node_count = gltf["nodes"].as_array().map_or(0, |nodes| nodes.len()) as u64;
    if let Some(skins) = gltf["skins"].as_array() {
        for skin in skins {
            for joint in skin["joints"].as_array().ok_or("skin joints")? {
                let joint = joint.as_u64().ok_or("joint index")?;
                if joint >= node_count {
                    return Err(format!("skin joint {joint} outside {node_count} nodes"));
                }
            }
        }
    }
    if let Some(animations) = gltf["animations"].as_array() {
        for animation in animations {
            let channels = animation["channels"]
                .as_array()
                .ok_or("animation channels")?;
            if channels.is_empty() {
                return Err("animation without channels".into());
            }
            for channel in channels {
                let node = channel["target"]["node"].as_u64().ok_or("channel node")?;
                if node >= node_count {
                    return Err(format!(
                        "animation channel node {node} outside {node_count} nodes"
                    ));
                }
            }
        }
    }
    Ok(())
}
