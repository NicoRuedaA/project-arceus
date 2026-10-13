//! Game audio: music, sound effects and Pokémon cries from the game's decoded Wwise media.
//!
//! * [`GameAudioPlugin`] loads the sound catalogue (`data/audio_index.tsv` +
//!   `data/audio_event_media.tsv`), watches the existing game state (battle session, pause
//!   menu, zone, thrown balls, battle models appearing) and plays the matching sounds.
//! * Other systems can ask for sounds with the [`GameAudio`] system parameter
//!   (`play_sfx`, `play_cry`, `play_music`, `stop_music`) or by writing [`PlayCue`] messages.
//! * Volumes come from `settings.txt` (`master/music/sfx/voice_volume`), applied live.
//! * Files are loaded on first use and kept in a byte-bounded LRU cache (the Ogg rewrap is
//!   preferred over the WAV when it exists). Missing or undecodable media are silent, with one
//!   warning per event or file.
//! * `--no-audio` or `PLA_NO_AUDIO=1` turns everything off ([`AudioMode::Off`]): no audio
//!   device is opened and nothing is loaded. Without a device (headless, busy device) Bevy logs
//!   "No audio device found." once and the game continues silently.
//!
//! The pure parts live in [`events`] (event -> sound mapping), [`catalog`] (index lookups),
//! [`mix`] (bus volumes, crossfade) and [`lru`]; they are unit-tested. Nothing here was
//! listened to by the author: what each sound *is* comes from its event name.

pub mod catalog;
pub mod events;
pub mod lru;
pub mod mix;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Mutex;

use bevy::asset::AssetApp;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_audio::{AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, Volume};

use self::catalog::{decodable_header, SoundCatalog};
use self::events::{
    ball_events, ball_vanished_events, cues_for_battle_events, cues_for_log_line, music_for_area,
    sfx_for_event, BallSnapshot, Bus, Cue, GameEvent, MusicId, SfxId, TimedCue, VariantPolicy,
};
use self::lru::LruCache;
use self::mix::{BusVolumes, MusicFader, MusicStart};
use super::ball_lifecycle::BallBody;
use super::battle_controller::{ActiveBattleSession, BattleMenuState, WILD};
use super::pause_menu::PauseMenu;
use super::pokemon_anim::PokemonBattleSlot;
use super::settings::UserSettings;
use super::state::Paused;
use super::ui::ActiveZoneInfo;

/// Whether and where sound goes.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioMode {
    /// `--no-audio` / `PLA_NO_AUDIO=1`: nothing is loaded or played.
    Off,
    /// Everything runs (catalogue, cache, voices as entities) but no audio device is opened:
    /// the voices are inert. Used by tests and headless checks.
    Null,
    /// Real output through Bevy's audio plugin (cpal/ALSA on Linux).
    Device,
}

/// True when the command line has `--no-audio` or `PLA_NO_AUDIO` is `1`/`true`/`yes`/`on`.
pub fn no_audio_requested(args: &[String], env_value: Option<&str>) -> bool {
    args.iter().any(|a| a == "--no-audio")
        || env_value.is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
}

/// Default byte budget of the media cache (override with `PLA_AUDIO_CACHE_MB`).
pub const DEFAULT_CACHE_MB: usize = 256;

/// Plays the game's audio. See the module documentation.
pub struct GameAudioPlugin {
    pub mode: AudioMode,
    /// Directory with `audio_index.tsv` (searched in the usual places when `None`).
    pub data_dir: Option<PathBuf>,
    /// Decode cache root (`$PLA_CACHE_ROOT`, else the default cache) when `None`.
    pub cache_root: Option<PathBuf>,
}

impl GameAudioPlugin {
    /// Device output unless `--no-audio` / `PLA_NO_AUDIO=1`.
    pub fn from_env_and_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let env = std::env::var("PLA_NO_AUDIO").ok();
        let mode = if no_audio_requested(&args, env.as_deref()) { AudioMode::Off } else { AudioMode::Device };
        Self { mode, data_dir: None, cache_root: None }
    }

    pub fn with_mode(mode: AudioMode) -> Self {
        Self { mode, data_dir: None, cache_root: None }
    }
}

/// Where `audio_index.tsv` is looked for when no directory is given.
pub fn find_data_dir() -> Option<PathBuf> {
    [
        PathBuf::from("data"),
        PathBuf::from("../../data"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        PathBuf::from("/home/nico/work/decompilacion/data"),
    ]
    .into_iter()
    .find(|p| p.join(SoundCatalog::INDEX_FILE).is_file())
}

/// Decode cache root: `$PLA_CACHE_ROOT`, else the default cache directory.
pub fn default_cache_root() -> PathBuf {
    std::env::var_os("PLA_CACHE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(super::world_objects::DEFAULT_CACHE_ROOT))
}

/// A sound request: `cue` plays `delay` seconds after the message is read.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct PlayCue {
    pub cue: Cue,
    pub delay: f32,
}

impl From<TimedCue> for PlayCue {
    fn from(t: TimedCue) -> Self {
        Self { cue: t.cue, delay: t.delay }
    }
}

/// Sound API for other systems.
#[derive(SystemParam)]
pub struct GameAudio<'w> {
    out: MessageWriter<'w, PlayCue>,
}

impl GameAudio<'_> {
    pub fn play_sfx(&mut self, sfx: SfxId) {
        self.out.write(PlayCue { cue: Cue::Sfx(sfx), delay: 0.0 });
    }
    pub fn play_cry(&mut self, species: u16, form: u8) {
        self.out.write(PlayCue { cue: Cue::Cry { species, form, faint: false }, delay: 0.0 });
    }
    pub fn play_music(&mut self, music: MusicId) {
        self.out.write(PlayCue { cue: Cue::Music(music), delay: 0.0 });
    }
    pub fn stop_music(&mut self) {
        self.out.write(PlayCue { cue: Cue::StopMusic, delay: 0.0 });
    }
    /// Sound of a game event (see [`sfx_for_event`]); silent when it has none.
    pub fn play_event(&mut self, e: GameEvent) {
        if let Some(s) = sfx_for_event(&e) {
            self.play_sfx(s);
        }
    }
}

/// Counters for checks and tests.
#[derive(Resource, Debug, Default)]
pub struct AudioStats {
    /// Cues executed (after their delay).
    pub executed: u64,
    /// Voices spawned (audible with a device).
    pub voices: u64,
    /// Cues that stayed silent (unknown event, no playable media, missing or bad file).
    pub silent: u64,
    /// Event names of the last executed cues, newest last (at most 64).
    pub recent: VecDeque<String>,
}

impl AudioStats {
    fn note(&mut self, name: String) {
        self.executed += 1;
        if self.recent.len() >= 64 {
            self.recent.pop_front();
        }
        self.recent.push_back(name);
    }
}

/// A playing (or inert, without a device) voice.
#[derive(Component, Debug, Clone, Copy)]
pub struct SoundVoice {
    pub bus: Bus,
    pub gain: f32,
    /// Music voices carry their fader id.
    pub music: Option<u64>,
}

/// Seconds until a one-shot voice is removed even if no device ever played it.
#[derive(Component, Debug)]
struct VoiceLifetime(f32);

struct MusicLoad {
    id: u64,
    path: PathBuf,
    rx: Receiver<Result<Vec<u8>, String>>,
}

/// Runtime state of the audio plugin.
#[derive(Resource)]
pub struct AudioRuntime {
    pub catalog: Option<SoundCatalog>,
    cache: LruCache<PathBuf, Handle<AudioSource>>,
    warned: HashSet<String>,
    rng: u32,
    pending: Vec<(f32, Cue)>,
    pub fader: MusicFader,
    music_entities: HashMap<u64, Entity>,
    loads: Mutex<Vec<MusicLoad>>,
    pub volumes: BusVolumes,
}

impl AudioRuntime {
    pub fn new(catalog: Option<SoundCatalog>, cache_bytes: usize) -> Self {
        Self {
            catalog,
            cache: LruCache::new(cache_bytes),
            warned: HashSet::new(),
            rng: 0x9e37_79b9,
            pending: Vec::new(),
            fader: MusicFader::default(),
            music_entities: HashMap::new(),
            loads: Mutex::new(Vec::new()),
            volumes: BusVolumes::default(),
        }
    }

    /// Bytes held by the media cache.
    pub fn cache_bytes(&self) -> usize {
        self.cache.used()
    }

    fn warn_once(&mut self, key: String) {
        if self.warned.insert(key.clone()) {
            warn!("audio: {key}");
        }
    }

    fn roll(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    /// Cached handle of a file, or read + validate + add it.
    fn handle_for(&mut self, path: &Path, assets: &mut Assets<AudioSource>) -> Option<Handle<AudioSource>> {
        if let Some(h) = self.cache.get(&path.to_path_buf()) {
            return Some(h.clone());
        }
        match std::fs::read(path) {
            Ok(bytes) => self.add_bytes(path, bytes, assets),
            Err(e) => {
                self.warn_once(format!("cannot read {}: {e}", path.display()));
                None
            }
        }
    }

    fn add_bytes(&mut self, path: &Path, bytes: Vec<u8>, assets: &mut Assets<AudioSource>) -> Option<Handle<AudioSource>> {
        if !decodable_header(&bytes) {
            self.warn_once(format!("{} is not a WAV or Ogg Vorbis file; skipped", path.display()));
            return None;
        }
        let len = bytes.len();
        let h = assets.add(AudioSource { bytes: bytes.into() });
        // Evicted handles are dropped here; a voice still playing keeps its own clone.
        self.cache.insert(path.to_path_buf(), h.clone(), len);
        Some(h)
    }
}

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.mode).init_resource::<AudioStats>().add_message::<PlayCue>();
        if self.mode == AudioMode::Off {
            info!("audio: off (--no-audio / PLA_NO_AUDIO)");
            return;
        }
        if self.mode == AudioMode::Device && !app.is_plugin_added::<bevy_audio::AudioPlugin>() {
            app.add_plugins(bevy_audio::AudioPlugin::default());
        }
        if self.mode == AudioMode::Null && app.world().contains_resource::<AssetServer>() {
            app.init_asset::<AudioSource>();
        }
        let data = self.data_dir.clone().or_else(find_data_dir);
        let cache_root = self.cache_root.clone().unwrap_or_else(default_cache_root);
        let catalog = match data {
            Some(d) => match SoundCatalog::load(&d, cache_root) {
                Ok(c) => {
                    info!("audio: {} media rows from {}", c.rows.len(), d.display());
                    Some(c)
                }
                Err(e) => {
                    warn!("audio: catalogue not loaded ({e}); the game stays silent");
                    None
                }
            },
            None => {
                warn!("audio: data/audio_index.tsv not found; the game stays silent");
                None
            }
        };
        let mb = std::env::var("PLA_AUDIO_CACHE_MB")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_CACHE_MB);
        app.insert_resource(AudioRuntime::new(catalog, mb.max(8) * 1024 * 1024));
        app.add_systems(
            Update,
            (
                observe_battle_session,
                observe_battle_slots,
                observe_zone_music,
                observe_pause,
                observe_balls,
                process_cues,
                update_music,
                apply_volumes,
                expire_voices,
            )
                .chain(),
        );
    }
}

fn send(out: &mut MessageWriter<PlayCue>, cues: impl IntoIterator<Item = TimedCue>) {
    for c in cues {
        out.write(c.into());
    }
}

fn sfx(e: GameEvent, delay: f32) -> Option<TimedCue> {
    sfx_for_event(&e).map(|s| TimedCue { cue: Cue::Sfx(s), delay })
}

#[derive(Default)]
struct BattleWatch {
    active: bool,
    events: usize,
    lines: usize,
    ended: bool,
    levels: Vec<u8>,
    menu: (Option<BattleMenuState>, usize, usize),
}

/// Battle sounds from the [`ActiveBattleSession`]: start/end, engine events (hits, faint cries,
/// switches, status), log lines (balls, capture, run), level ups and menu navigation.
fn observe_battle_session(
    session: Option<Res<ActiveBattleSession>>,
    mut w: Local<BattleWatch>,
    mut out: MessageWriter<PlayCue>,
) {
    let Some(s) = session else {
        *w = BattleWatch::default();
        return;
    };
    let levels: Vec<u8> = s.battle.combatants.iter().map(|c| c.level).collect();
    let menu = (Some(s.menu_state), s.selected_move_idx, s.list_cursor);
    if !w.active {
        *w = BattleWatch {
            active: true,
            events: s.event_log.len(),
            lines: s.log_lines.len(),
            ended: s.battle.ended,
            levels,
            menu,
        };
        send(&mut out, sfx(GameEvent::BattleStarted, 0.0));
        out.write(PlayCue { cue: Cue::Sfx(SfxId::BattleUiOpen), delay: 0.6 });
        out.write(PlayCue { cue: Cue::Music(MusicId::WildBattle), delay: 0.0 });
        return;
    }
    if s.event_log.len() < w.events {
        w.events = 0;
    }
    if s.log_lines.len() < w.lines {
        w.lines = 0;
    }
    let new_events = &s.event_log[w.events..];
    let turn_played = !new_events.is_empty();
    if turn_played && matches!(w.menu.0, Some(BattleMenuState::MainMenu | BattleMenuState::MoveSelect)) {
        send(&mut out, sfx(GameEvent::BattleMoveChosen, 0.0));
    }
    let species = |i: usize| s.battle.combatants.get(i).map(|c| c.species.dex).filter(|d| *d > 0);
    send(&mut out, cues_for_battle_events(new_events, &species));
    w.events = s.event_log.len();
    for line in &s.log_lines[w.lines..] {
        send(&mut out, cues_for_log_line(line));
    }
    w.lines = s.log_lines.len();
    for (i, l) in levels.iter().enumerate() {
        if w.levels.get(i).is_some_and(|old| l > old) {
            send(&mut out, sfx(GameEvent::LevelUp, 1.0));
        }
    }
    w.levels = levels;
    if s.battle.ended && !w.ended {
        send(&mut out, sfx(GameEvent::BattleEnded, 1.2));
        out.write(PlayCue { cue: Cue::StopMusic, delay: 1.0 });
    }
    w.ended = s.battle.ended;
    if !turn_played && menu != w.menu {
        let (old_state, old_move, old_cursor) = w.menu;
        let in_list = |m: Option<BattleMenuState>| {
            matches!(m, Some(BattleMenuState::ItemSelect | BattleMenuState::SwitchSelect))
        };
        let e = if menu.0 != old_state {
            match menu.0 {
                Some(BattleMenuState::ItemSelect) => Some(GameEvent::BattleBagOpened),
                Some(BattleMenuState::SwitchSelect) => Some(GameEvent::UiDecide),
                _ if in_list(old_state) => Some(GameEvent::BattleBack),
                _ => None,
            }
        } else if in_list(menu.0) && menu.2 != old_cursor {
            Some(GameEvent::BattleListCursor)
        } else if menu.1 != old_move {
            Some(GameEvent::BattleMoveCursor)
        } else {
            None
        };
        if let Some(e) = e {
            send(&mut out, sfx(e, 0.0));
        }
    }
    w.menu = menu;
}

/// Cry when a battle model appears (its `PokemonBattleSlot` is added): slot 1 is the wild
/// Pokémon, slot 0 the player's active one.
fn observe_battle_slots(
    session: Option<Res<ActiveBattleSession>>,
    slots: Query<&PokemonBattleSlot, Added<PokemonBattleSlot>>,
    mut out: MessageWriter<PlayCue>,
) {
    let Some(s) = session else { return };
    for slot in &slots {
        let who = if slot.0 == 1 { WILD } else { s.active_player() };
        let dex = s.battle.combatants.get(who).map(|c| c.species.dex).filter(|d| *d > 0);
        let dex = match (who, dex) {
            (WILD, None) if s.wild_dex > 0 => Some(s.wild_dex),
            (_, d) => d,
        };
        send(&mut out, sfx(GameEvent::PokemonAppeared, 0.1));
        if let Some(species) = dex {
            out.write(PlayCue { cue: Cue::Cry { species, form: 0, faint: false }, delay: 0.4 + 0.5 * slot.0 as f32 });
        }
    }
}

/// Field music of the current zone, again after each battle.
fn observe_zone_music(
    zone: Option<Res<ActiveZoneInfo>>,
    session: Option<Res<ActiveBattleSession>>,
    mut last: Local<Option<u8>>,
    mut rt: ResMut<AudioRuntime>,
    mut out: MessageWriter<PlayCue>,
) {
    if session.is_some() {
        *last = None;
        return;
    }
    let Some(z) = zone else { return };
    if *last == Some(z.area_idx) {
        return;
    }
    *last = Some(z.area_idx);
    match music_for_area(z.area_idx) {
        Some(m) => {
            out.write(PlayCue { cue: Cue::Music(m), delay: 0.0 });
        }
        None => {
            rt.warn_once(format!("no field music identified for area {}", z.area_idx));
            out.write(PlayCue { cue: Cue::StopMusic, delay: 0.0 });
        }
    }
}

/// Pause menu: open/close, cursor moves, and a click on a setting row.
fn observe_pause(
    paused: Option<Res<Paused>>,
    menu: Option<Res<PauseMenu>>,
    settings: Option<Res<UserSettings>>,
    mut last: Local<Option<(bool, usize)>>,
    mut out: MessageWriter<PlayCue>,
) {
    let (Some(p), Some(m)) = (paused, menu) else { return };
    let now = (p.0, m.selected);
    let Some(prev) = *last else {
        *last = Some(now);
        return;
    };
    *last = Some(now);
    let e = match (prev.0, now.0) {
        (false, true) => Some(GameEvent::PauseOpened),
        (true, false) => Some(GameEvent::PauseClosed),
        (true, true) if prev.1 != now.1 => Some(GameEvent::UiCursor),
        (true, true) if settings.as_ref().is_some_and(|s| s.is_changed() && !s.is_added()) => Some(GameEvent::UiDecide),
        _ => None,
    };
    if let Some(e) = e {
        send(&mut out, sfx(e, 0.0));
    }
}

/// Overworld balls: throw, bounce, hit, shakes, capture or escape.
fn observe_balls(
    balls: Query<(Entity, &BallBody)>,
    mut seen: Local<HashMap<Entity, BallSnapshot>>,
    mut out: MessageWriter<PlayCue>,
) {
    let mut alive = HashSet::new();
    let emit = |events: Vec<GameEvent>, out: &mut MessageWriter<PlayCue>| {
        for e in events {
            send(out, sfx(e, 0.0));
            if e == GameEvent::CaptureSucceeded {
                out.write(PlayCue { cue: Cue::Sfx(SfxId::CaptureJingle), delay: 0.4 });
            }
        }
    };
    for (e, b) in &balls {
        alive.insert(e);
        let snap = BallSnapshot { state: b.0.state, bounces: b.0.bounces };
        emit(ball_events(seen.get(&e).copied(), snap), &mut out);
        seen.insert(e, snap);
    }
    let gone: Vec<Entity> = seen.keys().copied().filter(|e| !alive.contains(e)).collect();
    for e in gone {
        if let Some(last) = seen.remove(&e) {
            emit(ball_vanished_events(last), &mut out);
        }
    }
}

/// Runs due cues: resolves them in the catalogue, loads files and spawns voices.
fn process_cues(
    mut cues: MessageReader<PlayCue>,
    time: Option<Res<Time>>,
    mut rt: ResMut<AudioRuntime>,
    mut stats: ResMut<AudioStats>,
    mut assets: Option<ResMut<Assets<AudioSource>>>,
    mut commands: Commands,
) {
    let dt = time.map_or(0.0, |t| t.delta_secs());
    for c in cues.read() {
        rt.pending.push((c.delay.max(0.0), c.cue));
    }
    if rt.pending.is_empty() {
        return;
    }
    let mut due = Vec::new();
    rt.pending.retain_mut(|(left, cue)| {
        if *left <= 0.0 {
            due.push(*cue);
            false
        } else {
            *left -= dt;
            true
        }
    });
    for cue in due {
        run_cue(cue, &mut rt, &mut stats, assets.as_deref_mut(), &mut commands);
    }
}

fn run_cue(
    cue: Cue,
    rt: &mut AudioRuntime,
    stats: &mut AudioStats,
    assets: Option<&mut Assets<AudioSource>>,
    commands: &mut Commands,
) {
    let (event, policy, bus, speed) = match cue {
        Cue::StopMusic => {
            stats.note("<stop music>".into());
            rt.fader.stop(MusicFader::DEFAULT_FADE_OUT_S);
            return;
        }
        Cue::Music(m) => {
            stats.note(m.event_name().unwrap_or("<no music>").to_string());
            start_music(m, rt, stats, assets, commands);
            return;
        }
        Cue::Sfx(s) => (Some(s.event_name().to_string()), s.policy(), s.bus(), 1.0),
        Cue::Cry { species, form, faint } => {
            let ev = rt.catalog.as_ref().and_then(|c| c.cry_event(species, form));
            if ev.is_none() {
                rt.warn_once(format!("no cry for species {species} form {form}"));
            }
            (ev, VariantPolicy::Random, Bus::Voice, if faint { 0.85 } else { 1.0 })
        }
    };
    let Some(event) = event else {
        stats.silent += 1;
        return;
    };
    stats.note(event.clone());
    let roll = rt.roll();
    enum Found {
        NoCatalog,
        NoMedia { known: bool },
        MissingFile(String),
        File(PathBuf, f32),
    }
    let found = match rt.catalog.as_ref() {
        None => Found::NoCatalog,
        Some(cat) => match cat.pick(&event, policy, roll) {
            None => Found::NoMedia { known: cat.has_event(&event) },
            Some(row) => match cat.file_for(row) {
                Some(f) => Found::File(f.path, row.duration_s),
                None => Found::MissingFile(
                    cat.candidates(row).last().map(|f| f.path.display().to_string()).unwrap_or_default(),
                ),
            },
        },
    };
    let (path, duration) = match found {
        Found::File(p, d) => (p, d),
        Found::NoCatalog => {
            stats.silent += 1;
            return;
        }
        Found::NoMedia { known } => {
            rt.warn_once(if known {
                format!("event {event} has no playable media (undecoded codec or silent)")
            } else {
                format!("event {event} is not in the sound catalogue")
            });
            stats.silent += 1;
            return;
        }
        Found::MissingFile(p) => {
            rt.warn_once(format!("media file missing for {event}: {p} (run examples/audio_export)"));
            stats.silent += 1;
            return;
        }
    };
    let Some(assets) = assets else {
        stats.silent += 1;
        return;
    };
    let Some(handle) = rt.handle_for(&path, assets) else {
        stats.silent += 1;
        return;
    };
    let gain = 1.0;
    let volume = rt.volumes.output(bus, gain, 1.0);
    commands.spawn((
        AudioPlayer(handle),
        PlaybackSettings { volume: Volume::Linear(volume), speed, ..PlaybackSettings::DESPAWN },
        SoundVoice { bus, gain, music: None },
        VoiceLifetime(duration / speed.max(0.1) + 0.5),
    ));
    stats.voices += 1;
}

fn start_music(
    m: MusicId,
    rt: &mut AudioRuntime,
    stats: &mut AudioStats,
    assets: Option<&mut Assets<AudioSource>>,
    commands: &mut Commands,
) {
    let MusicStart::Start(id) = rt.fader.play(m, MusicFader::DEFAULT_FADE_IN_S, MusicFader::DEFAULT_FADE_OUT_S) else {
        return;
    };
    let file = m.event_name().and_then(|ev| {
        let cat = rt.catalog.as_ref()?;
        let row = cat.pick(ev, VariantPolicy::Longest, 0)?;
        cat.file_for(row).map(|f| f.path)
    });
    let Some(path) = file else {
        rt.warn_once(format!("no playable music for {m:?} ({:?})", m.event_name()));
        rt.fader.forget(id);
        stats.silent += 1;
        return;
    };
    if let Some(h) = rt.cache.get(&path).cloned() {
        spawn_music(id, h, rt, commands);
        stats.voices += 1;
        return;
    }
    if assets.is_none() {
        rt.fader.forget(id);
        stats.silent += 1;
        return;
    }
    // Music files are large (up to ~50 MB WAV): read them off the main thread.
    let (tx, rx) = std::sync::mpsc::channel();
    let p = path.clone();
    let spawned = std::thread::Builder::new()
        .name("pla-audio-load".into())
        .spawn(move || {
            let _ = tx.send(std::fs::read(&p).map_err(|e| e.to_string()));
        });
    match spawned {
        Ok(_) => rt.loads.get_mut().unwrap_or_else(|e| e.into_inner()).push(MusicLoad { id, path, rx }),
        Err(e) => {
            rt.warn_once(format!("cannot start the music loader thread: {e}"));
            rt.fader.forget(id);
        }
    }
}

fn spawn_music(id: u64, h: Handle<AudioSource>, rt: &mut AudioRuntime, commands: &mut Commands) {
    let e = commands
        .spawn((
            AudioPlayer(h),
            PlaybackSettings { volume: Volume::Linear(0.0), ..PlaybackSettings::LOOP },
            SoundVoice { bus: Bus::Music, gain: 1.0, music: Some(id) },
        ))
        .id();
    rt.music_entities.insert(id, e);
    rt.fader.mark_ready(id);
}

/// Finishes music loads, advances the fades, despawns faded-out voices.
fn update_music(
    time: Option<Res<Time>>,
    mut rt: ResMut<AudioRuntime>,
    mut stats: ResMut<AudioStats>,
    mut assets: Option<ResMut<Assets<AudioSource>>>,
    mut commands: Commands,
) {
    let loads = std::mem::take(rt.loads.get_mut().unwrap_or_else(|e| e.into_inner()));
    let mut keep = Vec::new();
    for l in loads {
        match l.rx.try_recv() {
            Err(TryRecvError::Empty) => keep.push(l),
            Err(TryRecvError::Disconnected) => rt.fader.forget(l.id),
            Ok(res) => {
                let wanted = rt.fader.voices.iter().any(|v| v.id == l.id && !v.stopping);
                let handle = match (res, assets.as_deref_mut()) {
                    (Ok(bytes), Some(a)) => rt.add_bytes(&l.path, bytes, a),
                    (Err(e), _) => {
                        rt.warn_once(format!("cannot read {}: {e}", l.path.display()));
                        None
                    }
                    _ => None,
                };
                match handle {
                    Some(h) if wanted => {
                        spawn_music(l.id, h, &mut rt, &mut commands);
                        stats.voices += 1;
                    }
                    _ => {
                        rt.fader.forget(l.id);
                        if !wanted {
                            continue;
                        }
                        stats.silent += 1;
                    }
                }
            }
        }
    }
    rt.loads.get_mut().unwrap_or_else(|e| e.into_inner()).extend(keep);
    let dt = time.map_or(0.0, |t| t.delta_secs());
    for id in rt.fader.update(dt) {
        if let Some(e) = rt.music_entities.remove(&id) {
            commands.entity(e).try_despawn();
        }
    }
}

/// Applies bus volumes (from `settings.txt`, live) and music fade levels to the playing sinks.
fn apply_volumes(
    settings: Option<Res<UserSettings>>,
    mut rt: ResMut<AudioRuntime>,
    mut voices: Query<(&SoundVoice, &mut AudioSink)>,
) {
    if let Some(s) = settings {
        let v = BusVolumes::from_settings(&s);
        if v != rt.volumes {
            rt.volumes = v;
        }
    }
    for (voice, mut sink) in &mut voices {
        let fade = voice.music.map_or(1.0, |id| rt.fader.level(id));
        let want = rt.volumes.output(voice.bus, voice.gain, fade);
        if (sink.volume().to_linear() - want).abs() > 1e-4 {
            sink.set_volume(Volume::Linear(want));
        }
    }
}

/// Removes one-shot voices after their length (a device despawns them earlier itself; without
/// one they would pile up).
fn expire_voices(
    time: Option<Res<Time>>,
    mut q: Query<(Entity, &mut VoiceLifetime)>,
    mut commands: Commands,
) {
    let dt = time.map_or(0.0, |t| t.delta_secs());
    for (e, mut l) in &mut q {
        l.0 -= dt;
        if l.0 <= 0.0 {
            commands.entity(e).try_despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_audio_switches() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(no_audio_requested(&a(&["viewer", "--no-audio"]), None));
        assert!(no_audio_requested(&a(&["viewer"]), Some("1")));
        assert!(no_audio_requested(&a(&["viewer"]), Some(" TRUE ")));
        assert!(!no_audio_requested(&a(&["viewer"]), Some("0")));
        assert!(!no_audio_requested(&a(&["viewer", "--area", "1"]), None));
    }

    const INDEX: &str = "entry:string*\tbank:string\twem_id:u32\tsource:enum\tcodec:string\tchannels:u8\tsample_rate:u32\tduration_s:f32?\tsize_bytes:u32\tstatus:enum\treason:string?\tpeak:f32?\trms:f32?\tcentroid_hz:f32?\twav_rel:string?\tevent_count:u16\tcategory:string?\tname:string?\tname_confidence:enum\tspecies:u16?\n\
UI/1\tUI\t1\tbank\tpcm\t1\t8000\t0.1\t10\tok\t\t0.5\t0.01\t1\twork/cache/audio/UI/1.wav\t1\tui\tPlay_UI_common_select\thash_string\t\n\
BGM/2\tBGM\t2\tbank\tpcm\t1\t8000\t0.5\t10\tok\t\t0.5\t0.01\t1\twork/cache/audio/BGM/2.wav\t1\tbgm\tPLAY_BGM_HA_FI_AREA00\thash_string\t\n\
BGM/3\tBGM\t3\tbank\tpcm\t1\t8000\t0.5\t10\tok\t\t0.5\t0.01\t1\twork/cache/audio/BGM/3.wav\t1\tbgm\tPLAY_BGM_VS_NORA\thash_string\t\n";

    /// Writes a tiny valid WAV (8 kHz, 16-bit mono, `n` zero samples).
    fn wav(path: &Path, n: usize) {
        let data = n * 2;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data as u32).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&8000u32.to_le_bytes());
        b.extend_from_slice(&16000u32.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data as u32).to_le_bytes());
        b.resize(44 + data, 0);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b).unwrap();
    }

    fn app(dir: &Path) -> App {
        let data = dir.join("data");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(data.join(SoundCatalog::INDEX_FILE), INDEX).unwrap();
        wav(&dir.join("cache/audio/UI/1.wav"), 800);
        wav(&dir.join("cache/audio/BGM/2.wav"), 4000);
        // BGM/3 (battle music) is deliberately missing: it must stay silent, not panic.
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.add_plugins(GameAudioPlugin {
            mode: AudioMode::Null,
            data_dir: Some(data),
            cache_root: Some(dir.join("cache")),
        });
        app
    }

    fn voices(app: &mut App) -> Vec<SoundVoice> {
        app.world_mut().query::<&SoundVoice>().iter(app.world()).copied().collect()
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pla_audio_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn null_sink_plays_sfx_and_music_and_stays_silent_on_missing_media() {
        let dir = temp("null");
        let mut app = app(&dir);
        app.update();
        app.world_mut().write_message(PlayCue { cue: Cue::Sfx(SfxId::UiCursor), delay: 0.0 });
        app.world_mut().write_message(PlayCue { cue: Cue::Sfx(SfxId::BattleStart), delay: 0.0 });
        app.world_mut().write_message(PlayCue { cue: Cue::Music(MusicId::Area(0)), delay: 0.0 });
        app.update();
        // music loads on a thread: wait for it
        for _ in 0..200 {
            if voices(&mut app).iter().any(|v| v.music.is_some()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
            app.update();
        }
        let v = voices(&mut app);
        assert_eq!(v.iter().filter(|v| v.music.is_some()).count(), 1, "{v:?}");
        assert_eq!(app.world().resource::<AudioStats>().voices, 2, "one sfx + one music voice");
        let stats = app.world().resource::<AudioStats>();
        assert_eq!(stats.silent, 1, "BattleStart is not in the test index");
        assert!(stats.recent.iter().any(|n| n == "PLAY_BGM_HA_FI_AREA00"));
        let rt = app.world().resource::<AudioRuntime>();
        assert_eq!(rt.cache_bytes(), (44 + 1600) + (44 + 8000));
        // Battle music file is missing: silent, area music fades out.
        app.world_mut().write_message(PlayCue { cue: Cue::Music(MusicId::WildBattle), delay: 0.0 });
        app.update();
        assert_eq!(app.world().resource::<AudioStats>().silent, 2);
        assert!(app.world().resource::<AudioRuntime>().fader.current().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn same_file_is_loaded_once_and_cue_delays_wait() {
        let dir = temp("cache");
        let mut app = app(&dir);
        app.update();
        for _ in 0..3 {
            app.world_mut().write_message(PlayCue { cue: Cue::Sfx(SfxId::UiCursor), delay: 0.0 });
        }
        app.world_mut().write_message(PlayCue { cue: Cue::Sfx(SfxId::UiCursor), delay: 3600.0 });
        app.update();
        let rt = app.world().resource::<AudioRuntime>();
        assert_eq!(rt.cache_bytes(), 44 + 1600);
        assert_eq!(rt.pending.len(), 1);
        assert_eq!(app.world().resource::<AudioStats>().voices, 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn off_mode_registers_the_api_but_does_nothing() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, GameAudioPlugin::with_mode(AudioMode::Off)));
        app.world_mut().write_message(PlayCue { cue: Cue::Sfx(SfxId::UiCursor), delay: 0.0 });
        app.update();
        assert!(app.world().get_resource::<AudioRuntime>().is_none());
        assert_eq!(app.world().resource::<AudioStats>().executed, 0);
    }

    #[test]
    fn pause_menu_navigation_makes_sounds() {
        let dir = temp("pause");
        let mut app = app(&dir);
        app.insert_resource(Paused(false)).insert_resource(PauseMenu::default());
        app.update();
        app.world_mut().resource_mut::<Paused>().0 = true;
        app.update();
        app.world_mut().resource_mut::<PauseMenu>().selected = 2;
        app.update();
        app.world_mut().resource_mut::<Paused>().0 = false;
        app.update();
        let recent: Vec<String> = app.world().resource::<AudioStats>().recent.iter().cloned().collect();
        assert_eq!(
            recent,
            vec!["PLAY_UI_COMMON_PAUSE_MENU_OPEN", "Play_UI_common_select", "PLAY_UI_COMMON_PAUSE_MENU_CLOSE"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn zone_music_follows_the_area() {
        let dir = temp("zone");
        let mut app = app(&dir);
        app.insert_resource(ActiveZoneInfo { area_idx: 0, name: "a".into() });
        app.update();
        app.update();
        app.world_mut().resource_mut::<ActiveZoneInfo>().area_idx = 9;
        app.update();
        let recent: Vec<String> = app.world().resource::<AudioStats>().recent.iter().cloned().collect();
        assert_eq!(recent, vec!["PLAY_BGM_HA_FI_AREA00", "<stop music>"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
