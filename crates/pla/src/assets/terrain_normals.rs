//! Baked terrain tangent-space normal map.
//!
//! Mirrors [`super::terrain_albedo`]: the same eight normalised splat weights and
//! the same `SplatST / DETAIL_TILE_DIVISOR` detail tiling blend the material's
//! `SplatNormal0..7` layers (BC5) into one tangent-space normal map per area
//! (`work/cache/terrain_normals/areaNN.png`, RGB8, 2048²) plus an invalidation
//! sidecar with the same skip-if-unchanged semantics as the albedo bake.
//!
//! **Assumptions (not decoded from the game):**
//! * the tangent-space Z is rebuilt as `z = sqrt(max(0, 1 - x² - y²))`;
//! * layer normals are blended as a weighted sum of unit normals, then
//!   renormalised (no reoriented/RNM blending);
//! * the tangent-space Y axis points up (+Y, the OpenGL convention);
//! * the per-layer tiling is the layer's `SplatST` (as for the albedo layer of
//!   the same slot).
//!
//! The terrain's earlier `areaNN_0_0_normalmap` BNTX is **not used**: its format
//! (`0x0e01`, tentatively decoded as R16Unorm) is unverified.

use std::path::{Path, PathBuf};

use super::bntx::TextureData;
use super::terrain::TerrainArea;
use super::terrain_albedo::{texture_hash, texture_name_of, DETAIL_TILE_DIVISOR};

pub use super::terrain_albedo::BakeOutcome;

/// Default side of the baked area normal map (the CLI default).
pub const DEFAULT_BAKE_SIZE: u32 = 2048;

/// Version of the sidecar layout; a different version forces a re-bake.
const SIDECAR_VERSION: u64 = 1;

/// How the tangent-space Z is rebuilt from the two BC5 channels. **Assumed.**
pub const Z_RECONSTRUCTION: &str = "sqrt(max(0, 1 - x^2 - y^2))";

/// Options of one normal bake.
#[derive(Debug, Clone)]
pub struct NormalBakeOptions {
    /// Output side in pixels (square).
    pub size: u32,
    /// Tilt the per-layer normals (`true`) or use each layer's mean normal.
    pub detail: bool,
    /// Worker threads (0 = up to 3 / available).
    pub threads: usize,
}

impl Default for NormalBakeOptions {
    fn default() -> Self {
        Self { size: DEFAULT_BAKE_SIZE, detail: true, threads: 0 }
    }
}

/// One `SplatNormal<k>` layer with its decoded tangent-space (x, y) samples.
pub struct ResolvedNormalLayer {
    /// Material slot index `k` (0..8); selects the splat weight.
    pub slot: usize,
    /// Texture name without the `.bntx` suffix.
    pub texture_name: String,
    pub width: u32,
    pub height: u32,
    /// Decoded tangent-space (x, y) in `[-1, 1]`, row-major, `width * height` long.
    pub xy: Vec<[f32; 2]>,
    /// `SplatST<k>` x scale: `tiling` repeats per [`DETAIL_TILE_DIVISOR`] metres.
    pub tiling: f32,
    /// Content hash of the source texture (sidecar invalidation).
    pub hash: String,
}

/// Self-contained inputs of a normal bake (weight map + resolved layers), so the
/// blend can be exercised without reading a game pack.
pub struct NormalBakeInputs<'a> {
    pub area_number: u8,
    /// World extent along x/z in metres.
    pub extent: f32,
    pub splat_alpha: &'a TextureData,
    pub splat_alpha1: Option<&'a TextureData>,
    pub layers: Vec<ResolvedNormalLayer>,
}

impl<'a> NormalBakeInputs<'a> {
    /// Resolve every `SplatNormal<k>` layer of `area` from its terrain pack.
    pub fn from_area(area: &'a TerrainArea) -> Result<Self, String> {
        let layers = resolve_normal_layers(area)?;
        Ok(Self {
            area_number: area.area,
            extent: area.world_extent[0],
            splat_alpha: &area.splat_alpha,
            splat_alpha1: area.splat_alpha1.as_ref(),
            layers,
        })
    }
}

/// Decode the `SplatNormal0..7` slots of `area` (BC5 R/G) into tangent-space xy.
pub fn resolve_normal_layers(area: &TerrainArea) -> Result<Vec<ResolvedNormalLayer>, String> {
    let material = area.material.as_ref().ok_or("terrain material missing")?;
    let mut out = Vec::new();
    for (file, role, tex) in area.load_layer_normals()? {
        let Some(k) = role.strip_prefix("SplatNormal").and_then(|d| d.parse::<usize>().ok()) else {
            continue;
        };
        if k >= 8 {
            continue;
        }
        if tex.width == 0
            || tex.height == 0
            || tex.rgba.len() != (tex.width as usize * tex.height as usize * 4)
        {
            return Err(format!("normal layer {file} has invalid RGBA storage"));
        }
        // BC5 decodes to R,G with B=0, A=255 (see bntx::decode): the two channels
        // are the tangent-space x and y in [0,255] -> [-1,1].
        let xy = tex
            .rgba
            .chunks_exact(4)
            .map(|p| [p[0] as f32 / 127.5 - 1.0, p[1] as f32 / 127.5 - 1.0])
            .collect();
        let tiling = material.splat_tiling.get(k).map(|s| s[0]).unwrap_or(1.0);
        out.push(ResolvedNormalLayer {
            slot: k,
            texture_name: texture_name_of(&file),
            width: tex.width,
            height: tex.height,
            xy,
            tiling: if tiling.is_finite() && tiling > 0.0 { tiling } else { 1.0 },
            hash: texture_hash(&tex),
        });
    }
    out.sort_by_key(|layer| layer.slot);
    Ok(out)
}

/// Rebuild the tangent-space Z from x and y and return a unit normal.
pub fn unit_normal(x: f32, y: f32) -> [f32; 3] {
    let z = (1.0 - (x * x + y * y)).max(0.0).sqrt();
    normalize3([x, y, z])
}

/// Decode one BC5 (R, G) pair into a unit tangent-space normal.
pub fn decode_tangent_normal(rg: [u8; 2]) -> [f32; 3] {
    unit_normal(rg[0] as f32 / 127.5 - 1.0, rg[1] as f32 / 127.5 - 1.0)
}

/// Encode a normal into RGB8 (`[-1,1] -> [0,255]`); alpha is not part of it.
pub fn encode_tangent_normal(n: [f32; 3]) -> [u8; 3] {
    let enc = |v: f32| ((v.clamp(-1.0, 1.0) * 0.5 + 0.5) * 255.0 + 0.5) as u8;
    [enc(n[0]), enc(n[1]), enc(n[2])]
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-6 {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

/// Mip chain of the encoded (x, y) channels, sampled with wrap-around addressing.
struct NormalChain {
    levels: Vec<(usize, usize, Vec<u16>)>,
    /// Mean unit normal (used when `detail = false`).
    mean: [f32; 3],
}

fn enc_xy(v: f32) -> u16 {
    (((v.clamp(-1.0, 1.0) * 0.5) + 0.5) * 65535.0 + 0.5) as u16
}

fn dec_xy(v: u16) -> f32 {
    v as f32 / 32767.5 - 1.0
}

impl NormalChain {
    fn new(layer: &ResolvedNormalLayer) -> Self {
        let (w, h) = (layer.width as usize, layer.height as usize);
        let mut base = Vec::with_capacity(w * h * 2);
        let mut sum = [0.0f64; 3];
        for xy in &layer.xy {
            let n = unit_normal(xy[0], xy[1]);
            for c in 0..3 {
                sum[c] += n[c] as f64;
            }
            base.push(enc_xy(xy[0]));
            base.push(enc_xy(xy[1]));
        }
        let n = (w * h).max(1) as f64;
        let mean = normalize3([(sum[0] / n) as f32, (sum[1] / n) as f32, (sum[2] / n) as f32]);
        let mut levels = vec![(w, h, base)];
        while levels.last().is_some_and(|l| l.0 > 1 || l.1 > 1) {
            let (pw, ph, prev) = levels.last().unwrap();
            let (nw, nh) = ((*pw / 2).max(1), (*ph / 2).max(1));
            let mut next = vec![0u16; nw * nh * 2];
            for y in 0..nh {
                let (y0, y1) = ((2 * y).min(ph - 1), (2 * y + 1).min(ph - 1));
                for x in 0..nw {
                    let (x0, x1) = ((2 * x).min(pw - 1), (2 * x + 1).min(pw - 1));
                    for c in 0..2 {
                        let s = prev[(y0 * pw + x0) * 2 + c] as u32
                            + prev[(y0 * pw + x1) * 2 + c] as u32
                            + prev[(y1 * pw + x0) * 2 + c] as u32
                            + prev[(y1 * pw + x1) * 2 + c] as u32;
                        next[(y * nw + x) * 2 + c] = ((s + 2) / 4) as u16;
                    }
                }
            }
            levels.push((nw, nh, next));
        }
        Self { levels, mean }
    }

    /// Bilinear sample of (x, y) with wrap-around addressing; (u, v) in tile fractions.
    fn bilinear(&self, level: usize, u: f32, v: f32) -> [f32; 2] {
        let (w, h, d) = &self.levels[level];
        let (fx, fy) = (u * *w as f32 - 0.5, v * *h as f32 - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (xi0, yi0) = (
            (x0 as i64).rem_euclid(*w as i64) as usize,
            (y0 as i64).rem_euclid(*h as i64) as usize,
        );
        let (xi1, yi1) = ((xi0 + 1) % w, (yi0 + 1) % h);
        let mut out = [0.0f32; 2];
        for (c, o) in out.iter_mut().enumerate() {
            let t = |x: usize, y: usize| dec_xy(d[(y * w + x) * 2 + c]);
            let a = t(xi0, yi0) * (1.0 - tx) + t(xi1, yi0) * tx;
            let b = t(xi0, yi1) * (1.0 - tx) + t(xi1, yi1) * tx;
            *o = a * (1.0 - ty) + b * ty;
        }
        out
    }

    /// Trilinear sample (mip blend by `lod`) with wrap-around addressing.
    fn sample(&self, u: f32, v: f32, lod: f32) -> [f32; 2] {
        let max = (self.levels.len() - 1) as f32;
        let lod = lod.clamp(0.0, max);
        let l0 = lod.floor() as usize;
        let t = lod - l0 as f32;
        let a = self.bilinear(l0, u, v);
        if t <= 1e-3 || l0 + 1 >= self.levels.len() {
            return a;
        }
        let b = self.bilinear(l0 + 1, u, v);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
    }
}

/// Per-texel weights of layers 0..8 normalised to sum 1 (same rule as the albedo).
fn input_weights(inputs: &NormalBakeInputs) -> Result<Vec<[f32; 8]>, String> {
    let s0 = inputs.splat_alpha;
    let n = (s0.width as usize) * (s0.height as usize);
    if s0.width == 0 || s0.height == 0 || s0.rgba.len() != n * 4 {
        return Err("splat RGBA storage does not match its dimensions".into());
    }
    if let Some(s1) = inputs.splat_alpha1 {
        if s1.width != s0.width || s1.height != s0.height || s1.rgba.len() != n * 4 {
            return Err("splat layer 1 does not match layer 0".into());
        }
    }
    let mut out = vec![[0.0f32; 8]; n];
    for (i, w) in out.iter_mut().enumerate() {
        for c in 0..4 {
            w[c] = s0.rgba[i * 4 + c] as f32;
            if let Some(s1) = inputs.splat_alpha1 {
                w[4 + c] = s1.rgba[i * 4 + c] as f32;
            }
        }
        let sum: f32 = w.iter().sum();
        if sum > 0.0 {
            w.iter_mut().for_each(|v| *v /= sum);
        } else {
            w[0] = 1.0;
        }
    }
    Ok(out)
}

/// Compose the tangent-space normal map at `opts.size`² from `inputs`.
fn compose_normals(inputs: &NormalBakeInputs, opts: &NormalBakeOptions) -> Result<TextureData, String> {
    if opts.size == 0 {
        return Err("bake size must be > 0".into());
    }
    if inputs.layers.is_empty() {
        return Err("terrain has no SplatNormal layers".into());
    }
    if !(inputs.extent.is_finite() && inputs.extent > 0.0) {
        return Err("terrain extent must be finite and positive".into());
    }
    let w = input_weights(inputs)?;
    let (sw, sh) = (inputs.splat_alpha.width as usize, inputs.splat_alpha.height as usize);
    let chains: Vec<(usize, f32, NormalChain)> = inputs
        .layers
        .iter()
        .map(|layer| (layer.slot, layer.tiling, NormalChain::new(layer)))
        .collect();
    let extent = inputs.extent;
    let size = opts.size as usize;
    let mut rgba = vec![0u8; size * size * 4];
    let threads = if opts.threads > 0 {
        opts.threads
    } else {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(3)
    };
    let rows_per = size.div_ceil(threads.max(1));
    let (w, chains) = (&w, &chains);
    std::thread::scope(|scope| {
        for (band, chunk) in rgba.chunks_mut(rows_per * size * 4).enumerate() {
            scope.spawn(move || {
                for (ry, row) in chunk.chunks_mut(size * 4).enumerate() {
                    let y = band * rows_per + ry;
                    let fy =
                        ((y as f32 + 0.5) * sh as f32 / size as f32 - 0.5).clamp(0.0, sh as f32 - 1.0);
                    let (y0, ty) = (fy.floor() as usize, fy.fract());
                    let y1 = (y0 + 1).min(sh - 1);
                    let wz = (y as f32 + 0.5) * extent / size as f32;
                    for x in 0..size {
                        let fx = ((x as f32 + 0.5) * sw as f32 / size as f32 - 0.5)
                            .clamp(0.0, sw as f32 - 1.0);
                        let (x0, tx) = (fx.floor() as usize, fx.fract());
                        let x1 = (x0 + 1).min(sw - 1);
                        let wx = (x as f32 + 0.5) * extent / size as f32;
                        let mut acc = [0.0f32; 3];
                        let mut total = 0.0f32;
                        for (slot, tiling, chain) in chains.iter() {
                            let a = w[y0 * sw + x0][*slot] * (1.0 - tx) + w[y0 * sw + x1][*slot] * tx;
                            let b = w[y1 * sw + x0][*slot] * (1.0 - tx) + w[y1 * sw + x1][*slot] * tx;
                            let wk = a * (1.0 - ty) + b * ty;
                            if wk <= 0.0 {
                                continue;
                            }
                            let n = if opts.detail {
                                let rep = tiling / DETAIL_TILE_DIVISOR;
                                let (bw, bh, _) = &chain.levels[0];
                                let mpp = extent / size as f32;
                                let fp = ((*bw as f32 * rep * mpp) * (*bh as f32 * rep * mpp)).sqrt();
                                let xy = chain.sample(
                                    (wx * rep).rem_euclid(1.0),
                                    (wz * rep).rem_euclid(1.0),
                                    fp.max(1.0).log2(),
                                );
                                unit_normal(xy[0], xy[1])
                            } else {
                                chain.mean
                            };
                            for c in 0..3 {
                                acc[c] += n[c] * wk;
                            }
                            total += wk;
                        }
                        let n = if total > 0.0 { normalize3(acc) } else { [0.0, 0.0, 1.0] };
                        let encoded = encode_tangent_normal(n);
                        row[x * 4] = encoded[0];
                        row[x * 4 + 1] = encoded[1];
                        row[x * 4 + 2] = encoded[2];
                        row[x * 4 + 3] = 255;
                    }
                }
            });
        }
    });
    Ok(TextureData { width: opts.size, height: opts.size, rgba })
}

fn signature(inputs: &NormalBakeInputs, opts: &NormalBakeOptions) -> serde_json::Value {
    let splat1 = inputs.splat_alpha1.map(texture_hash).unwrap_or_default();
    serde_json::json!({
        "kind": "normals",
        "version": SIDECAR_VERSION,
        "area": inputs.area_number,
        "size": opts.size,
        "detail": opts.detail,
        "tile_divisor": DETAIL_TILE_DIVISOR,
        "extent": inputs.extent,
        "z_reconstruction": Z_RECONSTRUCTION,
        "splat_hash": format!("{}+{}", texture_hash(inputs.splat_alpha), splat1),
        "layers": inputs.layers.iter().map(|l| serde_json::json!({
            "slot": l.slot,
            "texture": l.texture_name,
            "width": l.width,
            "height": l.height,
            "hash": l.hash,
            "tiling": l.tiling,
        })).collect::<Vec<_>>(),
    })
}

/// Statistics of a baked normal map, read back by reconstructing each normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalStats {
    /// Mean tangent-space normal.
    pub mean: [f32; 3],
    pub mean_z: f32,
    pub min_z: f32,
    pub max_z: f32,
    /// Percentage of pixels whose reconstructed z is below 0.99 (visibly tilted).
    pub tilted_pct: f32,
}

/// Reconstruct and summarise every pixel of a baked normal map.
pub fn normal_stats(image: &TextureData) -> NormalStats {
    let count = (image.width as usize) * (image.height as usize);
    let mut sum = [0.0f64; 3];
    let (mut min_z, mut max_z) = (f32::INFINITY, f32::NEG_INFINITY);
    let mut tilted = 0usize;
    for p in image.rgba.chunks_exact(4) {
        let n = decode_tangent_normal([p[0], p[1]]);
        for c in 0..3 {
            sum[c] += n[c] as f64;
        }
        min_z = min_z.min(n[2]);
        max_z = max_z.max(n[2]);
        if n[2] < 0.99 {
            tilted += 1;
        }
    }
    let c = count.max(1) as f64;
    NormalStats {
        mean: [(sum[0] / c) as f32, (sum[1] / c) as f32, (sum[2] / c) as f32],
        mean_z: (sum[2] / c) as f32,
        min_z: if min_z.is_finite() { min_z } else { 1.0 },
        max_z: if max_z.is_finite() { max_z } else { 1.0 },
        tilted_pct: tilted as f32 / count.max(1) as f32 * 100.0,
    }
}

/// Result of a bake: the image, the sidecar signature and read-back statistics.
pub struct NormalBakeResult {
    pub image: TextureData,
    pub signature: serde_json::Value,
    pub stats: NormalStats,
}

/// Signature a normal bake of `area` with `opts` would have, without baking.
pub fn normal_bake_signature(area: &TerrainArea, opts: &NormalBakeOptions) -> Result<serde_json::Value, String> {
    let inputs = NormalBakeInputs::from_area(area)?;
    Ok(signature(&inputs, opts))
}

/// Compose the area's tangent-space normal map with the original layers.
pub fn bake_normals(area: &TerrainArea, out_size: u32) -> Result<TextureData, String> {
    Ok(bake_normals_with(area, &NormalBakeOptions { size: out_size, detail: true, threads: 0 })?.image)
}

/// Compose the area's tangent-space normal map, returning image, signature and stats.
pub fn bake_normals_with(area: &TerrainArea, opts: &NormalBakeOptions) -> Result<NormalBakeResult, String> {
    let inputs = NormalBakeInputs::from_area(area)?;
    let image = compose_normals(&inputs, opts)?;
    let stats = normal_stats(&image);
    Ok(NormalBakeResult { image, signature: signature(&inputs, opts), stats })
}

/// Sidecar path next to the baked PNG: `areaNN.normals.json`.
pub fn normal_sidecar_path(work_root: impl AsRef<Path>, area: u8) -> PathBuf {
    work_root.as_ref().join("cache/terrain_normals").join(format!("area{area:02}.normals.json"))
}

/// Cache location of the baked normal map for `area` under the repo work dir.
pub fn normals_cache_path(work_root: impl AsRef<Path>, area: u8) -> PathBuf {
    work_root.as_ref().join("cache/terrain_normals").join(format!("area{area:02}.png"))
}

/// Bake already-resolved `inputs` into `work_root/cache/terrain_normals/areaNN.png` plus its
/// sidecar, skipping the bake when the PNG exists and the stored signature equals the current one.
pub fn bake_inputs_to_cache(
    inputs: &NormalBakeInputs,
    work_root: &Path,
    opts: &NormalBakeOptions,
    skip_if_unchanged: bool,
) -> Result<BakeOutcome, String> {
    let png_path = normals_cache_path(work_root, inputs.area_number);
    let side_path = normal_sidecar_path(work_root, inputs.area_number);
    let sig = signature(inputs, opts);
    if skip_if_unchanged && png_path.is_file() {
        if let Ok(text) = std::fs::read_to_string(&side_path) {
            if let Ok(old) = serde_json::from_str::<serde_json::Value>(&text) {
                if old == sig {
                    return Ok(BakeOutcome::Skipped);
                }
            }
        }
    }
    let image = compose_normals(inputs, opts)?;
    save_png_rgb(&image, &png_path).map_err(|e| format!("write {}: {e}", png_path.display()))?;
    std::fs::write(&side_path, serde_json::to_string_pretty(&sig).unwrap() + "\n")
        .map_err(|e| format!("write {}: {e}", side_path.display()))?;
    Ok(BakeOutcome::Baked)
}

/// Bake `area` into the normal-map cache (convenience over [`bake_inputs_to_cache`]).
pub fn bake_normals_to_cache(
    area: &TerrainArea,
    work_root: &Path,
    opts: &NormalBakeOptions,
    skip_if_unchanged: bool,
) -> Result<BakeOutcome, String> {
    let inputs = NormalBakeInputs::from_area(area)?;
    bake_inputs_to_cache(&inputs, work_root, opts, skip_if_unchanged)
}

/// Write a baked normal map as a 3-channel PNG (alpha is dropped).
pub fn save_png_rgb(image: &TextureData, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let rgb: Vec<u8> = image.rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&rgb)?;
    Ok(())
}

/// Load a baked `areaNN.png` (RGB8 or RGBA8, alpha filled with 255).
pub fn load_baked_normals(path: impl AsRef<Path>) -> Result<TextureData, String> {
    let file = std::fs::File::open(path.as_ref())
        .map_err(|e| format!("open {}: {e}", path.as_ref().display()))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| format!("{}: {e}", path.as_ref().display()))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("png too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("{}: {e}", path.as_ref().display()))?;
    let d = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => d.to_vec(),
        png::ColorType::Rgb => d.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::Grayscale => d.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        other => {
            return Err(format!("{}: unsupported colour type {other:?}", path.as_ref().display()))
        }
    };
    if info.width == 0 || info.height == 0 {
        return Err(format!("{}: empty image", path.as_ref().display()));
    }
    Ok(TextureData { width: info.width, height: info.height, rgba })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn layer(slot: usize, w: u32, h: u32, xy: Vec<[f32; 2]>, tiling: f32) -> ResolvedNormalLayer {
        let rgba: Vec<u8> = xy
            .iter()
            .flat_map(|c| {
                [
                    ((c[0].clamp(-1.0, 1.0) * 0.5 + 0.5) * 255.0 + 0.5) as u8,
                    ((c[1].clamp(-1.0, 1.0) * 0.5 + 0.5) * 255.0 + 0.5) as u8,
                    0,
                    255,
                ]
            })
            .collect();
        let tex = TextureData { width: w, height: h, rgba };
        ResolvedNormalLayer {
            slot,
            texture_name: format!("test_nrm{slot}"),
            width: w,
            height: h,
            xy,
            tiling,
            hash: texture_hash(&tex),
        }
    }

    fn flat_layer(slot: usize, w: u32, h: u32, xy: [f32; 2], tiling: f32) -> ResolvedNormalLayer {
        layer(slot, w, h, vec![xy; (w * h) as usize], tiling)
    }

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pla_normals_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn encode_and_decode_round_trip_to_unit_normals() {
        assert_eq!(encode_tangent_normal([0.0, 0.0, 1.0]), [128, 128, 255]);
        for (r, g) in [(191u8, 128u8), (64, 128), (128, 200), (200, 200), (0, 0), (255, 255)] {
            let n = decode_tangent_normal([r, g]);
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-4, "|n|={len} for ({r},{g})");
            assert!(n[2] >= 0.0);
        }
    }

    #[test]
    fn flat_layer_bakes_to_flat_normal_map() {
        let splat = TextureData { width: 2, height: 2, rgba: [255, 0, 0, 0].repeat(4) };
        let inputs = NormalBakeInputs {
            area_number: 3,
            extent: 100.0,
            splat_alpha: &splat,
            splat_alpha1: None,
            layers: vec![flat_layer(0, 8, 8, [0.0, 0.0], 32.0)],
        };
        let img = compose_normals(&inputs, &NormalBakeOptions { size: 16, detail: true, threads: 2 })
            .unwrap();
        for p in img.rgba.chunks_exact(4) {
            assert_eq!(p[3], 255);
            assert!((p[0] as i32 - 128).abs() <= 1 && (p[1] as i32 - 128).abs() <= 1 && p[2] >= 254);
        }
        assert!(normal_stats(&img).mean_z > 0.99);
    }

    #[test]
    fn blended_opposite_normals_cancel_to_flat_and_stay_unit_length() {
        let splat = TextureData { width: 1, height: 1, rgba: vec![128, 128, 0, 0] };
        let inputs = NormalBakeInputs {
            area_number: 3,
            extent: 100.0,
            splat_alpha: &splat,
            splat_alpha1: None,
            layers: vec![
                flat_layer(0, 4, 4, [0.5, 0.0], 32.0),
                flat_layer(1, 4, 4, [-0.5, 0.0], 32.0),
            ],
        };
        let img = compose_normals(&inputs, &NormalBakeOptions { size: 8, detail: true, threads: 2 })
            .unwrap();
        for p in img.rgba.chunks_exact(4) {
            let n = decode_tangent_normal([p[0], p[1]]);
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 0.02, "|n|={len}");
        }
        let px = &img.rgba[(4 * 8 + 4) * 4..(4 * 8 + 4) * 4 + 3];
        assert!(
            (px[0] as i32 - 128).abs() <= 2 && (px[1] as i32 - 128).abs() <= 2 && px[2] >= 254,
            "expected a flat normal, got {px:?}"
        );
    }

    #[test]
    fn tiling_wraps_without_a_seam() {
        let mut xy = vec![[0.0f32; 2]; 64];
        for y in 0..8 {
            for x in 0..8 {
                xy[y * 8 + x] = [x as f32 / 7.0 * 2.0 - 1.0, 0.0];
            }
        }
        let chain = NormalChain::new(&layer(0, 8, 8, xy, 32.0));
        let eps = 1e-4;
        let right = chain.sample(1.0 - eps, 0.3, 0.0);
        let left = chain.sample(eps, 0.3, 0.0);
        assert!((right[0] - left[0]).abs() < 1e-2, "seam in u: {right:?} vs {left:?}");
        let step = (chain.sample(0.625, 0.3, 0.0)[0] - chain.sample(0.5, 0.3, 0.0)[0]).abs();
        assert!(step > 0.01);
        let top = chain.sample(0.4, 1.0 - eps, 0.0);
        let bottom = chain.sample(0.4, eps, 0.0);
        assert!((top[0] - bottom[0]).abs() < 1e-2);
    }

    #[test]
    fn sidecar_detects_changes_and_allows_skipping() {
        let splat = TextureData { width: 2, height: 2, rgba: [255, 0, 0, 0].repeat(4) };
        let work = tmpdir("sidecar");
        let opts = NormalBakeOptions { size: 8, detail: true, threads: 2 };
        let mut inputs = NormalBakeInputs {
            area_number: 3,
            extent: 100.0,
            splat_alpha: &splat,
            splat_alpha1: None,
            layers: vec![flat_layer(0, 4, 4, [0.0, 0.0], 32.0)],
        };
        assert_eq!(bake_inputs_to_cache(&inputs, &work, &opts, true).unwrap(), BakeOutcome::Baked);
        assert!(normal_sidecar_path(&work, 3).is_file());
        assert_eq!(bake_inputs_to_cache(&inputs, &work, &opts, true).unwrap(), BakeOutcome::Skipped);
        // changed layer content (different hash) -> re-bake, then skip again
        inputs.layers[0] = flat_layer(0, 4, 4, [0.3, 0.0], 32.0);
        assert_eq!(bake_inputs_to_cache(&inputs, &work, &opts, true).unwrap(), BakeOutcome::Baked);
        assert_eq!(bake_inputs_to_cache(&inputs, &work, &opts, true).unwrap(), BakeOutcome::Skipped);
        // different size -> re-bake
        let bigger = NormalBakeOptions { size: 16, detail: true, threads: 2 };
        assert_eq!(bake_inputs_to_cache(&inputs, &work, &bigger, true).unwrap(), BakeOutcome::Baked);
        let side: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(normal_sidecar_path(&work, 3)).unwrap())
                .unwrap();
        assert_eq!(side["kind"], "normals");
        assert_eq!(side["layers"][0]["hash"].as_str().unwrap().len(), 16);
        let _ = std::fs::remove_dir_all(work);
    }

    #[test]
    fn game_area_normals_bake_skips_when_game_pack_is_absent() {
        let bin_root = std::env::var_os("PLA_BIN_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
            });
        let pack = bin_root.join("archive/field/terrain/ha_area00.gfpak");
        if !pack.is_file() {
            eprintln!("[skip] terrain pack unavailable under {}", bin_root.display());
            return;
        }
        for area_number in [0u8, 1, 5] {
            let area = TerrainArea::load(&bin_root, area_number)
                .unwrap_or_else(|error| panic!("load area {area_number:02}: {error}"));
            let started = std::time::Instant::now();
            let result = bake_normals_with(
                &area,
                &NormalBakeOptions { size: 512, detail: true, threads: 2 },
            )
            .unwrap_or_else(|error| panic!("bake area {area_number:02}: {error}"));
            let elapsed = started.elapsed().as_secs_f32();
            for p in result.image.rgba.chunks_exact(4) {
                let n = decode_tangent_normal([p[0], p[1]]);
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                assert!((len - 1.0).abs() < 0.02, "area {area_number:02}: |n|={len}");
            }
            let s = result.stats;
            eprintln!(
                "area{area_number:02}: 512px in {elapsed:.2}s mean_z={:.4} min_z={:.4} max_z={:.4} tilted={:.1}%",
                s.mean_z, s.min_z, s.max_z, s.tilted_pct
            );
            assert!(s.mean_z > 0.5, "area {area_number:02} mean_z {}", s.mean_z);
        }
    }
}
