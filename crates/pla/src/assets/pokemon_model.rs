//! Pokémon GFLXPACK assembly using in-band asset names and the observed
//! RomFS-relative FNV-1a name index. Hashes resolve entry identity; native
//! readers still validate each decoded payload and linked mesh/buffer pair.

use super::{bntx, gfpak::GfPak, gfpak_hash::GfPakNameIndex, gltf_export, tr};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct PokemonTexture {
    pub entry_index: usize,
    pub internal_name: String,
    pub image: Result<bntx::TextureData, String>,
}

#[derive(Debug, Clone)]
pub struct PokemonMaterial {
    pub entry_index: usize,
    pub references: Vec<String>,
    pub parsed: tr::TrMtr,
    /// `(material role, BNTX internal name)` links; only exact normalized stem
    /// matches are included. Missing/ambiguous links remain absent.
    pub texture_links: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct PokemonLodMesh {
    pub entry_index: usize,
    pub name: String,
    pub shape_index: usize,
    pub lod: Option<u32>,
    pub buffer_entry_index: usize,
    pub mesh: tr::TrMbf,
    pub buffer_bytes: Vec<u8>,
    pub mesh_bytes: Vec<u8>,
    pub material_entry_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct PokemonClip {
    pub name: String,
    pub animation_entry_index: usize,
    pub companion_entry_index: Option<usize>,
    pub parsed: Result<tr::TrAnmClip, String>,
}

#[derive(Debug, Clone)]
pub struct PokemonModel {
    pub pack_path: PathBuf,
    pub entry_count: usize,
    pub meshes: Vec<PokemonLodMesh>,
    pub skeleton: Option<tr::TrSkl>,
    pub skeleton_entry_index: Option<usize>,
    pub skeleton_parse_error: Option<String>,
    pub skeleton_node_count: Option<usize>,
    pub skeleton_node_type_counts: BTreeMap<u32, usize>,
    /// Structural root bind-vector length even when the native reader rejects
    /// node metadata; not a promise that its bind records can be consumed.
    pub bind_record_count: Option<usize>,
    pub bind_records: Vec<tr::TrSklBindRecord>,
    pub bind_record_error: Option<String>,
    pub materials: Vec<PokemonMaterial>,
    pub textures: Vec<PokemonTexture>,
    pub clips: Vec<PokemonClip>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug)]
struct DecodedEntry {
    index: usize,
    bytes: Vec<u8>,
    references: Vec<String>,
}

/// Decode and assemble model assets from one Pokémon GFLXPACK. Hashes are
/// resolved only from exact in-band filenames plus the verified RomFS layout.
pub fn assemble_pack(path: impl AsRef<Path>) -> Result<PokemonModel, String> {
    let path = path.as_ref();
    let pack_bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let pack = GfPak::parse(&pack_bytes)?;
    let names = GfPakNameIndex::parse(&pack_bytes, &pack)?;
    let base = model_base(path)?;
    let mut decoded = Vec::with_capacity(pack.entries.len());
    let mut diagnostics = Vec::new();
    for (index, entry) in pack.entries.iter().enumerate() {
        match pack.decoded_data(&pack_bytes, entry) {
            Ok(bytes) => decoded.push(DecodedEntry {
                index,
                references: referenced_names(&bytes),
                bytes,
            }),
            Err(error) => diagnostics.push(format!("entry {index}: decode failed: {error}")),
        }
    }

    let by_index = decoded
        .iter()
        .map(|entry| (entry.index, entry))
        .collect::<BTreeMap<_, _>>();
    let descriptor_name = format!("{}.trmdl", pack_stem(path)?);
    let descriptor_index = names.entry_index(&romfs_path(&base, "mdl", &descriptor_name))?;
    let descriptor = by_index
        .get(&descriptor_index)
        .ok_or("model descriptor entry did not decode")?;
    let mut materials = Vec::new();
    for file in files_by_extension(&descriptor.references, "trmtr") {
        let entry_index = names.entry_index(&romfs_path(&base, "mdl", &file))?;
        let entry = by_index
            .get(&entry_index)
            .ok_or_else(|| format!("material entry {entry_index} did not decode"))?;
        match tr::TrMtr::parse(&entry.bytes) {
            Ok(parsed) => materials.push(PokemonMaterial {
                entry_index,
                references: entry.references.clone(),
                parsed,
                texture_links: Vec::new(),
            }),
            Err(error) => {
                diagnostics.push(format!("material {file} at entry {entry_index}: {error}"))
            }
        }
    }

    let mut texture_entries = BTreeMap::<usize, (Vec<u8>, bntx::Bntx)>::new();
    for material in &materials {
        for (file, _) in &material.parsed.textures {
            let entry_index = names.entry_index(&romfs_path(&base, "mdl", file))?;
            if texture_entries.contains_key(&entry_index) {
                continue;
            }
            let entry = by_index
                .get(&entry_index)
                .ok_or_else(|| format!("texture entry {entry_index} did not decode"))?;
            let parsed = bntx::Bntx::parse(&entry.bytes)
                .map_err(|e| format!("texture {file} entry {entry_index}: {e}"))?;
            texture_entries.insert(entry_index, (entry.bytes.clone(), parsed));
        }
    }
    let mut textures = Vec::new();
    for (entry_index, (bytes, parsed)) in texture_entries {
        for texture_index in 0..parsed.textures.len() {
            let texture = &parsed.textures[texture_index];
            textures.push(PokemonTexture {
                entry_index,
                internal_name: texture.name.clone(),
                image: parsed.decode(&bytes, texture_index),
            });
        }
    }
    for material in &mut materials {
        for (file, role) in &material.parsed.textures {
            let target = normalized_stem(file);
            let matched = textures
                .iter()
                .filter(|texture| normalized_stem(&texture.internal_name) == target)
                .collect::<Vec<_>>();
            if matched.len() == 1 {
                material
                    .texture_links
                    .push((role.clone(), matched[0].internal_name.clone()));
            } else {
                diagnostics.push(format!(
                    "material entry {} texture {file}: {} internal-name matches",
                    material.entry_index,
                    matched.len()
                ));
            }
        }
    }

    let skeleton_name = descriptor
        .references
        .iter()
        .find(|name| name.to_ascii_lowercase().ends_with(".trskl"))
        .ok_or("descriptor has no skeleton filename")?;
    let skeleton_entry_index = names.entry_index(&romfs_path(&base, "mdl", skeleton_name))?;
    let skeleton_bytes = &by_index
        .get(&skeleton_entry_index)
        .ok_or("skeleton entry did not decode")?
        .bytes;
    let bind_record_count = root_vector_count(skeleton_bytes, 2);
    let skeleton_node_count = root_vector_count(skeleton_bytes, 1);
    let skeleton_node_type_counts = skeleton_node_types(skeleton_bytes).unwrap_or_default();
    let (skeleton, skeleton_parse_error) = match tr::TrSkl::parse(skeleton_bytes) {
        Ok(skeleton) => (Some(skeleton), None),
        Err(error) => (None, Some(error)),
    };
    let (bind_records, bind_record_error) = if skeleton.is_some() {
        match tr::TrSkl::read_bind_records(skeleton_bytes) {
            Ok(records) if records.is_empty() => (records, Some("parsed skeleton contains zero bind records; rest transforms are not inverse binds".into())),
            Ok(records) => (records, None),
            Err(error) => (Vec::new(), Some(error)),
        }
    } else {
        (Vec::new(), skeleton_parse_error.clone())
    };

    let material_entry_index = materials
        .iter()
        .find(|material| {
            !material
                .references
                .iter()
                .any(|r| r.to_ascii_lowercase().contains("rare"))
        })
        .map(|material| material.entry_index);
    let mut meshes = Vec::new();
    for mesh_file in files_by_extension(&descriptor.references, "trmsh") {
        let entry_index = names.entry_index(&romfs_path(&base, "mdl", &mesh_file))?;
        let entry = by_index
            .get(&entry_index)
            .ok_or_else(|| format!("mesh entry {entry_index} did not decode"))?;
        let buffer_file = entry
            .references
            .iter()
            .find(|name| name.to_ascii_lowercase().ends_with(".trmbf"))
            .ok_or_else(|| format!("mesh entry {entry_index} does not declare a .trmbf name"))?;
        let buffer_entry_index = names.entry_index(&romfs_path(&base, "mdl", buffer_file))?;
        let buffer = by_index
            .get(&buffer_entry_index)
            .ok_or_else(|| format!("buffer entry {buffer_entry_index} did not decode"))?;
        let Some(shape_count) = root_vector_count(&entry.bytes, 1) else {
            diagnostics.push(format!(
                "mesh {mesh_file} entry {entry_index}: shape vector unavailable"
            ));
            continue;
        };
        for shape_index in 0..shape_count {
            match tr::TrMbf::parse_skin_shape(&buffer.bytes, &entry.bytes, shape_index) {
                Ok(mesh) => {
                    let name = flatbuffer_vector_element(&entry.bytes, 1, shape_index)
                        .and_then(|shape_at| flatbuffer_string_field(&entry.bytes, shape_at, 0))
                        .unwrap_or_else(|| format!("{}#shape{shape_index}", stem(&mesh_file)));
                    meshes.push(PokemonLodMesh {
                    entry_index, name, shape_index, lod: lod_number(&mesh_file),
                    buffer_entry_index, mesh, buffer_bytes: buffer.bytes.clone(), mesh_bytes: entry.bytes.clone(), material_entry_index,
                    })
                },
                Err(error) => diagnostics.push(format!("mesh {mesh_file} shape {shape_index} entry {entry_index} / buffer entry {buffer_entry_index}: {error}")),
            }
        }
    }

    let mut animation_files = BTreeSet::new();
    for entry in &decoded {
        animation_files.extend(files_by_extension(&entry.references, "tranm"));
    }
    let mut clips = Vec::new();
    for file in animation_files {
        let animation_entry_index = names.entry_index(&romfs_path(&base, "anm", &file))?;
        let animation = by_index
            .get(&animation_entry_index)
            .ok_or_else(|| format!("animation entry {animation_entry_index} did not decode"))?;
        let companion_name = format!("{}.tracm", stem(&file));
        let companion_entry_index = names
            .entry_index(&romfs_path(&base, "anm", &companion_name))
            .ok();
        let parsed = tr::TrAnm::parse_tracks(&animation.bytes);
        clips.push(PokemonClip {
            name: stem(&file),
            animation_entry_index,
            companion_entry_index,
            parsed,
        });
    }
    clips.sort_by(|a, b| a.name.cmp(&b.name));
    meshes.sort_by(|a, b| a.lod.cmp(&b.lod).then_with(|| a.name.cmp(&b.name)));

    Ok(PokemonModel {
        pack_path: path.to_path_buf(),
        entry_count: pack.entries.len(),
        meshes,
        skeleton,
        skeleton_entry_index: Some(skeleton_entry_index),
        skeleton_parse_error,
        skeleton_node_count,
        skeleton_node_type_counts,
        bind_record_count,
        bind_records,
        bind_record_error,
        materials,
        textures,
        clips,
        diagnostics,
    })
}

/// Export one proven mesh and the uniquely linked base-colour/normal images.
pub fn write_static_lod(
    model: &PokemonModel,
    mesh_name: &str,
    gltf_path: impl AsRef<Path>,
) -> Result<(), String> {
    let mesh = model
        .meshes
        .iter()
        .find(|mesh| mesh.name == mesh_name)
        .ok_or("mesh name is not assembled")?;
    let mut export = gltf_export::GltfExport::default();
    let material_index = add_static_material(model, mesh, &mut export, gltf_path.as_ref())?;
    export.primitives.push(gltf_export::primitive_from_tr(
        &mesh.name,
        &mesh.mesh,
        &mesh.buffer_bytes,
        None,
        material_index,
    )?);
    export.write(gltf_path)
}

/// Export every parsed shape of one LOD as a single glTF model.
pub fn write_static_lod_level(
    model: &PokemonModel,
    lod: u32,
    gltf_path: impl AsRef<Path>,
) -> Result<(), String> {
    let mut export = gltf_export::GltfExport::default();
    for mesh in model.meshes.iter().filter(|mesh| mesh.lod == Some(lod)) {
        let material = add_static_material(model, mesh, &mut export, gltf_path.as_ref())?;
        export.primitives.push(gltf_export::primitive_from_tr(
            &mesh.name,
            &mesh.mesh,
            &mesh.buffer_bytes,
            None,
            material,
        )?);
    }
    if export.primitives.is_empty() {
        return Err(format!("LOD {lod} has no parsed shapes"));
    }
    export.write(gltf_path)
}

/// Export a mesh with one decoded skeletal clip when authored bind records are
/// present and the existing TR adapters accept them. Reference transforms are
/// never substituted for absent inverse binds.
pub fn write_skinned_lod(
    model: &PokemonModel,
    mesh_name: &str,
    clip_name: &str,
    gltf_path: impl AsRef<Path>,
) -> Result<(), String> {
    let skeleton = model.skeleton.as_ref().ok_or_else(|| {
        format!(
            "pack has no supported parsed skeleton: {}",
            model
                .skeleton_parse_error
                .as_deref()
                .unwrap_or("unknown reason")
        )
    })?;
    if skeleton.bind_count == 0 || model.bind_records.is_empty() {
        return Err(format!(
            "skeleton entry {} has {} bind records; skinned glTF requires explicit bind data (rest transforms are not inverse binds)",
            model
                .skeleton_entry_index
                .map_or_else(|| "unknown".into(), |i| i.to_string()),
            model.bind_record_count.unwrap_or(skeleton.bind_count)
        ));
    }
    let mesh = model
        .meshes
        .iter()
        .find(|mesh| mesh.name == mesh_name)
        .ok_or("mesh name is not assembled")?;
    let clip = model
        .clips
        .iter()
        .find(|clip| clip.name == clip_name)
        .ok_or("clip name is not assembled")?;
    let clip = clip
        .parsed
        .as_ref()
        .map_err(|error| format!("clip {clip_name}: {error}"))?;
    let material = mesh
        .material_entry_index
        .and_then(|index| model.materials.iter().find(|m| m.entry_index == index));
    let mut export = gltf_export::GltfExport::default();
    let material_index = if let Some(material) = material {
        let base_name = material
            .texture_links
            .iter()
            .find(|(role, _)| role == "BaseColorMap")
            .map(|(_, name)| name);
        let normal_name = material
            .texture_links
            .iter()
            .find(|(role, _)| role == "NormalMap")
            .map(|(_, name)| name);
        let base_uri = save_texture(model, base_name, gltf_path.as_ref())?;
        let normal_uri = save_texture(model, normal_name, gltf_path.as_ref())?;
        export.materials.push(gltf_export::GltfMaterial {
            name: format!("material-entry-{}", material.entry_index),
            base_color_uri: base_uri,
            normal_uri,
        });
        Some(0)
    } else {
        None
    };
    let (nodes, skin) = gltf_export::skeleton_from_tr(skeleton, &model.bind_records, 0, 0)?;
    export.nodes = nodes;
    export.skins.push(skin);
    export.primitives.push(gltf_export::primitive_from_tr(
        &mesh.name,
        &mesh.mesh,
        &mesh.buffer_bytes,
        Some(skeleton),
        material_index,
    )?);
    export.animations.push((
        clip_name.to_owned(),
        gltf_export::animation_tracks_from_tr(skeleton, clip)?,
    ));
    export.write(gltf_path)
}

fn save_texture(
    model: &PokemonModel,
    name: Option<&String>,
    gltf_path: &Path,
) -> Result<Option<String>, String> {
    let Some(name) = name else { return Ok(None) };
    let texture = model
        .textures
        .iter()
        .find(|texture| &texture.internal_name == name)
        .ok_or("linked texture disappeared")?;
    let image = texture
        .image
        .as_ref()
        .map_err(|e| format!("texture {name}: {e}"))?;
    let output = gltf_path.parent().unwrap_or_else(|| Path::new("."));
    let safe_name = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let filename = format!("{safe_name}.png");
    image
        .save_png(&output.join(&filename))
        .map_err(|e| e.to_string())?;
    Ok(Some(filename))
}

fn add_static_material(
    model: &PokemonModel,
    mesh: &PokemonLodMesh,
    export: &mut gltf_export::GltfExport,
    gltf_path: &Path,
) -> Result<Option<usize>, String> {
    let material = mesh
        .material_entry_index
        .and_then(|index| model.materials.iter().find(|m| m.entry_index == index));
    let base_name = material.and_then(|material| {
        let textures = material
            .texture_links
            .iter()
            .filter(|(role, _)| role == "BaseColorMap")
            .map(|(_, name)| name)
            .collect::<Vec<_>>();
        let hint = if mesh.name.contains("eye") {
            "_eye_alb"
        } else if mesh.name.contains("sippo") {
            "_body_b_alb"
        } else if mesh.name.contains("body") || mesh.name.contains("tongue") {
            "_body_a_alb"
        } else {
            ""
        };
        textures
            .iter()
            .copied()
            .find(|name| name.ends_with(hint))
            .or_else(|| textures.first().copied())
    });
    let normal_name = base_name.and_then(|name| {
        name.strip_suffix("_alb")
            .map(|prefix| format!("{prefix}_nrm"))
    });
    let normal_name = normal_name.as_ref().filter(|name| {
        model
            .textures
            .iter()
            .any(|texture| &texture.internal_name == *name)
    });
    let base_uri = save_texture(model, base_name, gltf_path)?;
    let normal_uri = save_texture(model, normal_name, gltf_path)?;
    let material_index = if base_uri.is_some() || normal_uri.is_some() {
        let index = export.materials.len();
        export.materials.push(gltf_export::GltfMaterial {
            name: format!("{}-material", mesh.name),
            base_color_uri: base_uri,
            normal_uri,
        });
        Some(index)
    } else {
        None
    };
    Ok(material_index)
}

fn referenced_names(bytes: &[u8]) -> Vec<String> {
    const EXTS: [&str; 7] = ["trmsh", "trmbf", "trmtr", "trskl", "tranm", "tracm", "bntx"];
    let mut found = BTreeSet::new();
    let mut start = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == 0 {
            if index > start && index - start < 512 {
                if let Ok(text) = std::str::from_utf8(&bytes[start..index]) {
                    if EXTS
                        .iter()
                        .any(|ext| text.to_ascii_lowercase().ends_with(&format!(".{ext}")))
                    {
                        found.insert(text.to_owned());
                    }
                }
            }
            start = index + 1;
        }
    }
    found.into_iter().collect()
}

fn files_by_extension(references: &[String], extension: &str) -> Vec<String> {
    references
        .iter()
        .filter(|name| {
            name.to_ascii_lowercase()
                .ends_with(&format!(".{extension}"))
        })
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn pack_stem(path: &Path) -> Result<String, String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .ok_or_else(|| "pack filename is not UTF-8".into())
}

fn model_base(path: &Path) -> Result<String, String> {
    let stem = pack_stem(path)?;
    let species = stem
        .split('_')
        .next()
        .ok_or("pack filename has no species component")?;
    if species.len() != 6 || !species.starts_with("pm") {
        return Err(format!("unexpected Pokémon pack stem {stem}"));
    }
    Ok(format!("bin/pokemon/{species}/{stem}"))
}

fn romfs_path(base: &str, directory: &str, filename: &str) -> String {
    format!("{base}/{directory}/{filename}")
}

fn root_vector_count(bytes: &[u8], field_index: usize) -> Option<usize> {
    let root_offset = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?) as usize;
    let vtable = table_vtable_offset(bytes, root_offset)?;
    let vtable_size =
        u16::from_le_bytes(bytes.get(vtable..vtable.checked_add(2)?)?.try_into().ok()?) as usize;
    if vtable_size < 4 || !vtable_size.is_multiple_of(2) {
        return None;
    }
    let slot = vtable.checked_add(4 + field_index.checked_mul(2)?)?;
    let field_offset =
        u16::from_le_bytes(bytes.get(slot..slot.checked_add(2)?)?.try_into().ok()?) as usize;
    if field_offset == 0 {
        return Some(0);
    }
    let field = root_offset.checked_add(field_offset)?;
    let vector = field.checked_add(u32::from_le_bytes(
        bytes.get(field..field.checked_add(4)?)?.try_into().ok()?,
    ) as usize)?;
    let count =
        u32::from_le_bytes(bytes.get(vector..vector.checked_add(4)?)?.try_into().ok()?) as usize;
    let end = vector.checked_add(4)?.checked_add(count.checked_mul(4)?)?;
    (end <= bytes.len()).then_some(count)
}

fn flatbuffer_vector_element(bytes: &[u8], root_field: usize, index: usize) -> Option<usize> {
    let root = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?) as usize;
    let vtable = table_vtable_offset(bytes, root)?;
    let vsize =
        u16::from_le_bytes(bytes.get(vtable..vtable.checked_add(2)?)?.try_into().ok()?) as usize;
    let slot = vtable.checked_add(4 + root_field.checked_mul(2)?)?;
    if slot + 2 > vtable + vsize {
        return None;
    }
    let field_offset = u16::from_le_bytes(bytes.get(slot..slot + 2)?.try_into().ok()?) as usize;
    if field_offset == 0 {
        return None;
    }
    let field = root.checked_add(field_offset)?;
    let vector = field.checked_add(u32::from_le_bytes(
        bytes.get(field..field.checked_add(4)?)?.try_into().ok()?,
    ) as usize)?;
    let count =
        u32::from_le_bytes(bytes.get(vector..vector.checked_add(4)?)?.try_into().ok()?) as usize;
    if index >= count {
        return None;
    }
    let element = vector.checked_add(4 + index.checked_mul(4)?)?;
    let target = element.checked_add(u32::from_le_bytes(
        bytes
            .get(element..element.checked_add(4)?)?
            .try_into()
            .ok()?,
    ) as usize)?;
    (target < bytes.len()).then_some(target)
}

fn flatbuffer_string_field(bytes: &[u8], table: usize, field_index: usize) -> Option<String> {
    let vtable = table_vtable_offset(bytes, table)?;
    let vsize =
        u16::from_le_bytes(bytes.get(vtable..vtable.checked_add(2)?)?.try_into().ok()?) as usize;
    let slot = vtable.checked_add(4 + field_index.checked_mul(2)?)?;
    if slot + 2 > vtable + vsize {
        return None;
    }
    let field_offset = u16::from_le_bytes(bytes.get(slot..slot + 2)?.try_into().ok()?) as usize;
    if field_offset == 0 {
        return None;
    }
    let field = table.checked_add(field_offset)?;
    let target = field.checked_add(u32::from_le_bytes(
        bytes.get(field..field.checked_add(4)?)?.try_into().ok()?,
    ) as usize)?;
    let length =
        u32::from_le_bytes(bytes.get(target..target.checked_add(4)?)?.try_into().ok()?) as usize;
    if length == 0 || length > 1024 {
        return None;
    }
    let start = target.checked_add(4)?;
    let end = start.checked_add(length)?;
    if bytes.get(end).copied()? != 0 {
        return None;
    }
    std::str::from_utf8(bytes.get(start..end)?)
        .ok()
        .map(str::to_owned)
}

fn skeleton_node_types(bytes: &[u8]) -> Option<BTreeMap<u32, usize>> {
    let node_count = root_vector_count(bytes, 1)?;
    let root_offset = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?) as usize;
    let vtable = table_vtable_offset(bytes, root_offset)?;
    let slot = vtable.checked_add(4 + 2)?;
    let field_offset =
        u16::from_le_bytes(bytes.get(slot..slot.checked_add(2)?)?.try_into().ok()?) as usize;
    let vector_field = root_offset.checked_add(field_offset)?;
    let vector = vector_field.checked_add(u32::from_le_bytes(
        bytes
            .get(vector_field..vector_field.checked_add(4)?)?
            .try_into()
            .ok()?,
    ) as usize)?;
    let mut counts = BTreeMap::new();
    for index in 0..node_count {
        let target_field = vector.checked_add(4 + index.checked_mul(4)?)?;
        let node = target_field.checked_add(u32::from_le_bytes(
            bytes
                .get(target_field..target_field.checked_add(4)?)?
                .try_into()
                .ok()?,
        ) as usize)?;
        let node_vtable = table_vtable_offset(bytes, node)?;
        let node_vtable_size = u16::from_le_bytes(
            bytes
                .get(node_vtable..node_vtable.checked_add(2)?)?
                .try_into()
                .ok()?,
        ) as usize;
        if node_vtable_size < 4 || !node_vtable_size.is_multiple_of(2) {
            return None;
        }
        let type_slot = node_vtable.checked_add(4 + 2 * 7)?;
        let type_offset = if type_slot + 2 <= node_vtable + node_vtable_size {
            u16::from_le_bytes(bytes.get(type_slot..type_slot + 2)?.try_into().ok()?) as usize
        } else {
            0
        };
        let kind = if type_offset == 0 {
            0
        } else {
            let at = node.checked_add(type_offset)?;
            u32::from_le_bytes(bytes.get(at..at.checked_add(4)?)?.try_into().ok()?)
        };
        *counts.entry(kind).or_insert(0) += 1;
    }
    Some(counts)
}

fn table_vtable_offset(bytes: &[u8], table: usize) -> Option<usize> {
    let delta = i32::from_le_bytes(bytes.get(table..table.checked_add(4)?)?.try_into().ok()?);
    match delta.cmp(&0) {
        std::cmp::Ordering::Greater => table.checked_sub(delta as usize),
        std::cmp::Ordering::Less => table.checked_add(delta.unsigned_abs() as usize),
        std::cmp::Ordering::Equal => None,
    }
}

fn stem(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
        .to_owned()
}

fn normalized_stem(name: &str) -> String {
    stem(name).to_ascii_lowercase()
}

fn lod_number(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    let Some(marker) = lower.rfind("_lod") else {
        return Some(0);
    };
    let at = marker + 4;
    let digits = lower[at..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    digits.parse().ok()
}
