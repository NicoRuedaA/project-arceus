//! Mass outbreaks (Brief 39): regular outbreaks from decoded data, massive ones blocked.
//!
//! Regular outbreaks (`data/world_events_outbreak_areas.tsv`, `..._outbreak_spawners.tsv`):
//! - every area 01..05 has `outbreak_count` outbreak ids (46/44/44/48/45) and a story unlock gate
//!   (VERIFIED links);
//! - for each id exactly one spawner is gated `WSYS_MASS_GANERATION_AREAnn == id` (the outbreak
//!   spawner: one species, 4 alive) and up to four regular spawners are gated `!= id` (switched off
//!   during it). The disabled set is rebuilt here from `data/field_spawn_table.tsv` (every row
//!   gated `WSYS_MASS_GANERATION_AREAnn=id` that is not the outbreak spawner) and checked against
//!   the sheet's `disabled_spawners` count: 227/227 match (test below);
//! - outside their outbreak, the 227 outbreak spawners are OFF (they are in the regular spawn
//!   table, which the viewer otherwise loads unconditionally);
//! - ASSUMED: one outbreak per unlocked area per game day, id drawn uniformly from the session
//!   seed and the day; it ends after `total_min..=total_max` (10..15, ASSUMED meaning) Pokémon are
//!   defeated or caught, or when the day changes. How the game draws ids is UNKNOWN.
//!
//! Massive mass outbreaks: the per-area settings and the 166 groups are decoded, but none of the
//! encounter keys the groups use exists in any decoded table, so their species are UNKNOWN.
//! [`massive_outbreak_report`] resolves what it can (0 keys today) and every unresolved group is
//! skipped; nothing is invented.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use bevy::math::Vec3;

use super::distortion::{EventSpawnSlot, SpawnerRole};
use super::wild::{hash_str, splitmix64};
use super::world_events::{parse_slot, rng_range, EventSlot, Sheet, StoryGate};

/// One area row of `world_events_outbreak_areas.tsv`.
#[derive(Debug, Clone, PartialEq)]
pub struct OutbreakArea {
    pub area: String,
    pub work: String,
    pub gate: StoryGate,
    pub outbreak_count: u16,
    pub total_min: u16,
    pub total_max: u16,
    /// Unexplained raw values (20 and 25 in every row).
    pub f1_raw: u32,
    pub f7_raw: u32,
}

/// The single spawner of one outbreak id.
#[derive(Debug, Clone, PartialEq)]
pub struct OutbreakSpawner {
    pub spawner_id: String,
    pub pos: Vec3,
    pub radius: f32,
    pub min_count: u16,
    pub max_count: u16,
    pub slots: Vec<EventSlot>,
    /// The sheet's `disabled_spawners` count (for the cross-check).
    pub disabled_count: u16,
}

#[derive(Debug, Clone, Default)]
pub struct OutbreakData {
    pub areas: BTreeMap<String, OutbreakArea>,
    /// `(area, outbreak id)`.
    pub spawners: HashMap<(String, u16), OutbreakSpawner>,
    /// `(area, outbreak id)` -> regular spawner ids switched off during it.
    pub disabled: HashMap<(String, u16), BTreeSet<String>>,
}

impl OutbreakData {
    /// `spawn_table` is `data/field_spawn_table.tsv` (only its `gate_flags` column is used).
    pub fn parse(areas: &str, spawners: &str, spawn_table: &str) -> Self {
        let mut d = Self::default();
        let s = Sheet::parse(areas);
        for r in 0..s.len() {
            let Ok(count) = s.get(r, "outbreak_count").parse::<u16>() else { continue };
            let area = s.get(r, "area").to_string();
            d.areas.insert(
                area.clone(),
                OutbreakArea {
                    area,
                    work: s.get(r, "work").to_string(),
                    gate: StoryGate::new(s.get(r, "story_cmp"), s.get(r, "story_value"), ""),
                    outbreak_count: count,
                    total_min: s.get(r, "total_min").parse().unwrap_or(10),
                    total_max: s.get(r, "total_max").parse().unwrap_or(15),
                    f1_raw: s.get(r, "f1_raw").parse().unwrap_or(0),
                    f7_raw: s.get(r, "f7_raw").parse().unwrap_or(0),
                },
            );
        }
        let s = Sheet::parse(spawners);
        for r in 0..s.len() {
            let Ok(id) = s.get(r, "outbreak_id").parse::<u16>() else { continue };
            let f = |n: &str| s.get(r, n).parse::<f32>().ok().filter(|v| v.is_finite());
            let (Some(x), Some(y), Some(z)) = (f("x"), f("y"), f("z")) else { continue };
            let Some(slot) = parse_slot(&s, r) else { continue };
            let min_count = s.get(r, "min_count").parse().unwrap_or(4);
            let entry = d.spawners.entry((s.get(r, "area").to_string(), id)).or_insert_with(|| OutbreakSpawner {
                spawner_id: s.get(r, "spawner_id").to_string(),
                pos: Vec3::new(x, y, z),
                radius: f("radius").unwrap_or(7.0),
                min_count,
                max_count: s.get(r, "max_count").parse().unwrap_or(min_count).max(min_count),
                slots: Vec::new(),
                disabled_count: s.get(r, "disabled_spawners").parse().unwrap_or(0),
            });
            entry.slots.push(slot);
        }
        // Regular spawners gated on the outbreak work: `WSYS_MASS_GANERATION_AREAnn=<id>`.
        let s = Sheet::parse(spawn_table);
        for r in 0..s.len() {
            let gate = s.get(r, "gate_flags");
            let Some((work, value)) = gate.split_once('=') else { continue };
            let Some(area) = d.areas.values().find(|a| a.work == work).map(|a| a.area.clone()) else { continue };
            let Ok(id) = value.parse::<u16>() else { continue };
            let sid = s.get(r, "spawner_id");
            if d.spawners.get(&(area.clone(), id)).is_some_and(|o| o.spawner_id == sid) {
                continue;
            }
            d.disabled.entry((area, id)).or_default().insert(sid.to_string());
        }
        d
    }

    /// Spawner ids of every outbreak of `area`: they stay off unless their outbreak is active.
    pub fn outbreak_spawner_ids(&self, area: &str) -> BTreeSet<String> {
        self.spawners.iter().filter(|((a, _), _)| a == area).map(|(_, o)| o.spawner_id.clone()).collect()
    }

    /// Whether outbreaks are unlocked in `area` at `story`.
    pub fn unlocked(&self, area: &str, story: u32) -> bool {
        self.areas.get(area).is_some_and(|a| a.gate.matches(story))
    }

    /// ASSUMED draw: the outbreak id of `area` on game `day` (uniform over `1..=outbreak_count`),
    /// `None` when outbreaks are locked or the area has none.
    pub fn draw(&self, area: &str, story: u32, day: u32, seed: u64) -> Option<u16> {
        let a = self.areas.get(area)?;
        if !a.gate.matches(story) || a.outbreak_count == 0 {
            return None;
        }
        let mut rng = seed ^ hash_str(area) ^ (u64::from(day) << 32 | 0x0b5e);
        let id = 1 + (splitmix64(&mut rng) % u64::from(a.outbreak_count)) as u16;
        self.spawners.contains_key(&(area.to_string(), id)).then_some(id)
    }

    /// How many Pokémon this outbreak holds in total (ASSUMED `total_min..=total_max`).
    pub fn roll_total(&self, area: &str, id: u16, day: u32, seed: u64) -> u16 {
        let Some(a) = self.areas.get(area) else { return 0 };
        let mut rng = seed ^ hash_str(area) ^ u64::from(id) ^ (u64::from(day) << 40);
        rng_range(&mut rng, u32::from(a.total_min), u32::from(a.total_max.max(a.total_min))) as u16
    }

    /// The alive slots of outbreak `id`: `max_count` (4) slots around the spawner centre,
    /// spread inside its radius (spread ASSUMED; centre and radius are decoded).
    pub fn spawn_slots(&self, area: &str, id: u16, seed: u64) -> Vec<EventSpawnSlot> {
        let Some(o) = self.spawners.get(&(area.to_string(), id)) else { return Vec::new() };
        let mut rng = seed ^ hash_str(&o.spawner_id) ^ u64::from(id);
        (0..o.max_count.max(1))
            .map(|k| {
                let ang = (splitmix64(&mut rng) % 3600) as f32 / 3600.0 * std::f32::consts::TAU;
                let r = o.radius.max(1.0) * ((splitmix64(&mut rng) % 1000) as f32 / 1000.0).sqrt();
                EventSpawnSlot {
                    key: format!("ob:{area}:{id}:{k}"),
                    pos: o.pos + Vec3::new(ang.cos() * r, 0.0, ang.sin() * r),
                    radius: o.radius,
                    slots: o.slots.clone(),
                    role: SpawnerRole::Main,
                }
            })
            .collect()
    }

    /// Main species name of an outbreak (its non-alpha slot) for logs.
    pub fn species(&self, area: &str, id: u16) -> Option<&str> {
        self.spawners
            .get(&(area.to_string(), id))
            .and_then(|o| o.slots.iter().find(|s| !s.alpha).or(o.slots.first()))
            .map(|s| s.species.as_str())
    }
}

/// Result of resolving the massive-outbreak groups against every decoded encounter table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MassiveOutbreakReport {
    pub groups: usize,
    pub keys: usize,
    pub resolved_keys: usize,
    /// Groups with at least one resolved key (the only ones that could ever spawn).
    pub usable_groups: usize,
}

/// `groups` is `world_events_mmo_groups.tsv`; `known_keys` every encounter key of the decoded
/// tables (regular spawns, distortions, outbreaks).
pub fn massive_outbreak_report(groups: &str, known_keys: &BTreeSet<String>) -> MassiveOutbreakReport {
    let s = Sheet::parse(groups);
    let mut by_group: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for r in 0..s.len() {
        by_group.entry(s.get(r, "group_key")).or_default().push(s.get(r, "encount_key"));
    }
    let keys: BTreeSet<&str> = by_group.values().flatten().copied().collect();
    MassiveOutbreakReport {
        groups: by_group.len(),
        keys: keys.len(),
        resolved_keys: keys.iter().filter(|k| known_keys.contains(**k)).count(),
        usable_groups: by_group.values().filter(|ks| ks.iter().any(|k| known_keys.contains(*k))).count(),
    }
}

/// Every `encount_key` column value of a sheet (helper for [`massive_outbreak_report`]).
pub fn encount_keys(text: &str) -> BTreeSet<String> {
    let s = Sheet::parse(text);
    (0..s.len()).map(|r| s.get(r, "encount_key").to_string()).filter(|k| !k.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::world_objects::find_data_file;

    fn read(name: &str) -> Option<String> {
        find_data_file(name).and_then(|p| std::fs::read_to_string(p).ok())
    }

    pub(crate) fn real() -> Option<OutbreakData> {
        Some(OutbreakData::parse(
            &read("world_events_outbreak_areas.tsv")?,
            &read("world_events_outbreak_spawners.tsv")?,
            &read("field_spawn_table.tsv")?,
        ))
    }

    #[test]
    fn real_outbreak_data_and_disabled_sets_match_the_sheet() {
        let Some(d) = real() else {
            eprintln!("[skip] outbreak sheets not found");
            return;
        };
        let counts: Vec<u16> = d.areas.values().map(|a| a.outbreak_count).collect();
        assert_eq!(counts, vec![46, 44, 44, 48, 45]);
        assert_eq!(d.spawners.len(), 227, "one spawner per outbreak id");
        let mut ok = 0;
        for (k, o) in &d.spawners {
            let got = d.disabled.get(k).map_or(0, BTreeSet::len);
            assert_eq!(got, usize::from(o.disabled_count), "{k:?}");
            assert_eq!((o.min_count, o.max_count), (4, 4), "{k:?}");
            ok += 1;
        }
        assert_eq!(ok, 227, "227/227 disabled sets rebuilt from field_spawn_table");
        // Area 01, outbreak 1 = Pikachu at the decoded spawner.
        assert_eq!(d.species("ha_area01", 1), Some("pikachu"));
        assert_eq!(d.outbreak_spawner_ids("ha_area01").len(), 46);
        // Story gates: area 01 opens at 14800.
        assert!(!d.unlocked("ha_area01", 14_799));
        assert!(d.unlocked("ha_area01", 14_800));
        assert!(!d.unlocked("ha_area00", 1_000_000), "no outbreaks in the village");
    }

    #[test]
    fn draw_is_deterministic_in_range_and_gated() {
        let Some(d) = real() else {
            eprintln!("[skip] outbreak sheets not found");
            return;
        };
        let mut ids = BTreeSet::new();
        for day in 0..400 {
            let id = d.draw("ha_area02", 100_000, day, 9).expect("unlocked");
            assert!((1..=44).contains(&id));
            assert_eq!(Some(id), d.draw("ha_area02", 100_000, day, 9));
            ids.insert(id);
        }
        assert!(ids.len() > 30, "uniform draw reaches most of the 44 ids, got {}", ids.len());
        assert_eq!(d.draw("ha_area02", 0, 0, 9), None, "locked before the story gate");
        for day in 0..50 {
            let t = d.roll_total("ha_area03", 5, day, 1);
            assert!((10..=15).contains(&t));
        }
    }

    #[test]
    fn outbreak_slots_stay_inside_the_spawner_radius() {
        let Some(d) = real() else {
            eprintln!("[skip] outbreak sheets not found");
            return;
        };
        let o = &d.spawners[&("ha_area01".to_string(), 1)];
        let slots = d.spawn_slots("ha_area01", 1, 5);
        assert_eq!(slots.len(), 4);
        for s in &slots {
            assert!(s.pos.distance(o.pos) <= o.radius + 1e-3);
            let e = s.entries(1, 2);
            assert_eq!(e.len(), 1, "alpha slot skipped, one species");
            assert_eq!((e[0].species_name.as_str(), e[0].min_level, e[0].max_level), ("pikachu", 11, 13));
        }
        assert_eq!(slots, d.spawn_slots("ha_area01", 1, 5));
    }

    #[test]
    fn massive_outbreak_species_stay_unknown() {
        let (Some(groups), Some(cond), Some(dist)) = (
            read("world_events_mmo_groups.tsv"),
            read("field_spawn_conditions.tsv"),
            read("world_events_distortion_encounters.tsv"),
        ) else {
            eprintln!("[skip] massive outbreak sheets not found");
            return;
        };
        let mut known = encount_keys(&cond);
        known.extend(encount_keys(&dist));
        let r = massive_outbreak_report(&groups, &known);
        assert_eq!(r.groups, 166);
        assert_eq!(r.keys, 334);
        assert_eq!((r.resolved_keys, r.usable_groups), (0, 0), "no species may be invented");
        // A synthetic resolvable key is counted, so the check is not vacuous.
        let first = encount_keys(&groups).into_iter().next().unwrap();
        known.insert(first);
        assert_eq!(massive_outbreak_report(&groups, &known).resolved_keys, 1);
    }
}
