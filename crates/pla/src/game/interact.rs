//! World interactables: climbable surfaces (ladders, ropes, vines, cliffs) for Brief 13.
//!
//! Placement state: the original has **no ladder or rope objects**; climbing is the ride-Sneasler
//! wall climb, and the real climbable surfaces are the terrain cliffs. They are derived from the
//! decoded heightmaps with the decoded climb-angle rule in `data/field_climb_surfaces.tsv` and
//! spawned by [`super::climb_surfaces`] (see `reports/data-progress/climb_surfaces.md` and
//! `reports/data-progress/ropes_ladders_unknown.md`). The debug ladder (`PLA_DEBUG_CLIMB=1`,
//! [`spawn_debug_climbable`]) stays for eyeballing the mechanic without any data.
//!
//! Doors are NOT handled here: the decoded door/fast-travel table (`data/warp_links.tsv`)
//! and its trigger flow live in `game::zone` (Brief 8, [`game::zone::door_trigger_system`]),
//! which remains the only door path.

use bevy::prelude::*;

use super::player_anim::AVATAR_ROOT_ABOVE_GROUND;

/// Kind of climbable surface.
///
/// The taxonomy is ASSUMED (the original game only exposes the ride-Sneasler wall climb;
/// ladders/ropes/vines are port additions driven by this component).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClimbKind {
    #[default]
    Ladder,
    Rope,
    Vine,
    Cliff,
}

/// A vertical surface the avatar can climb, from `bottom` to `top` (world space, ground frame).
///
/// The avatar root (feet + [`AVATAR_ROOT_ABOVE_GROUND`]) climbs between `bottom.y + root`
/// and `top.y + root`; the horizontal position is held at `bottom.xz` while climbing, and the
/// top step-off lands on `top.xz` (for the derived cliff sheet that is the crest of the face,
/// elsewhere than the base; the debug ladder keeps `top.xz == bottom.xz`).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Climbable {
    pub kind: ClimbKind,
    /// Ground-level base of the surface.
    pub bottom: Vec3,
    /// Ground-level top of the surface (where the avatar steps off).
    pub top: Vec3,
    /// Horizontal reach radius around the base (m).
    pub radius: f32,
}

impl Climbable {
    /// World-space height of the avatar root at the bottom / top of the surface.
    pub fn root_range(&self) -> (f32, f32) {
        (
            self.bottom.y + AVATAR_ROOT_ABOVE_GROUND,
            self.top.y + AVATAR_ROOT_ABOVE_GROUND,
        )
    }

    /// Total climbable height (m), 0 when the surface is degenerate.
    pub fn height(&self) -> f32 {
        (self.top.y - self.bottom.y).max(0.0)
    }
}

/// Boundary reached by one [`climb_step`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClimbBoundary {
    #[default]
    None,
    /// The avatar reached the top (step off there).
    Top,
    /// The avatar slid back to the bottom.
    Bottom,
}

/// Is `p` (avatar root, world space) within reach of `c`?
///
/// Horizontal distance to the base at most `radius`, and `p.y` inside the climbable span
/// widened by `vertical_slack` (so a jump that grazes the top can still grab it).
pub fn in_reach(c: &Climbable, p: Vec3, vertical_slack: f32) -> bool {
    in_reach_within(c, p, vertical_slack, c.radius)
}

/// [`in_reach`] with an explicit horizontal cap: at most `max_horizontal` from the base
/// (never more than the surface's own `radius`) and the same vertical span.
///
/// The approach grab (`game::player_motion`) uses a short cap so crossing the wider interact
/// radius on foot does not pull the avatar while simply walking past.
pub fn in_reach_within(c: &Climbable, p: Vec3, vertical_slack: f32, max_horizontal: f32) -> bool {
    let reach = max_horizontal.min(c.radius);
    let dx = p.x - c.bottom.x;
    let dz = p.z - c.bottom.z;
    if dx * dx + dz * dz > reach * reach {
        return false;
    }
    let (low, high) = c.root_range();
    p.y >= low - vertical_slack && p.y <= high + vertical_slack
}

/// The nearest climbable in reach of `p`, if any.
pub fn nearest_climbable<'a>(
    climbables: impl Iterator<Item = &'a Climbable>,
    p: Vec3,
    vertical_slack: f32,
) -> Option<&'a Climbable> {
    climbables
        .filter(|c| in_reach(c, p, vertical_slack))
        .min_by(|a, b| {
            let da = (a.bottom.x - p.x).powi(2) + (a.bottom.z - p.z).powi(2);
            let db = (b.bottom.x - p.x).powi(2) + (b.bottom.z - p.z).powi(2);
            da.total_cmp(&db)
        })
}

/// The nearest climbable within `max_horizontal` of the base ([`in_reach_within`]).
pub fn nearest_climbable_within<'a>(
    climbables: impl Iterator<Item = &'a Climbable>,
    p: Vec3,
    vertical_slack: f32,
    max_horizontal: f32,
) -> Option<&'a Climbable> {
    climbables
        .filter(|c| in_reach_within(c, p, vertical_slack, max_horizontal))
        .min_by(|a, b| {
            let da = (a.bottom.x - p.x).powi(2) + (a.bottom.z - p.z).powi(2);
            let db = (b.bottom.x - p.x).powi(2) + (b.bottom.z - p.z).powi(2);
            da.total_cmp(&db)
        })
}

/// Alignment (dot of unit vectors) between the frame's planar move and the *face* direction
/// for the approach grab to fire: about a 60-degree cone in front of the walk.
pub const APPROACH_GRAB_DOT: f32 = 0.5;

/// Smallest planar move of a frame (m) that counts as "walking into" a surface.
pub const APPROACH_GRAB_MIN_STEP: f32 = 0.004;

/// Horizontal uphill direction of a surface (base -> crest), the direction a walker runs in
/// when running into the face. Degenerate for a perfectly vertical column (base == crest in
/// plan): `None` there, callers fall back to the direction of the base point.
pub fn face_uphill_dir(c: &Climbable) -> Option<Vec2> {
    let face = Vec2::new(c.top.x - c.bottom.x, c.top.z - c.bottom.z);
    (face.length_squared() > 0.25).then(|| face.normalize())
}

/// The climbable the frame's planar `moved_xz` walks into: within `max_horizontal` of the
/// base (vertical span like [`in_reach`]) and moving into the face
/// ([`face_uphill_dir`] within the [`APPROACH_GRAB_DOT`] cone; a vertical column falls back
/// to the direction of its base point).
///
/// This is the "advance into the wall" grab (a port reading of the original Sneasler wall
/// grab, which fires by running into the wall). Walking beside the face or away from it never
/// matches. Aiming at the **base point** was the first attempt and failed for wide faces: a
/// straight run at the face passes two metres to the side of the base cell and the cone
/// dropped below the threshold (measured on foot, 2026-10-15).
pub fn approach_climbable<'a>(
    climbables: impl Iterator<Item = &'a Climbable>,
    p: Vec3,
    moved_xz: Vec2,
    vertical_slack: f32,
    max_horizontal: f32,
) -> Option<&'a Climbable> {
    if moved_xz.length() < APPROACH_GRAB_MIN_STEP {
        return None;
    }
    let dir = moved_xz.normalize();
    climbables
        .filter(|c| in_reach_within(c, p, vertical_slack, max_horizontal))
        .filter(|c| match face_uphill_dir(c) {
            Some(face) => face.dot(dir) >= APPROACH_GRAB_DOT,
            None => {
                let to = Vec2::new(c.bottom.x - p.x, c.bottom.z - p.z);
                to.length_squared() < 1e-6 || to.normalize().dot(dir) >= APPROACH_GRAB_DOT
            }
        })
        .min_by(|a, b| {
            let da = (a.bottom.x - p.x).powi(2) + (a.bottom.z - p.z).powi(2);
            let db = (b.bottom.x - p.x).powi(2) + (b.bottom.z - p.z).powi(2);
            da.total_cmp(&db)
        })
}

/// One vertical climb step.
///
/// `y` is the avatar root height, `input_up` is +1 up / -1 down (analog allowed),
/// `speed` m/s. The result is clamped to the surface's root range; [`ClimbBoundary::Top`]
/// and [`ClimbBoundary::Bottom`] mean "leave the climb there".
pub fn climb_step(
    c: &Climbable,
    y: f32,
    input_up: f32,
    speed: f32,
    dt: f32,
) -> (f32, ClimbBoundary) {
    let (low, high) = c.root_range();
    let input = input_up.clamp(-1.0, 1.0);
    let next = y + input * speed * dt;
    // A boundary needs input toward it: standing still at the foot of a ladder is not "leaving".
    if input > 0.0 && next >= high {
        (high, ClimbBoundary::Top)
    } else if input < 0.0 && next <= low {
        (low, ClimbBoundary::Bottom)
    } else {
        (next.clamp(low, high), ClimbBoundary::None)
    }
}

/// Fractions of the climb completed at root height `y` (0 bottom, 1 top).
pub fn climb_progress(c: &Climbable, y: f32) -> f32 {
    let (low, high) = c.root_range();
    if high - low <= f32::EPSILON {
        return 1.0;
    }
    ((y - low) / (high - low)).clamp(0.0, 1.0)
}

// ------------------------------------------------------------------ climb hint (on-screen)

/// Contextual HUD line shown while a climbable is in reach: the key that starts the climb
/// (user-report fix 2026-10-15: the mechanic existed but nothing told the player how to use it).
pub const CLIMB_HINT_GRAB: &str = "TREPAR: Enter/Espacio (o camina contra la pared)";

/// HUD line while attached to a surface (`|`, not the middle dot: the default UI font lacks it).
pub const CLIMB_HINT_CLIMBING: &str = "TREPAR: W/S subir y bajar | K soltar";

/// HUD line on a ride-Sneasler, whose wall grab fires by running into the wall.
pub const CLIMB_HINT_SNEASLER: &str = "TREPAR (Sneasler): camina contra la pared";

/// The hint line for one frame; `None` means "do not show the bar".
///
/// `in_reach` is the same condition the grab key uses (the surface's own radius), so the
/// prompt never promises a key press that would do nothing.
pub fn climb_hint_line(
    climbing: bool,
    in_reach: bool,
    mounted_sneasler: bool,
) -> Option<&'static str> {
    if climbing {
        return Some(CLIMB_HINT_CLIMBING);
    }
    if !in_reach {
        return None;
    }
    Some(if mounted_sneasler {
        CLIMB_HINT_SNEASLER
    } else {
        CLIMB_HINT_GRAB
    })
}

/// `PLA_DEBUG_CLIMB=1` spawns one demo ladder in front of the avatar so the climb mechanic
/// can be eyeballed in the viewer without decoded placements.
pub fn debug_climb_enabled() -> bool {
    std::env::var_os("PLA_DEBUG_CLIMB").is_some_and(|v| v != "0")
}

/// Height (m) of the demo ladder spawned by [`spawn_debug_climbable`].
pub const DEBUG_CLIMB_HEIGHT: f32 = 4.0;

/// Spawns the demo ladder once, three metres in front of the avatar's spawn facing.
pub fn spawn_debug_climbable(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Query<&Transform, With<super::PlayerAvatar>>,
    mut done: Local<bool>,
) {
    if *done || !debug_climb_enabled() {
        return;
    }
    let Ok(tf) = player.single() else { return };
    *done = true;
    let forward = tf.rotation * Vec3::Z;
    let base = Vec3::new(tf.translation.x, 0.0, tf.translation.z)
        + Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero() * 3.0;
    let base = Vec3::new(base.x, tf.translation.y - AVATAR_ROOT_ABOVE_GROUND, base.z);
    let wood = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.36, 0.18),
        perceptual_roughness: 0.8,
        ..default()
    });
    let rung = meshes.add(Mesh::from(Cuboid::new(1.2, 0.08, 0.08)));
    let rail = meshes.add(Mesh::from(Cuboid::new(0.08, DEBUG_CLIMB_HEIGHT, 0.08)));
    commands
        .spawn((
            Climbable {
                kind: ClimbKind::Ladder,
                bottom: base,
                top: base + Vec3::Y * DEBUG_CLIMB_HEIGHT,
                radius: 1.6,
            },
            Transform::from_translation(base),
            Visibility::default(),
        ))
        .with_children(|p| {
            for side in [-0.6_f32, 0.6] {
                p.spawn((
                    Mesh3d(rail.clone()),
                    MeshMaterial3d(wood.clone()),
                    Transform::from_xyz(side, DEBUG_CLIMB_HEIGHT * 0.5, 0.0),
                ));
            }
            let rungs = (DEBUG_CLIMB_HEIGHT / 0.4) as i32;
            for i in 0..rungs {
                p.spawn((
                    Mesh3d(rung.clone()),
                    MeshMaterial3d(wood.clone()),
                    Transform::from_xyz(0.0, 0.2 + 0.4 * i as f32, 0.0),
                ));
            }
        });
    println!(
        "Debug climbable spawned at ({:.1}, {:.1}, {:.1}), height {DEBUG_CLIMB_HEIGHT:.1} m",
        base.x, base.y, base.z
    );
}

/// Adds the debug climbable spawner. The climb state machine itself lives in
/// [`super::player_motion`].
pub struct InteractPlugin;

impl Plugin for InteractPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_debug_climbable);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ladder() -> Climbable {
        Climbable {
            kind: ClimbKind::Ladder,
            bottom: Vec3::new(10.0, 5.0, -3.0),
            top: Vec3::new(10.0, 9.0, -3.0),
            radius: 1.2,
        }
    }

    #[test]
    fn reach_is_horizontal_radius_and_vertical_span() {
        let c = ladder();
        let root_lo = 5.0 + AVATAR_ROOT_ABOVE_GROUND;
        let root_hi = 9.0 + AVATAR_ROOT_ABOVE_GROUND;
        assert!(in_reach(&c, Vec3::new(10.0, root_lo, -3.0), 0.2));
        assert!(in_reach(&c, Vec3::new(11.1, root_hi, -3.0), 0.2));
        // Outside the radius.
        assert!(!in_reach(&c, Vec3::new(11.5, root_lo, -3.0), 0.2));
        // Below the base and above the top, beyond the slack.
        assert!(!in_reach(&c, Vec3::new(10.0, root_lo - 0.5, -3.0), 0.2));
        assert!(!in_reach(&c, Vec3::new(10.0, root_hi + 0.5, -3.0), 0.2));
        // Inside the slack.
        assert!(in_reach(&c, Vec3::new(10.0, root_lo - 0.1, -3.0), 0.2));
    }

    #[test]
    fn reach_within_caps_the_horizontal_distance() {
        // A real cliff keeps a 6 m interact radius; the approach cap only shrinks it.
        let cliff = Climbable {
            kind: ClimbKind::Cliff,
            radius: 6.0,
            ..ladder()
        };
        let root_lo = 5.0 + AVATAR_ROOT_ABOVE_GROUND;
        // 2 m from the base: inside both.
        assert!(in_reach(&cliff, Vec3::new(12.0, root_lo, -3.0), 0.2));
        assert!(in_reach_within(&cliff, Vec3::new(12.0, root_lo, -3.0), 0.2, 2.5));
        // 3 m from the base: the surface reach says yes, the short cap says no.
        let p = Vec3::new(13.0, root_lo, -3.0);
        assert!(in_reach(&cliff, p, 0.2));
        assert!(!in_reach_within(&cliff, p, 0.2, 2.5));
        // A 1.2 m ladder is never widened by a bigger cap.
        let c = ladder();
        assert!(in_reach_within(&c, Vec3::new(11.1, root_lo, -3.0), 0.2, 2.5));
        assert!(!in_reach_within(&c, Vec3::new(11.5, root_lo, -3.0), 0.2, 2.5));
        // Vertical span still applies.
        assert!(!in_reach_within(
            &cliff,
            Vec3::new(12.0, root_lo + 10.0, -3.0),
            0.2,
            2.5
        ));
    }

    #[test]
    fn approach_grab_needs_a_move_into_the_base() {
        // Ladder base at z = -3, radius 1.2 (the reach is the smaller of the two).
        let c = ladder(); // base (10.0, 5.0, -3.0)
        let root_lo = 5.0 + AVATAR_ROOT_ABOVE_GROUND;
        let p = Vec3::new(10.0, root_lo, -1.9); // 1.1 m from the base
        // Walking towards the base (z decreasing by 0.1): grabs.
        let toward = approach_climbable([&c].into_iter(), p, Vec2::new(0.0, -0.1), 0.2, 2.5);
        assert_eq!(toward.map(|c| c.bottom.z), Some(-3.0));
        // Walking away: no grab.
        assert!(approach_climbable([&c].into_iter(), p, Vec2::new(0.0, 0.1), 0.2, 2.5).is_none());
        // Walking past (parallel, no dot): no grab.
        assert!(approach_climbable([&c].into_iter(), p, Vec2::new(0.1, 0.0), 0.2, 2.5).is_none());
        // Standing still: no grab.
        assert!(approach_climbable([&c].into_iter(), p, Vec2::ZERO, 0.2, 2.5).is_none());
        // Outside the effective reach (1.2 m here): no grab even walking in.
        let far = Vec3::new(10.0, root_lo, -1.5);
        assert!(approach_climbable([&c].into_iter(), far, Vec2::new(0.0, -0.1), 0.2, 2.5).is_none());
        // A cliff radius 6 keeps the full 2.5 m approach reach.
        let cliff = Climbable {
            kind: ClimbKind::Cliff,
            radius: 6.0,
            ..c
        };
        assert!(approach_climbable(
            [&cliff].into_iter(),
            Vec3::new(10.0, root_lo, -0.8), // 2.2 m from the base
            Vec2::new(0.0, -0.1),
            0.2,
            2.5
        )
        .is_some());
        // Diagonal approach inside the 60-degree cone: grabs.
        let diag = approach_climbable([&c].into_iter(), p, Vec2::new(-0.06, -0.08), 0.2, 2.5);
        assert!(diag.is_some(), "diagonal walk into the base still grabs");
    }

    #[test]
    fn approach_grab_uses_the_face_direction_for_wide_faces() {
        // A face rising towards +z: base (0, 0, 0), crest (0, 5, 6).
        let face = Climbable {
            kind: ClimbKind::Cliff,
            bottom: Vec3::new(0.0, 0.0, 0.0),
            top: Vec3::new(0.0, 5.0, 6.0),
            radius: 6.0,
        };
        assert!(face_uphill_dir(&face).is_some());
        let root_lo = AVATAR_ROOT_ABOVE_GROUND;
        // Running straight at the face, 2 m to the side of the base cell (the on-foot case
        // that failed while aiming at the base point): grabs.
        let p = Vec3::new(2.0, root_lo, -2.5);
        assert!(
            approach_climbable([&face].into_iter(), p, Vec2::new(0.0, 0.4), 1.5, 4.0).is_some(),
            "a straight run into the face grabs even beside the base cell"
        );
        // Running across (past) the face: no grab.
        assert!(
            approach_climbable([&face].into_iter(), p, Vec2::new(0.4, 0.0), 1.5, 4.0).is_none()
        );
        // Backing away: no grab.
        assert!(
            approach_climbable([&face].into_iter(), p, Vec2::new(0.0, -0.4), 1.5, 4.0).is_none()
        );
        // A vertical column (base == crest in plan) has no face direction and keeps the
        // aim-at-the-base fallback.
        let column = Climbable {
            top: Vec3::new(0.0, 5.0, 0.0),
            ..face
        };
        assert!(face_uphill_dir(&column).is_none());
        assert!(
            approach_climbable(
                [&column].into_iter(),
                Vec3::new(0.0, root_lo, -2.0),
                Vec2::new(0.0, 0.4),
                1.5,
                4.0
            )
            .is_some()
        );
    }

    #[test]
    fn climb_hint_line_names_the_prompt_of_the_moment() {
        // Attached: the up/down/let-go keys.
        assert_eq!(
            climb_hint_line(true, false, false),
            Some(CLIMB_HINT_CLIMBING),
            "climbing wins even outside the reach test"
        );
        // In reach on foot: the grab keys.
        assert_eq!(climb_hint_line(false, true, false), Some(CLIMB_HINT_GRAB));
        // In reach on Sneasler: the wall-run prompt.
        assert_eq!(
            climb_hint_line(false, true, true),
            Some(CLIMB_HINT_SNEASLER)
        );
        // Nothing near: no line at all.
        assert_eq!(climb_hint_line(false, false, false), None);
    }

    #[test]
    fn nearest_prefers_the_closer_base() {
        let a = ladder();
        let mut b = ladder();
        b.bottom.x = 11.0;
        b.top.x = 11.0;
        let p = Vec3::new(11.4, 5.9, -3.0);
        let got = nearest_climbable([&a, &b].into_iter(), p, 0.2).unwrap();
        assert_eq!(got.bottom.x, 11.0);
        let far = nearest_climbable([&a].into_iter(), Vec3::new(50.0, 5.9, -3.0), 0.2);
        assert!(far.is_none());
    }

    #[test]
    fn climb_step_clamps_to_the_surface() {
        let c = ladder();
        let (lo, hi) = c.root_range();
        // Rising: stops exactly at the top.
        let (y, b) = climb_step(&c, hi - 0.05, 1.0, 2.0, 0.1);
        assert!((y - hi).abs() < 1e-6);
        assert_eq!(b, ClimbBoundary::Top);
        // Descending: stops exactly at the bottom.
        let (y, b) = climb_step(&c, lo + 0.05, -1.0, 2.0, 0.1);
        assert!((y - lo).abs() < 1e-6);
        assert_eq!(b, ClimbBoundary::Bottom);
        // Middle: plain integration, analog input clamped.
        let (y, b) = climb_step(&c, lo + 1.0, 0.5, 2.0, 0.5);
        assert!((y - (lo + 1.5)).abs() < 1e-6);
        assert_eq!(b, ClimbBoundary::None);
        let (y, _) = climb_step(&c, lo + 1.0, 4.0, 2.0, 0.5);
        assert!((y - (lo + 2.0)).abs() < 1e-6, "input clamped to 1");
        // No movement without input.
        let (y, b) = climb_step(&c, lo + 1.0, 0.0, 2.0, 0.5);
        assert_eq!(y, lo + 1.0);
        assert_eq!(b, ClimbBoundary::None);
    }

    #[test]
    fn climb_progress_runs_zero_to_one() {
        let c = ladder();
        let (lo, hi) = c.root_range();
        assert_eq!(climb_progress(&c, lo), 0.0);
        assert_eq!(climb_progress(&c, hi), 1.0);
        assert!((climb_progress(&c, (lo + hi) * 0.5) - 0.5).abs() < 1e-6);
        assert_eq!(climb_progress(&c, lo - 5.0), 0.0);
        assert_eq!(climb_progress(&c, hi + 5.0), 1.0);
        let flat = Climbable {
            bottom: Vec3::ZERO,
            top: Vec3::ZERO,
            radius: 1.0,
            ..c
        };
        assert_eq!(
            climb_progress(&flat, 0.5),
            1.0,
            "degenerate surface is fully climbed"
        );
    }

}
