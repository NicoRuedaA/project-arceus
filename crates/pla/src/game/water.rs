//! Water surfaces (the river planes placed in the zones) and swimming.
//!
//! Two halves that the brief (Brief 11) asked for in one crate module:
//!
//! * **Material**: [`WaterMaterial`] is an `ExtendedMaterial<StandardMaterial, WaterExt>` whose
//!   fragment shader (embedded `water.wgsl`) computes a procedural two-layer wave normal, a
//!   Schlick Fresnel and a shallow/deep colour ramp. The original game `Water` shader is NOT
//!   decoded, so every number in [`WaterUniform::default`] is ASSUMED, and the "sky reflection"
//!   is a tint, not a traced reflection (screen-space reflections need a render path we have not
//!   verified). There is no real water depth either.
//! * **Swimming**: [`WaterBodies`] is a flat list of horizontal water rectangles collected from
//!   the water placements the viewer streams in, [`step_swim`] is the Grounded/Swimming state
//!   machine with hysteresis, and [`PlayerSwim`] carries the state on the avatar entity.
//!
//! What is VERIFIED: the pure functions here have unit tests, and one windowless offscreen render
//! (`cargo run --example water_headless`) proves the pipeline and the WGSL compile and draws water
//! pixels. What is ASSUMED: colours, wave frequencies, speeds, the Fresnel f0, the depth ramp, the
//! swim speed and the vertical offsets. What is UNKNOWN: the real water depth of a river, the
//! game's own water shader parameters and whether the game's navmesh lets a walker reach the
//! water (see the report `reports/data-progress/water.md`).

use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::reflect::std_traits::ReflectDefault;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

/// Asset path of the embedded fragment shader (see the `embedded_asset!` call in [`WaterPlugin`]).
pub const EMBEDDED_SHADER_PATH: &str = "embedded://pla/game/water.wgsl";

/// Material binding of [`WaterUniform`]; StandardMaterial uses 0..~20, so 100 never collides.
pub const WATER_BINDING: u32 = 100;

// --- Swimming constants (all ASSUMED: the game's values are not decoded) -----------------------

/// Water deeper than this over the avatar's feet starts swimming. The brief says "chest below the
/// surface"; the chest is 0.8 x 1.5 m = 1.2 m above the feet (`player_anim::camera_offsets`).
pub const SWIM_ENTER_DEPTH: f32 = 1.2;
/// … and swimming stops once the water is shallower than this (hysteresis band with the entry).
pub const SWIM_EXIT_DEPTH: f32 = 0.9;
/// Swim speed (m/s); sprinting is not allowed in water.
pub const SWIM_SPEED: f32 = 2.4;
/// How far the avatar root sinks below the water surface while swimming (the model is 1.5 m tall
/// with the root 0.9 m above its feet).
pub const SWIM_ROOT_BELOW_SURFACE: f32 = 0.35;
/// How far a water surface may sit from the ground under it and still count as water.
///
/// The placed water meshes are FLAT sheets whose surface is authored at a fixed height, while the
/// terrain under them rises and falls. Measured with `water_report` (area 00): the Jubilife pond
/// fits its bed within 0.4 m, the long river plane is 49 m off on average (floating over the sea,
/// buried under the hills). A surface that far from the ground is not a surface you can enter, so
/// the swim rule only trusts water within this band. The value matches the ±3 m tolerance the
/// project's `placement_terrain_check` example uses for placements. ASSUMED.
pub const SWIM_MAX_GROUND_OFFSET: f32 = 3.0;

/// Is this a water plane model? The field index lists them as `ground_*water*` (and one
/// `wall_*_water*_asase`), all using the game's undecoded `Water` shader.
pub fn is_water_model(model_name: &str) -> bool {
    model_name.contains("water")
}

// --- Material ----------------------------------------------------------------------------------

/// GPU uniform of the water material. Keep the field order in sync with `WaterUniform` in
/// `water.wgsl` and only use `vec4` members so the std140 layout has no padding surprises.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq, Default)]
pub struct WaterUniform {
    pub deep_color: Vec4,
    pub shallow_color: Vec4,
    pub sky_color: Vec4,
    pub wave_a: Vec4,
    pub wave_b: Vec4,
    pub params: Vec4,
}

impl WaterUniform {
    /// ASSUMED colours and wave parameters: nothing here comes from the game data. Colours are
    /// linear RGB (the renderer's working space); alphas are the near/far transparency.
    pub fn assumed_default() -> Self {
        Self {
            deep_color: Vec4::new(0.020, 0.095, 0.170, 0.88),
            shallow_color: Vec4::new(0.105, 0.300, 0.360, 0.45),
            sky_color: Vec4::new(0.520, 0.700, 0.850, 1.0),
            // x: primary frequency (1/m), y: detail frequency, z: primary drift, w: detail drift.
            wave_a: Vec4::new(0.18, 0.90, 0.55, -0.35),
            // x: unused, y: slope strength, z: depth-fade rate (1/m), w: unused.
            wave_b: Vec4::new(0.0, 0.35, 0.035, 0.0),
            // x: Schlick f0, y: roughness at grazing, z: time scale, w: unused.
            params: Vec4::new(0.02, 0.05, 1.0, 0.0),
        }
    }
}

impl Default for WaterExt {
    fn default() -> Self {
        Self { params: WaterUniform::assumed_default() }
    }
}

/// Extension data of the water material. The whole struct is uploaded as one uniform at binding
/// [`WATER_BINDING`] (struct-level `#[uniform]`, converted through the `From` impl below).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[reflect(Default, Debug, Clone)]
#[uniform(100, WaterUniform)]
pub struct WaterExt {
    /// Not reflected: it is a GPU layout struct, not user-facing data.
    #[reflect(ignore)]
    pub params: WaterUniform,
}

impl From<&WaterExt> for WaterUniform {
    fn from(ext: &WaterExt) -> Self {
        ext.params
    }
}

impl MaterialExtension for WaterExt {
    fn fragment_shader() -> ShaderRef {
        EMBEDDED_SHADER_PATH.into()
    }

    /// Transparent: the alpha is decided in the shader from the view distance and the Fresnel.
    fn alpha_mode() -> Option<AlphaMode> {
        Some(AlphaMode::Blend)
    }

    /// A blended water plane neither writes the depth prepass nor casts a shadow.
    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// The water material type used by the instances: standard base + our extension.
pub type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterExt>;

/// Build a water material. The base is a white, smooth, double-sided blended plane; the colour
/// and the alpha come from the shader.
pub fn water_material(ext: WaterExt) -> WaterMaterial {
    WaterMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.18,
            metallic: 0.0,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        },
        extension: ext,
    }
}

/// Registers the water material, the embedded shader and the [`WaterBodies`] resource.
pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "water.wgsl");
        app.add_plugins(MaterialPlugin::<WaterMaterial>::default());
        app.init_resource::<WaterBodies>();
    }
}

// --- Water bodies (where the water is and how high) --------------------------------------------

/// A flat water surface: the world XZ triangles of the water mesh plus the surface height.
///
/// The footprint MUST be a triangle list, not a rectangle: measured with `water_report`, the
/// Jubilife river (`ground_area00_water01`) is a diagonal band whose axis-aligned box is
/// 524 x 1067 m and covers mostly dry land (only 13 of its 1760 vertices are near that box).
#[derive(Debug, Clone, PartialEq)]
pub struct WaterBody {
    pub min: Vec2,
    pub max: Vec2,
    pub level: f32,
    pub triangles: Vec<[Vec2; 3]>,
}

impl WaterBody {
    /// AABB reject first, then an exact point-in-triangle test over the water mesh.
    pub fn contains(&self, p: Vec2) -> bool {
        if p.x < self.min.x || p.x > self.max.x || p.y < self.min.y || p.y > self.max.y {
            return false;
        }
        self.triangles.iter().any(|t| point_in_triangle(p, t))
    }

    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }
}

/// Point in triangle (edges included). Degenerate slivers are rejected so they cannot claim a line.
pub fn point_in_triangle(p: Vec2, t: &[Vec2; 3]) -> bool {
    let area = (t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[2].x - t[0].x) * (t[1].y - t[0].y);
    if area.abs() < 1e-9 {
        return false;
    }
    let side = |a: Vec2, b: Vec2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    let (d1, d2, d3) = (side(t[0], t[1]), side(t[1], t[2]), side(t[2], t[0]));
    !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
}

/// One water model as it is loaded: local mesh bounds (for the surface height) and the local XZ
/// triangles (for the footprint).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WaterMesh {
    pub local_min: Vec3,
    pub local_max: Vec3,
    pub triangles: Vec<[Vec2; 3]>,
}

/// Every water rectangle the viewer has streamed in, in world XZ. Built by `world_objects` from
/// each water placement and its mesh bounds; consumed by the locomotion to decide swimming.
#[derive(Resource, Debug, Clone, Default)]
pub struct WaterBodies {
    pub bodies: Vec<WaterBody>,
    /// Placements dropped because [`Self::MAX_BODIES`] was reached (reported in the viewer log).
    pub dropped: usize,
}

impl WaterBodies {
    /// Bound on the list so a bad sheet cannot grow it without limit. Each body keeps the water
    /// mesh's triangles (tens of kB), so this also bounds the memory the water can take.
    pub const MAX_BODIES: usize = 64;

    /// Add `body` unless it is already there (the streaming loop can see a model several times).
    /// Returns `false` when it was dropped (duplicate or over the cap).
    pub fn insert(&mut self, body: WaterBody) -> bool {
        let finite = body.min.x.is_finite()
            && body.min.y.is_finite()
            && body.max.x.is_finite()
            && body.max.y.is_finite()
            && body.level.is_finite();
        if !finite || body.size().x <= 0.0 || body.size().y <= 0.0 {
            return false;
        }
        if self.bodies.contains(&body) {
            return true;
        }
        if self.bodies.len() >= Self::MAX_BODIES {
            self.dropped += 1;
            return false;
        }
        self.bodies.push(body);
        true
    }

    /// Highest water surface covering `p`, if any (exact triangle test).
    pub fn level_at(&self, p: Vec2) -> Option<f32> {
        self.bodies.iter().filter(|b| b.contains(p)).map(|b| b.level).reduce(f32::max)
    }

    /// Highest surface of a body whose *bounding box* is within `max_dist` of `p`. Used only for
    /// the "is there water ahead, deep enough to step in" test at a shore, where the exact
    /// triangle is a step away; the state itself always uses [`Self::level_at`].
    pub fn level_near(&self, p: Vec2, max_dist: f32) -> Option<f32> {
        let d = |v: f32, lo: f32, hi: f32| {
            if v < lo {
                lo - v
            } else if v > hi {
                v - hi
            } else {
                0.0
            }
        };
        self.bodies
            .iter()
            .filter(|b| {
                let dx = d(p.x, b.min.x, b.max.x);
                let dz = d(p.y, b.min.y, b.max.y);
                (dx * dx + dz * dz).sqrt() <= max_dist
            })
            .map(|b| b.level)
            .reduce(f32::max)
    }
}

/// World-space AABB of a local box under `tf` (rotation included: all eight corners).
pub fn world_aabb_of(local_min: Vec3, local_max: Vec3, tf: &Transform) -> (Vec3, Vec3) {
    let m = tf.to_matrix();
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for i in 0..8u32 {
        let c = Vec3::new(
            if i & 1 == 0 { local_min.x } else { local_max.x },
            if i & 2 == 0 { local_min.y } else { local_max.y },
            if i & 4 == 0 { local_min.z } else { local_max.z },
        );
        let w = m.transform_point3(c);
        lo = lo.min(w);
        hi = hi.max(w);
    }
    (lo, hi)
}

/// Water surface of a water mesh placed with `tf`: the mesh triangles in world XZ, the surface at
/// the top face of the transformed mesh bounds (a flat plane has min.y ~ max.y, so either works).
pub fn water_body_from_mesh(mesh: &WaterMesh, tf: &Transform) -> WaterBody {
    let (lo, hi) = world_aabb_of(mesh.local_min, mesh.local_max, tf);
    let m = tf.to_matrix();
    let triangles = mesh
        .triangles
        .iter()
        .map(|t| {
            t.map(|p| {
                let w = m.transform_point3(Vec3::new(p.x, 0.0, p.y));
                Vec2::new(w.x, w.z)
            })
        })
        .collect();
    WaterBody { min: Vec2::new(lo.x, lo.z), max: Vec2::new(hi.x, hi.z), level: hi.y, triangles }
}

// --- Swimming ----------------------------------------------------------------------------------

/// Overworld locomotion state of the avatar with respect to water.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SwimState {
    #[default]
    Grounded,
    Swimming,
}

/// State on the avatar entity. Written by the viewer's locomotion, read by the animation system.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerSwim {
    pub state: SwimState,
}

/// How deep the feet are below the water surface (negative = above it).
pub fn water_depth(level: f32, foot_y: f32) -> f32 {
    level - foot_y
}

/// Grounded -> Swimming when the chest would be under the surface; back to Grounded only when the
/// water is measurably shallower (hysteresis band [`SWIM_EXIT_DEPTH`]..[`SWIM_ENTER_DEPTH`]).
pub fn step_swim(current: SwimState, level: Option<f32>, foot_y: f32) -> SwimState {
    let Some(level) = level else { return SwimState::Grounded };
    let depth = water_depth(level, foot_y);
    match current {
        SwimState::Grounded if depth > SWIM_ENTER_DEPTH => SwimState::Swimming,
        SwimState::Swimming if depth < SWIM_EXIT_DEPTH => SwimState::Grounded,
        other => other,
    }
}

/// World y of the avatar root while swimming (`level` is the water surface).
pub fn swim_root_y(level: f32) -> f32 {
    level - SWIM_ROOT_BELOW_SURFACE
}

/// Water level at `p`, but only when the surface actually sits on the ground there: the flat water
/// meshes can float far above (or hang below) the terrain, and entering those is meaningless. See
/// [`SWIM_MAX_GROUND_OFFSET`]; `ground_y` is the terrain/navmesh height under the same point.
pub fn grounded_level_at(bodies: &WaterBodies, p: Vec2, ground_y: f32, max_offset: f32) -> Option<f32> {
    let level = bodies.level_at(p)?;
    ((level - ground_y).abs() <= max_offset).then_some(level)
}

// --- CPU mirrors of the shader maths (the WGSL cannot be unit-tested; these document the
// --- intended behaviour and are asserted in the tests below) -----------------------------------

/// Schlick Fresnel, mirroring `schlick` in `water.wgsl`.
pub fn fresnel_schlick(cos_v: f32, f0: f32) -> f32 {
    let c = cos_v.clamp(0.0, 1.0);
    f0 + (1.0 - f0) * (1.0 - c).powi(5)
}

/// Gradient of the three directional waves, mirroring `wave_gradient` in `water.wgsl`.
pub fn wave_gradient(p: Vec2, t: f32) -> Vec2 {
    let d0 = Vec2::new(0.860, 0.510);
    let d1 = Vec2::new(-0.420, 0.908);
    let d2 = Vec2::new(0.310, -0.951);
    let mut g = d0 * (p.dot(d0) + t).cos();
    g += d1 * (p.dot(d1) * 2.30 + t * 1.70).cos() * 0.55;
    g += d2 * (p.dot(d2) * 4.10 + t * 2.90).cos() * 0.28;
    g
}

/// The placeholder avatar entity marker re-exported so callers only need one import.
pub use super::PlayerAvatar as _PlayerAvatar;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_models_are_recognised_by_name() {
        assert!(is_water_model("ground_area00_water01"));
        assert!(is_water_model("wall_area01_water01_asase"));
        assert!(is_water_model("ground_area01_waterfall01"));
        assert!(!is_water_model("ground_area01_cliff01"));
        assert!(!is_water_model("objects_house01"));
    }

    #[test]
    fn only_grounded_water_counts_for_swimming() {
        let mut wb = WaterBodies::default();
        wb.insert(rect_body(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0), 10.0));
        let p = Vec2::new(5.0, 5.0);
        // The surface sits on the ground (a river bed at 9 m): usable.
        assert_eq!(grounded_level_at(&wb, p, 9.0, SWIM_MAX_GROUND_OFFSET), Some(10.0));
        // The bed is 1.3 m under the surface: chest-deep, swimming starts.
        assert_eq!(step_swim(SwimState::Grounded, grounded_level_at(&wb, p, 8.7, SWIM_MAX_GROUND_OFFSET), 8.7), SwimState::Swimming);
        // A flat plane floating 50 m above the terrain (measured in area 00): not water we can enter.
        assert_eq!(grounded_level_at(&wb, p, -40.0, SWIM_MAX_GROUND_OFFSET), None);
        assert_eq!(step_swim(SwimState::Grounded, grounded_level_at(&wb, p, -40.0, SWIM_MAX_GROUND_OFFSET), -40.0), SwimState::Grounded);
        // Buried under the hills: same.
        assert_eq!(grounded_level_at(&wb, p, 42.0, SWIM_MAX_GROUND_OFFSET), None);
        // Off the water entirely.
        assert_eq!(grounded_level_at(&wb, Vec2::new(50.0, 50.0), 9.0, SWIM_MAX_GROUND_OFFSET), None);
    }

    #[test]
    fn fresnel_is_monotonic_and_bounded() {
        let f0 = 0.02;
        let mut last = fresnel_schlick(0.0, f0);
        assert!((last - 1.0).abs() < 1e-6, "head-on grazing reflectance is 1, got {last}");
        for i in 1..=100 {
            let cos_v = i as f32 / 100.0;
            let f = fresnel_schlick(cos_v, f0);
            assert!(f <= last + 1e-6, "must not rise with the view angle ({cos_v}: {f} > {last})");
            assert!((0.0..=1.0).contains(&f));
            last = f;
        }
        assert!((fresnel_schlick(1.0, f0) - f0).abs() < 1e-6, "straight-on is f0");
        // Out-of-range inputs are clamped, never NaN.
        assert!(fresnel_schlick(2.0, f0).is_finite());
        assert!(fresnel_schlick(-1.0, f0).is_finite());
    }

    #[test]
    fn wave_gradient_is_bounded_and_animates() {
        let p = Vec2::new(3.5, -1.25);
        let a = wave_gradient(p, 0.0);
        let b = wave_gradient(p, 0.37);
        assert!(a.length() > 0.05, "a flat normal would not be visible: {a:?}");
        assert!(a.length() < 1.5, "gradient must stay a perturbation: {a:?}");
        assert!((a - b).length() > 1e-3, "waves must move with time");
        // Deterministic: the same inputs give the same normal.
        assert_eq!(a, wave_gradient(p, 0.0));
    }

    #[test]
    fn swim_needs_deep_water_and_has_hysteresis() {
        // No water at all: never swimming.
        assert_eq!(step_swim(SwimState::Swimming, None, 0.0), SwimState::Grounded);
        // 0.5 m of water over the feet: chest still dry, stay grounded.
        assert_eq!(step_swim(SwimState::Grounded, Some(10.5), 10.0), SwimState::Grounded);
        // 1.3 m: chest under, start swimming.
        assert_eq!(step_swim(SwimState::Grounded, Some(11.3), 10.0), SwimState::Swimming);
        // 1.0 m: inside the hysteresis band, keep swimming.
        assert_eq!(step_swim(SwimState::Swimming, Some(11.0), 10.0), SwimState::Swimming);
        // 0.8 m: shallow again, leave the water.
        assert_eq!(step_swim(SwimState::Swimming, Some(10.8), 10.0), SwimState::Grounded);
        // Exactly at the entry depth: no swimming yet (strict inequality).
        assert_eq!(step_swim(SwimState::Grounded, Some(11.2), 10.0), SwimState::Grounded);
        // Stepping out onto the bank (feet above the surface) exits.
        assert_eq!(step_swim(SwimState::Swimming, Some(10.0), 10.5), SwimState::Grounded);
        assert!((water_depth(12.0, 10.0) - 2.0).abs() < 1e-6);
        assert!((swim_root_y(12.0) - (12.0 - SWIM_ROOT_BELOW_SURFACE)).abs() < 1e-6);
    }

    /// A 10 x 4 rectangle from two triangles at level 5.
    fn rect_body(min: Vec2, max: Vec2, level: f32) -> WaterBody {
        let (a, b, c, d) = (
            Vec2::new(min.x, min.y),
            Vec2::new(max.x, min.y),
            Vec2::new(max.x, max.y),
            Vec2::new(min.x, max.y),
        );
        WaterBody { min, max, level, triangles: vec![[a, b, c], [a, c, d]] }
    }

    #[test]
    fn point_in_triangle_edges_and_degenerate() {
        let t = [Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(0.0, 2.0)];
        assert!(point_in_triangle(Vec2::new(0.5, 0.5), &t));
        assert!(point_in_triangle(Vec2::new(0.0, 0.0), &t), "corner counts");
        assert!(point_in_triangle(Vec2::new(1.0, 0.0), &t), "edge counts");
        assert!(!point_in_triangle(Vec2::new(1.5, 1.5), &t));
        assert!(!point_in_triangle(Vec2::new(-0.1, 0.5), &t));
        // A zero-area sliver never claims a point.
        let flat = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(2.0, 2.0)];
        assert!(!point_in_triangle(Vec2::new(1.0, 1.0), &flat));
    }

    #[test]
    fn water_bodies_lookup_uses_the_real_footprint_not_the_box() {
        let mut wb = WaterBodies::default();
        assert!(wb.insert(rect_body(Vec2::new(0.0, 0.0), Vec2::new(10.0, 4.0), 5.0)));
        assert!(wb.insert(rect_body(Vec2::new(2.0, 1.0), Vec2::new(6.0, 3.0), 6.0)));
        // Same rectangle again: accepted but not duplicated.
        assert!(wb.insert(rect_body(Vec2::new(0.0, 0.0), Vec2::new(10.0, 4.0), 5.0)));
        assert_eq!(wb.bodies.len(), 2);
        assert_eq!(wb.dropped, 0);

        assert_eq!(wb.level_at(Vec2::new(3.0, 2.0)), Some(6.0), "the top surface wins");
        assert_eq!(wb.level_at(Vec2::new(8.0, 2.0)), Some(5.0));
        assert_eq!(wb.level_at(Vec2::new(11.0, 2.0)), None);
        assert!((wb.level_at(Vec2::new(3.0, 2.0)).map(|l| water_depth(l, 0.0)).unwrap() - 6.0).abs() < 1e-6);

        // A diagonal band: points inside its bounding box but off the band are dry land.
        let band = WaterBody {
            min: Vec2::new(0.0, 0.0),
            max: Vec2::new(10.0, 10.0),
            level: 3.0,
            triangles: vec![[
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(1.0, 10.0),
            ], [
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 10.0),
                Vec2::new(0.0, 10.0),
            ]],
        };
        assert!(band.contains(Vec2::new(0.5, 5.0)));
        assert!(!band.contains(Vec2::new(5.0, 5.0)), "inside the box, off the band");
        let mut wb2 = WaterBodies::default();
        wb2.insert(band);
        assert_eq!(wb2.level_at(Vec2::new(5.0, 5.0)), None);

        // Degenerate or non-finite boxes are rejected.
        assert!(!wb.insert(WaterBody { min: Vec2::ZERO, max: Vec2::ZERO, level: 1.0, triangles: Vec::new() }));
        assert!(!wb.insert(WaterBody {
            min: Vec2::ZERO,
            max: Vec2::new(1.0, 1.0),
            level: f32::NAN,
            triangles: Vec::new(),
        }));
        assert_eq!(wb.bodies.len(), 2);

        // The cap drops silently but is counted. Two bodies are already in, so only
        // MAX_BODIES - 2 of the new ones fit; the rest are counted as dropped.
        for i in 0..WaterBodies::MAX_BODIES + 5 {
            let x = 100.0 + i as f32 * 10.0;
            let _ = wb.insert(rect_body(Vec2::new(x, 0.0), Vec2::new(x + 5.0, 5.0), 1.0));
        }
        assert_eq!(wb.bodies.len(), WaterBodies::MAX_BODIES);
        assert_eq!(wb.dropped, WaterBodies::MAX_BODIES + 5 - (WaterBodies::MAX_BODIES - 2));
    }

    #[test]
    fn level_near_finds_the_shore() {
        let mut wb = WaterBodies::default();
        wb.insert(rect_body(Vec2::new(10.0, 10.0), Vec2::new(20.0, 20.0), 3.0));
        // Inside the rectangle.
        assert_eq!(wb.level_near(Vec2::new(15.0, 15.0), 1.0), Some(3.0));
        // 1 m off the edge.
        assert_eq!(wb.level_near(Vec2::new(21.0, 15.0), 2.0), Some(3.0));
        // Too far away.
        assert_eq!(wb.level_near(Vec2::new(23.0, 15.0), 2.0), None);
    }

    #[test]
    fn world_aabb_handles_yaw_and_scale() {
        let min = Vec3::new(-1.0, 0.0, -2.0);
        let max = Vec3::new(1.0, 0.1, 2.0);
        // Identity.
        let (lo, hi) = world_aabb_of(min, max, &Transform::IDENTITY);
        assert_eq!((lo, hi), (min, max));
        // Translation only.
        let tf = Transform::from_xyz(10.0, 58.0, -3.0);
        let (lo, hi) = world_aabb_of(min, max, &tf);
        assert!((lo - Vec3::new(9.0, 58.0, -5.0)).length() < 1e-5);
        assert!((hi - Vec3::new(11.0, 58.1, -1.0)).length() < 1e-5);
        // 180 degrees about Y swaps the XZ signs; the box is symmetric so it does not move here.
        let tf = Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI));
        let (lo, hi) = world_aabb_of(min, max, &tf);
        assert!((lo - Vec3::new(-1.0, 0.0, -2.0)).length() < 1e-5);
        assert!((hi - Vec3::new(1.0, 0.1, 2.0)).length() < 1e-5);
        // Scale grows the box.
        let tf = Transform::from_scale(Vec3::splat(2.0));
        let (lo, hi) = world_aabb_of(min, max, &tf);
        assert!((lo - Vec3::new(-2.0, 0.0, -4.0)).length() < 1e-5);
        assert!((hi - Vec3::new(2.0, 0.2, 4.0)).length() < 1e-5);
    }

    #[test]
    fn water_body_level_is_the_top_face() {
        // A flat plane 0.02 m thick at y = 58.28: the body level is the top face and the footprint
        // follows the triangles (here one triangle).
        let mesh = WaterMesh {
            local_min: Vec3::new(0.0, 58.28, 0.0),
            local_max: Vec3::new(10.0, 58.30, 10.0),
            triangles: vec![[Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(0.0, 10.0)]],
        };
        let body = water_body_from_mesh(&mesh, &Transform::IDENTITY);
        assert!((body.level - 58.30).abs() < 1e-5);
        assert!((body.size() - Vec2::new(10.0, 10.0)).length() < 1e-5);
        assert!(body.contains(Vec2::new(2.0, 2.0)));
        assert!(!body.contains(Vec2::new(8.0, 8.0)), "outside the triangle");
        assert!(!body.contains(Vec2::new(5.0, 11.0)), "outside the box");
    }

    #[test]
    fn water_body_triangles_follow_the_placement() {
        let mesh = WaterMesh {
            local_min: Vec3::new(0.0, 1.0, 0.0),
            local_max: Vec3::new(10.0, 1.0, 10.0),
            triangles: vec![[Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(0.0, 10.0)]],
        };
        // 180 degrees about Y and a translation: the triangle ends up at x = 100 - x, z = 200 - z.
        let tf = Transform::from_xyz(100.0, 5.0, 200.0).with_rotation(Quat::from_rotation_y(std::f32::consts::PI));
        let body = water_body_from_mesh(&mesh, &tf);
        assert!((body.level - 6.0).abs() < 1e-5, "level is the placed height");
        assert_eq!(body.triangles.len(), 1);
        let (a, b, c) = (body.triangles[0][0], body.triangles[0][1], body.triangles[0][2]);
        assert!((a - Vec2::new(100.0, 200.0)).length() < 1e-4, "{a:?}");
        assert!((b - Vec2::new(90.0, 200.0)).length() < 1e-4, "{b:?}");
        assert!((c - Vec2::new(100.0, 190.0)).length() < 1e-4, "{c:?}");
        assert!(body.contains(Vec2::new(95.0, 195.0)));
        assert!(!body.contains(Vec2::new(85.0, 205.0)));
    }

    #[test]
    fn shader_is_embedded_and_matches_the_rust_interface() {
        let src = include_str!("water.wgsl");
        assert!(src.contains("fn fragment("), "no fragment entry point");
        assert!(src.contains("@binding(100)"), "the extension binding moved; update WATER_BINDING");
        assert!(src.contains("pbr_input_from_standard_material"), "the base material must be StandardMaterial");
        for field in ["deep_color", "shallow_color", "sky_color", "wave_a", "wave_b", "params"] {
            assert!(src.contains(field), "water.wgsl is missing the uniform member {field}");
        }
        assert!(EMBEDDED_SHADER_PATH.ends_with("water.wgsl"));
        // The embedded path is derived from the crate/file layout; a mismatch just means the
        // shader fails to load, which the offscreen example would catch.
        assert_eq!(WATER_BINDING, 100);
    }

    #[test]
    fn default_uniform_is_sane() {
        let u = WaterUniform::assumed_default();
        for v in [u.deep_color, u.shallow_color, u.sky_color, u.wave_a, u.wave_b, u.params] {
            assert!(v.is_finite(), "uniform values must be finite: {v:?}");
        }
        for c in [u.deep_color, u.shallow_color, u.sky_color] {
            for ch in [c.x, c.y, c.z] {
                assert!((0.0..=1.0).contains(&ch), "colours are linear 0..1: {ch}");
            }
        }
        assert!((0.0..=1.0).contains(&u.deep_color.w) && (0.0..=1.0).contains(&u.shallow_color.w));
        assert!(u.deep_color.w > u.shallow_color.w, "deep water is more opaque");
        assert!(u.wave_b.y > 0.0, "the waves must perturb the normal");
        assert!(u.params.x > 0.0 && u.params.x < 0.2, "f0 of water is small");
    }

}
