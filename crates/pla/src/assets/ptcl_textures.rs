//! Drawable textures of particle files (`.ptcl`): decode the embedded `BNTX`
//! bank into RGBA8 images ready for a particle shader, with an on-disk PNG cache.
//!
//! * Decoding uses [`crate::assets::bntx`] (mip 0). The texture's component
//!   mapping (BNTX `channels` bytes, NX convention 0 = zero, 1 = one, 2..=5 =
//!   R, G, B, A of the decoded texel) is applied so that one-channel masks (BC4,
//!   BC5, R8) end up where the game reads them. The NX meaning of these bytes is
//!   the documented Nintendo texture-view convention (LIKELY for this game: the
//!   effect masks read `(1, 1, 1, R)`-style mappings that turn into white sprites
//!   with an alpha mask, which is what the CPU preview needed to look right).
//! * Cache layout (git-ignored, under `$PLA_CACHE_ROOT`, default
//!   `work/cache`): `effects/textures/<texture name>.png` plus
//!   `effects/texture_index.tsv` (name, size, format, channels, source file).
//!   Texture names are shared between particle files (e.g. `cpt_1_circle0004`
//!   in 100 move effects); a cached PNG is reused only when its size matches
//!   the requesting file's `BNTX` entry, otherwise the texture is decoded again.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::assets::bntx::{BntxFormat, BntxTexture, TextureData};
use crate::assets::ptcl::PtclFile;

/// Default derived-data root when `$PLA_CACHE_ROOT` is not set.
pub const DEFAULT_CACHE_ROOT: &str = "/home/nico/work/decompilacion/work/cache";

/// `<cache root>/effects`.
pub fn effects_cache_dir() -> PathBuf {
    std::env::var_os("PLA_CACHE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CACHE_ROOT))
        .join("effects")
}

/// Path of a cached texture PNG.
pub fn cached_texture_path(cache_dir: &Path, name: &str) -> PathBuf {
    let safe: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    cache_dir.join("textures").join(format!("{safe}.png"))
}

/// Game data root (`$PLA_BIN_ROOT`, else the default extracted RomFS `bin`).
pub fn default_bin_root() -> PathBuf {
    std::env::var_os("PLA_BIN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into())
}

/// Apply an NX component mapping to decoded RGBA8 texels in place.
/// `channels[c]` selects the source of output component `c`: 0 = 0, 1 = 255,
/// 2..=5 = decoded R, G, B, A. Unknown selectors keep the decoded component.
pub fn apply_channel_mapping(rgba: &mut [u8], channels: [u8; 4]) {
    if channels == [2, 3, 4, 5] || channels == [0; 4] {
        return; // identity (or not stored)
    }
    for px in rgba.chunks_exact_mut(4) {
        let src = [px[0], px[1], px[2], px[3]];
        for c in 0..4 {
            px[c] = match channels[c] {
                0 => 0,
                1 => 255,
                s @ 2..=5 => src[(s - 2) as usize],
                _ => src[c],
            };
        }
    }
}

/// True for formats that store one or two data channels (masks).
pub fn is_mask_format(format: BntxFormat) -> bool {
    matches!(format, BntxFormat::Bc4 | BntxFormat::Bc5 | BntxFormat::R8 | BntxFormat::R8G8)
}

/// Decode one `BNTX` texture into drawable RGBA8 (mip 0, component mapping applied).
pub fn decode_drawable(tex: &BntxTexture, bntx_bytes: &[u8]) -> Result<TextureData, String> {
    let mut d = tex.decode(bntx_bytes)?;
    apply_channel_mapping(&mut d.rgba, tex.channels);
    Ok(d)
}

/// Load a cached PNG (RGBA or RGB).
pub fn load_png(path: &Path) -> Result<TextureData, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("png too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let data = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        other => return Err(format!("unsupported png colour type {other:?}")),
    };
    Ok(TextureData { width: info.width, height: info.height, rgba })
}

/// Where a texture came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureSource {
    Cache,
    Decoded,
}

/// One drawable texture of a particle file.
#[derive(Debug, Clone)]
pub struct DrawableTexture {
    pub name: String,
    pub data: TextureData,
    pub format: BntxFormat,
    pub channels: [u8; 4],
    pub source: TextureSource,
}

/// Textures of a particle file, by name. `wanted` limits decoding to those names
/// (`None` = all). Uses the PNG cache in `cache_dir` when given; newly decoded
/// textures are written there (best effort: write errors are ignored).
pub fn load_textures(
    bytes: &[u8],
    file: &PtclFile,
    wanted: Option<&[String]>,
    cache_dir: Option<&Path>,
) -> (HashMap<String, DrawableTexture>, Vec<(String, String)>) {
    let mut out = HashMap::new();
    let mut errors = Vec::new();
    let Some(parsed) = file.bntx(bytes) else {
        return (out, errors);
    };
    let (bntx, slice) = match parsed {
        Ok(v) => v,
        Err(e) => {
            errors.push(("<bntx>".into(), e));
            return (out, errors);
        }
    };
    for t in &bntx.textures {
        if wanted.is_some_and(|w| !w.iter().any(|n| n == &t.name)) {
            continue;
        }
        if let Some(dir) = cache_dir {
            let p = cached_texture_path(dir, &t.name);
            if let Ok(d) = load_png(&p) {
                if d.width == t.width && d.height == t.height {
                    out.insert(
                        t.name.clone(),
                        DrawableTexture {
                            name: t.name.clone(),
                            data: d,
                            format: t.format,
                            channels: t.channels,
                            source: TextureSource::Cache,
                        },
                    );
                    continue;
                }
            }
        }
        match decode_drawable(t, slice) {
            Ok(d) => {
                if let Some(dir) = cache_dir {
                    let _ = d.save_png(&cached_texture_path(dir, &t.name));
                }
                out.insert(
                    t.name.clone(),
                    DrawableTexture {
                        name: t.name.clone(),
                        data: d,
                        format: t.format,
                        channels: t.channels,
                        source: TextureSource::Decoded,
                    },
                );
            }
            Err(e) => errors.push((t.name.clone(), e)),
        }
    }
    (out, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_mapping_turns_a_red_mask_into_white_alpha() {
        let mut px = vec![200, 200, 200, 255, 10, 10, 10, 255];
        apply_channel_mapping(&mut px, [1, 1, 1, 2]);
        assert_eq!(px, vec![255, 255, 255, 200, 255, 255, 255, 10]);
        let mut id = vec![1, 2, 3, 4];
        apply_channel_mapping(&mut id, [2, 3, 4, 5]);
        assert_eq!(id, vec![1, 2, 3, 4]);
        let mut swap = vec![1, 2, 3, 4];
        apply_channel_mapping(&mut swap, [5, 4, 3, 0]);
        assert_eq!(swap, vec![4, 3, 2, 0]);
    }

    #[test]
    fn cache_paths_are_sanitised() {
        let p = cached_texture_path(Path::new("/c"), "cpt_1/circle 0004");
        assert_eq!(p, PathBuf::from("/c/textures/cpt_1_circle_0004.png"));
    }

    #[test]
    fn real_effect_textures_decode_and_round_trip_through_the_cache() {
        let path = default_bin_root().join("effect/battle/particle/ew033/ew033_df_hit.ptcl");
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("[skip] {} not present", path.display());
            return;
        };
        let file = PtclFile::parse(&bytes).unwrap();
        let dir = std::env::temp_dir().join(format!("pla_ptcl_tex_{}", std::process::id()));
        let (first, errors) = load_textures(&bytes, &file, None, Some(&dir));
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(first.len(), file.texture_names.len());
        assert!(first.values().all(|t| t.source == TextureSource::Decoded));
        let (second, _) = load_textures(&bytes, &file, None, Some(&dir));
        for (name, t) in &second {
            assert_eq!(t.source, TextureSource::Cache, "{name}");
            assert_eq!(t.data, first[name].data, "{name}: PNG round trip changes pixels");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
