//! Input mapping: keyboard and gamepad to logical [`Action`]s.
//!
//! The pure core ([`actions`], [`stick_curve`], [`combine_axis`]) takes a plain
//! [`RawInput`] snapshot and is unit tested without a Bevy `App`. The Bevy glue
//! ([`InputMapPlugin`]) fills the [`ActionState`] resource once per frame from
//! `ButtonInput<KeyCode>` and the connected `Gamepad` components, so gameplay systems
//! never read `KeyCode`s directly.
//!
//! Default mapping (Switch-style; the confidence of every row is stated in
//! `reports/data-progress/gamepad_input.md`):
//!
//! | Action | Keyboard | Gamepad |
//! |---|---|---|
//! | Move | WASD / arrows | Left stick (deadzone 0.15 + smoothstep curve) |
//! | Look | Q / E (yaw) | Right stick |
//! | Zoom | R (closer) / F (farther) | not mapped |
//! | Sprint | Shift (held) | L3, left stick click (held) |
//! | Aim toggle | Z / Tab | ZL |
//! | Dodge (roll) | H | Y (north) |
//! | Crouch (toggle) | Left Ctrl | R3, right stick click |
//! | Ride: mount / dismount (Brief 38) | V | Minus (Select; port choice, the original's call-ride binding is not decoded) |
//! | Ride: cycle mount (Brief 38) | G | not mapped |
//! | Jump / grab climbable | K | X (west; the original jumps only while riding; port choice) |
//! | Throw | not mapped | ZR |
//! | Confirm | Enter / NumpadEnter / Space | A (south) |
//! | Cancel | Escape / Backspace | B (east) |
//! | Menu (pause) | Escape | Plus (start) |
//! | Prev / Next | Left, Up / Right, Down / A / D | D-pad left, up / right, down |
//! | Battle move slots 1-4 | Digit1..Digit4 | not mapped |
//! | Battle Fight | F | Y |
//! | Battle Switch | S | X |
//! | Battle Item | I | L |
//! | Battle Capture | C / B | ZR |
//! | Battle Run | R | R |
//! | Battle style Agile / Strong | Q / E (just pressed; Q / E also turn the camera) | not mapped |
//!
//! Priority: keyboard wins per axis over the stick ([`combine_axis`]); digital edges from
//! both sources merge without duplicates.

use bevy::prelude::*;

// ------------------------------------------------------------------ pure logic

/// Logical game actions.
///
/// `MoveX`/`MoveY`/`LookX`/`LookY`/`ZoomIn` carry analog values (see
/// [`ActionState::axis`]); everything else is a digital button-like action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    // Analog axes.
    MoveX,
    MoveY,
    LookX,
    LookY,
    ZoomIn,
    // Digital.
    Confirm,
    Cancel,
    Menu,
    Sprint,
    Dodge,
    /// Jump / climb grab (Brief 13). The original has no plain on-foot jump
    /// (jumping is a ride action there), so the binding is a port choice.
    Jump,
    /// Crouch toggle (Brief 22). The original's crouch binding is not known; port choice.
    Crouch,
    /// Mount / dismount the selected ride Pokémon (Brief 38). The original calls the ride
    /// on the west/North button while on foot; the port binds `V` (gamepad `Select`).
    Ride,
    /// Cycle the selected ride Pokémon (Brief 38); keyboard-only dev key `G`.
    RideCycle,
    Aim,
    Throw,
    Prev,
    Next,
    /// Battle move slot 0..=3 (keyboard Digit1..Digit4).
    Slot(u8),
    OpenFight,
    OpenSwitch,
    OpenItem,
    OpenCapture,
    OpenRun,
    /// Battle: use the highlighted move in agile style (keyboard Q).
    StyleAgile,
    /// Battle: use the highlighted move in strong style (keyboard E).
    StyleStrong,
    /// Battle: arm the battle's gimmick for the next move (keyboard Y; port extension).
    Gimmick,
}

/// Stick movement below this magnitude is ignored.
pub const STICK_DEADZONE: f32 = 0.15;

/// Apply the stick deadzone and a smoothstep response curve to a raw stick vector.
///
/// Input longer than 1 is clamped (diagonal drift); the result keeps the input direction
/// and its magnitude is 0 below [`STICK_DEADZONE`], then rises smoothly to 1 at full
/// deflection.
pub fn stick_curve(raw: Vec2) -> Vec2 {
    let raw = raw.clamp_length_max(1.0);
    let magnitude = raw.length();
    if magnitude <= STICK_DEADZONE {
        return Vec2::ZERO;
    }
    let t = ((magnitude - STICK_DEADZONE) / (1.0 - STICK_DEADZONE)).clamp(0.0, 1.0);
    let shaped = t * t * (3.0 - 2.0 * t); // smoothstep
    raw.normalize_or_zero() * shaped
}

/// Merge one axis: a non-zero keyboard value wins outright over the stick.
///
/// Keyboard input is digital (−1/0/1), so a held key means full deflection; the stick
/// fills the axis only when the keyboard is neutral.
pub fn combine_axis(keyboard: f32, stick: f32) -> f32 {
    if keyboard != 0.0 {
        keyboard.clamp(-1.0, 1.0)
    } else {
        stick.clamp(-1.0, 1.0)
    }
}

/// One frame of raw device state, before deadzone, curve and source priority.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawInput {
    /// WASD / arrow keys as digital components in {-1, 0, 1} (x right, y forward).
    pub key_move: Vec2,
    /// Q (−1) / E (+1) camera yaw.
    pub key_look_x: f32,
    /// R (+1, closer) / F (−1, farther).
    pub key_zoom: f32,
    /// Keyboard actions held this frame.
    pub key_held: Vec<Action>,
    /// Keyboard actions that went down this frame.
    pub key_pressed: Vec<Action>,
    /// Left stick summed over connected gamepads (raw, no curve).
    pub pad_left_stick: Vec2,
    /// Right stick summed over connected gamepads (raw, no curve).
    pub pad_right_stick: Vec2,
    /// Gamepad actions held this frame.
    pub pad_held: Vec<Action>,
    /// Gamepad actions that went down this frame.
    pub pad_pressed: Vec<Action>,
}

impl RawInput {
    /// Snapshot the keyboard. Uses `bevy_input` types but no `App`, so it is unit testable.
    pub fn from_keyboard(keys: &ButtonInput<KeyCode>) -> Self {
        let mut key_move = Vec2::ZERO;
        if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
            key_move.y += 1.0;
        }
        if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
            key_move.y -= 1.0;
        }
        if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
            key_move.x -= 1.0;
        }
        if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
            key_move.x += 1.0;
        }

        let mut key_look_x = 0.0;
        if keys.pressed(KeyCode::KeyQ) {
            key_look_x -= 1.0;
        }
        if keys.pressed(KeyCode::KeyE) {
            key_look_x += 1.0;
        }

        let mut key_zoom = 0.0;
        if keys.pressed(KeyCode::KeyR) {
            key_zoom += 1.0;
        }
        if keys.pressed(KeyCode::KeyF) {
            key_zoom -= 1.0;
        }

        let mut key_held = Vec::new();
        if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
            key_held.push(Action::Sprint);
        }

        let mut key_pressed = Vec::new();
        if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space]) {
            key_pressed.push(Action::Confirm);
        }
        if keys.any_just_pressed([KeyCode::Escape, KeyCode::Backspace]) {
            key_pressed.push(Action::Cancel);
        }
        if keys.just_pressed(KeyCode::Escape) {
            key_pressed.push(Action::Menu);
        }
        if keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::ArrowLeft, KeyCode::KeyA]) {
            key_pressed.push(Action::Prev);
        }
        if keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::ArrowRight, KeyCode::KeyD]) {
            key_pressed.push(Action::Next);
        }
        for (slot, key) in [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
        ]
        .into_iter()
        .enumerate()
        {
            if keys.just_pressed(key) {
                key_pressed.push(Action::Slot(slot as u8));
            }
        }
        if keys.just_pressed(KeyCode::KeyF) {
            key_pressed.push(Action::OpenFight);
        }
        if keys.just_pressed(KeyCode::KeyS) {
            key_pressed.push(Action::OpenSwitch);
        }
        if keys.just_pressed(KeyCode::KeyI) {
            key_pressed.push(Action::OpenItem);
        }
        if keys.any_just_pressed([KeyCode::KeyC, KeyCode::KeyB]) {
            key_pressed.push(Action::OpenCapture);
        }
        if keys.just_pressed(KeyCode::KeyR) {
            key_pressed.push(Action::OpenRun);
        }
        if keys.just_pressed(KeyCode::KeyQ) {
            key_pressed.push(Action::StyleAgile);
        }
        if keys.just_pressed(KeyCode::KeyE) {
            key_pressed.push(Action::StyleStrong);
        }
        if keys.just_pressed(KeyCode::KeyY) {
            key_pressed.push(Action::Gimmick);
        }
        if keys.any_just_pressed([KeyCode::KeyZ, KeyCode::Tab]) {
            key_pressed.push(Action::Aim);
        }
        // H / Left Ctrl: free keys (Brief 22); the original bindings are not known.
        if keys.just_pressed(KeyCode::KeyH) {
            key_pressed.push(Action::Dodge);
        }
        if keys.just_pressed(KeyCode::ControlLeft) {
            key_pressed.push(Action::Crouch);
        }
        // K, not X: the viewer already uses X to throw bait while aiming.
        if keys.just_pressed(KeyCode::KeyK) {
            key_pressed.push(Action::Jump);
        }
        // V / G (Brief 38): free keys; the original call-ride binding is not decoded.
        if keys.just_pressed(KeyCode::KeyV) {
            key_pressed.push(Action::Ride);
        }
        if keys.just_pressed(KeyCode::KeyG) {
            key_pressed.push(Action::RideCycle);
        }

        Self {
            key_move,
            key_look_x,
            key_zoom,
            key_held,
            key_pressed,
            ..default()
        }
    }

    /// Fold one connected gamepad into this snapshot (sticks sum, digital edges dedupe).
    pub fn merge_gamepad(&mut self, gamepad: &Gamepad) {
        self.pad_left_stick += gamepad.left_stick();
        self.pad_right_stick += gamepad.right_stick();

        // The same physical button may map to a field action and a battle action
        // (e.g. Y: dodge in the world, Fight in battle); consumers are state gated.
        const BUTTONS: [(GamepadButton, Action); 18] = [
            (GamepadButton::South, Action::Confirm),
            (GamepadButton::East, Action::Cancel),
            (GamepadButton::Start, Action::Menu),
            (GamepadButton::Select, Action::Ride),
            (GamepadButton::DPadUp, Action::Prev),
            (GamepadButton::DPadLeft, Action::Prev),
            (GamepadButton::DPadDown, Action::Next),
            (GamepadButton::DPadRight, Action::Next),
            (GamepadButton::North, Action::Dodge),
                        (GamepadButton::North, Action::OpenFight),
            (GamepadButton::West, Action::OpenSwitch),
            (GamepadButton::West, Action::Jump),
            (GamepadButton::RightThumb, Action::Crouch),
            (GamepadButton::LeftTrigger, Action::OpenItem),
            (GamepadButton::RightTrigger, Action::OpenRun),
            (GamepadButton::RightTrigger2, Action::Throw),
            (GamepadButton::RightTrigger2, Action::OpenCapture),
            (GamepadButton::LeftTrigger2, Action::Aim),
        ];
        for (button, action) in BUTTONS {
            if gamepad.just_pressed(button) && !self.pad_pressed.contains(&action) {
                self.pad_pressed.push(action);
            }
        }
        if gamepad.pressed(GamepadButton::LeftThumb) && !self.pad_held.contains(&Action::Sprint) {
            self.pad_held.push(Action::Sprint);
        }
    }
}

/// The logical input state of one frame; the only resource gameplay systems read.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct ActionState {
    move_axis: Vec2,
    look_axis: Vec2,
    zoom: f32,
    held: Vec<Action>,
    pressed: Vec<Action>,
}

impl ActionState {
    /// Movement stick/keyboard axis (x right, y forward), components in [-1, 1].
    pub fn move_axis(&self) -> Vec2 {
        self.move_axis
    }

    /// Camera look axis (x right, y up), components in [-1, 1].
    pub fn look_axis(&self) -> Vec2 {
        self.look_axis
    }

    /// Zoom axis: +1 zooms in, −1 zooms out.
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Analog value of an axis action; 0 for digital actions.
    pub fn axis(&self, action: Action) -> f32 {
        match action {
            Action::MoveX => self.move_axis.x,
            Action::MoveY => self.move_axis.y,
            Action::LookX => self.look_axis.x,
            Action::LookY => self.look_axis.y,
            Action::ZoomIn => self.zoom,
            _ => 0.0,
        }
    }

    /// True while the action is held.
    pub fn pressed(&self, action: Action) -> bool {
        self.held.contains(&action)
    }

    /// True on the frame the action went down.
    pub fn just_pressed(&self, action: Action) -> bool {
        self.pressed.contains(&action)
    }

    /// Clear a just-pressed edge so later systems in the same frame do not consume it again
    /// (the pause menu consumes Menu/Cancel and its own navigation, like the old key clearing).
    pub fn consume(&mut self, action: Action) {
        self.pressed.retain(|a| *a != action);
    }
}

/// Pure mapping: one device snapshot in, the logical frame state out.
pub fn actions(raw: &RawInput) -> ActionState {
    let left = stick_curve(raw.pad_left_stick);
    let right = stick_curve(raw.pad_right_stick);
    let move_axis = Vec2::new(
        combine_axis(raw.key_move.x, left.x),
        combine_axis(raw.key_move.y, left.y),
    );
    let look_axis = Vec2::new(combine_axis(raw.key_look_x, right.x), right.y);

    let mut held = raw.key_held.clone();
    for action in &raw.pad_held {
        if !held.contains(action) {
            held.push(*action);
        }
    }
    let mut pressed = raw.key_pressed.clone();
    for action in &raw.pad_pressed {
        if !pressed.contains(action) {
            pressed.push(*action);
        }
    }

    ActionState {
        move_axis,
        look_axis,
        zoom: raw.key_zoom.clamp(-1.0, 1.0),
        held,
        pressed,
    }
}

// ------------------------------------------------------------------ bevy glue

/// Fills [`ActionState`] every frame from the keyboard and the connected gamepads.
///
/// The gamepad side needs the real `bevy_gilrs` backend (`GilrsPlugin` is part of
/// `DefaultPlugins` when the `bevy_gilrs` feature is on); without it the `Gamepad`
/// components simply never appear and the keyboard still works.
pub struct InputMapPlugin;

impl Plugin for InputMapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ActionState>()
            .add_systems(
                PreUpdate,
                fill_action_state.after(bevy::input::InputSystems),
            );
    }
}

/// Snapshot keyboard + gamepads into [`ActionState`]. Runs in `PreUpdate` after
/// `InputSystems` (so `just_pressed` edges are visible) and, in the viewer, after the
/// pause menu consumed its own edges.
pub fn fill_action_state(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut state: ResMut<ActionState>,
) {
    let mut raw = RawInput::from_keyboard(&keys);
    for gamepad in &gamepads {
        raw.merge_gamepad(gamepad);
    }
    *state = actions(&raw);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyboard(pressed: &[KeyCode]) -> ButtonInput<KeyCode> {
        let mut keys = ButtonInput::default();
        for key in pressed {
            keys.press(*key);
        }
        keys
    }

    fn gamepad(buttons: &[GamepadButton]) -> Gamepad {
        let mut pad = Gamepad::default();
        for button in buttons {
            pad.digital_mut().press(*button);
        }
        pad
    }

    // ---------------------------------------------------------- pure: sticks

    #[test]
    fn deadzone_is_fifteen_percent_and_curve_is_continuous() {
        assert_eq!(stick_curve(Vec2::new(0.10, 0.0)), Vec2::ZERO);
        assert_eq!(stick_curve(Vec2::new(0.15, 0.0)), Vec2::ZERO);
        let just_above = stick_curve(Vec2::new(0.16, 0.0));
        assert!(
            just_above.x > 0.0 && just_above.x < 0.01,
            "{}",
            just_above.x
        );
        assert!(just_above.y == 0.0);
    }

    #[test]
    fn curve_saturates_at_the_rim_and_keeps_direction() {
        let full = stick_curve(Vec2::new(0.6, 0.8));
        assert!((full.length() - 1.0).abs() < 1e-6);
        assert!((full.x - 0.6).abs() < 1e-6 && (full.y - 0.8).abs() < 1e-6);

        let diagonal = stick_curve(Vec2::new(1.0, 1.0));
        assert!((diagonal.length() - 1.0).abs() < 1e-6);
        assert!((diagonal.x - diagonal.y).abs() < 1e-6 && diagonal.x > 0.0);

        let negative = stick_curve(Vec2::new(-1.0, 0.0));
        assert!((negative.x + 1.0).abs() < 1e-6 && negative.y == 0.0);
    }

    #[test]
    fn curve_is_monotonic_with_magnitude() {
        let mut previous = -1.0;
        for step in 16..=100 {
            let magnitude = step as f32 / 100.0;
            let out = stick_curve(Vec2::new(magnitude, 0.0));
            assert!(out.x >= previous, "curve dipped at {magnitude}");
            previous = out.x;
        }
        assert!(previous > 0.999);
    }

    // ---------------------------------------------------------- pure: sources

    #[test]
    fn keyboard_maps_wasd_shift_and_menu_keys() {
        let raw = RawInput::from_keyboard(&keyboard(&[
            KeyCode::KeyW,
            KeyCode::KeyD,
            KeyCode::ShiftLeft,
        ]));
        assert_eq!(raw.key_move, Vec2::new(1.0, 1.0));
        assert_eq!(raw.key_held, vec![Action::Sprint]);
        // D is also battle/pause navigation (Next): held movement and edge action coexist.
        assert_eq!(raw.key_pressed, vec![Action::Next]);

        let keys = keyboard(&[
            KeyCode::Enter,
            KeyCode::Escape,
            KeyCode::Digit3,
            KeyCode::KeyC,
            KeyCode::KeyF,
            KeyCode::KeyZ,
        ]);
        let raw = RawInput::from_keyboard(&keys);
        for action in [
            Action::Confirm,
            Action::Cancel,
            Action::Menu,
            Action::Slot(2),
            Action::OpenCapture,
            Action::OpenFight,
            Action::Aim,
        ] {
            assert!(raw.key_pressed.contains(&action), "{action:?} missing");
        }
        // Escape must not masquerade as battle navigation or movement.
        assert!(!raw.key_pressed.contains(&Action::Prev));
        assert!(!raw.key_pressed.contains(&Action::Next));
    }

    #[test]
    fn keyboard_opposite_keys_cancel_out() {
        let raw = RawInput::from_keyboard(&keyboard(&[
            KeyCode::KeyA,
            KeyCode::KeyD,
            KeyCode::KeyW,
            KeyCode::KeyS,
        ]));
        assert_eq!(raw.key_move, Vec2::ZERO);
    }

    #[test]
    fn gamepad_maps_switch_style_buttons() {
        let mut raw = RawInput::default();
        raw.merge_gamepad(&gamepad(&[
            GamepadButton::South,
            GamepadButton::North,
            GamepadButton::West,
            GamepadButton::RightTrigger2,
            GamepadButton::LeftTrigger2,
            GamepadButton::LeftThumb,
        ]));
        let state = actions(&raw);
        assert!(state.just_pressed(Action::Confirm));
        assert!(state.just_pressed(Action::Dodge));
        assert!(state.just_pressed(Action::OpenFight));
        assert!(state.just_pressed(Action::OpenSwitch));
        assert!(state.just_pressed(Action::Throw));
        assert!(state.just_pressed(Action::OpenCapture));
        assert!(state.just_pressed(Action::Aim));
        assert!(state.pressed(Action::Sprint));
    }

    #[test]
    fn jump_maps_to_keyboard_k_and_pad_west() {
        let raw = RawInput::from_keyboard(&keyboard(&[KeyCode::KeyK]));
        assert_eq!(raw.key_pressed, vec![Action::Jump]);
        let mut raw = RawInput::default();
        raw.merge_gamepad(&gamepad(&[GamepadButton::West]));
        let state = actions(&raw);
        assert!(state.just_pressed(Action::Jump));
        // North stays the dodge button; it must not also jump.
        let mut raw = RawInput::default();
        raw.merge_gamepad(&gamepad(&[GamepadButton::North]));
        let state = actions(&raw);
        assert!(state.just_pressed(Action::Dodge));
        assert!(!state.just_pressed(Action::Jump));
    }

    #[test]
    fn dodge_and_crouch_have_keyboard_and_pad_bindings() {
        let raw = RawInput::from_keyboard(&keyboard(&[KeyCode::KeyH, KeyCode::ControlLeft]));
        assert!(raw.key_pressed.contains(&Action::Dodge));
        assert!(raw.key_pressed.contains(&Action::Crouch));
        let mut raw = RawInput::default();
        raw.merge_gamepad(&gamepad(&[GamepadButton::RightThumb]));
        assert!(actions(&raw).just_pressed(Action::Crouch));
    }

    #[test]
    fn gamepad_sticks_go_through_the_deadzone() {
        // A stick inside the deadzone magnitude reads as zero.
        let mut pad = Gamepad::default();
        pad.analog_mut().set(GamepadAxis::LeftStickX, 0.1);
        let mut raw = RawInput::default();
        raw.merge_gamepad(&pad);
        assert_eq!(actions(&raw).move_axis(), Vec2::ZERO);

        // Above it the axis is shaped by the curve; the deadzone is radial, not per axis.
        let mut pad = Gamepad::default();
        pad.analog_mut().set(GamepadAxis::LeftStickY, 0.8);
        pad.analog_mut().set(GamepadAxis::RightStickY, -1.0);
        let mut raw = RawInput::default();
        raw.merge_gamepad(&pad);
        let state = actions(&raw);
        assert_eq!(state.move_axis().x, 0.0);
        let expected_y = stick_curve(Vec2::new(0.0, 0.8)).y;
        assert!((state.move_axis().y - expected_y).abs() < 1e-6);
        assert!(expected_y > 0.7 && expected_y < 1.0);
        assert_eq!(state.look_axis().y, -1.0);
    }

    #[test]
    fn keyboard_wins_per_axis_over_the_stick() {
        let mut raw = RawInput {
            key_move: Vec2::new(-1.0, 0.0),
            ..default()
        };
        raw.pad_left_stick = Vec2::new(1.0, 1.0);
        let state = actions(&raw);
        // Keyboard forces x to −1; y stays on the stick (diagonal clamped to unit length).
        assert_eq!(state.move_axis().x, -1.0);
        assert!((state.move_axis().y - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    }

    #[test]
    fn merged_digital_edges_do_not_duplicate() {
        let mut pad = Gamepad::default();
        pad.digital_mut().press(GamepadButton::South);
        let mut raw = RawInput::from_keyboard(&keyboard(&[KeyCode::Enter]));
        raw.merge_gamepad(&pad);
        let state = actions(&raw);
        assert_eq!(state.pressed, vec![Action::Confirm]);
        let mut state = state;
        state.consume(Action::Confirm);
        assert!(!state.just_pressed(Action::Confirm));
    }

    #[test]
    fn analog_values_stay_in_range_when_sources_sum() {
        let mut raw = RawInput {
            key_move: Vec2::new(1.0, -1.0),
            ..default()
        };
        raw.pad_left_stick = Vec2::new(1.0, -1.0);
        let state = actions(&raw);
        assert_eq!(state.move_axis(), Vec2::new(1.0, -1.0));
        assert!(state.move_axis().abs().max_element() <= 1.0);
    }

    // ---------------------------------------------------------- bevy glue

    #[test]
    fn headless_app_fills_action_state_from_keyboard() {
        let mut app = App::new();
        app.add_plugins(InputMapPlugin);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.update();
        let state = app.world().resource::<ActionState>();
        assert_eq!(state.move_axis(), Vec2::new(0.0, 1.0));
        assert!(!state.pressed(Action::Sprint));
    }

    #[test]
    fn headless_app_fills_action_state_from_a_synthetic_gamepad() {
        let mut app = App::new();
        app.add_plugins(InputMapPlugin);
        let pad = app.world_mut().spawn(Gamepad::default()).id();
        {
            let mut gamepad = app.world_mut().get_mut::<Gamepad>(pad).unwrap();
            gamepad.analog_mut().set(GamepadAxis::LeftStickX, 0.5);
            gamepad.analog_mut().set(GamepadAxis::LeftStickY, -1.0);
            gamepad.digital_mut().press(GamepadButton::South);
            gamepad.digital_mut().press(GamepadButton::LeftThumb);
        }
        app.update();
        let state = app.world().resource::<ActionState>();
        let expected = stick_curve(Vec2::new(0.5, -1.0));
        assert!((state.move_axis() - expected).length() < 1e-6);
        assert!(state.move_axis().y < -0.5);
        assert!(state.just_pressed(Action::Confirm));
        assert!(state.pressed(Action::Sprint));
    }
}
