//! Evidence-bounded PLA terrain asset loading and chunked Bevy mesh creation.
//!
//! Terrain entries are resolved by their full relative-path FNV hashes; payload
//! names and signatures are checked as secondary evidence, never entry order.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::prelude::Vec3;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};

use super::bntx::{Bntx, FloatTextureData, TextureData};
use super::gfpak::GfPak;
use super::tr::TrMtr;

const MASK_HEADER_SIZE: usize = 72;
const MASK_SIDE: usize = 4096;
const MASK_PAYLOAD_SIZE: usize = MASK_SIDE * MASK_SIDE;

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialParameterCandidate {
    /// String immediately following a finite little-endian f32 in the file.
    /// This is an observed value/name layout, not proof of the value's role.
    pub name: String,
    pub value: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerrainMaterial {
    pub slots: TrMtr,
    pub parameter_candidates: Vec<MaterialParameterCandidate>,
    /// `SplatST<k>` x/y scale per layer `k` (index = layer). Layout observed in
    /// all inspected `.trmtr`: `[x, y, 0, 0][u32 len][name]`, the vec4 stored
    /// immediately before the name's length. 0.0 if absent.
    pub splat_tiling: Vec<[f32; 2]>,
}

/// Parse `SplatST<k>` (exact name, not `...Far`) vec4s from raw `.trmtr` bytes.
pub fn splat_tiling_from_trmtr(bytes: &[u8]) -> Vec<[f32; 2]> {
    let mut out = vec![[0.0f32; 2]; 8];
    let tag = b"SplatST";
    let mut i = 20;
    while i + 9 < bytes.len() {
        if &bytes[i..i + 7] == tag && bytes[i + 7].is_ascii_digit() && bytes[i + 8] == 0 {
            let k = (bytes[i + 7] - b'0') as usize;
            let f = |o: usize| f32::from_le_bytes(bytes[i - 20 + o..i - 16 + o].try_into().unwrap());
            if k < 8 && f(0).is_finite() && f(4).is_finite() {
                out[k] = [f(0), f(4)];
            }
        }
        i += 1;
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTerrainMask {
    pub source_entry_index: usize,
    pub width: u32,
    pub height: u32,
    /// Raw 0/255 mask payload, excluding its observed 72-byte header.
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerrainInstance {
    pub model_name: String,
    /// X/Z tile-cell suffix encoded in the `.trins` filename.
    pub grid_cell: [u32; 2],
    /// Row-vector transform: rows 0–2 are basis/scale, row 3 is translation.
    pub matrix: [[f32; 4]; 4],
}

#[derive(Debug, Clone)]
pub struct TerrainArea {
    pub area: u8,
    pub source_path: PathBuf,
    /// Descriptor world extent in the terrain's local x/z frame.
    pub world_extent: [f32; 2],
    /// Descriptor height range for normalized height samples (unit inferred).
    pub world_height: f32,
    pub heightmap_brti_name: String,
    pub navmesh_bounds: Option<NavmeshBounds>,
    pub heightmap: FloatTextureData,
    pub normal_map: TextureData,
    pub splat_alpha: TextureData,
    /// Array layer 1 of the same BNTX (the material declares two control
    /// textures); `None` if the texture has a single layer.
    pub splat_alpha1: Option<TextureData>,
    pub material: Option<TerrainMaterial>,
    pub ground_textures: HashMap<String, TextureData>,
    /// Material slots whose corresponding internal BNTX name was not uniquely
    /// identified; texture bindings are not silently fabricated.
    pub unresolved_ground_textures: Vec<String>,
    /// These are candidates described by the census, not proven vegetation
    /// semantics.
    pub masks: Vec<RawTerrainMask>,
    pub instances: Vec<TerrainInstance>,
}

/// Expose the parsed instance records without inventing placement semantics.
pub fn instances(area: &TerrainArea) -> &[TerrainInstance] {
    &area.instances
}

impl TerrainArea {
    /// Resolve explicit overrides against measured descriptor dimensions.
    pub fn mesh_settings(
        &self,
        overrides: TerrainScaleOverride,
        chunk_cells: u32,
    ) -> Result<TerrainMeshSettings, String> {
        let intervals = self.heightmap.width.saturating_sub(1).max(1) as f32;
        let settings = TerrainMeshSettings {
            horizontal_cell_size: overrides
                .horizontal_cell_size
                .unwrap_or(self.world_extent[0] / intervals),
            vertical_scale: overrides.vertical_scale.unwrap_or(self.world_height),
            chunk_cells,
        };
        if !settings.horizontal_cell_size.is_finite()
            || settings.horizontal_cell_size <= 0.0
            || !settings.vertical_scale.is_finite()
            || settings.vertical_scale <= 0.0
            || chunk_cells == 0
        {
            return Err("terrain scale overrides must be finite and positive".into());
        }
        Ok(settings)
    }

    /// Compose albedo using `SplatColor0..3` and RGBA splat channels as a
    /// provisional channel-to-layer mapping. The game binding is a hypothesis.
    pub fn blend_splat_by_slot_order_hypothesis(&self) -> Result<TextureData, String> {
        let material = self
            .material
            .as_ref()
            .ok_or("terrain material was not uniquely identified")?;
        let layers = (0..4)
            .filter_map(|layer_index| {
                let role = format!("SplatColor{layer_index}");
                material
                    .slots
                    .textures
                    .iter()
                    .find(|(_, slot_role)| slot_role == &role)
            })
            .map(|(name, _)| {
                self.ground_textures
                    .get(name)
                    .ok_or_else(|| format!("ground texture slot {name} is unresolved"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if layers.is_empty() {
            return Err("terrain material has no texture slots".into());
        }
        let output_size = (self.splat_alpha.width as usize)
            .checked_mul(self.splat_alpha.height as usize)
            .ok_or("splat dimensions overflow")?;
        if self.splat_alpha.rgba.len() != output_size * 4 {
            return Err("splat RGBA storage does not match its dimensions".into());
        }
        for (index, layer) in layers.iter().enumerate() {
            if layer.width == 0
                || layer.height == 0
                || layer.rgba.len() != layer.width as usize * layer.height as usize * 4
            {
                return Err(format!("material layer {index} has invalid RGBA storage"));
            }
        }
        let mut rgba = vec![0u8; output_size * 4];
        for y in 0..self.splat_alpha.height {
            for x in 0..self.splat_alpha.width {
                let pixel = (y * self.splat_alpha.width + x) as usize;
                let mut weights = [0u32; 4];
                let mut total = 0u32;
                for (layer_index, weight) in weights.iter_mut().enumerate().take(layers.len()) {
                    *weight = self.splat_alpha.rgba[pixel * 4 + layer_index] as u32;
                    total += *weight;
                }
                if total == 0 {
                    weights[0] = 255;
                    total = 255;
                }
                for channel in 0..3 {
                    let mut mixed = 0u32;
                    for (layer_index, layer) in layers.iter().enumerate() {
                        let sx =
                            (x as u64 * layer.width as u64 / self.splat_alpha.width as u64) as u32;
                        let sy = (y as u64 * layer.height as u64 / self.splat_alpha.height as u64)
                            as u32;
                        let sample = ((sy.min(layer.height - 1) * layer.width
                            + sx.min(layer.width - 1))
                            as usize)
                            * 4
                            + channel;
                        mixed += layer.rgba[sample] as u32 * weights[layer_index];
                    }
                    rgba[pixel * 4 + channel] = (mixed / total) as u8;
                }
                rgba[pixel * 4 + 3] = 255;
            }
        }
        Ok(TextureData {
            width: self.splat_alpha.width,
            height: self.splat_alpha.height,
            rgba,
        })
    }

    /// Sample the terrain surface elevation in world units (metres inferred) at (x, z).
    pub fn sample_world_height(&self, x: f32, z: f32) -> Option<f32> {
        let norm = sample_height(&self.heightmap, self.world_extent, x, z)?;
        Some(norm * self.world_height)
    }
}

impl TerrainArea {
    /// Load `archive/field/terrain/ha_areaNN.gfpak` from the extracted game `bin/` root.
    pub fn load(bin_root: impl AsRef<Path>, area: u8) -> Result<Self, String> {
        if area > 6 {
            return Err(format!("area must be in 0..=6, got {area}"));
        }
        let source_path = bin_root
            .as_ref()
            .join("archive/field/terrain")
            .join(format!("ha_area{area:02}.gfpak"));
        Self::load_pack(&source_path, area)
    }

    /// Load from one archive path. Kept public for fixture-based tests/tools.
    pub fn load_pack(path: impl AsRef<Path>, area: u8) -> Result<Self, String> {
        if area > 6 {
            return Err(format!("area must be in 0..=6, got {area}"));
        }
        let source_path = path.as_ref().to_path_buf();
        let bytes = std::fs::read(&source_path)
            .map_err(|e| format!("read terrain pack {}: {e}", source_path.display()))?;
        let pack = GfPak::parse(&bytes)?;
        // The last area's verified pack paths and BRTI names use `terrain_1_1`;
        // the others use `areaNN_0_0`.
        let tile_prefix = if area == 6 {
            "terrain_1_1".to_owned()
        } else {
            format!("area{area:02}_0_0")
        };
        let pack_prefix = format!("bin/field/terrain/ha_area{area:02}/terrain/");
        let height_path = format!("{pack_prefix}{tile_prefix}_heightmap.bntx");
        let normal_path = format!("{pack_prefix}{tile_prefix}_normalmap.bntx");
        let splat_path = format!("{pack_prefix}{tile_prefix}_splat_alpha.bntx");
        let height_entry = required_entry_index(&pack, &height_path)?;
        let normal_entry = required_entry_index(&pack, &normal_path)?;
        let splat_entry = required_entry_index(&pack, &splat_path)?;

        let mut masks = Vec::new();
        let mut terrain_descriptor_candidates = Vec::new();
        let mut sample_tail_scales = Vec::new();

        for (entry_index, entry) in pack.entries.iter().enumerate() {
            let decoded = pack
                .decoded_data(&bytes, entry)
                .map_err(|e| format!("decode terrain entry {entry_index}: {e}"))?;

            // Identify the descriptor by its referenced asset names, never by
            // pack order. Its inline extent struct is at the census-verified
            // offset 104; corroborate its H against a struct in the large
            // per-sample payload's tail after height dimensions are known.
            if descriptor_names_area(&decoded, area) {
                if let Some(scale) = scale_struct_at(&decoded, 104) {
                    terrain_descriptor_candidates.push((entry_index, scale, decoded.clone()));
                }
            }
            if let Some(scale) = scale_struct_near_tail(&decoded) {
                sample_tail_scales.push((entry_index, decoded.len(), scale));
            }
        }

        if terrain_descriptor_candidates.len() != 1 {
            return Err(format!(
                "expected one named terrain descriptor with a scale struct, found {}",
                terrain_descriptor_candidates.len()
            ));
        }
        let (descriptor_entry, (extent_x, world_height, extent_z), descriptor_data) =
            terrain_descriptor_candidates.remove(0);
        if !extent_x.is_finite()
            || !extent_z.is_finite()
            || !world_height.is_finite()
            || extent_x <= 0.0
            || extent_z <= 0.0
            || world_height <= 0.0
        {
            return Err("terrain descriptor contains a non-positive or non-finite scale".into());
        }

        let height_bytes = pack.decoded_data(&bytes, &pack.entries[height_entry])?;
        let height_bntx = Bntx::parse(&height_bytes)?;
        let height_matches: Vec<_> = height_bntx
            .textures
            .iter()
            .enumerate()
            .filter(|(_, texture)| texture.name.ends_with("_heightmap"))
            .map(|(index, texture)| (index, texture.name.clone()))
            .collect();
        if height_matches.len() != 1 {
            return Err(format!(
                "heightmap path {height_path} contains {} BRTI names ending in _heightmap",
                height_matches.len()
            ));
        }
        let (height_texture_index, heightmap_brti_name) = height_matches[0].clone();
        let heightmap = height_bntx.decode_f32(&height_bytes, height_texture_index)?;
        // R16F is the smallest verified height encoding, so a matching tail
        // only counts as the per-sample corroboration if the payload could
        // contain at least one halfword per sample.
        let sample_payload_min_size = heightmap.samples.len().saturating_mul(2);
        if !sample_tail_scales
            .iter()
            .any(|(index, decoded_len, (x, h, z))| {
                *index != descriptor_entry
                    && *decoded_len >= sample_payload_min_size
                    && (*x - extent_x).abs() < 0.01
                    && (*h - world_height).abs() < 0.01
                    && (*z - extent_z).abs() < 0.01
            })
        {
            return Err(format!(
                "descriptor scale ({extent_x}, {world_height}, {extent_z}) has no matching per-sample tail struct in an entry at least {sample_payload_min_size} bytes long"
            ));
        }

        let normal_bytes = pack.decoded_data(&bytes, &pack.entries[normal_entry])?;
        let normal_bntx = Bntx::parse(&normal_bytes)?;
        let normal_index = find_texture_suffix(&normal_bntx, "_normal")?;
        let normal_map = normal_bntx.decode(&normal_bytes, normal_index)?;

        let splat_bytes = pack.decoded_data(&bytes, &pack.entries[splat_entry])?;
        let splat_bntx = Bntx::parse(&splat_bytes)?;
        let splat_index = find_texture_suffix(&splat_bntx, "_splat_alpha0")?;
        let splat_alpha = splat_bntx.decode(&splat_bytes, splat_index)?;
        let splat_alpha1 = if splat_bntx.textures[splat_index].array_count > 1 {
            Some(splat_bntx.textures[splat_index].decode_layer(&splat_bytes, 1)?)
        } else {
            None
        };

        // FlatBuffer-like descriptor is found by its asset references and
        // scale, while every referenced payload is resolved by full path hash.
        let referenced_instances = extract_asset_names(&descriptor_data, ".trins");
        let referenced_masks = extract_asset_names(&descriptor_data, ".trtbf");
        let mut instances = Vec::new();
        for file_name in referenced_instances {
            let path = format!("{pack_prefix}{file_name}");
            let index = required_entry_index(&pack, &path)?;
            let decoded = pack.decoded_data(&bytes, &pack.entries[index])?;
            let (model_name, grid_cell) = parse_instance_filename(&file_name)?;
            instances.extend(find_instance_matrices(&decoded, &model_name, grid_cell));
        }
        for file_name in referenced_masks {
            let path = format!("{pack_prefix}{file_name}");
            let index = required_entry_index(&pack, &path)?;
            let decoded = pack.decoded_data(&bytes, &pack.entries[index])?;
            if decoded.len() == MASK_HEADER_SIZE + MASK_PAYLOAD_SIZE {
                masks.push(RawTerrainMask {
                    source_entry_index: index,
                    width: MASK_SIDE as u32,
                    height: MASK_SIDE as u32,
                    bytes: decoded[MASK_HEADER_SIZE..].to_vec(),
                });
            }
        }
        let navmesh_bounds = load_area_navmesh_bounds(
            source_path
                .parent()
                .ok_or("terrain archive has no parent directory")?,
            &source_path,
            area,
        )?;

        let tile_material = extract_asset_names(&descriptor_data, ".trmtr")
            .into_iter()
            .find(|name| name == &format!("{tile_prefix}.trmtr"))
            .ok_or_else(|| format!("descriptor does not name {tile_prefix}.trmtr"))?;
        let material_path = format!("{pack_prefix}{tile_material}");
        let material_index = required_entry_index(&pack, &material_path)?;
        let material_bytes = pack.decoded_data(&bytes, &pack.entries[material_index])?;
        let material = Some(TerrainMaterial {
            slots: TrMtr::parse(&material_bytes)
                .map_err(|error| format!("parse terrain material {material_path}: {error}"))?,
            parameter_candidates: material_parameter_candidates(&material_bytes),
            splat_tiling: splat_tiling_from_trmtr(&material_bytes),
        });
        let mut ground_textures = HashMap::new();
        let mut unresolved_ground_textures = Vec::new();
        if let Some(material) = &material {
            for (file_name, role) in &material.slots.textures {
                if !role.starts_with("SplatColor") {
                    continue;
                }
                let stem = file_name
                    .strip_suffix(".bntx")
                    .or_else(|| file_name.strip_suffix(".BNTX"))
                    .unwrap_or(file_name);
                let texture_path = format!("{pack_prefix}{stem}.bntx");
                let Some(entry_index) = pack.entry_index_by_path(&texture_path)? else {
                    unresolved_ground_textures.push(file_name.clone());
                    continue;
                };
                let entry = &pack.entries[entry_index];
                let decoded = pack.decoded_data(&bytes, entry)?;
                let bntx = Bntx::parse(&decoded)?;
                let Some(texture_index) = bntx.textures.iter().position(|t| t.name == stem) else {
                    unresolved_ground_textures.push(file_name.clone());
                    continue;
                };
                ground_textures.insert(file_name.clone(), bntx.decode(&decoded, texture_index)?);
            }
        }

        Ok(Self {
            area,
            source_path,
            world_extent: [extent_x, extent_z],
            world_height,
            heightmap_brti_name,
            navmesh_bounds,
            heightmap,
            normal_map,
            splat_alpha,
            splat_alpha1,
            material,
            ground_textures,
            unresolved_ground_textures,
            masks,
            instances,
        })
    }
}

fn descriptor_names_area(bytes: &[u8], area: u8) -> bool {
    let tile = if area == 6 {
        "terrain_1_1".to_owned()
    } else {
        format!("area{area:02}_0_0")
    };
    let text = String::from_utf8_lossy(bytes);
    text.contains(&format!("{tile}.trmtr")) && text.contains(".trtbf") && text.contains(".trins")
}

fn required_entry_index(pack: &GfPak, path: &str) -> Result<usize, String> {
    pack.entry_index_by_path(path)?
        .ok_or_else(|| format!("GFLXPACK has no entry for full path {path}"))
}

fn find_texture_suffix(bntx: &Bntx, suffix: &str) -> Result<usize, String> {
    let matches: Vec<_> = bntx
        .textures
        .iter()
        .enumerate()
        .filter(|(_, texture)| texture.name.ends_with(suffix))
        .map(|(index, _)| index)
        .collect();
    match matches.as_slice() {
        [index] => Ok(*index),
        _ => Err(format!(
            "expected one BRTI texture ending in {suffix}, found {}",
            matches.len()
        )),
    }
}

fn extract_asset_names(bytes: &[u8], extension: &str) -> Vec<String> {
    let mut names = std::collections::BTreeSet::new();
    let mut cursor = 0;
    while cursor + extension.len() <= bytes.len() {
        let Some(relative) = bytes[cursor..]
            .windows(extension.len())
            .position(|window| window == extension.as_bytes())
        else {
            break;
        };
        let end = cursor + relative + extension.len();
        let mut start = cursor + relative;
        while start > 0
            && (bytes[start - 1].is_ascii_alphanumeric() || b"_-./".contains(&bytes[start - 1]))
        {
            start -= 1;
        }
        if let Ok(name) = std::str::from_utf8(&bytes[start..end]) {
            if name.ends_with(extension)
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-./".contains(&byte))
            {
                names.insert(name.to_owned());
            }
        }
        cursor = end;
    }
    names.into_iter().collect()
}

fn parse_instance_filename(name: &str) -> Result<(String, [u32; 2]), String> {
    let stem = name
        .strip_suffix(".trins")
        .ok_or_else(|| format!("not a .trins filename: {name}"))?;
    let x_marker = stem
        .rfind("_X")
        .ok_or_else(|| format!("missing X/Z grid suffix in {name}"))?;
    let z_marker = stem[x_marker + 2..]
        .find('Z')
        .map(|n| n + x_marker + 2)
        .ok_or_else(|| format!("missing Z grid suffix in {name}"))?;
    let bucket_marker = stem[z_marker + 1..]
        .find("_d")
        .map(|n| n + z_marker + 1)
        .ok_or_else(|| format!("missing distance suffix in {name}"))?;
    let x = stem[x_marker + 2..z_marker]
        .parse()
        .map_err(|_| format!("invalid X grid in {name}"))?;
    let z = stem[z_marker + 1..bucket_marker]
        .parse()
        .map_err(|_| format!("invalid Z grid in {name}"))?;
    let model_start = stem
        .find("_plants_")
        .map(|position| position + 1)
        .unwrap_or_else(|| stem.find("_X").unwrap_or(0));
    Ok((stem[model_start..x_marker].to_owned(), [x, z]))
}

fn find_instance_matrices(
    bytes: &[u8],
    model_name: &str,
    grid_cell: [u32; 2],
) -> Vec<TerrainInstance> {
    if bytes.len() < 64 {
        return Vec::new();
    }
    let mut found = Vec::new();
    for offset in (0..=bytes.len() - 64).step_by(4) {
        let values: [f32; 16] = std::array::from_fn(|index| {
            f32::from_le_bytes(
                bytes[offset + index * 4..offset + index * 4 + 4]
                    .try_into()
                    .unwrap(),
            )
        });
        if values.iter().any(|value| !value.is_finite())
            || values[3] != 0.0
            || values[7] != 0.0
            || values[11] != 0.0
            || values[15] != 1.0
            || values[13] <= 0.0
            || !(0.0..=2100.0).contains(&values[12])
            || !(0.0..=2100.0).contains(&values[14])
            || values[5].abs() < 0.1
        {
            continue;
        }
        found.push(TerrainInstance {
            model_name: model_name.to_owned(),
            grid_cell,
            matrix: std::array::from_fn(|row| {
                std::array::from_fn(|column| values[row * 4 + column])
            }),
        });
    }
    found
}

/// Sample normalized height at world coordinates (x, z) using bilinear interpolation.
pub fn sample_height(heightmap: &FloatTextureData, extent: [f32; 2], x: f32, z: f32) -> Option<f32> {
    if heightmap.width < 2
        || heightmap.height < 2
        || heightmap.samples.len() != heightmap.width as usize * heightmap.height as usize
        || extent[0] <= 0.0
        || extent[1] <= 0.0
    {
        return None;
    }
    let u =
        (x * (heightmap.width - 1) as f32 / extent[0]).clamp(0.0, heightmap.width as f32 - 1.001);
    let v =
        (z * (heightmap.height - 1) as f32 / extent[1]).clamp(0.0, heightmap.height as f32 - 1.001);
    let x0 = u.floor() as usize;
    let z0 = v.floor() as usize;
    let (fx, fz) = (u - x0 as f32, v - z0 as f32);
    let width = heightmap.width as usize;
    let at = |x: usize, z: usize| heightmap.samples[z * width + x];
    let h00 = at(x0, z0);
    let h10 = at(x0 + 1, z0);
    let h01 = at(x0, z0 + 1);
    let h11 = at(x0 + 1, z0 + 1);
    Some(
        h00 * (1.0 - fx) * (1.0 - fz)
            + h10 * fx * (1.0 - fz)
            + h01 * (1.0 - fx) * fz
            + h11 * fx * fz,
    )
}

fn scale_struct_at(bytes: &[u8], offset: usize) -> Option<(f32, f32, f32)> {
    let data = bytes.get(offset..offset.checked_add(12)?)?;
    let x = f32::from_le_bytes(data[0..4].try_into().ok()?);
    let h = f32::from_le_bytes(data[4..8].try_into().ok()?);
    let z = f32::from_le_bytes(data[8..12].try_into().ok()?);
    (x.is_finite()
        && h.is_finite()
        && z.is_finite()
        && (x - 1025.0).abs() < 0.01
        && (z - 1025.0).abs() < 0.01
        && h > 0.0)
        .then_some((x, h, z))
}

fn scale_struct_near_tail(bytes: &[u8]) -> Option<(f32, f32, f32)> {
    let start = (bytes.len().saturating_sub(64) + 3) & !3;
    (start..bytes.len().saturating_sub(11))
        .step_by(4)
        .filter_map(|offset| scale_struct_at(bytes, offset))
        .last()
}

#[derive(Debug, Clone, Copy)]
pub struct NavmeshBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub max_y: f32,
    pub run_len: usize,
}

/// Heuristic, not a Havok parser: inside a TAG0 entry, find the longest
/// 16-byte-aligned run of little-endian (x,y,z,w) vectors whose coordinates
/// are finite and in [0,5000], and whose w is 0 or 1. This can miss valid
/// layouts or match unrelated vectors; callers report absence and only use a
/// detected run as a bounded cross-check.
fn scan_havok_navmesh_bounds(bytes: &[u8]) -> Option<NavmeshBounds> {
    if bytes.get(4..8) != Some(b"TAG0") {
        return None;
    }
    let mut best = None;
    // Vector arrays in the tagfile are 16-byte aligned. Test each 4-byte
    // alignment once; scanning every potential start would be quadratic on a
    // large contiguous vertex array.
    for alignment in (0..16).step_by(4) {
        let mut cursor = alignment;
        let mut run: Option<NavmeshBounds> = None;
        while let Some(v) = bytes.get(cursor..cursor + 16) {
            let f =
                |range: std::ops::Range<usize>| f32::from_le_bytes(v[range].try_into().unwrap());
            let (x, y, z, w) = (f(0..4), f(4..8), f(8..12), f(12..16));
            if !(x.is_finite()
                && y.is_finite()
                && z.is_finite()
                && w.is_finite()
                && (0.0..=5000.0).contains(&x)
                && y.abs() <= 5000.0
                && (0.0..=5000.0).contains(&z)
                && (w.abs() < 0.001 || (w - 1.0).abs() < 0.001))
            {
                if let Some(bounds) = run.take() {
                    if best
                        .as_ref()
                        .map_or(true, |prior: &NavmeshBounds| bounds.run_len > prior.run_len)
                    {
                        best = Some(bounds);
                    }
                }
                cursor += 16;
                continue;
            }
            match &mut run {
                Some(b) => {
                    b.min_x = b.min_x.min(x);
                    b.max_x = b.max_x.max(x);
                    b.min_z = b.min_z.min(z);
                    b.max_z = b.max_z.max(z);
                    b.max_y = b.max_y.max(y);
                    b.run_len += 1;
                }
                None => {
                    run = Some(NavmeshBounds {
                        min_x: x,
                        max_x: x,
                        min_z: z,
                        max_z: z,
                        max_y: y,
                        run_len: 1,
                    })
                }
            }
            cursor += 16;
        }
        if let Some(bounds) = run {
            if best
                .as_ref()
                .map_or(true, |b: &NavmeshBounds| bounds.run_len > b.run_len)
            {
                best = Some(bounds);
            }
        }
    }
    best.filter(|b| b.run_len >= 8)
}

fn load_area_navmesh_bounds(
    directory: &Path,
    main_pack: &Path,
    area: u8,
) -> Result<Option<NavmeshBounds>, String> {
    let prefix = format!("ha_area{area:02}_");
    let mut paths = std::fs::read_dir(directory)
        .map_err(|error| {
            format!(
                "read terrain archive directory {}: {error}",
                directory.display()
            )
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".gfpak"))
        })
        .collect::<Vec<_>>();
    paths.push(main_pack.to_path_buf());
    paths.sort();
    let mut best: Option<NavmeshBounds> = None;
    for path in paths {
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("read navmesh pack {}: {error}", path.display()))?;
        let pack = GfPak::parse(&bytes)
            .map_err(|error| format!("parse navmesh pack {}: {error}", path.display()))?;
        for (index, entry) in pack.entries.iter().enumerate() {
            let decoded = pack.decoded_data(&bytes, entry).map_err(|error| {
                format!(
                    "decode navmesh pack {} entry {index}: {error}",
                    path.display()
                )
            })?;
            if decoded.get(4..8) != Some(b"TAG0")
                || !decoded
                    .windows(b"hkaiNavMesh".len())
                    .any(|window| window == b"hkaiNavMesh")
            {
                continue;
            }
            if let Some(bounds) = scan_havok_navmesh_bounds(&decoded) {
                let span_x = bounds.max_x - bounds.min_x;
                let span_z = bounds.max_z - bounds.min_z;
                // A short navmesh sector may contain a valid vector run but
                // cannot verify the whole terrain extent. Only retain a run
                // whose x/z bounds independently cover the descriptor extent.
                if (span_x - 1025.0).abs() > 10.25 || (span_z - 1025.0).abs() > 10.25 {
                    continue;
                }
                if best
                    .as_ref()
                    .map_or(true, |prior| bounds.run_len > prior.run_len)
                {
                    best = Some(bounds);
                }
            }
        }
    }
    Ok(best)
}

fn material_parameter_candidates(bytes: &[u8]) -> Vec<MaterialParameterCandidate> {
    let mut candidates = Vec::new();
    let mut start = None;
    for (index, byte) in bytes.iter().copied().enumerate() {
        match (byte, start) {
            (0, Some(begin)) => {
                let name = &bytes[begin..index];
                let identifier =
                    name.len() >= 2 && name.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_');
                if identifier && begin >= 4 {
                    let value = f32::from_le_bytes(bytes[begin - 4..begin].try_into().unwrap());
                    if value.is_finite() {
                        candidates.push(MaterialParameterCandidate {
                            name: String::from_utf8_lossy(name).into_owned(),
                            value,
                        });
                    }
                }
                start = None;
            }
            (0, None) => {}
            (b, None) if b.is_ascii_alphanumeric() || b == b'_' => start = Some(index),
            _ => start = None,
        }
    }
    candidates
}

/// Optional explicit scale overrides; omitted values come from the terrain descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TerrainScaleOverride {
    pub horizontal_cell_size: Option<f32>,
    pub vertical_scale: Option<f32>,
}

/// Resolved settings for a terrain mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainMeshSettings {
    /// Resolved horizontal spacing between height samples.
    pub horizontal_cell_size: f32,
    /// Provisional default: 100 debug units for normalized height 0..1.
    pub vertical_scale: f32,
    /// Maximum cells per side per mesh (64 keeps 2049² areas split up).
    pub chunk_cells: u32,
}

#[derive(Debug)]
pub struct TerrainMeshChunk {
    pub sample_origin: [u32; 2],
    pub mesh: Mesh,
}

/// Generate independently renderable mesh patches, sharing identical world
/// coordinates and height-derived normals at patch boundaries.
pub fn build_mesh_chunks(
    heightmap: &FloatTextureData,
    settings: TerrainMeshSettings,
) -> Result<Vec<TerrainMeshChunk>, String> {
    let width = heightmap.width as usize;
    let height = heightmap.height as usize;
    if width < 2 || height < 2 || heightmap.samples.len() != width * height {
        return Err("heightmap dimensions must match at least a 2x2 sample grid".into());
    }
    if !settings.horizontal_cell_size.is_finite()
        || settings.horizontal_cell_size <= 0.0
        || !settings.vertical_scale.is_finite()
        || settings.vertical_scale <= 0.0
        || settings.chunk_cells == 0
    {
        return Err("mesh scales and chunk_cells must be finite and positive".into());
    }
    if let Some(index) = heightmap
        .samples
        .iter()
        .position(|sample| !sample.is_finite())
    {
        return Err(format!("heightmap sample {index} is not finite"));
    }

    let chunk_cells = settings.chunk_cells as usize;
    let mut chunks = Vec::new();
    for z0 in (0..height - 1).step_by(chunk_cells) {
        let z1 = (z0 + chunk_cells).min(height - 1);
        for x0 in (0..width - 1).step_by(chunk_cells) {
            let x1 = (x0 + chunk_cells).min(width - 1);
            let patch_width = x1 - x0 + 1;
            let patch_height = z1 - z0 + 1;
            let mut positions = Vec::with_capacity(patch_width * patch_height);
            let mut normals = Vec::with_capacity(patch_width * patch_height);
            let mut uvs = Vec::with_capacity(patch_width * patch_height);
            for z in z0..=z1 {
                for x in x0..=x1 {
                    let sample = heightmap.samples[z * width + x];
                    // The descriptor origin is the local terrain corner, not its center.
                    let world_x = x as f32 * settings.horizontal_cell_size;
                    let world_z = z as f32 * settings.horizontal_cell_size;
                    positions.push([world_x, sample * settings.vertical_scale, world_z]);
                    let left = heightmap.samples[z * width + x.saturating_sub(1)];
                    let right = heightmap.samples[z * width + (x + 1).min(width - 1)];
                    let up = heightmap.samples[z.saturating_sub(1) * width + x];
                    let down = heightmap.samples[(z + 1).min(height - 1) * width + x];
                    let dx = (right - left) * settings.vertical_scale
                        / (settings.horizontal_cell_size
                            * if x == 0 || x + 1 == width { 1.0 } else { 2.0 });
                    let dz = (down - up) * settings.vertical_scale
                        / (settings.horizontal_cell_size
                            * if z == 0 || z + 1 == height { 1.0 } else { 2.0 });
                    normals.push(Vec3::new(-dx, 1.0, -dz).normalize().to_array());
                    uvs.push([
                        x as f32 / (width - 1) as f32,
                        z as f32 / (height - 1) as f32,
                    ]);
                }
            }
            let mut indices = Vec::with_capacity((patch_width - 1) * (patch_height - 1) * 6);
            for z in 0..patch_height - 1 {
                for x in 0..patch_width - 1 {
                    let top_left = (z * patch_width + x) as u32;
                    let top_right = top_left + 1;
                    let bottom_left = top_left + patch_width as u32;
                    let bottom_right = bottom_left + 1;
                    indices.extend_from_slice(&[
                        top_left,
                        bottom_left,
                        top_right,
                        top_right,
                        bottom_left,
                        bottom_right,
                    ]);
                }
            }
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
            mesh.insert_indices(Indices::U32(indices));
            chunks.push(TerrainMeshChunk {
                sample_origin: [x0 as u32, z0 as u32],
                mesh,
            });
        }
    }
    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_meshes_cover_cells_without_exceeding_chunk_size() {
        let heightmap = FloatTextureData {
            width: 6,
            height: 5,
            samples: (0..30).map(|n| n as f32 / 30.0).collect(),
        };
        let chunks = build_mesh_chunks(
            &heightmap,
            TerrainMeshSettings {
                horizontal_cell_size: 1.0,
                vertical_scale: 100.0,
                chunk_cells: 2,
            },
        )
        .unwrap();
        assert_eq!(chunks.len(), 6);
        assert_eq!(chunks[0].sample_origin, [0, 0]);
        assert_eq!(chunks[1].sample_origin, [2, 0]);
        assert_eq!(chunks[3].sample_origin, [0, 2]);
    }

    #[test]
    fn mesh_rejects_invalid_samples_and_scale() {
        let mut heightmap = FloatTextureData {
            width: 2,
            height: 2,
            samples: vec![0.0; 4],
        };
        let invalid = TerrainMeshSettings {
            horizontal_cell_size: 0.0,
            vertical_scale: 100.0,
            chunk_cells: 64,
        };
        assert!(build_mesh_chunks(&heightmap, invalid).is_err());
        heightmap.samples[2] = f32::NAN;
        assert!(
            build_mesh_chunks(
                &heightmap,
                TerrainMeshSettings {
                    horizontal_cell_size: 1.0,
                    vertical_scale: 100.0,
                    chunk_cells: 64
                }
            )
            .is_err()
        );
    }

    #[test]
    fn trins_matrix_scan_preserves_row_vector_translation_and_grid_cell() {
        let mut values = [0.0f32; 16];
        values[0] = 1.0;
        values[5] = 1.0;
        values[10] = 1.0;
        values[12] = 10.0;
        values[13] = 20.0;
        values[14] = 30.0;
        values[15] = 1.0;
        let bytes = values
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        let (model, cell) =
            parse_instance_filename("area00_0_0_plants_tree01_X8Z9_d0.trins").unwrap();
        assert_eq!(model, "plants_tree01");
        assert_eq!(cell, [8, 9]);
        let records = find_instance_matrices(&bytes, &model, cell);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].matrix[3], [10.0, 20.0, 30.0, 1.0]);
    }

    #[test]
    fn splat_compositor_uses_material_slot_order_as_an_explicit_hypothesis() {
        let mut ground_textures = HashMap::new();
        ground_textures.insert(
            "first.bntx".into(),
            TextureData {
                width: 1,
                height: 1,
                rgba: vec![255, 0, 0, 255],
            },
        );
        ground_textures.insert(
            "second.bntx".into(),
            TextureData {
                width: 1,
                height: 1,
                rgba: vec![0, 0, 255, 255],
            },
        );
        let area = TerrainArea {
            area: 0,
            source_path: PathBuf::new(),
            world_extent: [1.0, 1.0],
            world_height: 1.0,
            navmesh_bounds: None,
            heightmap_brti_name: "test_heightmap".into(),
            heightmap: FloatTextureData {
                width: 1,
                height: 1,
                samples: vec![0.0],
            },
            normal_map: TextureData {
                width: 1,
                height: 1,
                rgba: vec![0; 4],
            },
            splat_alpha: TextureData {
                width: 1,
                height: 1,
                rgba: vec![128, 127, 0, 0],
            },
            splat_alpha1: None,
            material: Some(TerrainMaterial {
                slots: TrMtr {
                    textures: vec![
                        ("first.bntx".into(), "SplatColor0".into()),
                        ("second.bntx".into(), "SplatColor1".into()),
                    ],
                    flags: Vec::new(),
                },
                parameter_candidates: Vec::new(),
                splat_tiling: Vec::new(),
            }),
            ground_textures,
            unresolved_ground_textures: Vec::new(),
            masks: Vec::new(),
            instances: Vec::new(),
        };
        let blended = area.blend_splat_by_slot_order_hypothesis().unwrap();
        assert_eq!(blended.rgba, [128, 0, 127, 255]);
    }

    #[test]
    fn game_area_loader_skips_when_game_pack_is_absent() {
        const EXPECTED_HEIGHTS: [f32; 7] = [185.0, 121.756, 200.240, 127.079, 302.0, 168.0, 36.67];
        const EXPECTED_INSTANCE_COUNTS: [usize; 6] = [60, 1430, 2204, 788, 901, 828];
        let bin_root = std::env::var_os("PLA_BIN_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
            });
        if !(0..=6).all(|area| {
            bin_root
                .join("archive/field/terrain")
                .join(format!("ha_area{area:02}.gfpak"))
                .is_file()
        }) {
            eprintln!(
                "[skip] one or more terrain packs unavailable under {}/archive/field/terrain",
                bin_root.display()
            );
            return;
        }
        for area_number in 0..=6 {
            let area = TerrainArea::load(&bin_root, area_number)
                .unwrap_or_else(|error| panic!("load area {area_number:02}: {error}"));
            assert!(area.heightmap.width >= 1025);
            assert_eq!(area.heightmap.width, area.heightmap.height);
            assert_eq!(
                area.heightmap.samples.len(),
                (area.heightmap.width * area.heightmap.height) as usize
            );
            assert!((area.world_extent[0] - 1025.0).abs() < 0.01);
            assert!((area.world_extent[1] - 1025.0).abs() < 0.01);
            assert!((area.world_height - EXPECTED_HEIGHTS[area_number as usize]).abs() < 0.01);
            if area_number == 6 {
                let pack_path = bin_root
                    .join("archive/field/terrain")
                    .join("ha_area06.gfpak");
                let bytes = std::fs::read(&pack_path).expect("read area06 GFLXPACK");
                let pack = GfPak::parse(&bytes).expect("parse area06 GFLXPACK");
                let mut bntx_inventory = Vec::new();
                for (entry_index, entry) in pack.entries.iter().enumerate() {
                    let payload = pack.decoded_data(&bytes, entry).unwrap_or_else(|error| {
                        panic!("decode area06 entry {entry_index}: {error}")
                    });
                    if payload.starts_with(b"BNTX") {
                        let bntx = Bntx::parse(&payload).unwrap_or_else(|error| {
                            panic!("parse area06 BNTX entry {entry_index}: {error}")
                        });
                        let names: Vec<_> = bntx
                            .textures
                            .iter()
                            .map(|texture| texture.name.clone())
                            .collect();
                        eprintln!("area06 BNTX entry {entry_index} BRTI names: {names:?}");
                        bntx_inventory.push((entry_index, names));
                    }
                }
                assert!(!bntx_inventory.is_empty(), "area06 BNTX inventory is empty");
                let heightmap_names: Vec<_> = bntx_inventory
                    .iter()
                    .flat_map(|(_, names)| names.iter())
                    .filter(|name| name.ends_with("_heightmap"))
                    .cloned()
                    .collect();
                assert_eq!(
                    heightmap_names.len(),
                    1,
                    "expected one heightmap BRTI name across all area06 BNTX entries, found {heightmap_names:?}"
                );
                let identified_heightmap = &heightmap_names[0];
                eprintln!(
                    "area06 unique heightmap identified from full BNTX inventory: {identified_heightmap}"
                );
                assert_eq!(
                    &area.heightmap_brti_name, identified_heightmap,
                    "loader-selected area06 heightmap differs from BNTX inventory evidence"
                );
            }
            if area_number <= 5 {
                assert!(
                    !area.instances.is_empty(),
                    "area {area_number:02} has no parsed .trins matrices"
                );
                for (instance_index, instance) in area.instances.iter().enumerate() {
                    let x = instance.matrix[3][0];
                    let y = instance.matrix[3][1];
                    let z = instance.matrix[3][2];
                    assert!(
                        x >= 0.0
                            && x <= area.world_extent[0] + 1.0
                            && z >= 0.0
                            && z <= area.world_extent[1] + 1.0,
                        "area {area_number:02} instance {instance_index} outside terrain: ({x}, {y}, {z})"
                    );
                    let sampled_y = sample_height(&area.heightmap, area.world_extent, x, z)
                        .expect("valid square terrain heightmap");
                    let error = (sampled_y * area.world_height - y).abs();
                    assert!(
                        error <= 2.0,
                        "area {area_number:02} instance {instance_index} ({}, cell {:?}) differs from sampled terrain by {error:.3} m",
                        instance.model_name,
                        instance.grid_cell
                    );
                }
                eprintln!(
                    "area{area_number:02}: {} instance matrices checked against normalized heightmap x/y/z; all within 2 descriptor units (metres inferred)",
                    area.instances.len()
                );
                assert_eq!(
                    area.instances.len(),
                    EXPECTED_INSTANCE_COUNTS[area_number as usize],
                    "area {area_number:02} instance count differs from the verified census"
                );
            }
            let height_max = area
                .heightmap
                .samples
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            if let Some(bounds) = area.navmesh_bounds {
                let span_x = bounds.max_x - bounds.min_x;
                let span_z = bounds.max_z - bounds.min_z;
                assert!(
                    (span_x - area.world_extent[0]).abs() <= area.world_extent[0] * 0.01,
                    "area {area_number:02} TAG0 heuristic x span {span_x} differs from {}",
                    area.world_extent[0]
                );
                assert!(
                    (span_z - area.world_extent[1]).abs() <= area.world_extent[1] * 0.01,
                    "area {area_number:02} TAG0 heuristic z span {span_z} differs from {}",
                    area.world_extent[1]
                );
                let scaled_height_max = height_max * area.world_height;
                assert!(
                    bounds.max_y <= scaled_height_max * 1.01,
                    "area {area_number:02} navmesh max y {} exceeds scaled heightmap max {scaled_height_max}",
                    bounds.max_y
                );
                assert!(
                    bounds.max_y >= 0.95 * scaled_height_max,
                    "area {area_number:02} navmesh max y {} is below 95% of scaled heightmap max {scaled_height_max}",
                    bounds.max_y
                );
                eprintln!(
                    "area{area_number:02}: TAG0 heuristic run={} span=({span_x:.2}, {span_z:.2}) max_y={:.2}; terrain heightmax*H={scaled_height_max:.2}",
                    bounds.run_len, bounds.max_y
                );
            } else {
                eprintln!(
                    "area{area_number:02}: [unknown] no TAG0 16-byte-vector run detected by documented heuristic"
                );
            }
            assert!(area.normal_map.width > 0 && area.normal_map.height > 0);
            assert!(area.splat_alpha.width > 0 && area.splat_alpha.height > 0);
            assert!(
                area.material.is_some() || area_number == 6,
                "area {area_number:02} has no uniquely identified material"
            );
            assert!(
                area.masks.is_empty()
                    || area
                        .masks
                        .iter()
                        .all(|mask| mask.bytes.len() == 4096 * 4096)
            );
            eprintln!(
                "area{area_number:02}: {}x{} heights, {}x{} normals, {}x{} splat, {} material slots, {} unresolved textures, {} masks",
                area.heightmap.width,
                area.heightmap.height,
                area.normal_map.width,
                area.normal_map.height,
                area.splat_alpha.width,
                area.splat_alpha.height,
                area.material
                    .as_ref()
                    .map_or(0, |material| material.slots.textures.len()),
                area.unresolved_ground_textures.len(),
                area.masks.len(),
            );
        }
    }
}
