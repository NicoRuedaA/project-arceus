//! Baked terrain albedo: the ground colour of one area composed from the
//! terrain material's eight splat layers.
//!
//! Evidence (see `reports/data-progress/terrain_albedo.md`): the splat control
//! BNTX is a BC7 *array of two* 1024x1024 layers (`ControlTextureNum = Two`);
//! its 8 channels (layer 0 R,G,B,A then layer 1 R,G,B,A) are the weights of
//! material slots `SplatColor0..7`, and the 8 weights sum to ~255 per texel.
//! Verified on areas 00, 01, 05. Unproven: the world-space tiling period of the
//! ground textures (`SplatST` is only a relative scale) and the exact in-game
//! blend (height/threshold terms are ignored).

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use super::bntx::TextureData;
use super::terrain::TerrainArea;

/// World-space divisor applied to `SplatST` for the detail tiling period:
/// `repeats_per_metre = ST / DETAIL_TILE_DIVISOR`. **Assumption**, not decoded.
pub const DETAIL_TILE_DIVISOR: f32 = 32.0;

/// Default side of the baked area albedo (the CLI default; 2048 stays available).
pub const DEFAULT_BAKE_SIZE: u32 = 4096;

/// Sub-directory (under `work/texturas_mejoradas`) holding improved layer textures.
pub const OVERRIDE_SUBDIR: &str = "texturas_mejoradas/terrain_layers";

/// Version of the sidecar layout; a different version forces a re-bake.
const SIDECAR_VERSION: u64 = 1;

/// Options of one bake.
#[derive(Debug, Clone)]
pub struct BakeOptions {
    /// Output side in pixels (square).
    pub size: u32,
    /// Tile the ground textures (`true`) or use each layer's mean colour.
    pub detail: bool,
    /// Directory with improved layer PNGs named `<unique_name>.png`.
    pub override_dir: Option<PathBuf>,
    /// Worker threads (0 = up to 3 / available).
    pub threads: usize,
}

impl Default for BakeOptions {
    fn default() -> Self {
        Self { size: DEFAULT_BAKE_SIZE, detail: true, override_dir: None, threads: 0 }
    }
}

fn srgb_lut() -> [f32; 256] {
    let mut t = [0.0f32; 256];
    for (i, v) in t.iter_mut().enumerate() {
        let c = i as f32 / 255.0;
        *v = if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    }
    t
}

fn linear_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let v = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (v * 255.0 + 0.5) as u8
}

/// FNV-1a 64-bit (stable across Rust versions, unlike `DefaultHasher`).
pub fn fnv1a64(chunks: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for c in chunks {
        for &b in *c {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// Content hash of a decoded texture (size + RGBA bytes), 16 hex digits.
pub fn texture_hash(tex: &TextureData) -> String {
    format!("{:016x}", fnv1a64(&[&tex.width.to_le_bytes(), &tex.height.to_le_bytes(), &tex.rgba]))
}

/// `ha_area01_grass01_alb.bntx` -> `ha_area01_grass01_alb`.
pub fn texture_name_of(file: &str) -> String {
    file.strip_suffix(".bntx").or_else(|| file.strip_suffix(".BNTX")).unwrap_or(file).to_owned()
}

/// Per-texel weights of layers 0..8 normalised to sum 1, at splat resolution.
fn weights(area: &TerrainArea) -> Result<Vec<[f32; 8]>, String> {
    let s0 = &area.splat_alpha;
    let n = (s0.width * s0.height) as usize;
    let mut out = vec![[0.0f32; 8]; n];
    for (i, w) in out.iter_mut().enumerate() {
        for c in 0..4 {
            w[c] = s0.rgba[i * 4 + c] as f32;
            if let Some(s1) = &area.splat_alpha1 {
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

/// Normalised per-texel layer weights of an area (sum 1 per texel), with the splat size.
pub fn layer_weights(area: &TerrainArea) -> Result<(Vec<[f32; 8]>, u32, u32), String> {
    Ok((weights(area)?, area.splat_alpha.width, area.splat_alpha.height))
}

/// Path of the improved PNG for a layer: `<name>__aNN.png` (area specific) wins over `<name>.png`.
pub fn override_path(dir: &Path, texture_name: &str, area: u8) -> Option<PathBuf> {
    [format!("{texture_name}__a{area:02}.png"), format!("{texture_name}.png")]
        .into_iter()
        .map(|f| dir.join(f))
        .find(|p| p.is_file())
}

/// Decode a PNG of any size and colour type (8/16 bit, grey, palette, RGB, RGBA) to RGBA8.
pub fn load_png_rgba(path: &Path) -> Result<TextureData, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| format!("{}: {e}", path.display()))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("png too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("{}: {e}", path.display()))?;
    let d = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => d.to_vec(),
        png::ColorType::Rgb => d.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::Grayscale => d.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => d.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], 255]).collect(),
        other => return Err(format!("{}: unsupported colour type {other:?}", path.display())),
    };
    if info.width == 0 || info.height == 0 {
        return Err(format!("{}: empty image", path.display()));
    }
    Ok(TextureData { width: info.width, height: info.height, rgba })
}

/// One ground layer ready to bake (original or improved).
pub struct ResolvedLayer<'a> {
    pub slot: usize,
    pub texture_name: String,
    pub tex: Cow<'a, TextureData>,
    /// Improved PNG used instead of the original, if any.
    pub override_file: Option<PathBuf>,
    /// SplatST x scale (repeats per `DETAIL_TILE_DIVISOR` metres).
    pub tiling: f32,
}

/// Resolve the `SplatColor0..7` layers of `area`, applying improved PNGs from `override_dir`.
pub fn resolve_layers<'a>(area: &'a TerrainArea, override_dir: Option<&Path>) -> Result<Vec<ResolvedLayer<'a>>, String> {
    let material = area.material.as_ref().ok_or("terrain material missing")?;
    let mut out = Vec::new();
    for (file, role) in &material.slots.textures {
        let Some(k) = role.strip_prefix("SplatColor").and_then(|d| d.parse::<usize>().ok()) else {
            continue;
        };
        if k >= 8 {
            continue;
        }
        let original = area.ground_textures.get(file).ok_or_else(|| format!("ground texture {file} unresolved"))?;
        let texture_name = texture_name_of(file);
        let override_file = override_dir.and_then(|d| override_path(d, &texture_name, area.area));
        let tex = match &override_file {
            Some(p) => Cow::Owned(load_png_rgba(p)?),
            None => Cow::Borrowed(original),
        };
        let tiling = material.splat_tiling.get(k).map(|s| s[0]).unwrap_or(1.0);
        out.push(ResolvedLayer { slot: k, texture_name, tex, override_file, tiling });
    }
    out.sort_by_key(|l| l.slot);
    Ok(out)
}

/// Linear-light mip chain of a layer, RGB stored as u16 (keeps big improved textures small).
struct MipChain {
    levels: Vec<(usize, usize, Vec<u16>)>,
    mean: [f32; 3],
}

impl MipChain {
    fn new(tex: &TextureData, lut: &[f32; 256]) -> Self {
        let (w, h) = (tex.width as usize, tex.height as usize);
        let mut base = Vec::with_capacity(w * h * 3);
        let mut sum = [0.0f64; 3];
        for p in tex.rgba.chunks_exact(4) {
            for c in 0..3 {
                let l = lut[p[c] as usize];
                sum[c] += l as f64;
                base.push((l * 65535.0 + 0.5) as u16);
            }
        }
        let n = (w * h).max(1) as f64;
        let mean = [(sum[0] / n) as f32, (sum[1] / n) as f32, (sum[2] / n) as f32];
        let mut levels = vec![(w, h, base)];
        while levels.last().is_some_and(|l| l.0 > 1 || l.1 > 1) {
            let (pw, ph, prev) = levels.last().unwrap();
            let (nw, nh) = ((*pw / 2).max(1), (*ph / 2).max(1));
            let mut next = vec![0u16; nw * nh * 3];
            for y in 0..nh {
                let (y0, y1) = ((2 * y).min(ph - 1), (2 * y + 1).min(ph - 1));
                for x in 0..nw {
                    let (x0, x1) = ((2 * x).min(pw - 1), (2 * x + 1).min(pw - 1));
                    for c in 0..3 {
                        let s = prev[(y0 * pw + x0) * 3 + c] as u32
                            + prev[(y0 * pw + x1) * 3 + c] as u32
                            + prev[(y1 * pw + x0) * 3 + c] as u32
                            + prev[(y1 * pw + x1) * 3 + c] as u32;
                        next[(y * nw + x) * 3 + c] = ((s + 2) / 4) as u16;
                    }
                }
            }
            levels.push((nw, nh, next));
        }
        Self { levels, mean }
    }

    /// Bilinear sample with wrap-around addressing; (u, v) in tile fractions.
    fn bilinear(&self, level: usize, u: f32, v: f32) -> [f32; 3] {
        let (w, h, d) = &self.levels[level];
        let (fx, fy) = (u * *w as f32 - 0.5, v * *h as f32 - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (xi0, yi0) = ((x0 as i64).rem_euclid(*w as i64) as usize, (y0 as i64).rem_euclid(*h as i64) as usize);
        let (xi1, yi1) = ((xi0 + 1) % w, (yi0 + 1) % h);
        let mut out = [0.0f32; 3];
        for (c, o) in out.iter_mut().enumerate() {
            let t = |x: usize, y: usize| d[(y * w + x) * 3 + c] as f32;
            let a = t(xi0, yi0) * (1.0 - tx) + t(xi1, yi0) * tx;
            let b = t(xi0, yi1) * (1.0 - tx) + t(xi1, yi1) * tx;
            *o = (a * (1.0 - ty) + b * ty) * (1.0 / 65535.0);
        }
        out
    }

    /// Trilinear sample (mip blend by `lod`) with wrap-around addressing.
    fn sample(&self, u: f32, v: f32, lod: f32) -> [f32; 3] {
        let max = (self.levels.len() - 1) as f32;
        let lod = lod.clamp(0.0, max);
        let l0 = lod.floor() as usize;
        let t = lod - l0 as f32;
        let a = self.bilinear(l0, u, v);
        if t <= 1e-3 || l0 + 1 >= self.levels.len() {
            return a;
        }
        let b = self.bilinear(l0 + 1, u, v);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
    }
}

/// Result of a bake: the image and the signature describing what went into it.
pub struct BakeResult {
    pub image: TextureData,
    /// Sidecar JSON (`areaNN.bake.json`) content.
    pub signature: serde_json::Value,
}

fn signature(area: &TerrainArea, layers: &[ResolvedLayer], opts: &BakeOptions) -> serde_json::Value {
    let splat1 = area.splat_alpha1.as_ref().map(texture_hash).unwrap_or_default();
    serde_json::json!({
        "version": SIDECAR_VERSION,
        "area": area.area,
        "size": opts.size,
        "detail": opts.detail,
        "tile_divisor": DETAIL_TILE_DIVISOR,
        "extent": area.world_extent[0],
        "splat_hash": format!("{}+{}", texture_hash(&area.splat_alpha), splat1),
        "layers": layers.iter().map(|l| serde_json::json!({
            "slot": l.slot,
            "texture": l.texture_name,
            "source": if l.override_file.is_some() { "override" } else { "original" },
            "override_file": l.override_file.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()),
            "width": l.tex.width,
            "height": l.tex.height,
            "hash": texture_hash(&l.tex),
            "tiling": l.tiling,
        })).collect::<Vec<_>>(),
    })
}

/// Signature a bake of `area` with `opts` would have, without baking.
pub fn bake_signature(area: &TerrainArea, opts: &BakeOptions) -> Result<serde_json::Value, String> {
    let layers = resolve_layers(area, opts.override_dir.as_deref())?;
    Ok(signature(area, &layers, opts))
}

/// Compose the area albedo at `out_size`² (sRGB RGBA8, alpha 255) with the original layers.
/// Kept for callers that do not know about improved textures (e.g. the viewer fallback).
pub fn bake_albedo(area: &TerrainArea, out_size: u32, detail: bool) -> Result<TextureData, String> {
    Ok(bake_albedo_with(area, &BakeOptions { size: out_size, detail, override_dir: None, threads: 0 })?.image)
}

/// Compose the area albedo. The terrain spans `world_extent[0]` metres. With `detail = false`
/// each layer is its mean colour; with `detail = true` the ground textures (original or the
/// improved PNG from `override_dir`, any size) are tiled with wrap-around addressing at
/// `SplatST / DETAIL_TILE_DIVISOR` repeats per metre, trilinear-filtered in linear light.
pub fn bake_albedo_with(area: &TerrainArea, opts: &BakeOptions) -> Result<BakeResult, String> {
    if opts.size == 0 {
        return Err("bake size must be > 0".into());
    }
    let layers = resolve_layers(area, opts.override_dir.as_deref())?;
    let lut = srgb_lut();
    let chains: Vec<(usize, f32, MipChain)> =
        layers.iter().map(|l| (l.slot, l.tiling, MipChain::new(&l.tex, &lut))).collect();
    let w = weights(area)?;
    let (sw, sh) = (area.splat_alpha.width as usize, area.splat_alpha.height as usize);
    let extent = area.world_extent[0];
    let size = opts.size as usize;
    let mut rgba = vec![255u8; size * size * 4];
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
                    let fy = ((y as f32 + 0.5) * sh as f32 / size as f32 - 0.5).clamp(0.0, sh as f32 - 1.0);
                    let (y0, ty) = (fy.floor() as usize, fy.fract());
                    let y1 = (y0 + 1).min(sh - 1);
                    let wz = (y as f32 + 0.5) * extent / size as f32;
                    for x in 0..size {
                        let fx = ((x as f32 + 0.5) * sw as f32 / size as f32 - 0.5).clamp(0.0, sw as f32 - 1.0);
                        let (x0, tx) = (fx.floor() as usize, fx.fract());
                        let x1 = (x0 + 1).min(sw - 1);
                        let wx = (x as f32 + 0.5) * extent / size as f32;
                        let mut acc = [0.0f32; 3];
                        for (slot, tiling, chain) in chains.iter() {
                            let a = w[y0 * sw + x0][*slot] * (1.0 - tx) + w[y0 * sw + x1][*slot] * tx;
                            let b = w[y1 * sw + x0][*slot] * (1.0 - tx) + w[y1 * sw + x1][*slot] * tx;
                            let wk = a * (1.0 - ty) + b * ty;
                            if wk <= 0.0 {
                                continue;
                            }
                            let c: [f32; 3] = if opts.detail {
                                let rep = tiling / DETAIL_TILE_DIVISOR;
                                let (bw, bh, _) = &chain.levels[0];
                                // texels per output pixel along u and v -> mip level
                                let mpp = extent / size as f32;
                                let fp = ((*bw as f32 * rep * mpp) * (*bh as f32 * rep * mpp)).sqrt();
                                chain.sample((wx * rep).rem_euclid(1.0), (wz * rep).rem_euclid(1.0), fp.max(1.0).log2())
                            } else {
                                chain.mean
                            };
                            for ch in 0..3 {
                                acc[ch] += c[ch] * wk;
                            }
                        }
                        for ch in 0..3 {
                            row[x * 4 + ch] = linear_to_srgb(acc[ch]);
                        }
                    }
                }
            });
        }
    });
    let signature = signature(area, &layers, opts);
    Ok(BakeResult { image: TextureData { width: opts.size, height: opts.size, rgba }, signature })
}

/// Sidecar path next to the baked PNG: `areaNN.bake.json`.
pub fn bake_sidecar_path(work_root: impl AsRef<Path>, area: u8) -> PathBuf {
    work_root.as_ref().join("cache/terrain_albedo").join(format!("area{area:02}.bake.json"))
}

/// What `bake_area_to_cache` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BakeOutcome {
    Skipped,
    Baked,
}

/// Bake `area` into `work_root/cache/terrain_albedo/areaNN.png` plus its sidecar. With
/// `skip_if_unchanged` the bake is skipped when the PNG exists and the stored sidecar equals
/// the signature of the current inputs (layer hashes/sizes, options).
pub fn bake_area_to_cache(
    area: &TerrainArea,
    work_root: &Path,
    opts: &BakeOptions,
    skip_if_unchanged: bool,
) -> Result<BakeOutcome, String> {
    let png_path = albedo_cache_path(work_root, area.area);
    let side_path = bake_sidecar_path(work_root, area.area);
    if skip_if_unchanged && png_path.is_file() {
        if let Ok(text) = std::fs::read_to_string(&side_path) {
            if let Ok(old) = serde_json::from_str::<serde_json::Value>(&text) {
                if old == bake_signature(area, opts)? {
                    return Ok(BakeOutcome::Skipped);
                }
            }
        }
    }
    let result = bake_albedo_with(area, opts)?;
    result.image.save_png(&png_path).map_err(|e| format!("write {}: {e}", png_path.display()))?;
    std::fs::write(&side_path, serde_json::to_string_pretty(&result.signature).unwrap() + "\n")
        .map_err(|e| format!("write {}: {e}", side_path.display()))?;
    Ok(BakeOutcome::Baked)
}

/// Cache location of the baked albedo for `area` under the repo work dir.
pub fn albedo_cache_path(work_root: impl AsRef<Path>, area: u8) -> PathBuf {
    work_root.as_ref().join("cache/terrain_albedo").join(format!("area{area:02}.png"))
}

/// Load a baked `areaNN.png` (RGBA8 or RGB8) written by `terrain_albedo_bake`.
pub fn load_baked_albedo(path: impl AsRef<Path>) -> Result<TextureData, String> {
    let file = std::fs::File::open(path.as_ref())
        .map_err(|e| format!("open {}: {e}", path.as_ref().display()))?;
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("png too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let data = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        other => return Err(format!("unsupported baked albedo colour type {other:?}")),
    };
    Ok(TextureData { width: info.width, height: info.height, rgba })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::bntx::FloatTextureData;
    use crate::assets::terrain::TerrainMaterial;
    use crate::assets::tr::TrMtr;
    use std::collections::HashMap;

    fn solid(w: u32, h: u32, c: [u8; 3]) -> TextureData {
        TextureData { width: w, height: h, rgba: (0..w * h).flat_map(|_| [c[0], c[1], c[2], 255]).collect() }
    }

    /// One layer (slot 0, weight 1 everywhere), 100 m extent, tiling 32 => 1 repeat per metre.
    fn area_with(tex: TextureData) -> TerrainArea {
        let mut g = HashMap::new();
        g.insert("ha_test_alb.bntx".to_string(), tex);
        TerrainArea {
            area: 3,
            source_path: PathBuf::new(),
            world_extent: [100.0, 100.0],
            world_height: 1.0,
            heightmap_brti_name: "h".into(),
            navmesh_bounds: None,
            heightmap: FloatTextureData { width: 1, height: 1, samples: vec![0.0] },
            normal_map: solid(1, 1, [0, 0, 0]),
            splat_alpha: TextureData { width: 2, height: 2, rgba: [255, 0, 0, 0].repeat(4) },
            splat_alpha1: None,
            material: Some(TerrainMaterial {
                slots: TrMtr { textures: vec![("ha_test_alb.bntx".into(), "SplatColor0".into())], flags: vec![] },
                parameter_candidates: vec![],
                splat_tiling: vec![[32.0, 32.0]],
            }),
            ground_textures: g,
            unresolved_ground_textures: vec![],
            masks: vec![],
            instances: vec![],
        }
    }

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pla_albedo_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn opts(size: u32, dir: Option<PathBuf>) -> BakeOptions {
        BakeOptions { size, detail: true, override_dir: dir, threads: 2 }
    }

    #[test]
    fn override_is_used_instead_of_the_original() {
        let area = area_with(solid(4, 4, [255, 0, 0]));
        let dir = tmpdir("override");
        let before = bake_albedo_with(&area, &opts(8, Some(dir.clone()))).unwrap();
        assert_eq!(&before.image.rgba[0..3], &[255, 0, 0]);
        // 2x2 blue improved texture of a different size
        solid(2, 2, [0, 0, 255]).save_png(&dir.join("ha_test_alb.png")).unwrap();
        let after = bake_albedo_with(&area, &opts(8, Some(dir.clone()))).unwrap();
        assert_eq!(&after.image.rgba[0..3], &[0, 0, 255]);
        assert_eq!(after.signature["layers"][0]["source"], "override");
        assert_eq!(after.signature["layers"][0]["width"], 2);
        // area-specific file wins over the plain one
        solid(2, 2, [0, 255, 0]).save_png(&dir.join("ha_test_alb__a03.png")).unwrap();
        let spec = bake_albedo_with(&area, &opts(8, Some(dir.clone()))).unwrap();
        assert_eq!(&spec.image.rgba[0..3], &[0, 255, 0]);
        // another area ignores it and uses the plain file
        let mut other = area_with(solid(4, 4, [255, 0, 0]));
        other.area = 4;
        let o = bake_albedo_with(&other, &opts(8, Some(dir.clone()))).unwrap();
        assert_eq!(&o.image.rgba[0..3], &[0, 0, 255]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn tiling_wraps_without_a_seam() {
        // horizontal ramp 0..255 over 8 texels: wrapping must blend last and first texel
        let mut t = solid(8, 8, [0, 0, 0]);
        for y in 0..8 {
            for x in 0..8 {
                let v = (x * 36) as u8;
                let o = (y * 8 + x) * 4;
                t.rgba[o..o + 3].copy_from_slice(&[v, v, v]);
            }
        }
        let chain = MipChain::new(&t, &srgb_lut());
        let eps = 1e-4;
        let right = chain.sample(1.0 - eps, 0.3, 0.0);
        let left = chain.sample(eps, 0.3, 0.0);
        let mid_step = (chain.sample(0.5 + 0.125, 0.3, 0.0)[0] - chain.sample(0.5, 0.3, 0.0)[0]).abs();
        assert!((right[0] - left[0]).abs() < 1e-2, "seam in u: {right:?} vs {left:?}");
        assert!(mid_step > 0.01);
        let top = chain.sample(0.4, 1.0 - eps, 0.0);
        let bottom = chain.sample(0.4, eps, 0.0);
        assert!((top[0] - bottom[0]).abs() < 1e-2);
        // baked image: pixels 0 and 99 m apart are neighbours across the tile edge (period 1 m)
        let area = area_with(t);
        let img = bake_albedo_with(&area, &opts(100, None)).unwrap().image;
        assert_eq!(img.width, 100);
    }

    #[test]
    fn size_option_sets_the_output_size() {
        let area = area_with(solid(4, 4, [10, 200, 30]));
        for s in [16u32, 40] {
            let r = bake_albedo_with(&area, &opts(s, None)).unwrap();
            assert_eq!((r.image.width, r.image.height), (s, s));
            assert_eq!(r.image.rgba.len(), (s * s * 4) as usize);
            assert_eq!(r.signature["size"], s);
        }
        // solid colour survives linear-light blending and mipmapping
        let r = bake_albedo_with(&area, &opts(16, None)).unwrap();
        for p in r.image.rgba.chunks_exact(4) {
            assert!((p[0] as i32 - 10).abs() <= 1 && (p[1] as i32 - 200).abs() <= 1 && (p[2] as i32 - 30).abs() <= 1);
        }
    }

    #[test]
    fn sidecar_detects_changes_and_allows_skipping() {
        let area = area_with(solid(4, 4, [255, 0, 0]));
        let work = tmpdir("sidecar");
        let ov = work.join(OVERRIDE_SUBDIR);
        std::fs::create_dir_all(&ov).unwrap();
        let o = opts(8, Some(ov.clone()));
        assert_eq!(bake_area_to_cache(&area, &work, &o, true).unwrap(), BakeOutcome::Baked);
        assert!(bake_sidecar_path(&work, 3).is_file());
        assert_eq!(bake_area_to_cache(&area, &work, &o, true).unwrap(), BakeOutcome::Skipped);
        // improved texture appears -> re-bake, then skip again
        solid(2, 2, [0, 0, 255]).save_png(&ov.join("ha_test_alb.png")).unwrap();
        assert_eq!(bake_area_to_cache(&area, &work, &o, true).unwrap(), BakeOutcome::Baked);
        assert_eq!(bake_area_to_cache(&area, &work, &o, true).unwrap(), BakeOutcome::Skipped);
        // changed content (same size) -> re-bake
        solid(2, 2, [0, 255, 255]).save_png(&ov.join("ha_test_alb.png")).unwrap();
        assert_eq!(bake_area_to_cache(&area, &work, &o, true).unwrap(), BakeOutcome::Baked);
        // different size option -> re-bake
        assert_eq!(bake_area_to_cache(&area, &work, &opts(16, Some(ov.clone())), true).unwrap(), BakeOutcome::Baked);
        let side: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(bake_sidecar_path(&work, 3)).unwrap()).unwrap();
        assert_eq!(side["layers"][0]["hash"].as_str().unwrap().len(), 16);
        let _ = std::fs::remove_dir_all(work);
    }
}
