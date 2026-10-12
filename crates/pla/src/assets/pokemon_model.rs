//! Pokémon GFLXPACK assembly using in-band asset names and the observed
//! RomFS-relative FNV-1a name index. Hashes resolve entry identity; native
//! readers still validate each decoded payload and linked mesh/buffer pair.

use super::{bntx, gfpak::GfPak, gltf_export, tr};

/// Exact full-path lookup over the pack's verified path-hash table
/// (the shared [`GfPak::entry_index_by_path`] implementation).
struct PackPaths<'a>(&'a GfPak);

impl PackPaths<'_> {
    fn entry_index(&self, path: &str) -> Result<usize, String> {
        self.0
            .entry_index_by_path(path)?
            .ok_or_else(|| format!("GFLXPACK path hash not found: {path}"))
    }
}
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

const TRMBF_VERTEX_OFFSET: usize = 0x44;

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
    /// Decoded material bytes are retained for sized, length-prefixed property readback.
    pub raw_bytes: Vec<u8>,
    /// `(material role, BNTX internal name)` links; only exact normalized stem
    /// matches are included. Missing/ambiguous links remain absent.
    pub texture_links: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct PokemonLodMesh {
    pub entry_index: usize,
    pub name: String,
    /// TRMSH `SubMesh.AppliedMaterial`, preserved as authored.
    pub material_binding: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerBlendFormula {
    /// Hypothesis: albedo multiplied by the clamped weighted sum of mask layers.
    WeightedSum,
    /// Hypothesis: sequentially lerp albedo toward albedo times each layer color.
    SequentialLerp,
}

/// Compose an offline base-color texture from the observed albedo and layer mask.
/// Neither formula is claimed to match the game's native shader.
pub fn bake_layered_base_color(
    albedo: &bntx::TextureData,
    mask: &bntx::TextureData,
    layers: &[[f32; 4]; 4],
    formula: LayerBlendFormula,
) -> Result<bntx::TextureData, String> {
    let expected = (albedo.width as usize)
        .checked_mul(albedo.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or("layered texture size overflow")?;
    let mask_expected = (mask.width as usize)
        .checked_mul(mask.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or("layer-mask texture size overflow")?;
    if albedo.rgba.len() != expected || mask.rgba.len() != mask_expected {
        return Err("albedo or layer-mask payload size mismatch".into());
    }
    if albedo.width == 0 || albedo.height == 0 || mask.width == 0 || mask.height == 0 {
        return Err("albedo or layer-mask dimensions are zero".into());
    }
    if layers.iter().flatten().any(|value| !value.is_finite()) {
        return Err("non-finite material layer color".into());
    }
    let mut rgba = vec![0; expected];
    for (pixel, output) in rgba.chunks_exact_mut(4).enumerate() {
        let x = pixel % albedo.width as usize;
        let y = pixel / albedo.width as usize;
        let mask_x = ((2 * x + 1) * mask.width as usize / (2 * albedo.width as usize))
            .min(mask.width as usize - 1);
        let mask_y = ((2 * y + 1) * mask.height as usize / (2 * albedo.height as usize))
            .min(mask.height as usize - 1);
        let offset = pixel * 4;
        let mask_offset = (mask_y * mask.width as usize + mask_x) * 4;
        let source = albedo.rgba[offset..offset + 4]
            .iter()
            .map(|value| srgb_to_linear(f32::from(*value) / 255.0))
            .collect::<Vec<_>>();
        let weights = mask.rgba[mask_offset..mask_offset + 4]
            .iter()
            .map(|value| f32::from(*value) / 255.0)
            .collect::<Vec<_>>();
        for channel in 0..3 {
            let result = match formula {
                LayerBlendFormula::WeightedSum => {
                    let color = (0..4)
                        .map(|layer| weights[layer] * layers[layer][channel])
                        .sum::<f32>()
                        .clamp(0.0, 1.0);
                    source[channel] * color
                }
                LayerBlendFormula::SequentialLerp => {
                    (0..4).fold(source[channel], |current, layer| {
                        let target = source[channel] * layers[layer][channel];
                        current + (target - current) * weights[layer].clamp(0.0, 1.0)
                    })
                }
            };
            output[channel] = (linear_to_srgb(result.clamp(0.0, 1.0)) * 255.0).round() as u8;
        }
        output[3] = albedo.rgba[offset + 3];
    }
    Ok(bntx::TextureData {
        width: albedo.width,
        height: albedo.height,
        rgba,
    })
}

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
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
    let names = PackPaths(&pack);
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
                raw_bytes: entry.bytes.clone(),
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
                    let shape_name = flatbuffer_vector_element(&entry.bytes, 1, shape_index)
                        .and_then(|shape_at| flatbuffer_string_field(&entry.bytes, shape_at, 0))
                        .unwrap_or_else(|| format!("{}#shape{shape_index}", stem(&mesh_file)));
                    let submeshes = match shape_submeshes(&entry.bytes, shape_index) {
                        Some(submeshes) if !submeshes.is_empty() => submeshes,
                        _ => {
                            diagnostics.push(format!(
                                "mesh {mesh_file} shape {shape_index}: submesh/material records unavailable"
                            ));
                            continue;
                        }
                    };
                    for (submesh_index, submesh) in submeshes.into_iter().enumerate() {
                        let end = match submesh.index_offset.checked_add(submesh.index_count) {
                            Some(end) if end <= mesh.indices.len() => end,
                            _ => {
                                diagnostics.push(format!(
                                    "mesh {mesh_file} shape {shape_index} submesh {submesh_index}: index range outside shape"
                                ));
                                continue;
                            }
                        };
                        if submesh.index_count == 0 || !submesh.index_count.is_multiple_of(3) {
                            diagnostics.push(format!(
                                "mesh {mesh_file} shape {shape_index} submesh {submesh_index}: unsupported triangle index count {}",
                                submesh.index_count
                            ));
                            continue;
                        }
                        let (submesh_data, submesh_buffer) = match compact_submesh(
                            &mesh,
                            &buffer.bytes,
                            &mesh.indices[submesh.index_offset..end],
                        ) {
                            Ok(compacted) => compacted,
                            Err(error) => {
                                diagnostics.push(format!(
                                    "mesh {mesh_file} shape {shape_index} submesh {submesh_index}: {error}"
                                ));
                                continue;
                            }
                        };
                        let name = format!("{shape_name}#{}", submesh.applied_material);
                        meshes.push(PokemonLodMesh {
                            entry_index,
                            name,
                            material_binding: submesh.applied_material,
                            shape_index,
                            lod: lod_number(&mesh_file),
                            buffer_entry_index,
                            mesh: submesh_data,
                            buffer_bytes: submesh_buffer,
                            mesh_bytes: entry.bytes.clone(),
                            material_entry_index,
                        });
                    }
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
    let material_index = add_static_material(model, mesh, &mut export, gltf_path.as_ref(), None)?;
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
        let material = add_static_material(model, mesh, &mut export, gltf_path.as_ref(), None)?;
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
    add_mesh_nodes(&mut export, None, None)?;
    export.write(gltf_path)
}

/// Export a complete LOD using one explicitly hypothetical layered-color bake.
pub fn write_static_lod_level_baked(
    model: &PokemonModel,
    lod: u32,
    gltf_path: impl AsRef<Path>,
    formula: LayerBlendFormula,
) -> Result<(), String> {
    let mut export = gltf_export::GltfExport::default();
    for mesh in model.meshes.iter().filter(|mesh| mesh.lod == Some(lod)) {
        let material =
            add_static_material(model, mesh, &mut export, gltf_path.as_ref(), Some(formula))?;
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
    add_mesh_nodes(&mut export, None, None)?;
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
    let root = export.skins.first().and_then(|skin| skin.skeleton);
    add_mesh_nodes(&mut export, Some(0), root)?;
    export.write(gltf_path)
}

/// Export every parsed shape in an LOD with the same parsed skeleton and clip.
pub fn write_skinned_lod_level(
    model: &PokemonModel,
    lod: u32,
    clip_name: &str,
    gltf_path: impl AsRef<Path>,
    formula: Option<LayerBlendFormula>,
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
        return Err("skinned export requires authored bind records".into());
    }
    let clip = model
        .clips
        .iter()
        .find(|clip| clip.name == clip_name)
        .ok_or("clip name is not assembled")?;
    let clip = clip
        .parsed
        .as_ref()
        .map_err(|error| format!("clip {clip_name}: {error}"))?;
    let mut export = gltf_export::GltfExport::default();
    let mut primitive_count = 0;
    for mesh in model.meshes.iter().filter(|mesh| mesh.lod == Some(lod)) {
        let material = add_static_material(model, mesh, &mut export, gltf_path.as_ref(), formula)?;
        export.primitives.push(gltf_export::primitive_from_tr(
            &mesh.name,
            &mesh.mesh,
            &mesh.buffer_bytes,
            Some(skeleton),
            material,
        )?);
        primitive_count += 1;
    }
    if primitive_count == 0 {
        return Err(format!("LOD {lod} has no parsed shapes"));
    }
    let (nodes, skin) = gltf_export::skeleton_from_tr(skeleton, &model.bind_records, 0, 0)?;
    export.nodes = nodes;
    export.skins.push(skin);
    export.animations.push((
        clip_name.to_owned(),
        gltf_export::animation_tracks_from_tr(skeleton, clip)?,
    ));
    let root = export.skins.first().and_then(|skin| skin.skeleton);
    add_mesh_nodes(&mut export, Some(0), root)?;
    export.write(gltf_path)
}

fn add_mesh_nodes(
    export: &mut gltf_export::GltfExport,
    skin: Option<usize>,
    parent: Option<usize>,
) -> Result<(), String> {
    for mesh in 0..export.primitives.len() {
        let node_index = export.nodes.len();
        export.nodes.push(gltf_export::GltfNode {
            name: export.primitives[mesh].name.clone(),
            children: Vec::new(),
            mesh: Some(mesh),
            skin,
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        });
        if let Some(parent) = parent {
            export
                .nodes
                .get_mut(parent)
                .ok_or("mesh parent node is outside export node table")?
                .children
                .push(node_index);
        }
    }
    Ok(())
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
    formula: Option<LayerBlendFormula>,
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
        let binding = mesh.material_binding.to_ascii_lowercase();
        let hint = match binding.as_str() {
            "l_eye" | "r_eye" => "_eye_alb",
            "body_a" => "_body_a_alb",
            "body_b" => "_body_b_alb",
            "body_c" => "_body_c_alb",
            _ => "",
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
    let base_uri = match (material, base_name, formula) {
        (Some(material), Some(name), Some(formula)) => Some(save_baked_base_color(
            model, material, name, formula, gltf_path,
        )?),
        (_, name, _) => save_texture(model, name, gltf_path)?,
    };
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

fn save_baked_base_color(
    model: &PokemonModel,
    material: &PokemonMaterial,
    albedo_name: &str,
    formula: LayerBlendFormula,
    gltf_path: &Path,
) -> Result<String, String> {
    let base_files = material
        .parsed
        .textures
        .iter()
        .filter(|(_, role)| role == "BaseColorMap")
        .map(|(file, _)| file)
        .collect::<Vec<_>>();
    let group = base_files
        .iter()
        .position(|file| normalized_stem(file) == normalized_stem(albedo_name))
        .ok_or("albedo has no declared material group")?;
    let mask_file = material
        .parsed
        .textures
        .iter()
        .filter(|(_, role)| role == "LayerMaskMap")
        .nth(group)
        .map(|(file, _)| file)
        .ok_or("material group has no layer-mask texture")?;
    let albedo = decoded_texture(model, albedo_name)?;
    let mask = decoded_texture_by_file(model, mask_file)?;
    let layers: [Option<[f32; 4]>; 4] = std::array::from_fn(|index| {
        let property = format!("BaseColorLayer{}", index + 1);
        named_vector4_values(&material.raw_bytes, &property)
            .and_then(|values| values.get(group).copied())
    });
    let layers = layers
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or("material layer color count does not match texture groups")?;
    let layers: [[f32; 4]; 4] = layers.try_into().map_err(|_| "invalid layer color count")?;
    let baked = bake_layered_base_color(albedo, mask, &layers, formula)?;
    let suffix = match formula {
        LayerBlendFormula::WeightedSum => "weighted_sum",
        LayerBlendFormula::SequentialLerp => "sequential_lerp",
    };
    let filename = format!("{albedo_name}_{suffix}.png");
    let output = gltf_path.parent().unwrap_or_else(|| Path::new("."));
    baked
        .save_png(&output.join(&filename))
        .map_err(|error| error.to_string())?;
    Ok(filename)
}

fn decoded_texture<'a>(
    model: &'a PokemonModel,
    internal_name: &str,
) -> Result<&'a bntx::TextureData, String> {
    let texture = model
        .textures
        .iter()
        .find(|texture| texture.internal_name == internal_name)
        .ok_or_else(|| format!("texture {internal_name} is not assembled"))?;
    texture
        .image
        .as_ref()
        .map_err(|error| format!("texture {internal_name}: {error}"))
}

fn decoded_texture_by_file<'a>(
    model: &'a PokemonModel,
    file: &str,
) -> Result<&'a bntx::TextureData, String> {
    let wanted = normalized_stem(file);
    let texture = model
        .textures
        .iter()
        .find(|texture| normalized_stem(&texture.internal_name) == wanted)
        .ok_or_else(|| format!("texture file {file} is not assembled"))?;
    texture
        .image
        .as_ref()
        .map_err(|error| format!("texture {file}: {error}"))
}

fn named_vector4_values(bytes: &[u8], name: &str) -> Option<Vec<[f32; 4]>> {
    let needle = name.as_bytes();
    let mut values = Vec::new();
    let mut search_from = 0;
    while search_from < bytes.len() {
        let Some(relative) = bytes[search_from..]
            .windows(needle.len())
            .position(|window| window == needle)
        else {
            break;
        };
        let at = search_from + relative;
        if at < 24
            || u32::from_le_bytes(bytes[at - 24..at - 20].try_into().ok()?) != 20
            || u32::from_le_bytes(bytes[at - 4..at].try_into().ok()?) as usize != needle.len()
        {
            return None;
        }
        let mut vector = [0.0; 4];
        for (index, value) in vector.iter_mut().enumerate() {
            let start = at - 20 + index * 4;
            *value = f32::from_le_bytes(bytes[start..start + 4].try_into().ok()?);
        }
        if vector.iter().any(|value| !value.is_finite()) {
            return None;
        }
        values.push(vector);
        search_from = at + needle.len();
    }
    Some(values)
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

#[derive(Debug)]
struct PokemonSubMesh {
    index_count: usize,
    index_offset: usize,
    applied_material: String,
}

fn compact_submesh(
    mesh: &tr::TrMbf,
    buffer: &[u8],
    indices: &[u16],
) -> Result<(tr::TrMbf, Vec<u8>), String> {
    if indices.is_empty() || mesh.vertex_stride == 0 {
        return Err("empty submesh or zero vertex stride".into());
    }
    let source_vertices = mesh.vertices(buffer);
    let used = indices.iter().copied().collect::<BTreeSet<_>>();
    if used
        .iter()
        .any(|index| usize::from(*index) >= mesh.vertex_count)
    {
        return Err("submesh vertex index is outside its source mesh".into());
    }
    let remap = used
        .iter()
        .enumerate()
        .map(|(new, old)| (*old, new as u16))
        .collect::<BTreeMap<_, _>>();
    let remapped_indices = indices
        .iter()
        .map(|index| remap.get(index).copied().ok_or("missing compacted vertex"))
        .collect::<Result<Vec<_>, _>>()?;
    let vertex_size = used
        .len()
        .checked_mul(mesh.vertex_stride)
        .ok_or("compacted vertex payload size overflow")?;
    let total_size = TRMBF_VERTEX_OFFSET
        .checked_add(vertex_size)
        .ok_or("compacted vertex buffer size overflow")?;
    let mut compacted_buffer = vec![0; total_size];
    compacted_buffer[0x28..0x2c].copy_from_slice(
        &u32::try_from(vertex_size + 28)
            .map_err(|_| "compacted size exceeds TR header")?
            .to_le_bytes(),
    );
    compacted_buffer[0x40..0x44].copy_from_slice(
        &u32::try_from(vertex_size)
            .map_err(|_| "compacted payload exceeds TR header")?
            .to_le_bytes(),
    );
    let mut output_offset = TRMBF_VERTEX_OFFSET;
    for old in &used {
        let source_offset = usize::from(*old) * mesh.vertex_stride;
        let source = source_vertices
            .get(source_offset..source_offset + mesh.vertex_stride)
            .ok_or("source vertex record is out of range")?;
        compacted_buffer[output_offset..output_offset + mesh.vertex_stride].copy_from_slice(source);
        output_offset += mesh.vertex_stride;
    }
    let mut compacted = mesh.clone();
    compacted.vertex_offset = TRMBF_VERTEX_OFFSET;
    compacted.vertex_count = remap.len();
    compacted.vertex_size = vertex_size;
    compacted.indices = remapped_indices;
    Ok((compacted, compacted_buffer))
}

fn shape_submeshes(bytes: &[u8], shape_index: usize) -> Option<Vec<PokemonSubMesh>> {
    let shape = flatbuffer_vector_element(bytes, 1, shape_index)?;
    flatbuffer_table_vector(bytes, shape, 4)?
        .into_iter()
        .map(|submesh| {
            let index_count = flatbuffer_u32_field(bytes, submesh, 0)? as usize;
            let index_offset = flatbuffer_u32_field(bytes, submesh, 1).unwrap_or(0) as usize;
            let applied_material = flatbuffer_string_field(bytes, submesh, 3)?;
            Some(PokemonSubMesh {
                index_count,
                index_offset,
                applied_material,
            })
        })
        .collect()
}

fn flatbuffer_table_vector(bytes: &[u8], table: usize, field_index: usize) -> Option<Vec<usize>> {
    let vtable = table_vtable_offset(bytes, table)?;
    let vsize =
        u16::from_le_bytes(bytes.get(vtable..vtable.checked_add(2)?)?.try_into().ok()?) as usize;
    let slot = vtable.checked_add(4 + field_index.checked_mul(2)?)?;
    if slot + 2 > vtable + vsize {
        return None;
    }
    let offset = u16::from_le_bytes(bytes.get(slot..slot + 2)?.try_into().ok()?) as usize;
    if offset == 0 {
        return None;
    }
    let field = table.checked_add(offset)?;
    let vector = field.checked_add(u32::from_le_bytes(
        bytes.get(field..field.checked_add(4)?)?.try_into().ok()?,
    ) as usize)?;
    let count =
        u32::from_le_bytes(bytes.get(vector..vector.checked_add(4)?)?.try_into().ok()?) as usize;
    if count > 4096 {
        return None;
    }
    (0..count)
        .map(|index| {
            let element = vector.checked_add(4 + index.checked_mul(4)?)?;
            element.checked_add(u32::from_le_bytes(
                bytes
                    .get(element..element.checked_add(4)?)?
                    .try_into()
                    .ok()?,
            ) as usize)
        })
        .collect()
}

fn flatbuffer_u32_field(bytes: &[u8], table: usize, field_index: usize) -> Option<u32> {
    let vtable = table_vtable_offset(bytes, table)?;
    let vsize =
        u16::from_le_bytes(bytes.get(vtable..vtable.checked_add(2)?)?.try_into().ok()?) as usize;
    let slot = vtable.checked_add(4 + field_index.checked_mul(2)?)?;
    if slot + 2 > vtable + vsize {
        return None;
    }
    let offset = u16::from_le_bytes(bytes.get(slot..slot + 2)?.try_into().ok()?) as usize;
    if offset == 0 {
        return None;
    }
    let at = table.checked_add(offset)?;
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layered_color_formulas_keep_distinct_overlap_behavior() {
        let albedo = bntx::TextureData {
            width: 1,
            height: 1,
            rgba: vec![255, 255, 255, 255],
        };
        let mask = bntx::TextureData {
            width: 1,
            height: 1,
            rgba: vec![128, 128, 0, 0],
        };
        let layers = [
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [1.0; 4],
        ];
        let sum = bake_layered_base_color(&albedo, &mask, &layers, LayerBlendFormula::WeightedSum)
            .unwrap();
        let lerp =
            bake_layered_base_color(&albedo, &mask, &layers, LayerBlendFormula::SequentialLerp)
                .unwrap();
        assert_eq!(sum.rgba, [188, 188, 0, 255]);
        assert_eq!(lerp.rgba, [187, 225, 136, 255]);
    }

    #[test]
    fn material_layer_values_are_read_from_every_declared_group() {
        let mut bytes = Vec::new();
        for value in [0.25f32, 0.75] {
            bytes.extend(20u32.to_le_bytes());
            for component in [value, 0.0, 0.0, 1.0] {
                bytes.extend(component.to_le_bytes());
            }
            bytes.extend(15u32.to_le_bytes());
            bytes.extend(b"BaseColorLayer1\0");
        }
        assert_eq!(
            named_vector4_values(&bytes, "BaseColorLayer1"),
            Some(vec![[0.25, 0.0, 0.0, 1.0], [0.75, 0.0, 0.0, 1.0]])
        );
    }
}
