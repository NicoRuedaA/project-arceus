//! Player character skeleton: the shared biped base plus the per-part extension
//! skeletons (face, hair, clothes).
//!
//! Evidence (see reports/data-progress/player_model.md): only `p2_base0001_00_default` carries a
//! full `.trskl` (76 nodes, 74 rigs, 10 IK records). Face, hair and clothes packs each carry a
//! small extension `.trskl` whose root field 4 is a *rig offset* (face 74, hair 125, clothes
//! 147) and whose root nodes name their parent bone in the base skeleton in the "locator"
//! string (`head`, `hips`, `neck`, `left_arm_02`...). The vertex joint bytes of every part are
//! therefore indices in one global rig space; unused slots between the ranges exist.
//!
//! [`build_player_skeleton`] merges base + extensions into one ordinary [`TrSkl`] with compact
//! rig numbers plus the global-to-compact table needed to remap vertex joints.

use super::{
    gltf_export,
    pokemon_model::{add_static_material_for_player, save_player_eye_texture, save_player_eye_texture_named, LayerBlendFormula, PokemonModel},
    tr::{TrAnmClip, TrSkl, TrSklBindRecord, TrSklMatrix, TrSklNode, TrSklTransform},
};
use std::path::Path;

/// Bind/rest-pose agreement tolerance (metres). The Pokémon exports use 1e-4; the player's hair
/// extension bones disagree by up to 4.5e-4 (0.45 mm), so 1e-3 is used here and documented.
pub const BIND_EPSILON: f64 = 1.0e-3;

/// The 15 hair colour names of `appli/table/hair/hair_color_table.bin` (ids `hair_color_01..15`,
/// NAMES VERIFIED in the data, RGB NOT present there) with RGB values ASSUMED to match the names.
/// The eyebrows follow the same ids (dress-up slot "eyebrow colour").
pub const HAIR_COLOR_PALETTE: &[(&str, [u8; 3])] = &[
    ("darkbrown", [72, 48, 34]),
    ("darknavy", [28, 34, 62]),
    ("platinumbrown", [150, 112, 82]),
    ("black", [30, 28, 30]),
    ("white", [235, 232, 226]),
    ("red", [170, 48, 40]),
    ("navy", [48, 70, 130]),
    ("turquoise", [60, 170, 170]),
    ("violet", [130, 80, 160]),
    ("orange", [220, 130, 50]),
    ("peach", [240, 170, 150]),
    ("milk", [220, 190, 160]),
    ("skyblue", [120, 180, 230]),
    ("gray", [150, 150, 155]),
    ("blond", [225, 190, 100]),
];

/// The 14 eye colour names of the dress-up table face variants
/// (`p?_face0001_00_default_<name>`), NAMES VERIFIED, RGB ASSUMED to match the names.
pub const EYE_COLOR_PALETTE: &[(&str, [u8; 3])] = &[
    ("brown", [90, 58, 36]),
    ("hazel", [130, 95, 50]),
    ("black", [30, 26, 26]),
    ("gray", [130, 135, 140]),
    ("blue", [60, 100, 190]),
    ("ultramarine", [40, 60, 170]),
    ("green", [60, 150, 80]),
    ("yellowgreen", [150, 185, 60]),
    ("yellow", [225, 200, 60]),
    ("red", [190, 50, 50]),
    ("poppyred", [215, 70, 60]),
    ("purple", [130, 70, 160]),
    ("lavender", [175, 150, 215]),
    ("white", [235, 235, 235]),
];

/// Default iris colour (ASSUMED; the data has names only): blue-grey like the reference screenshots.
pub const DEFAULT_EYE_RGB: [u8; 3] = [88, 118, 150];

/// Merged skeleton and the vertex-joint remapping tables.
#[derive(Debug, Clone)]
pub struct PlayerSkeleton {
    /// Combined hierarchy with compact rig indices (`0..bind_count`).
    pub skeleton: TrSkl,
    /// Bind records aligned with the compact rig indices.
    pub binds: Vec<TrSklBindRecord>,
    /// Same nodes, but `rig_index` is the GLOBAL rig number (used to read vertex joints).
    pub lookup: TrSkl,
    /// Global rig number -> compact rig number (`None` for unused slots).
    pub global_to_compact: Vec<Option<u16>>,
    /// Human-readable layout notes (per extension: name, offset, node count).
    pub notes: Vec<String>,
}

struct Fb<'a>(&'a [u8]);

struct Table {
    at: usize,
    fields: Vec<u16>,
}

impl Fb<'_> {
    fn u32(&self, at: usize) -> Result<u32, String> {
        self.0
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .ok_or_else(|| "flatbuffer read outside file".to_owned())
    }
    fn u16(&self, at: usize) -> Result<u16, String> {
        self.0
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
            .ok_or_else(|| "flatbuffer read outside file".to_owned())
    }
    fn f32(&self, at: usize) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32(at)?))
    }
    fn target(&self, at: usize) -> Result<usize, String> {
        let delta = self.u32(at)? as usize;
        if delta < 4 {
            return Err("invalid relative offset".into());
        }
        Ok(at + delta)
    }
    fn table(&self, at: usize) -> Result<Table, String> {
        let soffset = self.u32(at)? as i32;
        let vtable = usize::try_from(at as i64 - i64::from(soffset)).map_err(|_| "bad vtable")?;
        let size = self.u16(vtable)? as usize;
        if size < 4 || size % 2 != 0 || size > 64 {
            return Err("unsupported vtable".into());
        }
        let mut fields = Vec::new();
        for i in 0..(size - 4) / 2 {
            fields.push(self.u16(vtable + 4 + 2 * i)?);
        }
        Ok(Table { at, fields })
    }
    fn field(&self, t: &Table, id: usize) -> Option<usize> {
        match t.fields.get(id) {
            Some(&o) if o != 0 => Some(t.at + o as usize),
            _ => None,
        }
    }
    fn vector(&self, t: &Table, id: usize) -> Result<Vec<usize>, String> {
        let Some(p) = self.field(t, id) else { return Ok(Vec::new()) };
        let v = self.target(p)?;
        let n = self.u32(v)? as usize;
        if n > 4096 {
            return Err("vector too long".into());
        }
        (0..n).map(|i| self.target(v + 4 + 4 * i)).collect()
    }
    fn string(&self, t: &Table, id: usize) -> Result<Option<String>, String> {
        let Some(p) = self.field(t, id) else { return Ok(None) };
        let s = self.target(p)?;
        let n = self.u32(s)? as usize;
        let bytes = self.0.get(s + 4..s + 4 + n).ok_or("string outside file")?;
        Ok(Some(String::from_utf8(bytes.to_vec()).map_err(|_| "string is not UTF-8")?))
    }
    fn vec3(&self, t: &Table, id: usize) -> Result<[f32; 3], String> {
        let p = self.field(t, id).ok_or_else(|| format!("missing vec3 field {id}"))?;
        Ok([self.f32(p)?, self.f32(p + 4)?, self.f32(p + 8)?])
    }
    fn index(&self, t: &Table, id: usize) -> Result<Option<usize>, String> {
        match self.field(t, id) {
            None => Ok(None),
            Some(p) => match self.u32(p)? as i32 {
                -1 => Ok(None),
                v if v >= 0 => Ok(Some(v as usize)),
                _ => Err("negative index".into()),
            },
        }
    }
}

struct ParsedExt {
    offset: usize,
    nodes: Vec<TrSklNode>,
    /// Parent as written: local index, or a base-bone name for roots.
    locators: Vec<Option<String>>,
    binds: Vec<TrSklBindRecord>,
}

fn parse_bind_records(fb: &Fb, root: &Table) -> Result<Vec<TrSklBindRecord>, String> {
    let mut out = Vec::new();
    for at in fb.vector(root, 2)? {
        let t = fb.table(at)?;
        let flags = [
            *fb.0.get(fb.field(&t, 0).ok_or("bind flag 0")?).ok_or("bind flag 0")?,
            *fb.0.get(fb.field(&t, 1).ok_or("bind flag 1")?).ok_or("bind flag 1")?,
        ];
        let m = fb.table(fb.target(fb.field(&t, 2).ok_or("bind matrix")?)?)?;
        let mut rows = TrSklMatrix::IDENTITY.rows;
        for column in 0..4 {
            let v = fb.vec3(&m, column)?;
            for (row, value) in v.iter().enumerate() {
                rows[row][column] = f64::from(*value);
            }
        }
        out.push(TrSklBindRecord { flags, matrix: TrSklMatrix { rows } });
    }
    Ok(out)
}

fn parse_ext(bytes: &[u8]) -> Result<ParsedExt, String> {
    let fb = Fb(bytes);
    let root = fb.table(fb.target(0)?)?;
    let offset = fb
        .field(&root, 4)
        .map(|p| fb.u32(p))
        .transpose()?
        .ok_or("extension skeleton has no rig offset")? as usize;
    let mut nodes = Vec::new();
    let mut locators = Vec::new();
    for at in fb.vector(&root, 1)? {
        let t = fb.table(at)?;
        let transform = fb.table(fb.target(fb.field(&t, 1).ok_or("node transform")?)?)?;
        let loc = fb.string(&t, 6)?.filter(|s| !s.is_empty());
        nodes.push(TrSklNode {
            name: fb.string(&t, 0)?.ok_or("node name")?,
            locator: None,
            local: TrSklTransform {
                scale: fb.vec3(&transform, 0)?,
                rotation: fb.vec3(&transform, 1)?,
                translation: fb.vec3(&transform, 2)?,
            },
            scale_pivot: fb.vec3(&t, 2)?,
            rotate_pivot: fb.vec3(&t, 3)?,
            parent: fb.index(&t, 4)?,
            rig_index: fb.index(&t, 5)?,
        });
        locators.push(loc);
    }
    let binds = parse_bind_records(&fb, &root)?;
    if binds.len() != nodes.len() {
        return Err(format!("extension has {} nodes but {} binds", nodes.len(), binds.len()));
    }
    Ok(ParsedExt { offset, nodes, locators, binds })
}

/// Rows of `chara/table/skin_color_table.bin`: `(u64 key hash, sRGB bytes)`.
/// Eight rows: two groups of the same four tones. The key hashes are unresolved names.
pub fn read_skin_color_table(bytes: &[u8]) -> Result<Vec<(u64, [u8; 3])>, String> {
    let fb = Fb(bytes);
    let root = fb.table(fb.target(0)?)?;
    let mut out = Vec::new();
    for at in fb.vector(&root, 0)? {
        let t = fb.table(at)?;
        let hash_at = fb.field(&t, 0).ok_or("skin row has no hash")?;
        let hash = u64::from(fb.u32(hash_at)?) | (u64::from(fb.u32(hash_at + 4)?) << 32);
        let mut rgb = [0u8; 3];
        for (i, v) in rgb.iter_mut().enumerate() {
            *v = *fb.0.get(fb.field(&t, 1 + i).ok_or("skin row channel missing")?).ok_or("outside file")?;
        }
        out.push((hash, rgb));
    }
    Ok(out)
}

/// Merge the base skeleton bytes with the extension skeletons `(label, bytes)`.
pub fn build_player_skeleton(
    base_bytes: &[u8],
    extensions: &[(String, Vec<u8>)],
) -> Result<PlayerSkeleton, String> {
    let base = TrSkl::parse_ignoring_ik(base_bytes)?;
    let base_binds = TrSkl::read_bind_records_ignoring_ik(base_bytes)?;
    let mut nodes = base.nodes.clone();
    let mut binds = base_binds;
    let mut global_to_compact: Vec<Option<u16>> = (0..base.bind_count).map(|i| Some(i as u16)).collect();
    let mut lookup_rig: Vec<Option<usize>> = nodes.iter().map(|n| n.rig_index).collect();
    let mut notes = vec![format!("base: {} nodes, {} rigs", nodes.len(), base.bind_count)];
    for (label, bytes) in extensions {
        let ext = parse_ext(bytes).map_err(|e| format!("{label}: {e}"))?;
        let first = nodes.len();
        let compact_first = binds.len();
        let max_rig = ext.nodes.iter().filter_map(|n| n.rig_index).max().map_or(0, |m| m + 1);
        if ext.nodes.iter().filter_map(|n| n.rig_index).count() != ext.nodes.len() || max_rig != ext.nodes.len() {
            return Err(format!("{label}: extension rigs are not a dense 0..n range"));
        }
        if global_to_compact.len() > ext.offset {
            // An earlier range already covers this offset only when ranges overlap.
            if global_to_compact[ext.offset..].iter().any(Option::is_some) {
                return Err(format!("{label}: rig range overlaps an earlier range"));
            }
        }
        if global_to_compact.len() < ext.offset + max_rig {
            global_to_compact.resize(ext.offset + max_rig, None);
        }
        for (i, node) in ext.nodes.iter().enumerate() {
            let mut node = node.clone();
            let local_rig = node.rig_index.unwrap_or(0);
            node.parent = match (node.parent, &ext.locators[i]) {
                (Some(p), _) => Some(first + p),
                (None, Some(name)) => Some(
                    nodes
                        .iter()
                        .position(|n| &n.name == name)
                        .ok_or_else(|| format!("{label}: parent bone {name} not in skeleton"))?,
                ),
                (None, None) => return Err(format!("{label}: node {} has no parent", node.name)),
            };
            if nodes.iter().any(|n| n.name == node.name) {
                return Err(format!("{label}: duplicate bone name {}", node.name));
            }
            node.rig_index = Some(compact_first + local_rig);
            global_to_compact[ext.offset + local_rig] = Some((compact_first + local_rig) as u16);
            lookup_rig.push(Some(ext.offset + local_rig));
            nodes.push(node);
        }
        binds.extend(ext.binds);
        notes.push(format!("{label}: {} nodes at global rig {}..{}", ext.nodes.len(), ext.offset, ext.offset + ext.nodes.len()));
    }
    let skeleton = TrSkl {
        bones: nodes.iter().map(|n| n.name.clone()).collect(),
        offsets: Vec::new(),
        nodes: nodes.clone(),
        bind_count: binds.len(),
        root_flag: base.root_flag,
    };
    let mut lookup_nodes = nodes;
    for (node, rig) in lookup_nodes.iter_mut().zip(&lookup_rig) {
        node.rig_index = *rig;
    }
    let lookup = TrSkl {
        bones: skeleton.bones.clone(),
        offsets: Vec::new(),
        nodes: lookup_nodes,
        bind_count: global_to_compact.len(),
        root_flag: base.root_flag,
    };
    Ok(PlayerSkeleton { skeleton, binds, lookup, global_to_compact, notes })
}

/// Export options for [`write_player_gltf`].
#[derive(Debug, Clone, Copy, Default)]
pub struct PlayerExportOptions {
    /// Drop meshes named `hide_*` (name-based: they are the dress-up system's hide/cover geometry).
    pub skip_hidden: bool,
    /// sRGB skin tone multiplied into the albedo of skin-bound materials
    /// (`face_skin`, `body_skin`, `face`). Hypothesis, not a proven shader step.
    pub skin_rgb: Option<[u8; 3]>,
    /// sRGB hair colour. The hair/eyebrow albedo is a grey tile: it is multiplied (in linear
    /// light) by this colour after normalising its mean grey to 1, so the shading detail is kept.
    /// Hair colour is a runtime choice in the game; the exact RGB is not in the data (ASSUMED).
    pub hair_rgb: Option<[u8; 3]>,
    /// sRGB eye colour, applied ONLY to the iris (layer-mask G of the eye material) of the grey
    /// albedo, normalised on its brightest iris pixels. Sclera and highlight stay albedo + emission.
    /// ASSUMED RGB, as above.
    pub eye_rgb: Option<[u8; 3]>,
    /// Write the whole colour palettes (hair 15, eyebrow 15, eye 14) of the shared grey albedos
    /// next to the glTF, so the runtime can swap the iris/hair texture for the chosen colour.
    /// (Clothes colour variants are NOT covered: how the catalogue index maps to the outfit
    /// texture is UNKNOWN.)
    pub color_variants: bool,
    /// Leave skin-bound albedos untinted and write the skin tone into the glTF `baseColorFactor`
    /// instead; the runtime then multiplies the material's base colour (identical maths, no
    /// 1 MB PNG per tone). Used by the per-piece dress-up export.
    pub skin_unbaked: bool,
}

/// Files written by [`write_player_gltf`] beyond the glTF itself.
#[derive(Debug, Default, Clone)]
pub struct PlayerExportReport {
    /// `(set, name, file)` for every colour-variant PNG written (`set` = `hair` | `eyebrow` | `eye`,
    /// `file` relative to the glTF). The runtime swaps the base-colour texture by set + name.
    pub color_files: Vec<(String, String, String)>,
}

/// How a grey albedo is normalised before the colour multiply.
#[derive(Clone, Copy)]
enum GreyRef {
    /// Mean of the opaque pixels maps to the target colour (hair, eyebrows).
    Mean,
}

fn is_skin_binding(binding: &str) -> bool {
    matches!(binding, "face" | "face_skin" | "body_skin") || binding.ends_with("_skin")
}

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

/// Multiply an RGB(A)8 PNG by an sRGB colour in linear light and save it under a new name.
fn tint_png(dir: &Path, name: &str, rgb: [u8; 3], grey_ref: Option<GreyRef>, suffix: &str) -> Result<String, String> {
    let src = dir.join(name);
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&src).map_err(|e| e.to_string())?));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("png too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err("tint expects 8-bit PNG".into());
    }
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        _ => return Err("tint expects RGB or RGBA PNG".into()),
    };
    let mut tone: [f32; 3] = std::array::from_fn(|i| srgb_to_linear(f32::from(rgb[i]) / 255.0));
    if let Some(r) = grey_ref {
        let greys: Vec<f32> = buf[..info.buffer_size()]
            .chunks_exact(channels)
            .map(|px| srgb_to_linear(f32::from(px[0]) / 255.0))
            .collect();
        let reference = match r {
            GreyRef::Mean => greys.iter().sum::<f32>() / greys.len().max(1) as f32,
        };
        let k = 1.0 / reference.max(1e-3);
        for t in &mut tone {
            *t *= k;
        }
    }
    for px in buf[..info.buffer_size()].chunks_exact_mut(channels) {
        for c in 0..3 {
            let lin = srgb_to_linear(f32::from(px[c]) / 255.0) * tone[c];
            px[c] = (linear_to_srgb(lin.min(1.0)).clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    let out_name = format!("{}_{suffix}.png", name.trim_end_matches(".png"));
    let file = std::fs::File::create(dir.join(&out_name)).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), info.width, info.height);
    enc.set_color(info.color_type);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(&buf[..info.buffer_size()]).map_err(|e| e.to_string())?;
    Ok(out_name)
}

/// Export the LOD0 meshes of all `parts` on the merged skeleton with the given clips.
pub fn write_player_gltf(
    parts: &[&PokemonModel],
    skel: &PlayerSkeleton,
    clips: &[(String, TrAnmClip)],
    gltf_path: impl AsRef<Path>,
    formula: Option<LayerBlendFormula>,
    options: PlayerExportOptions,
) -> Result<PlayerExportReport, String> {
    let mut export = gltf_export::GltfExport::default();
    let mut report = PlayerExportReport::default();
    let out_dir = gltf_path.as_ref().parent().unwrap_or_else(|| Path::new("."));
    for part in parts {
        for mesh in part.meshes.iter().filter(|m| m.lod == Some(0)) {
            if options.skip_hidden && mesh.name.starts_with("hide_") {
                continue;
            }
            let material = add_static_material_for_player(part, mesh, &mut export, gltf_path.as_ref(), formula)?;
            if let Some(index) = material {
                let uri = export.materials[index].base_color_uri.clone();
                if let Some(uri) = uri {
                    let is_skin = is_skin_binding(&mesh.material_binding);
                    let is_hair = uri.contains("_hairstyle_alb");
                    let is_eyebrow = uri.contains("_eyebrow_alb");
                    let is_eye = uri.contains("_eye_alb");
                    if is_skin && options.skin_unbaked {
                        // Untinted albedo + baseColorFactor: the runtime multiplies the skin tone.
                        if let Some(rgb) = options.skin_rgb {
                            export.materials[index].base_color_factor = Some(std::array::from_fn(|i| {
                                if i < 3 { srgb_to_linear(f32::from(rgb[i]) / 255.0) } else { 1.0 }
                            }));
                        }
                    } else {
                        let tinted = if let (Some(rgb), true) = (options.skin_rgb, is_skin) {
                            Some(tint_png(out_dir, &uri, rgb, None, "skintone")?)
                        } else if let (Some(rgb), true) = (options.hair_rgb, is_hair || is_eyebrow) {
                            Some(tint_png(out_dir, &uri, rgb, Some(GreyRef::Mean), "haircolor")?)
                        } else if let (Some(rgb), true) = (options.eye_rgb, is_eye) {
                            Some(save_player_eye_texture(part, mesh, rgb, gltf_path.as_ref())?)
                        } else {
                            None
                        };
                        if tinted.is_some() {
                            export.materials[index].base_color_uri = tinted;
                        }
                    }
                    if options.color_variants {
                        if is_hair {
                            for (name, rgb) in HAIR_COLOR_PALETTE {
                                let file = tint_png(out_dir, &uri, *rgb, Some(GreyRef::Mean), &format!("haircolor_{name}"))?;
                                report.color_files.push(("hair".into(), (*name).into(), file));
                            }
                        } else if is_eyebrow {
                            for (name, rgb) in HAIR_COLOR_PALETTE {
                                let file = tint_png(out_dir, &uri, *rgb, Some(GreyRef::Mean), &format!("eyebrowcolor_{name}"))?;
                                report.color_files.push(("eyebrow".into(), (*name).into(), file));
                            }
                        } else if is_eye {
                            for (name, rgb) in EYE_COLOR_PALETTE {
                                let file = save_player_eye_texture_named(part, mesh, *rgb, gltf_path.as_ref(), name)?;
                                report.color_files.push(("eye".into(), (*name).into(), file));
                            }
                        }
                    }
                }
            }
            let mut prim = gltf_export::primitive_from_tr(
                &mesh.name,
                &mesh.mesh,
                &mesh.buffer_bytes,
                Some(&skel.lookup),
                material,
            )?;
            if let (Some(joints), Some(weights)) = (prim.joints.as_mut(), prim.weights.as_ref()) {
                for (j, w) in joints.iter_mut().zip(weights) {
                    for lane in 0..4 {
                        let global = usize::from(j[lane]);
                        j[lane] = match skel.global_to_compact.get(global).copied().flatten() {
                            Some(c) => c,
                            None if w[lane] == 0.0 => 0,
                            None => return Err(format!("{}: joint {global} has no skeleton node", mesh.name)),
                        };
                    }
                }
            }
            export.primitives.push(prim);
        }
    }
    if export.primitives.is_empty() {
        return Err("no LOD0 mesh in any part".into());
    }
    let (mut nodes, skin) = gltf_export::skeleton_from_tr_with_epsilon(&skel.skeleton, &skel.binds, 0, 0, BIND_EPSILON)?;
    nodes.pop(); // placeholder mesh node, see write_merged_skinned
    export.nodes = nodes;
    export.skins.push(skin);
    for (name, clip) in clips {
        export
            .animations
            .push((name.clone(), gltf_export::animation_tracks_from_tr(&skel.skeleton, clip)?));
    }
    let root = export.skins.first().and_then(|s| s.skeleton);
    pokemon_model_add_mesh_nodes(&mut export, root)?;
    export.write(gltf_path)?;
    Ok(report)
}

fn pokemon_model_add_mesh_nodes(export: &mut gltf_export::GltfExport, parent: Option<usize>) -> Result<(), String> {
    for mesh in 0..export.primitives.len() {
        let node_index = export.nodes.len();
        export.nodes.push(gltf_export::GltfNode {
            name: export.primitives[mesh].name.clone(),
            children: Vec::new(),
            mesh: Some(mesh),
            skin: Some(0),
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        });
        if let Some(parent) = parent {
            export.nodes.get_mut(parent).ok_or("mesh parent outside node table")?.children.push(node_index);
        }
    }
    Ok(())
}
