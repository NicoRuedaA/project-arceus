//! SARC archive parser (Nintendo's standard archive format).
//!
//! Layout (verified against the Python extractor on real game archives):
//!   0x00 "SARC" | header_size u16 (0x14) | BOM u16 (0xFEFF) | file_size u32 |
//!        data_offset u32 | version u16 (0x0100) | reserved u16
//!   0x14 "SFAT" | header_size u16 (0x0C) | node_count u16 | hash_key u32
//!   0x20 nodes: { hash u32, attrs u32, data_start u32, data_end u32 }
//!        attrs = (0x01 << 24) | (name_offset / 4)
//!   then "SFNT" | header_size u16 (0x08) | name_count u16 | names (NUL-terminated,
//!        each padded to a 4-byte boundary; attrs counts 4-byte units)
//!   file data at data_offset + node.data_start

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SarcEntry {
    pub hash: u32,
    /// Byte offset of the entry name inside the SFNT name table.
    pub name_offset: u32,
    pub data_start: u32,
    pub data_end: u32,
}

impl SarcEntry {
    pub fn size(&self) -> u32 {
        self.data_end.saturating_sub(self.data_start)
    }
}

#[derive(Debug, Clone)]
pub struct Sarc {
    pub file_size: u32,
    pub data_offset: u32,
    pub version: u16,
    pub hash_key: u32,
    pub entries: Vec<SarcEntry>,
    name_table: Vec<u8>,
}

impl Sarc {
    pub fn parse(bytes: &[u8]) -> Result<Sarc, String> {
        if bytes.len() < 0x20 || &bytes[0..4] != b"SARC" {
            return Err("not a SARC archive".into());
        }
        let bom = u16::from_le_bytes([bytes[6], bytes[7]]);
        if bom != 0xFEFF {
            return Err(format!("bad BOM {bom:#06x}"));
        }
        let file_size = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let data_offset = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let version = u16::from_le_bytes([bytes[16], bytes[17]]);
        if &bytes[0x14..0x18] != b"SFAT" {
            return Err("missing SFAT".into());
        }
        let node_count = u16::from_le_bytes([bytes[0x1A], bytes[0x1B]]) as usize;
        let hash_key = u32::from_le_bytes(bytes[0x1C..0x20].try_into().unwrap());
        let mut entries = Vec::with_capacity(node_count);
        let mut pos = 0x20;
        for _ in 0..node_count {
            if pos + 0x10 > bytes.len() {
                return Err("truncated SFAT".into());
            }
            let hash = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());
            let attrs = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap());
            let data_start = u32::from_le_bytes(bytes[pos + 8..pos + 12].try_into().unwrap());
            let data_end = u32::from_le_bytes(bytes[pos + 12..pos + 16].try_into().unwrap());
            entries.push(SarcEntry {
                hash,
                // The low 24 bits count 4-byte units (verified on the
                // multi-file layout archives of `bin/appli`).
                name_offset: (attrs & 0x00FF_FFFF) * 4,
                data_start,
                data_end,
            });
            pos += 0x10;
        }
        if &bytes[pos..pos + 4] != b"SFNT" {
            return Err("missing SFNT".into());
        }
        // The SFNT header stores header_size at +4 and a zero field at +6; the
        // name count lives only in the SFAT (verified on real archives).
        let names_start = pos + 8;
        let name_table = bytes[names_start..].to_vec();
        Ok(Sarc {
            file_size,
            data_offset,
            version,
            hash_key,
            entries,
            name_table,
        })
    }

    pub fn file_count(&self) -> usize {
        self.entries.len()
    }

    /// Resolve an entry's name from the SFNT name table.
    pub fn name(&self, entry: &SarcEntry) -> String {
        let start = entry.name_offset as usize;
        if start >= self.name_table.len() {
            return String::new();
        }
        let rest = &self.name_table[start..];
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        String::from_utf8_lossy(&rest[..end]).to_string()
    }

    /// Borrow one entry's payload from the archive bytes.
    pub fn data<'a>(&self, bytes: &'a [u8], entry: &SarcEntry) -> Result<&'a [u8], String> {
        let start = self.data_offset as usize + entry.data_start as usize;
        let end = self.data_offset as usize + entry.data_end as usize;
        bytes
            .get(start..end)
            .ok_or_else(|| "entry out of range".to_string())
    }

    pub fn find(&self, name: &str) -> Option<&SarcEntry> {
        self.entries.iter().find(|e| self.name(e) == name)
    }
}

#[cfg(test)]
mod tests {
    use super::Sarc;

    /// Two-entry archive with the second name at a 4-byte-unit offset.
    fn two_entry_sarc() -> Vec<u8> {
        let names: &[u8] = b"a.txt\0\0\0timg/b.bntx\0\0";
        let data_offset = 0x20 + 2 * 16 + 8 + names.len();
        let mut b = Vec::new();
        b.extend_from_slice(b"SARC");
        b.extend_from_slice(&0x14u16.to_le_bytes());
        b.extend_from_slice(&0xFEFFu16.to_le_bytes());
        b.extend_from_slice(&((data_offset + 8) as u32).to_le_bytes());
        b.extend_from_slice(&(data_offset as u32).to_le_bytes());
        b.extend_from_slice(&0x0100u16.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(b"SFAT");
        b.extend_from_slice(&0x0Cu16.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&0x65u32.to_le_bytes());
        for (attrs, start, end) in [(0x0100_0000u32, 0u32, 4u32), (0x0100_0002, 4, 8)] {
            b.extend_from_slice(&0u32.to_le_bytes());
            b.extend_from_slice(&attrs.to_le_bytes());
            b.extend_from_slice(&start.to_le_bytes());
            b.extend_from_slice(&end.to_le_bytes());
        }
        b.extend_from_slice(b"SFNT");
        b.extend_from_slice(&8u16.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(names);
        b.extend_from_slice(b"AAAABBBB");
        b
    }

    #[test]
    fn names_use_four_byte_units() {
        let bytes = two_entry_sarc();
        let sarc = Sarc::parse(&bytes).unwrap();
        assert_eq!(sarc.name(&sarc.entries[0]), "a.txt");
        assert_eq!(sarc.name(&sarc.entries[1]), "timg/b.bntx");
        assert_eq!(sarc.data(&bytes, &sarc.entries[1]).unwrap(), b"BBBB");
        assert!(sarc.find("timg/b.bntx").is_some());
    }
}
