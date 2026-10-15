//! World interactables: climbable surfaces (ladders, ropes, vines, cliffs) for Brief 13.
//!
//! Placement state: **no real climbable placements are decoded**. The `bg_event` placement
//! files exist per area (`bin/field/param/placement/<area>/bg_event/bg_event.bin`, 13-40
//! records per main area) but their record semantics (event type, model hash) are not
//! decoded, and no `gimmick_*` model in `data/field_model_export_index.tsv` is a ladder,
//! rope, vine or climb surface (the 55 exported gimmick models are rocks, trees, boxes,
//! balloons, crystals, snow and `gimmick_yachi01`). The mechanic is therefore exercised
//! by the tests here plus an optional debug prop (`PLA_DEBUG_CLIMB=1`, see
//! [`spawn_debug_climbable`]) that the maintainer can look at in the viewer.
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
/// and `top.y + root`; the horizontal position is held at `bottom.xz` while climbing.
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
    let dx = p.x - c.bottom.x;
    let dz = p.z - c.bottom.z;
    if dx * dx + dz * dz > c.radius * c.radius {
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
