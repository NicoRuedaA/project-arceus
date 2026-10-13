//! Own versioned player save for the Bevy port (NOT the original game's save format; the
//! original's host save API emulation lives in `crate::save` and is unrelated).
//!
//! Format: strict line-based text, no extra dependencies.
//!
//! ```text
//! PLASAVE 4
//! CHECKSUM 0123456789abcdef      <- FNV-1a 64 over every byte after this line
//! key=value
//! ...
//! END
//! ```
//!
//! v3 adds `box` lines (the stored Pokemon; the key is lowercase like `member`). v4 adds
//! `play_seconds` (whole seconds of played time). Older files load via the migration chain: a v2
//! file gets an empty box, a v1-v3 file a play time of 0.
//!
//! Everything here is pure logic (no Bevy systems); [`SaveGame`] is the small Bevy resource.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use bevy::prelude::Resource;

use crate::battle::{BattleMove, BattleSpecies};
use crate::battle_engine::items::{ItemBag, ItemTable};
use crate::battle_engine::{Combatant, MajorStatus};

/// Version written by this build.
pub const CURRENT_VERSION: u32 = 4;
/// Maximum party size.
pub const MAX_PARTY: usize = 6;
/// Maximum moves per party member.
pub const MAX_MOVES: usize = 4;
/// Maximum stored (non-party) Pokemon. The real game's box capacity is **unknown**; 30 boxes of
/// 32 slots (960) is the documented, reasonable ceiling this port uses.
pub const MAX_BOX: usize = 960;
const MAGIC: &str = "PLASAVE";
const FILE_NAME: &str = "save.plasave";
const MAX_FILE_BYTES: u64 = 1 << 20;

// ---------------------------------------------------------------- errors

/// Friendly, non-panicking load/save errors.
#[derive(Debug)]
pub enum SaveError {
    Io(String),
    NotFound,
    Truncated,
    BadHeader(String),
    UnknownVersion(u32),
    ChecksumMismatch { expected: u64, actual: u64 },
    Parse { line: usize, msg: String },
    Invalid(String),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(m) => write!(f, "could not read or write the save file: {m}"),
            Self::NotFound => write!(f, "no save file found"),
            Self::Truncated => write!(f, "the save file is incomplete (cut short); try the .bak backup"),
            Self::BadHeader(m) => write!(f, "this is not a valid save file ({m})"),
            Self::UnknownVersion(v) => write!(
                f,
                "the save was written by a different version (v{v}); this build reads up to v{CURRENT_VERSION}"
            ),
            Self::ChecksumMismatch { .. } => {
                write!(f, "the save file is damaged (checksum mismatch); try the .bak backup")
            }
            Self::Parse { line, msg } => write!(f, "the save file is damaged at line {line}: {msg}"),
            Self::Invalid(m) => write!(f, "the save contents are not valid: {m}"),
        }
    }
}
impl std::error::Error for SaveError {}

// ---------------------------------------------------------------- data

/// Persistent status condition (mirrors `MajorStatus`, stable text names).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavedStatus {
    Drowsy,
    Paralysis,
    Poison,
    Burn,
    Frostbite,
}

impl SavedStatus {
    fn tag(self) -> &'static str {
        match self {
            Self::Drowsy => "drowsy",
            Self::Paralysis => "paralysis",
            Self::Poison => "poison",
            Self::Burn => "burn",
            Self::Frostbite => "frostbite",
        }
    }
    fn parse(s: &str) -> Option<Option<Self>> {
        Some(Some(match s {
            "-" => return Some(None),
            "drowsy" => Self::Drowsy,
            "paralysis" => Self::Paralysis,
            "poison" => Self::Poison,
            "burn" => Self::Burn,
            "frostbite" => Self::Frostbite,
            _ => return None,
        }))
    }
}

impl From<MajorStatus> for SavedStatus {
    fn from(s: MajorStatus) -> Self {
        match s {
            MajorStatus::Drowsy => Self::Drowsy,
            MajorStatus::Paralysis => Self::Paralysis,
            MajorStatus::Poison => Self::Poison,
            MajorStatus::Burn => Self::Burn,
            MajorStatus::Frostbite => Self::Frostbite,
        }
    }
}
impl From<SavedStatus> for MajorStatus {
    fn from(s: SavedStatus) -> Self {
        match s {
            SavedStatus::Drowsy => Self::Drowsy,
            SavedStatus::Paralysis => Self::Paralysis,
            SavedStatus::Poison => Self::Poison,
            SavedStatus::Burn => Self::Burn,
            SavedStatus::Frostbite => Self::Frostbite,
        }
    }
}

/// One known move with its remaining PP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedMove {
    pub move_id: u16,
    pub pp: u8,
}

/// One party member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMember {
    /// National dex id.
    pub species: u16,
    pub form: u8,
    pub level: u8,
    pub hp: u32,
    pub status: Option<SavedStatus>,
    pub moves: Vec<SavedMove>,
    /// Effort level per stat (HP/Atk/Def/SpA/SpD/Spe), as in `Combatant::effort`.
    pub effort: [u8; 6],
    pub nickname: Option<String>,
}

impl PartyMember {
    /// Snapshot a live combatant. Engine limits: `Combatant` has no form or nickname, so
    /// `form` is 0 and `nickname` None (set them afterwards if the game tracks them).
    pub fn from_combatant(c: &Combatant) -> Self {
        Self {
            species: c.species.dex,
            form: 0,
            level: c.level,
            hp: c.hp,
            status: c.status.map(Into::into),
            moves: c
                .moves
                .iter()
                .take(MAX_MOVES)
                .map(|m| SavedMove { move_id: m.data.id, pp: m.pp })
                .collect(),
            effort: c.effort,
            nickname: None,
        }
    }

    /// Rebuild a combatant. The caller supplies species data and move data (looked up by
    /// `species` / each `move_id`), because the engine's tables are not owned by the save.
    /// `nature_percent` is caller-supplied (engine does not model the nature table).
    /// Stats are recomputed from level + effort; HP, PP and status come from the save.
    pub fn to_combatant(
        &self,
        species: BattleSpecies,
        moves: Vec<BattleMove>,
        nature_percent: [u16; 6],
    ) -> Combatant {
        let mut c = Combatant::with_effort_nature(species, self.level, moves, self.effort, nature_percent);
        c.hp = self.hp.min(c.max_hp);
        c.status = self.status.map(Into::into);
        for (slot, saved) in c.moves.iter_mut().zip(&self.moves) {
            if slot.data.id == saved.move_id {
                slot.pp = saved.pp.min(slot.data.pp);
            }
        }
        c
    }
}

/// Everything the player keeps between sessions.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerSave {
    pub version: u32,
    /// Zone / area id, e.g. `ha_area01`. No whitespace.
    pub zone: String,
    pub position: [f32; 3],
    pub yaw: f32,
    pub time_slot: u8,
    pub party: Vec<PartyMember>,
    /// Pokemon caught while the party was full. Persisted as `box` lines (format v3, see [`MAX_BOX`]).
    pub box_pokemon: Vec<PartyMember>,
    /// item id -> quantity (zero entries are dropped).
    pub bag: BTreeMap<u16, u16>,
    pub money: u32,
    pub seen: BTreeSet<u16>,
    pub caught: BTreeSet<u16>,
    pub defeated: u32,
    pub caught_count: u32,
    pub rng_seed: u32,
    /// Whole seconds played while in the world or a battle, not paused (format v4). How the
    /// original game counts play time is **unknown**; this is wall-clock time of the port.
    pub play_seconds: u64,
}

impl Default for PlayerSave {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            zone: "ha_area00".into(),
            position: [0.0; 3],
            yaw: 0.0,
            time_slot: 0,
            party: Vec::new(),
            box_pokemon: Vec::new(),
            bag: BTreeMap::new(),
            money: 0,
            seen: BTreeSet::new(),
            caught: BTreeSet::new(),
            defeated: 0,
            caught_count: 0,
            rng_seed: 1,
            play_seconds: 0,
        }
    }
}

impl PlayerSave {
    /// Validate invariants (called before writing and after parsing).
    pub fn validate(&self) -> Result<(), SaveError> {
        let bad = |m: String| Err(SaveError::Invalid(m));
        if self.zone.is_empty() || self.zone.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return bad(format!("zone id {:?} must be non-empty without whitespace", self.zone));
        }
        if self.party.len() > MAX_PARTY {
            return bad(format!("party has {} members (max {MAX_PARTY})", self.party.len()));
        }
        if self.box_pokemon.len() > MAX_BOX {
            return bad(format!("box has {} Pokemon (max {MAX_BOX})", self.box_pokemon.len()));
        }
        if !self.position.iter().chain([&self.yaw]).all(|v| v.is_finite()) {
            return bad("position/yaw must be finite numbers".into());
        }
        for (i, m) in self.party.iter().enumerate() {
            if m.level == 0 || m.moves.len() > MAX_MOVES {
                return bad(format!("party member {i}: level 0 or more than {MAX_MOVES} moves"));
            }
        }
        for (i, m) in self.box_pokemon.iter().enumerate() {
            if m.level == 0 || m.moves.len() > MAX_MOVES {
                return bad(format!("box member {i}: level 0 or more than {MAX_MOVES} moves"));
            }
        }
        Ok(())
    }

    /// Bag from the engine bag. The engine's `ItemBag` has no iterator, so the caller passes the
    /// candidate item ids (e.g. every id of the `ItemTable`).
    pub fn bag_from_engine(bag: &ItemBag, ids: impl IntoIterator<Item = u16>) -> BTreeMap<u16, u16> {
        ids.into_iter()
            .filter_map(|id| Some((id, bag.count(id))).filter(|(_, n)| *n > 0))
            .collect()
    }

    /// Rebuild an engine bag, clamped to each item's stack ceiling. Unknown ids are skipped and
    /// returned so the caller can warn.
    pub fn bag_to_engine(&self, table: &ItemTable) -> (ItemBag, Vec<u16>) {
        let mut bag = ItemBag::default();
        let mut skipped = Vec::new();
        for (&id, &qty) in &self.bag {
            if bag.add(table, id, qty).is_err() {
                skipped.push(id);
            }
        }
        (bag, skipped)
    }

    pub fn encode(&self) -> Result<String, SaveError> {
        self.validate()?;
        let mut b = String::new();
        let mut kv = |k: &str, v: String| {
            b.push_str(k);
            b.push('=');
            b.push_str(&v);
            b.push('\n');
        };
        kv("zone", self.zone.clone());
        kv("pos", format!("{},{},{}", self.position[0], self.position[1], self.position[2]));
        kv("yaw", self.yaw.to_string());
        kv("time_slot", self.time_slot.to_string());
        kv("money", self.money.to_string());
        kv("defeated", self.defeated.to_string());
        kv("caught_count", self.caught_count.to_string());
        kv("rng_seed", self.rng_seed.to_string());
        kv("play_seconds", self.play_seconds.to_string());
        kv("seen", join_set(&self.seen));
        kv("caught", join_set(&self.caught));
        kv(
            "bag",
            self.bag
                .iter()
                .filter(|(_, n)| **n > 0)
                .map(|(i, n)| format!("{i}:{n}"))
                .collect::<Vec<_>>()
                .join(","),
        );
        for m in &self.party {
            kv("member", member_line(m));
        }
        for m in &self.box_pokemon {
            kv("box", member_line(m));
        }
        let sum = fnv1a64(b.as_bytes());
        // `END` sits inside the checksummed body so truncation is always detected.
        b.push_str("END\n");
        let sum = fnv1a64_continue(sum, b"END\n");
        Ok(format!("{MAGIC} {}\nCHECKSUM {:016x}\n{b}", self.version, sum))
    }

    /// Strict decode: header, checksum, migration, then field parsing.
    pub fn decode(text: &str) -> Result<Self, SaveError> {
        let mut parts = text.splitn(3, '\n');
        let head = parts.next().unwrap_or("");
        let sumline = parts.next().ok_or(SaveError::Truncated)?;
        let body = parts.next().ok_or(SaveError::Truncated)?;
        let version = match head.split_once(' ') {
            Some((MAGIC, v)) => v.trim().parse::<u32>().map_err(|_| SaveError::BadHeader("bad version number".into()))?,
            _ => return Err(SaveError::BadHeader("missing PLASAVE header".into())),
        };
        if version == 0 || version > CURRENT_VERSION {
            return Err(SaveError::UnknownVersion(version));
        }
        let expected = sumline
            .strip_prefix("CHECKSUM ")
            .and_then(|h| u64::from_str_radix(h.trim(), 16).ok())
            .ok_or_else(|| SaveError::BadHeader("missing checksum line".into()))?;
        if !body.ends_with("END\n") {
            return Err(SaveError::Truncated);
        }
        let actual = fnv1a64(body.as_bytes());
        if actual != expected {
            return Err(SaveError::ChecksumMismatch { expected, actual });
        }
        let lines: Vec<String> = body.lines().filter(|l| *l != "END").map(str::to_owned).collect();
        let lines = migrate(version, lines)?;
        parse_body(&lines)
    }
}

// ---------------------------------------------------------------- migration

/// Upgrade body lines from `from` to `CURRENT_VERSION`, one step at a time.
fn migrate(from: u32, mut lines: Vec<String>) -> Result<Vec<String>, SaveError> {
    let mut v = from;
    while v < CURRENT_VERSION {
        lines = match v {
            1 => migrate_v1_to_v2(lines)?,
            2 => migrate_v2_to_v3(lines)?,
            3 => migrate_v3_to_v4(lines)?,
            other => return Err(SaveError::UnknownVersion(other)),
        };
        v += 1;
    }
    Ok(lines)
}

/// v1 members were `species|level|hp|status|moves|nick` (no form, no effort). v2 adds both.
fn migrate_v1_to_v2(lines: Vec<String>) -> Result<Vec<String>, SaveError> {
    lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| match l.strip_prefix("member=") {
            None => Ok(l),
            Some(rest) => {
                let f: Vec<&str> = rest.split('|').collect();
                if f.len() != 6 {
                    return Err(SaveError::Parse { line: i + 3, msg: "v1 member needs 6 fields".into() });
                }
                Ok(format!("member={}|0|{}|{}|{}|0:0:0:0:0:0|{}|{}", f[0], f[1], f[2], f[3], f[4], f[5]))
            }
        })
        .collect()
}

/// v2 saves have no box; v3 adds `box_pokemon`, which starts empty. No line has to change, so
/// this step is a documented no-op that keeps the version chain explicit.
fn migrate_v2_to_v3(lines: Vec<String>) -> Result<Vec<String>, SaveError> {
    Ok(lines)
}

/// v3 saves have no play time; v4 adds `play_seconds`, which starts at 0 (the parser defaults a
/// missing key). No line has to change, so this step is a documented no-op that keeps the version
/// chain explicit and reserves v5 for the clock/weather task (Brief 9).
fn migrate_v3_to_v4(lines: Vec<String>) -> Result<Vec<String>, SaveError> {
    Ok(lines)
}

// ---------------------------------------------------------------- parsing

fn parse_body(lines: &[String]) -> Result<PlayerSave, SaveError> {
    let mut s = PlayerSave::default();
    let mut seen_keys: BTreeSet<&str> = BTreeSet::new();
    for (i, line) in lines.iter().enumerate() {
        let ln = i + 3;
        let err = |msg: &str| SaveError::Parse { line: ln, msg: msg.to_owned() };
        let (k, v) = line.split_once('=').ok_or_else(|| err("expected key=value"))?;
        if k != "member" && k != "box" && !seen_keys.insert(k) {
            return Err(err("duplicate key"));
        }
        match k {
            "zone" => s.zone = v.to_owned(),
            "pos" => {
                let p: Vec<f32> = v.split(',').map(|x| x.parse::<f32>().map_err(|_| err("bad number"))).collect::<Result<_, _>>()?;
                s.position = p.try_into().map_err(|_| err("pos needs 3 numbers"))?;
            }
            "yaw" => s.yaw = v.parse().map_err(|_| err("bad yaw"))?,
            "time_slot" => s.time_slot = v.parse().map_err(|_| err("bad time_slot"))?,
            "money" => s.money = v.parse().map_err(|_| err("bad money"))?,
            "defeated" => s.defeated = v.parse().map_err(|_| err("bad defeated"))?,
            "caught_count" => s.caught_count = v.parse().map_err(|_| err("bad caught_count"))?,
            "rng_seed" => s.rng_seed = v.parse().map_err(|_| err("bad rng_seed"))?,
            "play_seconds" => s.play_seconds = v.parse().map_err(|_| err("bad play_seconds"))?,
            "seen" => s.seen = parse_set(v).ok_or_else(|| err("bad seen list"))?,
            "caught" => s.caught = parse_set(v).ok_or_else(|| err("bad caught list"))?,
            "bag" => {
                for e in v.split(',').filter(|e| !e.is_empty()) {
                    let (a, b) = e.split_once(':').ok_or_else(|| err("bad bag entry"))?;
                    let id = a.parse().map_err(|_| err("bad item id"))?;
                    let n = b.parse().map_err(|_| err("bad quantity"))?;
                    if s.bag.insert(id, n).is_some() {
                        return Err(err("duplicate item id"));
                    }
                }
            }
            "member" => s.party.push(parse_member(v).map_err(|m| err(&m))?),
            "box" => s.box_pokemon.push(parse_member(v).map_err(|m| err(&m))?),
            _ => return Err(err("unknown key")),
        }
    }
    for required in ["zone", "pos", "yaw", "time_slot", "money", "rng_seed"] {
        if !seen_keys.contains(required) {
            return Err(SaveError::Invalid(format!("missing field `{required}`")));
        }
    }
    s.version = CURRENT_VERSION;
    s.validate()?;
    Ok(s)
}

/// One member encoded as the 8-field `species|form|level|hp|status|effort|moves|nickname` text,
/// shared by `member` (party) and `box` lines.
fn member_line(m: &PartyMember) -> String {
    let moves = m.moves.iter().map(|s| format!("{}:{}", s.move_id, s.pp)).collect::<Vec<_>>().join(",");
    let eff = m.effort.iter().map(u8::to_string).collect::<Vec<_>>().join(":");
    let nick = m.nickname.as_deref().map(hex_encode).unwrap_or_else(|| "-".into());
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        m.species,
        m.form,
        m.level,
        m.hp,
        m.status.map_or("-", SavedStatus::tag),
        eff,
        moves,
        nick
    )
}

fn parse_member(v: &str) -> Result<PartyMember, String> {
    let f: Vec<&str> = v.split('|').collect();
    if f.len() != 8 {
        return Err("member needs 8 fields".into());
    }
    let n = |s: &str| s.parse::<u32>().map_err(|_| format!("bad number `{s}`"));
    let eff: Vec<u8> = f[5].split(':').map(|x| x.parse::<u8>().map_err(|_| "bad effort".to_string())).collect::<Result<_, _>>()?;
    let mut moves = Vec::new();
    for e in f[6].split(',').filter(|e| !e.is_empty()) {
        let (a, b) = e.split_once(':').ok_or("bad move entry")?;
        moves.push(SavedMove { move_id: a.parse().map_err(|_| "bad move id")?, pp: b.parse().map_err(|_| "bad pp")? });
    }
    Ok(PartyMember {
        species: u16::try_from(n(f[0])?).map_err(|_| "species out of range")?,
        form: u8::try_from(n(f[1])?).map_err(|_| "form out of range")?,
        level: u8::try_from(n(f[2])?).map_err(|_| "level out of range")?,
        hp: n(f[3])?,
        status: SavedStatus::parse(f[4]).ok_or("unknown status")?,
        effort: eff.try_into().map_err(|_| "effort needs 6 values")?,
        moves,
        nickname: if f[7] == "-" { None } else { Some(hex_decode(f[7]).ok_or("bad nickname")?) },
    })
}

fn join_set(s: &BTreeSet<u16>) -> String {
    s.iter().map(u16::to_string).collect::<Vec<_>>().join(",")
}
fn parse_set(v: &str) -> Option<BTreeSet<u16>> {
    v.split(',').filter(|e| !e.is_empty()).map(|e| e.parse().ok()).collect()
}
/// Nicknames are hex-encoded so no separator can ever appear; `~` marks the empty string
/// (`-` means "no nickname").
fn hex_encode(s: &str) -> String {
    if s.is_empty() {
        return "~".into();
    }
    s.bytes().map(|b| format!("{b:02x}")).collect()
}
fn hex_decode(s: &str) -> Option<String> {
    if s == "~" {
        return Some(String::new());
    }
    if s.len() % 2 != 0 || !s.is_ascii() {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect();
    String::from_utf8(bytes?).ok()
}

// ---------------------------------------------------------------- checksum

fn fnv1a64(data: &[u8]) -> u64 {
    fnv1a64_continue(0xcbf2_9ce4_8422_2325, data)
}
fn fnv1a64_continue(mut h: u64, data: &[u8]) -> u64 {
    for b in data {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ---------------------------------------------------------------- files

/// Save directory: `PLA_SAVE_DIR`, else `$XDG_DATA_HOME/pla-port`, else `~/.local/share/pla-port`.
pub fn default_save_dir() -> PathBuf {
    resolve_save_dir(
        std::env::var_os("PLA_SAVE_DIR"),
        std::env::var_os("XDG_DATA_HOME"),
        std::env::var_os("HOME"),
    )
}

fn resolve_save_dir(
    pla: Option<std::ffi::OsString>,
    xdg: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> PathBuf {
    let non_empty = |v: Option<std::ffi::OsString>| v.filter(|s| !s.is_empty()).map(PathBuf::from);
    if let Some(p) = non_empty(pla) {
        return p;
    }
    if let Some(x) = non_empty(xdg) {
        return x.join("pla-port");
    }
    non_empty(home).unwrap_or_else(|| PathBuf::from(".")).join(".local/share/pla-port")
}

/// Atomic write: temp file in the same directory, fsync, then rename. The previous save becomes
/// `<file>.bak` (copied before the rename, so a crash never loses both).
pub fn write_atomic(path: &Path, text: &str) -> Result<(), SaveError> {
    let io = |e: std::io::Error| SaveError::Io(e.to_string());
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(dir).map_err(io)?;
    let tmp = with_suffix(path, ".tmp");
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        if path.exists() {
            fs::copy(path, with_suffix(path, ".bak"))?;
        }
        fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(io(e));
    }
    Ok(())
}

fn with_suffix(p: &Path, suffix: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

fn read_text(path: &Path) -> Result<String, SaveError> {
    let meta = fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => SaveError::NotFound,
        _ => SaveError::Io(e.to_string()),
    })?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(SaveError::BadHeader("file is far too large".into()));
    }
    let bytes = fs::read(path).map_err(|e| SaveError::Io(e.to_string()))?;
    String::from_utf8(bytes).map_err(|_| SaveError::BadHeader("not text".into()))
}

/// Save slot on disk.
#[derive(Debug, Clone)]
pub struct SaveFile {
    pub path: PathBuf,
}

/// Result of a load that may have fallen back to the backup.
#[derive(Debug)]
pub struct Loaded {
    pub save: PlayerSave,
    pub from_backup: bool,
    /// Error of the main file when the backup was used.
    pub main_error: Option<SaveError>,
}

impl SaveFile {
    pub fn in_dir(dir: &Path) -> Self {
        Self { path: dir.join(FILE_NAME) }
    }
    pub fn from_env() -> Self {
        Self::in_dir(&default_save_dir())
    }
    pub fn backup_path(&self) -> PathBuf {
        with_suffix(&self.path, ".bak")
    }
    pub fn exists(&self) -> bool {
        self.path.exists()
    }
    pub fn write(&self, save: &PlayerSave) -> Result<(), SaveError> {
        write_atomic(&self.path, &save.encode()?)
    }
    pub fn read(&self) -> Result<PlayerSave, SaveError> {
        PlayerSave::decode(&read_text(&self.path)?)
    }
    /// Read the main file; if it is damaged (not merely absent) fall back to `.bak`.
    pub fn load_with_fallback(&self) -> Result<Loaded, SaveError> {
        match self.read() {
            Ok(save) => Ok(Loaded { save, from_backup: false, main_error: None }),
            Err(SaveError::NotFound) => Err(SaveError::NotFound),
            Err(e) => match PlayerSave::decode(&read_text(&self.backup_path())?) {
                Ok(save) => Ok(Loaded { save, from_backup: true, main_error: Some(e) }),
                Err(_) => Err(e),
            },
        }
    }
}

// ---------------------------------------------------------------- Bevy resource

/// Bevy resource: where to save, the last loaded/saved snapshot and the last user-facing message.
#[derive(Resource, Debug, Clone)]
pub struct SaveGame {
    pub file: SaveFile,
    pub current: Option<PlayerSave>,
    pub message: Option<String>,
}

impl SaveGame {
    pub fn new(file: SaveFile) -> Self {
        Self { file, current: None, message: None }
    }
    pub fn from_env() -> Self {
        Self::new(SaveFile::from_env())
    }
    /// Write `save`; the message holds a friendly result either way.
    pub fn save(&mut self, save: PlayerSave) -> bool {
        match self.file.write(&save) {
            Ok(()) => {
                self.message = Some("Game saved.".into());
                self.current = Some(save);
                true
            }
            Err(e) => {
                self.message = Some(format!("Save failed: {e}"));
                false
            }
        }
    }
    /// Load (with backup fallback). `None` when there is no save or it is unreadable.
    pub fn load(&mut self) -> Option<PlayerSave> {
        match self.file.load_with_fallback() {
            Ok(l) => {
                self.message = Some(if l.from_backup {
                    "Main save was damaged; loaded the backup.".into()
                } else {
                    "Game loaded.".into()
                });
                self.current = Some(l.save.clone());
                Some(l.save)
            }
            Err(SaveError::NotFound) => {
                self.message = Some("No saved game yet.".into());
                None
            }
            Err(e) => {
                self.message = Some(format!("Load failed: {e}"));
                None
            }
        }
    }
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pla-save-test-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn sample() -> PlayerSave {
        PlayerSave {
            zone: "ha_area01".into(),
            position: [12.5, -3.25, 100.125],
            yaw: 1.5,
            time_slot: 3,
            party: vec![
                PartyMember {
                    species: 722,
                    form: 0,
                    level: 17,
                    hp: 40,
                    status: Some(SavedStatus::Burn),
                    moves: vec![SavedMove { move_id: 33, pp: 20 }, SavedMove { move_id: 10, pp: 3 }],
                    effort: [1, 0, 2, 0, 0, 3],
                    nickname: Some("Rowlé|,=".into()),
                },
                PartyMember {
                    species: 25,
                    form: 1,
                    level: 5,
                    hp: 0,
                    status: None,
                    moves: vec![],
                    effort: [0; 6],
                    nickname: Some(String::new()),
                },
            ],
            box_pokemon: vec![
                PartyMember {
                    species: 405,
                    form: 2,
                    level: 41,
                    hp: 130,
                    status: None,
                    moves: vec![SavedMove { move_id: 44, pp: 17 }],
                    effort: [3, 3, 3, 3, 3, 3],
                    nickname: None,
                },
                PartyMember {
                    species: 415,
                    form: 0,
                    level: 3,
                    hp: 1,
                    status: Some(SavedStatus::Poison),
                    moves: vec![],
                    effort: [0; 6],
                    nickname: Some("Comb".into()),
                },
            ],
            bag: BTreeMap::from([(4, 10), (1, 3)]),
            money: 1234,
            seen: BTreeSet::from([25, 722, 1]),
            caught: BTreeSet::from([722]),
            defeated: 7,
            caught_count: 2,
            rng_seed: 0xdead_beef,
            play_seconds: 12_345,
            ..PlayerSave::default()
        }
    }

    #[test]
    fn round_trip() {
        let s = sample();
        let back = PlayerSave::decode(&s.encode().unwrap()).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn round_trip_through_file() {
        let d = tmpdir("file");
        let f = SaveFile::in_dir(&d);
        assert!(matches!(f.read(), Err(SaveError::NotFound)));
        f.write(&sample()).unwrap();
        assert_eq!(f.read().unwrap(), sample());
    }

    #[test]
    fn corrupt_checksum_detected() {
        let text = sample().encode().unwrap().replace("money=1234", "money=9999");
        assert!(matches!(PlayerSave::decode(&text), Err(SaveError::ChecksumMismatch { .. })));
    }

    #[test]
    fn truncated_file_detected_at_every_length() {
        let text = sample().encode().unwrap();
        for n in 0..text.len() {
            if !text.is_char_boundary(n) {
                continue;
            }
            assert!(PlayerSave::decode(&text[..n]).is_err(), "prefix {n} must fail");
        }
    }

    #[test]
    fn garbage_never_panics() {
        for t in ["", "\n\n", "PLASAVE", "PLASAVE x\nCHECKSUM 0\nEND\n", "\u{0}\u{1}", "PLASAVE 2\nCHECKSUM zz\nEND\n"] {
            assert!(PlayerSave::decode(t).is_err());
        }
    }

    #[test]
    fn unknown_version_rejected() {
        let t = sample().encode().unwrap().replacen("PLASAVE 4", "PLASAVE 99", 1);
        assert!(matches!(PlayerSave::decode(&t), Err(SaveError::UnknownVersion(99))));
        let t0 = sample().encode().unwrap().replacen("PLASAVE 4", "PLASAVE 0", 1);
        assert!(matches!(PlayerSave::decode(&t0), Err(SaveError::UnknownVersion(0))));
    }

    #[test]
    fn backup_rotation_keeps_previous_save() {
        let d = tmpdir("bak");
        let f = SaveFile::in_dir(&d);
        let mut a = sample();
        a.money = 1;
        f.write(&a).unwrap();
        assert!(!f.backup_path().exists());
        let mut b = sample();
        b.money = 2;
        f.write(&b).unwrap();
        assert_eq!(f.read().unwrap().money, 2);
        assert_eq!(PlayerSave::decode(&fs::read_to_string(f.backup_path()).unwrap()).unwrap().money, 1);
    }

    #[test]
    fn damaged_main_falls_back_to_backup() {
        let d = tmpdir("fallback");
        let f = SaveFile::in_dir(&d);
        f.write(&sample()).unwrap();
        f.write(&sample()).unwrap();
        fs::write(&f.path, "PLASAVE 2\nCHECKSUM 00\nzone=").unwrap();
        let l = f.load_with_fallback().unwrap();
        assert!(l.from_backup && l.main_error.is_some());
        let mut g = SaveGame::new(f);
        assert_eq!(g.load().unwrap(), sample());
        assert!(g.message.unwrap().contains("backup"));
    }

    #[test]
    fn atomic_write_leaves_no_temp_and_failure_keeps_old() {
        let d = tmpdir("atomic");
        let f = SaveFile::in_dir(&d);
        f.write(&sample()).unwrap();
        let names: Vec<_> = fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        assert_eq!(names, vec![FILE_NAME.to_string()]);
        // Failed write (target is a directory): old content of a sibling save is untouched, no temp left.
        let blocked = d.join("blocked");
        fs::create_dir_all(&blocked).unwrap();
        assert!(write_atomic(&blocked, "x").is_err());
        assert!(!with_suffix(&blocked, ".tmp").exists());
        assert_eq!(f.read().unwrap(), sample());
    }

    #[test]
    fn invalid_data_refused_on_write() {
        let mut s = sample();
        s.party = vec![s.party[0].clone(); 7];
        assert!(s.encode().is_err());
        let mut s = sample();
        s.yaw = f32::NAN;
        assert!(s.encode().is_err());
        let mut s = sample();
        s.zone = "has space".into();
        assert!(s.encode().is_err());
    }

    /// Hand-build a v1 file (6-field members) with a valid checksum and migrate it.
    #[test]
    fn migration_v1_to_current() {
        let body = "zone=ha_area00\npos=1,2,3\nyaw=0\ntime_slot=1\nmoney=50\nrng_seed=9\n\
                    seen=25\ncaught=25\nbag=4:2\nmember=25|12|30|paralysis|33:10,10:5|-\nEND\n";
        let text = format!("PLASAVE 1\nCHECKSUM {:016x}\n{body}", fnv1a64(body.as_bytes()));
        let s = PlayerSave::decode(&text).unwrap();
        assert_eq!(s.version, CURRENT_VERSION);
        assert_eq!(s.money, 50);
        let m = &s.party[0];
        assert_eq!((m.species, m.form, m.level, m.hp), (25, 0, 12, 30));
        assert_eq!(m.status, Some(SavedStatus::Paralysis));
        assert_eq!(m.effort, [0; 6]);
        assert_eq!(m.moves, vec![SavedMove { move_id: 33, pp: 10 }, SavedMove { move_id: 10, pp: 5 }]);
        assert!(s.box_pokemon.is_empty(), "a v1 save has no box");
        assert_eq!(s.play_seconds, 0, "a v1 save has no play time");
        // and re-saving writes the current version
        assert!(s.encode().unwrap().starts_with("PLASAVE 4\n"));
    }

    /// A literal v2 body (8-field members, no `box` lines) loads through v2->v3 with an empty box.
    #[test]
    fn migration_v2_to_current_keeps_party_and_empty_box() {
        let body = "zone=ha_area02\npos=3,0,4\nyaw=0.5\ntime_slot=2\nmoney=777\nrng_seed=5\n\
                    seen=25,396\ncaught=25\nbag=4:3\nmember=25|0|15|22|drowsy|0:0:0:0:0:0|33:15|-\nEND\n";
        let text = format!("PLASAVE 2\nCHECKSUM {:016x}\n{body}", fnv1a64(body.as_bytes()));
        let s = PlayerSave::decode(&text).unwrap();
        assert_eq!(s.version, CURRENT_VERSION);
        assert_eq!(s.party.len(), 1);
        assert_eq!(s.party[0].species, 25);
        assert!(s.box_pokemon.is_empty());
        assert_eq!(s.play_seconds, 0, "a v2 save has no play time");
        assert!(s.encode().unwrap().starts_with("PLASAVE 4\n"));
    }

    /// A literal v3 body (box lines, no `play_seconds`) loads through v3->v4 with a play time of
    /// 0 and keeps its box.
    #[test]
    fn migration_v3_to_current_defaults_play_time_and_keeps_the_box() {
        let body = "zone=ha_area00\npos=0,0,0\nyaw=0\ntime_slot=1\nmoney=10\nrng_seed=2\n\
                    member=25|0|5|10|-|0:0:0:0:0:0|33:5|-\nbox=133|1|3|9|-|0:0:0:0:0:0||-\nEND\n";
        let text = format!("PLASAVE 3\nCHECKSUM {:016x}\n{body}", fnv1a64(body.as_bytes()));
        let s = PlayerSave::decode(&text).unwrap();
        assert_eq!(s.version, CURRENT_VERSION);
        assert_eq!(s.play_seconds, 0, "a v3 save has no play time");
        assert_eq!(s.box_pokemon.len(), 1);
        assert_eq!(s.box_pokemon[0].species, 133);
        let out = s.encode().unwrap();
        assert!(out.starts_with("PLASAVE 4\n"));
        assert!(out.contains("\nplay_seconds=0\n"), "the new field is written on re-save");
    }

    /// `play_seconds` is a u64 and survives the largest value.
    #[test]
    fn play_time_round_trips_at_u64_max() {
        let mut s = sample();
        s.play_seconds = u64::MAX;
        let back = PlayerSave::decode(&s.encode().unwrap()).unwrap();
        assert_eq!(back.play_seconds, u64::MAX);
    }

    /// The box survives encode/decode and the parser enforces `MAX_BOX`.
    #[test]
    fn box_round_trip_and_limit() {
        let s = sample();
        assert_eq!(s.box_pokemon.len(), 2, "sample carries a box");
        let text = s.encode().unwrap();
        assert!(text.contains("\nbox="), "box lines are written");
        let back = PlayerSave::decode(&text).unwrap();
        assert_eq!(s, back);
        // exactly at the limit is accepted; one more is refused on write...
        let mut at_limit = sample();
        at_limit.box_pokemon = vec![s.box_pokemon[0].clone(); MAX_BOX];
        assert!(at_limit.encode().is_ok());
        at_limit.box_pokemon.push(s.box_pokemon[1].clone());
        assert!(matches!(at_limit.encode(), Err(SaveError::Invalid(_))));
        // ...and rejected on read (checksum valid, content over the limit).
        let body = format!(
            "zone=z\npos=0,0,0\nyaw=0\ntime_slot=0\nmoney=0\nrng_seed=1\n{}END\n",
            "box=25|0|5|10|-|0:0:0:0:0:0|33:5|-\n".repeat(MAX_BOX + 1)
        );
        let text = format!("PLASAVE 3\nCHECKSUM {:016x}\n{body}", fnv1a64(body.as_bytes()));
        assert!(matches!(PlayerSave::decode(&text), Err(SaveError::Invalid(_))));
    }

    /// A malformed box member is refused like a malformed party member.
    #[test]
    fn malformed_box_member_rejected() {
        let body = "zone=z\npos=0,0,0\nyaw=0\ntime_slot=0\nmoney=0\nrng_seed=1\nbox=not-a-member\nEND\n";
        let text = format!("PLASAVE 3\nCHECKSUM {:016x}\n{body}", fnv1a64(body.as_bytes()));
        assert!(matches!(PlayerSave::decode(&text), Err(SaveError::Parse { .. })));
    }

    #[test]
    fn species_sets_are_deterministic() {
        let mut a = sample();
        let mut b = sample();
        a.seen = [900, 3, 77, 3].into_iter().collect();
        b.seen = [77, 900, 3].into_iter().collect();
        assert_eq!(a.encode().unwrap(), b.encode().unwrap());
        assert!(a.encode().unwrap().contains("seen=3,77,900\n"));
    }

    #[test]
    fn save_dir_resolution_order() {
        use std::ffi::OsString as O;
        assert_eq!(resolve_save_dir(Some(O::from("/a")), Some(O::from("/x")), Some(O::from("/h"))), PathBuf::from("/a"));
        assert_eq!(resolve_save_dir(None, Some(O::from("/x")), Some(O::from("/h"))), PathBuf::from("/x/pla-port"));
        assert_eq!(resolve_save_dir(Some(O::from("")), None, Some(O::from("/h"))), PathBuf::from("/h/.local/share/pla-port"));
    }

    #[test]
    fn status_conversion_round_trips() {
        for s in [MajorStatus::Drowsy, MajorStatus::Paralysis, MajorStatus::Poison, MajorStatus::Burn, MajorStatus::Frostbite] {
            assert_eq!(MajorStatus::from(SavedStatus::from(s)), s);
        }
    }

    #[test]
    fn duplicate_keys_and_unknown_keys_rejected() {
        let make = |extra: &str| {
            let body = format!("zone=z\npos=0,0,0\nyaw=0\ntime_slot=0\nmoney=0\nrng_seed=1\n{extra}END\n");
            format!("PLASAVE 2\nCHECKSUM {:016x}\n{body}", fnv1a64(body.as_bytes()))
        };
        assert!(PlayerSave::decode(&make("")).is_ok());
        assert!(PlayerSave::decode(&make("money=1\n")).is_err());
        assert!(PlayerSave::decode(&make("bogus=1\n")).is_err());
    }
}
