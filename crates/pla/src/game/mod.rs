//! Playable Game Loop modules for Pokémon Legends: Arceus port.
//!
//! Provides game loop states, collision handling with navmesh sliding,
//! deterministic wild creature placement, and turn-based battle state transitions.

pub mod battle_controller;
pub mod navigation;
pub mod state;
pub mod wild;

pub use battle_controller::{ActiveBattleSession, BattleMenuState};
pub use navigation::GameNavmeshResource;
pub use state::GameState;
pub use wild::{get_area00_wild_spawns, WildCreature};
