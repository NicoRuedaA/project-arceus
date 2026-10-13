//! Audiokinetic Wwise containers: soundbanks (`.bnk`), file packages (`.pck`) and WEM
//! media files (RIFF), plus the Vorbis codebook library the runtime keeps in its executable.
//!
//! Everything here reads DATA formats observed in the game files (and the public Vorbis /
//! Opus bit-stream specifications). No SDK code is translated.
//!
//! * [`Bank::parse`]: `BKHD` / `DIDX` / `DATA` / `HIRC` chunks of a `.bnk`.
//! * [`Pck::parse`]: the `AKPK` header of a `.pck` and its streamed WEM table.
//! * [`Wem::parse`]: RIFF chunks of one media file, with the codec tag and basic format.
//! * [`CodebookLibrary`]: the 599 packed Vorbis codebooks that Wwise Vorbis WEM setup
//!   headers reference by id (read from the game executable image, never stored in git).
//! * [`fnv1_lower`]: Wwise's 32-bit FNV-1 name hash (used for event / bus / state ids).

pub mod bits;
pub mod floor1_db;
pub mod hirc;
pub mod opus;
pub mod vorbis;
pub mod vorbis_dec;
pub mod wav;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::ops::Range;

fn u16le(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}

fn u32le(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// Wwise's name hash: 32-bit FNV-1 over the lower-cased ASCII name.
pub fn fnv1_lower(name: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for c in name.bytes() {
        h = h.wrapping_mul(16777619);
        h ^= c.to_ascii_lowercase() as u32;
    }
    h
}

// ---------------------------------------------------------------------------------------
// Soundbank
// ---------------------------------------------------------------------------------------

/// One entry of a bank's `DIDX` media index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DidxEntry {
    pub id: u32,
    /// Offset inside the `DATA` chunk.
    pub offset: u32,
    pub size: u32,
}

/// One object of the `HIRC` hierarchy: type byte, id and the byte range of its body
/// (after the id) inside the bank file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HircObject {
    pub kind: u8,
    pub id: u32,
    pub body: Range<usize>,
}

/// A parsed soundbank. The struct keeps byte ranges; the caller owns the file bytes.
#[derive(Debug, Clone, Default)]
pub struct Bank {
    pub version: u32,
    pub bank_id: u32,
    pub language_id: u32,
    pub didx: Vec<DidxEntry>,
    pub data: Range<usize>,
    pub hirc: Vec<HircObject>,
}

impl Bank {
    pub fn parse(b: &[u8]) -> Result<Bank, String> {
        let mut bank = Bank::default();
        let mut p = 0usize;
        let mut saw_bkhd = false;
        while p + 8 <= b.len() {
            let tag = &b[p..p + 4];
            let size = u32le(b, p + 4).ok_or("truncated chunk header")? as usize;
            let body = p + 8..(p + 8 + size).min(b.len());
            if body.end > b.len() || p + 8 + size > b.len() {
                return Err(format!(
                    "chunk {:?} overruns the file",
                    String::from_utf8_lossy(tag)
                ));
            }
            match tag {
                b"BKHD" => {
                    bank.version = u32le(b, body.start).ok_or("short BKHD")?;
                    bank.bank_id = u32le(b, body.start + 4).ok_or("short BKHD")?;
                    bank.language_id = u32le(b, body.start + 8).unwrap_or(0);
                    saw_bkhd = true;
                }
                b"DIDX" => {
                    for e in b[body.clone()].chunks_exact(12) {
                        bank.didx.push(DidxEntry {
                            id: u32le(e, 0).unwrap(),
                            offset: u32le(e, 4).unwrap(),
                            size: u32le(e, 8).unwrap(),
                        });
                    }
                }
                b"DATA" => bank.data = body.clone(),
                b"HIRC" => {
                    let count = u32le(b, body.start).ok_or("short HIRC")? as usize;
                    let mut q = body.start + 4;
                    for _ in 0..count {
                        if q + 9 > body.end {
                            return Err("HIRC object header overruns the chunk".into());
                        }
                        let kind = b[q];
                        let osize = u32le(b, q + 1).unwrap() as usize;
                        if osize < 4 || q + 5 + osize > body.end {
                            return Err("HIRC object overruns the chunk".into());
                        }
                        let id = u32le(b, q + 5).unwrap();
                        bank.hirc.push(HircObject {
                            kind,
                            id,
                            body: q + 9..q + 5 + osize,
                        });
                        q += 5 + osize;
                    }
                }
                _ => {}
            }
            p = body.end;
        }
        if !saw_bkhd {
            return Err("no BKHD chunk".into());
        }
        Ok(bank)
    }

    /// Bytes of the embedded media `index` (position in `didx`).
    pub fn media<'a>(&self, file: &'a [u8], index: usize) -> Option<&'a [u8]> {
        let e = self.didx.get(index)?;
        let s = self.data.start.checked_add(e.offset as usize)?;
        file.get(s..s.checked_add(e.size as usize)?)
    }
}

// ---------------------------------------------------------------------------------------
// Package (.pck)
// ---------------------------------------------------------------------------------------

/// One streamed file of an `AKPK` package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PckEntry {
    pub id: u32,
    pub block_size: u32,
    pub size: u32,
    pub start_block: u32,
    pub language: u32,
}

impl PckEntry {
    pub fn byte_offset(&self) -> u64 {
        self.start_block as u64 * self.block_size.max(1) as u64
    }
}

/// An `AKPK` package index (the media bytes stay on disk).
#[derive(Debug, Clone, Default)]
pub struct Pck {
    pub version: u32,
    /// Streamed media (the `stm` table).
    pub streams: Vec<PckEntry>,
    /// Bank entries (the `sb` table); empty in this game's packages.
    pub banks: Vec<PckEntry>,
}

impl Pck {
    pub fn parse(f: &mut File) -> Result<Pck, String> {
        let mut head = [0u8; 28];
        f.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        f.read_exact(&mut head).map_err(|e| e.to_string())?;
        if &head[..4] != b"AKPK" {
            return Err("not an AKPK package".into());
        }
        let header_size = u32le(&head, 4).unwrap() as usize;
        let version = u32le(&head, 8).unwrap();
        let lang = u32le(&head, 12).unwrap() as usize;
        let sb = u32le(&head, 16).unwrap() as usize;
        let stm = u32le(&head, 20).unwrap() as usize;
        let ext = u32le(&head, 24).unwrap() as usize;
        if lang + sb + stm + ext + 20 > header_size + 8 {
            return Err("AKPK section sizes exceed the header".into());
        }
        let mut rest = vec![0u8; lang + sb + stm];
        f.read_exact(&mut rest).map_err(|e| e.to_string())?;
        let read_table = |sec: &[u8]| -> Vec<PckEntry> {
            let n = u32le(sec, 0).unwrap_or(0) as usize;
            let mut v = Vec::new();
            for i in 0..n {
                let o = 4 + i * 20;
                if o + 20 > sec.len() {
                    break;
                }
                v.push(PckEntry {
                    id: u32le(sec, o).unwrap(),
                    block_size: u32le(sec, o + 4).unwrap(),
                    size: u32le(sec, o + 8).unwrap(),
                    start_block: u32le(sec, o + 12).unwrap(),
                    language: u32le(sec, o + 16).unwrap(),
                });
            }
            v
        };
        Ok(Pck {
            version,
            banks: read_table(&rest[lang..lang + sb]),
            streams: read_table(&rest[lang + sb..]),
        })
    }

    /// Reads the bytes of one stream entry.
    pub fn read(&self, f: &mut File, e: &PckEntry) -> Result<Vec<u8>, String> {
        f.seek(SeekFrom::Start(e.byte_offset()))
            .map_err(|e| e.to_string())?;
        let mut v = vec![0u8; e.size as usize];
        f.read_exact(&mut v).map_err(|e| e.to_string())?;
        Ok(v)
    }
}

// ---------------------------------------------------------------------------------------
// WEM media
// ---------------------------------------------------------------------------------------

/// Codec families, derived from the RIFF `fmt` format tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    /// 0xFFFF: Wwise-modified Vorbis.
    WwiseVorbis,
    /// 0x3039 / 0x3040 / 0x3041: Wwise Opus variants.
    WwiseOpus(u16),
    /// 0xFFFE (extensible) or 0x0001: linear PCM.
    Pcm,
    /// 0x0002 / 0x0011 / 0x0069 / 0x8311 ...: ADPCM-like or unidentified.
    Other(u16),
}

impl Codec {
    pub fn from_tag(tag: u16) -> Codec {
        match tag {
            0xFFFF => Codec::WwiseVorbis,
            0x3039 | 0x3040 | 0x3041 => Codec::WwiseOpus(tag),
            0x0001 | 0xFFFE => Codec::Pcm,
            t => Codec::Other(t),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Codec::WwiseVorbis => "wwise_vorbis".into(),
            Codec::WwiseOpus(t) => format!("wwise_opus_{t:04x}"),
            Codec::Pcm => "pcm".into(),
            Codec::Other(t) => format!("tag_{t:04x}"),
        }
    }
}

/// A parsed WEM (RIFF/WAVE container).
#[derive(Debug, Clone)]
pub struct Wem {
    pub tag: u16,
    pub codec: Codec,
    pub channels: u16,
    pub sample_rate: u32,
    pub avg_bytes_per_sec: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    /// The whole `fmt ` chunk body.
    pub fmt: Range<usize>,
    pub data: Range<usize>,
    /// Chunk tags in file order (for the census).
    pub chunks: Vec<[u8; 4]>,
    /// Range of a `vorb` chunk, when present (older Wwise layout).
    pub vorb: Option<Range<usize>>,
}

impl Wem {
    pub fn parse(b: &[u8]) -> Result<Wem, String> {
        if b.len() < 12 || &b[..4] != b"RIFF" || &b[8..12] != b"WAVE" {
            return Err("not a RIFF/WAVE".into());
        }
        let mut p = 12usize;
        let (mut fmt, mut data, mut vorb) = (None, None, None);
        let mut chunks = Vec::new();
        while p + 8 <= b.len() {
            let tag: [u8; 4] = b[p..p + 4].try_into().unwrap();
            let size = u32le(b, p + 4).unwrap() as usize;
            let end = (p + 8 + size).min(b.len());
            chunks.push(tag);
            match &tag {
                b"fmt " => fmt = Some(p + 8..end),
                b"data" => data = Some(p + 8..end),
                b"vorb" => vorb = Some(p + 8..end),
                _ => {}
            }
            p = p + 8 + size + (size & 1);
        }
        let fmt = fmt.ok_or("no fmt chunk")?;
        let data = data.ok_or("no data chunk")?;
        if fmt.len() < 16 {
            return Err("fmt chunk too short".into());
        }
        let tag = u16le(b, fmt.start).unwrap();
        Ok(Wem {
            tag,
            codec: Codec::from_tag(tag),
            channels: u16le(b, fmt.start + 2).unwrap(),
            sample_rate: u32le(b, fmt.start + 4).unwrap(),
            avg_bytes_per_sec: u32le(b, fmt.start + 8).unwrap(),
            block_align: u16le(b, fmt.start + 12).unwrap(),
            bits_per_sample: u16le(b, fmt.start + 14).unwrap(),
            fmt,
            data,
            chunks,
            vorb,
        })
    }
}

// ---------------------------------------------------------------------------------------
// Vorbis codebook library (kept in the executable image)
// ---------------------------------------------------------------------------------------

/// The packed Vorbis codebooks referenced by id from Wwise Vorbis setup headers.
///
/// The runtime keeps them in the executable's read-only data as 599 consecutive packed
/// records reached through a table of relative relocations; this reads that table out of
/// the ELF image. The bytes are game data and must never be committed.
#[derive(Debug, Clone)]
pub struct CodebookLibrary {
    pub books: Vec<Vec<u8>>,
}

impl CodebookLibrary {
    /// Extracts the library from an ELF64 image of the game executable (`main.elf`).
    pub fn from_elf(elf: &[u8]) -> Result<CodebookLibrary, String> {
        if elf.len() < 64 || &elf[..4] != b"\x7fELF" {
            return Err("not an ELF image".into());
        }
        // PT_LOAD segments give vaddr -> file offset.
        let phoff = u64::from_le_bytes(elf[32..40].try_into().unwrap()) as usize;
        let phentsize = u16le(elf, 54).unwrap() as usize;
        let phnum = u16le(elf, 56).unwrap() as usize;
        let mut loads = Vec::new();
        for i in 0..phnum {
            let o = phoff + i * phentsize;
            if u32le(elf, o) == Some(1) {
                let off = u64::from_le_bytes(elf[o + 8..o + 16].try_into().unwrap());
                let va = u64::from_le_bytes(elf[o + 16..o + 24].try_into().unwrap());
                let fsz = u64::from_le_bytes(elf[o + 32..o + 40].try_into().unwrap());
                loads.push((va, off, fsz));
            }
        }
        let to_file = |va: u64| -> Option<usize> {
            loads
                .iter()
                .find(|(v, _, fsz)| va >= *v && va < v + fsz)
                .map(|(v, off, _)| (off + (va - v)) as usize)
        };
        // Scan 24-byte relocation records (offset, info = R_AARCH64_RELATIVE, addend) for the
        // run of ~599 consecutive records whose addends strictly increase by small steps.
        let mut best: Option<(usize, usize)> = None; // (first record file offset, count)
        let n8 = elf.len() / 8;
        let word = |i: usize| u64::from_le_bytes(elf[i * 8..i * 8 + 8].try_into().unwrap());
        let mut i = 1usize;
        while i + 3 < n8 {
            if word(i) == 0x403 {
                // start of a candidate run at record (i-1)
                let start = i - 1;
                let mut count = 1usize;
                let mut prev = word(i + 1);
                let mut j = i + 3;
                while j < n8 && word(j) == 0x403 {
                    let add = word(j + 1);
                    if add <= prev || add - prev > 40_000 {
                        break;
                    }
                    prev = add;
                    count += 1;
                    j += 3;
                }
                if (400..=800).contains(&count) && best.is_none_or(|(_, c)| count > c) {
                    best = Some((start * 8, count));
                }
                i = j.max(i + 1);
            } else {
                i += 1;
            }
        }
        let (first, count) = best.ok_or("codebook pointer table not found")?;
        let mut addrs = Vec::with_capacity(count);
        for k in 0..count {
            let o = first + k * 24 + 16;
            addrs.push(u64::from_le_bytes(elf[o..o + 8].try_into().unwrap()));
        }
        let mut books = Vec::with_capacity(count);
        for k in 0..count {
            let s = to_file(addrs[k]).ok_or("book address outside the image")?;
            let e = if k + 1 < count {
                to_file(addrs[k + 1]).ok_or("book address outside the image")?
            } else {
                // Last pointer: measured to reference a non-codebook table (a byte table that
                // does not follow the packed grammar); keep it only when it parses as a book.
                match vorbis::packed_book_len(&elf[s..]) {
                    Some(n) => s + n,
                    None => break,
                }
            };
            if e <= s || e > elf.len() {
                return Err(format!("codebook {k} has an invalid extent"));
            }
            books.push(elf[s..e].to_vec());
        }
        Ok(CodebookLibrary { books })
    }

    pub fn from_elf_file(path: &str) -> Result<CodebookLibrary, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        Self::from_elf(&bytes)
    }
}

/// Default location of the game's soundbanks inside `romfs_patched`.
pub const SOUND_BANK_DIR: &str = "bin/sound/NX64/wwise/bank";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_matches_wwise_known_ids() {
        // Empty string hashes to the FNV offset basis.
        assert_eq!(fnv1_lower(""), 2166136261);
        // Case-insensitive.
        assert_eq!(fnv1_lower("Play_BGM"), fnv1_lower("play_bgm"));
    }

    #[test]
    fn bank_rejects_garbage() {
        assert!(Bank::parse(b"nope").is_err());
        assert!(Wem::parse(b"RIFF").is_err());
    }

    #[test]
    fn bank_parses_synthetic() {
        let mut b = Vec::new();
        b.extend_from_slice(b"BKHD");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&135u32.to_le_bytes());
        b.extend_from_slice(&7u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(b"DIDX");
        b.extend_from_slice(&12u32.to_le_bytes());
        for v in [9u32, 0, 4] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(b"DATA");
        b.extend_from_slice(&4u32.to_le_bytes());
        b.extend_from_slice(b"abcd");
        let bank = Bank::parse(&b).unwrap();
        assert_eq!((bank.version, bank.bank_id), (135, 7));
        assert_eq!(bank.media(&b, 0).unwrap(), b"abcd");
    }
}
