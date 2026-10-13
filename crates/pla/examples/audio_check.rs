//! Headless checks of the game audio pipeline. Never opens an audio device and never plays
//! anything: it only reads the index, the decoded files and (for `events`) the Wwise banks.
//!
//! ```text
//! cargo run -p pla --example audio_check -- census
//! cargo run -p pla --example audio_check -- decode [--all-cries]
//! cargo run -p pla --example audio_check -- events [--write data/audio_event_media.tsv]
//! ```
//!
//! * `census`: what every port sound (`SfxId`, `MusicId`, cries) resolves to: event, take,
//!   file (Ogg or WAV), present on disk.
//! * `decode`: decodes each chosen file with the same decoder Bevy uses at play time
//!   (`bevy_audio::AudioSource` -> rodio, `wav` + `vorbis` features) into memory and checks
//!   length against the index (±1 %), finiteness and peak; then mixes a few of them offline
//!   with the bus volumes of `settings.txt` and reports the mixed peak.
//! * `events`: hashes candidate music event names (`PLAY_BGM_HA_FI_AREAnn[_DAY|...]`, ...)
//!   with FNV-1 against the event ids of the banks, resolves the matches to media through the
//!   HIRC graph and writes `data/audio_event_media.tsv`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy_audio::{AudioSource, Decodable, Source};
use pla::assets::wwise::hirc::Graph;
use pla::assets::wwise::{fnv1_lower, Bank};
use pla::game::audio::catalog::{decodable_header, SoundCatalog};
use pla::game::audio::events::{cry_event_name, Bus, MusicId, SfxId, VariantPolicy};
use pla::game::audio::mix::BusVolumes;
use pla::game::audio::{default_cache_root, find_data_dir};
use pla::game::settings::UserSettings;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("census");
    let data = find_data_dir().ok_or("data/audio_index.tsv not found")?;
    match mode {
        "census" => census(&catalog(&data)?),
        "decode" => decode(&catalog(&data)?, args.iter().any(|a| a == "--all-cries")),
        "events" => {
            let write = args.iter().position(|a| a == "--write").and_then(|i| args.get(i + 1));
            events(&data, write.map(PathBuf::from))
        }
        _ => Err(format!("unknown mode {mode} (census | decode | events)")),
    }
}

fn catalog(data: &Path) -> Result<SoundCatalog, String> {
    SoundCatalog::load(data, default_cache_root())
}

/// (label, event, policy, bus) of every sound the port plays, plus 25 cries.
fn port_sounds(cat: &SoundCatalog, all_cries: bool) -> Vec<(String, String, VariantPolicy, Bus)> {
    let mut v = Vec::new();
    for s in SfxId::ALL {
        v.push((format!("{s:?}"), s.event_name().to_string(), s.policy(), s.bus()));
    }
    for m in [
        MusicId::Area(0),
        MusicId::Area(1),
        MusicId::Area(2),
        MusicId::Area(3),
        MusicId::Area(4),
        MusicId::Area(5),
        MusicId::Area(6),
        MusicId::FieldNight,
        MusicId::WildBattle,
        MusicId::TrainerBattle,
    ] {
        v.push((format!("{m:?}"), m.event_name().unwrap_or("").to_string(), VariantPolicy::Longest, Bus::Music));
    }
    let species = cat.species_with_cries();
    let take = if all_cries { species.len() } else { 25 };
    for sp in species.into_iter().take(take) {
        let ev = cat.cry_event(sp, 0).unwrap_or_else(|| cry_event_name(sp, 0, 0));
        v.push((format!("cry {sp}"), ev, VariantPolicy::Loudest, Bus::Voice));
    }
    v
}

fn census(cat: &SoundCatalog) -> Result<(), String> {
    println!("index rows {}, species with cries {}", cat.rows.len(), cat.species_with_cries().len());
    println!("cache root {}", cat.cache_root.display());
    let (mut ok, mut silent) = (0, 0);
    for (label, ev, policy, _) in port_sounds(cat, false) {
        let takes = cat.playable(&ev).len();
        let pick = cat.pick(&ev, policy, 0);
        let file = pick.and_then(|r| cat.file_for(r));
        match (&pick, &file) {
            (Some(r), Some(f)) => {
                ok += 1;
                println!(
                    "{label:<18} {ev:<36} takes {takes:>2}  pick {} {:>7.3}s  {}",
                    r.entry,
                    r.duration_s,
                    if f.ogg { "ogg" } else { "wav" }
                );
            }
            _ => {
                silent += 1;
                println!("{label:<18} {ev:<36} takes {takes:>2}  SILENT ({})", if pick.is_some() { "file missing" } else { "no playable media" });
            }
        }
    }
    println!("resolved {ok}, silent {silent}");
    Ok(())
}

struct Decoded {
    channels: u16,
    rate: u32,
    samples: Vec<f32>,
}

fn decode_file(path: &Path) -> Result<Decoded, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if !decodable_header(&bytes) {
        return Err("not WAV/Ogg Vorbis".into());
    }
    let src = AudioSource { bytes: bytes.into() };
    let dec = std::panic::catch_unwind(|| src.decoder()).map_err(|_| "decoder panicked".to_string())?;
    let channels = dec.channels().get();
    let rate = dec.sample_rate().get();
    let samples: Vec<f32> = dec.collect();
    Ok(Decoded { channels, rate, samples })
}

fn decode(cat: &SoundCatalog, all_cries: bool) -> Result<(), String> {
    let (mut pass, mut fail) = (0, 0);
    let mut mixed: Vec<(Bus, Decoded)> = Vec::new();
    for (label, ev, policy, bus) in port_sounds(cat, all_cries) {
        let Some(row) = cat.pick(&ev, policy, 0) else { continue };
        let Some(file) = cat.file_for(row) else {
            println!("{label:<18} file missing");
            continue;
        };
        match decode_file(&file.path) {
            Ok(d) => {
                let frames = d.samples.len() / d.channels.max(1) as usize;
                let secs = frames as f32 / d.rate.max(1) as f32;
                let finite = d.samples.iter().all(|x| x.is_finite());
                let peak = d.samples.iter().fold(0.0f32, |a, x| a.max(x.abs()));
                // Lewton (Bevy's Ogg Vorbis decoder) does not trim the last block to the Ogg
                // granule position, so an Ogg file may run up to one long block (2048 samples)
                // past the declared length; the WAV is exact.
                let slack = if file.ogg { 2048.0 / d.rate.max(1) as f32 } else { 0.0 };
                let len_ok = secs >= row.duration_s * 0.99 - 0.002 && secs <= row.duration_s * 1.01 + 0.002 + slack;
                if file.ogg {
                    let wav = file.path.with_extension("wav");
                    if let Ok(w) = decode_file(&wav) {
                        let tail = &d.samples[w.samples.len().min(d.samples.len())..];
                        let tail_rms = (tail.iter().map(|x| x * x).sum::<f32>() / tail.len().max(1) as f32).sqrt();
                        let common = w.samples.len().min(d.samples.len());
                        let err = w.samples[..common]
                            .iter()
                            .zip(&d.samples[..common])
                            .fold(0.0f32, |a, (x, y)| a.max((x - y).abs()));
                        println!(
                            "{:<18} ogg vs wav: extra samples {} (tail rms {tail_rms:.5}), max |diff| on common part {err:.4}",
                            "",
                            d.samples.len() as i64 - w.samples.len() as i64
                        );
                    }
                }
                let good = finite && len_ok && peak <= 1.0 && peak > 0.0;
                if good {
                    pass += 1;
                } else {
                    fail += 1;
                }
                println!(
                    "{label:<18} {} {} ch {} Hz {secs:>8.3}s (index {:>8.3}s) peak {peak:.3} {}",
                    if file.ogg { "ogg" } else { "wav" },
                    d.channels,
                    d.rate,
                    row.duration_s,
                    if good { "ok" } else { "FAIL" }
                );
                if mixed.len() < 4 && matches!(label.as_str(), "UiCursor" | "HitNormal" | "Area(0)" | "cry 25") {
                    mixed.push((bus, d));
                }
            }
            Err(e) => {
                fail += 1;
                println!("{label:<18} {} DECODE ERROR {e}", file.path.display());
            }
        }
    }
    println!("decoded ok {pass}, failed {fail}");
    // Offline mix of a few sounds at the bus volumes of a sample settings file (mono sum of
    // channel 0, first 2 s at each file's own rate; a numeric check of the gain math only).
    let s = UserSettings::parse("master_volume=80\nmusic_volume=50\nsfx_volume=100\nvoice_volume=90\n");
    let vol = BusVolumes::from_settings(&s);
    let n = 96_000;
    let mut mix = vec![0.0f32; n];
    for (bus, d) in &mixed {
        let g = vol.output(*bus, 1.0, 1.0);
        let ch = d.channels.max(1) as usize;
        for (i, m) in mix.iter_mut().enumerate() {
            if let Some(x) = d.samples.get(i * ch) {
                *m += x * g;
            }
        }
    }
    let peak = mix.iter().fold(0.0f32, |a, x| a.max(x.abs()));
    let rms = (mix.iter().map(|x| x * x).sum::<f32>() / n as f32).sqrt();
    println!(
        "offline mix of {} sounds (master 80, music 50, sfx 100, voice 90): peak {peak:.3} rms {rms:.4}",
        mixed.len()
    );
    if fail > 0 {
        return Err(format!("{fail} files failed"));
    }
    Ok(())
}

/// Candidate names tried against the bank event ids (only matches are written).
fn candidates() -> Vec<String> {
    let mut v = Vec::new();
    for a in 0..=9 {
        for suf in ["", "_DAY", "_NIGHT", "_MORNING", "_EVENING"] {
            v.push(format!("PLAY_BGM_HA_FI_AREA{a:02}{suf}"));
        }
    }
    v.push("PLAY_BGM_HA_FI_CMN_NIGHT".into());
    v.push("PLAY_BGM_FIELD".into());
    v
}

fn events(data: &Path, write: Option<PathBuf>) -> Result<(), String> {
    let bin = std::env::var_os("PLA_BIN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin"));
    let dir = bin.join("sound/NX64/wwise/bank");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "bnk"))
        .collect();
    paths.sort();
    let mut g = Graph::default();
    for p in &paths {
        let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
        let b = Bank::parse(&bytes).map_err(|e| format!("{}: {e}", p.display()))?;
        let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
        g.add_bank(&name, &bytes, &b);
    }
    g.finish();
    let events: std::collections::HashSet<u32> = g.events().into_iter().collect();
    // Sanity: the method finds a name that is known to be right.
    if !events.contains(&fnv1_lower("PLAY_BGM_HA_FI_CMN_WORMHOLE")) {
        return Err("FNV check failed: PLAY_BGM_HA_FI_CMN_WORMHOLE not among the events".into());
    }
    let cat = SoundCatalog::load(data, default_cache_root())?;
    let mut by_wem: HashMap<u32, Vec<&str>> = HashMap::new();
    for r in &cat.rows {
        by_wem.entry(r.wem_id).or_default().push(&r.entry);
    }
    let tried = candidates();
    let mut out = String::from(
        "# sheet: data/audio_event_media\n# version: 1\n# generator: crates/pla/examples/audio_check.rs events\n\
         # target: Pokémon Legends: Arceus update v262144 Wwise events whose media carry another name in data/audio_index.tsv (names found by FNV-1 hash of candidate names; media through the HIRC graph)\n\
         # requires: bin/sound/NX64/wwise/bank/*.bnk; data/audio_index.tsv\n\
         event:string*\tevent_id:u32\tbank:string\tmedia_count:u16\theuristic:bool\tmedia:string\tname_confidence:enum\n",
    );
    let mut found = 0;
    for name in &tried {
        let id = fnv1_lower(name);
        if !events.contains(&id) {
            continue;
        }
        found += 1;
        let r = g.resolve(id);
        let mut entries: Vec<&str> = r
            .sources
            .iter()
            .flat_map(|s| by_wem.get(&s.media_id).cloned().unwrap_or_default())
            .collect();
        entries.sort();
        entries.dedup();
        let bank = g.bank_of(id).unwrap_or("").to_string();
        println!("{name:<36} id {id:>10} bank {bank:<10} media {:>3} entries {:>3} heuristic {}", r.sources.len(), entries.len(), r.heuristic);
        out.push_str(&format!(
            "{name}\t{id}\t{bank}\t{}\t{}\t{}\thash_guess\n",
            entries.len(),
            u8::from(r.heuristic),
            entries.join(",")
        ));
    }
    println!("tried {} names, {found} are events", tried.len());
    if let Some(w) = write {
        std::fs::write(&w, out).map_err(|e| format!("{}: {e}", w.display()))?;
        println!("wrote {}", w.display());
    }
    Ok(())
}
