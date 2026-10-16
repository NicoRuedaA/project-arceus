//! On-screen control hints for riding (Brief 38 follow-up) and for climbable cliffs.
//!
//! The user feedback that opened this work: the ride keys were impossible to discover (the
//! guideless player pressed nothing, because nothing told them what to press). Two Bevy-UI
//! lines follow the port's existing bottom-anchored prompt pattern (`game::door_pairs`):
//!
//! * **Ride controls** — appears when a mount is summoned (and again on every mount change)
//!   and stays [`CONTROLS_HINT_TIME`] seconds: the port's keys for that mount, including its
//!   ability. The original's own ride bindings are not decoded (`reports/data-progress/
//!   gamepad_input.md`), so this line IS the documentation of the port's mapping.
//! * **Climb** — shown while an on-foot avatar stands in reach of a [`Climbable`] cliff
//!   (the same reach the climb grab uses). Climbing itself is Brief 13's / the trepar
//!   workstream's; this module only surfaces the key.

use bevy::prelude::*;

use super::interact::{nearest_climbable, Climbable};
use super::player_motion::GROUND_MARGIN;
use super::ride::{RideKind, RideState};
use super::state::{not_paused, GameState};
use super::PlayerAvatar;

/// Seconds the ride controls line stays visible after mounting / switching mounts.
pub const CONTROLS_HINT_TIME: f32 = 12.0;

/// Label of each mount on the controls line (Spanish, the user-facing game language of the
/// project guides; the species names are the game's own).
pub fn mount_label(kind: RideKind) -> &'static str {
    match kind {
        RideKind::Wyrdeer => "Wyrdeer",
        RideKind::Ursaluna => "Ursaluna",
        RideKind::Basculegion => "Basculegion",
        RideKind::Sneasler => "Sneasler",
        RideKind::Braviary => "Braviary",
    }
}

/// Ability fragment of the controls line for `kind` (empty when the port has no extra key:
/// Wyrdeer's and Basculegion's abilities ride along the normal movement keys). Separators stay
/// ASCII: the game's own font has no middle dot / em dash glyphs (they render as boxes).
pub fn ability_text(kind: RideKind) -> &'static str {
    match kind {
        RideKind::Wyrdeer => "",
        RideKind::Ursaluna => " | Buscar/cavar: H",
        RideKind::Basculegion => " | Nada rapido en el agua",
        RideKind::Sneasler => "",
        RideKind::Braviary => " | Volar: salta y pulsa K en el aire | Picado: Shift",
    }
}

/// Complete controls line for a mount.
pub fn controls_text(kind: RideKind) -> String {
    format!(
        "{} - Correr: Shift | Saltar: K{} | Cambiar montura: G | Desmontar: V",
        mount_label(kind),
        ability_text(kind),
    )
}

/// Text of the climb hint: the grab is the `Confirm` action (Enter / Space, Brief 13).
pub const CLIMB_HINT_TEXT: &str = "Trepar: Espacio";

/// What the ride line shows and for how long; `last_mounted` detects the summon / mount change.
#[derive(Resource, Debug, Default, PartialEq)]
pub struct RideHints {
    pub controls: String,
    pub controls_timer: f32,
    pub climb_visible: bool,
    pub last_mounted: Option<RideKind>,
}

#[derive(Component)]
struct RideControlsText;

#[derive(Component)]
struct ClimbHintText;

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
    commands.spawn((
        ClimbHintText,
        Text::new(""),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgb(0.95, 0.95, 0.80)),
        // Above the minimap corner and clear of the ride line below (both hide behind it
        // otherwise: measured with the minimap occupying the bottom-left ~230x210 px).
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(120.0),
            left: Val::Percent(30.0),
            ..default()
        },
        Visibility::Hidden,
    ));
}

/// Updates the hint state from the ride state and the nearby climbables.
fn update_hints(
    time: Res<Time>,
    ride: Res<RideState>,
    avatar: Query<&Transform, With<PlayerAvatar>>,
    climbables: Query<&Climbable>,
    mut hints: ResMut<RideHints>,
) {
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
    // Climb hint: on foot, within the grab reach of a climbable surface.
    let climb = ride.mounted.is_none()
        && avatar
            .single()
            .ok()
            .is_some_and(|tf| nearest_climbable(climbables.iter(), tf.translation, GROUND_MARGIN).is_some());
    if hints.climb_visible != climb {
        hints.climb_visible = climb;
    }
}

fn show_hints(
    hints: Res<RideHints>,
    mut controls: Query<(&mut Text, &mut Visibility), (With<RideControlsText>, Without<ClimbHintText>)>,
    mut climb: Query<(&mut Text, &mut Visibility), (With<ClimbHintText>, Without<RideControlsText>)>,
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
    for (mut text, mut vis) in &mut climb {
        if text.0 != CLIMB_HINT_TEXT {
            text.0 = CLIMB_HINT_TEXT.to_string();
        }
        let want = if hints.climb_visible { Visibility::Visible } else { Visibility::Hidden };
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
            assert!(text.contains(mount_label(kind)), "{kind:?}: {text}");
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
        app.world_mut().spawn((PlayerAvatar, Transform::default()));

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
