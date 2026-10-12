//! Pause menu (Esc): a Bevy UI overlay navigable with the keyboard (arrows or W/S + Enter) and
//! the mouse (hover + click).
//!
//! Options: Resume, Invert camera X axis, Invert camera Y axis, Save game, Quit.
//!
//! While [`Paused`] is `true` the plugin freezes the world by contract: every gameplay system must
//! carry `.run_if(pla::game::state::not_paused)` (the opposite condition is
//! `pla::game::state::is_paused`). The menu logic lives in plain functions ([`move_selection`],
//! [`activate`], [`option_label`]) so it is unit tested without a window.

use bevy::prelude::*;

use super::save_wire::{SaveRequested, SaveToast};
use super::settings::UserSettings;
use super::state::Paused;

// ---------------------------------------------------------------- pure logic

/// One row of the menu, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuOption {
    Resume,
    InvertX,
    InvertY,
    Save,
    Quit,
}

/// Menu rows in display order.
pub const OPTIONS: [MenuOption; 5] = [
    MenuOption::Resume,
    MenuOption::InvertX,
    MenuOption::InvertY,
    MenuOption::Save,
    MenuOption::Quit,
];

/// What the caller must do after an option was activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Close the menu and unfreeze the world.
    Resume,
    /// A setting flipped: persist `settings.txt`.
    SettingsChanged,
    /// Save the game exactly like F5.
    Save,
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
        MenuOption::Save => MenuAction::Save,
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
        MenuOption::Save => "Save game".into(),
        MenuOption::Quit => "Quit".into(),
    }
}

/// Esc toggles the menu; returns the new paused flag.
pub fn toggle_paused(paused: bool) -> bool {
    !paused
}

// ---------------------------------------------------------------- bevy glue

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

pub struct PauseMenuPlugin;

impl PauseMenuPlugin {
    /// Insert the resources and systems. `settings` was loaded from `dir` at start-up.
    pub fn install(app: &mut App, settings: UserSettings, dir: std::path::PathBuf) {
        app.insert_resource(settings)
            .insert_resource(SettingsDir(dir))
            .init_resource::<Paused>()
            .init_resource::<PauseMenu>()
            .init_resource::<SaveToast>()
            .init_resource::<SaveRequested>();
        app.add_systems(Startup, spawn_menu)
            // After the input systems so Esc is seen this frame, before every gameplay system
            // of `Update` so they already observe the new paused flag.
            .add_systems(PreUpdate, pause_input.after(bevy::input::InputSystems))
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
                    Text::new("Up/Down or W/S + Enter, or click. Esc resumes."),
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
    mut paused: ResMut<Paused>,
    mut menu: ResMut<PauseMenu>,
    mut settings: ResMut<UserSettings>,
    dir: Res<SettingsDir>,
    mut save_request: ResMut<SaveRequested>,
    mut toast: ResMut<SaveToast>,
    buttons: Query<(&PauseButton, &Interaction), Changed<Interaction>>,
    mut exit: MessageWriter<AppExit>,
) {
    // Esc toggles. The key is consumed so the battle ("back") never sees it on the same frame.
    if keys.just_pressed(KeyCode::Escape) {
        keys.clear_just_pressed(KeyCode::Escape);
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
    for k in [KeyCode::ArrowUp, KeyCode::KeyW] {
        if keys.just_pressed(k) {
            delta -= 1;
            keys.clear_just_pressed(k);
        }
    }
    for k in [KeyCode::ArrowDown, KeyCode::KeyS] {
        if keys.just_pressed(k) {
            delta += 1;
            keys.clear_just_pressed(k);
        }
    }
    if delta != 0 {
        menu.selected = move_selection(menu.selected, delta.signum());
    }
    for k in [KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space] {
        if keys.just_pressed(k) {
            keys.clear_just_pressed(k);
            activated = Some(menu.selected);
        }
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
    // The click must not also act on the world (e.g. throw a ball) once the menu closes.
    mouse.clear_just_pressed(MouseButton::Left);
    match activate(OPTIONS[i], &mut settings) {
        MenuAction::Resume => paused.0 = false,
        MenuAction::SettingsChanged => {
            if let Err(e) = settings.save_to_dir(&dir.0) {
                toast.show(format!("Could not save settings: {e}"));
            }
        }
        MenuAction::Save => save_request.0 = true,
        MenuAction::Quit => {
            exit.write(AppExit::Success);
        }
    }
}

fn sync_menu(
    paused: Res<Paused>,
    menu: Res<PauseMenu>,
    settings: Res<UserSettings>,
    mut root: Query<&mut Visibility, With<PauseRoot>>,
    mut buttons: Query<(&PauseButton, &mut BackgroundColor)>,
    mut texts: Query<(&PauseButtonText, &mut Text)>,
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
        assert_eq!(activate(MenuOption::Quit, &mut s), MenuAction::Quit);
        assert_eq!(s, UserSettings::default());
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
}
