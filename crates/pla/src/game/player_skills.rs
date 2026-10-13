//! Player skills: dodge roll, crouch and hiding (Brief 22).
//!
//! The viewer owns the horizontal movement; this module owns the *state* of the skills and the
//! rules around them, and `player_anim` turns that state into clips:
//!
//! ```text
//! Standing --Crouch--> Entering --0.417 s--> Crouched --Crouch--> Leaving --0.55 s--> Standing
//! Grounded --Dodge--> Rolling(dir) --1.517 s--> Grounded     (the roll cancels a crouch)
//! ```
//!
//! # What is decoded and what is not
//!
//! - **Clips** (VERIFIED names, read from the player pack's animation list): the dodge roll is
//!   `rolling01_{f,b,l,r}` (91 frames at 60 fps = 1.517 s), crouch idle `squat01_l_loop`, crouch
//!   walk `sqwalk01_loop`, crouch enter/exit `sqwait01_l_start` (25 frames) / `sqwait01_l_end`
//!   (33 frames). That these names are the dodge and the crouch is a reading of the names
//!   (PROBABLE); the native evidence report (`n2-avatar-locomotion-and-state-machine.md`) calls
//!   the Y-button roll the dodge and has a crouch state.
//! - **Roll distance** (VERIFIED from clip data): the `origin` bone of every `rolling01_*` clip
//!   travels exactly 5.25 m. The roll moves the avatar [`ROLL_DISTANCE`] in [`ROLL_DURATION`] at a
//!   constant speed (the constant speed profile is ASSUMED; the clip's own profile is not read).
//! - **Direction thresholds** (value VERIFIED, use PROBABLE): `rolling_forward_angle` 55,
//!   `rolling_side_angle` 135, `rolling_backward_angle` 180 in `player_controller_config.bin`.
//! - **Crouch speed** (value VERIFIED, role PROBABLE): `control_maxCrouchingSpeed` = 2.1 m/s (the
//!   file's own maximum for that parameter is 0, so the value breaks its bound).
//! - **Input buffer**: `buffered_input_frame_rolling` = 5 frames, read as 5/30 s (the 30 Hz
//!   tick is from the native evidence report; ASSUMED).
//! - **Invulnerability**: the native report says the roll asserts an i-frame bit; the port exposes
//!   [`PlayerSkillState::invulnerable`] for the whole roll (the window length is ASSUMED) and
//!   nothing in the port consumes it yet (wild attacks do not damage the player).
//! - **Hiding** (PROBABLE, from the same report): the game hides the player when standing in tall
//!   grass, or when crouching inside a stealth volume, and then wild Pokemon do not perceive the
//!   player. The port has NO tall-grass or stealth-volume data: [`StealthContext`] is false
//!   everywhere unless a future hook (or `PLA_STEALTH_VOLUME_EVERYWHERE=1`, a debug aid) sets it.
//!   There is no dedicated hide clip in the 423 player clips: hiding reuses the crouch.

use bevy::prelude::*;

use super::input_map::{Action, ActionState};
use super::player_motion::{motion_active, MotionMode, PlayerMotionState};
use super::water::{PlayerSwim, SwimState};
use super::PlayerAvatar;

// ------------------------------------------------------------------ decoded constants

/// Horizontal travel of the `origin` bone over each `rolling01_*` clip (m), measured by
/// `player_export` (dz = 5.250 for `_f`, -5.250 for `_b`, dx = +/-5.250 for `_l`/`_r`).
pub const ROLL_DISTANCE: f32 = 5.25;
/// Length of the `rolling01_*` clips: 91 frames at 60 fps.
pub const ROLL_DURATION: f32 = 91.0 / 60.0;
/// `rolling_forward_angle`: a roll within this angle of the facing uses the forward clip.
pub const ROLL_FORWARD_ANGLE: f32 = 55.0;
/// `rolling_side_angle`: up to this angle the roll is a sidestep (left or right).
pub const ROLL_SIDE_ANGLE: f32 = 135.0;
/// `control_maxCrouchingSpeed` (m/s).
pub const CROUCH_SPEED: f32 = 2.1;
/// Length of `sqwait01_l_start` (25 frames at 60 fps).
pub const CROUCH_ENTER_TIME: f32 = 25.0 / 60.0;
/// Length of `sqwait01_l_end` (33 frames at 60 fps).
pub const CROUCH_LEAVE_TIME: f32 = 33.0 / 60.0;
/// `buffered_input_frame_rolling` = 5 frames at an ASSUMED 30 Hz tick.
pub const DODGE_BUFFER: f32 = 5.0 / 30.0;

/// Constant roll speed (m/s) that covers [`ROLL_DISTANCE`] in [`ROLL_DURATION`].
pub fn roll_speed() -> f32 {
    ROLL_DISTANCE / ROLL_DURATION
}

// ------------------------------------------------------------------ pure rules

/// Which roll clip a dodge uses, relative to where the avatar faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DodgeDir {
    Forward,
    Back,
    Left,
    Right,
}

/// Signed angle (degrees) from `facing` to `dir` on the ground plane, positive towards the
/// avatar's left (the model faces +Z, so left is +X). Zero vectors give 0.
pub fn signed_angle_deg(facing: Vec2, dir: Vec2) -> f32 {
    if facing.length_squared() <= f32::EPSILON || dir.length_squared() <= f32::EPSILON {
        return 0.0;
    }
    let f = facing.normalize();
    let d = dir.normalize();
    let left = Vec2::new(f.y, -f.x);
    d.dot(left).atan2(d.dot(f)).to_degrees()
}

/// Roll clip direction for a signed angle, using the decoded thresholds.
pub fn dodge_direction(angle_deg: f32) -> DodgeDir {
    let a = angle_deg.abs();
    if a <= ROLL_FORWARD_ANGLE {
        DodgeDir::Forward
    } else if a <= ROLL_SIDE_ANGLE {
        if angle_deg > 0.0 {
            DodgeDir::Left
        } else {
            DodgeDir::Right
        }
    } else {
        DodgeDir::Back
    }
}

/// Is the player concealed from wild Pokemon? Standing in tall grass hides without crouching;
/// crouching hides only inside a stealth volume (PROBABLE, native evidence report).
pub fn is_hidden(crouching: bool, in_tall_grass: bool, in_stealth_volume: bool) -> bool {
    in_tall_grass || (crouching && in_stealth_volume)
}

/// Ground speed while crouched: the decoded crouch maximum caps the normal speed.
pub fn crouch_speed(ground_speed: f32) -> f32 {
    ground_speed.min(CROUCH_SPEED)
}

// ------------------------------------------------------------------ state

/// Where the crouch is in its enter / hold / leave cycle.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CrouchPhase {
    #[default]
    Standing,
    /// Playing `sqwait01_l_start`; seconds elapsed.
    Entering(f32),
    Crouched,
    /// Playing `sqwait01_l_end`; seconds elapsed.
    Leaving(f32),
}

/// A roll in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveDodge {
    pub dir: DodgeDir,
    /// Unit travel direction on the ground plane (world space).
    pub world_dir: Vec2,
    pub elapsed: f32,
}

/// Crouch and dodge state of the avatar (component, inserted automatically).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct PlayerSkillState {
    pub crouch: CrouchPhase,
    pub dodge: Option<ActiveDodge>,
    /// Seconds left of a buffered dodge press that could not start yet.
    pub dodge_buffer: f32,
}

impl PlayerSkillState {
    /// True from the moment the crouch starts until the leave animation starts.
    pub fn crouching(&self) -> bool {
        matches!(
            self.crouch,
            CrouchPhase::Entering(_) | CrouchPhase::Crouched
        )
    }

    pub fn dodging(&self) -> bool {
        self.dodge.is_some()
    }

    /// The roll asserts the game's i-frame bit for its whole length (window ASSUMED).
    pub fn invulnerable(&self) -> bool {
        self.dodge.is_some()
    }

    /// Toggle the crouch. Ignored while rolling. Returns the new `crouching()`.
    pub fn toggle_crouch(&mut self) -> bool {
        if self.dodging() {
            return self.crouching();
        }
        self.crouch = match self.crouch {
            CrouchPhase::Standing | CrouchPhase::Leaving(_) => CrouchPhase::Entering(0.0),
            CrouchPhase::Entering(_) | CrouchPhase::Crouched => CrouchPhase::Leaving(0.0),
        };
        self.crouching()
    }

    /// Stand up at once (the roll, a jump or a sprint cancels the crouch without the leave clip).
    pub fn stand_now(&mut self) {
        self.crouch = CrouchPhase::Standing;
    }

    /// Start a dodge towards `world_dir` (unit or any non-zero vector; a zero vector rolls along
    /// `facing`). Returns the clip direction, or `None` when a roll is already running or
    /// `can_roll` is false (airborne, climbing, swimming). The roll cancels a crouch.
    pub fn begin_dodge(
        &mut self,
        can_roll: bool,
        world_dir: Vec2,
        facing: Vec2,
    ) -> Option<DodgeDir> {
        if self.dodging() || !can_roll {
            return None;
        }
        let dir = if world_dir.length_squared() > f32::EPSILON {
            world_dir.normalize()
        } else if facing.length_squared() > f32::EPSILON {
            facing.normalize()
        } else {
            return None;
        };
        let clip = dodge_direction(signed_angle_deg(facing, dir));
        self.stand_now();
        self.dodge_buffer = 0.0;
        self.dodge = Some(ActiveDodge {
            dir: clip,
            world_dir: dir,
            elapsed: 0.0,
        });
        Some(clip)
    }

    /// Advance the timers by `dt`. Returns true when a roll just ended.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.dodge_buffer = (self.dodge_buffer - dt).max(0.0);
        self.crouch = match self.crouch {
            CrouchPhase::Entering(t) if t + dt >= CROUCH_ENTER_TIME => CrouchPhase::Crouched,
            CrouchPhase::Entering(t) => CrouchPhase::Entering(t + dt),
            CrouchPhase::Leaving(t) if t + dt >= CROUCH_LEAVE_TIME => CrouchPhase::Standing,
            CrouchPhase::Leaving(t) => CrouchPhase::Leaving(t + dt),
            other => other,
        };
        let mut ended = false;
        if let Some(d) = &mut self.dodge {
            d.elapsed += dt;
            if d.elapsed >= ROLL_DURATION {
                self.dodge = None;
                ended = true;
            }
        }
        ended
    }

    /// Remember a dodge press for the input-buffer window.
    pub fn buffer_dodge(&mut self) {
        self.dodge_buffer = DODGE_BUFFER;
    }
}

/// Whether a wild Pokemon can perceive the player this frame (written by [`update_stealth`]).
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerStealth {
    pub hidden: bool,
}

/// Where the player stands, as far as hiding is concerned. No tall-grass or stealth-volume data
/// exists in the port yet, so both are false unless something sets them.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StealthContext {
    pub in_tall_grass: bool,
    pub in_stealth_volume: bool,
}

/// `PLA_STEALTH_VOLUME_EVERYWHERE=1`: debug aid that treats the whole world as a stealth volume,
/// so crouching hides the player and the effect can be seen without decoded volumes.
pub fn debug_stealth_everywhere() -> bool {
    std::env::var_os("PLA_STEALTH_VOLUME_EVERYWHERE").is_some_and(|v| v != "0")
}

// ------------------------------------------------------------------ bevy glue

/// Systems of this module run in this set; the viewer's locomotion runs `.after` it so the
/// state of the frame (crouch toggled, roll advanced) is final when the movement reads it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerSkillsSet;

pub struct PlayerSkillsPlugin;

impl Plugin for PlayerSkillsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerStealth>()
            .init_resource::<StealthContext>();
        if debug_stealth_everywhere() {
            app.world_mut()
                .resource_mut::<StealthContext>()
                .in_stealth_volume = true;
        }
        app.add_systems(
            Update,
            (ensure_skill_state, update_skills, update_stealth)
                .chain()
                .in_set(PlayerSkillsSet)
                .run_if(motion_active),
        );
    }
}

pub fn ensure_skill_state(
    mut commands: Commands,
    avatars: Query<Entity, (With<PlayerAvatar>, Without<PlayerSkillState>)>,
) {
    for e in &avatars {
        commands.entity(e).insert(PlayerSkillState::default());
    }
}

/// Crouch toggle, forced stand-up, timers and the dodge input buffer.
pub fn update_skills(
    time: Res<Time>,
    input: Res<ActionState>,
    mut q: Query<(
        &mut PlayerSkillState,
        Option<&PlayerMotionState>,
        Option<&PlayerSwim>,
    )>,
) {
    let dt = time.delta_secs();
    for (mut s, motion, swim) in &mut q {
        let swimming = swim.is_some_and(|w| w.state == SwimState::Swimming);
        let grounded = motion.is_none_or(|m| m.mode == MotionMode::Grounded);
        // Anything that leaves the ground, the water or a sprint stands the avatar up.
        if s.crouching() && (!grounded || swimming || input.pressed(Action::Sprint)) {
            s.stand_now();
        }
        if input.just_pressed(Action::Crouch) && grounded && !swimming {
            s.toggle_crouch();
        }
        if input.just_pressed(Action::Dodge) {
            s.buffer_dodge();
        }
        s.tick(dt);
    }
}

/// Publish whether the player is hidden from wild Pokemon.
pub fn update_stealth(
    ctx: Res<StealthContext>,
    q: Query<&PlayerSkillState, With<PlayerAvatar>>,
    mut stealth: ResMut<PlayerStealth>,
) {
    let crouching = q.single().is_ok_and(|s| s.crouching());
    stealth.hidden = is_hidden(crouching, ctx.in_tall_grass, ctx.in_stealth_volume);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::input_map::InputMapPlugin;
    use crate::game::player_motion::PlayerMotionPlugin;

    #[test]
    fn roll_numbers_come_from_the_clip() {
        assert!((ROLL_DURATION - 1.5167).abs() < 1e-3);
        // 5.25 m in 1.517 s.
        assert!((roll_speed() - 3.461).abs() < 0.005, "{}", roll_speed());
        assert!((CROUCH_ENTER_TIME - 0.4167).abs() < 1e-3);
        assert!((CROUCH_LEAVE_TIME - 0.55).abs() < 1e-3);
    }

    #[test]
    fn signed_angle_is_positive_to_the_left() {
        let f = Vec2::new(0.0, 1.0); // facing +Z
        assert!((signed_angle_deg(f, Vec2::new(0.0, 1.0))).abs() < 1e-4);
        assert!(
            (signed_angle_deg(f, Vec2::new(1.0, 0.0)) - 90.0).abs() < 1e-3,
            "+X is left"
        );
        assert!((signed_angle_deg(f, Vec2::new(-1.0, 0.0)) + 90.0).abs() < 1e-3);
        assert!((signed_angle_deg(f, Vec2::new(0.0, -1.0)).abs() - 180.0).abs() < 1e-3);
        // Rotating the facing rotates the answer with it.
        let f = Vec2::new(1.0, 0.0); // facing +X
        assert!((signed_angle_deg(f, Vec2::new(0.0, -1.0)) - 90.0).abs() < 1e-3);
        assert_eq!(signed_angle_deg(Vec2::ZERO, f), 0.0);
    }

    #[test]
    fn dodge_direction_follows_the_decoded_angles() {
        assert_eq!(dodge_direction(0.0), DodgeDir::Forward);
        assert_eq!(dodge_direction(55.0), DodgeDir::Forward);
        assert_eq!(dodge_direction(-55.0), DodgeDir::Forward);
        assert_eq!(dodge_direction(56.0), DodgeDir::Left);
        assert_eq!(dodge_direction(-90.0), DodgeDir::Right);
        assert_eq!(dodge_direction(135.0), DodgeDir::Left);
        assert_eq!(dodge_direction(136.0), DodgeDir::Back);
        assert_eq!(dodge_direction(-180.0), DodgeDir::Back);
    }

    #[test]
    fn hiding_needs_grass_or_a_crouch_in_a_stealth_volume() {
        assert!(!is_hidden(false, false, false));
        assert!(
            !is_hidden(true, false, false),
            "crouching alone does not hide"
        );
        assert!(
            !is_hidden(false, false, true),
            "a stealth volume needs a crouch"
        );
        assert!(is_hidden(true, false, true));
        assert!(
            is_hidden(false, true, false),
            "tall grass hides a standing player"
        );
    }

    #[test]
    fn crouch_caps_the_speed() {
        assert_eq!(crouch_speed(4.5), CROUCH_SPEED);
        assert_eq!(crouch_speed(1.0), 1.0);
    }

    #[test]
    fn crouch_cycle_runs_enter_hold_leave() {
        let mut s = PlayerSkillState::default();
        assert!(!s.crouching());
        assert!(s.toggle_crouch());
        assert!(matches!(s.crouch, CrouchPhase::Entering(_)));
        s.tick(CROUCH_ENTER_TIME * 0.5);
        assert!(matches!(s.crouch, CrouchPhase::Entering(_)));
        s.tick(CROUCH_ENTER_TIME);
        assert_eq!(s.crouch, CrouchPhase::Crouched);
        assert!(!s.toggle_crouch(), "second press leaves");
        assert!(matches!(s.crouch, CrouchPhase::Leaving(_)));
        assert!(!s.crouching(), "the leave animation is already standing");
        s.tick(CROUCH_LEAVE_TIME + 0.01);
        assert_eq!(s.crouch, CrouchPhase::Standing);
        // Toggling while leaving crouches again.
        s.toggle_crouch();
        s.toggle_crouch();
        s.toggle_crouch();
        assert!(s.crouching());
    }

    #[test]
    fn a_dodge_runs_for_the_clip_length_then_ends() {
        let mut s = PlayerSkillState::default();
        let dir = s.begin_dodge(true, Vec2::new(0.0, 1.0), Vec2::new(0.0, 1.0));
        assert_eq!(dir, Some(DodgeDir::Forward));
        assert!(s.dodging() && s.invulnerable());
        // No second roll while one runs.
        assert_eq!(s.begin_dodge(true, Vec2::X, Vec2::Y), None);
        assert!(!s.tick(ROLL_DURATION - 0.01));
        assert!(s.dodging());
        assert!(s.tick(0.02), "the roll ends exactly once");
        assert!(!s.dodging() && !s.invulnerable());
    }

    #[test]
    fn a_dodge_picks_its_clip_from_the_facing_and_cancels_the_crouch() {
        let mut s = PlayerSkillState::default();
        s.toggle_crouch();
        assert!(s.crouching());
        // Facing +Z, rolling towards -X: a right-hand sidestep.
        let dir = s.begin_dodge(true, Vec2::new(-1.0, 0.0), Vec2::new(0.0, 1.0));
        assert_eq!(dir, Some(DodgeDir::Right));
        assert_eq!(s.crouch, CrouchPhase::Standing);
        // No input: rolls along the facing.
        let mut s = PlayerSkillState::default();
        assert_eq!(
            s.begin_dodge(true, Vec2::ZERO, Vec2::new(1.0, 0.0)),
            Some(DodgeDir::Forward)
        );
        assert_eq!(s.dodge.unwrap().world_dir, Vec2::new(1.0, 0.0));
        // Nothing to roll along: refused.
        let mut s = PlayerSkillState::default();
        assert_eq!(s.begin_dodge(true, Vec2::ZERO, Vec2::ZERO), None);
        // Not allowed in the air / water.
        assert_eq!(s.begin_dodge(false, Vec2::X, Vec2::Y), None);
    }

    #[test]
    fn crouch_is_ignored_while_rolling_and_the_buffer_expires() {
        let mut s = PlayerSkillState::default();
        s.begin_dodge(true, Vec2::Y, Vec2::Y);
        assert!(!s.toggle_crouch());
        assert_eq!(s.crouch, CrouchPhase::Standing);
        s.buffer_dodge();
        assert!(s.dodge_buffer > 0.0);
        s.tick(DODGE_BUFFER + 0.01);
        assert_eq!(s.dodge_buffer, 0.0);
    }

    // ---------------------------------------------------------- headless bevy

    fn skills_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(InputMapPlugin)
            .add_plugins(PlayerMotionPlugin)
            .add_plugins(PlayerSkillsPlugin);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(16),
        ));
        app
    }

    fn state(app: &App, e: Entity) -> PlayerSkillState {
        *app.world().get::<PlayerSkillState>(e).unwrap()
    }

    /// Press `key` for one frame. MinimalPlugins has no input plugin, so nothing clears the
    /// `just_pressed` edge between frames: clear it by hand or the key would "re-press" forever.
    fn tap(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(key);
        keys.clear();
    }

    #[test]
    fn headless_crouch_toggles_with_the_key_and_hides_in_a_stealth_volume() {
        let mut app = skills_app();
        let e = app
            .world_mut()
            .spawn((PlayerAvatar, Transform::default()))
            .id();
        app.update();
        app.update();
        assert!(!state(&app, e).crouching());

        tap(&mut app, KeyCode::ControlLeft);
        assert!(state(&app, e).crouching());
        // Crouching alone does not hide ...
        app.update();
        assert!(!app.world().resource::<PlayerStealth>().hidden);
        // ... inside a stealth volume it does.
        app.world_mut()
            .resource_mut::<StealthContext>()
            .in_stealth_volume = true;
        app.update();
        assert!(app.world().resource::<PlayerStealth>().hidden);

        // Let the enter clip finish, then toggle off: no longer crouching, no longer hidden.
        for _ in 0..40 {
            app.update();
        }
        assert_eq!(state(&app, e).crouch, CrouchPhase::Crouched);
        tap(&mut app, KeyCode::ControlLeft);
        app.update();
        assert!(!state(&app, e).crouching());
        assert!(!app.world().resource::<PlayerStealth>().hidden);
    }

    #[test]
    fn headless_a_jump_or_a_sprint_stands_the_avatar_up() {
        let mut app = skills_app();
        let e = app
            .world_mut()
            .spawn((PlayerAvatar, Transform::default()))
            .id();
        app.update();
        app.update();
        tap(&mut app, KeyCode::ControlLeft);
        assert!(state(&app, e).crouching());
        tap(&mut app, KeyCode::KeyK); // jump
        app.update();
        assert!(
            !state(&app, e).crouching(),
            "airborne avatars are not crouching"
        );

        // Sprint (shift held) cancels a crouch too.
        let mut app = skills_app();
        let e = app
            .world_mut()
            .spawn((PlayerAvatar, Transform::default()))
            .id();
        app.update();
        app.update();
        tap(&mut app, KeyCode::ControlLeft);
        assert!(state(&app, e).crouching());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        app.update();
        assert!(!state(&app, e).crouching());
    }

    #[test]
    fn headless_dodge_press_is_buffered() {
        let mut app = skills_app();
        let e = app
            .world_mut()
            .spawn((PlayerAvatar, Transform::default()))
            .id();
        app.update();
        app.update();
        tap(&mut app, KeyCode::KeyH);
        assert!(
            state(&app, e).dodge_buffer > 0.0,
            "the press is remembered for 5 frames"
        );
        for _ in 0..12 {
            app.update();
        }
        assert_eq!(state(&app, e).dodge_buffer, 0.0);
    }
}
