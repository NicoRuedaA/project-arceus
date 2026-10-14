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

/// How a battle with a wild creature ended, from the wild creature's side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WildFate {
    /// The wild creature fainted: it leaves the world.
    Defeated,
    /// The wild creature was caught: it leaves the world.
    Captured,
    /// The player ran away (or lost): the wild creature stays where it was.
    Fled,
}

/// Classifies the end of a battle from the wild combatant's final state.
pub fn wild_fate(wild_hp: u32, wild_caught: bool) -> WildFate {
    if wild_caught {
        WildFate::Captured
    } else if wild_hp == 0 {
        WildFate::Defeated
    } else {
        WildFate::Fled
    }
}

impl WildFate {
    /// True when the creature must be removed from the world.
    pub fn removes_creature(self) -> bool {
        !matches!(self, WildFate::Fled)
    }
}

/// How long (in game seconds) a defeated / captured spawner stays empty before it may
/// spawn a new creature again. Leaving and re-entering the zone also clears the set
/// (see [`ConsumedSpawners::clear`]).
pub const RESPAWN_AFTER_S: f64 = 600.0;

/// Seconds a creature that the player fled from will not start another encounter.
pub const FLEE_COOLDOWN_S: f32 = 6.0;

/// Spawner ids whose creature was defeated or captured, with the time they may respawn.
#[derive(Resource, Debug, Default, Clone)]
pub struct ConsumedSpawners {
    respawn_at: HashMap<String, f64>,
}

impl ConsumedSpawners {
    /// Marks `spawner_id` as consumed at game time `now_s`.
    pub fn consume(&mut self, spawner_id: &str, now_s: f64) {
        self.respawn_at
            .insert(spawner_id.to_string(), now_s + RESPAWN_AFTER_S);
    }

    /// True while the spawner must stay empty.
    pub fn is_consumed(&self, spawner_id: &str, now_s: f64) -> bool {
        self.respawn_at.get(spawner_id).is_some_and(|t| now_s < *t)
    }

    /// Forgets spawners whose waiting time is over.
    pub fn purge_expired(&mut self, now_s: f64) {
        self.respawn_at.retain(|_, t| now_s < *t);
    }

    /// Forgets everything (zone reload).
    pub fn clear(&mut self) {
        self.respawn_at.clear();
    }

    pub fn len(&self) -> usize {
        self.respawn_at.len()
    }

    pub fn is_empty(&self) -> bool {
        self.respawn_at.is_empty()
    }
}

/// Encounters are suppressed while this component is present (fled battle).
#[derive(Component, Debug, Clone, Copy)]
pub struct FleeCooldown(pub f32);

/// One cooldown step: remaining seconds after `dt`, or `None` when it has expired.
pub fn flee_cooldown_step(remaining: f32, dt: f32) -> Option<f32> {
    let left = remaining - dt;
    (left > 0.0).then_some(left)
}

fn tick_flee_cooldown(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut FleeCooldown)>,
) {
    for (e, mut c) in &mut q {
        match flee_cooldown_step(c.0, time.delta_secs()) {
            Some(left) => c.0 = left,
            None => {
                commands.entity(e).try_remove::<FleeCooldown>();
            }
        }
    }
}

fn purge_consumed(time: Res<Time>, mut consumed: ResMut<ConsumedSpawners>) {
    if !consumed.is_empty() {
        consumed.purge_expired(time.elapsed_secs_f64());
    }
}

/// Registers the bookkeeping of defeated / captured / fled wild creatures.
pub struct WildLifecyclePlugin;

impl Plugin for WildLifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsumedSpawners>()
            .add_systems(Update, (tick_flee_cooldown, purge_consumed));
    }
}

/// A possible species entry within an encounter group.
#[derive(Debug, Clone)]
pub struct SpawnSpeciesEntry {
    pub species_id: u16,
    pub species_name: String,
    /// Encounter form from `field_spawn_conditions.tsv`; zero is the explicit
    /// fallback when the companion evidence row is absent.
    pub form: u16,
    /// Alpha flags are decoded slot facts, not probabilities.
    pub alpha: bool,
    pub unique_alpha: bool,
    pub min_level: u8,
    pub max_level: u8,
    pub weight: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpawnVariant {
    pub form: u16,
    pub alpha: bool,
    pub size_scale: f32,
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
    let condition_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("field_spawn_conditions.tsv");
    let mut conditions = HashMap::<(String, String, u16), (u16, bool, bool)>::new();
    if let Ok(file) = File::open(&condition_path) {
        let reader = BufReader::new(file);
        for line_res in reader.lines().skip(9) {
            let Ok(line) = line_res else { continue };
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 9 || !fields[0].starts_with(&target_area) {
                continue;
            }
            let Ok(species_id) = fields[4].parse::<u16>() else {
                continue;
            };
            let form = fields[6].parse::<u16>().unwrap_or(0);
            let alpha = fields[7] == "1";
            let unique_alpha = fields[8] == "1";
            conditions.insert(
                (fields[1].to_owned(), fields[2].to_owned(), species_id),
                (form, alpha, unique_alpha),
            );
        }
    }
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
        let encount_key = parts[8];
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
        let (form, alpha, unique_alpha) = conditions
            .get(&(spawner_id.clone(), encount_key.to_owned(), species_id))
            .copied()
            .unwrap_or((0, false, false));

        let entry = SpawnSpeciesEntry {
            species_id,
            species_name,
            form,
            alpha,
            unique_alpha,
            min_level: min_lvl,
            max_level: max_lvl,
            weight: weight.max(1),
        };

        let spawner = spawner_map
            .entry(spawner_id.clone())
            .or_insert_with(|| SpawnerDefinition {
                spawner_id,
                area: area_str.to_string(),
                pos: Vec3::new(x, y, z),
                radius,
                entries: Vec::new(),
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
    roll_spawner_variant(spawner, base_seed).map(|(dex, name, level, _)| (dex, name, level))
}

/// Selects one encounter and a deterministic port-side appearance variant.
/// The original alpha draw probability is not present in the local evidence;
/// alpha therefore follows only an explicit decoded slot flag, while normal
/// size categories are a stable hash choice over the decoded S/M/L scales.
pub fn roll_spawner_variant(
    spawner: &SpawnerDefinition,
    base_seed: u64,
) -> Option<(u16, String, u8, SpawnVariant)> {
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

    let alpha = chosen_entry.alpha || chosen_entry.unique_alpha;
    let size_scale = if alpha {
        2.5 // LL_field; raw value verified in pokemon_variants.tsv.
    } else {
        match splitmix64(&mut rng_state) % 3 {
            0 => 0.85, // S_field
            1 => 1.0,  // M_field
            _ => 1.8,  // L_field
        }
    };
    Some((
        chosen_entry.species_id,
        chosen_entry.species_name.clone(),
        lvl,
        SpawnVariant {
            form: chosen_entry.form,
            alpha,
            size_scale,
        },
    ))
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
    vec![WildCreature {
        species_name: "pikachu".into(),
        national_dex: 25,
        level: 5,
        spawn_pos: Vec3::new(player_spawn.x + 6.0, 0.0, player_spawn.y - 5.0),
        aggro_range: 2.8,
        in_battle: false,
        defeated: false,
        spawner_id: "fallback_pika".into(),
    }]
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[test]
    fn fate_from_final_state() {
        assert_eq!(wild_fate(0, false), WildFate::Defeated);
        assert_eq!(wild_fate(0, true), WildFate::Captured);
        assert_eq!(wild_fate(7, true), WildFate::Captured);
        assert_eq!(wild_fate(7, false), WildFate::Fled);
        assert!(WildFate::Defeated.removes_creature());
        assert!(WildFate::Captured.removes_creature());
        assert!(!WildFate::Fled.removes_creature());
    }

    #[test]
    fn consumed_spawner_blocks_until_respawn_time() {
        let mut c = ConsumedSpawners::default();
        assert!(!c.is_consumed("a", 0.0));
        c.consume("a", 100.0);
        assert!(c.is_consumed("a", 100.0));
        assert!(c.is_consumed("a", 100.0 + RESPAWN_AFTER_S - 1.0));
        assert!(!c.is_consumed("a", 100.0 + RESPAWN_AFTER_S));
        assert!(!c.is_consumed("b", 100.0), "other spawners are unaffected");
    }

    #[test]
    fn purge_and_clear_forget_spawners() {
        let mut c = ConsumedSpawners::default();
        c.consume("a", 0.0);
        c.consume("b", 500.0);
        c.purge_expired(RESPAWN_AFTER_S + 1.0);
        assert_eq!(c.len(), 1);
        assert!(c.is_consumed("b", RESPAWN_AFTER_S + 1.0));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn flee_cooldown_counts_down_then_ends() {
        assert_eq!(flee_cooldown_step(1.0, 0.25), Some(0.75));
        assert_eq!(flee_cooldown_step(0.2, 0.25), None);
        assert_eq!(
            flee_cooldown_step(FLEE_COOLDOWN_S, 0.0),
            Some(FLEE_COOLDOWN_S)
        );
    }
}
