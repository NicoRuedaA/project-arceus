//! Turn-based battle controller connecting the Bevy game to `pla::battle_engine`.
//!
//! The session owns the engine `Battle`, a small party, the item bag and the log. All menu and
//! turn logic lives here as plain functions on `ActiveBattleSession` so it is unit-testable
//! without Bevy systems; `battle_input` only maps keys to `BattleInput`.

use std::path::Path;
use std::sync::Arc;

use bevy::prelude::*;

use super::battle_ai::choose_ai_action;
use super::battle_items::{self, BagEntry, ItemKind};
use super::battle_log::{self, tail};
pub use super::catch_rate::{CatchRateSource, CatchRateTable, CAPTURE_CATCH_RATE_PLACEHOLDER};
use crate::battle::BattleData;
use crate::battle_engine::effects::{MoveEffectsTable, Stages};
use crate::battle_engine::items::{ItemBag, ItemTable};
use crate::battle_engine::statuses::StatusRules;
use crate::battle_engine::{
    capture_index, capture_probability, Action, Battle, BattleRng, CaptureModifiers, Combatant,
    Event, MajorStatus, XorShift32,
};

/// Index of the wild combatant in `Battle::combatants` (side 1).
pub const WILD: usize = 1;
/// Slots shown at once in the move / item / party grid.
pub const GRID_SLOTS: usize = 4;

/// Menu selection state inside battle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleMenuState {
    /// Fight view: the four moves of the active Pokemon.
    MainMenu,
    MoveSelect,
    ItemSelect,
    SwitchSelect,
    ExecutingTurn,
}

/// Abstract player input (keys are mapped in `battle_input`; the demo script uses these too).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleInput {
    /// Digit key 1-4 (stored as 0-3): pick a move / list entry.
    Slot(usize),
    Prev,
    Next,
    Confirm,
    Back,
    OpenFight,
    OpenSwitch,
    OpenItem,
    /// Throw the first ball in the bag.
    Capture,
    Run,
}

/// Data tables needed to start a battle with the data-driven features.
#[derive(Resource, Clone)]
pub struct BattleAssets {
    pub data: Arc<BattleData>,
    pub move_effects: MoveEffectsTable,
    pub items: ItemTable,
    pub catch_rates: CatchRateTable,
    /// Battle rules applied when a session starts. Defaults to `StatusRules::game_rules()`
    /// (confusion, trapping, charge turns, Protect, drowsiness, the game's status rates and rank
    /// multipliers). `notes_rates` in it also selects the VERIFIED PLA damage formula, so in-game
    /// damage is roughly three times the old main-series numbers. Set it to
    /// `StatusRules::default()` to turn every rule off (legacy formula).
    pub status_rules: StatusRules,
}

impl BattleAssets {
    pub fn load(data_root: impl AsRef<Path>) -> Result<Self, String> {
        let root = data_root.as_ref();
        let catch_rates = CatchRateTable::load(root).unwrap_or_else(|e| {
            eprintln!("catch rates unavailable ({e}); using the placeholder rate");
            CatchRateTable::default()
        });
        Ok(Self {
            data: Arc::new(BattleData::load(root)?),
            move_effects: MoveEffectsTable::load(root)?,
            items: ItemTable::load(root)?,
            catch_rates,
            status_rules: StatusRules::game_rules(),
        })
    }
}

/// Move loadout (real move names from `pml_moves`); unknown species get a plain default.
pub fn moveset(species: &str) -> &'static [&'static str] {
    match species {
        "pikachu" => &["thunderbolt", "quick-attack", "thunder-wave", "iron-tail"],
        "bidoof" => &["tackle", "bite", "growl", "take-down"],
        "starly" => &["wing-attack", "quick-attack", "growl", "sand-attack"],
        "shinx" => &["tackle", "leer", "bite", "spark"],
        _ => &["tackle", "quick-attack"],
    }
}

/// Wild-side loadout; a few species have their own, the rest use the plain default.
fn wild_moveset(species: &str) -> &'static [&'static str] {
    match species {
        "pikachu" => &["spark", "quick-attack", "thunder-wave", "growl"],
        "bidoof" => &["tackle", "bite", "growl"],
        "starly" => &["quick-attack", "wing-attack", "growl"],
        "shinx" => &["tackle", "leer", "bite", "spark"],
        _ => &["tackle"],
    }
}

const BENCH_CANDIDATES: [&str; 4] = ["bidoof", "starly", "shinx", "pikachu"];

fn build_combatant(
    data: &BattleData,
    species: &str,
    level: u8,
    names: &[&str],
) -> Result<Combatant, String> {
    let spec = data
        .species_row(species)?
        .ok_or_else(|| format!("unknown species {species}"))?;
    let mut moves = Vec::new();
    for n in names {
        if let Some(m) = data.move_row(n)? {
            moves.push(m);
        }
    }
    if moves.is_empty() {
        moves.push(
            data.move_row("tackle")?
                .ok_or("move table has no tackle")?,
        );
    }
    Ok(Combatant::from_base(spec, level, moves))
}

/// What the capture would do, computed with the same inputs `attempt_capture` uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CapturePreview {
    pub probability: f64,
    /// Checks passed in order (0, 1 or 2); 2 means caught.
    pub checks_passed: u8,
}

/// Replays the two capture checks on a copy of the RNG so the log can show the shakes.
/// Mirrors `Battle::attempt_capture` (status, ball, rank, context factors).
pub fn capture_preview(
    battle: &Battle,
    data: &BattleData,
    ball_category: &str,
    target: usize,
    catch_rate: u8,
    rng: &XorShift32,
) -> Result<CapturePreview, String> {
    let c = battle.combatants.get(target).ok_or("capture target out of range")?;
    let ball = data.capture_ball_multiplier(ball_category, true)?;
    let status = data.capture_status_multiplier(match c.status {
        None => "None",
        Some(MajorStatus::Paralysis) => "Paralysis",
        Some(MajorStatus::Poison) => "Poison",
        Some(MajorStatus::Burn) => "Burn",
        Some(MajorStatus::Drowsy) => "Drowsiness / Sleep",
        Some(MajorStatus::Frostbite) => "Frostbite / Freeze",
    })?;
    let context = data
        .capture_factor("capture_index_backstrike_failure_factor")?
        .unwrap_or(1.0);
    let rank = data.capture_level_rank_adjustment(1, 1)?;
    let ratio = c.hp as f64 / c.max_hp.max(1) as f64;
    let index = capture_index(catch_rate, ratio, rank, status, ball, context, CaptureModifiers::default());
    let probability = capture_probability(index);
    let failure = (1.0 - probability.clamp(0.0, 1.0)).sqrt();
    let mut r = rng.clone();
    let mut passed = 0;
    for _ in 0..2 {
        if f64::from(r.below(65_536)) / 65_536.0 < failure {
            break;
        }
        passed += 1;
    }
    Ok(CapturePreview {
        probability,
        checks_passed: passed,
    })
}

#[derive(Resource)]
pub struct ActiveBattleSession {
    pub battle: Battle,
    pub wild_entity: Entity,
    pub wild_dex: u16,
    pub rng: XorShift32,
    pub menu_state: BattleMenuState,
    pub selected_menu_idx: usize,
    pub selected_move_idx: usize,
    pub event_log: Vec<Event>,
    pub current_turn: u32,
    /// Newest log lines (what the battle box shows).
    pub battle_summary: String,
    pub turn_timer: f32,
    /// Every log line of this battle, oldest first.
    pub log_lines: Vec<String>,
    pub bag: ItemBag,
    pub bag_order: Vec<BagEntry>,
    /// Cursor inside the item or party list.
    pub list_cursor: usize,
    pub catch_rate: u8,
    pub catch_rate_source: CatchRateSource,
    data: Option<Arc<BattleData>>,
}

impl ActiveBattleSession {
    /// Plain start without the data-driven tables (no move effects, no bag, placeholder catch
    /// rate, single-member party). Kept for callers that only have a `BattleData`.
    pub fn new(
        battle_data: &BattleData,
        player_species: &str,
        player_level: u8,
        wild_species: &str,
        wild_level: u8,
        wild_dex: u16,
        wild_entity: Entity,
    ) -> Result<Self, String> {
        Self::build(
            battle_data,
            None,
            player_species,
            player_level,
            wild_species,
            wild_level,
            wild_dex,
            wild_entity,
        )
    }

    /// Full start: move effects, item table and starter bag, real catch rate, 3-member party,
    /// engine-AI opponent.
    pub fn start(
        assets: &BattleAssets,
        player_species: &str,
        player_level: u8,
        wild_species: &str,
        wild_level: u8,
        wild_dex: u16,
        wild_entity: Entity,
    ) -> Result<Self, String> {
        Self::build(
            &assets.data,
            Some(assets),
            player_species,
            player_level,
            wild_species,
            wild_level,
            wild_dex,
            wild_entity,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        data: &BattleData,
        assets: Option<&BattleAssets>,
        player_species: &str,
        player_level: u8,
        wild_species: &str,
        wild_level: u8,
        wild_dex: u16,
        wild_entity: Entity,
    ) -> Result<Self, String> {
        let lead = build_combatant(data, player_species, player_level, moveset(player_species))?;
        let wild = build_combatant(data, wild_species, wild_level, wild_moveset(wild_species))?;
        let mut combatants = vec![lead, wild];
        let mut player_side = vec![0];
        if assets.is_some() {
            let mut level = player_level;
            for sp in BENCH_CANDIDATES.iter().filter(|s| **s != player_species) {
                if player_side.len() >= 3 {
                    break;
                }
                level = level.saturating_sub(if player_side.len() == 1 { 8 } else { 3 }).max(2);
                if let Ok(c) = build_combatant(data, sp, level, moveset(sp)) {
                    player_side.push(combatants.len());
                    combatants.push(c);
                }
            }
        }
        let mut battle = Battle::new(
            combatants,
            vec![player_side, vec![WILD]],
            data.type_chart().clone(),
        )?;
        let mut bag = ItemBag::default();
        let mut bag_order = Vec::new();
        let (catch_rate, catch_rate_source) = match assets {
            Some(a) => {
                battle.set_move_effects(a.move_effects.clone());
                battle.set_item_table(a.items.clone());
                battle.set_status_rules(a.status_rules);
                (bag, bag_order) = battle_items::starter_bag(&a.items);
                a.catch_rates.lookup(wild_species)
            }
            None => (CAPTURE_CATCH_RATE_PLACEHOLDER, CatchRateSource::Placeholder),
        };
        let mut s = Self {
            battle,
            wild_entity,
            wild_dex,
            rng: XorShift32(0x5ca1_e001),
            menu_state: BattleMenuState::MainMenu,
            selected_menu_idx: 0,
            selected_move_idx: 0,
            event_log: Vec::new(),
            current_turn: 0,
            battle_summary: String::new(),
            turn_timer: 0.0,
            log_lines: Vec::new(),
            bag,
            bag_order,
            list_cursor: 0,
            catch_rate,
            catch_rate_source,
            data: assets.map(|a| a.data.clone()),
        };
        s.push_lines(vec!["Battle Started!".into()]);
        Ok(s)
    }

    // ------------------------------------------------------------------ helpers

    pub fn active_player(&self) -> usize {
        self.battle.active_combatant(0).unwrap_or(0)
    }

    pub fn display_name(&self, i: usize) -> String {
        let n = self.battle.combatants[i].species.name.to_uppercase();
        if i == WILD {
            format!("Wild {n}")
        } else {
            n
        }
    }

    fn names(&self) -> Vec<String> {
        (0..self.battle.combatants.len())
            .map(|i| self.display_name(i))
            .collect()
    }

    /// Drain any queued volatile-status events (e.g. a refused switch) into log lines. Turn
    /// resolution already drains them; this is for engine calls made outside a turn.
    fn take_status_lines(&mut self) -> Vec<String> {
        let names = self.names();
        let events = self.battle.take_status_events();
        events.iter().filter_map(|e| battle_log::format_status(&|i| names[i].clone(), e)).collect()
    }

    pub fn push_lines(&mut self, lines: Vec<String>) {
        self.log_lines.extend(lines);
        self.battle_summary = tail(&self.log_lines);
    }

    pub fn stages_of(&self, i: usize) -> Stages {
        self.battle.stages(i)
    }

    /// Visible bag lines (entries with a quantity left), in menu order.
    pub fn bag_view(&self) -> Vec<(&BagEntry, u16)> {
        self.bag_order
            .iter()
            .map(|e| (e, self.bag.count(e.item_id)))
            .filter(|(_, n)| *n > 0)
            .collect()
    }

    fn party(&self) -> &[usize] {
        &self.battle.sides[0]
    }

    // ------------------------------------------------------------------ turns

    /// Resolve one engine turn. `player` is the player's action (None when the player used the
    /// turn on a switch, item or ball); the wild side always acts, chosen by the engine AI.
    fn resolve_turn(&mut self, player: Option<Action>) -> Vec<Event> {
        let before_active = self.active_player();
        let mut actions: Vec<Action> = player.into_iter().collect();
        if self.battle.combatants[WILD].is_alive() {
            let wild_action = match &self.data {
                Some(data) => choose_ai_action(&self.battle, data, WILD, before_active, &mut self.rng),
                None => {
                    let n = self.battle.combatants[WILD].moves.len().max(1);
                    Action {
                        actor: WILD,
                        move_slot: (self.rng.0 as usize) % n,
                    }
                }
            };
            actions.push(wild_action);
        }
        let status_moves: Vec<(usize, String)> = actions
            .iter()
            .filter_map(|a| {
                let m = self.battle.combatants[a.actor].moves.get(a.move_slot)?;
                (m.data.power == 0 && m.pp > 0).then(|| (a.actor, m.data.name.clone()))
            })
            .collect();
        let events = self.battle.play_turn(&actions, &mut self.rng);
        let effects = self.battle.take_effect_events();
        let statuses = self.battle.take_status_events();
        let rules = self.battle.take_rule_events();
        let names = self.names();
        let mut lines = battle_log::format_turn(
            &|i| names[i].clone(),
            &events,
            &effects,
            &statuses,
            &rules,
            &status_moves,
        );
        let after_active = self.active_player();
        if after_active != before_active && !self.battle.ended {
            let at = lines
                .iter()
                .rposition(|l| l.ends_with("fainted!"))
                .map_or(lines.len(), |p| p + 1);
            lines.insert(at, format!("Go! {}!", names[after_active]));
            self.selected_move_idx = 0;
        }
        self.event_log.extend(events.clone());
        self.current_turn = self.battle.turn;
        self.push_lines(lines);
        self.menu_state = BattleMenuState::MainMenu;
        self.selected_menu_idx = 0;
        events
    }

    /// The player attacks with `move_slot`; the wild Pokemon answers. Returns the turn events
    /// (empty when the choice is refused, e.g. no PP left, and no turn passes).
    pub fn execute_player_attack(&mut self, move_slot: usize) -> Vec<Event> {
        if self.battle.ended {
            return Vec::new();
        }
        let actor = self.active_player();
        let Some(slot) = self.battle.combatants[actor].moves.get(move_slot) else {
            return Vec::new();
        };
        if slot.pp == 0 {
            let msg = format!("{} has no PP left!", slot.data.name);
            self.push_lines(vec![msg]);
            return Vec::new();
        }
        self.resolve_turn(Some(Action { actor, move_slot }))
    }

    /// Use a bag item (medicine or ball). Returns true when a turn was spent.
    pub fn use_bag_item(&mut self, item_id: u16) -> bool {
        let Some(entry) = self.bag_order.iter().find(|e| e.item_id == item_id).cloned() else {
            return false;
        };
        if self.bag.count(item_id) == 0 {
            self.push_lines(vec![format!("No {} left.", entry.label)]);
            return false;
        }
        match entry.kind {
            ItemKind::Ball => self.throw_ball(item_id),
            ItemKind::Heal | ItemKind::Cure | ItemKind::Revive => {
                let target = if entry.kind == ItemKind::Revive {
                    match self
                        .party()
                        .iter()
                        .copied()
                        .find(|&i| self.battle.combatants[i].hp == 0)
                    {
                        Some(t) => t,
                        None => {
                            self.push_lines(vec!["No fainted Pokemon to revive.".into()]);
                            return false;
                        }
                    }
                } else {
                    self.active_player()
                };
                let who = self.display_name(target);
                match self.battle.use_item_on(0, item_id, target, None) {
                    Ok(u) => {
                        self.bag.take(item_id);
                        let mut line = format!("Used {}: ", entry.label);
                        if u.revived {
                            line.push_str(&format!("{who} was revived with {} HP!", u.hp_restored));
                        } else {
                            let mut parts = Vec::new();
                            if u.hp_restored > 0 {
                                parts.push(format!("{who} regained {} HP", u.hp_restored));
                            }
                            if let Some(s) = u.status_cured {
                                parts.push(format!("{who} was cured of {}", battle_items::status_tag(s)));
                            }
                            line.push_str(&parts.join(", "));
                            line.push('!');
                        }
                        self.push_lines(vec![line]);
                        self.resolve_turn(None);
                        true
                    }
                    Err(e) => {
                        self.push_lines(vec![format!("It would have no effect on {who}. ({e})")]);
                        false
                    }
                }
            }
            ItemKind::Other => {
                self.push_lines(vec![format!("{} can't be used in battle.", entry.label)]);
                false
            }
        }
    }

    /// Throw a ball item at the wild Pokemon. Returns true when a turn was spent.
    pub fn throw_ball(&mut self, item_id: u16) -> bool {
        let Some(data) = self.data.clone() else {
            self.push_lines(vec!["No capture data loaded.".into()]);
            return false;
        };
        if self.bag.count(item_id) == 0 {
            self.push_lines(vec!["No Poke Balls of that kind left.".into()]);
            return false;
        }
        let category = match self
            .battle
            .item_table()
            .and_then(|t| t.get(item_id))
            .and_then(|i| i.ball_category.clone())
        {
            Some(c) => c,
            None => {
                self.push_lines(vec![format!("Item #{item_id} is not a ball.")]);
                return false;
            }
        };
        let preview = capture_preview(&self.battle, &data, &category, WILD, self.catch_rate, &self.rng);
        let wild = self.display_name(WILD);
        let src = match self.catch_rate_source {
            CatchRateSource::GameData => "game data",
            CatchRateSource::Placeholder => "placeholder",
        };
        let result = self.battle.throw_ball_item(
            &data,
            item_id,
            WILD,
            true,
            1,
            1,
            self.catch_rate,
            false,
            CaptureModifiers::default(),
            &mut self.rng,
        );
        match result {
            Err(e) => {
                self.push_lines(vec![format!("The ball can't be thrown: {e}")]);
                false
            }
            Ok(caught) => {
                self.bag.take(item_id);
                let mut lines = vec![format!("You threw a {category} Ball at {wild}!")];
                if let Ok(p) = preview {
                    lines.push(format!(
                        "(catch rate {} [{src}], chance about {:.0}%)",
                        self.catch_rate,
                        p.probability * 100.0
                    ));
                    let shakes = if caught { 2 } else { (p.checks_passed + 1).min(2) };
                    lines.push(vec!["Shake..."; shakes as usize].join(" "));
                }
                if caught {
                    lines.push(format!("Gotcha! {wild} was caught!"));
                    self.battle.ended = true;
                    self.push_lines(lines);
                    self.menu_state = BattleMenuState::MainMenu;
                    true
                } else {
                    lines.push(format!("{wild} broke free!"));
                    self.push_lines(lines);
                    self.resolve_turn(None);
                    true
                }
            }
        }
    }

    /// Switch the active Pokemon to party member `target` (a combatant index of side 0).
    pub fn switch_to(&mut self, target: usize) -> bool {
        let old = self.active_player();
        if target == old {
            self.push_lines(vec![format!("{} is already out!", self.display_name(old))]);
            return false;
        }
        match self.battle.switch(0, target) {
            Ok(_) => {
                let (o, n) = (self.display_name(old), self.display_name(target));
                self.push_lines(vec![format!("{o}, come back!"), format!("Go! {n}!")]);
                self.selected_move_idx = 0;
                self.resolve_turn(None);
                true
            }
            Err(e) => {
                // A trap refusal queues a `CantSwitch` status event; show it with the engine error.
                let head = format!("{} can't be sent out: {e}", self.display_name(target));
                let mut lines = vec![head];
                lines.extend(self.take_status_lines());
                self.push_lines(lines);
                false
            }
        }
    }

    pub fn run_away(&mut self) {
        self.battle.ended = true;
        self.push_lines(vec!["Got away safely!".into()]);
    }

    /// Legacy capture entry kept for callers without the item bag (fixed Poke Ball, placeholder
    /// catch rate). The in-game input path uses `throw_ball` instead.
    pub fn attempt_capture_in_battle(&mut self, battle_data: &BattleData) -> Result<bool, String> {
        let res = self.battle.attempt_capture(
            battle_data,
            WILD,
            "Regular",
            true,
            1,
            1,
            self.catch_rate,
            false,
            CaptureModifiers::default(),
            &mut self.rng,
        )?;
        if res {
            self.battle.combatants[WILD].caught = true;
            self.battle.ended = true;
            self.push_lines(vec!["Wild Pokémon caught in Poké Ball!".into()]);
        } else {
            self.push_lines(vec!["Wild Pokémon broke free!".into()]);
        }
        Ok(res)
    }

    // ------------------------------------------------------------------ menus

    /// Which action tab is highlighted: 0 FIGHT, 1 SWITCH, 2 ITEM, 3 CAPTURE, 4 RUN.
    fn tab_for_state(&self) -> usize {
        match self.menu_state {
            BattleMenuState::SwitchSelect => 1,
            BattleMenuState::ItemSelect => 2,
            _ => self.selected_menu_idx.min(4),
        }
    }

    fn list_len(&self) -> usize {
        match self.menu_state {
            BattleMenuState::ItemSelect => self.bag_view().len(),
            BattleMenuState::SwitchSelect => self.party().len(),
            _ => self.battle.combatants[self.active_player()].moves.len(),
        }
    }

    /// Grid slot (0-3) to highlight.
    pub fn highlight_slot(&self) -> usize {
        match self.menu_state {
            BattleMenuState::ItemSelect | BattleMenuState::SwitchSelect => {
                self.list_cursor % GRID_SLOTS
            }
            _ => self.selected_move_idx,
        }
    }

    /// Title and detail text for each of the four grid slots in the current view.
    pub fn slot_texts(&self) -> [(String, String); GRID_SLOTS] {
        let empty = |k: usize| (format!("[{}] ---", k + 1), "Empty Slot".to_string());
        let mut out: [(String, String); GRID_SLOTS] = std::array::from_fn(empty);
        match self.menu_state {
            BattleMenuState::ItemSelect => {
                let view = self.bag_view();
                let page = self.list_cursor / GRID_SLOTS;
                for k in 0..GRID_SLOTS {
                    if let Some((e, n)) = view.get(page * GRID_SLOTS + k) {
                        out[k] = (format!("[{}] {}", k + 1, e.label), format!("x{n}"));
                    }
                }
            }
            BattleMenuState::SwitchSelect => {
                let page = self.list_cursor / GRID_SLOTS;
                for k in 0..GRID_SLOTS {
                    if let Some(&i) = self.party().get(page * GRID_SLOTS + k) {
                        let c = &self.battle.combatants[i];
                        let tag = c.status.map(|s| format!(" [{}]", battle_items::status_tag(s))).unwrap_or_default();
                        let state = if c.hp == 0 {
                            "FAINTED".to_string()
                        } else if i == self.active_player() {
                            format!("HP {}/{}{tag} (out)", c.hp, c.max_hp)
                        } else {
                            format!("HP {}/{}{tag}", c.hp, c.max_hp)
                        };
                        out[k] = (
                            format!("[{}] {} Lv{}", k + 1, c.species.name.to_uppercase(), c.level),
                            state,
                        );
                    }
                }
            }
            _ => {
                let c = &self.battle.combatants[self.active_player()];
                for k in 0..GRID_SLOTS {
                    if let Some(m) = c.moves.get(k) {
                        out[k] = (
                            format!("[{}] {}", k + 1, m.data.name),
                            format!("Type: {} | PP {}/{}", m.data.move_type, m.pp, m.data.pp),
                        );
                    }
                }
            }
        }
        out
    }

    /// Status tag and stage markers for the HP card of combatant `i`, e.g. "[PAR] CNF ATK+ SPE-".
    pub fn card_tags(&self, i: usize) -> String {
        let c = &self.battle.combatants[i];
        let mut parts = Vec::new();
        if let Some(s) = c.status {
            parts.push(format!("[{}]", battle_items::status_tag(s)));
        }
        if self.battle.is_confused(i) {
            parts.push("CNF".to_string());
        }
        if self.battle.is_trapped(i) {
            parts.push("TRAP".to_string());
        }
        let st = self.battle.stages(i);
        for (tag, v) in [("ATK", st.attack), ("DEF", st.defense), ("SPE", st.speed)] {
            match v.signum() {
                1 => parts.push(format!("{tag}+")),
                -1 => parts.push(format!("{tag}-")),
                _ => {}
            }
        }
        parts.join(" ")
    }

    fn enter_list(&mut self, state: BattleMenuState) {
        self.menu_state = state;
        self.list_cursor = 0;
        let hint = match state {
            BattleMenuState::ItemSelect => {
                let n = self.bag_view().len();
                if n == 0 {
                    "The bag is empty.".to_string()
                } else {
                    format!(
                        "BAG ({n} kinds, page 1/{}): 1-4 use, arrows scroll, Esc back.",
                        n.div_ceil(GRID_SLOTS)
                    )
                }
            }
            _ => "PARTY: 1-4 send out, arrows scroll, Esc back.".to_string(),
        };
        self.push_lines(vec![hint]);
    }

    fn confirm_list(&mut self) -> bool {
        match self.menu_state {
            BattleMenuState::ItemSelect => {
                let id = self.bag_view().get(self.list_cursor).map(|(e, _)| e.item_id);
                match id {
                    Some(id) => self.use_bag_item(id),
                    None => false,
                }
            }
            BattleMenuState::SwitchSelect => match self.party().get(self.list_cursor).copied() {
                Some(t) => self.switch_to(t),
                None => false,
            },
            _ => false,
        }
    }

    /// Handle one player input. Returns true when a battle turn was spent.
    pub fn handle_input(&mut self, input: BattleInput) -> bool {
        if self.battle.ended {
            return false;
        }
        match input {
            BattleInput::OpenFight => {
                self.menu_state = BattleMenuState::MainMenu;
                self.selected_menu_idx = 0;
                return false;
            }
            BattleInput::OpenSwitch => {
                self.enter_list(BattleMenuState::SwitchSelect);
                return false;
            }
            BattleInput::OpenItem => {
                self.enter_list(BattleMenuState::ItemSelect);
                return false;
            }
            BattleInput::Capture => {
                self.selected_menu_idx = 3;
                let ball = self
                    .bag_view()
                    .iter()
                    .find(|(e, _)| e.kind == ItemKind::Ball)
                    .map(|(e, _)| e.item_id);
                return match ball {
                    Some(id) => self.throw_ball(id),
                    None => {
                        self.push_lines(vec!["No Poke Balls in the bag!".into()]);
                        false
                    }
                };
            }
            BattleInput::Run => {
                self.selected_menu_idx = 4;
                self.run_away();
                return false;
            }
            _ => {}
        }
        let in_list = matches!(
            self.menu_state,
            BattleMenuState::ItemSelect | BattleMenuState::SwitchSelect
        );
        if in_list {
            let len = self.list_len();
            match input {
                BattleInput::Prev => self.list_cursor = self.list_cursor.saturating_sub(1),
                BattleInput::Next => self.list_cursor = (self.list_cursor + 1).min(len.saturating_sub(1)),
                BattleInput::Back => {
                    self.menu_state = BattleMenuState::MainMenu;
                    self.selected_menu_idx = 0;
                }
                BattleInput::Slot(n) => {
                    let idx = self.list_cursor / GRID_SLOTS * GRID_SLOTS + n;
                    if idx < len {
                        self.list_cursor = idx;
                        return self.confirm_list();
                    }
                }
                BattleInput::Confirm => return self.confirm_list(),
                _ => {}
            }
            return false;
        }
        let moves = self.battle.combatants[self.active_player()].moves.len();
        match input {
            BattleInput::Prev => self.selected_move_idx = self.selected_move_idx.saturating_sub(1),
            BattleInput::Next => {
                self.selected_move_idx = (self.selected_move_idx + 1).min(moves.saturating_sub(1))
            }
            BattleInput::Slot(n) if n < moves => {
                self.selected_move_idx = n;
                return !self.execute_player_attack(n).is_empty();
            }
            BattleInput::Confirm => {
                let n = self.selected_move_idx;
                return !self.execute_player_attack(n).is_empty();
            }
            _ => {}
        }
        false
    }

    /// Tab highlighted in the action row.
    pub fn highlighted_tab(&self) -> usize {
        self.tab_for_state()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assets() -> Option<BattleAssets> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        BattleAssets::load(dir).ok()
    }

    fn session(wild: &str) -> Option<ActiveBattleSession> {
        let a = assets()?;
        ActiveBattleSession::start(&a, "pikachu", 25, wild, 5, 0, Entity::PLACEHOLDER).ok()
    }

    #[test]
    fn start_builds_party_bag_and_real_catch_rate() {
        let Some(s) = session("bidoof") else { return };
        assert_eq!(s.party().len(), 3);
        assert_eq!(s.catch_rate, 255);
        assert_eq!(s.catch_rate_source, CatchRateSource::GameData);
        assert!(s.battle.move_effects().is_some());
        assert!(s.battle.item_table().is_some());
        assert!(s.bag_view().len() >= 7);
        // quick-attack priority comes from the effects sheet
        let qa = s.battle.combatants[0]
            .moves
            .iter()
            .find(|m| m.data.name == "quick-attack")
            .unwrap();
        assert_eq!(qa.priority, 1);
    }

    #[test]
    fn plain_new_keeps_placeholder_and_single_member() {
        let a = assets();
        let Some(a) = a else { return };
        let s = ActiveBattleSession::new(&a.data, "pikachu", 25, "pikachu", 5, 25, Entity::PLACEHOLDER).unwrap();
        assert_eq!(s.catch_rate, CAPTURE_CATCH_RATE_PLACEHOLDER);
        assert_eq!(s.party().len(), 1);
    }

    #[test]
    fn status_move_paralyzes_and_shows_tag() {
        let Some(mut s) = session("bidoof") else { return };
        // thunder-wave is slot 2 in the Pikachu loadout; a miss is possible, so retry a few turns
        for _ in 0..6 {
            s.execute_player_attack(2);
            if s.battle.combatants[WILD].status.is_some() {
                break;
            }
        }
        assert_eq!(s.battle.combatants[WILD].status, Some(MajorStatus::Paralysis));
        assert_eq!(s.card_tags(WILD), "[PAR]");
        assert!(s.log_lines.iter().any(|l| l.contains("paralyzed")));
    }

    #[test]
    fn full_session_enables_the_engine_status_rules() {
        let Some(s) = session("bidoof") else { return };
        assert_eq!(s.battle.status_rules(), StatusRules::game_rules());
    }

    #[test]
    fn plain_session_leaves_the_status_rules_off() {
        let Some(a) = assets() else { return };
        let s = ActiveBattleSession::new(&a.data, "pikachu", 25, "bidoof", 5, 0, Entity::PLACEHOLDER).unwrap();
        assert_eq!(s.battle.status_rules(), StatusRules::default());
    }

    #[test]
    fn confusion_move_is_logged_and_lost_actions_are_reported() {
        let Some(mut s) = session("bidoof") else { return };
        give_move(&mut s, 3, "confuse-ray");
        for _ in 0..10 {
            s.execute_player_attack(3);
            if s.battle.is_confused(WILD) {
                break;
            }
        }
        assert!(s.battle.is_confused(WILD));
        assert!(s.log_lines.iter().any(|l| l.contains("became confused")), "{:?}", s.log_lines);
        assert!(s.card_tags(WILD).contains("CNF"));
        // Keep going until the confusion runs its course; the self-hit/snap lines must be logged.
        for _ in 0..12 {
            if s.log_lines.iter().any(|l| l.contains("hurt itself") || l.contains("snapped out")) {
                break;
            }
            s.execute_player_attack(3);
        }
        assert!(s.log_lines.iter().any(|l| l.contains("hurt itself") || l.contains("snapped out")), "{:?}", s.log_lines);
    }

    #[test]
    fn charge_move_logs_a_charging_line() {
        let Some(mut s) = session("bidoof") else { return };
        give_move(&mut s, 3, "solar-beam");
        s.execute_player_attack(3);
        assert!(s.log_lines.iter().any(|l| l.contains("is charging solar-beam")), "{:?}", s.log_lines);
        assert!(s.battle.is_charging(0));
    }

    fn give_move(s: &mut ActiveBattleSession, slot: usize, name: &str) {
        let data = s.data.clone().unwrap();
        let m = data.move_row(name).unwrap().unwrap();
        let mut ms = crate::battle_engine::MoveSlot::new(m);
        if let Some(fx) = s.battle.move_effects().and_then(|t| t.get(name)) {
            ms.priority = fx.priority;
        }
        s.battle.combatants[0].moves[slot] = ms;
    }

    #[test]
    fn stat_stage_and_multi_hit_effects_reach_log_and_card() {
        let Some(mut s) = session("bidoof") else { return };
        give_move(&mut s, 3, "growl");
        give_move(&mut s, 1, "double-kick");
        for _ in 0..8 {
            s.execute_player_attack(3);
            if s.stages_of(WILD).attack < 0 {
                break;
            }
        }
        assert_eq!(s.stages_of(WILD).attack, -1);
        assert!(s.card_tags(WILD).contains("ATK-"));
        assert!(s.log_lines.iter().any(|l| l.contains("Attack fell")));
        // With the game rules on, the PLA damage formula applies. Hand calculation (effort 0,
        // neutral nature): Pikachu L25 Atk 65; Bidoof L5 HP 68, Def 31. Double Kick base damage
        // floor(floor(floor(30*(65+100+15*25))/(31+50))/5) = floor(floor(16200/81)/5) = 40, x2
        // Fighting vs Normal = 80, roll 85-100 % = 68..80 per kick, so the first kick always
        // faints a 68 HP Bidoof and "Hit 2 times!" never shows. The old formula gave 28..34 per
        // kick. Give the wild Bidoof a 500 HP pool so both kicks (at most 160) connect.
        s.battle.combatants[WILD].max_hp = 500;
        for _ in 0..8 {
            s.battle.combatants[WILD].hp = s.battle.combatants[WILD].max_hp;
            s.battle.combatants[0].hp = s.battle.combatants[0].max_hp;
            s.execute_player_attack(1);
            if s.log_lines.iter().any(|l| l == "Hit 2 times!") {
                break;
            }
        }
        assert!(s.log_lines.iter().any(|l| l == "Hit 2 times!"));
    }

    #[test]
    fn drain_move_heals_the_user_and_is_logged() {
        let Some(mut s) = session("bidoof") else { return };
        give_move(&mut s, 3, "giga-drain");
        s.battle.combatants[0].hp = 5;
        for _ in 0..6 {
            s.battle.combatants[WILD].hp = s.battle.combatants[WILD].max_hp;
            s.execute_player_attack(3);
            if s.log_lines.iter().any(|l| l.contains("drained")) {
                break;
            }
        }
        assert!(s.log_lines.iter().any(|l| l.contains("drained")));
    }

    #[test]
    fn potion_heals_and_consumes_but_full_hp_is_refused() {
        let Some(mut s) = session("bidoof") else { return };
        let potion = s.bag_order[0].item_id;
        let before = s.bag.count(potion);
        // full HP: refused, nothing consumed, no turn spent
        let turn = s.battle.turn;
        assert!(!s.use_bag_item(potion));
        assert_eq!(s.bag.count(potion), before);
        assert_eq!(s.battle.turn, turn);
        assert!(s.log_lines.last().unwrap().contains("no effect"));
        // damage the active Pokemon, then heal
        s.battle.combatants[0].hp = 10;
        assert!(s.use_bag_item(potion));
        assert_eq!(s.bag.count(potion), before - 1);
        assert!(s.log_lines.iter().any(|l| l.contains("regained")));
        assert!(s.battle.turn > turn);
    }

    #[test]
    fn revive_needs_a_fainted_member() {
        let Some(mut s) = session("bidoof") else { return };
        let revive = s
            .bag_order
            .iter()
            .find(|e| e.kind == ItemKind::Revive)
            .map(|e| e.item_id)
            .unwrap();
        assert!(!s.use_bag_item(revive));
        let bench = s.party()[1];
        s.battle.combatants[bench].hp = 0;
        assert!(s.use_bag_item(revive));
        assert!(s.battle.combatants[bench].hp > 0);
    }

    #[test]
    fn ball_throw_logs_shakes_and_matches_preview() {
        let Some(mut s) = session("bidoof") else { return };
        let ball = s
            .bag_order
            .iter()
            .find(|e| e.kind == ItemKind::Ball)
            .map(|e| e.item_id)
            .unwrap();
        let data = s.data.clone().unwrap();
        let pv = capture_preview(&s.battle, &data, "Regular", WILD, s.catch_rate, &s.rng).unwrap();
        let balls = s.bag.count(ball);
        assert!(s.throw_ball(ball));
        assert_eq!(s.bag.count(ball), balls - 1);
        assert_eq!(s.battle.combatants[WILD].caught, pv.checks_passed == 2);
        assert!(s.log_lines.iter().any(|l| l.starts_with("Shake...")));
        assert!(s.log_lines.iter().any(|l| l.contains("catch rate 255 [game data]")));
    }

    #[test]
    fn capture_preview_agrees_with_engine_over_many_seeds() {
        let Some(base) = session("bidoof") else { return };
        let data = base.data.clone().unwrap();
        for seed in 1..200u32 {
            let mut s = session("bidoof").unwrap();
            s.rng = XorShift32(seed.wrapping_mul(2_654_435_761) | 1);
            let pv = capture_preview(&s.battle, &data, "Regular", WILD, 255, &s.rng).unwrap();
            let caught = s
                .battle
                .throw_ball_item(&data, 4, WILD, true, 1, 1, 255, false, CaptureModifiers::default(), &mut s.rng)
                .unwrap();
            assert_eq!(caught, pv.checks_passed == 2, "seed {seed}");
        }
    }

    #[test]
    fn switch_menu_sends_out_a_party_member_and_costs_a_turn() {
        let Some(mut s) = session("bidoof") else { return };
        s.handle_input(BattleInput::OpenSwitch);
        assert_eq!(s.menu_state, BattleMenuState::SwitchSelect);
        let texts = s.slot_texts();
        assert!(texts[0].0.contains("PIKACHU"));
        assert!(texts[0].1.contains("(out)"));
        let turn = s.battle.turn;
        assert!(s.handle_input(BattleInput::Slot(1)));
        assert_ne!(s.active_player(), s.party()[0]);
        assert_eq!(s.menu_state, BattleMenuState::MainMenu);
        assert!(s.battle.turn > turn);
        // switching to the Pokemon already out is refused
        s.handle_input(BattleInput::OpenSwitch);
        let out = s.active_player();
        let idx = s.party().iter().position(|&i| i == out).unwrap();
        assert!(!s.handle_input(BattleInput::Slot(idx)));
    }

    #[test]
    fn item_menu_pages_and_back() {
        let Some(mut s) = session("bidoof") else { return };
        s.handle_input(BattleInput::OpenItem);
        assert_eq!(s.highlighted_tab(), 2);
        for _ in 0..5 {
            s.handle_input(BattleInput::Next);
        }
        assert_eq!(s.list_cursor, 5);
        assert_eq!(s.highlight_slot(), 1);
        let texts = s.slot_texts();
        assert!(texts[0].0.starts_with("[1] "));
        s.handle_input(BattleInput::Back);
        assert_eq!(s.menu_state, BattleMenuState::MainMenu);
    }

    #[test]
    fn full_battle_via_inputs_terminates_with_log() {
        let Some(mut s) = session("bidoof") else { return };
        for _ in 0..60 {
            if s.battle.ended {
                break;
            }
            s.handle_input(BattleInput::Slot(0));
        }
        assert!(s.battle.ended);
        assert!(s.log_lines.iter().any(|l| l.starts_with("Battle over")));
    }

    #[test]
    fn no_pp_move_is_refused_without_spending_a_turn() {
        let Some(mut s) = session("bidoof") else { return };
        s.battle.combatants[0].moves[0].pp = 0;
        let turn = s.battle.turn;
        assert!(s.execute_player_attack(0).is_empty());
        assert_eq!(s.battle.turn, turn);
        assert!(s.battle_summary.contains("no PP left"));
    }

    #[test]
    fn capture_input_without_balls_is_refused() {
        let Some(mut s) = session("bidoof") else { return };
        s.bag = ItemBag::default();
        assert!(!s.handle_input(BattleInput::Capture));
        assert!(s.battle_summary.contains("No Poke Balls"));
    }
}
