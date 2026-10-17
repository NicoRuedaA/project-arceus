//! Main-series battle gimmicks in the battle screen (port extension; engine rules in
//! `battle_engine::gimmicks`). Off by default. `--gimmick=<mega|z|dynamax|tera>` or
//! `PLA_GIMMICK=<...>` picks the one gimmick every battle uses; `off` turns it off.
//!
//! - The player arms it with `Y` on the move grid; it fires with the next move.
//! - Trainers use it on their first move when they can; wild Pokemon never do (main series).
//! - Members are equipped when they first try it: a Mega Stone, a Z-Crystal of the chosen
//!   move's type, the Tera type left at its default (first species type).
//!   Pure, no Bevy systems.

use crate::battle_engine::gimmicks::{
    Gimmick, GimmickEvent, GimmickRequest, MaxEffect, Refusal,
};
use crate::battle_engine::{Action, Battle};

/// `--gimmick=<kind>` or `--gimmick <kind>`; the last one wins; `off` / unknown = none.
pub fn gimmick_from_args(args: &[String]) -> Option<Gimmick> {
    let mut chosen = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if let Some(v) = a.strip_prefix("--gimmick=") {
            chosen = Gimmick::parse(v);
        } else if a == "--gimmick" {
            chosen = it.next().and_then(|v| Gimmick::parse(v));
        }
    }
    chosen
}

/// `PLA_GIMMICK` wins over the command line.
pub fn gimmick_from_env_and_args() -> Option<Gimmick> {
    match std::env::var("PLA_GIMMICK") {
        Ok(v) => Gimmick::parse(&v),
        Err(_) => gimmick_from_args(&std::env::args().collect::<Vec<_>>()),
    }
}

/// Give `actor` what the battle's gimmick needs before it tries it.
pub fn equip(battle: &mut Battle, action: Action) {
    match battle.battle_gimmick() {
        Some(Gimmick::Mega) => battle.give_mega_stone(action.actor),
        Some(Gimmick::ZMove) => {
            let move_type = battle.combatants[action.actor]
                .moves
                .get(action.move_slot)
                .map(|m| m.data.move_type.clone());
            if let Some(t) = move_type {
                battle.give_z_crystal(action.actor, &t);
            }
        }
        _ => {}
    }
}

/// A trainer's request for this action, when it can use the gimmick now.
pub fn ai_request(battle: &mut Battle, action: Action, trainer: bool) -> Option<GimmickRequest> {
    let gimmick = battle.battle_gimmick()?;
    if !trainer {
        return None;
    }
    equip(battle, action);
    battle
        .gimmick_refusal(action.actor, gimmick, Some(action.move_slot))
        .is_none()
        .then_some(GimmickRequest { actor: action.actor, gimmick })
}

/// Combatants whose move this turn became a Z-Move (from the turn's gimmick events).
pub fn z_move_users(events: &[GimmickEvent]) -> Vec<usize> {
    events
        .iter()
        .filter_map(|e| match e {
            GimmickEvent::ZMove { actor, .. } => Some(*actor),
            _ => None,
        })
        .collect()
}

/// Gimmick activations among the turn's gimmick events, as (combatant, gimmick).
pub fn activations(events: &[GimmickEvent]) -> Vec<(usize, Gimmick)> {
    events
        .iter()
        .filter_map(|e| match e {
            GimmickEvent::MegaEvolved { combatant, .. } => Some((*combatant, Gimmick::Mega)),
            GimmickEvent::Dynamaxed { combatant, .. } => Some((*combatant, Gimmick::Dynamax)),
            GimmickEvent::Terastallized { combatant, .. } => Some((*combatant, Gimmick::Terastal)),
            GimmickEvent::ZMove { actor, .. } => Some((*actor, Gimmick::ZMove)),
            _ => None,
        })
        .collect()
}

/// Seconds the battle screen keeps showing an activation's look, even when the Pokemon faints
/// or the battle ends that same turn.
pub const LOOK_HOLD_SECS: f64 = 2.0;

/// Gimmick a combatant shows in the arena: a lasting form (Mega, Dynamax, Tera) or a Z-Move used
/// this turn (`z_move_users`).
pub fn shown_gimmick(battle: &Battle, z_move_users: &[usize], combatant: usize) -> Option<Gimmick> {
    if battle.dynamax_turns(combatant).is_some() {
        Some(Gimmick::Dynamax)
    } else if battle.is_mega(combatant) {
        Some(Gimmick::Mega)
    } else if battle.terastallized(combatant).is_some() {
        Some(Gimmick::Terastal)
    } else if z_move_users.contains(&combatant) {
        Some(Gimmick::ZMove)
    } else {
        None
    }
}

/// How the battle screen shows an active gimmick on the real model: glow colour (linear RGB)
/// and model scale. Dynamax grows the model, less on the player's side, which stands close to
/// the camera.
pub fn look_style(gimmick: Gimmick, player_side: bool) -> ([f32; 3], f32) {
    match gimmick {
        Gimmick::Mega => ([0.6, 0.2, 0.9], 1.0),
        Gimmick::ZMove => ([1.0, 0.85, 0.1], 1.0),
        Gimmick::Dynamax => ([0.9, 0.1, 0.15], if player_side { 1.6 } else { 2.5 }),
        Gimmick::Terastal => ([0.1, 0.85, 0.95], 1.0),
    }
}

pub fn hint(gimmick: Gimmick) -> String {
    format!(
        "Gimmick: {} (one per battle). Y on the move grid arms it for the next move.",
        gimmick.label()
    )
}

pub fn refusal_text(reason: Refusal) -> String {
    match reason {
        Refusal::Disabled => "Gimmicks are off (--gimmick=<mega|z|dynamax|tera>).".into(),
        Refusal::OtherGimmickThisBattle(g) => {
            format!("Only one gimmick per battle: this battle uses {}.", g.label())
        }
        Refusal::AlreadyUsedBySide => "The gimmick was already used in this battle.".into(),
        Refusal::NotActive => "That Pokemon is not in battle.".into(),
        Refusal::NoMegaStone => "No Mega Stone.".into(),
        Refusal::NoMegaForm => "This Pokemon cannot Mega Evolve.".into(),
        Refusal::NoZCrystal => "No Z-Crystal.".into(),
        Refusal::CrystalTypeMismatch => "The Z-Crystal does not match the move's type.".into(),
        Refusal::StatusZMove => "Status Z-Moves are not available.".into(),
        Refusal::NoMove => "No usable move for a Z-Move.".into(),
    }
}

/// Log lines for the gimmick events (refusals included).
pub fn lines(name: &dyn Fn(usize) -> String, events: &[GimmickEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            GimmickEvent::Locked(_) => None,
            GimmickEvent::MegaEvolved { combatant, form } => {
                Some(format!("{} Mega Evolved into {form}!", name(*combatant)))
            }
            GimmickEvent::Dynamaxed { combatant, max_hp } => {
                Some(format!("{} Dynamaxed! (max HP {max_hp})", name(*combatant)))
            }
            GimmickEvent::DynamaxEnded { combatant } => {
                Some(format!("{} returned to normal size.", name(*combatant)))
            }
            GimmickEvent::Terastallized { combatant, tera_type } => {
                Some(format!("{} Terastallized into the {tera_type} type!", name(*combatant)))
            }
            GimmickEvent::ZMove { actor, power, .. } => {
                Some(format!("{} unleashes its full-force Z-Move! (power {power})", name(*actor)))
            }
            GimmickEvent::MaxMove { .. } | GimmickEvent::MaxGuard { .. } => None,
            GimmickEvent::ProtectPierced { target, .. } => {
                Some(format!("{} couldn't fully protect itself!", name(*target)))
            }
            GimmickEvent::MaxEffectApplied { effect: MaxEffect::Unsupported(what), .. } => {
                Some(format!("({what} is not modelled by the engine)"))
            }
            GimmickEvent::MaxEffectApplied { .. } => None,
            GimmickEvent::Refused { reason, .. } => Some(refusal_text(*reason)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_the_gimmick_flag() {
        assert_eq!(gimmick_from_args(&args(&["pla"])), None);
        assert_eq!(gimmick_from_args(&args(&["--gimmick=tera"])), Some(Gimmick::Terastal));
        assert_eq!(gimmick_from_args(&args(&["--gimmick", "z"])), Some(Gimmick::ZMove));
        assert_eq!(gimmick_from_args(&args(&["--gimmick=dmax", "--gimmick=off"])), None);
        assert_eq!(gimmick_from_args(&args(&["--gimmick=MEGA"])), Some(Gimmick::Mega));
    }

    #[test]
    fn each_gimmick_has_its_own_look() {
        let styles: Vec<_> = Gimmick::ALL.iter().map(|&g| look_style(g, false)).collect();
        for (i, a) in styles.iter().enumerate() {
            for b in &styles[i + 1..] {
                assert_ne!(a.0, b.0, "colours must differ");
            }
        }
        assert!(look_style(Gimmick::Dynamax, true).1 > look_style(Gimmick::Mega, true).1);
        assert_eq!(look_style(Gimmick::Dynamax, true).1, 1.6, "player side: not in the camera's face");
        assert_eq!(look_style(Gimmick::Dynamax, false).1, 2.5);
        assert_eq!(look_style(Gimmick::Terastal, false).1, 1.0, "only Dynamax changes the size");
        let events = [
            GimmickEvent::ZMove { actor: 2, name: "x".into(), power: 100 },
            GimmickEvent::MaxGuard { actor: 0 },
        ];
        assert_eq!(z_move_users(&events), vec![2]);
        let more = [
            GimmickEvent::Dynamaxed { combatant: 0, max_hp: 10 },
            GimmickEvent::DynamaxEnded { combatant: 0 },
            GimmickEvent::Terastallized { combatant: 1, tera_type: "Fire".into() },
        ];
        assert_eq!(activations(&more), vec![(0, Gimmick::Dynamax), (1, Gimmick::Terastal)]);
    }

    #[test]
    fn refusals_have_text() {
        assert!(refusal_text(Refusal::OtherGimmickThisBattle(Gimmick::Dynamax)).contains("Dynamax"));
    }
}
