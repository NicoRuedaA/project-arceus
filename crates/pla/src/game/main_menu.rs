//! Main menu (`--menu`): Continue / New game / Options / Exit before the world runs.
//!
//! The viewer starts in [`GameState::MainMenu`] only when `--menu` is passed, so the
//! direct start and the scripted runs (`--auto-demo`, `--script`) keep booting into
//! [`GameState::World`]. The menu overlays the (frozen) world: the world systems are
//! already gated on `in_state(GameState::World)`, so nothing moves while it is open.
//!
//! Continue carries the save loaded at start-up (it is disabled when there is no valid
//! [`SaveGame`]). New game resets the in-memory [`PlayerProgress`] to a fresh game
//! (keeping the same starter party/bag the direct start seeds), asks for the safe spawn
//! and leaves the file on disk untouched until the next save, exactly like `--new-game`.
//! Options edits and persists [`UserSettings`] (master volume, field of view, shadows,
//! quality) immediately; the page is reachable from the main menu.
//!
//! Input comes from [`ActionState`] (keyboard + gamepad, brief 3) plus the mouse; the
//! menu consumes its own edges so the world never sees the same press on the frame the
//! state changes. The pure logic ([`move_selection`], [`activate`],
//! [`activate_option`], the labels) is unit tested without a window.

use std::path::PathBuf;

use bevy::prelude::*;

use super::battle_controller::BattleAssets;
use super::input_map::{Action, ActionState, InputMapPlugin};
use super::pause_menu::{next_volume_step, SafeSpawnRequested, SettingsDir};
use super::save_game::SaveGame;
use super::save_wire::{default_bag, default_party, PlayerProgress, SaveToast};
use super::settings::UserSettings;
use super::state::GameState;

// ---------------------------------------------------------------- pure logic

/// One row of the main menu, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainOption {
    Continue,
    NewGame,
    Options,
    Exit,
}

/// Main rows in display order.
pub const OPTIONS: [MainOption; 4] = [
    MainOption::Continue,
    MainOption::NewGame,
    MainOption::Options,
    MainOption::Exit,
];

/// Whether a row can be activated. Continue needs a valid save.
pub fn option_enabled(option: MainOption, has_save: bool) -> bool {
    match option {
        MainOption::Continue => has_save,
        MainOption::NewGame | MainOption::Options | MainOption::Exit => true,
    }
}

/// Text shown for a row (menu text is English; localisation is out of scope).
pub fn option_label(option: MainOption) -> &'static str {
    match option {
        MainOption::Continue => "Continue",
        MainOption::NewGame => "New game",
        MainOption::Options => "Options",
        MainOption::Exit => "Exit",
    }
}

/// Row highlighted when the menu opens: Continue with a save, New game without.
pub fn initial_selection(has_save: bool) -> usize {
    if has_save {
        0
    } else {
        1
    }
}

/// Move the highlight by `delta` rows, wrapping and skipping disabled rows.
///
/// With no save, Continue is skipped in both directions; the cursor never rests on a
/// row that cannot be activated (a clicked disabled row is ignored by [`activate`]).
pub fn move_selection(selected: usize, delta: i32, has_save: bool) -> usize {
    let n = OPTIONS.len() as i32;
    let mut i = selected.min(OPTIONS.len() - 1) as i32;
    if delta == 0 {
        return i as usize;
    }
    let step = delta.signum();
    for _ in 0..delta.abs() {
        for _ in 0..n {
            i = (i + step).rem_euclid(n);
            if option_enabled(OPTIONS[i as usize], has_save) {
                break;
            }
        }
    }
    i as usize
}

/// What the caller must do after a main row is activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuAction {
    /// Enter the world with the save loaded at start-up.
    Continue,
    /// Reset the progress to a fresh game and enter the world.
    NewGame,
    /// Show the options page.
    Options,
    /// Leave the program.
    Exit,
}

/// Activate a row; `None` for a disabled row (Continue without a save).
pub fn activate(option: MainOption, has_save: bool) -> Option<MainMenuAction> {
    if !option_enabled(option, has_save) {
        return None;
    }
    Some(match option {
        MainOption::Continue => MainMenuAction::Continue,
        MainOption::NewGame => MainMenuAction::NewGame,
        MainOption::Options => MainMenuAction::Options,
        MainOption::Exit => MainMenuAction::Exit,
    })
}

/// One row of the options page, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsOption {
    Volume,
    Fov,
    Shadows,
    Quality,
    Back,
}

/// Options rows in display order.
pub const OPTIONS_PAGE: [OptionsOption; 5] = [
    OptionsOption::Volume,
    OptionsOption::Fov,
    OptionsOption::Shadows,
    OptionsOption::Quality,
    OptionsOption::Back,
];

/// Field-of-view presets cycled by the options page (degrees).
pub const FOV_STEPS: [u16; 5] = [45, 60, 75, 90, 110];

/// Next field-of-view preset: the first step above `current`, wrapping to the first.
pub fn next_fov(current: u16) -> u16 {
    FOV_STEPS
        .iter()
        .copied()
        .find(|&f| f > current)
        .unwrap_or(FOV_STEPS[0])
}

fn on_off(v: bool) -> &'static str {
    if v {
        "ON"
    } else {
        "OFF"
    }
}

/// Text shown for an options row.
pub fn options_label(option: OptionsOption, settings: &UserSettings) -> String {
    match option {
        OptionsOption::Volume => format!("Master volume: {}%", settings.master_volume),
        OptionsOption::Fov => format!("Field of view: {} deg", settings.fov_degrees),
        OptionsOption::Shadows => format!("Shadows: {}", on_off(settings.shadows)),
        OptionsOption::Quality => format!("Quality: {}", settings.quality.label()),
        OptionsOption::Back => "Back".into(),
    }
}

/// What the caller must do after an options row is activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsAction {
    /// A value changed: persist `settings.txt`.
    Changed,
    /// Return to the main page.
    Back,
}

/// Activate an options row. Value rows cycle their setting in place.
pub fn activate_option(option: OptionsOption, settings: &mut UserSettings) -> OptionsAction {
    match option {
        OptionsOption::Volume => {
            settings.master_volume = next_volume_step(settings.master_volume);
            OptionsAction::Changed
        }
        OptionsOption::Fov => {
            settings.fov_degrees = next_fov(settings.fov_degrees);
            OptionsAction::Changed
        }
        OptionsOption::Shadows => {
            settings.shadows = !settings.shadows;
            OptionsAction::Changed
        }
        OptionsOption::Quality => {
            settings.quality = settings.quality.next();
            OptionsAction::Changed
        }
        OptionsOption::Back => OptionsAction::Back,
    }
}

/// Move the highlight on the options page; every row is enabled, so it simply wraps.
pub fn move_options_selection(selected: usize, delta: i32) -> usize {
    let n = OPTIONS_PAGE.len() as i32;
    (selected as i32 + delta).rem_euclid(n) as usize
}

// ---------------------------------------------------------------- bevy glue

/// Which page of the menu is showing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MenuPage {
    #[default]
    Main,
    Options,
}

/// Menu state: open page, highlighted row and whether Continue is available.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct MainMenuState {
    pub page: MenuPage,
    pub selected: usize,
    /// A valid `SaveGame` was loaded at start-up (Continue enabled).
    pub has_save: bool,
}

impl Default for MainMenuState {
    fn default() -> Self {
        Self {
            page: MenuPage::Main,
            selected: initial_selection(false),
            has_save: false,
        }
    }
}

#[derive(Component)]
struct MainMenuRoot;
#[derive(Component)]
struct MainMenuPageNode(MenuPage);
#[derive(Component)]
struct MainMenuButton {
    page: MenuPage,
    index: usize,
}
#[derive(Component)]
struct MainMenuButtonText {
    page: MenuPage,
    index: usize,
}

/// Installs the main menu overlay.
///
/// `has_save` enables the Continue row. The menu needs [`GameState`] (the viewer
/// initializes it) and fills [`ActionState`] itself when `InputMapPlugin` is not
/// installed yet, so headless apps can install this plugin alone.
pub struct MainMenuPlugin;

impl MainMenuPlugin {
    /// Insert the resources and systems. `dir` is the `settings.txt` directory.
    pub fn install(app: &mut App, has_save: bool, dir: PathBuf) {
        if !app.is_plugin_added::<InputMapPlugin>() {
            app.add_plugins(InputMapPlugin);
        }
        if !app.world().contains_resource::<SettingsDir>() {
            app.insert_resource(SettingsDir(dir));
        }
        app.insert_resource(MainMenuState {
            page: MenuPage::Main,
            selected: initial_selection(has_save),
            has_save,
        })
        // The pause menu inserts the settings loaded from disk; keep a default around when
        // this plugin is installed alone (init_resource never overwrites an existing one).
        .init_resource::<UserSettings>()
        .init_resource::<SaveToast>()
        .init_resource::<SafeSpawnRequested>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_systems(Startup, spawn_main_menu)
        .add_systems(
            PreUpdate,
            main_menu_input
                .after(super::input_map::fill_action_state)
                .run_if(in_state(GameState::MainMenu)),
        )
        .add_systems(Update, sync_main_menu);
    }
}

fn row_node(width: f32) -> Node {
    Node {
        width: Val::Px(width),
        padding: UiRect::axes(Val::Px(16.0), Val::Px(9.0)),
        justify_content: JustifyContent::Center,
        ..default()
    }
}

fn spawn_main_menu(mut commands: Commands) {
    commands
        .spawn((
            MainMenuRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.06, 0.94)),
            GlobalZIndex(200),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                MainMenuPageNode(MenuPage::Main),
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(28.0)),
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.98)),
            ))
            .with_children(|page| {
                page.spawn((
                    Text::new("Pokémon Legends: Arceus — Bevy Port"),
                    TextFont::from_font_size(34.0),
                    TextColor(Color::srgb(0.95, 0.9, 0.6)),
                ));
                page.spawn((
                    Text::new("MAIN MENU"),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
                ));
                for (i, _) in OPTIONS.iter().enumerate() {
                    page.spawn((
                        Button,
                        MainMenuButton {
                            page: MenuPage::Main,
                            index: i,
                        },
                        row_node(380.0),
                        BackgroundColor(Color::srgba(0.15, 0.2, 0.28, 0.9)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            MainMenuButtonText {
                                page: MenuPage::Main,
                                index: i,
                            },
                            Text::new(""),
                            TextFont::from_font_size(22.0),
                            TextColor(Color::WHITE),
                        ));
                    });
                }
                page.spawn((
                    Text::new("Up/Down + Enter, or click. Esc closes the options page."),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
                ));
            });

            root.spawn((
                MainMenuPageNode(MenuPage::Options),
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(28.0)),
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.98)),
                Visibility::Hidden,
            ))
            .with_children(|page| {
                page.spawn((
                    Text::new("OPTIONS"),
                    TextFont::from_font_size(34.0),
                    TextColor(Color::srgb(0.95, 0.9, 0.6)),
                ));
                for (i, _) in OPTIONS_PAGE.iter().enumerate() {
                    page.spawn((
                        Button,
                        MainMenuButton {
                            page: MenuPage::Options,
                            index: i,
                        },
                        row_node(420.0),
                        BackgroundColor(Color::srgba(0.15, 0.2, 0.28, 0.9)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            MainMenuButtonText {
                                page: MenuPage::Options,
                                index: i,
                            },
                            Text::new(""),
                            TextFont::from_font_size(22.0),
                            TextColor(Color::WHITE),
                        ));
                    });
                }
                page.spawn((
                    Text::new(
                        "Up/Down + Enter, or click. Changes save immediately. Esc backs out.",
                    ),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
                ));
            });
        });
}

/// Edges the world still reads as raw keys (`creature_encounter_system` reads Enter).
fn consume_confirm_keys(keys: &mut ButtonInput<KeyCode>) {
    for key in [KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space] {
        keys.clear_just_pressed(key);
    }
}

#[allow(clippy::too_many_arguments)]
fn main_menu_input(
    mut actions: ResMut<ActionState>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut menu: ResMut<MainMenuState>,
    mut settings: ResMut<UserSettings>,
    dir: Res<SettingsDir>,
    mut save_game: Option<ResMut<SaveGame>>,
    mut progress: Option<ResMut<PlayerProgress>>,
    assets: Option<Res<BattleAssets>>,
    mut safe_request: Option<ResMut<SafeSpawnRequested>>,
    mut toast: ResMut<SaveToast>,
    mut next_state: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
    buttons: Query<(&MainMenuButton, &Interaction), Changed<Interaction>>,
) {
    // Navigation, confirm and back come from the shared input map (keyboard + gamepad).
    // Every edge the menu handles is consumed, so the pause overlay and the battle
    // never see the same press on the frame the state changes.
    let mut delta: i32 = 0;
    if actions.just_pressed(Action::Prev) {
        actions.consume(Action::Prev);
        delta -= 1;
    }
    if actions.just_pressed(Action::Next) {
        actions.consume(Action::Next);
        delta += 1;
    }
    let confirmed = actions.just_pressed(Action::Confirm);
    if confirmed {
        actions.consume(Action::Confirm);
    }
    let back = actions.just_pressed(Action::Cancel) || actions.just_pressed(Action::Menu);
    if back {
        actions.consume(Action::Cancel);
        actions.consume(Action::Menu);
    }

    // Mouse: hover selects, click activates (same rows as the keyboard).
    let mut clicked = false;
    for (b, interaction) in &buttons {
        if b.page != menu.page {
            continue;
        }
        match interaction {
            Interaction::Hovered => menu.selected = b.index,
            Interaction::Pressed => {
                menu.selected = b.index;
                clicked = true;
            }
            Interaction::None => {}
        }
    }

    match menu.page {
        MenuPage::Main => {
            if delta != 0 {
                menu.selected = move_selection(menu.selected, delta, menu.has_save);
            }
            // Esc/Backspace on the main page is consumed and inert: there is nothing to
            // back out to, and the pause overlay must not open over the menu.
            if !(confirmed || clicked) {
                return;
            }
            let Some(action) = activate(OPTIONS[menu.selected], menu.has_save) else {
                return;
            };
            consume_confirm_keys(&mut keys);
            if clicked {
                mouse.clear_just_pressed(MouseButton::Left);
            }
            match action {
                MainMenuAction::Continue => {
                    next_state.set(GameState::World);
                }
                MainMenuAction::NewGame => {
                    start_new_game(
                        progress.as_deref_mut(),
                        assets.as_deref(),
                        save_game.as_deref_mut(),
                        safe_request.as_deref_mut(),
                        &mut toast,
                    );
                    next_state.set(GameState::World);
                }
                MainMenuAction::Options => {
                    menu.page = MenuPage::Options;
                    menu.selected = 0;
                }
                MainMenuAction::Exit => {
                    exit.write(AppExit::Success);
                }
            }
        }
        MenuPage::Options => {
            if delta != 0 {
                menu.selected = move_options_selection(menu.selected, delta);
            }
            if back {
                menu.page = MenuPage::Main;
                menu.selected = OPTIONS
                    .iter()
                    .position(|o| *o == MainOption::Options)
                    .unwrap_or(0);
                return;
            }
            if !(confirmed || clicked) {
                return;
            }
            consume_confirm_keys(&mut keys);
            if clicked {
                mouse.clear_just_pressed(MouseButton::Left);
            }
            match activate_option(OPTIONS_PAGE[menu.selected], &mut settings) {
                OptionsAction::Changed => {
                    if let Err(e) = settings.save_to_dir(&dir.0) {
                        toast.show(format!("Could not save settings: {e}"));
                    }
                }
                OptionsAction::Back => {
                    menu.page = MenuPage::Main;
                    menu.selected = OPTIONS
                        .iter()
                        .position(|o| *o == MainOption::Options)
                        .unwrap_or(0);
                }
            }
        }
    }
}

/// Reset the run to a fresh game, like `--new-game`: fresh progress with the same
/// starter party/bag the direct start seeds, safe spawn requested and the loaded save
/// forgotten in memory (the file is only replaced by the next save).
fn start_new_game(
    progress: Option<&mut PlayerProgress>,
    assets: Option<&BattleAssets>,
    save_game: Option<&mut SaveGame>,
    safe_request: Option<&mut SafeSpawnRequested>,
    toast: &mut SaveToast,
) {
    if let Some(progress) = progress {
        *progress = PlayerProgress::new_game();
        if let Some(assets) = assets {
            progress.party = default_party(assets);
            progress.bag = default_bag(&assets.items);
        }
    }
    if let Some(game) = save_game {
        game.current = None;
        game.message = Some("New game (existing save ignored).".into());
    }
    if let Some(request) = safe_request {
        request.0 = true;
    }
    toast.show("New game. The old save file is kept until the next save.");
}

fn sync_main_menu(
    state: Res<State<GameState>>,
    menu: Res<MainMenuState>,
    settings: Res<UserSettings>,
    mut roots: Query<&mut Visibility, (With<MainMenuRoot>, Without<MainMenuPageNode>)>,
    mut pages: Query<(&MainMenuPageNode, &mut Visibility)>,
    mut buttons: Query<(&MainMenuButton, &mut BackgroundColor)>,
    mut texts: Query<(&MainMenuButtonText, &mut Text)>,
) {
    let open = *state.get() == GameState::MainMenu;
    for mut v in &mut roots {
        let want = if open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    if !open {
        return;
    }
    for (page, mut v) in &mut pages {
        let want = if page.0 == menu.page {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    for (b, mut bg) in &mut buttons {
        if b.page != menu.page {
            continue;
        }
        let enabled = b.page == MenuPage::Options
            || option_enabled(OPTIONS[b.index.min(OPTIONS.len() - 1)], menu.has_save);
        let selected = b.index == menu.selected;
        bg.0 = match (selected, enabled) {
            (true, true) => Color::srgba(0.85, 0.6, 0.15, 0.95),
            (true, false) => Color::srgba(0.35, 0.3, 0.18, 0.9),
            (false, true) => Color::srgba(0.15, 0.2, 0.28, 0.9),
            (false, false) => Color::srgba(0.1, 0.12, 0.16, 0.9),
        };
    }
    for (t, mut text) in &mut texts {
        let label = match t.page {
            MenuPage::Main => option_label(OPTIONS[t.index.min(OPTIONS.len() - 1)]).to_string(),
            MenuPage::Options => {
                options_label(OPTIONS_PAGE[t.index.min(OPTIONS_PAGE.len() - 1)], &settings)
            }
        };
        if text.0 != label {
            text.0 = label;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::settings::Quality;
    use bevy::state::app::StatesPlugin;

    #[test]
    fn continue_needs_a_save_and_is_skipped_by_navigation() {
        assert!(!option_enabled(MainOption::Continue, false));
        assert!(option_enabled(MainOption::Continue, true));
        assert!(option_enabled(MainOption::Exit, false));
        assert_eq!(activate(MainOption::Continue, false), None);

        // With a save the cursor walks every row and wraps both ways.
        assert_eq!(move_selection(0, 1, true), 1);
        assert_eq!(move_selection(3, 1, true), 0);
        assert_eq!(move_selection(0, -1, true), 3);
        assert_eq!(initial_selection(true), 0);

        // Without a save, Continue is skipped in both directions.
        assert_eq!(initial_selection(false), 1);
        assert_eq!(
            move_selection(1, -1, false),
            3,
            "up from New game wraps to Exit"
        );
        assert_eq!(
            move_selection(3, 1, false),
            1,
            "down from Exit wraps to New game"
        );
        assert_eq!(move_selection(1, 1, false), 2);
        assert_eq!(move_selection(2, 1, false), 3);
    }

    #[test]
    fn activation_maps_rows_to_actions() {
        assert_eq!(
            activate(MainOption::Continue, true),
            Some(MainMenuAction::Continue)
        );
        assert_eq!(
            activate(MainOption::NewGame, false),
            Some(MainMenuAction::NewGame)
        );
        assert_eq!(
            activate(MainOption::Options, false),
            Some(MainMenuAction::Options)
        );
        assert_eq!(
            activate(MainOption::Exit, false),
            Some(MainMenuAction::Exit)
        );
        assert_eq!(option_label(MainOption::Continue), "Continue");
        assert_eq!(option_label(MainOption::NewGame), "New game");
        assert_eq!(option_label(MainOption::Options), "Options");
        assert_eq!(option_label(MainOption::Exit), "Exit");
    }

    #[test]
    fn options_rows_cycle_and_show_their_value() {
        let mut s = UserSettings::default();
        assert_eq!(
            options_label(OptionsOption::Volume, &s),
            "Master volume: 100%"
        );
        assert_eq!(
            activate_option(OptionsOption::Volume, &mut s),
            OptionsAction::Changed
        );
        assert_eq!(s.master_volume, 75);
        assert_eq!(
            options_label(OptionsOption::Volume, &s),
            "Master volume: 75%"
        );

        assert_eq!(
            options_label(OptionsOption::Fov, &s),
            "Field of view: 60 deg"
        );
        assert_eq!(
            activate_option(OptionsOption::Fov, &mut s),
            OptionsAction::Changed
        );
        assert_eq!(s.fov_degrees, 75);
        assert_eq!(next_fov(110), 45, "the FOV cycle wraps");
        assert_eq!(
            next_fov(50),
            60,
            "a value between steps moves to the next one"
        );

        assert_eq!(options_label(OptionsOption::Shadows, &s), "Shadows: ON");
        assert_eq!(
            activate_option(OptionsOption::Shadows, &mut s),
            OptionsAction::Changed
        );
        assert!(!s.shadows);
        assert_eq!(options_label(OptionsOption::Shadows, &s), "Shadows: OFF");

        assert_eq!(options_label(OptionsOption::Quality, &s), "Quality: Medium");
        assert_eq!(
            activate_option(OptionsOption::Quality, &mut s),
            OptionsAction::Changed
        );
        assert_eq!(s.quality, Quality::High);

        assert_eq!(
            activate_option(OptionsOption::Back, &mut s),
            OptionsAction::Back
        );
    }

    #[test]
    fn options_selection_wraps() {
        assert_eq!(move_options_selection(0, -1), OPTIONS_PAGE.len() - 1);
        assert_eq!(move_options_selection(OPTIONS_PAGE.len() - 1, 1), 0);
        assert_eq!(move_options_selection(1, 2), 3);
    }

    // ------------------------------------------------------------ headless app

    /// Frame helper: discard the previous frame state, hold `down`, release `up`, update.
    ///
    /// `reset_all` (not `clear`) is required: `clear` keeps the held set, so pressing the
    /// same key again on the next frame would not produce a `just_pressed` edge.
    fn step(app: &mut App, down: &[KeyCode], up: &[KeyCode]) {
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            for key in down {
                keys.press(*key);
            }
            for key in up {
                keys.release(*key);
            }
        }
        app.update();
    }

    fn menu_app(has_save: bool, dir: &std::path::Path) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.insert_state(GameState::MainMenu);
        MainMenuPlugin::install(&mut app, has_save, dir.to_path_buf());
        app.insert_resource(PlayerProgress::new_game());
        app.insert_resource(SaveGame::new(super::super::save_game::SaveFile::in_dir(
            dir,
        )));
        app.update();
        app
    }

    fn state(app: &App) -> GameState {
        *app.world().resource::<State<GameState>>().get()
    }

    #[test]
    fn headless_new_game_walks_main_menu_to_world_with_synthetic_input() {
        let dir = std::env::temp_dir().join(format!("pla_menu_new_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = menu_app(false, &dir);
        assert_eq!(state(&app), GameState::MainMenu);
        // No save: Continue is disabled and absent from the walkable rows.
        assert_eq!(app.world().resource::<MainMenuState>().selected, 1);

        step(&mut app, &[KeyCode::ArrowDown], &[]);
        assert_eq!(
            app.world().resource::<MainMenuState>().selected,
            2,
            "Options"
        );
        step(&mut app, &[KeyCode::ArrowUp], &[]);
        assert_eq!(
            app.world().resource::<MainMenuState>().selected,
            1,
            "back to New game"
        );

        step(&mut app, &[KeyCode::Enter], &[]);
        assert_eq!(state(&app), GameState::World);
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::Enter),
            "the confirm edge is consumed for the world"
        );
        assert!(app.world().resource::<SaveGame>().current.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn headless_continue_needs_a_valid_save() {
        let dir = std::env::temp_dir().join(format!("pla_menu_cont_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = menu_app(true, &dir);
        assert_eq!(
            app.world().resource::<MainMenuState>().selected,
            0,
            "Continue is first"
        );

        // Esc on the main page is consumed and does not leave the menu.
        step(&mut app, &[KeyCode::Escape], &[]);
        assert_eq!(state(&app), GameState::MainMenu);

        step(&mut app, &[KeyCode::Enter], &[]);
        assert_eq!(state(&app), GameState::World);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn headless_options_page_edits_and_persists_settings() {
        let dir = std::env::temp_dir().join(format!("pla_menu_opts_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = menu_app(false, &dir);

        // New game -> Options -> Enter opens the options page.
        step(&mut app, &[KeyCode::ArrowDown], &[]); // New game -> Options
        step(&mut app, &[KeyCode::Enter], &[]);
        {
            let menu = app.world().resource::<MainMenuState>();
            assert_eq!(menu.page, MenuPage::Options);
            assert_eq!(menu.selected, 0);
        }
        // Volume row cycles 100 -> 75 and persists immediately.
        step(&mut app, &[KeyCode::Enter], &[]);
        assert_eq!(app.world().resource::<UserSettings>().master_volume, 75);
        let saved = UserSettings::load_from_dir(&dir);
        assert_eq!(
            saved.master_volume, 75,
            "the options page saved settings.txt"
        );

        // Down to Shadows, Enter flips it off.
        step(&mut app, &[KeyCode::ArrowDown], &[]);
        step(&mut app, &[KeyCode::ArrowDown], &[]);
        assert_eq!(app.world().resource::<MainMenuState>().selected, 2);
        step(&mut app, &[KeyCode::Enter], &[]);
        assert!(!app.world().resource::<UserSettings>().shadows);

        // Esc backs out to the main page with Options highlighted.
        step(&mut app, &[KeyCode::Escape], &[]);
        {
            let menu = app.world().resource::<MainMenuState>();
            assert_eq!(menu.page, MenuPage::Main);
            assert_eq!(menu.selected, 2);
        }
        assert_eq!(state(&app), GameState::MainMenu);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn headless_pause_overlay_stays_closed_in_the_main_menu() {
        let dir = std::env::temp_dir().join(format!("pla_menu_pause_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.insert_state(GameState::MainMenu);
        app.init_resource::<ButtonInput<MouseButton>>();
        MainMenuPlugin::install(&mut app, false, dir.clone());
        super::super::pause_menu::PauseMenuPlugin::install(
            &mut app,
            UserSettings::default(),
            dir.clone(),
        );
        app.insert_resource(PlayerProgress::new_game());
        app.insert_resource(SaveGame::new(super::super::save_game::SaveFile::in_dir(
            &dir,
        )));
        app.update();

        step(&mut app, &[KeyCode::Escape], &[]);
        assert_eq!(state(&app), GameState::MainMenu);
        assert!(
            !app.world().resource::<super::super::state::Paused>().0,
            "Esc in the main menu must not open the pause overlay"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
