//! Trainer and boss battles on top of [`ActiveBattleSession`] (Brief 30).
//!
//! A trainer battle fields the whole party of a `data/trainer_teams.tsv` trainer on side 1:
//! combatant 1 is its first member (where a wild battle keeps the wild Pokemon) and the other
//! members follow the player's party in `Battle::combatants`. The engine already sends the next
//! living member out when the active one faints (stored order; the game's send-out order is
//! UNKNOWN) and ends the battle when a whole side has fainted.
//!
//! Rules and their confidence:
//! - No capture (balls refused, nothing consumed) and no running away: PROBABLE (brief rule;
//!   the main series does the same; not checked against this game's code).
//! - Move choice: the trainer's AI module bits (`ai_basic`, `ai_strong`; LIKELY mapping) pick
//!   the recovered score rules. Expert sequence handlers are not used (their per-score
//!   predicates are only partly mapped: UNKNOWN).
//! - Switching (`ai_change`) and items (`ai_item`): the recovered gates of
//!   `data/ai/score_rules.tsv` and `reports/data-progress/trainers.md` §3 with an ASSUMED
//!   affinity scale (`battle_ai::affinity_level`) and ASSUMED simplifications (documented on
//!   each function). Each bag item is ASSUMED usable once (the count is not stored).
//! - Prize money: UNKNOWN formula. [`trainer_prize_assumed`] (`prize_base x level of the last
//!   Pokemon`, the franchise formula on root slot 11) is a SUPUESTO / ASSUMED placeholder.
//! - Giratina (`giratina01`): `grtnPowerCoef` 1.5 (VERIFIED value, same as Alphas) and
//!   `grtnReliveHpPer` 100 read as "rises again once at 100 % HP" (LIKELY): a second phase.
//! - Nobles: only their turn-based part is modelled (one Pokemon from `boss_battles.tsv`,
//!   `nsPowerCoef` = 1 so no power change). The field phase (frenzy gauge, balms, field attacks)
//!   is not implemented: balm damage and attack numbers are UNKNOWN.

use bevy::prelude::Entity;

use super::battle_ai::{affinity_level, choose_ai_action_rules, ItemUseTable, HIGH_AFFINITY_RULE};
use super::battle_controller::{ActiveBattleSession, BattleAssets, BattleMenuState, MemberMeta, WILD};
use super::trainer_data::{NobleDef, TrainerAi, TrainerDef};
use crate::battle::BattleData;
use crate::battle_engine::styles::apply_alpha_power;
use crate::battle_engine::{type_effectiveness, Action, Battle, BattleRng, Combatant, Event};

/// Species used to build the session before the trainer's party replaces it.
const PLACEHOLDER_OPPONENT: &str = "bidoof";

/// `grtnReliveHpPer` in `battle_logic_config.bin` (VERIFIED value; meaning LIKELY).
pub const GIRATINA_RELIVE_HP_PCT: u8 = 100;
/// The trainer file of the Giratina boss battle.
pub const GIRATINA_TRAINER: &str = "giratina01";

/// Switch AI gates from `data/ai/score_rules.tsv` (observed chances).
pub const SWITCH_BREAK_SE_CHANCE: u32 = 78;
pub const SWITCH_BREAK_ANY_CHANCE: u32 = 50;
pub const SWITCH_BETTER_REJECT_CHANCE: u32 = 90;
pub const SWITCH_BETTER_IMMUNE_CHANCE: u32 = 50;
pub const SWITCH_BETTER_RESIST_CHANCE: u32 = 33;
/// `switch_better_rankup`: "rank-up >= 4" read as "do not switch out a Pokemon that has
/// gathered 4 or more positive stages" (ASSUMED).
pub const SWITCH_KEEP_RANKUP: i32 = 4;

/// What kind of opponent the session fights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpponentKind {
    Trainer,
    Noble,
}

/// Trainer / boss facts the engine does not keep.
#[derive(Debug, Clone, PartialEq)]
pub struct TrainerBattleState {
    pub id: String,
    pub kind: OpponentKind,
    /// Development label (`Captain Gankutsu`), from the internal name label.
    pub label: String,
    pub ai: TrainerAi,
    /// Bag items still available to the opponent (each ASSUMED single-use).
    pub items: Vec<u16>,
    /// Root slot 11 (ASSUMED prize base).
    pub prize_base: Option<u32>,
    /// Revive once at this HP percent (Giratina).
    pub revive_pct: Option<u8>,
    pub revived: bool,
    /// Battle music event of the trainer (`PLAY_BGM_VS_TRAINER_01`...).
    pub bgm: String,
    /// Type of the player's last damaging move (input of the switch AI).
    pub last_incoming_type: Option<String>,
    /// Money paid for the win (SUPUESTO formula); set once when the battle ends.
    pub prize: Option<u32>,
    /// The end lines were written.
    pub finished: bool,
}

impl TrainerBattleState {
    pub fn for_trainer(def: &TrainerDef) -> Self {
        let boss = def.id == GIRATINA_TRAINER;
        Self {
            id: def.id.clone(),
            kind: OpponentKind::Trainer,
            label: def.display_label(),
            ai: def.ai,
            items: if def.ai.item { def.items.clone() } else { Vec::new() },
            prize_base: def.prize_base,
            revive_pct: boss.then_some(GIRATINA_RELIVE_HP_PCT),
            revived: false,
            bgm: def.bgm.clone(),
            last_incoming_type: None,
            prize: None,
            finished: false,
        }
    }

    /// A Noble's turn-based battle. Its AI profile is not in the data: Basic + Strong (the
    /// profile of most trainers) is ASSUMED.
    pub fn for_noble(def: &NobleDef) -> Self {
        Self {
            id: def.id.clone(),
            kind: OpponentKind::Noble,
            label: def.display_label(),
            ai: TrainerAi { basic: true, strong: true, ..TrainerAi::default() },
            items: Vec::new(),
            prize_base: None,
            revive_pct: None,
            revived: false,
            bgm: def.bgm.clone(),
            last_incoming_type: None,
            prize: None,
            finished: false,
        }
    }
}

/// What the opponent does with its turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpponentChoice {
    Move(Action),
    /// Switch to this side-1 combatant.
    Switch(usize),
    /// Use this bag item on the active Pokemon.
    Item(u16),
}

/// SUPUESTO / ASSUMED prize: `prize_base x level of the last opponent Pokemon` (the franchise
/// formula applied to root slot 11). The game's formula and reader are UNKNOWN.
pub fn trainer_prize_assumed(prize_base: u32, last_level: u8) -> u32 {
    prize_base.saturating_mul(u32::from(last_level))
}

/// Score rules of a trainer's AI modules (only fully mapped rules; see `battle_ai`).
///
/// Expert and Allowance modules are wired with the rules whose predicates are complete:
/// `expert_friendly_fire` (double battles only, never in this port's singles),
/// `expert_lock_on_bonus` (dormant: the engine has no lock-on state yet) and the two decoded
/// Allowance HP tiers (no sheet trainer carries the bit, so it stays dormant on real data).
/// The Expert per-move sequence handlers stay UNKNOWN (no `aiSeqNo` in the move table).
pub fn trainer_rules(ai: &TrainerAi) -> Vec<&'static str> {
    let mut rules = Vec::new();
    if ai.basic {
        rules.push("basic_affinity_ineffective");
    }
    if ai.strong {
        rules.push("strong_non_max_damage");
        rules.push(HIGH_AFFINITY_RULE);
    }
    if ai.expert {
        rules.push("expert_friendly_fire");
        rules.push("expert_lock_on_bonus");
    }
    if ai.allowance {
        rules.push("allowance_hp_under_20");
        rules.push("allowance_hp_under_40");
    }
    rules
}

/// Rebuild a battle with new combatants and sides, keeping the tables and rules of `old`.
pub(crate) fn rebuild_battle(
    old: &Battle,
    data: &BattleData,
    combatants: Vec<Combatant>,
    sides: Vec<Vec<usize>>,
    queue: bool,
) -> Result<Battle, String> {
    let mut b = Battle::new(combatants, sides, data.type_chart().clone())?;
    if let Some(e) = old.move_effects().cloned() {
        b.set_move_effects(e);
    }
    if let Some(t) = old.item_table().cloned() {
        b.set_item_table(t);
    }
    b.set_status_rules(old.status_rules());
    b.set_abilities(old.ability_rules());
    if let Some(t) = old.move_styles().cloned() {
        b.set_move_styles(t);
    }
    if queue {
        b.enable_action_queue();
    }
    Ok(b)
}

/// Item AI (`ItemUseAI`, only with the Item bit), now with the decoded rules read from
/// `battle.lua` (`data/ai/item_use_rules.tsv`): the trainer's remaining items are tried in
/// order and the first one whose rule scores positive is used. The game compares that score
/// against the move candidates; only part of the move scoring is decoded in this port, so the
/// sign decides here. Items with no decoded rule (the aux-power items 1615-1619 and
/// 1756-1758) can never fire (UNKNOWN).
pub fn choose_item(
    battle: &Battle,
    actor: usize,
    items: &[u16],
    rules: Option<&ItemUseTable>,
    rng: &mut impl BattleRng,
) -> Option<u16> {
    let rules = rules?;
    let c = battle.combatants.get(actor)?;
    if !c.is_alive() {
        return None;
    }
    items
        .iter()
        .copied()
        .find(|item| rules.score_item(battle, actor, *item, rng).is_some_and(|score| score > 0))
}

/// Affinities of `actor`'s usable damaging moves against `target`.
fn move_affinities(battle: &Battle, data: &BattleData, actor: usize, target: usize) -> Vec<i32> {
    let foe = &battle.combatants[target].species.types;
    battle.combatants[actor]
        .moves
        .iter()
        .filter(|m| m.pp > 0 && m.data.power > 0)
        .map(|m| affinity_level(type_effectiveness(data.type_chart(), &m.data.move_type, foe)))
        .collect()
}

/// "Bench maximum calculated damage": the best engine damage estimate of `actor` against
/// `target` (the same estimator the move AI uses; no critical chance).
fn damage_proxy(battle: &Battle, actor: usize, target: usize) -> f32 {
    let Some(me) = battle.combatants.get(actor) else { return 0.0 };
    me.moves
        .iter()
        .enumerate()
        .filter(|(_, m)| m.pp > 0 && m.data.power > 0)
        .filter_map(|(slot, _)| battle.damage_estimate(actor, slot, target))
        .fold(0.0, f32::max)
}

fn best_of(battle: &Battle, candidates: &[usize], target: usize) -> Option<usize> {
    candidates
        .iter()
        .copied()
        .max_by(|a, b| damage_proxy(battle, *a, target).total_cmp(&damage_proxy(battle, *b, target)))
}

/// Switch AI (`PokeChangeAI`, only with the Change bit), after the recovered rules:
/// 1. "0x break": every usable damaging move of the active Pokemon has no effect on the foe,
///    and a bench member has a super-effective move (78 %) or any effective move (50 %).
/// 2. "make better": an active Pokemon with a super-effective move stays 90 % of the time;
///    one with 4+ positive stages stays (ASSUMED reading); otherwise a bench member with a
///    super-effective move that is immune to the player's last damaging move type comes in
///    50 % of the time, one that resists it 33 %.
///
/// The bench member chosen is the one with the best [`damage_proxy`] (ASSUMED).
pub fn choose_switch(
    battle: &Battle,
    data: &BattleData,
    actor: usize,
    target: usize,
    last_incoming_type: Option<&str>,
    rng: &mut impl BattleRng,
) -> Option<usize> {
    let side = battle.sides.get(1)?;
    let bench: Vec<usize> = side
        .iter()
        .copied()
        .filter(|&i| i != actor && battle.combatants[i].is_alive())
        .collect();
    if bench.is_empty() || !battle.combatants.get(target)?.is_alive() {
        return None;
    }
    let active = move_affinities(battle, data, actor, target);
    let has = |i: usize, f: &dyn Fn(i32) -> bool| move_affinities(battle, data, i, target).into_iter().any(f);
    if !active.is_empty() && active.iter().all(|a| *a == 0) {
        let se: Vec<usize> = bench.iter().copied().filter(|&i| has(i, &|a| a > 7)).collect();
        if !se.is_empty() && rng.below(100) < SWITCH_BREAK_SE_CHANCE {
            return best_of(battle, &se, target);
        }
        let any: Vec<usize> = bench.iter().copied().filter(|&i| has(i, &|a| a > 0)).collect();
        if !any.is_empty() && rng.below(100) < SWITCH_BREAK_ANY_CHANCE {
            return best_of(battle, &any, target);
        }
        return None;
    }
    if active.iter().any(|a| *a > 7) && rng.below(100) < SWITCH_BETTER_REJECT_CHANCE {
        return None;
    }
    let st = battle.stages(actor);
    let rankup: i32 = [st.attack, st.defense, st.speed].iter().map(|v| i32::from((*v).max(0))).sum();
    if rankup >= SWITCH_KEEP_RANKUP {
        return None;
    }
    let incoming = last_incoming_type?;
    let mut immune = Vec::new();
    let mut resist = Vec::new();
    for &i in &bench {
        if !has(i, &|a| a > 7) {
            continue;
        }
        let aff = affinity_level(type_effectiveness(data.type_chart(), incoming, &battle.combatants[i].species.types));
        match aff {
            0 => immune.push(i),
            1..=6 => resist.push(i),
            _ => {}
        }
    }
    if !immune.is_empty() && rng.below(100) < SWITCH_BETTER_IMMUNE_CHANCE {
        return best_of(battle, &immune, target);
    }
    if !resist.is_empty() && rng.below(100) < SWITCH_BETTER_RESIST_CHANCE {
        return best_of(battle, &resist, target);
    }
    None
}

/// Block characters of a team counter: `#` able to battle, `-` fainted (ASCII on purpose: the
/// default UI font has no ball glyphs).
pub fn team_counter(battle: &Battle, side: usize) -> String {
    let Some(members) = battle.sides.get(side) else { return String::new() };
    let alive = members.iter().filter(|&&i| battle.combatants[i].is_alive()).count();
    let marks: String = members
        .iter()
        .map(|&i| if battle.combatants[i].is_alive() { '#' } else { '-' })
        .collect();
    format!("[{marks}] {alive}/{}", members.len())
}

impl ActiveBattleSession {
    /// Start a battle against a trainer of the sheets: its whole party on side 1, no capture,
    /// no running away, AI from its module bits. `player_species` / `player_level` build the
    /// demo party (the game replaces it with the saved party, see `save_wire::install_party`).
    pub fn start_trainer(
        assets: &BattleAssets,
        player_species: &str,
        player_level: u8,
        def: &TrainerDef,
        entity: Entity,
    ) -> Result<Self, String> {
        let mut team = def.combatants(&assets.data)?;
        if team.is_empty() {
            return Err(format!("trainer {} has no party", def.id));
        }
        let state = TrainerBattleState::for_trainer(def);
        if state.revive_pct.is_some() {
            // grtnPowerCoef 1.5 (VERIFIED value, the same coefficient as Alphas).
            team.iter_mut().for_each(apply_alpha_power);
        }
        let metas = def.party.iter().map(|m| MemberMeta { form: m.form, exp: None }).collect();
        let mut s = Self::start(assets, player_species, player_level, PLACEHOLDER_OPPONENT, 5, 0, entity)?;
        s.install_opponents(&assets.data, team, metas, state)?;
        Ok(s)
    }

    /// Start the turn-based battle of a Noble setting (`boss_battles.tsv`).
    pub fn start_noble(
        assets: &BattleAssets,
        player_species: &str,
        player_level: u8,
        def: &NobleDef,
        entity: Entity,
    ) -> Result<Self, String> {
        let c = def.to_combatant(&assets.data, assets.progression.as_deref())?;
        let state = TrainerBattleState::for_noble(def);
        let mut s = Self::start(assets, player_species, player_level, PLACEHOLDER_OPPONENT, 5, 0, entity)?;
        s.install_opponents(&assets.data, vec![c], vec![MemberMeta { form: def.form, exp: None }], state)?;
        Ok(s)
    }

    /// Replace side 1 with `team` (first member at combatant [`WILD`], the rest appended).
    fn install_opponents(
        &mut self,
        data: &BattleData,
        team: Vec<Combatant>,
        metas: Vec<MemberMeta>,
        state: TrainerBattleState,
    ) -> Result<(), String> {
        let mut combatants = self.battle.combatants.clone();
        let player_side = self.battle.sides[0].clone();
        let mut side1 = Vec::new();
        for (k, c) in team.into_iter().enumerate() {
            if k == 0 {
                combatants[WILD] = c;
                side1.push(WILD);
            } else {
                side1.push(combatants.len());
                combatants.push(c);
            }
        }
        let queue = self.turn_mode.uses_queue();
        self.battle = rebuild_battle(&self.battle, data, combatants, vec![player_side, side1.clone()], queue)?;
        self.member_meta = vec![MemberMeta::default(); self.battle.combatants.len()];
        for (&i, m) in side1.iter().zip(metas) {
            self.member_meta[i] = m;
        }
        self.wild_dex = self.battle.combatants[WILD].species.dex;
        self.trainer = Some(state);
        self.log_lines.clear();
        let mut lines = vec![self.intro_line()];
        if self.trainer.as_ref().is_some_and(|t| t.kind == OpponentKind::Trainer) {
            lines.push(format!("{} sent out {}!", self.trainer_label(), self.display_name(WILD)));
        }
        if self.uses_queue() {
            lines.push(self.styles_hint());
        }
        self.push_lines(lines);
        Ok(())
    }

    fn intro_line(&self) -> String {
        match &self.trainer {
            Some(t) if t.kind == OpponentKind::Noble => format!("The {} is enraged! (turn-based phase only)", t.label),
            Some(t) => format!("{} would like to battle! (team of {})", t.label, self.battle.sides[1].len()),
            None => "Battle Started!".into(),
        }
    }

    /// True for trainer and Noble battles (no capture, no running away).
    pub fn is_trainer_battle(&self) -> bool {
        self.trainer.is_some()
    }

    /// Opponent label (`Captain Gankutsu`), empty in a wild battle.
    pub fn trainer_label(&self) -> String {
        self.trainer.as_ref().map(|t| t.label.clone()).unwrap_or_default()
    }

    /// Team counter of a side, e.g. `[##-] 2/3`.
    pub fn team_counter(&self, side: usize) -> String {
        team_counter(&self.battle, side)
    }

    /// Title of the opponent's HP card.
    pub fn opponent_card_title(&self) -> String {
        let c = &self.battle.combatants[self.opponent()];
        let name = c.species.name.to_uppercase();
        match &self.trainer {
            None => format!("WILD {name}  Lv. {}", c.level),
            Some(t) => format!("{} {}  {name} Lv. {}", t.label.to_uppercase(), self.team_counter(1), c.level),
        }
    }

    /// Title of the player's HP card (the team counter is added in trainer battles).
    pub fn player_card_title(&self) -> String {
        let c = &self.battle.combatants[self.active_player()];
        let base = format!("PLAYER {}  Lv. {}", c.species.name.to_uppercase(), c.level);
        if self.trainer.is_some() {
            format!("{base}  {}", self.team_counter(0))
        } else {
            base
        }
    }

    /// Side-1 members that fainted (not caught): the Pokemon that give experience.
    pub fn defeated_opponents(&self) -> Vec<usize> {
        self.battle.sides.get(1).map_or_else(Vec::new, |side| {
            side.iter()
                .copied()
                .filter(|&i| self.battle.combatants[i].hp == 0 && !self.battle.combatants[i].caught)
                .collect()
        })
    }

    /// Refusal line for a capture attempt, `None` when capturing is allowed (wild battle).
    pub(crate) fn capture_refusal(&self) -> Option<String> {
        self.trainer.as_ref().map(|t| match t.kind {
            OpponentKind::Trainer => "You can't catch another trainer's Pokemon!".to_string(),
            OpponentKind::Noble => format!("The {} can't be caught in this battle!", t.label),
        })
    }

    /// Refusal line for running away, `None` in a wild battle.
    pub(crate) fn run_refusal(&self) -> Option<String> {
        self.trainer.as_ref().map(|_| "There's no running from this battle!".to_string())
    }

    /// The opponent's decision for this turn. Wild battles always attack; trainers may use an
    /// item (Item bit) or switch (Change bit) first.
    pub(crate) fn opponent_choice(&mut self, target: usize) -> OpponentChoice {
        let actor = self.opponent();
        let Some(data) = self.data.clone() else {
            let n = self.battle.combatants[actor].moves.len().max(1);
            return OpponentChoice::Move(Action { actor, move_slot: (self.rng.0 as usize) % n });
        };
        let Some(state) = self.trainer.as_ref() else {
            return OpponentChoice::Move(super::battle_ai::choose_ai_action(&self.battle, &data, actor, target, &mut self.rng));
        };
        if state.ai.item {
            let rules = self.item_rules.clone();
            if let Some(item) = choose_item(&self.battle, actor, &state.items, rules.as_deref(), &mut self.rng) {
                return OpponentChoice::Item(item);
            }
        }
        if state.ai.change {
            let incoming = state.last_incoming_type.clone();
            if let Some(to) = choose_switch(&self.battle, &data, actor, target, incoming.as_deref(), &mut self.rng) {
                return OpponentChoice::Switch(to);
            }
        }
        let rules = trainer_rules(&state.ai);
        OpponentChoice::Move(choose_ai_action_rules(&self.battle, &data, actor, target, &rules, &mut self.rng))
    }

    /// Carry out a switch or item choice of the opponent; returns the log lines (empty when the
    /// engine refused it, in which case no turn is spent).
    pub(crate) fn perform_opponent_special(&mut self, choice: OpponentChoice) -> Vec<String> {
        let label = self.trainer_label();
        let from = self.opponent();
        match choice {
            OpponentChoice::Move(_) => Vec::new(),
            OpponentChoice::Switch(to) => match self.battle.switch(1, to) {
                Ok(_) => vec![
                    format!("{label} withdrew {}!", self.display_name(from)),
                    format!("{label} sent out {}!", self.display_name(to)),
                ],
                Err(_) => Vec::new(),
            },
            OpponentChoice::Item(item) => match self.battle.use_item_on(1, item, from, None) {
                Ok(u) => {
                    if let Some(t) = self.trainer.as_mut() {
                        if let Some(p) = t.items.iter().position(|i| *i == item) {
                            t.items.remove(p);
                        }
                    }
                    let what = self
                        .battle
                        .item_table()
                        .and_then(|t| t.get(item))
                        .map(super::battle_items::label)
                        .unwrap_or_else(|| format!("item #{item}"));
                    let mut line = format!("{label} used {what} (item #{item}) on {}", self.display_name(from));
                    if u.hp_restored > 0 {
                        line.push_str(&format!(": +{} HP", u.hp_restored));
                    }
                    line.push('!');
                    vec![line]
                }
                Err(_) => Vec::new(),
            },
        }
    }

    /// Trainer bookkeeping after an engine turn: remember the player's last damaging move
    /// type, announce the next opponent sent out after a faint, and the Giratina second phase.
    pub(crate) fn trainer_after_turn(&mut self, events: &[Event], lines: &mut Vec<String>) {
        if self.trainer.is_none() {
            return;
        }
        let side0 = self.battle.sides[0].clone();
        let side1 = self.battle.sides[1].clone();
        let incoming = events.iter().rev().find_map(|e| match e {
            Event::UsedMove { actor, name, damage, .. } if side0.contains(actor) && *damage > 0 => self.battle.combatants[*actor]
                .moves
                .iter()
                .find(|m| m.data.name == *name)
                .map(|m| m.data.move_type.clone()),
            _ => None,
        });
        // Giratina rises again once (second phase).
        let revive = self.trainer.as_ref().and_then(|t| (!t.revived).then_some(t.revive_pct).flatten());
        if let Some(pct) = revive {
            let opp = self.opponent();
            if self.battle.ended && self.battle.winner_side() == Some(Some(0)) {
                let c = &mut self.battle.combatants[opp];
                c.hp = (c.max_hp * u32::from(pct) / 100).max(1);
                c.status = None;
                self.battle.ended = false;
                lines.retain(|l| !l.starts_with("Battle over"));
                lines.push(format!("{} rose again with renewed power! (second phase, {pct}% HP)", self.display_name(opp)));
                if let Some(t) = self.trainer.as_mut() {
                    t.revived = true;
                }
            }
        }
        let opp = self.opponent();
        let knocked = events.iter().any(|e| matches!(e, Event::KnockedOut(k) if side1.contains(k) && *k != opp));
        if knocked && !self.battle.ended && self.battle.combatants[opp].is_alive() {
            let at = lines.iter().rposition(|l| l.ends_with("fainted!")).map_or(lines.len(), |p| p + 1);
            lines.insert(at, format!("{} sent out {}!", self.trainer_label(), self.display_name(opp)));
        }
        if let (Some(t), Some(ty)) = (self.trainer.as_mut(), incoming) {
            t.last_incoming_type = Some(ty);
        }
    }

    /// End lines of a trainer battle (once): the win with the SUPUESTO prize, or the loss.
    pub(crate) fn trainer_battle_end(&mut self) {
        let Some(t) = self.trainer.as_ref() else { return };
        if t.finished || !self.battle.ended {
            return;
        }
        let (label, base, kind) = (t.label.clone(), t.prize_base, t.kind);
        // "Last opponent Pokemon" is the last member of the trainer's whole team (its stored
        // order), not whichever member happens to be active at the end: the switch AI can leave
        // another member out when the battle closes.
        let last_level = self
            .battle
            .sides
            .get(1)
            .and_then(|side| side.last())
            .and_then(|&i| self.battle.combatants.get(i))
            .map(|c| c.level)
            .unwrap_or_else(|| self.battle.combatants[self.opponent()].level);
        let mut lines = Vec::new();
        let mut prize = None;
        match self.battle.winner_side() {
            Some(Some(0)) => {
                lines.push(match kind {
                    OpponentKind::Trainer => format!("You defeated {label}!"),
                    OpponentKind::Noble => format!("The {label} was calmed!"),
                });
                if let Some(base) = base {
                    let p = trainer_prize_assumed(base, last_level);
                    prize = Some(p);
                    lines.push(format!("You got {p} money for winning! (ASSUMED formula: {base} x Lv{last_level})"));
                }
            }
            Some(Some(_)) => lines.push(format!("You lost to {label}...")),
            _ => {}
        }
        if let Some(t) = self.trainer.as_mut() {
            t.finished = true;
            t.prize = prize;
        }
        self.push_lines(lines);
        self.menu_state = BattleMenuState::MainMenu;
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::battle_engine::XorShift32;
    use crate::game::battle_controller::{BattleInput, TurnMode};
    use crate::game::battle_items::ItemKind;
    use crate::game::trainer_data::{NobleCatalog, TrainerCatalog};

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    fn fixtures() -> Option<(BattleAssets, TrainerCatalog)> {
        let a = BattleAssets::load(root()).ok()?;
        let c = TrainerCatalog::load(root()).ok()?;
        Some((a, c))
    }

    /// Play the player's best damaging move until the battle ends (bounded).
    fn auto_play(s: &mut ActiveBattleSession, max_inputs: usize) {
        for _ in 0..max_inputs {
            if s.battle.ended {
                break;
            }
            let p = s.active_player();
            let slot = s.battle.best_damage_action(p).map_or(0, |a| a.move_slot);
            if s.execute_player_attack(slot).is_empty() && !s.battle.ended {
                // no PP left anywhere: switch to someone else or give up the move
                let next = s.battle.sides[0].iter().copied().find(|&i| i != p && s.battle.combatants[i].is_alive());
                match next {
                    Some(n) => {
                        s.switch_to(n);
                    }
                    None => break,
                }
            }
        }
    }

    #[test]
    fn prize_placeholder_is_base_times_level() {
        assert_eq!(trainer_prize_assumed(20, 30), 600);
        assert_eq!(trainer_prize_assumed(u32::MAX, 100), u32::MAX);
    }

    #[test]
    fn ai_module_bits_pick_the_mapped_rules() {
        assert!(trainer_rules(&TrainerAi::default()).is_empty());
        let all = TrainerAi {
            basic: true,
            strong: true,
            expert: true,
            allowance: true,
            item: true,
            change: true,
        };
        let rules = trainer_rules(&all);
        for want in [
            "basic_affinity_ineffective",
            "strong_non_max_damage",
            HIGH_AFFINITY_RULE,
            "expert_friendly_fire",
            "expert_lock_on_bonus",
            "allowance_hp_under_20",
            "allowance_hp_under_40",
        ] {
            assert!(rules.contains(&want), "{want} missing from {rules:?}");
        }
        // Expert without Strong: the high-affinity rule stays out.
        let expert = TrainerAi { expert: true, ..TrainerAi::default() };
        let rules = trainer_rules(&expert);
        assert!(rules.contains(&"expert_lock_on_bonus"));
        assert!(!rules.contains(&HIGH_AFFINITY_RULE));
        // Every trainer of the sheets yields a runnable rule list (basic trainers included).
        let Some((_, c)) = fixtures() else { return };
        let mut expert_count = 0;
        for t in c.trainers.values() {
            let rules = trainer_rules(&t.ai);
            if t.ai.expert {
                expert_count += 1;
                assert!(rules.contains(&"expert_lock_on_bonus"), "{}: {rules:?}", t.id);
            }
            assert_eq!(rules.is_empty(), !t.ai.basic && !t.ai.strong && !t.ai.expert && !t.ai.allowance);
        }
        assert!(expert_count > 100, "expert trainers: {expert_count}");
    }

    /// D6: a real trainer with a Hyper Potion (trainer32_01 stores item 25, Item bit on) uses
    /// it on its badly hurt active Pokemon through the decoded `itemAI_001` branches; the
    /// item is spent and the log shows the healing.
    #[test]
    fn trainer_uses_its_item_through_the_decoded_rule() {
        let Some((a, c)) = fixtures() else { return };
        let t = c.get("trainer32_01").unwrap();
        assert_eq!(t.items, vec![25], "the sheet stores the trainer's Hyper Potion");
        assert!(t.ai.item, "the Item AI bit is set");
        let mut s = ActiveBattleSession::start_trainer(&a, "pikachu", 25, t, Entity::PLACEHOLDER).unwrap();
        let opp = s.opponent();
        s.battle.combatants[opp].hp = (s.battle.combatants[opp].max_hp * 10) / 100;
        let hp_before = s.battle.combatants[opp].hp;
        let mut used = false;
        for seed in 1..40 {
            s.rng = XorShift32(seed);
            let choice = s.opponent_choice(0);
            if choice == OpponentChoice::Item(25) {
                let lines = s.perform_opponent_special(choice);
                assert!(s.battle.combatants[opp].hp > hp_before, "the potion healed");
                assert!(
                    lines.iter().any(|l| l.contains("+") && l.contains("HP")),
                    "the log reports the healing: {lines:?}"
                );
                assert!(s.trainer.as_ref().unwrap().items.is_empty(), "the item is spent");
                used = true;
                break;
            }
        }
        assert!(used, "some seed must use the potion at 10 % HP");
    }

    /// The Item bit is what gates the behaviour: without it the trainer keeps its items even
    /// at 1 HP.
    #[test]
    fn trainer_without_the_item_bit_never_uses_an_item() {
        let Some((a, c)) = fixtures() else { return };
        let mut t = c.get("trainer32_01").unwrap().clone();
        t.ai.item = false;
        let mut s = ActiveBattleSession::start_trainer(&a, "pikachu", 25, &t, Entity::PLACEHOLDER).unwrap();
        assert!(s.trainer.as_ref().unwrap().items.is_empty(), "the Item bit clears the list");
        let opp = s.opponent();
        s.battle.combatants[opp].hp = 1;
        for seed in 1..20 {
            s.battle.turn = 0;
            s.battle.ended = false;
            s.rng = XorShift32(seed);
            assert!(matches!(s.opponent_choice(0), OpponentChoice::Move(_) | OpponentChoice::Switch(_)));
        }
    }

    #[test]
    fn team_counter_marks_fainted_members() {
        let Some((a, c)) = fixtures() else { return };
        let t = c.get("trainer10_01").unwrap(); // 6 members
        let mut s = ActiveBattleSession::start_trainer(&a, "pikachu", 25, t, Entity::PLACEHOLDER).unwrap();
        assert_eq!(s.battle.sides[1].len(), 6);
        assert_eq!(s.team_counter(1), "[######] 6/6");
        let second = s.battle.sides[1][1];
        s.battle.combatants[second].hp = 0;
        assert_eq!(s.team_counter(1), "[#-####] 5/6");
        assert!(s.opponent_card_title().contains("[#-####] 5/6"), "{}", s.opponent_card_title());
        assert!(s.player_card_title().contains("/3"), "{}", s.player_card_title());
    }

    #[test]
    fn start_fields_the_whole_party_in_stored_order() {
        let Some((a, c)) = fixtures() else { return };
        let t = c.get("trainer09_01").unwrap();
        let s = ActiveBattleSession::start_trainer(&a, "pikachu", 25, t, Entity::PLACEHOLDER).unwrap();
        let side1: Vec<u16> = s.battle.sides[1].iter().map(|&i| s.battle.combatants[i].species.dex).collect();
        let want: Vec<u16> = t.party.iter().map(|m| m.species_id).collect();
        assert_eq!(side1, want);
        assert_eq!(s.opponent(), WILD);
        assert_eq!(s.wild_dex, want[0]);
        assert!(s.log_lines[0].contains("would like to battle"), "{:?}", s.log_lines);
        assert!(s.display_name(WILD).starts_with("Foe "));
        // levels and moves come from the sheet
        for (&i, m) in s.battle.sides[1].iter().zip(&t.party) {
            assert_eq!(s.battle.combatants[i].level, m.level);
            assert_eq!(s.member_meta[i].form, m.form);
        }
    }

    #[test]
    fn balls_are_refused_without_spending_them_and_running_is_refused() {
        let Some((a, c)) = fixtures() else { return };
        let mut s = ActiveBattleSession::start_trainer(&a, "pikachu", 25, c.get("trainer01_01").unwrap(), Entity::PLACEHOLDER).unwrap();
        let ball = s.bag_order.iter().find(|e| e.kind == ItemKind::Ball).map(|e| e.item_id).unwrap();
        let (balls, turn) = (s.bag.count(ball), s.battle.turn);
        assert!(!s.handle_input(BattleInput::Capture));
        assert!(!s.use_bag_item(ball));
        assert!(!s.throw_ball(ball));
        assert_eq!(s.bag.count(ball), balls, "no ball consumed");
        assert_eq!(s.battle.turn, turn, "no turn spent");
        assert!(!s.battle.combatants[WILD].caught);
        assert!(s.log_lines.iter().any(|l| l.contains("can't catch another trainer's")), "{:?}", s.log_lines);
        assert!(s.attempt_capture_in_battle(&a.data).is_ok_and(|caught| !caught));
        s.handle_input(BattleInput::Run);
        assert!(!s.battle.ended, "no running from a trainer");
        assert!(s.log_lines.iter().any(|l| l.contains("no running")));
    }

    /// A ball of every kind in the bag is refused against a trainer through every entry point
    /// (menu capture, bag item, throw, legacy capture): no ball spent, no turn lost, the RNG
    /// untouched, nothing caught and the battle neither ended nor skipped.
    #[test]
    fn every_ball_kind_is_rejected_against_a_trainer_without_side_effects() {
        let Some((a, c)) = fixtures() else { return };
        let mut s = ActiveBattleSession::start_trainer(&a, "pikachu", 25, c.get("trainer01_01").unwrap(), Entity::PLACEHOLDER).unwrap();
        assert!(s.is_trainer_battle());
        assert!(s.capture_refusal().is_some(), "a trainer battle always carries a capture refusal");
        let balls: Vec<u16> = s.bag_order.iter().filter(|e| e.kind == ItemKind::Ball).map(|e| e.item_id).collect();
        assert!(!balls.is_empty());
        let counts: Vec<u16> = balls.iter().map(|&b| s.bag.count(b)).collect();
        let (turn, rng, party, caught) = (s.battle.turn, s.rng.0, s.battle.sides[1].len(), s.battle.combatants[WILD].caught);
        for &b in &balls {
            assert!(!s.use_bag_item(b), "bag use of ball {b}");
            assert!(!s.throw_ball(b), "throw of ball {b}");
        }
        assert!(!s.handle_input(BattleInput::Capture));
        assert!(s.attempt_capture_in_battle(&a.data).is_ok_and(|caught| !caught));
        let after: Vec<u16> = balls.iter().map(|&b| s.bag.count(b)).collect();
        assert_eq!(after, counts, "no ball consumed");
        assert_eq!(s.battle.turn, turn, "no turn lost");
        assert_eq!(s.rng.0, rng, "no capture roll drawn");
        assert_eq!(s.battle.sides[1].len(), party, "trainer team untouched");
        assert_eq!(s.battle.combatants[WILD].caught, caught);
        assert!(!s.battle.ended, "the battle neither ended nor was skipped");
        assert!(s.is_trainer_battle());
    }

    /// Acceptance: a headless 3-vs-3 battle to the end, no panic, same winner for the same seed.
    #[test]
    fn three_vs_three_runs_to_the_end_deterministically() {
        let Some((mut a, c)) = fixtures() else { return };
        // a three-member trainer
        let t = c.trainers.values().find(|t| t.party.len() == 3 && t.group != "single").unwrap().clone();
        for mode in [TurnMode::Pla, TurnMode::Classic] {
            a.turn_mode = mode;
            let run = |seed: u32| {
                let mut s = ActiveBattleSession::start_trainer(&a, "pikachu", 60, &t, Entity::PLACEHOLDER).unwrap();
                assert_eq!((s.battle.sides[0].len(), s.battle.sides[1].len()), (3, 3));
                s.rng = XorShift32(seed);
                auto_play(&mut s, 400);
                assert!(s.battle.ended, "{mode:?} seed {seed}: {:?}", s.log_lines);
                (s.battle.winner_side(), s.log_lines.clone(), s.trainer.clone().unwrap())
            };
            for seed in [1u32, 7, 0x5ca1_e001] {
                let (w1, log1, st) = run(seed);
                let (w2, log2, _) = run(seed);
                assert_eq!(w1, w2, "{mode:?} seed {seed}");
                assert_eq!(log1, log2, "{mode:?} seed {seed}: same log for the same seed");
                assert!(st.finished);
                match w1 {
                    Some(Some(0)) => {
                        assert!(log1.iter().any(|l| l.starts_with("You defeated")), "{log1:?}");
                        assert_eq!(st.prize, t.prize_base.map(|b| trainer_prize_assumed(b, t.party.last().unwrap().level)));
                        // every opponent member fainted and the next one was announced
                        assert!(log1.iter().filter(|l| l.contains("sent out")).count() >= 3, "{log1:?}");
                    }
                    Some(Some(1)) => assert!(log1.iter().any(|l| l.starts_with("You lost to")), "{log1:?}"),
                    Some(None) => {} // both sides fainted on the same turn (recoil): a draw
                    other => panic!("unexpected end {other:?}"),
                }
            }
        }
    }

    #[test]
    fn forced_replacement_sends_the_next_member_in_stored_order() {
        let Some((a, c)) = fixtures() else { return };
        let t = c.get("trainer09_01").unwrap();
        let mut s = ActiveBattleSession::start_trainer(&a, "pikachu", 90, t, Entity::PLACEHOLDER).unwrap();
        let side1 = s.battle.sides[1].clone();
        assert!(side1.len() >= 2);
        s.battle.combatants[side1[0]].hp = 1;
        let p = s.active_player();
        s.battle.combatants[p].max_hp = 5_000;
        s.battle.combatants[p].hp = 5_000;
        for _ in 0..10 {
            if s.battle.combatants[side1[0]].hp == 0 {
                break;
            }
            let slot = s.battle.best_damage_action(p).map_or(0, |x| x.move_slot);
            s.execute_player_attack(slot);
        }
        assert_eq!(s.battle.combatants[side1[0]].hp, 0);
        assert_eq!(s.opponent(), side1[1], "the next stored member is out");
        assert!(!s.battle.ended);
        let name = s.display_name(side1[1]);
        assert!(s.log_lines.iter().any(|l| l.ends_with(&format!("sent out {name}!"))), "{:?}", s.log_lines);
    }

    #[test]
    fn low_hp_trainer_uses_its_bag_item_once() {
        let Some((a, c)) = fixtures() else { return };
        let t = c.get("trainer10_01").unwrap(); // ai_item with item 23
        assert!(t.ai.item && !t.items.is_empty());
        let mut used = 0;
        for seed in 1..40u32 {
            let mut s = ActiveBattleSession::start_trainer(&a, "pikachu", 25, t, Entity::PLACEHOLDER).unwrap();
            s.rng = XorShift32(seed);
            let opp = s.opponent();
            s.battle.combatants[opp].hp = (s.battle.combatants[opp].max_hp / 10).max(1);
            let hp = s.battle.combatants[opp].hp;
            if let OpponentChoice::Item(item) = s.opponent_choice(0) {
                let lines = s.perform_opponent_special(OpponentChoice::Item(item));
                assert!(!lines.is_empty());
                assert!(s.battle.combatants[opp].hp > hp, "healed");
                assert!(s.trainer.as_ref().unwrap().items.is_empty(), "single use");
                assert!(!matches!(s.opponent_choice(0), OpponentChoice::Item(_)), "no item left");
                used += 1;
            }
        }
        // 66 % gate under 20 % HP: most seeds use it, not all.
        assert!((15..39).contains(&used), "used {used}/39");
        // full HP: never
        let mut s = ActiveBattleSession::start_trainer(&a, "pikachu", 25, t, Entity::PLACEHOLDER).unwrap();
        for seed in 1..30u32 {
            s.rng = XorShift32(seed);
            assert!(!matches!(s.opponent_choice(0), OpponentChoice::Item(_)));
        }
    }

    #[test]
    fn an_immune_active_pokemon_switches_to_a_bench_member_that_can_hit() {
        let Some((a, c)) = fixtures() else { return };
        let mut t = c.get("trainer09_01").unwrap().clone();
        t.ai.change = true;
        let mut s = ActiveBattleSession::start_trainer(&a, "pikachu", 25, &t, Entity::PLACEHOLDER).unwrap();
        let data = s.data.clone().unwrap();
        // The opponent's active Pokemon only knows a Normal move; the player's lead is a Ghost.
        let opp = s.opponent();
        let tackle = data.move_row("tackle").unwrap().unwrap();
        s.battle.combatants[opp].moves = vec![crate::battle_engine::MoveSlot::new(tackle)];
        let p = s.active_player();
        s.battle.combatants[p].species.types = ["Ghost".into(), "Ghost".into()];
        let mut switched = 0;
        for seed in 1..60u32 {
            s.rng = XorShift32(seed);
            if let OpponentChoice::Switch(to) = s.opponent_choice(p) {
                assert!(s.battle.sides[1].contains(&to) && to != opp);
                switched += 1;
            }
        }
        assert!(switched > 30, "78 %/50 % gates: switched {switched}/59");
        // Without the Change bit it never switches.
        s.trainer.as_mut().unwrap().ai.change = false;
        for seed in 1..30u32 {
            s.rng = XorShift32(seed);
            assert!(matches!(s.opponent_choice(p), OpponentChoice::Move(_)));
        }
    }

    #[test]
    fn giratina_rises_again_once_then_the_battle_ends() {
        let Some((a, c)) = fixtures() else { return };
        let t = c.get(GIRATINA_TRAINER).unwrap();
        let mut s = ActiveBattleSession::start_trainer(&a, "pikachu", 25, t, Entity::PLACEHOLDER).unwrap();
        let opp = s.opponent();
        let p = s.active_player();
        s.battle.combatants[p].max_hp = 100_000;
        for _ in 0..12 {
            if s.battle.ended {
                break;
            }
            s.battle.combatants[p].hp = s.battle.combatants[p].max_hp;
            if !s.trainer.as_ref().unwrap().revived || s.battle.combatants[opp].hp > 1 {
                s.battle.combatants[opp].hp = 1;
            }
            s.execute_player_attack(0);
        }
        let log = s.log_lines.join("\n");
        assert!(s.trainer.as_ref().unwrap().revived, "{log}");
        assert!(log.contains("rose again"), "{log}");
        assert!(s.battle.ended, "{log}");
        assert_eq!(log.matches("Battle over: you won!").count(), 1, "{log}");
        assert_eq!(s.trainer.as_ref().unwrap().prize, None, "giratina01 stores no prize base");
    }

    #[test]
    fn noble_turn_based_phase_starts_from_the_sheet() {
        let Ok(a) = BattleAssets::load(root()) else { return };
        let Ok(n) = NobleCatalog::load(root()) else { return };
        let def = n.get("nsi_ex_01_r").unwrap();
        let mut s = ActiveBattleSession::start_noble(&a, "pikachu", 25, def, Entity::PLACEHOLDER).unwrap();
        assert_eq!(s.battle.sides[1].len(), 1);
        assert_eq!(s.battle.combatants[WILD].species.dex, 900);
        assert_eq!(s.battle.combatants[WILD].level, 70);
        assert!(s.log_lines[0].contains("Noble Kleavor"), "{:?}", s.log_lines);
        assert!(!s.handle_input(BattleInput::Capture));
        assert!(s.log_lines.iter().any(|l| l.contains("can't be caught")));
    }

    #[test]
    fn wild_battles_are_unchanged() {
        let Some((a, _)) = fixtures() else { return };
        let s = ActiveBattleSession::start(&a, "pikachu", 25, "bidoof", 5, 399, Entity::PLACEHOLDER).unwrap();
        assert!(!s.is_trainer_battle());
        assert_eq!(s.display_name(WILD), "Wild BIDOOF");
        assert_eq!(s.opponent_card_title(), "WILD BIDOOF  Lv. 5");
        assert_eq!(s.log_lines[0], "Battle Started!");
    }

    /// Every trainer of the sheets can start a battle (377 / 377).
    #[test]
    fn every_trainer_starts_a_battle() {
        let Some((a, c)) = fixtures() else { return };
        let mut ok = 0;
        for t in c.trainers.values() {
            let s = ActiveBattleSession::start_trainer(&a, "pikachu", 25, t, Entity::PLACEHOLDER)
                .unwrap_or_else(|e| panic!("{}: {e}", t.id));
            assert_eq!(s.battle.sides[1].len(), t.party.len());
            ok += 1;
        }
        assert_eq!(ok, 377);
    }
}
