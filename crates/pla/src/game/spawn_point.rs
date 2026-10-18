//! Safe start position and anti-stuck helpers (pure logic, no window, no Bevy ECS).
//!
//! Problem: a fixed start (e.g. the terrain centre) can land on a steep slope or a pocket
//! where sliding collision traps the player.
//!
//! Method (user decision 2026-10-17): the start is chosen with the port's own terrain rule
//! (`game::terrain_collision`), not the decoded navmesh: a point of gentle slope with
//! walkable clearance around it, as close as possible to a landmark (village objects /
//! first spawners) or, failing that, to an area-wide scan. The navmesh is not consulted
//! anymore (report-only helpers live in `game::nav_report`).

use bevy::math::Vec2;
use bevy::prelude::Resource;

use super::terrain_collision::{nearest_walkable, WalkContext};

/// Tunables of the safe-start search.
#[derive(Debug, Clone, Copy)]
pub struct SpawnConfig {
    /// Maximum slope of the start point (degrees).
    pub max_slope_deg: f32,
    /// Required walkable radius around the start (metres).
    pub clearance: f32,
    /// Maximum height difference between the start and any clearance sample (metres).
    pub max_clearance_dy: f32,
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self { max_slope_deg: 15.0, clearance: 10.0, max_clearance_dy: 5.0 }
    }
}

/// The chosen safe start: a walkable point with clearance, on the terrain rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SafeSpawn {
    pub xz: Vec2,
    /// Ground height at `xz` (terrain or collider floor).
    pub y: f32,
}

/// The safe start of the current area (teleport target of the pause menu and the stuck escape).
#[derive(Resource, Debug, Clone, Copy)]
pub struct SafeSpawnPoint(pub Vec2);

/// Outcome of the start-position decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StartChoice {
    /// `--player-pos`: honoured as given.
    Explicit(Vec2),
    /// Saved position that is walkable.
    Saved(Vec2),
    /// No requested position: the safe start.
    Safe(Vec2),
    /// The saved position was not walkable: replaced by the safe start.
    SafeReplacingSaved(Vec2),
    /// No safe start could be computed: the caller keeps its old behaviour.
    NoSafe,
}

/// Every clearance ring sample around (x, z) is walkable and within `max_clearance_dy` of `y`:
/// five rings up to `clearance` metres, 16 directions each.
pub fn clearance_ok(ctx: &WalkContext, x: f32, z: f32, y: f32, cfg: &SpawnConfig) -> bool {
    for ring in [0.2, 0.4, 0.6, 0.8, 1.0] {
        let r = cfg.clearance * ring;
        for k in 0..16 {
            let a = k as f32 * std::f32::consts::TAU / 16.0;
            let (px, pz) = (x + r * a.cos(), z + r * a.sin());
            if !ctx.is_walkable(px, pz) {
                return false;
            }
            match ctx.ground_y(px, pz) {
                Some(py) if (py - y).abs() <= cfg.max_clearance_dy => {}
                _ => return false,
            }
        }
    }
    true
}

/// A candidate start: gentle enough slope and full clearance.
fn start_candidate(ctx: &WalkContext, x: f32, z: f32, cfg: &SpawnConfig) -> Option<SafeSpawn> {
    let y = ctx.ground_y(x, z)?;
    if ctx.slope_deg(x, z).is_some_and(|s| s > cfg.max_slope_deg) {
        return None;
    }
    if !clearance_ok(ctx, x, z, y, cfg) {
        return None;
    }
    Some(SafeSpawn { xz: Vec2::new(x, z), y })
}

/// Pick the safe start around `landmarks` (preferred anchor points, x,z, in priority order).
///
/// The anchor is the average of the first 24 landmarks that have walkable ground within 15 m
/// (each moved onto that ground); with none, the terrain centre is the anchor. The first
/// candidate within [`SAFE_SPIRAL_MAX`] metres of the anchor in 2 m rings wins; without any
/// candidate there (or without terrain data), an 8 m scan of the whole heightmap follows.
pub const SAFE_SPIRAL_MAX: f32 = 200.0;

pub fn safe_start(ctx: &WalkContext, landmarks: &[Vec2], cfg: &SpawnConfig) -> Option<SafeSpawn> {
    let mut usable: Vec<Vec2> = Vec::new();
    for l in landmarks.iter().take(24) {
        if ctx.is_walkable(l.x, l.y) {
            usable.push(*l);
        } else if let Some((p, _)) = nearest_walkable(ctx, l.x, l.y, 15.0) {
            usable.push(p);
        }
    }
    let anchor = if usable.is_empty() {
        let extent = ctx.terrain.map(|t| t.area.world_extent)?;
        Vec2::new(extent[0] * 0.5, extent[1] * 0.5)
    } else {
        usable.iter().copied().sum::<Vec2>() / usable.len() as f32
    };
    if let Some(s) = spiral_start(ctx, anchor, SAFE_SPIRAL_MAX, cfg) {
        return Some(s);
    }
    scan_start(ctx, cfg)
}

/// First qualifying candidate in 2 m rings around `anchor` (8 * ring directions per ring).
fn spiral_start(ctx: &WalkContext, anchor: Vec2, max_dist: f32, cfg: &SpawnConfig) -> Option<SafeSpawn> {
    if let Some(s) = start_candidate(ctx, anchor.x, anchor.y, cfg) {
        return Some(s);
    }
    let steps = (max_dist / 2.0).ceil() as i32;
    for ring in 1..=steps {
        let r = ring as f32 * 2.0;
        let n = 8 * ring;
        for k in 0..n {
            let a = k as f32 * std::f32::consts::TAU / n as f32;
            let q = anchor + Vec2::new(a.cos(), a.sin()) * r;
            if let Some(s) = start_candidate(ctx, q.x, q.y, cfg) {
                return Some(s);
            }
        }
    }
    None
}

/// Coarse 8 m scan of the terrain extent, first qualifying candidate in row order.
fn scan_start(ctx: &WalkContext, cfg: &SpawnConfig) -> Option<SafeSpawn> {
    const STEP: f32 = 8.0;
    let extent = ctx.terrain.map(|t| t.area.world_extent)?;
    let mut z = STEP;
    while z <= extent[1] {
        let mut x = STEP;
        while x <= extent[0] {
            if let Some(s) = start_candidate(ctx, x, z, cfg) {
                return Some(s);
            }
            x += STEP;
        }
        z += STEP;
    }
    None
}

/// Decide the start: explicit CLI position > valid saved position > safe start.
pub fn choose_start(
    ctx: &WalkContext,
    explicit: Option<Vec2>,
    saved: Option<Vec2>,
    safe: Option<SafeSpawn>,
) -> StartChoice {
    if let Some(p) = explicit {
        return StartChoice::Explicit(p);
    }
    if let Some(p) = saved {
        if ctx.is_walkable(p.x, p.y) {
            return StartChoice::Saved(p);
        }
        return safe.map_or(StartChoice::Saved(p), |s| StartChoice::SafeReplacingSaved(s.xz));
    }
    safe.map_or(StartChoice::NoSafe, |s| StartChoice::Safe(s.xz))
}

// ------------------------------------------------------------------ landmarks

/// Column index of `name` in the TSV header (`name:type...`).
fn col(header: &[&str], name: &str) -> Option<usize> {
    header.iter().position(|h| h.split(':').next() == Some(name))
}

/// Landmark anchors for an area from the two committed tables.
/// Area 0: the placed `objects_area00_house*` models (village centre); other areas, and area 0
/// without houses: the spawners of exactly `ha_areaNN`, in table order.
pub fn landmarks_for_area(area: u8, spawn_tsv: &str, placements_tsv: &str) -> Vec<Vec2> {
    let name = format!("ha_area{area:02}");
    let rows = |text: &str, key: &str, filter: &dyn Fn(&[&str], &[&str]) -> bool| -> Vec<Vec2> {
        let mut header: Option<Vec<&str>> = None;
        let mut out = Vec::new();
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
            let f: Vec<&str> = line.split('\t').collect();
            let Some(h) = &header else {
                header = Some(f);
                continue;
            };
            let (Some(ia), Some(ix), Some(iz)) = (col(h, key), col(h, "x"), col(h, "z")) else { return out };
            if f.get(ia) != Some(&name.as_str()) || !filter(h, &f) {
                continue;
            }
            if let (Some(x), Some(z)) = (f.get(ix).and_then(|v| v.parse().ok()), f.get(iz).and_then(|v| v.parse().ok())) {
                out.push(Vec2::new(x, z));
            }
        }
        out
    };
    if area == 0 {
        let houses = rows(placements_tsv, "area", &|h, f| {
            col(h, "model_name").and_then(|i| f.get(i)).is_some_and(|m| m.contains("house"))
        });
        if !houses.is_empty() {
            return houses;
        }
    }
    rows(spawn_tsv, "area", &|_, _| true)
}

// ------------------------------------------------------------------ anti-stuck

/// Collision step: the full move, then axis-projected slides, then the move rotated by
/// +-30/60/90 deg (so a path narrower than an axis-aligned step, e.g. a diagonal ridge trail,
/// still lets the player follow it). Returns `curr` when every candidate is blocked.
pub fn slide_move(curr: Vec2, delta: Vec2, walkable: impl Fn(Vec2) -> bool) -> Vec2 {
    let target = curr + delta;
    if walkable(target) {
        return target;
    }
    if delta.x.abs() > 1e-4 && walkable(Vec2::new(target.x, curr.y)) {
        return Vec2::new(target.x, curr.y);
    }
    if delta.y.abs() > 1e-4 && walkable(Vec2::new(curr.x, target.y)) {
        return Vec2::new(curr.x, target.y);
    }
    for deg in [30.0f32, -30.0, 60.0, -60.0, 90.0, -90.0] {
        let (s, c) = deg.to_radians().sin_cos();
        let cand = curr + Vec2::new(delta.x * c - delta.y * s, delta.x * s + delta.y * c);
        if walkable(cand) {
            return cand;
        }
    }
    curr
}

/// True when the player cannot step in any of 8 directions (`step` metres) from `curr`.
pub fn boxed_in(curr: Vec2, step: f32, walkable: impl Fn(Vec2) -> bool) -> bool {
    (0..8).all(|k| {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        !walkable(curr + Vec2::new(a.cos(), a.sin()) * step)
    })
}

/// Accumulates time spent boxed in while movement input is held.
#[derive(Debug, Default, Clone, Copy)]
pub struct StuckTracker {
    pub blocked_secs: f32,
}

impl StuckTracker {
    /// Returns true once the player has been input-held and boxed in for more than 1 s.
    pub fn update(&mut self, dt: f32, input_held: bool, boxed_in: bool) -> bool {
        if input_held && boxed_in {
            self.blocked_secs += dt;
        } else {
            self.blocked_secs = 0.0;
        }
        if self.blocked_secs > 1.0 {
            self.blocked_secs = 0.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::terrain_collision::test_terrain;

    fn flat() -> crate::game::zone::TerrainResource {
        test_terrain(129, |_, _| 0.0)
    }

    #[test]
    fn a_flat_landmark_is_the_start() {
        let t = flat();
        let ctx = WalkContext::player(Some(&t), None, None, false, 0.0);
        let s = safe_start(&ctx, &[Vec2::new(30.0, 30.0)], &SpawnConfig::default()).unwrap();
        assert!(s.xz.distance(Vec2::new(30.0, 30.0)) < 2.0, "{:?}", s.xz);
        assert!((s.y - 0.0).abs() < 1e-4);
    }

    #[test]
    fn a_landmark_on_a_steep_wall_moves_the_start_to_open_ground() {
        // A 20 m mesa for x in 40..=60; its rim cells are steep.
        let t = test_terrain(129, |x, _| if (40.0..=60.0).contains(&x) { 20.0 } else { 0.0 });
        let ctx = WalkContext::player(Some(&t), None, None, false, 0.0);
        // The landmark sits on the rim (transition cell): the search moves off it.
        let s = safe_start(&ctx, &[Vec2::new(40.0, 30.0)], &SpawnConfig::default()).unwrap();
        assert!(s.xz.x < 40.0, "found flat ground outside the mesa: {:?}", s.xz);
        assert!((s.y - 0.0).abs() < 1e-4);
    }

    #[test]
    fn saved_position_off_the_walkable_ground_is_replaced() {
        // Deep water everywhere except a raised plateau around (30, 30).
        use crate::game::water::WaterBody;
        let t = test_terrain(65, |x, z| {
            let d = ((x - 30.0).powi(2) + (z - 30.0).powi(2)).sqrt();
            if d <= 12.0 {
                1.0
            } else {
                0.0
            }
        });
        let mut water = crate::game::water::WaterBodies::default();
        water.insert(WaterBody {
            min: Vec2::ZERO,
            max: Vec2::new(64.0, 64.0),
            level: 4.0,
            triangles: vec![
                [Vec2::ZERO, Vec2::new(64.0, 0.0), Vec2::new(64.0, 64.0)],
                [Vec2::ZERO, Vec2::new(64.0, 64.0), Vec2::new(0.0, 64.0)],
            ],
        });
        let ctx = WalkContext::player(Some(&t), None, Some(&water), false, 1.0);
        let safe = safe_start(&ctx, &[Vec2::new(30.0, 30.0)], &SpawnConfig::default());
        assert!(safe.is_some(), "the island gives a start");
        let off = Vec2::new(2.0, 2.0); // deep water
        assert_eq!(
            choose_start(&ctx, None, Some(off), safe),
            StartChoice::SafeReplacingSaved(safe.unwrap().xz)
        );
        assert_eq!(
            choose_start(&ctx, None, Some(Vec2::new(30.0, 30.0)), safe),
            StartChoice::Saved(Vec2::new(30.0, 30.0))
        );
    }

    #[test]
    fn landmarks_parse_houses_for_area0_and_spawners_otherwise() {
        let spawn = "# c\narea:string*\tspawner_id:string*\tx:f32\ty:f32\tz:f32\nha_area01\ta\t1.0\t0\t2.0\nha_area01_s01\tb\t9\t0\t9\nha_area01\tc\t3.0\t0\t4.0\nha_area00\td\t7\t0\t8\n";
        let place = "area:string*\tmodel_name:string?\tx:f32\ty:f32\tz:f32\nha_area00\tobjects_area00_house01\t10\t0\t20\nha_area00\tobjects_area00_obj01\t99\t0\t99\nha_area00\tobjects_area00_house02\t30\t0\t40\n";
        assert_eq!(landmarks_for_area(0, spawn, place), vec![Vec2::new(10.0, 20.0), Vec2::new(30.0, 40.0)]);
        assert_eq!(landmarks_for_area(1, spawn, place), vec![Vec2::new(1.0, 2.0), Vec2::new(3.0, 4.0)]);
        assert_eq!(landmarks_for_area(0, spawn, "area:string*\tx:f32\tz:f32\n"), vec![Vec2::new(7.0, 8.0)]);
    }

    #[test]
    fn start_choice_priority() {
        let t = flat();
        let ctx = WalkContext::player(Some(&t), None, None, false, 0.0);
        let safe = safe_start(&ctx, &[], &SpawnConfig::default());
        let small = Vec2::new(10.5, 10.5);
        assert_eq!(choose_start(&ctx, Some(small), None, safe), StartChoice::Explicit(small));
        assert_eq!(choose_start(&ctx, None, Some(small), safe), StartChoice::Saved(small));
        assert_eq!(choose_start(&ctx, None, None, safe), StartChoice::Safe(safe.unwrap().xz));
        assert_eq!(choose_start(&ctx, None, None, None), StartChoice::NoSafe);
    }

    #[test]
    fn slide_follows_a_diagonal_trail_that_axis_slides_cannot() {
        // Walkable only within 0.2 m of the line x == z.
        let trail = |p: Vec2| (p.x - p.y).abs() < 0.2;
        let curr = Vec2::new(5.0, 5.0);
        // Heading mostly +x: full step and both axis projections leave the trail.
        let delta = Vec2::new(0.3, 0.0);
        assert!(!trail(curr + delta) && !trail(Vec2::new(curr.x + 0.3, curr.y)));
        let moved = slide_move(curr, delta, trail);
        assert!(trail(moved) && moved != curr, "{moved:?}");
        // Fully enclosed: stays.
        assert_eq!(slide_move(curr, delta, |_| false), curr);
        // Open ground: plain move.
        assert_eq!(slide_move(curr, delta, |_| true), curr + delta);
    }

    #[test]
    fn boxed_in_and_stuck_timer() {
        let only_here = |p: Vec2| p.length() < 0.05;
        assert!(boxed_in(Vec2::ZERO, 0.3, only_here));
        assert!(!boxed_in(Vec2::ZERO, 0.3, |p: Vec2| p.x > 0.2));
        let mut t = StuckTracker::default();
        assert!(!t.update(0.6, true, true));
        assert!(t.update(0.6, true, true)); // 1.2 s
        assert!(!t.update(0.6, true, true)); // reset after firing
        assert!(!t.update(0.6, false, true)); // input released resets
        assert!(!t.update(0.6, true, false)); // not boxed in resets
        assert!(!t.update(0.6, true, true));
    }
}
