//! Wwise audio census / exporter (headless; never plays audio).
//!
//! Usage: audio_export <command> [options]
//!   census                          counts of banks, media, codecs, HIRC objects
//!   probe <bank> <wem_id>           convert + decode one media (to a scratch dir)
//!   export [options]                decode every media, verify it, write WAVs and the index
//!
//! export options:
//!   --limit N        write at most N WAV files (default: all within the size cap)
//!   --max-gb G       stop writing WAVs after G gigabytes (default 3.0)
//!   --only BANK      restrict to one bank name, or `Default.pck`
//!   --no-wav         decode + verify + index only
//!   --ogg            also write the standard Ogg Vorbis / Ogg Opus rewrap next to each WAV
//!   --index PATH     typed index sheet to write (default data/audio_index.tsv)
//!   --threads N      worker threads (default 3)
//!   --crosscheck N   re-decode N Vorbis and N Opus files with ffmpeg and compare
//!   --names          also map events to media and hash-match event names (needs all banks)
//!
//! The Opus decode uses the system libopus through its C ABI (no crate); Vorbis uses the
//! decoder in `pla::assets::wwise::vorbis_dec`.
use pla::assets::wwise::hirc::Graph;
use pla::assets::wwise::{self, opus, vorbis, vorbis_dec, wav, Bank, Codec, CodebookLibrary, Pck, Wem};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::raw::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

#[link(name = "opus")]
extern "C" {
    fn opus_decoder_create(fs: i32, channels: i32, error: *mut i32) -> *mut c_void;
    fn opus_decode_float(st: *mut c_void, data: *const u8, len: i32, pcm: *mut f32, frame_size: i32, fec: i32) -> i32;
    fn opus_decoder_destroy(st: *mut c_void);
}

const GAME_ROOT: &str = "/home/nico/work/decompilacion/work/pla/pk2";

fn bank_dir() -> PathBuf {
    PathBuf::from(GAME_ROOT).join("romfs_patched").join(wwise::SOUND_BANK_DIR)
}

fn cache_root() -> PathBuf {
    PathBuf::from(std::env::var("PLA_CACHE_ROOT").unwrap_or_else(|_| "/home/nico/work/decompilacion/work/cache".into()))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("census") => census(),
        Some("probe") => probe(&args[1], args[2].parse().map_err(|_| "bad index")?),
        Some("export") => export(&args[1..]),
        _ => Err("usage: audio_export census | probe <bank> <index> | export [options]".into()),
    }
}

// ---------------------------------------------------------------------------------------
// Item discovery
// ---------------------------------------------------------------------------------------

#[derive(Clone)]
struct Item {
    bank: String,
    id: u32,
    source: &'static str, // "bank" | "pck"
    path: PathBuf,
    offset: u64,
    size: u32,
    /// Bank entry whose full file lives in a package (this entry is only a prefix).
    prefetch_of_pck: Option<u32>,
}

impl Item {
    fn key(&self) -> String {
        format!("{}/{}", self.bank, self.id)
    }

    fn read(&self) -> Result<Vec<u8>, String> {
        let mut f = File::open(&self.path).map_err(|e| e.to_string())?;
        f.seek(SeekFrom::Start(self.offset)).map_err(|e| e.to_string())?;
        let mut v = vec![0u8; self.size as usize];
        f.read_exact(&mut v).map_err(|e| e.to_string())?;
        Ok(v)
    }
}

struct Discovery {
    items: Vec<Item>,
    banks: Vec<(String, PathBuf, Bank)>,
    pck_ids: HashMap<u32, u32>,
}

fn discover() -> Result<Discovery, String> {
    let dir = bank_dir();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "bnk"))
        .collect();
    paths.sort();
    let mut items = Vec::new();
    let mut banks = Vec::new();
    for p in &paths {
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
        let b = Bank::parse(&bytes).map_err(|e| format!("{name}: {e}"))?;
        for e in &b.didx {
            items.push(Item {
                bank: name.clone(),
                id: e.id,
                source: "bank",
                path: p.clone(),
                offset: (b.data.start + e.offset as usize) as u64,
                size: e.size,
                prefetch_of_pck: None,
            });
        }
        banks.push((name, p.clone(), b));
    }
    let mut pck_ids = HashMap::new();
    for pck_name in ["Default.pck", "LooseMedia.pck"] {
        let p = dir.join(pck_name);
        let mut f = File::open(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let pck = Pck::parse(&mut f)?;
        for e in &pck.streams {
            pck_ids.insert(e.id, e.size);
            items.push(Item {
                bank: pck_name.to_string(),
                id: e.id,
                source: "pck",
                path: p.clone(),
                offset: e.byte_offset(),
                size: e.size,
                prefetch_of_pck: None,
            });
        }
    }
    for it in items.iter_mut().filter(|i| i.source == "bank") {
        if let Some(&full) = pck_ids.get(&it.id) {
            it.prefetch_of_pck = Some(full);
        }
    }
    Ok(Discovery { items, banks, pck_ids })
}

// ---------------------------------------------------------------------------------------
// Census
// ---------------------------------------------------------------------------------------

fn census() -> Result<(), String> {
    let d = discover()?;
    println!("banks: {}", d.banks.len());
    let mut total: BTreeMap<String, usize> = BTreeMap::new();
    let mut hirc_total: BTreeMap<u8, usize> = BTreeMap::new();
    for (name, _, b) in &d.banks {
        let mut kinds: BTreeMap<u8, usize> = BTreeMap::new();
        for o in &b.hirc {
            *kinds.entry(o.kind).or_default() += 1;
            *hirc_total.entry(o.kind).or_default() += 1;
        }
        let mut codecs: BTreeMap<String, usize> = BTreeMap::new();
        for it in d.items.iter().filter(|i| i.bank == *name) {
            let bytes = it.read()?;
            let key = match Wem::parse(&bytes) {
                Ok(w) => format!("{:#06x}", w.tag),
                Err(_) => "unparsed".into(),
            };
            *codecs.entry(key.clone()).or_default() += 1;
            *total.entry(format!("bank {key}")).or_default() += 1;
        }
        println!(
            "  {name:<18} v{} media={:<5} hirc={:<6} codecs={codecs:?} hirc_kinds={kinds:?}",
            b.version,
            b.didx.len(),
            b.hirc.len()
        );
    }
    println!("hirc objects total by type: {hirc_total:?}");
    let pck_items: Vec<&Item> = d.items.iter().filter(|i| i.source == "pck").collect();
    println!("pck streams: {}", pck_items.len());
    let mut pck_codecs: BTreeMap<String, usize> = BTreeMap::new();
    let mut f = File::open(bank_dir().join("Default.pck")).map_err(|e| e.to_string())?;
    for it in pck_items.iter().filter(|i| i.bank == "Default.pck") {
        f.seek(SeekFrom::Start(it.offset)).map_err(|e| e.to_string())?;
        let mut head = [0u8; 24];
        let n = f.read(&mut head).map_err(|e| e.to_string())?;
        let key = if n >= 22 && &head[..4] == b"RIFF" {
            format!("{:#06x}", u16::from_le_bytes([head[20], head[21]]))
        } else {
            "unparsed".into()
        };
        *pck_codecs.entry(key.clone()).or_default() += 1;
        *total.entry(format!("pck {key}")).or_default() += 1;
    }
    println!("pck codecs (all entries): {pck_codecs:?}");
    let prefetch = d.items.iter().filter(|i| i.prefetch_of_pck.is_some()).count();
    println!("bank entries that are prefetch prefixes of a pck file: {prefetch}");
    println!("totals: {total:?}");
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Decoding of one item
// ---------------------------------------------------------------------------------------

struct Decoded {
    codec: String,
    channels: u16,
    rate: u32,
    declared_frames: Option<u64>,
    pcm: Option<Vec<Vec<f32>>>,
    ogg: Option<Vec<u8>>,
    ogg_ext: &'static str,
    note: String,
}

/// Decodes at the stream's own sample rate when libopus supports it (else 48 kHz).
fn opus_decode(o: &opus::StdOpus) -> Result<(Vec<Vec<f32>>, u32), String> {
    let ch = o.channels as i32;
    let fs = match o.sample_rate {
        8000 | 12000 | 16000 | 24000 | 48000 => o.sample_rate,
        _ => 48000,
    };
    unsafe {
        let mut err = 0i32;
        let st = opus_decoder_create(fs as i32, ch, &mut err);
        if st.is_null() || err != 0 {
            return Err(format!("opus_decoder_create error {err}"));
        }
        let mut out: Vec<Vec<f32>> = vec![Vec::new(); ch as usize];
        let mut buf = vec![0f32; 5760 * ch as usize];
        let mut result = Ok(());
        for (i, pk) in o.packets.iter().enumerate() {
            let n = opus_decode_float(st, pk.as_ptr(), pk.len() as i32, buf.as_mut_ptr(), 5760, 0);
            if n < 0 {
                result = Err(format!("packet {i}: opus error {n}"));
                break;
            }
            for f in 0..n as usize {
                for c in 0..ch as usize {
                    out[c].push(buf[f * ch as usize + c]);
                }
            }
        }
        opus_decoder_destroy(st);
        result?;
        if o.sample_count > 0 {
            for c in out.iter_mut() {
                c.truncate(o.sample_count as usize);
            }
        }
        Ok((out, fs))
    }
}

fn decode_item(bytes: &[u8], lib: &CodebookLibrary, want_ogg: bool) -> Result<Decoded, String> {
    let w = Wem::parse(bytes)?;
    match w.codec {
        Codec::WwiseVorbis => {
            let v = vorbis::convert(bytes, lib)?;
            let pcm = vorbis_dec::decode_all(&v)?;
            let ogg = want_ogg.then(|| vorbis::to_ogg(&v));
            Ok(Decoded {
                codec: w.codec.name(),
                channels: w.channels,
                rate: w.sample_rate,
                declared_frames: Some(v.sample_count as u64),
                pcm: Some(pcm.channels),
                ogg,
                ogg_ext: "ogg",
                note: String::new(),
            })
        }
        Codec::WwiseOpus(_) => {
            let o = opus::convert(bytes)?;
            let (pcm, fs) = opus_decode(&o)?;
            let ogg = if want_ogg { Some(opus::to_ogg(&o)?) } else { None };
            Ok(Decoded {
                codec: w.codec.name(),
                channels: w.channels,
                rate: fs,
                declared_frames: Some(o.sample_count as u64),
                pcm: Some(pcm),
                ogg,
                ogg_ext: "opus",
                note: if o.seek_entries != o.packets.len() {
                    format!("seek entries {} != packets {}", o.seek_entries, o.packets.len())
                } else {
                    String::new()
                },
            })
        }
        Codec::Pcm => {
            if w.bits_per_sample != 16 {
                return Err(format!("PCM with {} bits per sample is not handled", w.bits_per_sample));
            }
            let data = &bytes[w.data.clone()];
            let ch = w.channels as usize;
            let frames = data.len() / (2 * ch);
            let mut chans = vec![Vec::with_capacity(frames); ch];
            for f in 0..frames {
                for (c, chv) in chans.iter_mut().enumerate() {
                    let o = (f * ch + c) * 2;
                    chv.push(i16::from_le_bytes([data[o], data[o + 1]]) as f32 / 32768.0);
                }
            }
            Ok(Decoded {
                codec: w.codec.name(),
                channels: w.channels,
                rate: w.sample_rate,
                declared_frames: Some(frames as u64),
                pcm: Some(chans),
                ogg: None,
                ogg_ext: "",
                note: String::new(),
            })
        }
        Codec::Other(tag) => {
            // 0x8311: 4 bits per sample, 36-byte frames of 64 samples (avg bytes / block align).
            let est = if w.block_align > 0 && w.avg_bytes_per_sec > 0 {
                let frames = (bytes[w.data.clone()].len() / w.block_align as usize) as u64;
                let per_frame = w.sample_rate as u64 * w.block_align as u64 / w.avg_bytes_per_sec.max(1) as u64;
                Some(frames * per_frame)
            } else {
                None
            };
            Ok(Decoded {
                codec: w.codec.name(),
                channels: w.channels,
                rate: w.sample_rate,
                declared_frames: est,
                pcm: None,
                ogg: None,
                ogg_ext: "",
                note: format!(
                    "unidentified codec tag {tag:#06x}: bps={} block_align={} avg_bytes_per_sec={}; no decoder",
                    w.bits_per_sample, w.block_align, w.avg_bytes_per_sec
                ),
            })
        }
    }
}

// ---------------------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------------------

#[derive(Default, Clone)]
struct Row {
    key: String,
    bank: String,
    id: u32,
    source: &'static str,
    codec: String,
    channels: u16,
    rate: u32,
    duration_s: Option<f32>,
    size: u32,
    status: &'static str,
    reason: String,
    peak: Option<f32>,
    rms: Option<f32>,
    centroid: Option<f32>,
    wav: Option<String>,
    events: u32,
    category: String,
    name: String,
    name_conf: &'static str,
    species: Option<u16>,
}

struct Opts {
    limit: usize,
    max_bytes: u64,
    only: Option<String>,
    no_wav: bool,
    ogg: bool,
    index: PathBuf,
    threads: usize,
    crosscheck: usize,
    names: bool,
}

fn parse_opts(a: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        limit: usize::MAX,
        max_bytes: 3 * 1024 * 1024 * 1024,
        only: None,
        no_wav: false,
        ogg: false,
        index: PathBuf::from("data/audio_index.tsv"),
        threads: 3,
        crosscheck: 0,
        names: false,
    };
    let mut i = 0;
    while i < a.len() {
        let next = |i: &mut usize| -> Result<String, String> {
            *i += 1;
            a.get(*i).cloned().ok_or_else(|| format!("{} needs a value", a[*i - 1]))
        };
        match a[i].as_str() {
            "--limit" => o.limit = next(&mut i)?.parse().map_err(|_| "bad --limit")?,
            "--max-gb" => o.max_bytes = (next(&mut i)?.parse::<f64>().map_err(|_| "bad --max-gb")? * 1073741824.0) as u64,
            "--only" => o.only = Some(next(&mut i)?),
            "--no-wav" => o.no_wav = true,
            "--ogg" => o.ogg = true,
            "--index" => o.index = PathBuf::from(next(&mut i)?),
            "--threads" => o.threads = next(&mut i)?.parse().map_err(|_| "bad --threads")?,
            "--crosscheck" => o.crosscheck = next(&mut i)?.parse().map_err(|_| "bad --crosscheck")?,
            "--names" => o.names = true,
            x => return Err(format!("unknown option {x}")),
        }
        i += 1;
    }
    Ok(o)
}

fn safe_dir(bank: &str) -> String {
    bank.replace('.', "_")
}

fn process(it: &Item, lib: &CodebookLibrary, o: &Opts, wavs: &AtomicUsize, bytes_written: &AtomicU64) -> Row {
    let mut row = Row {
        key: it.key(),
        bank: it.bank.clone(),
        id: it.id,
        source: it.source,
        size: it.size,
        status: "blocked",
        name_conf: "unknown",
        ..Default::default()
    };
    if let Some(full) = it.prefetch_of_pck {
        row.status = "prefetch";
        row.reason = format!("bank holds the first {} bytes; the full {} bytes are in Default.pck under the same id", it.size, full);
        if let Ok(b) = it.read() {
            if let Ok(w) = Wem::parse(&b) {
                row.codec = w.codec.name();
                row.channels = w.channels;
                row.rate = w.sample_rate;
            }
        }
        return row;
    }
    let bytes = match it.read() {
        Ok(b) => b,
        Err(e) => {
            row.reason = e;
            return row;
        }
    };
    let want_ogg = o.ogg && !o.no_wav;
    let dec = match decode_item(&bytes, lib, want_ogg) {
        Ok(d) => d,
        Err(e) => {
            if let Ok(w) = Wem::parse(&bytes) {
                row.codec = w.codec.name();
                row.channels = w.channels;
                row.rate = w.sample_rate;
            }
            row.reason = e;
            return row;
        }
    };
    row.codec = dec.codec.clone();
    row.channels = dec.channels;
    row.rate = dec.rate;
    row.duration_s = dec.declared_frames.map(|f| f as f32 / dec.rate.max(1) as f32);
    let Some(pcm) = dec.pcm else {
        row.reason = dec.note;
        return row;
    };
    let st = wav::stats(&pcm, dec.rate);
    row.peak = Some(st.peak);
    row.rms = Some(st.rms);
    row.centroid = Some(st.centroid_hz);
    let mut problems = Vec::new();
    if !dec.note.is_empty() {
        problems.push(dec.note.clone());
    }
    if st.non_finite > 0 {
        problems.push(format!("{} non-finite samples", st.non_finite));
    }
    if let Some(decl) = dec.declared_frames {
        let diff = (st.frames as i64 - decl as i64).unsigned_abs();
        if diff as f64 > (decl as f64 * 0.01).max(1.0) {
            problems.push(format!("decoded {} frames vs declared {}", st.frames, decl));
        }
    }
    if st.clipped_ratio > 0.01 {
        problems.push(format!("{:.1}% samples at full scale", st.clipped_ratio * 100.0));
    }
    if st.peak > 1.5 {
        problems.push(format!("peak {:.2} exceeds full scale", st.peak));
    }
    row.status = if !problems.is_empty() {
        "suspect"
    } else if st.rms < 1e-6 {
        "silent"
    } else {
        "ok"
    };
    if row.status == "silent" {
        let secs = dec.declared_frames.unwrap_or(0) as f32 / dec.rate.max(1) as f32;
        problems.push(format!(
            "decodes to digital silence; {} bytes for {:.2} s ({:.0} bytes/s)",
            it.size,
            secs,
            it.size as f32 / secs.max(0.001)
        ));
    }
    // A file whose peak exceeds full scale is written peak-normalised to 0.99 (never hard-clipped).
    let mut pcm = pcm;
    if st.peak > 1.0 {
        let k = 0.99 / st.peak;
        for c in pcm.iter_mut() {
            for x in c.iter_mut() {
                *x *= k;
            }
        }
        problems.push(format!("WAV written scaled by {k:.4} (peak-normalised)"));
    }
    row.reason = problems.join("; ");
    if !o.no_wav {
        let n = wavs.fetch_add(1, Ordering::SeqCst);
        let est = st.frames as u64 * pcm.len() as u64 * 2 + 44;
        if n < o.limit && bytes_written.load(Ordering::SeqCst) + est <= o.max_bytes {
            let rel = format!("audio/{}/{}.wav", safe_dir(&it.bank), it.id);
            let path = cache_root().join(&rel);
            let (pcm16, _) = wav::interleave_i16(&pcm);
            let data = wav::wav_bytes(dec.rate, pcm.len() as u16, &pcm16);
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match std::fs::write(&path, &data) {
                Ok(()) => {
                    bytes_written.fetch_add(data.len() as u64, Ordering::SeqCst);
                    row.wav = Some(format!("work/cache/{rel}"));
                    if let Some(ogg) = &dec.ogg {
                        if dec.ogg_ext == "ogg" {
                            let _ = std::fs::write(path.with_extension("ogg"), ogg);
                        }
                    }
                }
                Err(e) => row.reason = format!("{}; wav write failed: {e}", row.reason),
            }
        }
    }
    row
}

fn export(args: &[String]) -> Result<(), String> {
    let o = parse_opts(args)?;
    let log = |m: &str| {
        let line = format!("{} {m}\n", chrono_like());
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("work/progress/audio.log") {
            let _ = f.write_all(line.as_bytes());
        }
        eprintln!("{m}");
    };
    let lib = CodebookLibrary::from_elf_file(&format!("{GAME_ROOT}/main.elf"))?;
    log(&format!("codebook library: {} books", lib.books.len()));
    let d = discover()?;
    let selected: Vec<Item> = d
        .items
        .iter()
        .filter(|i| o.only.as_ref().is_none_or(|b| &i.bank == b))
        .cloned()
        .collect();
    log(&format!("{} media items selected of {}", selected.len(), d.items.len()));
    let next = AtomicUsize::new(0);
    let wavs = AtomicUsize::new(0);
    let written = AtomicU64::new(0);
    let rows: Mutex<Vec<Row>> = Mutex::new(Vec::with_capacity(selected.len()));
    let t0 = std::time::Instant::now();
    std::thread::scope(|s| {
        for _ in 0..o.threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(it) = selected.get(i) else { break };
                let r = process(it, &lib, &o, &wavs, &written);
                rows.lock().unwrap().push(r);
                if i % 500 == 499 {
                    eprintln!("  {} / {} ({:.0}s)", i + 1, selected.len(), t0.elapsed().as_secs_f32());
                }
            });
        }
    });
    let mut rows = rows.into_inner().unwrap();
    rows.sort_by(|a, b| a.key.cmp(&b.key));
    let mut by_status: BTreeMap<&str, usize> = BTreeMap::new();
    let mut by_codec: BTreeMap<(String, &str), usize> = BTreeMap::new();
    for r in &rows {
        *by_status.entry(r.status).or_default() += 1;
        *by_codec.entry((r.codec.clone(), r.status)).or_default() += 1;
    }
    log(&format!("status counts: {by_status:?}"));
    log(&format!("codec/status: {by_codec:?}"));
    log(&format!("wav files written: {} ({:.2} GB)", rows.iter().filter(|r| r.wav.is_some()).count(), written.load(Ordering::SeqCst) as f64 / 1073741824.0));
    if o.names {
        map_names(&d, &mut rows, &log)?;
    }
    if o.crosscheck > 0 {
        crosscheck(&selected, &lib, o.crosscheck, &log)?;
    }
    write_index(&o.index, &rows)?;
    log(&format!("index written: {} ({} rows)", o.index.display(), rows.len()));
    Ok(())
}

fn chrono_like() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{:02}:{:02}:{:02}", t / 3600 % 24, t / 60 % 60, t % 60)
}

fn tsv(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

fn write_index(path: &Path, rows: &[Row]) -> Result<(), String> {
    let mut out = String::new();
    out.push_str("# sheet: data/audio_index\n# version: 1\n# generator: crates/pla/examples/audio_export.rs\n");
    out.push_str("# target: Pokémon Legends: Arceus update v262144 Wwise media index (WEM files in .bnk banks and Default.pck; decode status measured, names by FNV hash)\n");
    out.push_str("# requires: bin/sound/NX64/wwise/bank/*.bnk; bin/sound/NX64/wwise/bank/Default.pck; main.elf (Vorbis codebook library)\n");
    out.push_str("entry:string*\tbank:string\twem_id:u32\tsource:enum\tcodec:string\tchannels:u8\tsample_rate:u32\tduration_s:f32?\tsize_bytes:u32\tstatus:enum\treason:string?\tpeak:f32?\trms:f32?\tcentroid_hz:f32?\twav_rel:string?\tevent_count:u16\tcategory:string?\tname:string?\tname_confidence:enum\tspecies:u16?\n");
    let f = |v: Option<f32>, p: usize| v.map(|x| format!("{x:.p$}")).unwrap_or_default();
    for r in rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.key,
            r.bank,
            r.id,
            r.source,
            r.codec,
            r.channels,
            r.rate,
            f(r.duration_s, 3),
            r.size,
            r.status,
            tsv(&r.reason),
            f(r.peak, 4),
            f(r.rms, 5),
            f(r.centroid, 0),
            r.wav.clone().unwrap_or_default(),
            r.events.min(65535),
            r.category,
            tsv(&r.name),
            r.name_conf,
            r.species.map(|v| v.to_string()).unwrap_or_default()
        ));
    }
    std::fs::write(path, out).map_err(|e| format!("{}: {e}", path.display()))
}

// ---------------------------------------------------------------------------------------
// Names and events
// ---------------------------------------------------------------------------------------

/// Scans every identifier-like run in `bytes` and records those whose hash is in `wanted`.
fn scan_bytes(bytes: &[u8], wanted: &HashSet<u32>, found: &mut HashMap<u32, String>) {
    let mut start: Option<usize> = None;
    for (i, &c) in bytes.iter().enumerate().chain(std::iter::once((bytes.len(), &0u8))) {
        let ok = c.is_ascii_alphanumeric() || c == b'_';
        match (ok, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                let len = i - s;
                if (4..=120).contains(&len) {
                    let h = wwise::fnv1_lower(std::str::from_utf8(&bytes[s..i]).unwrap_or(""));
                    if wanted.contains(&h) {
                        found.entry(h).or_insert_with(|| String::from_utf8_lossy(&bytes[s..i]).to_string());
                    }
                }
                start = None;
            }
            _ => {}
        }
    }
}

/// Hash-matches game strings against `wanted` ids. Reads the executable, the sound tables and
/// every data file under `bin/` (except the sound banks themselves) in bounded chunks.
fn find_names(wanted: &HashSet<u32>, log: &dyn Fn(&str)) -> HashMap<u32, String> {
    let root = PathBuf::from(GAME_ROOT);
    let mut files = vec![root.join("main.elf")];
    let mut stack = vec![root.join("romfs_patched/bin")];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.ends_with("sound/NX64") {
                    continue;
                }
                stack.push(p);
            } else {
                files.push(p);
            }
        }
    }
    log(&format!("scanning {} files for event names", files.len()));
    let next = AtomicUsize::new(0);
    let found: Mutex<HashMap<u32, String>> = Mutex::new(HashMap::new());
    std::thread::scope(|s| {
        for _ in 0..3 {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(path) = files.get(i) else { break };
                let Ok(mut f) = File::open(path) else { continue };
                let mut local = HashMap::new();
                let mut buf = vec![0u8; 16 << 20];
                let mut carry = 0usize;
                loop {
                    let Ok(n) = f.read(&mut buf[carry..]) else { break };
                    if n == 0 {
                        scan_bytes(&buf[..carry], wanted, &mut local);
                        break;
                    }
                    let total = carry + n;
                    // Cut after the last non-identifier byte so no identifier straddles chunks.
                    let cut = buf[..total]
                        .iter()
                        .rposition(|&c| !(c.is_ascii_alphanumeric() || c == b'_'))
                        .map_or(0, |i| i + 1);
                    scan_bytes(&buf[..cut], wanted, &mut local);
                    buf.copy_within(cut..total, 0);
                    carry = total - cut;
                    if carry > (4 << 20) {
                        carry = 0;
                    }
                }
                let mut g = found.lock().unwrap();
                for (h, n) in local {
                    g.entry(h).or_insert(n);
                }
            });
        }
    });
    found.into_inner().unwrap()
}


/// Builds candidate names from templates found in the game (not stored strings) and keeps the
/// ones whose hash matches a wanted id.
///   * `PLAY_PV_%03d_%02d_%02d` is a printf template in the executable (Pokémon voice events):
///     species 1..=1100, form 0..=40, variant 0..=99.
///   * `PLAY_PM_<nnn>_<suffix>` / `PLAY_PM_N_<nnn>_<suffix>` / `STOP_...`: the species-specific
///     suffixes seen in already matched names are tried for every species number.
fn generate_names(wanted: &HashSet<u32>, known: &HashMap<u32, String>) -> HashMap<u32, String> {
    let mut out = HashMap::new();
    for prefix in ["PLAY_PV_", "STOP_PV_"] {
        for sp in 1..=1100u32 {
            for form in 0..=40u32 {
                for var in 0..=99u32 {
                    let n = format!("{prefix}{sp:03}_{form:02}_{var:02}");
                    let h = wwise::fnv1_lower(&n);
                    if wanted.contains(&h) && !known.contains_key(&h) {
                        out.entry(h).or_insert(n);
                    }
                }
            }
        }
    }
    let prefixes = ["PLAY_PM_N_", "STOP_PM_N_", "PLAY_PM_", "STOP_PM_", "PLAY_PV_", "STOP_PV_"];
    let mut suffixes: HashMap<&str, HashSet<String>> = HashMap::new();
    for n in known.values() {
        let u = n.to_ascii_uppercase();
        for p in prefixes {
            if let Some(rest) = u.strip_prefix(p) {
                if let Some((num, suf)) = rest.split_once('_') {
                    if num.len() == 3 && num.bytes().all(|b| b.is_ascii_digit()) && !suf.is_empty() {
                        let fam = if p.contains("PV") { "PV" } else { "PM" };
                        suffixes.entry(fam).or_default().insert(suf.to_string());
                    }
                }
            }
        }
    }
    for (fam, sufs) in &suffixes {
        let ps: Vec<&str> = prefixes.iter().copied().filter(|p| p.contains(if *fam == "PV" { "PV" } else { "PM" })).collect();
        for suf in sufs {
            for sp in 1..=1100u32 {
                for p in &ps {
                    let n = format!("{p}{sp:03}_{suf}");
                    let h = wwise::fnv1_lower(&n);
                    if wanted.contains(&h) && !known.contains_key(&h) {
                        out.entry(h).or_insert(n);
                    }
                }
            }
        }
    }
    out
}

/// Species number encoded in a verified name (`PLAY_PV_025_00_00`, `PLAY_PM_025_...`).
fn species_of(name: &str) -> Option<u16> {
    let u = name.to_ascii_uppercase();
    for p in ["PLAY_PV_", "STOP_PV_", "PLAY_PM_N_", "STOP_PM_N_", "PLAY_PM_", "STOP_PM_"] {
        if let Some(rest) = u.strip_prefix(p) {
            let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if num.len() == 3 {
                return num.parse().ok();
            }
        }
    }
    None
}

fn category_from_name(n: &str) -> &'static str {
    let u = n.to_ascii_uppercase();
    if u.contains("BGM") {
        "bgm"
    } else if u.contains("_ME_") || u.starts_with("PLAY_ME") {
        "jingle"
    } else if u.starts_with("PLAY_UI") || u.starts_with("STOP_UI") {
        "ui"
    } else if u.contains("VOICE") || u.contains("CRY") || u.contains("_PV_") {
        "pokemon_voice"
    } else if u.starts_with("PLAY_PM") || u.starts_with("STOP_PM") {
        "pokemon_sfx"
    } else if u.starts_with("PLAY_BA") || u.starts_with("STOP_BA") {
        "battle"
    } else if u.starts_with("PLAY_PL") || u.starts_with("STOP_PL") {
        "player"
    } else if u.starts_with("PLAY_ENV") || u.starts_with("STOP_ENV") {
        "environment"
    } else {
        "other_sfx"
    }
}

fn category_from_bank(bank: &str) -> &'static str {
    match bank {
        "BGM" => "bgm",
        "ME" => "jingle",
        "UI" => "ui",
        "PM_VOICE" | "Default.pck" => "unknown",
        b if b.starts_with("PM_N") => "pokemon_sfx",
        "PM_NORMAL" | "PM_SKILLS" => "pokemon_sfx",
        "BATTLE_SYSTEM" => "battle",
        "PC" => "player",
        "NPC" => "npc",
        "PROPS" => "props",
        "SCENARIO_SCRIPT" | "Dialog" | "ADEMO" => "event_audio",
        "Common" => "common",
        _ => "unknown",
    }
}

fn map_names(d: &Discovery, rows: &mut [Row], log: &dyn Fn(&str)) -> Result<(), String> {
    let mut g = Graph::default();
    for (name, path, b) in &d.banks {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        g.add_bank(name, &bytes, b);
    }
    g.finish();
    let events = g.events();
    log(&format!("events: {}", events.len()));
    let ev_set: HashSet<u32> = events.iter().copied().collect();
    let mut names = find_names(&ev_set, log);
    let from_strings = names.len();
    let generated = generate_names(&ev_set, &names);
    log(&format!("event names from game strings {from_strings}; from templates {}", generated.len()));
    let generated_ids: HashSet<u32> = generated.keys().copied().collect();
    names.extend(generated);
    log(&format!("event names matched by FNV hash from game strings: {} of {}", names.len(), events.len()));
    // media id -> events
    let mut media_events: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut heuristic_events = 0usize;
    let mut resolved_events = 0usize;
    for &e in &events {
        let r = g.resolve(e);
        if !r.sources.is_empty() {
            resolved_events += 1;
        }
        if r.heuristic {
            heuristic_events += 1;
        }
        for s in r.sources {
            media_events.entry(s.media_id).or_default().push(e);
        }
    }
    log(&format!("events resolving to media: {resolved_events} (music child lists via scan: {heuristic_events})"));
    let mut named_media = 0usize;
    for r in rows.iter_mut() {
        let evs = media_events.get(&r.id).cloned().unwrap_or_default();
        r.events = evs.len() as u32;
        let mut cand: Vec<&String> = evs.iter().filter_map(|e| names.get(e)).collect();
        cand.sort();
        r.category = category_from_bank(&r.bank).to_string();
        if let Some(n) = cand.first() {
            r.name = (*n).clone();
            let id = evs.iter().find(|e| names.get(e) == Some(*n)).copied().unwrap_or(0);
            r.name_conf = if generated_ids.contains(&id) { "hash_template" } else { "hash_string" };
            r.category = category_from_name(n).to_string();
            r.species = species_of(n);
            named_media += 1;
        } else if !evs.is_empty() {
            r.name_conf = "bank_only";
        }
    }
    log(&format!("media rows with a hash-matched event name: {named_media} of {}", rows.len()));
    let unnamed_events: Vec<u32> = events.iter().copied().filter(|e| !names.contains_key(e)).collect();
    let mut per_bank: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for &e in &events {
        let b = g.bank_of(e).unwrap_or("?").to_string();
        let ent = per_bank.entry(b).or_default();
        ent.0 += 1;
        if names.contains_key(&e) {
            ent.1 += 1;
        }
    }
    log(&format!("events named per bank (total, named): {per_bank:?}; unnamed {}", unnamed_events.len()));
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Cross-check with ffmpeg
// ---------------------------------------------------------------------------------------

fn crosscheck(items: &[Item], lib: &CodebookLibrary, n: usize, log: &dyn Fn(&str)) -> Result<(), String> {
    let tmp = cache_root().join("audio").join("_crosscheck");
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let mut done = [0usize; 2];
    for it in items {
        if it.prefetch_of_pck.is_some() {
            continue;
        }
        let Ok(bytes) = it.read() else { continue };
        let Ok(w) = Wem::parse(&bytes) else { continue };
        let kind = match w.codec {
            Codec::WwiseVorbis => 0,
            Codec::WwiseOpus(_) => 1,
            _ => continue,
        };
        if done[kind] >= n || w.sample_rate == 0 {
            continue;
        }
        let Ok(dec) = decode_item(&bytes, lib, true) else { continue };
        let (Some(ogg), Some(pcm)) = (dec.ogg.clone(), dec.pcm.clone()) else { continue };
        let ogg_path = tmp.join(format!("{}_{}.{}", safe_dir(&it.bank), it.id, dec.ogg_ext));
        std::fs::write(&ogg_path, &ogg).map_err(|e| e.to_string())?;
        let out = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-nostdin", "-y", "-i"])
            .arg(&ogg_path)
            .args(["-f", "f32le", "-acodec", "pcm_f32le", "-"])
            .output()
            .map_err(|e| format!("ffmpeg: {e}"))?;
        let ch = pcm.len();
        let ff: Vec<f32> = out.stdout.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
        let frames = (ff.len() / ch.max(1)).min(pcm[0].len());
        let (mut err2, mut sig2) = (0f64, 0f64);
        for f in 0..frames {
            for (c, chv) in pcm.iter().enumerate() {
                let a = chv[f] as f64;
                let b = ff[f * ch + c] as f64;
                err2 += (a - b) * (a - b);
                sig2 += b * b;
            }
        }
        let snr = if err2 > 0.0 { 10.0 * (sig2 / err2).log10() } else { 200.0 };
        log(&format!(
            "crosscheck {} {}: ffmpeg frames {} vs ours {} ; SNR of ours vs ffmpeg {:.1} dB (ffmpeg stderr: {})",
            it.key(),
            dec.codec,
            ff.len() / ch.max(1),
            pcm[0].len(),
            snr,
            String::from_utf8_lossy(&out.stderr).trim().chars().take(80).collect::<String>()
        ));
        done[kind] += 1;
        if done == [n, n] {
            break;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Probe (development helper)
// ---------------------------------------------------------------------------------------

fn probe(bank: &str, id: usize) -> Result<(), String> {
    let lib = CodebookLibrary::from_elf_file(&format!("{GAME_ROOT}/main.elf"))?;
    println!("codebooks: {}", lib.books.len());
    let d = discover()?;
    let bank = bank.trim_end_matches(".bnk");
    let item = d
        .items
        .iter()
        .find(|i| i.bank == bank && i.id as usize == id)
        .ok_or_else(|| format!("no media {bank}/{id}"))?;
    let wem_bytes = item.read()?;
    let wem = Wem::parse(&wem_bytes)?;
    println!("{:?} ch={} rate={}", wem.codec, wem.channels, wem.sample_rate);
    let dec = decode_item(&wem_bytes, &lib, true)?;
    let pcm = dec.pcm.ok_or("no pcm")?;
    let st = wav::stats(&pcm, dec.rate);
    println!("declared {:?} decoded {} {:?}", dec.declared_frames, st.frames, st);
    let (i16s, clip) = wav::interleave_i16(&pcm);
    println!("clip={clip}");
    let scratch = std::env::var("PROBE_OUT").unwrap_or_else(|_| "/tmp/claude-1000".into());
    std::fs::write(format!("{scratch}/probe.wav"), wav::wav_bytes(dec.rate, pcm.len() as u16, &i16s)).map_err(|e| e.to_string())?;
    if let Some(o) = dec.ogg {
        std::fs::write(format!("{scratch}/probe.{}", dec.ogg_ext), o).map_err(|e| e.to_string())?;
    }
    Ok(())
}
