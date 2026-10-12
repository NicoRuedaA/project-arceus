//! Battle animations: turns the battle engine's events into Pokémon animation requests.
//!
//! The mapping is a pure function ([`requests_for_events`]) so it is unit-tested without
//! Bevy. [`drive_battle_anims`] reads the new events of the [`ActiveBattleSession`] each
//! frame and sends [`PokemonAnimMessage`]s to the arena creatures ([`PokemonBattleSlot`]).
//! Verified by tests only; never checked visually.

use bevy::prelude::*;

use super::battle_controller::{ActiveBattleSession, WILD};
use super::pokemon_anim::{AttackKind, PokemonAnimMessage, PokemonAnimRequest, PokemonBattleSlot};
use crate::battle_engine::Event;

/// Seconds between two moves of the same turn (the second mover waits for the first).
pub const MOVE_STEP_S: f32 = 1.4;
/// Seconds after a move starts until the target reacts.
pub const HIT_DELAY_S: f32 = 0.45;
/// Seconds after the killing move starts until the target falls.
pub const FAINT_DELAY_S: f32 = 0.65;

/// One request for one combatant, `delay` seconds after the turn's events arrived.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimedRequest {
    pub combatant: usize,
    pub request: PokemonAnimRequest,
    pub delay: f32,
}

/// Attack animation of a move category ("Physical" -> physical; "Special", status and
/// unknown categories -> special/ranged, except an unknown move which stays physical).
pub fn attack_kind(category: Option<&str>) -> AttackKind {
    match category {
        Some("Physical") | None => AttackKind::Physical,
        Some(_) => AttackKind::Special,
    }
}

/// Maps engine events to animation requests. `category(actor, move_name)` returns the
/// move category when known.
///
/// - used move: attacker attacks (physical vs special by category); the target is Hit
///   when the move dealt damage;
/// - missed move: attacker attacks only;
/// - recoil: Hit;
/// - knocked out: Faint just after the move that caused it;
/// - switched in: Appear for the new combatant.
pub fn requests_for_events(
    events: &[Event],
    category: &dyn Fn(usize, &str) -> Option<String>,
) -> Vec<TimedRequest> {
    let mut out = Vec::new();
    let mut cursor = 0.0_f32;
    let mut last_action = 0.0_f32;
    for ev in events {
        match ev {
            Event::UsedMove { actor, target, name, damage, .. } => {
                let kind = attack_kind(category(*actor, name).as_deref());
                out.push(TimedRequest { combatant: *actor, request: PokemonAnimRequest::Attack(kind), delay: cursor });
                if *damage > 0 {
                    out.push(TimedRequest { combatant: *target, request: PokemonAnimRequest::Hit, delay: cursor + HIT_DELAY_S });
                }
                last_action = cursor;
                cursor += MOVE_STEP_S;
            }
            Event::Missed { actor, name, .. } => {
                let kind = attack_kind(category(*actor, name).as_deref());
                out.push(TimedRequest { combatant: *actor, request: PokemonAnimRequest::Attack(kind), delay: cursor });
                last_action = cursor;
                cursor += MOVE_STEP_S;
            }
            Event::Recoil { target, .. } => {
                out.push(TimedRequest { combatant: *target, request: PokemonAnimRequest::Hit, delay: cursor });
            }
            Event::KnockedOut(i) => {
                out.push(TimedRequest { combatant: *i, request: PokemonAnimRequest::Faint, delay: last_action + FAINT_DELAY_S });
            }
            Event::Switched { to, .. } => {
                out.push(TimedRequest { combatant: *to, request: PokemonAnimRequest::Appear, delay: cursor });
            }
            _ => {}
        }
    }
    out
}

/// Arena slot of a combatant: the wild Pokémon is slot 1, every player-side one slot 0.
pub fn slot_of(combatant: usize) -> usize {
    if combatant == WILD {
        1
    } else {
        0
    }
}

/// Sends the animation requests of the events that arrived since the last frame.
pub fn drive_battle_anims(
    session: Option<Res<ActiveBattleSession>>,
    mut cursor: Local<usize>,
    slots: Query<(Entity, &PokemonBattleSlot)>,
    mut out: MessageWriter<PokemonAnimMessage>,
) {
    let Some(sess) = session else {
        *cursor = 0;
        return;
    };
    let len = sess.event_log.len();
    if len < *cursor {
        *cursor = 0;
    }
    if len == *cursor {
        return;
    }
    let category = |actor: usize, name: &str| -> Option<String> {
        sess.battle
            .combatants
            .get(actor)?
            .moves
            .iter()
            .find(|m| m.data.name == name)
            .map(|m| m.data.category.clone())
    };
    let requests = requests_for_events(&sess.event_log[*cursor..], &category);
    *cursor = len;
    for r in requests {
        let slot = slot_of(r.combatant);
        if let Some((entity, _)) = slots.iter().find(|(_, s)| s.0 == slot) {
            out.write(PokemonAnimMessage { entity, request: r.request, delay: r.delay });
        }
    }
}

/// Registers [`drive_battle_anims`]. Add after `PokemonAnimPlugin`.
pub struct BattleAnimPlugin;

impl Plugin for BattleAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PokemonAnimMessage>().add_systems(Update, drive_battle_anims);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PokemonAnimRequest::*;

    const PLAYER: usize = 0;

    fn cat(_: usize, name: &str) -> Option<String> {
        match name {
            "Quick Attack" | "Tackle" => Some("Physical".into()),
            "Thunderbolt" => Some("Special".into()),
            "Growl" => Some("Status".into()),
            _ => None,
        }
    }

    fn used(actor: usize, target: usize, name: &str, damage: u32) -> Event {
        Event::UsedMove { actor, target, name: name.into(), damage, critical: false, effectiveness: String::new() }
    }

    #[test]
    fn category_selects_physical_or_special() {
        assert_eq!(attack_kind(Some("Physical")), AttackKind::Physical);
        assert_eq!(attack_kind(Some("Special")), AttackKind::Special);
        assert_eq!(attack_kind(Some("Status")), AttackKind::Special);
        assert_eq!(attack_kind(None), AttackKind::Physical);
    }

    #[test]
    fn physical_move_makes_attacker_attack_and_target_get_hit() {
        let r = requests_for_events(&[used(PLAYER, WILD, "Tackle", 12)], &cat);
        assert_eq!(
            r,
            vec![
                TimedRequest { combatant: PLAYER, request: Attack(AttackKind::Physical), delay: 0.0 },
                TimedRequest { combatant: WILD, request: Hit, delay: HIT_DELAY_S },
            ]
        );
    }

    #[test]
    fn special_move_uses_the_ranged_attack() {
        let r = requests_for_events(&[used(PLAYER, WILD, "Thunderbolt", 20)], &cat);
        assert_eq!(r[0].request, Attack(AttackKind::Special));
    }

    #[test]
    fn zero_damage_and_miss_do_not_hit() {
        let r = requests_for_events(&[used(PLAYER, WILD, "Growl", 0)], &cat);
        assert_eq!(r.len(), 1);
        let r = requests_for_events(
            &[Event::Missed { actor: WILD, target: PLAYER, name: "Tackle".into() }],
            &cat,
        );
        assert_eq!(r, vec![TimedRequest { combatant: WILD, request: Attack(AttackKind::Physical), delay: 0.0 }]);
    }

    #[test]
    fn second_move_waits_for_the_first() {
        let r = requests_for_events(&[used(PLAYER, WILD, "Tackle", 5), used(WILD, PLAYER, "Tackle", 3)], &cat);
        let second = r.iter().find(|t| t.combatant == WILD && matches!(t.request, Attack(_))).unwrap();
        assert_eq!(second.delay, MOVE_STEP_S);
        let hit = r.iter().find(|t| t.combatant == PLAYER && t.request == Hit).unwrap();
        assert_eq!(hit.delay, MOVE_STEP_S + HIT_DELAY_S);
    }

    #[test]
    fn knockout_faints_after_the_killing_hit() {
        let r = requests_for_events(&[used(PLAYER, WILD, "Tackle", 99), Event::KnockedOut(WILD)], &cat);
        let hit = r.iter().find(|t| t.request == Hit).unwrap();
        let faint = r.iter().find(|t| t.request == Faint).unwrap();
        assert_eq!(faint.combatant, WILD);
        assert!(faint.delay > hit.delay, "the creature reacts, then falls");
    }

    #[test]
    fn unrelated_events_make_no_request() {
        let r = requests_for_events(
            &[Event::TurnStarted(1), Event::ActedFirst(0), Event::BattleEnded { winner_side: Some(0) }],
            &cat,
        );
        assert!(r.is_empty());
    }

    #[test]
    fn switch_in_appears() {
        let r = requests_for_events(&[Event::Switched { side: 0, from: 0, to: 2 }], &cat);
        assert_eq!(r, vec![TimedRequest { combatant: 2, request: Appear, delay: 0.0 }]);
    }

    #[test]
    fn combatants_map_to_arena_slots() {
        assert_eq!(slot_of(WILD), 1);
        assert_eq!(slot_of(0), 0);
        assert_eq!(slot_of(2), 0, "other party members use the player slot");
    }
}
