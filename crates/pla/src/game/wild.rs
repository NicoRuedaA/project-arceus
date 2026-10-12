//! Wild creature spawner and definitions for overworld exploration.
//!
//! Loads real field spawn tables from `data/field_spawn_table.tsv`
//! and manages deterministic, distance-based streaming around the player.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use bevy::prelude::*;

/// Component attached to spawned overworld wild creatures.
#[derive(Component, Debug, Clone)]
pub struct WildCreature {
    pub species_name: String,
    pub national_dex: u16,
    pub level: u8,
    pub spawn_pos: Vec3,
    pub aggro_range: f32,
    pub in_battle: bool,
    pub defeated: bool,
    /// Spawner ID from `field_spawn_table.tsv` that created this entity.
    pub spawner_id: String,
}

/// A possible species entry within an encounter group.
#[derive(Debug, Clone)]
pub struct SpawnSpeciesEntry {
    pub species_id: u16,
    pub species_name: String,
    pub min_level: u8,
    pub max_level: u8,
    pub weight: u16,
}

/// Raw spawner definition parsed from `data/field_spawn_table.tsv`.
#[derive(Debug, Clone)]
pub struct SpawnerDefinition {
    pub spawner_id: String,
    pub area: String,
    pub pos: Vec3,
    pub radius: f32,
    pub entries: Vec<SpawnSpeciesEntry>,
}

/// Resource holding all parsed spawner definitions for the active area,
/// as well as the active spawned entities mapped by spawner ID.
#[derive(Resource)]
pub struct ActiveAreaSpawners {
    pub spawners: Vec<SpawnerDefinition>,
    pub spawned_entities: HashMap<String, Entity>,
    pub streaming_radius: f32,
    pub seed: u64,
}

impl Default for ActiveAreaSpawners {
    fn default() -> Self {
        Self {
            spawners: Vec::new(),
            spawned_entities: HashMap::new(),
            streaming_radius: 120.0,
            seed: 0x5e1f_5eed_1234_5678,
        }
    }
}

/// Deterministic 64-bit LCG / SplitMix64 pseudo-random number generator.
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// Computes a deterministic 64-bit hash from a string identifier.
pub fn hash_str(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Loads spawner definitions for a given area index from `data/field_spawn_table.tsv`.
pub fn load_spawners_for_area(
    table_path: impl AsRef<Path>,
    area_idx: u8,
) -> Result<Vec<SpawnerDefinition>, String> {
    let path = table_path.as_ref();
    let file = File::open(path)
        .map_err(|e| format!("failed to open spawn table {}: {e}", path.display()))?;
    let reader = BufReader::new(file);

    let target_area = format!("ha_area{area_idx:02}");
    let mut spawner_map: HashMap<String, SpawnerDefinition> = HashMap::new();

    for line_res in reader.lines() {
        let line = line_res.map_err(|e| format!("read error: {e}"))?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = trimmed.split('\t').collect();
        if parts.len() < 16 {
            continue;
        }

        let area_str = parts[0];
        // Match exact area ha_areaNN or subareas ha_areaNN_sMM
        if !area_str.starts_with(&target_area) {
            continue;
        }

        let spawner_id = parts[1].to_string();
        let x: f32 = match parts[2].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let y: f32 = match parts[3].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let z: f32 = match parts[4].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let radius: f32 = parts[5].parse().unwrap_or(8.0);

        let species_id: u16 = match parts[9].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let species_name = parts[10].to_lowercase();
        let min_lvl: u8 = parts[12].parse().unwrap_or(5);
        let max_lvl: u8 = parts[13].parse().unwrap_or(min_lvl);
        let weight: u16 = parts[15].parse().unwrap_or(100);

        let entry = SpawnSpeciesEntry {
            species_id,
            species_name,
            min_level: min_lvl,
            max_level: max_lvl,
            weight: weight.max(1),
        };

        let spawner = spawner_map.entry(spawner_id.clone()).or_insert_with(|| {
            SpawnerDefinition {
                spawner_id,
                area: area_str.to_string(),
                pos: Vec3::new(x, y, z),
                radius,
                entries: Vec::new(),
            }
        });

        spawner.entries.push(entry);
    }

    let mut result: Vec<SpawnerDefinition> = spawner_map.into_values().collect();
    // Sort deterministically by spawner_id
    result.sort_by(|a, b| a.spawner_id.cmp(&b.spawner_id));
    Ok(result)
}

/// Chooses species and level deterministically for a spawner using seed + spawner_id.
pub fn roll_spawner(spawner: &SpawnerDefinition, base_seed: u64) -> Option<(u16, String, u8)> {
    if spawner.entries.is_empty() {
        return None;
    }

    let spawner_seed = base_seed ^ hash_str(&spawner.spawner_id);
    let mut rng_state = spawner_seed;

    let total_weight: u32 = spawner.entries.iter().map(|e| e.weight as u32).sum();
    let roll_weight = if total_weight > 0 {
        (splitmix64(&mut rng_state) % (total_weight as u64)) as u32
    } else {
        0
    };

    let mut accumulated = 0;
    let mut chosen_entry = &spawner.entries[0];
    for entry in &spawner.entries {
        accumulated += entry.weight as u32;
        if roll_weight < accumulated {
            chosen_entry = entry;
            break;
        }
    }

    let min_lvl = chosen_entry.min_level;
    let max_lvl = chosen_entry.max_level;
    let lvl = if max_lvl > min_lvl {
        let span = (max_lvl - min_lvl + 1) as u64;
        min_lvl + (splitmix64(&mut rng_state) % span) as u8
    } else {
        min_lvl
    };

    Some((chosen_entry.species_id, chosen_entry.species_name.clone(), lvl))
}

/// Backward compatibility helper for legacy calls.
pub fn get_area00_wild_spawns() -> Vec<WildCreature> {
    get_wild_spawns_for_area(0, Vec2::new(448.0, 652.0))
}

/// Helper that evaluates spawners for an area near player_spawn, falling back to real table.
pub fn get_wild_spawns_for_area(area: u8, player_spawn: Vec2) -> Vec<WildCreature> {
    let table_path = "data/field_spawn_table.tsv";
    if let Ok(spawners) = load_spawners_for_area(table_path, area) {
        if !spawners.is_empty() {
            let mut result = Vec::new();
            let base_seed = 0x5e1f_5eed_1234_5678;
            for spawner in &spawners {
                let dist_sq = (spawner.pos.x - player_spawn.x).powi(2)
                    + (spawner.pos.z - player_spawn.y).powi(2);
                if dist_sq <= 120.0 * 120.0 {
                    if let Some((dex, name, lvl)) = roll_spawner(spawner, base_seed) {
                        result.push(WildCreature {
                            species_name: name,
                            national_dex: dex,
                            level: lvl,
                            spawn_pos: spawner.pos,
                            aggro_range: 2.8,
                            in_battle: false,
                            defeated: false,
                            spawner_id: spawner.spawner_id.clone(),
                        });
                    }
                }
            }
            if !result.is_empty() {
                return result;
            }
        }
    }

    // Default fallback if table missing
    vec![
        WildCreature {
            species_name: "pikachu".into(),
            national_dex: 25,
            level: 5,
            spawn_pos: Vec3::new(player_spawn.x + 6.0, 0.0, player_spawn.y - 5.0),
            aggro_range: 2.8,
            in_battle: false,
            defeated: false,
            spawner_id: "fallback_pika".into(),
        },
    ]
}
