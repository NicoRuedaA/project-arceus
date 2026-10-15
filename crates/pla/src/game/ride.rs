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
//! The exported glTFs used by the port do not carry those bones, so the rider sits at a
//! saddle height read from the exported skeleton's own rest pose (`spine_02` / `waist`
//! world Y, PROBABLE) for all five mounts. See `reports/data-progress/mounts.md`.
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

/// Braviary glide: fall-speed cap (m/s). SUPUESTO: the decoded `main_max_fall_speed` = 0.2
/// (unit unknown, possibly a per-frame value); the port picks a playable 2.0 m/s.
pub const GLIDE_FALL_CAP: f32 = 2.0;

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
    /// `player_attach_joint_name` (the saddle bone in the ride pack skeleton, VERIFIED).
    pub attach_bone: &'static str,
    /// Height of the rider's feet above the mount root (m): read from the exported
    /// skeleton's rest pose (`spine_02` / `waist` world Y; PROBABLE).
    pub saddle_height: f32,
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
            // Speeds: ride_odoshishi_config. Saddle: spine_02 rest height 0.723 m (PROBABLE).
            RideKind::Wyrdeer => RideParams {
                internal_name: "odoshishi",
                dex: 899,
                form: 0,
                attach_bone: "parts",
                saddle_height: 0.75,
                domain: RideDomain::Ground,
                input_walk: 0.01,
                input_run: 0.97,
                input_dash: 1.5,
                walk: 3.0,
                run: 8.0,
                dash: 11.0,
            },
            // Speeds: ride_ringuma_controller_config (0.35/0.70/1.05 thresholds).
            // Saddle: spine_02 rest height 0.858 m (PROBABLE).
            RideKind::Ursaluna => RideParams {
                internal_name: "ringuma",
                dex: 901,
                form: 0,
                attach_bone: "parts_01",
                saddle_height: 0.90,
                domain: RideDomain::Ground,
                input_walk: 0.35,
                input_run: 0.70,
                input_dash: 1.05,
                walk: 2.0,
                run: 5.0,
                dash: 6.0,
            },
            // Speeds: ride_basurao_config. Saddle: waist rest height 0.71 m (PROBABLE).
            RideKind::Basculegion => RideParams {
                internal_name: "basurao",
                dex: 902,
                form: 0,
                attach_bone: "origin",
                saddle_height: 0.70,
                domain: RideDomain::Water,
                input_walk: 0.01,
                input_run: 0.97,
                input_dash: 1.5,
                walk: 5.0,
                run: 8.0,
                dash: 11.0,
            },
            // Speeds: ride_nyuura_controller_config; climb_move_speed_max = 4.8.
            // Saddle: spine_02 rest height 0.746 m of the exported skeleton (PROBABLE).
            RideKind::Sneasler => RideParams {
                internal_name: "nyuura",
                dex: 903,
                form: 0,
                attach_bone: "origin",
                saddle_height: 0.75,
                domain: RideDomain::Ground,
                input_walk: 0.01,
                input_run: 0.97,
                input_dash: 1.5,
                walk: 2.0,
                run: 6.2,
                dash: 8.4,
            },
            // Speeds: ride_whooguru_controller_config; air: main_max_move_speed = 9.5.
            // Saddle: spine_02 rest height 1.01 m of the exported skeleton (PROBABLE).
            RideKind::Braviary => RideParams {
                internal_name: "whooguru",
                dex: 628,
                form: 41,
                attach_bone: "parts_03",
                saddle_height: 1.00,
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
}

impl Default for RideState {
    fn default() -> Self {
        Self {
            mounted: None,
            selected: RideKind::Wyrdeer,
            stamina: STAMINA_MAX,
            transition: 0.0,
            transition_dir: 0.0,
        }
    }
}

impl RideState {
    /// Whether a mount is currently ridden.
    pub fn is_mounted(&self) -> bool {
        self.mounted.is_some()
    }

    /// Speed for this frame's input (see [`mounted_speed`]); `None` on foot.
    pub fn mounted_speed(&self, input: f32, sprint: bool) -> Option<f32> {
        mounted_speed(self.mounted, self.stamina, input, sprint)
    }

    /// Glide horizontal speed while in the air on a flyer; `None` otherwise.
    pub fn air_speed(&self) -> Option<f32> {
        self.mounted.and_then(glide_speed)
    }

    /// Fall-speed cap while mounted on a flyer.
    pub fn fall_cap(&self) -> Option<f32> {
        self.mounted
            .filter(|k| k.params().domain == RideDomain::Air)
            .map(|_| GLIDE_FALL_CAP)
    }

    /// Camera pull-back in metres, eased by the mount/dismount transition (`INOUT_SINE`,
    /// the decoded interpolation type). 0 when fully dismounted.
    pub fn camera_bonus(&self) -> f32 {
        let eased = smoothstep(self.transition.clamp(0.0, 1.0));
        CAMERA_BONUS * eased
    }

    /// Saddle offset of the player model while mounting, eased by the transition: the model
    /// child sits at `-(AVATAR_ROOT_ABOVE_GROUND)` on foot and rises to `saddle_height -
    /// AVATAR_ROOT_ABOVE_GROUND` when mounted (the mount being transitioned to, falling back
    /// to the selected one while dismounting).
    pub fn player_model_offset_y(&self) -> f32 {
        let kind = self.mounted.unwrap_or(self.selected);
        let saddle = kind.params().saddle_height;
        let eased = smoothstep(self.transition.clamp(0.0, 1.0));
        // On foot the child sits at -AVATAR_ROOT_ABOVE_GROUND (feet on the ground); at full
        // mount its feet are `saddle` metres up, so the term `saddle * eased` is added.
        saddle * eased - AVATAR_ROOT_ABOVE_GROUND
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

/// Reads the ride keys, toggling / cycling the mount, advancing the transition and draining
/// stamina from the speed the avatar really moved at this frame.
fn ride_input(
    time: Res<Time>,
    input: Res<ActionState>,
    avatar: Query<&Transform, With<PlayerAvatar>>,
    mut ride: ResMut<RideState>,
    mut last_position: Local<Option<Vec3>>,
) {
    if input.just_pressed(Action::Ride) {
        ride.toggle();
    }
    if input.just_pressed(Action::RideCycle) {
        ride.cycle();
    }
    let dt = time.delta_secs();
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

/// Query inputs of [`sync_mount_view`]: the player-model scene child (its local Y is the
/// seat height) and the glide marker entities.
type MountViewQueries<'w, 's> = (
    Query<'w, 's, &'static mut Transform, (With<PlayerModelScene>, Without<PlayerAvatar>, Without<RideMountView>)>,
    Query<'w, 's, (Entity, &'static GlideMount)>,
);

/// Keeps the mount view entity and the rider's seat in sync with the ride state.
#[allow(clippy::too_many_arguments)]
fn sync_mount_view(
    mut commands: Commands,
    asset_server: Option<Res<AssetServer>>,
    ride: Res<RideState>,
    avatar: Query<Entity, With<PlayerAvatar>>,
    views: Query<(Entity, &RideMountView)>,
    (mut scenes, glide): MountViewQueries,
    mut warned: Local<Option<RideKind>>,
) {
    let Ok(avatar_e) = avatar.single() else {
        return;
    };
    // Spawn / respawn / despawn the mount view (a child of the avatar, feet at its root).
    let want = ride.mounted;
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
    // Seat the rider: the player model child rises to the saddle over the transition.
    for mut tf in &mut scenes {
        let want_y = ride.player_model_offset_y();
        if (tf.translation.y - want_y).abs() > 1e-4 {
            tf.translation.y = want_y;
        }
    }
    // Glide marker on the avatar (flyers only), inserted and removed only when it changes.
    match ride.fall_cap() {
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

/// Registers the ride systems. Add with `app.add_plugins(RidePlugin)`.
pub struct RidePlugin;

impl Plugin for RidePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RideState>();
        app.add_systems(
            Update,
            (ride_input, sync_mount_view)
                .chain()
                .run_if(in_state(GameState::World))
                .run_if(not_paused),
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
    fn transition_eases_the_seat_and_the_camera() {
        let mut st = RideState::default();
        assert!((st.camera_bonus()).abs() < 1e-6);
        st.toggle();
        assert_eq!(st.transition_dir, 1.0);
        // Half way: smoothstep(0.5) = 0.5 exactly.
        st.transition = 0.5;
        assert!((st.camera_bonus() - CAMERA_BONUS * 0.5).abs() < 1e-4);
        let on_foot = -AVATAR_ROOT_ABOVE_GROUND;
        let seated = RideKind::Wyrdeer.params().saddle_height - AVATAR_ROOT_ABOVE_GROUND;
        let mid = st.player_model_offset_y();
        assert!((mid - (on_foot + seated) * 0.5).abs() < 1e-4, "{mid}");
        // Reaches the saddle after TRANSITION_TIME seconds.
        let mut t = 0.0;
        while t < TRANSITION_TIME + 0.01 {
            st.step_transition(1.0 / 60.0);
            t += 1.0 / 60.0;
        }
        assert_eq!(st.transition, 1.0);
        assert_eq!(st.transition_dir, 0.0);
        assert!((st.player_model_offset_y() - seated).abs() < 1e-4);
        assert!((st.camera_bonus() - CAMERA_BONUS).abs() < 1e-4);
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

        // Idle frames: stays mounted; the seat rises to the saddle and the camera eases back.
        for _ in 0..(TRANSITION_TIME * 60.0) as i32 + 2 {
            app.insert_resource(actions(&RawInput::default()));
            app.update();
        }
        let seat_y = app.world().get::<Transform>(scene).unwrap().translation.y;
        let want = RideKind::Wyrdeer.params().saddle_height - AVATAR_ROOT_ABOVE_GROUND;
        assert!((seat_y - want).abs() < 1e-3, "seat {seat_y} vs {want}");
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
    fn every_mount_has_a_decoded_attach_bone_and_model() {
        for kind in RideKind::ALL {
            let p = kind.params();
            assert!(!p.attach_bone.is_empty(), "{kind:?}");
            assert!(kind.pack_stem().starts_with("pm"), "{kind:?}");
            assert!(p.dash > p.run && p.run > p.walk, "{kind:?}");
        }
    }
}
