//! Playable Game Loop modules for Pokémon Legends: Arceus port.
//!
//! Provides game loop states, collision handling with navmesh sliding,
//! deterministic wild creature placement, and turn-based battle state transitions.

use bevy::prelude::*;

pub mod battle_ai;
pub mod battle_controller;
pub mod lighting;
pub mod battle_demo;
pub mod battle_input;
pub mod battle_items;
pub mod battle_log;
pub mod catch_rate;
pub mod navigation;
pub mod player_anim;
pub mod pokemon_anim;
pub mod pause_menu;
pub mod save_game;
pub mod save_wire;
pub mod spawn_point;
pub mod ball_lifecycle;
pub mod settings;
pub mod state;
pub mod ui;
pub mod world_objects;
pub mod wild;
pub mod wild_wander;

pub use battle_controller::{ActiveBattleSession, BattleAssets, BattleInput, BattleMenuState};
pub use battle_input::battle_input_system;
pub use navigation::GameNavmeshResource;
pub use state::GameState;
pub use ui::{
    setup_game_ui, sync_ui_visibility, update_battle_ui_system, update_world_hud_system,
    ActiveZoneInfo, BattleLogText, BattleMenuOptionBox, BattleMoveBox, BattleMoveDetailText,
    BattleMoveTitleText, BattleUiRoot, PlayerHpBarFill, PlayerHpText, PlayerNameText,
    WildHpBarFill, WildHpText, WildNameText, WorldCoordinatesText, WorldHudRoot, WorldRadarText,
    WorldZoneText,
};
pub use wild::{
    get_area00_wild_spawns, get_wild_spawns_for_area, load_spawners_for_area, roll_spawner,
    ActiveAreaSpawners, SpawnerDefinition, WildCreature,
};

/// Component marker for the controllable player entity in overworld and HUD queries.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PlayerAvatar;
