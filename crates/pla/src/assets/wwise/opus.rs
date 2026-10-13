//! Wwise Opus (`fmt` tag 0x3039, also 0x3040 / 0x3041) to standard Opus packets / Ogg Opus.
//!
//! Layout measured on this game's 0x3039 files: the `fmt` chunk is 40 bytes; after the
//! 16-byte WAVEFORMAT head the tail holds `u16 cbSize` (6), `u16` samples per frame (960 at
//! 48 kHz), `u32` unknown flags, `u32` total sample count at the stream's own rate (+24),
//! `u32` unknown (+28), `u32` unknown (+32; it equals the packet area size in some files and
//! not in others, so it is ignored) and `u32` size of the seek table (+36). The `data` chunk
//! begins with that seek table (one `u32` per packet) and the packets run to the end of the
//! chunk; each packet is stored as `u32` big-endian size, `u32` big-endian Opus final range, and
//! the plain Opus packet. Packets are ordinary Opus (CELT 20 ms in the files measured).

use super::vorbis::OggWriter;
use super::Wem;

/// Standard Opus packets of one WEM.
#[derive(Debug, Clone)]
pub struct StdOpus {
    pub channels: u8,
    pub sample_rate: u32,
    /// Declared total PCM frames per channel (at 48 kHz).
    pub sample_count: u32,
    pub packets: Vec<Vec<u8>>,
    /// Number of seek-table entries (should equal the packet count).
    pub seek_entries: usize,
}

/// Samples (at 48 kHz) in one Opus packet, from its table-of-contents byte.
pub fn packet_samples(pk: &[u8]) -> Option<u32> {
    let toc = *pk.first()?;
    let config = toc >> 3;
    let per_frame: u32 = match config {
        0..=11 => [480, 960, 1920, 2880][(config & 3) as usize],
        12..=15 => [480, 960][(config & 1) as usize],
        _ => [120, 240, 480, 960][(config & 3) as usize],
    };
    let frames = match toc & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => (*pk.get(1)? & 0x3f) as u32,
    };
    Some(per_frame * frames)
}

pub fn convert(wem_bytes: &[u8]) -> Result<StdOpus, String> {
    let wem = Wem::parse(wem_bytes)?;
    if !matches!(wem.tag, 0x3039 | 0x3040 | 0x3041) {
        return Err(format!("not a Wwise Opus WEM (tag {:#06x})", wem.tag));
    }
    let fmt = &wem_bytes[wem.fmt.clone()];
    if fmt.len() != 40 {
        return Err(format!("unsupported Wwise Opus fmt size {}", fmt.len()));
    }
    let u32at = |o: usize| u32::from_le_bytes(fmt[o..o + 4].try_into().unwrap());
    let sample_count = u32at(24);
    let table = u32at(36) as usize;
    let data = &wem_bytes[wem.data.clone()];
    if table > data.len() || table % 4 != 0 {
        return Err("Opus seek table outside the data chunk".into());
    }
    let mut packets = Vec::new();
    let mut p = table;
    while p + 8 <= data.len() {
        let size = u32::from_be_bytes(data[p..p + 4].try_into().unwrap()) as usize;
        if size == 0 || p + 8 + size > data.len() {
            break;
        }
        packets.push(data[p + 8..p + 8 + size].to_vec());
        p += 8 + size;
    }
    if packets.is_empty() {
        return Err("no Opus packets".into());
    }
    Ok(StdOpus {
        channels: wem.channels as u8,
        sample_rate: wem.sample_rate,
        sample_count,
        packets,
        seek_entries: table / 4,
    })
}

/// Ogg Opus stream (pre-skip 0, channel mapping family 0: mono or stereo).
pub fn to_ogg(o: &StdOpus) -> Result<Vec<u8>, String> {
    if o.channels == 0 || o.channels > 2 {
        return Err("only mono and stereo Opus are wrapped".into());
    }
    let mut head = b"OpusHead".to_vec();
    head.push(1);
    head.push(o.channels);
    head.extend_from_slice(&0u16.to_le_bytes());
    head.extend_from_slice(&o.sample_rate.to_le_bytes());
    head.extend_from_slice(&0i16.to_le_bytes());
    head.push(0);
    let mut tags = b"OpusTags".to_vec();
    let vendor = b"pla-wwise";
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor);
    tags.extend_from_slice(&0u32.to_le_bytes());
    let mut ow = OggWriter::new(o.sample_count ^ 0x4f50_5553);
    ow.page(&[&head], 0, true, false);
    ow.page(&[&tags], 0, false, false);
    let mut total = 0i64;
    let mut granules = Vec::with_capacity(o.packets.len());
    for pk in &o.packets {
        total += packet_samples(pk).ok_or("bad Opus packet")? as i64;
        granules.push(total);
    }
    if o.sample_count > 0 {
        // The declared count is in samples at the stream's own rate; Ogg Opus granules use 48 kHz.
        let limit = o.sample_count as i64 * 48000 / o.sample_rate.max(1) as i64;
        if let Some(l) = granules.last_mut() {
            *l = (*l).min(limit);
        }
    }
    ow.packets(&o.packets, &granules, true);
    Ok(ow.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toc_durations() {
        // CELT FB 20 ms stereo, one frame.
        assert_eq!(packet_samples(&[0xfc, 0]), Some(960));
        // CELT 5 ms, code 1 (two frames).
        assert_eq!(packet_samples(&[0xe9]), Some(480));
        // SILK 60 ms.
        assert_eq!(packet_samples(&[0x18]), Some(2880));
        assert_eq!(packet_samples(&[]), None);
    }
}
