//! 16-bit PCM WAV writing and simple signal statistics (used to verify decodes without
//! listening to them).

/// Writes interleaved 16-bit little-endian PCM as a canonical 44-byte-header WAV.
pub fn wav_bytes(sample_rate: u32, channels: u16, interleaved: &[i16]) -> Vec<u8> {
    let data_len = (interleaved.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + interleaved.len() * 2);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&channels.to_le_bytes());
    v.extend_from_slice(&sample_rate.to_le_bytes());
    v.extend_from_slice(&(sample_rate * channels as u32 * 2).to_le_bytes());
    v.extend_from_slice(&(channels * 2).to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in interleaved {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

/// Converts float channels (nominal range -1..1) to interleaved i16, counting clipped samples.
pub fn interleave_i16(channels: &[Vec<f32>]) -> (Vec<i16>, usize) {
    let frames = channels.iter().map(|c| c.len()).min().unwrap_or(0);
    let mut out = Vec::with_capacity(frames * channels.len());
    let mut clipped = 0usize;
    for i in 0..frames {
        for c in channels {
            let x = c[i] * 32768.0;
            if !(-32768.0..=32767.0).contains(&x) {
                clipped += 1;
            }
            out.push(x.clamp(-32768.0, 32767.0) as i16);
        }
    }
    (out, clipped)
}

/// Statistics of decoded audio.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SignalStats {
    pub frames: usize,
    pub peak: f32,
    pub rms: f32,
    /// Share of samples with magnitude at the clipping limit.
    pub clipped_ratio: f32,
    /// Count of NaN or infinite samples.
    pub non_finite: usize,
    /// Spectral centroid in Hz of a few analysed windows (0 when silent).
    pub centroid_hz: f32,
}

/// Peak / RMS / non-finite / centroid statistics over float channels.
pub fn stats(channels: &[Vec<f32>], sample_rate: u32) -> SignalStats {
    let frames = channels.iter().map(|c| c.len()).min().unwrap_or(0);
    let mut s = SignalStats { frames, ..Default::default() };
    let (mut acc, mut n, mut clip) = (0f64, 0usize, 0usize);
    for c in channels {
        for &x in c.iter().take(frames) {
            if !x.is_finite() {
                s.non_finite += 1;
                continue;
            }
            s.peak = s.peak.max(x.abs());
            acc += (x as f64) * (x as f64);
            n += 1;
            if x.abs() >= 0.9999 {
                clip += 1;
            }
        }
    }
    if n > 0 {
        s.rms = (acc / n as f64).sqrt() as f32;
        s.clipped_ratio = clip as f32 / n as f32;
    }
    s.centroid_hz = spectral_centroid(channels.first().map(|c| c.as_slice()).unwrap_or(&[]), sample_rate);
    s
}

/// Mean spectral centroid over up to 8 windows of 1024 samples (naive DFT on 128 bins is
/// avoided: a radix-2 FFT keeps this cheap).
pub fn spectral_centroid(x: &[f32], sample_rate: u32) -> f32 {
    const N: usize = 1024;
    if x.len() < N {
        return 0.0;
    }
    let windows = 8usize.min(x.len() / N);
    let stride = (x.len() - N) / windows.max(1);
    let (mut num, mut den) = (0f64, 0f64);
    for w in 0..windows {
        let start = w * stride.max(1);
        let mut re: Vec<f64> = (0..N)
            .map(|i| {
                let h = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / N as f64).cos();
                x[start + i] as f64 * h
            })
            .collect();
        let mut im = vec![0f64; N];
        fft64(&mut re, &mut im);
        for k in 1..N / 2 {
            let mag = (re[k] * re[k] + im[k] * im[k]).sqrt();
            num += mag * k as f64 * sample_rate as f64 / N as f64;
            den += mag;
        }
    }
    if den > 1e-9 {
        (num / den) as f32
    } else {
        0.0
    }
}

fn fft64(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let bits = n.trailing_zeros();
    for i in 0..n {
        let j = i.reverse_bits() >> (usize::BITS - bits);
        if j > i {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = ((ang * k as f64).cos(), (ang * k as f64).sin());
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * wr - im[b] * wi;
                let ti = re[b] * wi + im[b] * wr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_canonical() {
        let w = wav_bytes(48000, 2, &[0, 1, 2, 3]);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()), 36 + 8);
        assert_eq!(w.len(), 44 + 8);
    }

    #[test]
    fn centroid_of_sine_is_near_its_frequency() {
        let sr = 48000;
        let x: Vec<f32> = (0..8192).map(|i| (2.0 * std::f32::consts::PI * 3000.0 * i as f32 / sr as f32).sin()).collect();
        let c = spectral_centroid(&x, sr);
        assert!((c - 3000.0).abs() < 150.0, "centroid {c}");
    }

    #[test]
    fn stats_flag_nan_and_clipping() {
        let s = stats(&[vec![0.0, f32::NAN, 1.0, -1.0]], 48000);
        assert_eq!(s.non_finite, 1);
        assert!(s.clipped_ratio > 0.5);
    }
}
