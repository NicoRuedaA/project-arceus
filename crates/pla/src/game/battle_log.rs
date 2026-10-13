//! Battle log text built from engine events (pure, no Bevy).

use crate::battle_engine::effects::{EffectEvent, Stat};
use crate::battle_engine::statuses::StatusEvent;
use crate::battle_engine::{Event, MajorStatus};

/// How many log lines the battle box keeps on screen.
pub const LOG_VISIBLE_LINES: usize = 6;

pub fn status_name(status: MajorStatus) -> &'static str {
    match status {
        MajorStatus::Drowsy => "drowsy",
        MajorStatus::Paralysis => "paralyzed",
        MajorStatus::Poison => "poisoned",
        MajorStatus::Burn => "burned",
        MajorStatus::Frostbite => "frostbitten",
    }
}

pub fn stat_name(stat: Stat) -> &'static str {
    match stat {
        Stat::Attack => "Attack",
        Stat::Defense => "Defense",
        Stat::SpecialAttack => "Sp. Atk",
        Stat::SpecialDefense => "Sp. Def",
        Stat::Speed => "Speed",
        Stat::Accuracy => "Accuracy",
        Stat::Evasion => "Evasion",
        Stat::AllBattleStats => "all stats",
    }
}

/// Plain-text lines for one resolved turn. `name(i)` gives the display name of combatant `i`.
/// Effect lines (stages, drain, healing, multi-hit) come from a separate engine stream, so they
/// follow the main events of the turn; the end-of-battle line stays last.
///
/// `statuses` are the volatile-status events (`Battle::take_status_events`) recorded during the
/// same turn: confusion, trapping, charge/recharge, Protect and sleep. The engine stores them on a
/// separate stream with no position inside `events`, so exact interleaving is not possible; their
/// own emission order is kept and they print after the main and effect lines, before the
/// end-of-battle line.
///
/// The engine emits no `UsedMove` event for moves without power, so `status_moves` lists the
/// `(actor, move name)` pairs of such moves chosen this turn; each is announced at the first
/// point where that actor demonstrably acted (after `ActedFirst`, before an event caused by it,
/// or after all events), unless the engine reported a miss or an inability to act for it.
pub fn format_turn(
    name: &dyn Fn(usize) -> String,
    events: &[Event],
    effects: &[EffectEvent],
    statuses: &[StatusEvent],
    status_moves: &[(usize, String)],
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut ended = Vec::new();
    let mut pending: Vec<(usize, String)> = status_moves
        .iter()
        .filter(|(a, _)| {
            !events.iter().any(|e| {
                matches!(e, Event::Missed { actor, .. } | Event::UnableToAct { actor, .. } if actor == a)
            })
        })
        .cloned()
        .collect();
    let announce = |lines: &mut Vec<String>, pending: &mut Vec<(usize, String)>, who: &dyn Fn(usize) -> bool| {
        let mut i = 0;
        while i < pending.len() {
            if who(pending[i].0) {
                let (a, m) = pending.remove(i);
                lines.push(format!("{} used {m}!", name(a)));
            } else {
                i += 1;
            }
        }
    };
    for ev in events {
        // Events caused by a status move announce its user first.
        match ev {
            Event::StatusApplied { target, .. } => {
                announce(&mut lines, &mut pending, &|a| a != *target);
            }
            Event::UsedMove { actor, .. } => {
                announce(&mut lines, &mut pending, &|a| a == *actor);
            }
            _ => {}
        }
        match ev {
            Event::TurnStarted(t) => lines.push(format!("-- Turn {t} --")),
            Event::ActedFirst(first) => {
                announce(&mut lines, &mut pending, &|a| a == *first);
            }
            Event::UsedMove {
                actor,
                name: mv,
                damage,
                critical,
                effectiveness,
                ..
            } => {
                let crit = if *critical { ", critical hit" } else { "" };
                if *damage == 0 && effectiveness == "no effect" {
                    lines.push(format!("{} used {mv}! It had no effect.", name(*actor)));
                } else {
                    lines.push(format!(
                        "{} used {mv}! ({damage} dmg{crit}, {effectiveness})",
                        name(*actor)
                    ));
                }
            }
            Event::Missed { actor, name: mv, .. } => {
                lines.push(format!("{}'s {mv} missed!", name(*actor)));
            }
            Event::StatusApplied { target, status } => lines.push(format!(
                "{} was {}! [{}]",
                name(*target),
                status_name(*status),
                super::battle_items::status_tag(*status)
            )),
            Event::UnableToAct { actor, status } => lines.push(match status {
                MajorStatus::Paralysis => format!("{} is paralyzed and can't move!", name(*actor)),
                MajorStatus::Drowsy => format!("{} is drowsy and didn't act!", name(*actor)),
                s => format!("{} is {} and can't move!", name(*actor), status_name(*s)),
            }),
            Event::Recoil {
                target,
                status,
                damage,
            } => lines.push(format!(
                "{} is hurt by being {} (-{damage} HP)",
                name(*target),
                status_name(*status)
            )),
            Event::KnockedOut(i) => lines.push(format!("{} fainted!", name(*i))),
            Event::Switched { to, .. } => lines.push(format!("Go! {}!", name(*to))),
            Event::UsedHealingItem {
                combatant,
                item,
                healed,
            } => lines.push(format!("{} used {item}: +{healed} HP", name(*combatant))),
            Event::BattleEnded { winner_side } => ended.push(match winner_side {
                Some(0) => "Battle over: you won!".to_string(),
                Some(_) => "Battle over: you have no Pokemon left!".to_string(),
                None => "Battle over: draw.".to_string(),
            }),
        }
    }
    announce(&mut lines, &mut pending, &|_| true);
    lines.extend(effects.iter().filter_map(|e| format_effect(name, e)));
    lines.extend(statuses.iter().filter_map(|e| format_status(name, e)));
    lines.extend(ended);
    lines
}

/// One log line for a volatile-status event, or `None` when nothing should be shown.
/// `name(i)` gives the display name of combatant `i`. Failed actions that already have a main
/// `Event` (paralysis/drowsiness `UnableToAct`) are not repeated here; the remaining status events
/// have no main-event counterpart, so each maps to exactly one line.
pub fn format_status(name: &dyn Fn(usize) -> String, e: &StatusEvent) -> Option<String> {
    Some(match e {
        StatusEvent::Confused { target } => format!("{} became confused!", name(*target)),
        StatusEvent::SnappedOutOfConfusion { target } => format!("{} snapped out of confusion!", name(*target)),
        StatusEvent::HurtByConfusion { actor, damage } => format!("{} is confused and hurt itself! (-{damage} HP)", name(*actor)),
        StatusEvent::WokeUp { target } => format!("{} woke up!", name(*target)),
        StatusEvent::Trapped { target, by } => format!("{} was trapped by {}!", name(*target), name(*by)),
        StatusEvent::HurtByTrap { target, damage } => format!("{} is hurt by the trap! (-{damage} HP)", name(*target)),
        StatusEvent::TrapEnded { target } => format!("{} was freed from the trap!", name(*target)),
        StatusEvent::CantSwitch { combatant, .. } => format!("{} can't switch out!", name(*combatant)),
        StatusEvent::Charging { actor, move_name } => format!("{} is charging {move_name}!", name(*actor)),
        StatusEvent::MustRecharge { actor } => format!("{} must recharge!", name(*actor)),
        StatusEvent::Recharging { actor } => format!("{} is recharging!", name(*actor)),
        StatusEvent::ProtectUp { actor } => format!("{} is protected!", name(*actor)),
        StatusEvent::ProtectFailed { actor } => format!("{}'s protection failed!", name(*actor)),
        StatusEvent::ProtectedFrom { target, attacker, move_name } => format!("{} protected itself from {}'s {move_name}!", name(*target), name(*attacker)),
    })
}

pub fn format_effect(name: &dyn Fn(usize) -> String, e: &EffectEvent) -> Option<String> {
    Some(match e {
        EffectEvent::Hits { count, .. } if *count > 1 => format!("Hit {count} times!"),
        EffectEvent::Hits { .. } => return None,
        EffectEvent::StagesChanged {
            target,
            stat,
            new_state,
        } => match new_state {
            s if *s > 0 => format!("{}'s {} rose! [{}+]", name(*target), stat_name(*stat), short_stat(*stat)),
            s if *s < 0 => format!("{}'s {} fell! [{}-]", name(*target), stat_name(*stat), short_stat(*stat)),
            _ => format!("{}'s {} returned to normal.", name(*target), stat_name(*stat)),
        },
        EffectEvent::StageAlreadyAtLimit { target, stat } => {
            format!("{}'s {} can't go any further!", name(*target), stat_name(*stat))
        }
        EffectEvent::Healed { target, amount } => {
            format!("{} regained {amount} HP!", name(*target))
        }
        EffectEvent::Drained { actor, amount } => {
            format!("{} drained {amount} HP!", name(*actor))
        }
        EffectEvent::RecoilDamage { actor, amount } => {
            format!("{} was hurt by recoil (-{amount} HP)!", name(*actor))
        }
        EffectEvent::HpCost { actor, amount } => {
            format!("{} paid {amount} HP!", name(*actor))
        }
        EffectEvent::Flinched { target } => format!("{} flinched!", name(*target)),
        EffectEvent::FlinchedAndSkipped { actor } => {
            format!("{} flinched and couldn't move!", name(*actor))
        }
    })
}

/// Three-letter tag used on the HP cards.
pub fn short_stat(stat: Stat) -> &'static str {
    match stat {
        Stat::Attack => "ATK",
        Stat::Defense => "DEF",
        Stat::SpecialAttack => "SPA",
        Stat::SpecialDefense => "SPD",
        Stat::Speed => "SPE",
        Stat::Accuracy => "ACC",
        Stat::Evasion => "EVA",
        Stat::AllBattleStats => "ALL",
    }
}

/// Keep only the newest `LOG_VISIBLE_LINES` lines for the on-screen box.
pub fn tail(lines: &[String]) -> String {
    let start = lines.len().saturating_sub(LOG_VISIBLE_LINES);
    lines[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(i: usize) -> String {
        ["PIKACHU", "Wild BIDOOF"][i % 2].to_string()
    }

    #[test]
    fn formats_damage_status_and_effects_in_order() {
        let events = vec![
            Event::TurnStarted(2),
            Event::UsedMove {
                actor: 0,
                target: 1,
                name: "nuzzle".into(),
                damage: 7,
                critical: true,
                effectiveness: "super effective".into(),
            },
            Event::StatusApplied {
                target: 1,
                status: MajorStatus::Paralysis,
            },
            Event::BattleEnded {
                winner_side: Some(0),
            },
        ];
        let fx = vec![
            EffectEvent::Hits { actor: 0, count: 2 },
            EffectEvent::Drained { actor: 0, amount: 3 },
        ];
        let lines = format_turn(&names, &events, &fx, &[], &[]);
        assert_eq!(lines[0], "-- Turn 2 --");
        assert_eq!(
            lines[1],
            "PIKACHU used nuzzle! (7 dmg, critical hit, super effective)"
        );
        assert_eq!(lines[2], "Wild BIDOOF was paralyzed! [PAR]");
        assert_eq!(lines[3], "Hit 2 times!");
        assert_eq!(lines[4], "PIKACHU drained 3 HP!");
        assert_eq!(lines[5], "Battle over: you won!");
    }

    #[test]
    fn status_moves_are_announced_before_their_effect() {
        let events = vec![
            Event::TurnStarted(1),
            Event::ActedFirst(0),
            Event::StatusApplied {
                target: 1,
                status: MajorStatus::Paralysis,
            },
            Event::UsedMove {
                actor: 1,
                target: 0,
                name: "tackle".into(),
                damage: 3,
                critical: false,
                effectiveness: "normal".into(),
            },
        ];
        let lines = format_turn(&names, &events, &[], &[], &[(0, "thunder-wave".into())]);
        assert_eq!(lines[1], "PIKACHU used thunder-wave!");
        assert_eq!(lines[2], "Wild BIDOOF was paralyzed! [PAR]");
        // a missed status move is reported by the engine, not announced twice
        let missed = vec![Event::TurnStarted(1), Event::Missed { actor: 0, target: 1, name: "thunder-wave".into() }];
        let lines = format_turn(&names, &missed, &[], &[], &[(0, "thunder-wave".into())]);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn every_status_event_maps_to_one_line() {
        let cases = [
            (StatusEvent::Confused { target: 1 }, "Wild BIDOOF became confused!"),
            (StatusEvent::SnappedOutOfConfusion { target: 1 }, "Wild BIDOOF snapped out of confusion!"),
            (StatusEvent::HurtByConfusion { actor: 0, damage: 4 }, "PIKACHU is confused and hurt itself! (-4 HP)"),
            (StatusEvent::WokeUp { target: 1 }, "Wild BIDOOF woke up!"),
            (StatusEvent::Trapped { target: 1, by: 0 }, "Wild BIDOOF was trapped by PIKACHU!"),
            (StatusEvent::HurtByTrap { target: 1, damage: 9 }, "Wild BIDOOF is hurt by the trap! (-9 HP)"),
            (StatusEvent::TrapEnded { target: 1 }, "Wild BIDOOF was freed from the trap!"),
            (StatusEvent::CantSwitch { side: 0, combatant: 0 }, "PIKACHU can't switch out!"),
            (StatusEvent::Charging { actor: 0, move_name: "solar-beam".into() }, "PIKACHU is charging solar-beam!"),
            (StatusEvent::MustRecharge { actor: 0 }, "PIKACHU must recharge!"),
            (StatusEvent::Recharging { actor: 0 }, "PIKACHU is recharging!"),
            (StatusEvent::ProtectUp { actor: 1 }, "Wild BIDOOF is protected!"),
            (StatusEvent::ProtectFailed { actor: 1 }, "Wild BIDOOF's protection failed!"),
            (
                StatusEvent::ProtectedFrom { target: 1, attacker: 0, move_name: "tackle".into() },
                "Wild BIDOOF protected itself from PIKACHU's tackle!",
            ),
        ];
        for (event, expected) in cases {
            assert_eq!(format_status(&names, &event).as_deref(), Some(expected));
        }
    }

    #[test]
    fn status_lines_follow_the_turn_lines_in_engine_order() {
        let events = vec![
            Event::TurnStarted(1),
            Event::UsedMove {
                actor: 0,
                target: 1,
                name: "confuse-ray".into(),
                damage: 0,
                critical: false,
                effectiveness: "normal".into(),
            },
        ];
        let statuses = [
            StatusEvent::Confused { target: 1 },
            StatusEvent::HurtByTrap { target: 1, damage: 5 },
        ];
        let fx = [EffectEvent::Hits { actor: 0, count: 2 }];
        let lines = format_turn(&names, &events, &fx, &statuses, &[]);
        assert_eq!(
            lines,
            vec![
                "-- Turn 1 --".to_string(),
                "PIKACHU used confuse-ray! (0 dmg, normal)".to_string(),
                "Hit 2 times!".to_string(),
                "Wild BIDOOF became confused!".to_string(),
                "Wild BIDOOF is hurt by the trap! (-5 HP)".to_string(),
            ]
        );
    }

    #[test]
    fn stage_changes_name_the_direction() {
        let up = EffectEvent::StagesChanged {
            target: 0,
            stat: Stat::Speed,
            new_state: 1,
        };
        let down = EffectEvent::StagesChanged {
            target: 1,
            stat: Stat::Defense,
            new_state: -1,
        };
        assert_eq!(format_effect(&names, &up).unwrap(), "PIKACHU's Speed rose! [SPE+]");
        assert_eq!(format_effect(&names, &down).unwrap(), "Wild BIDOOF's Defense fell! [DEF-]");
        assert!(format_effect(&names, &EffectEvent::Hits { actor: 0, count: 1 }).is_none());
    }

    #[test]
    fn tail_keeps_only_recent_lines() {
        let lines: Vec<String> = (0..10).map(|i| format!("l{i}")).collect();
        let t = tail(&lines);
        assert_eq!(t.lines().count(), LOG_VISIBLE_LINES);
        assert!(t.ends_with("l9"));
        assert!(!t.contains("l3"));
    }
}
