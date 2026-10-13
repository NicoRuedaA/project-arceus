//! Wwise-modified Vorbis (`fmt` tag 0xFFFF) to standard Vorbis I.
//!
//! A Wwise Vorbis WEM stores the Vorbis packets without the Ogg framing and with three
//! space-saving changes that this module undoes:
//!
//! 1. The setup header has no codebooks of its own: each codebook is a 10-bit id into a
//!    library of packed codebooks ([`CodebookLibrary`]); several fixed fields (floor and
//!    mapping types, mode window/transform types, the time-domain placeholder) are dropped.
//! 2. Audio packets have a 2-byte size prefix (no granule position) and lose the leading
//!    packet-type bit and, for long blocks, the previous/next window flags.
//! 3. Ogg page granule positions are not stored; they follow from the block sizes.
//!
//! The result is a [`StdVorbis`]: the three standard headers plus standard audio packets,
//! ready to be decoded by [`super::vorbis_dec`] or wrapped into an Ogg stream by
//! [`to_ogg`].
//!
//! Layout facts used here were measured on this game's files (fmt chunk of 0x42 bytes whose
//! tail holds the sample count at +0x18, setup packet offset at +0x28, first audio packet
//! offset at +0x2c, block-size exponents at +0x40/+0x41); the packed codebook grammar and
//! the stripped-setup field order follow the public description of the format.

use super::bits::{ilog, BitReader, BitWriter};
use super::{CodebookLibrary, Wem};

/// Standard Vorbis headers plus audio packets.
#[derive(Debug, Clone)]
pub struct StdVorbis {
    pub channels: u8,
    pub sample_rate: u32,
    /// Declared total PCM frames per channel.
    pub sample_count: u32,
    /// Short and long block sizes in samples.
    pub blocksizes: [usize; 2],
    pub avg_bytes_per_sec: u32,
    pub ident: Vec<u8>,
    pub comment: Vec<u8>,
    pub setup: Vec<u8>,
    pub packets: Vec<Vec<u8>>,
    /// Block flag of each mode (true = long block).
    pub mode_blockflags: Vec<bool>,
}

/// Number of lookup values of a type-1 codebook (Vorbis `lookup1_values`).
pub fn quantvals(entries: u32, dims: u32) -> u32 {
    if dims == 0 {
        return 0;
    }
    let pow = |v: u64| -> u64 {
        let mut a = 1u64;
        for _ in 0..dims {
            a = a.saturating_mul(v);
        }
        a
    };
    let mut v = (entries as f64).powf(1.0 / dims as f64).floor() as u64;
    while pow(v + 1) <= entries as u64 {
        v += 1;
    }
    while v > 0 && pow(v) > entries as u64 {
        v -= 1;
    }
    v as u32
}

/// Length in bytes of one packed codebook, from its grammar. The library stores each book
/// as `bits / 8 + 1` bytes (a book ending exactly on a byte boundary gets one zero pad byte;
/// measured against the 598 books whose neighbour address gives the true length).
pub fn packed_book_len(b: &[u8]) -> Option<usize> {
    let mut w = BitWriter::new();
    let mut r = BitReader::new(&b[..b.len().min(8192)]);
    rebuild_book(&mut r, &mut w).ok()?;
    Some(r.bit_pos() / 8 + 1)
}

/// Rewrites one packed codebook into a standard Vorbis codebook.
pub fn rebuild_book(r: &mut BitReader, w: &mut BitWriter) -> Result<(), String> {
    let dims = r.read(4);
    let entries = r.read(14);
    if dims == 0 || entries == 0 {
        return Err("empty codebook".into());
    }
    w.write(0x564342, 24);
    w.write(dims, 16);
    w.write(entries, 24);
    let ordered = r.read(1);
    w.write(ordered, 1);
    if ordered != 0 {
        let initial = r.read(5);
        w.write(initial, 5);
        let mut cur = 0u32;
        while cur < entries {
            let nbits = ilog(entries - cur);
            let n = r.read(nbits);
            w.write(n, nbits);
            if n > entries - cur {
                return Err("ordered codebook overflows its entry count".into());
            }
            cur += n;
            if r.overrun {
                return Err("codebook truncated".into());
            }
        }
    } else {
        let cll = r.read(3);
        let sparse = r.read(1);
        if cll == 0 || cll > 5 {
            return Err("bad codeword length width".into());
        }
        w.write(sparse, 1);
        for _ in 0..entries {
            if sparse != 0 {
                let present = r.read(1);
                w.write(present, 1);
                if present == 0 {
                    continue;
                }
            }
            let l = r.read(cll);
            w.write(l, 5);
        }
    }
    let lookup = r.read(1);
    w.write(lookup, 4);
    if lookup == 1 {
        let min = r.read(32);
        let delta = r.read(32);
        let vbits = r.read(4);
        let seq = r.read(1);
        w.write(min, 32);
        w.write(delta, 32);
        w.write(vbits, 4);
        w.write(seq, 1);
        let q = quantvals(entries, dims);
        for _ in 0..q {
            let v = r.read(vbits + 1);
            w.write(v, vbits + 1);
        }
    }
    if r.overrun {
        return Err("codebook truncated".into());
    }
    Ok(())
}

/// Fields read from the `fmt` chunk's Wwise Vorbis tail.
#[derive(Debug, Clone, Copy)]
pub struct WwiseVorbisInfo {
    pub sample_count: u32,
    pub setup_packet_offset: u32,
    pub first_audio_packet_offset: u32,
    pub blocksize_pow: [u8; 2],
    pub uid: u32,
}

pub fn read_info(wem_bytes: &[u8], wem: &Wem) -> Result<WwiseVorbisInfo, String> {
    let fmt = &wem_bytes[wem.fmt.clone()];
    if fmt.len() != 0x42 {
        return Err(format!("unsupported Wwise Vorbis fmt size {:#x}", fmt.len()));
    }
    let u32at = |o: usize| u32::from_le_bytes(fmt[o..o + 4].try_into().unwrap());
    Ok(WwiseVorbisInfo {
        sample_count: u32at(0x18),
        setup_packet_offset: u32at(0x28),
        first_audio_packet_offset: u32at(0x2c),
        uid: u32at(0x3c),
        blocksize_pow: [fmt[0x40], fmt[0x41]],
    })
}

struct SetupInfo {
    setup: Vec<u8>,
    mode_blockflag: Vec<bool>,
}

fn convert_setup(raw: &[u8], channels: u32, lib: &CodebookLibrary) -> Result<SetupInfo, String> {
    let mut r = BitReader::new(raw);
    let mut w = BitWriter::new();
    // Packet type 5 + "vorbis".
    w.write(5, 8);
    for c in b"vorbis" {
        w.write(*c as u32, 8);
    }
    let ncb = r.read(8) + 1;
    w.write(ncb - 1, 8);
    for _ in 0..ncb {
        let id = r.read(10) as usize;
        let packed = lib.books.get(id).ok_or_else(|| format!("codebook id {id} outside the library"))?;
        let mut br = BitReader::new(packed);
        rebuild_book(&mut br, &mut w).map_err(|e| format!("codebook {id}: {e}"))?;
    }
    // Time-domain transform placeholder: count 1, value 0.
    w.write(0, 6);
    w.write(0, 16);
    // Floors.
    let nfloor = r.read(6) + 1;
    w.write(nfloor - 1, 6);
    for _ in 0..nfloor {
        w.write(1, 16);
        let parts = r.read(5);
        w.write(parts, 5);
        let mut classes = Vec::new();
        let mut maxc = 0u32;
        for _ in 0..parts {
            let c = r.read(4);
            w.write(c, 4);
            maxc = maxc.max(c);
            classes.push(c);
        }
        let mut cdims = vec![0u32; maxc as usize + 1];
        for d in cdims.iter_mut() {
            *d = r.read(3) + 1;
            w.write(*d - 1, 3);
            let sub = r.read(2);
            w.write(sub, 2);
            if sub != 0 {
                let mb = r.read(8);
                w.write(mb, 8);
                if mb >= ncb {
                    return Err("floor master book out of range".into());
                }
            }
            for _ in 0..(1u32 << sub) {
                let b = r.read(8);
                w.write(b, 8);
                if b != 0 && b - 1 >= ncb {
                    return Err("floor subclass book out of range".into());
                }
            }
        }
        let mult = r.read(2);
        w.write(mult, 2);
        let rangebits = r.read(4);
        w.write(rangebits, 4);
        for &c in &classes {
            for _ in 0..cdims[c as usize] {
                let x = r.read(rangebits);
                w.write(x, rangebits);
            }
        }
    }
    // Residues.
    let nres = r.read(6) + 1;
    w.write(nres - 1, 6);
    for _ in 0..nres {
        let rtype = r.read(2);
        if rtype > 2 {
            return Err("bad residue type".into());
        }
        w.write(rtype, 16);
        let begin = r.read(24);
        let end = r.read(24);
        let psize = r.read(24);
        let nclass = r.read(6) + 1;
        let classbook = r.read(8);
        if classbook >= ncb {
            return Err("residue classbook out of range".into());
        }
        w.write(begin, 24);
        w.write(end, 24);
        w.write(psize, 24);
        w.write(nclass - 1, 6);
        w.write(classbook, 8);
        let mut cascade = Vec::new();
        for _ in 0..nclass {
            let low = r.read(3);
            let flag = r.read(1);
            let high = if flag != 0 { r.read(5) } else { 0 };
            w.write(low, 3);
            w.write(flag, 1);
            if flag != 0 {
                w.write(high, 5);
            }
            cascade.push(high * 8 + low);
        }
        for c in cascade {
            for k in 0..8 {
                if c & (1 << k) != 0 {
                    let b = r.read(8);
                    if b >= ncb {
                        return Err("residue book out of range".into());
                    }
                    w.write(b, 8);
                }
            }
        }
    }
    // Mappings.
    let nmap = r.read(6) + 1;
    w.write(nmap - 1, 6);
    for _ in 0..nmap {
        w.write(0, 16);
        let sflag = r.read(1);
        w.write(sflag, 1);
        let mut submaps = 1;
        if sflag != 0 {
            let s = r.read(4);
            w.write(s, 4);
            submaps = s + 1;
        }
        let cflag = r.read(1);
        w.write(cflag, 1);
        if cflag != 0 {
            let steps = r.read(8) + 1;
            w.write(steps - 1, 8);
            let cb = ilog(channels.saturating_sub(1));
            for _ in 0..steps {
                let m = r.read(cb);
                let a = r.read(cb);
                w.write(m, cb);
                w.write(a, cb);
            }
        }
        let reserved = r.read(2);
        if reserved != 0 {
            return Err("mapping reserved bits are not zero".into());
        }
        w.write(0, 2);
        if submaps > 1 {
            for _ in 0..channels {
                let mux = r.read(4);
                w.write(mux, 4);
            }
        }
        for _ in 0..submaps {
            let _time_config = r.read(8); // stored but unused; the standard field is written as 0
            w.write(0, 8);
            let f = r.read(8);
            let res = r.read(8);
            if f >= nfloor || res >= nres {
                return Err("mapping floor/residue out of range".into());
            }
            w.write(f, 8);
            w.write(res, 8);
        }
    }
    // Modes.
    let nmodes = r.read(6) + 1;
    w.write(nmodes - 1, 6);
    let mut mode_blockflag = Vec::new();
    for _ in 0..nmodes {
        let bf = r.read(1);
        w.write(bf, 1);
        w.write(0, 16);
        w.write(0, 16);
        let m = r.read(8);
        if m >= nmap {
            return Err("mode mapping out of range".into());
        }
        w.write(m, 8);
        mode_blockflag.push(bf != 0);
    }
    w.write(1, 1); // framing
    if r.overrun {
        return Err("setup packet truncated".into());
    }
    let left = r.bits_left();
    if left >= 8 {
        return Err(format!("{left} unread bits left in the setup packet"));
    }
    Ok(SetupInfo { setup: w.into_bytes(), mode_blockflag })
}

fn identification(channels: u8, rate: u32, avg_bytes: u32, pow: [u8; 2]) -> Vec<u8> {
    let mut v = vec![1u8];
    v.extend_from_slice(b"vorbis");
    v.extend_from_slice(&0u32.to_le_bytes());
    v.push(channels);
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&0i32.to_le_bytes());
    v.extend_from_slice(&((avg_bytes as i64 * 8).min(i32::MAX as i64) as i32).to_le_bytes());
    v.extend_from_slice(&0i32.to_le_bytes());
    v.push((pow[0] & 15) | (pow[1] << 4));
    v.push(1);
    v
}

fn comment_header() -> Vec<u8> {
    let mut v = vec![3u8];
    v.extend_from_slice(b"vorbis");
    let vendor = b"pla-wwise";
    v.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    v.extend_from_slice(vendor);
    v.extend_from_slice(&0u32.to_le_bytes());
    v.push(1);
    v
}

/// Converts a Wwise Vorbis WEM (full RIFF bytes) into standard Vorbis headers and packets.
pub fn convert(wem_bytes: &[u8], lib: &CodebookLibrary) -> Result<StdVorbis, String> {
    let wem = Wem::parse(wem_bytes)?;
    if wem.tag != 0xFFFF {
        return Err(format!("not a Wwise Vorbis WEM (tag {:#06x})", wem.tag));
    }
    let info = read_info(wem_bytes, &wem)?;
    let channels = wem.channels as u32;
    if channels == 0 || channels > 8 {
        return Err("unsupported channel count".into());
    }
    let data = &wem_bytes[wem.data.clone()];
    let rd = |o: usize| -> Option<(usize, usize)> {
        let s = u16::from_le_bytes(data.get(o..o + 2)?.try_into().ok()?) as usize;
        data.get(o + 2..o + 2 + s)?;
        Some((o + 2, s))
    };
    let (so, ss) = rd(info.setup_packet_offset as usize).ok_or("setup packet outside the data chunk")?;
    let setup = convert_setup(&data[so..so + ss], channels, lib)?;
    let mode_count = setup.mode_blockflag.len() as u32;
    let mode_bits = ilog(mode_count - 1);
    // Wwise always spends at least one bit on the mode number, even with a single mode
    // (measured: streams with one mode carry a constant 0 bit there); the standard stream
    // spends none.
    let wire_mode_bits = mode_bits.max(1);
    // Collect raw audio packets.
    let mut raws: Vec<&[u8]> = Vec::new();
    let mut p = info.first_audio_packet_offset as usize;
    while p + 2 <= data.len() {
        let s = u16::from_le_bytes([data[p], data[p + 1]]) as usize;
        if p + 2 + s > data.len() {
            break;
        }
        raws.push(&data[p + 2..p + 2 + s]);
        p += 2 + s;
    }
    let mode_of = |pk: &[u8]| -> Option<usize> {
        let b = *pk.first()?;
        Some((b as u32 & ((1u32 << wire_mode_bits) - 1)) as usize)
    };
    let mut packets = Vec::with_capacity(raws.len());
    let mut prev_long = false;
    for (i, pk) in raws.iter().enumerate() {
        if pk.is_empty() {
            packets.push(vec![0]);
            continue;
        }
        let mode = mode_of(pk).unwrap();
        if mode >= setup.mode_blockflag.len() {
            return Err(format!("packet {i}: mode {mode} out of range"));
        }
        let long = setup.mode_blockflag[mode];
        let mut w = BitWriter::new();
        w.write(0, 1); // audio packet
        w.write(mode as u32, mode_bits);
        if long {
            let next_long = raws
                .get(i + 1)
                .and_then(|n| mode_of(n))
                .and_then(|m| setup.mode_blockflag.get(m).copied())
                .unwrap_or(false);
            w.write(prev_long as u32, 1);
            w.write(next_long as u32, 1);
        }
        // Remaining bits of the first byte, then the rest verbatim.
        w.write((pk[0] >> wire_mode_bits) as u32, 8 - wire_mode_bits);
        w.write_bytes(&pk[1..]);
        let out = w.into_bytes();
        packets.push(out);
        prev_long = long;
    }
    Ok(StdVorbis {
        channels: wem.channels as u8,
        sample_rate: wem.sample_rate,
        sample_count: info.sample_count,
        blocksizes: [1usize << info.blocksize_pow[0], 1usize << info.blocksize_pow[1]],
        avg_bytes_per_sec: wem.avg_bytes_per_sec,
        ident: identification(wem.channels as u8, wem.sample_rate, wem.avg_bytes_per_sec, info.blocksize_pow),
        comment: comment_header(),
        setup: setup.setup,
        packets,
        mode_blockflags: setup.mode_blockflag,
    })
}

// ---------------------------------------------------------------------------------------
// Ogg output
// ---------------------------------------------------------------------------------------

fn ogg_crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    for (i, e) in t.iter_mut().enumerate() {
        let mut r = (i as u32) << 24;
        for _ in 0..8 {
            r = if r & 0x8000_0000 != 0 { (r << 1) ^ 0x04c1_1db7 } else { r << 1 };
        }
        *e = r;
    }
    t
}

/// Writes Ogg pages into a byte vector.
pub struct OggWriter {
    out: Vec<u8>,
    serial: u32,
    seq: u32,
    table: [u32; 256],
}

impl OggWriter {
    pub fn new(serial: u32) -> Self {
        OggWriter { out: Vec::new(), serial, seq: 0, table: ogg_crc_table() }
    }

    /// Writes one page made of whole packets.
    pub fn page(&mut self, packets: &[&[u8]], granule: i64, bos: bool, eos: bool) {
        let mut lacing = Vec::new();
        for p in packets {
            let mut n = p.len();
            while n >= 255 {
                lacing.push(255u8);
                n -= 255;
            }
            lacing.push(n as u8);
        }
        debug_assert!(lacing.len() <= 255);
        let start = self.out.len();
        self.out.extend_from_slice(b"OggS");
        self.out.push(0);
        self.out.push((bos as u8) << 1 | (eos as u8) << 2);
        self.out.extend_from_slice(&granule.to_le_bytes());
        self.out.extend_from_slice(&self.serial.to_le_bytes());
        self.out.extend_from_slice(&self.seq.to_le_bytes());
        self.out.extend_from_slice(&[0, 0, 0, 0]);
        self.out.push(lacing.len() as u8);
        self.out.extend_from_slice(&lacing);
        for p in packets {
            self.out.extend_from_slice(p);
        }
        let mut crc = 0u32;
        for &b in &self.out[start..] {
            crc = (crc << 8) ^ self.table[((crc >> 24) as u8 ^ b) as usize];
        }
        self.out[start + 22..start + 26].copy_from_slice(&crc.to_le_bytes());
        self.seq += 1;
    }

    /// Packs packets into pages of bounded size. `granules[i]` is the granule position after
    /// packet `i`.
    pub fn packets(&mut self, packets: &[Vec<u8>], granules: &[i64], eos_last: bool) {
        let mut i = 0;
        while i < packets.len() {
            let mut j = i;
            let mut segs = 0usize;
            let mut bytes = 0usize;
            while j < packets.len() {
                let s = packets[j].len() / 255 + 1;
                if j > i && (segs + s > 250 || bytes > 6000) {
                    break;
                }
                segs += s;
                bytes += packets[j].len();
                j += 1;
            }
            let refs: Vec<&[u8]> = packets[i..j].iter().map(|v| v.as_slice()).collect();
            let last = j == packets.len();
            self.page(&refs, granules[j - 1], false, last && eos_last);
            i = j;
        }
    }

    pub fn finish(self) -> Vec<u8> {
        self.out
    }
}

/// Cumulative granule position after each audio packet (first packet 0), clamped to the
/// declared sample count on the last packet.
pub fn granules(v: &StdVorbis) -> Vec<i64> {
    let bits = ilog((v.mode_blockflags.len().max(1) - 1) as u32);
    let mut prev: Option<usize> = None;
    let mut total = 0i64;
    let mut out = Vec::with_capacity(v.packets.len());
    for p in &v.packets {
        let mode = (p.first().copied().unwrap_or(0) as u32 >> 1) & ((1u32 << bits) - 1);
        let size = v.blocksizes[v.mode_blockflags.get(mode as usize).copied().unwrap_or(false) as usize];
        if let Some(ps) = prev {
            total += (ps / 4 + size / 4) as i64;
        }
        prev = Some(size);
        out.push(total);
    }
    if v.sample_count > 0 {
        if let Some(l) = out.last_mut() {
            *l = (*l).min(v.sample_count as i64);
        }
    }
    out
}

/// Standard Ogg Vorbis stream for a converted WEM.
pub fn to_ogg(v: &StdVorbis) -> Vec<u8> {
    let mut ow = OggWriter::new(v.sample_count ^ 0x5741_5645);
    ow.page(&[&v.ident], 0, true, false);
    ow.page(&[&v.comment, &v.setup], 0, false, false);
    ow.packets(&v.packets, &granules(v), true);
    ow.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantvals_matches_spec() {
        assert_eq!(quantvals(9, 2), 3);
        assert_eq!(quantvals(10, 2), 3);
        assert_eq!(quantvals(16, 2), 4);
        assert_eq!(quantvals(64, 3), 4);
        assert_eq!(quantvals(63, 3), 3);
        assert_eq!(quantvals(5, 1), 5);
    }

    #[test]
    fn rebuild_sparse_book() {
        // dims 1, entries 2, unordered, cll 2, not sparse, lengths 1 and 1, no lookup.
        let mut w = BitWriter::new();
        w.write(1, 4);
        w.write(2, 14);
        w.write(0, 1);
        w.write(2, 3);
        w.write(0, 1);
        w.write(0, 2);
        w.write(0, 2);
        w.write(0, 1);
        let packed = w.into_bytes();
        let mut r = BitReader::new(&packed);
        let mut out = BitWriter::new();
        rebuild_book(&mut r, &mut out).unwrap();
        let b = out.into_bytes();
        assert_eq!(&b[..3], &[0x42, 0x43, 0x56]);
    }

    #[test]
    fn ogg_crc_known_vector() {
        // CRC of an empty page body is stable; just exercise the table.
        let t = ogg_crc_table();
        assert_eq!(t[1], 0x04c1_1db7);
    }
}
