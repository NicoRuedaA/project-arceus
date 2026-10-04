//! GFLXPACK container parser (Game Freak pack format).
//!
//! Observed layout (reverse-engineered from real packs; two independent
//! samples agree on the entry invariant):
//!   0x00 "GFLXPACK"
//!   0x08 u32 0x1000           (constant in all samples; alignment/header hint)
//!   0x0C u32 0
//!   0x10 u32 entry_count
//!   0x14 u32 kind             (1 = single table, 3 = multi table)
//!   0x18 u64 offsets...       (table offsets; the 16-byte hash block ends them)
//!   then a 16-byte hash, then tables.
//!
//! File-entry table: `entry_count` entries of 24 bytes
//!   { u16 type, u16 kind, u32 checksum, u32 size, u32 flags, u32 data_offset, u32 reserved }
//! Invariant verified on every sample: `entry[0].data_offset == table_offset + count * 24`
//! (data begins right after the table).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GfPakEntry {
    pub type_id: u16,
    pub kind: u16,
    pub checksum: u32,
    pub size: u32,
    pub flags: u32,
    pub data_offset: u32,
    pub reserved: u32,
}

#[derive(Debug, Clone)]
pub struct GfPak {
    pub entry_count: u32,
    pub kind: u32,
    pub table_offsets: Vec<u64>,
    pub entry_table_offset: u64,
    pub entries: Vec<GfPakEntry>,
}

impl GfPak {
    pub fn parse(bytes: &[u8]) -> Result<GfPak, String> {
        if bytes.len() < 0x40 || &bytes[0..8] != b"GFLXPACK" {
            return Err("not a GFLXPACK".into());
        }
        let entry_count = u32::from_le_bytes(bytes[0x10..0x14].try_into().unwrap());
        let kind = u32::from_le_bytes(bytes[0x14..0x18].try_into().unwrap());
        let size = bytes.len() as u64;

        // Collect the leading u64 table offsets; the 16-byte hash ends them.
        let mut table_offsets = Vec::new();
        let mut pos = 0x18;
        while pos + 8 <= bytes.len() {
            let value = u64::from_le_bytes(bytes[pos..pos + 8].try_into().unwrap());
            if value == 0 || value >= size {
                break;
            }
            table_offsets.push(value);
            pos += 8;
        }

        // The file-entry table is the offset whose first entry's data_offset
        // equals table_offset + count * 24.
        let entry_size = 24u64;
        let mut found = None;
        for &table in &table_offsets {
            let data_start = table + entry_count as u64 * entry_size;
            if data_start + entry_size > size {
                continue;
            }
            let first = parse_entry(bytes, table as usize)?;
            if first.data_offset as u64 == data_start {
                found = Some((table, data_start));
                break;
            }
        }
        let (entry_table_offset, _) = found.ok_or_else(|| {
            format!("no file-entry table among offsets {table_offsets:?} (count={entry_count})")
        })?;

        let mut entries = Vec::with_capacity(entry_count as usize);
        for i in 0..entry_count as u64 {
            entries.push(parse_entry(
                bytes,
                (entry_table_offset + i * entry_size) as usize,
            )?);
        }
        Ok(GfPak {
            entry_count,
            kind,
            table_offsets,
            entry_table_offset,
            entries,
        })
    }

    /// Borrow an entry's payload.
    pub fn data<'a>(&self, bytes: &'a [u8], entry: &GfPakEntry) -> Result<&'a [u8], String> {
        let start = entry.data_offset as usize;
        let end = start + entry.size as usize;
        bytes
            .get(start..end)
            .ok_or_else(|| "entry out of range".to_string())
    }
}

fn parse_entry(bytes: &[u8], offset: usize) -> Result<GfPakEntry, String> {
    let raw = bytes.get(offset..offset + 24).ok_or("truncated entry")?;
    Ok(GfPakEntry {
        type_id: u16::from_le_bytes(raw[0..2].try_into().unwrap()),
        kind: u16::from_le_bytes(raw[2..4].try_into().unwrap()),
        checksum: u32::from_le_bytes(raw[4..8].try_into().unwrap()),
        size: u32::from_le_bytes(raw[8..12].try_into().unwrap()),
        flags: u32::from_le_bytes(raw[12..16].try_into().unwrap()),
        data_offset: u32::from_le_bytes(raw[16..20].try_into().unwrap()),
        reserved: u32::from_le_bytes(raw[20..24].try_into().unwrap()),
    })
}
