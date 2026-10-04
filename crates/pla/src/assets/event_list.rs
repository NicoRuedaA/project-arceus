//! `bin/event/event_progress/event_list.bin` — the event system's name table.
//!
//! ## Verified layout
//!
//! ```text
//! 0x00 u32 version (12 or 16)
//! 0x04 u32 (version 16 only: zero; version 12 has the fields below here)
//!      u16 0 | u16 6 | u16 8 | u16 4 | u32 6 | u32 4
//!      u32 count
//!      u32 offsets[count]   (descending; they index the record graph)
//! ```
//!
//! Cross-checked on `destination_list.bin` (count 0), `event_result_sub_work.bin`
//! (46) and `event_list.bin` (739).
//!
//! After the offset array the file carries a **name table**: length-prefixed
//! identifiers (`u32 length` + `length` bytes), 1 821 of them in `event_list`,
//! non-overlapping. It holds event names (`TutorialNPC_area03_001`,
//! `NsRematchWinExtraAct`), flag names (`FEVE_*`, `FSYS_*`) and other event-system
//! identifiers — it is the string pool the records reference.
//!
//! The records themselves form a nested graph of offset tables (the fields hold
//! signed relative offsets into further tables); its layout is not decoded yet
//! (dec040), so the port uses the name table for event identity and keeps the
//! per-event data fields at their defaults.

use std::collections::HashMap;

/// A parsed `event_list.bin` header plus its name table.
#[derive(Debug, Clone)]
pub struct EventList {
    pub version: u32,
    /// Header count (739 in `event_list.bin`) — the number of record offsets.
    pub count: u32,
    pub offsets: Vec<u32>,
    pub names: Vec<String>,
    by_hash: HashMap<u64, usize>,
}

fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn is_ident(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && (bytes[0].is_ascii_alphabetic() || bytes[0] == b'_')
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
}

impl EventList {
    pub fn parse(bytes: &[u8]) -> Result<EventList, String> {
        if bytes.len() < 0x18 {
            return Err("event_list too short".into());
        }
        let version = u32le(bytes, 0);
        if version != 12 && version != 16 {
            return Err(format!("unsupported event_list version {version}"));
        }
        // Version 16 carries one extra zero word before the shared fields.
        let field = 0x04 + if version == 16 { 4 } else { 0 };
        let count_at = field + 0x10;
        let offsets_at = count_at + 4;
        let count = u32le(bytes, count_at);
        if offsets_at + count as usize * 4 > bytes.len() {
            return Err(format!("offset array ({count}) out of range"));
        }
        let offsets: Vec<u32> = (0..count as usize)
            .map(|i| u32le(bytes, offsets_at + i * 4))
            .collect();

        // Name table: every `u32 length` + identifier, non-overlapping.
        let mut names = Vec::new();
        let mut by_hash = HashMap::new();
        let mut pos = offsets_at + count as usize * 4;
        while pos + 4 <= bytes.len() {
            let len = u32le(bytes, pos) as usize;
            if (3..=64).contains(&len) && pos + 4 + len <= bytes.len() {
                let raw = &bytes[pos + 4..pos + 4 + len];
                if is_ident(raw) {
                    let name = String::from_utf8_lossy(raw).to_string();
                    by_hash
                        .entry(crate::save::fnv1a64_str(&name))
                        .or_insert(names.len());
                    names.push(name);
                    pos += 4 + len;
                    continue;
                }
            }
            pos += 1;
        }
        Ok(EventList {
            version,
            count,
            offsets,
            names,
            by_hash,
        })
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// The port's `FnvHash64` of the name resolves to that name (the scripts key
    /// event data by the hash of their class name).
    pub fn find_by_hash(&self, hash: u64) -> Option<&str> {
        self.by_hash.get(&hash).map(|&i| self.names[i].as_str())
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.by_hash.get(&crate::save::fnv1a64_str(name)).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic() -> Vec<u8> {
        // version 12, 2 offsets, then two length-prefixed names.
        let mut b = Vec::new();
        b.extend_from_slice(&12u32.to_le_bytes());
        b.extend_from_slice(&[0u8; 16]);
        b.extend_from_slice(&2u32.to_le_bytes()); // count
        b.extend_from_slice(&0x100u32.to_le_bytes());
        b.extend_from_slice(&0x80u32.to_le_bytes());
        for name in ["SomeEvent", "FSYS_FLAG_ONE"] {
            b.extend_from_slice(&(name.len() as u32).to_le_bytes());
            b.extend_from_slice(name.as_bytes());
        }
        b
    }

    #[test]
    fn parses_header_offsets_and_names() {
        let list = EventList::parse(&synthetic()).unwrap();
        assert_eq!(list.version, 12);
        assert_eq!(list.count, 2);
        assert_eq!(list.offsets, vec![0x100, 0x80]);
        assert_eq!(list.names, vec!["SomeEvent", "FSYS_FLAG_ONE"]);
        assert_eq!(list.find("SomeEvent"), Some(0));
        assert_eq!(list.find("Nope"), None);
    }

    #[test]
    fn resolves_names_by_hash() {
        let list = EventList::parse(&synthetic()).unwrap();
        let hash = crate::save::fnv1a64_str("FSYS_FLAG_ONE");
        assert_eq!(list.find_by_hash(hash), Some("FSYS_FLAG_ONE"));
        assert_eq!(list.find_by_hash(0), None);
    }

    #[test]
    fn rejects_a_bad_version() {
        let mut b = synthetic();
        b[0] = 99;
        assert!(EventList::parse(&b).is_err());
    }
}
