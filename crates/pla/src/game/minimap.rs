//! Minimap, compass and full-screen area map (Brief 15).
//!
//! The area map is generated from data that is already in the engine: the baked terrain
//! albedo (`work/cache/terrain_albedo/areaNN.png`) plus the area heightmap, shaded with a
//! fixed hill-shading light and with the sea (height 0) painted blue. The generated map is
//! finished with a **toon (cell-shading) pass**: the relief light is posterised to
//! [`TOON_BANDS`] flat bands, the colours get a small saturation boost, band edges and the
//! water shore draw a thin dark outline and the land side of the water gets a clear bank
//! (`apply_toon_outlines`). **Buildings** (houses, shops, gates, stone walls and the big
//! building, [`is_building_model`]) are painted as flat warm blocks with a dark outline from
//! their collision-shape footprint ([`load_building_footprints`]). An optional `--game-map`
//! mode loads the game's own `map_lmap_pic_NN.bntx` instead (areas 00-04 and 06; area 05 is
//! split in six tiles whose tiling is unknown, so it falls back to the generated map).
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
//! The minimap keeps north up and rotates the player marker ([`marker_rotation_deg`]); the
//! marker is the generated **arrow** ([`player_arrow_texture`]) pointing where the player is
//! looking - the same facing the compass strip reads, so both can never disagree. The crop
//! window follows the player ([`crop_window`]) and the compass strip shows which of the eight
//! directions the player is facing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::assets::bntx::{Bntx, FloatTextureData, TextureData};
use crate::assets::navmesh::NavMesh;
use crate::assets::terrain::{sample_height, TerrainArea};
use crate::assets::terrain_albedo::{load_baked_albedo, BakeOutcome};
use crate::game::state::{not_paused, GameState};
use crate::game::ui::WorldHudRoot;
use crate::game::world_collision::{placement_transform, CollisionPacks};
use crate::game::world_objects::{Placement, RotMode};

/// Version of the generation algorithm; part of the cache signature (3: toon finish and
/// building footprints; every look change must bump it so the old caches invalidate).
pub const MINIMAP_VERSION: u32 = 3;

/// Default map texture size (square, texels).
pub const DEFAULT_MAP_SIZE: u32 = 1024;

/// Side of the interior (sub-area) map texture. Interiors are small scenes, so half the
/// main-area size keeps the raster cheap while staying legible.
pub const INTERIOR_MAP_SIZE: u32 = 512;

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

// ------------------------------------------------------------------ toon (cell-shading) finish

/// Flat light bands the relief is posterised to. Three or four flat steps read as the painted
/// cell-shading style of the game's own map, instead of a smooth gradient. Presentation choice.
pub const TOON_BANDS: usize = 4;

/// Flat bands of the water shading (its own, darker-to-brighter sea steps).
pub const TOON_WATER_BANDS: usize = 3;

/// Band id offset of the water bands, so land and water ids never collide in one `u8`.
pub const WATER_BAND_BASE: u8 = 8;

/// Saturation multiplier applied after the banding (1.0 keeps the albedo's own colours).
pub const TOON_SATURATION: f32 = 1.3;

/// Dark ink of the toon outline (edges between bands and the water shore).
pub const TOON_OUTLINE_RGB: [u8; 3] = [38, 32, 30];

/// Clear line painted on the land side of a water edge (the water's "orilla clara").
pub const TOON_SHORE_RGB: [u8; 3] = [222, 232, 236];

// ------------------------------------------------------------------ buildings on the map

/// Roof colour of a building block: warm and muted, so the paths between houses stay readable.
pub const BUILDING_ROOF_RGB: [u8; 3] = [178, 118, 88];

/// Ink of the building block outline.
pub const BUILDING_OUTLINE_RGB: [u8; 3] = [34, 28, 24];

/// Thickness of the building outline, map texels (the outline is painted inside the block).
pub const BUILDING_OUTLINE_TEXELS: f32 = 0.8;

// ------------------------------------------------------------------ player arrow

/// Side of the player arrow on the minimap, logical pixels (square: a rotation cannot deform it).
pub const PLAYER_ARROW_PX: f32 = 18.0;

/// Side of the player arrow on the full-screen map, logical pixels.
pub const FULL_MAP_ARROW_PX: f32 = 16.0;

/// Fill of the player arrow (cream-yellow accent, legible over land and water).
pub const ARROW_FILL_RGB: [u8; 3] = [255, 226, 92];

/// Thin dark border of the player arrow.
pub const ARROW_INK_RGB: [u8; 3] = [58, 42, 10];

/// Supersampling factor of the generated arrow texture (smooth edges without a PNG asset).
pub const ARROW_SUPERSAMPLE: u32 = 4;

/// Default cache root (the main checkout's shared work dir, like the terrain bakes).
pub const DEFAULT_CACHE_ROOT: &str = "/home/nico/work/decompilacion/work/cache";

// ------------------------------------------------------------------ world <-> map

/// World `(x, z)` metres to texture `(u, v)` in `0..=1` (u = x column, v = z row). `origin` is
/// the world `(x, z)` of the map's `(0, 0)` corner (zero for main areas, the interior navmesh
/// box minimum for sub-areas).
pub fn world_to_uv(x: f32, z: f32, origin: Vec2, extent: Vec2) -> Vec2 {
    Vec2::new((x - origin.x) / extent.x, (z - origin.y) / extent.y)
}

/// Inverse of [`world_to_uv`].
pub fn uv_to_world(uv: Vec2, origin: Vec2, extent: Vec2) -> Vec2 {
    Vec2::new(uv.x * extent.x + origin.x, uv.y * extent.y + origin.y)
}

/// Texel-centre position of world `(x, z)` in a `map_px`-square map of `extent` metres:
/// texel `i` covers `[i, i+1) * extent / map_px`.
pub fn texel_of_world(x: f32, z: f32, origin: Vec2, extent: Vec2, map_px: u32) -> Vec2 {
    Vec2::new(
        (x - origin.x) / extent.x * map_px as f32 - 0.5,
        (z - origin.y) / extent.y * map_px as f32 - 0.5,
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

/// Toon band of one pixel, the id [`apply_toon_outlines`] compares between neighbours:
/// `0..TOON_BANDS` on land (the quantised relief light) and `WATER_BAND_BASE..+TOON_WATER_BANDS`
/// on water (quantised from the albedo luminance, the old sea modulation).
pub fn toon_band_index(
    albedo: [u8; 3],
    height_m: f32,
    water_level_m: Option<f32>,
    normal: Vec3,
) -> u8 {
    if water_level_m.is_some_and(|level| height_m <= level + WATER_BAND_M) {
        let luma =
            (0.2126 * albedo[0] as f32 + 0.7152 * albedo[1] as f32 + 0.0722 * albedo[2] as f32)
                / 255.0;
        let band = (luma * TOON_WATER_BANDS as f32).floor() as usize;
        return WATER_BAND_BASE
            + band.min(TOON_WATER_BANDS.saturating_sub(1)) as u8;
    }
    let diffuse = normal.dot(light_dir()).max(0.0);
    let band = (diffuse * TOON_BANDS as f32).floor() as usize;
    band.min(TOON_BANDS.saturating_sub(1)) as u8
}

/// Colour with the toon saturation boost applied around its own luminance.
pub fn saturate_rgb(rgb: [f32; 3], factor: f32) -> [u8; 3] {
    let luma = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    let mut out = [0u8; 3];
    for i in 0..3 {
        out[i] = (luma + (rgb[i] - luma) * factor).clamp(0.0, 255.0) as u8;
    }
    out
}

/// One minimap pixel in the toon finish: water near the area water level (blue, in flat bands
/// from the albedo luminance), otherwise the albedo with the posterised hill-shading applied
/// and the saturation boost on top.
pub fn shade_pixel(
    albedo: [u8; 3],
    height_m: f32,
    water_level_m: Option<f32>,
    normal: Vec3,
) -> [u8; 3] {
    let band = toon_band_index(albedo, height_m, water_level_m, normal);
    if band >= WATER_BAND_BASE {
        let n = (band - WATER_BAND_BASE) as f32 / (TOON_WATER_BANDS - 1).max(1) as f32;
        let k = 0.75 + 0.45 * n;
        return saturate_rgb(
            [
                WATER_RGB[0] as f32 * k,
                WATER_RGB[1] as f32 * k,
                WATER_RGB[2] as f32 * k,
            ],
            TOON_SATURATION,
        );
    }
    let n = band as f32 / (TOON_BANDS - 1).max(1) as f32;
    let k = SHADE_AMBIENT + (1.0 - SHADE_AMBIENT) * n;
    saturate_rgb(
        [
            albedo[0] as f32 * k,
            albedo[1] as f32 * k,
            albedo[2] as f32 * k,
        ],
        TOON_SATURATION,
    )
}

/// Toon line pass over a finished map: every pixel whose toon band ([`toon_band_index`]) differs
/// from a neighbour's gets ink - the darker side of a land band edge, the water side of the
/// shore - and the land side of a water edge gets the clear bank instead. One texel wide, so it
/// reads as a drawn contour and not as a smear.
pub fn apply_toon_outlines(rgba: &mut [u8], bands: &[u8], size: usize) {
    if size == 0 || rgba.len() < size * size * 4 || bands.len() < size * size {
        return;
    }
    let is_water = |b: u8| b >= WATER_BAND_BASE;
    let at = |x: usize, y: usize| y * size + x;
    let mut edits: Vec<(usize, [u8; 3])> = Vec::new();
    for y in 0..size {
        for x in 0..size {
            let b = bands[at(x, y)];
            let mut dark = false;
            let mut shore = false;
            let neighbours = [
                (x > 0).then(|| at(x - 1, y)),
                (x + 1 < size).then(|| at(x + 1, y)),
                (y > 0).then(|| at(x, y - 1)),
                (y + 1 < size).then(|| at(x, y + 1)),
            ];
            for nb in neighbours.into_iter().flatten() {
                let n = bands[nb];
                if n == b {
                    continue;
                }
                if is_water(b) {
                    dark = true; // the water side of any edge (other band or the shore) is ink
                } else if is_water(n) {
                    shore = true; // the land side of the water is the clear bank
                } else if b < n {
                    dark = true; // land band edge: ink the darker side
                }
            }
            if shore {
                edits.push((at(x, y), TOON_SHORE_RGB));
            } else if dark {
                edits.push((at(x, y), TOON_OUTLINE_RGB));
            }
        }
    }
    for (i, rgb) in edits {
        rgba[i * 4] = rgb[0];
        rgba[i * 4 + 1] = rgb[1];
        rgba[i * 4 + 2] = rgb[2];
    }
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
    let mut bands = vec![0u8; n * n];
    for (py, (row, band_row)) in rgba
        .chunks_exact_mut(n * 4)
        .zip(bands.chunks_exact_mut(n))
        .enumerate()
    {
        let z = (py as f32 + 0.5) * step.y;
        for (px, (pixel, band)) in row
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(band_row.iter_mut())
            .enumerate()
        {
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
            *band = toon_band_index(base, h, water_level, normal);
            let rgb = shade_pixel(base, h, water_level, normal);
            pixel[0] = rgb[0];
            pixel[1] = rgb[1];
            pixel[2] = rgb[2];
        }
    }
    apply_toon_outlines(&mut rgba, &bands, n);
    Ok(TextureData {
        width: size,
        height: size,
        rgba,
    })
}

/// [`build_map_from_heightmap`] for a loaded [`TerrainArea`] (terrain only, no buildings).
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

/// [`build_area_map`] plus the building blocks painted on top. Main areas use origin
/// `(0, 0)`, the same frame the placement sheet's world coordinates live in.
pub fn build_area_map_with_buildings(
    area: &TerrainArea,
    albedo: Option<&TextureData>,
    size: u32,
    footprints: &[BuildingFootprint],
) -> Result<TextureData, String> {
    let mut image = build_area_map(area, albedo, size)?;
    let extent = Vec2::new(area.world_extent[0], area.world_extent[1]);
    paint_buildings(&mut image.rgba, image.width, Vec2::ZERO, extent, footprints);
    Ok(image)
}

// ------------------------------------------------------------------ buildings on the map

/// One building footprint on the map: the model's collision-shape XZ rectangle (local `lo`/`hi`
/// bounds from its `_col.trcol` shape) placed in world metres with the visual spawner's own
/// transform (yaw about Y = `rot_y`, [`RotMode::YawY`], and the row's scale).
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingFootprint {
    /// Model of the placement the footprint comes from (e.g. `objects_area00_house12`).
    pub model: String,
    /// World `(x, z)` of the four rectangle corners, in local `lo`/`hi` order.
    pub corners: [Vec2; 4],
}

/// True for the model names that are painted as building blocks, decided on the decoded
/// placement sheet's model list (documented, VERIFIED against the names it holds):
///
/// - houses (`objects_*_houseNN`), the village shop (`sshop`), the village gates (`gateNN`),
///   stone walls (`stonewallNN`) and the big building (`objects_area04_building01`) are
///   structures with a closed collision footprint;
/// - state flags and flag poles (`*_flag`) are small props, fences (`fenceNN`), tents
///   (`genyatentNN`), trees, rocks, fields (`plants_*`, `ground_*`) and the numbered
///   `objects_*_objNN` props are NOT buildings and stay off the map;
/// - `wall_areaNN_*` rows are terrain wall pieces (several ship no valid geometry at all).
pub fn is_building_model(model: &str) -> bool {
    let tokens: Vec<&str> = model.split('_').collect();
    if tokens.iter().any(|t| *t == "flag") {
        return false;
    }
    tokens.iter().any(|t| {
        t.starts_with("building")
            || t.starts_with("house")
            || t.starts_with("sshop")
            || t.starts_with("gate")
            || t.starts_with("stonewall")
    })
}

/// Footprint of one placement whose shape bounds are `(lo, hi)` (model-local metres): the four
/// local XZ rectangle corners through the placement's transform.
pub fn footprint_of_placement(
    model: &str,
    placement: &Placement,
    bounds: ([f32; 3], [f32; 3]),
    rot: RotMode,
) -> BuildingFootprint {
    let (lo, hi) = bounds;
    let m = placement_transform(placement, rot);
    let mut corners = [Vec2::ZERO; 4];
    let local = [
        (lo[0], lo[2]),
        (hi[0], lo[2]),
        (hi[0], hi[2]),
        (lo[0], hi[2]),
    ];
    for (k, (cx, cz)) in local.into_iter().enumerate() {
        let w = m.transform_point3(Vec3::new(cx, 0.0, cz));
        corners[k] = Vec2::new(w.x, w.z);
    }
    BuildingFootprint {
        model: model.to_string(),
        corners,
    }
}

/// Footprints of every building placement (`is_building_model`) in `placements`, four corners in
/// world metres; the shape bounds are read once per model from the collision packs (the same
/// `_col.trcol` shapes the world walls use). Returns the footprints and how many placements
/// were skipped for having no collision shape.
pub fn building_footprints(
    placements: &[Placement],
    packs: &CollisionPacks,
    rot: RotMode,
) -> (Vec<BuildingFootprint>, usize) {
    let mut bounds: HashMap<&str, Option<([f32; 3], [f32; 3])>> = HashMap::new();
    let mut out = Vec::new();
    let mut skipped = 0usize;
    for p in placements.iter().filter(|p| is_building_model(&p.model)) {
        let b = *bounds
            .entry(p.model.as_str())
            .or_insert_with(|| packs.mesh_for_model(&p.model).ok().and_then(|m| m.bounds()));
        match b {
            Some(bounds) => out.push(footprint_of_placement(&p.model, p, bounds, rot)),
            None => skipped += 1,
        }
    }
    (out, skipped)
}

/// Building footprints of main area `area_idx`, from the placement sheet plus the collision
/// packs under `bin_root`. A missing sheet or unreadable packs are reported and yield no
/// blocks (the map is terrain-only), never an error: the map must still build.
pub fn load_building_footprints(area_idx: u8, bin_root: &Path) -> Vec<BuildingFootprint> {
    let area = format!("ha_area{area_idx:02}");
    let Some(text) = crate::game::world_objects::find_data_file("field_object_placements.tsv")
        .and_then(|p| std::fs::read_to_string(p).ok())
    else {
        eprintln!("Minimap: data/field_object_placements.tsv not found; no buildings on the map");
        return Vec::new();
    };
    // The sheet's own `model_status` snapshot lags the export index (it marks the Jubilife
    // houses `blocked` while the index exports them `ok`); what a footprint needs is the
    // collision shape, checked per model below.
    let placements = crate::game::world_objects::parse_placements_for_spawn(&text, &area);
    let packs = match CollisionPacks::load(bin_root) {
        Ok(packs) => packs,
        Err(e) => {
            eprintln!("Minimap: collision packs unreadable ({e}); no buildings on the map");
            return Vec::new();
        }
    };
    let considered = placements
        .iter()
        .filter(|p| is_building_model(&p.model))
        .count();
    let (footprints, skipped) = building_footprints(&placements, &packs, RotMode::YawY);
    println!(
        "Minimap: {area}: {} building footprints of {considered} building placements ({skipped} without a collision shape)",
        footprints.len()
    );
    footprints
}

/// Stamp distance from `p` to segment `a`-`b` (map texels).
pub fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 <= 1e-12 {
        0.0
    } else {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    };
    p.distance(a + ab * t)
}

/// True when `p` is inside (or on) the convex quad of `corners` (orientation-independent).
pub fn point_in_quad(p: Vec2, corners: &[Vec2; 4]) -> bool {
    let cross = |a: Vec2, b: Vec2, p: Vec2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    let mut positive = 0usize;
    let mut negative = 0usize;
    for k in 0..4 {
        let c = cross(corners[k], corners[(k + 1) % 4], p);
        if c > 1e-6 {
            positive += 1;
        } else if c < -1e-6 {
            negative += 1;
        }
    }
    positive == 0 || negative == 0
}

/// Paint the building blocks over a finished map: flat warm roofs with a thin dark outline
/// inside the block (`BUILDING_OUTLINE_TEXELS`). The blocks sit on their real footprints, so
/// the paths between the houses stay visible.
pub fn paint_buildings(
    rgba: &mut [u8],
    size: u32,
    origin: Vec2,
    extent: Vec2,
    footprints: &[BuildingFootprint],
) {
    let side = size as i64;
    if side <= 0 || rgba.len() < (side * side * 4) as usize {
        return;
    }
    for fp in footprints {
        let px: [Vec2; 4] = fp
            .corners
            .map(|c| world_to_uv(c.x, c.y, origin, extent) * size as f32);
        if !px.iter().all(|p| p.is_finite()) {
            continue;
        }
        let mut lo = Vec2::splat(f32::INFINITY);
        let mut hi = Vec2::splat(f32::NEG_INFINITY);
        for p in px {
            lo = lo.min(p);
            hi = hi.max(p);
        }
        let min_x = lo.x.floor().max(0.0) as i64;
        let min_y = lo.y.floor().max(0.0) as i64;
        let max_x = hi.x.ceil().min(size as f32) as i64;
        let max_y = hi.y.ceil().min(size as f32) as i64;
        for y in min_y..max_y {
            for x in min_x..max_x {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                if !point_in_quad(p, &px) {
                    continue;
                }
                let edge = (0..4)
                    .map(|k| segment_distance(p, px[k], px[(k + 1) % 4]))
                    .fold(f32::INFINITY, f32::min);
                let rgb = if edge < BUILDING_OUTLINE_TEXELS {
                    BUILDING_OUTLINE_RGB
                } else {
                    BUILDING_ROOF_RGB
                };
                let at = ((y * side + x) * 4) as usize;
                rgba[at] = rgb[0];
                rgba[at + 1] = rgb[1];
                rgba[at + 2] = rgb[2];
                rgba[at + 3] = 255;
            }
        }
    }
}

// ------------------------------------------------------------------ player arrow texture

/// Generate the player-arrow texture: a triangle pointing **up** - `-z` is up on the map, which
/// is the marker rotation 0 ([`marker_rotation_deg`]) - with a thin dark ink border and the
/// cream-yellow fill. Drawn in code (the game's font has no triangles and the port ships no PNG
/// assets), supersampled [`ARROW_SUPERSAMPLE`] times so the edges stay smooth when it rotates.
pub fn player_arrow_texture(px: u32) -> TextureData {
    let size = px.clamp(4, 256);
    let s = ARROW_SUPERSAMPLE.max(1);
    let w = (size * s) as f32;
    // Triangle in supersampled texels: tip at the top centre, base along the bottom.
    let tip = Vec2::new(w * 0.5, w * 0.06);
    let left = Vec2::new(w * 0.10, w * 0.94);
    let right = Vec2::new(w * 0.90, w * 0.94);
    let border = 1.1 * s as f32; // the "thin" outline, in supersampled texels
    let cross = |a: Vec2, b: Vec2, p: Vec2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for oy in 0..size {
        for ox in 0..size {
            let mut fill = 0u32;
            let mut ink = 0u32;
            for sy in 0..s {
                for sx in 0..s {
                    let p = Vec2::new(
                        (ox * s + sx) as f32 + 0.5,
                        (oy * s + sy) as f32 + 0.5,
                    );
                    let e0 = cross(tip, right, p);
                    let e1 = cross(right, left, p);
                    let e2 = cross(left, tip, p);
                    let inside = (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0)
                        || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0);
                    if !inside {
                        continue;
                    }
                    let d = [
                        segment_distance(p, tip, right),
                        segment_distance(p, right, left),
                        segment_distance(p, left, tip),
                    ]
                    .into_iter()
                    .fold(f32::INFINITY, f32::min);
                    if d < border {
                        ink += 1;
                    } else {
                        fill += 1;
                    }
                }
            }
            let total = (s * s) as f32;
            let covered = (fill + ink) as f32;
            if covered <= 0.0 {
                continue;
            }
            // Weighted colour of the sample: ink and fill blended by how many sub-texels each
            // gained, so the border keeps the supersampled smoothness at small sizes.
            let (wf, wi) = (fill as f32, ink as f32);
            let mix = |k: usize| {
                ((ARROW_FILL_RGB[k] as f32 * wf + ARROW_INK_RGB[k] as f32 * wi) / covered)
                    .clamp(0.0, 255.0) as u8
            };
            let at = (((oy * size + ox) * 4) as usize).min(rgba.len() - 4);
            rgba[at] = mix(0);
            rgba[at + 1] = mix(1);
            rgba[at + 2] = mix(2);
            rgba[at + 3] = (covered / total * 255.0).clamp(0.0, 255.0) as u8;
        }
    }
    TextureData {
        width: size,
        height: size,
        rgba,
    }
}

/// The arrow as a Bevy image (RGBA8 sRGB), ready for an `ImageNode`.
pub fn arrow_image_handle(images: &mut Assets<Image>, px: u32) -> Handle<Image> {
    let tex = player_arrow_texture(px);
    images.add(Image::new(
        Extent3d {
            width: tex.width,
            height: tex.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        tex.rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ))
}

// ------------------------------------------------------------------ interior maps

/// Colour of the walkable surface on an interior map.
pub const INTERIOR_FLOOR_RGB: [u8; 3] = [196, 192, 180];

/// Colour outside the walkable surface on an interior map (the void around the room).
pub const INTERIOR_VOID_RGB: [u8; 3] = [14, 16, 20];

/// Rasterise a sub-area navmesh top-down as its interior map.
///
/// Sub-areas ship no heightmap the main map generator could use (their `own_terrain` packs are
/// not loaded), but every one of them has a decoded navmesh: the walkable surface of the room.
/// Faces are painted with the same `x` columns / `z` rows convention as the main-area maps.
/// The returned `origin`/`extent` are the navmesh vertex box, so [`world_to_uv`] maps interior
/// coordinates directly. Returns `(image, origin, extent)`.
pub fn build_navmesh_map(mesh: &NavMesh, size: u32) -> Result<(TextureData, Vec2, Vec2), String> {
    let (lo, hi) = mesh.bounds().ok_or("navmesh has no vertices")?;
    if !lo.iter().all(|v| v.is_finite()) || !hi.iter().all(|v| v.is_finite()) {
        return Err("navmesh bounds are not finite".into());
    }
    let size = size.clamp(16, 4096);
    let extent = Vec2::new((hi[0] - lo[0]).max(1.0), (hi[2] - lo[2]).max(1.0));
    let origin = Vec2::new(lo[0], lo[2]);
    let side = size as f32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for px in rgba.chunks_exact_mut(4) {
        px.copy_from_slice(&[INTERIOR_VOID_RGB[0], INTERIOR_VOID_RGB[1], INTERIOR_VOID_RGB[2], 255]);
    }
    // Top-left world corner maps to texel (0, 0); +x grows with the column, +z with the row.
    let to_px = |x: f32, z: f32| Vec2::new((x - origin.x) / extent.x * side, (z - origin.y) / extent.y * side);
    for face in 0..mesh.faces.len() {
        let vs = mesh.face_vertices(face as u32);
        if vs.len() < 3 {
            continue;
        }
        let a = to_px(vs[0][0], vs[0][2]);
        for i in 1..vs.len() - 1 {
            let b = to_px(vs[i][0], vs[i][2]);
            let c = to_px(vs[i + 1][0], vs[i + 1][2]);
            paint_triangle(&mut rgba, size, a, b, c, INTERIOR_FLOOR_RGB);
        }
    }
    Ok((TextureData { width: size, height: size, rgba }, origin, extent))
}

/// Fill the pixels whose centre lies inside the triangle `abc` (pixel-space, `u`/`v` axes).
fn paint_triangle(rgba: &mut [u8], size: u32, a: Vec2, b: Vec2, c: Vec2, rgb: [u8; 3]) {
    let side = size as i64;
    let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as i64;
    let max_x = a.x.max(b.x).max(c.x).ceil().min(size as f32) as i64;
    let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as i64;
    let max_y = a.y.max(b.y).max(c.y).ceil().min(size as f32) as i64;
    let edge = |p: Vec2, q: Vec2, x: f32, y: f32| (q.x - p.x) * (y - p.y) - (q.y - p.y) * (x - p.x);
    // Orientation-independent inside test: same sign on all three edges (zeros count as inside).
    let area = edge(a, b, c.x, c.y);
    let positive = area >= 0.0;
    let inside = |x: f32, y: f32| {
        let e0 = edge(a, b, x, y);
        let e1 = edge(b, c, x, y);
        let e2 = edge(c, a, x, y);
        if positive {
            e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0
        } else {
            e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0
        }
    };
    for py in min_y..max_y {
        for px in min_x..max_x {
            let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
            if !inside(cx, cy) {
                continue;
            }
            let at = ((py * side + px) * 4) as usize;
            rgba[at] = rgb[0];
            rgba[at + 1] = rgb[1];
            rgba[at + 2] = rgb[2];
            rgba[at + 3] = 255;
        }
    }
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
/// equals the current signature (the building footprints are only read when the map really
/// has to be generated: they do not enter the signature, so a look change that alters them
/// must bump [`MINIMAP_VERSION`]).
pub fn bake_map_to_cache(
    area: &TerrainArea,
    albedo: Option<&TextureData>,
    bin_root: Option<&Path>,
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
    let footprints = bin_root
        .map(|root| load_building_footprints(area.area, root))
        .unwrap_or_default();
    let image = build_area_map_with_buildings(area, albedo, size, &footprints)?;
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
    let footprints = bin_root
        .map(|root| load_building_footprints(area_idx, root))
        .unwrap_or_default();
    let image = build_area_map_with_buildings(area, albedo, size, &footprints)?;
    Ok(ResolvedMap {
        image,
        source: MapSource::Generated,
        millis: started.elapsed().as_secs_f64() * 1000.0,
    })
}

// ------------------------------------------------------------------ Bevy integration

/// The minimap frame (top-right), child of the world HUD root so the battle screen hides it.
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

/// Whether the player wants the corner minimap on screen (Graphics > Display > Minimap). The
/// compass and the full-screen map stay either way.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimapShown(pub bool);

impl Default for MinimapShown {
    fn default() -> Self {
        Self(true)
    }
}

/// What the minimap currently shows.
#[derive(Resource, Debug, Default)]
pub struct MinimapState {
    /// Main-area index the current map belongs to (`None` inside an interior).
    pub area: Option<u8>,
    /// Sub-area zone id of the current map (`None` in a main area).
    pub interior: Option<String>,
    /// World `(x, z)` of the map's `(0, 0)` corner (zero for main areas).
    pub origin: Vec2,
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
                apply_minimap_setting,
            )
                .chain(),
        );
    }
}

/// Spawn the HUD nodes under the world HUD root (no-op without one, e.g. headless apps).
/// Without an image store (logic-only tests) the markers fall back to a plain accent square.
pub fn setup_minimap_ui(
    mut commands: Commands,
    hud: Query<Entity, With<WorldHudRoot>>,
    mut images: Option<ResMut<Assets<Image>>>,
) {
    let Ok(hud) = hud.single() else {
        return;
    };
    let arrow = images
        .as_mut()
        .map(|images| arrow_image_handle(images, PLAYER_ARROW_PX as u32 * ARROW_SUPERSAMPLE));
    commands.entity(hud).with_children(|root| {
        root.spawn((
            MinimapRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                top: Val::Px(16.0),
                width: Val::Px(FRAME_PX),
                height: Val::Px(FRAME_PX),
                // A round frame (fully rounded corners) with a cream ring, like the game's own
                // field map.
                border: UiRect::all(Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.92)),
            BorderColor::all(Color::srgba(0.96, 0.93, 0.82, 0.95)),
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
                    // The overflow clip of the frame is a rectangle: the picture rounds its own
                    // corners so the map fills the circular ring.
                    border_radius: BorderRadius::all(Val::Percent(50.0)),
                    ..default()
                },
                ImageNode {
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
            ));
            // The player arrow: a square node (a rotation cannot deform it) showing the
            // generated arrow texture, pointing where the player looks (rotated in
            // `update_minimap_hud`, the same facing the compass strip reads).
            let mut marker = frame.spawn((
                MinimapPlayerMarker,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(FRAME_PX * 0.5 - PLAYER_ARROW_PX * 0.5),
                    top: Val::Px(FRAME_PX * 0.5 - PLAYER_ARROW_PX * 0.5),
                    width: Val::Px(PLAYER_ARROW_PX),
                    height: Val::Px(PLAYER_ARROW_PX),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                UiTransform::default(),
            ));
            if let Some(handle) = arrow.clone() {
                marker.insert(ImageNode {
                    image: handle,
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                });
            }
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
                    // The port's own player arrow on the full-screen map (the original UI path
                    // hides it and draws the game's own arrow instead).
                    let mut marker = panel.spawn((
                        FullMapMarker,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(50.0),
                            top: Val::Percent(50.0),
                            width: Val::Px(FULL_MAP_ARROW_PX),
                            height: Val::Px(FULL_MAP_ARROW_PX),
                            margin: UiRect {
                                left: Val::Px(-FULL_MAP_ARROW_PX * 0.5),
                                top: Val::Px(-FULL_MAP_ARROW_PX * 0.5),
                                ..default()
                            },
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        UiTransform::default(),
                    ));
                    if let Some(handle) = arrow.clone() {
                        marker.insert(ImageNode {
                            image: handle,
                            image_mode: NodeImageMode::Stretch,
                            ..default()
                        });
                    }
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

/// Regenerate the map when the active main area changes (and once at start). Inside an interior
/// the map is rasterised from the sub-area navmesh; when the scene has no map data at all the
/// frame hides instead of keeping a stale main-area map.
#[allow(clippy::too_many_arguments)]
pub fn rebuild_minimap_on_zone_change(
    mut state: ResMut<MinimapState>,
    config: Res<MinimapConfig>,
    zone: Option<Res<crate::game::ui::ActiveZoneInfo>>,
    terrain: Option<Res<crate::game::zone::TerrainResource>>,
    navmesh: Option<Res<crate::game::navigation::GameNavmeshResource>>,
    images: Option<ResMut<Assets<Image>>>,
    mut minimap_q: Query<&mut ImageNode, With<MinimapImage>>,
    mut full_q: Query<&mut ImageNode, (With<FullMapImage>, Without<MinimapImage>)>,
    mut root_q: Query<&mut Visibility, With<MinimapRoot>>,
) {
    let Some(mut images) = images else {
        return; // headless app without an image store: nothing to show
    };
    let Some(zone) = zone else { return };
    let Some(area_idx) = zone.area_idx() else {
        rebuild_interior_map(
            &mut state,
            &zone.area,
            navmesh.as_deref(),
            &mut images,
            &mut minimap_q,
            &mut full_q,
            &mut root_q,
        );
        return;
    };
    if state.area == Some(area_idx) && state.interior.is_none() && state.map_px == config.size {
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
    state.interior = None;
    state.origin = Vec2::ZERO;
    state.extent = Vec2::new(terrain.area.world_extent[0], terrain.area.world_extent[1]);
    state.map_px = resolved.image.width;
    state.image = Some(handle);
    show_minimap_frame(&mut root_q, true);
}

/// Interior branch: rasterise the sub-area navmesh, or hide the frame when there is nothing to
/// draw. Waits (frame by frame, without hiding) for the interior navmesh the zone change
/// installs, so the map never mixes two frames.
#[allow(clippy::too_many_arguments)]
fn rebuild_interior_map(
    state: &mut MinimapState,
    zone: &str,
    navmesh: Option<&crate::game::navigation::GameNavmeshResource>,
    images: &mut Assets<Image>,
    minimap_q: &mut Query<&mut ImageNode, With<MinimapImage>>,
    full_q: &mut Query<&mut ImageNode, (With<FullMapImage>, Without<MinimapImage>)>,
    root_q: &mut Query<&mut Visibility, With<MinimapRoot>>,
) {
    if state.interior.as_deref() == Some(zone) {
        return;
    }
    let Some(navmesh) = navmesh else {
        // No navmesh yet: keep waiting without hiding (the scene is still loading).
        return;
    };
    if navmesh.nav.area != crate::game::interior::INTERIOR_TERRAIN_AREA {
        // The resource still holds the previous main-area navmesh: the zone change has not
        // swapped it yet.
        return;
    }
    match build_navmesh_map(navmesh.nav.primary(), INTERIOR_MAP_SIZE) {
        Ok((image, origin, extent)) => {
            let (w, h) = (image.width.max(1), image.height.max(1));
            let handle = images.add(Image::new(
                Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                image.rgba,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            ));
            println!("Minimap: interior {zone} map {w}x{h} from the sub-area navmesh");
            for mut node in minimap_q.iter_mut() {
                node.image = handle.clone();
            }
            for mut node in full_q.iter_mut() {
                node.image = handle.clone();
            }
            state.area = None;
            state.interior = Some(zone.to_string());
            state.origin = origin;
            state.extent = extent;
            state.map_px = image.width;
            state.image = Some(handle);
            show_minimap_frame(root_q, true);
        }
        Err(e) => {
            // No drawable data: hide the frame cleanly instead of leaving the main-area map up.
            eprintln!("Minimap: interior {zone} has no map data: {e}");
            state.area = None;
            state.interior = Some(zone.to_string());
            state.image = None;
            state.extent = Vec2::ZERO;
            state.map_px = 0;
            show_minimap_frame(root_q, false);
        }
    }
}

/// Show or hide the minimap frame. Uses `Inherited` (never `Visible`) to restore it, so the
/// hidden world HUD root keeps hiding it in battle and menus.
fn show_minimap_frame(root_q: &mut Query<&mut Visibility, With<MinimapRoot>>, show: bool) {
    let wanted = if show { Visibility::Inherited } else { Visibility::Hidden };
    for mut visibility in root_q.iter_mut() {
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// Keep the corner frame hidden while the player turned the minimap off (it runs last in the
/// minimap chain, so the zone-change logic can never bring it back) and give the frame back to
/// the other systems once it is turned on again.
pub fn apply_minimap_setting(
    shown: Option<Res<MinimapShown>>,
    mut root_q: Query<&mut Visibility, With<MinimapRoot>>,
    mut forced: Local<bool>,
) {
    let wanted = shown.is_none_or(|s| s.0);
    if !wanted {
        *forced = true;
        for mut visibility in root_q.iter_mut() {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
        }
    } else if *forced {
        *forced = false;
        for mut visibility in root_q.iter_mut() {
            *visibility = Visibility::Inherited;
        }
    }
}

/// Crop the map around the player and rotate the marker; update the compass strip.
#[allow(clippy::too_many_arguments)]
pub fn update_minimap_hud(
    state: Res<MinimapState>,
    player: Query<&Transform, With<crate::game::PlayerAvatar>>,
    mut minimap_q: Query<&mut ImageNode, With<MinimapImage>>,
    mut marker_q: Query<(&mut Node, &mut UiTransform), With<MinimapPlayerMarker>>,
    mut full_marker_q: Query<(&mut Node, &mut UiTransform), (With<FullMapMarker>, Without<MinimapPlayerMarker>)>,
    mut compass_q: Query<&mut Text, With<CompassText>>,
) {
    if state.image.is_none() || state.map_px == 0 {
        return;
    }
    let Ok(transform) = player.single() else {
        return;
    };
    let (x, z) = (transform.translation.x, transform.translation.z);
    let player_texel = texel_of_world(x, z, state.origin, state.extent, state.map_px);
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
        node.left = Val::Px(FRAME_PX * 0.5 - PLAYER_ARROW_PX * 0.5 + offset.x);
        node.top = Val::Px(FRAME_PX * 0.5 - PLAYER_ARROW_PX * 0.5 + offset.y);
        *ui = UiTransform::from_rotation(rotation);
    }
    // The port's own arrow on the full-screen map: place it with the current map's origin and
    // spin it with the facing (the original-UI path hides it and draws the game's arrow instead).
    let uv = full_map_marker_uv(x, z, state.origin, state.extent);
    for (mut node, mut ui) in &mut full_marker_q {
        node.left = Val::Percent(uv.x * 100.0);
        node.top = Val::Percent(uv.y * 100.0);
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

/// The overlay is only visible while open in the world state and a map exists (cheap enough to
/// set every frame; the change-detection gate proved fragile across state transitions).
pub fn full_map_visibility_system(
    state: Res<MinimapState>,
    game_state: Option<Res<State<GameState>>>,
    mut full_q: Query<&mut Visibility, With<FullMapRoot>>,
) {
    let in_world = game_state
        .as_ref()
        .is_none_or(|s| matches!(s.get(), GameState::World));
    let wanted = if state.open && in_world && state.image.is_some() {
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
pub fn full_map_marker_uv(x: f32, z: f32, origin: Vec2, extent: Vec2) -> Vec2 {
    world_to_uv(x, z, origin, extent).clamp(Vec2::ZERO, Vec2::ONE)
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
        let zero = Vec2::ZERO;
        assert_eq!(world_to_uv(0.0, 0.0, zero, extent), Vec2::ZERO);
        assert_eq!(world_to_uv(1025.0, 1025.0, zero, extent), Vec2::ONE);
        assert!(
            (uv_to_world(world_to_uv(512.3, 100.7, zero, extent), zero, extent)
                - Vec2::new(512.3, 100.7))
            .length()
                < 1e-3
        );
        // Texel centres: the world point of texel centre `(i + 0.5, j + 0.5)` maps back to `(i, j)`.
        let centre = uv_to_world(Vec2::new(511.5, 255.5) / 1024.0, zero, extent);
        let t = texel_of_world(centre.x, centre.y, zero, extent, 1024);
        assert!(
            (t - Vec2::new(511.0, 255.0)).length() < 1e-3,
            "texel centres line up: {t:?}"
        );
        assert!((texel_of_world(0.5, 0.5, zero, extent, 1024) - Vec2::splat(0.0)).length() < 0.01);
    }

    #[test]
    fn interior_origin_shifts_the_mapping() {
        // An interior's map covers the navmesh box, not the world origin: a room at
        // x 428..532, z 420..607 maps its corner to uv (0, 0) and its far corner to (1, 1).
        let origin = Vec2::new(428.0, 420.0);
        let extent = Vec2::new(104.0, 187.0);
        assert_eq!(world_to_uv(428.0, 420.0, origin, extent), Vec2::ZERO);
        assert!(
            (world_to_uv(532.0, 607.0, origin, extent) - Vec2::ONE).length() < 1e-5
        );
        let back = uv_to_world(Vec2::new(0.25, 0.75), origin, extent);
        assert!((back - Vec2::new(428.0 + 26.0, 420.0 + 140.25)).length() < 1e-3);
        // The texel of the room centre sits in the middle of the map.
        let t = texel_of_world(480.0, 513.5, origin, extent, 512);
        assert!((t - Vec2::splat(255.5)).length() < 0.5, "{t:?}");
        // The full-map marker uv is clamped into the map.
        let uv = full_map_marker_uv(400.0, 500.0, origin, extent);
        assert!(uv.x == 0.0 && (uv.y - 80.0 / 187.0).abs() < 1e-5, "{uv:?}");
    }

    #[test]
    fn navmesh_map_paints_the_walkable_face() {
        use crate::assets::navmesh::{NavEdge, NavFace, NavMesh};
        // One square face 0..10 m in x and z, at y = 2.
        let vertices = vec![
            [0.0, 2.0, 0.0],
            [10.0, 2.0, 0.0],
            [10.0, 2.0, 10.0],
            [0.0, 2.0, 10.0],
        ];
        let edges = vec![
            NavEdge { a: 0, b: 1, opposite_edge: u32::MAX, opposite_face: u32::MAX, flags: 0 },
            NavEdge { a: 1, b: 2, opposite_edge: u32::MAX, opposite_face: u32::MAX, flags: 0 },
            NavEdge { a: 2, b: 3, opposite_edge: u32::MAX, opposite_face: u32::MAX, flags: 0 },
            NavEdge { a: 3, b: 0, opposite_edge: u32::MAX, opposite_face: u32::MAX, flags: 0 },
        ];
        let faces = vec![NavFace { start_edge: 0, num_edges: 4 }];
        let mesh = NavMesh::from_parts(vertices, faces.clone(), edges.clone()).expect("synthetic navmesh");
        let (image, origin, extent) = build_navmesh_map(&mesh, 64).expect("raster");
        assert_eq!((image.width, image.height), (64, 64));
        assert_eq!(origin, Vec2::ZERO);
        assert_eq!(extent, Vec2::splat(10.0));
        let pixel = |x: u32, y: u32| {
            let at = ((y * 64 + x) * 4) as usize;
            [image.rgba[at], image.rgba[at + 1], image.rgba[at + 2]]
        };
        // Centre of the face is walkable floor colour; the corners are too (the face spans it).
        assert_eq!(pixel(32, 32), INTERIOR_FLOOR_RGB);
        assert_eq!(pixel(1, 1), INTERIOR_FLOOR_RGB);
        assert_eq!(pixel(62, 62), INTERIOR_FLOOR_RGB);

        // Orientation: a face covering only the low quadrant of the box (an unreferenced far
        // vertex sets the bounds) paints the low columns/rows only: +x grows with the column
        // and +z with the row, exactly like the main-area generated maps.
        let quad_vertices = vec![
            [0.0, 2.0, 0.0],
            [5.0, 2.0, 0.0],
            [5.0, 2.0, 5.0],
            [0.0, 2.0, 5.0],
            [10.0, 2.0, 10.0],
        ];
        let quad = NavMesh::from_parts(quad_vertices, faces.clone(), edges.clone()).expect("quad");
        let (image, origin, extent) = build_navmesh_map(&quad, 64).expect("raster");
        assert_eq!(origin, Vec2::ZERO);
        assert_eq!(extent, Vec2::splat(10.0));
        let pixel = |x: u32, y: u32| {
            let at = ((y * 64 + x) * 4) as usize;
            [image.rgba[at], image.rgba[at + 1], image.rgba[at + 2]]
        };
        assert_eq!(pixel(10, 10), INTERIOR_FLOOR_RGB, "low x, low z: inside");
        assert_eq!(pixel(50, 10), INTERIOR_VOID_RGB, "+x grows with the column");
        assert_eq!(pixel(10, 50), INTERIOR_VOID_RGB, "+z grows with the row");
        // A navmesh with no vertices fails cleanly.
        if let Ok(empty) = NavMesh::from_parts(Vec::new(), Vec::new(), Vec::new()) {
            assert!(build_navmesh_map(&empty, 64).is_err());
        }
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
    fn toon_finish_posterises_the_relief_to_flat_bands() {
        // Any normal: the relief light must fall into exactly TOON_BANDS flat shades (white
        // albedo makes the shade a pure function of the band), each visibly apart.
        let mut shades = std::collections::HashSet::new();
        for i in 0..=32 {
            for j in 0..=32 {
                let n = Vec3::new(-1.0 + i as f32 / 16.0, 1.0, -1.0 + j as f32 / 16.0).normalize();
                let band = toon_band_index([255, 255, 255], 50.0, None, n);
                assert!((band as usize) < TOON_BANDS, "land band in range: {band}");
                shades.insert(shade_pixel([255, 255, 255], 50.0, None, n));
            }
        }
        assert_eq!(
            shades.len(),
            TOON_BANDS,
            "posterised to flat shades: {shades:?}"
        );
        let mut sorted: Vec<[u8; 3]> = shades.into_iter().collect();
        sorted.sort_by_key(|c| c[0]);
        for w in sorted.windows(2) {
            assert!(
                w[1][0] as i32 - w[0][0] as i32 >= 8,
                "flat shades are visibly apart: {sorted:?}"
            );
        }
        // Water: its own band range, quantised from the albedo luminance.
        let mut water = std::collections::HashSet::new();
        for luma in [0u8, 90, 200, 255] {
            let band = toon_band_index([luma, luma, luma], 0.0, Some(0.0), Vec3::Y);
            assert!(band >= WATER_BAND_BASE, "water id range: {band}");
            water.insert(band);
        }
        assert_eq!(water.len(), TOON_WATER_BANDS, "flat water bands: {water:?}");
    }

    #[test]
    fn toon_saturation_boosts_colour_around_its_luminance() {
        // Grey stays grey (no tint invented).
        assert_eq!(saturate_rgb([128.0, 128.0, 128.0], TOON_SATURATION), [128, 128, 128]);
        // The dominant channel grows away from the luminance, the others shrink towards it.
        let red = saturate_rgb([200.0, 100.0, 100.0], TOON_SATURATION);
        assert!(red[0] > 200 && red[1] < 100, "{red:?}");
        let blue = saturate_rgb([60.0, 90.0, 180.0], TOON_SATURATION);
        assert!(blue[2] > 180 && blue[0] < 60, "{blue:?}");
    }

    #[test]
    fn toon_outlines_ink_band_edges_and_mark_the_shore() {
        // 4x4 map, same columns on every row: land band 0 | land band 2 | water band 2 |
        // water band 0.
        let size = 4usize;
        let row: [u8; 4] = [0, 2, WATER_BAND_BASE + 2, WATER_BAND_BASE];
        let bands: Vec<u8> = row.iter().copied().cycle().take(size * size).collect();
        let mut rgba = vec![200u8; size * size * 4];
        apply_toon_outlines(&mut rgba, &bands, size);
        let px = |x: usize, y: usize| {
            let i = (y * size + x) * 4;
            [rgba[i], rgba[i + 1], rgba[i + 2]]
        };
        // The darker side of the land band edge is inked.
        assert_eq!(px(0, 1), TOON_OUTLINE_RGB);
        // The land side of the water edge is the clear bank, not ink.
        assert_eq!(px(1, 1), TOON_SHORE_RGB);
        // The water side of the shore is ink, and so is a darker water band next to a lighter one.
        assert_eq!(px(2, 1), TOON_OUTLINE_RGB);
        assert_eq!(px(3, 1), TOON_OUTLINE_RGB);
        // A uniform map keeps every pixel untouched.
        let mut flat = vec![200u8; size * size * 4];
        apply_toon_outlines(&mut flat, &vec![0u8; size * size], size);
        assert!(flat
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p == &[200, 200, 200, 200]));
    }

    #[test]
    fn building_model_filter_follows_the_documented_list() {
        for m in [
            "objects_area00_house01",
            "objects_area00_house12",
            "objects_area00_sshop01",
            "objects_area00_gate01",
            "objects_stonewall02",
            "objects_area04_building01",
        ] {
            assert!(is_building_model(m), "{m} is a building");
        }
        for m in [
            "objects_area00_house06_b_flag",
            "objects_area00_fence01",
            "objects_area00_obj09",
            "objects_genyatent01_black",
            "objects_genyatent01_white",
            "ground_rock13_gray",
            "ground_area00_fallentree01",
            "plants_area00_farm01",
            "wall_area00_02",
            "ground_area04_rock_wall02",
        ] {
            assert!(!is_building_model(m), "{m} is not a building");
        }
    }

    #[test]
    fn footprint_of_placement_rotates_scales_and_places_the_shape_box() {
        // A 2x4 m local box (x -1..1, z -2..2), scaled x2, yawed 90 degrees, at (10, 20).
        let p = Placement {
            model: "objects_area00_house01".into(),
            pos: [10.0, 0.0, 20.0],
            rot_deg: [0.0, 90.0, 0.0],
            scale: [2.0, 1.0, 2.0],
        };
        let fp = footprint_of_placement(&p.model, &p, ([-1.0, 0.0, -2.0], [1.0, 3.0, 2.0]), RotMode::YawY);
        // Yaw 90: local +x maps to world -z and local +z to world +x; scale doubles both.
        let want = [
            Vec2::new(6.0, 22.0),
            Vec2::new(6.0, 18.0),
            Vec2::new(14.0, 18.0),
            Vec2::new(14.0, 22.0),
        ];
        for (got, want) in fp.corners.iter().zip(want) {
            assert!((*got - want).length() < 1e-4, "{got:?} != {want:?}");
        }
        assert_eq!(fp.model, "objects_area00_house01");
    }

    #[test]
    fn paint_buildings_paints_a_known_rectangle_with_its_outline() {
        // 32x32 map, 2 m per texel: a block from (4, 8) to (12, 16) m covers texels x 2..6,
        // y 4..8, its outline painted inside the block.
        let size = 32u32;
        let mut rgba = vec![0u8; (size * size * 4) as usize];
        let fp = BuildingFootprint {
            model: "objects_area00_house01".into(),
            corners: [
                Vec2::new(4.0, 8.0),
                Vec2::new(12.0, 8.0),
                Vec2::new(12.0, 16.0),
                Vec2::new(4.0, 16.0),
            ],
        };
        paint_buildings(&mut rgba, size, Vec2::ZERO, Vec2::splat(64.0), &[fp]);
        let px = |x: u32, y: u32| {
            let i = ((y * size + x) * 4) as usize;
            [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
        };
        let roof = [BUILDING_ROOF_RGB[0], BUILDING_ROOF_RGB[1], BUILDING_ROOF_RGB[2], 255];
        let ink = [BUILDING_OUTLINE_RGB[0], BUILDING_OUTLINE_RGB[1], BUILDING_OUTLINE_RGB[2], 255];
        assert_eq!(px(3, 6), roof, "interior is the roof colour");
        assert_eq!(px(4, 6), roof);
        assert_eq!(px(2, 6), ink, "left edge is inked");
        assert_eq!(px(5, 6), ink, "right edge is inked");
        assert_eq!(px(3, 4), ink, "top edge is inked");
        assert_eq!(px(3, 7), ink, "bottom edge is inked");
        assert_eq!(px(1, 6), [0, 0, 0, 0], "outside stays untouched");
        assert_eq!(px(6, 6), [0, 0, 0, 0]);
    }

    #[test]
    fn player_arrow_texture_points_up_with_an_ink_border() {
        let tex = player_arrow_texture(16);
        assert_eq!((tex.width, tex.height), (16, 16));
        let px = |x: u32, y: u32| {
            let i = ((y * 16 + x) * 4) as usize;
            [tex.rgba[i], tex.rgba[i + 1], tex.rgba[i + 2], tex.rgba[i + 3]]
        };
        // The tip (top centre) and the base (bottom centre) are covered; the corners are not.
        assert!(px(8, 2)[3] > 0, "tip covered: {:?}", px(8, 2));
        assert!(px(8, 14)[3] > 0, "base covered: {:?}", px(8, 14));
        assert_eq!(px(0, 0)[3], 0);
        assert_eq!(px(15, 15)[3], 0);
        // The centre is the pure fill.
        assert_eq!(px(8, 9), [ARROW_FILL_RGB[0], ARROW_FILL_RGB[1], ARROW_FILL_RGB[2], 255]);
        // There is a darker (ink) border: several opaque dark pixels exist and the darkest
        // one is clearly ink (the tip's whole interior lies within the border width).
        let luma = |p: [u8; 4]| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32;
        let dark: Vec<f32> = (0..16)
            .flat_map(|y| (0..16).map(move |x| (x, y)))
            .map(|(x, y)| px(x, y))
            .filter(|p| p[3] > 200 && luma(*p) < 160.0)
            .map(luma)
            .collect();
        assert!(dark.len() > 4, "ink border pixels: {}", dark.len());
        let darkest = dark.iter().copied().fold(f32::INFINITY, f32::min);
        assert!(darkest < 80.0, "a clearly ink pixel exists: {darkest}");
        // Mirror symmetry about the vertical axis (the arrow points straight up).
        for y in 0..16 {
            for x in 0..8 {
                let a = px(x, y);
                let b = px(15 - x, y);
                for k in 0..3 {
                    assert!(
                        (a[k] as i32 - b[k] as i32).abs() <= 2,
                        "symmetric at ({x},{y}): {a:?} vs {b:?}"
                    );
                }
            }
        }
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

    /// A navmesh resource with one 10x10 m quad face at `y = 2` (or empty for the no-data case).
    fn synthetic_navmesh_resource(area: u8, empty: bool) -> crate::game::navigation::GameNavmeshResource {
        use crate::assets::navmesh::{AreaNavigation, NavEdge, NavFace, NavMesh};
        let mesh = if empty {
            NavMesh::from_parts(Vec::new(), Vec::new(), Vec::new()).unwrap()
        } else {
            let vertices = vec![
                [0.0, 2.0, 0.0],
                [10.0, 2.0, 0.0],
                [10.0, 2.0, 10.0],
                [0.0, 2.0, 10.0],
            ];
            let edge = |a, b| NavEdge { a, b, opposite_edge: u32::MAX, opposite_face: u32::MAX, flags: 0 };
            let edges = vec![edge(0, 1), edge(1, 2), edge(2, 3), edge(3, 0)];
            let faces = vec![NavFace { start_edge: 0, num_edges: 4 }];
            NavMesh::from_parts(vertices, faces, edges).unwrap()
        };
        crate::game::navigation::GameNavmeshResource {
            nav: std::sync::Arc::new(AreaNavigation { area, meshes: vec![mesh] }),
        }
    }

    fn minimap_root_visibility(app: &mut App) -> Visibility {
        let mut query = app
            .world_mut()
            .query_filtered::<&Visibility, With<MinimapRoot>>();
        *query.iter(app.world()).next().expect("minimap root")
    }

    #[test]
    fn interior_zone_change_shows_the_room_map_and_leaving_restores() {
        let mut app = minimap_test_app(0);
        let main_image = app.world().resource::<MinimapState>().image.clone().unwrap();
        assert_eq!(minimap_root_visibility(&mut app), Visibility::Inherited);

        // Enter an interior: the sub-area navmesh (its `area` pin is the interior sentinel)
        // replaces the main-area one and the placeholder terrain is installed.
        app.insert_resource(crate::game::interior::placeholder_terrain());
        app.insert_resource(synthetic_navmesh_resource(
            crate::game::interior::INTERIOR_TERRAIN_AREA,
            false,
        ));
        app.insert_resource(crate::game::ui::ActiveZoneInfo {
            area: "ha_area00_s03_a".into(),
            name: "Interior".into(),
        });
        app.update();
        let state = app.world().resource::<MinimapState>();
        assert_eq!(state.area, None, "the main-area map is not claimed inside");
        assert_eq!(state.interior.as_deref(), Some("ha_area00_s03_a"));
        assert_eq!(state.origin, Vec2::ZERO, "the synthetic quad starts at the origin");
        assert_eq!(state.extent, Vec2::splat(10.0));
        assert_eq!(state.map_px, INTERIOR_MAP_SIZE);
        let interior_image = state.image.clone().expect("interior map wired");
        assert_ne!(interior_image.id(), main_image.id(), "the room map replaced it");
        assert_eq!(minimap_root_visibility(&mut app), Visibility::Inherited);

        // Leaving back to a main area restores the generated area map.
        app.insert_resource(synthetic_terrain(1, 1025.0, 300.0));
        app.insert_resource(synthetic_navmesh_resource(1, false));
        app.insert_resource(crate::game::ui::ActiveZoneInfo {
            area: "ha_area01".into(),
            name: "Area 01".into(),
        });
        app.update();
        let state = app.world().resource::<MinimapState>();
        assert_eq!(state.area, Some(1), "back on a main area");
        assert_eq!(state.interior, None);
        assert_eq!(state.origin, Vec2::ZERO);
        assert_eq!(state.map_px, 64, "back to the configured main map size");
        assert_ne!(
            state.image.clone().unwrap().id(),
            interior_image.id(),
            "the interior map is not left on screen"
        );
        assert_eq!(minimap_root_visibility(&mut app), Visibility::Inherited);
    }

    #[test]
    fn interior_without_map_data_hides_the_frame_cleanly() {
        let mut app = minimap_test_app(0);
        app.insert_resource(crate::game::interior::placeholder_terrain());
        app.insert_resource(synthetic_navmesh_resource(
            crate::game::interior::INTERIOR_TERRAIN_AREA,
            true,
        ));
        app.insert_resource(crate::game::ui::ActiveZoneInfo {
            area: "ha_area05_s02".into(),
            name: "Interior".into(),
        });
        app.update();
        let state = app.world().resource::<MinimapState>();
        assert_eq!(state.interior.as_deref(), Some("ha_area05_s02"));
        assert!(state.image.is_none(), "no data -> no map image");
        assert_eq!(
            minimap_root_visibility(&mut app),
            Visibility::Hidden,
            "no data -> the frame hides instead of keeping the previous map"
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
    fn real_jubilife_footprints_and_blocks_come_from_the_game_data() {
        let Some(root) = game_bin_root() else {
            eprintln!("[skip] game bin root not found (set PLA_BIN_ROOT)");
            return;
        };
        let Ok(area) = TerrainArea::load(&root, 0) else {
            eprintln!("[skip] area 00 terrain not loadable");
            return;
        };
        let footprints = load_building_footprints(0, &root);
        assert!(
            footprints.len() >= 20,
            "the village is full of houses: {} footprints",
            footprints.len()
        );
        let (w, d) = (area.world_extent[0], area.world_extent[1]);
        for fp in &footprints {
            for c in fp.corners {
                assert!(c.is_finite(), "{}: finite corners", fp.model);
                assert!(
                    c.x > -2.0 && c.x < w + 2.0 && c.y > -2.0 && c.y < d + 2.0,
                    "{}: corner {c:?} inside the area {w}x{d}",
                    fp.model
                );
            }
            let side = (fp.corners[1] - fp.corners[0])
                .length()
                .max((fp.corners[2] - fp.corners[1]).length());
            assert!(side > 0.5 && side < 120.0, "{}: box side {side} m", fp.model);
        }
        // The blocks reach the finished map: the roof colour has to appear on it.
        let albedo = cached_albedo(0);
        let map = build_area_map_with_buildings(&area, albedo.as_ref(), 512, &footprints)
            .expect("area 00 map");
        let roof = map
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[..3] == BUILDING_ROOF_RGB)
            .count();
        assert!(
            roof > 100,
            "building blocks painted: {roof} roof texels of {} footprints",
            footprints.len()
        );
        println!(
            "area 00: {} building footprints, {roof} roof texels on the 512 map",
            footprints.len()
        );
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
