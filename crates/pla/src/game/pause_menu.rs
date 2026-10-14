//! Pause menu (Esc or gamepad +): a Bevy UI overlay navigable with the keyboard
//! (arrows, W/S or A/D, D-pad, Enter/A) and the mouse (hover + click).
//!
//! Options: Resume, Bag, Party, Boxes, Pokédex, Crafting, Shop (test shop 0), Invert camera X
//! axis, Invert camera Y axis, Volume (master, cycles 100/75/50/25/0 %), Back to safe spawn, Save
//! game, Load game (the file Save writes), Rest (heals the party), Quit. The screen rows only raise a request resource
//! ([`BagOpenRequested`], [`PartyOpenRequested`], [`BoxOpenRequested`], [`DexOpenRequested`],
//! [`ShopOpenRequested`], [`crate::game::crafting_ui::CraftingOpenRequested`]); each screen plugin
//! consumes its own request, freezes the world and owns the input until it closes. A screen whose
//! plugin is not installed shows a toast instead. Party, Boxes, Pokédex, Crafting and Shop open
//! only in the overworld (never during a battle or a zone change).
//! The panel also shows the player's money and play time (from `PlayerProgress` and `PlayTime`).
//!
//! While [`Paused`] is `true` the plugin freezes the world by contract: every gameplay system must
//! carry `.run_if(pla::game::state::not_paused)` (the opposite condition is
//! `pla::game::state::is_paused`). The menu logic lives in plain functions ([`move_selection`],
//! [`activate`], [`option_label`], [`menu_info_text`]) so it is unit tested without a window; the
//! input edges come from [`crate::game::input_map::ActionState`] and are consumed here so the
//! battle does not also see them on the same frame.

use bevy::prelude::*;

use bevy::ecs::system::SystemParam;

use super::crafting_ui::CraftingOpenRequested;
use super::input_map::{Action, ActionState};
use super::play_time::{format_play_time, PlayTime};
use super::save_wire::{LoadRequested, PlayerProgress, RestRequested, SaveRequested, SaveToast};
use super::settings::UserSettings;
use super::state::{GameState, Paused};

// ---------------------------------------------------------------- pure logic

/// One row of the menu, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuOption {
    Resume,
    Bag,
    Party,
    Boxes,
    Pokedex,
    Crafting,
    Shop,
    InvertX,
    InvertY,
    Volume,
    SafeSpawn,
    Save,
    Load,
    Rest,
    Quit,
}

/// Menu rows in display order.
pub const OPTIONS: [MenuOption; 15] = [
    MenuOption::Resume,
    MenuOption::Bag,
    MenuOption::Party,
    MenuOption::Boxes,
    MenuOption::Pokedex,
    MenuOption::Crafting,
    MenuOption::Shop,
    MenuOption::InvertX,
    MenuOption::InvertY,
    MenuOption::Volume,
    MenuOption::SafeSpawn,
    MenuOption::Save,
    MenuOption::Load,
    MenuOption::Rest,
    MenuOption::Quit,
];

/// What the caller must do after an option was activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Close the menu and unfreeze the world.
    Resume,
    /// Open the bag screen (the bag plugin consumes [`BagOpenRequested`]).
    OpenBag,
    /// Open another fullscreen screen (party, boxes, Pokédex, crafting, shop).
    OpenScreen(Screen),
    /// A setting flipped: persist `settings.txt`.
    SettingsChanged,
    /// Teleport the player to the safe spawn of the current area.
    SafeSpawn,
    /// Save the game exactly like F5.
    Save,
    /// Load the save file (the one Save writes) into the running session.
    Load,
    /// Heal the party (refused during a battle; no clock advance yet, Brief 9 owns the clock).
    Rest,
    /// Leave the program.
    Quit,
}

/// Fullscreen screens reachable from the pause menu besides the bag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Party,
    Boxes,
    Pokedex,
    Crafting,
    /// The bag plugin's shop mode on shop 0 (the same test shop as `--debug-shop`).
    Shop,
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
        MenuOption::Bag => MenuAction::OpenBag,
        MenuOption::Party => MenuAction::OpenScreen(Screen::Party),
        MenuOption::Boxes => MenuAction::OpenScreen(Screen::Boxes),
        MenuOption::Pokedex => MenuAction::OpenScreen(Screen::Pokedex),
        MenuOption::Crafting => MenuAction::OpenScreen(Screen::Crafting),
        MenuOption::Shop => MenuAction::OpenScreen(Screen::Shop),
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
        MenuOption::Load => MenuAction::Load,
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
        MenuOption::Bag => "Bag".into(),
        MenuOption::Party => "Party".into(),
        MenuOption::Boxes => "Pokémon boxes".into(),
        MenuOption::Pokedex => "Pokédex".into(),
        MenuOption::Crafting => "Crafting".into(),
        MenuOption::Shop => "Shop (test shop)".into(),
        MenuOption::InvertX => format!("Invert camera X axis: {}", on_off(settings.invert_camera_x)),
        MenuOption::InvertY => format!("Invert camera Y axis: {}", on_off(settings.invert_camera_y)),
        MenuOption::Volume => format!("Volume: {}%", settings.master_volume),
        MenuOption::SafeSpawn => "Back to safe spawn".into(),
        MenuOption::Save => "Save game".into(),
        MenuOption::Load => "Load game".into(),
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

/// Set to `true` by the pause menu's `Bag` row; the bag screen consumes it and opens.
#[derive(Resource, Debug, Default)]
pub struct BagOpenRequested(pub bool);

/// Set to `true` by the pause menu's `Party` row; the party screen consumes it and opens.
#[derive(Resource, Debug, Default)]
pub struct PartyOpenRequested(pub bool);

/// Set to `true` by the pause menu's `Pokémon boxes` row; the box screen consumes it and opens.
#[derive(Resource, Debug, Default)]
pub struct BoxOpenRequested(pub bool);

/// Set to `true` by the pause menu's `Pokédex` row; the Pokédex screen consumes it and opens.
#[derive(Resource, Debug, Default)]
pub struct DexOpenRequested(pub bool);

/// Set to `true` by the pause menu's `Shop` row; the bag plugin opens its shop mode on shop 0.
#[derive(Resource, Debug, Default)]
pub struct ShopOpenRequested(pub bool);

/// One-shot requests of the action rows (save, load, rest, back to the safe spawn).
#[derive(SystemParam)]
pub struct ActionRequests<'w> {
    save: ResMut<'w, SaveRequested>,
    load: ResMut<'w, LoadRequested>,
    rest: ResMut<'w, RestRequested>,
    safe: ResMut<'w, SafeSpawnRequested>,
}

/// Open requests of the fullscreen screens. A screen whose plugin is not installed has no
/// resource here and its row only shows a toast.
#[derive(SystemParam)]
pub struct ScreenRequests<'w> {
    bag: ResMut<'w, BagOpenRequested>,
    party: Option<ResMut<'w, PartyOpenRequested>>,
    boxes: Option<ResMut<'w, BoxOpenRequested>>,
    dex: Option<ResMut<'w, DexOpenRequested>>,
    crafting: Option<ResMut<'w, CraftingOpenRequested>>,
    shop: Option<ResMut<'w, ShopOpenRequested>>,
}

impl ScreenRequests<'_> {
    /// Raise the request of `screen`; `false` when its plugin is not installed.
    fn request(&mut self, screen: Screen) -> bool {
        let flag = match screen {
            Screen::Party => self.party.as_deref_mut().map(|r| &mut r.0),
            Screen::Boxes => self.boxes.as_deref_mut().map(|r| &mut r.0),
            Screen::Pokedex => self.dex.as_deref_mut().map(|r| &mut r.0),
            Screen::Crafting => self.crafting.as_deref_mut().map(|r| &mut r.0),
            Screen::Shop => self.shop.as_deref_mut().map(|r| &mut r.0),
        };
        match flag {
            Some(flag) => {
                *flag = true;
                true
            }
            None => false,
        }
    }
}

/// Toast shown when a screen row cannot open now.
pub fn screen_unavailable_text(screen: Screen, in_world: bool) -> String {
    let name = match screen {
        Screen::Party => "Party",
        Screen::Boxes => "Pokémon boxes",
        Screen::Pokedex => "Pokédex",
        Screen::Crafting => "Crafting",
        Screen::Shop => "Shop",
    };
    if in_world {
        format!("{name} is not available in this build.")
    } else {
        format!("{name} opens only while exploring (not in a battle or a zone change).")
    }
}

/// Set by a fullscreen screen that owns the input while open (the bag/shop screen): the pause
/// menu hides its root and ignores every key until the flag is cleared.
#[derive(Resource, Debug, Default)]
pub struct PauseMenuSuppressed(pub bool);

/// Freeze (`on`) or release the world for a fullscreen screen: sets [`Paused`] and
/// [`PauseMenuSuppressed`] together, so the pause overlay stays hidden and ignores Esc while the
/// screen owns the input. Missing resources (standalone screen apps) are skipped.
pub fn freeze_for_screen(
    paused: &mut Option<ResMut<Paused>>,
    suppressed: &mut Option<ResMut<PauseMenuSuppressed>>,
    on: bool,
) {
    if let Some(p) = paused.as_deref_mut() {
        p.0 = on;
    }
    if let Some(s) = suppressed.as_deref_mut() {
        s.0 = on;
    }
}

/// Highlighted row.
#[derive(Resource, Debug, Default)]
pub struct PauseMenu {
    pub selected: usize,
}

/// Directory of `settings.txt`.
#[derive(Resource, Debug, Clone)]
pub struct SettingsDir(pub std::path::PathBuf);

/// Root of the provisional pause menu (hidden while the original one is shown, `orig_ui`).
#[derive(Component)]
pub struct PauseRoot;
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
            .init_resource::<LoadRequested>()
            .init_resource::<RestRequested>()
            .init_resource::<SafeSpawnRequested>()
            .init_resource::<BagOpenRequested>()
            .init_resource::<PauseMenuSuppressed>()
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
                    // 15 rows must fit a 720 px window: about 36 px per row plus the title.
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.95)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("PAUSED"),
                    TextFont::from_font_size(28.0),
                    TextColor(Color::srgb(0.95, 0.9, 0.6)),
                ));
                for (i, _) in OPTIONS.iter().enumerate() {
                    panel
                        .spawn((
                            Button,
                            PauseButton(i),
                            Node {
                                width: Val::Px(420.0),
                                padding: UiRect::axes(Val::Px(16.0), Val::Px(4.0)),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.15, 0.2, 0.28, 0.9)),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                PauseButtonText(i),
                                Text::new(""),
                                TextFont::from_font_size(20.0),
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
    mut requests: ActionRequests,
    mut screens: ScreenRequests,
    suppressed: Res<PauseMenuSuppressed>,
    mut toast: ResMut<SaveToast>,
    buttons: Query<(&PauseButton, &Interaction), Changed<Interaction>>,
    mut exit: MessageWriter<AppExit>,
    state: Option<Res<State<GameState>>>,
) {
    // The main menu owns the input while it is open (Escape backs out of its options
    // page); never show the pause overlay on top of it.
    if state.as_ref().is_some_and(|s| *s.get() == GameState::MainMenu) {
        return;
    }
    // Another fullscreen screen owns the input (bag/shop): do nothing, it handles Esc itself.
    if suppressed.0 {
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
        MenuAction::OpenBag => {
            screens.bag.0 = true;
            paused.0 = false;
        }
        MenuAction::OpenScreen(screen) => {
            // The screen keeps the world frozen (its plugin sets `Paused` and
            // `PauseMenuSuppressed` when it consumes the request this frame).
            let in_world = state.as_ref().is_none_or(|s| *s.get() == GameState::World);
            if !in_world || !screens.request(screen) {
                toast.show(screen_unavailable_text(screen, in_world));
            }
        }
        MenuAction::SettingsChanged => {
            if let Err(e) = settings.save_to_dir(&dir.0) {
                toast.show(format!("Could not save settings: {e}"));
            }
        }
        MenuAction::SafeSpawn => {
            requests.safe.0 = true;
            paused.0 = false;
        }
        MenuAction::Save => requests.save.0 = true,
        MenuAction::Load => {
            // The world resumes so the zone change of the load can run.
            requests.load.0 = true;
            paused.0 = false;
        }
        MenuAction::Rest => requests.rest.0 = true,
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
    suppressed: Res<PauseMenuSuppressed>,
    mut root: Query<&mut Visibility, With<PauseRoot>>,
    mut buttons: Query<(&PauseButton, &mut BackgroundColor)>,
    mut texts: Query<(&PauseButtonText, &mut Text)>,
    mut info: Query<&mut Text, (With<PauseInfoText>, Without<PauseButtonText>)>,
) {
    for mut v in &mut root {
        let want = if paused.0 && !suppressed.0 { Visibility::Visible } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
    }
    if !paused.0 || suppressed.0 {
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
        assert_eq!(activate(MenuOption::Bag, &mut s), MenuAction::OpenBag);
        assert_eq!(option_label(MenuOption::Bag, &s), "Bag");
        assert_eq!(activate(MenuOption::Party, &mut s), MenuAction::OpenScreen(Screen::Party));
        assert_eq!(activate(MenuOption::Boxes, &mut s), MenuAction::OpenScreen(Screen::Boxes));
        assert_eq!(activate(MenuOption::Pokedex, &mut s), MenuAction::OpenScreen(Screen::Pokedex));
        assert_eq!(
            activate(MenuOption::Crafting, &mut s),
            MenuAction::OpenScreen(Screen::Crafting)
        );
        assert_eq!(activate(MenuOption::Shop, &mut s), MenuAction::OpenScreen(Screen::Shop));
        assert_eq!(option_label(MenuOption::Pokedex, &s), "Pokédex");
        assert_eq!(activate(MenuOption::Save, &mut s), MenuAction::Save);
        assert_eq!(activate(MenuOption::Load, &mut s), MenuAction::Load);
        assert_eq!(option_label(MenuOption::Load, &s), "Load game");
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
        assert_eq!(OPTIONS.len(), 15);
        assert_eq!(move_selection(i - 1, 1), i);
        assert_eq!(move_selection(i, 1), i + 1);
    }

    #[test]
    fn screen_rows_follow_the_bag_row_in_order() {
        let bag = OPTIONS.iter().position(|o| *o == MenuOption::Bag).unwrap();
        assert_eq!(
            OPTIONS[bag + 1..bag + 6],
            [
                MenuOption::Party,
                MenuOption::Boxes,
                MenuOption::Pokedex,
                MenuOption::Crafting,
                MenuOption::Shop
            ]
        );
        assert_eq!(OPTIONS[bag + 6], MenuOption::InvertX);
        assert!(screen_unavailable_text(Screen::Boxes, false).contains("only while exploring"));
        assert!(screen_unavailable_text(Screen::Party, true).contains("not available"));
    }

    #[test]
    fn load_and_rest_rows_sit_between_save_and_quit() {
        let rest = OPTIONS.iter().position(|o| *o == MenuOption::Rest).unwrap();
        assert_eq!(OPTIONS[rest - 2], MenuOption::Save);
        assert_eq!(OPTIONS[rest - 1], MenuOption::Load);
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

    fn data_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    /// The viewer's screen set (party, boxes, Pokédex, crafting, bag/shop) behind the pause menu.
    fn screens_app(tag: &str) -> App {
        use crate::game::crafting::{Bag, TsvRecipeProvider};
        let dir = std::env::temp_dir().join(format!("pla_pause_{tag}_{}", std::process::id()));
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<ButtonInput<MouseButton>>();
        app.add_plugins(crate::game::input_map::InputMapPlugin);
        PauseMenuPlugin::install(&mut app, UserSettings::default(), dir);
        let recipes = TsvRecipeProvider::load(data_root()).unwrap();
        app.add_plugins((
            crate::game::party_screen::PartyScreenPlugin,
            crate::game::box_screen::BoxScreenPlugin,
            crate::game::dex_screen::DexScreenPlugin::with_data_dir(data_root()),
            crate::game::crafting_ui::CraftingPlugin::with_provider(&recipes, Bag::default()),
            crate::game::bag_screen::BagScreenPlugin::with_data_dir(data_root()),
        ));
        let mut progress = PlayerProgress::new_game();
        progress.party.push(crate::game::save_game::PartyMember {
            species: 25,
            level: 5,
            hp: 20,
            ..Default::default()
        });
        progress.bag.insert(1613, 2);
        app.insert_resource(progress);
        step(&mut app, &[], &[]);
        app
    }

    fn screen_open(app: &App, row: MenuOption) -> bool {
        let w = app.world();
        match row {
            MenuOption::Party => w.resource::<crate::game::party_screen::PartyScreenState>().open,
            MenuOption::Boxes => w.resource::<crate::game::box_screen::BoxScreenState>().open,
            MenuOption::Pokedex => w.resource::<crate::game::dex_screen::DexUi>().open,
            MenuOption::Crafting => w.resource::<crate::game::crafting_ui::CraftingUi>().open,
            MenuOption::Shop => {
                let ui = w.resource::<crate::game::bag_screen::BagUi>();
                ui.open && ui.mode == crate::game::bag_screen::BagMode::Shop
            }
            other => panic!("{other:?} is not a screen row"),
        }
    }

    fn pause_root_visible(app: &mut App) -> bool {
        let mut q = app
            .world_mut()
            .query_filtered::<&Visibility, With<PauseRoot>>();
        q.iter(app.world()).any(|v| *v == Visibility::Visible)
    }

    #[test]
    fn headless_every_screen_row_opens_its_screen_and_esc_returns_to_the_world() {
        let rows = [
            MenuOption::Party,
            MenuOption::Boxes,
            MenuOption::Pokedex,
            MenuOption::Crafting,
            MenuOption::Shop,
        ];
        for row in rows {
            let mut app = screens_app("screens");
            step(&mut app, &[KeyCode::Escape], &[]);
            assert!(app.world().resource::<Paused>().0, "{row:?}: menu open");
            app.world_mut().resource_mut::<PauseMenu>().selected =
                OPTIONS.iter().position(|o| *o == row).unwrap();
            step(&mut app, &[KeyCode::Enter], &[KeyCode::Escape]);
            assert!(screen_open(&app, row), "{row:?} must open from the pause menu");
            assert!(app.world().resource::<Paused>().0, "{row:?}: world stays frozen");
            assert!(app.world().resource::<PauseMenuSuppressed>().0, "{row:?}: owns input");
            step(&mut app, &[], &[KeyCode::Enter]);
            assert!(screen_open(&app, row), "{row:?} stays open");
            assert!(!pause_root_visible(&mut app), "{row:?}: pause overlay hidden");

            step(&mut app, &[KeyCode::Escape], &[]);
            assert!(!screen_open(&app, row), "{row:?} must close on Esc");
            assert!(!app.world().resource::<Paused>().0, "{row:?}: world resumes");
            assert!(!app.world().resource::<PauseMenuSuppressed>().0);
            step(&mut app, &[], &[KeyCode::Escape]);
            assert!(!app.world().resource::<Paused>().0, "{row:?}: Esc did not reopen the menu");
        }
    }

    #[test]
    fn headless_screens_show_live_progress_and_refuse_outside_the_world() {
        let mut app = screens_app("live");
        step(&mut app, &[KeyCode::Escape], &[]);
        app.world_mut().resource_mut::<PauseMenu>().selected =
            OPTIONS.iter().position(|o| *o == MenuOption::Party).unwrap();
        step(&mut app, &[KeyCode::Enter], &[KeyCode::Escape]);
        assert_eq!(
            app.world().resource::<crate::game::party_screen::PartyScreenData>().party[0].species,
            25,
            "the party screen mirrors PlayerProgress"
        );
        step(&mut app, &[KeyCode::Escape], &[KeyCode::Enter]);
        step(&mut app, &[], &[KeyCode::Escape]);

        // Crafting copies the player's bag when it opens.
        step(&mut app, &[KeyCode::Escape], &[]);
        app.world_mut().resource_mut::<PauseMenu>().selected =
            OPTIONS.iter().position(|o| *o == MenuOption::Crafting).unwrap();
        step(&mut app, &[KeyCode::Enter], &[KeyCode::Escape]);
        assert_eq!(
            app.world().resource::<crate::game::crafting_ui::CraftingBag>().0.get(&1613),
            Some(&2)
        );
        assert_eq!(app.world().resource::<crate::game::crafting_ui::CraftingCatalog>().0.len(), 43);
        step(&mut app, &[KeyCode::Escape], &[KeyCode::Enter]);
        step(&mut app, &[], &[KeyCode::Escape]);

        // During a battle the boxes row only shows a toast and the menu stays up.
        app.insert_resource(State::new(GameState::Battle));
        step(&mut app, &[KeyCode::Escape], &[]);
        app.world_mut().resource_mut::<PauseMenu>().selected =
            OPTIONS.iter().position(|o| *o == MenuOption::Boxes).unwrap();
        step(&mut app, &[KeyCode::Enter], &[KeyCode::Escape]);
        assert!(!screen_open(&app, MenuOption::Boxes));
        assert!(app.world().resource::<Paused>().0);
        assert!(!app.world().resource::<PauseMenuSuppressed>().0);
        assert!(app.world().resource::<SaveToast>().text.contains("only while exploring"));
    }

    #[test]
    fn screen_row_without_its_plugin_shows_a_toast() {
        let dir = std::env::temp_dir().join(format!("pla_pause_noscreen_{}", std::process::id()));
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.add_plugins(crate::game::input_map::InputMapPlugin);
        PauseMenuPlugin::install(&mut app, UserSettings::default(), dir);
        step(&mut app, &[], &[]);
        step(&mut app, &[KeyCode::Escape], &[]);
        app.world_mut().resource_mut::<PauseMenu>().selected =
            OPTIONS.iter().position(|o| *o == MenuOption::Pokedex).unwrap();
        step(&mut app, &[KeyCode::Enter], &[KeyCode::Escape]);
        assert!(app.world().resource::<Paused>().0, "the menu stays open");
        assert!(app.world().resource::<SaveToast>().text.contains("not available"));
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

        // D-pad/A-D/arrow navigation passes the Bag and screen rows and reaches Invert camera X.
        let invert_x = OPTIONS.iter().position(|o| *o == MenuOption::InvertX).unwrap();
        step(&mut app, &[KeyCode::ArrowDown], &[]);
        assert_eq!(app.world().resource::<PauseMenu>().selected, 1);
        for expected in 2..=invert_x {
            step(&mut app, &[], &[KeyCode::ArrowDown]);
            step(&mut app, &[KeyCode::ArrowDown], &[]);
            assert_eq!(app.world().resource::<PauseMenu>().selected, expected);
        }

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
        assert_eq!(app.world().resource::<PauseMenu>().selected, invert_x + 1);
        assert!(!app
            .world()
            .resource::<ActionState>()
            .just_pressed(Action::OpenSwitch));
        step(&mut app, &[KeyCode::KeyW], &[KeyCode::KeyS]);
        assert_eq!(app.world().resource::<PauseMenu>().selected, invert_x);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
