//! World events (Brief 39): space-time distortions and mass outbreaks in the overworld.
//!
//! Pure parts live in [`super::distortion`] and [`super::outbreak`]; this module holds the shared
//! sheet helpers and the Bevy glue ([`WorldEventsPlugin`]).
//!
//! How events reach the world: the plugin edits [`ActiveAreaSpawners`] (the list the viewer's
//! wild streaming reads). Each alive event Pokémon is one synthetic `SpawnerDefinition` with id
//! `we:<slot key>#<generation>`, so the existing streaming code spawns it like any wild creature
//! (same model loading, encounters, battles, capture). The 227 outbreak spawners of the regular
//! spawn table are removed while their outbreak is not active, and the regular spawners an active
//! outbreak switches off are removed while it runs. A defeated or caught event Pokémon is seen
//! through [`ConsumedSpawners`]; its slot gets a new generation (= a new spawner id).
//!
//! Conditions: species weights are multiplied by the decoded time-slot and weather multipliers of
//! each slot (from the sheets; 0 = never). Alpha slots are skipped because `WildCreature` has no
//! alpha flag yet (Brief 17). Story value: the port has no story progress, so `--story <n>`
//! sets it; default [`DEFAULT_STORY`] (ASSUMED: post-game, everything unlocked).
//!
//! Cues: every transition is printed, sent as a [`WorldEventCue`] message and shown in the HUD
//! toast; visuals are a translucent dome sphere (distortion), a thin pillar (outbreak) and small
//! glowing loot markers. No game effect, sound or BGM asset is used (their names are known from
//! the code reading, the assets are not ported). Events are session-only: nothing is saved
//! (save format stays v7).
//!
//! Flags: `--no-world-events`, `--story <n>`, `--event-seed <u64>`, `--event-time-scale <f>`
//! (speeds the distortion timers), `--distortion-now` (skip the first Idle timer),
//! `--outbreak <id|off>` (force the outbreak of the start area or disable outbreaks).

use std::collections::{BTreeSet, HashMap};

use bevy::prelude::*;

use super::clock::GameClock;
use super::distortion::{
    DistortionCue, DistortionData, DistortionMachine, EventSpawnSlot, Phase, WormholeConfig, ITEM_DROPS_PER_DISTORTION,
};
use super::input_map::{Action, ActionState};
use super::outbreak::{encount_keys, massive_outbreak_report, MassiveOutbreakReport, OutbreakData};
use super::save_wire::{PlayerProgress, SaveToast};
use super::state::{not_paused, GameState};
use super::ui::ActiveZoneInfo;
use super::weather::WeatherState;
use super::wild::{splitmix64, ActiveAreaSpawners, ConsumedSpawners, SpawnSpeciesEntry, SpawnerDefinition, WildCreature};
use super::world_objects::find_data_file;
use super::PlayerAvatar;

/// Story value used when `--story` is absent (ASSUMED: past every decoded gate, the port has no
/// story progress).
pub const DEFAULT_STORY: u32 = 100_000;
/// Prefix of every synthetic spawner id.
pub const EVENT_ID_PREFIX: &str = "we:";
/// Loot pick-up range in metres (same as `pickups::PICK_RANGE`).
pub const LOOT_RANGE: f32 = 3.0;

// ---------------------------------------------------------------- sheet helpers

/// A typed TSV sheet: `#` lines skipped, first other line is the header (`name:type`).
pub struct Sheet<'a> {
    cols: HashMap<&'a str, usize>,
    rows: Vec<Vec<&'a str>>,
}

impl<'a> Sheet<'a> {
    pub fn parse(text: &'a str) -> Self {
        let mut cols = HashMap::new();
        let mut rows = Vec::new();
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            let c: Vec<&str> = line.split('\t').collect();
            if cols.is_empty() {
                for (i, h) in c.iter().enumerate() {
                    cols.insert(h.split(':').next().unwrap_or(h), i);
                }
            } else {
                rows.push(c);
            }
        }
        Self { cols, rows }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Cell of row `r`, column `name`; empty when either is missing.
    pub fn get(&self, r: usize, name: &str) -> &'a str {
        self.cols.get(name).and_then(|&i| self.rows.get(r).and_then(|row| row.get(i))).copied().unwrap_or("")
    }
}

/// One encounter slot with its decoded time / weather multipliers.
#[derive(Debug, Clone, PartialEq)]
pub struct EventSlot {
    pub species_id: u16,
    pub species: String,
    pub form: u16,
    pub alpha: bool,
    pub weight: u16,
    pub min_level: u8,
    pub max_level: u8,
    /// Morning, day, evening, night (slot 3 = night VERIFIED by spawn data; 0..2 order ASSUMED).
    pub time: [f32; 4],
    /// Weather ids 2..9: sunny, cloudy, rain, snow, drought, fog, rainstorm, snowstorm.
    pub weather: [f32; 8],
    pub condition: String,
}

const TIME_COLS: [&str; 4] = ["time_morning", "time_day", "time_evening", "time_night"];
const WEATHER_COLS: [&str; 8] = [
    "weather_sunny",
    "weather_cloudy",
    "weather_rain",
    "weather_snow",
    "weather_drought",
    "weather_fog",
    "weather_rainstorm",
    "weather_snowstorm",
];

/// Parse one slot row (columns of `world_events_distortion_encounters` / `..._outbreak_spawners`).
pub fn parse_slot(s: &Sheet, r: usize) -> Option<EventSlot> {
    let species_id = s.get(r, "species_id").parse().ok()?;
    let min_level: u8 = s.get(r, "min_level").parse().unwrap_or(1);
    let mult = |n: &str| s.get(r, n).parse::<f32>().ok().filter(|v| v.is_finite()).unwrap_or(1.0);
    Some(EventSlot {
        species_id,
        species: s.get(r, "species").to_lowercase(),
        form: s.get(r, "form").parse().unwrap_or(0),
        alpha: s.get(r, "alpha") == "1",
        weight: s.get(r, "weight").parse().unwrap_or(0),
        min_level,
        max_level: s.get(r, "max_level").parse().unwrap_or(min_level).max(min_level),
        time: TIME_COLS.map(mult),
        weather: WEATHER_COLS.map(mult),
        condition: s.get(r, "condition").to_string(),
    })
}

/// Effective weight now: `weight x time[slot] x weather[id]` (unknown weather id = factor 1).
pub fn effective_weight(slot: &EventSlot, time_slot: usize, weather_id: u8) -> f32 {
    let t = slot.time[time_slot.min(3)];
    let w = match weather_id {
        2..=9 => slot.weather[usize::from(weather_id - 2)],
        _ => 1.0,
    };
    (f32::from(slot.weight) * t * w).max(0.0)
}

/// Species entries for the existing spawner code: alpha slots skipped, zero weights removed.
pub fn entries_for(slots: &[EventSlot], time_slot: usize, weather_id: u8) -> Vec<SpawnSpeciesEntry> {
    slots
        .iter()
        .filter(|s| !s.alpha)
        .filter_map(|s| {
            let w = effective_weight(s, time_slot, weather_id);
            (w > 0.0).then(|| SpawnSpeciesEntry {
                species_id: s.species_id,
                species_name: s.species.clone(),
                min_level: s.min_level,
                max_level: s.max_level,
                weight: w.round().clamp(1.0, f32::from(u16::MAX)) as u16,
            })
        })
        .collect()
}

/// Uniform integer in `lo..=hi` (hi < lo -> lo).
pub fn rng_range(rng: &mut u64, lo: u32, hi: u32) -> u32 {
    if hi <= lo {
        return lo;
    }
    lo + (splitmix64(rng) % u64::from(hi - lo + 1)) as u32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmp {
    /// No or unresolved comparator: always true.
    Any,
    Le,
    Ge,
    Between,
    Eq,
    Ne,
}

/// A `phase_condition` on `WPHA_STORY` (comparator names resolved by the decoder).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoryGate {
    pub cmp: Cmp,
    pub a: u32,
    pub b: u32,
}

impl StoryGate {
    pub const ANY: Self = Self { cmp: Cmp::Any, a: 0, b: 0 };

    pub fn new(cmp: &str, a: &str, b: &str) -> Self {
        let (Ok(av), bv) = (a.parse::<u32>(), b.parse::<u32>().ok()) else { return Self::ANY };
        let cmp = match cmp {
            "less_than_equal" => Cmp::Le,
            "greater_than_equal" => Cmp::Ge,
            "between" => Cmp::Between,
            "equal" => Cmp::Eq,
            "not_equal" => Cmp::Ne,
            _ => Cmp::Any,
        };
        Self { cmp, a: av, b: bv.unwrap_or(av) }
    }

    pub fn matches(&self, story: u32) -> bool {
        match self.cmp {
            Cmp::Any => true,
            Cmp::Le => story <= self.a,
            Cmp::Ge => story >= self.a,
            Cmp::Between => (self.a..=self.b).contains(&story),
            Cmp::Eq => story == self.a,
            Cmp::Ne => story != self.a,
        }
    }
}

// ---------------------------------------------------------------- runtime state (pure)

/// Options from the command line.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldEventsConfig {
    pub enabled: bool,
    pub story: u32,
    pub seed: u64,
    pub time_scale: f32,
    pub distortion_now: bool,
    /// `Some(None)` = outbreaks off, `Some(Some(id))` = force that id in the start area.
    pub force_outbreak: Option<Option<u16>>,
}

impl Default for WorldEventsConfig {
    fn default() -> Self {
        Self { enabled: true, story: DEFAULT_STORY, seed: 0x3e7e_17a5, time_scale: 1.0, distortion_now: false, force_outbreak: None }
    }
}

impl WorldEventsConfig {
    pub fn from_args(args: &[String]) -> Self {
        let val = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
        let d = Self::default();
        Self {
            enabled: !args.iter().any(|a| a == "--no-world-events"),
            story: val("--story").and_then(|s| s.parse().ok()).unwrap_or(d.story),
            seed: val("--event-seed").and_then(|s| s.parse().ok()).unwrap_or(d.seed),
            time_scale: val("--event-time-scale")
                .and_then(|s| s.parse::<f32>().ok())
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or(1.0),
            distortion_now: args.iter().any(|a| a == "--distortion-now"),
            force_outbreak: val("--outbreak").map(|s| s.parse::<u16>().ok()),
        }
    }
}

/// One alive-Pokémon slot of an event and its refill state.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotState {
    pub slot: EventSpawnSlot,
    pub generation: u32,
    /// Seconds until a cleared slot spawns again (`None` = filled).
    pub respawn_in: Option<f32>,
}

impl SlotState {
    fn new(slot: EventSpawnSlot) -> Self {
        Self { slot, generation: 0, respawn_in: None }
    }

    /// Current synthetic spawner id.
    pub fn spawner_id(&self) -> String {
        format!("{EVENT_ID_PREFIX}{}#{}", self.slot.key, self.generation)
    }
}

/// The running outbreak of the current area.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveOutbreak {
    pub area: String,
    pub id: u16,
    pub day: u32,
    pub species: String,
    pub total: u16,
    pub cleared: u16,
    pub slots: Vec<SlotState>,
}

impl ActiveOutbreak {
    /// Slots still allowed to hold a Pokémon: never more than what is left of the total.
    fn live_slots(&self) -> usize {
        usize::from(self.total.saturating_sub(self.cleared)).min(self.slots.len())
    }
}

/// One loot drop of the active distortion.
#[derive(Debug, Clone, PartialEq)]
pub struct LootDrop {
    pub pos: Vec3,
    pub item: u16,
    pub taken: bool,
}

/// Kinds of [`WorldEventCue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldEventKind {
    DistortionNotice,
    DistortionAppearCountdown,
    DistortionMaterialize,
    DistortionVanishWarning,
    DistortionEnd,
    OutbreakStart,
    OutbreakEnd,
    LootTaken,
}

/// Hook for HUD / audio / effects: emitted on every event transition. No asset is attached.
#[derive(Message, Debug, Clone, PartialEq)]
pub struct WorldEventCue {
    pub kind: WorldEventKind,
    pub pos: Vec3,
    pub text: String,
}

/// Everything the plugin knows. Public for tests and HUD use.
#[derive(Resource, Debug, Clone)]
pub struct WorldEvents {
    pub config: WorldEventsConfig,
    pub wormhole: Option<WormholeConfig>,
    pub distortions: DistortionData,
    pub outbreaks: OutbreakData,
    pub mmo: Option<MassiveOutbreakReport>,
    /// Main area (`ha_areaNN`) the events run in.
    pub area: String,
    pub day: u32,
    pub machine: Option<DistortionMachine>,
    pub distortion_slots: Vec<SlotState>,
    pub outbreak: Option<ActiveOutbreak>,
    pub loot: Vec<LootDrop>,
    /// Recent cue lines (last 32), for tests and a future HUD.
    pub log: Vec<String>,
    rng: u64,
    last_minutes: Option<u16>,
    /// Regular spawners taken out of `ActiveAreaSpawners` while suppressed (restored after).
    stash: HashMap<String, SpawnerDefinition>,
    applied_len: usize,
    applied_conditions: (usize, u8),
    dirty: bool,
    start_area_forced: bool,
}

impl WorldEvents {
    pub fn new(config: WorldEventsConfig, wormhole: Option<WormholeConfig>, distortions: DistortionData, outbreaks: OutbreakData) -> Self {
        let rng = config.seed ^ 0xd157_0271;
        Self {
            config,
            wormhole,
            distortions,
            outbreaks,
            mmo: None,
            area: String::new(),
            day: 0,
            machine: None,
            distortion_slots: Vec::new(),
            outbreak: None,
            loot: Vec::new(),
            log: Vec::new(),
            rng,
            last_minutes: None,
            stash: HashMap::new(),
            applied_len: usize::MAX,
            applied_conditions: (usize::MAX, 0),
            dirty: true,
            start_area_forced: false,
        }
    }

    fn note(&mut self, line: String) {
        println!("World events: {line}");
        self.log.push(line);
        if self.log.len() > 32 {
            self.log.remove(0);
        }
    }

    /// Enter `area` (main area id): reset the distortion machine and pick the day's outbreak.
    pub fn enter_area(&mut self, area: &str) -> Vec<WorldEventCue> {
        self.area = area.to_string();
        self.distortion_slots.clear();
        self.loot.clear();
        self.stash.clear();
        self.machine = self.wormhole.as_ref().map(|cfg| {
            let mut m = DistortionMachine::new(cfg, &mut self.rng);
            if self.config.distortion_now {
                m.force_soon();
            }
            m
        });
        let candidates = self.distortions.candidates(area, self.config.story).len();
        self.note(format!(
            "{area}: {candidates} distortion site(s) open at story {}; first check in {:.0} s",
            self.config.story,
            self.machine.as_ref().map_or(0.0, |m| m.remaining_s)
        ));
        self.dirty = true;
        self.roll_outbreak()
    }

    /// Choose the outbreak of the current area for the current day.
    pub fn roll_outbreak(&mut self) -> Vec<WorldEventCue> {
        let mut cues = Vec::new();
        if let Some(old) = self.outbreak.take() {
            cues.push(self.cue(WorldEventKind::OutbreakEnd, Vec3::ZERO, format!("outbreak {} ({}) is over (day changed)", old.id, old.species)));
        }
        let area = self.area.clone();
        let id = match self.config.force_outbreak {
            Some(None) => None,
            Some(Some(id)) if !self.start_area_forced => {
                self.start_area_forced = true;
                self.outbreaks.spawners.contains_key(&(area.clone(), id)).then_some(id)
            }
            _ => self.outbreaks.draw(&area, self.config.story, self.day, self.config.seed),
        };
        if let Some(id) = id {
            let slots: Vec<SlotState> = self.outbreaks.spawn_slots(&area, id, self.config.seed ^ u64::from(self.day)).into_iter().map(SlotState::new).collect();
            let species = self.outbreaks.species(&area, id).unwrap_or("?").to_string();
            let total = self.outbreaks.roll_total(&area, id, self.day, self.config.seed);
            let pos = slots.first().map_or(Vec3::ZERO, |s| s.slot.pos);
            cues.push(self.cue(
                WorldEventKind::OutbreakStart,
                pos,
                format!("mass outbreak {id} in {area}: {total} x {species} near ({:.0}, {:.0})", pos.x, pos.z),
            ));
            self.outbreak = Some(ActiveOutbreak { area, id, day: self.day, species, total, cleared: 0, slots });
        }
        self.dirty = true;
        cues
    }

    fn cue(&mut self, kind: WorldEventKind, pos: Vec3, text: String) -> WorldEventCue {
        self.note(text.clone());
        WorldEventCue { kind, pos, text }
    }

    /// A new game day began: draw a new outbreak.
    pub fn new_day(&mut self) -> Vec<WorldEventCue> {
        self.day = self.day.wrapping_add(1);
        self.roll_outbreak()
    }

    /// Advance the distortion machine and the refill timers by `dt` real seconds.
    pub fn tick(&mut self, dt: f32, player: Option<Vec3>) -> Vec<WorldEventCue> {
        let mut out = Vec::new();
        if !dt.is_finite() || dt <= 0.0 {
            return out;
        }
        let scaled = dt * self.config.time_scale;
        if let (Some(cfg), Some(mut m)) = (self.wormhole.clone(), self.machine.take()) {
            let candidates = self.distortions.candidates(&self.area, self.config.story);
            let sites = &self.distortions.sites;
            let dist = |i: usize| player.map_or(0.0, |p| sites.get(i).map_or(0.0, |s| s.centre.distance(p)));
            let cues = m.tick(scaled, &cfg, &candidates, dist, &mut self.rng);
            let serial = m.serial;
            self.machine = Some(m);
            for c in cues {
                out.extend(self.apply_distortion_cue(c, serial));
            }
            // Refill timers of cleared distortion slots (kWhSpawnTimeMin/Max seconds, ASSUMED).
            for s in &mut self.distortion_slots {
                if let Some(t) = s.respawn_in.as_mut() {
                    *t -= scaled;
                    if *t <= 0.0 {
                        s.respawn_in = None;
                        s.generation += 1;
                        self.dirty = true;
                    }
                }
            }
        }
        out
    }

    fn apply_distortion_cue(&mut self, c: DistortionCue, serial: u32) -> Vec<WorldEventCue> {
        let site_of = |d: &DistortionData, i: usize| d.sites.get(i).map(|s| (s.name.clone(), s.centre));
        let mut out = Vec::new();
        match c {
            DistortionCue::NoSite => self.note(format!("{}: no distortion site open at story {}", self.area, self.config.story)),
            DistortionCue::NoticeStart { site } => {
                let (name, pos) = site_of(&self.distortions, site).unwrap_or_default();
                let left = self.machine.as_ref().map_or(0.0, |m| m.remaining_s);
                out.push(self.cue(WorldEventKind::DistortionNotice, pos, format!("a space-time distortion is forming at {name} ({:.0}, {:.0}); it appears in {left:.0} s", pos.x, pos.z)));
            }
            DistortionCue::AppearCountdown { site } => {
                let (name, pos) = site_of(&self.distortions, site).unwrap_or_default();
                out.push(self.cue(WorldEventKind::DistortionAppearCountdown, pos, format!("distortion {name} appears in a few seconds")));
            }
            DistortionCue::Materialize { site } => {
                let seed = self.config.seed ^ u64::from(serial) << 20;
                self.distortion_slots = self.distortions.spawn_slots(site, self.config.story, seed).into_iter().map(SlotState::new).collect();
                for s in &mut self.distortion_slots {
                    s.slot.key = format!("{}@{serial}", s.slot.key);
                }
                self.loot = self
                    .distortions
                    .loot_plan(site, ITEM_DROPS_PER_DISTORTION, seed)
                    .into_iter()
                    .map(|(pos, item)| LootDrop { pos, item, taken: false })
                    .collect();
                self.dirty = true;
                let (name, pos) = site_of(&self.distortions, site).unwrap_or_default();
                let n = self.distortion_slots.len();
                let items = self.loot.len();
                out.push(self.cue(WorldEventKind::DistortionMaterialize, pos, format!("distortion {name} is active: {n} Pokémon slot(s), {items} item(s)")));
            }
            DistortionCue::VanishWarning { site, last } => {
                let (name, pos) = site_of(&self.distortions, site).unwrap_or_default();
                let left = self.machine.as_ref().map_or(0.0, |m| m.remaining_s);
                let what = if last { "final" } else { "middle" };
                out.push(self.cue(WorldEventKind::DistortionVanishWarning, pos, format!("distortion {name} fades in {left:.0} s ({what} warning)")));
            }
            DistortionCue::End { site } => {
                self.distortion_slots.clear();
                self.loot.clear();
                self.dirty = true;
                let (name, pos) = site_of(&self.distortions, site).unwrap_or_default();
                out.push(self.cue(WorldEventKind::DistortionEnd, pos, format!("distortion {name} is over")));
            }
        }
        out
    }

    /// Count defeated / caught event Pokémon (their spawner id was consumed).
    pub fn observe_consumed(&mut self, consumed: impl Fn(&str) -> bool) -> Vec<WorldEventCue> {
        let mut out = Vec::new();
        let (min_s, max_s) = self.wormhole.as_ref().map_or((15.0, 23.0), |c| (c.spawn_time_min_s, c.spawn_time_max_s));
        for i in 0..self.distortion_slots.len() {
            let s = &self.distortion_slots[i];
            if s.respawn_in.is_none() && consumed(&s.spawner_id()) {
                let span = (max_s - min_s).max(0.0);
                let r = (splitmix64(&mut self.rng) % 1000) as f32 / 1000.0;
                self.distortion_slots[i].respawn_in = Some(min_s + span * r);
                self.dirty = true;
            }
        }
        let mut ended = None;
        if let Some(o) = self.outbreak.as_mut() {
            let live = o.live_slots();
            for s in o.slots.iter_mut().take(live) {
                if consumed(&s.spawner_id()) {
                    o.cleared += 1;
                    s.generation += 1;
                    self.dirty = true;
                }
            }
            if o.cleared >= o.total {
                ended = Some((o.id, o.species.clone(), o.cleared));
            }
        }
        if let Some((id, species, n)) = ended {
            self.outbreak = None;
            out.push(self.cue(WorldEventKind::OutbreakEnd, Vec3::ZERO, format!("outbreak {id} cleared: {n} x {species}")));
        }
        out
    }

    /// The synthetic spawners that must exist now (entries follow time slot and weather).
    pub fn desired_spawners(&self, time_slot: usize, weather_id: u8) -> Vec<SpawnerDefinition> {
        let mut out = Vec::new();
        let def = |s: &SlotState| SpawnerDefinition {
            spawner_id: s.spawner_id(),
            area: self.area.clone(),
            pos: s.slot.pos,
            radius: s.slot.radius,
            entries: s.slot.entries(time_slot, weather_id),
        };
        if self.machine.as_ref().is_some_and(|m| m.phase == Phase::Active) {
            out.extend(self.distortion_slots.iter().filter(|s| s.respawn_in.is_none()).map(def));
        }
        if let Some(o) = &self.outbreak {
            out.extend(o.slots.iter().take(o.live_slots()).map(def));
        }
        out
    }

    /// Regular spawner ids that must be absent now: every inactive outbreak spawner of the area
    /// and the spawners the active outbreak switches off.
    pub fn suppressed_spawners(&self) -> BTreeSet<String> {
        let mut out = self.outbreaks.outbreak_spawner_ids(&self.area);
        if let Some(o) = &self.outbreak {
            if let Some(d) = self.outbreaks.disabled.get(&(o.area.clone(), o.id)) {
                out.extend(d.iter().cloned());
            }
        }
        out
    }

    /// Bring `active` in line with the events. Returns the entities to despawn (spawners that
    /// left the list while their creature was alive). Creatures in battle are kept for now.
    pub fn reconcile(&mut self, active: &mut ActiveAreaSpawners, time_slot: usize, weather_id: u8, in_battle: impl Fn(Entity) -> bool) -> Vec<Entity> {
        let conditions = (time_slot, weather_id);
        if !self.dirty && active.spawners.len() == self.applied_len && conditions == self.applied_conditions {
            return Vec::new();
        }
        let desired = self.desired_spawners(time_slot, weather_id);
        let desired_ids: BTreeSet<String> = desired.iter().map(|d| d.spawner_id.clone()).collect();
        let suppressed = self.suppressed_spawners();
        let mut despawn = Vec::new();
        let mut kept_busy = false;
        let mut stashed = Vec::new();
        active.spawners.retain(|s| {
            let event = s.spawner_id.starts_with(EVENT_ID_PREFIX);
            let unwanted = suppressed.contains(&s.spawner_id) || (event && !desired_ids.contains(&s.spawner_id));
            if !unwanted {
                return true;
            }
            if let Some(&e) = active.spawned_entities.get(&s.spawner_id) {
                if in_battle(e) {
                    kept_busy = true;
                    return true;
                }
            }
            if !event {
                stashed.push(s.clone());
            }
            false
        });
        for s in stashed {
            self.stash.insert(s.spawner_id.clone(), s);
        }
        // Suppression lifted (outbreak over): put the regular spawners back.
        let back: Vec<String> = self.stash.keys().filter(|id| !suppressed.contains(*id)).cloned().collect();
        for id in back {
            if let Some(def) = self.stash.remove(&id) {
                if !active.spawners.iter().any(|s| s.spawner_id == id) {
                    active.spawners.push(def);
                }
            }
        }
        let listed: BTreeSet<String> = active.spawners.iter().map(|s| s.spawner_id.clone()).collect();
        active.spawned_entities.retain(|id, e| {
            if listed.contains(id) {
                true
            } else {
                despawn.push(*e);
                false
            }
        });
        for d in desired {
            match active.spawners.iter_mut().find(|s| s.spawner_id == d.spawner_id) {
                Some(s) => s.entries = d.entries,
                None => active.spawners.push(d),
            }
        }
        self.applied_len = active.spawners.len();
        self.applied_conditions = conditions;
        self.dirty = kept_busy;
        despawn
    }

    /// Take the nearest untaken loot within [`LOOT_RANGE`] of `player`; returns its item id.
    pub fn take_loot(&mut self, player: Vec3) -> Option<(usize, u16)> {
        let p = Vec2::new(player.x, player.z);
        let (i, _) = self
            .loot
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.taken)
            .map(|(i, l)| (i, p.distance(Vec2::new(l.pos.x, l.pos.z))))
            .filter(|(_, d)| *d <= LOOT_RANGE)
            .min_by(|a, b| a.1.total_cmp(&b.1))?;
        self.loot[i].taken = true;
        Some((i, self.loot[i].item))
    }
}

/// Track the game day from the clock: a wrap of the minute-of-day counts as a new day.
pub fn day_rolled(last: Option<u16>, now: u16) -> bool {
    last.is_some_and(|l| now < l && l - now > 720)
}

// ---------------------------------------------------------------- Bevy side

/// Installs the world events for the start area. Data files missing -> a message and nothing.
pub struct WorldEventsPlugin {
    pub config: WorldEventsConfig,
}

impl WorldEventsPlugin {
    pub fn from_args(args: &[String]) -> Self {
        Self { config: WorldEventsConfig::from_args(args) }
    }
}

/// Load every sheet the events need (`None` when a required sheet is missing).
pub fn load_world_events(config: WorldEventsConfig) -> Option<WorldEvents> {
    let read = |n: &str| find_data_file(n).and_then(|p| std::fs::read_to_string(p).ok());
    let wormhole = read("world/wormhole_config_values.tsv").and_then(|t| WormholeConfig::parse(&t));
    let distortions = DistortionData::parse(
        &read("world_events_distortion_sites.tsv")?,
        &read("world_events_distortion_spawners.tsv")?,
        &read("world_events_distortion_encounters.tsv")?,
        &read("world_events_distortion_items.tsv")?,
        &read("world_events_distortion_points.tsv")?,
    );
    let table = read("field_spawn_table.tsv")?;
    let outbreaks = OutbreakData::parse(&read("world_events_outbreak_areas.tsv")?, &read("world_events_outbreak_spawners.tsv")?, &table);
    let mut ev = WorldEvents::new(config, wormhole, distortions, outbreaks);
    if let (Some(groups), Some(cond)) = (read("world_events_mmo_groups.tsv"), read("field_spawn_conditions.tsv")) {
        let mut known = encount_keys(&cond);
        if let Some(d) = read("world_events_distortion_encounters.tsv") {
            known.extend(encount_keys(&d));
        }
        ev.mmo = Some(massive_outbreak_report(&groups, &known));
    }
    Some(ev)
}

impl Plugin for WorldEventsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<WorldEventCue>();
        if !self.config.enabled {
            println!("World events: disabled (--no-world-events).");
            return;
        }
        let Some(ev) = load_world_events(self.config.clone()) else {
            println!("World events: event sheets not found under data/; skipping.");
            return;
        };
        println!(
            "World events: {} distortion sites, {} outbreak spawners, timers {} (session only, not saved).",
            ev.distortions.sites.len(),
            ev.outbreaks.spawners.len(),
            if ev.wormhole.is_some() { "decoded" } else { "MISSING: distortions off" }
        );
        if let Some(r) = &ev.mmo {
            println!(
                "World events: massive outbreaks: {} groups, {}/{} encounter keys resolved -> {} usable; species UNKNOWN, skipped.",
                r.groups, r.resolved_keys, r.keys, r.usable_groups
            );
        }
        app.insert_resource(ev).add_systems(
            Update,
            (
                track_area_and_day,
                tick_world_events.run_if(in_state(GameState::World)).run_if(not_paused),
                observe_consumed.run_if(in_state(GameState::World)),
                reconcile_spawners,
                sync_event_markers,
                loot_interact.run_if(in_state(GameState::World)).run_if(not_paused),
            )
                .chain(),
        );
    }
}

fn send(cues: Vec<WorldEventCue>, out: &mut MessageWriter<WorldEventCue>, toast: &mut Option<ResMut<SaveToast>>) {
    for c in cues {
        if let Some(t) = toast.as_deref_mut() {
            t.show(c.text.clone());
        }
        out.write(c);
    }
}

fn track_area_and_day(
    mut ev: ResMut<WorldEvents>,
    zone: Option<Res<ActiveZoneInfo>>,
    clock: Option<Res<GameClock>>,
    mut out: MessageWriter<WorldEventCue>,
    mut toast: Option<ResMut<SaveToast>>,
) {
    let area = zone.map(|z| super::zone::main_area_of(&z.area).to_string()).unwrap_or_default();
    if !area.is_empty() && area != ev.area {
        let cues = ev.enter_area(&area);
        send(cues, &mut out, &mut toast);
    }
    if let Some(clock) = clock {
        let now = clock.minutes();
        if day_rolled(ev.last_minutes, now) {
            let cues = ev.new_day();
            send(cues, &mut out, &mut toast);
        }
        ev.last_minutes = Some(now);
    }
}

fn tick_world_events(
    time: Res<Time>,
    mut ev: ResMut<WorldEvents>,
    player: Query<&Transform, With<PlayerAvatar>>,
    mut out: MessageWriter<WorldEventCue>,
    mut toast: Option<ResMut<SaveToast>>,
) {
    let p = player.iter().next().map(|t| t.translation);
    let cues = ev.tick(time.delta_secs(), p);
    send(cues, &mut out, &mut toast);
}

fn observe_consumed(
    time: Res<Time>,
    mut ev: ResMut<WorldEvents>,
    consumed: Option<Res<ConsumedSpawners>>,
    mut out: MessageWriter<WorldEventCue>,
    mut toast: Option<ResMut<SaveToast>>,
) {
    let Some(consumed) = consumed else { return };
    let now = time.elapsed_secs_f64();
    let cues = ev.observe_consumed(|id| consumed.is_consumed(id, now));
    send(cues, &mut out, &mut toast);
}

fn reconcile_spawners(
    mut commands: Commands,
    mut ev: ResMut<WorldEvents>,
    active: Option<ResMut<ActiveAreaSpawners>>,
    clock: Option<Res<GameClock>>,
    weather: Option<Res<WeatherState>>,
    wild: Query<&WildCreature>,
) {
    let Some(mut active) = active else { return };
    let slot = clock.map_or(1, |c| c.slot());
    let weather_id = weather.map_or(2, |w| w.machine.id);
    let busy = |e: Entity| wild.get(e).is_ok_and(|w| w.in_battle);
    for e in ev.reconcile(&mut active, slot, weather_id, busy) {
        commands.entity(e).despawn();
    }
}

/// Marker entities (dome, outbreak pillar, loot).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldEventMarker {
    Dome,
    OutbreakPillar,
    Loot(usize),
}

/// What markers should exist now: `(marker, position, alpha)`.
pub fn wanted_markers(ev: &WorldEvents) -> Vec<(WorldEventMarker, Vec3, f32)> {
    let mut out = Vec::new();
    if let Some(m) = &ev.machine {
        if let Some(site) = m.site.and_then(|i| ev.distortions.sites.get(i)) {
            // Faint while forming, stronger while active (ASSUMED look; no effect asset ported).
            let alpha = match m.phase {
                Phase::Notice => 0.10,
                Phase::Active => 0.22,
                _ => 0.0,
            };
            if alpha > 0.0 {
                out.push((WorldEventMarker::Dome, site.centre, alpha));
            }
        }
        if m.phase == Phase::Active {
            for (i, l) in ev.loot.iter().enumerate().filter(|(_, l)| !l.taken) {
                out.push((WorldEventMarker::Loot(i), l.pos, 1.0));
            }
        }
    }
    if let Some(o) = &ev.outbreak {
        if let Some(sp) = ev.outbreaks.spawners.get(&(o.area.clone(), o.id)) {
            out.push((WorldEventMarker::OutbreakPillar, sp.pos, 0.5));
        }
    }
    out
}

/// Dome radius of the main distortion spawner (decoded 50 m).
const DOME_RADIUS: f32 = 50.0;

/// World position of a marker entity for an event position `p`.
pub fn marker_pos(m: WorldEventMarker, p: Vec3) -> Vec3 {
    match m {
        WorldEventMarker::Dome => p,
        WorldEventMarker::OutbreakPillar => p + Vec3::Y * 20.0,
        WorldEventMarker::Loot(_) => p + Vec3::Y * 0.5,
    }
}

fn sync_event_markers(
    mut commands: Commands,
    ev: Res<WorldEvents>,
    markers: Query<(Entity, &WorldEventMarker, &Transform)>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    if !ev.is_changed() {
        return;
    }
    let wanted = wanted_markers(&ev);
    let mut have: Vec<WorldEventMarker> = Vec::new();
    for (e, m, t) in &markers {
        let keep = wanted.iter().any(|(w, p, _)| w == m && t.translation.distance(marker_pos(*w, *p)) < 0.01) && !have.contains(m);
        if keep {
            have.push(*m);
        } else {
            commands.entity(e).despawn();
        }
    }
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        // Headless apps without asset storage: markers as bare transforms (tests count them).
        for (m, p, _) in wanted.into_iter().filter(|(m, _, _)| !have.contains(m)) {
            commands.spawn((m, Transform::from_translation(marker_pos(m, p))));
        }
        return;
    };
    for (m, p, alpha) in wanted.into_iter().filter(|(m, _, _)| !have.contains(m)) {
        let (mesh, color, emissive) = match m {
            WorldEventMarker::Dome => (meshes.add(Sphere::new(DOME_RADIUS)), Color::srgba(0.55, 0.35, 1.0, alpha), LinearRgba::rgb(0.6, 0.3, 1.2)),
            WorldEventMarker::OutbreakPillar => (meshes.add(Cylinder::new(0.6, 40.0)), Color::srgba(1.0, 0.25, 0.2, alpha), LinearRgba::rgb(2.0, 0.4, 0.3)),
            WorldEventMarker::Loot(_) => (meshes.add(Sphere::new(0.35)), Color::srgb(1.0, 0.9, 0.4), LinearRgba::rgb(3.0, 2.4, 0.6)),
        };
        commands.spawn((
            m,
            Mesh3d(mesh),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                emissive,
                alpha_mode: if alpha < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque },
                cull_mode: None,
                unlit: matches!(m, WorldEventMarker::Dome),
                ..default()
            })),
            Transform::from_translation(marker_pos(m, p)),
        ));
    }
}

fn loot_interact(
    actions: Option<ResMut<ActionState>>,
    mut ev: ResMut<WorldEvents>,
    player: Query<&Transform, With<PlayerAvatar>>,
    progress: Option<ResMut<PlayerProgress>>,
    mut out: MessageWriter<WorldEventCue>,
    mut toast: Option<ResMut<SaveToast>>,
) {
    let Some(mut actions) = actions else { return };
    if ev.loot.is_empty() || !actions.just_pressed(Action::Confirm) {
        return;
    }
    let Some(p) = player.iter().next().map(|t| t.translation) else { return };
    let Some((_, item)) = ev.take_loot(p) else { return };
    actions.consume(Action::Confirm);
    let stored = progress.map_or(0, |mut pr| super::pickups::add_to_bag(&mut pr.bag, item, 1, super::pickups::MAX_STACK_FALLBACK));
    let cue = ev.cue(WorldEventKind::LootTaken, p, format!("distortion item {item} x{stored} picked up"));
    send(vec![cue], &mut out, &mut toast);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::wild::{load_spawners_for_area, roll_spawner};
    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    fn slot(weight: u16, alpha: bool, time: [f32; 4], weather: [f32; 8]) -> EventSlot {
        EventSlot { species_id: 25, species: "pikachu".into(), form: 0, alpha, weight, min_level: 5, max_level: 7, time, weather, condition: String::new() }
    }

    #[test]
    fn sheet_and_story_gates() {
        let s = Sheet::parse("# c\na:string*\tb:u8\nx\t1\ny\n");
        assert_eq!((s.len(), s.get(0, "a"), s.get(0, "b"), s.get(1, "b"), s.get(5, "a")), (2, "x", "1", "", ""));
        let le = StoryGate::new("less_than_equal", "39999", "");
        assert!(le.matches(39_999) && !le.matches(40_000));
        let bt = StoryGate::new("between", "40000", "49999");
        assert!(bt.matches(40_000) && bt.matches(49_999) && !bt.matches(50_000) && !bt.matches(39_999));
        let ge = StoryGate::new("greater_than_equal", "100000", "");
        assert!(ge.matches(100_000) && !ge.matches(99_999));
        assert!(StoryGate::new("", "", "").matches(0), "no gate");
        assert!(StoryGate::new("3ddd5feb4a68a149", "5", "").matches(0), "unresolved comparator = always");
    }

    #[test]
    fn weights_follow_time_and_weather_and_skip_alphas() {
        let mut night_only = slot(100, false, [0.0, 0.0, 0.0, 1.0], [1.0; 8]);
        night_only.weather[2] = 0.0; // never in rain (id 4)
        night_only.weather[4] = 2.0; // x2 in drought (id 6)
        assert_eq!(effective_weight(&night_only, 1, 2), 0.0);
        assert_eq!(effective_weight(&night_only, 3, 2), 100.0);
        assert_eq!(effective_weight(&night_only, 3, 4), 0.0);
        assert_eq!(effective_weight(&night_only, 3, 6), 200.0);
        assert_eq!(effective_weight(&night_only, 3, 42), 100.0, "unknown weather id = neutral");
        let alpha = slot(1, true, [1.0; 4], [1.0; 8]);
        assert!(entries_for(&[night_only.clone(), alpha.clone()], 1, 2).is_empty());
        let e = entries_for(&[night_only, alpha], 3, 6);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].weight, 200);
    }

    #[test]
    fn flags_parse() {
        let a = |s: &str| s.split_whitespace().map(str::to_string).collect::<Vec<_>>();
        let c = WorldEventsConfig::from_args(&a("viewer --story 50000 --event-seed 7 --event-time-scale 20 --distortion-now --outbreak 12"));
        assert_eq!((c.story, c.seed, c.time_scale, c.distortion_now, c.force_outbreak), (50_000, 7, 20.0, true, Some(Some(12))));
        assert_eq!(WorldEventsConfig::from_args(&a("v --outbreak off")).force_outbreak, Some(None));
        assert!(!WorldEventsConfig::from_args(&a("v --no-world-events")).enabled);
        assert_eq!(WorldEventsConfig::from_args(&a("v --event-time-scale -3")).time_scale, 1.0);
        assert_eq!(WorldEventsConfig::from_args(&a("v")), WorldEventsConfig::default());
    }

    #[test]
    fn day_roll_detection() {
        assert!(!day_rolled(None, 10));
        assert!(!day_rolled(Some(600), 601));
        assert!(day_rolled(Some(1439), 0));
        assert!(day_rolled(Some(1400), 30));
        assert!(!day_rolled(Some(700), 690), "a small step back (rest/load) is not a new day");
    }

    fn real_events(config: WorldEventsConfig) -> Option<WorldEvents> {
        let ev = load_world_events(config);
        if ev.is_none() {
            eprintln!("[skip] world event sheets not found");
        }
        ev
    }

    fn area01_spawners() -> Option<ActiveAreaSpawners> {
        let path = find_data_file("field_spawn_table.tsv")?;
        let spawners = load_spawners_for_area(path, 1).ok()?;
        Some(ActiveAreaSpawners { spawners, ..Default::default() })
    }

    #[test]
    fn an_outbreak_replaces_the_area_spawn_table_correctly() {
        let (Some(mut ev), Some(mut active)) = (real_events(WorldEventsConfig { force_outbreak: Some(Some(1)), ..Default::default() }), area01_spawners()) else {
            return;
        };
        let before: BTreeSet<String> = active.spawners.iter().map(|s| s.spawner_id.clone()).collect();
        let outbreak_ids = ev.outbreaks.outbreak_spawner_ids("ha_area01");
        assert!(outbreak_ids.is_subset(&before), "the regular table lists all 46 outbreak spawners");
        let disabled = ev.outbreaks.disabled[&("ha_area01".to_string(), 1)].clone();
        assert_eq!(disabled.len(), 4);
        ev.enter_area("ha_area01");
        let o = ev.outbreak.clone().expect("outbreak 1 forced");
        assert_eq!((o.id, o.species.as_str()), (1, "pikachu"));
        let gone = ev.reconcile(&mut active, 1, 2, |_| false);
        assert!(gone.is_empty());
        let after: BTreeSet<String> = active.spawners.iter().map(|s| s.spawner_id.clone()).collect();
        assert!(after.is_disjoint(&outbreak_ids), "no outbreak spawner left in its static form");
        assert!(after.is_disjoint(&disabled), "the 4 `not_equal` spawners are off during outbreak 1");
        let events: Vec<&SpawnerDefinition> = active.spawners.iter().filter(|s| s.spawner_id.starts_with(EVENT_ID_PREFIX)).collect();
        assert_eq!(events.len(), 4, "4 alive outbreak Pokémon");
        for s in &events {
            let (dex, name, lvl) = roll_spawner(s, active.seed).expect("species");
            assert_eq!((dex, name.as_str()), (25, "pikachu"));
            assert!((11..=13).contains(&lvl));
        }
        // Every other regular spawner is untouched.
        let regular_after = after.iter().filter(|id| !id.starts_with(EVENT_ID_PREFIX)).count();
        assert_eq!(regular_after, before.len() - outbreak_ids.len() - disabled.len());
        // Outbreak off -> the disabled spawners come back, the outbreak ones stay off.
        ev.outbreak = None;
        ev.dirty = true;
        ev.reconcile(&mut active, 1, 2, |_| false);
        let off: BTreeSet<String> = active.spawners.iter().map(|s| s.spawner_id.clone()).collect();
        assert!(!off.iter().any(|id| id.starts_with(EVENT_ID_PREFIX)));
        assert!(off.is_disjoint(&outbreak_ids));
        assert!(disabled.is_subset(&off), "the 4 switched-off spawners are back after the outbreak");
        assert_eq!(off.len(), before.len() - outbreak_ids.len(), "only the inactive outbreak spawners stay out");
    }

    #[test]
    fn outbreak_counts_cleared_pokemon_and_ends_at_its_total() {
        let Some(mut ev) = real_events(WorldEventsConfig { force_outbreak: Some(Some(1)), ..Default::default() }) else { return };
        ev.enter_area("ha_area01");
        let total = ev.outbreak.as_ref().unwrap().total;
        assert!((10..=15).contains(&total));
        let mut ends = 0;
        for _ in 0..total {
            let id = ev.desired_spawners(1, 2)[0].spawner_id.clone();
            let cues = ev.observe_consumed(|s| s == id);
            ends += cues.iter().filter(|c| c.kind == WorldEventKind::OutbreakEnd).count();
            let live = ev.desired_spawners(1, 2).len();
            if let Some(o) = &ev.outbreak {
                assert_eq!(live, usize::from(o.total - o.cleared).min(4), "never more alive than what is left");
                assert!(!ev.desired_spawners(1, 2).iter().any(|d| d.spawner_id == id), "a cleared slot gets a new id");
            }
        }
        assert_eq!(ends, 1);
        assert!(ev.outbreak.is_none());
        assert!(ev.desired_spawners(1, 2).is_empty());
    }

    /// Headless Bevy run: real sheets, area 01, accelerated timers. Checks that a distortion forms,
    /// materialises with Pokémon that the regular spawner list now holds, and ends.
    #[test]
    fn headless_distortion_cycle_and_outbreak_in_area01() {
        let (Some(_), Some(active)) = (real_events(WorldEventsConfig::default()), area01_spawners()) else { return };
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(250)));
        app.insert_resource(active);
        app.insert_resource(ConsumedSpawners::default());
        app.insert_resource(ActiveZoneInfo { area: "ha_area01".into(), name: "x".into() });
        app.insert_resource(GameClock::pinned_at_slot(1));
        app.world_mut().spawn((PlayerAvatar, Transform::from_xyz(523.3, 33.6, 169.4)));
        app.add_plugins(WorldEventsPlugin {
            config: WorldEventsConfig { distortion_now: true, time_scale: 40.0, force_outbreak: Some(Some(2)), ..Default::default() },
        });
        let mut saw_active_spawns = 0usize;
        let mut kinds = Vec::new();
        for _ in 0..80 {
            app.update();
            let ev = app.world().resource::<WorldEvents>();
            if ev.machine.as_ref().is_some_and(|m| m.phase == Phase::Active) {
                let n = app.world().resource::<ActiveAreaSpawners>().spawners.iter().filter(|s| s.spawner_id.starts_with("we:wh:")).count();
                saw_active_spawns = saw_active_spawns.max(n);
            }
            let msgs = app.world().resource::<Messages<WorldEventCue>>();
            let mut cursor = msgs.get_cursor();
            for m in cursor.read(msgs) {
                if !kinds.contains(&m.kind) {
                    kinds.push(m.kind);
                }
            }
        }
        // 80 frames x 0.25 s x 40 = 800 event seconds: notice (<= 60 + d/2) + active 240 + end; the next notice
        // needs >= 300 s more (timer A), so the cycle is over and no new one has started.
        for k in [
            WorldEventKind::OutbreakStart,
            WorldEventKind::DistortionNotice,
            WorldEventKind::DistortionMaterialize,
            WorldEventKind::DistortionVanishWarning,
            WorldEventKind::DistortionEnd,
        ] {
            assert!(kinds.contains(&k), "missing {k:?} in {kinds:?}");
        }
        assert!((5..=6).contains(&saw_active_spawns), "2..3 main + 3 sub slots, got {saw_active_spawns}");
        let active = app.world().resource::<ActiveAreaSpawners>();
        assert!(!active.spawners.iter().any(|s| s.spawner_id.starts_with("we:wh:")), "distortion contents removed after End");
        let ob: Vec<_> = active.spawners.iter().filter(|s| s.spawner_id.starts_with("we:ob:")).collect();
        assert_eq!(ob.len(), 4, "forced outbreak 2 stays");
        assert!(ob.iter().all(|s| s.entries.iter().all(|e| e.species_name == "pichu")));
        let mut q = app.world_mut().query::<&WorldEventMarker>();
        let markers = q.iter(app.world()).copied().collect::<Vec<_>>();
        assert!(markers.contains(&WorldEventMarker::OutbreakPillar));
        assert!(!markers.contains(&WorldEventMarker::Dome), "dome removed after the end");
    }

    #[test]
    fn loot_is_taken_once_and_only_in_range() {
        let mut ev = WorldEvents::new(WorldEventsConfig::default(), None, DistortionData::default(), OutbreakData::default());
        ev.loot = vec![LootDrop { pos: Vec3::new(10.0, 0.0, 0.0), item: 1611, taken: false }];
        assert_eq!(ev.take_loot(Vec3::ZERO), None);
        assert_eq!(ev.take_loot(Vec3::new(8.0, 5.0, 0.0)), Some((0, 1611)));
        assert_eq!(ev.take_loot(Vec3::new(10.0, 0.0, 0.0)), None, "taken");
    }
}
