//! Weather: the per-area lottery, the change timer and the simple rain/snow particles.
//!
//! Data (decoded, `reports/data-progress/world_events.md` §3):
//! - `data/world_events_area_weather.tsv` — per-area weights for weather ids 2..9
//!   (sunny, cloudy, rain, snow, drought, fog, rainstorm, snowstorm; rows sum to 100, LIKELY
//!   arithmetic). `ha_area00`/`06` are always sunny; area 05 is snowy; area 03 drought/storm.
//!   Fields `f1_raw`/`f2_raw` (2/4 or 4/6) are UNKNOWN; the port reads them as a duration range in
//!   game hours (ASSUMED).
//! - `data/weather/weather_lottery_matrix.tsv` — the global default weights (40/20/15/10/5/5/3/2),
//!   used when the area row is missing. `disable_system_flag` is read but IGNORED (the flag system
//!   does not exist in the port yet).
//!
//! Everything about the *visuals* is ASSUMED: fog/light multipliers per weather, rain/snow particle
//! counts, speeds and sizes are this port's own, because the game's `light.gfpak` weather modifier
//! and the `local_weather` placement type are not decoded and no decoded weather shader exists.
//! The lottery itself (selection + weights) is the decoded part; the game uses Xoroshiro128+ (not
//! ported), so the port uses the engine's `XorShift32` with a documented seed.

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

use crate::battle_engine::{BattleRng, XorShift32};

use super::clock::GameClock;

/// Weather ids and their decoded names, in data order (2..=9).
pub const WEATHER_IDS: [(u8, &str); 8] = [
    (2, "sunny"),
    (3, "cloudy"),
    (4, "rain"),
    (5, "snow"),
    (6, "drought"),
    (7, "fog"),
    (8, "rainstorm"),
    (9, "snowstorm"),
];

/// Decoded name of a weather id (`"?"` when unknown).
pub fn weather_name(id: u8) -> &'static str {
    WEATHER_IDS.iter().find(|(i, _)| *i == id).map(|(_, n)| *n).unwrap_or("?")
}

/// Parse `--weather`/`--weather-seed` values: an id 2..=9 or a decoded name (case-insensitive).
pub fn weather_id_from_arg(s: &str) -> Option<u8> {
    if let Ok(id) = s.parse::<u8>() {
        return (2..=9).contains(&id).then_some(id);
    }
    WEATHER_IDS.iter().find(|(_, n)| n.eq_ignore_ascii_case(s)).map(|(i, _)| *i)
}

/// Area weather row: duration range and weights (id order 2..=9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaWeather {
    /// UNKNOWN units, read as game hours (ASSUMED).
    pub f1: u32,
    pub f2: u32,
    pub weights: [(u8, u32); 8],
}

/// The global default weights (`weather_lottery_matrix.tsv`, decoded `default_weight` column).
pub fn global_default_weights() -> [(u8, u32); 8] {
    [(2, 40), (3, 20), (4, 15), (5, 10), (6, 5), (7, 5), (8, 3), (9, 2)]
}

/// Split a sheet into `(header column -> index, data rows)`; `#` lines are skipped.
fn sheet_rows(text: &str) -> Option<(Vec<&str>, Vec<Vec<&str>>)> {
    let mut lines = text.lines().skip_while(|l| l.starts_with('#') || l.trim().is_empty());
    let header: Vec<&str> = lines.next()?.split('\t').map(str::trim).collect();
    let rows = lines.filter(|l| !l.starts_with('#') && !l.trim().is_empty()).map(|l| l.split('\t').collect()).collect();
    Some((header, rows))
}

/// Read the per-area weather row (`ha_areaNN`) from `world_events_area_weather.tsv`.
/// The `total` column is not trusted: only the eight `weight_*` columns are read.
pub fn parse_area_weather(text: &str, area: &str) -> Option<AreaWeather> {
    let (header, rows) = sheet_rows(text)?;
    let col = |name: &str| header.iter().position(|h| h.starts_with(name));
    let (area_c, f1_c, f2_c) = (col("area")?, col("f1_raw")?, col("f2_raw")?);
    let w_c: Vec<usize> = WEATHER_IDS
        .iter()
        .map(|(_, n)| header.iter().position(|h| h.starts_with(&format!("weight_{n}"))))
        .collect::<Option<_>>()?;
    let row = rows.iter().find(|r| r.get(area_c) == Some(&area))?;
    let num = |i: usize| row.get(i).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    let mut weights = [(0u8, 0u32); 8];
    for (i, ((id, _), c)) in WEATHER_IDS.iter().zip(&w_c).enumerate() {
        weights[i] = (*id, num(*c));
    }
    Some(AreaWeather { f1: num(f1_c), f2: num(f2_c), weights })
}

/// Read the global `default_weight` column keyed by `weather_id`.
pub fn parse_global_lottery(text: &str) -> Option<[(u8, u32); 8]> {
    let (header, rows) = sheet_rows(text)?;
    let id_c = header.iter().position(|h| h.starts_with("weather_id"))?;
    let w_c = header.iter().position(|h| h.starts_with("default_weight"))?;
    let mut weights = [(0u8, 0u32); 8];
    for (i, (id, _)) in WEATHER_IDS.iter().enumerate() {
        let row = rows.iter().find(|r| r.get(id_c).and_then(|v| v.parse::<u8>().ok()) == Some(*id))?;
        weights[i] = (*id, row.get(w_c).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0));
    }
    Some(weights)
}

/// Find a committed data sheet from the usual working directories (same pattern as the lighting
/// sheet; `crates/pla` and the repo root are both supported).
pub fn find_sheet(rel: &str) -> Option<std::path::PathBuf> {
    [
        rel.to_string(),
        format!("../{rel}"),
        format!("../../{rel}"),
        format!("/home/nico/work/decompilacion/{rel}"),
    ]
    .iter()
    .map(std::path::Path::new)
    .find(|p| p.exists())
    .map(std::path::Path::to_path_buf)
}

/// Weighted pick over `(id, weight)` pairs; deterministic for a given RNG state. Zero total -> None.
pub fn select_weighted(rng: &mut XorShift32, weights: &[(u8, u32)]) -> Option<u8> {
    let total: u32 = weights.iter().map(|(_, w)| *w).sum();
    if total == 0 {
        return None;
    }
    let mut roll = rng.next_u32() % total;
    for &(id, w) in weights {
        if roll < w {
            return Some(id);
        }
        roll -= w;
    }
    weights.last().map(|(id, _)| *id)
}

/// The rules the weather machine follows: the lottery weights plus the change-interval range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeatherRules {
    pub weights: [(u8, u32); 8],
    /// Change interval in game hours (decoded `f1_raw`/`f2_raw`, ASSUMED to be hours).
    pub min_hours: u32,
    pub max_hours: u32,
}

impl WeatherRules {
    /// ASSUMED fallback (global lottery weights, 2..4 game hours) when no sheet is readable.
    pub const FALLBACK: Self = Self { weights: [(2, 40), (3, 20), (4, 15), (5, 10), (6, 5), (7, 5), (8, 3), (9, 2)], min_hours: 2, max_hours: 4 };

    /// Rules of one area: its decoded row when present (header column = `area`), the global lottery
    /// weights otherwise.
    pub fn for_area(area_row: Option<&str>, lottery: Option<&str>, area: &str) -> Self {
        let parsed = area_row.and_then(|t| parse_area_weather(t, area));
        let weights = parsed
            .map(|a| a.weights)
            .or_else(|| lottery.and_then(parse_global_lottery))
            .unwrap_or(Self::FALLBACK.weights);
        match parsed {
            Some(a) if a.f1 > 0 && a.f2 >= a.f1 => Self { weights, min_hours: a.f1, max_hours: a.f2 },
            _ => Self { weights, ..Self::FALLBACK },
        }
    }

    /// A change interval in game minutes, uniform over `[min_hours, max_hours]` whole hours (ASSUMED).
    pub fn duration_minutes(&self, rng: &mut XorShift32) -> f32 {
        let lo = self.min_hours.max(1);
        let hi = self.max_hours.max(lo);
        f32::from((rng.below(hi - lo + 1) + lo) as u16) * 60.0
    }
}

/// Weather lottery state machine (pure): current id and minutes until the next change.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherMachine {
    pub id: u8,
    pub remaining_minutes: f32,
}

/// Advance the machine by `advance_minutes` of game time; when the timer elapses, draw the next
/// weather with the decoded weights and a fresh interval. Deterministic for a given RNG state.
pub fn tick_weather(machine: &mut WeatherMachine, rules: &WeatherRules, rng: &mut XorShift32, advance_minutes: f32) {
    if !advance_minutes.is_finite() || advance_minutes <= 0.0 {
        return;
    }
    machine.remaining_minutes -= advance_minutes;
    if machine.remaining_minutes <= 0.0 {
        machine.id = select_weighted(rng, &rules.weights).unwrap_or(machine.id);
        machine.remaining_minutes = rules.duration_minutes(rng);
    }
}

/// Precipitation kind of a weather id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precip {
    None,
    Rain,
    Snow,
}

/// What the renderer does with the current weather. All numbers ASSUMED (see the module docs).
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct WeatherVisual {
    pub id: u8,
    /// Multiplier on the fog density/strength.
    pub fog_mult: f32,
    /// Multiplier on the sun and ambient light.
    pub light_mult: f32,
    pub precip: Precip,
}

impl WeatherVisual {
    pub const CLEAR: Self = Self { id: 2, fog_mult: 1.0, light_mult: 1.0, precip: Precip::None };

    /// ASSUMED per-weather look.
    pub fn for_id(id: u8) -> Self {
        match id {
            3 => Self { id, fog_mult: 1.15, light_mult: 0.92, precip: Precip::None },
            4 => Self { id, fog_mult: 1.60, light_mult: 0.78, precip: Precip::Rain },
            5 => Self { id, fog_mult: 1.50, light_mult: 0.88, precip: Precip::Snow },
            6 => Self { id, fog_mult: 0.80, light_mult: 1.08, precip: Precip::None },
            7 => Self { id, fog_mult: 3.00, light_mult: 0.85, precip: Precip::None },
            8 => Self { id, fog_mult: 2.00, light_mult: 0.65, precip: Precip::Rain },
            9 => Self { id, fog_mult: 2.20, light_mult: 0.70, precip: Precip::Snow },
            _ => Self { id, ..Self::CLEAR },
        }
    }

    pub fn name(&self) -> &'static str {
        weather_name(self.id)
    }
}

// ---------------------------------------------------------------- bevy glue

/// Live weather: machine, rules, RNG, last clock reading and the optional forced id.
#[derive(Resource, Debug)]
pub struct WeatherState {
    pub machine: WeatherMachine,
    pub rules: WeatherRules,
    pub rng: XorShift32,
    /// `--weather` / test override: the machine never draws.
    pub force: Option<u8>,
    last_minutes: Option<f32>,
}

/// One falling particle (rain streak or snow flake).
#[derive(Component, Debug, Clone, Copy)]
pub struct PrecipParticle {
    /// Fall speed (m/s), before per-particle variation.
    pub fall: f32,
    /// Horizontal sway amplitude (m/s).
    pub sway: f32,
    /// Stable per-particle phase for scatter and sway.
    pub phase: f32,
}

/// Shared particle assets and what is currently spawned.
#[derive(Resource, Debug)]
struct PrecipAssets {
    rain: Handle<Mesh>,
    snow: Handle<Mesh>,
    rain_mat: Handle<StandardMaterial>,
    snow_mat: Handle<StandardMaterial>,
    spawned: Precip,
}

/// ASSUMED particle budgets (cheap instanced quads; no custom shader).
pub const RAIN_PARTICLES: usize = 800;
pub const SNOW_PARTICLES: usize = 450;
/// ASSUMED fall speeds (m/s) and spawn box.
pub const RAIN_FALL: f32 = 17.0;
pub const SNOW_FALL: f32 = 2.1;
const SPAWN_RADIUS: f32 = 30.0;
const FALL_BELOW: f32 = 8.0;
const TOP_ABOVE: f32 = 14.0;

/// Installs the weather machine for `area` (`ha_areaNN`) and the particle systems. Read from
/// `--weather <id|name>` when `force` is None.
pub struct WeatherPlugin {
    pub area: u8,
    pub force: Option<u8>,
    pub seed: u32,
}

impl WeatherPlugin {
    /// `--weather <2..9|name>` and `--weather-seed <u32>` from the process arguments.
    pub fn from_args(area: u8, args: &[String]) -> Self {
        let force = args
            .iter()
            .position(|a| a == "--weather")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| weather_id_from_arg(s));
        let seed = args
            .iter()
            .position(|a| a == "--weather-seed")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0x9e37_79b9);
        Self { area, force, seed }
    }
}

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        let name = format!("ha_area{:02}", self.area);
        let rules = WeatherRules::for_area(
            find_sheet("data/world_events_area_weather.tsv").and_then(|p| std::fs::read_to_string(p).ok()).as_deref(),
            find_sheet("data/weather/weather_lottery_matrix.tsv").and_then(|p| std::fs::read_to_string(p).ok()).as_deref(),
            &name,
        );
        let mut rng = XorShift32(self.seed.max(1));
        let id = self.force.or_else(|| select_weighted(&mut rng, &rules.weights)).unwrap_or(2);
        let machine = WeatherMachine { id, remaining_minutes: rules.duration_minutes(&mut rng) };
        println!(
            "Weather: {} ({}) in {}; changes every {}-{} game h (ASSUMED)",
            weather_name(id),
            id,
            name,
            rules.min_hours,
            rules.max_hours
        );
        app.insert_resource(WeatherState { machine, rules, rng, force: self.force, last_minutes: None })
            .insert_resource(WeatherVisual::for_id(id))
            .add_systems(Startup, create_precip_assets)
            .add_systems(Update, (advance_weather, sync_precip, move_precip).chain());
    }
}

fn create_precip_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut quad = |w: f32, h: f32| meshes.add(Rectangle::new(w, h));
    let unlit = |color: Color| StandardMaterial {
        base_color: color,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    };
    commands.insert_resource(PrecipAssets {
        rain: quad(0.025, 0.7),
        snow: quad(0.05, 0.05),
        rain_mat: materials.add(unlit(Color::srgba(0.62, 0.72, 0.95, 0.5))),
        snow_mat: materials.add(unlit(Color::srgba(0.95, 0.97, 1.0, 0.85))),
        spawned: Precip::None,
    });
}

/// Game-minute delta of the clock since the last frame, wrapping at midnight; `None` without a clock.
fn clock_delta(clock: &GameClock, last: &mut Option<f32>) -> f32 {
    let now = f32::from(clock.minutes());
    let delta = last.map(|l| (now - l).rem_euclid(1440.0)).unwrap_or(0.0);
    *last = Some(now);
    delta
}

fn advance_weather(
    clock: Option<Res<GameClock>>,
    mut state: ResMut<WeatherState>,
    mut visual: ResMut<WeatherVisual>,
) {
    let Some(clock) = clock else { return };
    let delta = clock_delta(&clock, &mut state.last_minutes);
    if state.force.is_some() || delta <= 0.0 {
        return; // forced, or a pinned clock (no game time passes)
    }
    let before = state.machine.id;
    let WeatherState { machine, rules, rng, .. } = &mut *state;
    tick_weather(machine, rules, rng, delta);
    if state.machine.id != before {
        *visual = WeatherVisual::for_id(state.machine.id);
        println!("Weather: {} ({}) now", visual.name(), visual.id);
    }
}

/// Deterministic 2D scatter in -1..1 from a particle phase.
fn scatter(phase: f32) -> (f32, f32) {
    let h = |k: f32| {
        let x = (k * 12.9898).sin() * 43_758.547;
        x - x.floor()
    };
    (h(phase) * 2.0 - 1.0, h(phase + 7.13) * 2.0 - 1.0)
}

/// Spawn/despawn the particle set when the precipitation kind changes.
fn sync_precip(
    mut commands: Commands,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    visual: Res<WeatherVisual>,
    mut assets: ResMut<PrecipAssets>,
    existing: Query<Entity, With<PrecipParticle>>,
) {
    if assets.spawned == visual.precip {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    assets.spawned = visual.precip;
    let (mesh, mat, count, fall, sway) = match visual.precip {
        Precip::None => return,
        Precip::Rain => (assets.rain.clone(), assets.rain_mat.clone(), RAIN_PARTICLES, RAIN_FALL, 0.6),
        Precip::Snow => (assets.snow.clone(), assets.snow_mat.clone(), SNOW_PARTICLES, SNOW_FALL, 1.4),
    };
    let cam = camera.iter().next().map(GlobalTransform::translation).unwrap_or(Vec3::ZERO);
    for i in 0..count {
        let phase = i as f32 * 0.618_034;
        let (dx, dz) = scatter(phase);
        let y = cam.y + TOP_ABOVE * ((i as f32 * 0.37).fract());
        let p = PrecipParticle { fall: fall * (0.85 + 0.3 * (phase * 1.7).sin().abs()), sway, phase };
        commands.spawn((
            p,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::from_xyz(cam.x + dx * SPAWN_RADIUS, y, cam.z + dz * SPAWN_RADIUS),
        ));
    }
}

/// Fall, sway and recycle the particles around the camera. Quads billboard towards it.
fn move_precip(
    time: Res<Time>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    mut q: Query<(&mut Transform, &PrecipParticle)>,
) {
    let cam = camera.iter().next().map(GlobalTransform::translation).unwrap_or(Vec3::ZERO);
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (mut t, p) in &mut q {
        let mut pos = t.translation;
        pos.y -= p.fall * dt;
        pos.x += (time.elapsed_secs() * 1.3 + p.phase).sin() * p.sway * dt;
        let far = (pos.x - cam.x) * (pos.x - cam.x) + (pos.z - cam.z) * (pos.z - cam.z) > SPAWN_RADIUS * SPAWN_RADIUS * 1.2;
        if pos.y < cam.y - FALL_BELOW || far {
            let (dx, dz) = scatter(p.phase + time.elapsed_secs() * 0.01);
            pos = Vec3::new(cam.x + dx * SPAWN_RADIUS, cam.y + TOP_ABOVE * 0.8, cam.z + dz * SPAWN_RADIUS);
        }
        t.translation = pos;
        let to_cam = Vec3::new(cam.x - pos.x, 0.0, cam.z - pos.z);
        if to_cam.length_squared() > 1e-4 {
            t.rotation = Quat::from_rotation_y(to_cam.x.atan2(to_cam.z));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area_sheet() -> String {
        let h = "area\tf1_raw\tf2_raw\tweight_sunny\tweight_cloudy\tweight_rain\tweight_snow\tweight_drought\tweight_fog\tweight_rainstorm\tweight_snowstorm\ttotal\n";
        let rows = [
            "ha_area01\t4\t6\t60\t15\t15\t0\t4\t3\t3\t0\t100",
            "ha_area05\t2\t4\t40\t15\t0\t25\t0\t0\t0\t20\t100",
        ];
        format!("# note\n{h}{}\n", rows.join("\n"))
    }

    fn lottery_sheet() -> String {
        let h = "weather_id:u8*\tweather_name:string\tdefault_weight:u8\n";
        let rows: Vec<String> = WEATHER_IDS
            .iter()
            .zip([40u32, 20, 15, 10, 5, 5, 3, 2])
            .map(|((id, name), w)| format!("{id}\t{name}\t{w}"))
            .collect();
        format!("# note\n{h}{}\n", rows.join("\n"))
    }

    #[test]
    fn parses_area_rows_and_global_defaults() {
        let a = parse_area_weather(&area_sheet(), "ha_area01").unwrap();
        assert_eq!((a.f1, a.f2), (4, 6));
        assert_eq!(a.weights.iter().map(|(_, w)| w).sum::<u32>(), 100);
        assert_eq!(a.weights[0], (2, 60), "sunny column");
        assert!(parse_area_weather(&area_sheet(), "ha_area99").is_none());
        let g = parse_global_lottery(&lottery_sheet()).unwrap();
        assert_eq!(g, global_default_weights());
        assert!(parse_global_lottery("not a sheet").is_none());
    }

    #[test]
    fn rules_prefer_the_area_row_and_fall_back_to_the_global_weights() {
        let area = area_sheet();
        let lot = lottery_sheet();
        let r = WeatherRules::for_area(Some(&area), Some(&lot), "ha_area05");
        assert_eq!((r.min_hours, r.max_hours), (2, 4));
        assert_eq!(r.weights[3], (5, 25), "snow weight of area 05");
        // Missing area row -> global weights and the fallback interval.
        let g = WeatherRules::for_area(Some(&area), Some(&lot), "ha_area09");
        assert_eq!(g.weights, global_default_weights());
        assert_eq!((g.min_hours, g.max_hours), (2, 4));
        // Nothing readable -> the documented fallback.
        assert_eq!(WeatherRules::for_area(None, None, "x"), WeatherRules::FALLBACK);
        // A malformed interval (f2 < f1) falls back too.
        let bad = "# note\narea\tf1_raw\tf2_raw\tweight_sunny\tweight_cloudy\tweight_rain\tweight_snow\tweight_drought\tweight_fog\tweight_rainstorm\tweight_snowstorm\nha_area00\t9\t2\t100\t0\t0\t0\t0\t0\t0\t0\n";
        let r = WeatherRules::for_area(Some(bad), None, "ha_area00");
        assert_eq!((r.min_hours, r.max_hours), (2, 4));
        assert_eq!(r.weights[0].1, 100);
    }

    #[test]
    fn weighted_pick_is_deterministic_and_matches_the_weights() {
        let weights = global_default_weights();
        let mut a = XorShift32(0x1234_5678);
        let mut b = XorShift32(0x1234_5678);
        for _ in 0..64 {
            assert_eq!(select_weighted(&mut a, &weights), select_weighted(&mut b, &weights));
        }
        // 10_000 samples: observed share within 2.5 points of the decoded weights.
        let mut rng = XorShift32(0x1234_5678);
        let mut counts = [0u32; 8];
        for _ in 0..10_000 {
            let id = select_weighted(&mut rng, &weights).unwrap();
            let i = WEATHER_IDS.iter().position(|(w, _)| *w == id).unwrap();
            counts[i] += 1;
        }
        let total: u32 = weights.iter().map(|(_, w)| w).sum();
        for (i, (id, w)) in weights.iter().enumerate() {
            let observed = counts[i] as f32 / 10_000.0;
            let expected = *w as f32 / total as f32;
            assert!(
                (observed - expected).abs() < 0.025,
                "weather {id}: observed share {observed:.3} vs expected {expected:.3}"
            );
        }
        // A zero weight never appears, a zero total is refused.
        let mut rng = XorShift32(7);
        let only = select_weighted(&mut rng, &[(3, 0), (5, 10)]).unwrap();
        assert_eq!(only, 5);
        assert_eq!(select_weighted(&mut rng, &[(2, 0), (3, 0)]), None);
    }

    #[test]
    fn machine_changes_only_when_its_timer_elapses() {
        let rules = WeatherRules { weights: global_default_weights(), min_hours: 2, max_hours: 4 };
        let mut rng = XorShift32(99);
        let mut m = WeatherMachine { id: 2, remaining_minutes: 100.0 };
        tick_weather(&mut m, &rules, &mut rng, 99.0);
        assert_eq!(m.id, 2, "the timer has not elapsed");
        assert!((m.remaining_minutes - 1.0).abs() < 1e-5);
        tick_weather(&mut m, &rules, &mut rng, 2.0);
        assert!(WEATHER_IDS.iter().any(|(id, _)| *id == m.id), "a decoded id was drawn");
        assert!(
            (120.0..=240.0).contains(&m.remaining_minutes),
            "a fresh 2..4 game-hour interval, got {}",
            m.remaining_minutes
        );
        // Bad deltas never move the machine.
        let before = m;
        tick_weather(&mut m, &rules, &mut rng, f32::NAN);
        tick_weather(&mut m, &rules, &mut rng, -3.0);
        assert_eq!(m, before);
        // Two machines with the same seed draw the same weather.
        let mut r1 = XorShift32(5);
        let mut r2 = XorShift32(5);
        let mut m1 = WeatherMachine { id: 2, remaining_minutes: 0.1 };
        let mut m2 = m1;
        tick_weather(&mut m1, &rules, &mut r1, 1.0);
        tick_weather(&mut m2, &rules, &mut r2, 1.0);
        assert_eq!(m1.id, m2.id);
    }

    #[test]
    fn visual_multipliers_are_monotone_and_known() {
        let sunny = WeatherVisual::for_id(2);
        assert_eq!(sunny, WeatherVisual::CLEAR);
        assert_eq!((sunny.fog_mult, sunny.light_mult, sunny.precip), (1.0, 1.0, Precip::None));
        let fog = WeatherVisual::for_id(7);
        assert!(fog.fog_mult > sunny.fog_mult && fog.light_mult < sunny.light_mult);
        for id in 3..=9u8 {
            let v = WeatherVisual::for_id(id);
            assert!(v.fog_mult > 0.0 && v.light_mult > 0.0 && v.light_mult <= 1.2);
        }
        assert_eq!(WeatherVisual::for_id(4).precip, Precip::Rain);
        assert_eq!(WeatherVisual::for_id(8).precip, Precip::Rain);
        assert_eq!(WeatherVisual::for_id(5).precip, Precip::Snow);
        assert_eq!(WeatherVisual::for_id(9).precip, Precip::Snow);
        assert_eq!(WeatherVisual::for_id(3).precip, Precip::None);
        // Unknown ids are neutral, never a panic.
        assert_eq!(WeatherVisual::for_id(200).light_mult, 1.0);
        assert_eq!(weather_name(4), "rain");
        assert_eq!(weather_name(250), "?");
    }

    #[test]
    fn weather_arg_parsing() {
        assert_eq!(weather_id_from_arg("4"), Some(4));
        assert_eq!(weather_id_from_arg("rain"), Some(4));
        assert_eq!(weather_id_from_arg("RAIN"), Some(4));
        assert_eq!(weather_id_from_arg("snowstorm"), Some(9));
        assert_eq!(weather_id_from_arg("1"), None);
        assert_eq!(weather_id_from_arg("10"), None);
        assert_eq!(weather_id_from_arg("nonsense"), None);
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let p = WeatherPlugin::from_args(1, &a(&["--weather", "rain", "--weather-seed", "7"]));
        assert_eq!((p.area, p.force, p.seed), (1, Some(4), 7));
        assert_eq!(WeatherPlugin::from_args(0, &a(&[])).force, None);
    }

    /// The committed sheets parse and match the report (area 01 sunny 60, area 05 snow 25).
    /// Skips (green) when the sheets are absent.
    #[test]
    fn real_sheets_parse() {
        let Some(path) = find_sheet("data/world_events_area_weather.tsv") else {
            eprintln!("[skip] data/world_events_area_weather.tsv not found");
            return;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("[skip] {path:?} unreadable");
            return;
        };
        let a1 = parse_area_weather(&text, "ha_area01").expect("area 01 row");
        assert_eq!((a1.f1, a1.f2), (4, 6));
        assert_eq!(a1.weights.iter().map(|(_, w)| w).sum::<u32>(), 100);
        assert_eq!(a1.weights[0], (2, 60));
        let a5 = parse_area_weather(&text, "ha_area05").expect("area 05 row");
        assert_eq!(a5.weights[3], (5, 25), "snow weight");
        let a0 = parse_area_weather(&text, "ha_area00").expect("area 00 row");
        assert_eq!(a0.weights[0], (2, 100), "Jubilife is always sunny");
        let Some(lot) = find_sheet("data/weather/weather_lottery_matrix.tsv") else {
            eprintln!("[skip] weather_lottery_matrix.tsv not found");
            return;
        };
        let text = std::fs::read_to_string(&lot).unwrap();
        assert_eq!(parse_global_lottery(&text).unwrap(), global_default_weights());
    }
}
