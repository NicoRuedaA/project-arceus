//! Opponent move choice built on the engine's recovered AI score rules.
//!
//! Only the rules whose predicate is fully mapped in `BattleData::evaluate_rule` are used
//! (`basic_affinity_ineffective`, `strong_non_max_damage`, the gated `strong_high_affinity_bonus`,
//! the Allowance HP tiers, `expert_friendly_fire` and the gated `expert_lock_on_bonus` whose
//! predicate is read from `battle.lua`). The Expert sequence handlers (per-`aiSeqNo` score
//! suites) are still not reproduced: the move table carries no `aiSeqNo` and their per-score
//! predicates are only partly mapped (UNKNOWN). Picking randomly among equal best scores and
//! skipping most status moves is a local simplification, not recovered behaviour.

use crate::battle::{AiFacts, BattleData};
use crate::battle_engine::{type_effectiveness, Action, Battle, BattleRng};

/// Rules evaluated for every candidate move of a wild Pokemon.
pub const AI_RULES: [&str; 2] = ["basic_affinity_ineffective", "strong_non_max_damage"];

/// The gated Strong rule used by trainer AI (its observed 70 % chance is read from
/// `data/ai/score_rules.tsv`; see [`BattleData::rule_chance_pct`]).
pub const HIGH_AFFINITY_RULE: &str = "strong_high_affinity_bonus";

/// Native affinity value of a type multiplier.
///
/// ASSUMED: 0 = no effect, otherwise `7 + log2(multiplier)` (x1/4 = 5, x1/2 = 6, x1 = 7, x2 = 8,
/// x4 = 9). The scale is native and not recovered (`score_rules.tsv` notes); this reading makes
/// every recovered threshold coherent: "affinity == 0" = immune, "> 7" = super effective,
/// "1..=6" = resisted, ">= 9" = double super effective.
pub fn affinity_level(multiplier: f32) -> i32 {
    if multiplier <= 0.0 {
        0
    } else {
        7 + multiplier.log2().round() as i32
    }
}

/// Score of each usable move slot of `actor` against `target` (None = no PP left), with the
/// wild-Pokemon rules ([`AI_RULES`]).
pub fn score_moves(
    battle: &Battle,
    data: &BattleData,
    actor: usize,
    target: usize,
) -> Vec<Option<i32>> {
    score_with(battle, data, actor, target, &AI_RULES, &mut |_| None)
}

/// Score with an explicit rule list (trainer AI modules). Each gated rule gets its own
/// observed roll, with that rule's sheet percentage ([`BattleData::rule_chance_pct`]).
pub fn score_moves_rules<R: BattleRng>(
    battle: &Battle,
    data: &BattleData,
    actor: usize,
    target: usize,
    rules: &[&str],
    rng: &mut R,
) -> Vec<Option<i32>> {
    score_with(battle, data, actor, target, rules, &mut |id| {
        data.rule_chance_pct(id).map(|pct| rng.below(100) < u32::from(pct))
    })
}

fn score_with(
    battle: &Battle,
    data: &BattleData,
    actor: usize,
    target: usize,
    rules: &[&str],
    gate: &mut dyn FnMut(&str) -> Option<bool>,
) -> Vec<Option<i32>> {
    let best = battle.best_damage_action(actor).map(|a| a.move_slot);
    let me = &battle.combatants[actor];
    let foe = &battle.combatants[target];
    me.moves
        .iter()
        .enumerate()
        .map(|(slot, m)| {
            if m.pp == 0 {
                return None;
            }
            let damaging = m.data.power > 0;
            let affinity = damaging.then(|| {
                affinity_level(type_effectiveness(data.type_chart(), &m.data.move_type, &foe.species.types))
            });
            // One rule at a time: gated rules roll separately, exactly like the script's
            // per-check IfRand.
            let total = rules.iter().fold(0_i32, |acc, id| {
                let facts = AiFacts {
                    damaging_move: damaging,
                    candidate_is_max_damage: best == Some(slot),
                    affinity,
                    target_hp_percent: Some((foe.hp * 100 / foe.max_hp.max(1)).min(100) as u8),
                    move_id: m.data.id,
                    // The port does not model the lock-on state (UNKNOWN): the expert rule
                    // stays dormant instead of being guessed.
                    attacker_lock_on: false,
                    probability_gate_passed: gate(id),
                    ..AiFacts::default()
                };
                match data.evaluate_rule(id, &facts) {
                    Ok(crate::battle::RuleOutcome::Applied(delta)) => acc.saturating_add(delta),
                    Ok(_) => acc,
                    Err(_) => acc,
                }
            });
            Some(total)
        })
        .collect()
}

/// Choose the opponent's action: highest rule score, random among ties.
pub fn choose_ai_action(
    battle: &Battle,
    data: &BattleData,
    actor: usize,
    target: usize,
    rng: &mut impl BattleRng,
) -> Action {
    let scores = score_moves(battle, data, actor, target);
    pick(battle, actor, &scores, rng)
}

/// [`choose_ai_action`] with an explicit rule list (trainer AI modules).
pub fn choose_ai_action_rules<R: BattleRng>(
    battle: &Battle,
    data: &BattleData,
    actor: usize,
    target: usize,
    rules: &[&str],
    rng: &mut R,
) -> Action {
    let scores = score_moves_rules(battle, data, actor, target, rules, rng);
    pick(battle, actor, &scores, rng)
}

fn pick(battle: &Battle, actor: usize, scores: &[Option<i32>], rng: &mut impl BattleRng) -> Action {
    let moves = &battle.combatants[actor].moves;
    // Local simplification: status moves are tried one time in four.
    let allow_status = rng.below(4) == 0;
    let eligible: Vec<(usize, i32)> = scores
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.map(|s| (i, s)))
        .filter(|(i, _)| allow_status || moves[*i].data.power > 0)
        .collect();
    let pool = if eligible.is_empty() {
        scores
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.map(|s| (i, s)))
            .collect()
    } else {
        eligible
    };
    let Some(top) = pool.iter().map(|(_, s)| *s).max() else {
        return Action {
            actor,
            move_slot: 0,
        };
    };
    let best: Vec<usize> = pool
        .iter()
        .filter(|(_, s)| *s == top)
        .map(|(i, _)| *i)
        .collect();
    let move_slot = best[rng.below(best.len() as u32) as usize];
    Action { actor, move_slot }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affinity_levels_fit_the_recovered_thresholds() {
        assert_eq!(affinity_level(0.0), 0);
        assert_eq!(affinity_level(0.25), 5);
        assert_eq!(affinity_level(0.5), 6);
        assert_eq!(affinity_level(1.0), 7);
        assert_eq!(affinity_level(2.0), 8);
        assert_eq!(affinity_level(4.0), 9);
    }
}
