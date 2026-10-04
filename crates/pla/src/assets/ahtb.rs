//! AHTB name-table parser (Game Freak tables: event flags, works, map flags).
//!
//! Layout (verified byte-exact on real tables: 13 017/13 017 bytes consumed):
//!   "AHTB" | u32 entry_count
//!   entries: { u64 id, u16 name_len (includes the NUL), name bytes }
//!
//! The u64 is an opaque id (not FNV/CRC of the name); the port carries it
//! verbatim as the flag/work identity.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AhtbEntry {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Ahtb {
    pub entries: Vec<AhtbEntry>,
}

impl Ahtb {
    pub fn parse(bytes: &[u8]) -> Result<Ahtb, String> {
        if bytes.len() < 8 || &bytes[0..4] != b"AHTB" {
            return Err("not an AHTB table".into());
        }
        let count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let mut entries = Vec::with_capacity(count);
        let mut pos = 8;
        for i in 0..count {
            if pos + 10 > bytes.len() {
                return Err(format!("truncated at entry {i}"));
            }
            let id = u64::from_le_bytes(bytes[pos..pos + 8].try_into().unwrap());
            let name_len =
                u16::from_le_bytes(bytes[pos + 8..pos + 10].try_into().unwrap()) as usize;
            pos += 10;
            if name_len == 0 || pos + name_len > bytes.len() {
                return Err(format!("bad name length {name_len} at entry {i}"));
            }
            let raw = &bytes[pos..pos + name_len - 1]; // drop the NUL
            let name = String::from_utf8_lossy(raw).to_string();
            pos += name_len;
            entries.push(AhtbEntry { id, name });
        }
        if pos != bytes.len() {
            return Err(format!(
                "{} trailing bytes after {} entries",
                bytes.len() - pos,
                count
            ));
        }
        Ok(Ahtb { entries })
    }

    pub fn find(&self, name: &str) -> Option<&AhtbEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
