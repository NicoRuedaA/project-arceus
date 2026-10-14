//! Battle camera framing and arena spacing that adapt to the size of the opponent.
//!
//! The arena puts the player's creature and the opponent a few metres apart and the camera a
//! fixed distance behind the player's creature. That works for a Starly but not for Giratina
//! (several metres tall and long): the camera ended up at its knees (only legs on screen) or
//! inside it, so the opponent looked missing. The functions here are pure so they are testable
//! without a window; the viewer measures the models and calls them.

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

/// Camera position and look-at point for the battle.
///
/// `p` and `w` are the feet positions of the player's and the opponent's creature (after any
/// spacing), `foe` the opponent's measured size (zero while its model is still loading). The
/// camera stays on the line behind the player's creature, offset to the side, and backs off
/// and rises until the whole opponent fits in the view.
pub fn pose(p: Vec3, w: Vec3, foe: ModelSize) -> (Vec3, Vec3) {
    let to_wild = (w - p).normalize_or(Vec3::NEG_Z);
    let right_side = Vec3::new(to_wild.z, 0.0, -to_wild.x).normalize_or(Vec3::X);
    let foe_height = foe.height.max(0.0);
    let foe_dist = (w - p).length();
    // Distance from the camera to the opponent needed to see its full height with margin
    // (vertical field of view; the horizontal one is wider).
    let need = if foe_height > 1.0 {
        foe_height * 0.62 / (CAMERA_FOV_Y * 0.5).tan() + foe.radius * 0.5
    } else {
        0.0
    };
    let have = BASE_BACK + foe_dist;
    let extra = (need - have).clamp(0.0, MAX_EXTRA);
    let height = BASE_HEIGHT + extra * 0.25 + foe_height * 0.2;
    let cam = p - to_wild * (BASE_BACK + extra) - right_side * 1.0 + Vec3::new(0.0, height, 0.0);
    // Aim between the two creatures; for a tall opponent aim higher so its head is in view.
    let look = (p + w) * 0.5 + Vec3::new(0.0, 0.35 + foe_height * 0.3, 0.0) + right_side * 0.2;
    (cam, look)
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
        assert!(in_view(cam, look, 16.0 / 9.0, PLAYER + Vec3::Y * 0.3));
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
}
