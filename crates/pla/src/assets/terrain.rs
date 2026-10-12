//! Evidence-bounded PLA terrain asset loading and chunked Bevy mesh creation.
//!
//! Terrain archives do not expose entry names through `GfPak`; payloads are
//! identified by BNTX internal names, signatures, and independently observed
//! material strings. Vegetation masks retain their entry indices only as
//! provenance: entry order is not treated as semantic identity.

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTerrainMask {
    pub source_entry_index: usize,
    pub width: u32,
    pub height: u32,
    /// Raw 0/255 mask payload, excluding its observed 72-byte header.
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct TerrainArea {
    pub area: u8,
    pub source_path: PathBuf,
    pub heightmap: FloatTextureData,
    pub normal_map: TextureData,
    pub splat_alpha: TextureData,
    pub material: Option<TerrainMaterial>,
    pub ground_textures: HashMap<String, TextureData>,
    /// Material slots whose corresponding internal BNTX name was not uniquely
    /// identified; texture bindings are not silently fabricated.
    pub unresolved_ground_textures: Vec<String>,
    /// These are candidates described by the census, not proven vegetation
    /// semantics.
    pub masks: Vec<RawTerrainMask>,
}

impl TerrainArea {
    /// Compose albedo using BaseColorMap slot order as the provisional R/G/B/A
    /// to layer mapping. The game channel binding is a hypothesis, not a
    /// verified material fact.
    pub fn blend_splat_by_slot_order_hypothesis(&self) -> Result<TextureData, String> {
        let material = self
            .material
            .as_ref()
            .ok_or("terrain material was not uniquely identified")?;
        let layers = material
            .slots
            .textures
            .iter()
            .filter(|(_, role)| role == "BaseColorMap")
            .take(4)
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
        // The seventh pack names its one terrain cell `terrain_1_1`; areas
        // 00–05 use `areaNN_0_0` in their BNTX names.
        let tile_prefix = if area == 6 {
            "terrain_1_1".to_owned()
        } else {
            format!("area{area:02}_0_0")
        };
        let expected_height = format!("{tile_prefix}_heightmap");
        let expected_normal = format!("{tile_prefix}_normal");
        let expected_splat = format!("{tile_prefix}_splat_alpha0");

        let mut heightmap = None;
        let mut normal_map = None;
        let mut splat_alpha = None;
        let mut masks = Vec::new();
        let mut material_candidates: Vec<(Vec<u8>, TrMtr, usize)> = Vec::new();
        let mut bntx_entries: Vec<(usize, Vec<String>)> = Vec::new();

        for (entry_index, entry) in pack.entries.iter().enumerate() {
            let decoded = pack
                .decoded_data(&bytes, entry)
                .map_err(|e| format!("decode terrain entry {entry_index}: {e}"))?;

            if decoded.starts_with(b"BNTX") {
                let bntx = Bntx::parse(&decoded)
                    .map_err(|e| format!("parse BNTX in terrain entry {entry_index}: {e}"))?;
                bntx_entries.push((
                    entry_index,
                    bntx.textures
                        .iter()
                        .map(|texture| texture.name.clone())
                        .collect(),
                ));
                for (texture_index, texture) in bntx.textures.iter().enumerate() {
                    if texture.name == expected_height {
                        heightmap = Some(bntx.decode_f32(&decoded, texture_index)?);
                    } else if texture.name == expected_normal {
                        normal_map = Some(bntx.decode(&decoded, texture_index)?);
                    } else if texture.name == expected_splat {
                        splat_alpha = Some(bntx.decode(&decoded, texture_index)?);
                    }
                }
                continue;
            }

            if decoded.len() == MASK_HEADER_SIZE + MASK_PAYLOAD_SIZE {
                masks.push(RawTerrainMask {
                    source_entry_index: entry_index,
                    width: MASK_SIDE as u32,
                    height: MASK_SIDE as u32,
                    bytes: decoded[MASK_HEADER_SIZE..].to_vec(),
                });
            }

            // TRMTR has no simple signature in the existing parser. Admit a
            // candidate only when it actually contains texture slots. Prefer
            // a candidate whose bytes mention this tile; resolve ties as
            // ambiguous rather than trusting unknown entry order.
            if let Ok(slots) = TrMtr::parse(&decoded) {
                let score = slots
                    .textures
                    .iter()
                    .filter(|(file, _)| file.contains(&tile_prefix))
                    .count()
                    + usize::from(
                        decoded
                            .windows(tile_prefix.len())
                            .any(|w| w == tile_prefix.as_bytes()),
                    );
                material_candidates.push((decoded, slots, score));
            }
        }

        let material = choose_material(material_candidates)?;
        let mut ground_textures = HashMap::new();
        let mut unresolved_ground_textures = Vec::new();
        if let Some(material) = &material {
            for (file_name, _) in &material.slots.textures {
                let stem = file_name
                    .strip_suffix(".bntx")
                    .or_else(|| file_name.strip_suffix(".BNTX"))
                    .unwrap_or(file_name);
                let matches: Vec<_> = bntx_entries
                    .iter()
                    .filter(|(_, names)| names.iter().any(|name| name == stem))
                    .map(|(entry_index, _)| *entry_index)
                    .collect();
                if matches.len() != 1 {
                    unresolved_ground_textures.push(file_name.clone());
                    continue;
                }
                let entry_index = matches[0];
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
            heightmap: heightmap
                .ok_or_else(|| format!("missing internal BNTX texture {expected_height}"))?,
            normal_map: normal_map
                .ok_or_else(|| format!("missing internal BNTX texture {expected_normal}"))?,
            splat_alpha: splat_alpha
                .ok_or_else(|| format!("missing internal BNTX texture {expected_splat}"))?,
            material,
            ground_textures,
            unresolved_ground_textures,
            masks,
        })
    }
}

fn choose_material(
    candidates: Vec<(Vec<u8>, TrMtr, usize)>,
) -> Result<Option<TerrainMaterial>, String> {
    if candidates.is_empty() {
        return Ok(None);
    }
    let best_score = candidates
        .iter()
        .map(|candidate| candidate.2)
        .max()
        .unwrap_or(0);
    let mut best: Vec<_> = candidates
        .into_iter()
        .filter(|candidate| candidate.2 == best_score)
        .collect();
    // Main field packs can include duplicate material variants whose opaque
    // bytes differ but whose parsed texture bindings and parameter candidates
    // are identical. Coalesce only when all currently exposed facts agree.
    let mut unique = Vec::new();
    for candidate in best.drain(..) {
        let parameters = material_parameter_candidates(&candidate.0);
        if !unique.iter().any(|existing: &(Vec<u8>, TrMtr, usize)| {
            existing.1 == candidate.1 && material_parameter_candidates(&existing.0) == parameters
        }) {
            unique.push(candidate);
        }
    }
    if best_score == 0 && unique.len() != 1 {
        return Ok(None);
    }
    if unique.len() > 1 {
        // Do not bind one of several different top-scoring materials by entry
        // order; callers still get the terrain maps and can report the gap.
        return Ok(None);
    }
    let (bytes, slots, _) = unique.remove(0);
    Ok(Some(TerrainMaterial {
        parameter_candidates: material_parameter_candidates(&bytes),
        slots,
    }))
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

/// Settings for a terrain mesh. Defaults are deliberately provisional debug
/// units; no evidence currently establishes in-game metres per sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainMeshSettings {
    /// Provisional default: one horizontal debug unit per height sample.
    pub horizontal_cell_size: f32,
    /// Provisional default: 100 debug units for normalized height 0..1.
    pub vertical_scale: f32,
    /// Maximum cells per side per mesh (64 keeps 2049² areas split up).
    pub chunk_cells: u32,
}

impl Default for TerrainMeshSettings {
    fn default() -> Self {
        Self {
            horizontal_cell_size: 1.0,
            vertical_scale: 100.0,
            chunk_cells: 64,
        }
    }
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
                    let world_x =
                        (x as f32 - (width - 1) as f32 * 0.5) * settings.horizontal_cell_size;
                    let world_z =
                        (z as f32 - (height - 1) as f32 * 0.5) * settings.horizontal_cell_size;
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
            ..TerrainMeshSettings::default()
        };
        assert!(build_mesh_chunks(&heightmap, invalid).is_err());
        heightmap.samples[2] = f32::NAN;
        assert!(build_mesh_chunks(&heightmap, TerrainMeshSettings::default()).is_err());
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
            material: Some(TerrainMaterial {
                slots: TrMtr {
                    textures: vec![
                        ("first.bntx".into(), "BaseColorMap".into()),
                        ("second.bntx".into(), "BaseColorMap".into()),
                    ],
                    flags: Vec::new(),
                },
                parameter_candidates: Vec::new(),
            }),
            ground_textures,
            unresolved_ground_textures: Vec::new(),
            masks: Vec::new(),
        };
        let blended = area.blend_splat_by_slot_order_hypothesis().unwrap();
        assert_eq!(blended.rgba, [128, 0, 127, 255]);
    }

    #[test]
    fn game_area_loader_skips_when_game_pack_is_absent() {
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
