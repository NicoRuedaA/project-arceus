//! Battle log text built from engine events (pure, no Bevy).

use crate::battle_engine::effects::{EffectEvent, Stat};
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
/// The engine emits no `UsedMove` event for moves without power, so `status_moves` lists the
/// `(actor, move name)` pairs of such moves chosen this turn; each is announced at the first
/// point where that actor demonstrably acted (after `ActedFirst`, before an event caused by it,
/// or after all events), unless the engine reported a miss or an inability to act for it.
pub fn format_turn(
    name: &dyn Fn(usize) -> String,
    events: &[Event],
    effects: &[EffectEvent],
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
    lines.extend(ended);
    lines
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
        let lines = format_turn(&names, &events, &fx, &[]);
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
        let lines = format_turn(&names, &events, &[], &[(0, "thunder-wave".into())]);
        assert_eq!(lines[1], "PIKACHU used thunder-wave!");
        assert_eq!(lines[2], "Wild BIDOOF was paralyzed! [PAR]");
        // a missed status move is reported by the engine, not announced twice
        let missed = vec![Event::TurnStarted(1), Event::Missed { actor: 0, target: 1, name: "thunder-wave".into() }];
        let lines = format_turn(&names, &missed, &[], &[(0, "thunder-wave".into())]);
        assert_eq!(lines.len(), 2);
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
