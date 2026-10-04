//! VFXB particle container parser.
//!
//! Layout (observed): `"VFXB"` + 4 spaces (padded name) + version/flags +
//! BOM `0xFEFF` at +0x0C (same header family as BNTX), then an embedded BNTX texture at a 0x1000-aligned
//! offset (the particle file carries its own texture).
//!
//! The parser validates the container header and hands the embedded texture to
//! the BNTX parser, so particle metadata is only as trustworthy as BNTX's.

use crate::assets::bntx::Bntx;

#[derive(Debug, Clone)]
pub struct Vfxb {
    /// Offset of the embedded BNTX inside the container.
    pub texture_offset: usize,
    /// Declared size of the embedded BNTX.
    pub texture_size: u32,
    /// Parsed embedded texture.
    pub texture: Bntx,
    /// Bytes after the embedded texture (particle payload).
    pub payload: (usize, usize),
}

impl Vfxb {
    pub fn parse(bytes: &[u8]) -> Result<Vfxb, String> {
        if bytes.len() < 0x1000 || &bytes[0..4] != b"VFXB" {
            return Err("not a VFXB".into());
        }
        let bom = u16::from_le_bytes([bytes[0x0C], bytes[0x0D]]);
        if bom != 0xFEFF {
            return Err(format!("bad BOM {bom:#06x}"));
        }
        let texture_offset = find(bytes, b"BNTX").ok_or("no embedded BNTX")?;
        if texture_offset + 0x20 > bytes.len() {
            return Err("truncated embedded BNTX header".into());
        }
        let texture_size = u32::from_le_bytes(
            bytes[texture_offset + 0x1C..texture_offset + 0x20]
                .try_into()
                .unwrap(),
        );
        let end = texture_offset + texture_size as usize;
        if end > bytes.len() {
            return Err(format!(
                "embedded BNTX declares {texture_size} bytes but only {} remain",
                bytes.len() - texture_offset
            ));
        }
        let texture = Bntx::parse(&bytes[texture_offset..end])?;
        Ok(Vfxb {
            texture_offset,
            texture_size,
            texture,
            payload: (end, bytes.len()),
        })
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
