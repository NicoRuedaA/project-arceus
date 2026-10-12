//! Game state enum and transition markers.

use bevy::prelude::*;

/// Top-level state of the interactive game loop.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    /// Overworld exploration: real terrain heightmap, navmesh sliding locomotion, wild creature encounters.
    #[default]
    World,
    /// Active turn-based Pokémon battle against a wild creature.
    Battle,
    /// Post-battle summary or transition screen returning to World.
    BattleEnded,
}
