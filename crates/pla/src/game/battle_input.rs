//! Input mapping and end-of-battle handling for the in-game battle.
//!
//! The battle reads the shared [`ActionState`] filled by
//! [`crate::game::input_map::InputMapPlugin`] (keyboard and gamepad), so no system here
//! reads `KeyCode`s directly.

use bevy::prelude::*;

use super::battle_controller::{ActiveBattleSession, BattleInput};
use super::input_map::{Action, ActionState};
use super::state::GameState;
use super::wild::{wild_fate, ActiveAreaSpawners, ConsumedSpawners, FleeCooldown, WildCreature, FLEE_COOLDOWN_S};

/// Map the logical actions pressed this frame to a battle input, if any.
/// Digits pick a move or list entry; F / S / I / C / R open the fight, switch, item,
/// capture and run actions (gamepad: Y / X / L / ZR / R); D-pad, arrows or A / D scroll;
/// Enter / Space / A confirm; Escape / Backspace / B go back.
pub fn map_actions(input: &ActionState) -> Option<BattleInput> {
    for slot in 0..4u8 {
        if input.just_pressed(Action::Slot(slot)) {
            return Some(BattleInput::Slot(slot as usize));
        }
    }
    if input.just_pressed(Action::Prev) {
        Some(BattleInput::Prev)
    } else if input.just_pressed(Action::Next) {
        Some(BattleInput::Next)
    } else if input.just_pressed(Action::Confirm) {
        Some(BattleInput::Confirm)
    } else if input.just_pressed(Action::Cancel) {
        Some(BattleInput::Back)
    } else if input.just_pressed(Action::OpenFight) {
        Some(BattleInput::OpenFight)
    } else if input.just_pressed(Action::OpenSwitch) {
        Some(BattleInput::OpenSwitch)
    } else if input.just_pressed(Action::OpenItem) {
        Some(BattleInput::OpenItem)
    } else if input.just_pressed(Action::OpenCapture) {
        Some(BattleInput::Capture)
    } else if input.just_pressed(Action::OpenRun) {
        Some(BattleInput::Run)
    } else {
        None
    }
}

/// Battle turn system: turns logical actions into `BattleInput`s and leaves the battle once it ended.
pub fn battle_input_system(
    mut commands: Commands,
    time: Res<Time>,
    input: Res<ActionState>,
    mut session: Option<ResMut<ActiveBattleSession>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut wild_query: Query<&mut WildCreature>,
    mut consumed: Option<ResMut<ConsumedSpawners>>,
    mut spawners: Option<ResMut<ActiveAreaSpawners>>,
) {
    let Some(ref mut sess) = session else {
        return;
    };
    sess.turn_timer += time.delta_secs();

    if sess.battle.ended {
        if sess.turn_timer >= 1.5 || input.just_pressed(Action::Confirm) {
            println!("Battle completed. Returning to GameState::World.");
            let wild_state = &sess.battle.combatants[super::battle_controller::WILD];
            let fate = wild_fate(wild_state.hp as u32, wild_state.caught);
            if let Ok(mut wild) = wild_query.get_mut(sess.wild_entity) {
                wild.in_battle = false;
                if fate.removes_creature() {
                    // Defeated or caught: leave the world and keep the spawner empty
                    // for a while so streaming does not refill it at once.
                    wild.defeated = true;
                    if let Some(c) = consumed.as_deref_mut() {
                        c.consume(&wild.spawner_id, time.elapsed_secs_f64());
                    }
                    if let Some(s) = spawners.as_deref_mut() {
                        s.spawned_entities.remove(&wild.spawner_id);
                    }
                    println!("Wild #{} {:?}: removed from the world (spawner {})", wild.national_dex, fate, wild.spawner_id);
                    commands.entity(sess.wild_entity).despawn();
                } else {
                    // Fled: the creature stays alive; no new encounter for a few seconds.
                    commands.entity(sess.wild_entity).insert(FleeCooldown(FLEE_COOLDOWN_S));
                }
            }
            commands.remove_resource::<ActiveBattleSession>();
            next_state.set(GameState::World);
        }
        return;
    }

    if let Some(input) = map_actions(&input) {
        if sess.handle_input(input) {
            sess.turn_timer = 0.0;
        }
        if sess.battle.ended {
            sess.turn_timer = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::input_map::{actions, RawInput};

    /// Build an `ActionState` with exactly these just-pressed actions (no devices involved).
    fn state_with(pressed: &[Action]) -> ActionState {
        actions(&RawInput {
            key_pressed: pressed.to_vec(),
            ..default()
        })
    }

    #[test]
    fn slots_win_over_everything_else() {
        assert_eq!(
            map_actions(&state_with(&[
                Action::Slot(2),
                Action::Prev,
                Action::Confirm
            ])),
            Some(BattleInput::Slot(2))
        );
        assert_eq!(
            map_actions(&state_with(&[Action::Slot(0)])),
            Some(BattleInput::Slot(0))
        );
    }

    #[test]
    fn navigation_beats_confirm_and_back() {
        assert_eq!(
            map_actions(&state_with(&[Action::Prev])),
            Some(BattleInput::Prev)
        );
        assert_eq!(
            map_actions(&state_with(&[Action::Next])),
            Some(BattleInput::Next)
        );
        assert_eq!(
            map_actions(&state_with(&[Action::Confirm, Action::Cancel])),
            Some(BattleInput::Confirm)
        );
        assert_eq!(
            map_actions(&state_with(&[Action::Cancel])),
            Some(BattleInput::Back)
        );
    }

    #[test]
    fn submenus_map_to_their_battle_inputs() {
        assert_eq!(
            map_actions(&state_with(&[Action::OpenFight])),
            Some(BattleInput::OpenFight)
        );
        assert_eq!(
            map_actions(&state_with(&[Action::OpenSwitch])),
            Some(BattleInput::OpenSwitch)
        );
        assert_eq!(
            map_actions(&state_with(&[Action::OpenItem])),
            Some(BattleInput::OpenItem)
        );
        assert_eq!(
            map_actions(&state_with(&[Action::OpenCapture])),
            Some(BattleInput::Capture)
        );
        assert_eq!(
            map_actions(&state_with(&[Action::OpenRun])),
            Some(BattleInput::Run)
        );
    }

    #[test]
    fn held_actions_are_not_battle_input() {
        // Sprint is held, never just_pressed here: no battle input may come out of it.
        assert_eq!(
            map_actions(&state_with(&[Action::Sprint, Action::Dodge, Action::Aim])),
            None
        );
        assert_eq!(map_actions(&ActionState::default()), None);
    }

    #[test]
    fn exhausted_edges_stop_producing_input() {
        let mut state = state_with(&[Action::Confirm]);
        assert_eq!(map_actions(&state), Some(BattleInput::Confirm));
        state.consume(Action::Confirm);
        assert_eq!(map_actions(&state), None);
    }
}
