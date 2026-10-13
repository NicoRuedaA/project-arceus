//! Particle effect files (`bin/effect/**/*.ptcl`, `VFXB` version 0x16).
//!
//! This is a structural decoder written from the data alone (1,748 files,
//! 9,647 emitters; see `reports/data-progress/effects.md`). It goes further
//! than [`crate::assets::vfxb::Vfxb`], which only finds the embedded texture.
//!
//! # Layout (VERIFIED on all 1,748 game files unless a field says otherwise)
//!
//! File header (0x20 bytes): `"VFXB"`, 4 spaces, u16 graphics version
//! (0x0400), u16 VFX version (0x0016), BOM `0xFEFF`, u8 alignment, u8 target
//! offset, u32 header size, u16 flags, u16 first section offset (0x40), u32
//! pad, u32 file size.
//!
//! Every section starts with a 0x20-byte header: magic, u32 size, u32 child
//! offset, u32 next offset, u32 attribute offset, u32 data offset, u32 pad,
//! u16 child count, u16 pad. Offsets are relative to the section start and
//! `0xFFFF_FFFF` means "none". The top level chains `ESTA` (emitter sets),
//! `GRTF` (textures: `GTNT` name table + embedded `BNTX`), `PRMA` (meshes),
//! `G3PR` and `GRSN` (shaders). `ESTA` holds one `ESET` whose children are
//! `EMTR` emitters; an emitter can own child emitters and a chain of attribute
//! sections (`CSDP`, `FRND`, `EA**`, `EP0*`, ...).
//!
//! An emitter's data block is always 2,752 bytes. Field confidence:
//! * VERIFIED (structural): name, the six key-frame tables and their key
//!   counts (keys past the count are all zero and key times never decrease in
//!   9,647/9,647 emitters), texture sampler ids (15,335/15,335 non-empty ids
//!   resolve in the file's `GTNT` table), mesh id (resolves to a `PRIM` entry in
//!   the sampled file), shader index (dense `0..n` per file in 1,748/1,748).
//! * LIKELY (statistics + names): display type (`_ring`/`_wave` names are
//!   polygon types, `_line`/`spark` names are velocity-look), blend type (byte
//!   value 2 in 69 % of `_sub` names and 0 % of `_add`/`_nor` ones; value 1 in
//!   46 % of `_add`), particle life in frames, emission rate (particles per
//!   emission), one-time flag (0 for 82 % of `loop` emitters), constant colours,
//!   which table is colour/alpha/scale (alpha tables always hold x = y = z).
//! * ASSUMED: emission start/duration/interval, life randomness, gravity, base
//!   scale, omni speed. They are exposed with that label and must not be
//!   treated as facts.

use std::collections::HashMap;

use crate::assets::bntx::Bntx;

/// Size of one emitter data block in VFX version 0x16.
pub const EMITTER_DATA_SIZE: usize = 2752;
const NONE: u32 = 0xFFFF_FFFF;
const SECTION_HEADER: usize = 0x20;
/// Guard against corrupt next/child chains looping forever.
const MAX_CHAIN: usize = 4096;

/// Byte offsets inside the 2,752-byte emitter block.
mod off {
    pub const NAME: usize = 0x10;
    pub const NAME_LEN: usize = 0x40;
    /// Six u32 key counts, one per table in [`KEY_TABLES`].
    pub const KEY_COUNTS: usize = 0x60;
    /// Six tables of 8 keys x (x, y, z, time) f32.
    pub const KEY_TABLES: [usize; 6] = [0x3C0, 0x440, 0x4C0, 0x540, 0x600, 0x680];
    pub const GRAVITY: usize = 0xB0;
    pub const ONE_TIME: usize = 0x7F0;
    pub const EMIT_START: usize = 0x7F4;
    pub const EMIT_DURATION: usize = 0x7F8;
    pub const EMIT_INTERVAL: usize = 0x7FC;
    pub const EMIT_RATE: usize = 0x800;
    pub const OMNI_SPEED: usize = 0x858;
    pub const PRIMITIVE_ID: usize = 0x8D0;
    pub const BLEND: usize = 0x89E;
    pub const DISPLAY: usize = 0x8AA;
    pub const LIFE: usize = 0x8B8;
    pub const LIFE_RANDOM: usize = 0x8BC;
    pub const SHADER_INDEX: usize = 0x914;
    pub const COLOR0: usize = 0x9A0;
    pub const COLOR1: usize = 0x9B0;
    pub const BASE_SCALE: usize = 0x9C0;
    /// Three samplers of 0x20 bytes, each starting with a u64 texture id.
    pub const SAMPLERS: [usize; 3] = [0x9F0, 0xA10, 0xA30];
}

/// How a particle is drawn (byte 0x8AA). LIKELY names; values 0-7 occur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayType {
    Billboard,
    ComplexBillboard,
    YBillboard,
    PolygonXY,
    PolygonXZ,
    VelocityLook,
    VelocityLookPolygon,
    Other(u8),
}

impl DisplayType {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Billboard,
            1 => Self::ComplexBillboard,
            2 => Self::YBillboard,
            3 => Self::PolygonXY,
            4 => Self::PolygonXZ,
            5 => Self::VelocityLook,
            6 => Self::VelocityLookPolygon,
            other => Self::Other(other),
        }
    }
}

/// Colour blending (byte 0x89E). 0/1/2 are LIKELY; the rest are ASSUMED.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendType {
    Normal,
    Add,
    Sub,
    Other(u8),
}

impl BlendType {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Normal,
            1 => Self::Add,
            2 => Self::Sub,
            other => Self::Other(other),
        }
    }
}

/// One animation key: a value (rgb, alpha replicated in x/y/z, or xyz scale)
/// at a normalised particle-life time in `0..=1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    pub value: [f32; 3],
    pub time: f32,
}

/// An "over life" curve of up to 8 keys. An empty table means "not animated".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeyTable {
    pub keys: Vec<Key>,
}

impl KeyTable {
    /// Piecewise-linear value at normalised life `t`; `None` when empty.
    pub fn sample(&self, t: f32) -> Option<[f32; 3]> {
        let first = self.keys.first()?;
        if t <= first.time || self.keys.len() == 1 {
            return Some(first.value);
        }
        for pair in self.keys.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if t <= b.time {
                let span = b.time - a.time;
                let f = if span > 0.0 { (t - a.time) / span } else { 1.0 };
                return Some([
                    a.value[0] + (b.value[0] - a.value[0]) * f,
                    a.value[1] + (b.value[1] - a.value[1]) * f,
                    a.value[2] + (b.value[2] - a.value[2]) * f,
                ]);
            }
        }
        self.keys.last().map(|k| k.value)
    }
}

/// One decoded emitter.
#[derive(Debug, Clone, PartialEq)]
pub struct Emitter {
    pub name: String,
    /// Index of the parent emitter in [`PtclFile::emitters`], for child emitters.
    pub parent: Option<usize>,
    pub display: DisplayType,
    pub blend: BlendType,
    pub shader_index: u32,
    /// Particle life in frames (LIKELY).
    pub life_frames: i32,
    /// Life randomness, probably a percentage (ASSUMED).
    pub life_random: i32,
    /// Emits once instead of looping (LIKELY).
    pub one_time: bool,
    /// ASSUMED emission timing fields, in frames.
    pub emit_start_frame: i32,
    pub emit_duration_frames: i32,
    pub emit_interval_frames: i32,
    /// Particles per emission (LIKELY).
    pub emit_rate: f32,
    /// Constant colours used when the matching table is empty (LIKELY).
    pub color0: [f32; 4],
    pub color1: [f32; 4],
    /// ASSUMED base particle size.
    pub base_scale: [f32; 3],
    /// ASSUMED gravity direction (x, y, z) and strength.
    pub gravity: [f32; 4],
    /// ASSUMED initial speed in all directions.
    pub omni_speed: f32,
    pub color0_keys: KeyTable,
    pub alpha0_keys: KeyTable,
    pub color1_keys: KeyTable,
    pub alpha1_keys: KeyTable,
    pub scale_keys: KeyTable,
    /// Sixth key table; meaning unknown (RAW).
    pub table6_keys: KeyTable,
    /// Texture ids of the three samplers (`None` = unused slot).
    pub texture_ids: [Option<u64>; 3],
    /// Mesh (primitive) id for mesh particles.
    pub primitive_id: Option<u64>,
    /// Attribute section magics attached to the emitter (`EP03` = stripe/trail, LIKELY).
    pub attributes: Vec<String>,
}

impl Emitter {
    /// LIKELY: emitters with an `EP03` attribute draw a ribbon (stripe/trail).
    pub fn is_stripe(&self) -> bool {
        self.attributes.iter().any(|a| a == "EP03")
    }

    /// Broad drawing shape used by the playback plan.
    pub fn shape(&self) -> &'static str {
        if self.primitive_id.is_some() {
            "mesh"
        } else if self.is_stripe() {
            "ribbon"
        } else {
            match self.display {
                DisplayType::PolygonXY | DisplayType::PolygonXZ => "polygon",
                DisplayType::VelocityLook | DisplayType::VelocityLookPolygon => "streak",
                _ => "billboard",
            }
        }
    }

    /// Colour (rgb) at normalised life `t`: animated table, else constant colour 0.
    pub fn color_at(&self, t: f32) -> [f32; 3] {
        self.color0_keys
            .sample(t)
            .unwrap_or([self.color0[0], self.color0[1], self.color0[2]])
    }

    /// Alpha at normalised life `t`: animated table, else constant colour 0 alpha.
    pub fn alpha_at(&self, t: f32) -> f32 {
        self.alpha0_keys
            .sample(t)
            .map(|v| v[0])
            .unwrap_or(self.color0[3])
    }

    /// Size multiplier at normalised life `t` (1 when not animated).
    pub fn scale_at(&self, t: f32) -> [f32; 3] {
        self.scale_keys.sample(t).unwrap_or([1.0; 3])
    }
}

/// A texture name entry from `GTNT`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureName {
    pub id: u64,
    pub name: String,
}

/// A parsed `.ptcl` file.
#[derive(Debug, Clone)]
pub struct PtclFile {
    pub graphics_version: u16,
    pub vfx_version: u16,
    pub file_size: u32,
    /// Top-level section magics in file order.
    pub top_sections: Vec<String>,
    pub emitter_set_name: String,
    pub emitters: Vec<Emitter>,
    pub texture_names: Vec<TextureName>,
    /// Byte range of the embedded `BNTX`, if any.
    pub bntx_range: Option<(usize, usize)>,
    /// Number of `PRIM` mesh entries under `PRMA`.
    pub primitive_count: usize,
}

#[derive(Debug, Clone, Copy)]
struct Section {
    magic: [u8; 4],
    start: usize,
    child: u32,
    next: u32,
    attr: u32,
    data: u32,
    child_count: u16,
}

fn rd_u16(b: &[u8], o: usize) -> Result<u16, String> {
    b.get(o..o + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| format!("u16 out of range at {o:#x}"))
}

fn rd_u32(b: &[u8], o: usize) -> Result<u32, String> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| format!("u32 out of range at {o:#x}"))
}

fn rd_u64(b: &[u8], o: usize) -> Result<u64, String> {
    b.get(o..o + 8)
        .map(|s| u64::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| format!("u64 out of range at {o:#x}"))
}

fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn i32_at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

fn section(b: &[u8], start: usize) -> Result<Section, String> {
    if start + SECTION_HEADER > b.len() {
        return Err(format!("section header out of range at {start:#x}"));
    }
    Ok(Section {
        magic: b[start..start + 4].try_into().unwrap(),
        start,
        child: rd_u32(b, start + 0x08)?,
        next: rd_u32(b, start + 0x0C)?,
        attr: rd_u32(b, start + 0x10)?,
        data: rd_u32(b, start + 0x14)?,
        child_count: rd_u16(b, start + 0x1C)?,
    })
}

/// Walk a `next`-linked chain starting at `start`.
fn chain(b: &[u8], start: usize) -> Result<Vec<Section>, String> {
    let mut out = Vec::new();
    let mut pos = start;
    for _ in 0..MAX_CHAIN {
        let s = section(b, pos)?;
        out.push(s);
        if s.next == NONE {
            return Ok(out);
        }
        pos = pos
            .checked_add(s.next as usize)
            .filter(|&p| p > s.start)
            .ok_or("section chain does not advance")?;
    }
    Err("section chain too long".into())
}

fn key_table(d: &[u8], table: usize, count: u32) -> Result<KeyTable, String> {
    if count > 8 {
        return Err(format!("key count {count} > 8"));
    }
    let keys = (0..count as usize)
        .map(|k| {
            let o = table + 16 * k;
            Key {
                value: [f32_at(d, o), f32_at(d, o + 4), f32_at(d, o + 8)],
                time: f32_at(d, o + 12),
            }
        })
        .collect();
    Ok(KeyTable { keys })
}

fn opt_id(v: u64) -> Option<u64> {
    (v != u64::MAX).then_some(v)
}

fn decode_emitter(b: &[u8], s: &Section, parent: Option<usize>) -> Result<Emitter, String> {
    let start = s.start + s.data as usize;
    let d = b
        .get(start..start + EMITTER_DATA_SIZE)
        .ok_or_else(|| format!("emitter data out of range at {start:#x}"))?;
    let mut tables = Vec::with_capacity(6);
    for (i, &t) in off::KEY_TABLES.iter().enumerate() {
        tables.push(key_table(d, t, rd_u32(d, off::KEY_COUNTS + 4 * i)?)?);
    }
    let mut attributes = Vec::new();
    if s.attr != NONE {
        for a in chain(b, s.start + s.attr as usize)? {
            attributes.push(String::from_utf8_lossy(&a.magic).into_owned());
        }
    }
    let v4 = |o: usize| {
        [
            f32_at(d, o),
            f32_at(d, o + 4),
            f32_at(d, o + 8),
            f32_at(d, o + 12),
        ]
    };
    let v3 = |o: usize| [f32_at(d, o), f32_at(d, o + 4), f32_at(d, o + 8)];
    let mut tables = tables.into_iter();
    Ok(Emitter {
        name: cstr(&d[off::NAME..off::NAME + off::NAME_LEN]),
        parent,
        display: DisplayType::from_u8(d[off::DISPLAY]),
        blend: BlendType::from_u8(d[off::BLEND]),
        shader_index: rd_u32(d, off::SHADER_INDEX)?,
        life_frames: i32_at(d, off::LIFE),
        life_random: i32_at(d, off::LIFE_RANDOM),
        one_time: d[off::ONE_TIME] != 0,
        emit_start_frame: i32_at(d, off::EMIT_START),
        emit_duration_frames: i32_at(d, off::EMIT_DURATION),
        emit_interval_frames: i32_at(d, off::EMIT_INTERVAL),
        emit_rate: f32_at(d, off::EMIT_RATE),
        color0: v4(off::COLOR0),
        color1: v4(off::COLOR1),
        base_scale: v3(off::BASE_SCALE),
        gravity: v4(off::GRAVITY),
        omni_speed: f32_at(d, off::OMNI_SPEED),
        color0_keys: tables.next().unwrap(),
        alpha0_keys: tables.next().unwrap(),
        color1_keys: tables.next().unwrap(),
        alpha1_keys: tables.next().unwrap(),
        scale_keys: tables.next().unwrap(),
        table6_keys: tables.next().unwrap(),
        texture_ids: [
            opt_id(rd_u64(d, off::SAMPLERS[0])?),
            opt_id(rd_u64(d, off::SAMPLERS[1])?),
            opt_id(rd_u64(d, off::SAMPLERS[2])?),
        ],
        primitive_id: opt_id(rd_u64(d, off::PRIMITIVE_ID)?),
        attributes,
    })
}

fn walk_emitters(
    b: &[u8],
    first: usize,
    parent: Option<usize>,
    depth: usize,
    out: &mut Vec<Emitter>,
) -> Result<(), String> {
    if depth > 8 {
        return Err("emitter nesting too deep".into());
    }
    for s in chain(b, first)? {
        if &s.magic != b"EMTR" {
            return Err(format!("expected EMTR at {:#x}", s.start));
        }
        let index = out.len();
        out.push(decode_emitter(b, &s, parent)?);
        if s.child != NONE && s.child_count > 0 {
            walk_emitters(b, s.start + s.child as usize, Some(index), depth + 1, out)?;
        }
    }
    Ok(())
}

fn texture_names(b: &[u8], gtnt: &Section) -> Result<Vec<TextureName>, String> {
    let mut out = Vec::new();
    let mut p = gtnt.start + gtnt.data as usize;
    for _ in 0..MAX_CHAIN {
        let id = rd_u64(b, p)?;
        let next = rd_u32(b, p + 8)? as usize;
        let len = rd_u32(b, p + 12)? as usize;
        let name = b
            .get(p + 16..p + 16 + len)
            .ok_or("texture name out of range")?;
        out.push(TextureName {
            id,
            name: cstr(name),
        });
        if next == 0 {
            return Ok(out);
        }
        p += next;
    }
    Err("texture name table too long".into())
}

impl PtclFile {
    pub fn parse(b: &[u8]) -> Result<PtclFile, String> {
        if b.len() < 0x40 || &b[0..4] != b"VFXB" {
            return Err("not a VFXB file".into());
        }
        if rd_u16(b, 0x0C)? != 0xFEFF {
            return Err("unexpected byte order mark".into());
        }
        let graphics_version = rd_u16(b, 0x08)?;
        let vfx_version = rd_u16(b, 0x0A)?;
        if vfx_version != 0x16 {
            return Err(format!("unsupported VFX version {vfx_version:#x}"));
        }
        let file_size = rd_u32(b, 0x1C)?;
        let first = rd_u16(b, 0x16)? as usize;

        let mut out = PtclFile {
            graphics_version,
            vfx_version,
            file_size,
            top_sections: Vec::new(),
            emitter_set_name: String::new(),
            emitters: Vec::new(),
            texture_names: Vec::new(),
            bntx_range: None,
            primitive_count: 0,
        };
        for s in chain(b, first)? {
            out.top_sections
                .push(String::from_utf8_lossy(&s.magic).into_owned());
            let has_child = s.child != NONE && s.child_count > 0;
            match &s.magic {
                b"ESTA" if has_child => {
                    for set in chain(b, s.start + s.child as usize)? {
                        if &set.magic != b"ESET" {
                            continue;
                        }
                        if out.emitter_set_name.is_empty() {
                            let n = set.start + set.data as usize + 0x10;
                            out.emitter_set_name =
                                cstr(b.get(n..n + 0x40).ok_or("set name out of range")?);
                        }
                        if set.child != NONE && set.child_count > 0 {
                            walk_emitters(
                                b,
                                set.start + set.child as usize,
                                None,
                                0,
                                &mut out.emitters,
                            )?;
                        }
                    }
                }
                b"GRTF" => {
                    if has_child {
                        let g = section(b, s.start + s.child as usize)?;
                        if &g.magic == b"GTNT" {
                            out.texture_names = texture_names(b, &g)?;
                        }
                    }
                    if s.data != NONE {
                        let start = s.start + s.data as usize;
                        if b.get(start..start + 4) == Some(b"BNTX") {
                            let size = rd_u32(b, start + 0x1C)? as usize;
                            if start + size <= b.len() {
                                out.bntx_range = Some((start, start + size));
                            }
                        }
                    }
                }
                b"PRMA" if has_child => {
                    out.primitive_count = chain(b, s.start + s.child as usize)?
                        .iter()
                        .filter(|p| &p.magic == b"PRIM")
                        .count();
                }
                _ => {}
            }
        }
        Ok(out)
    }

    /// Texture name for a sampler id, from the file's `GTNT` table.
    pub fn texture_name(&self, id: u64) -> Option<&str> {
        self.texture_names
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.name.as_str())
    }

    /// Parse the embedded `BNTX` (if any).
    pub fn bntx<'a>(&self, b: &'a [u8]) -> Option<Result<(Bntx, &'a [u8]), String>> {
        let (s, e) = self.bntx_range?;
        let slice = &b[s..e];
        Some(Bntx::parse(slice).map(|x| (x, slice)))
    }

    /// Map from texture name to its index inside the embedded `BNTX`.
    pub fn bntx_index_by_name(bntx: &Bntx) -> HashMap<String, usize> {
        bntx.textures
            .iter()
            .enumerate()
            .map(|(i, t)| (t.name.clone(), i))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn put_u32(b: &mut [u8], o: usize, v: u32) {
        b[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }

    fn put_f32(b: &mut [u8], o: usize, v: f32) {
        b[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }

    fn header(b: &mut [u8], at: usize, magic: &[u8; 4], child: u32, next: u32, data: u32, n: u16) {
        b[at..at + 4].copy_from_slice(magic);
        put_u32(b, at + 0x08, child);
        put_u32(b, at + 0x0C, next);
        put_u32(b, at + 0x10, NONE);
        put_u32(b, at + 0x14, data);
        b[at + 0x1C..at + 0x1E].copy_from_slice(&n.to_le_bytes());
    }

    /// A synthetic one-emitter file in the observed layout.
    fn synthetic() -> Vec<u8> {
        let emtr = 0x100;
        let emtr_data = emtr + 0x100;
        let grtf = emtr_data + EMITTER_DATA_SIZE;
        let gtnt = grtf + 0x20;
        let total = gtnt + 0x20 + 0x20;
        let mut b = vec![0u8; total];
        b[0..8].copy_from_slice(b"VFXB    ");
        b[0x08..0x0A].copy_from_slice(&0x0400u16.to_le_bytes());
        b[0x0A..0x0C].copy_from_slice(&0x0016u16.to_le_bytes());
        b[0x0C..0x0E].copy_from_slice(&0xFEFFu16.to_le_bytes());
        b[0x16..0x18].copy_from_slice(&0x40u16.to_le_bytes());
        put_u32(&mut b, 0x1C, total as u32);
        header(&mut b, 0x40, b"ESTA", 0x20, (grtf - 0x40) as u32, 0x20, 1);
        header(&mut b, 0x60, b"ESET", (emtr - 0x60) as u32, NONE, 0x20, 1);
        b[0x90..0x94].copy_from_slice(b"set1");
        header(&mut b, emtr, b"EMTR", NONE, NONE, 0x100, 0);
        let d = emtr_data;
        b[d + 0x10..d + 0x14].copy_from_slice(b"glow");
        put_u32(&mut b, d + off::KEY_COUNTS + 4, 2); // alpha0: 2 keys
        put_f32(&mut b, d + 0x440, 1.0);
        put_f32(&mut b, d + 0x444, 1.0);
        put_f32(&mut b, d + 0x448, 1.0);
        put_f32(&mut b, d + 0x44C, 0.0);
        put_f32(&mut b, d + 0x45C, 1.0); // second key: 0 at t = 1
        b[d + off::DISPLAY] = 5;
        b[d + off::BLEND] = 1;
        put_u32(&mut b, d + off::LIFE, 24);
        put_f32(&mut b, d + off::EMIT_RATE, 3.0);
        b[d + off::SAMPLERS[0]..d + off::SAMPLERS[0] + 8].copy_from_slice(&7u64.to_le_bytes());
        for s in &off::SAMPLERS[1..] {
            b[d + s..d + s + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        }
        b[d + off::PRIMITIVE_ID..d + off::PRIMITIVE_ID + 8]
            .copy_from_slice(&u64::MAX.to_le_bytes());
        header(&mut b, grtf, b"GRTF", 0x20, NONE, NONE, 1);
        header(&mut b, gtnt, b"GTNT", NONE, NONE, 0x20, 0);
        let e = gtnt + 0x20;
        b[e..e + 8].copy_from_slice(&7u64.to_le_bytes());
        put_u32(&mut b, e + 12, 5);
        b[e + 16..e + 20].copy_from_slice(b"tex0");
        b
    }

    #[test]
    fn parses_synthetic_layout() {
        let f = PtclFile::parse(&synthetic()).unwrap();
        assert_eq!(f.top_sections, ["ESTA", "GRTF"]);
        assert_eq!(f.emitter_set_name, "set1");
        assert_eq!(f.emitters.len(), 1);
        let e = &f.emitters[0];
        assert_eq!(e.name, "glow");
        assert_eq!(e.display, DisplayType::VelocityLook);
        assert_eq!(e.blend, BlendType::Add);
        assert_eq!(e.life_frames, 24);
        assert_eq!(e.emit_rate, 3.0);
        assert_eq!(e.alpha0_keys.keys.len(), 2);
        assert!((e.alpha_at(0.5) - 0.5).abs() < 1e-6);
        assert_eq!(e.texture_ids, [Some(7), None, None]);
        assert_eq!(f.texture_name(7), Some("tex0"));
        assert_eq!(e.shape(), "streak");
    }

    #[test]
    fn rejects_other_versions_and_truncation() {
        let mut b = synthetic();
        b[0x0A] = 0x15;
        assert!(PtclFile::parse(&b).is_err());
        let b = synthetic();
        assert!(PtclFile::parse(&b[..0x300]).is_err());
        assert!(PtclFile::parse(b"VFXB").is_err());
    }

    #[test]
    fn key_table_interpolates_and_clamps() {
        let t = KeyTable {
            keys: vec![
                Key {
                    value: [1.0, 0.0, 0.0],
                    time: 0.0,
                },
                Key {
                    value: [0.0, 1.0, 0.0],
                    time: 0.5,
                },
                Key {
                    value: [0.0, 0.0, 1.0],
                    time: 1.0,
                },
            ],
        };
        assert_eq!(t.sample(-1.0), Some([1.0, 0.0, 0.0]));
        assert_eq!(t.sample(0.25), Some([0.5, 0.5, 0.0]));
        assert_eq!(t.sample(2.0), Some([0.0, 0.0, 1.0]));
        assert_eq!(KeyTable::default().sample(0.3), None);
    }

    fn particle_root() -> PathBuf {
        std::env::var_os("PLA_BIN_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into()
            })
            .join("effect/battle/particle")
    }

    /// (file, emitters, textures, first emitter name, its display byte, blend byte, life, first texture)
    const REAL: [(&str, usize, usize, &str, DisplayType, BlendType, i32, &str); 5] = [
        (
            "ew033/ew033_df_hit.ptcl",
            2,
            2,
            "glow",
            DisplayType::Billboard,
            BlendType::Add,
            24,
            "cpt_1_circle0004",
        ),
        (
            "ew085/ew085_spark01.ptcl",
            10,
            10,
            "spark02_up_side_nor",
            DisplayType::VelocityLook,
            BlendType::Normal,
            8,
            "cpt_2_thunder0209a",
        ),
        (
            "ew052/ew052_hit.ptcl",
            6,
            6,
            "fire2",
            DisplayType::Billboard,
            BlendType::Normal,
            28,
            "cpt_2_fire0004",
        ),
        (
            "ew052/ew052_bullet.ptcl",
            4,
            6,
            "fire_core",
            DisplayType::Billboard,
            BlendType::Normal,
            20,
            "cpt_2_fire0010",
        ),
        (
            "ew098/ew098_df_hit.ptcl",
            9,
            7,
            "back",
            DisplayType::Billboard,
            BlendType::Sub,
            16,
            "cpt_1_circle0001",
        ),
    ];

    #[test]
    fn decodes_five_real_battle_effects_if_present() {
        let root = particle_root();
        if !root.join(REAL[0].0).exists() {
            eprintln!(
                "[skip] game effect files not present under {}",
                root.display()
            );
            return;
        }
        for (file, n_emit, n_tex, name, display, blend, life, tex) in REAL {
            let bytes = std::fs::read(root.join(file)).unwrap();
            let f = PtclFile::parse(&bytes).unwrap_or_else(|e| panic!("{file}: {e}"));
            assert_eq!(f.file_size as usize, bytes.len(), "{file}");
            assert_eq!(f.emitters.len(), n_emit, "{file}");
            assert_eq!(f.texture_names.len(), n_tex, "{file}");
            let e = &f.emitters[0];
            assert_eq!(e.name, name, "{file}");
            assert_eq!(e.display, display, "{file}");
            assert_eq!(e.blend, blend, "{file}");
            assert_eq!(e.life_frames, life, "{file}");
            assert_eq!(
                f.texture_name(e.texture_ids[0].unwrap()),
                Some(tex),
                "{file}"
            );
            // Every sampler id resolves in the file's own name table.
            for em in &f.emitters {
                for id in em.texture_ids.iter().flatten() {
                    assert!(f.texture_name(*id).is_some(), "{file}: {}", em.name);
                }
            }
            // The embedded texture bank parses and holds every named texture.
            let (bntx, _) = f.bntx(&bytes).unwrap().unwrap();
            let names = PtclFile::bntx_index_by_name(&bntx);
            for t in &f.texture_names {
                assert!(names.contains_key(&t.name), "{file}: {}", t.name);
            }
        }
    }
}
