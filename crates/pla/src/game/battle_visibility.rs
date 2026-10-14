//! One visible model per combatant during a battle.
//!
//! When a battle starts the viewer spawns the two arena combatants (`BattleArenaMarker`,
//! `PokemonBattleSlot(0|1)`). The overworld entity of the wild creature being fought
//! (`WildCreature`, `in_battle == true`) used to stay visible at its world position, so the
//! same species showed twice. This module hides that overworld entity while the battle runs
//! and shows it again afterwards if it still exists (fled). Defeated / captured creatures are
//! despawned by `battle_input_system`, so nothing is restored for them.
//!
//! Only entities carrying [`HiddenForBattle`] are touched on exit, so a creature hidden for
//! another reason (e.g. inside a thrown ball) is never un-hidden by mistake.

use bevy::prelude::*;

use super::state::GameState;
use super::wild::WildCreature;

/// Marks an overworld entity that this module hid for the current battle.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct HiddenForBattle;

/// Pure rule: an overworld wild creature must be hidden while it is the battle target.
pub fn overworld_hidden_in_battle(in_battle: bool, defeated: bool) -> bool {
    in_battle && !defeated
}

/// Pure bookkeeping: given `(is_arena_model, hidden_by_us, wild_in_battle)` per entity, how many
/// models of the same combatant are visible. Arena models are always visible; overworld ones
/// are visible unless hidden.
pub fn visible_model_count(entities: &[(bool, bool)]) -> usize {
    entities.iter().filter(|(arena, hidden)| *arena || !*hidden).count()
}

fn hide_overworld_target(
    mut commands: Commands,
    mut q: Query<(Entity, &WildCreature, &mut Visibility), Without<HiddenForBattle>>,
) {
    for (e, wild, mut vis) in &mut q {
        if overworld_hidden_in_battle(wild.in_battle, wild.defeated) {
            *vis = Visibility::Hidden;
            commands.entity(e).try_insert(HiddenForBattle);
        }
    }
}

fn restore_overworld_target(mut commands: Commands, mut q: Query<(Entity, &mut Visibility), With<HiddenForBattle>>) {
    for (e, mut vis) in &mut q {
        *vis = Visibility::Inherited;
        commands.entity(e).try_remove::<HiddenForBattle>();
    }
}

/// Registers the hide-on-enter / restore-on-exit systems for the battle state.
pub struct BattleVisibilityPlugin;

impl Plugin for BattleVisibilityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Battle), hide_overworld_target)
            .add_systems(OnExit(GameState::Battle), restore_overworld_target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    fn wild(in_battle: bool) -> WildCreature {
        WildCreature {
            species_name: "starly".into(),
            national_dex: 396,
            level: 5,
            spawn_pos: Vec3::ZERO,
            aggro_range: 2.8,
            in_battle,
            defeated: false,
            spawner_id: "s".into(),
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.add_plugins(BattleVisibilityPlugin);
        app.update();
        app
    }

    fn go(app: &mut App, s: GameState) {
        app.world_mut().resource_mut::<NextState<GameState>>().set(s);
        app.update();
    }

    fn vis(app: &App, e: Entity) -> Visibility {
        *app.world().get::<Visibility>(e).unwrap()
    }

    #[test]
    fn pure_rules() {
        assert!(overworld_hidden_in_battle(true, false));
        assert!(!overworld_hidden_in_battle(false, false));
        assert!(!overworld_hidden_in_battle(true, true));
        // Bug: arena model + visible overworld model = 2 copies. Fix: 1.
        assert_eq!(visible_model_count(&[(true, false), (false, false)]), 2);
        assert_eq!(visible_model_count(&[(true, false), (false, true)]), 1);
    }

    #[test]
    fn only_the_fought_creature_is_hidden_and_it_comes_back_after_fleeing() {
        let mut app = app();
        let target = app.world_mut().spawn((wild(true), Visibility::default())).id();
        let other = app.world_mut().spawn((wild(false), Visibility::default())).id();
        go(&mut app, GameState::Battle);
        assert_eq!(vis(&app, target), Visibility::Hidden);
        assert_eq!(vis(&app, other), Visibility::Inherited, "other creatures stay visible");
        go(&mut app, GameState::World);
        assert_ne!(vis(&app, target), Visibility::Hidden, "fled creature is shown again");
        assert!(app.world().get::<HiddenForBattle>(target).is_none());
    }

    #[test]
    fn defeated_creature_despawned_in_battle_is_not_resurrected() {
        let mut app = app();
        let target = app.world_mut().spawn((wild(true), Visibility::default())).id();
        go(&mut app, GameState::Battle);
        app.world_mut().despawn(target); // what battle_input_system does
        go(&mut app, GameState::World); // must not panic or respawn
        assert!(app.world().get_entity(target).is_err());
    }

    #[test]
    fn creature_hidden_for_another_reason_is_not_unhidden() {
        let mut app = app();
        // Hidden inside a ball, not in a battle: never touched by enter/exit.
        let ball = app.world_mut().spawn((wild(false), Visibility::Hidden)).id();
        go(&mut app, GameState::Battle);
        go(&mut app, GameState::World);
        assert_eq!(vis(&app, ball), Visibility::Hidden);
    }

    #[test]
    fn second_battle_hides_again() {
        let mut app = app();
        let t = app.world_mut().spawn((wild(true), Visibility::default())).id();
        go(&mut app, GameState::Battle);
        go(&mut app, GameState::World);
        app.world_mut().get_mut::<WildCreature>(t).unwrap().in_battle = true;
        go(&mut app, GameState::Battle);
        assert_eq!(vis(&app, t), Visibility::Hidden);
    }
}
