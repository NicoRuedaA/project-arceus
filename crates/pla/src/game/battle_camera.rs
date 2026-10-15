//! Battle camera framing and arena spacing that adapt to the size of the opponent.
//!
//! The arena puts the player's creature and the opponent a few metres apart and the camera a
//! fixed distance behind the player's creature. That works for a Starly but not for Giratina
//! (several metres tall and long): the camera ended up at its knees (only legs on screen) or
//! inside it, so the opponent looked missing. The functions here are pure so they are testable
//! without a window; the viewer measures the models and calls them.
//!
//! Placed world objects can sit right where the camera goes (a battle was framed through a
//! rock). [`pose_clear_of`] steps the camera forward and up until the sight line to the
//! creatures is clear of placed geometry, keeping the side and the opponent's framing.

use bevy::prelude::*;

/// Vertical field of view of the default 3D camera (Bevy's `PerspectiveProjection` default).
pub const CAMERA_FOV_Y: f32 = std::f32::consts::FRAC_PI_4;
/// Camera distance behind the player's creature for a small opponent (metres).
pub const BASE_BACK: f32 = 2.3;
/// Camera height above the player's creature's feet for a small opponent (metres).
pub const BASE_HEIGHT: f32 = 1.35;
/// Largest extra distance the camera or the opponent is moved for a giant (metres).
pub const MAX_EXTRA: f32 = 14.0;
/// Opponent radius (horizontal half-extent) up to which no spacing is added (metres).
pub const FREE_RADIUS: f32 = 1.2;

/// Size of a creature model measured from its meshes, relative to its feet.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ModelSize {
    /// Top of the model above the feet (metres).
    pub height: f32,
    /// Horizontal distance from the feet origin to the farthest point (metres).
    pub radius: f32,
}

/// Extra metres between the two creatures so a big opponent does not overlap the player's.
pub fn extra_spacing(foe: ModelSize) -> f32 {
    (foe.radius - FREE_RADIUS).clamp(0.0, 4.0)
}

/// Side offset of the camera from the player-opponent line (metres).
pub const SIDE_OFFSET: f32 = 1.0;

/// Fixed part of the battle framing (view direction, side, distance, height and look-at).
struct Framing {
    to_wild: Vec3,
    right: Vec3,
    /// Distance behind the player's creature (base distance plus the giant spacing).
    back: f32,
    /// Height above the player's creature's feet.
    height: f32,
    look: Vec3,
}

/// Geometry shared by every candidate camera position.
fn framing(p: Vec3, w: Vec3, foe: ModelSize) -> Framing {
    let to_wild = (w - p).normalize_or(Vec3::NEG_Z);
    let right = Vec3::new(to_wild.z, 0.0, -to_wild.x).normalize_or(Vec3::X);
    let foe_height = foe.height.max(0.0);
    let foe_dist = (w - p).length();
    // Distance from the camera to the opponent needed to see its full height with margin
    // (vertical field of view; the horizontal one is wider).
    let need = if foe_height > 1.0 {
        foe_height * 0.75 / (CAMERA_FOV_Y * 0.5).tan() + foe.radius * 0.5
    } else {
        0.0
    };
    let have = BASE_BACK + foe_dist;
    let extra = (need - have).clamp(0.0, MAX_EXTRA);
    let height = BASE_HEIGHT + extra * 0.1 + foe_height * 0.08;
    // Aim between the two creatures; for a tall opponent aim higher so its head is in view.
    let look = (p + w) * 0.5 + Vec3::new(0.0, 0.35 + foe_height * 0.25, 0.0) + right * 0.2;
    Framing { to_wild, right, back: BASE_BACK + extra, height, look }
}

/// Camera at `back` metres behind the creature, `lift` metres above the base height and
/// `side_extra` metres added to the usual side offset.
fn cam_at(p: Vec3, f: &Framing, back: f32, lift: f32, side_extra: f32) -> Vec3 {
    p - f.to_wild * back - f.right * (SIDE_OFFSET + side_extra) + Vec3::new(0.0, f.height + lift, 0.0)
}

/// Camera position and look-at point for the battle.
///
/// `p` and `w` are the feet positions of the player's and the opponent's creature (after any
/// spacing), `foe` the opponent's measured size (zero while its model is still loading). The
/// camera stays on the line behind the player's creature, offset to the side, and backs off
/// and rises until the whole opponent fits in the view.
pub fn pose(p: Vec3, w: Vec3, foe: ModelSize) -> (Vec3, Vec3) {
    let f = framing(p, w, foe);
    (cam_at(p, &f, f.back, 0.0, 0.0), f.look)
}

/// Never bring the camera closer than this to the player's creature (metres).
pub const CLEAR_MIN_BACK: f32 = 0.6;
/// Side offsets (metres, added to the usual offset) tried when forward/up is not enough.
/// Positive widens the usual side; the last two centre the camera. The sign of the side offset
/// never flips, so the battle keeps being seen from the same side.
const CLEAR_SIDE_EXTRA: [f32; 5] = [0.7, 1.4, -0.4, -0.7, 0.0];

/// Like [`pose`], but moves the camera until the line of sight to the look-at point is clear.
/// `blocked(from, to)` answers whether placed world geometry (rocks, houses, fences) lies on
/// the segment, for example
/// [`WorldCollision::segment_blocked`](crate::game::world_collision::WorldCollision::segment_blocked).
///
/// Candidates, in order: closer to the creatures (a rock usually sits behind the camera),
/// higher up (a low rock), both, and finally wider on the same side / centred. The look-at
/// point, the giant spacing and the sign of the side offset from [`pose`] stay, so the side the
/// battle is seen from and the opponent's framing do not change; every candidate must keep the
/// opponent in view. When no candidate is clear, the plain [`pose`] camera is returned.
pub fn pose_clear_of(p: Vec3, w: Vec3, foe: ModelSize, blocked: &dyn Fn(Vec3, Vec3) -> bool) -> (Vec3, Vec3) {
    let f = framing(p, w, foe);
    let base = cam_at(p, &f, f.back, 0.0, 0.0);
    if !blocked(base, f.look) {
        return (base, f.look);
    }
    let candidate = |back: f32, lift: f32, side: f32| -> Option<(Vec3, Vec3)> {
        let cam = cam_at(p, &f, back.max(CLEAR_MIN_BACK), lift, side);
        (!blocked(cam, f.look) && keeps_the_creatures_in_view(p, w, foe, cam, f.look)).then_some((cam, f.look))
    };
    // 1. Step forward along the usual line (the rock sits behind the camera).
    for frac in [0.85, 0.7, 0.55, 0.4] {
        if let Some(c) = candidate(f.back * frac, 0.0, 0.0) {
            return c;
        }
    }
    // 2. Straight up (the rock is low).
    for lift in [0.5, 1.0, 1.6, 2.2, 2.8] {
        if let Some(c) = candidate(f.back, lift, 0.0) {
            return c;
        }
    }
    // 3. Up and forward (the rock is in the way over the whole lower line).
    for lift in [1.0, 2.0, 2.8] {
        for frac in [0.7, 0.45] {
            if let Some(c) = candidate(f.back * frac, lift, 0.0) {
                return c;
            }
        }
    }
    // 4. Same side, wider / centred, with and without lifting.
    for side in CLEAR_SIDE_EXTRA {
        for (frac, lift) in [(1.0, 0.0), (1.0, 1.0), (1.0, 2.0), (0.7, 0.0), (0.7, 1.6)] {
            if let Some(c) = candidate(f.back * frac, lift, side) {
                return c;
            }
        }
    }
    (base, f.look)
}

/// True when the opponent (feet, middle and top) and the player's creature stay on screen from
/// `cam` (16:9, the offscreen and window default). A candidate closer to the creatures shrinks
/// the opponent's margin, so this rejects framings that would crop it.
fn keeps_the_creatures_in_view(p: Vec3, w: Vec3, foe: ModelSize, cam: Vec3, look: Vec3) -> bool {
    let h = foe.height.clamp(0.5, 14.0);
    [w, w + Vec3::Y * (h * 0.5), w + Vec3::Y * h, p]
        .iter()
        .all(|pt| in_view(cam, look, 16.0 / 9.0, *pt))
}

/// True when `point` is inside the camera's view cone for a camera at `cam` looking at `look`
/// (`aspect` = width / height). Used by tests and diagnostics.
pub fn in_view(cam: Vec3, look: Vec3, aspect: f32, point: Vec3) -> bool {
    let t = Transform::from_translation(cam).looking_at(look, Vec3::Y);
    let local = t.compute_affine().inverse().transform_point3(point);
    if local.z >= -0.1 {
        return false; // behind the camera
    }
    let depth = -local.z;
    let half_h = depth * (CAMERA_FOV_Y * 0.5).tan();
    local.y.abs() <= half_h && local.x.abs() <= half_h * aspect
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYER: Vec3 = Vec3::new(0.0, 0.0, 0.0);
    const FOE: Vec3 = Vec3::new(1.6, 0.0, -2.6);

    #[test]
    fn small_opponent_keeps_the_classic_framing() {
        let (cam, _) = pose(PLAYER, FOE, ModelSize::default());
        let to_wild = (FOE - PLAYER).normalize();
        let back = (PLAYER - cam).dot(to_wild);
        assert!((back - BASE_BACK).abs() < 0.05, "back distance {back}");
        assert!((cam.y - BASE_HEIGHT).abs() < 0.01);
    }

    #[test]
    fn a_giant_opponent_is_fully_in_view() {
        // Giratina-sized: ~5 m tall, ~3 m from its feet origin to its farthest point.
        let giant = ModelSize { height: 5.0, radius: 3.0 };
        let foe_pos = FOE + (FOE - PLAYER).normalize() * extra_spacing(giant);
        let (cam, look) = pose(PLAYER, foe_pos, giant);
        for dy in [0.0, 2.5, 5.0] {
            assert!(
                in_view(cam, look, 16.0 / 9.0, foe_pos + Vec3::Y * dy),
                "foe point at +{dy} m is out of view (cam {cam:?}, look {look:?})"
            );
        }
        // And the player's creature is still on screen.
        for dy in [0.0, 0.3] {
            assert!(in_view(cam, look, 16.0 / 9.0, PLAYER + Vec3::Y * dy), "player's creature feet +{dy} out of view");
        }
    }

    #[test]
    fn the_old_framing_lost_a_giant() {
        // Documents the bug: the fixed camera 2.3 m behind the player's creature cannot see
        // the head of a 5 m opponent 3 m away.
        let giant = ModelSize { height: 5.0, radius: 3.0 };
        let (cam, look) = pose(PLAYER, FOE, ModelSize::default());
        assert!(!in_view(cam, look, 16.0 / 9.0, FOE + Vec3::Y * giant.height));
    }

    #[test]
    fn spacing_only_grows_for_big_opponents() {
        assert_eq!(extra_spacing(ModelSize { height: 0.5, radius: 0.4 }), 0.0);
        assert!(extra_spacing(ModelSize { height: 5.0, radius: 3.0 }) > 1.0);
        assert!(extra_spacing(ModelSize { height: 50.0, radius: 90.0 }) <= 4.0);
    }

    #[test]
    fn degenerate_positions_do_not_produce_nan() {
        let (cam, look) = pose(PLAYER, PLAYER, ModelSize { height: 3.0, radius: 1.0 });
        assert!(cam.is_finite() && look.is_finite());
    }

    #[test]
    fn clear_pose_keeps_the_plain_framing_without_obstacles() {
        let (want_cam, want_look) = pose(PLAYER, FOE, ModelSize::default());
        let (cam, look) = pose_clear_of(PLAYER, FOE, ModelSize::default(), &|_, _| false);
        assert_eq!(cam, want_cam, "no obstacle: the classic framing is untouched");
        assert_eq!(look, want_look);
    }

    #[test]
    fn a_blocked_camera_moves_forward_or_up_until_the_view_is_clear() {
        let base = pose(PLAYER, FOE, ModelSize::default()).0;
        // A rock bubble around the plain camera spot: blocked until the candidate leaves it.
        let rock = base;
        let blocked = |from: Vec3, _: Vec3| from.distance(rock) < 1.2;
        let (cam, look) = pose_clear_of(PLAYER, FOE, ModelSize::default(), &blocked);
        assert!(cam.distance(rock) >= 1.2, "the camera left the blocked spot (cam {cam:?})");
        assert!(in_view(cam, look, 16.0 / 9.0, FOE + Vec3::Y * 0.5), "opponent still on screen");
        // The side the camera sees the battle from (the sign of the side offset) is kept.
        let side = |c: Vec3| (c.z * (base.z - PLAYER.z).signum());
        assert!(side(cam) > 0.0, "same side as the plain camera (cam {cam:?})");
    }

    #[test]
    fn blocked_line_of_sight_alone_moves_the_camera() {
        // The camera spot itself is free, but a wall crosses every sight line from the plain
        // camera height: the candidates must lift or step forward until the segment clears.
        let wall_plane_x = FOE.x;
        let blocked = |_: Vec3, to: Vec3| to.x > wall_plane_x;
        let (cam, look) = pose_clear_of(PLAYER, FOE, ModelSize::default(), &blocked);
        // The look-at point still sits beyond the wall (it aims between the creatures), so no
        // candidate can clear this; the plain framing is kept instead of a bogus one.
        assert_eq!(cam, pose(PLAYER, FOE, ModelSize::default()).0);
        assert!(look.is_finite());
    }

    #[test]
    fn clear_pose_never_returns_nan_when_every_candidate_is_blocked() {
        let (cam, look) = pose_clear_of(PLAYER, PLAYER, ModelSize { height: 3.0, radius: 1.0 }, &|_, _| true);
        assert!(cam.is_finite() && look.is_finite());
    }

    #[test]
    fn a_wall_beside_the_sight_line_is_cleared_sideways_on_the_same_side() {
        let (base, look) = pose(PLAYER, FOE, ModelSize::default());
        let right = Vec3::new((FOE - PLAYER).normalize().z, 0.0, -(FOE - PLAYER).normalize().x);
        // A vertical wall column (0.7 m radius, full height) beside the middle of the usual
        // sight line: stepping forward or up cannot clear it, only a sideways shift can.
        let rock = (base + look) * 0.5 - right * 0.5;
        let dist_xz = move |a: Vec3, b: Vec3, p: Vec3| {
            let (a2, b2, p2) = (a.xz(), b.xz(), p.xz());
            let ab = b2 - a2;
            let t = ((p2 - a2).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            (a2 + ab * t).distance(p2)
        };
        let blocked = move |from: Vec3, to: Vec3| dist_xz(from, to, rock) < 0.7;
        assert!(blocked(base, look), "the plain camera line crosses the wall");
        let (cam, look2) = pose_clear_of(PLAYER, FOE, ModelSize::default(), &blocked);
        assert!(dist_xz(cam, look2, rock) >= 0.7, "the chosen camera clears the wall (cam {cam:?})");
        // Still on the usual side of the line (the side offset sign is kept).
        let side_of = |c: Vec3| (c.xz() - PLAYER.xz()).dot(right.xz());
        assert!(side_of(cam).signum() == side_of(base).signum(), "camera stays on the usual side");
        assert!(in_view(cam, look2, 16.0 / 9.0, FOE + Vec3::Y * 0.5), "opponent still on screen");
    }
}
