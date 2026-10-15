//! Real-time terrain material (`--terrain splat`, the default): the ground is drawn from the
//! area's 8 splat layers on the GPU, every frame, instead of one baked 4096 px picture per area.
//!
//! Why: the bake (`assets::terrain_albedo`) squeezes a ~1025 m area into 4096 px (~0.25 m per
//! pixel), so each tiled ground layer ends up 16-50 px wide on screen and a sharper (1024+ px)
//! improved layer only selects a coarser mip. Here every layer keeps its own resolution and is
//! tiled in the shader, so near the camera the improved textures show at full sharpness.
//!
//! What the material does (shader `terrain_splat.wgsl`, an `ExtendedMaterial` over the viewer's
//! `StandardMaterial`, so PBR lighting, shadows, SSAO, fog and the art style keep working):
//! * blends the 8 layers with the decoded splat weights (two RGBA control textures, sampled
//!   bilinearly with a small noise warp), plus a height-like blend driven by each layer's own
//!   detail so boundaries follow the texture instead of the 1 m splat steps;
//! * projects every layer triplanar on slopes (top + two side projections; flat ground keeps one
//!   top-down sample) and hands very steep faces to the cliff layers, so nothing stretches on
//!   walls;
//! * fades to the old baked albedo ("macro" colour, the StandardMaterial base texture) with the
//!   camera distance, which keeps the area's overall colour and hides tiling far away; a
//!   two-sample anti-tiling and a low-frequency brightness variation hide it at mid distance;
//! * normal-maps with the per-layer `*_nrm` textures (live switch, falls back to the baked one).
//!
//! The old baked path stays: `--terrain bake` (weak PCs) keeps the plain `StandardMaterial`;
//! the splat path also falls back to it when an area has no usable splat data.
//!
//! How it hooks in without touching the terrain spawn code: the viewer and the zone change still
//! spawn the chunks with the baked `StandardMaterial`; this plugin notices them
//! ([`TerrainChunkMarker`]), builds the layer textures of the area on a background thread and
//! then swaps each chunk's material. Chunks, frustum culling and the mesh are unchanged.
//!
//! Evidence labels: VERIFIED (decoded data) - the 8 weights, the layer textures, `SplatST`, which
//! slots are cliffs (`TriplanarLayerCount` 1/2 in areas 00/01 matches the leading cliff names).
//! SUPUESTO / ASSUMED - the tiling period (`SplatST / DETAIL_TILE_DIVISOR` per metre, the same
//! guess as the bake), the height-like blend, slope thresholds, fade distances, anti-tiling,
//! the brightness variation, the normal-map orientation and every strength below.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use bevy::asset::{embedded_asset, RenderAssetUsages};
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat, TextureViewDescriptor,
    TextureViewDimension,
};
use bevy::shader::ShaderRef;
use bevy::tasks::{futures_lite::future, AsyncComputeTaskPool, Task};

use crate::assets::bntx::TextureData;
use crate::assets::terrain::TerrainArea;
use crate::assets::terrain_albedo::{load_png_rgba, override_path, texture_name_of, DETAIL_TILE_DIVISOR, OVERRIDE_SUBDIR};
use crate::assets::terrain_normals::{resolve_normal_layers, unit_normal};

use super::cli_flags;
use super::zone::{TerrainChunkMarker, TerrainResource};

/// Embedded fragment shader (see the `embedded_asset!` call in [`TerrainSplatPlugin`]).
pub const TERRAIN_SPLAT_SHADER_PATH: &str = "embedded://pla/game/terrain_splat.wgsl";
/// The shader source, for the naga validation test.
pub const TERRAIN_SPLAT_WGSL: &str = include_str!("terrain_splat.wgsl");

/// Material slots of the terrain (`SplatColor0..7`, two RGBA control layers).
pub const SLOTS: usize = 8;
/// Default side of every layer in the albedo array (CPU-resized, full mip chain).
pub const DEFAULT_LAYER_SIZE: u32 = 1024;
/// Accepted layer sizes (powers of two).
pub const LAYER_SIZE_RANGE: (u32, u32) = (64, 4096);

// --- Tuning (all SUPUESTO: chosen by eye on the windowless captures, not decoded) ----------------

/// Camera distance (m) where the layers start fading into the baked macro colour.
pub const FADE_START_M: f32 = 70.0;
/// Camera distance (m) beyond which only the macro colour is drawn (no layer is sampled).
pub const FADE_END_M: f32 = 220.0;
/// Height-like blend: only layers within this score of the best one survive.
pub const HEIGHT_BLEND_DEPTH: f32 = 0.25;
/// Weight of a layer's detail (luminance relative to its mean) in the blend score.
pub const HEIGHT_STRENGTH: f32 = 0.35;
/// Slope (1 - |normal.y|) where cliff layers start to take over: 0.45 = 57 degrees. Gentler
/// slopes keep their painted layer (projected triplanar, so it does not stretch); a first try at
/// 41-53 degrees put dark rock on the snow banks of area 05.
pub const STEEP_START: f32 = 0.45;
/// Slope where only cliff layers remain: 0.62 = 68 degrees.
pub const STEEP_END: f32 = 0.62;
/// Strength of the per-layer normal maps (tangent-space xy scale).
pub const NORMAL_STRENGTH: f32 = 1.0;
/// Low-frequency brightness variation (+/- fraction).
pub const BRIGHTNESS_VARIATION: f32 = 0.06;
/// Noise warp of the splat lookup (metres) that breaks the 1 px/m stair steps.
pub const SPLAT_WARP_M: f32 = 0.6;
/// Anti-tiling noise frequency (noise cells per tile).
pub const ANTI_TILING_FREQ: f32 = 0.6;

// --- Settings -------------------------------------------------------------------------------------

/// Which terrain path draws the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TerrainMode {
    /// One baked albedo image per area on a plain `StandardMaterial` (the previous path).
    Bake,
    /// Real-time splat material (this module).
    #[default]
    Splat,
}

/// Debug views of the splat material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TerrainDebugView {
    #[default]
    Off,
    /// False colour per slot (the blended weights).
    Weights,
    /// Red = layer detail amount (distance fade), green = steepness.
    Detail,
}

/// Terrain settings. Everything except `mode`, `layer_size` and `anisotropy` applies live (the
/// uniform is rewritten); those three rebuild the area's textures. Ready for a Graphics-screen row
/// (layer count, triplanar, anti-tiling, layer normals, layer size).
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct TerrainSplatSettings {
    pub mode: TerrainMode,
    /// Side of each layer in the albedo array (power of two; capped at the largest source layer).
    pub layer_size: u32,
    /// How many slots are drawn (1..=8); the most covering slots of the area are kept.
    pub max_layers: u8,
    pub triplanar: bool,
    pub anti_tiling: bool,
    /// Per-layer normal maps (else the baked area normal map of the base material).
    pub layer_normals: bool,
    /// Anisotropic filtering of the layers (1 = off).
    pub anisotropy: u16,
    pub fade_start: f32,
    pub fade_end: f32,
    /// 0 = plain weights, else [`HEIGHT_STRENGTH`]-like strength.
    pub height_strength: f32,
    /// Metres per `SplatST` unit: a layer repeats `SplatST / tile_divisor` times per metre.
    /// Default [`DETAIL_TILE_DIVISOR`] (4.0, the shared assumption: grass tiles every 1.6 m);
    /// SUPUESTO, `--terrain-tile-divisor` overrides it.
    pub tile_divisor: f32,
    pub debug_view: TerrainDebugView,
}

impl Default for TerrainSplatSettings {
    fn default() -> Self {
        Self {
            mode: TerrainMode::Splat,
            layer_size: DEFAULT_LAYER_SIZE,
            max_layers: SLOTS as u8,
            triplanar: true,
            anti_tiling: true,
            layer_normals: true,
            anisotropy: 16,
            fade_start: FADE_START_M,
            fade_end: FADE_END_M,
            height_strength: HEIGHT_STRENGTH,
            tile_divisor: DETAIL_TILE_DIVISOR,
            debug_view: TerrainDebugView::Off,
        }
    }
}

fn on_off(args: &[String], flag: &str, default: bool) -> bool {
    match cli_flags::value_of(args, flag) {
        None => default,
        Some(v) if matches!(v.to_ascii_lowercase().as_str(), "on" | "true" | "1" | "yes") => true,
        Some(v) if matches!(v.to_ascii_lowercase().as_str(), "off" | "false" | "0" | "no") => false,
        Some(v) => {
            cli_flags::report_bad_value(flag, v, "on|off", if default { "on" } else { "off" });
            default
        }
    }
}

impl TerrainSplatSettings {
    /// `--terrain bake|splat` (default splat), `--terrain-layer-size 256..4096` (power of two),
    /// `--terrain-layers 1..8`, `--terrain-triplanar on|off`, `--terrain-antitile on|off`,
    /// `--terrain-normals layers|baked`, `--terrain-debug off|weights|detail`,
    /// `--terrain-tile-divisor F` (default 4: a layer repeats `SplatST / F` times per metre).
    pub fn from_args(args: &[String]) -> Self {
        let mut s = Self::default();
        if let Some(v) = cli_flags::value_of(args, "--terrain") {
            match v.to_ascii_lowercase().as_str() {
                "bake" | "baked" => s.mode = TerrainMode::Bake,
                "splat" => s.mode = TerrainMode::Splat,
                _ => cli_flags::report_bad_value("--terrain", v, "bake|splat", "splat"),
            }
        }
        if let Some(n) = cli_flags::parse_or_warn::<u32>(
            args,
            "--terrain-layer-size",
            "a power of two 64..4096",
            DEFAULT_LAYER_SIZE,
            |n| n.is_power_of_two() && (LAYER_SIZE_RANGE.0..=LAYER_SIZE_RANGE.1).contains(n),
        ) {
            s.layer_size = n;
        }
        if let Some(n) = cli_flags::parse_or_warn::<u8>(args, "--terrain-layers", "1..8", SLOTS, |n| (1..=8).contains(n)) {
            s.max_layers = n;
        }
        if let Some(f) = cli_flags::parse_or_warn::<f32>(
            args,
            "--terrain-tile-divisor",
            "a number in 1..256",
            DETAIL_TILE_DIVISOR,
            |f| f.is_finite() && (1.0..=256.0).contains(f),
        ) {
            s.tile_divisor = f;
        }
        s.triplanar = on_off(args, "--terrain-triplanar", s.triplanar);
        s.anti_tiling = on_off(args, "--terrain-antitile", s.anti_tiling);
        if let Some(v) = cli_flags::value_of(args, "--terrain-normals") {
            match v.to_ascii_lowercase().as_str() {
                "layers" | "on" => s.layer_normals = true,
                "baked" | "off" => s.layer_normals = false,
                _ => cli_flags::report_bad_value("--terrain-normals", v, "layers|baked", "layers"),
            }
        }
        if let Some(v) = cli_flags::value_of(args, "--terrain-debug") {
            s.debug_view = match v.to_ascii_lowercase().as_str() {
                "weights" => TerrainDebugView::Weights,
                "detail" => TerrainDebugView::Detail,
                "off" => TerrainDebugView::Off,
                _ => {
                    cli_flags::report_bad_value("--terrain-debug", v, "off|weights|detail", "off");
                    TerrainDebugView::Off
                }
            };
        }
        s
    }

    /// The settings that need the textures rebuilt when they change.
    fn build_key(&self) -> (u32, u16) {
        (self.layer_size, self.anisotropy)
    }
}

// --- CPU data -------------------------------------------------------------------------------------

/// Number of mip levels of a `size`-sided square down to 1x1 (1024 -> 11).
pub fn mip_count(size: u32) -> u32 {
    32 - size.max(1).leading_zeros()
}

/// A square texture array with a full mip chain. `data` is layer-major (every mip of layer 0,
/// then layer 1, ...), the order `wgpu::TextureDataOrder::LayerMajor` (Bevy's default) expects.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerArray {
    pub size: u32,
    pub layers: u32,
    pub mip_levels: u32,
    /// RGBA8, sRGB-encoded for albedo, linear for normals / control weights.
    pub data: Vec<u8>,
}

impl LayerArray {
    /// Bytes of one layer (all its mips).
    pub fn layer_bytes(&self) -> usize {
        (0..self.mip_levels).map(|m| ((self.size >> m).max(1) as usize).pow(2) * 4).sum()
    }

    /// Level-0 RGBA of texel (x, y) of `layer`.
    pub fn texel(&self, layer: u32, x: u32, y: u32) -> [u8; 4] {
        let o = layer as usize * self.layer_bytes() + ((y * self.size + x) as usize) * 4;
        [self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]]
    }
}

fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
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

/// Source taps (index, weight) of one destination texel when resampling `n_src` texels to
/// `n_dst` with wrap-around addressing: a box (area) filter when shrinking, linear when growing.
fn resample_taps(n_src: usize, n_dst: usize) -> Vec<Vec<(usize, f32)>> {
    let scale = n_src as f32 / n_dst as f32;
    (0..n_dst)
        .map(|i| {
            if scale <= 1.0 {
                let f = (i as f32 + 0.5) * scale - 0.5;
                let x0 = f.floor();
                let t = f - x0;
                let a = (x0 as i64).rem_euclid(n_src as i64) as usize;
                let b = (a + 1) % n_src;
                vec![(a, 1.0 - t), (b, t)]
            } else {
                let (lo, hi) = (i as f32 * scale, (i as f32 + 1.0) * scale);
                let mut taps = Vec::new();
                let mut x = lo.floor();
                while x < hi {
                    let w = (x + 1.0).min(hi) - x.max(lo);
                    if w > 0.0 {
                        taps.push(((x as i64).rem_euclid(n_src as i64) as usize, w / scale));
                    }
                    x += 1.0;
                }
                taps
            }
        })
        .collect()
}

/// Resample a float RGBA image (`w` x `h`) to `size` x `size`, wrap-around (the layers tile).
fn resample_wrap(px: &[[f32; 4]], w: usize, h: usize, size: usize) -> Vec<[f32; 4]> {
    if w == size && h == size {
        return px.to_vec();
    }
    let tx = resample_taps(w, size);
    let ty = resample_taps(h, size);
    let mut rows = vec![[0.0f32; 4]; size * h];
    for y in 0..h {
        for (x, taps) in tx.iter().enumerate() {
            let mut acc = [0.0f32; 4];
            for &(sx, wt) in taps {
                let s = px[y * w + sx];
                for c in 0..4 {
                    acc[c] += s[c] * wt;
                }
            }
            rows[y * size + x] = acc;
        }
    }
    let mut out = vec![[0.0f32; 4]; size * size];
    for (y, taps) in ty.iter().enumerate() {
        for x in 0..size {
            let mut acc = [0.0f32; 4];
            for &(sy, wt) in taps {
                let s = rows[sy * size + x];
                for c in 0..4 {
                    acc[c] += s[c] * wt;
                }
            }
            out[y * size + x] = acc;
        }
    }
    out
}

/// 2x2 box reduction of a square float image.
fn halve(px: &[[f32; 4]], size: usize) -> Vec<[f32; 4]> {
    let n = (size / 2).max(1);
    let mut out = vec![[0.0f32; 4]; n * n];
    for y in 0..n {
        for x in 0..n {
            let (x0, y0) = ((2 * x).min(size - 1), (2 * y).min(size - 1));
            let (x1, y1) = ((2 * x + 1).min(size - 1), (2 * y + 1).min(size - 1));
            let mut acc = [0.0f32; 4];
            for (sx, sy) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                let s = px[sy * size + sx];
                for c in 0..4 {
                    acc[c] += s[c] * 0.25;
                }
            }
            out[y * n + x] = acc;
        }
    }
    out
}

/// How a float texel becomes RGBA8.
#[derive(Clone, Copy)]
enum Encode {
    /// Linear light -> sRGB (albedo).
    Srgb,
    /// Tangent-space normal in [-1,1]: renormalised, then `n * 0.5 + 0.5`.
    Normal,
    /// Plain [0,1] values (splat weights).
    Linear,
}

fn encode(px: [f32; 4], how: Encode) -> [u8; 4] {
    let lin = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    match how {
        Encode::Srgb => [linear_to_srgb(px[0]), linear_to_srgb(px[1]), linear_to_srgb(px[2]), 255],
        Encode::Normal => {
            let n = unit_normal(px[0], px[1]);
            let z = px[2].max(1e-3);
            let len = (px[0] * px[0] + px[1] * px[1] + z * z).sqrt();
            let v = if len > 1e-6 { [px[0] / len, px[1] / len, z / len] } else { n };
            let e = |c: f32| ((c.clamp(-1.0, 1.0) * 0.5 + 0.5) * 255.0 + 0.5) as u8;
            [e(v[0]), e(v[1]), e(v[2]), 255]
        }
        Encode::Linear => [lin(px[0]), lin(px[1]), lin(px[2]), lin(px[3])],
    }
}

/// Append level 0 (`size`²) and every mip of one layer to `out`.
fn push_mip_chain(mut level: Vec<[f32; 4]>, size: usize, how: Encode, out: &mut Vec<u8>) {
    let mut s = size;
    loop {
        out.extend(level.iter().flat_map(|p| encode(*p, how)));
        if s == 1 {
            break;
        }
        level = halve(&level, s);
        s = (s / 2).max(1);
    }
}

/// Build a texture array from float layers (each `(w, h, texels)`, any size), every layer
/// resampled to `size` with wrap-around and mip-mapped down to 1x1.
fn build_array(layers: &[(usize, usize, Vec<[f32; 4]>)], size: u32, how: Encode) -> LayerArray {
    let s = size as usize;
    let mut data = Vec::new();
    for (w, h, px) in layers {
        push_mip_chain(resample_wrap(px, *w, *h, s), s, how, &mut data);
    }
    LayerArray { size, layers: layers.len() as u32, mip_levels: mip_count(size), data }
}

fn linear_rgba(tex: &TextureData) -> Vec<[f32; 4]> {
    let lut: Vec<f32> = (0..=255u8).map(srgb_to_linear).collect();
    tex.rgba.chunks_exact(4).map(|p| [lut[p[0] as usize], lut[p[1] as usize], lut[p[2] as usize], 1.0]).collect()
}

/// Is this ground layer a cliff/rock layer (triplanar, takes the steep faces)? Matches the
/// decoded `TriplanarLayerCount` of areas 00 (1: cliff01) and 01 (2: cliff01, cliff02); the
/// other areas' leading slots are named the same way (cliff03/04, mountain01/02). SUPUESTO for
/// areas whose count was not read.
pub fn is_cliff_layer(texture_name: &str) -> bool {
    let n = texture_name.to_ascii_lowercase();
    n.contains("cliff") || n.contains("mountain") || n.contains("rock")
}

/// Repeats per metre of a layer: `SplatST / DETAIL_TILE_DIVISOR` (the bake's assumption).
pub fn repeats_per_metre(tiling: f32) -> f32 {
    repeats_per_metre_with(tiling, DETAIL_TILE_DIVISOR)
}

/// Repeats per metre of a layer with a chosen divisor (`--terrain-tile-divisor`).
pub fn repeats_per_metre_with(tiling: f32, divisor: f32) -> f32 {
    let t = if tiling.is_finite() && tiling > 0.0 { tiling } else { 1.0 };
    let d = if divisor.is_finite() && divisor > 0.0 { divisor } else { DETAIL_TILE_DIVISOR };
    t / d
}

/// One material slot as built.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotInfo {
    pub slot: usize,
    pub texture_name: String,
    /// Improved PNG used instead of the original.
    pub improved: Option<PathBuf>,
    /// Size of the source layer (original or improved).
    pub source_size: (u32, u32),
    /// `SplatST` x.
    pub tiling: f32,
    pub cliff: bool,
    /// Mean linear luminance of the layer.
    pub mean_luminance: f32,
    /// Share of the area's splat weight (0..1).
    pub coverage: f32,
}

/// Everything the material needs for one area, built off the main thread.
#[derive(Debug, Clone)]
pub struct SplatBuild {
    pub area: u8,
    pub extent: f32,
    /// Slots 0..8 (`None`: the area has no layer in that slot).
    pub slots: Vec<Option<SlotInfo>>,
    pub albedo: LayerArray,
    /// Per-layer tangent-space normals (`None` when the pack's normal layers could not be read).
    pub normals: Option<LayerArray>,
    /// Splat control layers 0 and 1 (RGBA = 4 weights each), with mips.
    pub control: [LayerArray; 2],
    pub build_ms: f64,
}

/// Default directory of improved layers: `$PLA_TERRAIN_OVERRIDES`, else
/// `<work>/texturas_mejoradas/terrain_layers` next to `$PLA_CACHE_ROOT` or under `./work`.
pub fn default_override_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("PLA_TERRAIN_OVERRIDES").map(PathBuf::from) {
        return p.is_dir().then_some(p);
    }
    let mut roots = Vec::new();
    if let Some(cache) = std::env::var_os("PLA_CACHE_ROOT").map(PathBuf::from) {
        if let Some(parent) = cache.parent() {
            roots.push(parent.to_path_buf());
        }
    }
    roots.push(PathBuf::from("work"));
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../work"));
    roots.into_iter().map(|r| r.join(OVERRIDE_SUBDIR)).find(|d| d.is_dir())
}

/// Splat control layer `i` (0/1) of `area` as a full-chain array of one layer.
fn control_array(tex: Option<&TextureData>, size: (u32, u32)) -> LayerArray {
    let (w, h) = (size.0.max(1) as usize, size.1.max(1) as usize);
    let px: Vec<[f32; 4]> = match tex {
        Some(t) if t.rgba.len() == w * h * 4 => {
            t.rgba.chunks_exact(4).map(|p| [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, p[3] as f32 / 255.0]).collect()
        }
        _ => vec![[0.0; 4]; w * h],
    };
    // Control maps are 1024² (power of two); pad other sizes by resampling to the next one.
    let side = (w.max(h) as u32).next_power_of_two();
    build_array(&[(w, h, px)], side, Encode::Linear)
}

/// Build the layer textures of `area`: albedo array (improved PNGs from `override_dir` win),
/// normal array, control weights and per-slot info. Pure CPU work, safe off the main thread.
pub fn build_splat_data(area: &TerrainArea, override_dir: Option<&Path>, layer_size: u32) -> Result<SplatBuild, String> {
    let t0 = Instant::now();
    let material = area.material.as_ref().ok_or("terrain material missing")?;
    let sw = area.splat_alpha.width as usize;
    let sh = area.splat_alpha.height as usize;
    if sw == 0 || sh == 0 || area.splat_alpha.rgba.len() != sw * sh * 4 {
        return Err("splat control texture missing or malformed".into());
    }
    // Coverage per slot (share of the summed weights).
    let mut cover = [0f64; SLOTS];
    for (i, p) in area.splat_alpha.rgba.chunks_exact(4).enumerate() {
        for c in 0..4 {
            cover[c] += p[c] as f64;
            if let Some(s1) = &area.splat_alpha1 {
                cover[4 + c] += s1.rgba.get(i * 4 + c).copied().unwrap_or(0) as f64;
            }
        }
    }
    let cover_total: f64 = cover.iter().sum::<f64>().max(1.0);

    // Resolve the albedo slots (same rule as the bake: improved file wins).
    let mut sources: Vec<Option<(SlotInfo, TextureData)>> = vec![None; SLOTS];
    for (file, role) in &material.slots.textures {
        let Some(k) = role.strip_prefix("SplatColor").and_then(|d| d.parse::<usize>().ok()) else { continue };
        if k >= SLOTS || sources[k].is_some() {
            continue;
        }
        let Some(original) = area.ground_textures.get(file) else { continue };
        let name = texture_name_of(file);
        let improved = override_dir.and_then(|d| override_path(d, &name, area.area));
        let tex = match &improved {
            Some(p) => match load_png_rgba(p) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("terrain splat: improved layer {} unreadable ({e}); using the original", p.display());
                    original.clone()
                }
            },
            None => original.clone(),
        };
        if tex.width == 0 || tex.height == 0 || tex.rgba.len() != (tex.width * tex.height * 4) as usize {
            continue;
        }
        let tiling = material.splat_tiling.get(k).map(|s| s[0]).unwrap_or(1.0);
        let info = SlotInfo {
            slot: k,
            cliff: is_cliff_layer(&name),
            texture_name: name,
            improved,
            source_size: (tex.width, tex.height),
            tiling,
            mean_luminance: 0.0,
            coverage: (cover[k] / cover_total) as f32,
        };
        sources[k] = Some((info, tex));
    }
    if sources.iter().all(Option::is_none) {
        return Err("no ground layer resolved".into());
    }

    // Albedo array: every slot (an empty slot gets a mid-grey layer it never draws).
    let largest = sources.iter().flatten().map(|(_, t)| t.width.max(t.height)).max().unwrap_or(1);
    let size = layer_size.clamp(LAYER_SIZE_RANGE.0, LAYER_SIZE_RANGE.1).min(largest.next_power_of_two()).max(1);
    let mut float_layers = Vec::with_capacity(SLOTS);
    let mut slots = Vec::with_capacity(SLOTS);
    for src in sources {
        match src {
            Some((mut info, tex)) => {
                let px = linear_rgba(&tex);
                let n = px.len().max(1) as f32;
                let mean = px.iter().fold([0f32; 3], |a, p| [a[0] + p[0], a[1] + p[1], a[2] + p[2]]);
                info.mean_luminance = (0.2126 * mean[0] + 0.7152 * mean[1] + 0.0722 * mean[2]) / n;
                float_layers.push((tex.width as usize, tex.height as usize, px));
                slots.push(Some(info));
            }
            None => {
                float_layers.push((1, 1, vec![[0.2, 0.2, 0.2, 1.0]]));
                slots.push(None);
            }
        }
    }
    let albedo = build_array(&float_layers, size, Encode::Srgb);
    drop(float_layers);

    // Normal array (best effort: an unreadable pack only turns the layer normals off).
    let normals = match resolve_normal_layers(area) {
        Ok(list) if !list.is_empty() => {
            let nsize = list.iter().map(|l| l.width.max(l.height)).max().unwrap_or(1).next_power_of_two().min(size);
            let mut layers: Vec<(usize, usize, Vec<[f32; 4]>)> = (0..SLOTS).map(|_| (1, 1, vec![[0.0, 0.0, 1.0, 1.0]])).collect();
            for l in list {
                if l.slot < SLOTS {
                    let px = l.xy.iter().map(|xy| {
                        let n = unit_normal(xy[0], xy[1]);
                        [n[0], n[1], n[2], 1.0]
                    });
                    layers[l.slot] = (l.width as usize, l.height as usize, px.collect());
                }
            }
            Some(build_array(&layers, nsize, Encode::Normal))
        }
        Ok(_) => None,
        Err(e) => {
            eprintln!("terrain splat: layer normals unavailable for area {:02} ({e}); using the baked normal map", area.area);
            None
        }
    };

    let ssize = (area.splat_alpha.width, area.splat_alpha.height);
    let control = [control_array(Some(&area.splat_alpha), ssize), control_array(area.splat_alpha1.as_ref(), ssize)];
    Ok(SplatBuild {
        area: area.area,
        extent: area.world_extent[0],
        slots,
        albedo,
        normals,
        control,
        build_ms: t0.elapsed().as_secs_f64() * 1000.0,
    })
}

// --- Shader-mirroring helpers (unit-tested; keep in sync with terrain_splat.wgsl) -----------------

/// Normalised splat weights: masked by `enabled`, divided by their sum; nothing left -> the first
/// enabled slot gets everything.
pub fn normalized_weights(raw: [f32; SLOTS], enabled: [bool; SLOTS]) -> [f32; SLOTS] {
    let mut w = [0f32; SLOTS];
    for k in 0..SLOTS {
        w[k] = if enabled[k] { raw[k].max(0.0) } else { 0.0 };
    }
    let sum: f32 = w.iter().sum();
    if sum < 1e-4 {
        let mut out = [0f32; SLOTS];
        if let Some(k) = enabled.iter().position(|e| *e) {
            out[k] = 1.0;
        }
        return out;
    }
    w.map(|v| v / sum)
}

/// Steep-face override: blend towards the cliff slots by `steep` (0..1).
pub fn steep_override(w: [f32; SLOTS], cliff: [bool; SLOTS], steep: f32) -> [f32; SLOTS] {
    let count = cliff.iter().filter(|c| **c).count() as f32;
    if steep <= 0.0 || count == 0.0 {
        return w;
    }
    let cliff_sum: f32 = (0..SLOTS).filter(|k| cliff[*k]).map(|k| w[k]).sum();
    let mut out = w;
    for k in 0..SLOTS {
        let c = if cliff[k] { 1.0 } else { 0.0 };
        let target = if cliff_sum > 0.01 { w[k] * c / cliff_sum } else { c / count };
        out[k] = w[k] + (target - w[k]) * steep;
    }
    out
}

/// Height-like blend: `score = w + h * strength`; layers within `depth` of the best score keep
/// `score - (best - depth)`, renormalised. `h` in [-1, 1].
pub fn height_blend(w: [f32; SLOTS], h: [f32; SLOTS], strength: f32, depth: f32) -> [f32; SLOTS] {
    const MIN_WEIGHT: f32 = 0.004;
    let active = |k: usize| w[k] > MIN_WEIGHT;
    let score: Vec<f32> = (0..SLOTS).map(|k| w[k] + h[k].clamp(-1.0, 1.0) * strength).collect();
    let best = (0..SLOTS).filter(|k| active(*k)).map(|k| score[k]).fold(f32::MIN, f32::max);
    let mut b = [0f32; SLOTS];
    for k in 0..SLOTS {
        if active(k) {
            b[k] = if strength > 0.0 { (score[k] - (best - depth)).max(0.0) } else { w[k] };
        }
    }
    let sum: f32 = b.iter().sum();
    if sum < 1e-6 {
        return w;
    }
    b.map(|v| v / sum)
}

/// Which slots are drawn: present, and among the `max_layers` most covering ones.
pub fn enabled_slots(slots: &[Option<SlotInfo>], max_layers: u8) -> [bool; SLOTS] {
    let mut order: Vec<(usize, f32)> = slots.iter().flatten().map(|s| (s.slot, s.coverage)).collect();
    order.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut out = [false; SLOTS];
    for (k, _) in order.into_iter().take(max_layers.clamp(1, SLOTS as u8) as usize) {
        if k < SLOTS {
            out[k] = true;
        }
    }
    out
}

// --- Material ------------------------------------------------------------------------------------

/// GPU uniform; keep the field order in sync with `TerrainSplatUniform` in the shader (vec4 only).
#[derive(ShaderType, Clone, Copy, Debug, PartialEq, Default)]
pub struct TerrainSplatUniform {
    pub rep_lo: Vec4,
    pub rep_hi: Vec4,
    pub cliff_lo: Vec4,
    pub cliff_hi: Vec4,
    pub enabled_lo: Vec4,
    pub enabled_hi: Vec4,
    pub mean_lum_lo: Vec4,
    pub mean_lum_hi: Vec4,
    /// x: extent (m), y: fade start, z: fade end.
    pub area: Vec4,
    /// x: height depth, y: height strength, z: steep start, w: steep end.
    pub blend: Vec4,
    /// x: triplanar, y: anti-tiling, z: layer normals, w: normal strength.
    pub flags: Vec4,
    /// x: brightness variation, y: splat warp (m), z: anti-tiling frequency, w: debug view.
    pub extra: Vec4,
}

fn split8(v: [f32; SLOTS]) -> (Vec4, Vec4) {
    (Vec4::new(v[0], v[1], v[2], v[3]), Vec4::new(v[4], v[5], v[6], v[7]))
}

/// The uniform of an area's build under `settings`.
pub fn splat_uniform(build: &SplatBuild, settings: &TerrainSplatSettings) -> TerrainSplatUniform {
    let mut rep = [0f32; SLOTS];
    let mut cliff = [0f32; SLOTS];
    let mut lum = [0.2f32; SLOTS];
    for s in build.slots.iter().flatten() {
        rep[s.slot] = repeats_per_metre_with(s.tiling, settings.tile_divisor);
        cliff[s.slot] = if s.cliff { 1.0 } else { 0.0 };
        lum[s.slot] = s.mean_luminance;
    }
    let enabled = enabled_slots(&build.slots, settings.max_layers).map(|e| if e { 1.0 } else { 0.0 });
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    let (rep_lo, rep_hi) = split8(rep);
    let (cliff_lo, cliff_hi) = split8(cliff);
    let (enabled_lo, enabled_hi) = split8(enabled);
    let (mean_lum_lo, mean_lum_hi) = split8(lum);
    let fade_end = settings.fade_end.max(settings.fade_start + 1.0);
    TerrainSplatUniform {
        rep_lo,
        rep_hi,
        cliff_lo,
        cliff_hi,
        enabled_lo,
        enabled_hi,
        mean_lum_lo,
        mean_lum_hi,
        area: Vec4::new(build.extent, settings.fade_start.max(0.0), fade_end, 0.0),
        blend: Vec4::new(HEIGHT_BLEND_DEPTH, settings.height_strength.max(0.0), STEEP_START, STEEP_END),
        flags: Vec4::new(
            flag(settings.triplanar),
            flag(settings.anti_tiling),
            flag(settings.layer_normals && build.normals.is_some()),
            NORMAL_STRENGTH,
        ),
        extra: Vec4::new(
            BRIGHTNESS_VARIATION,
            SPLAT_WARP_M,
            ANTI_TILING_FREQ,
            match settings.debug_view {
                TerrainDebugView::Off => 0.0,
                TerrainDebugView::Weights => 1.0,
                TerrainDebugView::Detail => 2.0,
            },
        ),
    }
}

/// Extension data of the terrain material (bindings 100..107, see the shader).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[uniform(100, TerrainSplatUniform)]
pub struct TerrainSplatExt {
    #[reflect(ignore)]
    pub params: TerrainSplatUniform,
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    pub layers: Handle<Image>,
    #[texture(103, dimension = "2d_array")]
    #[sampler(104)]
    pub normals: Handle<Image>,
    #[texture(105)]
    #[sampler(106)]
    pub control0: Handle<Image>,
    #[texture(107)]
    pub control1: Handle<Image>,
    /// Kept so live setting changes can rebuild the uniform (not uploaded).
    #[reflect(ignore)]
    pub build: Option<Arc<SplatBuildInfo>>,
    /// Cliff / rock objects: every face takes the cliff layers (not uploaded, see [`cliff_params`]).
    #[reflect(ignore)]
    pub force_cliff: bool,
}

/// The per-slot part of a [`SplatBuild`] (no pixel data), kept on the material.
#[derive(Debug, Clone)]
pub struct SplatBuildInfo {
    pub area: u8,
    pub extent: f32,
    pub slots: Vec<Option<SlotInfo>>,
    pub has_normals: bool,
}

impl SplatBuildInfo {
    fn as_build(&self) -> SplatBuild {
        let empty = LayerArray { size: 1, layers: 1, mip_levels: 1, data: vec![0; 4] };
        SplatBuild {
            area: self.area,
            extent: self.extent,
            slots: self.slots.clone(),
            albedo: empty.clone(),
            normals: self.has_normals.then(|| empty.clone()),
            control: [empty.clone(), empty],
            build_ms: 0.0,
        }
    }
}

impl From<&TerrainSplatExt> for TerrainSplatUniform {
    fn from(ext: &TerrainSplatExt) -> Self {
        ext.params
    }
}

impl MaterialExtension for TerrainSplatExt {
    fn fragment_shader() -> ShaderRef {
        TERRAIN_SPLAT_SHADER_PATH.into()
    }
}

/// The terrain material: the viewer's terrain `StandardMaterial` (baked macro albedo + baked
/// normal map, roughness) + [`TerrainSplatExt`].
pub type TerrainSplatMaterial = ExtendedMaterial<StandardMaterial, TerrainSplatExt>;

fn sampler(address: ImageAddressMode, anisotropy: u16) -> ImageSampler {
    let mut d = ImageSamplerDescriptor {
        address_mode_u: address,
        address_mode_v: address,
        address_mode_w: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    };
    if anisotropy > 1 {
        d.set_anisotropic_filter(anisotropy);
    }
    ImageSampler::Descriptor(d)
}

/// A Bevy image of a [`LayerArray`]: `D2` texture with `layers` array layers and every mip,
/// viewed as a `D2Array` (also for a single layer, as the shader binding requires).
pub fn array_image(a: &LayerArray, format: TextureFormat, image_sampler: ImageSampler, as_array: bool) -> Image {
    let mut img = Image::new_uninit(
        Extent3d { width: a.size, height: a.size, depth_or_array_layers: a.layers },
        TextureDimension::D2,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.texture_descriptor.mip_level_count = a.mip_levels;
    img.data = Some(a.data.clone());
    img.sampler = image_sampler;
    if as_array {
        img.texture_view_descriptor = Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2Array),
            ..default()
        });
    }
    img
}

/// Turn a finished build into the material (adds the images).
pub fn splat_material(
    build: &SplatBuild,
    base: StandardMaterial,
    settings: &TerrainSplatSettings,
    images: &mut Assets<Image>,
) -> TerrainSplatMaterial {
    let aniso = settings.anisotropy.clamp(1, 16);
    let layers = images.add(array_image(&build.albedo, TextureFormat::Rgba8UnormSrgb, sampler(ImageAddressMode::Repeat, aniso), true));
    let flat = LayerArray { size: 1, layers: 1, mip_levels: 1, data: vec![128, 128, 255, 255] };
    let normals = images.add(array_image(
        build.normals.as_ref().unwrap_or(&flat),
        TextureFormat::Rgba8Unorm,
        sampler(ImageAddressMode::Repeat, aniso),
        true,
    ));
    let control = |a: &LayerArray| array_image(a, TextureFormat::Rgba8Unorm, sampler(ImageAddressMode::ClampToEdge, 1), false);
    let control0 = images.add(control(&build.control[0]));
    let control1 = images.add(control(&build.control[1]));
    TerrainSplatMaterial {
        base,
        extension: TerrainSplatExt {
            params: splat_uniform(build, settings),
            layers,
            normals,
            control0,
            control1,
            build: Some(Arc::new(SplatBuildInfo {
                area: build.area,
                extent: build.extent,
                slots: build.slots.clone(),
                has_normals: build.normals.is_some(),
            })),
            force_cliff: false,
        },
    }
}

/// Uniform of a cliff / rock object: the steep-face thresholds are pushed below zero so the
/// "steep" factor is 1 on every face and the cliff layers draw the whole object (its own baked
/// 256 px texture, stretched over 15 m of rock, is only the far-distance macro colour).
pub fn cliff_params(mut p: TerrainSplatUniform) -> TerrainSplatUniform {
    p.blend.z = -1.0;
    p.blend.w = -0.5;
    p
}

/// A placed model that is a cliff face or a rock (`ground_*cliff*`, `ground_*rock*`, mountains).
/// Water, waterfalls and rock bridges keep their own material.
pub fn is_cliff_model(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with("ground_")
        && (n.contains("cliff") || n.contains("rock") || n.contains("mountain"))
        && !n.contains("water")
        && !n.contains("bridge")
}

/// Marks the mesh entity of a placed cliff / rock model; the splat plugin gives it the terrain
/// material with the cliff layers projected in world space (no UV stretching).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct CliffSurface;

// --- Runtime -------------------------------------------------------------------------------------

/// Set on a terrain chunk drawn with the splat material; `base` is its baked material, restored
/// by `--terrain bake` / a rebuild.
#[derive(Component, Debug, Clone)]
pub struct TerrainSplatApplied {
    pub base: Handle<StandardMaterial>,
}

type BuildTask = Task<Result<SplatBuild, String>>;

/// Built and pending splat materials, keyed by the chunks' baked material.
#[derive(Resource, Default)]
pub struct TerrainSplatCache {
    ready: HashMap<AssetId<StandardMaterial>, Handle<TerrainSplatMaterial>>,
    /// Cliff-object materials, keyed by the object's own baked material.
    cliff_ready: HashMap<AssetId<StandardMaterial>, Handle<TerrainSplatMaterial>>,
    failed: HashSet<AssetId<StandardMaterial>>,
    pending: Vec<(AssetId<StandardMaterial>, BuildTask)>,
    built_with: (u32, u16),
    /// Lines logged (one per area build), for tests and the automation log.
    pub log: Vec<String>,
}

impl TerrainSplatCache {
    /// Splat material built for the baked material `base`, if any.
    pub fn material_for(&self, base: AssetId<StandardMaterial>) -> Option<&Handle<TerrainSplatMaterial>> {
        self.ready.get(&base)
    }

    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}

/// Override directory used by the builds (resolved once).
#[derive(Resource, Debug, Clone, Default)]
pub struct TerrainOverrideDir(pub Option<PathBuf>);

#[allow(clippy::type_complexity)]
fn queue_builds(
    settings: Res<TerrainSplatSettings>,
    terrain: Option<Res<TerrainResource>>,
    overrides: Res<TerrainOverrideDir>,
    mut cache: ResMut<TerrainSplatCache>,
    chunks: Query<&MeshMaterial3d<StandardMaterial>, (With<TerrainChunkMarker>, Without<TerrainSplatApplied>)>,
) {
    if settings.mode != TerrainMode::Splat {
        return;
    }
    let Some(terrain) = terrain else { return };
    let mut seen = HashSet::new();
    for mat in &chunks {
        let id = mat.0.id();
        if !seen.insert(id) || cache.ready.contains_key(&id) || cache.failed.contains(&id) || cache.pending.iter().any(|(p, _)| *p == id) {
            continue;
        }
        let area = Arc::clone(&terrain.area);
        let dir = overrides.0.clone();
        let size = settings.layer_size;
        let task = AsyncComputeTaskPool::get_or_init(Default::default)
            .spawn(async move { build_splat_data(&area, dir.as_deref(), size) });
        cache.pending.push((id, task));
    }
}

fn finish_builds(
    settings: Res<TerrainSplatSettings>,
    mut cache: ResMut<TerrainSplatCache>,
    std_materials: Res<Assets<StandardMaterial>>,
    mut splat_materials: ResMut<Assets<TerrainSplatMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut i = 0;
    while i < cache.pending.len() {
        let Some(result) = future::block_on(future::poll_once(&mut cache.pending[i].1)) else {
            i += 1;
            continue;
        };
        let (id, _) = cache.pending.swap_remove(i);
        let base = std_materials.get(id).cloned();
        match (result, base) {
            (Ok(build), Some(base)) => {
                let mat = splat_material(&build, base, &settings, &mut images);
                let improved = build.slots.iter().flatten().filter(|s| s.improved.is_some()).count();
                let cliffs: Vec<usize> = build.slots.iter().flatten().filter(|s| s.cliff).map(|s| s.slot).collect();
                let line = format!(
                    "Terrain: real-time splat material for area {:02}: {} layers at {} px ({} improved), normals {}, cliff slots {:?}, built in {:.0} ms",
                    build.area,
                    build.slots.iter().flatten().count(),
                    build.albedo.size,
                    improved,
                    build.normals.as_ref().map_or("baked".to_string(), |n| format!("{} px per layer", n.size)),
                    cliffs,
                    build.build_ms
                );
                println!("{line}");
                cache.log.push(line);
                let h = splat_materials.add(mat);
                cache.ready.insert(id, h);
                cache.cliff_ready.clear();
            }
            (Err(e), _) => {
                let line = format!("Terrain: splat material unavailable ({e}); keeping the baked ground");
                eprintln!("{line}");
                cache.log.push(line);
                cache.failed.insert(id);
            }
            (Ok(_), None) => {
                // The baked material is gone (zone changed meanwhile): nothing to swap.
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn apply_splat(
    mut commands: Commands,
    settings: Res<TerrainSplatSettings>,
    mut cache: ResMut<TerrainSplatCache>,
    chunks: Query<(Entity, &MeshMaterial3d<StandardMaterial>), (With<TerrainChunkMarker>, Without<TerrainSplatApplied>)>,
    applied: Query<&TerrainSplatApplied>,
) {
    if settings.mode != TerrainMode::Splat {
        return;
    }
    let mut swapped = false;
    for (e, mat) in &chunks {
        if let Some(h) = cache.ready.get(&mat.0.id()) {
            commands
                .entity(e)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert((MeshMaterial3d(h.clone()), TerrainSplatApplied { base: mat.0.clone() }));
            swapped = true;
        }
    }
    if swapped {
        // Drop the materials of areas no chunk uses any more (zone changes).
        let mut live: HashSet<AssetId<StandardMaterial>> = applied.iter().map(|a| a.base.id()).collect();
        live.extend(chunks.iter().map(|(_, m)| m.0.id()));
        cache.ready.retain(|id, _| live.contains(id));
        cache.failed.retain(|id| live.contains(id));
    }
}

/// Placed cliff / rock models get the terrain material (cliff layers, world-space triplanar) as
/// soon as the area's layers are built; their own baked texture stays as the far macro colour.
#[allow(clippy::type_complexity)]
fn apply_cliffs(
    mut commands: Commands,
    settings: Res<TerrainSplatSettings>,
    mut cache: ResMut<TerrainSplatCache>,
    std_materials: Res<Assets<StandardMaterial>>,
    mut splat_materials: ResMut<Assets<TerrainSplatMaterial>>,
    objects: Query<(Entity, &MeshMaterial3d<StandardMaterial>), (With<CliffSurface>, Without<TerrainSplatApplied>)>,
) {
    if settings.mode != TerrainMode::Splat || objects.is_empty() {
        return;
    }
    let Some(template) = cache.ready.values().next().and_then(|h| splat_materials.get(h)).map(|m| m.extension.clone()) else {
        return;
    };
    for (e, mat) in &objects {
        let id = mat.0.id();
        let handle = match cache.cliff_ready.get(&id) {
            Some(h) => h.clone(),
            None => {
                let Some(base) = std_materials.get(id).cloned() else { continue };
                let mut extension = template.clone();
                extension.force_cliff = true;
                extension.params = cliff_params(extension.params);
                let h = splat_materials.add(TerrainSplatMaterial { base, extension });
                cache.cliff_ready.insert(id, h.clone());
                h
            }
        };
        commands
            .entity(e)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert((MeshMaterial3d(handle), TerrainSplatApplied { base: mat.0.clone() }));
    }
}

/// `--terrain bake` (live) or a rebuild: give every splat chunk its baked material back.
fn revert_to_bake(commands: &mut Commands, applied: &Query<(Entity, &TerrainSplatApplied)>) {
    for (e, a) in applied.iter() {
        commands
            .entity(e)
            .remove::<(MeshMaterial3d<TerrainSplatMaterial>, TerrainSplatApplied)>()
            .insert(MeshMaterial3d(a.base.clone()));
    }
}

/// Live settings: rewrite the uniform; a mode change reverts/re-applies; a layer size or
/// anisotropy change rebuilds.
fn apply_settings(
    mut commands: Commands,
    settings: Res<TerrainSplatSettings>,
    mut cache: ResMut<TerrainSplatCache>,
    mut materials: ResMut<Assets<TerrainSplatMaterial>>,
    applied: Query<(Entity, &TerrainSplatApplied)>,
) {
    if !settings.is_changed() {
        return;
    }
    if cache.built_with != settings.build_key() || settings.mode == TerrainMode::Bake {
        if cache.built_with != settings.build_key() {
            cache.ready.clear();
            cache.cliff_ready.clear();
            cache.failed.clear();
            cache.pending.clear();
            cache.built_with = settings.build_key();
        }
        revert_to_bake(&mut commands, &applied);
        return;
    }
    for (_, mat) in materials.iter_mut() {
        if let Some(info) = mat.extension.build.clone() {
            mat.extension.params = splat_uniform(&info.as_build(), &settings);
            if mat.extension.force_cliff {
                mat.extension.params = cliff_params(mat.extension.params);
            }
        }
    }
}

/// Registers the logic only (no render pipeline): used by [`TerrainSplatPlugin`] and the tests.
pub struct TerrainSplatLogicPlugin {
    pub settings: TerrainSplatSettings,
    pub override_dir: Option<PathBuf>,
}

impl Plugin for TerrainSplatLogicPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.settings)
            .insert_resource(TerrainOverrideDir(self.override_dir.clone()))
            .insert_resource(TerrainSplatCache { built_with: self.settings.build_key(), ..default() })
            .add_systems(Update, (apply_settings, queue_builds, finish_builds, apply_splat, apply_cliffs).chain());
    }
}

/// Real-time terrain material: the shader, the material type and the chunk material swap.
pub struct TerrainSplatPlugin {
    pub settings: TerrainSplatSettings,
}

impl TerrainSplatPlugin {
    pub fn from_args(args: &[String]) -> Self {
        Self { settings: TerrainSplatSettings::from_args(args) }
    }
}

impl Plugin for TerrainSplatPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "terrain_splat.wgsl");
        app.add_plugins(MaterialPlugin::<TerrainSplatMaterial>::default());
        let override_dir = default_override_dir();
        match self.settings.mode {
            TerrainMode::Splat => println!(
                "Terrain: real-time splat material (--terrain splat; --terrain bake for the single baked image); improved layers from {}",
                override_dir.as_ref().map_or("<none>".to_string(), |d| d.display().to_string())
            ),
            TerrainMode::Bake => println!("Terrain: baked single-image ground (--terrain bake)"),
        }
        app.add_plugins(TerrainSplatLogicPlugin { settings: self.settings, override_dir });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::bntx::FloatTextureData;
    use crate::assets::terrain::{TerrainMaterial, TerrainMeshSettings};
    use crate::assets::tr::TrMtr;

    fn solid(w: u32, h: u32, c: [u8; 3]) -> TextureData {
        TextureData { width: w, height: h, rgba: (0..w * h).flat_map(|_| [c[0], c[1], c[2], 255]).collect() }
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    /// Area with layers in slots 0 (cliff, 4x4 red), 1 (grass, 8x8 green), 4 (sand, 2x2 blue);
    /// the control map puts slot 1 everywhere except the left column (slot 0).
    fn area() -> TerrainArea {
        let mut g = std::collections::HashMap::new();
        g.insert("ha_t_cliff01_alb.bntx".to_string(), solid(4, 4, [200, 0, 0]));
        g.insert("ha_t_grass01_alb.bntx".to_string(), solid(8, 8, [0, 180, 0]));
        g.insert("ha_t_sand01_alb.bntx".to_string(), solid(2, 2, [0, 0, 220]));
        let mut s0 = Vec::new();
        for _y in 0..4 {
            for x in 0..4 {
                s0.extend_from_slice(if x == 0 { &[255, 0, 0, 0] } else { &[0, 255, 0, 0] });
            }
        }
        TerrainArea {
            area: 7,
            source_path: PathBuf::from("/nonexistent/ha_area07.gfpak"),
            world_extent: [100.0, 100.0],
            world_height: 1.0,
            heightmap_brti_name: "h".into(),
            navmesh_bounds: None,
            heightmap: FloatTextureData { width: 2, height: 2, samples: vec![0.0; 4] },
            normal_map: solid(1, 1, [0, 0, 0]),
            splat_alpha: TextureData { width: 4, height: 4, rgba: s0 },
            splat_alpha1: Some(TextureData { width: 4, height: 4, rgba: vec![0; 64] }),
            material: Some(TerrainMaterial {
                slots: TrMtr {
                    textures: vec![
                        ("ha_t_cliff01_alb.bntx".into(), "SplatColor0".into()),
                        ("ha_t_grass01_alb.bntx".into(), "SplatColor1".into()),
                        ("ha_t_sand01_alb.bntx".into(), "SplatColor4".into()),
                    ],
                    flags: vec![],
                },
                parameter_candidates: vec![],
                splat_tiling: vec![[4.0, 4.0], [5.0, 5.0], [0.0, 0.0], [0.0, 0.0], [2.5, 2.5]],
            }),
            ground_textures: g,
            unresolved_ground_textures: vec![],
            masks: vec![],
            instances: vec![],
        }
    }

    #[test]
    fn mip_count_matches_the_chain() {
        assert_eq!(mip_count(1), 1);
        assert_eq!(mip_count(2), 2);
        assert_eq!(mip_count(256), 9);
        assert_eq!(mip_count(1024), 11);
        assert_eq!(mip_count(4096), 13);
    }

    #[test]
    fn layer_array_has_every_slot_and_mip_and_keeps_colours() {
        let b = build_splat_data(&area(), None, 1024).unwrap();
        // Capped at the largest source (8 px), all 8 slots, full chain.
        assert_eq!((b.albedo.size, b.albedo.layers, b.albedo.mip_levels), (8, 8, 4));
        assert_eq!(b.albedo.layer_bytes(), (64 + 16 + 4 + 1) * 4);
        assert_eq!(b.albedo.data.len(), b.albedo.layer_bytes() * 8);
        // Solid colours survive the resize (4->8, 2->8) and the mip chain, in sRGB.
        let near = |a: [u8; 4], c: [u8; 3]| (0..3).all(|i| (a[i] as i32 - c[i] as i32).abs() <= 1);
        assert!(near(b.albedo.texel(0, 3, 5), [200, 0, 0]));
        assert!(near(b.albedo.texel(1, 7, 0), [0, 180, 0]));
        assert!(near(b.albedo.texel(4, 0, 7), [0, 0, 220]));
        let last_mip = b.albedo.layer_bytes() - 4;
        assert!(near(b.albedo.data[b.albedo.layer_bytes() + last_mip..][..4].try_into().unwrap(), [0, 180, 0]));
        // Slot info: cliff by name, coverage from the control map, empty slots None.
        let s0 = b.slots[0].as_ref().unwrap();
        assert!(s0.cliff && !b.slots[1].as_ref().unwrap().cliff);
        assert!((s0.coverage - 0.25).abs() < 1e-3, "{}", s0.coverage);
        assert!((b.slots[1].as_ref().unwrap().coverage - 0.75).abs() < 1e-3);
        assert!(b.slots[2].is_none() && b.slots[4].is_some());
        // No pack on disk: layer normals are off, not an error.
        assert!(b.normals.is_none());
        assert_eq!((b.control[0].size, b.control[0].mip_levels), (4, 3));
    }

    #[test]
    fn improved_layer_wins_and_sets_the_size() {
        let dir = std::env::temp_dir().join(format!("pla_splat_override_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        solid(128, 128, [250, 250, 0]).save_png(&dir.join("ha_t_grass01_alb.png")).unwrap();
        let b = build_splat_data(&area(), Some(&dir), 64).unwrap();
        assert_eq!(b.albedo.size, 64, "capped by the setting, not the 128 px source");
        let g = b.slots[1].as_ref().unwrap();
        assert_eq!(g.source_size, (128, 128));
        assert!(g.improved.is_some());
        assert_eq!(&b.albedo.texel(1, 0, 0)[..3], &[250, 250, 0]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn resampling_wraps_and_keeps_the_mean() {
        // 2-texel ramp grown to 4: the edge texels blend with the opposite side (wrap).
        let px = vec![[0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0]];
        let up = resample_wrap(&px, 2, 1, 4);
        assert_eq!(up.len(), 16);
        assert!((up[0][0] - 0.25).abs() < 1e-5 && (up[3][0] - 0.75).abs() < 1e-5, "{up:?}");
        // 5 -> 4 box filter keeps the mean of a row.
        let row: Vec<[f32; 4]> = (0..25).map(|i| [((i % 5) as f32) / 4.0, 0.0, 0.0, 1.0]).collect();
        let down = resample_wrap(&row, 5, 5, 4);
        let mean = down.iter().map(|p| p[0]).sum::<f32>() / 16.0;
        assert!((mean - 0.5).abs() < 1e-4, "{mean}");
        for taps in resample_taps(1254, 1024) {
            assert!((taps.iter().map(|t| t.1).sum::<f32>() - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn tiling_math_and_uniform() {
        assert!((repeats_per_metre(5.0) - 5.0 / DETAIL_TILE_DIVISOR).abs() < 1e-7);
        assert!(
            (repeats_per_metre(0.0) - 1.0 / DETAIL_TILE_DIVISOR).abs() < 1e-7,
            "a missing ST falls back to 1"
        );
        assert!((repeats_per_metre_with(2.5, 8.0) - 2.5 / 8.0).abs() < 1e-7);
        assert!(
            (repeats_per_metre_with(2.5, 0.0) - 2.5 / DETAIL_TILE_DIVISOR).abs() < 1e-7,
            "a bad divisor falls back to the default"
        );
        let b = build_splat_data(&area(), None, 64).unwrap();
        let u = splat_uniform(&b, &TerrainSplatSettings::default());
        assert_eq!(
            u.rep_lo,
            Vec4::new(
                4.0 / DETAIL_TILE_DIVISOR,
                5.0 / DETAIL_TILE_DIVISOR,
                0.0,
                0.0
            )
        );
        assert_eq!(u.rep_hi.x, 2.5 / DETAIL_TILE_DIVISOR);
        assert_eq!(u.cliff_lo, Vec4::new(1.0, 0.0, 0.0, 0.0));
        assert_eq!((u.enabled_lo, u.enabled_hi), (Vec4::new(1.0, 1.0, 0.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 0.0)));
        assert_eq!(u.area.x, 100.0);
        assert_eq!(u.flags.z, 0.0, "no normal layers -> baked normals");
        // Layer-count limit keeps the most covering slots (grass 75 %, cliff 25 %, sand 0 %).
        let two = splat_uniform(&b, &TerrainSplatSettings { max_layers: 1, triplanar: false, ..default() });
        assert_eq!((two.enabled_lo, two.enabled_hi), (Vec4::new(0.0, 1.0, 0.0, 0.0), Vec4::ZERO));
        assert_eq!(two.flags.x, 0.0);
        let fine = splat_uniform(&b, &TerrainSplatSettings { tile_divisor: 8.0, ..default() });
        assert_eq!(fine.rep_lo.y, 5.0 / 8.0);
        assert!(std::mem::size_of::<TerrainSplatUniform>() == 12 * 16);
    }

    #[test]
    fn splat_weights_normalise_and_respect_the_mask() {
        let all = [true; SLOTS];
        let w = normalized_weights([100.0, 50.0, 0.0, 0.0, 50.0, 0.0, 0.0, 30.0], all);
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        assert!((w[0] - 100.0 / 230.0).abs() < 1e-6);
        let mut mask = all;
        mask[0] = false;
        let m = normalized_weights([100.0, 50.0, 0.0, 0.0, 50.0, 0.0, 0.0, 30.0], mask);
        assert_eq!(m[0], 0.0);
        assert!((m.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        let z = normalized_weights([0.0; SLOTS], mask);
        assert_eq!(z[1], 1.0, "nothing -> first enabled slot");
        // Steep override: half way hands half the weight to the cliff slots, keeping their split.
        let mut cliff = [false; SLOTS];
        cliff[0] = true;
        cliff[1] = true;
        let o = steep_override([0.2, 0.2, 0.6, 0.0, 0.0, 0.0, 0.0, 0.0], cliff, 1.0);
        assert!((o[0] - 0.5).abs() < 1e-6 && (o[1] - 0.5).abs() < 1e-6 && o[2].abs() < 1e-6);
        let none = steep_override([0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0], cliff, 0.5);
        assert!((none[0] - 0.25).abs() < 1e-6 && (none[2] - 0.5).abs() < 1e-6);
        assert!((none.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        // Height blend: sums to 1, a zero weight stays zero, the brighter detail wins a tie.
        let hb = height_blend([0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], [0.8, -0.8, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0], 0.35, 0.25);
        assert!((hb.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        assert_eq!(hb[2], 0.0);
        assert!(hb[0] > 0.99, "{hb:?}");
        let plain = height_blend([0.7, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], [0.0; SLOTS], 0.0, 0.25);
        assert!((plain[0] - 0.7).abs() < 1e-6);
    }

    #[test]
    fn cli_flags_parse() {
        let d = TerrainSplatSettings::from_args(&s(&["v"]));
        assert_eq!(d, TerrainSplatSettings::default());
        assert_eq!(d.mode, TerrainMode::Splat);
        let b = TerrainSplatSettings::from_args(&s(&[
            "v", "--terrain", "bake", "--terrain-layer-size", "512", "--terrain-layers", "4", "--terrain-triplanar", "off",
            "--terrain-antitile=off", "--terrain-normals", "baked", "--terrain-debug", "weights", "--terrain-tile-divisor", "8",
        ]));
        assert_eq!(b.tile_divisor, 8.0);
        assert_eq!(b.mode, TerrainMode::Bake);
        assert_eq!((b.layer_size, b.max_layers, b.triplanar, b.anti_tiling, b.layer_normals), (512, 4, false, false, false));
        assert_eq!(b.debug_view, TerrainDebugView::Weights);
        let bad = TerrainSplatSettings::from_args(&s(&["v", "--terrain", "nope", "--terrain-layer-size", "1000", "--terrain-layers", "9"]));
        assert_eq!((bad.mode, bad.layer_size, bad.max_layers), (TerrainMode::Splat, DEFAULT_LAYER_SIZE, 8));
    }

    /// The library part of the shader (everything above the entry point, without the bevy
    /// imports) parses and validates with naga.
    #[test]
    fn wgsl_library_validates_with_naga() {
        let src = TERRAIN_SPLAT_WGSL;
        let start = src.find("// ---- library").expect("library marker");
        let end = src.find("// ---- entry point ----").expect("entry marker");
        let lib = src[start..end].replace("#{MATERIAL_BIND_GROUP}", "2");
        // A tiny entry point so the derivative uniformity of the call is checked too.
        let test = format!(
            "{lib}\n@fragment\nfn test_main(@builtin(position) pos: vec4<f32>, @location(0) wp: vec3<f32>, @location(1) wn: vec3<f32>) -> @location(0) vec4<f32> {{\n    let s = terrain_surface(wp, wn, vec3<f32>(0.0, 10.0, 0.0), vec3<f32>(0.3), normalize(wn));\n    return vec4<f32>(s.albedo + s.normal * 0.0, 1.0);\n}}\n"
        );
        let module = naga::front::wgsl::parse_str(&test).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&test)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&test)));
        // The full shader keeps its entry point and bindings in sync with the Rust side.
        for needle in ["@binding(100)", "@binding(107)", "fn fragment(", "texture_2d_array<f32>"] {
            assert!(src.contains(needle), "{needle}");
        }
    }

    fn logic_app(settings: TerrainSplatSettings) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>().init_asset::<Image>().init_asset::<StandardMaterial>().init_asset::<TerrainSplatMaterial>();
        app.add_plugins(TerrainSplatLogicPlugin { settings, override_dir: None });
        let a = area();
        app.insert_resource(TerrainResource {
            area: Arc::new(a),
            chunks: vec![],
            blended_albedo: None,
            settings: TerrainMeshSettings { horizontal_cell_size: 1.0, vertical_scale: 1.0, chunk_cells: 64 },
        });
        app
    }

    fn spawn_chunks(app: &mut App, n: usize) -> (Handle<StandardMaterial>, Vec<Entity>) {
        let std = app.world_mut().resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial::default());
        let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(Mesh::from(Plane3d::default()));
        let ents = (0..n)
            .map(|_| app.world_mut().spawn((TerrainChunkMarker, Mesh3d(mesh.clone()), MeshMaterial3d(std.clone()))).id())
            .collect();
        (std, ents)
    }

    fn update_until(app: &mut App, done: impl Fn(&mut App) -> bool) {
        for _ in 0..400 {
            app.update();
            if done(app) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("condition not reached");
    }

    #[test]
    fn headless_terrain_chunks_get_the_splat_material_and_bake_reverts() {
        let mut app = logic_app(TerrainSplatSettings::default());
        assert!(crate::game::schedule_check::initialize_all_schedules(&mut app).is_empty());
        let (std, ents) = spawn_chunks(&mut app, 3);
        update_until(&mut app, |app| app.world().get::<TerrainSplatApplied>(ents[2]).is_some());
        let h = app.world().get::<MeshMaterial3d<TerrainSplatMaterial>>(ents[0]).expect("splat material").0.clone();
        assert!(ents.iter().all(|e| app.world().get::<MeshMaterial3d<StandardMaterial>>(*e).is_none()));
        assert!(ents.iter().all(|e| app.world().get::<MeshMaterial3d<TerrainSplatMaterial>>(*e).unwrap().0 == h), "one shared material");
        {
            let mats = app.world().resource::<Assets<TerrainSplatMaterial>>();
            let m = mats.get(&h).unwrap();
            let imgs = app.world().resource::<Assets<Image>>();
            let layers = imgs.get(&m.extension.layers).unwrap();
            assert_eq!(layers.texture_descriptor.size.depth_or_array_layers, 8);
            assert_eq!(layers.texture_descriptor.mip_level_count, 4);
            assert_eq!(layers.texture_descriptor.format, TextureFormat::Rgba8UnormSrgb);
            assert_eq!(
                layers.texture_view_descriptor.as_ref().and_then(|v| v.dimension),
                Some(TextureViewDimension::D2Array)
            );
            let ImageSampler::Descriptor(d) = &layers.sampler else { panic!("sampler") };
            assert_eq!((d.address_mode_u, d.anisotropy_clamp), (ImageAddressMode::Repeat, 16));
            let c0 = imgs.get(&m.extension.control0).unwrap();
            assert_eq!(c0.texture_descriptor.format, TextureFormat::Rgba8Unorm);
            assert_eq!(m.extension.params.area.x, 100.0);
        }
        let log = app.world().resource::<TerrainSplatCache>().log.clone();
        assert!(log.iter().any(|l| l.contains("area 07") && l.contains("cliff slots [0]")), "{log:?}");
        // Live change: anti-tiling off rewrites the uniform.
        app.world_mut().resource_mut::<TerrainSplatSettings>().anti_tiling = false;
        app.update();
        assert_eq!(app.world().resource::<Assets<TerrainSplatMaterial>>().get(&h).unwrap().extension.params.flags.y, 0.0);
        // Bake mode: the baked material comes back.
        app.world_mut().resource_mut::<TerrainSplatSettings>().mode = TerrainMode::Bake;
        app.update();
        app.update();
        for e in &ents {
            assert_eq!(app.world().get::<MeshMaterial3d<StandardMaterial>>(*e).unwrap().0, std);
            assert!(app.world().get::<TerrainSplatApplied>(*e).is_none());
        }
    }

    #[test]
    fn bake_mode_never_touches_the_chunks_and_a_bad_area_keeps_the_bake() {
        let mut app = logic_app(TerrainSplatSettings { mode: TerrainMode::Bake, ..default() });
        let (_, ents) = spawn_chunks(&mut app, 1);
        for _ in 0..5 {
            app.update();
        }
        assert!(app.world().get::<MeshMaterial3d<StandardMaterial>>(ents[0]).is_some());
        assert!(!app.world().resource::<TerrainSplatCache>().is_pending());

        let mut app = logic_app(TerrainSplatSettings::default());
        let mut a = area();
        a.material = None;
        app.world_mut().resource_mut::<TerrainResource>().area = Arc::new(a);
        let (_, ents) = spawn_chunks(&mut app, 1);
        update_until(&mut app, |app| !app.world().resource::<TerrainSplatCache>().log.is_empty());
        for _ in 0..3 {
            app.update();
        }
        assert!(app.world().get::<MeshMaterial3d<StandardMaterial>>(ents[0]).is_some());
        assert!(app.world().resource::<TerrainSplatCache>().log[0].contains("keeping the baked ground"));
    }

    /// Real game data (skipped without the extracted game): area 01 builds 8 layers with the
    /// two cliff slots, and every improved layer present is picked up.
    #[test]
    fn real_area_01_builds_when_the_game_is_present() {
        let root = std::env::var_os("PLA_BIN_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin"));
        if !root.join("archive/field/terrain/ha_area01.gfpak").is_file() {
            eprintln!("skip: no game data");
            return;
        }
        let area = TerrainArea::load(&root, 1).unwrap();
        let dir = default_override_dir();
        let b = build_splat_data(&area, dir.as_deref(), 1024).unwrap();
        let cliffs: Vec<usize> = b.slots.iter().flatten().filter(|s| s.cliff).map(|s| s.slot).collect();
        assert_eq!(cliffs, vec![0, 1]);
        assert_eq!(b.slots.iter().flatten().count(), 8);
        assert!(b.normals.is_some());
        assert_eq!(b.control[0].size, 1024);
        let improved = b.slots.iter().flatten().filter(|s| s.improved.is_some()).count();
        println!("area 01: layer size {}, improved {improved}, normals {:?}, {:.0} ms", b.albedo.size, b.normals.as_ref().map(|n| n.size), b.build_ms);
        if dir.is_some() && improved > 0 {
            assert_eq!(b.albedo.size, 1024);
        }
    }

    #[test]
    fn cliff_and_rock_models_are_recognised_and_water_is_not() {
        for yes in ["ground_area01_cliff01", "ground_cliff09", "ground_rock04_gray", "ground_area01_comrock01", "ground_mountain01_tengan"] {
            assert!(is_cliff_model(yes), "{yes}");
        }
        for no in ["ground_cliff01_waterfall", "ground_area01_water01", "ground_rockbridge01", "objects_stonewall01", "plants_area01_tree01", "ground_stones02"] {
            assert!(!is_cliff_model(no), "{no}");
        }
    }

    #[test]
    fn cliff_params_make_every_face_steep() {
        let p = cliff_params(TerrainSplatUniform { blend: Vec4::new(0.25, 0.35, STEEP_START, STEEP_END), ..default() });
        assert!(p.blend.z < 0.0 && p.blend.w < p.blend.z.max(0.0) && p.blend.w < 0.0);
        assert_eq!((p.blend.x, p.blend.y), (0.25, 0.35), "the height blend is untouched");
    }
}
