//! Wild creature spawner and definitions for overworld exploration.

use bevy::prelude::*;

#[derive(Component, Debug, Clone)]
pub struct WildCreature {
    pub species_name: String,
    pub national_dex: u16,
    pub level: u8,
    pub spawn_pos: Vec3,
    pub aggro_range: f32,
    pub in_battle: bool,
    pub defeated: bool,
}

#[derive(Resource, Default)]
pub struct WildCreatureCatalog {
    pub spawns: Vec<WildCreature>,
}

/// Returns deterministic initial wild spawns for Area 00 (Jubilife Village).
/// Positions are validated inside walkable meadow regions.
pub fn get_area00_wild_spawns() -> Vec<WildCreature> {
    vec![
        WildCreature {
            species_name: "pikachu".into(),
            national_dex: 25,
            level: 5,
            spawn_pos: Vec3::new(448.0, 0.0, 646.0), // ~6m in front of starting point
            aggro_range: 2.8,
            in_battle: false,
            defeated: false,
        },
        WildCreature {
            species_name: "bidoof".into(),
            national_dex: 399,
            level: 3,
            spawn_pos: Vec3::new(460.0, 0.0, 648.0),
            aggro_range: 2.5,
            in_battle: false,
            defeated: false,
        },
        WildCreature {
            species_name: "starly".into(),
            national_dex: 396,
            level: 4,
            spawn_pos: Vec3::new(440.0, 0.0, 658.0),
            aggro_range: 2.5,
            in_battle: false,
            defeated: false,
        },
        WildCreature {
            species_name: "shinx".into(),
            national_dex: 403,
            level: 4,
            spawn_pos: Vec3::new(455.0, 0.0, 665.0),
            aggro_range: 3.0,
            in_battle: false,
            defeated: false,
        },
    ]
}

/// Returns deterministic wild spawns for a given area, positioned near the player spawn.
pub fn get_wild_spawns_for_area(area: u8, player_spawn: Vec2) -> Vec<WildCreature> {
    if area == 0 {
        return get_area00_wild_spawns();
    }

    let base_lvl = match area {
        1 => 14,
        2 => 24,
        3 => 32,
        4 => 42,
        _ => 50,
    };

    vec![
        WildCreature {
            species_name: "pikachu".into(),
            national_dex: 25,
            level: base_lvl + 2,
            spawn_pos: Vec3::new(player_spawn.x + 6.0, 0.0, player_spawn.y - 5.0),
            aggro_range: 2.8,
            in_battle: false,
            defeated: false,
        },
        WildCreature {
            species_name: "bidoof".into(),
            national_dex: 399,
            level: base_lvl,
            spawn_pos: Vec3::new(player_spawn.x - 7.0, 0.0, player_spawn.y + 6.0),
            aggro_range: 2.5,
            in_battle: false,
            defeated: false,
        },
        WildCreature {
            species_name: "starly".into(),
            national_dex: 396,
            level: base_lvl + 1,
            spawn_pos: Vec3::new(player_spawn.x + 9.0, 0.0, player_spawn.y + 8.0),
            aggro_range: 2.5,
            in_battle: false,
            defeated: false,
        },
        WildCreature {
            species_name: "shinx".into(),
            national_dex: 403,
            level: base_lvl + 2,
            spawn_pos: Vec3::new(player_spawn.x - 8.0, 0.0, player_spawn.y - 7.0),
            aggro_range: 3.0,
            in_battle: false,
            defeated: false,
        },
    ]
}
