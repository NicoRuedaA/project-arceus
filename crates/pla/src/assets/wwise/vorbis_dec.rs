//! A small, dependency-free Vorbis I audio decoder (floor 1, residue 0/1/2, channel
//! coupling, windowed IMDCT overlap-add), written from the public Vorbis I specification.
//!
//! It consumes the three standard headers plus audio packets of a [`StdVorbis`]
//! (see [`super::vorbis`]) and returns 32-bit float PCM per channel.

use super::bits::{ilog, BitReader};
use super::floor1_db::FLOOR1_INVERSE_DB;
use super::vorbis::{quantvals, StdVorbis};
use std::collections::HashMap;
use std::f32::consts::PI;

#[derive(Debug)]
pub struct Codebook {
    pub dims: usize,
    pub entries: usize,
    trie: Vec<[i32; 2]>,
    vq: Vec<f32>,
}

fn float32_unpack(x: u32) -> f32 {
    let mut mant = (x & 0x1f_ffff) as f64;
    let sign = x & 0x8000_0000;
    let exp = ((x & 0x7fe0_0000) >> 21) as i32;
    if sign != 0 {
        mant = -mant;
    }
    (mant * 2f64.powi(exp - 788)) as f32
}

impl Codebook {
    fn parse(r: &mut BitReader) -> Result<Codebook, String> {
        if r.read(24) != 0x564342 {
            return Err("codebook sync pattern missing".into());
        }
        let dims = r.read(16) as usize;
        let entries = r.read(24) as usize;
        if dims == 0 || entries == 0 {
            return Err("empty codebook".into());
        }
        let mut lengths = vec![0u8; entries];
        if r.read(1) != 0 {
            let mut cur_len = r.read(5) + 1;
            let mut i = 0usize;
            while i < entries {
                let num = r.read(ilog((entries - i) as u32)) as usize;
                if i + num > entries || cur_len > 32 {
                    return Err("bad ordered codebook".into());
                }
                for l in lengths.iter_mut().skip(i).take(num) {
                    *l = cur_len as u8;
                }
                i += num;
                cur_len += 1;
                if r.overrun {
                    return Err("codebook truncated".into());
                }
            }
        } else {
            let sparse = r.read(1) != 0;
            for l in lengths.iter_mut() {
                if sparse && r.read(1) == 0 {
                    *l = 0;
                } else {
                    *l = (r.read(5) + 1) as u8;
                }
            }
        }
        // Huffman trie from lengths (codeword assignment per the specification).
        let mut trie: Vec<[i32; 2]> = vec![[0, 0]];
        let mut available = [0u32; 33];
        let mut first = true;
        for (entry, &len) in lengths.iter().enumerate() {
            if len == 0 {
                continue;
            }
            let len = len as usize;
            let code: u32;
            if first {
                first = false;
                code = 0;
                for (i, a) in available.iter_mut().enumerate().take(len + 1).skip(1) {
                    *a = 1u32 << (32 - i);
                }
            } else {
                let mut y = len;
                while y > 0 && available[y] == 0 {
                    y -= 1;
                }
                if y == 0 {
                    return Err("oversubscribed codebook".into());
                }
                let res = available[y];
                available[y] = 0;
                code = res;
                for yy in (y + 1..=len).rev() {
                    available[yy] = res + (1u32 << (32 - yy));
                }
            }
            // Walk / build the trie MSB-first.
            let mut node = 0usize;
            for bit_i in 0..len {
                let bit = ((code >> (31 - bit_i)) & 1) as usize;
                if bit_i + 1 == len {
                    if trie[node][bit] != 0 {
                        return Err("codeword collision".into());
                    }
                    trie[node][bit] = -(entry as i32) - 1;
                } else {
                    let nxt = trie[node][bit];
                    if nxt < 0 {
                        return Err("codeword prefix collision".into());
                    }
                    if nxt == 0 {
                        trie.push([0, 0]);
                        let idx = trie.len() - 1;
                        trie[node][bit] = idx as i32;
                        node = idx;
                    } else {
                        node = nxt as usize;
                    }
                }
            }
        }
        let lookup = r.read(4);
        let mut vq = Vec::new();
        if lookup == 1 || lookup == 2 {
            let min = float32_unpack(r.read(32));
            let delta = float32_unpack(r.read(32));
            let vbits = r.read(4) + 1;
            let seq = r.read(1) != 0;
            let nvals = if lookup == 1 {
                quantvals(entries as u32, dims as u32) as usize
            } else {
                entries * dims
            };
            let mults: Vec<u32> = (0..nvals).map(|_| r.read(vbits)).collect();
            if r.overrun {
                return Err("codebook truncated".into());
            }
            vq = vec![0f32; entries * dims];
            for e in 0..entries {
                let mut last = 0f32;
                let mut div = 1usize;
                for d in 0..dims {
                    let idx = if lookup == 1 { (e / div) % nvals } else { e * dims + d };
                    let v = mults[idx] as f32 * delta + min + last;
                    vq[e * dims + d] = v;
                    if seq {
                        last = v;
                    }
                    if lookup == 1 {
                        div *= nvals;
                    }
                }
            }
        } else if lookup != 0 {
            return Err("bad codebook lookup type".into());
        }
        Ok(Codebook { dims, entries, trie, vq })
    }

    /// Decodes one scalar entry; `None` at end of data or on an undefined codeword.
    fn scalar(&self, r: &mut BitReader) -> Option<usize> {
        let mut node = 0usize;
        loop {
            let bit = r.read(1) as usize;
            if r.overrun {
                return None;
            }
            let n = self.trie[node][bit];
            if n < 0 {
                return Some((-n - 1) as usize);
            }
            if n == 0 {
                return None;
            }
            node = n as usize;
        }
    }

    fn has_vq(&self) -> bool {
        !self.vq.is_empty()
    }
}

#[derive(Debug)]
struct Floor1 {
    partition_class: Vec<usize>,
    class_dims: Vec<usize>,
    class_subs: Vec<u32>,
    class_master: Vec<usize>,
    class_books: Vec<Vec<i32>>,
    multiplier: i32,
    rangebits: u32,
    xlist: Vec<i32>,
    sorted: Vec<usize>,
    lo_nb: Vec<usize>,
    hi_nb: Vec<usize>,
}

#[derive(Debug)]
struct Residue {
    rtype: u32,
    begin: usize,
    end: usize,
    psize: usize,
    classbook: usize,
    nclass: usize,
    books: Vec<[i32; 8]>,
}

#[derive(Debug)]
struct Mapping {
    coupling: Vec<(usize, usize)>,
    mux: Vec<usize>,
    submaps: Vec<(usize, usize)>, // (floor, residue)
}

#[derive(Debug)]
pub struct Mode {
    pub blockflag: bool,
    mapping: usize,
}

#[derive(Debug)]
pub struct Setup {
    codebooks: Vec<Codebook>,
    floors: Vec<Floor1>,
    residues: Vec<Residue>,
    mappings: Vec<Mapping>,
    pub modes: Vec<Mode>,
}

impl Setup {
    pub fn parse(bytes: &[u8], channels: usize) -> Result<Setup, String> {
        if bytes.len() < 7 || bytes[0] != 5 || &bytes[1..7] != b"vorbis" {
            return Err("not a Vorbis setup header".into());
        }
        let mut r = BitReader::new(&bytes[7..]);
        let ncb = r.read(8) as usize + 1;
        let mut codebooks = Vec::with_capacity(ncb);
        for i in 0..ncb {
            codebooks.push(Codebook::parse(&mut r).map_err(|e| format!("codebook {i}: {e}"))?);
        }
        let ntime = r.read(6) + 1;
        for _ in 0..ntime {
            if r.read(16) != 0 {
                return Err("unsupported time domain transform".into());
            }
        }
        let nfloor = r.read(6) as usize + 1;
        let mut floors = Vec::new();
        for _ in 0..nfloor {
            if r.read(16) != 1 {
                return Err("only floor type 1 is supported".into());
            }
            let parts = r.read(5) as usize;
            let mut partition_class = Vec::new();
            let mut maxc = 0usize;
            for _ in 0..parts {
                let c = r.read(4) as usize;
                maxc = maxc.max(c);
                partition_class.push(c);
            }
            let mut class_dims = vec![0usize; maxc + 1];
            let mut class_subs = vec![0u32; maxc + 1];
            let mut class_master = vec![0usize; maxc + 1];
            let mut class_books = vec![Vec::new(); maxc + 1];
            for c in 0..=maxc {
                class_dims[c] = r.read(3) as usize + 1;
                class_subs[c] = r.read(2);
                if class_subs[c] != 0 {
                    class_master[c] = r.read(8) as usize;
                    if class_master[c] >= ncb {
                        return Err("floor master book out of range".into());
                    }
                }
                for _ in 0..(1u32 << class_subs[c]) {
                    let b = r.read(8) as i32 - 1;
                    if b >= ncb as i32 {
                        return Err("floor subclass book out of range".into());
                    }
                    class_books[c].push(b);
                }
            }
            let multiplier = r.read(2) as i32 + 1;
            let rangebits = r.read(4);
            let mut xlist = vec![0, 1 << rangebits];
            for &c in &partition_class {
                for _ in 0..class_dims[c] {
                    xlist.push(r.read(rangebits) as i32);
                }
            }
            let n = xlist.len();
            let mut sorted: Vec<usize> = (0..n).collect();
            sorted.sort_by_key(|&i| xlist[i]);
            let mut lo_nb = vec![0usize; n];
            let mut hi_nb = vec![0usize; n];
            for i in 2..n {
                let (mut lo, mut hi) = (0usize, 1usize);
                let (mut lov, mut hiv) = (i32::MIN, i32::MAX);
                for j in 0..i {
                    if xlist[j] < xlist[i] && xlist[j] > lov {
                        lov = xlist[j];
                        lo = j;
                    }
                    if xlist[j] > xlist[i] && xlist[j] < hiv {
                        hiv = xlist[j];
                        hi = j;
                    }
                }
                lo_nb[i] = lo;
                hi_nb[i] = hi;
            }
            floors.push(Floor1 {
                partition_class,
                class_dims,
                class_subs,
                class_master,
                class_books,
                multiplier,
                rangebits,
                xlist,
                sorted,
                lo_nb,
                hi_nb,
            });
        }
        let nres = r.read(6) as usize + 1;
        let mut residues = Vec::new();
        for _ in 0..nres {
            let rtype = r.read(16);
            if rtype > 2 {
                return Err("bad residue type".into());
            }
            let begin = r.read(24) as usize;
            let end = r.read(24) as usize;
            let psize = r.read(24) as usize + 1;
            let nclass = r.read(6) as usize + 1;
            let classbook = r.read(8) as usize;
            if classbook >= ncb {
                return Err("residue classbook out of range".into());
            }
            let mut cascade = Vec::new();
            for _ in 0..nclass {
                let low = r.read(3);
                let high = if r.read(1) != 0 { r.read(5) } else { 0 };
                cascade.push(high * 8 + low);
            }
            let mut books = Vec::new();
            for &c in &cascade {
                let mut b = [-1i32; 8];
                for (k, slot) in b.iter_mut().enumerate() {
                    if c & (1 << k) != 0 {
                        let v = r.read(8) as usize;
                        if v >= ncb {
                            return Err("residue book out of range".into());
                        }
                        *slot = v as i32;
                    }
                }
                books.push(b);
            }
            residues.push(Residue { rtype, begin, end, psize, classbook, nclass, books });
        }
        let nmap = r.read(6) as usize + 1;
        let mut mappings = Vec::new();
        for _ in 0..nmap {
            if r.read(16) != 0 {
                return Err("bad mapping type".into());
            }
            let nsub = if r.read(1) != 0 { r.read(4) as usize + 1 } else { 1 };
            let mut coupling = Vec::new();
            if r.read(1) != 0 {
                let steps = r.read(8) as usize + 1;
                let cb = ilog(channels as u32 - 1);
                for _ in 0..steps {
                    let m = r.read(cb) as usize;
                    let a = r.read(cb) as usize;
                    if m >= channels || a >= channels || m == a {
                        return Err("bad coupling channel".into());
                    }
                    coupling.push((m, a));
                }
            }
            if r.read(2) != 0 {
                return Err("mapping reserved bits set".into());
            }
            let mut mux = vec![0usize; channels];
            if nsub > 1 {
                for m in mux.iter_mut() {
                    *m = r.read(4) as usize;
                    if *m >= nsub {
                        return Err("bad submap mux".into());
                    }
                }
            }
            let mut submaps = Vec::new();
            for _ in 0..nsub {
                r.read(8);
                let f = r.read(8) as usize;
                let rs = r.read(8) as usize;
                if f >= floors.len() || rs >= residues.len() {
                    return Err("submap floor/residue out of range".into());
                }
                submaps.push((f, rs));
            }
            mappings.push(Mapping { coupling, mux, submaps });
        }
        let nmodes = r.read(6) as usize + 1;
        let mut modes = Vec::new();
        for _ in 0..nmodes {
            let blockflag = r.read(1) != 0;
            r.read(16);
            r.read(16);
            let mapping = r.read(8) as usize;
            if mapping >= mappings.len() {
                return Err("mode mapping out of range".into());
            }
            modes.push(Mode { blockflag, mapping });
        }
        if r.read(1) == 0 || r.overrun {
            return Err("setup framing bit missing".into());
        }
        Ok(Setup { codebooks, floors, residues, mappings, modes })
    }
}

/// Decoded PCM, one vector per channel.
#[derive(Debug, Clone)]
pub struct Pcm {
    pub sample_rate: u32,
    pub channels: Vec<Vec<f32>>,
}

impl Pcm {
    pub fn frames(&self) -> usize {
        self.channels.first().map_or(0, |c| c.len())
    }
}

struct Fft {
    n: usize,
    cos: Vec<f32>,
    sin: Vec<f32>,
    rev: Vec<usize>,
}

impl Fft {
    fn new(n: usize) -> Fft {
        let bits = n.trailing_zeros();
        let rev = (0..n).map(|i| i.reverse_bits() >> (usize::BITS - bits)).collect();
        let cos = (0..n / 2).map(|k| (2.0 * std::f64::consts::PI * k as f64 / n as f64).cos() as f32).collect();
        let sin = (0..n / 2).map(|k| (2.0 * std::f64::consts::PI * k as f64 / n as f64).sin() as f32).collect();
        Fft { n, cos, sin, rev }
    }

    /// In-place unnormalised transform with kernel e^{+i 2 pi k m / n}.
    fn run(&self, re: &mut [f32], im: &mut [f32]) {
        let n = self.n;
        for i in 0..n {
            let j = self.rev[i];
            if j > i {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= n {
            let half = len / 2;
            let step = n / len;
            for start in (0..n).step_by(len) {
                for k in 0..half {
                    let (wr, wi) = (self.cos[k * step], self.sin[k * step]);
                    let (a, b) = (start + k, start + k + half);
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
}

/// Inverse MDCT of `spec` (n/2 coefficients) to `n` samples, per the Vorbis I definition
/// y[k] = sum_m X[m] cos(2 pi / n (k + 1/2 + n/4)(m + 1/2)).
fn imdct(fft: &Fft, spec: &[f32], out: &mut [f32], re: &mut [f32], im: &mut [f32]) {
    let n = fft.n;
    let theta = 2.0 * std::f64::consts::PI / n as f64;
    let n0 = 0.5 + n as f64 / 4.0;
    for m in 0..n / 2 {
        let ph = theta * n0 * m as f64;
        re[m] = spec[m] * ph.cos() as f32;
        im[m] = spec[m] * ph.sin() as f32;
    }
    for m in n / 2..n {
        re[m] = 0.0;
        im[m] = 0.0;
    }
    fft.run(re, im);
    for k in 0..n {
        let ph = theta * (k as f64 + n0) * 0.5;
        out[k] = re[k] * ph.cos() as f32 - im[k] * ph.sin() as f32;
    }
}

fn slope(len: usize) -> Vec<f32> {
    (0..len)
        .map(|i| {
            let x = (i as f32 + 0.5) / len as f32 * (PI / 2.0);
            ((PI / 2.0) * x.sin() * x.sin()).sin()
        })
        .collect()
}

pub struct Decoder {
    setup: Setup,
    channels: usize,
    blocksizes: [usize; 2],
    fft: HashMap<usize, Fft>,
    slopes: HashMap<usize, Vec<f32>>,
    prev: Option<(usize, Vec<Vec<f32>>)>, // (block size, windowed output per channel)
}

impl Decoder {
    pub fn new(v: &StdVorbis) -> Result<Decoder, String> {
        let setup = Setup::parse(&v.setup, v.channels as usize)?;
        let mut fft = HashMap::new();
        let mut slopes = HashMap::new();
        for bs in v.blocksizes {
            fft.insert(bs, Fft::new(bs));
            slopes.insert(bs / 2, slope(bs / 2));
        }
        Ok(Decoder { setup, channels: v.channels as usize, blocksizes: v.blocksizes, fft, slopes, prev: None })
    }

    fn floor_decode(&self, f: &Floor1, r: &mut BitReader) -> Option<Vec<i32>> {
        if r.read(1) == 0 {
            return None;
        }
        let range: i32 = [256, 128, 86, 64][(f.multiplier - 1) as usize];
        let yb = ilog((range - 1) as u32);
        let mut y: Vec<i32> = vec![r.read(yb) as i32, r.read(yb) as i32];
        for &c in &f.partition_class {
            let cdim = f.class_dims[c];
            let cbits = f.class_subs[c];
            let csub = (1u32 << cbits) - 1;
            let mut cval = 0u32;
            if cbits > 0 {
                cval = self.setup.codebooks[f.class_master[c]].scalar(r)? as u32;
            }
            for _ in 0..cdim {
                let book = f.class_books[c][(cval & csub) as usize];
                cval >>= cbits;
                if book >= 0 {
                    y.push(self.setup.codebooks[book as usize].scalar(r)? as i32);
                } else {
                    y.push(0);
                }
            }
        }
        if r.overrun {
            return None;
        }
        // Amplitude value synthesis (step 2).
        let n = y.len();
        let mut flag = vec![false; n];
        let mut fy = vec![0i32; n];
        flag[0] = true;
        flag[1] = true;
        fy[0] = y[0];
        fy[1] = y[1];
        for i in 2..n {
            let (lo, hi) = (f.lo_nb[i], f.hi_nb[i]);
            let pred = render_point(f.xlist[lo], fy[lo], f.xlist[hi], fy[hi], f.xlist[i]);
            let val = y[i];
            let highroom = range - pred;
            let lowroom = pred;
            let room = if highroom < lowroom { highroom * 2 } else { lowroom * 2 };
            if val != 0 {
                flag[lo] = true;
                flag[hi] = true;
                flag[i] = true;
                fy[i] = if val >= room {
                    if highroom > lowroom {
                        val - lowroom + pred
                    } else {
                        pred - val + highroom - 1
                    }
                } else if val % 2 == 1 {
                    pred - (val + 1) / 2
                } else {
                    pred + val / 2
                };
            } else {
                flag[i] = false;
                fy[i] = pred;
            }
        }
        // Mark unused points with -1 so the renderer skips them.
        for i in 0..n {
            if !flag[i] {
                fy[i] = -1 - fy[i];
            }
        }
        Some(fy)
    }

    fn floor_curve(&self, f: &Floor1, fy: &[i32], half: usize) -> Vec<f32> {
        let mut floor = vec![0i32; half];
        let get = |i: usize| -> (i32, bool) {
            if fy[i] < 0 {
                (-1 - fy[i], false)
            } else {
                (fy[i], true)
            }
        };
        let mut lx = 0i32;
        let mut ly = get(f.sorted[0]).0 * f.multiplier;
        let mut hx = 0i32;
        let mut hy = 0i32;
        for &idx in f.sorted.iter().skip(1) {
            let (v, used) = get(idx);
            if used {
                hy = v * f.multiplier;
                hx = f.xlist[idx];
                render_line(lx, ly, hx, hy, &mut floor);
                lx = hx;
                ly = hy;
            }
        }
        let _ = hy;
        if (hx as usize) < half {
            for slot in floor.iter_mut().take(half).skip(hx as usize) {
                *slot = ly;
            }
        }
        floor.iter().map(|&y| FLOOR1_INVERSE_DB[y.clamp(0, 255) as usize]).collect()
    }

    fn residue_decode(
        &self,
        res: &Residue,
        r: &mut BitReader,
        vecs: &mut [Vec<f32>],
        skip: &[bool],
        half: usize,
    ) {
        let nch = vecs.len();
        let ch_total = if res.rtype == 2 { nch } else { 1 };
        // Type 2 works on one interleaved vector.
        let actual = if res.rtype == 2 { half * nch } else { half };
        if res.rtype == 2 {
            if skip.iter().all(|&s| s) {
                return;
            }
        }
        let mut inter: Vec<f32> = if res.rtype == 2 { vec![0.0; actual] } else { Vec::new() };
        let begin = res.begin.min(actual);
        let end = res.end.min(actual);
        if end <= begin {
            return;
        }
        let cb = &self.setup.codebooks[res.classbook];
        let cw = cb.dims;
        let n_to_read = end - begin;
        let parts = n_to_read / res.psize;
        if parts == 0 {
            return;
        }
        let nvec = if res.rtype == 2 { 1 } else { nch };
        let mut classes = vec![vec![0usize; parts + cw]; nvec];
        let _ = ch_total;
        'passes: for pass in 0..8 {
            let mut pc = 0usize;
            while pc < parts {
                if pass == 0 {
                    for (j, cl) in classes.iter_mut().enumerate().take(nvec) {
                        if res.rtype != 2 && skip[j] {
                            continue;
                        }
                        let Some(mut temp) = cb.scalar(r) else { break 'passes };
                        for i in (0..cw).rev() {
                            if pc + i < cl.len() {
                                cl[pc + i] = temp % res.nclass;
                            }
                            temp /= res.nclass;
                        }
                    }
                }
                for _ in 0..cw {
                    if pc >= parts {
                        break;
                    }
                    for j in 0..nvec {
                        if res.rtype != 2 && skip[j] {
                            continue;
                        }
                        let cls = classes[j][pc];
                        let book = res.books[cls][pass];
                        if book >= 0 {
                            let bk = &self.setup.codebooks[book as usize];
                            if !bk.has_vq() {
                                break 'passes;
                            }
                            let off = begin + pc * res.psize;
                            let dst: &mut [f32] = if res.rtype == 2 { &mut inter } else { &mut vecs[j] };
                            let dims = bk.dims;
                            if res.rtype == 0 {
                                let step = res.psize / dims;
                                for i in 0..step {
                                    let Some(e) = bk.scalar(r) else { break 'passes };
                                    for d in 0..dims {
                                        let idx = off + i + d * step;
                                        if idx < dst.len() {
                                            dst[idx] += bk.vq[e * dims + d];
                                        }
                                    }
                                }
                            } else {
                                let mut i = 0;
                                while i < res.psize {
                                    let Some(e) = bk.scalar(r) else { break 'passes };
                                    for d in 0..dims {
                                        let idx = off + i + d;
                                        if idx < dst.len() && i + d < res.psize {
                                            dst[idx] += bk.vq[e * dims + d];
                                        }
                                    }
                                    i += dims;
                                }
                            }
                        }
                    }
                    pc += 1;
                }
            }
        }
        if res.rtype == 2 {
            for (c, v) in vecs.iter_mut().enumerate() {
                for i in 0..half {
                    v[i] = inter[i * nch + c];
                }
            }
        }
    }

    /// Decodes one audio packet. Returns the finished PCM samples (none for the very first
    /// packet).
    pub fn decode_packet(&mut self, pk: &[u8]) -> Result<Option<Vec<Vec<f32>>>, String> {
        let mut r = BitReader::new(pk);
        if r.read(1) != 0 {
            return Err("non-audio packet".into());
        }
        let mode_bits = ilog(self.setup.modes.len() as u32 - 1);
        let mode_i = r.read(mode_bits) as usize;
        let mode = self.setup.modes.get(mode_i).ok_or("bad mode number")?;
        let long = mode.blockflag;
        let n = self.blocksizes[long as usize];
        let (prev_long, next_long) = if long { (r.read(1) != 0, r.read(1) != 0) } else { (false, false) };
        let half = n / 2;
        let mapping = &self.setup.mappings[mode.mapping];
        let nch = self.channels;
        // Floors.
        let mut curves: Vec<Option<Vec<f32>>> = Vec::with_capacity(nch);
        for ch in 0..nch {
            let sm = mapping.submaps[mapping.mux[ch]];
            let f = &self.setup.floors[sm.0];
            let c = self.floor_decode(f, &mut r).map(|fy| self.floor_curve(f, &fy, half));
            curves.push(c);
        }
        let mut no_res: Vec<bool> = curves.iter().map(|c| c.is_none()).collect();
        for &(m, a) in &mapping.coupling {
            if !no_res[m] || !no_res[a] {
                no_res[m] = false;
                no_res[a] = false;
            }
        }
        let mut spec: Vec<Vec<f32>> = vec![vec![0.0; half]; nch];
        for (si, &(_, rs)) in mapping.submaps.iter().enumerate() {
            let chans: Vec<usize> = (0..nch).filter(|&c| mapping.mux[c] == si).collect();
            if chans.is_empty() {
                continue;
            }
            let skip: Vec<bool> = chans.iter().map(|&c| no_res[c]).collect();
            let mut vecs: Vec<Vec<f32>> = vec![vec![0.0; half]; chans.len()];
            self.residue_decode(&self.setup.residues[rs], &mut r, &mut vecs, &skip, half);
            for (k, &c) in chans.iter().enumerate() {
                spec[c] = std::mem::take(&mut vecs[k]);
            }
        }
        for &(m, a) in mapping.coupling.iter().rev() {
            for i in 0..half {
                let (mv, av) = (spec[m][i], spec[a][i]);
                let (nm, na) = if mv > 0.0 {
                    if av > 0.0 { (mv, mv - av) } else { (mv + av, mv) }
                } else if av > 0.0 {
                    (mv, mv + av)
                } else {
                    (mv - av, mv)
                };
                spec[m][i] = nm;
                spec[a][i] = na;
            }
        }
        // Window shape.
        let bs0 = self.blocksizes[0];
        let (lstart, lend) = if long && !prev_long { (n / 4 - bs0 / 4, n / 4 + bs0 / 4) } else { (0, n / 2) };
        let (rstart, rend) = if long && !next_long { (n * 3 / 4 - bs0 / 4, n * 3 / 4 + bs0 / 4) } else { (n / 2, n) };
        let lslope = &self.slopes[&(lend - lstart)];
        let rslope = &self.slopes[&(rend - rstart)];
        let mut window = vec![0f32; n];
        for i in lstart..lend {
            window[i] = lslope[i - lstart];
        }
        for w in window.iter_mut().take(rstart).skip(lend) {
            *w = 1.0;
        }
        for i in rstart..rend {
            window[i] = rslope[rend - 1 - i];
        }
        // IMDCT per channel.
        let fft = &self.fft[&n];
        let mut out: Vec<Vec<f32>> = Vec::with_capacity(nch);
        let (mut re, mut im) = (vec![0f32; n], vec![0f32; n]);
        for ch in 0..nch {
            let mut y = vec![0f32; n];
            if !no_res[ch] || curves[ch].is_some() {
                if let Some(curve) = &curves[ch] {
                    for i in 0..half {
                        spec[ch][i] *= curve[i];
                    }
                    imdct(fft, &spec[ch], &mut y, &mut re, &mut im);
                    for i in 0..n {
                        y[i] *= window[i];
                    }
                }
            }
            out.push(y);
        }
        // Overlap-add against the previous block.
        let result = if let Some((pn, pwin)) = &self.prev {
            let len = pn / 4 + n / 4;
            let mut pcm = vec![vec![0f32; len]; nch];
            for ch in 0..nch {
                for j in 0..len {
                    let p = pn / 2 + j;
                    let c = (n / 2 + j) as isize - len as isize;
                    let mut s = 0.0;
                    if p < *pn {
                        s += pwin[ch][p];
                    }
                    if c >= 0 && (c as usize) < n {
                        s += out[ch][c as usize];
                    }
                    pcm[ch][j] = s;
                }
            }
            Some(pcm)
        } else {
            None
        };
        self.prev = Some((n, out));
        Ok(result)
    }
}

fn render_point(x0: i32, y0: i32, x1: i32, y1: i32, x: i32) -> i32 {
    let dy = y1 - y0;
    let adx = x1 - x0;
    let ady = dy.abs();
    let err = ady * (x - x0);
    let off = err / adx.max(1);
    if dy < 0 {
        y0 - off
    } else {
        y0 + off
    }
}

fn render_line(x0: i32, y0: i32, x1: i32, y1: i32, floor: &mut [i32]) {
    let dy = y1 - y0;
    let adx = x1 - x0;
    if adx <= 0 {
        return;
    }
    let ady0 = dy.abs();
    let base = dy / adx;
    let mut y = y0;
    let sy = if dy < 0 { base - 1 } else { base + 1 };
    let ady = ady0 - base.abs() * adx;
    let mut err = 0;
    let mut x = x0;
    if (x as usize) < floor.len() {
        floor[x as usize] = y;
    }
    x += 1;
    let lim = x1.min(floor.len() as i32);
    while x < lim {
        err += ady;
        if err >= adx {
            err -= adx;
            y += sy;
        } else {
            y += base;
        }
        floor[x as usize] = y;
        x += 1;
    }
}

/// Decodes a whole converted stream, trimmed to the declared sample count.
pub fn decode_all(v: &StdVorbis) -> Result<Pcm, String> {
    let mut d = Decoder::new(v)?;
    let mut chans: Vec<Vec<f32>> = vec![Vec::new(); v.channels as usize];
    for (i, pk) in v.packets.iter().enumerate() {
        match d.decode_packet(pk) {
            Ok(Some(block)) => {
                for (c, b) in block.into_iter().enumerate() {
                    chans[c].extend_from_slice(&b);
                }
            }
            Ok(None) => {}
            Err(e) => return Err(format!("packet {i}: {e}")),
        }
    }
    if v.sample_count > 0 {
        for c in chans.iter_mut() {
            c.truncate(v.sample_count as usize);
        }
    }
    Ok(Pcm { sample_rate: v.sample_rate, channels: chans })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imdct_matches_direct_formula() {
        let n = 32usize;
        let fft = Fft::new(n);
        let spec: Vec<f32> = (0..n / 2).map(|i| ((i * 7 % 5) as f32 - 2.0) * 0.3).collect();
        let (mut re, mut im, mut out) = (vec![0f32; n], vec![0f32; n], vec![0f32; n]);
        imdct(&fft, &spec, &mut out, &mut re, &mut im);
        for k in 0..n {
            let mut s = 0f64;
            for (m, x) in spec.iter().enumerate() {
                s += *x as f64
                    * (2.0 * std::f64::consts::PI / n as f64 * (k as f64 + 0.5 + n as f64 / 4.0) * (m as f64 + 0.5))
                        .cos();
            }
            assert!((s - out[k] as f64).abs() < 1e-3, "k={k} {s} vs {}", out[k]);
        }
    }

    #[test]
    fn float32_unpack_basics() {
        // mantissa 1, exponent 788 -> 1.0
        assert_eq!(float32_unpack(1 | (788 << 21)), 1.0);
        assert_eq!(float32_unpack(1 | (788 << 21) | 0x8000_0000), -1.0);
    }

    #[test]
    fn render_line_endpoints() {
        let mut f = vec![0i32; 16];
        render_line(0, 0, 8, 8, &mut f);
        assert_eq!(f[0], 0);
        assert_eq!(f[7], 7);
    }

    #[test]
    fn slope_is_monotone() {
        let s = slope(64);
        assert!(s.windows(2).all(|w| w[1] >= w[0]));
        assert!(s[0] > 0.0 && s[63] < 1.0001);
    }
}
