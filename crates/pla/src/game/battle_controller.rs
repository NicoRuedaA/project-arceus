//! Turn-based battle controller connecting the Bevy game to `pla::battle_engine`.
//!
//! The session owns the engine `Battle`, a small party, the item bag and the log. All menu and
//! turn logic lives here as plain functions on `ActiveBattleSession` so it is unit-testable
//! without Bevy systems; `battle_input` only maps keys to `BattleInput`.
//!
//! Turn flow ([`TurnMode`], see `battle_turns`): by default the PLA action-order queue decides
//! who acts (one combatant per step, the opponent acts automatically when the queue picks it)
//! and the player may use a move in agile (Q) or strong (E) style. `--classic-turns` keeps the
//! old once-per-round flow. After the battle, `battle_progress` awards experience and runs the
//! move-learn prompts and evolution before the battle screen closes.

use std::path::Path;
use std::sync::Arc;

use bevy::prelude::*;

use super::trainer_battle::{OpponentChoice, TrainerBattleState};
use super::battle_items::{self, BagEntry, ItemKind};
use super::battle_log::{self, tail};
use super::battle_progress::ProgressFlow;
use super::battle_style_ai::choose_ai_style;
pub use super::battle_turns::TurnMode;
use super::battle_turns::{style_word, turn_order_lines};
pub use super::catch_rate::{CatchRateSource, CatchRateTable, CAPTURE_CATCH_RATE_PLACEHOLDER};
use crate::battle::BattleData;
use crate::battle_engine::abilities::AbilityRules;
use crate::battle_engine::drops::DropTable;
use crate::battle_engine::effects::{MoveEffectsTable, Stages};
use crate::battle_engine::items::{ItemBag, ItemTable};
use crate::battle_engine::progression::ProgressionTables;
use crate::battle_engine::statuses::StatusRules;
use crate::battle_engine::styles::{MoveStyleTable, Style, StyledAction};
use crate::battle_engine::{
    capture_index, capture_probability, Action, Battle, BattleRng, CaptureModifiers, Combatant,
    Event, MajorStatus, XorShift32,
};

/// Index of the wild combatant in `Battle::combatants` (side 1). In a trainer battle it is the
/// trainer's first Pokemon; use [`ActiveBattleSession::opponent`] for the one that is out.
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
    /// After the battle: pick a move to forget (1-4) or give up (Back).
    LearnMove,
    /// After the battle: evolution animation (Back stops it).
    Evolving,
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
    /// Use the highlighted move in agile style (key Q; PLA turn flow only).
    StyleAgile,
    /// Use the highlighted move in strong style (key E; PLA turn flow only).
    StyleStrong,
}

/// Per-combatant facts the engine does not keep: the form and the cumulative experience
/// (`None` = unknown, the level threshold is used). Filled from the save by `install_party`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemberMeta {
    pub form: u8,
    pub exp: Option<u32>,
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
    /// Agile / strong values per move (`MoveStyleTable::load`); `None` when the sheets are
    /// missing (every move is then used normally).
    pub move_styles: Option<MoveStyleTable>,
    /// Experience curves, learnsets, evolutions and progression items; `None` disables
    /// experience, level-ups and evolution.
    pub progression: Option<Arc<ProgressionTables>>,
    /// Turn flow of new battles (default PLA queue; `--classic-turns` / `PLA_TURNS=classic`).
    pub turn_mode: TurnMode,
    /// Decoded drop tables (`data/poke_drop_items.tsv`); `None` disables the drops when a
    /// battle is won or a Pokemon is caught.
    pub drops: Option<Arc<DropTable>>,
}

impl BattleAssets {
    pub fn load(data_root: impl AsRef<Path>) -> Result<Self, String> {
        let root = data_root.as_ref();
        let catch_rates = CatchRateTable::load(root).unwrap_or_else(|e| {
            eprintln!("catch rates unavailable ({e}); using the placeholder rate");
            CatchRateTable::default()
        });
        let move_styles = MoveStyleTable::load(root)
            .map_err(|e| eprintln!("move styles unavailable ({e}); every move is used normally"))
            .ok();
        let progression = ProgressionTables::load(root)
            .map_err(|e| eprintln!("progression tables unavailable ({e}); no experience or evolution"))
            .ok()
            .map(Arc::new);
        let drops = DropTable::load(root)
            .map_err(|e| eprintln!("drop tables unavailable ({e}); no items are dropped"))
            .ok()
            .map(Arc::new);
        Ok(Self {
            data: Arc::new(BattleData::load(root)?),
            move_effects: MoveEffectsTable::load(root)?,
            items: ItemTable::load(root)?,
            catch_rates,
            status_rules: StatusRules::game_rules(),
            move_styles,
            progression,
            turn_mode: TurnMode::from_env_and_args(),
            drops,
        })
    }
}

/// Move loadout (real move names from `pml_moves`); unknown species get a plain default.
///
/// Every pick must be a move *usable in this game*: its `waza` record has `style_flag = 1`
/// and the game ships a `bin/effect/battle/sequence/ewNNN.wbin` timeline for it (see
/// `reports/data-progress/effects.md`). Moves with `style_flag = 0` (growl, take-down,
/// wing-attack, sand-attack, leer, ...) exist in the learnsets as tutor leftovers but the
/// game never plays them, so a battler built on them has no move effect at all.
pub fn moveset(species: &str) -> &'static [&'static str] {
    match species {
        "pikachu" => &["thunderbolt", "quick-attack", "thunder-wave", "iron-tail"],
        "bidoof" => &["tackle", "bite", "rollout", "crunch"],
        "starly" => &["quick-attack", "gust", "aerial-ace", "double-edge"],
        "shinx" => &["tackle", "bite", "spark", "thunder-shock"],
        _ => &["tackle", "quick-attack"],
    }
}

/// Wild-side loadout; a few species have their own, the rest use the plain default.
/// Same usable-move rule as [`moveset`].
fn wild_moveset(species: &str) -> &'static [&'static str] {
    match species {
        "pikachu" => &["spark", "quick-attack", "thunder-wave", "thunder-shock"],
        "bidoof" => &["tackle", "bite", "rollout"],
        "starly" => &["quick-attack", "gust", "aerial-ace"],
        "shinx" => &["tackle", "bite", "spark", "thunder-shock"],
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
    pub(crate) data: Option<Arc<BattleData>>,
    /// Turn flow of this battle.
    pub turn_mode: TurnMode,
    /// Form and experience per combatant index (see [`MemberMeta`]).
    pub member_meta: Vec<MemberMeta>,
    pub progression: Option<Arc<ProgressionTables>>,
    /// After-battle steps (move prompts, evolution); see `battle_progress`.
    pub progress: ProgressFlow,
    /// Trainer / boss battle facts (`None` in a wild battle); see `trainer_battle`.
    pub trainer: Option<TrainerBattleState>,
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
        let turn_mode = assets.map_or(TurnMode::Classic, |a| a.turn_mode);
        let (catch_rate, catch_rate_source) = match assets {
            Some(a) => {
                battle.set_move_effects(a.move_effects.clone());
                battle.set_item_table(a.items.clone());
                battle.set_status_rules(a.status_rules);
                // The two live abilities of the game (Slow Start, Flower Gift);
                // `PLA_ABILITIES=off` turns them off for comparison captures.
                battle.set_abilities(AbilityRules::from_env());
                if let Some(t) = &a.move_styles {
                    battle.set_move_styles(t.clone());
                }
                if turn_mode.uses_queue() {
                    battle.enable_action_queue();
                }
                (bag, bag_order) = battle_items::starter_bag(&a.items);
                if let Some(p) = &a.progression {
                    battle_items::add_progress_items(&mut bag, &mut bag_order, &a.items, p);
                }
                a.catch_rates.lookup(wild_species)
            }
            None => (CAPTURE_CATCH_RATE_PLACEHOLDER, CatchRateSource::Placeholder),
        };
        let member_meta = vec![MemberMeta::default(); battle.combatants.len()];
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
            turn_mode,
            member_meta,
            progression: assets.and_then(|a| a.progression.clone()),
            progress: ProgressFlow::default(),
            trainer: None,
        };
        let mut start = vec!["Battle Started!".to_string()];
        if s.turn_mode.uses_queue() {
            start.push(s.styles_hint());
        }
        s.push_lines(start);
        Ok(s)
    }

    // ------------------------------------------------------------------ helpers

    pub fn active_player(&self) -> usize {
        self.battle.active_combatant(0).unwrap_or(0)
    }

    /// The opponent's Pokemon that is out (always [`WILD`] in a wild battle).
    pub fn opponent(&self) -> usize {
        self.battle.active_combatant(1).unwrap_or(WILD)
    }

    /// The battle tables, when the session was started with them.
    pub fn battle_data(&self) -> Option<&BattleData> {
        self.data.as_deref()
    }

    /// The PLA action-order queue drives this battle.
    pub fn uses_queue(&self) -> bool {
        self.turn_mode.uses_queue() && self.battle.action_queue().is_some()
    }

    /// Start-of-battle hint for the PLA turn flow. Mastery (brief D3) is not tracked yet, so
    /// every move whose data allows styles counts as mastered; the hint says so.
    pub fn styles_hint(&self) -> String {
        if self.battle.move_styles().is_some() {
            "Styles: Q = Agile, E = Strong on the highlighted move (mastery not tracked yet: every move counts as mastered).".into()
        } else {
            "Styles unavailable (move style sheets not loaded).".into()
        }
    }

    /// HUD turn-order list (next 6 turns) for the PLA flow; empty in the classic flow.
    pub fn turn_order_lines(&self) -> Vec<String> {
        if !self.uses_queue() {
            return Vec::new();
        }
        let names = self.names();
        let preview = self.battle.turn_order_preview(crate::battle_engine::styles::TURN_ORDER_PREVIEW_LEN);
        turn_order_lines(&preview, &|i| names[i].clone(), 0)
    }

    /// Power / accuracy of a move slot in each style, as shown on the move grid
    /// (`"Pow 90 A70 S110 | Acc S100"`); `None` when the move has no usable styles.
    pub fn style_summary(&self, combatant: usize, slot: usize) -> Option<String> {
        let m = self.battle.combatants.get(combatant)?.moves.get(slot)?;
        let d = self.battle.move_styles()?.get(&m.data.name)?;
        if !d.styles_usable {
            return None;
        }
        let mut out = format!(
            "Pow {} A{} S{}",
            d.power(Style::Normal),
            d.power(Style::Agile),
            d.power(Style::Strong)
        );
        if d.accuracy(Style::Strong) != d.accuracy(Style::Normal) {
            out.push_str(&format!(" | Acc S{}", d.accuracy(Style::Strong)));
        }
        Some(out)
    }

    pub fn display_name(&self, i: usize) -> String {
        let n = self.battle.combatants[i].species.name.to_uppercase();
        if !self.battle.sides.get(1).is_some_and(|s| s.contains(&i)) {
            n
        } else if self.trainer.is_some() {
            format!("Foe {n}")
        } else {
            format!("Wild {n}")
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

    /// Status moves (no power) among `actions`: the engine emits no `UsedMove` for them, so the
    /// log announces them itself.
    fn status_moves_of(&self, actions: &[Action]) -> Vec<(usize, String)> {
        actions
            .iter()
            .filter_map(|a| {
                let m = self.battle.combatants[a.actor].moves.get(a.move_slot)?;
                (m.data.power == 0 && m.pp > 0).then(|| (a.actor, m.data.name.clone()))
            })
            .collect()
    }

    /// The opponent's move for this turn; a trainer's switch or item is carried out here and
    /// returns `None` with its log lines (the turn is spent on it). Wild Pokemon always attack
    /// with the engine-rule AI (random slot without tables).
    fn opponent_action(&mut self, target: usize) -> (Option<Action>, Vec<String>) {
        match self.opponent_choice(target) {
            OpponentChoice::Move(a) => (Some(a), Vec::new()),
            special => {
                let lines = self.perform_opponent_special(special);
                if lines.is_empty() {
                    // refused by the engine: attack instead
                    let actor = self.opponent();
                    let slot = self.battle.best_damage_action(actor).map_or(0, |a| a.move_slot);
                    (Some(Action { actor, move_slot: slot }), Vec::new())
                } else {
                    (None, lines)
                }
            }
        }
    }

    /// Log and record one resolved engine turn (classic round or PLA single action).
    fn record_turn(&mut self, before_active: usize, events: Vec<Event>, status_moves: &[(usize, String)]) -> Vec<Event> {
        let effects = self.battle.take_effect_events();
        let statuses = self.battle.take_status_events();
        let rules = self.battle.take_rule_events();
        let styles = self.battle.take_style_events();
        let abilities = self.battle.take_ability_events();
        let names = self.names();
        let name = |i: usize| names[i].clone();
        let mut lines = battle_log::format_turn(&name, &events, &effects, &statuses, &rules, status_moves);
        battle_log::insert_style_lines(&mut lines, &name, &styles);
        lines.extend(battle_log::ability_lines(&name, &abilities));
        let after_active = self.active_player();
        if after_active != before_active && !self.battle.ended {
            let at = lines
                .iter()
                .rposition(|l| l.ends_with("fainted!"))
                .map_or(lines.len(), |p| p + 1);
            lines.insert(at, format!("Go! {}!", names[after_active]));
            self.selected_move_idx = 0;
        }
        self.trainer_after_turn(&events, &mut lines);
        self.event_log.extend(events.clone());
        self.current_turn = self.battle.turn;
        self.push_lines(lines);
        self.menu_state = BattleMenuState::MainMenu;
        self.selected_menu_idx = 0;
        self.check_end();
        events
    }

    /// Classic flow: resolve one engine round. `player` is the player's action (None when the
    /// player used the turn on a switch, item or ball); the wild side always acts, chosen by
    /// the engine AI.
    fn resolve_turn(&mut self, player: Option<Action>) -> Vec<Event> {
        let before_active = self.active_player();
        let mut actions: Vec<Action> = player.into_iter().collect();
        if self.battle.combatants[self.opponent()].is_alive() {
            let (action, lines) = self.opponent_action(before_active);
            self.push_lines(lines);
            actions.extend(action);
        }
        let status_moves = self.status_moves_of(&actions);
        let events = self.battle.play_turn(&actions, &mut self.rng);
        self.record_turn(before_active, events, &status_moves)
    }

    /// PLA flow: one combatant acts alone with its style; the queue step follows the move.
    fn resolve_single(&mut self, action: StyledAction) -> Vec<Event> {
        let before_active = self.active_player();
        let status_moves = self.status_moves_of(&[action.action]);
        let events = self.battle.play_turn_styled(&[action], &mut self.rng);
        self.record_turn(before_active, events, &status_moves)
    }

    /// PLA flow: while the queue picks the wild Pokemon, it acts (style from
    /// `battle_style_ai`). Bounded to 8 actions in a row. Returns the events.
    fn run_ai_turns(&mut self) -> Vec<Event> {
        let mut all = Vec::new();
        for _ in 0..8 {
            let opp = self.opponent();
            if self.battle.ended || !self.uses_queue() || !self.battle.combatants[opp].is_alive() {
                break;
            }
            if self.battle.next_actor() != Some(opp) {
                break;
            }
            let target = self.active_player();
            let (action, lines) = self.opponent_action(target);
            let Some(action) = action else {
                // a trainer's switch or item: its queue step ends without a move
                self.push_lines(lines);
                self.battle.advance_action_queue(opp);
                all.push(Event::TurnStarted(self.battle.turn));
                continue;
            };
            let style = match &self.data {
                Some(d) => choose_ai_style(&self.battle, d.type_chart(), opp, action.move_slot, target),
                None => Style::Normal,
            };
            all.extend(self.resolve_single(StyledAction { action, style }));
        }
        all
    }

    /// Let the opponent act if the queue says it is its turn (PLA flow; no-op otherwise).
    /// Called every frame by `battle_input` and before each player input. True when it acted.
    pub fn poll_ai(&mut self) -> bool {
        !self.run_ai_turns().is_empty()
    }

    /// The opponent's answer to a player action that was not a move (item, ball, switch).
    fn opponent_turn(&mut self) {
        if self.uses_queue() {
            let player = self.active_player();
            self.battle.advance_action_queue(player);
            self.run_ai_turns();
            if !self.battle.ended {
                self.menu_state = BattleMenuState::MainMenu;
                self.selected_menu_idx = 0;
            }
        } else {
            self.resolve_turn(None);
        }
    }

    /// Run the after-battle progression once the battle has ended (idempotent).
    fn check_end(&mut self) {
        if self.battle.ended {
            self.trainer_battle_end();
            self.process_battle_end();
        }
    }

    /// The player attacks with `move_slot` in the normal style; the wild Pokemon answers.
    /// Returns the turn events (empty when the choice is refused, e.g. no PP left, and no turn
    /// passes).
    pub fn execute_player_attack(&mut self, move_slot: usize) -> Vec<Event> {
        self.execute_player_attack_styled(move_slot, Style::Normal)
    }

    /// The player attacks with `move_slot` in `style`. PLA flow: a due opponent acts first,
    /// then the player's move, then the opponent while the queue picks it. Classic flow: one
    /// round, the style is ignored (and the log says so).
    pub fn execute_player_attack_styled(&mut self, move_slot: usize, style: Style) -> Vec<Event> {
        if self.battle.ended {
            return Vec::new();
        }
        let mut events = self.run_ai_turns();
        if self.battle.ended {
            return events;
        }
        let actor = self.active_player();
        let Some(slot) = self.battle.combatants[actor].moves.get(move_slot) else {
            return events;
        };
        if slot.pp == 0 {
            let msg = format!("{} has no PP left!", slot.data.name);
            self.push_lines(vec![msg]);
            return events;
        }
        let action = Action { actor, move_slot };
        if !self.uses_queue() {
            if style != Style::Normal {
                self.push_lines(vec![format!(
                    "{} Style needs the PLA turn order (off with --classic-turns): used normally.",
                    style_word(style)
                )]);
            }
            events.extend(self.resolve_turn(Some(action)));
            return events;
        }
        events.extend(self.resolve_single(StyledAction { action, style }));
        events.extend(self.run_ai_turns());
        events
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
                        self.opponent_turn();
                        true
                    }
                    Err(e) => {
                        self.push_lines(vec![format!("It would have no effect on {who}. ({e})")]);
                        false
                    }
                }
            }
            ItemKind::Progress => {
                if self.use_progress_bag_item(item_id, &entry.label) {
                    self.opponent_turn();
                    true
                } else {
                    false
                }
            }
            ItemKind::Other => {
                self.push_lines(vec![format!("{} can't be used in battle.", entry.label)]);
                false
            }
        }
    }

    /// Throw a ball item at the wild Pokemon. Returns true when a turn was spent. Refused in
    /// trainer and Noble battles (nothing is consumed).
    pub fn throw_ball(&mut self, item_id: u16) -> bool {
        if let Some(line) = self.capture_refusal() {
            self.push_lines(vec![line]);
            return false;
        }
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
                    self.check_end();
                    true
                } else {
                    lines.push(format!("{wild} broke free!"));
                    self.push_lines(lines);
                    self.opponent_turn();
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
                self.opponent_turn();
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
        if let Some(line) = self.run_refusal() {
            self.push_lines(vec![line]);
            return;
        }
        self.battle.ended = true;
        self.push_lines(vec!["Got away safely!".into()]);
        self.check_end();
    }

    /// Legacy capture entry kept for callers without the item bag (fixed Poke Ball, placeholder
    /// catch rate). The in-game input path uses `throw_ball` instead.
    pub fn attempt_capture_in_battle(&mut self, battle_data: &BattleData) -> Result<bool, String> {
        if let Some(line) = self.capture_refusal() {
            self.push_lines(vec![line]);
            return Ok(false);
        }
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
            self.check_end();
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
            BattleMenuState::LearnMove if self.learn_prompt().is_some() => {
                let (ci, _) = self.learn_prompt().unwrap_or_default();
                let c = &self.battle.combatants[ci];
                for k in 0..GRID_SLOTS {
                    if let Some(m) = c.moves.get(k) {
                        out[k] = (format!("[{}] {}", k + 1, m.data.name), "Press to forget this move".to_string());
                    }
                }
            }
            _ => {
                let p = self.active_player();
                let c = &self.battle.combatants[p];
                let styles = self.uses_queue();
                for k in 0..GRID_SLOTS {
                    if let Some(m) = c.moves.get(k) {
                        let mut detail = format!("Type: {} | PP {}/{}", m.data.move_type, m.pp, m.data.pp);
                        if let Some(sum) = self.style_summary(p, k).filter(|_| styles) {
                            detail = format!("{} PP {}/{} | {sum}", m.data.move_type, m.pp, m.data.pp);
                        }
                        out[k] = (format!("[{}] {}", k + 1, m.data.name), detail);
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
            return self.progress_input(input);
        }
        // PLA flow: an opponent whose turn it is acts before the player's choice.
        self.poll_ai();
        if self.battle.ended {
            return true;
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
                if let Some(line) = self.capture_refusal() {
                    self.push_lines(vec![line]);
                    return false;
                }
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
            BattleInput::StyleAgile | BattleInput::StyleStrong if self.selected_move_idx < moves => {
                let style = if input == BattleInput::StyleAgile { Style::Agile } else { Style::Strong };
                let n = self.selected_move_idx;
                return !self.execute_player_attack_styled(n, style).is_empty();
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

    // ------------------------------------------------------------ PLA turns and styles

    fn session_mode(player: &str, level: u8, wild: &str, wild_level: u8, mode: TurnMode) -> Option<ActiveBattleSession> {
        let mut a = assets()?;
        a.turn_mode = mode;
        ActiveBattleSession::start(&a, player, level, wild, wild_level, 0, Entity::PLACEHOLDER).ok()
    }

    #[test]
    fn pla_flow_enables_the_queue_and_lists_six_turns() {
        let Some(s) = session_mode("pikachu", 25, "bidoof", 5, TurnMode::Pla) else { return };
        assert!(s.uses_queue());
        assert!(s.battle.move_styles().is_some());
        let order = s.turn_order_lines();
        assert_eq!(order.len(), 6);
        assert!(order[0].starts_with(">1. PIKACHU"), "the faster Pokemon acts first: {order:?}");
        assert!(s.log_lines.iter().any(|l| l.contains("mastery not tracked")), "{:?}", s.log_lines);
        // the move grid shows the style values of a PLA move
        let texts = s.slot_texts();
        assert!(texts[0].1.contains("Pow "), "{:?}", texts[0]);
    }

    #[test]
    fn classic_flow_keeps_one_round_for_both_sides() {
        let Some(mut s) = session_mode("pikachu", 25, "bidoof", 5, TurnMode::Classic) else { return };
        assert!(!s.uses_queue());
        assert!(s.turn_order_lines().is_empty());
        s.battle.combatants[WILD].max_hp = 5_000;
        s.battle.combatants[WILD].hp = 5_000;
        s.execute_player_attack(0);
        assert_eq!(s.battle.turn, 1);
        assert!(s.log_lines.iter().any(|l| l.starts_with("PIKACHU used")));
        assert!(s.log_lines.iter().any(|l| l.starts_with("Wild BIDOOF")), "{:?}", s.log_lines);
        // a style request in the classic flow is used normally and explained
        s.handle_input(BattleInput::StyleStrong);
        assert!(s.log_lines.iter().any(|l| l.contains("Strong Style needs the PLA turn order")));
    }

    #[test]
    fn strong_style_is_logged_and_delays_the_user() {
        let Some(mut s) = session_mode("pikachu", 25, "bidoof", 5, TurnMode::Pla) else { return };
        s.battle.combatants[WILD].max_hp = 5_000;
        s.battle.combatants[WILD].hp = 5_000;
        let wait_before = s.battle.action_queue().unwrap().entries[0].wait;
        assert!(s.handle_input(BattleInput::StyleStrong));
        let log = s.log_lines.join("\n");
        assert!(log.contains("PIKACHU used a Strong Style move!"), "{log}");
        assert!(log.contains("PIKACHU's next turn was pushed back (+5)."), "{log}");
        let at = s.log_lines.iter().position(|l| l == "PIKACHU used a Strong Style move!").unwrap();
        assert!(s.log_lines[at + 1].starts_with("PIKACHU used thunderbolt"), "{log}");
        assert!(wait_before > 0);
    }

    #[test]
    fn agile_style_pushes_the_target_back() {
        let Some(mut s) = session_mode("pikachu", 25, "bidoof", 5, TurnMode::Pla) else { return };
        s.battle.combatants[WILD].max_hp = 5_000;
        s.battle.combatants[WILD].hp = 5_000;
        s.handle_input(BattleInput::Next); // highlight quick-attack
        assert_eq!(s.selected_move_idx, 1);
        assert!(s.handle_input(BattleInput::StyleAgile));
        let log = s.log_lines.join("\n");
        assert!(log.contains("PIKACHU used an Agile Style move!"), "{log}");
        assert!(log.contains("Wild BIDOOF's next turn was pushed back (+3)."), "{log}");
        assert_eq!(s.battle.agile_streak(0), 1);
    }

    #[test]
    fn style_keys_are_ignored_in_lists_and_need_a_move() {
        let Some(mut s) = session_mode("pikachu", 25, "bidoof", 5, TurnMode::Pla) else { return };
        s.handle_input(BattleInput::OpenItem);
        let lines = s.log_lines.len();
        assert!(!s.handle_input(BattleInput::StyleAgile), "no move is used from the bag list");
        assert_eq!(s.log_lines.len(), lines);
        assert_eq!(s.menu_state, BattleMenuState::ItemSelect);
        s.handle_input(BattleInput::Back);
        s.selected_move_idx = 7; // out of range: nothing happens
        assert!(!s.handle_input(BattleInput::StyleStrong));
    }

    #[test]
    fn a_faster_wild_pokemon_acts_before_the_player() {
        // The wild Bidoof gets speed 1000 (base wait 1): the queue picks it first, and the
        // poll made before the player's input lets it act.
        let Some(mut s) = session_mode("pikachu", 5, "bidoof", 40, TurnMode::Pla) else { return };
        s.battle.combatants[WILD].speed = 1_000;
        s.battle.enable_action_queue();
        assert!(s.turn_order_lines()[0].contains("Wild BIDOOF"), "{:?}", s.turn_order_lines());
        s.battle.combatants[0].max_hp = 5_000;
        s.battle.combatants[0].hp = 5_000;
        s.handle_input(BattleInput::Slot(0));
        let first_use = s.log_lines.iter().find(|l| l.contains(" used ") || l.contains("missed")).unwrap();
        assert!(first_use.starts_with("Wild BIDOOF"), "{:?}", s.log_lines);
        assert!(s.log_lines.iter().any(|l| l.starts_with("PIKACHU used")), "{:?}", s.log_lines);
    }

    #[test]
    fn non_move_actions_advance_the_queue() {
        let Some(mut s) = session_mode("pikachu", 25, "bidoof", 5, TurnMode::Pla) else { return };
        s.battle.combatants[0].hp = 10;
        let potion = s.bag_order[0].item_id;
        assert!(s.use_bag_item(potion));
        // Pikachu waited its full base wait again, so the slower Bidoof (wait 3) acted.
        assert!(s.log_lines.iter().any(|l| l.starts_with("Wild BIDOOF")), "{:?}", s.log_lines);
        assert_eq!(s.menu_state, BattleMenuState::MainMenu);
    }

    // ------------------------------------------------------------ progression after battle

    /// Shinx L14 one point short of level 15 (evolution level and thunder-fang level; its
    /// four moves are full), against a wild Bidoof with 1 HP.
    fn shinx_on_the_edge() -> Option<ActiveBattleSession> {
        let mut s = session_mode("shinx", 14, "bidoof", 5, TurnMode::Pla)?;
        let t = s.progression.clone()?;
        let group = t.growth_group(403, 0);
        s.member_meta[0].exp = Some(crate::battle_engine::progression::exp_for_level(&t, group, 15) - 1);
        s.battle.combatants[WILD].hp = 1;
        Some(s)
    }

    fn win(s: &mut ActiveBattleSession) {
        for _ in 0..6 {
            if s.battle.ended {
                break;
            }
            s.battle.combatants[0].hp = s.battle.combatants[0].max_hp;
            s.execute_player_attack(0);
        }
        assert!(s.battle.ended, "{:?}", s.log_lines);
    }

    #[test]
    fn win_gives_exp_level_up_and_a_move_prompt() {
        let Some(mut s) = shinx_on_the_edge() else { return };
        let max_hp_before = s.battle.combatants[0].max_hp;
        win(&mut s);
        let log = s.log_lines.join("\n");
        assert!(log.contains("SHINX gained "), "{log}");
        assert!(log.contains("SHINX grew to level 15!"), "{log}");
        let won_at = s.log_lines.iter().position(|l| l == "Battle over: you won!").unwrap();
        let gain_at = s.log_lines.iter().position(|l| l.starts_with("SHINX gained ")).unwrap();
        assert!(gain_at > won_at, "experience follows the win line");
        assert_eq!(s.battle.combatants[0].level, 15);
        assert!(s.battle.combatants[0].max_hp > max_hp_before, "stats recomputed");
        assert_eq!(s.menu_state, BattleMenuState::LearnMove);
        assert!(log.contains("wants to learn thunder-fang"), "{log}");
        assert!(s.slot_texts()[0].1.contains("forget"));
        // inputs other than 1-4 / Back do nothing; 1 forgets tackle
        assert!(!s.handle_input(BattleInput::Confirm));
        assert!(s.handle_input(BattleInput::Slot(0)));
        assert_eq!(s.battle.combatants[0].moves[0].data.name, "thunder-fang");
        assert!(s.log_lines.iter().any(|l| l.contains("forgot tackle and learned thunder-fang")));
        // then the level-15 evolution starts
        assert_eq!(s.menu_state, BattleMenuState::Evolving);
        assert!(s.log_lines.iter().any(|l| l.contains("SHINX is evolving!")));
    }

    #[test]
    fn evolution_completes_after_the_animation() {
        let Some(mut s) = shinx_on_the_edge() else { return };
        win(&mut s);
        s.handle_input(BattleInput::Back); // give up thunder-fang
        assert!(s.log_lines.iter().any(|l| l == "SHINX did not learn thunder-fang."));
        assert_eq!(s.menu_state, BattleMenuState::Evolving);
        let hp_lost = s.battle.combatants[0].max_hp - s.battle.combatants[0].hp;
        s.tick_progress(1.0);
        assert_eq!(s.battle.combatants[0].species.dex, 403, "still animating");
        s.tick_progress(crate::game::battle_progress::EVOLVE_SECONDS);
        assert_eq!(s.battle.combatants[0].species.dex, 404);
        assert_eq!(s.battle.combatants[0].max_hp - s.battle.combatants[0].hp, hp_lost, "damage kept");
        assert!(s.log_lines.iter().any(|l| l == "Congratulations! Your SHINX evolved into LUXIO!"), "{:?}", s.log_lines);
        assert!(!s.progress.busy());
        assert_eq!(s.menu_state, BattleMenuState::MainMenu);
        assert!(s.member_meta[0].exp.is_some());
    }

    #[test]
    fn evolution_can_be_stopped() {
        let Some(mut s) = shinx_on_the_edge() else { return };
        win(&mut s);
        s.handle_input(BattleInput::Back);
        s.tick_progress(0.5);
        assert!(s.handle_input(BattleInput::Back));
        assert!(s.log_lines.iter().any(|l| l == "Huh? SHINX stopped evolving!"));
        s.tick_progress(10.0);
        assert_eq!(s.battle.combatants[0].species.dex, 403);
        assert!(!s.progress.busy());
    }

    #[test]
    fn running_or_catching_gives_no_experience() {
        let Some(mut s) = shinx_on_the_edge() else { return };
        s.handle_input(BattleInput::Run);
        assert!(s.battle.ended);
        assert!(!s.log_lines.iter().any(|l| l.contains("Exp. Points")));
        assert!(!s.progress.busy());
        assert_eq!(s.battle.combatants[0].level, 14);
    }

    #[test]
    fn rare_candy_levels_up_and_queues_the_prompt_for_after_the_battle() {
        let Some(mut s) = shinx_on_the_edge() else { return };
        s.battle.combatants[WILD].max_hp = 5_000;
        s.battle.combatants[WILD].hp = 5_000;
        let candy = s.bag_order.iter().find(|e| e.kind == ItemKind::Progress && e.label.starts_with("Level")).map(|e| e.item_id);
        let Some(candy) = candy else { panic!("no rare candy in the bag: {:?}", s.bag_order) };
        let before = s.bag.count(candy);
        assert!(s.use_bag_item(candy));
        assert_eq!(s.bag.count(candy), before - 1);
        assert_eq!(s.battle.combatants[0].level, 15);
        assert!(s.log_lines.iter().any(|l| l == "SHINX grew to level 15!"));
        assert!(s.progress.busy(), "thunder-fang and the evolution wait for the battle end");
        assert_eq!(s.menu_state, BattleMenuState::MainMenu, "the battle goes on");
        s.run_away();
        assert_eq!(s.menu_state, BattleMenuState::LearnMove);
    }

    #[test]
    fn effort_item_raises_effort_and_recomputes_stats() {
        let Some(mut s) = session_mode("pikachu", 25, "bidoof", 5, TurnMode::Pla) else { return };
        s.battle.combatants[WILD].max_hp = 5_000;
        s.battle.combatants[WILD].hp = 5_000;
        let item = s.bag_order.iter().find(|e| e.kind == ItemKind::Progress && e.label.contains('+') && e.label.starts_with("Effort")).cloned();
        let Some(item) = item else { panic!("no effort item: {:?}", s.bag_order) };
        let before = s.battle.combatants[0].clone();
        assert!(s.use_bag_item(item.item_id));
        let after = &s.battle.combatants[0];
        let raised = (0..6).find(|&i| after.effort[i] > before.effort[i]).expect("one effort rose");
        assert_eq!(after.effort[raised], before.effort[raised] + 1);
        assert!(s.log_lines.iter().any(|l| l.contains("effort rose to level 1")), "{:?}", s.log_lines);
        let stat = |c: &Combatant| [c.max_hp, c.attack, c.defense, c.special_attack, c.special_defense, c.speed][raised];
        assert!(stat(after) > stat(&before), "the stat grows with its effort level");
    }
}
