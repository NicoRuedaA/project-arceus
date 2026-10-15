//! Walking off a ledge onto lower ground (play-test 2026-10-14).
//!
//! The decoded navmesh has no faces on a cliff: an upper ridge and the field below it are
//! separate walkable regions with an unwalkable strip between them. The viewer's navmesh
//! slide therefore stopped the player at every edge, while the game lets the player drop down
//! (fall + land, Brief 13). This module decides when stepping off the walkable surface is a
//! legal drop and keeps the player inside a short "drop corridor" until it lands on the lower
//! navmesh again; the vertical fall itself is `game::player_motion`.
//!
//! Rules (all SUPUESTO, the game's own ledge rule is not decoded):
//! - the lower walkable surface must be found within [`LEDGE_PROBE_DISTANCE`] metres ahead;
//! - the drop must be more than a step (`STEP_HEIGHT_MAX`, decoded) and at most
//!   [`LEDGE_MAX_DROP`]: 6 m, just under the first decoded fall-penalty tier (7 m), so every
//!   drop the port allows is damage-free; higher cliffs stay blocked;
//! - the terrain between the edge and the landing may never rise above the edge (a wall or a
//!   hump is not a ledge) and the landing must not be water deep enough to swim (water keeps
//!   its own entry rule) nor off the navmesh (out of bounds).

use bevy::math::Vec2;

use super::player_motion::STEP_HEIGHT_MAX;

/// Horizontal distance searched ahead of the edge for the lower walkable surface (m).
pub const LEDGE_PROBE_DISTANCE: f32 = 4.0;
/// Highest drop the player may walk off (m). SUPUESTO, below the 7 m penalty tier.
pub const LEDGE_MAX_DROP: f32 = 6.0;
/// Probe spacing along the move direction (m).
const PROBE_STEP: f32 = 0.25;
/// Half width of the drop corridor (m): how far the player may stray from the edge-landing line.
pub const CORRIDOR_HALF_WIDTH: f32 = 1.0;
/// A drop that has not reached the lower navmesh after this long is abandoned (s).
pub const DROP_TIMEOUT: f32 = 3.0;

/// An approved drop from `start` (on the upper navmesh) to `landing` (on the lower one).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LedgeDrop {
    pub start: Vec2,
    pub start_y: f32,
    pub landing: Vec2,
    pub landing_y: f32,
    /// Seconds since the drop started.
    pub elapsed: f32,
}

impl LedgeDrop {
    pub fn drop_height(&self) -> f32 {
        self.start_y - self.landing_y
    }

    /// True when `p` lies in the corridor between the edge and the landing (plus the
    /// corridor half width around both ends).
    pub fn corridor_contains(&self, p: Vec2) -> bool {
        let ab = self.landing - self.start;
        let len2 = ab.length_squared();
        let t = if len2 > 0.0 { ((p - self.start).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
        (self.start + ab * t).distance(p) <= CORRIDOR_HALF_WIDTH
    }
}

/// Looks for a legal drop when a move from `start` (walkable, ground `start_y`) along `dir`
/// left the navmesh. `nav_y` is the walkable surface height (None off the navmesh),
/// `terrain_y` the heightmap and `is_water` true where the ground is under swimmable water.
pub fn find_ledge_drop(
    start: Vec2,
    start_y: f32,
    dir: Vec2,
    nav_y: impl Fn(Vec2) -> Option<f32>,
    terrain_y: impl Fn(Vec2) -> Option<f32>,
    is_water: impl Fn(Vec2) -> bool,
) -> Option<LedgeDrop> {
    let dir = dir.try_normalize()?;
    let mut d = PROBE_STEP;
    while d <= LEDGE_PROBE_DISTANCE + 1e-4 {
        let p = start + dir * d;
        // A wall or a hump between the edge and the landing: not a ledge.
        if terrain_y(p).is_some_and(|y| y > start_y + STEP_HEIGHT_MAX) {
            return None;
        }
        if let Some(y) = nav_y(p) {
            let drop = start_y - y;
            if drop <= STEP_HEIGHT_MAX {
                // Walkable ground at (about) the same level: an ordinary gap, not a ledge.
                return None;
            }
            if drop > LEDGE_MAX_DROP || is_water(p) {
                return None;
            }
            // Land a little past the edge of the lower surface, still on it when possible.
            let inner = start + dir * (d + 0.5);
            let landing = if nav_y(inner).is_some_and(|yy| (yy - y).abs() < STEP_HEIGHT_MAX) {
                inner
            } else {
                p
            };
            return Some(LedgeDrop {
                start,
                start_y,
                landing,
                landing_y: nav_y(landing).unwrap_or(y),
                elapsed: 0.0,
            });
        }
        d += PROBE_STEP;
    }
    None
}

/// Horizontal move while a drop is in progress: the full `target` when it stays in the
/// corridor (or is walkable), otherwise the player stays at `current`.
pub fn drop_move(drop: &LedgeDrop, current: Vec2, target: Vec2, walkable: impl Fn(Vec2) -> bool) -> Vec2 {
    if walkable(target) || drop.corridor_contains(target) {
        target
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Upper shelf (y = 10) for x < 0, an unwalkable cliff strip 0 <= x < 1.5 whose terrain
    /// falls linearly, lower field (y = 10 - drop) for x >= 1.5.
    fn cliff(drop: f32) -> (impl Fn(Vec2) -> Option<f32>, impl Fn(Vec2) -> Option<f32>) {
        let nav = move |p: Vec2| {
            if p.x < 0.0 {
                Some(10.0)
            } else if p.x >= 1.5 {
                Some(10.0 - drop)
            } else {
                None
            }
        };
        let terrain = move |p: Vec2| Some(if p.x < 0.0 { 10.0 } else if p.x >= 1.5 { 10.0 - drop } else { 10.0 - drop * p.x / 1.5 });
        (nav, terrain)
    }

    #[test]
    fn a_two_metre_ledge_is_a_legal_drop() {
        let (nav, terrain) = cliff(2.0);
        let d = find_ledge_drop(Vec2::new(-0.1, 0.0), 10.0, Vec2::X, nav, terrain, |_| false).expect("drop");
        assert!((d.drop_height() - 2.0).abs() < 1e-4);
        assert!(d.landing.x >= 1.5);
        assert!(d.corridor_contains(Vec2::new(0.7, 0.3)));
        assert!(!d.corridor_contains(Vec2::new(0.7, 3.0)));
    }

    #[test]
    fn too_high_walls_water_and_out_of_bounds_stay_blocked() {
        let (nav, terrain) = cliff(20.0);
        assert!(find_ledge_drop(Vec2::new(-0.1, 0.0), 10.0, Vec2::X, nav, terrain, |_| false).is_none(), "20 m");
        let (nav, terrain) = cliff(2.0);
        assert!(find_ledge_drop(Vec2::new(-0.1, 0.0), 10.0, Vec2::X, &nav, &terrain, |_| true).is_none(), "water");
        // A wall: the terrain rises past the edge.
        let wall = |p: Vec2| Some(if p.x < 0.0 { 10.0 } else { 15.0 });
        assert!(find_ledge_drop(Vec2::new(-0.1, 0.0), 10.0, Vec2::X, &nav, wall, |_| false).is_none(), "wall");
        // Out of bounds: no navmesh ahead at all.
        let none = |_: Vec2| None;
        assert!(find_ledge_drop(Vec2::new(-0.1, 0.0), 10.0, Vec2::X, none, &terrain, |_| false).is_none(), "oob");
        // A step-sized gap is not a ledge.
        let (nav, terrain) = cliff(0.2);
        assert!(find_ledge_drop(Vec2::new(-0.1, 0.0), 10.0, Vec2::X, nav, terrain, |_| false).is_none(), "step");
    }

    #[test]
    fn drop_moves_stay_in_the_corridor() {
        let (nav, terrain) = cliff(2.0);
        let d = find_ledge_drop(Vec2::new(-0.1, 0.0), 10.0, Vec2::X, &nav, terrain, |_| false).unwrap();
        let walk = |p: Vec2| nav(p).is_some();
        assert_eq!(drop_move(&d, Vec2::new(0.5, 0.0), Vec2::new(0.6, 0.0), walk), Vec2::new(0.6, 0.0));
        assert_eq!(drop_move(&d, Vec2::new(0.5, 0.0), Vec2::new(0.5, 2.0), walk), Vec2::new(0.5, 0.0));
    }

    /// Real area 01 data: near the start there are ledges of 1.5-6 m that are now legal drops,
    /// and walking off one (horizontal corridor + the Brief 13 fall) lands on the lower navmesh.
    /// Higher edges stay blocked. Skipped without the extracted game files.
    #[test]
    fn real_area01_ledges_near_the_start_can_be_walked_down() {
        use crate::assets::navmesh::AreaNavigation;
        use crate::assets::terrain::TerrainArea;
        use crate::game::player_motion::{air_step, is_ledge_drop, should_land};
        let root = std::env::var_os("PLA_BIN_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into());
        if !root.join("archive/field/terrain/ha_area01.gfpak").exists() {
            eprintln!("[skip] game data not present");
            return;
        }
        let nav = AreaNavigation::load(&root, 1).expect("navmesh");
        let terrain = TerrainArea::load(&root, 1).expect("terrain");
        let nav_y = |p: Vec2| nav.height_at(p.x, p.y);
        let ter_y = |p: Vec2| terrain.sample_world_height(p.x, p.y);

        // Edges within 120 m of the start area used in the play-test (x 511, z 409).
        let centre = Vec2::new(511.0, 409.0);
        let mut legal = Vec::new();
        let mut too_high = 0usize;
        let mut z = centre.y - 120.0;
        while z <= centre.y + 120.0 {
            let mut x = centre.x - 120.0;
            while x <= centre.x + 120.0 {
                let p = Vec2::new(x, z);
                if let Some(y) = nav_y(p) {
                    for k in 0..8 {
                        let a = k as f32 * std::f32::consts::FRAC_PI_4;
                        let dir = Vec2::new(a.cos(), a.sin());
                        if nav_y(p + dir * 0.5).is_some() {
                            continue;
                        }
                        // Edge: classify what lies below.
                        let far = (1..=16).find_map(|i| nav_y(p + dir * (i as f32 * 0.25)).map(|yy| y - yy));
                        match find_ledge_drop(p, y, dir, nav_y, ter_y, |_| false) {
                            Some(d) => legal.push(d),
                            None => {
                                if far.is_some_and(|h| h > LEDGE_MAX_DROP) {
                                    too_high += 1;
                                }
                            }
                        }
                    }
                }
                x += 2.0;
            }
            z += 2.0;
        }
        let tall: Vec<&LedgeDrop> = legal.iter().filter(|d| d.drop_height() >= 1.5).collect();
        println!(
            "area01 near start: {} legal drops ({} of 1.5 m or more), {} edges higher than {LEDGE_MAX_DROP} m stay blocked",
            legal.len(),
            tall.len(),
            too_high
        );
        assert!(!tall.is_empty(), "no 1.5 m+ ledge found near the start");
        let d = *tall[0];
        println!(
            "example: from ({:.1}, {:.1}) y {:.2} to ({:.1}, {:.1}) y {:.2}, drop {:.2} m",
            d.start.x, d.start.y, d.start_y, d.landing.x, d.landing.y, d.landing_y, d.drop_height()
        );

        // Simulate the viewer: walk toward the landing at 4.5 m/s, grounded snapping with the
        // ledge rule, airborne integration from player_motion, 60 fps.
        let ground = |p: Vec2| nav_y(p).or_else(|| ter_y(p));
        let dir = (d.landing - d.start).normalize();
        let dt = 1.0 / 60.0;
        let mut pos = d.start;
        let mut root_y = d.start_y + 0.9;
        let mut airborne = false;
        let mut vy = 0.0;
        let mut landed_on_lower = false;
        for _ in 0..600 {
            let speed = if airborne { crate::game::player_motion::air_speed(4.5) } else { 4.5 };
            let target = pos + dir * speed * dt;
            pos = drop_move(&d, pos, target, |p| nav_y(p).is_some());
            let g = ground(pos).expect("ground") + 0.9;
            if airborne {
                let (v, dy) = air_step(vy, dt);
                vy = v;
                root_y += dy;
                if should_land(root_y, g, vy) {
                    root_y = g;
                    airborne = false;
                }
            } else if is_ledge_drop(root_y, g) {
                airborne = true;
                vy = 0.0;
            } else {
                root_y = g;
            }
            if !airborne && nav_y(pos).is_some_and(|y| (y - d.landing_y).abs() < 1.0) {
                landed_on_lower = true;
                break;
            }
        }
        assert!(landed_on_lower, "never reached the lower navmesh (pos {pos:?}, y {root_y})");
    }
}
