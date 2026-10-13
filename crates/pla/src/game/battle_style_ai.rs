//! Opponent style choice (agile / strong / normal) for the PLA turn flow.
//!
//! ASSUMED rule, written for the port. The game scores styles in `FUN_0081f190` (strong) and
//! `FUN_0081f97c` (agile) by simulating the action queue, with the `modeChangeAi_*` weights
//! (normal 10; strong -10 unnecessary, +20 can knock out, -6 behind, +6 not behind; agile
//! -10 cannot knock out, +15 can act again first; see `reports/data-progress/battle_styles.md`).
//! The exact predicates were not traced, so this is a deterministic simplification that uses
//! the same ideas in a fixed order:
//!
//! 1. no style data, styles not usable or a status move: normal;
//! 2. the normal move is expected to knock out: normal (strong would be unnecessary);
//! 3. only the strong move is expected to knock out: strong;
//! 4. after a strong use the user still acts before the target (the target is slow): strong;
//! 5. after an agile use the user acts again before the target: agile;
//! 6. otherwise normal.
//!
//! "Expected to knock out" uses the mean damage roll (x0.925), STAB x1.25 and type
//! effectiveness, the same estimate as `Battle::best_damage_action`; no critical hits.

use crate::battle::{BattleMove, TypeChart};
use crate::battle_engine::rules::base_damage;
use crate::battle_engine::styles::{MoveStyleData, Style};
use crate::battle_engine::{type_effectiveness, Battle, Combatant};

/// Mean damage of `power` for this move (no randomness, no critical hit).
pub fn expected_damage(
    battle: &Battle,
    chart: &TypeChart,
    attacker: &Combatant,
    target: &Combatant,
    mv: &BattleMove,
    power: u16,
) -> f32 {
    if power == 0 {
        return 0.0;
    }
    let (atk, def) = if mv.category == "Physical" {
        (attacker.attack, target.defense)
    } else {
        (attacker.special_attack, target.special_defense)
    };
    let base = base_damage(
        battle.pla_damage_formula(),
        u32::from(power),
        atk,
        u32::from(attacker.level),
        def,
    );
    let stab = attacker.species.types.iter().any(|t| t == &mv.move_type);
    let eff = type_effectiveness(chart, &mv.move_type, &target.species.types);
    base as f32 * if stab { 1.25 } else { 1.0 } * eff * 0.925
}

fn side_of(battle: &Battle, combatant: usize) -> Option<usize> {
    battle.sides.iter().position(|s| s.contains(&combatant))
}

/// Does `actor` act next after using `style`, given the action points it would add?
fn acts_next_after(battle: &Battle, actor: usize, target: usize, data: &MoveStyleData, style: Style) -> bool {
    let (Some(queue), Some(a_side), Some(t_side)) =
        (battle.action_queue(), side_of(battle, actor), side_of(battle, target))
    else {
        return false;
    };
    let streak = battle.agile_streak(actor);
    let mut q = queue.clone();
    q.add_points(a_side, data.self_action_points(style, streak));
    q.add_points(t_side, data.target_action_points(style, streak));
    q.settle(Some(a_side));
    q.next().is_some_and(|e| e.side == a_side)
}

/// The style the opponent uses for `move_slot` against `target` (rule in the module docs).
/// `chart` is the battle's type chart (`BattleData::type_chart`).
pub fn choose_ai_style(battle: &Battle, chart: &TypeChart, actor: usize, move_slot: usize, target: usize) -> Style {
    let Some(slot) = battle.combatants.get(actor).and_then(|c| c.moves.get(move_slot)) else {
        return Style::Normal;
    };
    let Some(data) = battle.move_styles().and_then(|t| t.get(&slot.data.name)) else {
        return Style::Normal;
    };
    if !data.styles_usable || slot.data.power == 0 {
        return Style::Normal;
    }
    let me = &battle.combatants[actor];
    let foe = &battle.combatants[target];
    let hp = foe.hp as f32;
    let normal = expected_damage(battle, chart, me, foe, &slot.data, data.power(Style::Normal));
    if normal >= hp {
        return Style::Normal;
    }
    let strong = expected_damage(battle, chart, me, foe, &slot.data, data.power(Style::Strong));
    if strong >= hp {
        return Style::Strong;
    }
    if acts_next_after(battle, actor, target, data, Style::Strong) {
        return Style::Strong;
    }
    if acts_next_after(battle, actor, target, data, Style::Agile) {
        return Style::Agile;
    }
    Style::Normal
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::BattleData;
    use crate::battle_engine::styles::MoveStyleTable;
    use std::path::Path;

    fn setup(attacker_level: u8, foe_level: u8) -> Option<(Battle, TypeChart)> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let data = BattleData::load(&root).ok()?;
        let table = MoveStyleTable::load(&root).ok()?;
        let tackle = data.move_row("tackle").ok()??;
        let growl = data.move_row("growl").ok()??;
        let a = Combatant::from_base(data.species_row("bidoof").ok()??, attacker_level, vec![tackle.clone(), growl]);
        let b = Combatant::from_base(data.species_row("bidoof").ok()??, foe_level, vec![tackle]);
        let mut battle = Battle::new(vec![a, b], vec![vec![0], vec![1]], data.type_chart().clone()).ok()?;
        battle.set_move_styles(table);
        battle.enable_action_queue();
        Some((battle, data.type_chart().clone()))
    }

    #[test]
    fn status_moves_and_missing_tables_stay_normal() {
        let Some((mut b, c)) = setup(20, 20) else { return };
        assert_eq!(choose_ai_style(&b, &c, 0, 1, 1), Style::Normal, "growl has no power");
        b.combatants[0].moves[0].data.name = "not-a-move".into();
        assert_eq!(choose_ai_style(&b, &c, 0, 0, 1), Style::Normal, "no style row");
    }

    #[test]
    fn a_knockout_picks_normal_and_a_strong_only_knockout_picks_strong() {
        let Some((mut b, c)) = setup(30, 30) else { return };
        b.combatants[1].hp = 1;
        assert_eq!(choose_ai_style(&b, &c, 0, 0, 1), Style::Normal, "normal already knocks out");
        let normal = expected_damage(&b, &c, &b.combatants[0], &b.combatants[1], &b.combatants[0].moves[0].data, 40);
        let data = b.move_styles().unwrap().get("tackle").unwrap().clone();
        let strong = expected_damage(
            &b,
            &c,
            &b.combatants[0],
            &b.combatants[1],
            &b.combatants[0].moves[0].data,
            data.power(Style::Strong),
        );
        assert!(strong > normal);
        // HP between the two estimates: only strong knocks out.
        b.combatants[1].hp = ((normal + strong) / 2.0) as u32;
        assert_eq!(choose_ai_style(&b, &c, 0, 0, 1), Style::Strong);
    }

    #[test]
    fn a_much_slower_foe_draws_strong_and_equal_speed_stays_normal() {
        // Equal speed: a strong use (+5) hands the next turn to the foe, an agile use pushes
        // the foe back 3 but the user restarts its full wait, so neither keeps the lead.
        let Some((even, c)) = setup(30, 30) else { return };
        assert_eq!(choose_ai_style(&even, &c, 0, 0, 1), Style::Normal);
        // Speed 1000 (base wait 1) vs speed 1 (base wait 15): after a strong use the user
        // waits 1 + 5 = 6 ticks, the foe 15 - 1 = 14, so the user still acts first. The foe
        // gets a big HP pool so no style is expected to knock it out.
        let Some((mut fast, c)) = setup(30, 30) else { return };
        fast.combatants[0].speed = 1_000;
        fast.combatants[1].speed = 1;
        fast.combatants[1].max_hp = 10_000;
        fast.combatants[1].hp = 10_000;
        fast.enable_action_queue();
        assert_eq!(choose_ai_style(&fast, &c, 0, 0, 1), Style::Strong);
    }
}
