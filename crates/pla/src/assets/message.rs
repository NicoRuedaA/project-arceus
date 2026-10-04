//! Game Freak message tables: the AHTB key table (`.tbl`) and the message
//! payload (`.dat`).
//!
//! ## `.dat` layout (verified on 322/322 English files)
//!
//! ```text
//! 0x00 u16 version (1) | u16 count
//! 0x04 u32 size        (== file size - 16)
//! 0x08 u32 zero
//! 0x0C u32 base (0x10)
//! 0x10 u32 size        (repeated)
//! 0x14 count x 8 bytes:
//!        u32 offset   (relative to `base`)
//!        u16 length   (UTF-16 code units)
//!        u16 flags
//! base + offset: length*2 bytes of string data, 4-byte aligned
//! ```
//!
//! Every entry's offset is exactly the previous entry's aligned end and the
//! last entry ends at the file end — that is the parity check the tests run.
//!
//! ## Text encoding (cracked, dec046)
//!
//! `plain_char[p] = ciphertext[2p] XOR key[p % 16]`, where `key` is a 16-byte
//! key that depends only on the entry **index** and repeats every 16 bytes.
//! Characters live in the even bytes (the odd byte of each pair is a per-char
//! attribute) and `0x00` terminates the string. The keys are recovered by
//! frequency analysis (`.tools/message_cipher.py`); they are derived from game
//! text, so they stay outside the repository and are loaded at runtime.
//!
//! The `.tbl` is an AHTB name table (see [`crate::assets::ahtb`]). Its entries
//! map 1:1 to the `.dat` entries in order; the trailing sentinel key (e.g.
//! `msg_bag_pocket_max`) has no message.

use crate::assets::ahtb::Ahtb;

/// One message entry: where its code units live and how many there are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageEntry {
    /// Offset relative to the file's `base` (0x10).
    pub offset: u32,
    /// Length in UTF-16 code units.
    pub length: u16,
    /// Entry flags (observed values: 0 and 4; meaning unidentified).
    pub flags: u16,
}

/// A parsed `.dat` message file.
#[derive(Debug, Clone)]
pub struct MessageFile {
    pub version: u16,
    /// Absolute file offset the entry offsets are relative to (0x10).
    pub base: u32,
    pub entries: Vec<MessageEntry>,
}

impl MessageFile {
    pub fn parse(bytes: &[u8]) -> Result<MessageFile, String> {
        if bytes.len() < 0x1C {
            return Err("message .dat too short".into());
        }
        let version = u16::from_le_bytes([bytes[0], bytes[1]]);
        let count = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
        let size = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != 1 {
            return Err(format!("unsupported message version {version}"));
        }
        if size as usize + 16 != bytes.len() {
            return Err(format!(
                "declared size {size} != actual {}",
                bytes.len() - 16
            ));
        }
        let base = u32::from_le_bytes(bytes[0x0C..0x10].try_into().unwrap());
        if base as usize != 0x10 {
            return Err(format!("unexpected base {base:#x}"));
        }
        let mut entries = Vec::with_capacity(count);
        let mut expected: Option<usize> = None;
        for i in 0..count {
            let e = 0x14 + i * 8;
            if e + 8 > bytes.len() {
                return Err(format!("entry table truncated at {i}"));
            }
            let offset = u32::from_le_bytes(bytes[e..e + 4].try_into().unwrap());
            let length = u16::from_le_bytes([bytes[e + 4], bytes[e + 5]]);
            let flags = u16::from_le_bytes([bytes[e + 6], bytes[e + 7]]);
            let start = base as usize + offset as usize;
            let end = start + length as usize * 2;
            if end > bytes.len() {
                return Err(format!("entry {i} out of range"));
            }
            if let Some(expected) = expected {
                if start != expected {
                    return Err(format!(
                        "entry {i} starts at {start:#x}, expected {expected:#x}"
                    ));
                }
            }
            expected = Some((end + 3) & !3);
            entries.push(MessageEntry {
                offset,
                length,
                flags,
            });
        }
        if let Some(expected) = expected {
            if expected != bytes.len() {
                return Err(format!(
                    "last entry ends at {expected:#x}, file is {:#x}",
                    bytes.len()
                ));
            }
        }
        Ok(MessageFile {
            version,
            base,
            entries,
        })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Raw code units of entry `index` (the text is still encoded; see the
    /// module docs).
    pub fn payload<'a>(&self, bytes: &'a [u8], index: usize) -> Option<&'a [u8]> {
        let entry = self.entries.get(index)?;
        let start = self.base as usize + entry.offset as usize;
        Some(&bytes[start..start + entry.length as usize * 2])
    }
}

/// A `.tbl` + `.dat` pair: message key name -> raw payload.
#[derive(Debug, Clone)]
pub struct MessageStore {
    names: Vec<String>,
    file: MessageFile,
}

impl MessageStore {
    /// Parse a `.tbl` (AHTB keys) and its `.dat` payload. The keys map 1:1 to
    /// the payload entries in order; extra trailing keys (the sentinel) are
    /// kept but resolve to no payload.
    pub fn parse(tbl: &[u8], dat: &[u8]) -> Result<MessageStore, String> {
        let table = Ahtb::parse(tbl)?;
        let file = MessageFile::parse(dat)?;
        if table.len() < file.len() {
            return Err(format!(
                "table has {} keys but the payload has {} entries",
                table.len(),
                file.len()
            ));
        }
        Ok(MessageStore {
            names: table.entries.iter().map(|e| e.name.clone()).collect(),
            file,
        })
    }

    pub fn count(&self) -> usize {
        self.file.len()
    }

    /// Entry index for a key name, if it has a payload.
    pub fn resolve(&self, key: &str) -> Option<usize> {
        let index = self.names.iter().position(|n| n == key)?;
        if index < self.file.len() {
            Some(index)
        } else {
            None
        }
    }

    /// Raw code units for a key, if it has a payload.
    pub fn raw<'a>(&self, dat: &'a [u8], key: &str) -> Option<&'a [u8]> {
        self.file.payload(dat, self.resolve(key)?)
    }

    pub fn file(&self) -> &MessageFile {
        &self.file
    }
}

/// The per-entry-index XOR keys (16 bytes each) that decrypt message text.
#[derive(Debug, Clone, Default)]
pub struct MessageKeystream {
    keys: Vec<[u8; 16]>,
}

impl MessageKeystream {
    /// Parse the `message_cipher.py` output: one `index:hex(32)` line per key.
    pub fn parse(text: &str) -> Result<MessageKeystream, String> {
        let mut keys: Vec<[u8; 16]> = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (index, hex) = line.split_once(':').ok_or("missing ':'")?;
            let index: usize = index.trim().parse().map_err(|_| "bad index")?;
            let bytes = hex.trim().as_bytes();
            if bytes.len() != 32 {
                return Err(format!("key {index} is not 16 bytes"));
            }
            let mut key = [0u8; 16];
            for (i, slot) in key.iter_mut().enumerate() {
                let pair = std::str::from_utf8(&bytes[i * 2..i * 2 + 2]).map_err(|_| "bad hex")?;
                *slot = u8::from_str_radix(pair, 16).map_err(|_| "bad hex")?;
            }
            if keys.len() <= index {
                keys.resize(index + 1, [0u8; 16]);
            }
            keys[index] = key;
        }
        if keys.is_empty() {
            return Err("empty keystream".into());
        }
        Ok(MessageKeystream { keys })
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Decrypt entry `index`'s code units (characters are the even bytes,
    /// `0x00` terminates).
    pub fn decode(&self, index: usize, ciphertext: &[u8]) -> Option<String> {
        let key = self.keys.get(index)?;
        let mut out = String::new();
        for (p, pair) in ciphertext.chunks(2).enumerate() {
            let b = pair[0] ^ key[p % 16];
            if b == 0 {
                break;
            }
            out.push(b as char);
        }
        Some(out)
    }

    /// Decrypt by message key name through a [`MessageStore`].
    pub fn decode_key(&self, store: &MessageStore, dat: &[u8], key: &str) -> Option<String> {
        let index = store.resolve(key)?;
        self.decode(index, store.file().payload(dat, index)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_with_the_repeating_key() {
        // key = 01 02 .. 10, text "ABC" (0x41 0x42 0x43) then NUL.
        let mut text = String::from("0:0102030405060708090a0b0c0d0e0f10\n");
        let ks = MessageKeystream::parse(&text).unwrap();
        assert_eq!(ks.len(), 1);
        let mut cipher = Vec::new();
        for (p, ch) in "ABC".bytes().enumerate() {
            cipher.push(ch ^ (p as u8 + 1));
            cipher.push(0);
        }
        // terminator: plaintext 0x00 XOR key[3]
        cipher.extend_from_slice(&[0x04, 0]);
        assert_eq!(ks.decode(0, &cipher).as_deref(), Some("ABC"));
        // a missing index decodes to nothing
        assert_eq!(ks.decode(9, &cipher), None);
        text.push_str("1:00000000000000000000000000000000\n");
        assert!(MessageKeystream::parse(&text).is_ok());
        assert!(MessageKeystream::parse("bad").is_err());
    }

    #[test]
    fn rejects_a_truncated_header() {
        assert!(MessageFile::parse(&[0u8; 8]).is_err());
    }

    #[test]
    fn rejects_a_size_mismatch() {
        // version 1, one entry, declared size 0x100 but a 0x20-byte buffer.
        let mut bytes = vec![0u8; 0x20];
        bytes[0] = 1;
        bytes[4..8].copy_from_slice(&0x100u32.to_le_bytes());
        bytes[0x0C..0x10].copy_from_slice(&0x10u32.to_le_bytes());
        assert!(MessageFile::parse(&bytes).is_err());
    }
}
