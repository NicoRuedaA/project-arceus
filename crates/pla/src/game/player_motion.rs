//! Player vertical motion: jump, fall, landing and climbing (Brief 13).
//!
//! The viewer (`crates/pla/examples/port_viewer.rs`) owns the horizontal part of the
//! locomotion (camera-relative input, navmesh sliding, wall push-out, water entry) and
//! `game::water` owns swimming. This module owns the vertical state machine:
//!
//! ```text
//! Grounded --Jump--> Airborne --ground contact--> Grounded (land clip timer)
//! Grounded --Confirm at a Climbable--> Climbing --top/bottom--> Grounded
//! Airborne or Climbing --Jump--> Airborne
//! Swimming (game::water) always forces Grounded; the viewer writes the swim height
//! ```
//!
//! # Values
//!
//! `GRAVITY`, `JUMP_SPEED`, `MAX_FALL_SPEED`, `GROUND_MARGIN`, `FIND_GROUND_RANGE`,
//! `STEP_HEIGHT_MAX`, `AIR_MOVE_RATE` and `AIR_HORIZONTAL_SPEED_MAX` are decoded from the
//! game (see `data/player_controller.tsv` and `reports/data-progress/player_moves.md`):
//! `JUMP_SPEED` is `control_firstJumpPower` (3.25, second/third/fourth jump powers are 0:
//! no double jump) and `GRAVITY` is the float at `0x0413a0f8` of the main NSO (`main.elf`),
//! used by `FUN_01984fc8` as the character controller's gravity scale.
//!
//! [ASSUMED] `control_firstJumpPower` is read as a take-off speed in m/s; the original
//! game has no plain on-foot jump (guides: jumping is a ride action, dodge roll is Y on
//! foot), so the on-foot jump here is a port decision. `LAND_TIME`, `CLIMB_SPEED` and the
//! climb numbers are not decoded at all.

use bevy::prelude::*;

use super::input_map::{Action, ActionState};
use super::interact::{climb_step, nearest_climbable, ClimbBoundary, Climbable};
use super::player_anim::AVATAR_ROOT_ABOVE_GROUND;
use super::ride::GlideMount;
use super::state::{GameState, Paused};
use super::water::{PlayerSwim, SwimState};
use super::zone::TerrainResource;
use super::{navigation::GameNavmeshResource, PlayerAvatar};

// ------------------------------------------------------------------ decoded constants

/// Character-controller gravity (m/s^2): the `f32` at `0x0413a0f8` in
/// `work/pla/pk2/main.elf` (verified by reading the ELF segment; the value is `9.8`).
pub const GRAVITY: f32 = 9.8;

/// Jump take-off speed (m/s): decoded `control_firstJumpPower` = 3.25
/// (`player_controller_config.bin`). Reading the value as a speed is ASSUMED.
pub const JUMP_SPEED: f32 = 3.25;

/// Terminal fall speed (m/s): decoded `control_maxFallSpeed` = 50.
pub const MAX_FALL_SPEED: f32 = 50.0;

/// Vertical slack above a climbable where it can still be grabbed: decoded
/// `jump_groundMargin` = 1.5 (semantics ASSUMED).
pub const GROUND_MARGIN: f32 = 1.5;

/// Ground probe range (m): decoded `find_ground_range` = 0.15.
pub const FIND_GROUND_RANGE: f32 = 0.15;

/// Maximum step-up height (m): decoded `control_stepHeightMax` = 0.4
/// (`control_stepHeightMin` = 0.01). The viewer steps through navmesh/terrain sampling;
/// this constant documents the game's limit and is enforced by [`can_step_up`].
pub const STEP_HEIGHT_MAX: f32 = 0.4;

/// Horizontal speed factor while airborne: decoded `control_airMoveRate` = 0.25.
pub const AIR_MOVE_RATE: f32 = 0.25;

/// Maximum horizontal speed while airborne (m/s): decoded
/// `control_airHorizontalVelocityMax` = 11 (the shipped file's own max is 10).
pub const AIR_HORIZONTAL_SPEED_MAX: f32 = 11.0;

/// [ASSUMED] Seconds the `land` clip gets after touchdown (clip length was not measured).
pub const LAND_TIME: f32 = 0.35;

/// [ASSUMED] Climb speed (m/s): no decoded ladder/rope parameter exists.
pub const CLIMB_SPEED: f32 = 1.5;

/// Horizontal move longer than this is a teleport (battle arena, save load, zone change),
/// not locomotion: the motion state resets instead of carrying the fall into the new place.
pub const TELEPORT_DISTANCE: f32 = 25.0;

// ------------------------------------------------------------------ pure physics

/// Apex height of a jump with take-off speed `speed` under gravity `gravity` (m).
pub fn jump_apex_height(speed: f32, gravity: f32) -> f32 {
    speed * speed / (2.0 * gravity)
}

/// One airborne step: gravity integration with a terminal speed.
///
/// `vertical_speed` is +up. Returns `(new speed, dy)`; `dy` is what the transform gets.
pub fn air_step(vertical_speed: f32, dt: f32) -> (f32, f32) {
    air_step_capped(vertical_speed, dt, MAX_FALL_SPEED)
}

/// [`air_step`] with an explicit fall-speed cap: a glide mount ([`GlideMount`], Brief 38)
/// falls no faster than its cap while the cap holds under gravity.
pub fn air_step_capped(vertical_speed: f32, dt: f32, fall_cap: f32) -> (f32, f32) {
    let cap = fall_cap.max(0.1);
    let speed = (vertical_speed - GRAVITY * dt).max(-cap);
    // Trapezoid (average of the old and new speed): exact for constant gravity, so the
    // simulated apex matches `v^2 / 2g` at any frame rate.
    (speed, 0.5 * (vertical_speed + speed) * dt)
}

/// True when an airborne avatar at root height `y` touches the ground (root height
/// `ground_root`) while moving down (or level): the game's `find_ground_range` probe.
///
/// An avatar that is *below* the ground (a jump into a rising slope) lands whatever its
/// vertical speed is, so it never tunnels through the terrain.
pub fn should_land(y: f32, ground_root: f32, vertical_speed: f32) -> bool {
    y < ground_root || (vertical_speed <= 0.0 && y <= ground_root + FIND_GROUND_RANGE)
}

/// True when the ground under a grounded avatar is further below its root than a step the
/// game lets it walk down (decoded `control_stepHeightMax`): a ledge, so the avatar falls
/// instead of snapping to the lower surface. The viewer's ground snapping uses the same test.
pub fn is_ledge_drop(y: f32, ground_root: f32) -> bool {
    y - ground_root > STEP_HEIGHT_MAX
}

/// Fall penalty tiers decoded from `player_config.bin` (`penalty_landHeight0..3` = 7, 10, 14,
/// 19 m with `penalty_landDamage0..3` = 0, 45, 55, 70). The unit of the damage is UNKNOWN
/// (the port has no player HP yet), so [`fall_penalty_tier`] only classifies the height.
pub const FALL_PENALTY_HEIGHTS: [f32; 4] = [7.0, 10.0, 14.0, 19.0];
/// Damage figure of each tier (see [`FALL_PENALTY_HEIGHTS`]); unit UNKNOWN.
pub const FALL_PENALTY_DAMAGE: [u16; 4] = [0, 45, 55, 70];

/// Tier (0-3) reached by a fall of `distance` metres; `None` below the first threshold.
pub fn fall_penalty_tier(distance: f32) -> Option<usize> {
    FALL_PENALTY_HEIGHTS.iter().rposition(|h| distance >= *h)
}

/// Horizontal speed while airborne: the ground speed scaled by the decoded
/// `control_airMoveRate`, capped at the decoded air maximum.
pub fn air_speed(ground_speed: f32) -> f32 {
    (ground_speed * AIR_MOVE_RATE).clamp(0.0, AIR_HORIZONTAL_SPEED_MAX)
}

/// True when a `dy` surface change is a step the avatar can walk up (decoded
/// `control_stepHeightMax`), as opposed to a wall or a drop.
pub fn can_step_up(dy: f32) -> bool {
    (0.0..=STEP_HEIGHT_MAX).contains(&dy)
}

/// Feet-level ground under `(x, z)`: navmesh first (the walkable surface), then the
/// terrain heightmap, exactly like the viewer's locomotion.
pub fn ground_surface_y(
    terrain: Option<&TerrainResource>,
    navmesh: Option<&GameNavmeshResource>,
    x: f32,
    z: f32,
) -> Option<f32> {
    let nav = navmesh.and_then(|n| n.height_at(x, z));
    nav.or_else(|| terrain.and_then(|t| t.area.sample_world_height(x, z)))
}

// ------------------------------------------------------------------ state

/// Vertical locomotion mode of the avatar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MotionMode {
    #[default]
    Grounded,
    Airborne,
    Climbing,
}

/// Vertical motion state of the avatar (component, inserted automatically).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PlayerMotionState {
    pub mode: MotionMode,
    /// Vertical speed while airborne (+up, m/s). 0 on the ground.
    pub vertical_speed: f32,
    /// Fallen distance since take-off (m), for future fall-damage rules.
    pub fall_distance: f32,
    /// Time left of the `land` clip after touchdown (s).
    pub land_timer: f32,
    /// Climbable currently attached to, if any.
    pub climb: Option<Climbable>,
    /// The avatar stood on the ground last frame (so a sudden drop is a ledge, not a spawn).
    pub supported: bool,
    /// Distance of the last completed fall (m); cleared by the consumer.
    pub last_fall: f32,
}

impl PlayerMotionState {
    /// True while the avatar is not standing on the ground.
    pub fn airborne(&self) -> bool {
        self.mode == MotionMode::Airborne
    }

    /// True while attached to a climbable.
    pub fn climbing(&self) -> bool {
        self.mode == MotionMode::Climbing
    }

    /// Start a jump (take-off speed of the decoded first jump power; no double jump).
    pub fn begin_jump(&mut self) {
        self.begin_jump_with(JUMP_SPEED);
    }

    /// Start a jump with an explicit take-off speed: while mounted, `game::ride` supplies the
    /// mount's decoded `control_firstJumpPower` (Wyrdeer 9 m/s and so on).
    pub fn begin_jump_with(&mut self, speed: f32) {
        self.mode = MotionMode::Airborne;
        self.vertical_speed = speed;
        self.fall_distance = 0.0;
        self.climb = None;
    }

    /// Attach to `c` (climb entry).
    pub fn begin_climb(&mut self, c: Climbable) {
        self.mode = MotionMode::Climbing;
        self.vertical_speed = 0.0;
        self.climb = Some(c);
    }

    /// Start falling off a ledge (no take-off speed).
    pub fn begin_fall(&mut self) {
        self.mode = MotionMode::Airborne;
        self.vertical_speed = 0.0;
        self.fall_distance = 0.0;
        self.supported = false;
        self.climb = None;
    }

    /// Leave the climb (top or bottom).
    pub fn end_climb(&mut self) {
        self.mode = MotionMode::Grounded;
        self.climb = None;
        self.vertical_speed = 0.0;
    }
}

// ------------------------------------------------------------------ bevy glue

/// Systems of this module run in this set; the viewer's locomotion chain is ordered
/// `.before(PlayerMotionSet)` so the horizontal move of a frame is already applied.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerMotionSet;

/// Registers the vertical motion state machine.
pub struct PlayerMotionPlugin;

impl Plugin for PlayerMotionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (ensure_motion_state, apply_vertical_motion)
                .chain()
                .in_set(PlayerMotionSet)
                .run_if(motion_active),
        );
    }
}

/// Run condition: the overworld is running (not battle/loading/menu, not paused). Apps that
/// never installed the state or the pause plugin (headless tests) count as running.
pub fn motion_active(state: Option<Res<State<GameState>>>, paused: Option<Res<Paused>>) -> bool {
    state.is_none_or(|s| *s.get() == GameState::World) && !paused.is_some_and(|p| p.0)
}

/// Gives every avatar a [`PlayerMotionState`] once.
pub fn ensure_motion_state(
    mut commands: Commands,
    avatars: Query<Entity, (With<PlayerAvatar>, Without<PlayerMotionState>)>,
) {
    for e in &avatars {
        commands.entity(e).insert(PlayerMotionState::default());
    }
}

/// Frame inputs of [`apply_vertical_motion`]: the avatar transform and motion state, its
/// swim state (`game::water`) and the glide marker of a flyer mount (`game::ride`).
type VerticalInputs<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut PlayerMotionState,
        Option<&'static PlayerSwim>,
        Option<&'static GlideMount>,
    ),
    With<PlayerAvatar>,
>;

/// Jump / fall / land / climb. Runs after the viewer's horizontal locomotion, before the
/// camera and HUD systems read the final transform of the frame.
pub fn apply_vertical_motion(
    time: Res<Time>,
    input: Res<ActionState>,
    ride: Option<Res<super::ride::RideState>>,
    mut q: VerticalInputs,
    climbables: Query<&Climbable>,
    terrain: Option<Res<TerrainResource>>,
    navmesh: Option<Res<GameNavmeshResource>>,
    mut last_position: Local<Option<Vec3>>,
) {
    let dt = time.delta_secs();
    let Ok((mut tf, mut state, swim, glide)) = q.single_mut() else {
        return;
    };

    // A teleport (arena, save load, zone change) resets the vertical state instead of
    // carrying the fall distance into the new place.
    let teleported = last_position
        .map(|last| {
            Vec2::new(tf.translation.x - last.x, tf.translation.z - last.z).length()
                > TELEPORT_DISTANCE
        })
        .unwrap_or(false);
    if teleported {
        state.mode = MotionMode::Grounded;
        state.vertical_speed = 0.0;
        state.fall_distance = 0.0;
        state.land_timer = 0.0;
        state.climb = None;
        state.supported = false;
        *last_position = Some(tf.translation);
        return;
    }
    *last_position = Some(tf.translation);

    // Swimming wins over the whole vertical machine; `game::water` owns that height.
    if swim.is_some_and(|s| s.state == SwimState::Swimming) {
        state.mode = MotionMode::Grounded;
        state.vertical_speed = 0.0;
        state.climb = None;
        state.supported = false;
        return;
    }

    if dt <= 0.0 {
        return;
    }

    let x = tf.translation.x;
    let z = tf.translation.z;
    let ground_root = ground_surface_y(terrain.as_deref(), navmesh.as_deref(), x, z)
        .map(|y| y + AVATAR_ROOT_ABOVE_GROUND);

    // Sequential blocks (not a match): a jump pressed on the ground integrates in the same frame.
    if state.mode == MotionMode::Grounded {
        state.vertical_speed = 0.0;
        state.land_timer = (state.land_timer - dt).max(0.0);
        // Ledge detection: the viewer leaves Y alone when the ground drops by more than a
        // step, so a supported avatar floating above the ground starts to fall; an
        // unsupported one (spawn, teleport) is snapped onto the ground instead.
        if let Some(ground) = ground_root {
            if is_ledge_drop(tf.translation.y, ground) {
                if state.supported {
                    state.begin_fall();
                    return;
                }
                tf.translation.y = ground;
            }
            state.supported = true;
        }
        if input.just_pressed(Action::Jump) {
            // While mounted the take-off speed is the mount's decoded jump power (Brief 38).
            let speed = ride
                .as_ref()
                .and_then(|r| r.mounted)
                .map_or(JUMP_SPEED, |k| k.jump_speed());
            state.begin_jump_with(speed);
        } else if input.just_pressed(Action::Confirm) {
            // Grab the nearest climbable in reach of the avatar root.
            let c = nearest_climbable(climbables.iter(), tf.translation, GROUND_MARGIN).copied();
            if let Some(c) = c {
                state.begin_climb(c);
                tf.translation.x = c.bottom.x;
                tf.translation.z = c.bottom.z;
            }
        }
    }
    if state.mode == MotionMode::Airborne {
        // A flying mount (Braviary) flaps: the jump key pressed in the air lifts it again.
        if input.just_pressed(Action::Jump) {
            if let Some(up) = ride.as_ref().and_then(|r| r.mounted).and_then(|k| k.flap_speed()) {
                state.vertical_speed = up;
            }
        }
        // A glide mount (Braviary, Brief 38) caps the fall speed with its own value.
        let cap = glide.map_or(MAX_FALL_SPEED, |g| g.fall_cap);
        let (speed, dy) = air_step_capped(state.vertical_speed, dt, cap);
        state.vertical_speed = speed;
        state.fall_distance += (-dy).max(0.0);
        tf.translation.y += dy;
        if let Some(ground) = ground_root {
            if should_land(tf.translation.y, ground, state.vertical_speed) {
                tf.translation.y = ground;
                state.mode = MotionMode::Grounded;
                state.vertical_speed = 0.0;
                state.land_timer = LAND_TIME;
                state.last_fall = state.fall_distance;
                state.supported = true;
            }
        }
    }
    if state.mode == MotionMode::Climbing {
        let Some(c) = state.climb else {
            state.end_climb();
            return;
        };
        // Vertical input (W/S / stick) moves along the surface; jump lets go.
        if input.just_pressed(Action::Jump) {
            state.begin_jump();
            return;
        }
        let up = input.move_axis().y;
        let (y, boundary) = climb_step(&c, tf.translation.y, up, CLIMB_SPEED, dt);
        tf.translation.y = y;
        tf.translation.x = c.bottom.x;
        tf.translation.z = c.bottom.z;
        match boundary {
            ClimbBoundary::Top => {
                tf.translation.y = c.top.y + AVATAR_ROOT_ABOVE_GROUND;
                // Step off onto the surface top, which for a cliff sits elsewhere than the
                // base (the real sheet's crest); the debug ladder keeps base == top.
                tf.translation.x = c.top.x;
                tf.translation.z = c.top.z;
                state.end_climb();
                state.supported = true;
            }
            ClimbBoundary::Bottom => {
                tf.translation.y = c.bottom.y + AVATAR_ROOT_ABOVE_GROUND;
                state.end_climb();
                state.supported = true;
            }
            ClimbBoundary::None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glide_cap_slows_the_fall() {
        // Free fall reaches the terminal speed; the same fall under a 2 m/s glide cap does not.
        let dt = 1.0 / 60.0;
        let mut v = 0.0_f32;
        let mut v_glide = 0.0_f32;
        for _ in 0..600 {
            (v, _) = air_step(v, dt);
            (v_glide, _) = air_step_capped(v_glide, dt, 2.0);
        }
        assert!(v <= -MAX_FALL_SPEED + 1e-3);
        assert!((v_glide + 2.0).abs() < 1e-3, "{v_glide}");
        // Rising is not capped (a glide does not slow the jump's ascent).
        let (up, _) = air_step_capped(JUMP_SPEED, dt, 2.0);
        assert!(up > 0.0);
    }

    #[test]
    fn jump_apex_matches_the_formula() {
        // v0^2 / 2g with the decoded values: 0.539 m.
        let h = jump_apex_height(JUMP_SPEED, GRAVITY);
        assert!((h - 0.539).abs() < 0.002, "{h}");
        assert_eq!(jump_apex_height(0.0, GRAVITY), 0.0);
        // Halving gravity doubles the height (sanity of the formula).
        assert!((jump_apex_height(JUMP_SPEED, GRAVITY / 2.0) - 2.0 * h).abs() < 1e-4);
    }

    #[test]
    fn a_simulated_jump_rises_to_the_apex_and_falls_back() {
        // Fixed 60 Hz simulation of the pure integrator.
        let dt = 1.0 / 60.0;
        let (mut v, mut y) = (JUMP_SPEED, 0.0);
        let mut apex = 0.0_f32;
        let mut frames = 0;
        loop {
            let (nv, dy) = air_step(v, dt);
            v = nv;
            y += dy;
            apex = apex.max(y);
            frames += 1;
            if should_land(y, 0.0, v) {
                break;
            }
            assert!(frames < 600, "never landed");
        }
        let expected_apex = jump_apex_height(JUMP_SPEED, GRAVITY);
        assert!(
            (apex - expected_apex).abs() < 0.01,
            "apex {apex} vs {expected_apex}"
        );
        // Airtime until the avatar is within the 0.15 m ground probe on the way down:
        // (v0 + sqrt(v0^2 - 2 g 0.15)) / g = 0.613 s (a full 2 v0 / g would be 0.663 s).
        let airtime = frames as f32 * dt;
        assert!((airtime - 0.613).abs() < 0.02, "airtime {airtime}");
        // The simulation never tunnels through the ground.
        assert!(y >= -0.01, "{y}");
    }

    #[test]
    fn fall_saturates_at_the_decoded_terminal_speed() {
        let (v, dy) = air_step(-MAX_FALL_SPEED, 1.0);
        assert_eq!(v, -MAX_FALL_SPEED);
        assert_eq!(dy, -MAX_FALL_SPEED, "dy = speed * dt with dt = 1");
        // Gravity cannot push past the terminal speed in one big step either.
        assert_eq!(air_step(-100.0, 1.0).0, -MAX_FALL_SPEED);
        // A long fall stays at the terminal speed, never faster.
        let mut v = 0.0;
        for _ in 0..600 {
            v = air_step(v, 1.0 / 60.0).0;
        }
        assert!((v + MAX_FALL_SPEED).abs() < 1e-3, "{v}");
    }

    #[test]
    fn landing_needs_the_probe_range_and_a_downward_speed() {
        // Inside the range going down: lands.
        assert!(should_land(0.1, 0.0, -1.0));
        // Level at the exact ground: lands (the last descending step).
        assert!(should_land(0.0, 0.0, 0.0));
        // Above the probe range: still airborne.
        assert!(!should_land(0.2, 0.0, -1.0));
        // Rising through the probe range: does not land.
        assert!(!should_land(0.05, 0.0, 2.0));
        // Below the ground (a jump into a rising slope): lands even while rising.
        assert!(should_land(-0.2, 0.0, 2.0));
    }

    #[test]
    fn air_speed_and_step_limits_follow_the_decoded_values() {
        assert!((air_speed(4.5) - 1.125).abs() < 1e-6);
        assert!((air_speed(8.0) - 2.0).abs() < 1e-6);
        assert_eq!(air_speed(100.0), AIR_HORIZONTAL_SPEED_MAX);
        assert!(can_step_up(0.0));
        assert!(can_step_up(STEP_HEIGHT_MAX));
        assert!(!can_step_up(STEP_HEIGHT_MAX + 0.01));
        assert!(!can_step_up(-0.01));
    }

    #[test]
    fn ledge_drop_uses_the_decoded_step_height() {
        assert!(!is_ledge_drop(1.0, 1.0));
        assert!(!is_ledge_drop(1.0 + STEP_HEIGHT_MAX, 1.0));
        assert!(is_ledge_drop(1.0 + STEP_HEIGHT_MAX + 0.01, 1.0));
        // Ground above the avatar is never a ledge.
        assert!(!is_ledge_drop(0.0, 5.0));
    }

    #[test]
    fn fall_penalty_tiers_follow_the_decoded_heights() {
        assert_eq!(fall_penalty_tier(6.99), None);
        assert_eq!(fall_penalty_tier(7.0), Some(0));
        assert_eq!(fall_penalty_tier(10.0), Some(1));
        assert_eq!(fall_penalty_tier(14.5), Some(2));
        assert_eq!(fall_penalty_tier(40.0), Some(3));
        assert_eq!(FALL_PENALTY_DAMAGE[3], 70);
    }

    #[test]
    fn state_transitions_stay_consistent() {
        let mut s = PlayerMotionState::default();
        assert!(s.mode == MotionMode::Grounded && !s.airborne() && !s.climbing());
        s.begin_jump();
        assert!(s.airborne() && s.vertical_speed == JUMP_SPEED && s.climb.is_none());
        let c = Climbable {
            kind: super::super::interact::ClimbKind::Rope,
            bottom: Vec3::ZERO,
            top: Vec3::Y * 3.0,
            radius: 1.0,
        };
        s.begin_climb(c);
        assert!(s.climbing() && s.climb == Some(c));
        s.end_climb();
        assert!(s.mode == MotionMode::Grounded && s.climb.is_none());
        assert!(!s.airborne() && !s.climbing());
    }

    // ---------------------------------------------------------- headless bevy

    use crate::game::input_map::InputMapPlugin;
    use crate::game::PlayerAvatar as Avatar;

    fn motion_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(InputMapPlugin)
            .add_plugins(PlayerMotionPlugin);
        // Deterministic frame time: 16 ms per update.
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(16),
        ));
        app
    }

    fn avatar(app: &mut App) -> Entity {
        app.world_mut().spawn((Avatar, Transform::default())).id()
    }

    #[test]
    fn headless_jump_from_synthetic_input() {
        let mut app = motion_app();
        let e = avatar(&mut app);
        app.update(); // one frame to insert PlayerMotionState and get a non-zero delta
        app.update();
        assert_eq!(
            app.world().get::<PlayerMotionState>(e).unwrap().mode,
            MotionMode::Grounded
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyK);
        app.update();

        let state = *app.world().get::<PlayerMotionState>(e).unwrap();
        assert_eq!(state.mode, MotionMode::Airborne, "KeyK starts the jump");
        assert!(state.vertical_speed <= JUMP_SPEED);
        let y = app.world().get::<Transform>(e).unwrap().translation.y;
        assert!(y > 0.0, "airborne avatar left the ground: {y}");

        // Release and let gravity work: the speed must fall, the avatar must never move
        // below the take-off point (no ground data here, it keeps falling).
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyK);
        let v0 = app
            .world()
            .get::<PlayerMotionState>(e)
            .unwrap()
            .vertical_speed;
        // 60 frames * 16 ms: past the apex (0.33 s), falling, with no ground data to land on.
        for _ in 0..60 {
            app.update();
        }
        let state = *app.world().get::<PlayerMotionState>(e).unwrap();
        assert!(state.vertical_speed < v0, "gravity accelerated the fall");
        assert!(state.fall_distance > 0.0);
    }

    fn ladder_at(origin: Vec3) -> Climbable {
        Climbable {
            kind: crate::game::interact::ClimbKind::Ladder,
            bottom: origin,
            top: origin + Vec3::Y * 3.0,
            radius: 1.5,
        }
    }

    #[test]
    fn headless_climb_up_and_step_off_at_the_top() {
        let mut app = motion_app();
        let e = avatar(&mut app);
        let ladder = ladder_at(Vec3::ZERO);
        app.world_mut().spawn(ladder);
        app.update();
        app.update();

        // Confirm grabs the ladder (the avatar is inside its radius).
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        let state = *app.world().get::<PlayerMotionState>(e).unwrap();
        assert_eq!(state.mode, MotionMode::Climbing);
        assert_eq!(
            state.climb.map(|c| c.kind),
            Some(crate::game::interact::ClimbKind::Ladder)
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Enter);

        // Hold W: the root rises along the ladder.
        let y0 = app.world().get::<Transform>(e).unwrap().translation.y;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        for _ in 0..4 {
            app.update();
        }
        let y1 = app.world().get::<Transform>(e).unwrap().translation.y;
        assert!(y1 > y0, "climbing up: {y0} -> {y1}");

        // Reaching the top leaves the climb standing on the surface.
        let top = ladder.root_range().1;
        app.world_mut()
            .get_mut::<Transform>(e)
            .unwrap()
            .translation
            .y = top;
        app.update();
        let state = *app.world().get::<PlayerMotionState>(e).unwrap();
        assert_eq!(state.mode, MotionMode::Grounded);
        assert!(state.climb.is_none());
        let y = app.world().get::<Transform>(e).unwrap().translation.y;
        assert!((y - top).abs() < 1e-4, "stepped off at the top: {y}");
    }

    #[test]
    fn headless_cliff_steps_off_on_the_crest() {
        // A derived cliff's top sits elsewhere than its base: reaching the top lands the avatar
        // on the crest's horizontal position, not above the foot.
        let mut app = motion_app();
        let e = avatar(&mut app);
        let mut cliff = ladder_at(Vec3::ZERO);
        cliff.top = Vec3::new(4.0, 3.0, -2.0);
        app.world_mut().spawn(cliff);
        app.update();
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Enter);
        assert_eq!(
            app.world().get::<PlayerMotionState>(e).unwrap().mode,
            MotionMode::Climbing
        );
        // Climb to the top.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        for _ in 0..400 {
            app.update();
        }
        let state = *app.world().get::<PlayerMotionState>(e).unwrap();
        assert_eq!(state.mode, MotionMode::Grounded, "stepped off at the top");
        let tf = app.world().get::<Transform>(e).unwrap().translation;
        assert!((tf.x - 4.0).abs() < 1e-4, "steps onto the crest x: {tf}");
        assert!((tf.z + 2.0).abs() < 1e-4, "steps onto the crest z: {tf}");
        assert!(
            (tf.y - (3.0 + crate::game::player_anim::AVATAR_ROOT_ABOVE_GROUND)).abs() < 1e-4,
            "crest height: {tf}"
        );
    }

    #[test]
    fn headless_jump_off_a_climbable() {
        let mut app = motion_app();
        let e = avatar(&mut app);
        app.world_mut().spawn(ladder_at(Vec3::ZERO));
        app.update();
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Enter);
        assert_eq!(
            app.world().get::<PlayerMotionState>(e).unwrap().mode,
            MotionMode::Climbing
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyK);
        app.update();
        let state = *app.world().get::<PlayerMotionState>(e).unwrap();
        assert_eq!(state.mode, MotionMode::Airborne);
        assert!(state.climb.is_none());
    }

    #[test]
    fn headless_confirm_far_from_any_climbable_does_nothing() {
        let mut app = motion_app();
        let e = avatar(&mut app);
        app.world_mut().spawn(ladder_at(Vec3::new(50.0, 0.0, 50.0)));
        app.update();
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        assert_eq!(
            app.world().get::<PlayerMotionState>(e).unwrap().mode,
            MotionMode::Grounded
        );
    }

    // ------------------------------------------------- headless with synthetic terrain

    /// 17x17 samples over 16 m: height `high` m for x < 9 m, 0 m for x >= 10 m (a cliff).
    fn cliff_terrain(high: f32) -> crate::game::zone::TerrainResource {
        use crate::assets::bntx::{FloatTextureData, TextureData};
        use crate::assets::terrain::{TerrainArea, TerrainMeshSettings};
        let size = 17u32;
        let mut samples = Vec::new();
        for _z in 0..size {
            for x in 0..size {
                samples.push(if x <= 9 { 1.0 } else { 0.0 });
            }
        }
        let area = TerrainArea {
            area: 1,
            source_path: std::path::PathBuf::new(),
            world_extent: [16.0, 16.0],
            world_height: high,
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
        crate::game::zone::TerrainResource {
            area: std::sync::Arc::new(area),
            chunks: vec![],
            blended_albedo: None,
            settings: TerrainMeshSettings {
                horizontal_cell_size: 1.0,
                vertical_scale: high,
                chunk_cells: 4,
            },
        }
    }

    fn y_of(app: &App, e: Entity) -> f32 {
        app.world().get::<Transform>(e).unwrap().translation.y
    }

    fn state_of(app: &App, e: Entity) -> PlayerMotionState {
        *app.world().get::<PlayerMotionState>(e).unwrap()
    }

    #[test]
    fn an_unsupported_avatar_is_snapped_to_the_ground_not_dropped() {
        let mut app = motion_app();
        app.insert_resource(cliff_terrain(16.0));
        let e = avatar(&mut app);
        app.world_mut().get_mut::<Transform>(e).unwrap().translation = Vec3::new(5.0, 50.0, 5.0);
        app.update();
        app.update();
        // Spawn / teleport: no fall of 34 m, the root is put on the 16 m plateau.
        assert_eq!(state_of(&app, e).mode, MotionMode::Grounded);
        assert!(
            (y_of(&app, e) - (16.0 + AVATAR_ROOT_ABOVE_GROUND)).abs() < 0.01,
            "{}",
            y_of(&app, e)
        );
    }

    #[test]
    fn walking_off_a_cliff_falls_lands_and_records_the_distance() {
        let mut app = motion_app();
        app.insert_resource(cliff_terrain(16.0));
        let e = avatar(&mut app);
        let top = 16.0 + AVATAR_ROOT_ABOVE_GROUND;
        app.world_mut().get_mut::<Transform>(e).unwrap().translation = Vec3::new(5.0, top, 5.0);
        app.update();
        app.update();
        assert!(state_of(&app, e).supported);

        // The locomotion walks the avatar over the edge (9 m < teleport distance).
        app.world_mut()
            .get_mut::<Transform>(e)
            .unwrap()
            .translation
            .x = 14.0;
        app.update();
        assert_eq!(
            state_of(&app, e).mode,
            MotionMode::Airborne,
            "a 16 m ledge is a fall"
        );

        let mut landed = false;
        for _ in 0..400 {
            app.update();
            if state_of(&app, e).mode == MotionMode::Grounded {
                landed = true;
                break;
            }
        }
        assert!(landed, "never landed");
        let st = state_of(&app, e);
        assert!(
            (y_of(&app, e) - AVATAR_ROOT_ABOVE_GROUND).abs() < 0.01,
            "on the lower ground"
        );
        assert!((st.last_fall - 15.9).abs() < 0.5, "fell {}", st.last_fall);
        assert_eq!(
            fall_penalty_tier(st.last_fall),
            Some(2),
            "15.9 m is tier 2 (14-19 m)"
        );
        assert!(
            st.land_timer > 0.0,
            "the land clip timer runs after touchdown"
        );
    }

    #[test]
    fn a_jump_on_flat_ground_returns_to_the_same_height() {
        let mut app = motion_app();
        app.insert_resource(cliff_terrain(16.0));
        let e = avatar(&mut app);
        let top = 16.0 + AVATAR_ROOT_ABOVE_GROUND;
        app.world_mut().get_mut::<Transform>(e).unwrap().translation = Vec3::new(5.0, top, 5.0);
        app.update();
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyK);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyK);
        assert!(state_of(&app, e).airborne());
        let mut apex = 0.0_f32;
        for _ in 0..120 {
            app.update();
            apex = apex.max(y_of(&app, e) - top);
            if !state_of(&app, e).airborne() {
                break;
            }
        }
        assert_eq!(state_of(&app, e).mode, MotionMode::Grounded);
        assert!(
            (y_of(&app, e) - top).abs() < 0.01,
            "back on the plateau, never below it"
        );
        // The 16 ms frames sample the parabola: the apex is the formula's within a frame step.
        assert!(
            (apex - jump_apex_height(JUMP_SPEED, GRAVITY)).abs() < 0.03,
            "apex {apex}"
        );
    }
}
