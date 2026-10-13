//! Small, data-driven party screen.
//!
//! The screen owns no save data and opens no window. A caller supplies [`PartyScreenData`] as a
//! resource, which makes the UI usable by the viewer, a future menu, or a headless Bevy test.

use bevy::prelude::*;

use super::save_game::{PartyMember, MAX_PARTY};
use super::save_wire::PlayerProgress;

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

/// Root marker, public so a headless caller can assert that the screen was installed.
#[derive(Component, Debug, Clone, Copy)]
pub struct PartyScreenRoot;

#[derive(Component, Clone, Copy)]
enum PartyText {
    Title,
    Name(usize),
    Stats(usize),
    Moves(usize),
    Hint,
}

/// A simple party UI plugin. It only installs Bevy UI entities and syncs explicit resources.
pub struct PartyScreenPlugin;

impl Plugin for PartyScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PartyScreenData>()
            .init_resource::<PartyScreenState>()
            .add_systems(Startup, spawn_party_screen)
            .add_systems(Update, sync_party_screen);
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
                    PartyText::Hint,
                    Text::new(""),
                    TextFont::from_font_size(13.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
                ));
            });
        });
}

fn sync_party_screen(
    data: Res<PartyScreenData>,
    state: Res<PartyScreenState>,
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

    for (marker, mut text) in &mut texts {
        let desired = match marker {
            PartyText::Title => "PARTY".to_string(),
            PartyText::Name(index) => data.party.get(*index).map(member_name).unwrap_or_default(),
            PartyText::Stats(index) => data.party.get(*index).map(member_stats).unwrap_or_default(),
            PartyText::Moves(index) => data.party.get(*index).map(member_moves).unwrap_or_default(),
            PartyText::Hint => "Party order is the stored order. HP 0 means fainted.".to_string(),
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
