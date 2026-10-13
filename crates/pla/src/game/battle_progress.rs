//! After-battle progression on the live battle session: experience, level-ups, move learning
//! (with the 4-move replacement prompt), evolution (with a cancellable animation phase) and
//! the progression items of the bag (rare candy, effort items).
//!
//! The rules themselves are the pure functions of `battle_engine::progression`; this module
//! only sequences them on an [`ActiveBattleSession`] and writes the log lines. Confidence:
//! - VERIFIED (tables / recovered code, see `reports/data-progress/progression.md`): growth
//!   curves, level-from-experience, experience cap, learnsets, evolution rows, stat formulas.
//! - ASSUMED: experience yield `base_exp * level / 7` (x1.5 trainers) split among the
//!   participants; only the Pokemon that is out when the battle ends participates (the port
//!   does not track who fought yet); caught Pokemon give no experience; the evolution context
//!   has no friendship, gender, time or place (those evolutions never fire here).
//!
//! Flow: when the battle ends, [`ActiveBattleSession::process_battle_end`] awards the
//! experience and queues [`ProgressStep`]s. While steps remain the battle screen stays up:
//! a move prompt waits for 1-4 (forget that move) or Back (give up), an evolution animates for
//! [`EVOLVE_SECONDS`] and can be stopped with Back. `battle_input` leaves the battle only when
//! the queue is empty.

use std::collections::VecDeque;

use super::battle_controller::{ActiveBattleSession, BattleInput, BattleMenuState, MemberMeta, WILD};
use crate::battle_engine::progression::{
    check_evolution, evolve, evolve_combatant, exp_yield_for, level_up_combatant, use_progress_item,
    AwardOutcome, EffortError, Evolution, EvolutionContext, ProgressMember, ProgressionTables, MAX_LEVEL,
};
use crate::battle_engine::{progression, MoveSlot};

/// Length of the placeholder evolution animation (flash + scale pulse), seconds.
pub const EVOLVE_SECONDS: f32 = 2.5;
/// Effort stat names in `Combatant::effort` order.
pub const EFFORT_STATS: [&str; 6] = ["HP", "Attack", "Defense", "Sp. Atk", "Sp. Def", "Speed"];

/// One decision or animation still owed after the battle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressStep {
    /// All four slots are taken: the player picks a move to forget (or gives up).
    LearnMove { combatant: usize, move_id: u16 },
    /// Evolution animation, then the species change (unless stopped).
    Evolve { combatant: usize, evo: Evolution },
}

/// Pending after-battle steps of a session.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProgressFlow {
    pub queue: VecDeque<ProgressStep>,
    /// Seconds into the running evolution animation (`Some` while it plays).
    pub evolve_elapsed: Option<f32>,
    /// The battle end was processed (experience is awarded once).
    pub end_processed: bool,
}

impl ProgressFlow {
    pub fn busy(&self) -> bool {
        !self.queue.is_empty()
    }
    pub fn current(&self) -> Option<&ProgressStep> {
        self.queue.front()
    }
    /// 0..=1 progress of the evolution animation, when one plays.
    pub fn evolve_progress(&self) -> Option<f32> {
        self.evolve_elapsed.map(|t| (t / EVOLVE_SECONDS).clamp(0.0, 1.0))
    }
}

/// Experience a participant gets for defeating `species`/`form` at `level` (ASSUMED formula,
/// see `progression::exp_yield`); `participants` share it.
pub fn battle_exp(t: &ProgressionTables, species: u16, form: u8, level: u8, participants: u32, trainer: bool) -> u32 {
    exp_yield_for(t, species, form, level, participants, trainer)
}

/// Log lines for an award: the gain (when `gained` > 0), one line per level crossed and the
/// moves learned at each level. `move_name(id)` names a move.
pub fn award_lines(who: &str, out: &AwardOutcome, move_name: &dyn Fn(u16) -> String) -> Vec<String> {
    let mut lines = Vec::new();
    if out.gained > 0 {
        lines.push(format!("{who} gained {} Exp. Points!", out.gained));
    }
    for level in out.old_level.saturating_add(1)..=out.new_level {
        if level <= out.old_level {
            break;
        }
        lines.push(format!("{who} grew to level {level}!"));
        for (_, mv) in out.learned.iter().filter(|(l, _)| *l == level) {
            lines.push(format!("{who} learned {}!", move_name(*mv)));
        }
    }
    lines
}

/// Effort-change lines between two effort arrays.
pub fn effort_lines(who: &str, before: &[u8; 6], after: &[u8; 6]) -> Vec<String> {
    (0..6)
        .filter(|&i| before[i] != after[i])
        .map(|i| {
            let dir = if after[i] > before[i] { "rose" } else { "fell" };
            format!("{who}'s {} effort {dir} to level {}!", EFFORT_STATS[i], after[i])
        })
        .collect()
}

/// The evolution a member takes after a level change (none when the level did not change).
/// Only the facts the port has are filled in the context (see the module docs).
pub fn level_up_evolution(
    t: &ProgressionTables,
    m: &ProgressMember,
    out: &AwardOutcome,
    party_species: Vec<u16>,
) -> Option<Evolution> {
    if !out.leveled_up() {
        return None;
    }
    let ctx = EvolutionContext { party_species, ..EvolutionContext::default() };
    check_evolution(t, m, &ctx)
}

/// What a 1-4 / Back press does while a move prompt is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptChoice {
    Forget(usize),
    GiveUp,
    Ignore,
}

/// Prompt key mapping: digits 1-4 forget that slot (only slots that hold a move), Back gives
/// up, everything else is ignored.
pub fn prompt_choice(input: BattleInput, known_moves: usize) -> PromptChoice {
    match input {
        BattleInput::Slot(k) if k < known_moves => PromptChoice::Forget(k),
        BattleInput::Back => PromptChoice::GiveUp,
        _ => PromptChoice::Ignore,
    }
}

impl ActiveBattleSession {
    fn move_name(&self, id: u16) -> String {
        self.battle_data()
            .and_then(|d| d.move_row_by_id(id).ok().flatten())
            .map_or_else(|| format!("move #{id}"), |m| m.name)
    }

    fn meta(&self, ci: usize) -> MemberMeta {
        self.member_meta.get(ci).copied().unwrap_or_default()
    }

    /// The persistent progression view of combatant `ci`.
    pub fn progress_member(&self, t: &ProgressionTables, ci: usize) -> ProgressMember {
        let meta = self.meta(ci);
        ProgressMember::from_combatant(t, &self.battle.combatants[ci], meta.form, meta.exp)
    }

    fn party_species(&self) -> Vec<u16> {
        self.battle.sides[0].iter().map(|&i| self.battle.combatants[i].species.dex).collect()
    }

    /// Mirror a progression result on the combatant (stats, moves, experience), queue the
    /// pending move prompts and a level-up evolution. Returns the log lines (no gain line).
    pub(crate) fn apply_progress(&mut self, t: &ProgressionTables, ci: usize, m: &ProgressMember, out: &AwardOutcome) -> Vec<String> {
        let who = self.display_name(ci);
        let names: Vec<(u16, String)> =
            out.learned.iter().chain(&out.pending).map(|(_, id)| (*id, self.move_name(*id))).collect();
        let name_of = |id: u16| names.iter().find(|(i, _)| *i == id).map_or_else(String::new, |(_, n)| n.clone());
        let lines = award_lines(&who, &AwardOutcome { gained: 0, ..out.clone() }, &name_of);
        let rows: Vec<_> = out
            .learned
            .iter()
            .filter_map(|(_, id)| self.battle_data().and_then(|d| d.move_row_by_id(*id).ok().flatten()))
            .collect();
        let c = &mut self.battle.combatants[ci];
        c.effort = m.effort;
        level_up_combatant(c, m.level);
        for row in rows {
            if c.moves.len() < progression::MAX_KNOWN_MOVES && !c.moves.iter().any(|s| s.data.id == row.id) {
                c.moves.push(MoveSlot::new(row));
            }
        }
        if let Some(meta) = self.member_meta.get_mut(ci) {
            meta.exp = Some(m.exp);
        }
        for (_, id) in &out.pending {
            self.progress.queue.push_back(ProgressStep::LearnMove { combatant: ci, move_id: *id });
        }
        let already = self.progress.queue.iter().any(|s| matches!(s, ProgressStep::Evolve { combatant, .. } if *combatant == ci));
        if !already {
            if let Some(evo) = level_up_evolution(t, m, out, self.party_species()) {
                self.progress.queue.push_back(ProgressStep::Evolve { combatant: ci, evo });
            }
        }
        lines
    }

    /// Once per battle, when it has ended: award experience on a win, then start the queued
    /// steps (also those queued by a rare candy during the battle).
    pub fn process_battle_end(&mut self) {
        if !self.battle.ended || self.progress.end_processed {
            return;
        }
        self.progress.end_processed = true;
        let wild = &self.battle.combatants[WILD];
        let won = wild.hp == 0 && !wild.caught;
        let ci = self.active_player();
        let alive = self.battle.combatants.get(ci).is_some_and(|c| c.hp > 0);
        if let (true, true, Some(t)) = (won, alive, self.progression.clone()) {
            let (dex, level) = (wild.species.dex, wild.level);
            // ASSUMED: wild battles, the active Pokemon is the only participant.
            let amount = battle_exp(&t, dex, 0, level, 1, false);
            let mut m = self.progress_member(&t, ci);
            let out = progression::award_exp(&t, &mut m, amount);
            let mut lines = vec![format!("{} gained {amount} Exp. Points!", self.display_name(ci))];
            lines.extend(self.apply_progress(&t, ci, &m, &out));
            self.push_lines(lines);
        }
        self.start_next_step();
    }

    /// Show the prompt of the step at the head of the queue (or return to the plain menu).
    fn start_next_step(&mut self) {
        self.progress.evolve_elapsed = None;
        let Some(step) = self.progress.current().cloned() else {
            self.menu_state = BattleMenuState::MainMenu;
            return;
        };
        match step {
            ProgressStep::LearnMove { combatant, move_id } => {
                let who = self.display_name(combatant);
                let mv = self.move_name(move_id);
                self.menu_state = BattleMenuState::LearnMove;
                self.push_lines(vec![
                    format!("{who} wants to learn {mv}, but it already knows 4 moves."),
                    format!("Press 1-4 to forget a move, or Backspace to give up on {mv}."),
                ]);
            }
            ProgressStep::Evolve { combatant, .. } => {
                let who = self.display_name(combatant);
                self.menu_state = BattleMenuState::Evolving;
                self.progress.evolve_elapsed = Some(0.0);
                self.push_lines(vec![format!("What? {who} is evolving! (Backspace to stop it)")]);
            }
        }
    }

    /// Move-learn prompt contents: (who, new move name, current moves) when one is open.
    pub fn learn_prompt(&self) -> Option<(usize, u16)> {
        match self.progress.current()? {
            ProgressStep::LearnMove { combatant, move_id } => Some((*combatant, *move_id)),
            ProgressStep::Evolve { .. } => None,
        }
    }

    /// Input while after-battle steps remain. Returns true when a step was resolved.
    pub fn progress_input(&mut self, input: BattleInput) -> bool {
        match self.progress.current().cloned() {
            Some(ProgressStep::LearnMove { combatant, move_id }) => {
                let known = self.battle.combatants[combatant].moves.len();
                let who = self.display_name(combatant);
                let mv = self.move_name(move_id);
                match prompt_choice(input, known) {
                    PromptChoice::Forget(k) => {
                        let row = self.battle_data().and_then(|d| d.move_row_by_id(move_id).ok().flatten());
                        let Some(row) = row else {
                            self.push_lines(vec![format!("{mv} is missing from the move table.")]);
                            return false;
                        };
                        let old = std::mem::replace(&mut self.battle.combatants[combatant].moves[k], MoveSlot::new(row));
                        self.push_lines(vec![format!("1, 2 and... Poof! {who} forgot {} and learned {mv}!", old.data.name)]);
                    }
                    PromptChoice::GiveUp => self.push_lines(vec![format!("{who} did not learn {mv}.")]),
                    PromptChoice::Ignore => return false,
                }
                self.progress.queue.pop_front();
                self.start_next_step();
                true
            }
            Some(ProgressStep::Evolve { combatant, .. }) => {
                if input != BattleInput::Back {
                    return false;
                }
                let who = self.display_name(combatant);
                self.push_lines(vec![format!("Huh? {who} stopped evolving!")]);
                self.progress.queue.pop_front();
                self.start_next_step();
                true
            }
            None => false,
        }
    }

    /// Advance the evolution animation; completes the evolution after [`EVOLVE_SECONDS`].
    pub fn tick_progress(&mut self, dt: f32) {
        let Some(elapsed) = self.progress.evolve_elapsed.as_mut() else {
            return;
        };
        *elapsed += dt.max(0.0);
        if *elapsed < EVOLVE_SECONDS {
            return;
        }
        if let Some(ProgressStep::Evolve { combatant, evo }) = self.progress.current().cloned() {
            self.finish_evolution(combatant, &evo);
        }
        self.progress.queue.pop_front();
        self.start_next_step();
    }

    fn finish_evolution(&mut self, ci: usize, evo: &Evolution) {
        let who = self.display_name(ci);
        let row = self.battle_data().and_then(|d| d.species_row_by_dex(evo.target_species).ok().flatten());
        let (Some(row), Some(t)) = (row, self.progression.clone()) else {
            self.push_lines(vec![format!("{who} could not evolve (species #{} missing).", evo.target_species)]);
            return;
        };
        let mut m = self.progress_member(&t, ci);
        evolve(&t, &mut m, evo);
        evolve_combatant(&mut self.battle.combatants[ci], row);
        if let Some(meta) = self.member_meta.get_mut(ci) {
            *meta = MemberMeta { form: m.form, exp: Some(m.exp) };
        }
        let new_name = self.display_name(ci);
        self.push_lines(vec![format!("Congratulations! Your {who} evolved into {new_name}!")]);
    }

    /// Bag handler for progression items (rare candy, effort items) on the active Pokemon.
    /// Returns true when the item was used (the caller then spends the turn). A move that does
    /// not fit and a level-up evolution are queued for after the battle.
    pub fn use_progress_bag_item(&mut self, item_id: u16, label: &str) -> bool {
        let Some(t) = self.progression.clone() else {
            self.push_lines(vec![format!("{label} can't be used: progression tables are not loaded.")]);
            return false;
        };
        let ci = self.active_player();
        let who = self.display_name(ci);
        let mut m = self.progress_member(&t, ci);
        let before = m.effort;
        let level_item = t.item(item_id).is_some_and(|p| p.level_up);
        if level_item && m.level >= MAX_LEVEL {
            self.push_lines(vec![format!("It won't have any effect on {who}.")]);
            return false;
        }
        match use_progress_item(&t, &mut m, item_id) {
            Ok(out) => {
                self.bag.take(item_id);
                let mut lines = vec![format!("Used {label} on {who}.")];
                lines.extend(effort_lines(&who, &before, &m.effort));
                lines.extend(self.apply_progress(&t, ci, &m, &out));
                if !out.pending.is_empty() || self.progress.busy() {
                    lines.push("(new moves and evolution are offered after the battle)".into());
                }
                self.push_lines(lines);
                true
            }
            Err(e) => {
                let why = match e {
                    EffortError::AlreadyMax => "that effort is already at its maximum",
                    EffortError::AlreadyZero => "that effort is already at zero",
                    EffortError::BadStat | EffortError::NotAnEffortItem => "it is not a progression item",
                };
                self.push_lines(vec![format!("It won't have any effect on {who} ({why}).")]);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn award_lines_list_gain_levels_and_moves_in_order() {
        let out = AwardOutcome {
            gained: 340,
            old_level: 14,
            new_level: 16,
            learned: vec![(16, 44)],
            pending: vec![(15, 422)],
        };
        let name = |id: u16| if id == 44 { "bite".to_string() } else { "thunder-fang".to_string() };
        let lines = award_lines("SHINX", &out, &name);
        assert_eq!(
            lines,
            vec![
                "SHINX gained 340 Exp. Points!",
                "SHINX grew to level 15!",
                "SHINX grew to level 16!",
                "SHINX learned bite!",
            ]
        );
        let flat = AwardOutcome { gained: 0, old_level: 100, new_level: 100, ..AwardOutcome::default() };
        assert!(award_lines("X", &flat, &name).is_empty());
    }

    #[test]
    fn effort_lines_name_the_changed_stats() {
        let lines = effort_lines("PIKACHU", &[0, 1, 0, 0, 0, 10], &[0, 2, 0, 0, 0, 9]);
        assert_eq!(lines, vec!["PIKACHU's Attack effort rose to level 2!", "PIKACHU's Speed effort fell to level 9!"]);
    }

    #[test]
    fn prompt_keys_forget_a_slot_or_give_up() {
        assert_eq!(prompt_choice(BattleInput::Slot(0), 4), PromptChoice::Forget(0));
        assert_eq!(prompt_choice(BattleInput::Slot(3), 4), PromptChoice::Forget(3));
        assert_eq!(prompt_choice(BattleInput::Slot(3), 3), PromptChoice::Ignore, "empty slot");
        assert_eq!(prompt_choice(BattleInput::Back, 4), PromptChoice::GiveUp);
        assert_eq!(prompt_choice(BattleInput::Confirm, 4), PromptChoice::Ignore);
        assert_eq!(prompt_choice(BattleInput::Run, 4), PromptChoice::Ignore);
    }

    #[test]
    fn flow_progress_and_busy() {
        let mut f = ProgressFlow::default();
        assert!(!f.busy() && f.evolve_progress().is_none());
        f.queue.push_back(ProgressStep::LearnMove { combatant: 0, move_id: 1 });
        f.evolve_elapsed = Some(EVOLVE_SECONDS / 2.0);
        assert!(f.busy());
        assert_eq!(f.evolve_progress(), Some(0.5));
        f.evolve_elapsed = Some(EVOLVE_SECONDS * 3.0);
        assert_eq!(f.evolve_progress(), Some(1.0));
    }

    fn tables() -> Option<ProgressionTables> {
        ProgressionTables::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).ok()
    }

    /// Hand check of the ASSUMED yield: Bidoof base_exp from the table, level 5, one
    /// participant: floor(base * 5 / 7).
    #[test]
    fn battle_exp_uses_the_assumed_formula() {
        let Some(t) = tables() else { return };
        let base = u32::from(t.base_exp(399, 0));
        assert!(base > 0);
        assert_eq!(battle_exp(&t, 399, 0, 5, 1, false), base * 5 / 7);
        assert_eq!(battle_exp(&t, 399, 0, 5, 1, true), base * 5 / 7 * 3 / 2);
        assert_eq!(battle_exp(&t, 399, 0, 5, 2, false), (base * 5 / 7 / 2).max(1));
    }

    /// Shinx evolves into Luxio at 15 (method 4) and learns thunder-fang at 15 (both from the
    /// checked-in tables); the decision helper sees both only when the level changed.
    #[test]
    fn level_up_evolution_needs_a_level_change() {
        let Some(t) = tables() else { return };
        let mut m = ProgressMember { species: 403, form: 0, level: 14, exp: 0, moves: vec![33, 43, 44, 209], effort: [0; 6] };
        m.exp = progression::exp_for_level(&t, t.growth_group(403, 0), 15) - 1;
        let out = progression::award_exp(&t, &mut m, 1);
        assert_eq!((out.old_level, out.new_level), (14, 15));
        assert!(out.pending.iter().any(|(l, _)| *l == 15), "thunder-fang needs a slot: {out:?}");
        let evo = level_up_evolution(&t, &m, &out, vec![403]).expect("Shinx evolves at 15");
        assert_eq!((evo.target_species, evo.target_form), (404, 0));
        let none = AwardOutcome { old_level: 15, new_level: 15, ..AwardOutcome::default() };
        assert!(level_up_evolution(&t, &m, &none, vec![]).is_none());
    }
}
