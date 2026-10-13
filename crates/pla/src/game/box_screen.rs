//! A headless-friendly box screen for the stored party members.
//!
//! The screen deliberately owns no species names, icons, or save-schema logic. It renders numeric
//! species/form ids from the explicit [`BoxScreenData`] provider and sends party/box mutations
//! through the existing [`PlayerProgress`] operation boundary (`deposit`, `withdraw`, and `swap`).
//! The `B` key is only a small standalone opener; callers can also set [`BoxScreenState::open`]
//! directly from another menu.

use bevy::prelude::*;

use super::pause_menu::PauseMenuSuppressed;
use super::save_game::{MAX_BOX, MAX_PARTY, PartyMember};
use super::save_wire::{BoxError, PlayerProgress};
use super::state::Paused;

/// The documented provisional capacity is 30 boxes of 32 slots.
pub const SLOTS_PER_BOX: usize = 32;
/// The readable grid has eight columns and four rows.
pub const GRID_COLUMNS: usize = 8;
/// The readable grid has four rows and eight columns.
pub const GRID_ROWS: usize = SLOTS_PER_BOX / GRID_COLUMNS;

const _: () = assert!(GRID_COLUMNS * GRID_ROWS == SLOTS_PER_BOX);

/// Which side currently owns keyboard selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxFocus {
    Box,
    Party,
}

/// A selected storage location. This is UI state, not a save-schema type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxSelection {
    Box(usize),
    Party(usize),
}

/// Screen state kept separate from save data so a caller can open it without a viewer/window.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct BoxScreenState {
    pub open: bool,
    /// Zero-based box page. Each page contains [`SLOTS_PER_BOX`] slots.
    pub page: usize,
    /// Zero-based slot inside the current page.
    pub selected_slot: usize,
    pub selected_party: usize,
    pub focus: BoxFocus,
    pub held: Option<BoxSelection>,
    pub message: String,
}

impl Default for BoxScreenState {
    fn default() -> Self {
        Self {
            open: false,
            page: 0,
            selected_slot: 0,
            selected_party: 0,
            focus: BoxFocus::Box,
            held: None,
            message: String::new(),
        }
    }
}

/// Explicit render provider for the screen. It is a snapshot, so rendering never borrows or
/// invents data from an asset table. The plugin refreshes it from [`PlayerProgress`].
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct BoxScreenData {
    pub party: Vec<PartyMember>,
    pub box_list: Vec<PartyMember>,
}

impl BoxScreenData {
    pub fn from_progress(progress: &PlayerProgress) -> Self {
        Self {
            party: progress.party.clone(),
            box_list: progress.box_list.clone(),
        }
    }
}

/// Root marker for headless tests and callers that want to control visibility.
#[derive(Component, Debug, Clone, Copy)]
pub struct BoxScreenRoot;

#[derive(Component, Clone, Copy)]
enum BoxText {
    Title,
    Page,
    Cell(usize),
    Party(usize),
    Status,
    Hint,
}

/// Direction used by the deterministic grid-selection helper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Number of pages required for the declared maximum box size.
pub const fn page_count() -> usize {
    MAX_BOX.div_ceil(SLOTS_PER_BOX)
}

/// Convert a page-local slot to a row/column in the visible grid.
pub const fn grid_position(slot: usize) -> (usize, usize) {
    (slot / GRID_COLUMNS, slot % GRID_COLUMNS)
}

/// Move a page-local selection with wrapping in both axes.
pub fn move_grid_selection(slot: usize, direction: GridDirection) -> usize {
    let slot = slot.min(SLOTS_PER_BOX - 1);
    let (row, column) = grid_position(slot);
    let row = match direction {
        GridDirection::Up => (row + GRID_ROWS - 1) % GRID_ROWS,
        GridDirection::Down => (row + 1) % GRID_ROWS,
        GridDirection::Left | GridDirection::Right => row,
    };
    let column = match direction {
        GridDirection::Left => (column + GRID_COLUMNS - 1) % GRID_COLUMNS,
        GridDirection::Right => (column + 1) % GRID_COLUMNS,
        GridDirection::Up | GridDirection::Down => column,
    };
    row * GRID_COLUMNS + column
}

/// Move a linear party selection with wrapping; an empty party stays at zero.
pub fn move_party_selection(selected: usize, delta: i32, party_len: usize) -> usize {
    if party_len == 0 {
        return 0;
    }
    ((selected as i32 + delta).rem_euclid(party_len as i32)) as usize
}

/// Move a party member into the next free box slot without partial mutation.
pub fn move_party_to_box(
    progress: &mut PlayerProgress,
    party_index: usize,
) -> Result<usize, BoxError> {
    if party_index >= progress.party.len() {
        return Err(BoxError::BadPartySlot);
    }
    if progress.box_list.len() >= MAX_BOX {
        return Err(BoxError::BoxFull);
    }
    let member = progress.party.remove(party_index);
    match progress.deposit(member) {
        Ok(slot) => Ok(slot),
        Err(error) => unreachable!("box capacity was checked before deposit: {error}"),
    }
}

/// Move a boxed member into the party through the existing pure save operation.
pub fn move_box_to_party(
    progress: &mut PlayerProgress,
    box_index: usize,
) -> Result<PartyMember, BoxError> {
    progress.withdraw(box_index)
}

/// Exchange one party member with one boxed member through the existing pure save operation.
pub fn swap_party_with_box(
    progress: &mut PlayerProgress,
    party_index: usize,
    box_index: usize,
) -> Result<(), BoxError> {
    progress.swap(box_index, party_index)
}

pub struct BoxScreenPlugin;

impl Plugin for BoxScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoxScreenState>()
            .init_resource::<BoxScreenData>()
            .init_resource::<PlayerProgress>()
            .init_resource::<Paused>()
            .init_resource::<PauseMenuSuppressed>()
            .add_systems(Startup, spawn_box_screen)
            .add_systems(Update, (box_input, sync_box_data, sync_box_screen).chain());
    }
}

fn spawn_box_screen(mut commands: Commands) {
    commands
        .spawn((
            BoxScreenRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(125),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Px(1040.0),
                    max_height: Val::Percent(92.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.13, 0.98)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    BoxText::Title,
                    Text::new("POKEMON BOX"),
                    TextFont::from_font_size(28.0),
                    TextColor(Color::srgb(0.95, 0.9, 0.6)),
                ));
                panel.spawn((
                    BoxText::Page,
                    Text::new(""),
                    TextFont::from_font_size(17.0),
                    TextColor(Color::srgb(0.75, 0.85, 1.0)),
                ));
                panel
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            column_gap: Val::Px(4.0),
                            row_gap: Val::Px(4.0),
                            width: Val::Px(1000.0),
                            ..default()
                        },
                    ))
                    .with_children(|grid| {
                        for index in 0..SLOTS_PER_BOX {
                            grid.spawn((
                                Node {
                                    width: Val::Px(122.0),
                                    min_height: Val::Px(68.0),
                                    padding: UiRect::all(Val::Px(6.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.12, 0.16, 0.23, 0.92)),
                            ))
                            .with_children(|cell| {
                                cell.spawn((
                                    BoxText::Cell(index),
                                    Text::new(""),
                                    TextFont::from_font_size(14.0),
                                    TextColor(Color::WHITE),
                                ));
                            });
                        }
                    });
                panel.spawn((
                    Text::new("PARTY"),
                    TextFont::from_font_size(19.0),
                    TextColor(Color::srgb(0.9, 0.82, 0.55)),
                ));
                for index in 0..MAX_PARTY {
                    panel.spawn((
                        BoxText::Party(index),
                        Text::new(""),
                        TextFont::from_font_size(15.0),
                        TextColor(Color::srgb(0.82, 0.88, 0.95)),
                    ));
                }
                panel.spawn((
                    BoxText::Status,
                    Text::new(""),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::srgb(0.95, 0.75, 0.45)),
                ));
                panel.spawn((
                    BoxText::Hint,
                    Text::new(
                        "Arrows move | PageUp/PageDown change box | Tab changes side | Enter selects/swaps | D stores party | W withdraws | Esc closes | B opens",
                    ),
                    TextFont::from_font_size(13.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
                ));
            });
        });
}

fn set_freeze(
    paused: &mut Option<ResMut<Paused>>,
    suppressed: &mut Option<ResMut<PauseMenuSuppressed>>,
    frozen: bool,
) {
    if let Some(paused) = paused.as_deref_mut() {
        paused.0 = frozen;
    }
    if let Some(suppressed) = suppressed.as_deref_mut() {
        suppressed.0 = frozen;
    }
}

#[allow(clippy::too_many_arguments)]
fn box_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<BoxScreenState>,
    mut progress: ResMut<PlayerProgress>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
) {
    if keys.just_pressed(KeyCode::KeyB) {
        state.open = !state.open;
        state.held = None;
        state.message.clear();
        set_freeze(&mut paused, &mut suppressed, state.open);
        return;
    }
    if !state.open {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        state.open = false;
        state.held = None;
        set_freeze(&mut paused, &mut suppressed, false);
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        state.focus = match state.focus {
            BoxFocus::Box => BoxFocus::Party,
            BoxFocus::Party => BoxFocus::Box,
        };
        state.message.clear();
    }
    if keys.just_pressed(KeyCode::PageDown) {
        state.page = (state.page + 1) % page_count();
        state.selected_slot = 0;
        state.held = None;
        state.message.clear();
    } else if keys.just_pressed(KeyCode::PageUp) {
        state.page = (state.page + page_count() - 1) % page_count();
        state.selected_slot = 0;
        state.held = None;
        state.message.clear();
    }

    let direction = if keys.just_pressed(KeyCode::ArrowUp) {
        Some(GridDirection::Up)
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        Some(GridDirection::Down)
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        Some(GridDirection::Left)
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        Some(GridDirection::Right)
    } else {
        None
    };
    if let Some(direction) = direction {
        match state.focus {
            BoxFocus::Box => {
                state.selected_slot = move_grid_selection(state.selected_slot, direction)
            }
            BoxFocus::Party => {
                let delta = match direction {
                    GridDirection::Up | GridDirection::Left => -1,
                    GridDirection::Down | GridDirection::Right => 1,
                };
                state.selected_party =
                    move_party_selection(state.selected_party, delta, progress.party.len());
            }
        }
    }

    if keys.just_pressed(KeyCode::KeyD) {
        if state.focus != BoxFocus::Party {
            state.message = "Focus PARTY before storing a member.".into();
        } else {
            match move_party_to_box(&mut progress, state.selected_party) {
                Ok(slot) => state.message = format!("Stored party member in box slot #{slot:03}."),
                Err(error) => state.message = error.to_string(),
            }
        }
    }
    if keys.just_pressed(KeyCode::KeyW) {
        if state.focus != BoxFocus::Box {
            state.message = "Focus BOX before withdrawing a member.".into();
        } else {
            let index = state.page * SLOTS_PER_BOX + state.selected_slot;
            match move_box_to_party(&mut progress, index) {
                Ok(_) => {
                    state.focus = BoxFocus::Party;
                    state.selected_party = progress.party.len().saturating_sub(1);
                    state.message = format!("Moved box slot #{index:03} to the party.");
                }
                Err(error) => state.message = error.to_string(),
            }
        }
    }
    if keys.just_pressed(KeyCode::Enter) {
        activate_selection(&mut state, &mut progress);
    }
}

fn activate_selection(state: &mut BoxScreenState, progress: &mut PlayerProgress) {
    let current = match state.focus {
        BoxFocus::Box => {
            let index = state.page * SLOTS_PER_BOX + state.selected_slot;
            (progress.box_list.get(index).is_some()).then_some(BoxSelection::Box(index))
        }
        BoxFocus::Party => progress
            .party
            .get(state.selected_party)
            .map(|_| BoxSelection::Party(state.selected_party)),
    };
    let Some(current) = current else {
        state.message = "That slot is empty.".into();
        return;
    };

    match (state.held, current) {
        (None, current) => {
            state.held = Some(current);
            state.message = "Selected. Choose the other side and press Enter.".into();
        }
        (Some(BoxSelection::Box(box_index)), BoxSelection::Party(party_index)) => {
            state.message =
                operation_message(swap_party_with_box(progress, party_index, box_index));
            state.held = None;
        }
        (Some(BoxSelection::Party(party_index)), BoxSelection::Box(box_index)) => {
            state.message =
                operation_message(swap_party_with_box(progress, party_index, box_index));
            state.held = None;
        }
        (Some(previous), current) if previous == current => {
            state.message = "Selection cleared.".into();
            state.held = None;
        }
        (Some(_), current) => {
            state.held = Some(current);
            state.message = "Selection changed.".into();
        }
    }
}

fn operation_message(result: Result<(), BoxError>) -> String {
    match result {
        Ok(()) => "Party and box members exchanged.".into(),
        Err(error) => error.to_string(),
    }
}

fn sync_box_data(progress: Res<PlayerProgress>, mut data: ResMut<BoxScreenData>) {
    let snapshot = BoxScreenData::from_progress(&progress);
    if *data != snapshot {
        *data = snapshot;
    }
}

fn sync_box_screen(
    data: Res<BoxScreenData>,
    state: Res<BoxScreenState>,
    mut root: Query<&mut Visibility, With<BoxScreenRoot>>,
    mut texts: Query<(&BoxText, &mut Text)>,
) {
    for mut visibility in &mut root {
        *visibility = if state.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !state.open {
        return;
    }

    let first = state.page * SLOTS_PER_BOX;
    for (marker, mut text) in &mut texts {
        let desired = match marker {
            BoxText::Title => "POKEMON BOX".into(),
            BoxText::Page => format!(
                "BOX {:02}/{:02}  |  stored {}/{}  |  slots {:03}-{:03}",
                state.page + 1,
                page_count(),
                data.box_list.len(),
                MAX_BOX,
                first + 1,
                (first + SLOTS_PER_BOX).min(MAX_BOX)
            ),
            BoxText::Cell(local) => {
                let index = first + *local;
                let selected = state.focus == BoxFocus::Box && state.selected_slot == *local;
                let held = state.held == Some(BoxSelection::Box(index));
                slot_label(index, data.box_list.get(index), selected, held)
            }
            BoxText::Party(index) => {
                let selected = state.focus == BoxFocus::Party && state.selected_party == *index;
                let held = state.held == Some(BoxSelection::Party(*index));
                party_label(*index, data.party.get(*index), selected, held)
            }
            BoxText::Status => state.message.clone(),
            BoxText::Hint => "Arrows move | PageUp/PageDown change box | Tab changes side | Enter selects/swaps | D stores party | W withdraws | Esc closes | B opens".into(),
        };
        if text.0 != desired {
            text.0 = desired;
        }
    }
}

fn slot_label(index: usize, member: Option<&PartyMember>, selected: bool, held: bool) -> String {
    let cursor = if held {
        "<>"
    } else if selected {
        ">>"
    } else {
        "  "
    };
    match member {
        Some(member) => format!(
            "{cursor} #{:03}\nSpecies #{:04}\nLv {} | HP {}",
            index + 1,
            member.species,
            member.level,
            member.hp
        ),
        None => format!("{cursor} #{:03}\n(empty)", index + 1),
    }
}

fn party_label(index: usize, member: Option<&PartyMember>, selected: bool, held: bool) -> String {
    let cursor = if held {
        "<>"
    } else if selected {
        ">>"
    } else {
        "  "
    };
    match member {
        Some(member) => format!(
            "{cursor} Party {}: Species #{:04} | Lv {} | HP {}",
            index + 1,
            member.species,
            member.level,
            member.hp
        ),
        None => format!("{cursor} Party {}: (empty)", index + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::save_game::SavedMove;

    fn member(species: u16, hp: u32) -> PartyMember {
        PartyMember {
            species,
            level: 10,
            hp,
            moves: vec![SavedMove {
                move_id: species,
                pp: 10,
            }],
            ..Default::default()
        }
    }

    fn progress(party: &[u16], boxed: &[u16]) -> PlayerProgress {
        PlayerProgress {
            party: party.iter().map(|&species| member(species, 10)).collect(),
            box_list: boxed.iter().map(|&species| member(species, 10)).collect(),
            ..PlayerProgress::new_game()
        }
    }

    #[test]
    fn grid_layout_is_deterministic_and_complete() {
        assert_eq!(page_count(), 30);
        assert_eq!(grid_position(0), (0, 0));
        assert_eq!(grid_position(7), (0, 7));
        assert_eq!(grid_position(8), (1, 0));
        assert_eq!(grid_position(31), (3, 7));
        assert_eq!(
            (0..SLOTS_PER_BOX)
                .map(grid_position)
                .collect::<Vec<_>>()
                .len(),
            SLOTS_PER_BOX
        );
    }

    #[test]
    fn selection_moves_and_wraps_without_changing_box_slot_order() {
        assert_eq!(move_grid_selection(0, GridDirection::Left), 7);
        assert_eq!(move_grid_selection(7, GridDirection::Right), 0);
        assert_eq!(move_grid_selection(0, GridDirection::Up), 24);
        assert_eq!(move_grid_selection(31, GridDirection::Down), 7);
        assert_eq!(move_party_selection(0, -1, 3), 2);
        assert_eq!(move_party_selection(2, 1, 3), 0);

        let p = progress(&[1, 2], &[3, 4]);
        assert_eq!(
            p.box_list.iter().map(|m| m.species).collect::<Vec<_>>(),
            [3, 4]
        );
    }

    #[test]
    fn empty_box_operation_and_render_provider_are_stable() {
        let mut p = progress(&[25], &[]);
        assert_eq!(move_box_to_party(&mut p, 0), Err(BoxError::BadBoxSlot));
        let data = BoxScreenData::from_progress(&p);
        assert!(data.box_list.is_empty());
        assert!(slot_label(0, None, true, false).contains("(empty)"));
    }

    #[test]
    fn full_box_refuses_party_deposit_without_mutating_party() {
        let mut p = progress(&[25], &[]);
        p.box_list = (0..MAX_BOX).map(|i| member(i as u16, 10)).collect();
        let before = p.party.clone();
        assert_eq!(move_party_to_box(&mut p, 0), Err(BoxError::BoxFull));
        assert_eq!(p.party, before);
        assert_eq!(p.box_list.len(), MAX_BOX);
    }

    #[test]
    fn party_box_moves_use_the_existing_operation_boundary() {
        let mut p = progress(&[25, 26], &[133]);
        assert_eq!(move_party_to_box(&mut p, 1), Ok(1));
        assert_eq!(p.party.iter().map(|m| m.species).collect::<Vec<_>>(), [25]);
        assert_eq!(move_box_to_party(&mut p, 0).unwrap().species, 133);
        assert_eq!(
            p.party.iter().map(|m| m.species).collect::<Vec<_>>(),
            [25, 133]
        );
        swap_party_with_box(&mut p, 0, 0).unwrap();
        assert_eq!(p.party[0].species, 26);
        assert_eq!(p.box_list[0].species, 25);
    }

    #[test]
    fn headless_plugin_renders_empty_and_full_pages() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, BoxScreenPlugin));
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(PlayerProgress::new_game());
        app.insert_resource(BoxScreenState {
            open: true,
            ..default()
        });
        app.update();

        let mut query = app.world_mut().query::<&Text>();
        let empty_text = query
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect::<Vec<_>>();
        assert!(empty_text.iter().any(|text| text.contains("(empty)")));

        let mut full = PlayerProgress::new_game();
        full.box_list = (0..SLOTS_PER_BOX)
            .map(|i| member(i as u16 + 1, 10))
            .collect();
        app.insert_resource(full);
        app.update();
        let mut query = app.world_mut().query::<&Text>();
        let full_text = query
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect::<Vec<_>>();
        assert!(full_text.iter().any(|text| text.contains("stored 32/960")));
        assert!(full_text.iter().any(|text| text.contains("Species #0032")));
    }
}
