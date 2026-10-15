//! Pokémon summary screen: one party member's level, experience, types, stats and moves.
//!
//! Opens from the party screen (Enter on the highlighted member, back to the party list on
//! Esc) or from the pause menu's `Summary` row (first member; Esc returns to the world). While
//! open the world stays frozen like every other menu screen. Keys: Left / Right (A / D, Q / E)
//! change the page, Up / Down (W / S) change the member (on the Moves page they pick the move
//! whose details show; PageUp / PageDown change the member everywhere), Esc / Backspace close.
//!
//! The screen owns no save data: [`summary_info`] derives everything from the live
//! [`PlayerProgress`] party and the battle data tables (the port's own English names). The
//! original look (`orig_ui`, layout `status_top_00`) and the provisional `bevy_ui` panel below
//! show the same [`SummaryInfo`]. Natures are not stored by the port's save and the game's
//! battles have no abilities, so both are shown as placeholders.

use bevy::prelude::*;

use super::battle_controller::BattleAssets;
use super::input_map::{Action, ActionState};
use super::pause_menu::{freeze_for_screen, PauseMenuSuppressed};
use super::party_screen::PartyScreenState;
use super::save_game::PartyMember;
use super::save_wire::PlayerProgress;
use super::state::Paused;
use crate::battle_engine::progression::exp_for_level;
use crate::battle_engine::Combatant;

/// Pages of the summary, in the game's tab order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SummaryPage {
    /// Name, level, types, experience, nature / ability placeholders.
    #[default]
    Info,
    /// The four moves with type, category, power, accuracy and PP.
    Moves,
    /// HP and the five battle stats.
    Stats,
    /// Notes: the port's own text about what is (not) tracked.
    Memo,
}

impl SummaryPage {
    pub const ALL: [SummaryPage; 4] = [
        SummaryPage::Info,
        SummaryPage::Moves,
        SummaryPage::Stats,
        SummaryPage::Memo,
    ];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }

    /// Next (`delta` = 1) or previous (-1) page, wrapping.
    pub fn step(self, delta: i32) -> SummaryPage {
        let n = Self::ALL.len() as i32;
        Self::ALL[(self.index() as i32 + delta).rem_euclid(n) as usize]
    }

    pub fn title(self) -> &'static str {
        match self {
            SummaryPage::Info => "Info",
            SummaryPage::Moves => "Moves",
            SummaryPage::Stats => "Stats",
            SummaryPage::Memo => "Notes",
        }
    }
}

/// Raised by the pause menu's `Summary` row (or a test) to open the summary on member 0.
#[derive(Resource, Debug, Default)]
pub struct SummaryOpenRequested(pub bool);

/// Raised by the party screen to open the summary of one member (back to the party on Esc).
#[derive(Resource, Debug, Default)]
pub struct SummaryFromParty(pub Option<usize>);

/// Summary screen state.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct SummaryState {
    pub open: bool,
    /// Party index shown.
    pub member: usize,
    pub page: SummaryPage,
    /// Move whose details the Moves page shows.
    pub move_cursor: usize,
    /// Opened from the party screen: Esc returns to it instead of the world.
    pub from_party: bool,
}

/// One move row of the summary.
#[derive(Debug, Clone, PartialEq)]
pub struct SummaryMove {
    pub name: String,
    pub move_type: String,
    /// "Physical" / "Special" / "Status" as the battle data spells it.
    pub category: String,
    pub power: u16,
    pub accuracy: u8,
    pub pp: u8,
    pub max_pp: Option<u8>,
}

/// Everything the summary shows about one member.
#[derive(Debug, Clone, PartialEq)]
pub struct SummaryInfo {
    pub dex: u16,
    pub form: u8,
    pub name: String,
    pub level: u8,
    /// One or two type names (the second is omitted when it repeats the first).
    pub types: Vec<String>,
    pub hp: u32,
    /// HP / Attack / Defense / Sp. Atk / Sp. Def / Speed at this level and effort (neutral
    /// nature); `None` when the species is missing from the battle data.
    pub stats: Option<[u32; 6]>,
    pub effort: [u8; 6],
    /// Total experience (the level threshold when the save does not hold it).
    pub exp: Option<u32>,
    /// Experience still needed for the next level (`Some(0)` at level 100).
    pub exp_to_next: Option<u32>,
    /// Share of the current level already earned (0-1).
    pub exp_ratio: f32,
    pub status: &'static str,
    pub moves: Vec<SummaryMove>,
}

/// Stat names in the order of [`SummaryInfo::stats`].
pub const STAT_NAMES: [&str; 6] = ["HP", "Attack", "Defense", "Sp. Atk", "Sp. Def", "Speed"];

/// Placeholder shown for the nature (not stored by the port's save).
pub const NATURE_TEXT: &str = "Not recorded yet";
/// Placeholder shown for the ability (the game's battles do not use abilities).
pub const ABILITY_TEXT: &str = "None in this game";

fn title_case(s: &str) -> String {
    s.split([' ', '-'])
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn status_text(m: &PartyMember) -> &'static str {
    use super::save_game::SavedStatus;
    if m.hp == 0 {
        return "Fainted";
    }
    match m.status {
        None => "Healthy",
        Some(SavedStatus::Drowsy) => "Drowsy",
        Some(SavedStatus::Paralysis) => "Paralysis",
        Some(SavedStatus::Poison) => "Poison",
        Some(SavedStatus::Burn) => "Burn",
        Some(SavedStatus::Frostbite) => "Frostbite",
    }
}

/// Derive the summary of `m` from the battle data (`assets` may be missing: names fall back to
/// numbers and stats / experience are unknown).
pub fn summary_info(assets: Option<&BattleAssets>, m: &PartyMember) -> SummaryInfo {
    let species = assets.and_then(|a| {
        a.data
            .species_row_by_dex_form(m.species, m.form)
            .ok()
            .flatten()
    });
    let name = m
        .nickname
        .clone()
        .or_else(|| species.as_ref().map(|s| title_case(&s.name)))
        .unwrap_or_else(|| format!("#{:04}", m.species));
    let mut types = Vec::new();
    let mut stats = None;
    if let Some(s) = &species {
        for t in &s.types {
            let t = title_case(t);
            if !t.is_empty() && !types.contains(&t) {
                types.push(t);
            }
        }
        let c = Combatant::with_effort_nature(s.clone(), m.level, Vec::new(), m.effort, [100; 6]);
        stats = Some([
            c.max_hp,
            c.attack,
            c.defense,
            c.special_attack,
            c.special_defense,
            c.speed,
        ]);
    }
    let (mut exp, mut exp_to_next, mut exp_ratio) = (m.exp, None, 0.0);
    if let Some(t) = assets.and_then(|a| a.progression.as_deref()) {
        let group = t.growth_group(m.species, m.form);
        let floor = exp_for_level(t, group, m.level);
        let total = m.exp.unwrap_or(floor).max(floor);
        exp = Some(total);
        if m.level >= 100 {
            exp_to_next = Some(0);
            exp_ratio = 1.0;
        } else {
            let next = exp_for_level(t, group, m.level + 1);
            exp_to_next = Some(next.saturating_sub(total));
            let span = next.saturating_sub(floor).max(1);
            exp_ratio = (total - floor) as f32 / span as f32;
        }
    }
    let moves = m
        .moves
        .iter()
        .map(|mv| match assets.and_then(|a| a.data.move_row_by_id(mv.move_id).ok().flatten()) {
            Some(row) => SummaryMove {
                name: title_case(&row.name),
                move_type: title_case(&row.move_type),
                category: title_case(&row.category),
                power: row.power,
                accuracy: row.accuracy,
                pp: mv.pp,
                max_pp: Some(row.pp),
            },
            None => SummaryMove {
                name: format!("Move #{}", mv.move_id),
                move_type: String::new(),
                category: String::new(),
                power: 0,
                accuracy: 0,
                pp: mv.pp,
                max_pp: None,
            },
        })
        .collect();
    SummaryInfo {
        dex: m.species,
        form: m.form,
        name,
        level: m.level,
        types,
        hp: m.hp,
        stats,
        effort: m.effort,
        exp,
        exp_to_next,
        exp_ratio: exp_ratio.clamp(0.0, 1.0),
        status: status_text(m),
        moves,
    }
}

/// Text of the notes page (the port's own words).
pub fn memo_text(info: &SummaryInfo) -> String {
    format!(
        "No. {:04}. Status: {}.\nNature: {NATURE_TEXT}.\nAbility: {ABILITY_TEXT}.\nEffort levels: {}.",
        info.dex,
        info.status,
        info.effort
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("/")
    )
}

/// Root of the provisional (debug UI) summary panel.
#[derive(Component, Debug, Clone, Copy)]
pub struct SummaryRoot;

#[derive(Component)]
struct SummaryText;

pub struct SummaryScreenPlugin;

impl Plugin for SummaryScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SummaryState>()
            .init_resource::<SummaryOpenRequested>()
            .init_resource::<SummaryFromParty>()
            .add_systems(Startup, spawn_summary_panel)
            // In PreUpdate after the action mapping (like the pause menu) so Esc is consumed
            // before the party screen, which stays open underneath, reads it in Update.
            .add_systems(
                PreUpdate,
                summary_input
                    .after(bevy::input::InputSystems)
                    .after(super::input_map::fill_action_state),
            )
            .add_systems(Update, (summary_open_requests, sync_summary_panel).chain());
    }
}

/// Consume the open requests (Update: the pause menu raises its request in PreUpdate and the
/// party screen in Update, so the summary opens in the same or the next frame).
pub fn summary_open_requests(
    mut request: ResMut<SummaryOpenRequested>,
    mut from_party: ResMut<SummaryFromParty>,
    mut state: ResMut<SummaryState>,
    progress: Option<Res<PlayerProgress>>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
) {
    let party_len = progress.as_ref().map_or(0, |p| p.party.len());
    if let Some(member) = from_party.0.take() {
        if member < party_len {
            let page = state.page;
            *state = SummaryState {
                open: true,
                member,
                page,
                move_cursor: 0,
                from_party: true,
            };
            freeze_for_screen(&mut paused, &mut suppressed, true);
        }
    }
    if std::mem::take(&mut request.0) && party_len > 0 {
        *state = SummaryState {
            open: true,
            member: 0,
            page: SummaryPage::Info,
            move_cursor: 0,
            from_party: false,
        };
        freeze_for_screen(&mut paused, &mut suppressed, true);
    }
}

/// Navigate and close (PreUpdate, see the plugin).
pub fn summary_input(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut state: ResMut<SummaryState>,
    progress: Option<Res<PlayerProgress>>,
    party: Option<Res<PartyScreenState>>,
    actions: Option<ResMut<ActionState>>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
) {
    let party_len = progress.as_ref().map_or(0, |p| p.party.len());
    if !state.open {
        return;
    }
    if party_len == 0 {
        state.open = false;
        freeze_for_screen(&mut paused, &mut suppressed, false);
        return;
    }
    state.member = state.member.min(party_len - 1);
    if let Some(mut actions) = actions {
        if actions.just_pressed(Action::Cancel) || actions.just_pressed(Action::Menu) {
            actions.consume(Action::Cancel);
            actions.consume(Action::Menu);
            state.open = false;
            let back_to_party = state.from_party && party.is_some_and(|p| p.open);
            if !back_to_party {
                freeze_for_screen(&mut paused, &mut suppressed, false);
            }
            return;
        }
        // The party screen underneath must not also act on this frame's arrows / Enter.
        for a in [Action::Prev, Action::Next, Action::Confirm] {
            actions.consume(a);
        }
    }
    let Some(keys) = keys else {
        return;
    };
    let any = |k: &[KeyCode]| keys.any_just_pressed(k.iter().copied());
    if any(&[KeyCode::ArrowRight, KeyCode::KeyD, KeyCode::KeyE]) {
        state.page = state.page.step(1);
    } else if any(&[KeyCode::ArrowLeft, KeyCode::KeyA, KeyCode::KeyQ]) {
        state.page = state.page.step(-1);
    }
    let down = any(&[KeyCode::ArrowDown, KeyCode::KeyS]);
    let up = any(&[KeyCode::ArrowUp, KeyCode::KeyW]);
    let moves = progress
        .as_ref()
        .and_then(|p| p.party.get(state.member))
        .map_or(0, |m| m.moves.len());
    // On the Moves page Up / Down pick the move whose details show; PageUp / PageDown always
    // change the member.
    if state.page == SummaryPage::Moves && moves > 0 && (up || down) {
        state.move_cursor = if down {
            (state.move_cursor + 1) % moves
        } else {
            (state.move_cursor + moves - 1) % moves
        };
        return;
    }
    let next = down || any(&[KeyCode::PageDown]);
    let prev = up || any(&[KeyCode::PageUp]);
    if next {
        state.member = (state.member + 1) % party_len;
        state.move_cursor = 0;
    } else if prev {
        state.member = (state.member + party_len - 1) % party_len;
        state.move_cursor = 0;
    }
}

fn spawn_summary_panel(mut commands: Commands) {
    commands
        .spawn((
            SummaryRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(135),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Px(760.0),
                    max_height: Val::Percent(90.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.13, 0.98)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    SummaryText,
                    Text::new(""),
                    TextFont::from_font_size(17.0),
                    TextColor(Color::srgb(0.88, 0.92, 0.97)),
                ));
            });
        });
}

/// Plain-text body of the provisional panel for `page`.
pub fn panel_text(info: &SummaryInfo, page: SummaryPage, member: usize, party_len: usize) -> String {
    let mut s = format!(
        "SUMMARY  {} / {}   [{}]\n{}   Lv. {}   {}\n\n",
        member + 1,
        party_len,
        page.title(),
        info.name,
        info.level,
        info.types.join(" / ")
    );
    match page {
        SummaryPage::Info => {
            s += &format!("No. {:04}\nStatus: {}\n", info.dex, info.status);
            match (info.exp, info.exp_to_next) {
                (Some(e), Some(n)) => s += &format!("Exp. points: {e}\nTo next level: {n}\n"),
                _ => s += "Exp. points: unknown\n",
            }
            s += &format!("Nature: {NATURE_TEXT}\nAbility: {ABILITY_TEXT}\n");
        }
        SummaryPage::Moves => {
            for m in &info.moves {
                let pp = m.max_pp.map_or(m.pp.to_string(), |x| format!("{}/{x}", m.pp));
                s += &format!(
                    "{}  ({} {})  power {}  accuracy {}  PP {pp}\n",
                    m.name, m.move_type, m.category, m.power, m.accuracy
                );
            }
        }
        SummaryPage::Stats => match info.stats {
            Some(st) => {
                for (k, v) in st.iter().enumerate() {
                    let shown = if k == 0 {
                        format!("{}/{v}", info.hp)
                    } else {
                        v.to_string()
                    };
                    s += &format!("{}: {shown}\n", STAT_NAMES[k]);
                }
            }
            None => s += "Stats unknown (species missing from the battle data)\n",
        },
        SummaryPage::Memo => s += &memo_text(info),
    }
    s += "\nLeft / Right: page   Up / Down: member (Moves page: move)   PgUp / PgDn: member   Esc: back";
    s
}

fn sync_summary_panel(
    state: Res<SummaryState>,
    progress: Option<Res<PlayerProgress>>,
    assets: Option<Res<BattleAssets>>,
    mut root: Query<&mut Visibility, With<SummaryRoot>>,
    mut text: Query<&mut Text, With<SummaryText>>,
) {
    let member = progress
        .as_ref()
        .and_then(|p| p.party.get(state.member))
        .filter(|_| state.open);
    for mut v in &mut root {
        v.set_if_neq(if member.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }
    let Some(m) = member else {
        return;
    };
    let info = summary_info(assets.as_deref(), m);
    let len = progress.as_ref().map_or(0, |p| p.party.len());
    let body = panel_text(&info, state.page, state.member, len);
    for mut t in &mut text {
        if t.0 != body {
            t.0 = body.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::save_game::SavedMove;

    #[test]
    fn pages_wrap_both_ways() {
        assert_eq!(SummaryPage::Info.step(1), SummaryPage::Moves);
        assert_eq!(SummaryPage::Info.step(-1), SummaryPage::Memo);
        assert_eq!(SummaryPage::Memo.step(1), SummaryPage::Info);
    }

    #[test]
    fn info_without_battle_data_falls_back_to_numbers() {
        let m = PartyMember {
            species: 25,
            level: 12,
            hp: 30,
            moves: vec![SavedMove { move_id: 84, pp: 20 }],
            ..Default::default()
        };
        let info = summary_info(None, &m);
        assert_eq!(info.name, "#0025");
        assert_eq!(info.stats, None);
        assert_eq!(info.moves[0].name, "Move #84");
        assert_eq!(info.status, "Healthy");
        let text = panel_text(&info, SummaryPage::Memo, 0, 1);
        assert!(text.contains(NATURE_TEXT) && text.contains(ABILITY_TEXT));
    }

    fn app_with_party(n: usize) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<ActionState>();
        app.init_resource::<Paused>();
        app.init_resource::<PauseMenuSuppressed>();
        app.init_resource::<PartyScreenState>();
        let mut progress = PlayerProgress::new_game();
        progress.party = (0..n)
            .map(|i| PartyMember {
                species: 25 + i as u16,
                level: 5,
                hp: 10,
                ..Default::default()
            })
            .collect();
        app.insert_resource(progress);
        app.add_plugins(SummaryScreenPlugin);
        app
    }

    #[test]
    fn opens_from_the_pause_request_and_cycles_members_and_pages() {
        let mut app = app_with_party(3);
        app.world_mut().resource_mut::<SummaryOpenRequested>().0 = true;
        app.update();
        let st = app.world().resource::<SummaryState>().clone();
        assert!(st.open && st.member == 0 && !st.from_party);
        assert!(app.world().resource::<Paused>().0, "world frozen");
        let press = |app: &mut App, k: KeyCode| {
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(k);
            app.update();
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release(k);
            keys.clear();
        };
        press(&mut app, KeyCode::ArrowUp);
        assert_eq!(app.world().resource::<SummaryState>().member, 2, "wraps upwards");
        press(&mut app, KeyCode::ArrowRight);
        assert_eq!(app.world().resource::<SummaryState>().page, SummaryPage::Moves);
        // The provisional panel shows while open.
        let mut q = app.world_mut().query_filtered::<&Visibility, With<SummaryRoot>>();
        assert_eq!(q.single(app.world()).ok(), Some(&Visibility::Visible));
    }

    #[test]
    fn party_request_opens_that_member_and_empty_party_never_opens() {
        let mut app = app_with_party(2);
        app.world_mut().resource_mut::<SummaryFromParty>().0 = Some(1);
        app.update();
        let st = app.world().resource::<SummaryState>().clone();
        assert!(st.open && st.member == 1 && st.from_party);

        let mut empty = app_with_party(0);
        empty.world_mut().resource_mut::<SummaryOpenRequested>().0 = true;
        empty.update();
        assert!(!empty.world().resource::<SummaryState>().open);
        assert!(!empty.world().resource::<Paused>().0);
    }
}
