//! BNTX texture container parser and decoder (Nintendo's texture format).
//!
//! Verified layout (Wexos's Wiki, cross-checked byte-exact on the game's
//! files):
//!
//! * Header: `BNTX`, version at +0x08, BOM at +0x0C, file size at +0x1C,
//!   `NX  ` at +0x20, texture count at +0x24, texture table offset at +0x28
//!   (an absolute u64 pointer per texture to its `BRTI` block), texture data
//!   offset at +0x30.
//! * `BRTI` texture info: flags +0x10, dimension +0x11 (2 = 2D), tile mode
//!   +0x12 (0 = optimal/swizzled, 1 = linear), swizzle +0x14, mip count +0x16,
//!   sample count +0x18, format +0x1C, gpu access +0x20, width +0x24,
//!   height +0x28, depth +0x2C, array count +0x30, texture layout +0x34,
//!   image size +0x50, alignment +0x54, channels +0x58, image data table +0x70
//!   (an absolute u64 pointer per mip level).
//! * `BRTD` at the data offset: 16-byte header (`BRTD`, next, size, reserved),
//!   then the swizzled pixel data.
//! * `_RLT` at the end: relocation table (not needed to decode).
//!
//! ## Formats
//!
//! The format field uses the GX2 `SurfaceFormat` table (the last two hex
//! digits are the variant: `01` linear, `06` sRGB, `05` float). Census over
//! both builds (3 646 + 1 082 + 536 + 434 + 146 + 48 + 24 + 12 + 4 + 2 files):
//! BC7_UNORM, BC4_UNORM, BC7_SRGB, BC5_UNORM, RGBA8_UNORM, BC3_UNORM,
//! BC6_FLOAT, **R32_G32_FLOAT (0x1505, 6 faces)**, **R32_G32_B32_A32_FLOAT
//! (0x1905, 6 faces)** and R8_UNORM. BC1/BC2 and **ASTC do not appear**: the
//! game ships no ASTC texture, so no ASTC decoder is needed.
//!
//! ## Swizzle
//!
//! Tile mode 0 means the pixel data uses the Tegra X1 block-linear layout, so
//! the port deswizzles with `tegra_swizzle` before decoding blocks. Tile mode 1
//! is linear.

use tegra_swizzle::surface::{deswizzle_surface, BlockDim};

/// The texture formats the port decodes, plus a catch-all for the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BntxFormat {
    Bc1,
    Bc2,
    Bc3,
    Bc4,
    Bc5,
    Bc6,
    Bc7,
    R8,
    R8G8,
    Rgba8,
    Rgba8Srgb,
    /// 0x1505: two 32-bit floats per pixel (HDR probe cubemaps).
    Rg32Float,
    /// 0x1905: four 32-bit floats per pixel (HDR probe cubemaps).
    Rgba32Float,
    Unknown(u32),
}

impl BntxFormat {
    /// Decode the GX2 format code (e.g. `0x2001` = BC7_UNORM).
    pub fn from_code(code: u32) -> BntxFormat {
        match code {
            0x1a01 | 0x1a06 => BntxFormat::Bc1,
            0x1b01 | 0x1b06 => BntxFormat::Bc2,
            0x1c01 | 0x1c06 => BntxFormat::Bc3,
            0x1d01 | 0x1d02 => BntxFormat::Bc4,
            0x1e01 | 0x1e02 => BntxFormat::Bc5,
            0x1f05 | 0x1f0a => BntxFormat::Bc6,
            0x2001 | 0x2006 => BntxFormat::Bc7,
            0x0201 => BntxFormat::R8,
            0x0901 => BntxFormat::R8G8,
            0x0b01 => BntxFormat::Rgba8,
            0x0b06 => BntxFormat::Rgba8Srgb,
            0x1505 => BntxFormat::Rg32Float,
            0x1905 => BntxFormat::Rgba32Float,
            other => BntxFormat::Unknown(other),
        }
    }

    pub fn code(&self) -> u32 {
        match self {
            BntxFormat::Bc1 => 0x1a01,
            BntxFormat::Bc2 => 0x1b01,
            BntxFormat::Bc3 => 0x1c01,
            BntxFormat::Bc4 => 0x1d01,
            BntxFormat::Bc5 => 0x1e01,
            BntxFormat::Bc6 => 0x1f05,
            BntxFormat::Bc7 => 0x2001,
            BntxFormat::R8 => 0x0201,
            BntxFormat::R8G8 => 0x0901,
            BntxFormat::Rgba8 => 0x0b01,
            BntxFormat::Rgba8Srgb => 0x0b06,
            BntxFormat::Rg32Float => 0x1505,
            BntxFormat::Rgba32Float => 0x1905,
            BntxFormat::Unknown(c) => *c,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            BntxFormat::Bc1 => "BC1_UNORM",
            BntxFormat::Bc2 => "BC2_UNORM",
            BntxFormat::Bc3 => "BC3_UNORM",
            BntxFormat::Bc4 => "BC4_UNORM",
            BntxFormat::Bc5 => "BC5_UNORM",
            BntxFormat::Bc6 => "BC6_FLOAT",
            BntxFormat::Bc7 => "BC7_UNORM",
            BntxFormat::R8 => "R8_UNORM",
            BntxFormat::R8G8 => "R8_G8_UNORM",
            BntxFormat::Rgba8 => "R8_G8_B8_A8_UNORM",
            BntxFormat::Rgba8Srgb => "R8_G8_B8_A8_SRGB",
            BntxFormat::Rg32Float => "R32_G32_FLOAT",
            BntxFormat::Rgba32Float => "R32_G32_B32_A32_FLOAT",
            BntxFormat::Unknown(_) => "unknown",
        }
    }

    /// `(block width, block height, bytes per block)`.
    pub fn block(&self) -> (u32, u32, u32) {
        match self {
            BntxFormat::Bc1 | BntxFormat::Bc4 => (4, 4, 8),
            BntxFormat::Bc2
            | BntxFormat::Bc3
            | BntxFormat::Bc5
            | BntxFormat::Bc6
            | BntxFormat::Bc7 => (4, 4, 16),
            BntxFormat::R8 => (1, 1, 1),
            BntxFormat::R8G8 => (1, 1, 2),
            BntxFormat::Rgba8 | BntxFormat::Rgba8Srgb => (1, 1, 4),
            BntxFormat::Rg32Float => (1, 1, 8),
            BntxFormat::Rgba32Float => (1, 1, 16),
            BntxFormat::Unknown(_) => (1, 1, 0),
        }
    }

    pub fn is_compressed(&self) -> bool {
        matches!(
            self,
            BntxFormat::Bc1
                | BntxFormat::Bc2
                | BntxFormat::Bc3
                | BntxFormat::Bc4
                | BntxFormat::Bc5
                | BntxFormat::Bc6
                | BntxFormat::Bc7
        )
    }
}

/// One texture's metadata (the `BRTI` block).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BntxTexture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_count: u16,
    pub array_count: u32,
    pub format: BntxFormat,
    /// Size of all mipmap data in bytes (BRTI +0x50).
    pub image_size: u32,
    pub alignment: u32,
    /// 0 = optimal (Tegra block-linear), 1 = linear.
    pub tile_mode: u16,
    pub swizzle: u16,
    pub texture_layout: u64,
    pub channels: [u8; 4],
    /// Absolute file offset of the mip-level pointer table (BRTI +0x70).
    pub image_table_offset: u64,
}

/// A parsed BNTX container.
#[derive(Debug, Clone)]
pub struct Bntx {
    pub version: u32,
    pub declared_size: u32,
    pub texture_count: u32,
    pub data_offset: u64,
    pub textures: Vec<BntxTexture>,
}

/// Decoded mip 0 of a texture as tightly packed RGBA8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

/// HDR float -> 8-bit display approximation (clamped; the probe cubemaps carry
/// values above 1.0 that a non-HDR target cannot show).
fn float_to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0) as u8
}

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn u64le(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

impl Bntx {
    pub fn parse(bytes: &[u8]) -> Result<Bntx, String> {
        if bytes.len() < 0x48 || &bytes[0..4] != b"BNTX" {
            return Err("not a BNTX".into());
        }
        let bom = u16le(bytes, 0x0C);
        if bom != 0xFEFF {
            return Err(format!("bad BOM {bom:#06x}"));
        }
        if &bytes[0x20..0x24] != b"NX  " {
            return Err("missing NX platform signature".into());
        }
        let version = u32le(bytes, 0x08);
        let declared_size = u32le(bytes, 0x1C);
        if declared_size as usize != bytes.len() {
            return Err(format!(
                "declared size {declared_size} != actual {}",
                bytes.len()
            ));
        }
        let texture_count = u32le(bytes, 0x24);
        let table_offset = u64le(bytes, 0x28) as usize;
        let data_offset = u64le(bytes, 0x30);

        let mut textures = Vec::new();
        for i in 0..texture_count as usize {
            let entry = table_offset + i * 8;
            if entry + 8 > bytes.len() {
                return Err(format!("texture table truncated at entry {i}"));
            }
            let brti = u64le(bytes, entry) as usize;
            if brti + 0x80 > bytes.len() {
                return Err(format!("BRTI {i} out of range"));
            }
            if &bytes[brti..brti + 4] != b"BRTI" {
                return Err(format!("texture {i}: missing BRTI magic"));
            }
            let name_offset = u64le(bytes, brti + 0x60) as usize;
            let name = if name_offset > 0 && name_offset + 2 <= bytes.len() {
                let len = u16le(bytes, name_offset) as usize;
                if name_offset + 2 + len <= bytes.len() {
                    String::from_utf8_lossy(&bytes[name_offset + 2..name_offset + 2 + len])
                        .to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            textures.push(BntxTexture {
                name,
                width: u32le(bytes, brti + 0x24),
                height: u32le(bytes, brti + 0x28),
                depth: u32le(bytes, brti + 0x2C),
                mip_count: u16le(bytes, brti + 0x16),
                array_count: u32le(bytes, brti + 0x30),
                format: BntxFormat::from_code(u32le(bytes, brti + 0x1C)),
                image_size: u32le(bytes, brti + 0x50),
                alignment: u32le(bytes, brti + 0x54),
                tile_mode: u16le(bytes, brti + 0x12),
                swizzle: u16le(bytes, brti + 0x14),
                texture_layout: u64le(bytes, brti + 0x34),
                channels: [
                    bytes[brti + 0x58],
                    bytes[brti + 0x59],
                    bytes[brti + 0x5A],
                    bytes[brti + 0x5B],
                ],
                image_table_offset: u64le(bytes, brti + 0x70),
            });
        }

        // Sanity: the data block must exist when there are textures.
        if texture_count > 0 {
            let off = data_offset as usize;
            if off + 4 > bytes.len() || &bytes[off..off + 4] != b"BRTD" {
                return Err("missing BRTD texture data block".into());
            }
        }

        Ok(Bntx {
            version,
            declared_size,
            texture_count,
            data_offset,
            textures,
        })
    }

    pub fn texture(&self) -> Option<&BntxTexture> {
        self.textures.first()
    }

    /// Bytes of the raw texture data block (the BRTD payload, mip data).
    pub fn data<'a>(&self, bytes: &'a [u8]) -> &'a [u8] {
        let start = (self.data_offset as usize + 16).min(bytes.len());
        &bytes[start..]
    }

    /// Decode mip 0 of texture `index` to RGBA8.
    pub fn decode(&self, bytes: &[u8], index: usize) -> Result<TextureData, String> {
        let t = self
            .textures
            .get(index)
            .ok_or_else(|| format!("texture {index} out of range"))?;
        t.decode(bytes)
    }
}

impl BntxTexture {
    /// Absolute file offset of mip level `level`.
    pub fn mip_offset(&self, bytes: &[u8], level: usize) -> Option<u64> {
        let table = self.image_table_offset as usize;
        if table + 8 * (level + 1) > bytes.len() {
            return None;
        }
        Some(u64le(bytes, table + 8 * level))
    }

    /// Decode mip 0 to tightly packed RGBA8.
    pub fn decode(&self, bytes: &[u8]) -> Result<TextureData, String> {
        let (bw, bh, bpb) = self.format.block();
        if bpb == 0 {
            return Err(format!("unsupported format {:#x}", self.format.code()));
        }
        let mip0 = self
            .mip_offset(bytes, 0)
            .ok_or_else(|| "no mip table".to_string())? as usize;
        if mip0 >= bytes.len() {
            return Err("mip 0 out of range".into());
        }
        let source = &bytes[mip0..];
        let width = self.width;
        let height = self.height;

        let linear: Vec<u8> = if self.tile_mode == 1 {
            source.to_vec()
        } else {
            let block = BlockDim {
                width: std::num::NonZeroU32::new(bw).unwrap(),
                height: std::num::NonZeroU32::new(bh).unwrap(),
                depth: std::num::NonZeroU32::new(1).unwrap(),
            };
            deswizzle_surface(
                width,
                height,
                self.depth.max(1),
                source,
                block,
                None,
                bpb,
                self.mip_count.max(1) as u32,
                self.array_count.max(1),
            )
            .map_err(|e| format!("deswizzle: {e:?}"))?
        };

        let mut rgba = vec![0u8; (width * height * 4) as usize];
        if self.format.is_compressed() {
            let grid_w = width.div_ceil(bw);
            let grid_h = height.div_ceil(bh);
            let mut tile = [0u8; 64];
            let mut channels = [0u8; 64];
            for by in 0..grid_h {
                for bx in 0..grid_w {
                    let off = ((by * grid_w + bx) * bpb) as usize;
                    if off + bpb as usize > linear.len() {
                        return Err(format!("block {bx},{by} out of range"));
                    }
                    let block = &linear[off..off + bpb as usize];
                    match self.format {
                        BntxFormat::Bc1 => bcdec_rs::bc1(block, &mut tile, 16),
                        BntxFormat::Bc2 => bcdec_rs::bc2(block, &mut tile, 16),
                        BntxFormat::Bc3 => bcdec_rs::bc3(block, &mut tile, 16),
                        BntxFormat::Bc4 => {
                            bcdec_rs::bc4(block, &mut channels[..16], 4, false);
                            for i in 0..16 {
                                let v = channels[i];
                                tile[i * 4] = v;
                                tile[i * 4 + 1] = v;
                                tile[i * 4 + 2] = v;
                                tile[i * 4 + 3] = 255;
                            }
                        }
                        BntxFormat::Bc5 => {
                            bcdec_rs::bc5(block, &mut channels[..32], 8, false);
                            for i in 0..16 {
                                tile[i * 4] = channels[i * 2];
                                tile[i * 4 + 1] = channels[i * 2 + 1];
                                tile[i * 4 + 2] = 0;
                                tile[i * 4 + 3] = 255;
                            }
                        }
                        BntxFormat::Bc6 => {
                            let mut floats = [0f32; 64];
                            bcdec_rs::bc6h_float(block, &mut floats, 16, false);
                            for i in 0..16 {
                                for c in 0..3 {
                                    // HDR -> 8-bit display approximation.
                                    let v = floats[i * 4 + c].clamp(0.0, 1.0);
                                    tile[i * 4 + c] = (v * 255.0) as u8;
                                }
                                tile[i * 4 + 3] = 255;
                            }
                        }
                        BntxFormat::Bc7 => bcdec_rs::bc7(block, &mut tile, 16),
                        _ => unreachable!(),
                    }
                    for py in 0..bh {
                        for px in 0..bw {
                            let x = bx * bw + px;
                            let y = by * bh + py;
                            if x >= width || y >= height {
                                continue;
                            }
                            let o = ((y * width + x) * 4) as usize;
                            let s = ((py * bw + px) * 4) as usize;
                            rgba[o..o + 4].copy_from_slice(&tile[s..s + 4]);
                        }
                    }
                }
            }
        } else {
            // Uncompressed: bpb bytes per pixel.
            for y in 0..height {
                for x in 0..width {
                    let s = ((y * width + x) * bpb) as usize;
                    if s + bpb as usize > linear.len() {
                        return Err("pixel out of range".into());
                    }
                    let o = ((y * width + x) * 4) as usize;
                    match self.format {
                        BntxFormat::R8 => {
                            let v = linear[s];
                            rgba[o] = v;
                            rgba[o + 1] = v;
                            rgba[o + 2] = v;
                            rgba[o + 3] = 255;
                        }
                        BntxFormat::R8G8 => {
                            rgba[o] = linear[s];
                            rgba[o + 1] = linear[s + 1];
                            rgba[o + 2] = 0;
                            rgba[o + 3] = 255;
                        }
                        BntxFormat::Rg32Float => {
                            rgba[o] = float_to_u8(f32_at(&linear, s));
                            rgba[o + 1] = float_to_u8(f32_at(&linear, s + 4));
                            rgba[o + 2] = 0;
                            rgba[o + 3] = 255;
                        }
                        BntxFormat::Rgba32Float => {
                            rgba[o] = float_to_u8(f32_at(&linear, s));
                            rgba[o + 1] = float_to_u8(f32_at(&linear, s + 4));
                            rgba[o + 2] = float_to_u8(f32_at(&linear, s + 8));
                            rgba[o + 3] = float_to_u8(f32_at(&linear, s + 12));
                        }
                        _ => {
                            rgba[o..o + 4].copy_from_slice(&linear[s..s + 4]);
                        }
                    }
                }
            }
        }
        Ok(TextureData {
            width,
            height,
            rgba,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_table_matches_the_game_codes() {
        assert_eq!(BntxFormat::from_code(0x2001), BntxFormat::Bc7);
        assert_eq!(BntxFormat::from_code(0x2006), BntxFormat::Bc7);
        assert_eq!(BntxFormat::from_code(0x1d01), BntxFormat::Bc4);
        assert_eq!(BntxFormat::from_code(0x1e01), BntxFormat::Bc5);
        assert_eq!(BntxFormat::from_code(0x1c01), BntxFormat::Bc3);
        assert_eq!(BntxFormat::from_code(0x1f05), BntxFormat::Bc6);
        assert_eq!(BntxFormat::from_code(0x0b01), BntxFormat::Rgba8);
        assert_eq!(BntxFormat::from_code(0x0201), BntxFormat::R8);
        assert_eq!(BntxFormat::from_code(0x1905), BntxFormat::Rgba32Float);
        assert_eq!(BntxFormat::from_code(0x1505), BntxFormat::Rg32Float);
        assert_eq!(BntxFormat::Rgba32Float.block(), (1, 1, 16));
        assert_eq!(BntxFormat::Rg32Float.block(), (1, 1, 8));
        assert_eq!(BntxFormat::Bc1.block(), (4, 4, 8));
        assert_eq!(BntxFormat::Bc7.block(), (4, 4, 16));
        assert_eq!(BntxFormat::Rgba8.block(), (1, 1, 4));
        assert!(BntxFormat::Bc7.is_compressed());
        assert!(!BntxFormat::Rgba8.is_compressed());
    }

    #[test]
    fn bc4_block_interpolates_endpoints() {
        // a0 = 0, a1 = 255, all indices 7 -> 255 (the a0 <= a1 escape value).
        let block = [0u8, 255, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
        let mut out = [0u8; 16];
        bcdec_rs::bc4(&block, &mut out, 4, false);
        assert!(out.iter().all(|&v| v == 255), "{out:?}");

        // All indices 0 -> the a0 endpoint.
        let block = [0u8, 255, 0, 0, 0, 0, 0, 0];
        bcdec_rs::bc4(&block, &mut out, 4, false);
        assert!(out.iter().all(|&v| v == 0), "{out:?}");
    }

    #[test]
    fn bc7_zero_block_decodes_to_zero() {
        // Mode 0 with zero endpoints: fully zero output.
        let block = [0u8; 16];
        let mut out = [0u8; 64];
        bcdec_rs::bc7(&block, &mut out, 16);
        assert!(out.iter().all(|&v| v == 0), "{out:?}");
    }

    #[test]
    fn bc7_decodes_the_placeholder_block() {
        // icon_ball_0000's single block: a transparent white 2x2 texture (all
        // 24 icon_ball files share these pixels; they are placeholders).
        let block: [u8; 16] = [
            0x20, 0x7f, 0xfe, 0x9f, 0xff, 0xe7, 0x03, 0x00, 0x40, 0x41, 0x55, 0x55, 0x51, 0x50,
            0x55, 0x55,
        ];
        let mut out = [0u8; 64];
        bcdec_rs::bc7(&block, &mut out, 16);
        // The 2x2 texture uses the top-left 2x2 of the 4x4 block: pixels 0, 1,
        // 4 and 5.
        for i in [0usize, 1, 4, 5] {
            assert_eq!(&out[i * 4..i * 4 + 4], &[255, 255, 255, 0], "pixel {i}");
        }
    }
}
