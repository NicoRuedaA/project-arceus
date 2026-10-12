//! Keyboard mapping and end-of-battle handling for the in-game battle.

use bevy::prelude::*;

use super::battle_controller::{ActiveBattleSession, BattleInput};
use super::state::GameState;
use super::wild::{wild_fate, ActiveAreaSpawners, ConsumedSpawners, FleeCooldown, WildCreature, FLEE_COOLDOWN_S};

/// Map the keys pressed this frame to a battle input, if any.
/// Digits pick a move or list entry; S / I / C / R open the switch, item, capture and run
/// actions; arrows or A / D scroll; Enter or Space confirms; Esc or Backspace goes back.
pub fn map_keys(keys: &ButtonInput<KeyCode>) -> Option<BattleInput> {
    let digit = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4]
        .iter()
        .position(|k| keys.just_pressed(*k));
    if let Some(d) = digit {
        return Some(BattleInput::Slot(d));
    }
    let any = |ks: &[KeyCode]| ks.iter().any(|k| keys.just_pressed(*k));
    if any(&[KeyCode::ArrowLeft, KeyCode::ArrowUp, KeyCode::KeyA]) {
        Some(BattleInput::Prev)
    } else if any(&[KeyCode::ArrowRight, KeyCode::ArrowDown, KeyCode::KeyD]) {
        Some(BattleInput::Next)
    } else if any(&[KeyCode::Enter]) {
        Some(BattleInput::Confirm)
    } else if any(&[KeyCode::Escape, KeyCode::Backspace]) {
        Some(BattleInput::Back)
    } else if any(&[KeyCode::KeyF]) {
        Some(BattleInput::OpenFight)
    } else if any(&[KeyCode::KeyS]) {
        Some(BattleInput::OpenSwitch)
    } else if any(&[KeyCode::KeyI]) {
        Some(BattleInput::OpenItem)
    } else if any(&[KeyCode::KeyC, KeyCode::KeyB]) {
        Some(BattleInput::Capture)
    } else if any(&[KeyCode::KeyR]) {
        Some(BattleInput::Run)
    } else {
        None
    }
}

/// Battle turn system: turns key presses into `BattleInput`s and leaves the battle once it ended.
pub fn battle_input_system(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
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
        if sess.turn_timer >= 1.5
            || keys.just_pressed(KeyCode::Space)
            || keys.just_pressed(KeyCode::Enter)
        {
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

    if let Some(input) = map_keys(&keys) {
        if sess.handle_input(input) {
            sess.turn_timer = 0.0;
        }
        if sess.battle.ended {
            sess.turn_timer = 0.0;
        }
    }
}
