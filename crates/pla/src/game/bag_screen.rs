//! Bag and shop screens (Bevy UI) plus item use in the field.
//!
//! No game texts: every line shows the decoded item id and the effect derived from
//! `data/item_effects.tsv` (`battle_items::label`), grouped by the LIKELY `pocket_code`.
//! Medicine is used out of battle through the battle engine itself
//! (`Battle::use_item_on`) on a throwaway battle that contains the party plus one dummy
//! opponent, so field and battle share the same effect rules.
//!
//! Opening: `I` (or the pause menu's `Bag` row). With `--debug-shop` a shop trigger is
//! spawned at the player's start position: stand on it and press `E` to trade. The pause menu's
//! `Shop (test shop)` row ([`ShopOpenRequested`]) opens the same shop 0 without the trigger; no
//! real merchant positions are decoded.
//!
//! Limits (declared): the merchant identity behind each decoded shop hash is UNKNOWN, the
//! sell price is ASSUMED half, the world is frozen by the shared `Paused` resource while a
//! screen is open, and closing the bag does not return to the pause menu.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use crate::battle_engine::items::{ItemEffect, ItemTable};
use crate::battle_engine::{Battle, MajorStatus};
use crate::game::battle_controller::BattleAssets;
use crate::game::battle_items::{label, status_tag};
use crate::game::pause_menu::{BagOpenRequested, PauseMenuSuppressed, ShopOpenRequested};
use crate::game::save_game::PartyMember;
use crate::game::save_wire::{member_to_combatant, PlayerProgress};
use crate::game::shop::Shop;
use crate::game::state::Paused;
use crate::game::{GameState, PlayerAvatar};

/// Rows drawn at once; the window follows the selection.
pub const VISIBLE_ROWS: usize = 12;
/// Quantity step ceiling for buy/sell.
pub const MAX_TRADE_QUANTITY: u16 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagMode {
    /// The player's item list, grouped by pocket.
    Bag,
    /// A decoded shop's stock.
    Shop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopTab {
    Buy,
    Sell,
}

/// Screen state. All fields are plain data so the pure helpers below can be unit tested.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct BagUi {
    pub open: bool,
    pub mode: BagMode,
    /// Index into [`ShopCatalog`].
    pub shop: usize,
    pub tab: ShopTab,
    /// Index into the pockets with at least one item.
    pub pocket: usize,
    pub selected: usize,
    pub quantity: u16,
    pub message: String,
}

impl Default for BagUi {
    fn default() -> Self {
        Self {
            open: false,
            mode: BagMode::Bag,
            shop: 0,
            tab: ShopTab::Buy,
            pocket: 0,
            selected: 0,
            quantity: 1,
            message: String::new(),
        }
    }
}

/// Pocket codes that hold at least one item with a positive count, ascending.
pub fn pocket_codes(table: &ItemTable, bag: &BTreeMap<u16, u16>) -> Vec<u8> {
    let mut set: std::collections::BTreeSet<u8> = bag
        .iter()
        .filter(|(_, n)| **n > 0)
        .filter_map(|(id, _)| table.get(*id))
        .map(|i| i.pocket_code)
        .collect();
    set.insert(0); // The misc pocket is always shown, even when empty.
    set.into_iter().collect()
}

/// `(item_id, count)` of one pocket, ascending by id, with positive counts only.
pub fn pocket_entries(table: &ItemTable, bag: &BTreeMap<u16, u16>, pocket: u8) -> Vec<(u16, u16)> {
    bag.iter()
        .filter(|(_, n)| **n > 0)
        .filter_map(|(id, n)| {
            table
                .get(*id)
                .filter(|i| i.pocket_code == pocket)
                .map(|_| (*id, *n))
        })
        .collect()
}

/// One list line: effect text plus the count.
pub fn line_for(item: &ItemEffect, count: u16) -> String {
    format!("{}  x{count}", label(item))
}

/// Detail line of the selected item (id, effect, price, stack, pocket).
pub fn detail_for(item: &ItemEffect, count: u16) -> String {
    let mut s = line_for(item, count);
    s.push_str(&format!(" | id #{}", item.item_id));
    if item.price == 0 {
        s.push_str(" | price 0 (not sold)");
    } else {
        s.push_str(&format!(
            " | price {} | sell {}",
            item.price,
            Shop::sell_unit_price(item)
        ));
    }
    s.push_str(&format!(
        " | stack {} | pocket {}",
        item.max_quantity, item.pocket_code
    ));
    s
}

/// Rows the screen shows for the current selection (mode decides what a "list" is).
pub fn screen_entries(
    ui: &BagUi,
    table: &ItemTable,
    bag: &BTreeMap<u16, u16>,
    shops: &[Shop],
) -> Vec<(u16, u16)> {
    match ui.mode {
        BagMode::Bag => {
            let pockets = pocket_codes(table, bag);
            match pockets.get(ui.pocket) {
                Some(&p) => pocket_entries(table, bag, p),
                None => Vec::new(),
            }
        }
        BagMode::Shop => match shops.get(ui.shop) {
            None => Vec::new(),
            Some(shop) => match ui.tab {
                ShopTab::Buy => shop
                    .stock
                    .iter()
                    .map(|id| (*id, bag.get(id).copied().unwrap_or(0)))
                    .collect(),
                ShopTab::Sell => bag
                    .iter()
                    .filter(|(id, n)| **n > 0 && table.get(**id).is_some_and(|i| i.price > 0))
                    .map(|(id, n)| (*id, *n))
                    .collect(),
            },
        },
    }
}

/// Wrapping selection move (empty lists stay at 0).
pub fn move_selection(selected: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    ((selected as i32 + delta).rem_euclid(len as i32)) as usize
}

/// What one successful field use did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldUse {
    pub item_id: u16,
    pub target: usize,
    pub species: u16,
    pub level: u8,
    pub hp_restored: u32,
    pub status_cured: Option<MajorStatus>,
    pub revived: bool,
    pub pp_restored: u32,
}

/// Human message built from the decoded effects (no game texts).
pub fn field_use_message(u: &FieldUse) -> String {
    let mut s = format!("Used #{} on #{} (Lv{})", u.item_id, u.species, u.level);
    if u.revived {
        s.push_str(": revived");
    }
    if u.hp_restored > 0 {
        s.push_str(&format!(": +{} HP", u.hp_restored));
    }
    if let Some(st) = u.status_cured {
        s.push_str(&format!(": cured {}", status_tag(st)));
    }
    if u.pp_restored > 0 {
        s.push_str(&format!(": +{} PP", u.pp_restored));
    }
    s
}

/// Use `item_id` on one party member out of battle. The rules are the engine's
/// (`Battle::use_item_on`), applied on a throwaway battle whose first side is the whole
/// party and whose second side is one dummy clone. Refused uses do not consume the item.
pub fn field_use_medicine(
    progress: &mut PlayerProgress,
    assets: &BattleAssets,
    item_id: u16,
    party_index: usize,
    move_slot: Option<usize>,
) -> Result<FieldUse, String> {
    let item = assets
        .items
        .get(item_id)
        .ok_or_else(|| format!("unknown item #{item_id}"))?;
    if item.effect_kind != 1 {
        return Err(format!("#{item_id} is not a medicine"));
    }
    if progress.bag.get(&item_id).copied().unwrap_or(0) == 0 {
        return Err(format!("no #{item_id} left"));
    }
    let mut combatants = Vec::new();
    let mut party_to_combatant: Vec<(usize, usize)> = Vec::new();
    for (pi, m) in progress.party.iter().enumerate() {
        if let Some(c) = member_to_combatant(&assets.data, m) {
            party_to_combatant.push((pi, combatants.len()));
            combatants.push(c);
        }
    }
    let target_ci = party_to_combatant
        .iter()
        .find(|(pi, _)| *pi == party_index)
        .map(|(_, ci)| *ci)
        .ok_or_else(|| format!("party member {party_index} cannot be rebuilt from the data"))?;
    if !combatants.iter().any(|c| c.is_alive()) {
        // The game never leaves the world with a fully fainted party (a blackout heals it).
        return Err("the whole party has fainted".into());
    }
    // Second side: one dummy so the engine constructor accepts the battle. It never acts.
    let dummy_index = combatants.len();
    let mut dummy = combatants[0].clone();
    dummy.hp = dummy.max_hp;
    dummy.caught = false;
    dummy.status = None;
    combatants.push(dummy);
    let side0: Vec<usize> = party_to_combatant.iter().map(|(_, ci)| *ci).collect();
    let mut battle = Battle::new(
        combatants,
        vec![side0, vec![dummy_index]],
        assets.data.type_chart().clone(),
    )?;
    battle.set_item_table(assets.items.clone());
    let used = battle.use_item_on(0, item_id, target_ci, move_slot)?;
    for (pi, ci) in &party_to_combatant {
        let now = PartyMember::from_combatant(&battle.combatants[*ci]);
        if let Some(slot) = progress.party.get_mut(*pi) {
            let (form, nickname) = (slot.form, slot.nickname.clone());
            *slot = PartyMember {
                form,
                nickname,
                ..now
            };
        }
    }
    let left = progress.bag.get(&item_id).copied().unwrap_or(0) - 1;
    if left == 0 {
        progress.bag.remove(&item_id);
    } else {
        progress.bag.insert(item_id, left);
    }
    Ok(FieldUse {
        item_id,
        target: party_index,
        species: progress.party[party_index].species,
        level: progress.party[party_index].level,
        hp_restored: used.hp_restored,
        status_cured: used.status_cured,
        revived: used.revived,
        pp_restored: used.pp_restored,
    })
}

/// The first move slot of a party member with missing PP, for single-move PP items.
fn first_pp_slot(
    progress: &PlayerProgress,
    assets: &BattleAssets,
    party_index: usize,
) -> Option<usize> {
    let m = progress.party.get(party_index)?;
    m.moves.iter().position(|sm| {
        assets
            .data
            .move_row_by_id(sm.move_id)
            .ok()
            .flatten()
            .is_some_and(|b| sm.pp < b.pp)
    })
}

/// Use a medicine on the first party member (in party order) that benefits from it. This is
/// the deterministic default of the bag screen; the real game asks for a target.
pub fn use_medicine_on_best_target(
    progress: &mut PlayerProgress,
    assets: &BattleAssets,
    item_id: u16,
) -> Result<FieldUse, String> {
    let item = assets
        .items
        .get(item_id)
        .ok_or_else(|| format!("unknown item #{item_id}"))?;
    if item.effect_kind != 1 {
        return Err(format!("#{item_id} is not a medicine"));
    }
    if progress.party.is_empty() {
        return Err("the party is empty".into());
    }
    let mut last = "no party member can benefit from this item".to_string();
    for pi in 0..progress.party.len() {
        let slot = first_pp_slot(progress, assets, pi);
        match field_use_medicine(progress, assets, item_id, pi, slot) {
            Ok(u) => return Ok(u),
            Err(e) => last = e,
        }
    }
    Err(last)
}

// ---------------------------------------------------------------- bevy glue

/// The item table used by the screen (loaded once at install; `None` = not found).
#[derive(Resource, Debug, Clone, Default)]
pub struct BagItemTable(pub Option<ItemTable>);

/// Decoded shops, in sheet order. Empty when the sheet is missing.
#[derive(Resource, Debug, Clone, Default)]
pub struct ShopCatalog(pub Vec<Shop>);

/// Whether `--debug-shop` asked for a shop trigger at the start position.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct DebugShop(pub bool);

/// Standing within `radius` and pressing E opens shop `shop`.
#[derive(Component, Debug, Clone, Copy)]
pub struct ShopTrigger {
    pub shop: usize,
    pub radius: f32,
}

/// Root of the provisional bag (hidden while the original one is shown, `orig_ui`).
#[derive(Component)]
pub struct BagRoot;

#[derive(Component, Clone, Copy)]
enum BagText {
    Title,
    Money,
    Tab,
    Row(usize),
    Detail,
    Message,
    Hint,
}

#[derive(Component, Clone, Copy)]
struct BagRow(usize);

pub struct BagScreenPlugin {
    /// `--debug-shop`: spawn a test shop trigger at the player's start.
    pub debug_shop: bool,
    /// Directory with `item_effects.tsv` / `shop_stock.tsv`; searched next to the executable
    /// (`data`, `../../data`, `$CARGO_MANIFEST_DIR/../../data`) when `None`.
    pub data_dir: Option<PathBuf>,
}

impl BagScreenPlugin {
    pub fn from_env_and_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        Self {
            debug_shop: args.iter().any(|a| a == "--debug-shop"),
            data_dir: None,
        }
    }

    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        Self {
            debug_shop: false,
            data_dir: Some(data_dir),
        }
    }
}

/// Where `item_effects.tsv` lives when no directory is given.
pub fn find_item_data_dir() -> Option<PathBuf> {
    [
        PathBuf::from("data"),
        PathBuf::from("../../data"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
    ]
    .into_iter()
    .find(|p| p.join("item_effects.tsv").is_file())
}

fn set_freeze(
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

impl Plugin for BagScreenPlugin {
    fn build(&self, app: &mut App) {
        let root = self.data_dir.clone().or_else(find_item_data_dir);
        let table = root.as_ref().and_then(|dir| {
            ItemTable::load(dir)
                .map_err(|e| eprintln!("[bag] item table unavailable ({e})"))
                .ok()
        });
        let shops = match (&root, &table) {
            (Some(dir), Some(t)) => Shop::load(dir)
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    eprintln!("[bag] shop_stock.tsv missing or empty; using the declared provisional stock");
                    vec![Shop::provisional(t)]
                }),
            _ => Vec::new(),
        };
        if let (Some(t), Some(shops)) = (&table, &shops.first()) {
            println!(
                "[bag] item table: {} items; shops: {} (first stock: {})",
                t.len(),
                shops.stock.len(),
                if shops.hash == 0 {
                    "provisional"
                } else {
                    "decoded"
                }
            );
        }
        app.insert_resource(BagItemTable(table))
            .insert_resource(ShopCatalog(shops))
            .insert_resource(DebugShop(self.debug_shop))
            .init_resource::<BagUi>()
            .init_resource::<Paused>()
            .init_resource::<PauseMenuSuppressed>()
            .init_resource::<BagOpenRequested>()
            .init_resource::<ShopOpenRequested>();
        app.add_systems(Startup, spawn_bag_ui);
        app.add_systems(
            Update,
            (
                open_requested,
                shop_open_requested,
                spawn_debug_shop_trigger,
                shop_trigger_interact,
                bag_input,
                sync_bag_ui,
            )
                .chain(),
        );
    }
}

fn spawn_bag_ui(mut commands: Commands) {
    commands
        .spawn((
            BagRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.62)),
            GlobalZIndex(120),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Px(860.0),
                    max_height: Val::Percent(88.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.96)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    BagText::Title,
                    Text::new("BAG"),
                    TextFont::from_font_size(28.0),
                    TextColor(Color::srgb(0.95, 0.9, 0.6)),
                ));
                panel.spawn((
                    BagText::Money,
                    Text::new(""),
                    TextFont::from_font_size(18.0),
                    TextColor(Color::srgb(0.85, 0.9, 0.7)),
                ));
                panel.spawn((
                    BagText::Tab,
                    Text::new(""),
                    TextFont::from_font_size(18.0),
                    TextColor(Color::srgb(0.75, 0.85, 1.0)),
                ));
                for i in 0..VISIBLE_ROWS {
                    panel
                        .spawn((
                            BagRow(i),
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(Color::NONE),
                        ))
                        .with_children(|row| {
                            row.spawn((
                                BagText::Row(i),
                                Text::new(""),
                                TextFont::from_font_size(17.0),
                                TextColor(Color::WHITE),
                            ));
                        });
                }
                panel.spawn((
                    BagText::Detail,
                    Text::new(""),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::srgb(0.8, 0.85, 0.9)),
                ));
                panel.spawn((
                    BagText::Message,
                    Text::new(""),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::srgb(0.95, 0.75, 0.45)),
                ));
                panel.spawn((
                    BagText::Hint,
                    Text::new(""),
                    TextFont::from_font_size(13.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
                ));
            });
        });
}

fn open_requested(
    mut request: ResMut<BagOpenRequested>,
    mut ui: ResMut<BagUi>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
) {
    if !request.0 {
        return;
    }
    request.0 = false;
    ui.open = true;
    ui.mode = BagMode::Bag;
    ui.pocket = 0;
    ui.selected = 0;
    ui.message.clear();
    set_freeze(&mut paused, &mut suppressed, true);
}

/// The pause menu's `Shop` row: open shop 0 (the `--debug-shop` test shop) in shop mode.
fn shop_open_requested(
    mut request: ResMut<ShopOpenRequested>,
    shops: Res<ShopCatalog>,
    mut ui: ResMut<BagUi>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
) {
    if !request.0 {
        return;
    }
    request.0 = false;
    if shops.0.is_empty() {
        // No shop sheet: nothing to open, the pause menu stays as it was.
        eprintln!("[bag] shop requested but no shop data is loaded");
        return;
    }
    ui.open = true;
    ui.mode = BagMode::Shop;
    ui.shop = 0;
    ui.tab = ShopTab::Buy;
    ui.selected = 0;
    ui.pocket = 0;
    ui.message.clear();
    set_freeze(&mut paused, &mut suppressed, true);
}

fn spawn_debug_shop_trigger(
    mut commands: Commands,
    debug: Res<DebugShop>,
    players: Query<&Transform, With<PlayerAvatar>>,
    triggers: Query<(), With<ShopTrigger>>,
) {
    if !debug.0 || !triggers.is_empty() {
        return;
    }
    let Ok(pf) = players.single() else {
        return;
    };
    commands.spawn((
        ShopTrigger {
            shop: 0,
            radius: 3.0,
        },
        Transform::from_translation(pf.translation),
    ));
    println!(
        "[bag] --debug-shop: shop trigger at the start position ({:.1}, {:.1}, {:.1}); press E to trade",
        pf.translation.x, pf.translation.y, pf.translation.z
    );
}

#[allow(clippy::too_many_arguments)]
fn shop_trigger_interact(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    triggers: Query<(&Transform, &ShopTrigger)>,
    players: Query<&Transform, (With<PlayerAvatar>, Without<ShopTrigger>)>,
    shops: Res<ShopCatalog>,
    state: Option<Res<State<GameState>>>,
    mut ui: ResMut<BagUi>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
) {
    if ui.open || shops.0.is_empty() {
        return;
    }
    if state.as_ref().is_some_and(|s| **s != GameState::World) {
        return;
    }
    if paused.as_ref().is_some_and(|p| p.0) {
        return;
    }
    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    let Ok(pf) = players.single() else {
        return;
    };
    let found = triggers
        .iter()
        .find(|(t, trig)| {
            trig.shop < shops.0.len() && pf.translation.distance(t.translation) <= trig.radius
        })
        .map(|(_, trig)| *trig);
    let Some(trig) = found else {
        return;
    };
    keys.clear_just_pressed(KeyCode::KeyE);
    ui.open = true;
    ui.mode = BagMode::Shop;
    ui.shop = trig.shop;
    ui.tab = ShopTab::Buy;
    ui.selected = 0;
    ui.pocket = 0;
    ui.message.clear();
    set_freeze(&mut paused, &mut suppressed, true);
}

#[allow(clippy::too_many_arguments)]
fn bag_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut ui: ResMut<BagUi>,
    table: Res<BagItemTable>,
    shops: Res<ShopCatalog>,
    assets: Option<Res<BattleAssets>>,
    mut progress: Option<ResMut<PlayerProgress>>,
    state: Option<Res<State<GameState>>>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
) {
    let toggle = keys.just_pressed(KeyCode::KeyI);
    if !ui.open {
        // `I` opens the bag in the world, unless the pause menu already owns the input.
        let pause_menu_open = paused.as_ref().is_some_and(|p| p.0);
        if toggle && !pause_menu_open && !state.as_ref().is_some_and(|s| **s != GameState::World) {
            ui.open = true;
            ui.mode = BagMode::Bag;
            ui.pocket = 0;
            ui.selected = 0;
            ui.message.clear();
            set_freeze(&mut paused, &mut suppressed, true);
        }
        return;
    }
    if toggle || keys.just_pressed(KeyCode::Escape) {
        keys.clear_just_pressed(KeyCode::KeyI);
        keys.clear_just_pressed(KeyCode::Escape);
        ui.open = false;
        ui.message.clear();
        set_freeze(&mut paused, &mut suppressed, false);
        return;
    }
    let Some(ref table) = table.0 else {
        return;
    };
    let empty = BTreeMap::new();
    let bag = progress.as_ref().map(|p| &p.bag).unwrap_or(&empty);
    let entries = screen_entries(&ui, table, bag, &shops.0);
    let len = entries.len();

    // Selection.
    let mut delta = 0i32;
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
        ui.selected = move_selection(ui.selected, delta.signum(), len);
    }

    // Pocket (bag) or buy/sell tab (shop).
    let mut axis = 0i32;
    for k in [KeyCode::ArrowLeft, KeyCode::KeyA] {
        if keys.just_pressed(k) {
            axis -= 1;
            keys.clear_just_pressed(k);
        }
    }
    for k in [KeyCode::ArrowRight, KeyCode::KeyD] {
        if keys.just_pressed(k) {
            axis += 1;
            keys.clear_just_pressed(k);
        }
    }
    if axis != 0 {
        match ui.mode {
            BagMode::Bag => {
                let pockets = pocket_codes(table, bag);
                ui.pocket = move_selection(ui.pocket, axis.signum(), pockets.len().max(1));
                ui.selected = 0;
            }
            BagMode::Shop => {
                ui.tab = if axis < 0 {
                    ShopTab::Buy
                } else {
                    ShopTab::Sell
                };
                ui.selected = 0;
            }
        }
    }

    // Quantity.
    if keys.just_pressed(KeyCode::BracketLeft) {
        keys.clear_just_pressed(KeyCode::BracketLeft);
        ui.quantity = ui.quantity.saturating_sub(1).max(1);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        keys.clear_just_pressed(KeyCode::BracketRight);
        ui.quantity = (ui.quantity + 1).min(MAX_TRADE_QUANTITY);
    }

    // Action.
    let activate = [KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space]
        .into_iter()
        .find(|k| keys.just_pressed(*k));
    let Some(activate) = activate else {
        return;
    };
    keys.clear_just_pressed(activate);
    let Some((item_id, _count)) = entries.get(ui.selected).copied() else {
        ui.message = "Nothing selected".into();
        return;
    };
    let Some(progress) = progress.as_deref_mut() else {
        ui.message = "No player progress loaded".into();
        return;
    };
    match ui.mode {
        BagMode::Bag => {
            let Some(assets) = assets.as_deref() else {
                ui.message = "Battle data unavailable; field use is disabled".into();
                return;
            };
            ui.message = match use_medicine_on_best_target(progress, assets, item_id) {
                Ok(u) => field_use_message(&u),
                Err(e) => format!("#{item_id}: {e}"),
            };
        }
        BagMode::Shop => {
            let Some(shop) = shops.0.get(ui.shop) else {
                ui.message = "No shop loaded".into();
                return;
            };
            let qty = ui.quantity;
            ui.message = match ui.tab {
                ShopTab::Buy => match shop.buy(progress, table, item_id, qty) {
                    Ok(paid) => format!("Bought #{} x{} for {}", item_id, qty, paid),
                    Err(e) => format!("Buy #{} x{} failed: {}", item_id, qty, e),
                },
                ShopTab::Sell => match shop.sell(progress, table, item_id, qty) {
                    Ok(gained) => format!("Sold #{} x{} for {}", item_id, qty, gained),
                    Err(e) => format!("Sell #{} x{} failed: {}", item_id, qty, e),
                },
            };
        }
    }
}

/// All strings the screen shows, computed in one place so the sync system stays small.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct BagViewStrings {
    title: String,
    money: String,
    tab: String,
    lines: Vec<String>,
    highlighted: Option<usize>,
    detail: String,
    hint: String,
}

impl BagViewStrings {
    fn missing_table() -> Self {
        Self {
            title: "BAG".to_string(),
            tab: "data/item_effects.tsv not found".to_string(),
            lines: vec![String::new(); VISIBLE_ROWS],
            hint: String::new(),
            ..Self::default()
        }
    }

    fn compute(
        ui: &BagUi,
        table: &ItemTable,
        shops: &[Shop],
        bag: &BTreeMap<u16, u16>,
        money: u32,
    ) -> Self {
        let entries = screen_entries(ui, table, bag, shops);
        let first = ui.selected.saturating_sub(VISIBLE_ROWS - 1);
        let mut out = Self {
            title: match ui.mode {
                BagMode::Bag => "BAG".to_string(),
                BagMode::Shop => match shops.get(ui.shop) {
                    Some(s) if s.hash != 0 => format!("SHOP {} (id 0x{:016x})", ui.shop, s.hash),
                    _ => "SHOP (provisional stock)".to_string(),
                },
            },
            money: format!("Money: {money}"),
            tab: match ui.mode {
                BagMode::Bag => {
                    let pockets = pocket_codes(table, bag);
                    let pocket = pockets.get(ui.pocket).copied().unwrap_or(0);
                    let n = pocket_entries(table, bag, pocket).len();
                    format!("Pocket {pocket} ({n} lines) - Left/Right to change")
                }
                BagMode::Shop => match ui.tab {
                    ShopTab::Buy => format!(
                        "BUY ({} in stock) - Left/Right to switch",
                        shops.get(ui.shop).map_or(0, |s| s.stock.len())
                    ),
                    ShopTab::Sell => "SELL (your priced items) - Left/Right to switch".to_string(),
                },
            },
            lines: vec![String::new(); VISIBLE_ROWS],
            highlighted: None,
            detail: entries
                .get(ui.selected)
                .and_then(|(id, count)| table.get(*id).map(|item| detail_for(item, *count)))
                .map(|s| format!("{}  [{}/{}]", s, ui.selected + 1, entries.len()))
                .unwrap_or_else(|| "Empty".to_string()),
            hint: match ui.mode {
                BagMode::Bag => {
                    "I/Esc close - W/S select - A/D pocket - [ ] quantity - Enter: use on the first party member that benefits"
                }
                BagMode::Shop => "I/Esc close - W/S select - A/D Buy/Sell - [ ] quantity - Enter: trade",
            }
            .to_string(),
        };
        for (i, line) in out.lines.iter_mut().enumerate() {
            if let Some((id, count)) = entries.get(first + i).copied() {
                if let Some(item) = table.get(id) {
                    *line = line_for(item, count);
                }
                if first + i == ui.selected {
                    out.highlighted = Some(i);
                }
            }
        }
        out
    }
}

fn sync_bag_ui(
    ui: Res<BagUi>,
    table: Res<BagItemTable>,
    shops: Res<ShopCatalog>,
    progress: Option<Res<PlayerProgress>>,
    mut root: Query<&mut Visibility, With<BagRoot>>,
    mut rows: Query<(&BagRow, &mut BackgroundColor)>,
    mut texts: Query<(&BagText, &mut Text)>,
) {
    for mut v in &mut root {
        *v = if ui.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !ui.open {
        return;
    }

    // Compute every string first, then write it into the marked texts.
    let empty = BTreeMap::new();
    let view = match &table.0 {
        Some(table) => BagViewStrings::compute(
            &ui,
            table,
            &shops.0,
            progress.as_ref().map(|p| &p.bag).unwrap_or(&empty),
            progress.as_ref().map_or(0, |p| p.money),
        ),
        None => BagViewStrings::missing_table(),
    };

    for (marker, mut text) in &mut texts {
        let wish: Option<&str> = match marker {
            BagText::Title => Some(&view.title),
            BagText::Money => Some(&view.money),
            BagText::Tab => Some(&view.tab),
            BagText::Row(i) => view.lines.get(*i).map(String::as_str),
            BagText::Detail => Some(&view.detail),
            BagText::Message => Some(&ui.message),
            BagText::Hint => Some(&view.hint),
        };
        if let Some(wish) = wish {
            if text.0 != wish {
                text.0 = wish.to_string();
            }
        }
    }
    for (r, mut bg) in &mut rows {
        bg.0 = if view.highlighted == Some(r.0) {
            Color::srgba(0.85, 0.6, 0.15, 0.85)
        } else if !view.lines[r.0].is_empty() {
            Color::srgba(0.13, 0.17, 0.24, 0.9)
        } else {
            Color::NONE
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle_engine::items::HpHeal;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    fn fx(id: u16, price: u32, pocket: u8) -> ItemEffect {
        ItemEffect {
            item_id: id,
            price,
            max_quantity: 999,
            effect_kind: 1,
            pocket_code: pocket,
            storage_class: 0,
            ball_category: None,
            hp_heal: HpHeal::Fixed(30),
            cures: vec![],
            cures_confusion: false,
            revives: false,
            pp_restore_one: false,
            pp_restore_all: false,
            pp_amount: 0,
            level_up: false,
        }
    }

    fn table() -> ItemTable {
        let mut ball = fx(4, 0, 2);
        ball.effect_kind = 0;
        ball.ball_category = Some("Regular".into());
        ItemTable::from_items([
            fx(17, 200, 6),
            fx(26, 400, 6),
            ball,
            fx(155, 80, 7),
            fx(1694, 120, 0),
        ])
    }

    fn bag(pairs: &[(u16, u16)]) -> BTreeMap<u16, u16> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn pockets_group_items_and_keep_the_misc_pocket() {
        let t = table();
        let b = bag(&[(17, 2), (26, 1), (4, 5), (155, 3), (1694, 1)]);
        assert_eq!(pocket_codes(&t, &b), vec![0, 2, 6, 7]);
        assert_eq!(pocket_entries(&t, &b, 6), vec![(17, 2), (26, 1)]);
        assert_eq!(pocket_entries(&t, &b, 2), vec![(4, 5)]);
        assert_eq!(pocket_entries(&t, &b, 7), vec![(155, 3)]);
        assert_eq!(pocket_entries(&t, &b, 0), vec![(1694, 1)]);
        // Unknown ids are skipped, zero counts never show.
        let b = bag(&[(999, 9), (17, 0)]);
        assert_eq!(pocket_entries(&t, &b, 6), Vec::new());
    }

    #[test]
    fn lines_show_the_decoded_effect_and_count() {
        let t = table();
        let line = line_for(t.get(17).unwrap(), 3);
        assert!(line.contains("#17"), "{line}");
        assert!(line.ends_with("x3"), "{line}");
        let detail = detail_for(t.get(17).unwrap(), 3);
        assert!(detail.contains("price 200"), "{detail}");
        assert!(detail.contains("sell 100"), "{detail}");
        assert!(detail.contains("pocket 6"), "{detail}");
    }

    #[test]
    fn shop_rows_follow_the_selected_tab() {
        let t = table();
        let shops = vec![Shop {
            hash: 7,
            stock: vec![17, 4],
        }];
        let b = bag(&[(17, 1), (155, 4)]);
        let mut ui = BagUi {
            open: true,
            mode: BagMode::Shop,
            shop: 0,
            ..BagUi::default()
        };
        assert_eq!(screen_entries(&ui, &t, &b, &shops), vec![(17, 1), (4, 0)]);
        // The sell tab lists every held item with a price, in bag (id) order.
        ui.tab = ShopTab::Sell;
        assert_eq!(screen_entries(&ui, &t, &b, &shops), vec![(17, 1), (155, 4)]);
        // A price-0 item never appears in the sell list (4 has price 0).
        let b = bag(&[(17, 1), (4, 2)]);
        assert_eq!(screen_entries(&ui, &t, &b, &shops), vec![(17, 1)]);
    }

    #[test]
    fn selection_wraps_and_empty_lists_stay_at_zero() {
        assert_eq!(move_selection(0, -1, 3), 2);
        assert_eq!(move_selection(2, 1, 3), 0);
        assert_eq!(move_selection(5, 1, 0), 0);
    }

    #[test]
    fn every_real_item_lands_in_exactly_one_pocket() {
        let t = ItemTable::load(root()).unwrap();
        assert_eq!(t.len(), 806, "real item_effects.tsv rows");
        let b: BTreeMap<u16, u16> = (0..=2000u16)
            .filter(|id| t.get(*id).is_some())
            .map(|id| (id, 1))
            .collect();
        assert_eq!(b.len(), 806);
        let total: usize = pocket_codes(&t, &b)
            .into_iter()
            .map(|p| pocket_entries(&t, &b, p).len())
            .sum();
        assert_eq!(total, 806, "no item may be dropped by the pocket grouping");
    }

    #[test]
    fn real_item_table_parses_the_pocket_column() {
        let t = ItemTable::load(root()).unwrap();
        assert_eq!(t.get(1710).unwrap().pocket_code, 2, "Poke Ball");
        assert_eq!(t.get(17).unwrap().pocket_code, 6, "Potion");
        assert_eq!(t.get(155).unwrap().pocket_code, 7, "berry");
    }

    fn assets() -> BattleAssets {
        BattleAssets::load(root()).expect("real battle data")
    }

    fn damaged_member() -> PartyMember {
        PartyMember {
            species: 25,
            form: 0,
            level: 5,
            hp: 1,
            status: None,
            moves: vec![],
            effort: [0; 6],
            nickname: None,
            exp: None,
        }
    }

    #[test]
    fn field_use_runs_the_engine_rule_and_consumes_the_item() {
        let a = assets();
        let mut p = PlayerProgress {
            party: vec![damaged_member()],
            bag: bag(&[(17, 2)]),
            money: 0,
            ..Default::default()
        };
        let u = use_medicine_on_best_target(&mut p, &a, 17).unwrap();
        assert_eq!(u.target, 0);
        assert!(u.hp_restored > 0, "potion heals through the engine");
        assert!(p.party[0].hp > 1);
        assert_eq!(p.bag.get(&17), Some(&1));
        let msg = field_use_message(&u);
        assert!(msg.contains("#25"), "{msg}");
        assert!(msg.contains("HP"), "{msg}");
    }

    #[test]
    fn field_use_picks_the_first_member_that_benefits() {
        let a = assets();
        let mut healthy = damaged_member();
        healthy.hp = 120;
        let mut p = PlayerProgress {
            party: vec![healthy, damaged_member()],
            bag: bag(&[(17, 1)]),
            money: 0,
            ..Default::default()
        };
        let u = use_medicine_on_best_target(&mut p, &a, 17).unwrap();
        assert_eq!(
            u.target, 1,
            "the healthy member would make the item a no-op"
        );
        assert_eq!(p.bag.get(&17), None, "the last unit leaves the bag");
    }

    #[test]
    fn field_use_is_refused_without_effect_and_keeps_the_item() {
        let a = assets();
        let mut healthy = damaged_member();
        healthy.hp = 120;
        let mut p = PlayerProgress {
            party: vec![healthy],
            bag: bag(&[(17, 3)]),
            money: 0,
            ..Default::default()
        };
        assert!(use_medicine_on_best_target(&mut p, &a, 17).is_err());
        assert_eq!(p.bag.get(&17), Some(&3), "a refused use consumes nothing");
        // A ball is not a field medicine.
        let mut p = PlayerProgress {
            party: vec![damaged_member()],
            bag: bag(&[(4, 1)]),
            money: 0,
            ..Default::default()
        };
        assert!(use_medicine_on_best_target(&mut p, &a, 4)
            .unwrap_err()
            .contains("not a medicine"));
    }

    fn app(data_dir: PathBuf, debug_shop: bool) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_plugins(BagScreenPlugin {
            debug_shop,
            data_dir: Some(data_dir),
        });
        app.insert_resource(PlayerProgress {
            party: vec![damaged_member()],
            bag: bag(&[(17, 3), (26, 5)]),
            money: 500,
            ..Default::default()
        });
        app.insert_resource(assets());
        app.update();
        app
    }

    fn tap(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }

    fn root_visible(app: &mut App) -> bool {
        let mut q = app
            .world_mut()
            .query_filtered::<&Visibility, With<BagRoot>>();
        let world = app.world();
        q.iter(world)
            .next()
            .is_some_and(|v| *v == Visibility::Visible)
    }

    #[test]
    fn keyboard_opens_the_bag_and_freezes_the_world() {
        let mut app = app(root(), false);
        assert!(!root_visible(&mut app));
        tap(&mut app, KeyCode::KeyI);
        assert!(app.world().resource::<BagUi>().open);
        assert!(root_visible(&mut app), "the bag root is shown");
        assert!(
            app.world().resource::<Paused>().0,
            "the world freezes while the bag is open"
        );
        assert!(
            app.world().resource::<PauseMenuSuppressed>().0,
            "the pause menu yields the input"
        );
        // Escape closes and unfreezes.
        tap(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<BagUi>().open);
        assert!(!app.world().resource::<Paused>().0);
        assert!(!app.world().resource::<PauseMenuSuppressed>().0);
        assert!(!root_visible(&mut app));
    }

    #[test]
    fn enter_uses_the_selected_medicine_on_the_party() {
        let mut app = app(root(), false);
        tap(&mut app, KeyCode::KeyI);
        // Pocket 0 (misc) is shown first; step once to the medicine pocket (codes [0, 6]).
        tap(&mut app, KeyCode::ArrowRight);
        assert_eq!(app.world().resource::<BagUi>().pocket, 1);
        tap(&mut app, KeyCode::Enter);
        let progress = app.world().resource::<PlayerProgress>();
        assert!(
            progress.party[0].hp > 1,
            "the field use healed the damaged member"
        );
        assert_eq!(progress.bag.get(&17), Some(&2), "one potion was consumed");
        assert!(app.world().resource::<BagUi>().message.contains("HP"));
    }

    #[test]
    fn arrows_move_the_selection_inside_the_open_bag() {
        let mut app = app(root(), false);
        tap(&mut app, KeyCode::KeyI);
        // Point the screen at the medicine pocket (the bag holds #17 and #26 there).
        let table = ItemTable::load(root()).unwrap();
        let bag = app.world().resource::<PlayerProgress>().bag.clone();
        let pockets = pocket_codes(&table, &bag);
        let medicine = pockets
            .iter()
            .position(|p| *p == 6)
            .expect("medicine pocket");
        {
            let mut ui = app.world_mut().resource_mut::<BagUi>();
            ui.pocket = medicine;
            ui.selected = 0;
        }
        app.update();
        assert_eq!(
            screen_entries(app.world().resource::<BagUi>(), &table, &bag, &[]).len(),
            2
        );
        tap(&mut app, KeyCode::ArrowDown);
        assert_eq!(app.world().resource::<BagUi>().selected, 1);
        tap(&mut app, KeyCode::ArrowUp);
        assert_eq!(app.world().resource::<BagUi>().selected, 0);
    }

    #[test]
    fn shop_trigger_opens_the_decoded_stock() {
        let mut app = app(root(), true);
        // The trigger spawns in the first update, at the player position; without a player it
        // is not spawned yet, so add one and run one more frame.
        app.world_mut()
            .spawn((PlayerAvatar, Transform::from_translation(Vec3::ZERO)));
        app.update();
        let mut q = app.world_mut().query::<&ShopTrigger>();
        let world = app.world();
        assert_eq!(q.iter(world).count(), 1);
        tap(&mut app, KeyCode::KeyE);
        let ui = app.world().resource::<BagUi>();
        assert!(ui.open);
        assert_eq!(ui.mode, BagMode::Shop);
        assert_eq!(ui.shop, 0);
        let shops = app.world().resource::<ShopCatalog>();
        assert_eq!(shops.0.len(), 8, "the real sheet loads 8 decoded shops");
        let table = app.world().resource::<BagItemTable>().0.clone().unwrap();
        assert!(shops.0[0].stock.iter().all(|id| table.get(*id).is_some()));
    }

    #[test]
    fn pause_menu_shop_request_opens_shop_zero_and_esc_closes_it() {
        let mut app = app(root(), false);
        app.world_mut().resource_mut::<ShopOpenRequested>().0 = true;
        app.update();
        {
            let ui = app.world().resource::<BagUi>();
            assert!(ui.open);
            assert_eq!((ui.mode, ui.shop), (BagMode::Shop, 0));
        }
        assert!(!app.world().resource::<ShopOpenRequested>().0);
        assert!(app.world().resource::<Paused>().0);
        assert!(app.world().resource::<PauseMenuSuppressed>().0);
        tap(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<BagUi>().open);
        assert!(!app.world().resource::<Paused>().0);
    }

    #[test]
    fn pause_menu_request_opens_the_bag_once() {
        let mut app = app(root(), false);
        app.world_mut().resource_mut::<BagOpenRequested>().0 = true;
        app.update();
        assert!(app.world().resource::<BagUi>().open);
        assert!(
            !app.world().resource::<BagOpenRequested>().0,
            "the request is consumed"
        );
        assert!(app.world().resource::<Paused>().0);
        // A second frame without the request keeps the bag open.
        app.update();
        assert!(app.world().resource::<BagUi>().open);
    }
}
