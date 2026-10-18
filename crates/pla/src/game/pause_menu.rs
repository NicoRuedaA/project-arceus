//! Pause menu (Esc or gamepad +): the game's tabbed menu. Esc opens the first tab (the bag) at once;
//! **Q / E** pivot between the tabs (Bag, Party, Summary, Boxes, Pokédex, Crafting, Shop,
//! Customize and a System page) while the arrows, W/S/A/D and Enter keep working INSIDE the open
//! page; Esc closes the whole menu. The System page is a list: Volume (master, cycles
//! 100/75/50/25/0 %), Graphics..., Back to safe spawn, Save game, Load game (the file Save
//! writes), Rest (heals the party) and Quit; Graphics opens its own screen and Esc comes back to
//! that list. The camera-axis inversions live in the Graphics screen. A tab only raises a request
//! resource ([`BagOpenRequested`], [`PartyOpenRequested`], [`BoxOpenRequested`],
//! [`DexOpenRequested`], [`ShopOpenRequested`],
//! [`crate::game::crafting_ui::CraftingOpenRequested`], ...); each screen plugin consumes its own
//! request, freezes the world and owns the input until it closes. A screen whose plugin is not
//! installed shows a toast instead. Party, Boxes, Pokédex, Crafting and Shop open only in the
//! overworld (never during a battle or a zone change).
//! The System page also shows the player's money and play time (from `PlayerProgress` and
//! `PlayTime`).
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
    /// The Pokémon summary (`game::summary_screen`), on the first party member.
    Summary,
    Boxes,
    Pokedex,
    Crafting,
    Shop,
    Volume,
    Graphics,
    /// The customization screen (`game::customize_screen`, Brief 36).
    Customize,
    SafeSpawn,
    Save,
    Load,
    Rest,
    Quit,
}

/// Menu rows in display order. Every camera-axis inversion is a Graphics-screen option, so the
/// list is exactly four pages of four icons in the original pause-menu strip.
pub const OPTIONS: [MenuOption; 16] = [
    MenuOption::Resume,
    MenuOption::Bag,
    MenuOption::Party,
    MenuOption::Summary,
    MenuOption::Boxes,
    MenuOption::Pokedex,
    MenuOption::Crafting,
    MenuOption::Shop,
    MenuOption::Volume,
    MenuOption::Graphics,
    MenuOption::Customize,
    MenuOption::SafeSpawn,
    MenuOption::Save,
    MenuOption::Load,
    MenuOption::Rest,
    MenuOption::Quit,
];

/// A tab of the pause menu: eight pages that are the game's own screens, and the System list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Bag,
    Party,
    Summary,
    Boxes,
    Pokedex,
    Crafting,
    Shop,
    Customize,
    /// The list of the port's own actions ([`SYSTEM_OPTIONS`]).
    System,
}

/// Tabs in display order (Q / E walk this list, wrapping around).
pub const TABS: [Tab; 9] = [
    Tab::Bag,
    Tab::Party,
    Tab::Summary,
    Tab::Boxes,
    Tab::Pokedex,
    Tab::Crafting,
    Tab::Shop,
    Tab::Customize,
    Tab::System,
];

/// Rows of the System tab.
pub const SYSTEM_OPTIONS: [MenuOption; 7] = [
    MenuOption::Volume,
    MenuOption::Graphics,
    MenuOption::SafeSpawn,
    MenuOption::Save,
    MenuOption::Load,
    MenuOption::Rest,
    MenuOption::Quit,
];

impl Tab {
    /// Title of the tab (the strip's header text).
    pub fn label(self) -> &'static str {
        match self {
            Tab::Bag => "Bag",
            Tab::Party => "Party",
            Tab::Summary => "Summary",
            Tab::Boxes => "Pokémon boxes",
            Tab::Pokedex => "Pokédex",
            Tab::Crafting => "Crafting",
            Tab::Shop => "Shop",
            Tab::Customize => "Customize",
            Tab::System => "System",
        }
    }

    /// The fullscreen screen this tab shows; `None` for the bag (own request) and the System list.
    pub fn screen(self) -> Option<Screen> {
        match self {
            Tab::Party => Some(Screen::Party),
            Tab::Summary => Some(Screen::Summary),
            Tab::Boxes => Some(Screen::Boxes),
            Tab::Pokedex => Some(Screen::Pokedex),
            Tab::Crafting => Some(Screen::Crafting),
            Tab::Shop => Some(Screen::Shop),
            Tab::Customize => Some(Screen::Customize),
            Tab::Bag | Tab::System => None,
        }
    }
}

/// Tab reached from `tab` after `delta` presses of Q (-1) / E (+1), wrapping around both ends.
pub fn step_tab(tab: usize, delta: i32) -> usize {
    (tab as i32 + delta).rem_euclid(TABS.len() as i32) as usize
}

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
    /// The Pokémon summary (`game::summary_screen`).
    Summary,
    Boxes,
    Pokedex,
    Crafting,
    /// The graphics settings screen (`game::graphics_screen`).
    Graphics,
    /// The customization screen (`game::customize_screen`).
    Customize,
    /// The bag plugin's shop mode on shop 0 (the same test shop as `--debug-shop`).
    Shop,
}

/// Move the highlighted row of the System list by `delta`, wrapping around both ends.
pub fn move_selection(selected: usize, delta: i32) -> usize {
    let n = SYSTEM_OPTIONS.len() as i32;
    (selected as i32 + delta).rem_euclid(n) as usize
}

/// Activate a row. Toggle rows flip their setting in place.
pub fn activate(option: MenuOption, settings: &mut UserSettings) -> MenuAction {
    match option {
        MenuOption::Resume => MenuAction::Resume,
        MenuOption::Bag => MenuAction::OpenBag,
        MenuOption::Party => MenuAction::OpenScreen(Screen::Party),
        MenuOption::Summary => MenuAction::OpenScreen(Screen::Summary),
        MenuOption::Boxes => MenuAction::OpenScreen(Screen::Boxes),
        MenuOption::Pokedex => MenuAction::OpenScreen(Screen::Pokedex),
        MenuOption::Crafting => MenuAction::OpenScreen(Screen::Crafting),
        MenuOption::Shop => MenuAction::OpenScreen(Screen::Shop),
        MenuOption::Graphics => MenuAction::OpenScreen(Screen::Graphics),
        MenuOption::Customize => MenuAction::OpenScreen(Screen::Customize),
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

/// Text shown for a row.
pub fn option_label(option: MenuOption, settings: &UserSettings) -> String {
    match option {
        MenuOption::Resume => "Resume".into(),
        MenuOption::Bag => "Bag".into(),
        MenuOption::Party => "Party".into(),
        MenuOption::Summary => "Summary".into(),
        MenuOption::Boxes => "Pokémon boxes".into(),
        MenuOption::Pokedex => "Pokédex".into(),
        MenuOption::Crafting => "Crafting".into(),
        MenuOption::Shop => "Shop (test shop)".into(),
        MenuOption::Customize => "Customize".into(),
        MenuOption::Volume => format!("Volume: {}%", settings.master_volume),
        MenuOption::Graphics => "Graphics...".into(),
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

/// Set to `true` by the pause menu's `Graphics` row; the graphics screen consumes it and opens.
#[derive(Resource, Debug, Default)]
pub struct GraphicsOpenRequested(pub bool);

/// Set to `true` by the pause menu's `Customize` row; the customization screen consumes it.
#[derive(Resource, Debug, Default)]
pub struct CustomizeOpenRequested(pub bool);

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
    summary: Option<ResMut<'w, crate::game::summary_screen::SummaryOpenRequested>>,
    boxes: Option<ResMut<'w, BoxOpenRequested>>,
    dex: Option<ResMut<'w, DexOpenRequested>>,
    crafting: Option<ResMut<'w, CraftingOpenRequested>>,
    shop: Option<ResMut<'w, ShopOpenRequested>>,
    graphics: Option<ResMut<'w, GraphicsOpenRequested>>,
    customize: Option<ResMut<'w, super::customize_screen::CustomizeOpenRequested>>,
}

impl ScreenRequests<'_> {
    /// Raise the request of `screen`; `false` when its plugin is not installed.
    fn request(&mut self, screen: Screen) -> bool {
        let flag = match screen {
            Screen::Party => self.party.as_deref_mut().map(|r| &mut r.0),
            Screen::Summary => self.summary.as_deref_mut().map(|r| &mut r.0),
            Screen::Boxes => self.boxes.as_deref_mut().map(|r| &mut r.0),
            Screen::Pokedex => self.dex.as_deref_mut().map(|r| &mut r.0),
            Screen::Crafting => self.crafting.as_deref_mut().map(|r| &mut r.0),
            Screen::Shop => self.shop.as_deref_mut().map(|r| &mut r.0),
            Screen::Graphics => self.graphics.as_deref_mut().map(|r| &mut r.0),
            Screen::Customize => self.customize.as_deref_mut().map(|r| &mut r.0),
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
        Screen::Summary => "Summary",
        Screen::Boxes => "Pokémon boxes",
        Screen::Pokedex => "Pokédex",
        Screen::Crafting => "Crafting",
        Screen::Shop => "Shop",
        Screen::Graphics => "Graphics",
        Screen::Customize => "Customize",
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

/// Highlighted row and the first row shown by the original list (its scrollbar / wheel write it).
#[derive(Resource, Debug, Default)]
pub struct PauseMenu {
    pub selected: usize,
    pub scroll: usize,
}

/// The open tab of the pause menu. `active` is on from the Esc that opens the menu until the world
/// runs again; `tab` indexes [`TABS`].
#[derive(Resource, Debug, Default)]
pub struct PauseTabs {
    pub active: bool,
    pub tab: usize,
}

impl PauseTabs {
    /// The tab being shown.
    pub fn current(&self) -> Tab {
        TABS[self.tab.min(TABS.len() - 1)]
    }
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
            .init_resource::<PauseTabs>()
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
                (pause_input, pause_tab_switch)
                    .chain()
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
                    // 16 rows must fit a 720 px window: about 36 px per row plus the title.
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
                for (i, _) in SYSTEM_OPTIONS.iter().enumerate() {
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
    mut tabs: ResMut<PauseTabs>,
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
        // The page that opens with the menu reads Esc as its own "close" key: it must not see
        // the press that opened it.
        keys.clear_just_pressed(KeyCode::Escape);
        paused.0 = toggle_paused(paused.0);
        tabs.active = paused.0;
        if paused.0 {
            menu.selected = 0;
            menu.scroll = 0;
            // The menu opens on its first page (the bag) at once; outside the overworld only the
            // System list is available.
            let in_world = state.as_ref().is_none_or(|s| *s.get() == GameState::World);
            if in_world {
                tabs.tab = 0;
                screens.bag.0 = true;
            } else {
                tabs.tab = TABS.len() - 1;
            }
        }
        return;
    }
    if !paused.0 {
        tabs.active = false;
        return;
    }
    // A pause opened by another path (a test, an automation command) is the System list.
    if !tabs.active {
        tabs.active = true;
        tabs.tab = TABS.len() - 1;
    }
    // The screen tabs are owned by their screens; only the System list answers here.
    if tabs.current() != Tab::System {
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
    match activate(SYSTEM_OPTIONS[i.min(SYSTEM_OPTIONS.len() - 1)], &mut settings) {
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

/// The screens a tab can show, so Q / E can close the open one without releasing the frozen world.
#[derive(SystemParam)]
pub struct TabScreens<'w> {
    bag: Option<ResMut<'w, super::bag_screen::BagUi>>,
    party: Option<ResMut<'w, super::party_screen::PartyScreenState>>,
    party_ops: Option<ResMut<'w, super::party_screen::PartyOpsState>>,
    summary: Option<ResMut<'w, super::summary_screen::SummaryState>>,
    boxes: Option<ResMut<'w, super::box_screen::BoxScreenState>>,
    dex: Option<ResMut<'w, super::dex_screen::DexUi>>,
    crafting: Option<ResMut<'w, super::crafting_ui::CraftingUi>>,
    customize: Option<ResMut<'w, super::customize_screen::CustomizeScreenState>>,
    graphics: Option<ResMut<'w, super::graphics_screen::GraphicsScreenState>>,
}

impl TabScreens<'_> {
    /// Whether the Graphics screen (opened from the System list) owns the input.
    fn graphics_open(&self) -> bool {
        self.graphics.as_deref().is_some_and(|g| g.open)
    }

    /// Close every tab screen. The world stays frozen: only their `open` flags change.
    fn close_all(&mut self) {
        if let Some(s) = self.bag.as_deref_mut() {
            s.open = false;
        }
        if let Some(s) = self.party.as_deref_mut() {
            s.open = false;
        }
        if let Some(s) = self.party_ops.as_deref_mut() {
            s.open = false;
        }
        if let Some(s) = self.summary.as_deref_mut() {
            s.open = false;
        }
        if let Some(s) = self.boxes.as_deref_mut() {
            s.open = false;
        }
        if let Some(s) = self.dex.as_deref_mut() {
            s.open = false;
        }
        if let Some(s) = self.crafting.as_deref_mut() {
            s.open = false;
        }
        if let Some(s) = self.customize.as_deref_mut() {
            s.open = false;
        }
    }
}

/// Q / E pivot between the tabs of the open pause menu: the open page closes (the world stays
/// frozen) and the next one opens through its usual request. Runs right after [`pause_input`] and
/// consumes the two keys so the page underneath never sees them.
#[allow(clippy::too_many_arguments)]
fn pause_tab_switch(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    paused: Res<Paused>,
    mut tabs: ResMut<PauseTabs>,
    mut suppressed: ResMut<PauseMenuSuppressed>,
    mut screens: ScreenRequests,
    mut open: TabScreens,
    mut toast: ResMut<SaveToast>,
    state: Option<Res<State<GameState>>>,
) {
    if !tabs.active || !paused.0 {
        return;
    }
    // The tabs are the overworld's pages; the Graphics screen (opened from the System list) keeps
    // Q / E for itself.
    if state.as_ref().is_some_and(|s| *s.get() != GameState::World) || open.graphics_open() {
        return;
    }
    let delta = if keys.just_pressed(KeyCode::KeyQ) {
        -1
    } else if keys.just_pressed(KeyCode::KeyE) {
        1
    } else {
        return;
    };
    keys.clear_just_pressed(KeyCode::KeyQ);
    keys.clear_just_pressed(KeyCode::KeyE);
    let next = step_tab(tabs.tab, delta);
    open.close_all();
    tabs.tab = next;
    match TABS[next] {
        Tab::System => suppressed.0 = false,
        Tab::Bag => screens.bag.0 = true,
        tab => {
            if let Some(screen) = tab.screen() {
                if !screens.request(screen) {
                    toast.show(screen_unavailable_text(screen, true));
                    suppressed.0 = false;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_menu(
    paused: Res<Paused>,
    tabs: Res<PauseTabs>,
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
        let want = if paused.0 && !suppressed.0 && tabs.current() == Tab::System {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    if !paused.0 || suppressed.0 || tabs.current() != Tab::System {
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
        let label = option_label(SYSTEM_OPTIONS[t.0], &settings);
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
        assert_eq!(move_selection(0, -1), SYSTEM_OPTIONS.len() - 1);
        assert_eq!(move_selection(SYSTEM_OPTIONS.len() - 1, 1), 0);
        assert_eq!(move_selection(1, 1), 2);
        assert_eq!(move_selection(2, -1), 1);
    }

    #[test]
    fn camera_inversions_live_only_in_the_graphics_screen() {
        // The pause menu keeps the game's own distribution: no camera-axis rows. The Graphics
        // screen carries them (`graphics_screen::Row::InvertX` / `InvertY`).
        let mut s = UserSettings::default();
        for o in OPTIONS {
            let label = option_label(o, &s);
            assert!(
                !label.to_ascii_lowercase().contains("invert camera"),
                "{o:?} still shows a camera inversion: {label}"
            );
        }
        // The settings themselves still work and stay independent.
        s.invert_camera_x = true;
        assert_eq!(s.apply_invert(3.0, 2.0), (-3.0, 2.0));
        s.invert_camera_y = true;
        assert_eq!(s.apply_invert(3.0, 2.0), (-3.0, -2.0));
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
    fn screen_rows_follow_the_bag_row_in_order() {
        let bag = OPTIONS.iter().position(|o| *o == MenuOption::Bag).unwrap();
        assert_eq!(
            OPTIONS[bag + 1..bag + 7],
            [
                MenuOption::Party,
                MenuOption::Summary,
                MenuOption::Boxes,
                MenuOption::Pokedex,
                MenuOption::Crafting,
                MenuOption::Shop
            ]
        );
        assert_eq!(OPTIONS[bag + 7], MenuOption::Volume);
        let mut s = UserSettings::default();
        assert_eq!(activate(MenuOption::Summary, &mut s), MenuAction::OpenScreen(Screen::Summary));
        assert!(screen_unavailable_text(Screen::Boxes, false).contains("only while exploring"));
        assert!(screen_unavailable_text(Screen::Party, true).contains("not available"));
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
    fn graphics_row_sits_after_volume_and_opens_the_graphics_screen() {
        let mut s = UserSettings::default();
        let vol = OPTIONS.iter().position(|o| *o == MenuOption::Volume).unwrap();
        assert_eq!(OPTIONS[vol + 1], MenuOption::Graphics);
        assert_eq!(OPTIONS[vol + 2], MenuOption::Customize);
        assert_eq!(OPTIONS[vol + 3], MenuOption::SafeSpawn);
        assert_eq!(activate(MenuOption::Graphics, &mut s), MenuAction::OpenScreen(Screen::Graphics));
        assert_eq!(option_label(MenuOption::Graphics, &s), "Graphics...");
        assert_eq!(s, UserSettings::default());
        assert!(screen_unavailable_text(Screen::Graphics, true).contains("Graphics"));
        // The customization row (Brief 36) opens its screen right after Graphics.
        assert_eq!(activate(MenuOption::Customize, &mut s), MenuAction::OpenScreen(Screen::Customize));
        assert_eq!(option_label(MenuOption::Customize, &s), "Customize");
        assert!(screen_unavailable_text(Screen::Customize, true).contains("Customize"));
    }

    #[test]
    fn toggled_setting_survives_a_restart_in_a_temp_dir() {
        let dir = std::env::temp_dir().join(format!("pla_pause_settings_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = UserSettings::load_from_dir(&dir);
        activate(MenuOption::Volume, &mut s);
        s.save_to_dir(&dir).unwrap();
        let reloaded = UserSettings::load_from_dir(&dir);
        assert_eq!(reloaded.master_volume, 75);
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
            crate::game::summary_screen::SummaryScreenPlugin,
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
            MenuOption::Summary => w.resource::<crate::game::summary_screen::SummaryState>().open,
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
    fn step_tab_wraps_both_ways_and_tabs_cover_the_game_pages() {
        assert_eq!(step_tab(0, -1), TABS.len() - 1);
        assert_eq!(step_tab(TABS.len() - 1, 1), 0);
        assert_eq!(step_tab(2, 1), 3);
        assert_eq!(TABS[0], Tab::Bag, "the menu opens on the bag");
        assert_eq!(*TABS.last().unwrap(), Tab::System);
        // Every tab but the bag and the System list is a request-driven screen.
        for t in TABS {
            assert_eq!(t.screen().is_some(), !matches!(t, Tab::Bag | Tab::System), "{t:?}");
        }
        assert!(!SYSTEM_OPTIONS.contains(&MenuOption::Resume), "Esc closes the menu: no Resume row");
    }

    #[test]
    fn headless_menu_opens_the_bag_and_q_e_pivot_between_the_game_pages() {
        let pages = [
            (Tab::Party, MenuOption::Party),
            (Tab::Summary, MenuOption::Summary),
            (Tab::Boxes, MenuOption::Boxes),
            (Tab::Pokedex, MenuOption::Pokedex),
            (Tab::Crafting, MenuOption::Crafting),
            (Tab::Shop, MenuOption::Shop),
        ];
        let mut app = screens_app("tabs");
        step(&mut app, &[KeyCode::Escape], &[]);
        assert!(app.world().resource::<Paused>().0, "Esc opens the menu");
        assert!(app.world().resource::<PauseTabs>().active);
        step(&mut app, &[], &[KeyCode::Escape]);
        let bag = app.world().resource::<crate::game::bag_screen::BagUi>();
        assert!(bag.open && bag.mode == crate::game::bag_screen::BagMode::Bag, "the bag opens at once");
        assert!(app.world().resource::<PauseMenuSuppressed>().0, "the page owns the input");
        assert!(!pause_root_visible(&mut app), "the list stays hidden over a page");

        // E walks forward through the pages; the previous page closes, the world stays frozen.
        for (tab, row) in pages {
            step(&mut app, &[KeyCode::KeyE], &[]);
            step(&mut app, &[], &[KeyCode::KeyE]);
            assert_eq!(app.world().resource::<PauseTabs>().current(), tab);
            assert!(screen_open(&app, row), "{tab:?} must open with E");
            assert!(app.world().resource::<Paused>().0, "{tab:?}: world stays frozen");
            assert!(app.world().resource::<PauseMenuSuppressed>().0, "{tab:?}: owns input");
            assert!(!app.world().resource::<crate::game::bag_screen::BagUi>().open
                || tab == Tab::Shop, "the bag closed when leaving its tab");
        }
        // Q goes back one page: Shop -> Crafting.
        step(&mut app, &[KeyCode::KeyQ], &[]);
        step(&mut app, &[], &[KeyCode::KeyQ]);
        assert_eq!(app.world().resource::<PauseTabs>().current(), Tab::Crafting);
        assert!(screen_open(&app, MenuOption::Crafting));
        assert!(!screen_open(&app, MenuOption::Shop));
        // Q / E never reach the page underneath as raw edges.
        step(&mut app, &[KeyCode::KeyE], &[]);
        assert!(!app.world().resource::<ButtonInput<KeyCode>>().just_pressed(KeyCode::KeyE));

        // Esc inside a page closes the whole menu.
        step(&mut app, &[], &[KeyCode::KeyE]);
        step(&mut app, &[KeyCode::Escape], &[]);
        step(&mut app, &[], &[KeyCode::Escape]);
        assert!(!app.world().resource::<Paused>().0, "the world resumes");
        assert!(!app.world().resource::<PauseTabs>().active);
        assert!(!app.world().resource::<PauseMenuSuppressed>().0);
        step(&mut app, &[KeyCode::Escape], &[]);
        assert!(app.world().resource::<Paused>().0, "Esc reopens it on the first page");
        assert_eq!(app.world().resource::<PauseTabs>().current(), Tab::Bag);
    }

    #[test]
    fn headless_screens_show_live_progress_and_refuse_outside_the_world() {
        let mut app = screens_app("live");
        step(&mut app, &[KeyCode::Escape], &[]);
        step(&mut app, &[], &[KeyCode::Escape]);
        // Party is the second tab.
        step(&mut app, &[KeyCode::KeyE], &[]);
        step(&mut app, &[], &[KeyCode::KeyE]);
        assert_eq!(
            app.world().resource::<crate::game::party_screen::PartyScreenData>().party[0].species,
            25,
            "the party screen mirrors PlayerProgress"
        );
        // Crafting (tab 5) copies the player's bag when it opens.
        for _ in 0..4 {
            step(&mut app, &[KeyCode::KeyE], &[]);
            step(&mut app, &[], &[KeyCode::KeyE]);
        }
        assert_eq!(app.world().resource::<PauseTabs>().current(), Tab::Crafting);
        assert_eq!(
            app.world().resource::<crate::game::crafting_ui::CraftingBag>().0.get(&1613),
            Some(&2)
        );
        assert_eq!(app.world().resource::<crate::game::crafting_ui::CraftingCatalog>().0.len(), 43);
        step(&mut app, &[KeyCode::Escape], &[]);
        step(&mut app, &[], &[KeyCode::Escape]);
        assert!(!app.world().resource::<Paused>().0);

        // During a battle only the System list opens and Q / E do not leave it.
        app.insert_resource(State::new(GameState::Battle));
        step(&mut app, &[KeyCode::Escape], &[]);
        step(&mut app, &[], &[KeyCode::Escape]);
        assert!(app.world().resource::<Paused>().0);
        assert_eq!(app.world().resource::<PauseTabs>().current(), Tab::System);
        assert!(!app.world().resource::<PauseMenuSuppressed>().0);
        step(&mut app, &[KeyCode::KeyE], &[]);
        step(&mut app, &[], &[KeyCode::KeyE]);
        assert_eq!(app.world().resource::<PauseTabs>().current(), Tab::System);
        assert!(!screen_open(&app, MenuOption::Boxes));
    }

    #[test]
    fn page_without_its_plugin_shows_a_toast() {
        let dir = std::env::temp_dir().join(format!("pla_pause_noscreen_{}", std::process::id()));
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.add_plugins(crate::game::input_map::InputMapPlugin);
        PauseMenuPlugin::install(&mut app, UserSettings::default(), dir);
        step(&mut app, &[], &[]);
        step(&mut app, &[KeyCode::Escape], &[]);
        step(&mut app, &[KeyCode::KeyE], &[KeyCode::Escape]);
        assert!(app.world().resource::<Paused>().0, "the menu stays open");
        assert!(app.world().resource::<SaveToast>().text.contains("not available"));
    }

    #[test]
    fn headless_system_list_navigates_and_confirms_with_consumed_edges() {
        let dir = std::env::temp_dir().join(format!("pla_pause_nav_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.add_plugins(crate::game::input_map::InputMapPlugin);
        PauseMenuPlugin::install(&mut app, UserSettings::default(), dir.clone());
        step(&mut app, &[], &[]);

        step(&mut app, &[KeyCode::Escape], &[]);
        assert_eq!(app.world().resource::<PauseMenu>().selected, 0);
        assert_eq!(app.world().resource::<PauseTabs>().current(), Tab::Bag, "opens on the bag");
        // Q wraps back to the last tab: the System list.
        step(&mut app, &[KeyCode::KeyQ], &[KeyCode::Escape]);
        assert_eq!(app.world().resource::<PauseTabs>().current(), Tab::System);
        assert!(!app.world().resource::<PauseMenuSuppressed>().0, "the list owns the input");

        // Volume is the first System row: Enter steps it down and keeps the menu open.
        step(&mut app, &[KeyCode::Enter], &[KeyCode::KeyQ]);
        assert_eq!(app.world().resource::<UserSettings>().master_volume, 75);
        assert!(app.world().resource::<Paused>().0, "a toggle row keeps the menu open");
        // Enter is consumed for the raw world readers (creature encounter reads it directly).
        assert!(!app.world().resource::<ButtonInput<KeyCode>>().just_pressed(KeyCode::Enter));

        // Arrows, W and S move through the vertical list; S does not leak as the battle Switch.
        step(&mut app, &[KeyCode::ArrowDown], &[KeyCode::Enter]);
        assert_eq!(app.world().resource::<PauseMenu>().selected, 1);
        step(&mut app, &[KeyCode::KeyS], &[KeyCode::ArrowDown]);
        assert_eq!(app.world().resource::<PauseMenu>().selected, 2);
        assert!(!app.world().resource::<ActionState>().just_pressed(Action::OpenSwitch));
        step(&mut app, &[KeyCode::KeyW], &[KeyCode::KeyS]);
        assert_eq!(app.world().resource::<PauseMenu>().selected, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
