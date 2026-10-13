//! Sound catalogue: event name -> decoded media files, from `data/audio_index.tsv` plus the
//! extra event table `data/audio_event_media.tsv`.
//!
//! The index has one row per decoded media (`<bank>/<wem_id>`) with the name of the first
//! named event that reaches it; the extra table lists events whose media carry another name in
//! the index (area field music). Files live in the decode cache
//! (`$PLA_CACHE_ROOT/audio/<bank>/<wem_id>.wav`, plus `.ogg` for Vorbis-source media, which the
//! catalogue prefers because it is about 20 times smaller). Nothing is read here except the two
//! tables; files are only checked for existence when a sound is resolved.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::events::VariantPolicy;

/// One decoded media row of the index.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaRow {
    /// `<bank>/<wem_id>` (index key).
    pub entry: String,
    pub wem_id: u32,
    pub codec: String,
    pub duration_s: f32,
    pub rms: f32,
    /// `ok`, `silent`, `blocked` or `prefetch`.
    pub status: String,
    /// WAV path relative to the repository root (`work/cache/audio/...`), empty when none.
    pub wav_rel: String,
    pub category: String,
    pub name: String,
    pub species: Option<u16>,
}

impl MediaRow {
    /// Decoded to a non-silent file (`status = ok` and a WAV path recorded).
    pub fn playable(&self) -> bool {
        self.status == "ok" && !self.wav_rel.is_empty()
    }
}

/// Where a media file is on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaFile {
    pub path: PathBuf,
    /// True for the Ogg Vorbis rewrap, false for the 16-bit WAV.
    pub ogg: bool,
}

/// Event name lookups over the index.
#[derive(Debug, Clone, Default)]
pub struct SoundCatalog {
    pub rows: Vec<MediaRow>,
    /// lower-case event name -> row indices (index names plus the extra event table).
    by_event: HashMap<String, Vec<usize>>,
    by_entry: HashMap<String, usize>,
    /// species -> row indices of `pokemon_voice` rows.
    by_species: HashMap<u16, Vec<usize>>,
    /// Root of the decode cache (`.../work/cache`).
    pub cache_root: PathBuf,
}

fn columns(header: &str) -> HashMap<String, usize> {
    header
        .split('\t')
        .enumerate()
        .map(|(i, c)| (c.split(':').next().unwrap_or("").to_string(), i))
        .collect()
}

/// Data lines of a sheet: skips `#` comments and returns (header, rows).
fn sheet_lines(text: &str) -> Option<(&str, impl Iterator<Item = &str>)> {
    let mut lines = text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty());
    let header = lines.next()?;
    Some((header, lines))
}

impl SoundCatalog {
    pub const INDEX_FILE: &'static str = "audio_index.tsv";
    pub const EXTRA_FILE: &'static str = "audio_event_media.tsv";

    /// Parse the index text (and optionally the extra event table text).
    pub fn parse(index: &str, extra: Option<&str>, cache_root: PathBuf) -> Result<Self, String> {
        let (header, lines) = sheet_lines(index).ok_or("audio index is empty")?;
        let col = columns(header);
        let need = |n: &str| col.get(n).copied().ok_or_else(|| format!("audio index has no column {n}"));
        let (c_entry, c_wem, c_codec, c_dur, c_rms, c_status, c_wav, c_cat, c_name, c_sp) = (
            need("entry")?,
            need("wem_id")?,
            need("codec")?,
            need("duration_s")?,
            need("rms")?,
            need("status")?,
            need("wav_rel")?,
            need("category")?,
            need("name")?,
            need("species")?,
        );
        let mut cat = SoundCatalog { cache_root, ..Default::default() };
        for line in lines {
            let f: Vec<&str> = line.split('\t').collect();
            let get = |i: usize| f.get(i).copied().unwrap_or("");
            let row = MediaRow {
                entry: get(c_entry).to_string(),
                wem_id: get(c_wem).parse().unwrap_or(0),
                codec: get(c_codec).to_string(),
                duration_s: get(c_dur).parse().unwrap_or(0.0),
                rms: get(c_rms).parse().unwrap_or(0.0),
                status: get(c_status).to_string(),
                wav_rel: get(c_wav).to_string(),
                category: get(c_cat).to_string(),
                name: get(c_name).to_string(),
                species: get(c_sp).parse().ok(),
            };
            let i = cat.rows.len();
            if !row.name.is_empty() {
                cat.by_event.entry(row.name.to_ascii_lowercase()).or_default().push(i);
            }
            if let (Some(sp), "pokemon_voice") = (row.species, row.category.as_str()) {
                cat.by_species.entry(sp).or_default().push(i);
            }
            cat.by_entry.insert(row.entry.clone(), i);
            cat.rows.push(row);
        }
        if let Some(extra) = extra {
            cat.add_extra(extra)?;
        }
        Ok(cat)
    }

    /// Add the `event -> media entries` table (`event`, `media` = comma-separated entries).
    fn add_extra(&mut self, text: &str) -> Result<(), String> {
        let Some((header, lines)) = sheet_lines(text) else { return Ok(()) };
        let col = columns(header);
        let (Some(&c_ev), Some(&c_media)) = (col.get("event"), col.get("media")) else {
            return Err("audio event table needs columns event and media".into());
        };
        for line in lines {
            let f: Vec<&str> = line.split('\t').collect();
            let ev = f.get(c_ev).copied().unwrap_or("").to_ascii_lowercase();
            if ev.is_empty() {
                continue;
            }
            for entry in f.get(c_media).copied().unwrap_or("").split(',').filter(|e| !e.is_empty()) {
                if let Some(&i) = self.by_entry.get(entry) {
                    let v = self.by_event.entry(ev.clone()).or_default();
                    if !v.contains(&i) {
                        v.push(i);
                    }
                }
            }
        }
        Ok(())
    }

    /// Load `<data_dir>/audio_index.tsv` (required) and `<data_dir>/audio_event_media.tsv`
    /// (optional).
    pub fn load(data_dir: &Path, cache_root: PathBuf) -> Result<Self, String> {
        let p = data_dir.join(Self::INDEX_FILE);
        let index = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let extra = std::fs::read_to_string(data_dir.join(Self::EXTRA_FILE)).ok();
        Self::parse(&index, extra.as_deref(), cache_root)
    }

    /// Every row of an event (case-insensitive name), playable or not.
    pub fn event_rows(&self, event: &str) -> Vec<&MediaRow> {
        self.by_event
            .get(&event.to_ascii_lowercase())
            .map(|v| v.iter().map(|&i| &self.rows[i]).collect())
            .unwrap_or_default()
    }

    /// Playable rows of an event, one per distinct WEM id (the same media is often copied into
    /// several banks), ordered by WEM id.
    pub fn playable(&self, event: &str) -> Vec<&MediaRow> {
        let mut v: Vec<&MediaRow> = self.event_rows(event).into_iter().filter(|r| r.playable()).collect();
        v.sort_by_key(|r| (r.wem_id, r.entry.clone()));
        v.dedup_by_key(|r| r.wem_id);
        v
    }

    /// True when the event name is known (with or without playable media).
    pub fn has_event(&self, event: &str) -> bool {
        self.by_event.contains_key(&event.to_ascii_lowercase())
    }

    /// Pick one playable take of `event`. `roll` is any random number (used by
    /// [`VariantPolicy::Random`]).
    pub fn pick(&self, event: &str, policy: VariantPolicy, roll: u32) -> Option<&MediaRow> {
        let v = self.playable(event);
        pick_variant(&v, policy, roll)
    }

    /// Cry event of a species: `PLAY_PV_<sp>_<form>_00`, else form 00, else any voice row of the
    /// species (lowest event name). Returns the event name that has playable media.
    pub fn cry_event(&self, species: u16, form: u8) -> Option<String> {
        use super::events::cry_event_name;
        for f in [form, 0] {
            let n = cry_event_name(species, f, 0);
            if !self.playable(&n).is_empty() {
                return Some(n);
            }
        }
        let mut names: Vec<&str> = self
            .by_species
            .get(&species)?
            .iter()
            .map(|&i| &self.rows[i])
            .filter(|r| r.playable() && r.name.to_ascii_uppercase().starts_with("PLAY_PV_"))
            .map(|r| r.name.as_str())
            .collect();
        names.sort();
        names.first().map(|s| s.to_string())
    }

    /// Species with at least one playable cry row.
    pub fn species_with_cries(&self) -> Vec<u16> {
        let mut v: Vec<u16> = self
            .by_species
            .iter()
            .filter(|(_, rows)| rows.iter().any(|&i| self.rows[i].playable()))
            .map(|(&s, _)| s)
            .collect();
        v.sort();
        v
    }

    /// File candidates for a row: the `.ogg` next to the WAV first (Vorbis-source media only),
    /// then the WAV. Paths are rebased from `work/cache/` onto [`Self::cache_root`].
    pub fn candidates(&self, row: &MediaRow) -> Vec<MediaFile> {
        if row.wav_rel.is_empty() {
            return Vec::new();
        }
        let rel = row.wav_rel.strip_prefix("work/cache/").unwrap_or(&row.wav_rel);
        let wav = self.cache_root.join(rel);
        let mut out = Vec::new();
        if row.codec == "wwise_vorbis" {
            out.push(MediaFile { path: wav.with_extension("ogg"), ogg: true });
        }
        out.push(MediaFile { path: wav, ogg: false });
        out
    }

    /// First candidate that exists on disk.
    pub fn file_for(&self, row: &MediaRow) -> Option<MediaFile> {
        self.candidates(row).into_iter().find(|c| c.path.is_file())
    }
}

/// Variant choice over playable rows (see [`VariantPolicy`]).
pub fn pick_variant<'a>(rows: &[&'a MediaRow], policy: VariantPolicy, roll: u32) -> Option<&'a MediaRow> {
    if rows.is_empty() {
        return None;
    }
    let by = |a: &&&MediaRow, b: &&&MediaRow| a.rms.total_cmp(&b.rms).then(b.wem_id.cmp(&a.wem_id));
    match policy {
        VariantPolicy::Random => Some(rows[roll as usize % rows.len()]),
        VariantPolicy::Loudest => rows.iter().max_by(by).copied(),
        VariantPolicy::Longest => rows
            .iter()
            .max_by(|a, b| a.duration_s.total_cmp(&b.duration_s).then(by(a, b)))
            .copied(),
    }
}

/// True when `bytes` start like a file the port's decoders accept: RIFF/WAVE or Ogg with a
/// Vorbis identification header. Bevy's decoder panics on anything else, so a file failing this
/// check is never handed to it.
pub fn decodable_header(bytes: &[u8]) -> bool {
    let wav = bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE";
    let ogg = bytes.len() >= 36 && &bytes[0..4] == b"OggS" && &bytes[28..35] == b"\x01vorbis";
    wav || ogg
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &str = "# sheet: data/audio_index\n\
entry:string*\tbank:string\twem_id:u32\tsource:enum\tcodec:string\tchannels:u8\tsample_rate:u32\tduration_s:f32?\tsize_bytes:u32\tstatus:enum\treason:string?\tpeak:f32?\trms:f32?\tcentroid_hz:f32?\twav_rel:string?\tevent_count:u16\tcategory:string?\tname:string?\tname_confidence:enum\tspecies:u16?\n\
UI/1\tUI\t1\tbank\twwise_vorbis\t1\t48000\t0.2\t10\tok\t\t0.5\t0.010\t1\twork/cache/audio/UI/1.wav\t1\tui\tPlay_UI_common_select\thash_string\t\n\
UI/2\tUI\t2\tbank\twwise_vorbis\t1\t48000\t0.1\t10\tok\t\t0.5\t0.070\t1\twork/cache/audio/UI/2.wav\t1\tui\tPlay_UI_common_select\thash_string\t\n\
X/2\tX\t2\tbank\twwise_vorbis\t1\t48000\t0.1\t10\tok\t\t0.5\t0.070\t1\twork/cache/audio/X/2.wav\t1\tui\tPlay_UI_common_select\thash_string\t\n\
Default.pck/3\tDefault.pck\t3\tpck\twwise_opus_3039\t2\t48000\t150.0\t10\tok\t\t0.5\t0.030\t1\twork/cache/audio/Default_pck/3.wav\t1\tbgm\tPLAY_BGM_HA_FI_AREA00\thash_string\t\n\
Default.pck/4\tDefault.pck\t4\tpck\twwise_opus_3039\t2\t48000\t20.0\t10\tok\t\t0.5\t0.090\t1\twork/cache/audio/Default_pck/4.wav\t1\tbgm\tPLAY_BGM_HA_FI_AREA00\thash_string\t\n\
Default.pck/5\tDefault.pck\t5\tpck\ttag_8311\t1\t48000\t1.0\t10\tblocked\t\t\t\t\t\t1\tother_sfx\tPLAY_EW0071_02\thash_string\t\n\
Default.pck/6\tDefault.pck\t6\tpck\twwise_vorbis\t1\t48000\t0.9\t10\tok\t\t0.5\t0.030\t1\twork/cache/audio/Default_pck/6.wav\t1\tpokemon_voice\tPLAY_PV_399_00_01\thash_template\t399\n";

    const EXTRA: &str = "# sheet: data/audio_event_media\nevent:string*\tevent_id:u32\tbank:string\tmedia:string\tname_confidence:enum\n\
PLAY_BGM_HA_FI_AREA01_DAY\t1\tBGM\tDefault.pck/3,Default.pck/999\thash_guess\n";

    fn cat() -> SoundCatalog {
        SoundCatalog::parse(INDEX, Some(EXTRA), PathBuf::from("/cache")).unwrap()
    }

    #[test]
    fn lookup_is_case_insensitive_and_dedups_copies() {
        let c = cat();
        let v = c.playable("PLAY_UI_COMMON_SELECT");
        assert_eq!(v.iter().map(|r| r.wem_id).collect::<Vec<_>>(), vec![1, 2]);
        assert!(c.has_event("play_ui_common_select"));
        assert!(c.playable("PLAY_EW0071_02").is_empty());
        assert!(c.has_event("PLAY_EW0071_02"));
        assert!(c.playable("NO_SUCH_EVENT").is_empty());
    }

    #[test]
    fn variant_policies() {
        let c = cat();
        assert_eq!(c.pick("Play_UI_common_select", VariantPolicy::Loudest, 0).unwrap().wem_id, 2);
        assert_eq!(c.pick("PLAY_BGM_HA_FI_AREA00", VariantPolicy::Longest, 0).unwrap().wem_id, 3);
        assert_eq!(c.pick("Play_UI_common_select", VariantPolicy::Random, 3).unwrap().wem_id, 2);
        assert_eq!(c.pick("Play_UI_common_select", VariantPolicy::Random, 4).unwrap().wem_id, 1);
        assert!(c.pick("PLAY_EW0071_02", VariantPolicy::Random, 0).is_none());
    }

    #[test]
    fn extra_table_adds_events_and_skips_unknown_entries() {
        let c = cat();
        let v = c.playable("PLAY_BGM_HA_FI_AREA01_DAY");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].wem_id, 3);
    }

    #[test]
    fn cry_fallbacks() {
        let c = cat();
        assert_eq!(c.cry_event(399, 0).as_deref(), Some("PLAY_PV_399_00_01"));
        assert_eq!(c.cry_event(25, 0), None);
        assert_eq!(c.species_with_cries(), vec![399]);
    }

    #[test]
    fn files_prefer_ogg_for_vorbis_and_rebase_on_cache_root() {
        let c = cat();
        let r = &c.playable("Play_UI_common_select")[0];
        let f = c.candidates(r);
        assert_eq!(f[0], MediaFile { path: PathBuf::from("/cache/audio/UI/1.ogg"), ogg: true });
        assert_eq!(f[1], MediaFile { path: PathBuf::from("/cache/audio/UI/1.wav"), ogg: false });
        let m = &c.playable("PLAY_BGM_HA_FI_AREA00")[0];
        assert_eq!(c.candidates(m).len(), 1, "opus-source media have no ogg");
        assert_eq!(c.file_for(r), None, "missing files resolve to nothing");
    }

    #[test]
    fn header_check() {
        let mut wav = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        assert!(decodable_header(&wav));
        wav[8] = b'X';
        assert!(!decodable_header(&wav));
        let mut ogg = vec![0u8; 40];
        ogg[0..4].copy_from_slice(b"OggS");
        ogg[28..35].copy_from_slice(b"\x01vorbis");
        assert!(decodable_header(&ogg));
        ogg[29] = b'o';
        assert!(!decodable_header(&ogg));
        assert!(!decodable_header(b""));
    }

    /// Real index: `[skip]` when the repository data is missing.
    #[test]
    fn real_index_known_cries_and_events() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let Ok(c) = SoundCatalog::load(&data, PathBuf::from("/nonexistent")) else {
            eprintln!("[skip] data/audio_index.tsv missing");
            return;
        };
        assert_eq!(c.rows.len(), 11_970);
        // 25 well-known species have a playable PLAY_PV_<sp>_00_00 cry.
        let known: [u16; 25] = [
            25, 133, 399, 396, 403, 406, 412, 418, 425, 427, 431, 433, 434, 436, 438, 440, 441,
            442, 443, 446, 447, 449, 451, 453, 456,
        ];
        for sp in known {
            assert_eq!(c.cry_event(sp, 0), Some(format!("PLAY_PV_{sp:03}_00_00")), "species {sp}");
        }
        assert!(c.species_with_cries().len() >= 242);
        for s in super::super::events::SfxId::ALL {
            assert!(!c.playable(s.event_name()).is_empty(), "{s:?} -> {} has no playable media", s.event_name());
        }
        for m in [
            super::super::events::MusicId::Area(0),
            super::super::events::MusicId::Area(1),
            super::super::events::MusicId::Area(2),
            super::super::events::MusicId::Area(3),
            super::super::events::MusicId::Area(4),
            super::super::events::MusicId::Area(5),
            super::super::events::MusicId::Area(6),
            super::super::events::MusicId::FieldNight,
            super::super::events::MusicId::WildBattle,
            super::super::events::MusicId::TrainerBattle,
        ] {
            assert!(!c.playable(m.event_name().unwrap()).is_empty(), "{m:?}");
        }
    }
}
