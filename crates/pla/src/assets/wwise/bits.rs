//! Least-significant-bit-first bit reader / writer (the bit order of Vorbis and of the
//! Wwise packed Vorbis setup).

/// Reads bits LSB first. Reading past the end yields zeros and sets [`BitReader::overrun`].
pub struct BitReader<'a> {
    data: &'a [u8],
    pos: usize, // in bits
    pub overrun: bool,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        BitReader { data, pos: 0, overrun: false }
    }

    pub fn bit_pos(&self) -> usize {
        self.pos
    }

    pub fn bits_left(&self) -> usize {
        (self.data.len() * 8).saturating_sub(self.pos)
    }

    /// Reads `n` (0..=32) bits.
    pub fn read(&mut self, n: u32) -> u32 {
        debug_assert!(n <= 32);
        let mut v: u64 = 0;
        let mut got = 0u32;
        while got < n {
            let byte = self.pos / 8;
            if byte >= self.data.len() {
                self.overrun = true;
                self.pos += (n - got) as usize;
                break;
            }
            let off = (self.pos % 8) as u32;
            let take = (8 - off).min(n - got);
            let bits = ((self.data[byte] >> off) as u64) & ((1u64 << take) - 1);
            v |= bits << got;
            got += take;
            self.pos += take as usize;
        }
        v as u32
    }

    pub fn read_bit(&mut self) -> bool {
        self.read(1) != 0
    }
}

/// Writes bits LSB first.
#[derive(Default)]
pub struct BitWriter {
    pub bytes: Vec<u8>,
    nbits: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write(&mut self, value: u32, n: u32) {
        for i in 0..n {
            let bit = (value >> i) & 1;
            if self.nbits % 8 == 0 {
                self.bytes.push(0);
            }
            if bit != 0 {
                let last = self.bytes.len() - 1;
                self.bytes[last] |= 1 << (self.nbits % 8);
            }
            self.nbits += 1;
        }
    }

    /// Appends whole bytes at the current (possibly unaligned) bit position.
    pub fn write_bytes(&mut self, data: &[u8]) {
        let sh = (self.nbits % 8) as u32;
        if sh == 0 {
            self.bytes.extend_from_slice(data);
            self.nbits += data.len() * 8;
            return;
        }
        for &b in data {
            let last = self.bytes.len() - 1;
            self.bytes[last] |= b << sh;
            self.bytes.push(b >> (8 - sh));
        }
        self.nbits += data.len() * 8;
        // The pushed spill byte may be beyond the bit count when nothing spills; keep length exact.
        let need = self.nbits.div_ceil(8);
        self.bytes.truncate(need);
    }

    pub fn bit_len(&self) -> usize {
        self.nbits
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Number of bits needed to represent `v` (`ilog` of the Vorbis specification).
pub fn ilog(v: u32) -> u32 {
    32 - v.leading_zeros()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut w = BitWriter::new();
        w.write(5, 3);
        w.write(0x1234, 14);
        w.write(0xdead_beef, 32);
        w.write(1, 1);
        let b = w.into_bytes();
        let mut r = BitReader::new(&b);
        assert_eq!(r.read(3), 5);
        assert_eq!(r.read(14), 0x1234);
        assert_eq!(r.read(32), 0xdead_beef);
        assert_eq!(r.read(1), 1);
        assert!(!r.overrun);
        r.read(8);
        assert!(r.overrun);
    }

    #[test]
    fn ilog_values() {
        assert_eq!(ilog(0), 0);
        assert_eq!(ilog(1), 1);
        assert_eq!(ilog(7), 3);
        assert_eq!(ilog(8), 4);
    }
}
