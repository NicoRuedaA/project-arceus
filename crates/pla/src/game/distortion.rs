//! Space-time distortions (Brief 39): decoded data and a pure state machine.
//!
//! Data (all decoded by `.tools/events_decode.py`, see `reports/data-progress/world_events.md`):
//! - `data/world/wormhole_config_values.tsv`: the 37 values of `field_wormhole_config.bin`
//!   (timer A lottery, notice/active durations, countdowns, respawn window);
//! - `data/world_events_distortion_sites.tsv`: 25 dome sites x story variants (116 rows);
//! - `data/world_events_distortion_spawners.tsv`: one main + three sub spawners per variant;
//! - `data/world_events_distortion_encounters.tsv`: the species tables those spawners use;
//! - `data/world_events_distortion_items.tsv` / `..._points.tsv`: item lottery and dome points.
//!
//! Machine: `Idle -> Notice -> Active -> End -> Idle`.
//! - Idle lasts timer A: roll 0..99, pick one of five minute values by weight (the "before" table
//!   until the event flag `kWhEventFlag` is modelled). Read from code (`FUN_024a2030`), LIKELY;
//!   not checked against the running game.
//! - Notice lasts `kWhTimerB_AddValue + distance / kWhTimerB_DivValue` seconds (60 s + half the
//!   distance in metres; which value adds and which divides is inferred from the key names).
//! - Active lasts `kWhTimerC` minutes (4). Vanish warnings at 15 s and 7 s before the end
//!   (`kWhVanishCountMiddle/Final`); the appear countdown is the last 7 s of Notice
//!   (`kWhAppearBeforeCount`). Cue semantics are ASSUMED from the key names.
//! - End is a short clean-up phase ([`END_SECONDS`], ASSUMED) before the next timer A.
//!
//! Site choice: uniform among the area's sites whose story gate matches (ASSUMED: the game
//! weights zones, the source of those weights is UNKNOWN). The RNG is SplitMix64 seeded from the
//! session seed; the game's Xoroshiro128+ is not ported.

use std::collections::HashMap;

use bevy::math::Vec3;

use super::wild::{splitmix64, SpawnSpeciesEntry};
use super::world_events::{entries_for, parse_slot, rng_range, EventSlot, Sheet, StoryGate};

/// Seconds of the End phase before the next Idle timer (ASSUMED: no duration is decoded).
pub const END_SECONDS: f32 = 3.0;
/// Item drops placed when a distortion materialises (ASSUMED: the per-event count is UNKNOWN).
pub const ITEM_DROPS_PER_DISTORTION: usize = 6;

/// The decoded wormhole configuration (`field_wormhole_config.bin`).
#[derive(Debug, Clone, PartialEq)]
pub struct WormholeConfig {
    /// Timer A before the event flag: `(minutes, weight)` x5, weights sum to 100.
    pub before: [(u32, u32); 5],
    /// Timer A after the event flag.
    pub after: [(u32, u32); 5],
    /// Flag name that switches the timer table (`FSYS_FORCE_WEATHER_ENABLE`).
    pub event_flag: String,
    pub notice_add_s: f32,
    pub notice_div: f32,
    pub active_minutes: f32,
    pub appear_count_s: f32,
    pub vanish_middle_s: f32,
    pub vanish_final_s: f32,
    /// `kWhSpawnTimeMin/Max` (15/23): ASSUMED seconds until a cleared slot spawns again.
    pub spawn_time_min_s: f32,
    pub spawn_time_max_s: f32,
}

impl WormholeConfig {
    /// Parse `data/world/wormhole_config_values.tsv`; `None` when any required key is missing.
    pub fn parse(text: &str) -> Option<Self> {
        let sheet = Sheet::parse(text);
        let mut v: HashMap<&str, &str> = HashMap::new();
        for r in 0..sheet.len() {
            v.insert(sheet.get(r, "config_key"), sheet.get(r, "value_text"));
        }
        let num = |k: &str| v.get(k).and_then(|s| s.parse::<u32>().ok());
        let table = |kind: &str| -> Option<[(u32, u32); 5]> {
            let mut t = [(0, 0); 5];
            for (i, slot) in t.iter_mut().enumerate() {
                *slot = (
                    num(&format!("kWhTimerA_Event{kind}Minutes{:02}", i + 1))?,
                    num(&format!("kWhTimerA_Event{kind}Lottery{:02}", i + 1))?,
                );
            }
            Some(t)
        };
        Some(Self {
            before: table("Before")?,
            after: table("After")?,
            event_flag: v.get("kWhEventFlag")?.to_string(),
            notice_add_s: num("kWhTimerB_AddValue")? as f32,
            notice_div: num("kWhTimerB_DivValue")? as f32,
            active_minutes: num("kWhTimerC")? as f32,
            appear_count_s: num("kWhAppearBeforeCount")? as f32,
            vanish_middle_s: num("kWhVanishCountMiddle")? as f32,
            vanish_final_s: num("kWhVanishCountFinal")? as f32,
            spawn_time_min_s: num("kWhSpawnTimeMin")? as f32,
            spawn_time_max_s: num("kWhSpawnTimeMax")? as f32,
        })
    }

    /// Timer A in seconds: roll 0..99 against the cumulative weights of the chosen table.
    pub fn roll_timer_a_s(&self, after_flag: bool, rng: &mut u64) -> f32 {
        let table = if after_flag { &self.after } else { &self.before };
        let total: u64 = table.iter().map(|(_, w)| u64::from(*w)).sum::<u64>().max(1);
        let mut roll = splitmix64(rng) % total;
        for &(minutes, w) in table {
            if roll < u64::from(w) {
                return minutes as f32 * 60.0;
            }
            roll -= u64::from(w);
        }
        table[4].0 as f32 * 60.0
    }

    /// Notice duration for a site `distance_m` metres from the player (30 frames = 1 s when the
    /// divisor is below 1, as in the code reading).
    pub fn notice_s(&self, distance_m: f32) -> f32 {
        if self.notice_div < 1.0 || !distance_m.is_finite() {
            return 1.0;
        }
        self.notice_add_s + distance_m.max(0.0) / self.notice_div
    }

    pub fn active_s(&self) -> f32 {
        self.active_minutes * 60.0
    }
}

/// One distortion site variant (`world_events_distortion_sites.tsv`).
#[derive(Debug, Clone, PartialEq)]
pub struct DistortionSite {
    pub area: String,
    pub name: String,
    pub site: String,
    pub variant: String,
    pub centre: Vec3,
    pub gate: StoryGate,
    pub item_table_key: String,
    pub spawners: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnerRole {
    Main,
    Sub,
}

/// One `wh_spawner` (`world_events_distortion_spawners.tsv`).
#[derive(Debug, Clone, PartialEq)]
pub struct DistortionSpawner {
    pub name: String,
    pub role: SpawnerRole,
    pub pos: Vec3,
    pub radius: f32,
    pub min_count: u16,
    pub max_count: u16,
    pub encount_key: String,
    pub gate: StoryGate,
}

/// All decoded distortion sheets, indexed for lookups.
#[derive(Debug, Clone, Default)]
pub struct DistortionData {
    pub sites: Vec<DistortionSite>,
    /// `(area, spawner name)`.
    pub spawners: HashMap<(String, String), DistortionSpawner>,
    /// `(area, encount_key)`: the key resolves per area.
    pub encounters: HashMap<(String, String), Vec<EventSlot>>,
    /// `(area, item_table_key)` -> `(item id, weight)`.
    pub items: HashMap<(String, String), Vec<(u16, u16)>>,
    /// `(area, site)` -> candidate points inside the dome.
    pub points: HashMap<(String, String), Vec<Vec3>>,
}

fn vec3(sheet: &Sheet, r: usize) -> Option<Vec3> {
    let f = |n: &str| sheet.get(r, n).parse::<f32>().ok().filter(|v| v.is_finite());
    Some(Vec3::new(f("x")?, f("y")?, f("z")?))
}

impl DistortionData {
    pub fn parse(sites: &str, spawners: &str, encounters: &str, items: &str, points: &str) -> Self {
        let mut d = Self::default();
        let s = Sheet::parse(sites);
        for r in 0..s.len() {
            let Some(centre) = vec3(&s, r) else { continue };
            d.sites.push(DistortionSite {
                area: s.get(r, "area").to_string(),
                name: s.get(r, "name").to_string(),
                site: s.get(r, "site").to_string(),
                variant: s.get(r, "variant").to_string(),
                centre,
                gate: StoryGate::new(s.get(r, "story_cmp"), s.get(r, "story_min"), s.get(r, "story_max")),
                item_table_key: s.get(r, "item_table_key").to_string(),
                spawners: s.get(r, "spawners").split(';').filter(|n| !n.is_empty()).map(str::to_string).collect(),
            });
        }
        let s = Sheet::parse(spawners);
        for r in 0..s.len() {
            let Some(pos) = vec3(&s, r) else { continue };
            let role = match s.get(r, "role") {
                "main" => SpawnerRole::Main,
                "sub" => SpawnerRole::Sub,
                _ => continue,
            };
            let min_count = s.get(r, "min_count").parse().unwrap_or(1);
            d.spawners.insert(
                (s.get(r, "area").to_string(), s.get(r, "spawner_name").to_string()),
                DistortionSpawner {
                    name: s.get(r, "spawner_name").to_string(),
                    role,
                    pos,
                    radius: s.get(r, "radius").parse().unwrap_or(6.0),
                    min_count,
                    max_count: s.get(r, "max_count").parse().unwrap_or(min_count).max(min_count),
                    encount_key: s.get(r, "encount_key").to_string(),
                    gate: StoryGate::new(s.get(r, "gate_cmp"), s.get(r, "gate_min"), s.get(r, "gate_max")),
                },
            );
        }
        let s = Sheet::parse(encounters);
        for r in 0..s.len() {
            if let Some(slot) = parse_slot(&s, r) {
                d.encounters
                    .entry((s.get(r, "area").to_string(), s.get(r, "encount_key").to_string()))
                    .or_default()
                    .push(slot);
            }
        }
        let s = Sheet::parse(items);
        for r in 0..s.len() {
            let (Ok(id), Ok(w)) = (s.get(r, "item_id").parse::<u16>(), s.get(r, "weight").parse::<u16>()) else {
                continue;
            };
            d.items
                .entry((s.get(r, "area").to_string(), s.get(r, "item_table_key").to_string()))
                .or_default()
                .push((id, w));
        }
        let s = Sheet::parse(points);
        for r in 0..s.len() {
            if let Some(p) = vec3(&s, r) {
                d.points.entry((s.get(r, "area").to_string(), s.get(r, "site").to_string())).or_default().push(p);
            }
        }
        d
    }

    /// Indices into [`Self::sites`] of the variants active in `area` at `story`: at most one per
    /// site (the first matching variant), in sheet order.
    pub fn candidates(&self, area: &str, story: u32) -> Vec<usize> {
        let mut seen: Vec<&str> = Vec::new();
        let mut out = Vec::new();
        for (i, s) in self.sites.iter().enumerate() {
            if s.area == area && s.gate.matches(story) && !seen.contains(&s.site.as_str()) {
                seen.push(&s.site);
                out.push(i);
            }
        }
        out
    }

    /// Spawn slots of an active site: one per alive Pokémon (main: `min..=max` alive, sub: 1).
    /// Main slots use the site's dome points as positions (ASSUMED; the points' meaning is LIKELY
    /// "positions inside the dome"), sub slots their own small sphere centre.
    pub fn spawn_slots(&self, site_idx: usize, story: u32, seed: u64) -> Vec<EventSpawnSlot> {
        let Some(site) = self.sites.get(site_idx) else { return Vec::new() };
        let mut rng = seed ^ super::wild::hash_str(&site.name);
        let points = self.points.get(&(site.area.clone(), site.site.clone()));
        let mut out = Vec::new();
        for name in &site.spawners {
            let Some(sp) = self.spawners.get(&(site.area.clone(), name.clone())) else { continue };
            if !sp.gate.matches(story) {
                continue;
            }
            let Some(slots) = self.encounters.get(&(site.area.clone(), sp.encount_key.clone())) else { continue };
            let count = rng_range(&mut rng, u32::from(sp.min_count.max(1)), u32::from(sp.max_count.max(1)));
            for k in 0..count {
                let pos = match (sp.role, points) {
                    (SpawnerRole::Main, Some(pts)) if !pts.is_empty() => pts[(splitmix64(&mut rng) % pts.len() as u64) as usize],
                    _ => sp.pos,
                };
                out.push(EventSpawnSlot {
                    key: format!("wh:{}:{}:{k}", site.area, name),
                    pos,
                    radius: sp.radius.min(8.0),
                    slots: slots.clone(),
                    role: sp.role,
                });
            }
        }
        out
    }

    /// `count` item drops at distinct dome points, items drawn by weight from the site's table.
    pub fn loot_plan(&self, site_idx: usize, count: usize, seed: u64) -> Vec<(Vec3, u16)> {
        let Some(site) = self.sites.get(site_idx) else { return Vec::new() };
        let Some(table) = self.items.get(&(site.area.clone(), site.item_table_key.clone())) else { return Vec::new() };
        let Some(points) = self.points.get(&(site.area.clone(), site.site.clone())) else { return Vec::new() };
        let mut rng = seed ^ super::wild::hash_str(&site.item_table_key) ^ 0x10_07;
        let mut idx: Vec<usize> = (0..points.len()).collect();
        let mut out = Vec::new();
        for i in 0..count.min(points.len()) {
            let j = i + (splitmix64(&mut rng) % (idx.len() - i) as u64) as usize;
            idx.swap(i, j);
            if let Some(item) = super::pickups::roll_loot(table, &mut rng) {
                out.push((points[idx[i]], item));
            }
        }
        out
    }
}

/// One alive-Pokémon slot of an event (distortion or outbreak). The Bevy side turns it into a
/// `SpawnerDefinition` with id `<key>#<generation>` so that a refilled slot is a new spawner.
#[derive(Debug, Clone, PartialEq)]
pub struct EventSpawnSlot {
    pub key: String,
    pub pos: Vec3,
    pub radius: f32,
    pub slots: Vec<EventSlot>,
    pub role: SpawnerRole,
}

impl EventSpawnSlot {
    /// Species entries allowed now (time slot / weather multipliers applied, alpha slots skipped).
    pub fn entries(&self, time_slot: usize, weather_id: u8) -> Vec<SpawnSpeciesEntry> {
        entries_for(&self.slots, time_slot, weather_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Notice,
    Active,
    End,
}

/// What a tick did, for logs, markers and (future) audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistortionCue {
    /// Timer A elapsed but the area has no site for the story value: Idle again.
    NoSite,
    /// `SET_STATE_WORMHOLE_NOTICE_START`: the dome starts forming at `site`.
    NoticeStart { site: usize },
    /// Last `kWhAppearBeforeCount` seconds of Notice.
    AppearCountdown { site: usize },
    /// `SET_STATE_WORMHOLE_MATERIALIZATION_START`: contents appear.
    Materialize { site: usize },
    /// `kWhVanishCountMiddle` (final = false) / `kWhVanishCountFinal` (final = true) before the end.
    VanishWarning { site: usize, last: bool },
    /// `..._MATERIALIZATION_END`: contents removed.
    End { site: usize },
}

/// The distortion state of one area.
#[derive(Debug, Clone, PartialEq)]
pub struct DistortionMachine {
    pub phase: Phase,
    pub remaining_s: f32,
    pub site: Option<usize>,
    /// Increments on every Notice start; part of every spawned id.
    pub serial: u32,
    warned_appear: bool,
    warned_middle: bool,
    warned_final: bool,
}

impl DistortionMachine {
    pub fn new(cfg: &WormholeConfig, rng: &mut u64) -> Self {
        Self {
            phase: Phase::Idle,
            remaining_s: cfg.roll_timer_a_s(false, rng),
            site: None,
            serial: 0,
            warned_appear: false,
            warned_middle: false,
            warned_final: false,
        }
    }

    /// Advance by `dt` seconds. `candidates` are site indices allowed now and `distance` gives
    /// the player's distance to a site. Large `dt` may cross several phases; every crossing
    /// emits its cue in order. Non-finite or negative `dt` does nothing.
    pub fn tick(
        &mut self,
        dt: f32,
        cfg: &WormholeConfig,
        candidates: &[usize],
        distance: impl Fn(usize) -> f32,
        rng: &mut u64,
    ) -> Vec<DistortionCue> {
        let mut cues = Vec::new();
        if !dt.is_finite() || dt <= 0.0 {
            return cues;
        }
        let mut left = dt;
        for _ in 0..32 {
            // Stop at the next warning threshold so a long step still emits it.
            let stop = self.next_threshold(cfg).unwrap_or(0.0);
            let step = left.min((self.remaining_s - stop).max(0.0));
            self.remaining_s -= step;
            left -= step;
            self.threshold_cues(cfg, &mut cues);
            if self.remaining_s > 0.0 {
                if left <= 0.0 {
                    break;
                }
                continue;
            }
            self.advance_phase(cfg, candidates, &distance, rng, &mut cues);
            if left <= 0.0 {
                break;
            }
        }
        cues
    }

    /// Remaining-time value of the next warning not yet emitted in this phase.
    fn next_threshold(&self, cfg: &WormholeConfig) -> Option<f32> {
        let pending = |done: bool, t: f32| (!done && t > 0.0 && t < self.remaining_s).then_some(t);
        match self.phase {
            Phase::Notice => pending(self.warned_appear, cfg.appear_count_s),
            Phase::Active => pending(self.warned_middle, cfg.vanish_middle_s).or(pending(self.warned_final, cfg.vanish_final_s)),
            _ => None,
        }
    }

    fn threshold_cues(&mut self, cfg: &WormholeConfig, cues: &mut Vec<DistortionCue>) {
        let Some(site) = self.site else { return };
        match self.phase {
            Phase::Notice if !self.warned_appear && self.remaining_s <= cfg.appear_count_s && self.remaining_s > 0.0 => {
                self.warned_appear = true;
                cues.push(DistortionCue::AppearCountdown { site });
            }
            Phase::Active => {
                if !self.warned_middle && self.remaining_s <= cfg.vanish_middle_s && self.remaining_s > 0.0 {
                    self.warned_middle = true;
                    cues.push(DistortionCue::VanishWarning { site, last: false });
                }
                if !self.warned_final && self.remaining_s <= cfg.vanish_final_s && self.remaining_s > 0.0 {
                    self.warned_final = true;
                    cues.push(DistortionCue::VanishWarning { site, last: true });
                }
            }
            _ => {}
        }
    }

    fn advance_phase(
        &mut self,
        cfg: &WormholeConfig,
        candidates: &[usize],
        distance: &impl Fn(usize) -> f32,
        rng: &mut u64,
        cues: &mut Vec<DistortionCue>,
    ) {
        match self.phase {
            Phase::Idle => {
                if candidates.is_empty() {
                    self.remaining_s = cfg.roll_timer_a_s(false, rng);
                    cues.push(DistortionCue::NoSite);
                    return;
                }
                let site = candidates[(splitmix64(rng) % candidates.len() as u64) as usize];
                self.site = Some(site);
                self.serial = self.serial.wrapping_add(1);
                self.phase = Phase::Notice;
                self.remaining_s = cfg.notice_s(distance(site));
                self.warned_appear = false;
                self.warned_middle = false;
                self.warned_final = false;
                cues.push(DistortionCue::NoticeStart { site });
            }
            Phase::Notice => {
                self.phase = Phase::Active;
                self.remaining_s = cfg.active_s();
                cues.push(DistortionCue::Materialize { site: self.site.unwrap_or(0) });
            }
            Phase::Active => {
                self.phase = Phase::End;
                self.remaining_s = END_SECONDS;
                cues.push(DistortionCue::End { site: self.site.unwrap_or(0) });
            }
            Phase::End => {
                self.phase = Phase::Idle;
                self.site = None;
                self.remaining_s = cfg.roll_timer_a_s(false, rng);
            }
        }
    }

    /// Skip the rest of the Idle timer (debug flag `--distortion-now`).
    pub fn force_soon(&mut self) {
        if self.phase == Phase::Idle {
            self.remaining_s = self.remaining_s.min(0.01);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::world_objects::find_data_file;

    fn read(name: &str) -> Option<String> {
        find_data_file(name).and_then(|p| std::fs::read_to_string(p).ok())
    }

    pub(crate) fn real_config() -> Option<WormholeConfig> {
        WormholeConfig::parse(&read("world/wormhole_config_values.tsv")?)
    }

    pub(crate) fn real_data() -> Option<DistortionData> {
        Some(DistortionData::parse(
            &read("world_events_distortion_sites.tsv")?,
            &read("world_events_distortion_spawners.tsv")?,
            &read("world_events_distortion_encounters.tsv")?,
            &read("world_events_distortion_items.tsv")?,
            &read("world_events_distortion_points.tsv")?,
        ))
    }

    fn cfg() -> WormholeConfig {
        WormholeConfig {
            before: [(5, 10), (10, 20), (15, 20), (25, 25), (40, 25)],
            after: [(5, 20), (10, 10), (15, 20), (25, 25), (40, 25)],
            event_flag: "FSYS_FORCE_WEATHER_ENABLE".into(),
            notice_add_s: 60.0,
            notice_div: 2.0,
            active_minutes: 4.0,
            appear_count_s: 7.0,
            vanish_middle_s: 15.0,
            vanish_final_s: 7.0,
            spawn_time_min_s: 15.0,
            spawn_time_max_s: 23.0,
        }
    }

    #[test]
    fn config_matches_the_decoded_sheet() {
        let Some(c) = real_config() else {
            eprintln!("[skip] data/world/wormhole_config_values.tsv not found");
            return;
        };
        assert_eq!(c, cfg(), "every value read from the 37-row sheet");
        assert_eq!(c.before.iter().map(|t| t.1).sum::<u32>(), 100);
        assert_eq!(c.after.iter().map(|t| t.1).sum::<u32>(), 100);
    }

    #[test]
    fn timer_a_only_draws_table_minutes_with_the_table_frequencies() {
        let c = cfg();
        let mut rng = 7u64;
        let mut counts = HashMap::new();
        for _ in 0..20_000 {
            *counts.entry(c.roll_timer_a_s(false, &mut rng) as u32).or_insert(0u32) += 1;
        }
        let mut keys: Vec<u32> = counts.keys().copied().collect();
        keys.sort();
        assert_eq!(keys, vec![300, 600, 900, 1500, 2400]);
        // 10/20/20/25/25 % within 1.5 points.
        for (secs, pct) in [(300, 10.0), (600, 20.0), (900, 20.0), (1500, 25.0), (2400, 25.0)] {
            let got = counts[&secs] as f32 / 200.0;
            assert!((got - pct).abs() < 1.5, "{secs}s: {got}% vs {pct}%");
        }
    }

    #[test]
    fn notice_and_active_durations() {
        let c = cfg();
        assert_eq!(c.notice_s(0.0), 60.0);
        assert_eq!(c.notice_s(100.0), 110.0);
        assert_eq!(c.notice_s(f32::NAN), 1.0);
        assert_eq!(c.active_s(), 240.0);
        let bad = WormholeConfig { notice_div: 0.5, ..c };
        assert_eq!(bad.notice_s(100.0), 1.0, "divisor < 1 -> 30 frames");
    }

    #[test]
    fn machine_runs_idle_notice_active_end_with_the_decoded_durations() {
        let c = cfg();
        let mut rng = 42u64;
        let mut m = DistortionMachine::new(&c, &mut rng);
        assert_eq!(m.phase, Phase::Idle);
        let idle = m.remaining_s;
        assert!([300.0, 600.0, 900.0, 1500.0, 2400.0].contains(&idle));
        let dist = |_s: usize| 40.0;
        // Just before the timer: still idle, no cue.
        assert!(m.tick(idle - 0.5, &c, &[3, 7], dist, &mut rng).is_empty());
        let cues = m.tick(0.5, &c, &[3, 7], dist, &mut rng);
        let site = m.site.expect("a site was picked");
        assert!([3, 7].contains(&site));
        assert_eq!(cues, vec![DistortionCue::NoticeStart { site }]);
        assert_eq!((m.phase, m.remaining_s), (Phase::Notice, 80.0), "60 s + 40 m / 2");
        let cues = m.tick(73.0, &c, &[3, 7], dist, &mut rng);
        assert_eq!(cues, vec![DistortionCue::AppearCountdown { site }]);
        let cues = m.tick(7.0, &c, &[3, 7], dist, &mut rng);
        assert_eq!(cues, vec![DistortionCue::Materialize { site }]);
        assert_eq!((m.phase, m.remaining_s), (Phase::Active, 240.0));
        let cues = m.tick(240.0 - 15.0, &c, &[3, 7], dist, &mut rng);
        assert_eq!(cues, vec![DistortionCue::VanishWarning { site, last: false }]);
        let cues = m.tick(8.0, &c, &[3, 7], dist, &mut rng);
        assert_eq!(cues, vec![DistortionCue::VanishWarning { site, last: true }]);
        let cues = m.tick(7.0, &c, &[3, 7], dist, &mut rng);
        assert_eq!(cues, vec![DistortionCue::End { site }]);
        assert_eq!((m.phase, m.remaining_s), (Phase::End, END_SECONDS));
        m.tick(END_SECONDS, &c, &[3, 7], dist, &mut rng);
        assert_eq!(m.phase, Phase::Idle);
        assert!(m.site.is_none());
        assert!(m.remaining_s >= 300.0);
    }

    #[test]
    fn one_large_step_emits_every_crossing_in_order_and_bad_steps_do_nothing() {
        let c = cfg();
        let mut rng = 1u64;
        let mut m = DistortionMachine::new(&c, &mut rng);
        let before = m.clone();
        assert!(m.tick(f32::NAN, &c, &[0], |_| 0.0, &mut rng).is_empty());
        assert!(m.tick(-5.0, &c, &[0], |_| 0.0, &mut rng).is_empty());
        assert_eq!(m, before);
        let total = m.remaining_s + 60.0 + 240.0 + 1.0;
        let cues = m.tick(total, &c, &[0], |_| 0.0, &mut rng);
        let kinds: Vec<_> = cues.iter().map(std::mem::discriminant).collect();
        let expect = [
            DistortionCue::NoticeStart { site: 0 },
            DistortionCue::AppearCountdown { site: 0 },
            DistortionCue::Materialize { site: 0 },
            DistortionCue::VanishWarning { site: 0, last: false },
            DistortionCue::VanishWarning { site: 0, last: true },
            DistortionCue::End { site: 0 },
        ];
        assert_eq!(kinds, expect.iter().map(std::mem::discriminant).collect::<Vec<_>>());
        assert_eq!(m.phase, Phase::End);
    }

    #[test]
    fn no_candidate_keeps_the_machine_idle() {
        let c = cfg();
        let mut rng = 3u64;
        let mut m = DistortionMachine::new(&c, &mut rng);
        let cues = m.tick(m.remaining_s, &c, &[], |_| 0.0, &mut rng);
        assert_eq!(cues, vec![DistortionCue::NoSite]);
        assert_eq!(m.phase, Phase::Idle);
        assert!(m.remaining_s >= 300.0);
    }

    #[test]
    fn same_seed_same_sequence_of_sites() {
        let c = cfg();
        let run = |seed: u64| {
            let mut rng = seed;
            let mut m = DistortionMachine::new(&c, &mut rng);
            let mut sites = Vec::new();
            for _ in 0..200 {
                for cue in m.tick(120.0, &c, &[0, 1, 2, 3, 4], |_| 10.0, &mut rng) {
                    if let DistortionCue::NoticeStart { site } = cue {
                        sites.push(site);
                    }
                }
            }
            sites
        };
        let a = run(99);
        assert!(a.len() >= 5, "several distortions in 400 minutes, got {}", a.len());
        assert_eq!(a, run(99));
        assert_ne!(a, run(100), "another seed gives another sequence");
        assert!(a.iter().collect::<std::collections::BTreeSet<_>>().len() > 1, "more than one site used");
    }

    #[test]
    fn real_sites_spawners_and_tables_link_up() {
        let Some(d) = real_data() else {
            eprintln!("[skip] distortion sheets not found");
            return;
        };
        assert_eq!(d.sites.len(), 116);
        assert_eq!(d.spawners.len(), 464);
        // Every site lists 1 main + 3 sub spawners that resolve, and every key has a table.
        for s in &d.sites {
            assert_eq!(s.spawners.len(), 4, "{}", s.name);
            for n in &s.spawners {
                let sp = d.spawners.get(&(s.area.clone(), n.clone())).unwrap_or_else(|| panic!("{n}"));
                assert!(d.encounters.contains_key(&(s.area.clone(), sp.encount_key.clone())), "{n}");
            }
            assert!(d.items.contains_key(&(s.area.clone(), s.item_table_key.clone())), "{}", s.name);
        }
        // 4/5/5/5/6 sites in areas 01..05 after the story; one site of areas 02..04 opens later
        // (its first variant is gated `between`).
        for (area, n, early) in [
            ("ha_area01", 4, 4),
            ("ha_area02", 5, 4),
            ("ha_area03", 5, 4),
            ("ha_area04", 5, 4),
            ("ha_area05", 6, 6),
        ] {
            assert_eq!(d.candidates(area, 100_000).len(), n, "{area}");
            assert_eq!(d.candidates(area, 0).len(), early, "{area} at story 0");
        }
        assert!(d.candidates("ha_area00", 100_000).is_empty(), "no distortions in the village");
    }

    #[test]
    fn real_spawn_slots_use_the_area_table_and_counts() {
        let Some(d) = real_data() else {
            eprintln!("[skip] distortion sheets not found");
            return;
        };
        let site = d.candidates("ha_area04", 50_000)[0];
        let slots = d.spawn_slots(site, 50_000, 1);
        let main = slots.iter().filter(|s| s.role == SpawnerRole::Main).count();
        let sub = slots.iter().filter(|s| s.role == SpawnerRole::Sub).count();
        assert!((2..=3).contains(&main), "main 2..3 alive, got {main}");
        assert_eq!(sub, 3, "three sub spheres, one alive each");
        let names: std::collections::BTreeSet<String> = slots
            .iter()
            .filter(|s| s.role == SpawnerRole::Sub)
            .flat_map(|s| s.entries(1, 2))
            .map(|e| e.species_name)
            .collect();
        for want in ["cranidos", "shieldon"] {
            assert!(names.contains(want), "area 04 sub table has {want}: {names:?}");
        }
        assert!(slots.iter().all(|s| s.entries(1, 2).iter().all(|e| e.min_level >= 30)), "area 04 levels >= 30");
        assert_eq!(d.spawn_slots(site, 50_000, 1), slots, "deterministic for a seed");
        let loot = d.loot_plan(site, ITEM_DROPS_PER_DISTORTION, 1);
        assert_eq!(loot.len(), ITEM_DROPS_PER_DISTORTION);
        let pts = &d.points[&(d.sites[site].area.clone(), d.sites[site].site.clone())];
        assert!(loot.iter().all(|(p, _)| pts.contains(p)), "drops sit on decoded dome points");
        let distinct: std::collections::BTreeSet<_> = loot.iter().map(|(p, _)| (p.x.to_bits(), p.z.to_bits())).collect();
        assert_eq!(distinct.len(), loot.len(), "distinct points");
    }
}
