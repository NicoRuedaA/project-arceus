//! Pause menu (Esc or gamepad +): a Bevy UI overlay navigable with the keyboard
//! (arrows, W/S or A/D, D-pad, Enter/A) and the mouse (hover + click).
//!
//! Options: Resume, Invert camera X axis, Invert camera Y axis, Volume (master, cycles
//! 100/75/50/25/0 %), Back to safe spawn, Save game, Rest (heals the party, no clock yet), Quit.
//! The panel also shows the player's money and play time (from `PlayerProgress` and `PlayTime`).
//!
//! While [`Paused`] is `true` the plugin freezes the world by contract: every gameplay system must
//! carry `.run_if(pla::game::state::not_paused)` (the opposite condition is
//! `pla::game::state::is_paused`). The menu logic lives in plain functions ([`move_selection`],
//! [`activate`], [`option_label`], [`menu_info_text`]) so it is unit tested without a window; the
//! input edges come from [`crate::game::input_map::ActionState`] and are consumed here so the
//! battle does not also see them on the same frame.

use bevy::prelude::*;

use super::input_map::{Action, ActionState};
use super::play_time::{format_play_time, PlayTime};
use super::save_wire::{PlayerProgress, RestRequested, SaveRequested, SaveToast};
use super::settings::UserSettings;
use super::state::{GameState, Paused};

// ---------------------------------------------------------------- pure logic

/// One row of the menu, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuOption {
    Resume,
    InvertX,
    InvertY,
    Volume,
    SafeSpawn,
    Save,
    Rest,
    Quit,
}

/// Menu rows in display order.
pub const OPTIONS: [MenuOption; 8] = [
    MenuOption::Resume,
    MenuOption::InvertX,
    MenuOption::InvertY,
    MenuOption::Volume,
    MenuOption::SafeSpawn,
    MenuOption::Save,
    MenuOption::Rest,
    MenuOption::Quit,
];

/// What the caller must do after an option was activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Close the menu and unfreeze the world.
    Resume,
    /// A setting flipped: persist `settings.txt`.
    SettingsChanged,
    /// Teleport the player to the safe spawn of the current area.
    SafeSpawn,
    /// Save the game exactly like F5.
    Save,
    /// Heal the party (refused during a battle; no clock advance yet, Brief 9 owns the clock).
    Rest,
    /// Leave the program.
    Quit,
}

/// Move the highlighted row by `delta`, wrapping around both ends.
pub fn move_selection(selected: usize, delta: i32) -> usize {
    let n = OPTIONS.len() as i32;
    (selected as i32 + delta).rem_euclid(n) as usize
}

/// Activate a row. Toggle rows flip their setting in place.
pub fn activate(option: MenuOption, settings: &mut UserSettings) -> MenuAction {
    match option {
        MenuOption::Resume => MenuAction::Resume,
        MenuOption::InvertX => {
            settings.invert_camera_x = !settings.invert_camera_x;
            MenuAction::SettingsChanged
        }
        MenuOption::InvertY => {
            settings.invert_camera_y = !settings.invert_camera_y;
            MenuAction::SettingsChanged
        }
        MenuOption::Volume => {
            settings.master_volume = next_volume_step(settings.master_volume);
            MenuAction::SettingsChanged
        }
        MenuOption::SafeSpawn => MenuAction::SafeSpawn,
        MenuOption::Save => MenuAction::Save,
        MenuOption::Rest => MenuAction::Rest,
        MenuOption::Quit => MenuAction::Quit,
    }
}

fn on_off(v: bool) -> &'static str {
    if v {
        "ON"
    } else {
        "OFF"
    }
}

/// Text shown for a row.
pub fn option_label(option: MenuOption, settings: &UserSettings) -> String {
    match option {
        MenuOption::Resume => "Resume".into(),
        MenuOption::InvertX => format!("Invert camera X axis: {}", on_off(settings.invert_camera_x)),
        MenuOption::InvertY => format!("Invert camera Y axis: {}", on_off(settings.invert_camera_y)),
        MenuOption::Volume => format!("Volume: {}%", settings.master_volume),
        MenuOption::SafeSpawn => "Back to safe spawn".into(),
        MenuOption::Save => "Save game".into(),
        MenuOption::Rest => "Rest (heal party)".into(),
        MenuOption::Quit => "Quit".into(),
    }
}

/// One-line player summary shown under the rows: money and play time.
pub fn menu_info_text(money: u32, play_seconds: u64) -> String {
    format!("Money: {money}   ·   Play time: {}", format_play_time(play_seconds))
}

/// Next master volume of the Volume row: 100 -> 75 -> 50 -> 25 -> 0 -> 100 (a value between
/// steps goes down to the next step).
pub fn next_volume_step(v: u8) -> u8 {
    match v {
        0 => 100,
        v => (v.saturating_sub(1) / 25) * 25,
    }
}

/// Esc toggles the menu; returns the new paused flag.
pub fn toggle_paused(paused: bool) -> bool {
    !paused
}

// ---------------------------------------------------------------- bevy glue

/// Set to `true` by the pause menu; the game teleports the player to the safe spawn and clears it.
#[derive(Resource, Debug, Default)]
pub struct SafeSpawnRequested(pub bool);

/// Highlighted row.
#[derive(Resource, Debug, Default)]
pub struct PauseMenu {
    pub selected: usize,
}

/// Directory of `settings.txt`.
#[derive(Resource, Debug, Clone)]
pub struct SettingsDir(pub std::path::PathBuf);

#[derive(Component)]
struct PauseRoot;
#[derive(Component)]
struct PauseButton(usize);
#[derive(Component)]
struct PauseButtonText(usize);
#[derive(Component)]
struct PauseInfoText;

pub struct PauseMenuPlugin;

impl PauseMenuPlugin {
    /// Insert the resources and systems. `settings` was loaded from `dir` at start-up.
    pub fn install(app: &mut App, settings: UserSettings, dir: std::path::PathBuf) {
        app.insert_resource(settings)
            .insert_resource(SettingsDir(dir))
            .init_resource::<Paused>()
            .init_resource::<PauseMenu>()
            .init_resource::<SaveToast>()
            .init_resource::<SaveRequested>()
            .init_resource::<RestRequested>()
            .init_resource::<SafeSpawnRequested>()
            // Also initialised by `InputMapPlugin`; kept here so the menu can live without it.
            .init_resource::<ActionState>();
        app.add_systems(Startup, spawn_menu)
            // After `InputSystems` (so key/button edges are fresh) and after the action
            // mapping so the menu can consume its own edges before the gameplay systems of
            // `Update` read the same frame, and before every gameplay system so they already
            // observe the new paused flag.
            .add_systems(
                PreUpdate,
                pause_input
                    .after(bevy::input::InputSystems)
                    .after(super::input_map::fill_action_state),
            )
            .add_systems(Update, sync_menu);
    }
}

fn spawn_menu(mut commands: Commands) {
    commands
        .spawn((
            PauseRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(100),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(24.0)),
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.95)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("PAUSED"),
                    TextFont::from_font_size(34.0),
                    TextColor(Color::srgb(0.95, 0.9, 0.6)),
                ));
                for (i, _) in OPTIONS.iter().enumerate() {
                    panel
                        .spawn((
                            Button,
                            PauseButton(i),
                            Node {
                                width: Val::Px(420.0),
                                padding: UiRect::axes(Val::Px(16.0), Val::Px(9.0)),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.15, 0.2, 0.28, 0.9)),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                PauseButtonText(i),
                                Text::new(""),
                                TextFont::from_font_size(22.0),
                                TextColor(Color::WHITE),
                            ));
                        });
                }
                panel.spawn((
                    PauseInfoText,
                    Text::new(""),
                    TextFont::from_font_size(16.0),
                    TextColor(Color::srgb(0.85, 0.85, 0.6)),
                ));
                panel.spawn((
                    Text::new("Arrows, W/S or D-pad + Enter/A, or click. Esc or + resumes."),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
                ));
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn pause_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut actions: ResMut<ActionState>,
    mut paused: ResMut<Paused>,
    mut menu: ResMut<PauseMenu>,
    mut settings: ResMut<UserSettings>,
    dir: Res<SettingsDir>,
    mut save_request: ResMut<SaveRequested>,
    mut rest_request: ResMut<RestRequested>,
    mut safe_request: ResMut<SafeSpawnRequested>,
    mut toast: ResMut<SaveToast>,
    buttons: Query<(&PauseButton, &Interaction), Changed<Interaction>>,
    mut exit: MessageWriter<AppExit>,
    state: Option<Res<State<GameState>>>,
) {
    // The main menu owns the input while it is open (Escape backs out of its options
    // page); never show the pause overlay on top of it.
    if state.is_some_and(|s| *s.get() == GameState::MainMenu) {
        return;
    }
    // Menu (Esc or gamepad +) toggles. The action edges are consumed so the battle
    // ("back") never sees Esc on the same frame; Enter/Space keep their raw readers in the
    // world (creature encounter, ball throw) so the consumed edge is mirrored there too.
    if actions.just_pressed(Action::Menu) {
        actions.consume(Action::Menu);
        actions.consume(Action::Cancel);
        paused.0 = toggle_paused(paused.0);
        if paused.0 {
            menu.selected = 0;
        }
        return;
    }
    if !paused.0 {
        return;
    }

    let mut activated: Option<usize> = None;
    let mut delta: i32 = 0;
    if actions.just_pressed(Action::Prev) {
        actions.consume(Action::Prev);
        delta -= 1;
    }
    if actions.just_pressed(Action::Next) {
        actions.consume(Action::Next);
        delta += 1;
    }
    // W/S keep navigating the (vertical) menu as before the action layer: they are movement
    // keys, not Prev/Next actions, and S is also the battle Switch action, so it is consumed.
    if keys.just_pressed(KeyCode::KeyW) {
        keys.clear_just_pressed(KeyCode::KeyW);
        delta -= 1;
    }
    if keys.just_pressed(KeyCode::KeyS) {
        keys.clear_just_pressed(KeyCode::KeyS);
        actions.consume(Action::OpenSwitch);
        delta += 1;
    }
    if delta != 0 {
        menu.selected = move_selection(menu.selected, delta.signum());
    }
    if actions.just_pressed(Action::Confirm) {
        actions.consume(Action::Confirm);
        activated = Some(menu.selected);
    }
    for (b, interaction) in &buttons {
        match interaction {
            Interaction::Hovered => menu.selected = b.0,
            Interaction::Pressed => {
                menu.selected = b.0;
                activated = Some(b.0);
            }
            Interaction::None => {}
        }
    }

    let Some(i) = activated else { return };
    // The click must not also act on the world (e.g. throw a ball) once the menu closes;
    // same for the confirm keys that still have raw readers in the world.
    mouse.clear_just_pressed(MouseButton::Left);
    for key in [KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space] {
        keys.clear_just_pressed(key);
    }
    match activate(OPTIONS[i], &mut settings) {
        MenuAction::Resume => paused.0 = false,
        MenuAction::SettingsChanged => {
            if let Err(e) = settings.save_to_dir(&dir.0) {
                toast.show(format!("Could not save settings: {e}"));
            }
        }
        MenuAction::SafeSpawn => {
            safe_request.0 = true;
            paused.0 = false;
        }
        MenuAction::Save => save_request.0 = true,
        MenuAction::Rest => rest_request.0 = true,
        MenuAction::Quit => {
            exit.write(AppExit::Success);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_menu(
    paused: Res<Paused>,
    menu: Res<PauseMenu>,
    settings: Res<UserSettings>,
    progress: Option<Res<PlayerProgress>>,
    play: Option<Res<PlayTime>>,
    mut root: Query<&mut Visibility, With<PauseRoot>>,
    mut buttons: Query<(&PauseButton, &mut BackgroundColor)>,
    mut texts: Query<(&PauseButtonText, &mut Text)>,
    mut info: Query<&mut Text, (With<PauseInfoText>, Without<PauseButtonText>)>,
) {
    for mut v in &mut root {
        let want = if paused.0 { Visibility::Visible } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
    }
    if !paused.0 {
        return;
    }
    for (b, mut bg) in &mut buttons {
        bg.0 = if b.0 == menu.selected {
            Color::srgba(0.85, 0.6, 0.15, 0.95)
        } else {
            Color::srgba(0.15, 0.2, 0.28, 0.9)
        };
    }
    for (t, mut text) in &mut texts {
        let label = option_label(OPTIONS[t.0], &settings);
        if text.0 != label {
            text.0 = label;
        }
    }
    let label = menu_info_text(
        progress.as_ref().map_or(0, |p| p.money),
        play.as_ref().map_or(0, |p| p.seconds),
    );
    for mut text in &mut info {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_wraps_both_ways() {
        assert_eq!(move_selection(0, -1), OPTIONS.len() - 1);
        assert_eq!(move_selection(OPTIONS.len() - 1, 1), 0);
        assert_eq!(move_selection(1, 1), 2);
        assert_eq!(move_selection(2, -1), 1);
    }

    #[test]
    fn toggles_flip_only_their_axis_and_show_on_off() {
        let mut s = UserSettings::default();
        assert_eq!(option_label(MenuOption::InvertX, &s), "Invert camera X axis: OFF");
        assert_eq!(activate(MenuOption::InvertX, &mut s), MenuAction::SettingsChanged);
        assert!(s.invert_camera_x && !s.invert_camera_y);
        assert_eq!(option_label(MenuOption::InvertX, &s), "Invert camera X axis: ON");
        assert_eq!(option_label(MenuOption::InvertY, &s), "Invert camera Y axis: OFF");
        assert_eq!(activate(MenuOption::InvertY, &mut s), MenuAction::SettingsChanged);
        assert!(s.invert_camera_x && s.invert_camera_y);
        assert_eq!(activate(MenuOption::InvertX, &mut s), MenuAction::SettingsChanged);
        assert!(!s.invert_camera_x && s.invert_camera_y);
    }

    #[test]
    fn other_rows_map_to_actions_without_touching_settings() {
        let mut s = UserSettings::default();
        assert_eq!(activate(MenuOption::Resume, &mut s), MenuAction::Resume);
        assert_eq!(activate(MenuOption::Save, &mut s), MenuAction::Save);
        assert_eq!(activate(MenuOption::SafeSpawn, &mut s), MenuAction::SafeSpawn);
        assert_eq!(option_label(MenuOption::SafeSpawn, &s), "Back to safe spawn");
        assert_eq!(activate(MenuOption::Rest, &mut s), MenuAction::Rest);
        assert_eq!(option_label(MenuOption::Rest, &s), "Rest (heal party)");
        assert_eq!(activate(MenuOption::Quit, &mut s), MenuAction::Quit);
        assert_eq!(s, UserSettings::default());
    }

    #[test]
    fn safe_spawn_row_is_reachable_by_navigation() {
        let i = OPTIONS.iter().position(|o| *o == MenuOption::SafeSpawn).unwrap();
        assert_eq!(OPTIONS.len(), 8);
        assert_eq!(move_selection(i - 1, 1), i);
        assert_eq!(move_selection(i, 1), i + 1);
    }

    #[test]
    fn rest_row_sits_between_save_and_quit() {
        let rest = OPTIONS.iter().position(|o| *o == MenuOption::Rest).unwrap();
        assert_eq!(OPTIONS[rest - 1], MenuOption::Save);
        assert_eq!(OPTIONS[rest + 1], MenuOption::Quit);
    }

    #[test]
    fn info_line_shows_money_and_play_time() {
        assert_eq!(menu_info_text(1234, 3661), "Money: 1234   ·   Play time: 1:01:01");
        assert_eq!(menu_info_text(0, 0), "Money: 0   ·   Play time: 0:00:00");
    }

    #[test]
    fn volume_row_cycles_master_volume() {
        let mut s = UserSettings::default();
        assert_eq!(option_label(MenuOption::Volume, &s), "Volume: 100%");
        let mut seen = Vec::new();
        for _ in 0..5 {
            assert_eq!(activate(MenuOption::Volume, &mut s), MenuAction::SettingsChanged);
            seen.push(s.master_volume);
        }
        assert_eq!(seen, vec![75, 50, 25, 0, 100]);
        assert_eq!(next_volume_step(60), 50);
        assert_eq!(next_volume_step(1), 0);
        assert_eq!(s.music_volume, 100, "only the master volume changes");
    }

    #[test]
    fn esc_toggles() {
        assert!(toggle_paused(false));
        assert!(!toggle_paused(true));
    }

    #[test]
    fn toggled_setting_survives_a_restart_in_a_temp_dir() {
        let dir = std::env::temp_dir().join(format!("pla_pause_settings_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = UserSettings::load_from_dir(&dir);
        activate(MenuOption::InvertY, &mut s);
        s.save_to_dir(&dir).unwrap();
        let reloaded = UserSettings::load_from_dir(&dir);
        assert!(reloaded.invert_camera_y && !reloaded.invert_camera_x);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Frame helper for headless runs: refresh key edges, hold `down`, release `up`, update.
    fn step(app: &mut App, down: &[KeyCode], up: &[KeyCode]) {
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.clear();
            for key in down {
                keys.press(*key);
            }
            for key in up {
                keys.release(*key);
            }
        }
        app.update();
    }

    #[test]
    fn headless_app_esc_toggles_and_consumes_its_edges_for_the_battle() {
        let dir = std::env::temp_dir().join(format!("pla_pause_app_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.add_plugins(crate::game::input_map::InputMapPlugin);
        PauseMenuPlugin::install(&mut app, UserSettings::default(), dir.clone());
        step(&mut app, &[], &[]);
        assert!(!app.world().resource::<Paused>().0);

        step(&mut app, &[KeyCode::Escape], &[]);
        assert!(app.world().resource::<Paused>().0, "Esc must open the menu");
        // The battle reads the same ActionState later in the frame: both edges must be gone.
        let actions = app.world().resource::<ActionState>();
        assert!(!actions.just_pressed(Action::Menu));
        assert!(!actions.just_pressed(Action::Cancel));

        // Still held: no new edge, the menu stays open.
        step(&mut app, &[], &[]);
        assert!(app.world().resource::<Paused>().0);

        // Release and press again: closes.
        step(&mut app, &[], &[KeyCode::Escape]);
        step(&mut app, &[KeyCode::Escape], &[]);
        assert!(!app.world().resource::<Paused>().0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn headless_app_navigates_and_confirms_with_consumed_edges() {
        let dir = std::env::temp_dir().join(format!("pla_pause_nav_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.add_plugins(crate::game::input_map::InputMapPlugin);
        PauseMenuPlugin::install(&mut app, UserSettings::default(), dir.clone());
        step(&mut app, &[], &[]);

        step(&mut app, &[KeyCode::Escape], &[]);
        assert_eq!(app.world().resource::<PauseMenu>().selected, 0);

        // D-pad/A-D/arrow navigation reaches the second row (Invert camera X axis).
        step(&mut app, &[KeyCode::ArrowDown], &[]);
        assert_eq!(app.world().resource::<PauseMenu>().selected, 1);

        step(&mut app, &[KeyCode::Enter], &[]);
        assert!(app.world().resource::<UserSettings>().invert_camera_x);
        assert!(
            app.world().resource::<Paused>().0,
            "a toggle row keeps the menu open"
        );
        // Enter is consumed for the raw world readers (creature encounter reads it directly).
        assert!(!app
            .world()
            .resource::<ButtonInput<KeyCode>>()
            .just_pressed(KeyCode::Enter));

        // W/S still move through the vertical menu; S does not leak as the battle Switch.
        step(&mut app, &[KeyCode::KeyS], &[KeyCode::ArrowDown, KeyCode::Enter]);
        assert_eq!(app.world().resource::<PauseMenu>().selected, 2);
        assert!(!app
            .world()
            .resource::<ActionState>()
            .just_pressed(Action::OpenSwitch));
        step(&mut app, &[KeyCode::KeyW], &[KeyCode::KeyS]);
        assert_eq!(app.world().resource::<PauseMenu>().selected, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
