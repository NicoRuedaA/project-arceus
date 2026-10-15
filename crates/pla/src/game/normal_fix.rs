//! Repairs the two-channel normal maps of the exported characters and Pokémon.
//!
//! The decoded game textures store tangent-space normals as two channels (the console's BC5 / RG
//! format). The exporter wrote them as plain RGBA PNG files with the blue channel left at 0.
//! Bevy reads `(r, g, b) * 2 - 1`, so every texel became `(x, y, -1)`: a normal pointing INTO the
//! surface. The lighting of every character was therefore inverted wherever the map is nearly flat
//! (almost everywhere): faces and bodies were dark where they face the sun and bright on the side
//! facing away, with jagged "cracks" along the terminator. This plugin rebuilds the missing
//! component (`z = sqrt(1 - x^2 - y^2)`) once, in place, for every normal map used by a material
//! whose blue channel is entirely 0. Maps with real blue data are never touched.
//!
//! The fix is generic (any `StandardMaterial` normal map) so it also covers models added later.

use std::collections::HashSet;

use bevy::asset::AssetId;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// Frames a not-yet-loaded normal map is waited for before it is given up on.
const MAX_WAIT_FRAMES: u32 = 3600;

/// True when `rgba` (8-bit RGBA, any size) is a two-channel normal map exported with B = 0:
/// at least one texel and the blue channel is 0 everywhere.
pub fn is_two_channel_normal_map(rgba: &[u8]) -> bool {
    rgba.len() >= 4 && rgba.len() % 4 == 0 && rgba.chunks_exact(4).all(|px| px[2] == 0)
}

/// How much of the decoded relief is kept (1 = as decoded). The relief of the character maps is
/// strong and sharp; under the two-tone sun shading it crosses the light/shade threshold in many
/// places and paints ragged light patches over clothes, scarf and hair (seen on the player). Half
/// the slope keeps the folds but not the speckle. ASSUMED, picked by eye on the player.
pub const RELIEF_STRENGTH: f32 = 0.5;

/// Rebuilds the blue channel of a two-channel normal map in place and scales its relief by
/// [`RELIEF_STRENGTH`]. Texels whose (x, y) lie outside the unit disc are clamped to z = 0 (a
/// vector lying in the surface plane) before the scaling.
pub fn reconstruct_blue(rgba: &mut [u8]) {
    let to_byte = |v: f32| ((v * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
    for px in rgba.chunks_exact_mut(4) {
        let x = px[0] as f32 / 255.0 * 2.0 - 1.0;
        let y = px[1] as f32 / 255.0 * 2.0 - 1.0;
        let z = (1.0 - x * x - y * y).max(0.0).sqrt();
        // Scale the slope (x/z, y/z), then renormalise.
        let (sx, sy, sz) = (x * RELIEF_STRENGTH, y * RELIEF_STRENGTH, z);
        let len = (sx * sx + sy * sy + sz * sz).sqrt().max(1e-6);
        px[0] = to_byte(sx / len);
        px[1] = to_byte(sy / len);
        px[2] = to_byte(sz / len);
    }
}

/// Fixes `image` if it is a two-channel normal map. Returns whether it changed.
pub fn fix_image(image: &mut Image) -> bool {
    let fmt = image.texture_descriptor.format;
    if !matches!(fmt, TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb) {
        return false;
    }
    let Some(data) = image.data.as_mut() else { return false };
    if !is_two_channel_normal_map(data) {
        return false;
    }
    reconstruct_blue(data);
    true
}

#[derive(Default)]
struct Tracker {
    /// Normal maps already examined (fixed or found fine).
    done: HashSet<AssetId<Image>>,
    /// Normal maps whose image was not loaded yet, with the frames waited.
    pending: Vec<(Handle<Image>, u32)>,
}

fn fix_two_channel_normals(
    mats: Res<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut t: Local<Tracker>,
) {
    if mats.is_changed() {
        for (_, m) in mats.iter() {
            let Some(h) = m.normal_map_texture.as_ref() else { continue };
            if t.done.contains(&h.id()) || t.pending.iter().any(|(p, _)| p.id() == h.id()) {
                continue;
            }
            t.pending.push((h.clone(), 0));
        }
    }
    if t.pending.is_empty() {
        return;
    }
    let pending = std::mem::take(&mut t.pending);
    for (h, waited) in pending {
        let id = h.id();
        let state = images.get(id).map(|img| {
            matches!(img.texture_descriptor.format, TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb)
                && img.data.as_deref().is_some_and(is_two_channel_normal_map)
        });
        match state {
            Some(true) => {
                if let Some(mut img) = images.get_mut(id) {
                    fix_image(&mut img);
                }
                t.done.insert(id);
            }
            Some(false) => {
                t.done.insert(id);
            }
            None if waited < MAX_WAIT_FRAMES => t.pending.push((h, waited + 1)),
            None => {}
        }
    }
}

/// Registers the repair. Add with `app.add_plugins(NormalMapFixPlugin)`.
pub struct NormalMapFixPlugin;

impl Plugin for NormalMapFixPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, fix_two_channel_normals);
        super::face_shading::add_systems(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_only_maps_with_an_empty_blue_channel() {
        assert!(is_two_channel_normal_map(&[128, 128, 0, 255, 90, 200, 0, 255]));
        assert!(!is_two_channel_normal_map(&[128, 128, 255, 255]), "a real RGB normal map");
        assert!(!is_two_channel_normal_map(&[128, 128, 0, 255, 128, 128, 7, 255]));
        assert!(!is_two_channel_normal_map(&[]));
    }

    #[test]
    fn flat_texel_becomes_an_outward_normal() {
        // (0.5, 0.5) is the flat normal (0, 0, 1); before the fix it decoded to (0, 0, -1).
        let mut px = [128u8, 128, 0, 255];
        let before = px[2] as f32 / 255.0 * 2.0 - 1.0;
        assert!(before < -0.9, "the bug: z = {before}");
        reconstruct_blue(&mut px);
        let z = px[2] as f32 / 255.0 * 2.0 - 1.0;
        assert!(z > 0.99, "fixed z = {z}");
        assert_eq!(px[3], 255, "alpha untouched");
    }

    #[test]
    fn reconstructed_vectors_are_unit_length_and_point_outwards() {
        let mut data = Vec::new();
        for x in (0..=255).step_by(15) {
            for y in (0..=255).step_by(15) {
                data.extend_from_slice(&[x as u8, y as u8, 0, 255]);
            }
        }
        let n = data.len() / 4;
        reconstruct_blue(&mut data);
        for px in data.chunks_exact(4) {
            let v = [px[0], px[1], px[2]].map(|c| c as f32 / 255.0 * 2.0 - 1.0);
            assert!(v[2] >= -1e-3, "points into the surface: {v:?}");
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            // Inside the unit disc the length is 1 (8-bit quantisation); outside it is clamped.
            if v[0] * v[0] + v[1] * v[1] <= 0.95 {
                assert!((len - 1.0).abs() < 0.03, "length {len} for {v:?}");
            }
        }
        assert!(n > 100);
    }

    /// The real exported normal maps of the player and of Pikachu are all two-channel (skipped
    /// without a cache); after the fix their average z is clearly positive.
    #[test]
    fn real_exported_normal_maps_are_two_channel_and_get_fixed() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../work/cache");
        let files = [
            "player/p1_default/p1_face0001_00_face_nrm.png",
            "player/p1_default/p1_hrs0001_00_hairstyle_nrm.png",
            "player/p1_default/p1_drs0001_00_body_nrm.png",
            "pokemon_batch/pm0025_00_00/pm0025_00_00_body_a_nrm.png",
            "pokemon_batch/pm0399_00_00/pm0399_00_00_body_a_nrm.png",
        ];
        let mut checked = 0;
        for f in files {
            let Ok(file) = std::fs::File::open(root.join(f)) else { continue };
            let mut dec = png::Decoder::new(std::io::BufReader::new(file));
            dec.set_transformations(png::Transformations::EXPAND);
            let mut reader = dec.read_info().unwrap();
            let mut buf = vec![0u8; reader.output_buffer_size().unwrap()];
            let info = reader.next_frame(&mut buf).unwrap();
            assert_eq!(info.color_type, png::ColorType::Rgba, "{f}");
            let rgba = &mut buf[..info.buffer_size()];
            assert!(is_two_channel_normal_map(rgba), "{f}: blue channel is not empty");
            reconstruct_blue(rgba);
            let mean_z: f32 =
                rgba.chunks_exact(4).map(|p| p[2] as f32 / 255.0 * 2.0 - 1.0).sum::<f32>() / (rgba.len() / 4) as f32;
            eprintln!("{f}: mean z after fix {mean_z:.3}");
            assert!(mean_z > 0.8, "{f}: mean z {mean_z}");
            checked += 1;
        }
        if checked == 0 {
            eprintln!("[skip] no exported normal maps");
        }
    }
}
