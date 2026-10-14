//! Vegetation ("detail") placement data and a deterministic scatter generator.
//!
//! What is decoded (see `reports/data-progress/vegetation.md` for the evidence):
//! * `areaNN_0_0_detailmap_K.trtbf` is a 2048x2048 **RGBA8** texture (28-byte FlatBuffer header,
//!   payload, 44-byte trailer holding the dimensions), every channel 0 or 255. It is NOT a 4096x4096
//!   single-channel mask (the earlier reading, which also lost the first 44 texels).
//! * The terrain descriptor lists `P` plant models (`plants_*.trmdl`) with per-plant parameters; the
//!   number of detail maps is exactly `ceil(P / 4)` in all seven areas.
//! * HYPOTHESIS (supported by counts, not proven by the game's own code): plant `i` is driven by
//!   channel `i % 4` of detail map `i / 4`.
//! * Map orientation: column = x, row = z, no flips (correlation with the baked ground colour is the
//!   highest of the 8 flip/transpose variants: 0.16 against <= 0.07).
//!
//! What is assumed: instances per texel (tri-budget rule below), scale ranges (descriptor floats
//! 1..4), tint endpoints (floats 6/7), draw distance. The scatter itself is pure and deterministic:
//! the same (seed, area, plant, texel) always produces the same instances, texel ownership makes
//! chunk borders seamless.

use std::path::Path;

use super::bntx::{Bntx, FloatTextureData};
use super::gfpak::GfPak;
use super::terrain::sample_height;

pub const MASK_TEXELS: usize = 2048;
pub const MASK_HEADER_LEN: usize = 28;
pub const MASK_TRAILER_LEN: usize = 44;
/// Texels per chunk side (64 texels = 32.03 m): chunks tile the mask exactly.
pub const CHUNK_TEXELS: u32 = 64;

// ---------------------------------------------------------------------------------------------
// Minimal bounds-checked FlatBuffer reading (schema-free; only what the descriptor needs).
// ---------------------------------------------------------------------------------------------

struct Fb<'a>(&'a [u8]);

impl<'a> Fb<'a> {
    fn u16(&self, o: usize) -> Option<u16> {
        Some(u16::from_le_bytes(self.0.get(o..o.checked_add(2)?)?.try_into().ok()?))
    }
    fn u32(&self, o: usize) -> Option<u32> {
        Some(u32::from_le_bytes(self.0.get(o..o.checked_add(4)?)?.try_into().ok()?))
    }
    fn i32(&self, o: usize) -> Option<i32> {
        Some(i32::from_le_bytes(self.0.get(o..o.checked_add(4)?)?.try_into().ok()?))
    }
    fn f32(&self, o: usize) -> Option<f32> {
        Some(f32::from_le_bytes(self.0.get(o..o.checked_add(4)?)?.try_into().ok()?))
    }
    /// Absolute position of field `idx` of the table at `t`, or None when absent.
    fn field(&self, t: usize, idx: usize) -> Option<usize> {
        let vt = (t as i64 - self.i32(t)? as i64) as usize;
        let vs = self.u16(vt)? as usize;
        if 4 + 2 * idx + 2 > vs {
            return None;
        }
        let off = self.u16(vt + 4 + 2 * idx)? as usize;
        if off == 0 {
            None
        } else {
            Some(t + off)
        }
    }
    /// Follow a forward offset stored at `pos`.
    fn follow(&self, pos: usize) -> Option<usize> {
        pos.checked_add(self.u32(pos)? as usize)
    }
    fn string_at(&self, pos: usize) -> Option<String> {
        let s = self.follow(pos)?;
        let n = self.u32(s)? as usize;
        let bytes = self.0.get(s + 4..s.checked_add(4)?.checked_add(n)?)?;
        String::from_utf8(bytes.to_vec()).ok()
    }
    fn vector_len_start(&self, pos: usize) -> Option<(usize, usize)> {
        let v = self.follow(pos)?;
        Some((self.u32(v)? as usize, v + 4))
    }
}

// ---------------------------------------------------------------------------------------------
// Detail mask
// ---------------------------------------------------------------------------------------------

/// A decoded `*_detailmap_K.trtbf`: `width x height` texels, 4 interleaved 0/255 channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailMask {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl DetailMask {
    #[inline]
    pub fn get(&self, col: u32, row: u32, channel: usize) -> u8 {
        self.rgba[((row as usize * self.width as usize) + col as usize) * 4 + channel]
    }
}

/// Parse a decoded detail-map entry. Layout (verified on all 22 maps of areas 00-06): FlatBuffer
/// header of 28 bytes whose last u32 is the byte length `n` of the payload vector, then `n` bytes
/// of texels, then a 44-byte table with `[1, width, height, 1, 1, n]`.
pub fn parse_detail_mask(decoded: &[u8]) -> Result<DetailMask, String> {
    let fb = Fb(decoded);
    let n = fb.u32(24).ok_or("detail map shorter than its header")? as usize;
    if decoded.len() != MASK_HEADER_LEN + n + MASK_TRAILER_LEN {
        return Err(format!(
            "detail map length {} != 28 + {n} + 44",
            decoded.len()
        ));
    }
    let tail = MASK_HEADER_LEN + n;
    // trailer = vtable (16 bytes) + table; field 1 = width, field 2 = height.
    let table = tail + 16;
    let width = fb
        .field(table, 1)
        .and_then(|p| fb.u32(p))
        .ok_or("detail map trailer has no width")?;
    let height = fb
        .field(table, 2)
        .and_then(|p| fb.u32(p))
        .ok_or("detail map trailer has no height")?;
    if width == 0 || height == 0 || n != width as usize * height as usize * 4 {
        return Err(format!(
            "detail map payload {n} bytes is not {width}x{height} RGBA8"
        ));
    }
    Ok(DetailMask {
        width,
        height,
        rgba: decoded[MASK_HEADER_LEN..MASK_HEADER_LEN + n].to_vec(),
    })
}

// ---------------------------------------------------------------------------------------------
// Descriptor: plant table
// ---------------------------------------------------------------------------------------------

/// One entry of the descriptor's plant table (`plants_*.trmdl`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlantSpec {
    /// Model name without extension (e.g. `plants_area00_grass01_alb`).
    pub model: String,
    /// Descriptor floats 1,2: min/max horizontal scale (meaning ASSUMED).
    pub scale_h: [f32; 2],
    /// Descriptor floats 3,4: min/max vertical scale (meaning ASSUMED).
    pub scale_v: [f32; 2],
    /// Float 5, 0.1 everywhere (meaning unknown).
    pub param5: f32,
    /// Floats 6 and 7 (two RGBA tints; used as lerp endpoints, ASSUMED).
    pub tint_a: [f32; 4],
    pub tint_b: [f32; 4],
    /// Integer field 8 (always 4) and 9 (32..64; looks like a distance in metres; unproven).
    pub field8: u32,
    pub field9: u32,
    /// Floats 10..18 (16, 3, 1, ... 10.5; meaning unknown), 0.0 when absent.
    pub extra: [f32; 9],
}

/// Everything the descriptor says about vegetation.
#[derive(Debug, Clone, PartialEq)]
pub struct VegetationDescriptor {
    /// World extent (x, z) in metres (verified elsewhere, 1025 in every area).
    pub extent: [f32; 2],
    /// Height scale H for the normalised heightmap.
    pub height: f32,
    pub detailmap_files: Vec<String>,
    pub plants: Vec<PlantSpec>,
}

impl PlantSpec {
    /// Detail map index and channel that drive plant `plant_index` (HYPOTHESIS: `i/4`, `i%4`).
    pub fn mask_slot(plant_index: usize) -> (usize, usize) {
        (plant_index / 4, plant_index % 4)
    }
}

pub fn parse_descriptor(bytes: &[u8]) -> Result<VegetationDescriptor, String> {
    let fb = Fb(bytes);
    let root = fb.u32(0).ok_or("descriptor too short")? as usize;
    let info = fb.field(root, 10).ok_or("descriptor has no extent table")?;
    let info_t = fb.follow(info).filter(|&t| fb.i32(t).is_some());
    // Field 10 is an inline table stored directly (offset to table); fall back to reading it in place.
    let table = match info_t {
        Some(t) if fb.field(t, 1).is_some() => t,
        _ => info,
    };
    let ext = fb.field(table, 1).ok_or("descriptor extent struct missing")?;
    let extent = [
        fb.f32(ext).ok_or("extent x")?,
        fb.f32(ext + 8).ok_or("extent z")?,
    ];
    let height = fb.f32(ext + 4).ok_or("extent H")?;
    if !(extent[0].is_finite() && extent[1].is_finite() && height.is_finite()) || extent[0] <= 0.0 {
        return Err("descriptor extent is not finite".into());
    }

    let mut detailmap_files = Vec::new();
    if let Some(p) = fb.field(root, 8) {
        let (n, start) = fb.vector_len_start(p).ok_or("detailmap vector")?;
        for i in 0..n.min(64) {
            detailmap_files.push(fb.string_at(start + 4 * i).ok_or("detailmap name")?);
        }
    }

    let mut plants = Vec::new();
    if let Some(p) = fb.field(root, 6) {
        let (n, start) = fb.vector_len_start(p).ok_or("plant vector")?;
        for i in 0..n.min(256) {
            let t = fb.follow(start + 4 * i).ok_or("plant table")?;
            let name_pos = fb.field(t, 0).ok_or("plant without name")?;
            let model = fb
                .string_at(name_pos)
                .ok_or("plant name")?
                .trim_end_matches(".trmdl")
                .to_owned();
            let f = |idx: usize, d: f32| fb.field(t, idx).and_then(|p| fb.f32(p)).unwrap_or(d);
            let u = |idx: usize| fb.field(t, idx).and_then(|p| fb.u32(p)).unwrap_or(0);
            let v4 = |idx: usize| -> [f32; 4] {
                match fb.field(t, idx) {
                    Some(p) => [
                        fb.f32(p).unwrap_or(1.0),
                        fb.f32(p + 4).unwrap_or(1.0),
                        fb.f32(p + 8).unwrap_or(1.0),
                        fb.f32(p + 12).unwrap_or(1.0),
                    ],
                    None => [1.0; 4],
                }
            };
            let mut extra = [0.0f32; 9];
            for (k, e) in extra.iter_mut().enumerate() {
                *e = f(10 + k, 0.0);
            }
            plants.push(PlantSpec {
                model,
                scale_h: [f(1, 1.0), f(2, 1.0)],
                scale_v: [f(3, 1.0), f(4, 1.0)],
                param5: f(5, 0.0),
                tint_a: v4(6),
                tint_b: v4(7),
                field8: u(8),
                field9: u(9),
                extra,
            });
        }
    }
    if plants.is_empty() {
        return Err("descriptor lists no plants".into());
    }
    Ok(VegetationDescriptor { extent, height, detailmap_files, plants })
}

// ---------------------------------------------------------------------------------------------
// Area loader
// ---------------------------------------------------------------------------------------------

/// Vegetation inputs of one area: plant table, detail maps and the heightmap for ground height.
#[derive(Debug, Clone)]
pub struct VegetationArea {
    pub area: u8,
    pub descriptor: VegetationDescriptor,
    pub masks: Vec<DetailMask>,
    pub heightmap: FloatTextureData,
}

impl VegetationArea {
    /// Ground height in metres at (x, z) (bilinear heightmap x H).
    pub fn ground_y(&self, x: f32, z: f32) -> f32 {
        sample_height(&self.heightmap, self.descriptor.extent, x, z).unwrap_or(0.0) * self.descriptor.height
    }
    /// Size of one mask texel in metres.
    pub fn texel_size(&self) -> f32 {
        self.descriptor.extent[0] / self.masks.first().map_or(MASK_TEXELS as u32, |m| m.width) as f32
    }
    /// The mask and channel driving plant `i`, if the area has that map.
    pub fn mask_for(&self, plant: usize) -> Option<(&DetailMask, usize)> {
        let (map, ch) = PlantSpec::mask_slot(plant);
        self.masks.get(map).map(|m| (m, ch))
    }
}

/// Load vegetation inputs from `<bin_root>/archive/field/terrain/ha_areaNN.gfpak`.
pub fn load_vegetation_area(bin_root: impl AsRef<Path>, area: u8) -> Result<VegetationArea, String> {
    if area > 6 {
        return Err(format!("area must be in 0..=6, got {area}"));
    }
    let path = bin_root
        .as_ref()
        .join("archive/field/terrain")
        .join(format!("ha_area{area:02}.gfpak"));
    let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let pack = GfPak::parse(&bytes)?;
    let tile = if area == 6 { "terrain_1_1".to_owned() } else { format!("area{area:02}_0_0") };
    let prefix = format!("bin/field/terrain/ha_area{area:02}/terrain/");

    // Descriptor: the entry naming the tile material, the detail maps and the plant models.
    let mut descriptor = None;
    for entry in &pack.entries {
        // The descriptor is small (a few KiB); skip the big textures without decoding them.
        if entry.declared_decompressed_size() > 1 << 20 {
            continue;
        }
        let decoded = pack.decoded_data(&bytes, entry)?;
        let text = String::from_utf8_lossy(&decoded);
        if text.contains(&format!("{tile}.trmtr")) && text.contains(".trtbf") && text.contains(".trmdl") {
            if let Ok(d) = parse_descriptor(&decoded) {
                descriptor = Some(d);
                break;
            }
        }
    }
    let descriptor = descriptor.ok_or("no vegetation descriptor found in the pack")?;

    let mut masks = Vec::new();
    for name in &descriptor.detailmap_files {
        let idx = pack
            .entry_index_by_path(&format!("{prefix}{name}"))?
            .ok_or_else(|| format!("pack has no entry for {name}"))?;
        let decoded = pack.decoded_data(&bytes, &pack.entries[idx])?;
        masks.push(parse_detail_mask(&decoded).map_err(|e| format!("{name}: {e}"))?);
    }

    let height_path = format!("{prefix}{tile}_heightmap.bntx");
    let hi = pack
        .entry_index_by_path(&height_path)?
        .ok_or("pack has no heightmap")?;
    let hb = pack.decoded_data(&bytes, &pack.entries[hi])?;
    let bntx = Bntx::parse(&hb)?;
    let ti = bntx
        .textures
        .iter()
        .position(|t| t.name.ends_with("_heightmap"))
        .ok_or("no *_heightmap texture")?;
    let heightmap = bntx.decode_f32(&hb, ti)?;
    Ok(VegetationArea { area, descriptor, masks, heightmap })
}

// ---------------------------------------------------------------------------------------------
// Quality, density rules
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GrassQuality {
    Off,
    #[default]
    Low,
    High,
}

impl GrassQuality {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "off" | "0" | "none" => Some(Self::Off),
            "low" | "1" => Some(Self::Low),
            "high" | "2" => Some(Self::High),
            _ => None,
        }
    }
    /// `--grass off|low|high` from CLI arguments (default low).
    pub fn from_args(args: &[String]) -> Self {
        let Some(raw) = crate::game::cli_flags::value_of(args, "--grass") else {
            return Self::default();
        };
        Self::parse(raw).unwrap_or_else(|| {
            crate::game::cli_flags::report_bad_value("--grass", raw, "off|low|high", "low");
            Self::default()
        })
    }
    /// Target triangles per square metre of masked ground (ASSUMED budget, not game data).
    pub fn tri_budget_per_m2(self) -> f32 {
        match self {
            Self::Off => 0.0,
            Self::Low => 14.0,
            Self::High => 40.0,
        }
    }
    /// Streaming radius (m) for small cards (grass) and for everything else.
    pub fn radii(self) -> (f32, f32) {
        match self {
            Self::Off => (0.0, 0.0),
            Self::Low => (40.0, 64.0),
            Self::High => (70.0, 110.0),
        }
    }
    /// Hard cap on instances alive at once.
    pub fn max_live_instances(self) -> usize {
        match self {
            Self::Off => 0,
            Self::Low => 120_000,
            Self::High => 400_000,
        }
    }
    /// Hard cap on instances generated in one frame (streaming budget).
    pub fn max_instances_per_frame(self) -> usize {
        match self {
            Self::Off => 0,
            Self::Low => 20_000,
            Self::High => 40_000,
        }
    }
}

/// Instances per set texel for a model of `tris` triangles under the tri budget.
/// Cards (<= 8 tris) are capped at 4 per texel; heavy models never go below 0.02.
pub fn density_per_texel(tris: u32, texel_m: f32, quality: GrassQuality) -> f32 {
    if quality == GrassQuality::Off || tris == 0 {
        return 0.0;
    }
    let d = quality.tri_budget_per_m2() * texel_m * texel_m / tris as f32;
    let (lo, hi) = if tris <= 8 { (0.5, 4.0) } else { (0.02, 1.0) };
    d.clamp(lo, hi)
}

/// Streaming radius for a model of `tris` triangles.
pub fn radius_for(tris: u32, quality: GrassQuality) -> f32 {
    let (near, far) = quality.radii();
    if tris <= 8 {
        near
    } else {
        far
    }
}

// ---------------------------------------------------------------------------------------------
// Deterministic scatter
// ---------------------------------------------------------------------------------------------

/// One scattered plant (position in area metres; ground height is added by the caller).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlantInstance {
    pub x: f32,
    pub z: f32,
    pub yaw: f32,
    pub scale_h: f32,
    pub scale_v: f32,
    /// 0..1 mix between the plant's two tints.
    pub tint: f32,
}

/// splitmix64 finalizer: a good integer hash, no `rand` crate needed.
#[inline]
pub fn hash64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Uniform [0, 1) from the top 24 bits.
#[inline]
fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

#[derive(Debug, Clone, Copy)]
pub struct ScatterParams {
    pub seed: u64,
    pub area: u8,
    pub plant: u16,
    /// Instances per set texel (fractional part resolved per texel by hash).
    pub density: f32,
}

/// Instances of one texel range `[col0, col1) x [row0, row1)` of channel `channel`.
/// Texel ownership makes any partition of the map into ranges produce exactly the same set.
pub fn scatter_range(
    mask: &DetailMask,
    channel: usize,
    extent: [f32; 2],
    cols: (u32, u32),
    rows: (u32, u32),
    p: &ScatterParams,
    spec: &PlantSpec,
    out: &mut Vec<PlantInstance>,
) {
    let tx = extent[0] / mask.width as f32;
    let tz = extent[1] / mask.height as f32;
    let key = hash64(p.seed ^ ((p.area as u64) << 56) ^ ((p.plant as u64) << 40));
    for row in rows.0..rows.1.min(mask.height) {
        for col in cols.0..cols.1.min(mask.width) {
            if mask.get(col, row, channel) == 0 {
                continue;
            }
            let h0 = hash64(key ^ ((row as u64) << 20 | col as u64));
            let n = (p.density + unit(h0)).floor() as u32;
            for k in 0..n {
                let h = hash64(h0 ^ (0xA5A5_0000 + k as u64));
                let h2 = hash64(h);
                let h3 = hash64(h2);
                let h4 = hash64(h3);
                out.push(PlantInstance {
                    x: (col as f32 + unit(h)) * tx,
                    z: (row as f32 + unit(h2)) * tz,
                    yaw: unit(h3) * std::f32::consts::TAU,
                    scale_h: lerp(spec.scale_h[0], spec.scale_h[1], unit(h4)),
                    scale_v: lerp(spec.scale_v[0], spec.scale_v[1], unit(hash64(h4))),
                    tint: unit(hash64(hash64(h4))),
                });
            }
        }
    }
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Instances of chunk `(cx, cz)` (chunks are `CHUNK_TEXELS` texels square, row-major from the origin).
pub fn chunk_instances(
    mask: &DetailMask,
    channel: usize,
    extent: [f32; 2],
    chunk: (u32, u32),
    p: &ScatterParams,
    spec: &PlantSpec,
) -> Vec<PlantInstance> {
    let mut out = Vec::new();
    scatter_range(
        mask,
        channel,
        extent,
        (chunk.0 * CHUNK_TEXELS, (chunk.0 + 1) * CHUNK_TEXELS),
        (chunk.1 * CHUNK_TEXELS, (chunk.1 + 1) * CHUNK_TEXELS),
        p,
        spec,
        &mut out,
    );
    out
}

/// Number of set texels of `channel` per chunk, `chunks_x * chunks_z` entries, row-major.
pub fn chunk_coverage(mask: &DetailMask, channel: usize) -> (u32, u32, Vec<u32>) {
    let cw = mask.width.div_ceil(CHUNK_TEXELS);
    let ch = mask.height.div_ceil(CHUNK_TEXELS);
    let mut counts = vec![0u32; (cw * ch) as usize];
    for row in 0..mask.height {
        for col in 0..mask.width {
            if mask.get(col, row, channel) != 0 {
                counts[((row / CHUNK_TEXELS) * cw + col / CHUNK_TEXELS) as usize] += 1;
            }
        }
    }
    (cw, ch, counts)
}

/// Pick a stand-in for a plant model that could not be exported (skinned/animated models are
/// blocked in `data/field_model_export_index.tsv`): the first available model of the same family
/// (grass / flower / shrub / detail / stones / gimmick) among `available`. ASSUMPTION: a different
/// model of the same family is a better approximation than nothing.
pub fn substitute_model(wanted: &str, available: &[&str]) -> Option<String> {
    const FAMILIES: [&str; 6] = ["grass", "flower", "shrub", "detail", "stones", "gimmick"];
    let fam = FAMILIES.iter().find(|f| wanted.contains(**f))?;
    available
        .iter()
        .find(|a| **a != wanted && a.contains(fam))
        .map(|s| (*s).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_mask(w: u32, h: u32, on: impl Fn(u32, u32) -> bool) -> DetailMask {
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for r in 0..h {
            for c in 0..w {
                if on(c, r) {
                    rgba[((r * w + c) * 4) as usize] = 255;
                }
            }
        }
        DetailMask { width: w, height: h, rgba }
    }

    fn spec() -> PlantSpec {
        PlantSpec {
            model: "plants_test".into(),
            scale_h: [0.9, 1.1],
            scale_v: [0.8, 1.2],
            param5: 0.1,
            tint_a: [1.0; 4],
            tint_b: [0.5, 1.0, 0.5, 1.0],
            field8: 4,
            field9: 38,
            extra: [0.0; 9],
        }
    }

    const EXT: [f32; 2] = [1025.0, 1025.0];

    fn params(density: f32) -> ScatterParams {
        ScatterParams { seed: 7, area: 0, plant: 2, density }
    }

    fn encode_mask(w: u32, h: u32, ch0: u8) -> Vec<u8> {
        let n = (w * h * 4) as usize;
        let mut v = vec![0u8; MASK_HEADER_LEN];
        v[0..4].copy_from_slice(&12u32.to_le_bytes());
        v[24..28].copy_from_slice(&(n as u32).to_le_bytes());
        v.extend((0..n).map(|i| if i % 4 == 0 { ch0 } else { 0 }));
        // trailer: vtable(16) + table [soffset 16, 1, w, h, 1, 1, n]
        v.extend([0x10, 0, 0x1c, 0, 4, 0, 8, 0, 0x0c, 0, 0x10, 0, 0x14, 0, 0x18, 0]);
        for x in [16u32, 1, w, h, 1, 1, n as u32] {
            v.extend(x.to_le_bytes());
        }
        v.truncate(MASK_HEADER_LEN + n + MASK_TRAILER_LEN);
        v
    }

    #[test]
    fn mask_parses_dimensions_and_payload() {
        let m = parse_detail_mask(&encode_mask(4, 2, 255)).unwrap();
        assert_eq!((m.width, m.height, m.rgba.len()), (4, 2, 32));
        assert_eq!(m.get(3, 1, 0), 255);
        assert_eq!(m.get(3, 1, 1), 0);
    }

    #[test]
    fn mask_rejects_inconsistent_sizes() {
        let mut bad = encode_mask(4, 2, 255);
        bad.pop();
        assert!(parse_detail_mask(&bad).is_err());
        assert!(parse_detail_mask(&[0u8; 10]).is_err());
    }

    #[test]
    fn scatter_is_deterministic() {
        let m = synth_mask(128, 128, |c, r| (c / 8 + r / 8) % 2 == 0);
        let a = chunk_instances(&m, 0, EXT, (1, 0), &params(1.5), &spec());
        let b = chunk_instances(&m, 0, EXT, (1, 0), &params(1.5), &spec());
        assert!(!a.is_empty());
        assert_eq!(a, b);
        let c = chunk_instances(&m, 0, EXT, (1, 0), &ScatterParams { seed: 8, ..params(1.5) }, &spec());
        assert_ne!(a, c);
    }

    #[test]
    fn chunks_partition_the_map_without_seams() {
        let m = synth_mask(256, 128, |c, r| (c * 7 + r * 3) % 5 < 3);
        let mut whole = Vec::new();
        scatter_range(&m, 0, EXT, (0, 256), (0, 128), &params(1.3), &spec(), &mut whole);
        let mut parts = Vec::new();
        for cz in 0..2 {
            for cx in 0..4 {
                parts.extend(chunk_instances(&m, 0, EXT, (cx, cz), &params(1.3), &spec()));
            }
        }
        let key = |i: &PlantInstance| (i.x.to_bits(), i.z.to_bits());
        let mut a: Vec<_> = whole.iter().map(key).collect();
        let mut b: Vec<_> = parts.iter().map(key).collect();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b, "chunked scatter must equal the whole-map scatter");
    }

    #[test]
    fn instances_stay_inside_their_chunk_and_texel() {
        let m = synth_mask(128, 128, |_, _| true);
        let t = EXT[0] / 128.0;
        for inst in chunk_instances(&m, 0, EXT, (1, 1), &params(2.0), &spec()) {
            assert!(inst.x >= 64.0 * t && inst.x < 128.0 * t, "x {}", inst.x);
            assert!(inst.z >= 64.0 * t && inst.z < 128.0 * t, "z {}", inst.z);
        }
    }

    #[test]
    fn density_matches_expectation_within_ten_percent() {
        // 100 chunks' worth of texels, fully set, density 1.7 per texel.
        let m = synth_mask(640, 640, |_, _| true);
        let mut v = Vec::new();
        scatter_range(&m, 0, EXT, (0, 640), (0, 640), &params(1.7), &spec(), &mut v);
        let expected = 1.7 * 640.0 * 640.0;
        let err = (v.len() as f32 - expected).abs() / expected;
        assert!(err < 0.10, "count {} vs {expected} (err {err})", v.len());
    }

    #[test]
    fn unset_texels_produce_nothing() {
        let m = synth_mask(64, 64, |_, _| false);
        assert!(chunk_instances(&m, 0, EXT, (0, 0), &params(3.0), &spec()).is_empty());
        let m2 = synth_mask(64, 64, |c, _| c < 32);
        for i in chunk_instances(&m2, 0, EXT, (0, 0), &params(3.0), &spec()) {
            assert!(i.x < 32.0 * EXT[0] / 64.0 + 1e-3);
        }
    }

    #[test]
    fn corners_map_to_world_extent_and_values_are_finite() {
        let m = synth_mask(2048, 2048, |c, r| (c == 0 && r == 0) || (c == 2047 && r == 2047));
        let mut v = Vec::new();
        scatter_range(&m, 0, EXT, (0, 2048), (0, 2048), &params(4.0), &spec(), &mut v);
        assert!(v.len() >= 4);
        let t = EXT[0] / 2048.0;
        for i in &v {
            assert!(i.x.is_finite() && i.z.is_finite() && i.yaw.is_finite());
            assert!((i.x < t && i.z < t) || (i.x > EXT[0] - t && i.z > EXT[1] - t));
            assert!((0.9..=1.1).contains(&i.scale_h) && (0.8..=1.2).contains(&i.scale_v));
            assert!((0.0..1.0).contains(&i.tint));
        }
    }

    #[test]
    fn other_plant_or_area_gives_other_positions() {
        let m = synth_mask(64, 64, |_, _| true);
        let a = chunk_instances(&m, 0, EXT, (0, 0), &params(1.0), &spec());
        let b = chunk_instances(&m, 0, EXT, (0, 0), &ScatterParams { plant: 3, ..params(1.0) }, &spec());
        let c = chunk_instances(&m, 0, EXT, (0, 0), &ScatterParams { area: 1, ..params(1.0) }, &spec());
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn chunk_coverage_counts_set_texels() {
        let m = synth_mask(128, 64, |c, r| c < 10 && r < 10);
        let (cw, ch, counts) = chunk_coverage(&m, 0);
        assert_eq!((cw, ch), (2, 1));
        assert_eq!(counts, vec![100, 0]);
    }

    #[test]
    fn density_rule_and_quality() {
        let t = 0.5005;
        assert_eq!(density_per_texel(2, t, GrassQuality::Off), 0.0);
        let low = density_per_texel(2, t, GrassQuality::Low);
        let high = density_per_texel(2, t, GrassQuality::High);
        assert!(low >= 0.5 && high > low && high <= 4.0, "{low} {high}");
        // heavy model: sparse, never zero
        let shrub = density_per_texel(276, t, GrassQuality::Low);
        assert!((0.02..=1.0).contains(&shrub));
        assert!(radius_for(2, GrassQuality::Low) < radius_for(276, GrassQuality::Low));
        assert_eq!(GrassQuality::from_args(&["--grass".into(), "high".into()]), GrassQuality::High);
        assert_eq!(GrassQuality::from_args(&[]), GrassQuality::Low);
    }

    #[test]
    fn plant_to_mask_slot_is_four_channels_per_map() {
        assert_eq!(PlantSpec::mask_slot(0), (0, 0));
        assert_eq!(PlantSpec::mask_slot(5), (1, 1));
        assert_eq!(PlantSpec::mask_slot(19), (4, 3));
    }

    #[test]
    fn substitute_picks_same_family() {
        let avail = ["plants_area00_grass01_alb", "plants_flower04", "plants_shrub02"];
        assert_eq!(substitute_model("plants_area00_grass02_alb", &avail).as_deref(), Some("plants_area00_grass01_alb"));
        assert_eq!(substitute_model("plants_shrub03", &avail).as_deref(), Some("plants_shrub02"));
        assert_eq!(substitute_model("plants_tree01", &avail), None);
    }

    /// Real data: area 0 descriptor and masks (skipped when the game files are absent).
    #[test]
    fn real_area0_descriptor_and_masks() {
        let root = std::env::var_os("PLA_BIN_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into());
        if !root.join("archive/field/terrain/ha_area00.gfpak").exists() {
            eprintln!("[skip] game data not found at {}", root.display());
            return;
        }
        let a = load_vegetation_area(&root, 0).unwrap();
        assert_eq!(a.descriptor.plants.len(), 11);
        assert_eq!(a.masks.len(), 3);
        assert_eq!(a.descriptor.plants.len().div_ceil(4), a.masks.len());
        assert_eq!(a.descriptor.plants[0].model, "plants_area00_grass01_alb");
        assert_eq!((a.masks[0].width, a.masks[0].height), (2048, 2048));
        assert!((a.descriptor.extent[0] - 1025.0).abs() < 1e-3 && (a.descriptor.height - 185.0).abs() < 1e-3);
        assert!(a.ground_y(512.0, 512.0) > 10.0);
    }
}
