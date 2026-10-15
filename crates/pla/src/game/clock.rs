//! Game clock (time of day) and the smooth blend between the four decoded time slots.
//!
//! The sheet `data/field_lighting_env.tsv` stores four time slots per environment parameter;
//! `lighting.rs` reads them into [`ZoneEnv`]. This module owns *when* each slot applies:
//!
//! - A pure [`GameClock`] advancing at `--time-scale` game minutes per real second. The default
//!   speed is one game minute per real second (a game day in 24 real minutes) — PROBABLE: the
//!   game's own clock rate is not decoded, but the external hint states exactly "one real minute
//!   is one game hour", which is this rate. `--time-scale` overrides it.
//! - `--time-slot N` still works: it pins the clock to the centre of slot N (no advance), so every
//!   view stays reproducible.
//! - [`slot_blend`] turns a time in hours into the two surrounding slots and a 0..1 factor, with
//!   the two slots exactly 50/50 at a band boundary and exactly slot N at its centre.
//! - The **band boundaries** are 05:00, 12:00, 18:00, 21:00 ([`SLOT_START_HOURS`]): morning
//!   05-12, day 12-18, evening 18-21, night 21-05. PROBABLE: no unpacked file states the hours
//!   (`reports/data-progress/world_events.md`, "Time-of-day slot boundaries (hours) are not in
//!   these files"); the external hint fixes exactly these four (its rest menu jumps to the start
//!   of each band, 05:00/12:00/18:00/21:00). Slot 3 = night is VERIFIED by the spawn multipliers;
//!   the order of slots 0-2 (morning/day/evening) is ASSUMED (identical everywhere in the spawn
//!   data).
//! - [`blend_env`] interpolates two decoded [`ZoneEnv`]s component-wise (linear RGB), which is this
//!   port's own interpretation (ASSUMED); the game blends slots its own way (not decoded).
//!
//! The save stores whole game minutes ([`crate::game::save_game`] v6), so a loaded game resumes at
//! the exact time of day it was left at.

use bevy::prelude::*;

use super::lighting::ZoneEnv;
use super::play_time::counts_play_time;
use super::save_wire::PlayerProgress;

/// Number of decoded time slots (4).
pub const SLOT_COUNT: usize = 4;
/// First hour of each slot's band: morning 05:00, day 12:00, evening 18:00, night 21:00 (the last
/// band wraps past midnight to 05:00). PROBABLE (external hint; no hour data in the unpacked
/// game; see the module docs).
pub const SLOT_START_HOURS: [f32; SLOT_COUNT] = [5.0, 12.0, 18.0, 21.0];
/// Game minutes in a full day.
pub const MINUTES_PER_DAY: u32 = 1440;
/// Default clock speed: one game minute per real second, i.e. one game hour per real minute (a
/// full day in 24 real minutes). PROBABLE: not decoded from the game, and exactly the external
/// hint's rate. `--time-scale` overrides it.
pub const DEFAULT_MINUTES_PER_REAL_SECOND: f32 = 1.0;
/// Start time of a fresh game, in game minutes (15:00, the centre of the "day" slot, which matches
/// the previous fixed `DEFAULT_TIME_SLOT = 1`).
pub const DEFAULT_START_MINUTES: u16 = 15 * 60;
/// ASSUMED rest duration: resting advances to the start of the next slot (the original's rest menu
/// advances to a chosen phase, `n2-base-camps-and-travel.md` §2; the menu durations are UNKNOWN).
pub const REST_ADVANCES_TO_NEXT_SLOT: bool = true;

/// `smoothstep(f)`, used so the blend has no kink at slot centres.
pub fn smoothstep(f: f32) -> f32 {
    let x = f.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Time of day in hours 0..24, normalised; non-finite input becomes 0.
pub fn normalize_hours(t: f32) -> f32 {
    if t.is_finite() {
        t.rem_euclid(24.0)
    } else {
        0.0
    }
}

/// Centre of slot `s` in hours: 08:30 morning, 15:00 day, 19:30 evening, 01:00 night (the middle
/// of each [`SLOT_START_HOURS`] band; the night band wraps).
pub fn slot_centre_hours(slot: usize) -> f32 {
    const CENTRES: [f32; SLOT_COUNT] = [8.5, 15.0, 19.5, 1.0];
    CENTRES[slot % SLOT_COUNT]
}

/// Centre of slot `s` in game minutes (0..1439).
pub fn slot_centre_minutes(slot: usize) -> u16 {
    (slot_centre_hours(slot) * 60.0) as u16
}

/// Slot whose band contains `t_hours`: night 21-05, morning 05-12, day 12-18, evening 18-21.
/// Slot 3 = night is VERIFIED by the spawn data; the band hours are PROBABLE (see the module docs).
pub fn slot_of_hours(t_hours: f32) -> usize {
    let t = normalize_hours(t_hours);
    // Ascending: [5, 12, 18, 21]; the gaps are slots 0, 1, 2 and the wrap 21..5 is slot 3.
    if t < SLOT_START_HOURS[1] {
        if t < SLOT_START_HOURS[0] { 3 } else { 0 }
    } else if t < SLOT_START_HOURS[2] {
        1
    } else if t < SLOT_START_HOURS[3] {
        2
    } else {
        3
    }
}

/// Slot of a minute-of-day value.
pub fn slot_of_minutes(minutes: u16) -> usize {
    slot_of_hours(f32::from(minutes) / 60.0)
}

/// The two slots bracketing `t_hours` and the smooth 0..1 factor between them. At a slot centre
/// the result is `(slot, next, 0.0)` (exactly that slot's decoded values); at a band boundary it
/// is 50/50 (`f = 0.5`). Continuous everywhere; the shape (smoothstep halves) is ASSUMED.
pub fn slot_blend(t_hours: f32) -> (usize, usize, f32) {
    let t = normalize_hours(t_hours);
    let s = slot_of_hours(t);
    let start = SLOT_START_HOURS[s];
    let len = (SLOT_START_HOURS[(s + 1) % SLOT_COUNT] - start).rem_euclid(24.0);
    // Offsets inside the band: at the boundary 0, at the centre `to_centre`, at the next
    // boundary `len`; both halve-spans are nonzero (3.5/3/1.5/4 and 3.5/3/1.5/4 hours).
    let u = (t - start).rem_euclid(24.0);
    let to_centre = (slot_centre_hours(s) - start).rem_euclid(24.0);
    if u < to_centre {
        let f = 0.5 + 0.5 * smoothstep(u / to_centre);
        ((s + SLOT_COUNT - 1) % SLOT_COUNT, s, f)
    } else {
        let f = 0.5 * smoothstep((u - to_centre) / (len - to_centre));
        (s, (s + 1) % SLOT_COUNT, f)
    }
}

fn mix1(a: f32, b: f32, f: f32) -> f32 {
    a + (b - a) * f
}
fn mix3(a: [f32; 3], b: [f32; 3], f: f32) -> [f32; 3] {
    [mix1(a[0], b[0], f), mix1(a[1], b[1], f), mix1(a[2], b[2], f)]
}
fn mix4(a: [f32; 4], b: [f32; 4], f: f32) -> [f32; 4] {
    [mix1(a[0], b[0], f), mix1(a[1], b[1], f), mix1(a[2], b[2], f), mix1(a[3], b[3], f)]
}

/// Component-wise blend of two decoded environments (`f = 0` gives `a`, `f = 1` gives `b`).
/// ASSUMED: linear interpolation in linear RGB; the sun direction is mixed and re-normalised.
pub fn blend_env(a: &ZoneEnv, b: &ZoneEnv, f: f32) -> ZoneEnv {
    let f = f.clamp(0.0, 1.0);
    if f <= 0.0 {
        return a.clone();
    }
    if f >= 1.0 {
        return b.clone();
    }
    let dir = mix3(a.sun_dir, b.sun_dir, f);
    let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let sun_dir = if len > 1e-4 {
        [dir[0] / len, dir[1] / len, dir[2] / len]
    } else {
        a.sun_dir
    };
    ZoneEnv {
        sun_color: mix3(a.sun_color, b.sun_color, f),
        sun_intensity: mix1(a.sun_intensity, b.sun_intensity, f),
        sun_dir,
        ambient: mix4(a.ambient, b.ambient, f),
        sky: std::array::from_fn(|i| (mix3(a.sky[i].0, b.sky[i].0, f), mix1(a.sky[i].1, b.sky[i].1, f))),
        fog_color: mix4(a.fog_color, b.fog_color, f),
        fog_near: mix1(a.fog_near, b.fog_near, f),
        fog_far: mix1(a.fog_far, b.fog_far, f).max(1.0),
        bloom_intensity: mix1(a.bloom_intensity, b.bloom_intensity, f),
    }
}

// ---------------------------------------------------------------- clock

/// The running game clock. `t_hours` is the time of day 0..24 (a fresh day starts at 15:00).
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct GameClock {
    /// Game minutes per real second (`--time-scale`; ASSUMED default 1.0).
    pub minutes_per_real_second: f32,
    /// Time of day in hours, 0..24.
    pub t_hours: f32,
    /// `--time-slot` pinned the clock: it does not advance.
    pub pinned: bool,
}

impl Default for GameClock {
    fn default() -> Self {
        Self { minutes_per_real_second: DEFAULT_MINUTES_PER_REAL_SECOND, t_hours: f32::from(DEFAULT_START_MINUTES) / 60.0, pinned: false }
    }
}

impl GameClock {
    /// A clock at `t_hours` (normalised), optionally pinned, with a guarded speed.
    pub fn new(minutes_per_real_second: f32, t_hours: f32, pinned: bool) -> Self {
        let speed = if minutes_per_real_second.is_finite() && minutes_per_real_second >= 0.0 {
            minutes_per_real_second
        } else {
            DEFAULT_MINUTES_PER_REAL_SECOND
        };
        Self { minutes_per_real_second: speed, t_hours: normalize_hours(t_hours), pinned }
    }

    /// A running clock starting at a minute-of-day.
    pub fn from_minutes(minutes_per_real_second: f32, minutes: u16) -> Self {
        Self::new(minutes_per_real_second, f32::from(minutes) / 60.0, false)
    }

    /// A pinned clock showing exactly slot `s` (its centre); `--time-slot` and the examples use it.
    pub fn pinned_at_slot(slot: usize) -> Self {
        Self::new(DEFAULT_MINUTES_PER_REAL_SECOND, slot_centre_hours(slot), true)
    }

    /// A pinned clock at an arbitrary hour (tests, `look_headless`).
    pub fn pinned_at_hours(t_hours: f32) -> Self {
        Self::new(DEFAULT_MINUTES_PER_REAL_SECOND, t_hours, true)
    }

    /// Advance by one real frame. Pinned clocks and non-finite/negative deltas do nothing; the day
    /// wraps at 24:00.
    pub fn advance(&mut self, dt_seconds: f32) {
        if self.pinned || !dt_seconds.is_finite() || dt_seconds <= 0.0 {
            return;
        }
        self.t_hours = (self.t_hours + dt_seconds * self.minutes_per_real_second / 60.0).rem_euclid(24.0);
    }

    /// Rest: jump to the start of the next slot (05:00, 12:00, 18:00 or 21:00; ASSUMED duration;
    /// the original offers a choice of phase, see the module docs). Pinned clocks do not move.
    pub fn advance_to_next_slot(&mut self) {
        if self.pinned || !REST_ADVANCES_TO_NEXT_SLOT {
            return;
        }
        let t = self.t_hours;
        let mut best = f32::INFINITY;
        for boundary in SLOT_START_HOURS {
            let delta = (boundary - t).rem_euclid(24.0);
            if delta > 1e-6 && delta < best {
                best = delta;
            }
        }
        if best.is_finite() {
            self.t_hours = (t + best).rem_euclid(24.0);
        }
    }

    /// Whole game minutes since midnight, 0..=1439.
    pub fn minutes(&self) -> u16 {
        let m = (self.t_hours * 60.0).floor();
        (m.rem_euclid(1440.0)) as u16
    }

    /// The two slots around the current time and their smooth blend factor.
    pub fn blend(&self) -> (usize, usize, f32) {
        slot_blend(self.t_hours)
    }

    /// Slot whose segment contains the current time.
    pub fn slot(&self) -> usize {
        slot_of_hours(self.t_hours)
    }

    /// `HH:MM` for HUDs/toasts.
    pub fn hhmm(&self) -> String {
        let m = self.minutes();
        format!("{:02}:{:02}", m / 60, m % 60)
    }
}

// ---------------------------------------------------------------- command line

/// Value of `--time-slot N` when the flag is present and in range (0..=3).
pub fn time_slot_arg(args: &[String]) -> Option<usize> {
    super::cli_flags::parse_or_warn(args, "--time-slot", "a slot 0-3", "the clock", |s: &usize| *s < SLOT_COUNT)
}

/// Value of `--time-scale X` (game minutes per real second); a bad or negative value falls back to
/// the ASSUMED default.
pub fn time_scale_from_args(args: &[String]) -> f32 {
    super::cli_flags::parse_or_warn(
        args,
        "--time-scale",
        "a number >= 0",
        DEFAULT_MINUTES_PER_REAL_SECOND,
        |v: &f32| v.is_finite() && *v >= 0.0,
    )
    .unwrap_or(DEFAULT_MINUTES_PER_REAL_SECOND)
}

/// The clock the app starts with: `--time-slot` pins to that slot, otherwise `--time-scale` runs
/// from the saved minutes (or [`DEFAULT_START_MINUTES`] for a fresh game).
pub fn clock_from_args(args: &[String], start_minutes: Option<u16>) -> GameClock {
    match time_slot_arg(args) {
        Some(slot) => GameClock::pinned_at_slot(slot),
        None => GameClock::from_minutes(time_scale_from_args(args), start_minutes.unwrap_or(DEFAULT_START_MINUTES)),
    }
}

// ---------------------------------------------------------------- Bevy glue

/// Installs the [`GameClock`] resource (from `--time-slot` / `--time-scale` / the saved minutes),
/// advances it while the world or a battle runs (never while paused, like play time) and mirrors it
/// into [`PlayerProgress`] so the save always stores the live time.
pub struct GameClockPlugin;

impl GameClockPlugin {
    /// `args` is the process argument list (like the other `*_from_args` helpers).
    pub fn install(app: &mut App, args: &[String], start_minutes: Option<u16>) {
        let clock = clock_from_args(args, start_minutes);
        println!(
            "Clock: {} ({:.2} game min per real second{})",
            clock.hhmm(),
            clock.minutes_per_real_second,
            if clock.pinned { ", pinned by --time-slot" } else { "" }
        );
        app.insert_resource(clock)
            .add_systems(Update, (tick_clock.run_if(counts_play_time), mirror_clock_to_progress).chain());
    }
}

fn tick_clock(time: Res<Time>, mut clock: ResMut<GameClock>) {
    clock.advance(time.delta_secs());
}

/// Keep the save's time in sync with the clock (the viewer always has both; tiny test apps may not).
fn mirror_clock_to_progress(clock: Res<GameClock>, progress: Option<ResMut<PlayerProgress>>) {
    if let Some(mut p) = progress {
        p.time_minutes = clock.minutes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(sun: f32, tint: [f32; 3]) -> ZoneEnv {
        ZoneEnv {
            sun_color: tint,
            sun_intensity: sun,
            sun_dir: [0.0, -1.0, 0.0],
            ambient: [0.2, 0.3, 0.4, 0.5],
            sky: [
                (tint, 0.0),
                ([tint[0] * 0.8, tint[1] * 0.8, tint[2] * 0.8], 0.25),
                ([tint[0] * 0.6, tint[1] * 0.6, tint[2] * 0.6], 0.5),
                ([tint[0] * 0.4, tint[1] * 0.4, tint[2] * 0.4], 0.75),
            ],
            fog_color: [0.5, 0.6, 0.7, 0.5],
            fog_near: 0.0,
            fog_far: 600.0,
            bloom_intensity: 0.2,
        }
    }

    #[test]
    fn slot_centres_are_exactly_their_slot() {
        for s in 0..SLOT_COUNT {
            let (a, b, f) = slot_blend(slot_centre_hours(s));
            assert_eq!((a, b), (s, (s + 1) % SLOT_COUNT), "centre of slot {s}");
            assert!(f < 1e-6, "centre of slot {s} must be exactly that slot");
        }
        // 08:30 = morning (slot 0), 15:00 day (1), 19:30 evening (2), 01:00 night (3).
        assert_eq!(slot_centre_hours(0), 8.5);
        assert_eq!(slot_centre_hours(1), 15.0);
        assert_eq!(slot_centre_hours(2), 19.5);
        assert_eq!(slot_centre_hours(3), 1.0);
        assert_eq!(slot_blend(9.0).0, 0);
        assert_eq!(slot_blend(15.0).0, 1);
        assert_eq!(slot_blend(21.0).0, 2, "21:00 is the evening/night boundary");
        assert_eq!(slot_blend(3.0).0, 3);
        assert_eq!(slot_centre_minutes(0), 510);
        assert_eq!(slot_centre_minutes(1), 15 * 60);
        assert_eq!(slot_centre_minutes(2), 19 * 60 + 30);
        assert_eq!(slot_centre_minutes(3), 60);
    }

    #[test]
    fn slot_boundaries_are_fifty_fifty() {
        // The four band boundaries: 05:00, 12:00, 18:00, 21:00 (PROBABLE hours).
        for h in SLOT_START_HOURS {
            let (_, _, f) = slot_blend(h);
            assert!((f - 0.5).abs() < 1e-5, "boundary {h} must be 50/50, got {f}");
        }
    }

    #[test]
    fn slot_segments_contain_their_slot() {
        // morning 05-12, day 12-18, evening 18-21, night 21-05 (PROBABLE bands; slot 3 = night
        // is VERIFIED by the spawn multipliers).
        assert_eq!(slot_of_hours(0.0), 3);
        assert_eq!(slot_of_hours(4.99), 3);
        assert_eq!(slot_of_hours(5.0), 0);
        assert_eq!(slot_of_hours(8.5), 0);
        assert_eq!(slot_of_hours(11.9), 0);
        assert_eq!(slot_of_hours(12.0), 1);
        assert_eq!(slot_of_hours(17.9), 1);
        assert_eq!(slot_of_hours(18.0), 2);
        assert_eq!(slot_of_hours(20.9), 2);
        assert_eq!(slot_of_hours(21.0), 3);
        assert_eq!(slot_of_hours(23.9), 3);
        assert_eq!(slot_of_minutes(3 * 60), 3);
        assert_eq!(slot_of_minutes(14 * 60), 1);
        assert_eq!(slot_of_minutes(6 * 60), 0);
    }

    #[test]
    fn blend_is_continuous_and_closes_the_day() {
        // Four distinct slot environments; the composed environment must follow the pair+f together
        // (the raw factor resets at a slot centre, but there the pair swaps, so the env is smooth).
        let slots: [ZoneEnv; 4] = std::array::from_fn(|s| {
            env(2.8 - 0.6 * s as f32, [0.2 + 0.2 * s as f32, 0.3, 0.9 - 0.1 * s as f32])
        });
        let env_at = |t: f32| {
            let (x, y, f) = slot_blend(t);
            blend_env(&slots[x], &slots[y], f)
        };
        let mut prev = env_at(0.0);
        let mut max_env_jump = 0.0f32;
        for i in 1..=4800 {
            let t = i as f32 * 24.0 / 4800.0; // 0..24 in 0.005 h steps
            let p = env_at(t);
            max_env_jump = max_env_jump.max((p.sun_intensity - prev.sun_intensity).abs());
            for (x, y) in p.sun_color.iter().zip(prev.sun_color.iter()) {
                max_env_jump = max_env_jump.max((x - y).abs());
            }
            assert!(p.sun_intensity.is_finite());
            prev = p;
        }
        assert!(max_env_jump < 0.02, "no env jump > eps, got {max_env_jump}");
        // The cycle closes: 00:00 and 24:00 are the same blend, and every neighbouring pair appears.
        assert_eq!(slot_blend(0.0), slot_blend(24.0));
        assert_eq!(env_at(0.0), env_at(24.0));
        let mut pairs = std::collections::BTreeSet::new();
        for i in 0..480 {
            let (s, n, _) = slot_blend(i as f32 * 24.0 / 480.0);
            pairs.insert((s, n));
        }
        assert_eq!(pairs.len(), SLOT_COUNT, "all four neighbouring pairs are visited");
        assert_eq!((slot_blend(0.0).0, slot_blend(0.0).1), (2, 3), "midnight blends evening into night");
    }

    #[test]
    fn blend_hits_both_ends_and_normalises_the_sun() {
        let a = env(2.8, [1.0, 1.0, 1.0]);
        let b = env(0.3, [0.0, 0.0, 1.0]);
        assert_eq!(blend_env(&a, &b, 0.0), a);
        assert_eq!(blend_env(&a, &b, 1.0), b);
        let m = blend_env(&a, &b, 0.5);
        assert!((m.sun_intensity - 1.55).abs() < 1e-5);
        assert!((m.sun_color[0] - 0.5).abs() < 1e-5);
        let mut c = a.clone();
        c.sun_dir = [0.0, -2.0, 0.0];
        let l = blend_env(&a, &c, 0.5).sun_dir;
        assert!(((l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt() - 1.0).abs() < 1e-5);
        // Opposite directions degenerate: keeps `a`'s direction instead of NaN.
        let mut d = a.clone();
        d.sun_dir = [0.0, 1.0, 0.0];
        assert_eq!(blend_env(&a, &d, 0.5).sun_dir, a.sun_dir);
    }

    #[test]
    fn clock_advances_wraps_and_respects_pinning() {
        let mut c = GameClock::from_minutes(2.0, 23 * 60); // 23:00, 2 min/s
        c.advance(30.0); // +60 game minutes -> 00:00
        assert!((c.t_hours - 0.0).abs() < 1e-4);
        assert_eq!(c.minutes(), 0);
        assert_eq!(c.hhmm(), "00:00");
        c.advance(60.0 * 60.0); // a whole game day
        assert_eq!(c.minutes(), 0);
        c.advance(f32::NAN);
        c.advance(-5.0);
        assert_eq!(c.minutes(), 0, "bad deltas are ignored");
        let mut p = GameClock::pinned_at_hours(9.0);
        p.advance(600.0);
        assert_eq!(p.minutes(), 9 * 60, "a pinned clock never moves");
        assert_eq!(GameClock::new(f32::NAN, 9.0, false).minutes_per_real_second, DEFAULT_MINUTES_PER_REAL_SECOND);
    }

    #[test]
    fn rest_advances_to_the_next_slot_and_wraps() {
        let mut c = GameClock::from_minutes(1.0, 13 * 60); // 13:00 -> evening 18:00
        c.advance_to_next_slot();
        assert_eq!(c.hhmm(), "18:00");
        let mut c = GameClock::from_minutes(1.0, 18 * 60); // exactly a boundary -> the next one
        c.advance_to_next_slot();
        assert_eq!(c.hhmm(), "21:00");
        let mut c = GameClock::from_minutes(1.0, 23 * 60); // 23:00 -> night's end 05:00
        c.advance_to_next_slot();
        assert_eq!(c.hhmm(), "05:00");
        let mut c = GameClock::from_minutes(1.0, 30); // 00:30 -> 05:00
        c.advance_to_next_slot();
        assert_eq!(c.hhmm(), "05:00");
        let mut p = GameClock::pinned_at_slot(1);
        p.advance_to_next_slot();
        assert_eq!(p.hhmm(), "15:00", "a pinned clock does not rest-advance");
    }

    #[test]
    fn args_parse_time_slot_and_time_scale() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(time_slot_arg(&a(&[])), None);
        assert_eq!(time_slot_arg(&a(&["--time-slot", "3"])), Some(3));
        assert_eq!(time_slot_arg(&a(&["--time-slot", "9"])), None, "out of range");
        assert_eq!(time_slot_arg(&a(&["--time-slot", "x"])), None);
        assert_eq!(time_scale_from_args(&a(&[])), DEFAULT_MINUTES_PER_REAL_SECOND);
        assert_eq!(time_scale_from_args(&a(&["--time-scale", "60"])), 60.0);
        assert_eq!(time_scale_from_args(&a(&["--time-scale", "0"])), 0.0);
        assert_eq!(time_scale_from_args(&a(&["--time-scale", "-1"])), DEFAULT_MINUTES_PER_REAL_SECOND);
        assert_eq!(time_scale_from_args(&a(&["--time-scale", "NaN"])), DEFAULT_MINUTES_PER_REAL_SECOND);
        let pinned = clock_from_args(&a(&["--time-slot", "3"]), Some(123));
        assert!(pinned.pinned);
        assert_eq!(pinned.hhmm(), "01:00");
        let running = clock_from_args(&a(&["--time-scale", "10"]), Some(8 * 60));
        assert!(!running.pinned && running.minutes_per_real_second == 10.0);
        assert_eq!(running.hhmm(), "08:00");
        assert_eq!(clock_from_args(&a(&[]), None).hhmm(), "15:00");
    }

    /// The real system set: a running clock ticks only in World/Battle (like play time) and the
    /// mirror system keeps `PlayerProgress.time_minutes` at the clock's time.
    #[test]
    fn plugin_ticks_and_mirrors_into_the_progress() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(250)))
            .insert_resource(PlayerProgress { time_minutes: 0, ..Default::default() });
        GameClockPlugin::install(&mut app, &["prog".into()], Some(9 * 60));
        app.update(); // warm-up
        for _ in 0..4 {
            app.update();
        }
        let clock = *app.world().resource::<GameClock>();
        assert_eq!(clock.hhmm(), "09:01", "1 s at 1 min/s is one game minute");
        assert_eq!(app.world().resource::<PlayerProgress>().time_minutes, 541,
                   "the mirror system writes the save's time");
    }
}
