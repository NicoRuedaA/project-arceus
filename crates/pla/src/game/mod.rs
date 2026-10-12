//! Playable Game Loop modules for Pokémon Legends: Arceus port.
//!
//! Provides game loop states, collision handling with navmesh sliding,
//! deterministic wild creature placement, and turn-based battle state transitions.

use bevy::prelude::*;

pub mod battle_controller;
pub mod lighting;
pub mod navigation;
pub mod state;
pub mod ui;
pub mod world_objects;
pub mod wild;

pub use battle_controller::{ActiveBattleSession, BattleMenuState};
pub use navigation::GameNavmeshResource;
pub use state::GameState;
pub use ui::{
    setup_game_ui, sync_ui_visibility, update_battle_ui_system, update_world_hud_system,
    ActiveZoneInfo, BattleLogText, BattleMenuOptionBox, BattleMoveBox, BattleMoveDetailText,
    BattleMoveTitleText, BattleUiRoot, PlayerHpBarFill, PlayerHpText, PlayerNameText,
    WildHpBarFill, WildHpText, WildNameText, WorldCoordinatesText, WorldHudRoot, WorldRadarText,
    WorldZoneText,
};
pub use wild::{get_area00_wild_spawns, get_wild_spawns_for_area, WildCreature};

/// Component marker for the controllable player entity in overworld and HUD queries.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PlayerAvatar;
