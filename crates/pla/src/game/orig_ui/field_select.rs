//! Field item / Pokémon selector (the game's `pokeget_item_change_00`, loaded from
//! `pokeget.json`): the wheel the original shows at the bottom right of the field screen with
//! the current item — or the party's Pokémon — in the middle, its neighbours at the sides, `L` /
//! `R` to change, the name bar below and the `X` / change badges above.
//!
//! Pane map (VERIFIED against `work/ui/layouts/pokeget.json`, screen `pokeget`, layout
//! `pokeget_item_change_00`):
//!
//! | Part of the layout | Port binding |
//! |---|---|
//! | `N_icon_item_00` / `P_icon_current_00` | current item icon (centre, 1.18×), `x` count below |
//! | `N_icon_item_01` (+ `P_icon_left_00`) | previous item (left, 0.76×) |
//! | `N_icon_item_04` (+ `P_icon_right_02`) | next item (right, 0.76×) |
//! | `N_icon_poke_00…04` (+ `P_icon_poke_00…04`) | the party Pokémon of the same five slots |
//! | `T_item_name_00` | name of the current item / Pokémon |
//! | `T_item_num_00`, `P_icon_infinity_00` | bag count (the ∞ icon is never shown: counts are known) |
//! | `T_poke_lv_00` | `Lv. N` of the current Pokémon |
//! | `L_icon_key_l_00` / `L_icon_key_r_00` | the `L` / `R` shoulder badges (the layout holds the `L` texture in both; the port swaps the right one) |
//! | `L_btn_zr_00` | the `X` badge and the change icon above the centre slot |
//!
//! The layout keeps one panel visible at a time through a run-time state animation: freezing
//! `switch_item_state` / `switch_poke_state` at frame 0 (as the game does when the selection
//! settles) shows the item / Pokémon panel.
//!
//! Hidden, no port data: the sick banners (`N_icon_sick_00…04`: the game's own icon art carries
//! Japanese status text, which the port never draws), the max-count box (`L_boxmax_00`: its
//! `_keep` loop blinks it and its text is the game's own) and the gauge (`L_gauge_00`, only
//! filled for a caught Pokémon in the original).
//!
//! State: [`FieldSelector`], plain memory (nothing is written to the save; the v10 format and
//! the party are untouched). Keys (also in `docs/GUIA-DE-PRUEBAS.md`): `X` opens the wheel and
//! then switches item ↔ Pokémon panel, `[` / `]` step to the previous / next entry, `Backspace`
//! closes. The ball letter shortcuts and the bait key of the viewer keep working: the item list
//! derives from `BallInventory` (the nine ball lines, then the bait items the bag holds), so the
//! wheel and the shortcuts always agree, and the item it shows is the one a throw uses.

use bevy::prelude::*;

use super::field::NameCache;
use super::icons::IconIndex;
use super::scene::{ExternalImage, TextAlign};
use super::{free_text, OrigScreen, OrigUiConfig, OrigUiSystems, ScreenId};
use crate::game::bag_screen::BagItemTable;
use crate::game::battle_controller::BattleAssets;
use crate::game::capture_world::BaitTable;
use crate::game::save_wire::PlayerProgress;
use crate::game::state::{GameState, Paused};
use crate::game::throwing::{BallInventory, BallKind, ALL_BALL_KINDS};

/// Whether the viewer's camera is in the over-the-shoulder aim mode. The camera controller
/// lives in the viewer example, so the viewer mirrors its mode here: while aiming `X` throws the
/// selected bait (the viewer's own binding) and must not open the wheel; on foot it opens it.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct AimOverShoulder(pub bool);

/// Which of the two panels the wheel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelKind {
    #[default]
    Items,
    Pokemon,
}

impl PanelKind {
    fn other(self) -> Self {
        match self {
            Self::Items => Self::Pokemon,
            Self::Pokemon => Self::Items,
        }
    }
}

/// Selector state. Plain memory: the wheel is transient in the game too and nothing about it is
/// saved (`PlayerSave` stays v10), so the chosen field partner is forgotten between sessions.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct FieldSelector {
    pub open: bool,
    pub panel: PanelKind,
    /// Index inside `PlayerProgress::party` of the chosen field partner: the Pokémon that walks
    /// with the player (`game::follower`). Clamped to the party length when it shrinks.
    pub pokemon_index: usize,
}

impl Default for FieldSelector {
    fn default() -> Self {
        Self {
            open: false,
            panel: PanelKind::Items,
            pokemon_index: 0,
        }
    }
}

/// One entry of the item wheel: a ball line, or a bait item the bag holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldItem {
    Ball(BallKind),
    Bait(u16),
}

/// The item list: every ball line first (the same order as the ball shortcuts), then the baits
/// the bag holds (the same list the bait key cycles).
pub fn item_entries(baits: &[u16]) -> Vec<FieldItem> {
    ALL_BALL_KINDS
        .iter()
        .map(|k| FieldItem::Ball(*k))
        .chain(baits.iter().map(|id| FieldItem::Bait(*id)))
        .collect()
}

/// Wrap `index` one step forward / backward over a list of `len` entries; `None` when empty.
pub fn step(index: usize, len: usize, forward: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let i = index.min(len - 1);
    Some(if forward {
        (i + 1) % len
    } else {
        (i + len - 1) % len
    })
}

/// Index of the entry the throw inventory holds: a bait when one is selected (the bait throw
/// takes priority while aiming), else the selected ball. `None` only for an empty list.
pub fn current_entry(entries: &[FieldItem], inventory: &BallInventory) -> Option<usize> {
    if let Some(id) = inventory.selected_bait {
        if let Some(i) = entries.iter().position(|e| *e == FieldItem::Bait(id)) {
            return Some(i);
        }
    }
    entries
        .iter()
        .position(|e| *e == FieldItem::Ball(inventory.selected))
}

/// Make `entries[i]` the item the field throws (`false` when `i` is out of range).
pub fn apply_entry(entries: &[FieldItem], i: usize, inventory: &mut BallInventory) -> bool {
    match entries.get(i) {
        Some(FieldItem::Ball(kind)) => {
            inventory.selected = *kind;
            inventory.selected_bait = None;
            true
        }
        Some(FieldItem::Bait(id)) => {
            inventory.selected_bait = Some(*id);
            true
        }
        None => false,
    }
}

/// Item id an entry shows (icon and count come from here).
pub fn item_id(entry: FieldItem) -> u16 {
    match entry {
        FieldItem::Ball(kind) => kind.item_id(),
        FieldItem::Bait(id) => id,
    }
}

/// Slot offsets, slots and picture panes of the wheel. Only −1 / 0 / +1 are drawn, as the
/// layout's own resting animation `shift_refresh` has it: the outer two (`N_icon_item_02` /
/// `N_icon_item_03` and their Pokémon pair) start transparent and stay hidden.
const ITEM_SLOTS: [(i32, &str, &str); 3] = [
    (-1, "N_icon_item_01", "P_icon_left_00"),
    (0, "N_icon_item_00", "P_icon_current_00"),
    (1, "N_icon_item_04", "P_icon_right_02"),
];
const POKE_SLOTS: [(i32, &str, &str); 3] = [
    (-1, "N_icon_poke_01", "P_icon_poke_01"),
    (0, "N_icon_poke_00", "P_icon_poke_00"),
    (1, "N_icon_poke_04", "P_icon_poke_04"),
];
/// Slots the wheel never draws (the two extra ring positions of the original).
const OUTER_SLOTS: [&str; 4] = [
    "N_icon_item_02",
    "N_icon_item_03",
    "N_icon_poke_02",
    "N_icon_poke_03",
];
/// Sick banners of the party slots (hidden: the game's icons carry status text the port does
/// not draw).
const SICK_PARTS: [&str; 5] = [
    "N_icon_sick_00",
    "N_icon_sick_01",
    "N_icon_sick_02",
    "N_icon_sick_03",
    "N_icon_sick_04",
];
/// Texture name (in the screen's own archive) of the right shoulder badge: the layout holds the
/// `L` art in both badge parts and the game swaps the right one at run time.
const R_BADGE_TEXTURE: &str = "button_icon_R_round_00^t";

/// The run-time state animations that show one panel at a time (frame 0 = settled).
const STATE_ITEM: &str = "switch_item_state";
const STATE_POKE: &str = "switch_poke_state";

pub fn install(app: &mut App) {
    app.init_resource::<FieldSelector>();
    app.init_resource::<AimOverShoulder>();
    app.add_systems(
        Update,
        (field_select_input, bind_field_select)
            .chain()
            .in_set(OrigUiSystems::Bind),
    );
}

/// Keys of the wheel (field screen only; the ball / bait shortcuts are untouched).
#[allow(clippy::too_many_arguments)]
pub fn field_select_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut selector: ResMut<FieldSelector>,
    state: Res<State<GameState>>,
    paused: Option<Res<Paused>>,
    aim: Option<Res<AimOverShoulder>>,
    progress: Option<Res<PlayerProgress>>,
    balls: Option<ResMut<BallInventory>>,
    baits: Option<Res<BaitTable>>,
) {
    if *state.get() != GameState::World || paused.as_ref().is_some_and(|p| p.0) {
        return;
    }
    // `X` is the game's own key for the wheel (its badge sits above the centre slot). While
    // aiming it throws the selected bait instead (the viewer's binding): the wheel is a key of
    // the on-foot screen.
    let aiming = aim.as_ref().is_some_and(|a| a.0);
    if keys.just_pressed(KeyCode::KeyX) && !aiming {
        if selector.open {
            selector.panel = selector.panel.other();
            info!("field selector: {:?} panel", selector.panel);
        } else {
            selector.open = true;
            selector.panel = PanelKind::Items;
            info!("field selector: open (items); [ ] change, X switches, Backspace closes");
        }
    }
    if keys.just_pressed(KeyCode::Backspace) && selector.open {
        selector.open = false;
        info!("field selector: closed");
    }
    if !selector.open {
        return;
    }
    let forward = keys.just_pressed(KeyCode::BracketRight);
    let backward = keys.just_pressed(KeyCode::BracketLeft);
    if !forward && !backward {
        return;
    }
    match selector.panel {
        PanelKind::Items => {
            let Some(mut inventory) = balls else {
                return;
            };
            let bait_ids = baits
                .as_ref()
                .zip(progress.as_ref())
                .map(|(table, p)| table.present_in(&p.bag))
                .unwrap_or_default();
            let entries = item_entries(&bait_ids);
            let current = current_entry(&entries, &inventory).unwrap_or(0);
            if let Some(next) = step(current, entries.len(), forward) {
                apply_entry(&entries, next, &mut inventory);
                info!(
                    "field selector: item {} of {} ({})",
                    next + 1,
                    entries.len(),
                    item_id(entries[next])
                );
            }
        }
        PanelKind::Pokemon => {
            let len = progress.as_ref().map_or(0, |p| p.party.len());
            if let Some(next) = step(selector.pokemon_index, len, forward) {
                selector.pokemon_index = next;
                info!("field selector: party member {} of {}", next + 1, len);
            }
        }
    }
}

/// Fills the wheel when it is open (and hides the resting ball shortcut of the field HUD).
#[allow(clippy::too_many_arguments)]
pub fn bind_field_select(
    config: Res<OrigUiConfig>,
    state: Res<State<GameState>>,
    paused: Option<Res<Paused>>,
    mut selector: ResMut<FieldSelector>,
    progress: Option<Res<PlayerProgress>>,
    balls: Option<Res<BallInventory>>,
    baits: Option<Res<BaitTable>>,
    items: Option<Res<BagItemTable>>,
    assets: Option<Res<BattleAssets>>,
    mut names: Local<NameCache>,
    icons: Res<IconIndex>,
    mut screens: Query<&mut OrigScreen>,
) {
    if !config.enabled() {
        return;
    }
    let Some(mut s) = screens
        .iter_mut()
        .find(|s| s.id == ScreenId::FieldSelect)
    else {
        return;
    };
    let show = *state.get() == GameState::World
        && !paused.as_ref().is_some_and(|p| p.0)
        && selector.open;
    s.shown = show;
    if !show {
        return;
    }
    // One panel at a time: the game's own settled states (`switch_*_state`, frame 0).
    match selector.panel {
        PanelKind::Items => s.freeze(0, STATE_ITEM, 0.0, "panel_state"),
        PanelKind::Pokemon => s.freeze(0, STATE_POKE, 0.0, "panel_state"),
    }
    // The outer slots and the sick banners are never drawn; the ∞ count icon is only for
    // unknown counts.
    for node in OUTER_SLOTS {
        s.set_visible(node, false);
    }
    for part in SICK_PARTS {
        s.set(part, |o| o.visible = Some(false));
    }
    s.set_visible("P_icon_infinity_00", false);
    // The "registered" gauge under the name has no port data: hidden, never drawn half filled.
    s.set_visible("N_gauge_00", false);
    // The max-count box blinks in its `_keep` loop and holds the game's own text, which the port
    // never fills: an empty blinking rectangle is worse than no box.
    s.set_visible("L_boxmax_00", false);
    show_r_badge(&mut s);

    let party: &[crate::game::save_game::PartyMember] = progress
        .as_ref()
        .map(|p| p.party.as_slice())
        .unwrap_or(&[]);
    if !party.is_empty() && selector.pokemon_index >= party.len() {
        selector.pokemon_index = party.len() - 1;
    }
    let bait_ids = baits
        .as_ref()
        .zip(progress.as_ref())
        .map(|(table, p)| table.present_in(&p.bag))
        .unwrap_or_default();
    let entries = item_entries(&bait_ids);
    let inventory = balls.as_deref();

    match selector.panel {
        PanelKind::Items => {
            let current = inventory.and_then(|inv| current_entry(&entries, inv));
            let count = current.and_then(|i| {
                let id = item_id(entries[i]);
                progress
                    .as_ref()
                    .map(|p| p.bag.get(&id).copied().unwrap_or(0))
            });
            bind_item_slots(&mut s, &entries, current, &icons);
            let name = current.map(|i| match entries[i] {
                FieldItem::Ball(kind) => kind.display_name().to_string(),
                FieldItem::Bait(id) => super::menus::short_label(&super::menus::item_label(
                    items.as_ref().and_then(|t| t.0.as_ref()),
                    id,
                ))
                .to_string(),
            });
            s.set_text("T_item_name_00", name.unwrap_or_default());
            s.set_text("T_item_num_00", count.map_or(String::new(), |c| c.to_string()));
            s.set_text("T_poke_lv_00", String::new());
        }
        PanelKind::Pokemon => {
            let current = if party.is_empty() {
                None
            } else {
                Some(selector.pokemon_index.min(party.len() - 1))
            };
            bind_poke_slots(&mut s, party, current, &icons);
            let label = current.map(|i| {
                names.get(assets.as_deref(), party[i].species, party[i].form)
            });
            let level = current.map(|i| format!("Lv. {}", party[i].level));
            s.set_text("T_item_name_00", label.unwrap_or_default());
            s.set_text("T_poke_lv_00", level.unwrap_or_default());
            s.set_text("T_item_num_00", String::new());
        }
    }
    let hint = match selector.panel {
        PanelKind::Items => {
            "X: Pokémon panel   ·   [ ]: previous / next item   ·   Backspace: close"
        }
        PanelKind::Pokemon => {
            "X: item panel   ·   [ ]: previous / next Pokémon   ·   Backspace: close"
        }
    };
    s.set_free_texts(vec![free_text(
        1060.0,
        772.0,
        820.0,
        38.0,
        24.0,
        [0.94, 0.94, 0.9, 0.85],
        TextAlign::Right,
        hint,
    )]);
}

/// Shows the centre slot and its two neighbours of the item list (wrapping), hiding the rest.
fn bind_item_slots(
    s: &mut OrigScreen,
    entries: &[FieldItem],
    current: Option<usize>,
    icons: &IconIndex,
) {
    for (offset, node, pane) in ITEM_SLOTS {
        let entry = neighbour(current, entries.len(), offset);
        let image = entry
            .map(|i| item_id(entries[i]))
            .and_then(|id| icons.item_or_dummy(id));
        s.set(node, |o| o.visible = Some(entry.is_some()));
        let shown = image.is_some();
        s.set(pane, |o| o.visible = Some(shown));
        if let Some(image) = image {
            s.set(pane, move |o| o.image = Some(image));
        }
    }
}

/// Shows the centre party slot and its neighbours of the Pokémon panel (wrapping).
fn bind_poke_slots(
    s: &mut OrigScreen,
    party: &[crate::game::save_game::PartyMember],
    current: Option<usize>,
    icons: &IconIndex,
) {
    for (offset, node, pane) in POKE_SLOTS {
        let member = neighbour(current, party.len(), offset).map(|i| &party[i]);
        let image = member.and_then(|m| icons.pokemon(m.species, u16::from(m.form)));
        s.set(node, |o| o.visible = Some(member.is_some()));
        let shown = image.is_some();
        s.set(pane, |o| o.visible = Some(shown));
        if let Some(image) = image {
            s.set(pane, move |o| o.image = Some(image));
        }
    }
}

/// Entry at `offset` steps from `current` (wrapping); `None` without a current entry.
fn neighbour(current: Option<usize>, len: usize, offset: i32) -> Option<usize> {
    let current = current?;
    if len == 0 {
        return None;
    }
    Some(((current as i32 + offset).rem_euclid(len as i32)) as usize)
}

/// The right shoulder badge: the layout holds the left one twice, the game swaps the texture.
fn show_r_badge(s: &mut OrigScreen) {
    let Some(png) = s
        .scene
        .archive()
        .textures
        .get(R_BADGE_TEXTURE)
        .and_then(|t| t.png.clone())
    else {
        return;
    };
    let image = ExternalImage {
        png,
        uv: [0.0, 0.0, 1.0, 1.0],
    };
    s.set_in("L_icon_key_r_00", "P_icon_key_00", |o| o.image = Some(image));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inventory() -> BallInventory {
        BallInventory::default()
    }

    #[test]
    fn step_wraps_both_ways_and_handles_empty_and_single_lists() {
        assert_eq!(step(0, 3, true), Some(1));
        assert_eq!(step(2, 3, true), Some(0), "forward wraps");
        assert_eq!(step(0, 3, false), Some(2), "backward wraps");
        assert_eq!(step(0, 1, true), Some(0), "one element stays");
        assert_eq!(step(0, 1, false), Some(0));
        assert_eq!(step(0, 0, true), None, "empty list has no step");
        assert_eq!(step(9, 0, false), None);
        assert_eq!(step(7, 3, true), Some(0), "out-of-range index clamps first");
        assert_eq!(step(7, 3, false), Some(1));
    }

    #[test]
    fn item_list_is_the_nine_balls_then_the_bag_baits() {
        let entries = item_entries(&[149, 164]);
        assert_eq!(entries.len(), 11);
        assert_eq!(entries[0], FieldItem::Ball(BallKind::PokeBall));
        assert_eq!(entries[8], FieldItem::Ball(BallKind::GigatonBall));
        assert_eq!(entries[9], FieldItem::Bait(149));
        assert_eq!(entries[10], FieldItem::Bait(164));
        assert_eq!(item_entries(&[]).len(), 9);
    }

    #[test]
    fn current_entry_follows_the_selected_ball_and_the_argument_wins() {
        let entries = item_entries(&[149]);
        let mut inv = inventory();
        assert_eq!(current_entry(&entries, &inv), Some(0), "Poké Ball by default");
        inv.selected = BallKind::UltraBall;
        assert_eq!(current_entry(&entries, &inv), Some(2));
        // The bait throw takes priority while aiming, so the bait is the current entry.
        inv.selected_bait = Some(149);
        assert_eq!(current_entry(&entries, &inv), Some(9));
        // A bait that left the bag falls back to the ball.
        inv.selected_bait = Some(999);
        assert_eq!(current_entry(&entries, &inv), Some(2));
    }

    #[test]
    fn apply_entry_selects_balls_and_baits() {
        let entries = item_entries(&[149]);
        let mut inv = inventory();
        assert!(apply_entry(&entries, 1, &mut inv));
        assert_eq!(inv.selected, BallKind::GreatBall);
        assert_eq!(inv.selected_bait, None);
        assert!(apply_entry(&entries, 9, &mut inv));
        assert_eq!(inv.selected_bait, Some(149));
        assert_eq!(
            inv.selected,
            BallKind::GreatBall,
            "the ball line is kept while a bait is held"
        );
        // Back to a ball clears the bait.
        assert!(apply_entry(&entries, 0, &mut inv));
        assert_eq!(inv.selected, BallKind::PokeBall);
        assert_eq!(inv.selected_bait, None);
        assert!(!apply_entry(&entries, 99, &mut inv));
    }

    #[test]
    fn full_cycle_visits_every_entry_once() {
        let entries = item_entries(&[149, 164]);
        let mut inv = inventory();
        let mut idx = current_entry(&entries, &inv).unwrap();
        let mut seen = vec![idx];
        for _ in 1..entries.len() {
            idx = step(idx, entries.len(), true).unwrap();
            apply_entry(&entries, idx, &mut inv);
            seen.push(current_entry(&entries, &inv).unwrap());
        }
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), entries.len(), "every entry reached once");
        assert_eq!(step(idx, entries.len(), true), Some(0), "and wraps home");
    }

    #[test]
    fn neighbour_wraps_and_needs_a_current_entry() {
        assert_eq!(neighbour(Some(0), 3, -1), Some(2));
        assert_eq!(neighbour(Some(2), 3, 1), Some(0));
        assert_eq!(neighbour(Some(1), 1, 2), Some(0), "one element wraps to it");
        assert_eq!(neighbour(None, 3, 1), None);
        assert_eq!(neighbour(Some(0), 0, 1), None);
    }

    #[test]
    fn panel_kind_alternates() {
        assert_eq!(PanelKind::Items.other(), PanelKind::Pokemon);
        assert_eq!(PanelKind::Pokemon.other().other(), PanelKind::Pokemon);
        assert_eq!(FieldSelector::default().panel, PanelKind::Items);
        assert!(!FieldSelector::default().open);
    }
}
