//! Minimal reader for Havok binary tagfiles (`TAG0`), as found in PLA terrain
//! navigation meshes and collision packs.
//!
//! The layout below was derived from the bytes of the game files themselves
//! (embedded type-name strings, section tags and sizes) and checked on all 63
//! terrain navmesh files plus the 712 collision files; no SDK code is used.
//!
//! Observed structure
//! - Section header: big-endian `u32` = `size | flags << 30` (size counts the
//!   8-byte header; flag bit 0 = leaf with data), then a 4-byte ASCII tag.
//!   Containers (`TAG0`, `TYPE`, `INDX`) hold child sections.
//! - `TST1`/`FST1`: NUL-separated type names and member names (0-based).
//! - `TNA1`: packed-int type count `n`, then types `1..n`, each
//!   `name, template_arg_count, (arg_name, arg_value)*`.
//! - `TBDY`: per type `index, parent, flags` then optional parts in this order:
//!   `format` (flag 1), `subtype` (2), `version` (4), `size, align` (8),
//!   `abstract` (0x10), `members` (0x20: `count, (name, flags, offset, type)*`),
//!   `interfaces` (0x40). Quirk: before the member count some types carry an
//!   extra 2-byte value starting with 0xc1/0xc2 (meaning unknown); it is
//!   skipped. Parsing also resynchronises on the next plausible type header.
//! - `DATA` is the item payload; `ITEM` is an array of 12-byte little-endian
//!   records `(type | kind << 28, offset, count)`: kind 0x1 = object,
//!   0x2 = array of `count` elements. Pointers and arrays inside an object are
//!   stored as a 4-byte index into this item table (0 = null); array lengths
//!   come from the item's `count`, not from the object.
//!
//! Packed integers: first byte `< 0x80` is the value; `10xxxxxx` is 2 bytes
//! (14 bits), `110xxxxx` 3 bytes (21 bits), `1110xxxx` 4 bytes (28 bits).

use std::collections::HashSet;

/// One member of a type definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagField {
    pub name: String,
    pub flags: u32,
    /// Byte offset inside the serialized object (not the in-memory offset).
    pub offset: u32,
    pub type_index: u32,
}

#[derive(Debug, Clone, Default)]
pub struct TagType {
    pub name: String,
    /// `(name string index, type index or value)` template arguments.
    pub args: Vec<(u32, u32)>,
    pub parent: u32,
    pub flags: u32,
    pub format: u32,
    pub subtype: u32,
    pub size: u32,
    pub align: u32,
    pub fields: Vec<TagField>,
    pub defined: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Object,
    Array,
    Other(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TagItem {
    pub type_index: u32,
    pub kind: ItemKind,
    /// Offset into the `DATA` section.
    pub offset: u32,
    pub count: u32,
}

#[derive(Debug, Clone)]
pub struct TagFile {
    pub sdk_version: String,
    /// Index 0 is an unused placeholder, as in the file.
    pub types: Vec<TagType>,
    pub items: Vec<TagItem>,
    pub data: Vec<u8>,
}

struct PackedReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> PackedReader<'a> {
    fn byte(&mut self) -> Result<u32, String> {
        let b = *self.bytes.get(self.pos).ok_or("tagfile: packed int past end")?;
        self.pos += 1;
        Ok(b as u32)
    }

    fn packed(&mut self) -> Result<u32, String> {
        let b = self.byte()?;
        if b & 0x80 == 0 {
            return Ok(b);
        }
        if b & 0xc0 == 0x80 {
            return Ok(((b & 0x3f) << 8) | self.byte()?);
        }
        if b & 0xe0 == 0xc0 {
            let (b1, b2) = (self.byte()?, self.byte()?);
            return Ok(((b & 0x1f) << 16) | (b1 << 8) | b2);
        }
        if b & 0xf0 == 0xe0 {
            let (b1, b2, b3) = (self.byte()?, self.byte()?, self.byte()?);
            return Ok(((b & 0x0f) << 24) | (b1 << 16) | (b2 << 8) | b3);
        }
        Err(format!("tagfile: unsupported packed int prefix {b:#04x}"))
    }
}

fn read_sections<'a>(
    bytes: &'a [u8],
    start: usize,
    end: usize,
    out: &mut Vec<([u8; 4], &'a [u8])>,
) -> Result<(), String> {
    let mut o = start;
    while o + 8 <= end {
        let v = u32::from_be_bytes(bytes[o..o + 4].try_into().unwrap());
        let size = (v & 0x3fff_ffff) as usize;
        let leaf = (v >> 30) & 1 == 1;
        let tag: [u8; 4] = bytes[o + 4..o + 8].try_into().unwrap();
        if size < 8 || o + size > end {
            return Err(format!("tagfile: bad section size {size} at {o}"));
        }
        if leaf {
            out.push((tag, &bytes[o + 8..o + size]));
        } else {
            read_sections(bytes, o + 8, o + size, out)?;
        }
        o += size;
    }
    Ok(())
}

fn split_strings(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

impl TagFile {
    pub fn parse(bytes: &[u8]) -> Result<TagFile, String> {
        if bytes.len() < 16 || &bytes[4..8] != b"TAG0" {
            return Err("tagfile: missing TAG0 signature".into());
        }
        let mut sections = Vec::new();
        read_sections(bytes, 0, bytes.len(), &mut sections)?;
        let find = |tag: &[u8; 4]| -> Result<&[u8], String> {
            sections
                .iter()
                .find(|(t, _)| t == tag)
                .map(|(_, d)| *d)
                .ok_or_else(|| format!("tagfile: missing {} section", String::from_utf8_lossy(tag)))
        };
        let sdk_version = String::from_utf8_lossy(find(b"SDKV")?)
            .trim_end_matches('\0')
            .to_owned();
        let names = split_strings(find(b"TST1")?);
        let member_names = split_strings(find(b"FST1")?);
        let name_of = |i: u32| names.get(i as usize).cloned().unwrap_or_default();

        let mut r = PackedReader { bytes: find(b"TNA1")?, pos: 0 };
        let count = r.packed()? as usize;
        if count == 0 || count > 100_000 {
            return Err(format!("tagfile: implausible type count {count}"));
        }
        let mut types = vec![TagType::default()];
        for _ in 1..count {
            let name = name_of(r.packed()?);
            let arg_count = r.packed()?;
            let mut args = Vec::new();
            for _ in 0..arg_count {
                args.push((r.packed()?, r.packed()?));
            }
            types.push(TagType { name, args, ..TagType::default() });
        }

        let tbdy = find(b"TBDY")?;
        let mut r = PackedReader { bytes: tbdy, pos: 0 };
        let mut done: HashSet<u32> = HashSet::new();
        let plausible = |pos: usize, done: &HashSet<u32>| -> bool {
            let mut q = PackedReader { bytes: tbdy, pos };
            match (q.packed(), q.packed(), q.packed()) {
                (Ok(i), Ok(p), Ok(f)) => {
                    (1..count as u32).contains(&i) && !done.contains(&i) && p < count as u32 && f < 0x80
                }
                _ => false,
            }
        };
        while r.pos + 3 < tbdy.len() {
            if !plausible(r.pos, &done) {
                match (1..6).find(|k| plausible(r.pos + k, &done)) {
                    Some(k) => r.pos += k,
                    None => break,
                }
            }
            let index = r.packed()?;
            done.insert(index);
            let parent = r.packed()?;
            let flags = r.packed()?;
            let mut t = TagType {
                name: types[index as usize].name.clone(),
                args: std::mem::take(&mut types[index as usize].args),
                parent,
                flags,
                defined: true,
                ..TagType::default()
            };
            if flags & 1 != 0 {
                t.format = r.packed()?;
            }
            if flags & 2 != 0 {
                t.subtype = r.packed()?;
            }
            if flags & 4 != 0 {
                r.packed()?; // version
            }
            if flags & 8 != 0 {
                t.size = r.packed()?;
                t.align = r.packed()?;
            }
            if flags & 0x10 != 0 {
                r.packed()?; // abstract value
            }
            if flags & 0x20 != 0 {
                // Some types carry an unexplained 2-byte packed value (first byte
                // 0xc1/0xc2, e.g. `hkReferencedObject`: `c2 00`) before the member
                // count; no real count starts with a byte >= 0xc0, so skip it.
                if r.bytes.get(r.pos).is_some_and(|b| *b >= 0xc0) {
                    r.pos += 2;
                }
                let n = r.packed()?;
                for _ in 0..n {
                    let name = member_names
                        .get(r.packed()? as usize)
                        .cloned()
                        .ok_or("tagfile: member name index out of range")?;
                    let flags = r.packed()?;
                    let offset = r.packed()?;
                    let type_index = r.packed()?;
                    t.fields.push(TagField { name, flags, offset, type_index });
                }
            }
            if flags & 0x40 != 0 {
                for _ in 0..r.packed()? {
                    r.packed()?;
                    r.packed()?;
                }
            }
            types[index as usize] = t;
        }

        let data = find(b"DATA")?.to_vec();
        let raw_items = find(b"ITEM")?;
        let mut items = Vec::with_capacity(raw_items.len() / 12);
        for c in raw_items.chunks_exact(12) {
            let w = u32::from_le_bytes(c[0..4].try_into().unwrap());
            let kind = match (w >> 28) as u8 {
                1 => ItemKind::Object,
                2 => ItemKind::Array,
                other => ItemKind::Other(other),
            };
            items.push(TagItem {
                type_index: w & 0x00ff_ffff,
                kind,
                offset: u32::from_le_bytes(c[4..8].try_into().unwrap()),
                count: u32::from_le_bytes(c[8..12].try_into().unwrap()),
            });
        }
        Ok(TagFile { sdk_version, types, items, data })
    }

    pub fn type_by_name(&self, name: &str) -> Option<u32> {
        self.types.iter().position(|t| t.defined && t.name == name).map(|i| i as u32)
    }

    /// Serialized byte size of one element, following the parent chain for
    /// typedef-like types that carry no size of their own.
    pub fn type_size(&self, mut index: u32) -> usize {
        for _ in 0..16 {
            let Some(t) = self.types.get(index as usize) else { return 0 };
            if t.size != 0 || t.parent == 0 {
                return t.size as usize;
            }
            index = t.parent;
        }
        0
    }

    pub fn field(&self, type_index: u32, name: &str) -> Option<&TagField> {
        let mut index = type_index;
        for _ in 0..16 {
            let t = self.types.get(index as usize)?;
            if let Some(f) = t.fields.iter().find(|f| f.name == name) {
                return Some(f);
            }
            if t.parent == 0 {
                return None;
            }
            index = t.parent;
        }
        None
    }

    /// Indices of all items whose type is `type_name`.
    pub fn items_of_type(&self, type_name: &str) -> Vec<usize> {
        let Some(ti) = self.type_by_name(type_name) else { return Vec::new() };
        self.items
            .iter()
            .enumerate()
            .filter(|(_, it)| it.type_index == ti)
            .map(|(i, _)| i)
            .collect()
    }

    /// Raw element bytes of an item (`count * element size`).
    pub fn item_bytes(&self, item: usize) -> Result<&[u8], String> {
        let it = self.items.get(item).ok_or("tagfile: item index out of range")?;
        let size = self.type_size(it.type_index);
        let start = it.offset as usize;
        let end = start
            .checked_add(size.checked_mul(it.count as usize).ok_or("tagfile: item size overflow")?)
            .ok_or("tagfile: item size overflow")?;
        self.data.get(start..end).ok_or_else(|| format!("tagfile: item {item} outside DATA"))
    }

    /// Follow a pointer/array member of an object item to the item it names.
    /// `Ok(None)` means a null reference.
    pub fn member_item(&self, object: usize, member: &str) -> Result<Option<usize>, String> {
        let it = self.items.get(object).ok_or("tagfile: item index out of range")?;
        let f = self
            .field(it.type_index, member)
            .ok_or_else(|| format!("tagfile: no member {member}"))?;
        let at = it.offset as usize + f.offset as usize;
        let v = self.u32_at(at)? as usize;
        if v == 0 {
            return Ok(None);
        }
        if v >= self.items.len() {
            return Err(format!("tagfile: member {member} references item {v} out of range"));
        }
        Ok(Some(v))
    }

    /// Read a scalar `i32`-sized member of an object item.
    pub fn member_u32(&self, object: usize, member: &str) -> Result<u32, String> {
        let it = self.items.get(object).ok_or("tagfile: item index out of range")?;
        let f = self
            .field(it.type_index, member)
            .ok_or_else(|| format!("tagfile: no member {member}"))?;
        self.u32_at(it.offset as usize + f.offset as usize)
    }

    pub fn u32_at(&self, at: usize) -> Result<u32, String> {
        self.data
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .ok_or_else(|| format!("tagfile: read at {at} outside DATA"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_tagfile() {
        assert!(TagFile::parse(b"not a tagfile at all....").is_err());
    }

    #[test]
    fn packed_int_widths() {
        let mut r = PackedReader { bytes: &[0x05, 0xa0, 0x04, 0xc0, 0x82, 0x04], pos: 0 };
        assert_eq!(r.packed().unwrap(), 5);
        assert_eq!(r.packed().unwrap(), 0x2004);
        assert_eq!(r.packed().unwrap(), 0x8204);
    }
}
