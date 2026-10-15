//! Small, data-driven party screen.
//!
//! The screen owns no save data and opens no window. A caller supplies [`PartyScreenData`] as a
//! resource, which makes the UI usable by the viewer, a future menu, or a headless Bevy test.
//!
//! Viewer wiring: the pause menu's `Party` row raises [`PartyOpenRequested`]; the screen then
//! refreshes [`PartyScreenData`] from [`PlayerProgress`] (when that resource exists), freezes the
//! world and closes on Cancel/Menu (Esc, Backspace, gamepad B/+).
//!
//! Operations: Enter opens the list's submenu ([`PartyOpsState`], entries in [`OPS`]) whose
//! entries run the validated [`super::party_ops`] functions on [`PlayerProgress`]: Summary,
//! Move up / Move down (reorder), Send to box and Release (asked twice, like the box screen).
//! Esc leaves the submenu first, then the screen.

use bevy::prelude::*;

use super::input_map::{Action, ActionState};
use super::pause_menu::{freeze_for_screen, PartyOpenRequested, PauseMenuSuppressed};
use super::save_game::{PartyMember, MAX_PARTY};
use super::save_wire::PlayerProgress;
use super::state::Paused;
use super::summary_screen::{SummaryFromParty, SummaryState};

/// Explicit snapshot consumed by the party UI. It intentionally contains only party data, not a
/// `PlayerProgress` resource, so storage operations remain in [`super::party_ops`].
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct PartyScreenData {
    pub party: Vec<PartyMember>,
}

impl PartyScreenData {
    pub fn from_progress(progress: &PlayerProgress) -> Self {
        Self {
            party: progress.party.clone(),
        }
    }
}

/// Visibility state supplied by the owner of the screen.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartyScreenState {
    pub open: bool,
}

impl Default for PartyScreenState {
    fn default() -> Self {
        Self { open: false }
    }
}

/// Highlighted party row (Up / Down); Enter opens its operations (`PartyOpsState`).
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartyCursor(pub usize);

/// The operations submenu of the highlighted member (the game's party actions).
pub const OPS: [&'static str; 6] =
    ["Summary", "Move up", "Move down", "Send to box", "Release", "Cancel"];

/// Open state, highlighted entry and last message of the operations submenu.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct PartyOpsState {
    pub open: bool,
    pub selected: usize,
    /// A release was asked once; the next confirm releases (same rule as the boxes screen).
    pub pending_release: bool,
    pub message: String,
}

/// What activating an operations entry did; the caller closes the submenu or opens the summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpsAction {
    /// The submenu stays open (reorder, store, release, message).
    Stay,
    /// The submenu closes (Cancel).
    Close,
    /// The caller opens the summary of the highlighted member.
    Summary,
}

/// Apply operations entry `index` to party `member` through [`super::party_ops`] (the same
/// validated operations the box screen uses). Messages stay in the port's own English words.
pub fn use_party_op(
    index: usize,
    member: usize,
    ops: &mut PartyOpsState,
    cursor: &mut PartyCursor,
    progress: &mut PlayerProgress,
) -> OpsAction {
    use super::party_ops::{move_party_to_box, release_party_member, reorder_party};
    let len = progress.party.len();
    if member >= len {
        ops.message = "No party member there.".into();
        return OpsAction::Stay;
    }
    match index {
        0 => OpsAction::Summary,
        1 | 2 => {
            let target = if index == 1 {
                member.checked_sub(1)
            } else {
                (member + 1 < len).then_some(member + 1)
            };
            match target {
                None => ops.message = "Already at the end of the party.".into(),
                Some(to) => match reorder_party(progress, member, to) {
                    Ok(()) => {
                        cursor.0 = to;
                        ops.message = format!("Moved to slot {}.", to + 1);
                    }
                    Err(e) => ops.message = e.to_string(),
                },
            }
            OpsAction::Stay
        }
        3 => {
            match move_party_to_box(progress, member) {
                Ok(slot) => {
                    cursor.0 = cursor.0.min(progress.party.len().saturating_sub(1));
                    ops.message = format!("Stored in box slot #{slot:03}.");
                }
                Err(e) => ops.message = e.to_string(),
            }
            OpsAction::Stay
        }
        4 => {
            if !ops.pending_release {
                ops.pending_release = true;
                ops.message = format!(
                    "Release {}? Choose Release again to confirm.",
                    member_name(&progress.party[member])
                );
                return OpsAction::Stay;
            }
            ops.pending_release = false;
            match release_party_member(progress, member) {
                Ok(_) => {
                    cursor.0 = member.min(progress.party.len().saturating_sub(1));
                    ops.message = "Released. Bye-bye!".into();
                }
                Err(e) => ops.message = e.to_string(),
            }
            OpsAction::Stay
        }
        _ => OpsAction::Close,
    }
}

/// Root marker, public so a headless caller can assert that the screen was installed.
#[derive(Component, Debug, Clone, Copy)]
pub struct PartyScreenRoot;

#[derive(Component, Clone, Copy)]
enum PartyText {
    Title,
    Name(usize),
    Stats(usize),
    Moves(usize),
    OpsTitle,
    Ops(usize),
    OpsMessage,
    Hint,
}

/// A simple party UI plugin. It only installs Bevy UI entities and syncs explicit resources.
pub struct PartyScreenPlugin;

impl Plugin for PartyScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PartyScreenData>()
            .init_resource::<PartyScreenState>()
            .init_resource::<PartyOpenRequested>()
            .init_resource::<PartyCursor>()
            .init_resource::<PartyOpsState>()
            .add_systems(Startup, spawn_party_screen)
            .add_systems(
                Update,
                (party_open_close, refresh_party_data, sync_party_screen).chain(),
            );
    }
}

fn spawn_party_screen(mut commands: Commands) {
    commands
        .spawn((
            PartyScreenRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.68)),
            GlobalZIndex(130),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Px(980.0),
                    max_height: Val::Percent(90.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.13, 0.98)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    PartyText::Title,
                    Text::new("PARTY"),
                    TextFont::from_font_size(28.0),
                    TextColor(Color::srgb(0.95, 0.9, 0.6)),
                ));
                for index in 0..MAX_PARTY {
                    panel
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                                row_gap: Val::Px(2.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.12, 0.16, 0.23, 0.92)),
                        ))
                        .with_children(|row| {
                            row.spawn((
                                PartyText::Name(index),
                                Text::new(""),
                                TextFont::from_font_size(18.0),
                                TextColor(Color::WHITE),
                            ));
                            row.spawn((
                                PartyText::Stats(index),
                                Text::new(""),
                                TextFont::from_font_size(14.0),
                                TextColor(Color::srgb(0.82, 0.88, 0.95)),
                            ));
                            row.spawn((
                                PartyText::Moves(index),
                                Text::new(""),
                                TextFont::from_font_size(14.0),
                                TextColor(Color::srgb(0.75, 0.9, 0.78)),
                            ));
                        });
                }
                panel.spawn((
                    PartyText::OpsTitle,
                    Text::new(""),
                    TextFont::from_font_size(17.0),
                    TextColor(Color::srgb(0.95, 0.9, 0.6)),
                ));
                for index in 0..OPS.len() {
                    panel.spawn((
                        PartyText::Ops(index),
                        Text::new(""),
                        TextFont::from_font_size(16.0),
                        TextColor(Color::WHITE),
                    ));
                }
                panel.spawn((
                    PartyText::OpsMessage,
                    Text::new(""),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::srgb(0.95, 0.8, 0.5)),
                ));
                panel.spawn((
                    PartyText::Hint,
                    Text::new(""),
                    TextFont::from_font_size(13.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
                ));
            });
        });
}

/// Opens on the pause menu's request and closes on Cancel/Menu, freezing the world meanwhile.
/// Prev / Next move the cursor and Confirm opens the highlighted member's operations submenu;
/// while the submenu or the summary is open they own the input (Esc leaves the submenu first).
#[allow(clippy::too_many_arguments)]
fn party_open_close(
    mut request: ResMut<PartyOpenRequested>,
    mut state: ResMut<PartyScreenState>,
    mut cursor: ResMut<PartyCursor>,
    mut ops: ResMut<PartyOpsState>,
    mut progress: Option<ResMut<PlayerProgress>>,
    summary: Option<Res<SummaryState>>,
    mut to_summary: Option<ResMut<SummaryFromParty>>,
    actions: Option<ResMut<ActionState>>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
) {
    if request.0 {
        request.0 = false;
        state.open = true;
        cursor.0 = 0;
        *ops = PartyOpsState::default();
        freeze_for_screen(&mut paused, &mut suppressed, true);
        return;
    }
    if !state.open || summary.is_some_and(|s| s.open) {
        return;
    }
    let Some(mut actions) = actions else { return };
    if ops.open {
        if actions.just_pressed(Action::Next) {
            actions.consume(Action::Next);
            ops.selected = (ops.selected + 1) % OPS.len();
            ops.pending_release = false;
        } else if actions.just_pressed(Action::Prev) {
            actions.consume(Action::Prev);
            ops.selected = (ops.selected + OPS.len() - 1) % OPS.len();
            ops.pending_release = false;
        }
        if actions.just_pressed(Action::Confirm) {
            actions.consume(Action::Confirm);
            if let Some(progress) = progress.as_deref_mut() {
                match use_party_op(ops.selected, cursor.0, &mut ops, &mut cursor, progress) {
                    OpsAction::Summary => {
                        ops.open = false;
                        if let Some(req) = to_summary.as_deref_mut() {
                            req.0 = Some(cursor.0);
                        }
                    }
                    OpsAction::Close => {
                        ops.open = false;
                        ops.message.clear();
                    }
                    OpsAction::Stay => {}
                }
            }
        }
        if actions.just_pressed(Action::Cancel) || actions.just_pressed(Action::Menu) {
            actions.consume(Action::Cancel);
            actions.consume(Action::Menu);
            ops.open = false;
            ops.pending_release = false;
            ops.message.clear();
        }
        return;
    }
    let len = progress.as_deref().map_or(0, |p| p.party.len());
    if len > 0 {
        if actions.just_pressed(Action::Next) {
            cursor.0 = (cursor.0 + 1) % len;
        } else if actions.just_pressed(Action::Prev) {
            cursor.0 = (cursor.0 + len - 1) % len;
        }
        cursor.0 = cursor.0.min(len - 1);
        if actions.just_pressed(Action::Confirm) {
            actions.consume(Action::Confirm);
            ops.open = true;
            ops.selected = 0;
            ops.pending_release = false;
            ops.message.clear();
        }
    }
    if actions.just_pressed(Action::Cancel) || actions.just_pressed(Action::Menu) {
        actions.consume(Action::Cancel);
        actions.consume(Action::Menu);
        state.open = false;
        freeze_for_screen(&mut paused, &mut suppressed, false);
    }
}

/// While open, mirror the live party (callers without `PlayerProgress` keep their own data).
fn refresh_party_data(
    state: Res<PartyScreenState>,
    progress: Option<Res<PlayerProgress>>,
    mut data: ResMut<PartyScreenData>,
) {
    let (true, Some(progress)) = (state.open, progress) else {
        return;
    };
    if data.party != progress.party {
        *data = PartyScreenData::from_progress(&progress);
    }
}

fn sync_party_screen(
    data: Res<PartyScreenData>,
    state: Res<PartyScreenState>,
    cursor: Option<Res<PartyCursor>>,
    ops: Option<Res<PartyOpsState>>,
    mut root: Query<&mut Visibility, With<PartyScreenRoot>>,
    mut texts: Query<(&PartyText, &mut Text)>,
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
    let ops_open = ops.as_ref().is_some_and(|o| o.open);

    for (marker, mut text) in &mut texts {
        let desired = match marker {
            PartyText::Title => "PARTY".to_string(),
            PartyText::Name(index) => {
                let mark = if cursor.as_ref().is_some_and(|c| c.0 == *index) { "> " } else { "" };
                data.party
                    .get(*index)
                    .map(|m| format!("{mark}{}", member_name(m)))
                    .unwrap_or_default()
            }
            PartyText::Stats(index) => data.party.get(*index).map(member_stats).unwrap_or_default(),
            PartyText::Moves(index) => data.party.get(*index).map(member_moves).unwrap_or_default(),
            PartyText::OpsTitle => {
                if ops_open {
                    let who = cursor
                        .as_ref()
                        .and_then(|c| data.party.get(c.0))
                        .map(member_name)
                        .unwrap_or_default();
                    format!("Actions: {who}")
                } else {
                    String::new()
                }
            }
            PartyText::Ops(index) => match (ops_open, ops.as_deref()) {
                (true, Some(ops)) => ops
                    .selected
                    .eq(index)
                    .then_some(OPS[*index])
                    .map(|label| format!("> {label}"))
                    .unwrap_or_else(|| format!("  {}", OPS[*index])),
                _ => String::new(),
            },
            PartyText::OpsMessage => match (ops_open, ops.as_deref()) {
                (true, Some(ops)) if !ops.message.is_empty() => ops.message.clone(),
                (true, _) => "Enter: choose   Esc: back".to_string(),
                _ => String::new(),
            },
            PartyText::Hint => {
                "Up / Down: choose   Enter: actions   Esc: close".to_string()
            }
        };
        if text.0 != desired {
            text.0 = desired;
        }
    }
}

/// Format a member's heading without relying on species or move-name tables.
pub fn member_name(member: &PartyMember) -> String {
    let name = member.nickname.as_deref().unwrap_or("unnamed");
    format!(
        "{name}  | species #{:04}  | form {}",
        member.species, member.form
    )
}

/// Format the persisted stats visible without a decoded personal-data provider.
pub fn member_stats(member: &PartyMember) -> String {
    let status = member.status.map_or("healthy", |status| match status {
        super::save_game::SavedStatus::Drowsy => "drowsy",
        super::save_game::SavedStatus::Paralysis => "paralysis",
        super::save_game::SavedStatus::Poison => "poison",
        super::save_game::SavedStatus::Burn => "burn",
        super::save_game::SavedStatus::Frostbite => "frostbite",
    });
    format!(
        "Level {}  | HP {}  | status {}  | effort [{},{},{},{},{},{}]",
        member.level,
        member.hp,
        status,
        member.effort[0],
        member.effort[1],
        member.effort[2],
        member.effort[3],
        member.effort[4],
        member.effort[5]
    )
}

/// Format all persisted move slots, including remaining PP.
pub fn member_moves(member: &PartyMember) -> String {
    if member.moves.is_empty() {
        return "Moves: none".to_string();
    }
    let moves = member
        .moves
        .iter()
        .map(|m| format!("move #{:04} (PP {})", m.move_id, m.pp))
        .collect::<Vec<_>>()
        .join("  ");
    format!("Moves: {moves}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::save_game::SavedMove;

    fn member() -> PartyMember {
        PartyMember {
            species: 25,
            form: 1,
            level: 12,
            hp: 34,
            moves: vec![SavedMove {
                move_id: 33,
                pp: 12,
            }],
            effort: [1, 2, 3, 4, 5, 6],
            nickname: Some("Sparky".into()),
            ..Default::default()
        }
    }

    #[test]
    fn formatters_render_stats_and_moves_from_explicit_data() {
        let m = member();
        assert!(member_name(&m).contains("Sparky"));
        assert!(member_stats(&m).contains("HP 34"));
        assert!(member_stats(&m).contains("effort [1,2,3,4,5,6]"));
        assert!(member_moves(&m).contains("move #0033 (PP 12)"));
    }

    fn ops_app(n: usize) -> App {
        use crate::game::input_map::InputMapPlugin;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_plugins(InputMapPlugin);
        app.init_resource::<Paused>();
        app.init_resource::<PauseMenuSuppressed>();
        app.init_resource::<SummaryFromParty>();
        let mut progress = PlayerProgress::new_game();
        progress.party = (0..n)
            .map(|i| PartyMember {
                species: 25 + i as u16,
                level: 5,
                hp: 10,
                ..Default::default()
            })
            .collect();
        progress.box_list.clear();
        app.insert_resource(progress);
        app.add_plugins(PartyScreenPlugin);
        app
    }

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(key);
        keys.clear();
    }

    /// Enter opens the operations submenu on the highlighted member; its entries run the
    /// validated `party_ops` functions (reorder, send to box, release asked twice).
    #[test]
    fn operations_submenu_reorders_stores_and_releases_through_party_ops() {
        let mut app = ops_app(3);
        app.world_mut().resource_mut::<PartyOpenRequested>().0 = true;
        app.update();
        assert!(app.world().resource::<PartyScreenState>().open);
        press(&mut app, KeyCode::Enter);
        assert!(app.world().resource::<PartyOpsState>().open, "Enter opens the submenu");
        assert_eq!(app.world().resource::<PartyOpsState>().selected, 0);
        // Summary: asks the summary screen for the highlighted member and closes the submenu.
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.world().resource::<SummaryFromParty>().0, Some(0));
        assert!(!app.world().resource::<PartyOpsState>().open);

        // Move down: member 0 changes places with member 1 and the cursor follows it.
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(app.world().resource::<PartyOpsState>().selected, 2);
        press(&mut app, KeyCode::Enter);
        let party: Vec<u16> = app
            .world()
            .resource::<PlayerProgress>()
            .party
            .iter()
            .map(|m| m.species)
            .collect();
        assert_eq!(party, [26, 25, 27]);
        assert_eq!(app.world().resource::<PartyCursor>().0, 1);
        assert!(app.world().resource::<PartyOpsState>().message.contains("slot 2"));

        // Send to box: the highlighted member leaves the party.
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        let progress = app.world().resource::<PlayerProgress>();
        assert_eq!(progress.party.iter().map(|m| m.species).collect::<Vec<_>>(), [26, 27]);
        assert_eq!(progress.box_list.iter().map(|m| m.species).collect::<Vec<_>>(), [25]);
        assert!(app.world().resource::<PartyOpsState>().message.contains("box slot #000"));

        // Release asks twice, then refuses the last healthy member.
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        assert!(app.world().resource::<PartyOpsState>().pending_release);
        assert!(app.world().resource::<PartyOpsState>().message.contains("confirm"));
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.world().resource::<PlayerProgress>().party.len(), 1);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);
        let ops = app.world().resource::<PartyOpsState>();
        assert!(!ops.pending_release);
        assert!(
            ops.message.contains("non-fainted"),
            "the last healthy member is protected: {}",
            ops.message
        );

        // Esc leaves the submenu first, then the screen.
        press(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<PartyOpsState>().open);
        assert!(app.world().resource::<PartyScreenState>().open);
        press(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<PartyScreenState>().open);
        assert!(!app.world().resource::<Paused>().0, "resumes the world");
    }

    #[test]
    fn headless_plugin_renders_without_a_window() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, PartyScreenPlugin));
        app.insert_resource(PartyScreenData {
            party: vec![member()],
        });
        app.insert_resource(PartyScreenState { open: true });
        app.update();

        let mut query = app.world_mut().query::<&Text>();
        let rendered = query
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect::<Vec<_>>();
        assert!(rendered.iter().any(|text| text.contains("Sparky")));
        assert!(rendered.iter().any(|text| text.contains("move #0033")));
        let mut roots = app
            .world_mut()
            .query_filtered::<Entity, With<PartyScreenRoot>>();
        assert!(roots.iter(app.world()).next().is_some());
    }
}
