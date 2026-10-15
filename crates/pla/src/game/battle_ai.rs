//! Opponent move choice built on the engine's recovered AI score rules.
//!
//! Only the rules whose predicate is fully mapped in `BattleData::evaluate_rule` are used
//! (`basic_affinity_ineffective`, `strong_non_max_damage`, the gated `strong_high_affinity_bonus`,
//! the Allowance HP tiers, `expert_friendly_fire` and the gated `expert_lock_on_bonus` whose
//! predicate is read from `battle.lua`). The Expert sequence handlers (per-`aiSeqNo` score
//! suites) are still not reproduced: the move table carries no `aiSeqNo` and their per-score
//! predicates are only partly mapped (UNKNOWN). Picking randomly among equal best scores and
//! skipping most status moves is a local simplification, not recovered behaviour.

use crate::battle::{AiFacts, BattleData, Table};
use crate::battle_engine::{type_effectiveness, Action, Battle, BattleRng};
use std::path::Path;

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

// ---------------------------------------------------------------- item AI

/// One branch of a decoded `ItemUseAI` rule (`data/ai/item_use_rules.tsv`), in evaluation
/// order. The first branch whose non-random gates hold and whose chance roll passes scores;
/// the last branch of every rule has no gates and chance 100 (the script's fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemUseBranch {
    /// `hp < hp_under` (percent); `None` = no HP gate.
    pub hp_under: Option<u8>,
    /// Only when the side has no other standing Pokemon (`GetStandByPokeCount == 0`).
    pub standby_zero: bool,
    /// Only when the active Pokemon has a status condition (`IsPokeSick` / confused /
    /// badly poisoned).
    pub status_any: bool,
    pub chance_pct: u8,
    pub score: i32,
}

/// One decoded rule: the item ids it dispatches from and its ordered branches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemUseRule {
    pub rule_id: String,
    pub items: Vec<u16>,
    pub branches: Vec<ItemUseBranch>,
}

/// The decoded `ItemUseAI` rules (`data/ai/item_use_rules.tsv`).
#[derive(Debug, Clone, Default)]
pub struct ItemUseTable {
    pub rules: Vec<ItemUseRule>,
}

impl ItemUseTable {
    pub fn load(data_root: impl AsRef<Path>) -> Result<Self, String> {
        let table = Table::load(&data_root.as_ref().join("ai").join("item_use_rules.tsv"), None)?;
        let mut rules: Vec<ItemUseRule> = Vec::new();
        for row in &table.rows {
            let rule_id = Table::value(row, "rule_id:string")?.to_owned();
            let entry = match rules.iter_mut().find(|r| r.rule_id == rule_id) {
                Some(entry) => entry,
                None => {
                    rules.push(ItemUseRule {
                        rule_id,
                        items: Vec::new(),
                        branches: Vec::new(),
                    });
                    rules.last_mut().expect("just pushed")
                }
            };
            if let Some(ids) = row.get("item_ids:string").filter(|v| !v.is_empty()) {
                if entry.items.is_empty() {
                    entry.items = ids
                        .split(',')
                        .filter_map(|id| id.trim().parse().ok())
                        .collect();
                }
            }
            let number = |key: &str| row.get(key).filter(|v| !v.is_empty());
            entry.branches.push(ItemUseBranch {
                hp_under: number("hp_under:u8?").and_then(|v| v.parse().ok()),
                standby_zero: row.get("standby_zero:bool").is_some_and(|v| v == "1"),
                status_any: row.get("status_any:bool").is_some_and(|v| v == "1"),
                chance_pct: Table::value(row, "chance_pct:u8")?
                    .parse()
                    .map_err(|_| "bad chance_pct")?,
                score: Table::value(row, "score:i32")?.parse().map_err(|_| "bad score")?,
            });
        }
        Ok(Self { rules })
    }

    pub fn rule_for_item(&self, item: u16) -> Option<&ItemUseRule> {
        self.rules.iter().find(|r| r.items.contains(&item))
    }

    /// Score of using `item` on `actor` right now, or `None` when no decoded rule covers the
    /// item (the aux-power handlers 1615-1619 and 1756-1758 are not decoded: UNKNOWN).
    pub fn score_item(
        &self,
        battle: &Battle,
        actor: usize,
        item: u16,
        rng: &mut impl BattleRng,
    ) -> Option<i32> {
        let rule = self.rule_for_item(item)?;
        let c = battle.combatants.get(actor)?;
        let hp_pct = (c.hp.saturating_mul(100) / c.max_hp.max(1)) as u8;
        let side = battle.sides.iter().position(|s| s.contains(&actor))?;
        let standby = battle.sides[side]
            .iter()
            .filter(|&&i| i != actor && battle.combatants[i].is_alive())
            .count();
        let statused = c.status.is_some() || battle.is_confused(actor);
        for branch in &rule.branches {
            if branch.hp_under.is_some_and(|limit| hp_pct >= limit) {
                continue;
            }
            if branch.standby_zero && standby != 0 {
                continue;
            }
            if branch.status_any && !statused {
                continue;
            }
            if branch.chance_pct >= 100 || rng.below(100) < u32::from(branch.chance_pct) {
                return Some(branch.score);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle_engine::{Combatant, MajorStatus, XorShift32};
    use crate::game::trainer_battle::choose_item;
    use std::path::PathBuf;

    fn data() -> BattleData {
        BattleData::load(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
    }

    fn table() -> ItemUseTable {
        ItemUseTable::load(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
    }

    fn combatant(data: &BattleData, species: &str, level: u8) -> Combatant {
        let species = data.species_row(species).unwrap().unwrap();
        let moves = vec![data.move_row("tackle").unwrap().unwrap()];
        Combatant::from_base(species, level, moves)
    }

    /// Side 0 is the player, side 1 the item user (the trainer), as in a trainer battle.
    fn battle(data: &BattleData, actor: Combatant, foe: Combatant) -> Battle {
        Battle::new(vec![foe, actor], vec![vec![0], vec![1]], data.type_chart().clone()).unwrap()
    }

    #[test]
    fn affinity_levels_fit_the_recovered_thresholds() {
        assert_eq!(affinity_level(0.0), 0);
        assert_eq!(affinity_level(0.25), 5);
        assert_eq!(affinity_level(0.5), 6);
        assert_eq!(affinity_level(1.0), 7);
        assert_eq!(affinity_level(2.0), 8);
        assert_eq!(affinity_level(4.0), 9);
    }

    #[test]
    fn loads_the_four_decoded_item_rules() {
        let t = table();
        let ids: Vec<&str> = t.rules.iter().map(|r| r.rule_id.as_str()).collect();
        assert_eq!(ids, ["itemAI_001", "itemAI_023", "itemAI_027", "itemAI_057"]);
        let branches: Vec<usize> = t.rules.iter().map(|r| r.branches.len()).collect();
        assert_eq!(branches, [7, 9, 4, 2], "every rule ends with its -20 fallback");
        assert_eq!(t.rule_for_item(24).unwrap().rule_id, "itemAI_001", "Max Potion");
        assert_eq!(t.rule_for_item(155).unwrap().rule_id, "itemAI_001", "Oran Berry");
        assert_eq!(t.rule_for_item(23).unwrap().rule_id, "itemAI_023", "Full Restore");
        assert_eq!(t.rule_for_item(157).unwrap().rule_id, "itemAI_027", "Lum Berry");
        assert_eq!(t.rule_for_item(57).unwrap().rule_id, "itemAI_057", "X Attack");
        assert!(t.rule_for_item(1615).is_none(), "aux-power items stay UNKNOWN");
    }

    #[test]
    fn full_restore_fires_on_a_low_hp_last_pokemon() {
        let d = data();
        let mut actor = combatant(&d, "garchomp", 50);
        let foe = combatant(&d, "pikachu", 50);
        let mut b = battle(&d, actor.clone(), foe);
        actor.hp = (actor.max_hp * 10) / 100;
        b.combatants[1] = actor;
        let t = table();
        let mut rng = XorShift32(1);
        assert_eq!(t.score_item(&b, 1, 23, &mut rng), Some(10), "hp<20 and no standby: always");
    }

    #[test]
    fn high_hp_never_uses_an_item() {
        let d = data();
        let foe = combatant(&d, "pikachu", 50);
        let mut b = battle(&d, combatant(&d, "garchomp", 50), foe);
        let t = table();
        // Full health: no HP branch matches, so the -20 fallback scores on every seed.
        for seed in 1..40 {
            let mut rng = XorShift32(seed);
            assert_eq!(t.score_item(&b, 1, 23, &mut rng), Some(-20), "seed {seed}");
            assert_eq!(choose_item(&b, 1, &[23], Some(&t), &mut XorShift32(seed)), None, "seed {seed}");
        }
        b.combatants[1].hp = 0;
        assert_eq!(choose_item(&b, 1, &[23], Some(&t), &mut XorShift32(1)), None, "a fainted Pokemon uses nothing");
    }

    #[test]
    fn full_heal_needs_a_status_condition() {
        let d = data();
        let t = table();
        let foe = combatant(&d, "pikachu", 50);
        let mut b = battle(&d, combatant(&d, "garchomp", 50), foe);
        // Healthy: the rule only scores its fallback.
        let mut rng = XorShift32(5);
        assert_eq!(t.score_item(&b, 1, 27, &mut rng), Some(-20));
        // Poisoned and last member: the status branch always scores 10.
        b.combatants[1].status = Some(MajorStatus::Poison);
        let mut rng = XorShift32(5);
        assert_eq!(t.score_item(&b, 1, 27, &mut rng), Some(10));
        assert_eq!(choose_item(&b, 1, &[27], Some(&t), &mut XorShift32(5)), Some(27));
    }

    #[test]
    fn x_items_use_their_chance_gate() {
        let d = data();
        let t = table();
        let foe = combatant(&d, "pikachu", 50);
        let mut b = battle(&d, combatant(&d, "garchomp", 50), foe);
        // 78 % branch: some seeds score 2, others fall through to -20.
        let mut some = false;
        let mut never = false;
        for seed in 1..40 {
            match t.score_item(&b, 1, 57, &mut XorShift32(seed)) {
                Some(2) => some = true,
                Some(-20) => never = true,
                other => panic!("unexpected score {other:?}"),
            }
        }
        assert!(some && never, "the 78 % gate must separate seeds");
        let chosen: Vec<u32> = (1..40)
            .filter(|seed| choose_item(&b, 1, &[57], Some(&t), &mut XorShift32(*seed)).is_some())
            .collect();
        assert!(!chosen.is_empty() && chosen.len() < 39, "both outcomes appear: {chosen:?}");
    }
}
