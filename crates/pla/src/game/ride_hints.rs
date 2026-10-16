//! On-screen control hints for riding (Brief 38 follow-up).
//!
//! The user feedback that opened this work: the ride keys were impossible to discover (the
//! guideless player pressed nothing, because nothing told them what to press). One Bevy-UI
//! line follows the port's existing bottom-anchored prompt pattern (`game::door_pairs`):
//!
//! * **Ride controls** — appears when a mount is summoned (and again on every mount change)
//!   and stays [`CONTROLS_HINT_TIME`] seconds: the port's keys for that mount, including its
//!   ability. The text itself is `game::ride::mount_controls_line` (single source, tested
//!   there); the original's own ride bindings are not decoded (`reports/data-progress/
//!   gamepad_input.md`), so this line IS the documentation of the port's mapping.
//!
//! Climbing has its own contextual prompt (`game::interact::climb_hint_line`, rendered by the
//! viewer): this module does not duplicate it.

use bevy::prelude::*;

use super::ride::{RideKind, RideState};
use super::state::{not_paused, GameState};

/// Seconds the ride controls line stays visible after mounting / switching mounts.
pub const CONTROLS_HINT_TIME: f32 = 12.0;

/// Complete controls line for a mount: the single source lives in `game::ride`
/// ([`super::ride::mount_controls_line`]) so the keys and their tests stay in one place.
pub fn controls_text(kind: RideKind) -> String {
    super::ride::mount_controls_line(kind)
}

/// What the ride line shows and for how long; `last_mounted` detects the summon / mount change.
#[derive(Resource, Debug, Default, PartialEq)]
pub struct RideHints {
    pub controls: String,
    pub controls_timer: f32,
    pub last_mounted: Option<RideKind>,
}

#[derive(Component)]
struct RideControlsText;

fn spawn_hint_text(mut commands: Commands) {
    commands.spawn((
        RideControlsText,
        Text::new(""),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgb(0.95, 0.95, 0.80)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(58.0),
            left: Val::Percent(6.0),
            ..default()
        },
        Visibility::Hidden,
    ));
}

/// Updates the hint state from the ride state.
fn update_hints(time: Res<Time>, ride: Res<RideState>, mut hints: ResMut<RideHints>) {
    let dt = time.delta_secs();
    // Summon / dismount / mount change: (re)show the controls line.
    if ride.mounted != hints.last_mounted {
        hints.last_mounted = ride.mounted;
        match ride.mounted {
            Some(kind) => {
                hints.controls = controls_text(kind);
                hints.controls_timer = CONTROLS_HINT_TIME;
            }
            None => {
                hints.controls.clear();
                hints.controls_timer = 0.0;
            }
        }
    } else if hints.controls_timer > 0.0 {
        hints.controls_timer = (hints.controls_timer - dt).max(0.0);
        if hints.controls_timer == 0.0 {
            hints.controls.clear();
        }
    }
}

fn show_hints(
    hints: Res<RideHints>,
    mut controls: Query<(&mut Text, &mut Visibility), With<RideControlsText>>,
) {
    if !hints.is_changed() {
        return;
    }
    for (mut text, mut vis) in &mut controls {
        if text.0 != hints.controls {
            text.0 = hints.controls.clone();
        }
        let want = if hints.controls.is_empty() { Visibility::Hidden } else { Visibility::Visible };
        if *vis != want {
            *vis = want;
        }
    }
}

/// Registers the hint texts. Add in the World state setup.
pub struct RideHintsPlugin;

impl Plugin for RideHintsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RideHints>();
        app.add_systems(Startup, spawn_hint_text);
        app.add_systems(
            Update,
            (update_hints, show_hints)
                .chain()
                .after(super::ride::RideSystems)
                .run_if(in_state(GameState::World))
                .run_if(not_paused),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_line_names_the_keys_of_each_mount() {
        for kind in RideKind::ALL {
            let text = controls_text(kind);
            assert!(text.contains("Shift"), "{kind:?}: {text}");
            assert!(text.contains("Desmontar: V"), "{kind:?}: {text}");
            assert!(text.contains(&format!("{kind:?}")), "{kind:?}: {text}");
        }
        assert!(controls_text(RideKind::Ursaluna).contains("H"));
        assert!(controls_text(RideKind::Braviary).contains("aire"));
    }

    #[test]
    fn headless_hints_follow_the_ride_state() {
        use crate::game::input_map::{actions, RawInput};
        use crate::game::ride::RidePlugin;
        use bevy::state::app::StatesPlugin;
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            1.0 / 60.0,
        )));
        app.init_state::<GameState>();
        app.add_plugins((RidePlugin, RideHintsPlugin));
        app.world_mut().spawn((crate::game::PlayerAvatar, Transform::default()));

        // Mount with a real V edge.
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyV);
        app.insert_resource(actions(&RawInput::from_keyboard(&keys)));
        app.update();
        let hints = app.world().resource::<RideHints>();
        assert_eq!(hints.last_mounted, Some(RideKind::Wyrdeer));
        assert!(hints.controls.contains("Wyrdeer"), "{}", hints.controls);
        assert!(hints.controls_timer > 0.0);

        // The line clears after its time runs out.
        for _ in 0..(CONTROLS_HINT_TIME * 60.0) as i32 + 2 {
            app.insert_resource(actions(&RawInput::default()));
            app.update();
        }
        let hints = app.world().resource::<RideHints>();
        assert!(hints.controls.is_empty(), "{}", hints.controls);

        // Dismount hides it (and the timer stays off).
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyV);
        app.insert_resource(actions(&RawInput::from_keyboard(&keys)));
        app.update();
        let hints = app.world().resource::<RideHints>();
        assert_eq!(hints.last_mounted, None);
        assert!(hints.controls.is_empty());
    }
}
