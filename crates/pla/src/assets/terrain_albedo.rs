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

use std::path::{Path, PathBuf};

use super::bntx::TextureData;
use super::terrain::TerrainArea;

/// World-space divisor applied to `SplatST` for the detail tiling period:
/// `repeats_per_metre = ST / DETAIL_TILE_DIVISOR`. **Assumption**, not decoded.
pub const DETAIL_TILE_DIVISOR: f32 = 32.0;

fn srgb_to_linear(v: u8) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let v = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (v * 255.0 + 0.5) as u8
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

/// Compose the area albedo at `out_size`² (sRGB RGBA8, alpha 255). The terrain
/// spans `world_extent[0]` metres. With `detail = false` each layer is its mean
/// colour (robust, no tiling assumption); with `detail = true` the ground
/// textures are tiled using `SplatST / DETAIL_TILE_DIVISOR` repeats per metre.
pub fn bake_albedo(area: &TerrainArea, out_size: u32, detail: bool) -> Result<TextureData, String> {
    let material = area.material.as_ref().ok_or("terrain material missing")?;
    let mut layers: Vec<Option<(&TextureData, f32, Vec<f32>)>> = (0..8).map(|_| None).collect();
    for (file, role) in &material.slots.textures {
        let Some(k) = role.strip_prefix("SplatColor").and_then(|d| d.parse::<usize>().ok()) else {
            continue;
        };
        if k >= 8 {
            continue;
        }
        let tex = area
            .ground_textures
            .get(file)
            .ok_or_else(|| format!("ground texture {file} unresolved"))?;
        let lin: Vec<f32> = tex
            .rgba
            .chunks_exact(4)
            .flat_map(|p| [srgb_to_linear(p[0]), srgb_to_linear(p[1]), srgb_to_linear(p[2])])
            .collect();
        let n = (tex.width * tex.height) as f32;
        let _ = n;
        let scale = material.splat_tiling.get(k).map(|s| s[0]).unwrap_or(1.0);
        layers[k] = Some((tex, scale, lin));
    }
    let w = weights(area)?;
    let (sw, sh) = (area.splat_alpha.width as usize, area.splat_alpha.height as usize);
    let extent = area.world_extent[0];
    let size = out_size as usize;
    let mut rgba = vec![255u8; size * size * 4];
    for y in 0..size {
        // bilinear weight sample
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
            for (k, layer) in layers.iter().enumerate() {
                let Some((tex, scale, lin)) = layer else { continue };
                let a = w[y0 * sw + x0][k] * (1.0 - tx) + w[y0 * sw + x1][k] * tx;
                let b = w[y1 * sw + x0][k] * (1.0 - tx) + w[y1 * sw + x1][k] * tx;
                let wk = a * (1.0 - ty) + b * ty;
                if wk <= 0.0 {
                    continue;
                }
                let c: [f32; 3] = if detail {
                    let rep = scale / DETAIL_TILE_DIVISOR;
                    let u = (wx * rep).fract();
                    let v = (wz * rep).fract();
                    let tx_ = ((u * tex.width as f32) as usize).min(tex.width as usize - 1);
                    let ty_ = ((v * tex.height as f32) as usize).min(tex.height as usize - 1);
                    let o = (ty_ * tex.width as usize + tx_) * 3;
                    [lin[o], lin[o + 1], lin[o + 2]]
                } else {
                    mean_colour(lin)
                };
                for ch in 0..3 {
                    acc[ch] += c[ch] * wk;
                }
            }
            let o = (y * size + x) * 4;
            for ch in 0..3 {
                rgba[o + ch] = linear_to_srgb(acc[ch]);
            }
        }
    }
    Ok(TextureData { width: out_size, height: out_size, rgba })
}

fn mean_colour(lin: &[f32]) -> [f32; 3] {
    let n = (lin.len() / 3).max(1) as f32;
    let mut s = [0.0f32; 3];
    for p in lin.chunks_exact(3) {
        for c in 0..3 {
            s[c] += p[c];
        }
    }
    [s[0] / n, s[1] / n, s[2] / n]
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
