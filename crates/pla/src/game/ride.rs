//! Ride Pokémon (Brief 38): mounting, per-mount locomotion and stamina.
//!
//! The game decodes into [`data/ride_params.tsv`] (1,230 rows, `.tools/decode_ride.py`) and the
//! five ride bodies map through the model catalog (`data/pokemon_resource_table.tsv`):
//!
//! | Internal name | Species | Model | Role |
//! |---|---|---|---|
//! | `odoshishi` | 899 Wyrdeer | `pm1001_00_00` | ground runner |
//! | `ringuma` | 901 Ursaluna | `pm1007_00_00` | ground searcher |
//! | `basurao` | 902 Basculegion | `pm1006_11_41` | swimmer |
//! | `nyuura` | 903 Sneasler | `pm1003_00_41` | climber |
//! | `whooguru` | 628/41 Hisuian Braviary | `pm0628_00_41` | flyer (glide) |
//!
//! Decoded per-mount speeds (`control_speed_stop/walk/run/dash` in the config or its
//! `*_controller_config`, VERIFIED numbers; units PROBABLE as m/s):
//!
//! ```text
//! odoshishi   0 / 3   / 8   / 11     ride_odoshishi_config
//! basurao     0 / 5   / 8   / 11     ride_basurao_config
//! nyuura      0 / 2   / 6.2 / 8.4    ride_nyuura_controller_config (climb 4.8)
//! ringuma     0 / 2   / 5   / 6      ride_ringuma_controller_config
//! whooguru    0 / 2   / 5   / 6      ride_whooguru_controller_config (fly 9.5, swoop 16)
//! ```
//!
//! Input thresholds (`control_input_*`) decide the gait from the stick magnitude: the dash
//! threshold is 1.5 (> 1) in four mounts, so the sprint button scales the input by
//! [`SPRINT_INPUT_SCALE`] and reaches dash (PROBABLE reading). Ringuma uses 0.35/0.70/1.05.
//!
//! Saddle: the original attaches the rider to a per-mount `player_attach_joint_name` bone
//! (`parts` / `parts_01` / `parts_03` / `origin`; VERIFIED in the ride pack's skeletons).
//! The exported glTFs used by the port do not carry those bones, so the rider follows the
//! skeleton bone that matches the seat area instead ([`RiderSeat`]): each frame the rider
//! model root is the anchor bone's transform times the constant `rider_local` offset, so the
//! rider follows the mount's animation (bobbing, turning) instead of a fixed height.
//!
//! Where the rider belongs comes from the game's OWN rider clips: the `origin` bone channels
//! of `ride_<mount>_idle` (exported from the player pack) place the rider in the ride model's
//! frame — Wyrdeer 1.304 m, Ursaluna 1.760 m, Braviary -0.530 m (hanging below), Basculegion
//! and Sneasler 0 (PROBABLE: read off the exported clip, see `reports/data-progress/mounts.md`).
//! The port keeps those numbers as the starting placement and corrects the two mounts whose
//! clip carries no offset (the authored pose alone does not sit on the mount's back), measured
//! against the official art. Every correction is marked in [`RiderSeat::rider_pos`].
//!
//! Stamina: `ride_stamina_parameter.bin` decodes to a `default` record with 18000 / 3000 /
//! 20.0 and per-mount vectors whose semantics are UNKNOWN (see the report). The port uses
//! the decoded 18000 as the maximum and a simple drain model marked ASSUMED where it is not.

use bevy::prelude::*;

use super::input_map::{Action, ActionState};
use super::player_anim::{PlayerModelScene, AVATAR_ROOT_ABOVE_GROUND};
use super::pokemon_anim::PokemonAnimSource;
use super::state::{not_paused, GameState};
use super::PlayerAvatar;

// ------------------------------------------------------------------ decoded parameters

/// Maximum stamina of the `default` record of `ride_stamina_parameter.bin` (VERIFIED value;
/// meaning PROBABLE and unit UNKNOWN).
pub const STAMINA_MAX: f32 = 18000.0;

/// Dash stamina drain per second. The file's per-mount vector is `[10, 10, 10, 5, 30]`
/// (three of five values are 10) and `*_frame` values in this family are frames of a 30 Hz
/// tick (project convention), so the port reads 10 as 10 units per frame -> 300 per second.
/// PROBABLE at best: the vector's order and unit are unproven.
pub const STAMINA_DRAIN_DASH: f32 = 300.0;

/// Run drain per second: ASSUMED (dash / 3).
pub const STAMINA_DRAIN_RUN: f32 = 100.0;

/// Walk drain per second: ASSUMED (dash / 10).
pub const STAMINA_DRAIN_WALK: f32 = 30.0;

/// Recovery per second while standing (or dismounted): ASSUMED (matches the dash drain).
pub const STAMINA_REGEN: f32 = 300.0;

/// Sprint multiplies the move input; the decoded dash threshold is 1.5 in four of the five
/// mounts, so full stick + sprint reaches it (PROBABLE).
pub const SPRINT_INPUT_SCALE: f32 = 1.5;

/// Mount / dismount transition, seconds. `ride_change_config` decodes 30-frame camera
/// interpolations of type `INOUT_SINE`; at the project's assumed 30 Hz that is 1.0 s.
pub const TRANSITION_TIME: f32 = 1.0;

/// Extra camera distance while mounted (m). No base ride-camera distance is decoded (the
/// files only carry dash orbit rates): ASSUMED +60 % of the default 5 m orbit.
pub const CAMERA_BONUS: f32 = 3.0;

/// Braviary glide: horizontal speed (m/s), decoded `main_max_move_speed` = 9.5.
pub const GLIDE_SPEED: f32 = 9.5;

/// Braviary swoop (sprint held while flying): horizontal speed (m/s), decoded
/// `swoop_max_move_speed` = 16 (VERIFIED number; when the game enters the swoop is PROBABLE).
pub const SWOOP_SPEED: f32 = 16.0;

/// Braviary swoop: fall-speed cap (m/s) while sprinting airborne, so the dive gains height
/// instead of floating down at the glide cap. SUPUESTO (no decoded unit for the swoop fall
/// speed); the glide cap itself is also SUPUESTO.
pub const SWOOP_FALL_CAP: f32 = 12.0;

/// Braviary glide: fall-speed cap (m/s). SUPUESTO: the decoded `main_max_fall_speed` = 0.2
/// (unit unknown, possibly a per-frame value); the port picks a playable 2.0 m/s.
pub const GLIDE_FALL_CAP: f32 = 2.0;

/// Braviary flap (jump pressed while airborne): upward speed (m/s). The decoded
/// `control_firstJumpPower` of whooguru is 4 and the fly-speed rates
/// (`first_fly_move_speed_rate` 2, `plus_first_fly_move_speed_rate` 3.5) do not resolve to a
/// speed, so the port reuses the jump power (PROBABLE reading).
pub const FLAP_SPEED: f32 = 4.0;

/// Ursaluna search/dig: seconds the search animates before the result. The decoded
/// `dig_not_found_wait_frame_min/max` = 25 (frames of a 30 Hz tick -> 0.83 s, PROBABLE).
pub const DIG_SEARCH_TIME: f32 = 25.0 / 30.0;

// ------------------------------------------------------------------ kinds and params

/// Which ride Pokémon is being ridden (or selected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RideKind {
    Wyrdeer,
    Ursaluna,
    Basculegion,
    Sneasler,
    Braviary,
}

/// The locomotion domain a mount is built for (decoded speeds exist for each; the domain
/// comes from the game's own roles and the effect/parameter groups).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RideDomain {
    Ground,
    Water,
    Air,
}

/// Speeds and routing values of one mount. Speeds are the decoded
/// `control_speed_stop/walk/run/dash` of the mount's config (VERIFIED numbers).
#[derive(Debug, Clone, Copy)]
pub struct RideParams {
    /// Internal config name (`odoshishi`, ...).
    pub internal_name: &'static str,
    /// Species + form for the model catalog (`data/pokemon_resource_table.tsv`).
    pub dex: u16,
    pub form: u16,
    /// `player_attach_joint_name` (the saddle bone in the ride pack skeleton, VERIFIED name;
    /// the port cannot use it because its model exports do not carry that bone).
    pub attach_bone: &'static str,
    /// Where the rider rides this mount (anchor bone + placement).
    pub seat: RiderSeat,
    /// Locomotion domain of the mount.
    pub domain: RideDomain,
    /// `control_input_*`: magnitudes that separate stop / walk / run / dash.
    pub input_walk: f32,
    pub input_run: f32,
    pub input_dash: f32,
    /// `control_speed_*` (m/s).
    pub walk: f32,
    pub run: f32,
    pub dash: f32,
}

/// How the rider sits on one mount: the exported-skeleton bone the rider follows and the
/// rider's model root in that bone's REST frame.
///
/// `rest_*` is the bone's rest transform relative to the model root, read from the exported
/// skeleton (PROBABLE: same pose the glTF spawns with). `rider_pos` is the rider's model root
/// in MOUNT-ROOT space: the game's authored clip placement (`ride_<mount>_idle` `origin`
/// channel) plus the per-mount correction measured against the official art with offscreen
/// captures. All five models' roots sit at the feet on the ground (y = 0), the mount's head
/// facing +Z, so a mount-space point reads as (side, height, forward).
#[derive(Debug, Clone, Copy)]
pub struct RiderSeat {
    /// Bone of the exported mount model the rider follows (the back, between the shoulders).
    pub bone: &'static str,
    /// Bone rest translation in model space (m), from the exported skeleton.
    pub rest_pos: [f32; 3],
    /// Bone rest rotation in model space, from the exported skeleton.
    pub rest_rot: [f32; 4],
    /// Rider model root in mount space (m): authored clip placement + correction.
    pub rider_pos: [f32; 3],
    /// Rotation of the rider's model root in mount space (identity: the rider clips' own
    /// `origin` rotation carries the body tilt).
    pub rider_rot: [f32; 4],
    /// Mount-space floor (m) for the rider's model root while the mount is on the ground;
    /// `None` keeps the authored placement (used by the flyer, whose clip hangs the rider
    /// *below* the mount root and would otherwise sink under the terrain while it stands).
    pub grounded_floor: Option<f32>,
}

impl RiderSeat {
    /// Rider model root in the ANCHOR BONE's rest frame: `inverse(bone_rest) * rider_root`.
    /// This is the constant the runtime multiplies by the bone's live transform.
    pub fn rider_local(&self) -> Transform {
        let bone = Transform {
            translation: Vec3::from(self.rest_pos),
            rotation: Quat::from_array(self.rest_rot),
            scale: Vec3::ONE,
        };
        let rider = Transform {
            translation: Vec3::from(self.rider_pos),
            rotation: Quat::from_array(self.rider_rot),
            scale: Vec3::ONE,
        };
        inverse_transform(&bone).mul_transform(rider)
    }
}

/// Inverse of a scale-rotation-translation transform (`T + R * S * p`, the [`Transform`]
/// convention): `S⁻¹ * R⁻¹ * (p - T)`.
pub fn inverse_transform(t: &Transform) -> Transform {
    let inv_rot = t.rotation.inverse();
    let inv_scale = Vec3::ONE / t.scale;
    Transform {
        translation: inv_rot * (-t.translation * inv_scale),
        rotation: inv_rot,
        scale: inv_scale,
    }
}

impl RideKind {
    /// Every mount, in the order the port cycles them.
    pub const ALL: [RideKind; 5] = [
        RideKind::Wyrdeer,
        RideKind::Ursaluna,
        RideKind::Basculegion,
        RideKind::Sneasler,
        RideKind::Braviary,
    ];

    /// Decoded parameters of this mount (see the module docs for the sources).
    pub fn params(self) -> RideParams {
        match self {
            // Speeds: ride_odoshishi_config. Seat: spine_02 (the back behind the neck); the
            // authored clip places the root at y = 1.304 and the correction lowers it onto
            // the exported back (top ~1.0-1.14 m), feet hanging down the flanks.
            RideKind::Wyrdeer => RideParams {
                internal_name: "odoshishi",
                dex: 899,
                form: 0,
                attach_bone: "parts",
                seat: RiderSeat {
                    bone: "spine_02",
                    rest_pos: [0.0, 0.7232, -0.0124],
                    rest_rot: [0.3800, -0.3800, 0.5963, 0.5963],
                    rider_pos: [0.0, 1.05, -0.18],
                    rider_rot: [0.0, 0.0, 0.0, 1.0],
                    grounded_floor: None,
                },
                domain: RideDomain::Ground,
                input_walk: 0.01,
                input_run: 0.97,
                input_dash: 1.5,
                walk: 3.0,
                run: 8.0,
                dash: 11.0,
            },
            // Speeds: ride_ringuma_controller_config (0.35/0.70/1.05 thresholds).
            // Seat: spine_02 (between the shoulders); the authored clip placement (1.760)
            // sits on the hump with the feet resting on the back (VERIFIED in captures).
            RideKind::Ursaluna => RideParams {
                internal_name: "ringuma",
                dex: 901,
                form: 0,
                attach_bone: "parts_01",
                seat: RiderSeat {
                    bone: "spine_02",
                    rest_pos: [0.0, 0.8582, -0.5048],
                    rest_rot: [0.5747, 0.5747, 0.4120, -0.4120],
                    rider_pos: [0.008, 1.760, -0.348],
                    rider_rot: [0.0, 0.0, 0.0, 1.0],
                    grounded_floor: None,
                },
                domain: RideDomain::Ground,
                input_walk: 0.35,
                input_run: 0.70,
                input_dash: 1.05,
                walk: 2.0,
                run: 5.0,
                dash: 6.0,
            },
            // Speeds: ride_basurao_config. Seat: spine_01 (the body); the clip carries NO
            // offset (the game attaches the rider at this mount's root joint) and its pose
            // alone leaves the rider inside the fish, so the correction lifts the model root
            // onto the back in front of the dorsal fin (fish top ~1.06 m at z ~ +0.6).
            RideKind::Basculegion => RideParams {
                internal_name: "basurao",
                dex: 902,
                form: 0,
                attach_bone: "origin",
                seat: RiderSeat {
                    bone: "spine_01",
                    rest_pos: [0.0, 0.7100, 0.5103],
                    rest_rot: [-0.0, -0.7071, 0.0, 0.7071],
                    rider_pos: [0.0, 0.38, 0.60],
                    rider_rot: [0.0, 0.0, 0.0, 1.0],
                    grounded_floor: None,
                },
                domain: RideDomain::Water,
                input_walk: 0.01,
                input_run: 0.97,
                input_dash: 1.5,
                walk: 5.0,
                run: 8.0,
                dash: 11.0,
            },
            // Speeds: ride_nyuura_controller_config; climb_move_speed_max = 4.8.
            // Seat: spine_02; the clip carries NO offset and its perched pose already sits
            // the hips at the back height (pelvis ~1.23 m above the root), so the authored
            // placement is kept (correction measured in captures).
            RideKind::Sneasler => RideParams {
                internal_name: "nyuura",
                dex: 903,
                form: 0,
                attach_bone: "origin",
                seat: RiderSeat {
                    bone: "spine_02",
                    rest_pos: [0.0, 0.7461, -0.0],
                    rest_rot: [-0.0, -0.0, 0.7071, 0.7071],
                    rider_pos: [0.0, 0.18, -0.05],
                    rider_rot: [0.0, 0.0, 0.0, 1.0],
                    grounded_floor: None,
                },
                domain: RideDomain::Ground,
                input_walk: 0.01,
                input_run: 0.97,
                input_dash: 1.5,
                walk: 2.0,
                run: 6.2,
                dash: 8.4,
            },
            // Speeds: ride_whooguru_controller_config; air: main_max_move_speed = 9.5.
            // Seat: spine_01 (the body). The clip hangs the rider BELOW the mount root
            // (-0.530 m) in a prone, hands-forward pose: the official art shows the rider
            // hanging from a bar held by the talons. SUPUESTO: the bar itself is not in the
            // exported model, so only the hanging pose is shown.
            RideKind::Braviary => RideParams {
                internal_name: "whooguru",
                dex: 628,
                form: 41,
                attach_bone: "parts_03",
                seat: RiderSeat {
                    bone: "spine_01",
                    rest_pos: [0.0, 0.9562, 0.1093],
                    rest_rot: [-0.5531, -0.5531, -0.4405, 0.4405],
                    rider_pos: [0.083, -0.530, 0.382],
                    rider_rot: [0.0, 0.0, 0.0, 1.0],
                    grounded_floor: Some(0.30),
                },
                domain: RideDomain::Air,
                input_walk: 0.01,
                input_run: 0.97,
                input_dash: 1.5,
                walk: 2.0,
                run: 5.0,
                dash: 6.0,
            },
        }
    }

    /// Model pack stem for the port's glTF cache (`pokemon_batch` layout).
    pub fn pack_stem(self) -> &'static str {
        match self {
            RideKind::Wyrdeer => "pm1001_00_00",
            RideKind::Ursaluna => "pm1007_00_00",
            RideKind::Basculegion => "pm1006_11_41",
            RideKind::Sneasler => "pm1003_00_41",
            RideKind::Braviary => "pm0628_00_41",
        }
    }

    /// Jump take-off speed (m/s) with the mount ridden: the decoded `control_firstJumpPower`
    /// of the mount's config (VERIFIED numbers; reading them as an initial vertical speed is
    /// PROBABLE, the unit is unproven). Wyrdeer 9, Basculegion 8, Sneasler 6.5, Ursaluna 4,
    /// Braviary 4.
    pub fn jump_speed(self) -> f32 {
        match self {
            RideKind::Wyrdeer => 9.0,
            RideKind::Basculegion => 8.0,
            RideKind::Sneasler => 6.5,
            RideKind::Ursaluna => 4.0,
            RideKind::Braviary => 4.0,
        }
    }

    /// Upward speed of a flap (jump pressed while airborne) for a flying mount; `None` for
    /// the ground/water mounts (their air jump does nothing).
    pub fn flap_speed(self) -> Option<f32> {
        (self.params().domain == RideDomain::Air).then_some(FLAP_SPEED)
    }

    /// Whether this mount has the search/dig ability (Ursaluna, decoded `dig_*` parameters).
    pub fn can_dig(self) -> bool {
        self == RideKind::Ursaluna
    }
}

// ------------------------------------------------------------------ pure kinematics

/// Movement gait of a mount for a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Gait {
    Stop,
    Walk,
    Run,
    Dash,
}

impl Gait {
    /// The gait one step down (used when stamina runs out).
    pub fn slower(self) -> Gait {
        match self {
            Gait::Stop => Gait::Stop,
            Gait::Walk => Gait::Stop,
            Gait::Run => Gait::Walk,
            Gait::Dash => Gait::Run,
        }
    }
}

/// Gait for a stick magnitude and the sprint flag, from the decoded `control_input_*`
/// thresholds. `input` is the deflection (0..1 for a stick, 1 for keys); sprint scales it by
/// [`SPRINT_INPUT_SCALE`] so full-stick + sprint crosses the 1.5 dash threshold of four mounts.
pub fn gait_for(params: &RideParams, input: f32, sprint: bool) -> Gait {
    let magnitude = if sprint { input * SPRINT_INPUT_SCALE } else { input };
    if magnitude >= params.input_dash {
        Gait::Dash
    } else if magnitude >= params.input_run {
        Gait::Run
    } else if magnitude > params.input_walk {
        Gait::Walk
    } else {
        Gait::Stop
    }
}

/// Gait implied by a measured planar speed (used to drain stamina from what really moved).
pub fn gait_from_speed(params: &RideParams, speed: f32) -> Gait {
    let threshold = |a: f32, b: f32| (a + b) * 0.5;
    if speed >= threshold(params.run, params.dash) {
        Gait::Dash
    } else if speed >= threshold(params.walk, params.run) {
        Gait::Run
    } else if speed > 0.05 {
        Gait::Walk
    } else {
        Gait::Stop
    }
}

/// Speed (m/s) of a gait; `0` for [`Gait::Stop`].
pub fn speed_for(params: &RideParams, gait: Gait) -> f32 {
    match gait {
        Gait::Stop => 0.0,
        Gait::Walk => params.walk,
        Gait::Run => params.run,
        Gait::Dash => params.dash,
    }
}

/// Drain per second of a gait (units of [`STAMINA_MAX`]).
pub fn drain_for(gait: Gait) -> f32 {
    match gait {
        Gait::Stop => 0.0,
        Gait::Walk => STAMINA_DRAIN_WALK,
        Gait::Run => STAMINA_DRAIN_RUN,
        Gait::Dash => STAMINA_DRAIN_DASH,
    }
}

/// One stamina step: draining while moving, recovering while standing still or dismounted.
/// Returns the new stamina, clamped to `0..=STAMINA_MAX`.
pub fn stamina_step(stamina: f32, gait: Gait, mounted: bool, dt: f32) -> f32 {
    let rate = if !mounted { STAMINA_REGEN } else { -drain_for(gait) };
    let next = if rate >= 0.0 {
        stamina + STAMINA_REGEN * dt
    } else {
        stamina + rate * dt
    };
    next.clamp(0.0, STAMINA_MAX)
}

/// Gait capped by the remaining stamina: an empty bar drops dash to run and, below the walk
/// cost for a frame, everything to stop (the mount then simply walks, ASSUMED mechanic).
pub fn gait_with_stamina(gait: Gait, stamina: f32) -> Gait {
    if stamina <= 0.0 {
        Gait::Walk.min(gait)
    } else if stamina < STAMINA_DRAIN_DASH / 60.0 && gait == Gait::Dash {
        Gait::Run
    } else {
        gait
    }
}

/// Mount speed for a frame: `None` when not mounted, otherwise the decoded speed of the
/// stamina-capped gait. Full-stick + sprint is dash; half stick is walk (keys give full).
pub fn mounted_speed(
    kind: Option<RideKind>,
    stamina: f32,
    input: f32,
    sprint: bool,
) -> Option<f32> {
    let kind = kind?;
    let params = kind.params();
    let gait = gait_with_stamina(gait_for(&params, input, sprint), stamina);
    Some(speed_for(&params, gait))
}

/// Horizontal air speed while mounted on a glider: the decoded fly speed
/// (`main_max_move_speed`); `None` for the other mounts.
pub fn glide_speed(kind: RideKind) -> Option<f32> {
    (kind.params().domain == RideDomain::Air).then_some(GLIDE_SPEED)
}

// ------------------------------------------------------------------ state and plugin

/// Marker on the avatar while a glide-capable mount is active: the vertical motion system
/// caps the fall speed with [`GlideMount::fall_cap`].
#[derive(Component, Debug, Clone, Copy)]
pub struct GlideMount {
    pub fall_cap: f32,
}

/// Marker on the spawned mount view entity (a child of the avatar).
#[derive(Component, Debug, Clone, Copy)]
pub struct RideMountView {
    pub kind: RideKind,
}

/// Seat binding of one mount view: the exported-skeleton bone the rider follows. Added once
/// the bone exists in the spawned scene (assets load asynchronously); `seat_rider` places
/// every [`PlayerModelScene`] at that bone's live transform times the constant offset.
#[derive(Component, Debug, Clone, Copy)]
pub struct MountSeat {
    pub kind: RideKind,
    pub bone: Entity,
    /// Rider model root in the bone's rest frame (see [`RiderSeat::rider_local`]).
    pub local: Transform,
}

/// The rider's transform for one frame: the anchor bone's live transform relative to the
/// mount view, times the constant bone-frame offset, expressed in the avatar's frame.
/// `mount_tf` is the mount view's own local transform (feet on the ground).
pub fn rider_attached_transform(mount_tf: &Transform, bone_rel: &Transform, local: &Transform) -> Transform {
    mount_tf.mul_transform(*bone_rel).mul_transform(*local)
}

/// The on-foot transform of the player model child (its feet on the ground).
pub fn rider_on_foot_transform() -> Transform {
    Transform::from_xyz(0.0, -AVATAR_ROOT_ABOVE_GROUND, 0.0)
}

/// Eases the rider between the on-foot transform (`eased` 0) and the attached one (1).
pub fn blend_rider_transform(on_foot: &Transform, attached: &Transform, eased: f32) -> Transform {
    let t = eased.clamp(0.0, 1.0);
    Transform {
        translation: on_foot.translation.lerp(attached.translation, t),
        rotation: on_foot.rotation.slerp(attached.rotation, t),
        scale: on_foot.scale.lerp(attached.scale, t),
    }
}

/// The ride state of the player. The viewer reads [`RideState::mounted_speed`] for its
/// locomotion and [`RideState::camera_bonus`] for the camera pull-back.
#[derive(Resource, Debug, Clone)]
pub struct RideState {
    /// Currently ridden mount (`None` on foot).
    pub mounted: Option<RideKind>,
    /// Mount that [`Action::Ride`] will mount next (cycled by [`Action::RideCycle`]).
    pub selected: RideKind,
    /// Current stamina (units of [`STAMINA_MAX`]).
    pub stamina: f32,
    /// 0 -> 1 transition progress of the last mount/dismount (see [`TRANSITION_TIME`]).
    pub transition: f32,
    /// Direction of the current transition: +1 mounting, -1 dismounting, 0 settled.
    pub transition_dir: f32,
    /// Seconds left of an Ursaluna search/dig ([`Action::RideAbility`]); 0 when not digging.
    /// While it runs the mount stands still (the original stops to dig: PROBABLE).
    pub dig_timer: f32,
}

impl Default for RideState {
    fn default() -> Self {
        Self {
            mounted: None,
            selected: RideKind::Wyrdeer,
            stamina: STAMINA_MAX,
            transition: 0.0,
            transition_dir: 0.0,
            dig_timer: 0.0,
        }
    }
}

impl RideState {
    /// Whether a mount is currently ridden.
    pub fn is_mounted(&self) -> bool {
        self.mounted.is_some()
    }

    /// Speed for this frame's input (see [`mounted_speed`]); `None` on foot. Standing still
    /// while an Ursaluna search/dig runs.
    pub fn mounted_speed(&self, input: f32, sprint: bool) -> Option<f32> {
        if self.dig_timer > 0.0 && self.mounted.is_some() {
            return Some(0.0);
        }
        mounted_speed(self.mounted, self.stamina, input, sprint)
    }

    /// Glide horizontal speed while in the air on a flyer; `None` otherwise.
    pub fn air_speed(&self) -> Option<f32> {
        self.air_speed_input(false)
    }

    /// Horizontal speed in the air with the sprint input: a flyer swoops at the decoded
    /// `swoop_max_move_speed` while sprinting, glides at `main_max_move_speed` otherwise.
    pub fn air_speed_input(&self, sprint: bool) -> Option<f32> {
        self.mounted.and_then(|k| {
            glide_speed(k).map(|glide| if sprint { SWOOP_SPEED } else { glide })
        })
    }

    /// Fall-speed cap while mounted on a flyer: the swoop cap while sprinting (dive), the
    /// glide cap otherwise.
    pub fn fall_cap_input(&self, sprint: bool) -> Option<f32> {
        self.mounted
            .filter(|k| k.params().domain == RideDomain::Air)
            .map(|_| if sprint { SWOOP_FALL_CAP } else { GLIDE_FALL_CAP })
    }

    /// Fall-speed cap while mounted on a flyer (glide tier).
    pub fn fall_cap(&self) -> Option<f32> {
        self.fall_cap_input(false)
    }

    /// True while an Ursaluna search/dig runs.
    pub fn digging(&self) -> bool {
        self.dig_timer > 0.0
    }

    /// Starts an Ursaluna search when the mount can dig; returns `true` when it started.
    pub fn begin_dig(&mut self) -> bool {
        if self.mounted.is_some_and(|k| k.can_dig()) && self.dig_timer <= 0.0 {
            self.dig_timer = DIG_SEARCH_TIME;
            true
        } else {
            false
        }
    }

    /// Advances the dig timer; returns `true` on the frame the search finishes.
    pub fn step_dig(&mut self, dt: f32) -> bool {
        if self.dig_timer <= 0.0 {
            return false;
        }
        self.dig_timer = (self.dig_timer - dt).max(0.0);
        self.dig_timer == 0.0
    }

    /// Camera pull-back in metres, eased by the mount/dismount transition (`INOUT_SINE`,
    /// the decoded interpolation type). 0 when fully dismounted.
    pub fn camera_bonus(&self) -> f32 {
        let eased = smoothstep(self.transition.clamp(0.0, 1.0));
        CAMERA_BONUS * eased
    }

    /// Mount whose view must exist this frame: the ridden one, or the mount being left while
    /// the dismount transition still runs (so the rider eases down off the saddle instead of
    /// snapping to the ground when the body disappears).
    pub fn view_target(&self) -> Option<RideKind> {
        if self.mounted.is_some() {
            self.mounted
        } else if self.transition > 0.0 {
            Some(self.selected)
        } else {
            None
        }
    }

    /// Transition easing (0 on foot, 1 fully seated) shared by the camera and the seat.
    pub fn seat_ease(&self) -> f32 {
        smoothstep(self.transition.clamp(0.0, 1.0))
    }

    /// Advances the mount/dismount transition; returns `true` when it changed this frame.
    pub fn step_transition(&mut self, dt: f32) -> bool {
        if self.transition_dir == 0.0 {
            return false;
        }
        let before = self.transition;
        self.transition = (self.transition + self.transition_dir * dt / TRANSITION_TIME)
            .clamp(0.0, 1.0);
        if (self.transition == 1.0 && self.transition_dir > 0.0)
            || (self.transition == 0.0 && self.transition_dir < 0.0)
        {
            self.transition_dir = 0.0;
        }
        self.transition != before
    }

    /// Toggles the mount on/off (called on [`Action::Ride`]).
    pub fn toggle(&mut self) {
        if self.is_mounted() {
            self.mounted = None;
            self.transition_dir = -1.0;
        } else {
            self.mounted = Some(self.selected);
            self.transition_dir = 1.0;
        }
    }

    /// Cycles the selected mount; while mounted the ridden one changes with it.
    pub fn cycle(&mut self) {
        let idx = RideKind::ALL
            .iter()
            .position(|k| *k == self.selected)
            .unwrap_or(0);
        self.selected = RideKind::ALL[(idx + 1) % RideKind::ALL.len()];
        if self.is_mounted() {
            self.mounted = Some(self.selected);
        }
    }
}

/// Smoothstep easing (the `INOUT_SINE`-style shape of the decoded interpolations).
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Controls line of a mount, the single source of the on-screen ride help (the persistent
/// hint line of `ride_hints` shows it after mounting). Every line carries the label, the run
/// key, the ability key of the mount and the change/dismount keys.
///
/// Separators are `|`, not the middle dot or em dash: the default UI font has no glyph for
/// those and they render as boxes (seen in the automation captures).
pub fn mount_controls_line(kind: RideKind) -> String {
    match kind {
        RideKind::Wyrdeer => {
            "Wyrdeer - Correr: Shift | Cambiar montura: G | Desmontar: V".to_string()
        }
        RideKind::Ursaluna => {
            "Ursaluna - Correr: Shift | Buscar/cavar: H | Cambiar montura: G | Desmontar: V"
                .to_string()
        }
        RideKind::Basculegion => {
            "Basculegion - Nada rapido en el agua | Correr: Shift | Cambiar montura: G | Desmontar: V"
                .to_string()
        }
        RideKind::Sneasler => {
            "Sneasler - Correr: Shift | Trepar: camina contra una pared | Cambiar montura: G | Desmontar: V"
                .to_string()
        }
        RideKind::Braviary => {
            "Braviary - Salta con K: puedes planear | Picado: Shift en el aire | Cambiar montura: G | Desmontar: V"
                .to_string()
        }
    }
}

/// Reads the ride keys, toggling / cycling the mount, running the ability, advancing the
/// transition and draining stamina from the speed the avatar really moved at this frame.
fn ride_input(
    time: Res<Time>,
    input: Res<ActionState>,
    avatar: Query<&Transform, With<PlayerAvatar>>,
    mut ride: ResMut<RideState>,
    mut toast: Option<ResMut<super::save_wire::SaveToast>>,
    mut last_position: Local<Option<Vec3>>,
) {
    if input.just_pressed(Action::Ride) {
        ride.toggle();
        // No toast here: the persistent on-screen controls line of `ride_hints` shows
        // `mount_controls_line` on every mount change (it replaced the 8 s toast of the
        // climb-discoverability fix so the keys are not shown twice at once).
    }
    if input.just_pressed(Action::RideCycle) {
        ride.cycle();
    }
    let dt = time.delta_secs();
    // Ursaluna's search/dig (its ride ability). The loot tables are not decoded, so the
    // result is always "nothing found" until they are: the toast says so.
    if input.just_pressed(Action::RideAbility) && ride.begin_dig() {
        if let Some(t) = toast.as_deref_mut() {
            t.show("Ursaluna cava buscando algo bajo tierra...");
        }
    }
    if ride.step_dig(dt) {
        if let Some(t) = toast.as_deref_mut() {
            t.show("Ursaluna no encuentra nada (objetos enterrados sin decodificar).");
        }
    }
    ride.step_transition(dt);

    // Measured planar speed of the avatar; a long step is a teleport (zone change, save
    // load), not locomotion, exactly as `player_anim` / `pokemon_anim` treat it.
    let mut measured = 0.0;
    if let Ok(tf) = avatar.single() {
        if let Some(last) = *last_position {
            let step = Vec2::new(tf.translation.x - last.x, tf.translation.z - last.z).length()
                / dt.max(1e-4);
            measured = if step > TELEPORT_SPEED { 0.0 } else { step };
        }
        *last_position = Some(tf.translation);
    }
    let gait = ride
        .mounted
        .map(|kind| gait_from_speed(&kind.params(), measured))
        .unwrap_or(Gait::Stop);
    ride.stamina = stamina_step(ride.stamina, gait, ride.is_mounted(), dt.min(0.1));
}

/// A per-frame planar speed above this is a teleport, not locomotion (m/s).
const TELEPORT_SPEED: f32 = 25.0;

/// Keeps the mount view entity in sync with the ride state; the rider's seat position itself
/// is placed by [`seat_rider`].
#[allow(clippy::too_many_arguments)]
fn sync_mount_view(
    mut commands: Commands,
    asset_server: Option<Res<AssetServer>>,
    ride: Res<RideState>,
    input: Res<ActionState>,
    avatar: Query<Entity, With<PlayerAvatar>>,
    views: Query<(Entity, &RideMountView)>,
    glide: Query<'_, '_, (Entity, &'static GlideMount)>,
    mut warned: Local<Option<RideKind>>,
) {
    let Ok(avatar_e) = avatar.single() else {
        return;
    };
    // Spawn / respawn / despawn the mount view (a child of the avatar, feet at its root).
    // While the dismount transition runs the view of the mount being left stays alive, so the
    // rider eases down off the saddle instead of snapping to the ground.
    let want = ride.view_target();
    let stale = views.iter().any(|(_, v)| Some(v.kind) != want);
    if stale {
        for (e, _) in &views {
            commands.entity(e).despawn();
        }
    }
    let need_spawn = want.is_some() && (stale || views.is_empty());
    if let (true, Some(kind)) = (need_spawn, want) {
        // Without an asset server (headless tests) the logic still runs, body-less.
        if let Some(asset_server) = asset_server.as_deref() {
            let stem = kind.pack_stem();
            match super::pokemon_resource::batch_gltf_rel_for_stem(stem, &["idle", "static"]) {
                Some(rel) => {
                    commands.entity(avatar_e).with_children(|p| {
                        p.spawn((
                            RideMountView { kind },
                            PokemonAnimSource::from_idle_path(asset_server, &rel),
                            Transform::from_xyz(0.0, -AVATAR_ROOT_ABOVE_GROUND, 0.0),
                            Visibility::default(),
                        ));
                    });
                }
                None if *warned != Some(kind) => {
                    *warned = Some(kind);
                    eprintln!(
                        "ride: no exported model for {kind:?} ({stem}); riding without a body"
                    );
                }
                None => {}
            }
        }
    }
    // Glide marker on the avatar (flyers only), inserted and removed only when it changes.
    // Sprinting while airborne swoops: the cap rises so the dive actually descends.
    match ride.fall_cap_input(input.pressed(Action::Sprint)) {
        Some(cap) if glide.is_empty() => {
            commands.entity(avatar_e).insert(GlideMount { fall_cap: cap });
        }
        Some(cap) => {
            for (e, g) in &glide {
                if g.fall_cap != cap {
                    commands.entity(e).insert(GlideMount { fall_cap: cap });
                }
            }
        }
        None => {
            for (e, _) in &glide {
                commands.entity(e).remove::<GlideMount>();
            }
        }
    }
}

/// Finds the anchor bone of every mount view whose scene has spawned and binds [`MountSeat`].
/// Runs until the asset is loaded; a mount whose export lacks the bone simply gets no seat
/// (the rider then stays at the on-foot transform).
fn find_mount_seat(
    mut commands: Commands,
    views: Query<(Entity, &RideMountView), Without<MountSeat>>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    for (view, mount) in &views {
        let bone = mount.kind.params().seat.bone;
        let Some(found) = children
            .iter_descendants(view)
            .find(|d| names.get(*d).is_ok_and(|n| n.as_str() == bone))
        else {
            continue;
        };
        commands.entity(view).insert(MountSeat {
            kind: mount.kind,
            bone: found,
            local: mount.kind.params().seat.rider_local(),
        });
    }
}

/// Places every player-model scene on the saddle: the model root is the mount's anchor bone
/// (live, animated transform relative to the mount view) times the constant bone-frame offset,
/// eased from the on-foot transform by the mount/dismount transition. Runs after the
/// animation system wrote the bones and before transforms propagate, so the seat follows the
/// mount's animation exactly on the frame it moves.
fn seat_rider(
    ride: Res<RideState>,
    flight: Query<&super::player_motion::PlayerMotionState, With<PlayerAvatar>>,
    seats: Query<(Entity, &MountSeat, &Transform), Without<PlayerModelScene>>,
    parents: Query<&ChildOf>,
    bones: Query<&Transform, (Without<PlayerModelScene>, Without<MountSeat>)>,
    mut scenes: Query<&mut Transform, (With<PlayerModelScene>, Without<MountSeat>)>,
) {
    let on_foot = rider_on_foot_transform();
    let eased = ride.seat_ease();
    let airborne = flight
        .iter()
        .any(|m| m.mode == super::player_motion::MotionMode::Airborne);
    let mut target = on_foot;
    if eased > 0.0 {
        if let Some((view, seat, mount_tf)) = seats.iter().next() {
            // The bone's transform relative to the mount view: compose the locals up to (and
            // excluding) the view entity.
            if let Ok(bone_tf) = bones.get(seat.bone) {
                let mut rel = *bone_tf;
                let mut cur = seat.bone;
                while let Ok(child_of) = parents.get(cur) {
                    let parent = child_of.parent();
                    if parent == view {
                        break;
                    }
                    let Ok(parent_tf) = bones.get(parent) else { break };
                    rel = parent_tf.mul_transform(rel);
                    cur = parent;
                }
                // Mount-space placement of the rider's model root (the anchor bone's live
                // transform times the constant bone-frame offset), then the floor of mounts
                // whose clip hangs the rider below their root while they stand on the ground.
                // The mount view itself is a plain offset (no rotation), so composing in this
                // order keeps the clamp in the mount's own vertical axis.
                let mut placed = rider_attached_transform(&Transform::IDENTITY, &rel, &seat.local);
                if let Some(floor) = seat.kind.params().seat.grounded_floor {
                    if !airborne && placed.translation.y < floor {
                        placed.translation.y = floor;
                    }
                }
                target = blend_rider_transform(
                    &on_foot,
                    &rider_attached_transform(mount_tf, &placed, &Transform::IDENTITY),
                    eased,
                );
            }
        }
    }
    for mut tf in &mut scenes {
        if *tf != target {
            *tf = target;
        }
    }
}

/// Systems of [`RidePlugin`] (mount input, then view sync). Other modules that must see the
/// ride state of the current frame (the HUD hints) order `.after(RideSystems)`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RideSystems;

/// Registers the ride systems. Add with `app.add_plugins(RidePlugin)`.
pub struct RidePlugin;

impl Plugin for RidePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RideState>();
        app.add_systems(
            Update,
            (ride_input, sync_mount_view, find_mount_seat)
                .chain()
                .in_set(RideSystems)
                .run_if(in_state(GameState::World))
                .run_if(not_paused),
        );
        // The seat follows the mount's animation: after the animation system wrote the bone
        // transforms and before they propagate (same slot as the player's root-motion pin).
        app.add_systems(
            PostUpdate,
            seat_rider
                .after(bevy::app::AnimationSystems)
                .before(bevy::transform::TransformSystems::Propagate),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gait_thresholds_come_from_the_decoded_inputs() {
        let w = RideKind::Wyrdeer.params();
        assert_eq!(gait_for(&w, 0.0, false), Gait::Stop);
        assert_eq!(gait_for(&w, 0.5, false), Gait::Walk);
        assert_eq!(gait_for(&w, 1.0, false), Gait::Run);
        assert_eq!(gait_for(&w, 1.0, true), Gait::Dash, "sprint reaches the 1.5 threshold");
        assert_eq!(gait_for(&w, 1.5, false), Gait::Dash);
        // Ringuma's tighter thresholds (0.35 / 0.70 / 1.05).
        let r = RideKind::Ursaluna.params();
        assert_eq!(gait_for(&r, 0.2, false), Gait::Stop);
        assert_eq!(gait_for(&r, 0.5, false), Gait::Walk);
        assert_eq!(gait_for(&r, 0.8, false), Gait::Run);
        assert_eq!(gait_for(&r, 0.8, true), Gait::Dash);
    }

    #[test]
    fn speeds_match_the_decoded_table() {
        let kinds = [
            (RideKind::Wyrdeer, 3.0, 8.0, 11.0),
            (RideKind::Ursaluna, 2.0, 5.0, 6.0),
            (RideKind::Basculegion, 5.0, 8.0, 11.0),
            (RideKind::Sneasler, 2.0, 6.2, 8.4),
            (RideKind::Braviary, 2.0, 5.0, 6.0),
        ];
        for (kind, walk, run, dash) in kinds {
            let p = kind.params();
            assert_eq!((p.walk, p.run, p.dash), (walk, run, dash), "{kind:?}");
            assert_eq!(speed_for(&p, Gait::Stop), 0.0);
        }
    }

    #[test]
    fn mounted_speed_only_exists_when_mounted() {
        assert_eq!(mounted_speed(None, STAMINA_MAX, 1.0, false), None);
        let s = mounted_speed(Some(RideKind::Wyrdeer), STAMINA_MAX, 1.0, false);
        assert_eq!(s, Some(8.0));
        let s = mounted_speed(Some(RideKind::Wyrdeer), STAMINA_MAX, 0.5, false);
        assert_eq!(s, Some(3.0), "half stick walks");
        let s = mounted_speed(Some(RideKind::Wyrdeer), STAMINA_MAX, 1.0, true);
        assert_eq!(s, Some(11.0), "sprint dashes");
    }

    #[test]
    fn stamina_drains_with_the_gait_and_recovers_standing() {
        let mut stamina = STAMINA_MAX;
        // One second of dash: 300 units.
        stamina = stamina_step(stamina, Gait::Dash, true, 1.0);
        assert!((stamina - (STAMINA_MAX - STAMINA_DRAIN_DASH)).abs() < 1e-3);
        // Ten seconds standing mounted: full again (regen matches the dash drain).
        stamina = stamina_step(stamina, Gait::Stop, true, 10.0);
        assert_eq!(stamina, STAMINA_MAX);
        // Dismounted never drains.
        assert_eq!(stamina_step(stamina, Gait::Dash, false, 1.0), STAMINA_MAX);
    }

    #[test]
    fn empty_stamina_forces_the_mount_to_walk() {
        assert_eq!(gait_with_stamina(Gait::Dash, 0.0), Gait::Walk);
        assert_eq!(gait_with_stamina(Gait::Run, 0.0), Gait::Walk);
        assert_eq!(gait_with_stamina(Gait::Dash, STAMINA_MAX), Gait::Dash);
        // Below a frame of dash: dash falls back to run, run still works.
        assert_eq!(gait_with_stamina(Gait::Dash, 1.0), Gait::Run);
        assert_eq!(gait_with_stamina(Gait::Run, 1.0), Gait::Run);
        // Stamina never goes below zero.
        assert_eq!(stamina_step(1.0, Gait::Dash, true, 1.0), 0.0);
    }

    #[test]
    fn toggle_and_cycle_follow_the_selected_mount() {
        let mut st = RideState::default();
        assert!(!st.is_mounted());
        assert_eq!(st.selected, RideKind::Wyrdeer);
        st.toggle();
        assert_eq!(st.mounted, Some(RideKind::Wyrdeer));
        st.cycle();
        assert_eq!(st.selected, RideKind::Ursaluna);
        assert_eq!(st.mounted, Some(RideKind::Ursaluna), "swaps while mounted");
        st.toggle();
        assert!(!st.is_mounted());
        // One full cycle comes back to the start.
        for _ in 0..RideKind::ALL.len() {
            st.cycle();
        }
        assert_eq!(st.selected, RideKind::Ursaluna);
    }

    #[test]
    fn transition_eases_the_camera_and_keeps_the_mount_view_alive() {
        let mut st = RideState::default();
        assert!((st.camera_bonus()).abs() < 1e-6);
        assert_eq!(st.view_target(), None, "no mount view on foot");
        st.toggle();
        assert_eq!(st.transition_dir, 1.0);
        assert_eq!(st.view_target(), Some(RideKind::Wyrdeer));
        // Half way: smoothstep(0.5) = 0.5 exactly.
        st.transition = 0.5;
        assert!((st.camera_bonus() - CAMERA_BONUS * 0.5).abs() < 1e-4);
        assert!((st.seat_ease() - 0.5).abs() < 1e-4);
        // Reaches the saddle after TRANSITION_TIME seconds.
        let mut t = 0.0;
        while t < TRANSITION_TIME + 0.01 {
            st.step_transition(1.0 / 60.0);
            t += 1.0 / 60.0;
        }
        assert_eq!(st.transition, 1.0);
        assert_eq!(st.transition_dir, 0.0);
        assert!((st.camera_bonus() - CAMERA_BONUS).abs() < 1e-4);
        assert!((st.seat_ease() - 1.0).abs() < 1e-4);
        // Dismounting: the view of the mount being left stays alive while the transition runs.
        st.toggle();
        assert_eq!(st.view_target(), Some(RideKind::Wyrdeer));
        let mut t = 0.0;
        while t < TRANSITION_TIME + 0.01 {
            st.step_transition(1.0 / 60.0);
            t += 1.0 / 60.0;
        }
        assert_eq!(st.transition, 0.0);
        assert_eq!(st.view_target(), None);
    }

    #[test]
    fn the_bone_frame_offset_reproduces_the_authored_placement() {
        // `rider_local` is folded so that bone_rest * rider_local == rider_pos for every mount:
        // the runtime multiplies the live bone transform by `rider_local`, so at rest the rider
        // must land exactly where the table says.
        for kind in RideKind::ALL {
            let seat = kind.params().seat;
            let bone = Transform {
                translation: Vec3::from(seat.rest_pos),
                rotation: Quat::from_array(seat.rest_rot),
                scale: Vec3::ONE,
            };
            let placed = bone.mul_transform(seat.rider_local());
            let want = Vec3::from(seat.rider_pos);
            assert!(
                (placed.translation - want).length() < 1e-3,
                "{kind:?}: {} vs {}",
                placed.translation,
                want
            );
            assert!((placed.rotation.dot(Quat::from_array(seat.rider_rot))).abs() > 0.9999, "{kind:?}");
        }
    }

    #[test]
    fn the_blend_eases_between_the_ground_and_the_saddle() {
        let on_foot = rider_on_foot_transform();
        let attached = Transform::from_xyz(0.0, 1.05, -0.06);
        assert_eq!(blend_rider_transform(&on_foot, &attached, 0.0), on_foot);
        assert_eq!(blend_rider_transform(&on_foot, &attached, 1.0), attached);
        let mid = blend_rider_transform(&on_foot, &attached, 0.5);
        assert!((mid.translation.y - (on_foot.translation.y + 1.05) * 0.5).abs() < 1e-4);
    }

    #[test]
    fn seat_rider_follows_the_anchor_bone() {
        use crate::game::input_map::{actions, RawInput};
        use bevy::state::app::StatesPlugin;
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            1.0 / 60.0,
        )));
        app.init_state::<GameState>();
        app.add_plugins(RidePlugin);

        let avatar = app.world_mut().spawn((PlayerAvatar, Transform::default())).id();
        let scene = app.world_mut().spawn((PlayerModelScene, rider_on_foot_transform())).id();
        app.world_mut().entity_mut(avatar).add_child(scene);

        // A synthetic mount: view at the feet-on-ground offset, a fake root chain whose final
        // link is the anchor bone (named like the exported skeleton's).
        let seat = RideKind::Wyrdeer.params().seat;
        let view = app
            .world_mut()
            .spawn((
                RideMountView { kind: RideKind::Wyrdeer },
                Transform::from_xyz(0.0, -AVATAR_ROOT_ABOVE_GROUND, 0.0),
            ))
            .id();
        app.world_mut().entity_mut(avatar).add_child(view);
        let chain = app
            .world_mut()
            .spawn((
                Transform {
                    translation: Vec3::from(seat.rest_pos),
                    rotation: Quat::from_array(seat.rest_rot),
                    scale: Vec3::ONE,
                },
                Name::new(seat.bone),
            ))
            .id();
        app.world_mut().entity_mut(view).add_child(chain);
        app.world_mut().entity_mut(view).insert(MountSeat {
            kind: RideKind::Wyrdeer,
            bone: chain,
            local: seat.rider_local(),
        });

        // V mounts; the rider eases from the ground onto the bone over the transition.
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyV);
        app.insert_resource(actions(&RawInput::from_keyboard(&keys)));
        app.update();
        for _ in 0..(TRANSITION_TIME * 60.0) as i32 + 2 {
            app.insert_resource(actions(&RawInput::default()));
            app.update();
        }
        let t = *app.world().get::<Transform>(scene).unwrap();
        let want = Vec3::from(seat.rider_pos) + Vec3::new(0.0, -AVATAR_ROOT_ABOVE_GROUND, 0.0);
        assert!((t.translation - want).length() < 1e-3, "seat {:?} vs {want:?}", t.translation);

        // The bone moving (the mount's animation) carries the rider with it: a bone local
        // translation delta moves the rider by exactly that delta in mount space.
        app.world_mut().get_mut::<Transform>(chain).unwrap().translation.y += 0.25;
        app.insert_resource(actions(&RawInput::default()));
        app.update();
        let moved = app.world().get::<Transform>(scene).unwrap().translation;
        let expected = want + Vec3::new(0.0, 0.25, 0.0);
        assert!(
            (moved - expected).length() < 1e-3,
            "rider did not follow the bone: {moved:?} vs {expected:?}"
        );
    }

    #[test]
    fn a_bone_displacement_carries_the_rider_by_the_same_amount() {
        // What a spine bob does at runtime: the bone's transform gains a parent-space offset.
        // The rider must follow rigidly, anchored at the bone.
        let mount = Transform::from_xyz(0.0, -AVATAR_ROOT_ABOVE_GROUND, 0.0);
        for kind in RideKind::ALL {
            let seat = kind.params().seat;
            let local = seat.rider_local();
            let rest = Transform {
                translation: Vec3::from(seat.rest_pos),
                rotation: Quat::from_array(seat.rest_rot),
                scale: Vec3::ONE,
            };
            let shift = Vec3::new(0.02, 0.12, -0.05);
            let shifted = Transform::from_translation(shift).mul_transform(rest);
            let at_rest = rider_attached_transform(&mount, &rest, &local);
            let moved = rider_attached_transform(&mount, &shifted, &local);
            assert!(
                (moved.translation - at_rest.translation - shift).length() < 1e-4,
                "{kind:?}: {:?} vs {:?}",
                moved.translation,
                at_rest.translation
            );
        }
    }

    #[test]
    fn only_the_flyer_glides() {
        assert_eq!(glide_speed(RideKind::Braviary), Some(GLIDE_SPEED));
        assert_eq!(glide_speed(RideKind::Wyrdeer), None);
        let mut st = RideState { mounted: Some(RideKind::Braviary), ..default() };
        assert_eq!(st.air_speed(), Some(GLIDE_SPEED));
        assert_eq!(st.fall_cap(), Some(GLIDE_FALL_CAP));
        st.mounted = Some(RideKind::Sneasler);
        assert_eq!(st.air_speed(), None);
        assert_eq!(st.fall_cap(), None);
    }

    #[test]
    fn the_flyer_swoops_while_sprinting() {
        let st = RideState { mounted: Some(RideKind::Braviary), ..default() };
        assert_eq!(st.air_speed_input(false), Some(GLIDE_SPEED));
        assert_eq!(st.air_speed_input(true), Some(SWOOP_SPEED));
        assert_eq!(st.fall_cap_input(false), Some(GLIDE_FALL_CAP));
        assert_eq!(st.fall_cap_input(true), Some(SWOOP_FALL_CAP));
        // Ground mounts have no air tier whichever the input.
        let st = RideState { mounted: Some(RideKind::Wyrdeer), ..default() };
        assert_eq!(st.air_speed_input(true), None);
        assert_eq!(st.fall_cap_input(true), None);
    }

    #[test]
    fn jump_speeds_are_the_decoded_first_jump_powers() {
        assert_eq!(RideKind::Wyrdeer.jump_speed(), 9.0);
        assert_eq!(RideKind::Basculegion.jump_speed(), 8.0);
        assert_eq!(RideKind::Sneasler.jump_speed(), 6.5);
        assert_eq!(RideKind::Ursaluna.jump_speed(), 4.0);
        assert_eq!(RideKind::Braviary.jump_speed(), 4.0);
        // Every mount jumps higher than the on-foot avatar (3.25 m/s).
        for kind in RideKind::ALL {
            assert!(kind.jump_speed() > 3.25, "{kind:?}");
        }
        // Only the flyer flaps.
        assert_eq!(RideKind::Braviary.flap_speed(), Some(FLAP_SPEED));
        assert_eq!(RideKind::Wyrdeer.flap_speed(), None);
        assert_eq!(RideKind::Basculegion.flap_speed(), None);
    }

    #[test]
    fn only_ursaluna_digs_and_the_mount_stands_while_digging() {
        let mut st = RideState { mounted: Some(RideKind::Wyrdeer), ..default() };
        assert!(!st.begin_dig(), "Wyrdeer has no dig");
        st.mounted = Some(RideKind::Ursaluna);
        assert!(st.begin_dig());
        assert!(st.digging());
        assert_eq!(st.mounted_speed(1.0, true), Some(0.0), "stands while digging");
        // Two seconds later the search finishes (0.83 s) and movement resumes.
        assert!(st.step_dig(2.0));
        assert!(!st.digging());
        assert_eq!(st.mounted_speed(1.0, false), Some(5.0));
    }

    #[test]
    fn headless_plugin_mounts_dismounts_and_seats_the_rider() {
        use crate::game::input_map::{actions, RawInput};
        use bevy::state::app::StatesPlugin;
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            1.0 / 60.0,
        )));
        app.init_state::<GameState>();
        app.add_plugins(RidePlugin);

        let avatar = app
            .world_mut()
            .spawn((PlayerAvatar, Transform::default()))
            .id();
        let scene = app
            .world_mut()
            .spawn((
                PlayerModelScene,
                Transform::from_xyz(0.0, -AVATAR_ROOT_ABOVE_GROUND, 0.0),
            ))
            .id();
        app.world_mut().entity_mut(avatar).add_child(scene);

        // Frame 1: V pressed -> mounts the selected Wyrdeer.
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyV);
        app.insert_resource(actions(&RawInput::from_keyboard(&keys)));
        app.update();
        assert_eq!(
            app.world().resource::<RideState>().mounted,
            Some(RideKind::Wyrdeer)
        );

        // Idle frames: stays mounted; the camera eases back. Without an asset server no mount
        // body spawns, so there is no seat bone and the rider stays at the on-foot transform
        // (the same fallback as an export without the bone).
        for _ in 0..(TRANSITION_TIME * 60.0) as i32 + 2 {
            app.insert_resource(actions(&RawInput::default()));
            app.update();
        }
        let seat_y = app.world().get::<Transform>(scene).unwrap().translation.y;
        assert!(
            (seat_y + AVATAR_ROOT_ABOVE_GROUND).abs() < 1e-3,
            "seat {seat_y}"
        );
        let bonus = app.world().resource::<RideState>().camera_bonus();
        assert!((bonus - CAMERA_BONUS).abs() < 1e-3, "bonus {bonus}");

        // G cycles to Ursaluna and swaps the ridden mount in place.
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyG);
        app.insert_resource(actions(&RawInput::from_keyboard(&keys)));
        app.update();
        assert_eq!(
            app.world().resource::<RideState>().mounted,
            Some(RideKind::Ursaluna)
        );

        // V again -> dismounts; the seat returns to the on-foot offset.
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyV);
        app.insert_resource(actions(&RawInput::from_keyboard(&keys)));
        app.update();
        for _ in 0..(TRANSITION_TIME * 60.0) as i32 + 2 {
            app.insert_resource(actions(&RawInput::default()));
            app.update();
        }
        assert!(!app.world().resource::<RideState>().is_mounted());
        let seat_y = app.world().get::<Transform>(scene).unwrap().translation.y;
        assert!(
            (seat_y + AVATAR_ROOT_ABOVE_GROUND).abs() < 1e-3,
            "seat {seat_y}"
        );
    }

    #[test]
    fn mount_controls_line_names_the_keys_of_each_mount() {
        // Sneasler: the climb is the ability (user-report fix 2026-10-15).
        let sneasler = mount_controls_line(RideKind::Sneasler);
        assert!(sneasler.contains("Sneasler"), "{sneasler}");
        assert!(sneasler.contains("Trepar"), "{sneasler}");
        assert!(sneasler.contains("V"), "{sneasler}");
        // Braviary: the glide is the ability.
        let braviary = mount_controls_line(RideKind::Braviary);
        assert!(braviary.contains("planear"), "{braviary}");
        // Ground mounts keep the run/jump/dismount line.
        assert!(mount_controls_line(RideKind::Wyrdeer).contains("Shift"));
        assert!(mount_controls_line(RideKind::Ursaluna).contains("Desmontar: V"));
        // The ability key of each mount rides its own line (Brief 38 follow-up).
        assert!(mount_controls_line(RideKind::Ursaluna).contains("H"), "dig key");
        assert!(mount_controls_line(RideKind::Braviary).contains("aire"), "flap/swoop");
    }

    #[test]
    fn every_mount_has_a_decoded_attach_bone_and_model() {
        for kind in RideKind::ALL {
            let p = kind.params();
            assert!(!p.attach_bone.is_empty(), "{kind:?}");
            assert!(kind.pack_stem().starts_with("pm"), "{kind:?}");
            assert!(p.dash > p.run && p.run > p.walk, "{kind:?}");
        }
    }

    /// The seat constants (`rest_pos`/`rest_rot` of [`RiderSeat`]) are read from the exported
    /// skeleton. This recomputes them from the cached glTF and compares, so a re-export that
    /// moves the anchor bone is caught here (skipped when the cache is not present).
    #[test]
    fn seat_bones_and_rest_transforms_match_the_exported_skeletons() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../work/cache/pokemon_batch");
        let cache_roots = [
            root.clone(),
            std::path::PathBuf::from(std::env::var("PLA_CACHE_DIR").unwrap_or_default()).join("pokemon_batch"),
        ];
        for kind in RideKind::ALL {
            let seat = kind.params().seat;
            let stem = kind.pack_stem();
            let path = cache_roots
                .iter()
                .map(|r| r.join(stem).join(format!("{stem}_idle.gltf")))
                .find(|p| p.is_file());
            let Some(path) = path else {
                eprintln!("[skip] no cache export for {stem}");
                continue;
            };
            let text = std::fs::read_to_string(&path).unwrap();
            let gltf: serde_json::Value = serde_json::from_str(&text).unwrap();
            let nodes = gltf["nodes"].as_array().unwrap();
            let name_of = |i: usize| nodes[i]["name"].as_str().unwrap_or_default().to_owned();
            let bone_index = (0..nodes.len()).find(|i| name_of(*i) == seat.bone);
            let Some(mut cur) = bone_index else {
                panic!("{stem}: seat bone {} missing from the export", seat.bone);
            };
            // Chain from the anchor up to the scene root, composing the rest locals top-down.
            let mut chain = vec![cur];
            'outer: loop {
                for (i, n) in nodes.iter().enumerate() {
                    if n["children"].as_array().is_some_and(|c| c.iter().any(|x| x.as_u64() == Some(cur as u64))) {
                        chain.push(i);
                        cur = i;
                        continue 'outer;
                    }
                }
                break;
            }
            let mut t = Transform::IDENTITY;
            for index in chain.iter().rev() {
                let n = &nodes[*index];
                let local = Transform {
                    translation: n["translation"]
                        .as_array()
                        .map(|a| Vec3::new(a[0].as_f64().unwrap() as f32, a[1].as_f64().unwrap() as f32, a[2].as_f64().unwrap() as f32))
                        .unwrap_or(Vec3::ZERO),
                    rotation: n["rotation"]
                        .as_array()
                        .map(|a| {
                            Quat::from_xyzw(
                                a[0].as_f64().unwrap() as f32,
                                a[1].as_f64().unwrap() as f32,
                                a[2].as_f64().unwrap() as f32,
                                a[3].as_f64().unwrap() as f32,
                            )
                        })
                        .unwrap_or(Quat::IDENTITY),
                    scale: Vec3::ONE,
                };
                t = t.mul_transform(local);
            }
            assert!(
                (t.translation - Vec3::from(seat.rest_pos)).length() < 2e-3,
                "{kind:?}: rest_pos {} vs {}",
                t.translation,
                Vec3::from(seat.rest_pos)
            );
            assert!(
                t.rotation.dot(Quat::from_array(seat.rest_rot)).abs() > 0.999,
                "{kind:?}: rest_rot {} vs {}",
                t.rotation,
                Quat::from_array(seat.rest_rot)
            );
        }
    }
}
