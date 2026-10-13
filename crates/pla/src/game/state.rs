//! Game state enum and transition markers.

use bevy::prelude::*;

/// Top-level state of the interactive game loop.
///
/// [`GameState::MainMenu`] is opt-in: the viewer starts there only with `--menu`, so
/// `--auto-demo` / `--script` and the normal direct start keep booting into
/// [`GameState::World`]. The state enum itself is unchanged for every other tool.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    /// Overworld exploration: real terrain heightmap, navmesh sliding locomotion, wild creature encounters.
    #[default]
    World,
    /// Main menu (`--menu`): Continue / New game / Options / Exit before the world runs.
    MainMenu,
    /// Active turn-based Pokémon battle against a wild creature.
    Battle,
    /// Post-battle summary or transition screen returning to World.
    BattleEnded,
}

/// Whether the pause menu is open. A plain resource (not a Bevy `State`) so the run conditions
/// below also work in apps that never installed the pause plugin (missing resource = running).
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Paused(pub bool);

/// Run condition: the game is paused (pause menu open).
pub fn is_paused(paused: Option<Res<Paused>>) -> bool {
    paused.is_some_and(|p| p.0)
}

/// Run condition for ALL gameplay systems (movement, ballistics, wild streaming/wandering,
/// battle input, timers): `.run_if(pla::game::state::not_paused)`.
pub fn not_paused(paused: Option<Res<Paused>>) -> bool {
    !paused.is_some_and(|p| p.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_conditions_follow_the_resource() {
        let mut app = App::new();
        assert!(app.world_mut().run_system_cached(not_paused).unwrap());
        assert!(!app.world_mut().run_system_cached(is_paused).unwrap());
        app.insert_resource(Paused(true));
        assert!(!app.world_mut().run_system_cached(not_paused).unwrap());
        assert!(app.world_mut().run_system_cached(is_paused).unwrap());
    }
}
