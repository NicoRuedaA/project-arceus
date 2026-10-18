//! Terrain collision: the port's own "can I walk here" rule (user decision 2026-10-17).
//!
//! Movement and placement no longer consult the decoded navmesh (`hkaiNavMesh`): a spot is
//! walkable when
//!
//! * the ground is known: the terrain heightmap ([`TerrainResource`]) at (x, z), or — where
//!   there is no samplable terrain (interiors ship a non-samplable placeholder) — a floor of
//!   the placed-object collision world ([`WorldCollision`]) within probe reach of the
//!   reference feet (room shells, decks);
//! * the local terrain slope is under [`MAX_WALK_SLOPE_DEG`] (SUPUESTO, the game's own limit
//!   is not decoded) *or* the spot does not rise above a walkable step from the reference
//!   (walking off a drop is always allowed: `super::player_motion` handles the fall and the
//!   decoded fall damage);
//! * it is not deep water while on foot (entering water stays with `super::water`; a mount
//!   crosses it);
//! * the walker's capsule does not collide with placed-object walls.
//!
//! The decoded navmesh stays in the tree for the interior minimap (`super::minimap`) and the
//! report examples only; no movement or placement code reads it anymore.

use bevy::prelude::*;

use super::player_motion::STEP_HEIGHT_MAX;
use super::water::{WaterBodies, SWIM_ENTER_DEPTH};
use super::world_collision::{WorldCollision, PLAYER_HEIGHT, PLAYER_RADIUS};
use super::zone::TerrainResource;

// ------------------------------------------------------------------ tuning

/// Maximum walkable terrain slope (degrees, SUPUESTO — the game's own slope limit is not
/// decoded; the port's value is checked against the decoded navmesh coverage in
/// `reports/terreno-colision/terrain-walk-vs-navmesh.md`).
pub const MAX_WALK_SLOPE_DEG: f32 = 45.0;

/// Distance of the finite-difference slope samples (m).
pub const SLOPE_SAMPLE_STEP: f32 = 1.5;

/// The collider ground probe starts this high above the reference feet (m): a floor within a
/// step of the feet still counts as the ground under them.
pub const GROUND_PROBE_UP: f32 = 0.5;

/// The collider ground probe looks this far down from its start (m): a room floor a few
/// metres under a wrong placement height is still found.
pub const GROUND_PROBE_DOWN: f32 = 5.0;

/// A capsule push-out longer than this marks the spot blocked (m).
pub const COLLIDER_BLOCK_EPS: f32 = 0.01;

// ------------------------------------------------------------------ pure helpers

/// Terrain slope at (x, z) in degrees (0 = flat), from central differences of the heightmap.
/// `None` without samplable terrain data.
pub fn terrain_slope_deg(terrain: Option<&TerrainResource>, x: f32, z: f32) -> Option<f32> {
    let area = &terrain?.area;
    let s = SLOPE_SAMPLE_STEP;
    let hx = area.sample_world_height(x + s, z)? - area.sample_world_height(x - s, z)?;
    let hz = area.sample_world_height(x, z + s)? - area.sample_world_height(x, z - s)?;
    let grad = Vec2::new(hx, hz).length() / (2.0 * s);
    Some(grad.atan().to_degrees())
}

/// Ground height at (x, z): the terrain heightmap first; where it is not samplable (interiors
/// ship a placeholder), the highest placed-object floor within probe reach of `from_y`.
///
/// `from_y` is the reference feet height (avatar root minus its own offset, placement height):
/// the collider probe starts [`GROUND_PROBE_UP`] above it and looks [`GROUND_PROBE_DOWN`] down.
pub fn ground_y(
    terrain: Option<&TerrainResource>,
    collision: Option<&WorldCollision>,
    x: f32,
    z: f32,
    from_y: f32,
) -> Option<f32> {
    let terrain_y = terrain.and_then(|t| t.area.sample_world_height(x, z));
    let floor_y = collision.and_then(|c| {
        let start = from_y + GROUND_PROBE_UP;
        c.raycast(
            Vec3::new(x, start, z),
            Vec3::NEG_Y,
            GROUND_PROBE_UP + GROUND_PROBE_DOWN,
        )
        .map(|d| start - d)
    });
    match (terrain_y, floor_y) {
        (Some(t), Some(f)) => Some(t.max(f)),
        (Some(t), None) => Some(t),
        (None, Some(f)) => Some(f),
        (None, None) => None,
    }
}

// ------------------------------------------------------------------ walk rule

/// Everything the walk rule reads. `Copy`: cheap to capture in a closure.
#[derive(Clone, Copy)]
pub struct WalkContext<'a> {
    pub terrain: Option<&'a TerrainResource>,
    pub collision: Option<&'a WorldCollision>,
    pub water: Option<&'a WaterBodies>,
    /// True while a mount is ridden: deep water is passable (Basculegion swims; on foot the
    /// water system still lets a swimmer in, this only gates the walk rule).
    pub mounted: bool,
    /// Feet-level reference of the walker (m): the ground probe and the step rule pivot here.
    pub from_y: f32,
    /// Capsule used for the placed-object block test.
    pub capsule_radius: f32,
    pub capsule_height: f32,
}

impl<'a> WalkContext<'a> {
    /// Context for the player capsule (`game::world_collision` constants).
    pub fn player(
        terrain: Option<&'a TerrainResource>,
        collision: Option<&'a WorldCollision>,
        water: Option<&'a WaterBodies>,
        mounted: bool,
        from_y: f32,
    ) -> Self {
        Self {
            terrain,
            collision,
            water,
            mounted,
            from_y,
            capsule_radius: PLAYER_RADIUS,
            capsule_height: PLAYER_HEIGHT,
        }
    }

    /// [`ground_y`] for this walker.
    pub fn ground_y(&self, x: f32, z: f32) -> Option<f32> {
        ground_y(self.terrain, self.collision, x, z, self.from_y)
    }

    /// [`terrain_slope_deg`] for this walker.
    pub fn slope_deg(&self, x: f32, z: f32) -> Option<f32> {
        terrain_slope_deg(self.terrain, x, z)
    }

    /// True when the water surface over (x, z) stands more than [`SWIM_ENTER_DEPTH`] above the
    /// ground: deep water (swimming depth).
    pub fn deep_water(&self, x: f32, z: f32, ground: f32) -> bool {
        self.water
            .and_then(|w| w.level_at(Vec2::new(x, z)))
            .is_some_and(|level| level - ground > SWIM_ENTER_DEPTH)
    }

    /// True when the walker's capsule at (x, z), feet on `ground`, overlaps a wall: the probe
    /// is pushed out and any horizontal displacement marks the spot blocked.
    pub fn blocked_by_colliders(&self, x: f32, z: f32, ground: f32) -> bool {
        let Some(c) = self.collision else {
            return false;
        };
        let center = Vec3::new(x, ground + self.capsule_height * 0.5, z);
        let out = c.push_out_capsule(center, self.capsule_radius, self.capsule_height);
        Vec2::new(out.x - x, out.z - z).length() > COLLIDER_BLOCK_EPS
    }

    /// Map extent of samplable terrain (placeholder interiors expose a 1x1 map and no extent).
    pub fn terrain_extent(&self) -> Option<[f32; 2]> {
        let area = &self.terrain?.area;
        (area.heightmap.width >= 2
            && area.heightmap.height >= 2
            && area.world_extent[0] > 0.0
            && area.world_extent[1] > 0.0)
            .then_some(area.world_extent)
    }

    /// The one rule.
    pub fn is_walkable(&self, x: f32, z: f32) -> bool {
        // No ground source at all (no extracted terrain, no collision world: bare tests,
        // fallback worlds): everything is walkable, exactly like the old "no navmesh" fallback.
        if self.terrain.is_none() && self.collision.is_none() {
            return true;
        }
        // Off the mapped terrain: no walking past the map edge (the sampler clamps, so the
        // rule has to bound it).
        if let Some(extent) = self.terrain_extent() {
            if x < 0.0 || z < 0.0 || x > extent[0] || z > extent[1] {
                return false;
            }
        }
        let Some(ground) = self.ground_y(x, z) else {
            return false; // no terrain and no collider floor: nothing to walk on
        };
        if let Some(slope) = self.slope_deg(x, z) {
            // A steep spot blocks only when it is a wall ahead (rises above a walkable step):
            // steep ground at or below the feet is a drop, and the fall is handled by gravity.
            if slope > MAX_WALK_SLOPE_DEG && ground > self.from_y + STEP_HEIGHT_MAX {
                return false;
            }
        }
        if !self.mounted && self.deep_water(x, z, ground) {
            return false;
        }
        if self.blocked_by_colliders(x, z, ground) {
            return false;
        }
        true
    }
}

// ------------------------------------------------------------------ nearest walkable

/// Nearest walkable point to (x, z) within `max_dist` metres: `Some((point, distance))`.
/// `None` when the point itself is walkable (`distance` 0 is reported as the point itself) or
/// nothing walkable is in range. Ring search: fine steps first (0.5 m to 4 m), then 1 m to
/// 16 m, then 4 m, 16 directions per ring.
pub fn nearest_walkable(ctx: &WalkContext, x: f32, z: f32, max_dist: f32) -> Option<(Vec2, f32)> {
    let p = Vec2::new(x, z);
    if ctx.is_walkable(x, z) {
        return Some((p, 0.0));
    }
    let mut ring = 0.5_f32;
    while ring <= max_dist {
        for k in 0..16 {
            let a = k as f32 * std::f32::consts::TAU / 16.0;
            let q = p + Vec2::new(a.cos(), a.sin()) * ring;
            if ctx.is_walkable(q.x, q.y) {
                return Some((q, ring));
            }
        }
        ring = if ring < 4.0 {
            ring + 0.5
        } else if ring < 16.0 {
            ring + 1.0
        } else {
            ring + 4.0
        };
    }
    None
}

/// Search grid step of the safe-start / landing scans (m).
pub const SCAN_STEP: f32 = 2.0;

/// The first walkable point of a square spiral around `anchor`, every [`SCAN_STEP`] metres,
/// out to `max_dist`. Deterministic (ring order, first angle wins).
pub fn walkable_near(ctx: &WalkContext, anchor: Vec2, max_dist: f32) -> Option<Vec2> {
    if ctx.is_walkable(anchor.x, anchor.y) {
        return Some(anchor);
    }
    let steps = (max_dist / SCAN_STEP).ceil() as i32;
    for ring in 1..=steps {
        let r = ring as f32 * SCAN_STEP;
        let n = 8 * ring;
        for k in 0..n {
            let a = k as f32 * std::f32::consts::TAU / n as f32;
            let q = anchor + Vec2::new(a.cos(), a.sin()) * r;
            if ctx.is_walkable(q.x, q.y) {
                return Some(q);
            }
        }
    }
    None
}

// ------------------------------------------------------------------ tests

/// Synthetic `size` x `size` terrain over `size - 1` metres with a height function in metres
/// (test helper shared by other modules' tests).
///
/// Heightmap samples are normalized like the decoded ones: `sample_world_height` multiplies
/// them by `world_height`, so the helper stores `height / max` and `world_height = max`.
#[cfg(test)]
pub fn test_terrain(size: u32, height: impl Fn(f32, f32) -> f32) -> TerrainResource {
    use crate::assets::bntx::{FloatTextureData, TextureData};
    use crate::assets::terrain::{TerrainArea, TerrainMeshSettings};
    use std::sync::Arc;

    let mut raw = Vec::new();
    let mut max_abs = 0.0f32;
    for zi in 0..size {
        for xi in 0..size {
            let h = height(xi as f32, zi as f32);
            max_abs = max_abs.max(h.abs());
            raw.push(h);
        }
    }
    let scale = max_abs.max(1.0);
    let samples: Vec<f32> = raw.iter().map(|h| h / scale).collect();
    let area = TerrainArea {
        area: 1,
        source_path: std::path::PathBuf::new(),
        world_extent: [(size - 1) as f32, (size - 1) as f32],
        world_height: scale,
        heightmap_brti_name: "h".into(),
        navmesh_bounds: None,
        heightmap: FloatTextureData {
            width: size,
            height: size,
            samples,
        },
        normal_map: TextureData {
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
        },
        splat_alpha: TextureData {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
        },
        splat_alpha1: None,
        material: None,
        ground_textures: Default::default(),
        unresolved_ground_textures: vec![],
        masks: vec![],
        instances: vec![],
    };
    TerrainResource {
        area: Arc::new(area),
        chunks: vec![],
        blended_albedo: None,
        settings: TerrainMeshSettings {
            horizontal_cell_size: 1.0,
            vertical_scale: 1.0,
            chunk_cells: 4,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smooth_terrain(size: u32, height: impl Fn(f32, f32) -> f32) -> TerrainResource {
        test_terrain(size, height)
    }

    fn flat() -> TerrainResource {
        smooth_terrain(33, |_, _| 10.0)
    }

    /// Axis-aligned water sheet over the rectangle with the given surface level.
    fn water_rect(x0: f32, z0: f32, x1: f32, z1: f32, level: f32) -> crate::game::water::WaterBody {
        let (a, b, c, d) = (
            Vec2::new(x0, z0),
            Vec2::new(x1, z0),
            Vec2::new(x1, z1),
            Vec2::new(x0, z1),
        );
        crate::game::water::WaterBody {
            min: Vec2::new(x0, z0),
            max: Vec2::new(x1, z1),
            level,
            triangles: vec![[a, b, c], [a, c, d]],
        }
    }

    #[test]
    fn flat_ground_is_walkable_and_its_height_is_returned() {
        let t = flat();
        let ctx = WalkContext::player(Some(&t), None, None, false, 10.0);
        assert!(ctx.is_walkable(5.0, 5.0));
        assert!((ctx.ground_y(5.0, 5.0).unwrap() - 10.0).abs() < 1e-4);
        assert!(ctx.slope_deg(5.0, 5.0).unwrap() < 0.01);
    }

    #[test]
    fn a_gentle_ramp_is_walkable_and_a_wall_is_not() {
        // 30 deg ramp in +x for x >= 16, flat elsewhere.
        let ramp = smooth_terrain(33, |x, _| {
            if x <= 16.0 {
                0.0
            } else {
                (x - 16.0) * 0.577 // tan(30 deg)
            }
        });
        let ctx = WalkContext::player(Some(&ramp), None, None, false, 0.0);
        assert!(ctx.is_walkable(20.0, 10.0), "30 deg slope is walkable");

        // 60 deg step-face: rises > 1 step within the slope window -> wall. The probe sits at
        // the face (the step is one cell wide at x = 16): 1.5 m in, the window no longer
        // straddles it and the point is the plateau top.
        let wall = smooth_terrain(33, |x, _| if x < 16.0 { 0.0 } else { 20.0 });
        let ctx = WalkContext::player(Some(&wall), None, None, false, 0.0);
        assert!(
            !ctx.is_walkable(16.5, 10.0),
            "a 20 m step face rising over the feet is blocked"
        );
    }

    #[test]
    fn a_drop_is_not_blocked_by_the_slope_rule() {
        // Cliff at x = 16: 20 m down. Standing on the top, the far side is a drop.
        let cliff = smooth_terrain(33, |x, _| if x < 16.0 { 20.0 } else { 0.0 });
        let ctx = WalkContext::player(Some(&cliff), None, None, false, 20.0);
        assert!(
            ctx.is_walkable(17.5, 10.0),
            "the far side of the edge is a drop, not a wall: gravity handles it"
        );
        // From below, the same face is a wall.
        let ctx = WalkContext::player(Some(&cliff), None, None, false, 0.0);
        assert!(!ctx.is_walkable(15.5, 10.0), "climbing the cliff face is blocked");
    }

    #[test]
    fn a_low_ridge_within_the_step_height_is_walkable() {
        // A 0.3 m bump over one cell (within the decoded 0.4 m step).
        let ridge = smooth_terrain(33, |x, _| if (x - 16.0).abs() < 0.5 { 0.3 } else { 0.0 });
        let ctx = WalkContext::player(Some(&ridge), None, None, false, 0.0);
        assert!(ctx.is_walkable(16.0, 10.0), "a step-height ridge passes");
    }

    #[test]
    fn deep_water_blocks_on_foot_and_passes_mounted() {
        let t = smooth_terrain(33, |_, _| 0.0);
        let mut water = WaterBodies::default();
        // 4 m above the bed: deep.
        water.insert(water_rect(0.0, 0.0, 32.0, 32.0, 4.0));
        let on_foot = WalkContext::player(Some(&t), None, Some(&water), false, 0.0);
        assert!(!on_foot.is_walkable(10.0, 10.0), "deep water blocks on foot");
        let mounted = WalkContext::player(Some(&t), None, Some(&water), true, 0.0);
        assert!(mounted.is_walkable(10.0, 10.0), "a mount crosses deep water");
        // A shallow sheet (0.4 m) is wadeable.
        let mut shallow = WaterBodies::default();
        shallow.insert(water_rect(0.0, 0.0, 32.0, 32.0, 0.4));
        let wade = WalkContext::player(Some(&t), None, Some(&shallow), false, 0.0);
        assert!(wade.is_walkable(10.0, 10.0));
    }

    #[test]
    fn off_the_map_is_not_walkable() {
        let t = flat();
        let ctx = WalkContext::player(Some(&t), None, None, false, 10.0);
        // flat() is 33 samples over 32 m: the edge is walkable, beyond it is not.
        assert!(ctx.is_walkable(31.5, 10.0));
        assert!(!ctx.is_walkable(32.5, 10.0), "beyond the +x edge");
        assert!(!ctx.is_walkable(-0.5, 10.0), "before the -x edge");
        assert!(!ctx.is_walkable(10.0, 40.0), "beyond the +z edge");
    }

    #[test]
    fn without_any_ground_source_everything_is_walkable() {
        let ctx = WalkContext::player(None, None, None, false, 0.0);
        assert!(ctx.is_walkable(1234.0, -99.0), "no data: free movement fallback");
        assert!(ctx.ground_y(0.0, 0.0).is_none());
    }

    #[test]
    fn nearest_walkable_finds_the_island_and_reports_distance() {
        // Deep water everywhere except a raised plateau (the "island") around (20, 20).
        let island = smooth_terrain(33, |x, z| {
            let d = ((x - 20.0).powi(2) + (z - 20.0).powi(2)).sqrt();
            if d <= 3.0 {
                10.0
            } else {
                0.0
            }
        });
        let mut island_water = WaterBodies::default();
        island_water.insert(water_rect(0.0, 0.0, 32.0, 32.0, 4.0));
        // The island is a 10 m tower: from its own height the top is walkable; the water
        // around it is blocked on foot.
        let ctx = WalkContext::player(Some(&island), None, Some(&island_water), false, 10.0);
        assert!(ctx.is_walkable(20.0, 20.0));
        let (p, d) = nearest_walkable(&ctx, 0.0, 0.0, 40.0).expect("island found");
        assert!(d > 15.0, "the island is far: {d}");
        assert!(p.distance(Vec2::new(20.0, 20.0)) < 4.0, "{p:?}");
        // Nothing walkable: deep water everywhere and no island.
        let flat = smooth_terrain(33, |_, _| 0.0);
        let mut water = WaterBodies::default();
        water.insert(water_rect(0.0, 0.0, 32.0, 32.0, 4.0));
        let ctx = WalkContext::player(Some(&flat), None, Some(&water), false, 0.0);
        assert!(nearest_walkable(&ctx, 10.0, 10.0, 16.0).is_none());
    }

    #[test]
    fn walkable_near_scans_outward_from_the_anchor() {
        let t = smooth_terrain(65, |x, z| {
            let d = ((x - 32.0).powi(2) + (z - 32.0).powi(2)).sqrt();
            if d <= 8.0 {
                10.0
            } else {
                0.0
            }
        });
        let mut water = WaterBodies::default();
        water.insert(water_rect(0.0, 0.0, 64.0, 64.0, 4.0));
        // On foot: the anchor is deep water, so the scan has to run outward to the island
        // (a mount crosses deep water and the anchor itself would be walkable).
        let ctx = WalkContext::player(Some(&t), None, Some(&water), false, 10.0);
        let p = walkable_near(&ctx, Vec2::new(0.0, 0.0), 40.0).expect("the island");
        assert!(p.distance(Vec2::new(32.0, 32.0)) < 24.5, "{p:?}");
    }
}
