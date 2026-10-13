//! Wiring of the player save (`save_game`) into the game loop.
//!
//! Persistent resources ([`PlayerProgress`]: party, box, bag, money, seen/caught sets, counters),
//! pure helpers (install the party into a battle, snapshot it afterwards, apply the result) and a
//! small Bevy plugin (load at start-up, F5, autosave after each battle and on zone change, HUD
//! toast). All rules live in pure functions so they are unit tested without a window.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::battle_controller::{ActiveBattleSession, BattleAssets, WILD};
use super::battle_items;
use super::play_time::PlayTime;
use super::save_game::{PartyMember, PlayerSave, SaveFile, SaveGame, MAX_BOX, MAX_PARTY};
use super::ui::ActiveZoneInfo;
use super::PlayerAvatar;
use crate::battle::{BattleData, BattleMove};
use crate::battle_engine::items::{ItemBag, ItemTable};
use crate::battle_engine::{Battle, Combatant, XorShift32};

/// Level of the demo lead (matches `ActivePokemonState::level` in the viewer).
pub const DEFAULT_LEAD_LEVEL: u8 = 25;

// ---------------------------------------------------------------- command line

/// Save-related command-line flags: `--new-game`, `--save-dir <path>`, `--save-selftest`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireCli {
    pub new_game: bool,
    pub save_dir: Option<PathBuf>,
    pub selftest: bool,
    pub has_area: bool,
    pub has_player_pos: bool,
    pub has_time_slot: bool,
}

impl WireCli {
    pub fn from_args(args: &[String]) -> Self {
        let has = |f: &str| args.iter().any(|a| a == f);
        Self {
            new_game: has("--new-game"),
            save_dir: args
                .iter()
                .position(|a| a == "--save-dir")
                .and_then(|i| args.get(i + 1))
                .map(PathBuf::from),
            selftest: has("--save-selftest"),
            has_area: has("--area"),
            has_player_pos: has("--player-pos"),
            has_time_slot: has("--time-slot"),
        }
    }

    /// `--save-dir` sets `PLA_SAVE_DIR` (call before `SaveGame::from_env`, at start-up only).
    pub fn apply_env(&self) {
        if let Some(dir) = &self.save_dir {
            std::env::set_var("PLA_SAVE_DIR", dir);
        }
    }
}

// ---------------------------------------------------------------- progress

/// Everything that persists between battles and sessions, in the save's own terms.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct PlayerProgress {
    /// Up to 6 members, in party order.
    pub party: Vec<PartyMember>,
    /// Caught Pokemon that did not fit in the party. Persisted as `PlayerSave::box_pokemon`
    /// (save format v3+, up to [`MAX_BOX`]).
    pub box_list: Vec<PartyMember>,
    /// item id -> quantity.
    pub bag: BTreeMap<u16, u16>,
    pub money: u32,
    pub seen: BTreeSet<u16>,
    pub caught: BTreeSet<u16>,
    pub defeated: u32,
    pub caught_count: u32,
    pub rng_seed: u32,
    pub time_slot: u8,
}

impl PlayerProgress {
    pub fn new_game() -> Self {
        Self { rng_seed: 0x5ca1_e001, time_slot: 1, ..Self::default() }
    }

    pub fn from_save(s: &PlayerSave) -> Self {
        Self {
            party: s.party.clone(),
            box_list: s.box_pokemon.clone(),
            bag: s.bag.clone(),
            money: s.money,
            seen: s.seen.clone(),
            caught: s.caught.clone(),
            defeated: s.defeated,
            caught_count: s.caught_count,
            rng_seed: if s.rng_seed == 0 { 0x5ca1_e001 } else { s.rng_seed },
            time_slot: s.time_slot,
        }
    }

    /// Snapshot for the file. `zone` is the zone id (`ha_area00`, `ha_area00_s03_a`),
    /// `position`/`yaw` the player's pose.
    pub fn to_save(&self, zone: &str, position: [f32; 3], yaw: f32) -> PlayerSave {
        PlayerSave {
            zone: zone.to_string(),
            position,
            yaw,
            time_slot: self.time_slot,
            party: self.party.iter().take(MAX_PARTY).cloned().collect(),
            box_pokemon: self.box_list.iter().take(MAX_BOX).cloned().collect(),
            bag: self.bag.iter().filter(|(_, n)| **n > 0).map(|(i, n)| (*i, *n)).collect(),
            money: self.money,
            seen: self.seen.clone(),
            caught: self.caught.clone(),
            defeated: self.defeated,
            caught_count: self.caught_count,
            rng_seed: self.rng_seed,
            ..PlayerSave::default()
        }
    }

    /// Add money (saturating).
    pub fn add_money(&mut self, amount: u32) {
        self.money = self.money.saturating_add(amount);
    }

    /// Store `m` in the box. Returns its slot, or [`BoxError::BoxFull`] at [`MAX_BOX`].
    pub fn deposit(&mut self, m: PartyMember) -> Result<usize, BoxError> {
        if self.box_list.len() >= MAX_BOX {
            return Err(BoxError::BoxFull);
        }
        self.box_list.push(m);
        Ok(self.box_list.len() - 1)
    }

    /// Move the box Pokemon at `box_index` back into the party.
    pub fn withdraw(&mut self, box_index: usize) -> Result<PartyMember, BoxError> {
        if box_index >= self.box_list.len() {
            return Err(BoxError::BadBoxSlot);
        }
        if self.party.len() >= MAX_PARTY {
            return Err(BoxError::PartyFull);
        }
        let m = self.box_list.remove(box_index);
        self.party.push(m.clone());
        Ok(m)
    }

    /// Exchange the party member at `party_index` with the box Pokemon at `box_index`.
    pub fn swap(&mut self, box_index: usize, party_index: usize) -> Result<(), BoxError> {
        if box_index >= self.box_list.len() {
            return Err(BoxError::BadBoxSlot);
        }
        if party_index >= self.party.len() {
            return Err(BoxError::BadPartySlot);
        }
        std::mem::swap(&mut self.box_list[box_index], &mut self.party[party_index]);
        Ok(())
    }
}

/// Why a box operation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxError {
    /// The box already holds [`MAX_BOX`] Pokemon.
    BoxFull,
    /// The party already holds [`MAX_PARTY`] Pokemon.
    PartyFull,
    /// No Pokemon at that box slot.
    BadBoxSlot,
    /// No Pokemon at that party slot.
    BadPartySlot,
}

impl fmt::Display for BoxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoxFull => write!(f, "the box is full"),
            Self::PartyFull => write!(f, "the party is full"),
            Self::BadBoxSlot => write!(f, "no Pokemon in that box slot"),
            Self::BadPartySlot => write!(f, "no Pokemon in that party slot"),
        }
    }
}
impl std::error::Error for BoxError {}

/// `ha_area01` for area index 1.
pub fn zone_id(area_idx: u8) -> String {
    format!("ha_area{area_idx:02}")
}

/// Provisional prize paid when a wild Pokemon faints in battle: `50 + 10 x level` (ASSUMED — the
/// original game's money rewards are not decoded, and in PLA money comes mostly from requests
/// and selling, so this port formula is a placeholder). Saturating through
/// [`PlayerProgress::add_money`].
pub fn victory_reward(level: u8) -> u32 {
    50 + 10 * u32::from(level)
}

/// Inverse of [`zone_id`].
pub fn parse_zone(zone: &str) -> Option<u8> {
    zone.strip_prefix("ha_area")?.parse().ok()
}

/// Demo party: the exact player side a fresh battle builds (Pikachu plus two bench members).
pub fn default_party(assets: &BattleAssets) -> Vec<PartyMember> {
    match ActiveBattleSession::start(
        assets,
        "pikachu",
        DEFAULT_LEAD_LEVEL,
        "bidoof",
        5,
        399,
        Entity::PLACEHOLDER,
    ) {
        Ok(s) => s.battle.sides[0]
            .iter()
            .map(|&i| PartyMember::from_combatant(&s.battle.combatants[i]))
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Starter bag as saved quantities.
pub fn default_bag(items: &ItemTable) -> BTreeMap<u16, u16> {
    let mut bag = ItemBag::default();
    let mut out = BTreeMap::new();
    for (id, qty) in battle_items::starter_selection(items) {
        if bag.add(items, id, qty).is_ok() {
            out.insert(id, bag.count(id));
        }
    }
    out
}

// ---------------------------------------------------------------- start-up load

/// What the start-up load decided.
#[derive(Debug, Clone)]
pub struct StartupState {
    pub progress: PlayerProgress,
    pub area: Option<u8>,
    pub position: Option<[f32; 3]>,
    pub yaw: f32,
    pub time_slot: Option<usize>,
    /// Play time stored in the save (0 for a new game or a pre-v4 file).
    pub play_seconds: u64,
    pub loaded: bool,
    pub message: String,
}

/// Load the save unless `--new-game`. `SaveGame::message` becomes the HUD text.
pub fn load_startup(game: &mut SaveGame, cli: &WireCli) -> StartupState {
    if cli.new_game {
        game.message = Some("New game (existing save ignored).".into());
        game.current = None;
        return StartupState {
            progress: PlayerProgress::new_game(),
            area: None,
            position: None,
            yaw: 0.0,
            time_slot: None,
            play_seconds: 0,
            loaded: false,
            message: game.message.clone().unwrap_or_default(),
        };
    }
    match game.load() {
        Some(s) => StartupState {
            area: parse_zone(&s.zone),
            position: Some(s.position),
            yaw: s.yaw,
            time_slot: Some(usize::from(s.time_slot.min(3))),
            play_seconds: s.play_seconds,
            progress: PlayerProgress::from_save(&s),
            loaded: true,
            message: game.message.clone().unwrap_or_default(),
        },
        None => StartupState {
            progress: PlayerProgress::new_game(),
            area: None,
            position: None,
            yaw: 0.0,
            time_slot: None,
            play_seconds: 0,
            loaded: false,
            message: game.message.clone().unwrap_or_default(),
        },
    }
}

// ---------------------------------------------------------------- battle bridge

/// Party indices fielded in the current battle, in side order. Position `j` is combatant `0`
/// for `j == 0` and `j + 1` otherwise (index 1 is the wild Pokemon).
pub type Roster = Vec<usize>;

pub fn combatant_index(position: usize) -> usize {
    if position == 0 {
        0
    } else {
        position + 1
    }
}

fn heal_full(c: &mut Combatant) {
    c.hp = c.max_hp;
    c.status = None;
    for m in &mut c.moves {
        m.pp = m.data.pp;
    }
}

/// Rebuild one saved member as an engine combatant (None when the species is unknown).
pub fn member_to_combatant(data: &BattleData, m: &PartyMember) -> Option<Combatant> {
    let species = data.species_row_by_dex(m.species).ok().flatten()?;
    let mut moves: Vec<BattleMove> = m
        .moves
        .iter()
        .filter_map(|sm| data.move_row_by_id(sm.move_id).ok().flatten())
        .collect();
    if moves.is_empty() {
        moves.push(data.move_row("tackle").ok().flatten()?);
    }
    Some(m.to_combatant(species, moves, [100; 6]))
}

/// Replace the session's player side with `party` (HP, PP, status restored; fainted members stay
/// fainted). If every member is fainted the party is fully restored first (a blackout heal: the
/// port has no village return yet). The wild combatant, effects and item tables are kept.
pub fn install_party(
    sess: &mut ActiveBattleSession,
    data: &BattleData,
    party: &[PartyMember],
) -> Result<(Roster, bool), String> {
    let mut roster = Vec::new();
    let mut mons = Vec::new();
    for (i, m) in party.iter().enumerate().take(MAX_PARTY) {
        if let Some(c) = member_to_combatant(data, m) {
            roster.push(i);
            mons.push(c);
        }
    }
    if mons.is_empty() {
        return Err("no party member could be rebuilt".into());
    }
    let blackout = mons.iter().all(|c| c.hp == 0);
    if blackout {
        mons.iter_mut().for_each(heal_full);
    }
    let wild = sess.battle.combatants[WILD].clone();
    let effects = sess.battle.move_effects().cloned();
    let items = sess.battle.item_table().cloned();
    let mut combatants = Vec::with_capacity(mons.len() + 1);
    let mut side = Vec::new();
    for (j, c) in mons.into_iter().enumerate() {
        if j == 1 {
            combatants.push(wild.clone());
        }
        side.push(combatants.len());
        combatants.push(c);
    }
    if combatants.len() == 1 {
        combatants.push(wild);
    }
    let mut battle = Battle::new(combatants, vec![side, vec![WILD]], data.type_chart().clone())?;
    if let Some(e) = effects {
        battle.set_move_effects(e);
    }
    if let Some(t) = items {
        battle.set_item_table(t);
    }
    sess.battle = battle;
    sess.selected_move_idx = 0;
    Ok((roster, blackout))
}

/// Engine bag from saved quantities (unknown ids and stack overflow are dropped by the engine).
pub fn engine_bag(map: &BTreeMap<u16, u16>, table: &ItemTable) -> ItemBag {
    let mut bag = ItemBag::default();
    for (&id, &qty) in map {
        let _ = bag.add(table, id, qty);
    }
    bag
}

/// What a battle left behind, taken from the session.
#[derive(Debug, Clone, PartialEq)]
pub struct BattleOutcome {
    /// (party index, member as it is now).
    pub party: Vec<(usize, PartyMember)>,
    /// Current quantity of every candidate item id (zero allowed).
    pub bag: BTreeMap<u16, u16>,
    pub wild_dex: u16,
    /// Level of the wild Pokemon (used by [`victory_reward`]).
    pub wild_level: u8,
    /// The wild Pokemon fainted and was not caught.
    pub won: bool,
    /// The wild Pokemon as a party member, when it was caught.
    pub caught: Option<PartyMember>,
    pub rng: u32,
}

pub fn snapshot(sess: &ActiveBattleSession, roster: &[usize], bag_ids: &[u16]) -> BattleOutcome {
    let party = roster
        .iter()
        .enumerate()
        .map(|(j, &p)| (p, PartyMember::from_combatant(&sess.battle.combatants[combatant_index(j)])))
        .collect();
    let wild = &sess.battle.combatants[WILD];
    BattleOutcome {
        party,
        bag: bag_ids.iter().map(|&id| (id, sess.bag.count(id))).collect(),
        wild_dex: sess.wild_dex,
        wild_level: wild.level,
        won: wild.hp == 0 && !wild.caught,
        caught: wild.caught.then(|| PartyMember::from_combatant(wild)),
        rng: sess.rng.0,
    }
}

/// Candidate item ids for syncing the bag back: the menu entries plus anything already stored.
pub fn bag_candidates(sess: &ActiveBattleSession, stored: &BTreeMap<u16, u16>) -> Vec<u16> {
    let mut ids: BTreeSet<u16> = sess.bag_order.iter().map(|e| e.item_id).collect();
    ids.extend(stored.keys().copied());
    ids.into_iter().collect()
}

/// Write the live party and bag back into the progress (every frame of a battle).
pub fn apply_sync(progress: &mut PlayerProgress, out: &BattleOutcome) {
    for (idx, now) in &out.party {
        if let Some(slot) = progress.party.get_mut(*idx) {
            let (form, nickname) = (slot.form, slot.nickname.clone());
            *slot = PartyMember { form, nickname, ..now.clone() };
        }
    }
    for (&id, &n) in &out.bag {
        if n == 0 {
            progress.bag.remove(&id);
        } else {
            progress.bag.insert(id, n);
        }
    }
    if out.rng != 0 {
        progress.rng_seed = out.rng;
    }
}

/// Record the encounter (seen set) when a battle starts.
pub fn note_encounter(progress: &mut PlayerProgress, wild_dex: u16) {
    progress.seen.insert(wild_dex);
}

/// Apply a finished battle: sync, then seen / caught / defeated counters, the prize money for a
/// win and the caught Pokemon (party if there is room, otherwise the box list).
pub fn apply_outcome(progress: &mut PlayerProgress, out: &BattleOutcome) {
    apply_sync(progress, out);
    progress.seen.insert(out.wild_dex);
    if let Some(m) = &out.caught {
        progress.caught.insert(out.wild_dex);
        progress.caught_count = progress.caught_count.saturating_add(1);
        if progress.party.len() < MAX_PARTY {
            progress.party.push(m.clone());
        } else if progress.deposit(m.clone()).is_err() {
            // Party AND box are full: the catch is still recorded as caught, but the Pokemon
            // cannot be stored (the real game's behaviour at this extreme is unknown).
        }
    } else if out.won {
        progress.defeated = progress.defeated.saturating_add(1);
        progress.add_money(victory_reward(out.wild_level));
    }
}

// ---------------------------------------------------------------- rest

/// Outcome of one Rest action.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RestReport {
    /// Members whose HP, PP and status were restored.
    pub healed: usize,
    /// Members that could not be rebuilt from the engine tables (left untouched).
    pub skipped: usize,
}

impl RestReport {
    /// Toast text for the pause menu.
    pub fn message(&self) -> String {
        match (self.healed, self.skipped) {
            (0, 0) => "Rest failed: the party is empty.".into(),
            (0, n) => format!("Rest failed: {n} party members could not be rebuilt."),
            (h, 0) => format!("Party fully healed ({h} members)."),
            (h, s) => format!("Party healed ({h} of {} members).", h + s),
        }
    }
}

/// Fully restore HP, PP and status of the party in place (the pause menu's Rest action). Max HP
/// and max PP come from the engine tables; a member whose species is unknown is left untouched and
/// counted as skipped. Fainted members are revived. No clock is advanced: the day/night clock is
/// Brief 9 and does not exist yet; the original's rest durations are not decoded (UNKNOWN).
pub fn rest_party(party: &mut [PartyMember], data: &BattleData) -> RestReport {
    let mut report = RestReport::default();
    for m in party.iter_mut() {
        let Some(c) = member_to_combatant(data, m) else {
            report.skipped += 1;
            continue;
        };
        m.hp = c.max_hp;
        m.status = None;
        for saved in &mut m.moves {
            if let Some(live) = c.moves.iter().find(|bm| bm.data.id == saved.move_id) {
                saved.pp = live.data.pp;
            }
        }
        report.healed += 1;
    }
    report
}

fn hp_line(party: &[PartyMember]) -> String {
    party
        .iter()
        .map(|m| format!("#{} Lv{} hp={}", m.species, m.level, m.hp))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Evidence text: party HP before the battle, after it (memory) and after reloading the file.
pub fn selftest_report(before: &[PartyMember], after: &[PartyMember], loaded: &[PartyMember]) -> String {
    format!(
        "[save-selftest] party before battle : {}\n[save-selftest] party after battle  : {}\n[save-selftest] party reloaded file : {}\n[save-selftest] reload matches memory: {}",
        hp_line(before),
        hp_line(after),
        hp_line(loaded),
        after == loaded
    )
}

// ---------------------------------------------------------------- bevy glue

/// Settings and bookkeeping of the plugin.
#[derive(Resource, Debug, Clone)]
pub struct WireSettings {
    pub yaw: f32,
    pub apply_yaw: bool,
    pub selftest: bool,
    pub startup_message: String,
}

/// Live battle bookkeeping.
#[derive(Resource, Debug, Default)]
pub struct BattleTracker {
    pub started: bool,
    pub roster: Roster,
    pub bag_ids: Vec<u16>,
    pub last: Option<BattleOutcome>,
    pub before: Vec<PartyMember>,
}

/// HUD message with a countdown.
#[derive(Resource, Debug, Default)]
pub struct SaveToast {
    pub text: String,
    pub timer: f32,
}

impl SaveToast {
    pub fn show(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.timer = 4.0;
    }
}

/// Set to `true` by another system (the pause menu) to save exactly like F5 does.
#[derive(Resource, Debug, Default)]
pub struct SaveRequested(pub bool);

/// Set to `true` by the pause menu's Rest row; [`rest_action`] heals the party and clears it.
#[derive(Resource, Debug, Default)]
pub struct RestRequested(pub bool);

#[derive(Component)]
struct ToastRoot;
#[derive(Component)]
struct ToastText;

pub struct SaveWirePlugin;

impl SaveWirePlugin {
    /// Insert the resources and systems. `game` is the (already loaded) `SaveGame`.
    pub fn install(app: &mut App, game: SaveGame, start: StartupState, cli: &WireCli) {
        app.insert_resource(game)
            .insert_resource(start.progress)
            .insert_resource(WireSettings {
                yaw: start.yaw,
                apply_yaw: start.loaded,
                selftest: cli.selftest,
                startup_message: start.message,
            })
            .init_resource::<BattleTracker>()
            .init_resource::<SaveToast>()
            .init_resource::<SaveRequested>()
            .init_resource::<RestRequested>()
            // Seeded by `PlayTimePlugin` at start-up; the default keeps this plugin usable alone.
            .init_resource::<PlayTime>();
        app.add_systems(Startup, (seed_defaults, spawn_toast))
            .add_systems(PostStartup, apply_loaded_yaw)
            .add_systems(PreUpdate, battle_wire)
            .add_systems(Update, (save_hotkey, zone_autosave, toast_tick).chain())
            // Runs even while paused (that is when the menu is open).
            .add_systems(Update, rest_action);
    }
}

fn seed_defaults(
    assets: Option<Res<BattleAssets>>,
    mut progress: ResMut<PlayerProgress>,
    settings: Res<WireSettings>,
    mut toast: ResMut<SaveToast>,
) {
    toast.show(settings.startup_message.clone());
    let Some(assets) = assets else { return };
    if progress.party.is_empty() {
        progress.party = default_party(&assets);
    }
    if progress.bag.is_empty() {
        progress.bag = default_bag(&assets.items);
    }
}

fn apply_loaded_yaw(settings: Res<WireSettings>, mut q: Query<&mut Transform, With<PlayerAvatar>>) {
    if settings.apply_yaw {
        for mut t in &mut q {
            t.rotation = Quat::from_rotation_y(settings.yaw);
        }
    }
}

fn spawn_toast(mut commands: Commands) {
    commands
        .spawn((
            ToastRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(24.0),
                bottom: Val::Px(24.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.88)),
            // Above the pause menu overlay so "Game saved." stays readable while paused.
            GlobalZIndex(200),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((
                ToastText,
                Text::new(""),
                TextFont::from_font_size(15.0),
                TextColor(Color::srgb(0.9, 0.95, 0.8)),
            ));
        });
}

fn toast_tick(
    time: Res<Time>,
    mut toast: ResMut<SaveToast>,
    mut root: Query<&mut Visibility, With<ToastRoot>>,
    mut text: Query<&mut Text, With<ToastText>>,
) {
    toast.timer = (toast.timer - time.delta_secs()).max(0.0);
    for mut v in &mut root {
        *v = if toast.timer > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
    }
    if toast.timer > 0.0 {
        for mut t in &mut text {
            if t.0 != toast.text {
                t.0 = toast.text.clone();
            }
        }
    }
}

fn pose(q: &Query<&Transform, With<PlayerAvatar>>) -> Option<([f32; 3], f32)> {
    let t = q.single().ok()?;
    Some(([t.translation.x, t.translation.y, t.translation.z], t.rotation.to_euler(EulerRot::YXZ).0))
}

fn do_save(
    game: &mut SaveGame,
    progress: &PlayerProgress,
    play: &PlayTime,
    zone: &ActiveZoneInfo,
    player: &Query<&Transform, With<PlayerAvatar>>,
    toast: &mut SaveToast,
) -> bool {
    let Some((pos, yaw)) = pose(player) else {
        toast.show("Save skipped: no player yet.");
        return false;
    };
    let mut save = progress.to_save(&zone.area, pos, yaw);
    save.play_seconds = play.seconds;
    let ok = game.save(save);
    toast.show(game.message.clone().unwrap_or_default());
    ok
}

/// Heal the party when the pause menu asked for a Rest. Refused during a battle or when the
/// battle tables are not loaded; the result is shown as a toast.
fn rest_action(
    mut requested: ResMut<RestRequested>,
    mut progress: ResMut<PlayerProgress>,
    assets: Option<Res<BattleAssets>>,
    session: Option<Res<ActiveBattleSession>>,
    mut toast: ResMut<SaveToast>,
) {
    if !std::mem::take(&mut requested.0) {
        return;
    }
    if session.is_some() {
        toast.show("Can't rest during a battle.");
        return;
    }
    let Some(assets) = assets else {
        toast.show("Rest needs the battle tables (not loaded).");
        return;
    };
    let report = rest_party(&mut progress.party, &assets.data);
    toast.show(report.message());
}

#[allow(clippy::too_many_arguments)]
fn save_hotkey(
    keys: Res<ButtonInput<KeyCode>>,
    mut requested: ResMut<SaveRequested>,
    mut game: ResMut<SaveGame>,
    progress: Res<PlayerProgress>,
    play: Res<PlayTime>,
    zone: Res<ActiveZoneInfo>,
    player: Query<&Transform, With<PlayerAvatar>>,
    mut toast: ResMut<SaveToast>,
    session: Option<Res<ActiveBattleSession>>,
) {
    let asked = std::mem::take(&mut requested.0);
    if !keys.just_pressed(KeyCode::F5) && !asked {
        return;
    }
    if session.is_some() {
        toast.show("Can't save during a battle.");
        return;
    }
    do_save(&mut game, &progress, &play, &zone, &player, &mut toast);
}

fn zone_autosave(
    zone: Res<ActiveZoneInfo>,
    mut game: ResMut<SaveGame>,
    progress: Res<PlayerProgress>,
    play: Res<PlayTime>,
    player: Query<&Transform, With<PlayerAvatar>>,
    mut toast: ResMut<SaveToast>,
    session: Option<Res<ActiveBattleSession>>,
) {
    if zone.is_changed() && !zone.is_added() && session.is_none() {
        do_save(&mut game, &progress, &play, &zone, &player, &mut toast);
    }
}

/// Install the persistent party/bag when a battle starts, mirror its changes every frame, and
/// finish (counters, prize money, caught Pokemon, autosave) once the session is gone.
#[allow(clippy::too_many_arguments)]
fn battle_wire(
    session: Option<ResMut<ActiveBattleSession>>,
    assets: Option<Res<BattleAssets>>,
    mut progress: ResMut<PlayerProgress>,
    mut tracker: ResMut<BattleTracker>,
    mut game: ResMut<SaveGame>,
    settings: Res<WireSettings>,
    play: Res<PlayTime>,
    zone: Res<ActiveZoneInfo>,
    player: Query<&Transform, With<PlayerAvatar>>,
    mut toast: ResMut<SaveToast>,
) {
    match (session, tracker.started) {
        (Some(mut sess), false) => {
            let Some(assets) = assets else { return };
            let Ok((roster, blackout)) = install_party(&mut sess, &assets.data, &progress.party) else {
                return;
            };
            if blackout {
                toast.show("All Pokemon had fainted: party restored.");
            }
            sess.bag = engine_bag(&progress.bag, &assets.items);
            sess.rng = XorShift32(progress.rng_seed.max(1));
            note_encounter(&mut progress, sess.wild_dex);
            tracker.before = progress.party.clone();
            tracker.bag_ids = bag_candidates(&sess, &progress.bag);
            tracker.roster = roster;
            tracker.started = true;
            let out = snapshot(&sess, &tracker.roster, &tracker.bag_ids);
            apply_sync(&mut progress, &out);
            tracker.last = Some(out);
        }
        (Some(sess), true) => {
            let out = snapshot(&sess, &tracker.roster, &tracker.bag_ids);
            apply_sync(&mut progress, &out);
            tracker.last = Some(out);
        }
        (None, true) => {
            tracker.started = false;
            if let Some(out) = tracker.last.take() {
                apply_outcome(&mut progress, &out);
            }
            let ok = do_save(&mut game, &progress, &play, &zone, &player, &mut toast);
            if ok && settings.selftest {
                run_selftest(&game, &progress, &tracker.before);
            }
        }
        _ => {}
    }
}

/// In-process "restart": read the file back with a fresh handle and log before/after HP.
fn run_selftest(game: &SaveGame, progress: &PlayerProgress, before: &[PartyMember]) {
    let fresh = SaveFile { path: game.file.path.clone() };
    let loaded = fresh.load_with_fallback().map(|l| PlayerProgress::from_save(&l.save));
    let text = match loaded {
        Ok(p) => selftest_report(before, &progress.party, &p.party),
        Err(e) => format!("[save-selftest] reload failed: {e}"),
    };
    println!("{text}");
    append_log(game.file.path.parent().unwrap_or(Path::new(".")), &text);
}

fn append_log(dir: &Path, text: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("selftest.log")) {
        let _ = writeln!(f, "{text}");
    }
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::save_game::{SavedMove, SavedStatus};

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pla-savewire-test-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn member(species: u16, hp: u32, pp: u8) -> PartyMember {
        PartyMember {
            species,
            form: 0,
            level: 20,
            hp,
            status: None,
            moves: vec![SavedMove { move_id: 33, pp }],
            effort: [0; 6],
            nickname: Some("Nick".into()),
        }
    }

    fn progress2() -> PlayerProgress {
        let mut p = PlayerProgress::new_game();
        p.party = vec![member(25, 50, 20), member(399, 30, 20)];
        p.bag = BTreeMap::from([(10, 5), (11, 2)]);
        p
    }

    #[test]
    fn party_changes_persist_and_fainted_stay_fainted() {
        let mut p = progress2();
        let mut hurt = member(25, 12, 7);
        hurt.status = Some(SavedStatus::Burn);
        let out = BattleOutcome {
            party: vec![(0, hurt), (1, member(399, 0, 20))],
            bag: BTreeMap::new(),
            wild_dex: 396,
            wild_level: 17,
            won: true,
            caught: None,
            rng: 77,
        };
        apply_outcome(&mut p, &out);
        assert_eq!(p.party[0].hp, 12);
        assert_eq!(p.party[0].moves[0].pp, 7);
        assert_eq!(p.party[0].status, Some(SavedStatus::Burn));
        assert_eq!(p.party[0].nickname.as_deref(), Some("Nick"), "nickname kept");
        assert_eq!(p.party[1].hp, 0, "fainted stays fainted");
        assert_eq!(p.rng_seed, 77);
        // survives the file
        let dir = tmpdir("party");
        let mut game = SaveGame::new(SaveFile::in_dir(&dir));
        assert!(game.save(p.to_save("ha_area01", [1.0, 2.0, 3.0], 0.5)));
        let mut game2 = SaveGame::new(SaveFile::in_dir(&dir));
        let st = load_startup(&mut game2, &WireCli::default());
        assert!(st.loaded);
        assert_eq!(st.progress.party, p.party);
        assert_eq!(st.area, Some(1));
        assert_eq!(st.position, Some([1.0, 2.0, 3.0]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bag_quantities_follow_use_and_zero_removes() {
        let mut p = progress2();
        let out = BattleOutcome {
            party: vec![],
            bag: BTreeMap::from([(10, 4), (11, 0), (12, 0)]),
            wild_dex: 1,
            wild_level: 5,
            won: false,
            caught: None,
            rng: 0,
        };
        apply_outcome(&mut p, &out);
        assert_eq!(p.bag, BTreeMap::from([(10, 4)]));
        assert_eq!(p.rng_seed, 0x5ca1_e001, "zero rng does not overwrite the seed");
    }

    #[test]
    fn sets_and_counters_update_for_win_catch_and_run() {
        let mut p = progress2();
        note_encounter(&mut p, 396);
        assert!(p.seen.contains(&396));
        let base = BattleOutcome {
            party: vec![],
            bag: BTreeMap::new(),
            wild_dex: 396,
            wild_level: 17,
            won: true,
            caught: None,
            rng: 1,
        };
        apply_outcome(&mut p, &base);
        assert_eq!((p.defeated, p.caught_count), (1, 0));
        assert_eq!(p.money, victory_reward(17), "a win pays the provisional prize");
        let ran = BattleOutcome { won: false, ..base.clone() };
        apply_outcome(&mut p, &ran);
        assert_eq!((p.defeated, p.caught_count), (1, 0), "running changes no counter");
        assert_eq!(p.money, victory_reward(17), "running pays nothing");
        let catch = BattleOutcome { won: false, caught: Some(member(396, 9, 5)), ..base.clone() };
        apply_outcome(&mut p, &catch);
        assert_eq!(p.caught_count, 1);
        assert!(p.caught.contains(&396));
        assert_eq!(p.money, victory_reward(17), "catching pays nothing");
        assert_eq!(p.party.len(), 3, "caught goes to the party when there is room");
        for _ in 0..4 {
            apply_outcome(&mut p, &catch);
        }
        assert_eq!(p.party.len(), MAX_PARTY);
        apply_outcome(&mut p, &catch);
        assert_eq!(p.box_list.len(), 2, "full party sends the catch to the box");
        p.add_money(u32::MAX);
        p.add_money(5);
        assert_eq!(p.money, u32::MAX);
    }

    #[test]
    fn victory_prize_saturates_and_grows_with_level() {
        let mut p = progress2();
        p.money = u32::MAX - 10;
        let out = BattleOutcome {
            party: vec![],
            bag: BTreeMap::new(),
            wild_dex: 25,
            wild_level: 100,
            won: true,
            caught: None,
            rng: 1,
        };
        apply_outcome(&mut p, &out);
        assert_eq!(p.money, u32::MAX, "the prize saturates instead of overflowing");
        assert!(victory_reward(1) > 0);
        assert!(victory_reward(50) > victory_reward(49), "the prize grows with the wild level");
        assert_eq!(victory_reward(0), 50, "ASSUMED formula: 50 + 10 x level");
    }

    #[test]
    fn play_seconds_survive_the_save_load_round_trip() {
        let p = progress2();
        let dir = tmpdir("playtime");
        let mut game = SaveGame::new(SaveFile::in_dir(&dir));
        let mut save = p.to_save("ha_area02", [0.0; 3], 0.0);
        save.play_seconds = 4242;
        assert!(game.save(save));
        let mut game2 = SaveGame::new(SaveFile::in_dir(&dir));
        let st = load_startup(&mut game2, &WireCli::default());
        assert!(st.loaded);
        assert_eq!(st.play_seconds, 4242);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn full_party_catch_goes_to_box_and_survives_the_file() {
        let mut p = progress2();
        p.party = (0..MAX_PARTY).map(|i| member(25 + i as u16, 10, 5)).collect();
        let catch = BattleOutcome {
            party: vec![],
            bag: BTreeMap::new(),
            wild_dex: 396,
            wild_level: 17,
            won: false,
            caught: Some(member(396, 9, 5)),
            rng: 1,
        };
        apply_outcome(&mut p, &catch);
        assert_eq!(p.party.len(), MAX_PARTY, "party stays full");
        assert_eq!(p.box_list.len(), 1, "the catch went to the box");
        assert_eq!(p.box_list[0].species, 396);
        // format v4 writes it and it reloads identical
        let dir = tmpdir("boxroute");
        let mut game = SaveGame::new(SaveFile::in_dir(&dir));
        assert!(game.save(p.to_save("ha_area01", [1.0, 2.0, 3.0], 0.5)));
        let mut game2 = SaveGame::new(SaveFile::in_dir(&dir));
        let st = load_startup(&mut game2, &WireCli::default());
        assert!(st.loaded);
        assert_eq!(st.progress.party, p.party);
        assert_eq!(st.progress.box_list, p.box_list);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn box_deposit_withdraw_swap_and_limits() {
        let mut p = progress2();
        let idx = p.deposit(member(396, 9, 5)).unwrap();
        assert_eq!(idx, 0);
        let taken = p.withdraw(0).unwrap();
        assert_eq!(taken.species, 396);
        assert!(p.box_list.is_empty() && p.party.len() == 3);
        assert!(matches!(p.withdraw(0), Err(BoxError::BadBoxSlot)));
        // swap party[0] <-> box[0]
        p.deposit(member(111, 1, 1)).unwrap();
        let party0 = p.party[0].species;
        p.swap(0, 0).unwrap();
        assert_eq!(p.party[0].species, 111);
        assert_eq!(p.box_list[0].species, party0);
        assert!(matches!(p.swap(9, 0), Err(BoxError::BadBoxSlot)));
        assert!(matches!(p.swap(0, 9), Err(BoxError::BadPartySlot)));
        // withdrawing into a full party is refused and nothing moves
        p.party = (0..MAX_PARTY).map(|i| member(25 + i as u16, 10, 5)).collect();
        p.deposit(member(222, 1, 1)).unwrap();
        let last = p.box_list.len() - 1;
        assert!(matches!(p.withdraw(last), Err(BoxError::PartyFull)));
        assert_eq!(p.box_list.len(), 2);
    }

    #[test]
    fn full_box_catch_is_counted_but_not_stored() {
        let mut p = progress2();
        p.party = (0..MAX_PARTY).map(|i| member(25 + i as u16, 10, 5)).collect();
        p.box_list = (0..MAX_BOX).map(|i| member(1 + (i as u16 % 800), 1, 1)).collect();
        let catch = BattleOutcome {
            party: vec![],
            bag: BTreeMap::new(),
            wild_dex: 396,
            wild_level: 17,
            won: false,
            caught: Some(member(396, 9, 5)),
            rng: 1,
        };
        apply_outcome(&mut p, &catch);
        assert_eq!(p.box_list.len(), MAX_BOX, "the box stays at the limit");
        assert!(p.caught.contains(&396), "the catch is still recorded");
        assert_eq!(p.caught_count, 1);
    }

    #[test]
    fn startup_new_game_ignores_existing_save_and_missing_save_is_reported() {
        let dir = tmpdir("startup");
        let mut game = SaveGame::new(SaveFile::in_dir(&dir));
        let st = load_startup(&mut game, &WireCli::default());
        assert!(!st.loaded);
        assert!(st.message.to_lowercase().contains("no saved game"), "{}", st.message);
        assert!(game.save(progress2().to_save("ha_area02", [0.0; 3], 0.0)));
        let mut again = SaveGame::new(SaveFile::in_dir(&dir));
        let cli = WireCli { new_game: true, ..WireCli::default() };
        let st = load_startup(&mut again, &cli);
        assert!(!st.loaded && st.progress.party.is_empty());
        let mut again = SaveGame::new(SaveFile::in_dir(&dir));
        assert_eq!(load_startup(&mut again, &WireCli::default()).area, Some(2));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn damaged_main_file_restores_backup_with_message() {
        let dir = tmpdir("damaged");
        let mut game = SaveGame::new(SaveFile::in_dir(&dir));
        assert!(game.save(progress2().to_save("ha_area01", [0.0; 3], 0.0)));
        assert!(game.save(progress2().to_save("ha_area01", [1.0, 0.0, 0.0], 0.0))); // creates .bak
        std::fs::write(dir.join("save.plasave"), "garbage").unwrap();
        let mut g = SaveGame::new(SaveFile::in_dir(&dir));
        let st = load_startup(&mut g, &WireCli::default());
        assert!(st.loaded);
        assert!(st.message.contains("backup"), "{}", st.message);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cli_and_zone_parsing() {
        let a: Vec<String> =
            ["--new-game", "--save-dir", "/x/y", "--area", "3"].iter().map(|s| s.to_string()).collect();
        let c = WireCli::from_args(&a);
        assert!(c.new_game && c.has_area && !c.has_time_slot);
        assert_eq!(c.save_dir, Some(PathBuf::from("/x/y")));
        assert_eq!(parse_zone(&zone_id(7)), Some(7));
        assert_eq!(parse_zone("nonsense"), None);
    }

    #[test]
    fn selftest_report_flags_mismatch() {
        let a = vec![member(25, 10, 5)];
        let b = vec![member(25, 11, 5)];
        assert!(selftest_report(&a, &a, &a).ends_with("true"));
        assert!(selftest_report(&a, &a, &b).ends_with("false"));
    }

    // ---- real engine (skipped when the checked-in tables are not found)

    fn assets() -> Option<BattleAssets> {
        BattleAssets::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).ok()
    }

    fn fresh(a: &BattleAssets) -> ActiveBattleSession {
        ActiveBattleSession::start(a, "pikachu", 25, "bidoof", 5, 399, Entity::PLACEHOLDER).unwrap()
    }

    #[test]
    fn real_battle_round_trip_party_bag_and_file() {
        let Some(a) = assets() else { return };
        let mut p = PlayerProgress::new_game();
        p.party = default_party(&a);
        p.bag = default_bag(&a.items);
        assert_eq!(p.party.len(), 3);
        assert!(!p.bag.is_empty());
        // bench member 1 starts fainted; lead is hurt
        p.party[1].hp = 0;
        let lead_before = p.party[0].hp;
        let mut s = fresh(&a);
        let (roster, blackout) = install_party(&mut s, &a.data, &p.party).unwrap();
        assert!(!blackout);
        assert_eq!(roster, vec![0, 1, 2]);
        assert_eq!(s.battle.combatants[0].hp, lead_before);
        assert_eq!(s.battle.combatants[combatant_index(1)].hp, 0, "fainted member restored as fainted");
        s.bag = engine_bag(&p.bag, &a.items);
        let ids = bag_candidates(&s, &p.bag);
        // use a ball, lower HP and PP
        let ball = s.bag_order.iter().find(|e| e.kind == battle_items::ItemKind::Ball).unwrap().item_id;
        let balls_before = s.bag.count(ball);
        s.battle.combatants[0].hp = lead_before.saturating_sub(7).max(1);
        s.battle.combatants[0].moves[0].pp -= 1;
        s.battle.combatants[0].status = Some(crate::battle_engine::MajorStatus::Burn);
        assert!(s.throw_ball(ball));
        let out = snapshot(&s, &roster, &ids);
        apply_outcome(&mut p, &out);
        assert_eq!(p.bag.get(&ball).copied().unwrap_or(0), balls_before - 1);
        assert!(p.party[0].hp < lead_before + 1);
        assert_eq!(p.party[0].status, Some(SavedStatus::Burn));
        assert_eq!(p.party[1].hp, 0);
        // file round trip, then a second battle starts from the loaded state
        let dir = tmpdir("real");
        let mut game = SaveGame::new(SaveFile::in_dir(&dir));
        assert!(game.save(p.to_save("ha_area01", [5.0, 1.0, 5.0], 1.0)));
        let mut g2 = SaveGame::new(SaveFile::in_dir(&dir));
        let loaded = load_startup(&mut g2, &WireCli::default()).progress;
        assert_eq!(loaded.party, p.party);
        assert_eq!(loaded.bag, p.bag);
        assert_eq!(loaded.seen, p.seen);
        let mut s2 = fresh(&a);
        install_party(&mut s2, &a.data, &loaded.party).unwrap();
        assert_eq!(s2.battle.combatants[0].hp, loaded.party[0].hp);
        assert_eq!(s2.battle.combatants[0].moves[0].pp, loaded.party[0].moves[0].pp);
        assert_eq!(s2.battle.combatants[0].status, Some(crate::battle_engine::MajorStatus::Burn));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fainted_lead_is_skipped_and_full_wipe_heals() {
        let Some(a) = assets() else { return };
        let mut party = default_party(&a);
        party[0].hp = 0;
        let mut s = fresh(&a);
        install_party(&mut s, &a.data, &party).unwrap();
        assert_eq!(s.active_player(), combatant_index(1), "first living member leads");
        for m in &mut party {
            m.hp = 0;
        }
        let mut s = fresh(&a);
        let (_, blackout) = install_party(&mut s, &a.data, &party).unwrap();
        assert!(blackout);
        assert!(s.battle.combatants[0].hp > 0);
    }

    #[test]
    fn real_catch_adds_to_caught_and_party() {
        let Some(a) = assets() else { return };
        let mut p = PlayerProgress::new_game();
        p.party = default_party(&a);
        let mut s = fresh(&a);
        let (roster, _) = install_party(&mut s, &a.data, &p.party).unwrap();
        s.battle.combatants[WILD].caught = true;
        s.battle.ended = true;
        let out = snapshot(&s, &roster, &[]);
        assert!(out.caught.is_some() && !out.won);
        apply_outcome(&mut p, &out);
        assert!(p.caught.contains(&399) && p.seen.contains(&399));
        assert_eq!(p.party.len(), 4);
        assert_eq!(p.caught_count, 1);
    }

    #[test]
    fn real_catch_with_full_party_lands_in_the_box() {
        let Some(a) = assets() else { return };
        let mut p = PlayerProgress::new_game();
        p.party = default_party(&a);
        while p.party.len() < MAX_PARTY {
            p.party.push(p.party[0].clone());
        }
        let mut s = fresh(&a);
        let (roster, _) = install_party(&mut s, &a.data, &p.party).unwrap();
        assert_eq!(roster.len(), MAX_PARTY);
        s.battle.combatants[WILD].caught = true;
        s.battle.ended = true;
        let out = snapshot(&s, &roster, &[]);
        apply_outcome(&mut p, &out);
        assert_eq!(p.party.len(), MAX_PARTY);
        assert_eq!(p.box_list.len(), 1, "the caught Pokemon went to the box");
        assert_eq!(p.box_list[0].species, 399);
        // and it survives a save/reload with a 6-member party plus the box
        let dir = tmpdir("realbox");
        let mut game = SaveGame::new(SaveFile::in_dir(&dir));
        assert!(game.save(p.to_save("ha_area01", [0.0; 3], 0.0)));
        let mut g2 = SaveGame::new(SaveFile::in_dir(&dir));
        let loaded = load_startup(&mut g2, &WireCli::default()).progress;
        assert_eq!(loaded.party, p.party);
        assert_eq!(loaded.box_list, p.box_list);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn win_pays_the_prize_through_the_real_engine_snapshot() {
        let Some(a) = assets() else { return };
        let mut p = PlayerProgress::new_game();
        p.party = default_party(&a);
        let mut s = fresh(&a);
        let (roster, _) = install_party(&mut s, &a.data, &p.party).unwrap();
        let out = snapshot(&s, &roster, &[]);
        assert!(!out.won, "the wild is still standing at battle start");
        assert!(out.wild_level > 0, "the wild level travels in the outcome");
        s.battle.combatants[WILD].hp = 0;
        let out = snapshot(&s, &roster, &[]);
        assert!(out.won);
        apply_outcome(&mut p, &out);
        assert_eq!(p.money, victory_reward(out.wild_level));
    }

    #[test]
    fn rest_heals_hp_pp_and_status_and_keeps_form_and_nickname() {
        let Some(a) = assets() else { return };
        let mut party = default_party(&a);
        party[0].hp = 1;
        party[0].status = Some(SavedStatus::Burn);
        party[0].moves[0].pp = 0;
        party[0].nickname = Some("Lead".into());
        if party.len() > 1 {
            party[1].hp = 0;
        }
        let report = rest_party(&mut party, &a.data);
        assert_eq!(report.healed, party.len());
        assert_eq!(report.skipped, 0);
        assert!(party.iter().all(|m| m.hp > 0 && m.status.is_none()), "all HP restored, status cleared");
        assert!(party[0].moves.iter().all(|s| s.pp > 0), "all PP restored");
        assert_eq!(party[0].nickname.as_deref(), Some("Lead"), "nickname kept");
        assert!(report.message().contains("fully healed"));
        // A second rest changes nothing (idempotent).
        let hp = party[0].hp;
        let report = rest_party(&mut party, &a.data);
        assert_eq!((report.healed, party[0].hp), (party.len(), hp));
    }

    #[test]
    fn rest_skips_members_with_unknown_species() {
        let Some(a) = assets() else { return };
        let mut party = vec![member(65_000, 10, 5)];
        let report = rest_party(&mut party, &a.data);
        assert_eq!((report.healed, report.skipped), (0, 1));
        assert_eq!(party[0].hp, 10, "the member is left untouched");
        assert!(report.message().contains("could not be rebuilt"));
    }

    #[test]
    fn rest_request_is_consumed_and_reported_without_tables() {
        let mut app = App::new();
        app.init_resource::<PlayerProgress>()
            .init_resource::<RestRequested>()
            .init_resource::<SaveToast>();
        app.add_systems(Update, rest_action);
        assert!(!std::mem::take(&mut app.world_mut().resource_mut::<RestRequested>().0), "nothing asked yet");
        app.world_mut().resource_mut::<RestRequested>().0 = true;
        app.update();
        assert!(!app.world().resource::<RestRequested>().0, "the request is consumed");
        assert!(app.world().resource::<SaveToast>().text.contains("battle tables"));
    }

    #[test]
    fn rest_action_heals_the_party_through_the_toast() {
        let Some(a) = assets() else { return };
        let mut app = App::new();
        app.init_resource::<PlayerProgress>()
            .init_resource::<RestRequested>()
            .init_resource::<SaveToast>();
        {
            let mut progress = app.world_mut().resource_mut::<PlayerProgress>();
            progress.party = default_party(&a);
            progress.party[0].hp = 1;
        }
        app.insert_resource(a);
        app.add_systems(Update, rest_action);
        app.world_mut().resource_mut::<RestRequested>().0 = true;
        app.update();
        assert!(app.world().resource::<PlayerProgress>().party[0].hp > 1);
        assert!(app.world().resource::<SaveToast>().text.contains("healed"));
    }
}
