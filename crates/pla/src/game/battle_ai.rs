//! Opponent move choice built on the engine's recovered AI score rules.
//!
//! Only the rules whose predicate is fully mapped in `BattleData::evaluate_rule` are used
//! (`basic_affinity_ineffective`, `strong_non_max_damage`). The random-gated rules and the rest
//! of the game's AI are not reproduced (UNKNOWN). Picking randomly among equal best scores and
//! skipping most status moves is a local simplification, not recovered behaviour.

use crate::battle::{AiFacts, BattleData};
use crate::battle_engine::{type_effectiveness, Action, Battle, BattleRng};

/// Rules evaluated for every candidate move.
pub const AI_RULES: [&str; 2] = ["basic_affinity_ineffective", "strong_non_max_damage"];

/// Score of each usable move slot of `actor` against `target` (None = no PP left).
pub fn score_moves(
    battle: &Battle,
    data: &BattleData,
    actor: usize,
    target: usize,
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
            let affinity = (damaging
                && type_effectiveness(data.type_chart(), &m.data.move_type, &foe.species.types)
                    == 0.0)
                .then_some(0);
            let facts = AiFacts {
                damaging_move: damaging,
                candidate_is_max_damage: best == Some(slot),
                affinity,
                target_hp_percent: Some((foe.hp * 100 / foe.max_hp.max(1)).min(100) as u8),
                ..AiFacts::default()
            };
            Some(data.score_rules(&AI_RULES, &facts).unwrap_or(0))
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
