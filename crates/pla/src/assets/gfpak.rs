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
//! Entry tables may be followed directly by payloads or by an 8-byte pad so the
//! first payload is 16-byte aligned (observed in kind-4 Pokémon model packs).

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

impl GfPakEntry {
    /// Observed declared decoded size in Pokémon kind-4 Oodle entries.
    /// The field is named `checksum` for compatibility with the earlier parser.
    pub fn declared_decompressed_size(&self) -> u32 {
        self.checksum
    }
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

        // Locate the file-entry table by checking whether its first payload
        // begins immediately after the rows or at the next 16-byte boundary.
        let entry_size = 24u64;
        let mut found = None;
        for &table in &table_offsets {
            let table_end = table.checked_add(entry_count as u64 * entry_size);
            let Some(table_end) = table_end else {
                continue;
            };
            if table_end + entry_size > size {
                continue;
            }
            let first = parse_entry(bytes, table as usize)?;
            let aligned_start = table_end.checked_add(15).map(|n| n & !15);
            if first.data_offset as u64 == table_end
                || Some(first.data_offset as u64) == aligned_start
            {
                found = Some((table, table_end));
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
        let end = start
            .checked_add(entry.size as usize)
            .ok_or_else(|| "entry range overflow".to_string())?;
        bytes
            .get(start..end)
            .ok_or_else(|| "entry out of range".to_string())
    }

    /// Return an Oodle-framed entry decoded in memory, or a copy of a raw
    /// payload when the entry does not use the observed Oodle frame headers.
    ///
    /// In Pokémon kind-4 packs, `checksum` is observed to be the declared
    /// decompressed byte count and `size` is the compressed payload length.
    pub fn decoded_data(&self, bytes: &[u8], entry: &GfPakEntry) -> Result<Vec<u8>, String> {
        let payload = self.data(bytes, entry)?;
        if !is_oodle_frame(payload) {
            return Ok(payload.to_vec());
        }

        let expected = usize::try_from(entry.declared_decompressed_size())
            .map_err(|_| "declared decompressed size does not fit this platform".to_string())?;
        if expected == 0 || expected > 256 * 1024 * 1024 {
            return Err(format!("invalid declared decompressed size {expected}"));
        }
        let mut output = vec![0; expected];
        let written = oozextract::Extractor::new()
            .read_from_slice(payload, &mut output)
            .map_err(|error| format!("Oodle decode failed: {error}"))?;
        if written != expected {
            return Err(format!(
                "Oodle wrote {written} bytes; entry declares {expected}"
            ));
        }
        Ok(output)
    }
}

/// The Pokémon model corpus observed so far uses Oodle Kraken frames. `0x8c`
/// is a compressed block; `0xcc` marks an uncompressed block in the same frame.
pub fn is_oodle_frame(payload: &[u8]) -> bool {
    payload.len() >= 2
        && payload[0] & 0x0f == 0x0c
        && (payload[0] >> 4) & 0x03 == 0
        && payload[1] & 0x7f == 0x06
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
