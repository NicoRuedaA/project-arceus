//! Playable Game Loop modules for Pokémon Legends: Arceus port.
//!
//! Provides game loop states, collision handling with navmesh sliding,
//! deterministic wild creature placement, and turn-based battle state transitions.

use bevy::prelude::*;

pub mod art_style;
pub mod audio;
pub mod automation;
pub mod bag_screen;
pub mod ball_lifecycle;
pub mod battle_ai;
pub mod battle_anim;
pub mod battle_controller;
pub mod battle_demo;
#[cfg(test)]
mod battle_flow_headless;
pub mod battle_input;
pub mod battle_items;
pub mod battle_log;
pub mod battle_progress;
pub mod battle_style_ai;
pub mod battle_turns;
pub mod evolution_fx;
pub mod ledge;
pub mod lighting;
pub mod main_menu;
pub mod minimap;
pub mod npc;
pub mod render_look;
pub mod input_map;
pub mod interact;
pub mod battle_visibility;
pub mod blink;
pub mod box_screen;
pub mod catch_rate;
pub mod cli_flags;
pub mod clock;
pub mod crafting;
pub mod crafting_ui;
pub mod dex_screen;
pub mod distortion;
pub mod effects;
pub mod follower;
pub mod graphics_screen;
pub mod navigation;
pub mod outbreak;
pub mod party_ops;
pub mod party_progress;
pub mod party_screen;
pub mod pause_menu;
pub mod pickups;
pub mod play_time;
pub mod player_anim;
pub mod player_motion;
pub mod player_skills;
pub mod pokemon_anim;
pub mod pokemon_resource;
pub mod save_game;
pub mod save_wire;
pub mod schedule_check;
pub mod spawn_point;
pub mod capture_world;
pub mod settings;
pub mod shop;
pub mod state;
pub mod throwing;
pub mod trainer_battle;
pub mod trainer_data;
pub mod trainer_encounter;
pub mod ui;
pub mod vegetation;
pub mod water;
pub mod weather;
pub mod wild;
pub mod wild_behavior;
pub mod wild_wander;
pub mod door_pairs;
pub mod world_collision;
pub mod world_events;
pub mod world_objects;
pub mod zone;

pub use battle_controller::{ActiveBattleSession, BattleAssets, BattleInput, BattleMenuState};
pub use battle_input::battle_input_system;
pub use navigation::GameNavmeshResource;
pub use party_screen::{PartyScreenData, PartyScreenPlugin, PartyScreenRoot, PartyScreenState};
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
    roll_spawner_variant, ActiveAreaSpawners, SpawnerDefinition, WildCreature,
};

/// Component marker for the controllable player entity in overworld and HUD queries.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PlayerAvatar;
