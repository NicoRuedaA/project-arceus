//! Minimap, compass and full-screen area map (Brief 15).
//!
//! The area map is generated from data that is already in the engine: the baked terrain
//! albedo (`work/cache/terrain_albedo/areaNN.png`) plus the area heightmap, shaded with a
//! fixed hill-shading light and with the sea (height 0) painted blue. An optional
//! `--game-map` mode loads the game's own `map_lmap_pic_NN.bntx` instead (areas 00-04 and
//! 06; area 05 is split in six tiles whose tiling is unknown, so it falls back to the
//! generated map).
//!
//! # Axis / orientation convention (ASSUMED, declared explicitly)
//!
//! The world frame draws `x` along the map columns and `z` along the rows, which is the
//! convention the terrain loader already verified (`x` columns, `z` rows, no flips). The
//! generated texture therefore stores world `x` in `u` and world `z` in `v`, and a UI
//! `ImageNode` shows row 0 at the top: **`-z` is up ("north") on the minimap and `+z` is
//! down ("south")** - the same reading the existing creature radar HUD uses, where a
//! creature at `+z` is labelled `S`. The real orientation of the original game's compass is
//! unknown; see `reports/data-progress/minimap.md`.
//!
//! The compass reuses the radar's eight-sector heading calculation ([`compass_dir`]), so
//! the radar and the compass can never disagree. Note that the radar labels `+x` as `W`
//! (west); under a plain top-down reading of the surface texture `+x` would be east, so
//! east and west are mirrored relative to a geographic map. This is kept as it is (it is
//! the existing HUD behaviour) and documented as UNKNOWN in the report.
//!
//! The minimap keeps north up and rotates the player marker ([`marker_rotation_deg`]);
//! the crop window follows the player ([`crop_window`]) and the compass strip shows which
//! of the eight directions the player is facing.

use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::assets::bntx::{Bntx, FloatTextureData, TextureData};
use crate::assets::terrain::{sample_height, TerrainArea};
use crate::assets::terrain_albedo::{load_baked_albedo, BakeOutcome};
use crate::game::state::{not_paused, GameState};
use crate::game::ui::WorldHudRoot;

/// Version of the generation algorithm; part of the cache signature (2: data-driven water level).
pub const MINIMAP_VERSION: u32 = 2;

/// Default map texture size (square, texels).
pub const DEFAULT_MAP_SIZE: u32 = 1024;

/// Side of the on-screen minimap frame, logical pixels.
pub const FRAME_PX: f32 = 200.0;

/// World span the minimap crop shows around the player, metres.
pub const VIEW_METRES: f32 = 160.0;

/// Terrain within this band (metres) above the estimated water level is drawn as water.
pub const WATER_BAND_M: f32 = 0.5;

/// The water level is only painted when the most common height sits within this fraction of
/// the area's height range (otherwise a low flat *land* plateau would be painted blue).
pub const WATER_MODE_MAX_RANGE: f32 = 0.10;

/// Colour of the sea on the generated map.
pub const WATER_RGB: [u8; 3] = [42, 84, 140];

/// Shading floor: flat ground keeps this fraction of the albedo brightness.
pub const SHADE_AMBIENT: f32 = 0.55;

/// Default cache root (the main checkout's shared work dir, like the terrain bakes).
pub const DEFAULT_CACHE_ROOT: &str = "/home/nico/work/decompilacion/work/cache";

// ------------------------------------------------------------------ world <-> map

/// World `(x, z)` metres to texture `(u, v)` in `0..=1` (u = x column, v = z row).
pub fn world_to_uv(x: f32, z: f32, extent: Vec2) -> Vec2 {
    Vec2::new(x / extent.x, z / extent.y)
}

/// Inverse of [`world_to_uv`].
pub fn uv_to_world(uv: Vec2, extent: Vec2) -> Vec2 {
    Vec2::new(uv.x * extent.x, uv.y * extent.y)
}

/// Texel-centre position of world `(x, z)` in a `map_px`-square map of `extent` metres:
/// texel `i` covers `[i, i+1) * extent / map_px`.
pub fn texel_of_world(x: f32, z: f32, extent: Vec2, map_px: u32) -> Vec2 {
    Vec2::new(
        x / extent.x * map_px as f32 - 0.5,
        z / extent.y * map_px as f32 - 0.5,
    )
}

/// Crop window of `texels` centred on `center_texel`, shifted (not shrunk) to stay inside
/// the `map_px`-square texture. Returns the window centre and the pixel rect for
/// `ImageNode::rect`.
pub fn crop_window(center_texel: Vec2, texels: f32, map_px: u32) -> (Vec2, Rect) {
    let span = texels.clamp(1.0, map_px as f32);
    let half = span * 0.5;
    let center = center_texel.clamp(Vec2::splat(half), Vec2::splat(map_px as f32 - half));
    (
        center,
        Rect::new(
            center.x - half,
            center.y - half,
            center.x + half,
            center.y + half,
        ),
    )
}

/// How far off the crop centre the player is, in frame pixels (`frame_px` for `texels`).
pub fn marker_offset_px(
    player_texel: Vec2,
    window_center: Vec2,
    texels: f32,
    frame_px: f32,
) -> Vec2 {
    (player_texel - window_center) * (frame_px / texels)
}

/// Rotation (degrees, clockwise on screen) of the up-pointing player marker so that it
/// points at the world facing `(fx, fz)`: screen right is `+x`, screen down is `+z`.
pub fn marker_rotation_deg(facing_x: f32, facing_z: f32) -> f32 {
    facing_x.atan2(-facing_z).to_degrees()
}

/// Facing `(x, z)` of an avatar whose model front is `+Z` (see `player_anim::step_facing`).
pub fn facing_from_yaw(yaw_rad: f32) -> Vec2 {
    Vec2::new(yaw_rad.sin(), yaw_rad.cos())
}

// ------------------------------------------------------------------ compass

/// The eight compass directions, in the radar's reading of the world frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compass {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl Compass {
    /// All eight, clockwise from north (the order of the compass strip).
    pub const ALL: [Compass; 8] = [
        Compass::N,
        Compass::NE,
        Compass::E,
        Compass::SE,
        Compass::S,
        Compass::SW,
        Compass::W,
        Compass::NW,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Compass::N => "N",
            Compass::NE => "NE",
            Compass::E => "E",
            Compass::SE => "SE",
            Compass::S => "S",
            Compass::SW => "SW",
            Compass::W => "W",
            Compass::NW => "NW",
        }
    }
}

/// Eight-sector direction of a planar offset, reusing the radar HUD's convention:
/// `(0, +z)` is south, `(+x, 0)` is west (see the module docs).
pub fn compass_dir(dx: f32, dz: f32) -> Compass {
    // Sector index: 0 = S, +1 per 45 deg counter-clockwise towards W..N..E.
    let sector = (dx.atan2(dz) / std::f32::consts::FRAC_PI_4).round() as i32;
    match ((sector % 8) + 8) % 8 {
        0 => Compass::S,
        1 => Compass::SW,
        2 => Compass::W,
        3 => Compass::NW,
        4 => Compass::N,
        5 => Compass::NE,
        6 => Compass::E,
        _ => Compass::SE,
    }
}

/// The compass strip: all eight labels in north-up order with the facing one bracketed,
/// e.g. `N NE E [SE] S SW W NW`.
pub fn compass_line(facing: Compass) -> String {
    Compass::ALL
        .iter()
        .map(|d| {
            if *d == facing {
                format!("[{}]", d.label())
            } else {
                d.label().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ------------------------------------------------------------------ shading

/// Light direction of the generated map's relief shading: from above, screen top-left
/// (`-x`, `-z`). A presentation choice, not decoded game data.
pub fn light_dir() -> Vec3 {
    Vec3::new(-0.55, 0.70, -0.55).normalize()
}

/// Estimated water level of an area, in metres, or `None` when the area has no sea.
///
/// The water surface of the game heightmaps is a large flat region at a low constant
/// **positive** normalized height (NOT 0; measured on areas 00/01: ~0.008 and ~0.012 of the
/// range, i.e. ~1.5 m), so the level is estimated as the most populated histogram bin when
/// that bin sits in the lowest [`WATER_MODE_MAX_RANGE`] of the height range.
pub fn estimate_water_level_m(heightmap: &FloatTextureData, world_height: f32) -> Option<f32> {
    if heightmap.samples.is_empty() {
        return None;
    }
    let min = heightmap
        .samples
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min);
    let max = heightmap
        .samples
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    if !min.is_finite() || !max.is_finite() {
        return None;
    }
    if max <= min {
        return (min <= WATER_MODE_MAX_RANGE).then_some(min.max(0.0) * world_height);
    }
    const BINS: usize = 512;
    let mut buckets = vec![0u32; BINS];
    let scale = BINS as f32 / (max - min);
    for &sample in &heightmap.samples {
        let bin = ((sample - min) * scale) as usize;
        buckets[bin.min(BINS - 1)] += 1;
    }
    let (mode_bin, _) = buckets
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| **count)
        .expect("non-empty histogram");
    let mode = (min + (mode_bin as f32 + 0.5) / scale).clamp(min, max);
    let range = max - min;
    if range > 0.0 && (mode - min) / range > WATER_MODE_MAX_RANGE {
        return None;
    }
    Some(mode * world_height)
}

/// Surface normal of a height field from the four neighbour heights at `(+/-dx, +/-dz)` m.
pub fn normal_from_neighbours(hx0: f32, hx1: f32, hz0: f32, hz1: f32, dx: f32, dz: f32) -> Vec3 {
    let gx = (hx1 - hx0) / (2.0 * dx.max(1e-6));
    let gz = (hz1 - hz0) / (2.0 * dz.max(1e-6));
    Vec3::new(-gx, 1.0, -gz).normalize()
}

/// One minimap pixel: water near the area water level (blue, modulated by the albedo
/// luminance), otherwise the albedo with the hill-shading factor applied.
pub fn shade_pixel(
    albedo: [u8; 3],
    height_m: f32,
    water_level_m: Option<f32>,
    normal: Vec3,
) -> [u8; 3] {
    if water_level_m.is_some_and(|level| height_m <= level + WATER_BAND_M) {
        let luma =
            (0.2126 * albedo[0] as f32 + 0.7152 * albedo[1] as f32 + 0.0722 * albedo[2] as f32)
                / 255.0;
        let k = 0.75 + 0.45 * luma;
        return [
            (WATER_RGB[0] as f32 * k).clamp(0.0, 255.0) as u8,
            (WATER_RGB[1] as f32 * k).clamp(0.0, 255.0) as u8,
            (WATER_RGB[2] as f32 * k).clamp(0.0, 255.0) as u8,
        ];
    }
    let diffuse = normal.dot(light_dir()).max(0.0);
    let k = SHADE_AMBIENT + (1.0 - SHADE_AMBIENT) * diffuse;
    [
        (albedo[0] as f32 * k).clamp(0.0, 255.0) as u8,
        (albedo[1] as f32 * k).clamp(0.0, 255.0) as u8,
        (albedo[2] as f32 * k).clamp(0.0, 255.0) as u8,
    ]
}

/// Bilinear albedo sample at `(u, v)` in `0..=1`, clamped at the edges (wrap around the
/// whole area map would show the wrong side of the world).
pub fn sample_albedo(tex: &TextureData, u: f32, v: f32) -> [u8; 3] {
    if tex.width == 0 || tex.height == 0 || tex.rgba.len() < 4 {
        return [128, 128, 128];
    }
    let fx = (u.clamp(0.0, 1.0) * tex.width as f32 - 0.5).clamp(0.0, tex.width as f32 - 1.0);
    let fy = (v.clamp(0.0, 1.0) * tex.height as f32 - 0.5).clamp(0.0, tex.height as f32 - 1.0);
    let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
    let x1 = (x0 + 1).min(tex.width - 1);
    let y1 = (y0 + 1).min(tex.height - 1);
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let texel = |x: u32, y: u32| {
        let at = ((y * tex.width + x) * 4) as usize;
        [
            tex.rgba[at] as f32,
            tex.rgba[at + 1] as f32,
            tex.rgba[at + 2] as f32,
        ]
    };
    let (a, b, c, d) = (texel(x0, y0), texel(x1, y0), texel(x0, y1), texel(x1, y1));
    let mut out = [0u8; 3];
    for ch in 0..3 {
        let top = a[ch] * (1.0 - tx) + b[ch] * tx;
        let bottom = c[ch] * (1.0 - tx) + d[ch] * tx;
        out[ch] = (top * (1.0 - ty) + bottom * ty).clamp(0.0, 255.0) as u8;
    }
    out
}

// ------------------------------------------------------------------ map building

/// Generate the area map from a heightmap (normalized samples scaled by `world_height`)
/// and an optional albedo. Output pixel `(px, py)` covers world
/// `x = (px + 0.5) * extent.x / size`, `z = (py + 0.5) * extent.y / size` - the same
/// row/column convention as the baked albedo (and as [`world_to_uv`]).
pub fn build_map_from_heightmap(
    heightmap: &FloatTextureData,
    extent: Vec2,
    world_height: f32,
    albedo: Option<&TextureData>,
    size: u32,
) -> Result<TextureData, String> {
    if size == 0 {
        return Err("minimap size must be > 0".into());
    }
    if !(extent.x > 0.0 && extent.y > 0.0) {
        return Err(format!("invalid terrain extent {extent:?}"));
    }
    if heightmap.width < 2 || heightmap.height < 2 {
        return Err("heightmap smaller than 2x2".into());
    }
    let n = size as usize;
    let step = Vec2::new(extent.x / size as f32, extent.y / size as f32);
    let extent_arr = [extent.x, extent.y];
    let water_level = estimate_water_level_m(heightmap, world_height);
    let height_at = |x: f32, z: f32| {
        sample_height(
            heightmap,
            extent_arr,
            x.clamp(0.0, extent.x),
            z.clamp(0.0, extent.y),
        )
        .unwrap_or(0.0)
            * world_height
    };
    let mut rgba = vec![255u8; n * n * 4];
    for (py, row) in rgba.chunks_exact_mut(n * 4).enumerate() {
        let z = (py as f32 + 0.5) * step.y;
        for (px, pixel) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let x = (px as f32 + 0.5) * step.x;
            let h = height_at(x, z);
            let base = match albedo {
                Some(tex) => sample_albedo(tex, x / extent.x, z / extent.y),
                None => {
                    // No albedo available: a neutral grey brightness that still shows relief.
                    let g = (128.0 + h * 0.5).clamp(60.0, 200.0) as u8;
                    [g, g, g]
                }
            };
            let normal = normal_from_neighbours(
                height_at(x - step.x, z),
                height_at(x + step.x, z),
                height_at(x, z - step.y),
                height_at(x, z + step.y),
                step.x,
                step.y,
            );
            let rgb = shade_pixel(base, h, water_level, normal);
            pixel[0] = rgb[0];
            pixel[1] = rgb[1];
            pixel[2] = rgb[2];
        }
    }
    Ok(TextureData {
        width: size,
        height: size,
        rgba,
    })
}

/// [`build_map_from_heightmap`] for a loaded [`TerrainArea`].
pub fn build_area_map(
    area: &TerrainArea,
    albedo: Option<&TextureData>,
    size: u32,
) -> Result<TextureData, String> {
    build_map_from_heightmap(
        &area.heightmap,
        Vec2::new(area.world_extent[0], area.world_extent[1]),
        area.world_height,
        albedo,
        size,
    )
}

// ------------------------------------------------------------------ game map images

/// Path of the game's own single-tile map image for `area`, when it exists.
/// Area 05 ships six tiles (`map_lmap_pic_05_00..05`) and has no single image; its tiling
/// is unknown.
pub fn game_map_path(bin_root: &Path, area: u8) -> Option<PathBuf> {
    let path = bin_root
        .join("appli/map/map_texture")
        .join(format!("map_lmap_pic_{area:02}.bntx"));
    path.is_file().then_some(path)
}

/// Decode the game's `map_lmap_pic_NN.bntx` as the area map (alpha forced opaque).
pub fn load_game_map(bin_root: &Path, area: u8) -> Result<TextureData, String> {
    let path = game_map_path(bin_root, area).ok_or_else(|| {
        format!("appli/map/map_texture/map_lmap_pic_{area:02}.bntx not found (area 05 is tiled; tiling unknown)")
    })?;
    let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let bntx = Bntx::parse(&bytes)?;
    let name = format!("map_lmap_pic_{area:02}");
    let index = bntx
        .textures
        .iter()
        .position(|t| t.name == name)
        .ok_or_else(|| format!("{} has no texture named {name}", path.display()))?;
    let mut decoded = bntx.textures[index].decode(&bytes)?;
    for pixel in decoded.rgba.as_chunks_mut::<4>().0.iter_mut() {
        pixel[3] = 255;
    }
    Ok(TextureData {
        width: decoded.width,
        height: decoded.height,
        rgba: decoded.rgba,
    })
}

// ------------------------------------------------------------------ cache

/// Cache roots to search, in order: `PLA_CACHE_ROOT`, the main checkout's work dir, the
/// worktree-relative `work/cache` and `work` (same order the terrain albedo uses).
pub fn candidate_cache_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = std::env::var("PLA_CACHE_ROOT") {
        roots.push(PathBuf::from(dir));
    }
    roots.push(PathBuf::from(DEFAULT_CACHE_ROOT));
    roots.push(PathBuf::from("work/cache"));
    roots.push(PathBuf::from("work"));
    roots
}

/// `work/cache/minimap/areaNN.png` under `work_root`.
pub fn minimap_cache_path(work_root: impl AsRef<Path>, area: u8) -> PathBuf {
    work_root
        .as_ref()
        .join("minimap")
        .join(format!("area{area:02}.png"))
}

/// Sidecar next to the cached map: `areaNN.minimap.json`.
pub fn minimap_sidecar_path(work_root: impl AsRef<Path>, area: u8) -> PathBuf {
    work_root
        .as_ref()
        .join("minimap")
        .join(format!("area{area:02}.minimap.json"))
}

/// FNV-1a 64 of a byte slice (the signature ingredient).
pub fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Signature describing everything a generated map depends on, stored in the sidecar.
pub fn map_signature(
    area: u8,
    size: u32,
    albedo: Option<&TextureData>,
    heightmap: &FloatTextureData,
    world_height: f32,
    extent: Vec2,
) -> serde_json::Value {
    let mut samples = Vec::with_capacity(heightmap.samples.len() * 4);
    for value in &heightmap.samples {
        samples.extend_from_slice(&value.to_le_bytes());
    }
    serde_json::json!({
        "version": MINIMAP_VERSION,
        "area": area,
        "size": size,
        "world_height": world_height,
        "extent": [extent.x, extent.y],
        "heightmap": format!("{}x{}:{:016x}", heightmap.width, heightmap.height, hash_bytes(&samples)),
        "albedo": albedo.map(|t| format!("{}x{}:{:016x}", t.width, t.height, hash_bytes(&t.rgba))),
    })
}

/// Bake `area`'s map into `work_root/minimap/areaNN.png` plus its sidecar. With
/// `skip_if_unchanged` the bake is skipped when the PNG exists and the stored sidecar
/// equals the current signature.
pub fn bake_map_to_cache(
    area: &TerrainArea,
    albedo: Option<&TextureData>,
    work_root: &Path,
    size: u32,
    skip_if_unchanged: bool,
) -> Result<(BakeOutcome, f64), String> {
    let started = std::time::Instant::now();
    let extent = Vec2::new(area.world_extent[0], area.world_extent[1]);
    let png_path = minimap_cache_path(work_root, area.area);
    let side_path = minimap_sidecar_path(work_root, area.area);
    let signature = map_signature(
        area.area,
        size,
        albedo,
        &area.heightmap,
        area.world_height,
        extent,
    );
    if skip_if_unchanged && png_path.is_file() {
        if let Ok(text) = std::fs::read_to_string(&side_path) {
            if let Ok(old) = serde_json::from_str::<serde_json::Value>(&text) {
                if old == signature {
                    return Ok((
                        BakeOutcome::Skipped,
                        started.elapsed().as_secs_f64() * 1000.0,
                    ));
                }
            }
        }
    }
    let image = build_area_map(area, albedo, size)?;
    image
        .save_png(&png_path)
        .map_err(|e| format!("write {}: {e}", png_path.display()))?;
    std::fs::write(
        &side_path,
        serde_json::to_string_pretty(&signature).unwrap() + "\n",
    )
    .map_err(|e| format!("write {}: {e}", side_path.display()))?;
    Ok((BakeOutcome::Baked, started.elapsed().as_secs_f64() * 1000.0))
}

/// Where an area map came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapSource {
    /// The game's own `map_lmap_pic_NN.bntx` (`--game-map`).
    GameImage,
    /// A cached `minimap/areaNN.png` whose sidecar matches the current inputs.
    Cache,
    /// Generated in memory from the heightmap and the baked albedo.
    Generated,
}

/// A loaded area map plus where it came from and how long that took.
pub struct ResolvedMap {
    pub image: TextureData,
    pub source: MapSource,
    pub millis: f64,
}

/// Load the map of `area`: the game image with `game_map`, else a valid cache entry, else
/// generate it in memory. Never writes to the cache (examples do that).
pub fn resolve_area_map(
    area_idx: u8,
    area: &TerrainArea,
    albedo: Option<&TextureData>,
    bin_root: Option<&Path>,
    game_map: bool,
    size: u32,
) -> Result<ResolvedMap, String> {
    if game_map {
        if let Some(root) = bin_root {
            match load_game_map(root, area_idx) {
                Ok(image) => {
                    return Ok(ResolvedMap {
                        image,
                        source: MapSource::GameImage,
                        millis: 0.0,
                    })
                }
                Err(e) => {
                    eprintln!("Minimap: --game-map unavailable: {e}; using the generated map")
                }
            }
        } else {
            eprintln!("Minimap: --game-map needs the game bin root; using the generated map");
        }
    }
    let extent = Vec2::new(area.world_extent[0], area.world_extent[1]);
    let signature = map_signature(
        area_idx,
        size,
        albedo,
        &area.heightmap,
        area.world_height,
        extent,
    );
    for root in candidate_cache_roots() {
        let png = minimap_cache_path(&root, area_idx);
        if !png.is_file() {
            continue;
        }
        let side = minimap_sidecar_path(&root, area_idx);
        let matches = std::fs::read_to_string(&side)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .is_some_and(|old| old == signature);
        if !matches {
            continue;
        }
        match load_baked_albedo(&png) {
            Ok(image) => {
                return Ok(ResolvedMap {
                    image,
                    source: MapSource::Cache,
                    millis: 0.0,
                })
            }
            Err(e) => eprintln!("Minimap: cached map {} unreadable: {e}", png.display()),
        }
    }
    let started = std::time::Instant::now();
    let image = build_area_map(area, albedo, size)?;
    Ok(ResolvedMap {
        image,
        source: MapSource::Generated,
        millis: started.elapsed().as_secs_f64() * 1000.0,
    })
}

// ------------------------------------------------------------------ Bevy integration

/// The minimap frame (bottom-left), child of the world HUD root so the battle screen hides it.
#[derive(Component)]
pub struct MinimapRoot;

/// The cropped map image inside the frame.
#[derive(Component)]
pub struct MinimapImage;

/// The rotated player bar at the crop centre.
#[derive(Component)]
pub struct MinimapPlayerMarker;

/// The "N" mark at the top of the frame (and of the full-screen map).
#[derive(Component)]
pub struct NorthMark;

/// The eight-direction compass strip at the top of the screen.
#[derive(Component)]
pub struct CompassText;

/// Full-screen map overlay root (toggled with `M`).
#[derive(Component)]
pub struct FullMapRoot;

/// The whole-area image inside the overlay.
#[derive(Component)]
pub struct FullMapImage;

/// The player dot inside the overlay.
#[derive(Component)]
pub struct FullMapMarker;

/// What the minimap currently shows.
#[derive(Resource, Debug, Default)]
pub struct MinimapState {
    /// Main-area index the current map belongs to.
    pub area: Option<u8>,
    /// World extent of the current area (metres).
    pub extent: Vec2,
    /// Texture side of the current map (texels).
    pub map_px: u32,
    /// Full-screen map open.
    pub open: bool,
    /// Bevy image handle of the current map.
    pub image: Option<Handle<Image>>,
}

/// Plugin settings (from [`MinimapPlugin`]).
#[derive(Resource, Debug, Clone)]
pub struct MinimapConfig {
    pub bin_root: Option<PathBuf>,
    pub game_map: bool,
    pub size: u32,
}

/// Minimap, compass and full-screen map. One plugin line in the viewer.
pub struct MinimapPlugin {
    pub enabled: bool,
    pub bin_root: Option<PathBuf>,
    pub game_map: bool,
    pub size: u32,
}

impl MinimapPlugin {
    /// Settings from the command line: `--no-minimap` disables everything,
    /// `--game-map` prefers the game's own map images.
    pub fn from_env_and_args(bin_root: PathBuf) -> Self {
        let args: Vec<String> = std::env::args().collect();
        Self {
            enabled: !args.iter().any(|a| a == "--no-minimap"),
            bin_root: Some(bin_root),
            game_map: args.iter().any(|a| a == "--game-map"),
            size: DEFAULT_MAP_SIZE,
        }
    }
}

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        if !self.enabled {
            return;
        }
        app.insert_resource(MinimapConfig {
            bin_root: self.bin_root.clone(),
            game_map: self.game_map,
            size: self.size,
        });
        app.init_resource::<MinimapState>();
        app.add_systems(
            Startup,
            setup_minimap_ui.after(crate::game::ui::setup_game_ui),
        );
        app.add_systems(
            Update,
            (
                rebuild_minimap_on_zone_change,
                toggle_full_map
                    .run_if(not_paused)
                    .run_if(in_state(GameState::World)),
                update_minimap_hud,
                full_map_visibility_system,
            )
                .chain(),
        );
    }
}

/// Spawn the HUD nodes under the world HUD root (no-op without one, e.g. headless apps).
pub fn setup_minimap_ui(mut commands: Commands, hud: Query<Entity, With<WorldHudRoot>>) {
    let Ok(hud) = hud.single() else {
        return;
    };
    commands.entity(hud).with_children(|root| {
        root.spawn((
            MinimapRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                bottom: Val::Px(16.0),
                width: Val::Px(FRAME_PX),
                height: Val::Px(FRAME_PX),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.92)),
            BorderColor::all(Color::srgba(0.35, 0.50, 0.70, 0.85)),
            // Inherited (not Visible): an explicit Visible would stay on screen in battle and
            // overlap the battle log, ignoring the hidden world HUD parent.
            Visibility::Inherited,
        ))
        .with_children(|frame| {
            frame.spawn((
                MinimapImage,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ImageNode {
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
            ));
            frame.spawn((
                MinimapPlayerMarker,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(FRAME_PX * 0.5 - 3.0),
                    top: Val::Px(FRAME_PX * 0.5 - 7.0),
                    width: Val::Px(6.0),
                    height: Val::Px(14.0),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(1.0, 0.90, 0.25)),
                UiTransform::default(),
            ));
            frame.spawn((
                NorthMark,
                Text::new("N"),
                TextFont::from_font_size(12.0),
                TextColor(Color::srgb(0.90, 0.95, 1.0)),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(FRAME_PX * 0.5 - 4.0),
                    top: Val::Px(2.0),
                    ..default()
                },
            ));
        });

        // Compass strip, top centre.
        root.spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            top: Val::Px(14.0),
            display: Display::Flex,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                CompassText,
                Text::new(compass_line(Compass::N)),
                TextFont::from_font_size(14.0),
                TextColor(Color::srgb(0.92, 0.95, 1.0)),
                TextLayout::justify(Justify::Center),
            ));
        });
    });

    // Full-screen map overlay, its own root (hidden until M).
    commands
        .spawn((
            FullMapRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(900),
            Visibility::Hidden,
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::VMin(72.0),
                        height: Val::VMin(72.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.96)),
                    BorderColor::all(Color::srgba(0.40, 0.55, 0.75, 0.90)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        FullMapImage,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            top: Val::Px(0.0),
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        ImageNode {
                            image_mode: NodeImageMode::Stretch,
                            ..default()
                        },
                    ));
                    panel.spawn((
                        FullMapMarker,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(50.0),
                            top: Val::Percent(50.0),
                            width: Val::Px(8.0),
                            height: Val::Px(8.0),
                            border_radius: BorderRadius::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(1.0, 0.90, 0.25)),
                        UiTransform::default(),
                    ));
                    panel.spawn((
                        NorthMark,
                        Text::new("N"),
                        TextFont::from_font_size(13.0),
                        TextColor(Color::srgb(0.90, 0.95, 1.0)),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(6.0),
                            top: Val::Px(4.0),
                            ..default()
                        },
                    ));
                });
            overlay.spawn((
                Text::new("[M] close the map"),
                TextFont::from_font_size(14.0),
                TextColor(Color::srgb(0.85, 0.90, 0.95)),
            ));
        });
}

/// Regenerate the map when the active main area changes (and once at start).
pub fn rebuild_minimap_on_zone_change(
    mut state: ResMut<MinimapState>,
    config: Res<MinimapConfig>,
    zone: Option<Res<crate::game::ui::ActiveZoneInfo>>,
    terrain: Option<Res<crate::game::zone::TerrainResource>>,
    images: Option<ResMut<Assets<Image>>>,
    mut minimap_q: Query<&mut ImageNode, With<MinimapImage>>,
    mut full_q: Query<&mut ImageNode, (With<FullMapImage>, Without<MinimapImage>)>,
) {
    let Some(mut images) = images else {
        return; // headless app without an image store: nothing to show
    };
    let Some(zone) = zone else { return };
    let Some(area_idx) = zone.area_idx() else {
        return;
    };
    if state.area == Some(area_idx) && state.map_px == config.size {
        return;
    }
    let Some(terrain) = terrain else { return };
    if terrain.area.area != area_idx {
        return; // the scene is still the previous one; retry next frame
    }
    let albedo = terrain.blended_albedo.as_ref();
    let resolved = match resolve_area_map(
        area_idx,
        &terrain.area,
        albedo,
        config.bin_root.as_deref(),
        config.game_map,
        config.size,
    ) {
        Ok(resolved) => resolved,
        Err(e) => {
            eprintln!("Minimap: cannot build the area {area_idx:02} map: {e}");
            return;
        }
    };
    let (w, h) = (resolved.image.width.max(1), resolved.image.height.max(1));
    let handle = images.add(Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        resolved.image.rgba.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));
    println!(
        "Minimap: area {area_idx:02} map {w}x{h} from {:?} in {:.0} ms",
        resolved.source, resolved.millis
    );
    for mut node in &mut minimap_q {
        node.image = handle.clone();
    }
    for mut node in &mut full_q {
        node.image = handle.clone();
    }
    state.area = Some(area_idx);
    state.extent = Vec2::new(terrain.area.world_extent[0], terrain.area.world_extent[1]);
    state.map_px = resolved.image.width;
    state.image = Some(handle);
}

/// Crop the map around the player and rotate the marker; update the compass strip.
#[allow(clippy::too_many_arguments)]
pub fn update_minimap_hud(
    state: Res<MinimapState>,
    player: Query<&Transform, With<crate::game::PlayerAvatar>>,
    mut minimap_q: Query<&mut ImageNode, With<MinimapImage>>,
    mut marker_q: Query<(&mut Node, &mut UiTransform), With<MinimapPlayerMarker>>,
    mut full_marker_q: Query<&mut UiTransform, (With<FullMapMarker>, Without<MinimapPlayerMarker>)>,
    mut compass_q: Query<&mut Text, With<CompassText>>,
) {
    if state.image.is_none() || state.map_px == 0 {
        return;
    }
    let Ok(transform) = player.single() else {
        return;
    };
    let (x, z) = (transform.translation.x, transform.translation.z);
    let player_texel = texel_of_world(x, z, state.extent, state.map_px);
    let view_texels = VIEW_METRES * state.map_px as f32 / state.extent.x.max(1e-3);
    let (window_center, rect) = crop_window(player_texel, view_texels, state.map_px);
    let offset = marker_offset_px(player_texel, window_center, view_texels, FRAME_PX);
    let yaw = transform.rotation.to_euler(bevy::math::EulerRot::YXZ).0;
    let facing = facing_from_yaw(yaw);
    let rotation = Rot2::degrees(marker_rotation_deg(facing.x, facing.y));

    for mut node in &mut minimap_q {
        node.rect = Some(rect);
    }
    for (mut node, mut ui) in &mut marker_q {
        node.left = Val::Px(FRAME_PX * 0.5 - 3.0 + offset.x);
        node.top = Val::Px(FRAME_PX * 0.5 - 7.0 + offset.y);
        *ui = UiTransform::from_rotation(rotation);
    }
    for mut ui in &mut full_marker_q {
        *ui = UiTransform::from_rotation(rotation);
    }
    let dir = compass_dir(facing.x, facing.y);
    let line = compass_line(dir);
    for mut text in &mut compass_q {
        if text.0 != line {
            text.0 = line.clone();
        }
    }
}

/// `M` toggles the full-screen map (World state, not paused).
pub fn toggle_full_map(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<MinimapState>,
    mut full_q: Query<&mut Visibility, With<FullMapRoot>>,
) {
    if !keys.just_pressed(KeyCode::KeyM) {
        return;
    }
    state.open = !state.open;
    for mut visibility in &mut full_q {
        *visibility = if state.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// The overlay is only visible while open in the world state (cheap enough to set every frame;
/// the change-detection gate proved fragile across state transitions).
pub fn full_map_visibility_system(
    state: Res<MinimapState>,
    game_state: Option<Res<State<GameState>>>,
    mut full_q: Query<&mut Visibility, With<FullMapRoot>>,
) {
    let in_world = game_state
        .as_ref()
        .is_none_or(|s| matches!(s.get(), GameState::World));
    let wanted = if state.open && in_world {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut full_q {
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// The full-screen player dot position as a fraction of the map (`u`, `v` in `0..=1`).
pub fn full_map_marker_uv(x: f32, z: f32, extent: Vec2) -> Vec2 {
    world_to_uv(x, z, extent).clamp(Vec2::ZERO, Vec2::ONE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_heightmap(size: u32, value: f32) -> FloatTextureData {
        FloatTextureData {
            width: size,
            height: size,
            samples: vec![value; (size * size) as usize],
        }
    }

    fn ramp_albedo(size: u32) -> TextureData {
        let mut rgba = Vec::with_capacity((size * size * 4) as usize);
        for y in 0..size {
            for x in 0..size {
                rgba.push((x * 255 / size.max(1)) as u8);
                rgba.push((y * 255 / size.max(1)) as u8);
                rgba.push(128);
                rgba.push(255);
            }
        }
        TextureData {
            width: size,
            height: size,
            rgba,
        }
    }

    #[test]
    fn world_uv_and_texels_round_trip_at_the_corners() {
        let extent = Vec2::new(1025.0, 1025.0);
        assert_eq!(world_to_uv(0.0, 0.0, extent), Vec2::ZERO);
        assert_eq!(world_to_uv(1025.0, 1025.0, extent), Vec2::ONE);
        assert!(
            (uv_to_world(world_to_uv(512.3, 100.7, extent), extent) - Vec2::new(512.3, 100.7))
                .length()
                < 1e-3
        );
        // Texel centres: the world point of texel centre `(i + 0.5, j + 0.5)` maps back to `(i, j)`.
        let centre = uv_to_world(Vec2::new(511.5, 255.5) / 1024.0, extent);
        let t = texel_of_world(centre.x, centre.y, extent, 1024);
        assert!(
            (t - Vec2::new(511.0, 255.0)).length() < 1e-3,
            "texel centres line up: {t:?}"
        );
        assert!((texel_of_world(0.5, 0.5, extent, 1024) - Vec2::splat(0.0)).length() < 0.01);
    }

    #[test]
    fn crop_window_shifts_to_stay_inside_the_map() {
        let map = 1024u32;
        let (center, rect) = crop_window(Vec2::new(500.0, 500.0), 160.0, map);
        assert_eq!(center, Vec2::new(500.0, 500.0));
        assert_eq!(rect.size(), Vec2::splat(160.0));
        // Near the low corner the window shifts in, it does not shrink.
        let (center, rect) = crop_window(Vec2::new(10.0, 10.0), 160.0, map);
        assert_eq!(center, Vec2::splat(80.0));
        assert_eq!(rect.size(), Vec2::splat(160.0));
        assert!(rect.min.x >= 0.0 && rect.min.y >= 0.0);
        // And near the far corner.
        let (center, rect) = crop_window(Vec2::new(1020.0, 5.0), 160.0, map);
        assert_eq!(center, Vec2::new(944.0, 80.0));
        assert!(rect.max.x <= 1024.0 && rect.max.y <= 1024.0);
        // The marker offset follows the player: centred -> zero, shifted -> opposite sign.
        let offset = marker_offset_px(
            Vec2::new(500.0, 500.0),
            Vec2::new(500.0, 500.0),
            160.0,
            200.0,
        );
        assert_eq!(offset, Vec2::ZERO);
        let offset = marker_offset_px(Vec2::new(10.0, 10.0), Vec2::splat(80.0), 160.0, 200.0);
        assert!(
            offset.x < 0.0 && offset.y < 0.0,
            "player left of the shifted window: {offset:?}"
        );
    }

    #[test]
    fn marker_rotation_points_with_the_facing() {
        // North (-z) is up on the map: no rotation.
        assert!(marker_rotation_deg(0.0, -1.0).abs() < 1e-3);
        // South (+z) is down: 180 deg.
        assert!((marker_rotation_deg(0.0, 1.0).abs() - 180.0).abs() < 1e-3);
        // +x is to the right on the map: 90 deg clockwise.
        assert!((marker_rotation_deg(1.0, 0.0) - 90.0).abs() < 1e-3);
        assert!((marker_rotation_deg(-1.0, 0.0) + 90.0).abs() < 1e-3);
        // Facing follows the avatar yaw (model front +Z).
        let f = facing_from_yaw(0.0);
        assert!((f - Vec2::new(0.0, 1.0)).length() < 1e-5);
        let f = facing_from_yaw(std::f32::consts::FRAC_PI_2);
        assert!((f - Vec2::new(1.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn compass_dir_matches_the_radar_sectors() {
        assert_eq!(compass_dir(0.0, 1.0), Compass::S);
        assert_eq!(compass_dir(0.0, -1.0), Compass::N);
        assert_eq!(compass_dir(1.0, 0.0), Compass::W);
        assert_eq!(compass_dir(-1.0, 0.0), Compass::E);
        assert_eq!(compass_dir(1.0, 1.0), Compass::SW);
        assert_eq!(compass_dir(1.0, -1.0), Compass::NW);
        assert_eq!(compass_dir(-1.0, -1.0), Compass::NE);
        assert_eq!(compass_dir(-1.0, 1.0), Compass::SE);
        // Sector edges at 22.5 deg from south.
        let inside = 22.4f32.to_radians();
        let outside = 22.6f32.to_radians();
        assert_eq!(compass_dir(inside.sin(), inside.cos()), Compass::S);
        assert_eq!(compass_dir(outside.sin(), outside.cos()), Compass::SW);
    }

    #[test]
    fn compass_line_marks_exactly_one_direction() {
        for dir in Compass::ALL {
            let line = compass_line(dir);
            assert_eq!(line.matches('[').count(), 1, "{line}");
            assert_eq!(line.matches(']').count(), 1, "{line}");
            assert!(line.contains(&format!("[{}]", dir.label())), "{line}");
            for other in Compass::ALL.iter().filter(|d| **d != dir) {
                assert!(!line.contains(&format!("[{}]", other.label())));
            }
        }
        assert_eq!(compass_line(Compass::S), "N NE E SE [S] SW W NW");
    }

    #[test]
    fn normals_and_shading_follow_the_relief() {
        // Flat ground: straight up.
        let n = normal_from_neighbours(5.0, 5.0, 5.0, 5.0, 1.0, 1.0);
        assert!((n - Vec3::Y).length() < 1e-5);
        // Rising towards +x: the normal leans towards -x.
        let n = normal_from_neighbours(0.0, 2.0, 1.0, 1.0, 1.0, 1.0);
        assert!(n.x < -0.5 && n.y > 0.0, "{n:?}");
        // Sea is blue and reads brighter over brighter albedo.
        let dark = shade_pixel([10, 10, 10], 0.0, Some(0.0), Vec3::Y);
        let bright = shade_pixel([240, 240, 240], 0.0, Some(0.0), Vec3::Y);
        for c in [dark, bright] {
            assert!(c[2] > c[0] && c[2] > c[1], "water is blue: {c:?}");
        }
        assert!(bright[2] > dark[2]);
        // A band above the estimated level is still water; higher ground is land.
        let shallow = shade_pixel([200, 200, 200], 1.6, Some(1.5), Vec3::Y);
        assert!(
            shallow[2] > shallow[0],
            "within the water band: {shallow:?}"
        );
        let land = shade_pixel([200, 200, 200], 2.5, Some(1.5), Vec3::Y);
        assert!(
            land[0] == land[1] && land[1] == land[2],
            "land keeps the albedo tint"
        );
        // Flat land keeps the ambient fraction, a slope facing the light brightens.
        let flat = shade_pixel([200, 200, 200], 10.0, None, Vec3::Y);
        assert!(flat[0] < 200 && flat[0] > 100);
        let light = light_dir();
        let towards = shade_pixel([200, 200, 200], 10.0, None, light);
        let away = shade_pixel([200, 200, 200], 10.0, None, -light + Vec3::Y * 0.2);
        assert!(towards[0] > away[0], "{towards:?} vs {away:?}");
    }

    #[test]
    fn water_level_estimate_reads_the_flat_sea_band() {
        // Two thirds of the map flat at 0.02 (the sea), the rest rising land.
        let mut heightmap = flat_heightmap(64, 0.02);
        for (i, sample) in heightmap.samples.iter_mut().enumerate() {
            if i % 3 == 0 {
                *sample = 0.05 + (i as f32 % 100.0) / 200.0;
            }
        }
        let level = estimate_water_level_m(&heightmap, 100.0).expect("sea level found");
        assert!((level - 2.0).abs() < 1.0, "sea at 0.02 x 100 m: {level}");
        // A flat plateau in the middle of the range is land, not water.
        assert_eq!(
            estimate_water_level_m(&flat_heightmap(16, 0.5), 100.0),
            None
        );
        // A constant zero map is water at level 0.
        assert_eq!(
            estimate_water_level_m(&flat_heightmap(16, 0.0), 100.0),
            Some(0.0)
        );
    }

    #[test]
    fn generated_map_keeps_the_column_row_convention() {
        // A ramp albedo: red grows with x (columns), green with z (rows).
        let albedo = ramp_albedo(16);
        let extent = Vec2::splat(80.0);
        let heightmap = flat_heightmap(9, 10.0);
        let map = build_map_from_heightmap(&heightmap, extent, 20.0, Some(&albedo), 32).unwrap();
        assert_eq!((map.width, map.height), (32, 32));
        let at = |x: u32, y: u32| {
            let i = ((y * 32 + x) * 4) as usize;
            (map.rgba[i], map.rgba[i + 1])
        };
        let (r0, g0) = at(0, 0);
        let (r1, g1) = at(31, 0);
        let (r2, g2) = at(0, 31);
        assert!(r1 > r0 + 100, "x grows with the column: {r0} -> {r1}");
        assert!(g2 > g0 + 100, "z grows with the row: {g0} -> {g2}");
        assert!(g1 < 60 && r2 < 60, "no axis swap: ({g1}, {r2})");
        // Deterministic.
        let again = build_map_from_heightmap(&heightmap, extent, 20.0, Some(&albedo), 32).unwrap();
        assert_eq!(map.rgba, again.rgba);
    }

    #[test]
    fn generated_map_paints_the_sea_blue_and_sizes_validate() {
        let mut heightmap = flat_heightmap(9, 0.0);
        heightmap.samples[5 * 9 + 5] = 0.5; // one land sample
        let extent = Vec2::splat(80.0);
        let map = build_map_from_heightmap(&heightmap, extent, 20.0, None, 16).unwrap();
        let water = [map.rgba[0], map.rgba[1], map.rgba[2]];
        assert!(water[2] > water[0], "corner is sea: {water:?}");
        assert!(build_map_from_heightmap(&heightmap, extent, 20.0, None, 0).is_err());
        assert!(
            build_map_from_heightmap(&heightmap, Vec2::new(0.0, 80.0), 20.0, None, 16).is_err()
        );
    }

    #[test]
    fn signature_and_cache_paths_name_the_area() {
        let albedo = ramp_albedo(8);
        let heightmap = flat_heightmap(8, 1.0);
        let a = map_signature(
            3,
            1024,
            Some(&albedo),
            &heightmap,
            100.0,
            Vec2::splat(1025.0),
        );
        let b = map_signature(
            3,
            512,
            Some(&albedo),
            &heightmap,
            100.0,
            Vec2::splat(1025.0),
        );
        assert_ne!(a, b);
        assert_eq!(a["version"], MINIMAP_VERSION);
        assert!(minimap_cache_path("/tmp", 4).ends_with("minimap/area04.png"));
        assert!(minimap_sidecar_path("/tmp", 4).ends_with("minimap/area04.minimap.json"));
        assert_ne!(hash_bytes(b"abc"), hash_bytes(b"abd"));
    }

    // ---------------------------------------------------------------- headless

    fn synthetic_area(area: u8, extent: f32, height: f32) -> TerrainArea {
        use crate::assets::terrain::TerrainMaterial;
        use crate::assets::tr::TrMtr;
        use std::collections::HashMap;
        TerrainArea {
            area,
            source_path: PathBuf::new(),
            world_extent: [extent, extent],
            world_height: height,
            heightmap_brti_name: "h".into(),
            navmesh_bounds: None,
            heightmap: flat_heightmap(9, 5.0),
            normal_map: TextureData {
                width: 1,
                height: 1,
                rgba: vec![0, 0, 0, 255],
            },
            splat_alpha: TextureData {
                width: 2,
                height: 2,
                rgba: [255, 0, 0, 0].repeat(4),
            },
            splat_alpha1: None,
            material: Some(TerrainMaterial {
                slots: TrMtr {
                    textures: vec![],
                    flags: vec![],
                },
                parameter_candidates: vec![],
                splat_tiling: vec![[32.0, 32.0]],
            }),
            ground_textures: HashMap::new(),
            unresolved_ground_textures: vec![],
            masks: vec![],
            instances: vec![],
        }
    }

    fn synthetic_terrain(area: u8, extent: f32, height: f32) -> crate::game::zone::TerrainResource {
        use crate::assets::terrain::TerrainMeshSettings;
        crate::game::zone::TerrainResource {
            area: std::sync::Arc::new(synthetic_area(area, extent, height)),
            chunks: vec![],
            blended_albedo: Some(ramp_albedo(32)),
            settings: TerrainMeshSettings {
                horizontal_cell_size: extent / 8.0,
                vertical_scale: height,
                chunk_cells: 4,
            },
        }
    }

    fn minimap_test_app(area: u8) -> App {
        use bevy::state::app::StatesPlugin;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            bevy::asset::AssetPlugin::default(),
        ));
        app.init_state::<GameState>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(Assets::<Image>::default());
        app.insert_resource(crate::game::ui::ActiveZoneInfo {
            area: format!("ha_area{area:02}"),
            name: "Test Area".into(),
        });
        app.insert_resource(synthetic_terrain(area, 1025.0, 80.0));
        app.world_mut().spawn((Node::default(), WorldHudRoot));
        app.world_mut().spawn((
            crate::game::PlayerAvatar,
            Transform::from_xyz(500.0, 10.0, 500.0),
        ));
        app.add_plugins(MinimapPlugin {
            enabled: true,
            bin_root: None,
            game_map: false,
            size: 64,
        });
        app.update();
        app
    }

    /// Press `key`, run one frame and release it (no `InputPlugin` in the test app, so
    /// `just_pressed` would otherwise stay set and the input would never read as a new press).
    fn press_once(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(key);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }

    fn minimap_image(app: &mut App) -> ImageNode {
        let mut query = app
            .world_mut()
            .query_filtered::<&ImageNode, With<MinimapImage>>();
        query
            .iter(app.world())
            .next()
            .cloned()
            .expect("minimap image node")
    }

    #[test]
    fn plugin_spawns_the_hud_and_tracks_the_player() {
        let mut app = minimap_test_app(0);
        let state = app.world().resource::<MinimapState>();
        assert_eq!(state.area, Some(0), "startup generated the area 00 map");
        assert!(state.image.is_some());
        assert_eq!(state.map_px, 64);

        let image = minimap_image(&mut app);
        let rect = image.rect.expect("crop rect set for a centred player");
        assert!(
            (rect.size().x - 160.0 * 64.0 / 1025.0).abs() < 0.5,
            "{rect:?}"
        );
        let handle = image.image.id();
        assert_ne!(
            handle,
            Handle::<Image>::default().id(),
            "a real image is wired"
        );

        // Move the player to the map corner: the crop shifts but keeps its size.
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::game::PlayerAvatar>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut()
            .entity_mut(player)
            .insert(Transform::from_xyz(2.0, 10.0, 2.0));
        app.update();
        let rect = minimap_image(&mut app).rect.unwrap();
        assert!(rect.min.x >= 0.0 && rect.min.y >= 0.0);
        assert!((rect.size().x - 160.0 * 64.0 / 1025.0).abs() < 0.5);

        // The compass strip follows the facing (yaw 0 -> +z -> south).
        let mut query = app.world_mut().query_filtered::<&Text, With<CompassText>>();
        let text = query
            .iter(app.world())
            .next()
            .cloned()
            .expect("compass text");
        assert_eq!(text.0, compass_line(Compass::S));
    }

    #[test]
    fn full_map_toggles_with_m_and_hides_outside_the_world_state() {
        let mut app = minimap_test_app(1);
        let visibility = |app: &mut App| {
            let mut query = app
                .world_mut()
                .query_filtered::<&Visibility, With<FullMapRoot>>();
            *query.iter(app.world()).next().unwrap()
        };
        assert_eq!(visibility(&mut app), Visibility::Hidden);
        press_once(&mut app, KeyCode::KeyM);
        assert_eq!(visibility(&mut app), Visibility::Visible);
        press_once(&mut app, KeyCode::KeyM);
        assert_eq!(visibility(&mut app), Visibility::Hidden);
        // Opening it and entering a battle hides it again.
        press_once(&mut app, KeyCode::KeyM);
        assert_eq!(visibility(&mut app), Visibility::Visible);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Battle);
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Battle,
            "the test moved to Battle"
        );
        assert_eq!(visibility(&mut app), Visibility::Hidden);
    }

    #[test]
    fn zone_change_regenerates_the_map() {
        let mut app = minimap_test_app(0);
        let first = app
            .world()
            .resource::<MinimapState>()
            .image
            .clone()
            .unwrap();
        app.insert_resource(synthetic_terrain(1, 1025.0, 300.0));
        app.insert_resource(crate::game::ui::ActiveZoneInfo {
            area: "ha_area01".into(),
            name: "Area 01".into(),
        });
        app.update();
        let state = app.world().resource::<MinimapState>();
        assert_eq!(state.area, Some(1));
        assert_ne!(
            state.image.clone().unwrap().id(),
            first.id(),
            "new map texture"
        );
    }

    #[test]
    fn plugin_without_terrain_does_not_panic() {
        use bevy::state::app::StatesPlugin;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            bevy::asset::AssetPlugin::default(),
        ));
        app.init_state::<GameState>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(crate::game::ui::ActiveZoneInfo {
            area: "ha_area00".into(),
            name: "Test".into(),
        });
        app.world_mut().spawn((Node::default(), WorldHudRoot));
        app.add_plugins(MinimapPlugin {
            enabled: true,
            bin_root: None,
            game_map: false,
            size: 32,
        });
        app.update();
        assert_eq!(app.world().resource::<MinimapState>().area, None);
    }

    // ---------------------------------------------------------------- real data

    fn game_bin_root() -> Option<PathBuf> {
        let root = std::env::var_os("PLA_BIN_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
            });
        root.join("archive/field/terrain/ha_area00.gfpak")
            .is_file()
            .then_some(root)
    }

    fn cached_albedo(area: u8) -> Option<TextureData> {
        for root in candidate_cache_roots() {
            for p in [
                root.join("terrain_albedo")
                    .join(format!("area{area:02}.png")),
                crate::assets::terrain_albedo::albedo_cache_path(&root, area),
            ] {
                if p.is_file() {
                    if let Ok(tex) = load_baked_albedo(&p) {
                        return Some(tex);
                    }
                }
            }
        }
        None
    }

    #[test]
    fn real_area_maps_build_with_measured_size_and_time() {
        let Some(root) = game_bin_root() else {
            eprintln!("[skip] game bin root not found (set PLA_BIN_ROOT)");
            return;
        };
        let measured = [
            (0u8, "Jubilife Village"),
            (1, "Obsidian Fieldlands"),
            (5, "Alabaster Icelands"),
        ];
        let mut total_ms = 0.0f64;
        let mut built = 0usize;
        for (area, name) in measured {
            let Ok(area_data) = TerrainArea::load(&root, area) else {
                eprintln!("[skip] area {area:02}: terrain not loadable");
                continue;
            };
            let albedo = cached_albedo(area);
            if albedo.is_none() {
                eprintln!(
                    "[skip] area {area:02}: no baked albedo in the cache (heightmap-only map)"
                );
            }
            let started = std::time::Instant::now();
            let map = build_area_map(&area_data, albedo.as_ref(), 1024).unwrap();
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            total_ms += ms;
            built += 1;
            assert_eq!((map.width, map.height), (1024, 1024));
            // A real map must not be a single flat colour.
            let mut min = [255u8; 3];
            let mut max = [0u8; 3];
            let mut water = 0usize;
            for p in map.rgba.as_chunks::<4>().0 {
                for ch in 0..3 {
                    min[ch] = min[ch].min(p[ch]);
                    max[ch] = max[ch].max(p[ch]);
                }
                if p[2] as u32 > p[0] as u32 + 20 && p[2] as u32 > p[1] as u32 + 20 {
                    water += 1;
                }
            }
            let spread =
                (max[0] - min[0]) as u32 + (max[1] - min[1]) as u32 + (max[2] - min[2]) as u32;
            assert!(
                spread > 30,
                "area {area:02} map is flat: min={min:?} max={max:?}"
            );
            let water_pct = water as f64 / (1024.0 * 1024.0) * 100.0;
            if area != 5 {
                assert!(
                    water_pct > 1.0,
                    "area {area:02} is coastal/lake-rich: {water_pct:.2}% water"
                );
            }
            let level = estimate_water_level_m(&area_data.heightmap, area_data.world_height);
            println!(
                "minimap area {area:02} ({name}): 1024x1024 from albedo={} in {ms:.0} ms, channel spread {spread}, water {water_pct:.1}% at level {level:?} m",
                albedo.is_some()
            );
        }
        assert!(
            built >= 1,
            "the game data root exists but no area map was built"
        );
        assert!(total_ms > 0.0);
    }

    #[test]
    fn real_game_map_loads_or_skips_with_a_reason() {
        let Some(root) = game_bin_root() else {
            eprintln!("[skip] game bin root not found (set PLA_BIN_ROOT)");
            return;
        };
        for area in [0u8, 5] {
            match load_game_map(&root, area) {
                Ok(image) => {
                    assert!(image.width >= 512 && image.height >= 512);
                    assert!(image.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
                    println!("game map area {area:02}: {}x{}", image.width, image.height);
                }
                Err(e) => {
                    assert_eq!(
                        area, 5,
                        "only area 05 is expected to have no single image: {e}"
                    );
                    println!("game map area {area:02}: [skip] {e}");
                }
            }
        }
    }
}
