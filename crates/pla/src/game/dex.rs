//! Pokédex research (brief 43): the rank ladder and the per-species research tasks.
//!
//! What IS decoded (PROBABLE): the player's research **rank ladder** ([`RANKS`] and
//! `data/dex_ranks.tsv`) recovered from `bin/appli/pokedex/res_table/pokedex_rank_table.bin` by
//! `.tools/dex_research_probe.py`, and the per-species **research-task catalog** ([`DexTasks`] and
//! `data/dex_tasks.tsv`, 1611 tasks over 242 species) recovered from
//! `pokedex_research_task_table.bin` by `.tools/decode_dex_tasks.py`. The tasks carry a kind
//! (0..=27), a target (a move for kind 1) and up to five ascending count thresholds; the game's
//! own reader functions (`FUN_012f7198`, `FUN_01030984`, `FUN_0102ecd0`, `FUN_010308d0`) are what
//! fixes that structure — see `reports/data-progress/dex_research.md`.
//!
//! What stays UNKNOWN (do not invent it): the label of every task kind except the two the port
//! feeds (caught, used-move), the meaning of the raw `type_byte`/`param5` fields, of the constant
//! `10` field used for [`POINTS_PER_STEP`] (ASSUMED multiplier) and of the two 64-bit hash slots.
//! Game text is out of scope; nothing here reads it.

use std::path::Path;

use bevy::prelude::Resource;

/// `(rank id, research points needed)`, ascending. PROBABLE: nine records of the rank table form
/// a monotone ladder (rank 10 needs 60000 points, rank 2 needs 1800).
pub const RANKS: [(u8, u32); 9] = [
    (2, 1800),
    (3, 3500),
    (4, 6000),
    (5, 8500),
    (6, 11000),
    (7, 15000),
    (8, 20000),
    (9, 30000),
    (10, 60000),
];

/// Research rank for a number of research points: the highest decoded rank whose threshold is
/// reached. Below the lowest decoded threshold the rank is 1 (SUPUESTO: the game's rank 1
/// threshold has no readable record).
pub fn rank_for_points(points: u32) -> u8 {
    RANKS
        .iter()
        .rev()
        .find(|(_, needed)| points >= *needed)
        .map_or(1, |(rank, _)| *rank)
}

/// Research points the next rank needs, `None` at the top rank.
pub fn next_rank_at(points: u32) -> Option<(u8, u32)> {
    RANKS
        .iter()
        .find(|(_, needed)| points < *needed)
        .copied()
}

// -------------------------------------------------------------- research tasks

/// Points one completed task step awards to its species. ASSUMED multiplier: every task record of
/// `pokedex_research_task_table.bin` carries the constant `10` in the field right after its
/// thresholds (UNKNOWN meaning, see `data/dex_tasks.tsv`), and the level delta the game's
/// `FUN_01030984` computes per task is the only reward quantity recovered; `10` per step is the
/// reading that makes a species' full task list sum to the 200 clamp the code applies
/// (`FUN_010308d0`). Not proven by code.
pub const POINTS_PER_STEP: u32 = 10;

/// Highest species research level (the ladder is 1..10).
pub const MAX_SPECIES_LEVEL: u8 = 10;

/// Highest task level (five thresholds → level 2..6, plus the 1 base level).
pub const MAX_TASK_LEVEL: u8 = 6;

/// Kind whose counter is the species' registration counter. PROBABLE: it is the default kind
/// (absent field), the first task of 240 of the 242 species, and the counter the game's own
/// readers (`FUN_0102ef8c`, `FUN_0103442c`) test for "has been registered".
pub const KIND_CAUGHT: u8 = 0;

/// Kind whose target is a move of the species. PROBABLE: every one of the 393 kind-1 records
/// carries a move id the species can learn (spot-checked: Pikachu 84/85 thunder-shock/thunderbolt,
/// Vulpix 52 ember), and `FUN_01030984` counts per-species move slots for it.
pub const KIND_USED_MOVE: u8 = 1;

/// Contribution of one species to the rank total when it is not perfect (code: `FUN_010308d0`
/// uses the capped species points; the 99/100 split needs a "perfect" flag the port reads as all
/// tasks complete).
pub const RANK_POINTS_CAP: u32 = 99;

/// Contribution of a perfectly researched species (code: `FUN_010308d0` returns 200 for it).
pub const RANK_POINTS_PERFECT: u32 = 200;

/// One research task of one species, as decoded by `.tools/decode_dex_tasks.py`
/// (see `data/dex_tasks.tsv`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DexTask {
    /// National dex id of the species the task researches.
    pub species: u16,
    /// Index of the task inside its species (the order the game's loader walks).
    pub slot: u8,
    /// Task kind (0..=27). Only [`KIND_CAUGHT`] and [`KIND_USED_MOVE`] are fed by port events;
    /// the meaning of the other kinds is UNKNOWN.
    pub kind: u8,
    /// Move id for [`KIND_USED_MOVE`]; 0 otherwise.
    pub move_target: u16,
    /// Raw second parameter (0..=17 across the 127 records that carry one). UNKNOWN.
    pub type_byte: u8,
    /// Raw third parameter (2..=4 across the 26 records that carry one). UNKNOWN.
    pub param5: u8,
    /// Up to five strictly ascending count thresholds; zeros are absent slots.
    pub thresholds: [u32; 5],
    /// Number of leading nonzero thresholds (the task's step count, 1..=5).
    pub steps: u8,
}

impl DexTask {
    /// Counter key this task reads: `(species, kind, target)`. Only kinds 0 and 1 are fed by
    /// events today; the other kinds map their own parameter so a future feed addresses the same
    /// key the record uses.
    pub fn counter_target(&self) -> u16 {
        match self.kind {
            KIND_USED_MOVE => self.move_target,
            2 => u16::from(self.type_byte),
            6 | 7 | 8 | 9 => u16::from(self.param5),
            10 => u16::from(self.param5),
            _ => 0,
        }
    }

    /// Level of the task (1..=6) for a counter value: the base level 1 plus one per threshold
    /// passed, capped at `steps + 1`. The cascade is the game's `FUN_01030984`; the 3-bit per-task
    /// level field of its save slot holds exactly this value.
    pub fn level(&self, counter: u32) -> u8 {
        let passed = self
            .thresholds
            .iter()
            .take(usize::from(self.steps))
            .take_while(|threshold| counter >= **threshold)
            .count() as u8;
        (1 + passed).min(MAX_TASK_LEVEL)
    }

    /// Points the task's current level is worth to its species.
    pub fn points(&self, counter: u32) -> u32 {
        u32::from(self.level(counter) - 1) * POINTS_PER_STEP
    }

    /// The task is complete when its last threshold is passed.
    pub fn complete(&self, counter: u32) -> bool {
        self.level(counter) == self.steps.min(MAX_TASK_LEVEL - 1) + 1
    }
}

/// The decoded research-task catalog (`data/dex_tasks.tsv`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Resource)]
pub struct DexTasks {
    pub entries: Vec<DexTask>,
}

impl DexTasks {
    /// Parse the tracked sheet; only typed rows are read.
    pub fn from_tsv(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'));
        let headers: Vec<&str> = lines
            .next()
            .ok_or_else(|| "dex_tasks: missing header".to_string())?
            .split('\t')
            .map(|field| field.split_once(':').map_or(field, |(name, _)| name))
            .collect();
        let column = |name: &str| {
            headers
                .iter()
                .position(|header| *header == name)
                .ok_or_else(|| format!("dex_tasks: missing column {name}"))
        };
        let num = |values: &[&str], index: usize| {
            values[index]
                .parse::<u32>()
                .map_err(|_| format!("dex_tasks: bad number {index}"))
        };
        let (c_species, c_slot, c_kind) = (column("species")?, column("slot")?, column("kind")?);
        let (c_move, c_type, c_param5) = (column("move")?, column("type_byte")?, column("param5")?);
        let (c_t1, c_steps) = (column("t1")?, column("steps")?);
        let mut entries = Vec::new();
        for (line_index, line) in lines.enumerate() {
            let values: Vec<&str> = line.split('\t').collect();
            if values.len() != headers.len() {
                return Err(format!(
                    "dex_tasks row {}: expected {} fields, got {}",
                    line_index + 2,
                    headers.len(),
                    values.len()
                ));
            }
            let mut thresholds = [0u32; 5];
            for (i, threshold) in thresholds.iter_mut().enumerate() {
                *threshold = num(&values, c_t1 + i)?;
            }
            let steps = num(&values, c_steps)?;
            if steps == 0 || steps > 5 {
                return Err(format!("dex_tasks row {}: steps {steps} out of 1..=5", line_index + 2));
            }
            entries.push(DexTask {
                species: num(&values, c_species)? as u16,
                slot: num(&values, c_slot)? as u8,
                kind: num(&values, c_kind)? as u8,
                move_target: num(&values, c_move)? as u16,
                type_byte: num(&values, c_type)? as u8,
                param5: num(&values, c_param5)? as u8,
                thresholds,
                steps: steps as u8,
            });
        }
        Ok(Self { entries })
    }

    /// Load `dex_tasks.tsv` from a tracked data directory.
    pub fn load(data_dir: impl AsRef<Path>) -> Result<Self, String> {
        let path = data_dir.as_ref().join("dex_tasks.tsv");
        let text = std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        Self::from_tsv(&text)
    }

    pub fn tasks_for(&self, species: u16) -> impl Iterator<Item = &DexTask> {
        self.entries.iter().filter(move |task| task.species == species)
    }

    /// Distinct species with at least one decoded task, ascending.
    pub fn species(&self) -> Vec<u16> {
        let mut ids: Vec<u16> = Vec::new();
        for task in &self.entries {
            if ids.last() != Some(&task.species) {
                ids.push(task.species);
            }
        }
        ids.sort();
        ids.dedup();
        ids
    }
}

/// Per-species research counters: `(species, kind, target) -> count`. The port stores this in the
/// save (v9, `PlayerProgress::research`) and feeds it from real events.
pub type ResearchCounters = std::collections::BTreeMap<(u16, u8, u16), u32>;

/// Registered when the species has been caught at least once (the counter feature the game's
/// readers use as "has research").
pub fn is_registered(counters: &ResearchCounters, species: u16) -> bool {
    counters.get(&(species, KIND_CAUGHT, 0)).copied().unwrap_or(0) > 0
}

/// Research points of one species: the sum of its tasks' level points.
pub fn species_points(tasks: &[DexTask], counters: &ResearchCounters, species: u16) -> u32 {
    tasks
        .iter()
        .filter(|task| task.species == species)
        .map(|task| {
            let count = counters
                .get(&(task.species, task.kind, task.counter_target()))
                .copied()
                .unwrap_or(0);
            task.points(count)
        })
        .sum()
}

/// Research level of a species (1..=10). SUPUESTO: the level is points/10, because the code caps
/// a species at 100/200 points and the dex ladder is 1..10; the exact level boundaries were not
/// recovered.
pub fn species_level(points: u32) -> u8 {
    (1 + points / POINTS_PER_STEP).min(u32::from(MAX_SPECIES_LEVEL)) as u8
}

/// A species is perfect when every one of its tasks is complete (all thresholds passed).
pub fn species_perfect(tasks: &[DexTask], counters: &ResearchCounters, species: u16) -> bool {
    let mut any = false;
    for task in tasks.iter().filter(|task| task.species == species) {
        any = true;
        let count = counters
            .get(&(task.species, task.kind, task.counter_target()))
            .copied()
            .unwrap_or(0);
        if !task.complete(count) {
            return false;
        }
    }
    any
}

/// Rank points one species contributes: 200 when perfect, otherwise its points capped at 99
/// (the shape of `FUN_010308d0`, whose 99/100 split follows the species' "perfect" flag).
pub fn species_rank_contribution(points: u32, perfect: bool) -> u32 {
    if perfect {
        RANK_POINTS_PERFECT
    } else {
        points.min(RANK_POINTS_CAP)
    }
}

/// Total rank points: the sum over every registered species ([`is_registered`]) of its
/// contribution. Deterministic (species order).
pub fn rank_points(tasks: &DexTasks, counters: &ResearchCounters) -> u32 {
    tasks
        .species()
        .into_iter()
        .filter(|species| is_registered(counters, *species))
        .map(|species| {
            let points = species_points(&tasks.entries, counters, species);
            species_rank_contribution(points, species_perfect(&tasks.entries, counters, species))
        })
        .sum()
}

/// Rank for a counter map (the ladder of [`rank_for_points`]).
pub fn rank_for_counters(tasks: &DexTasks, counters: &ResearchCounters) -> u8 {
    rank_for_points(rank_points(tasks, counters))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_is_sorted_unique_and_monotone() {
        assert_eq!(RANKS.len(), 9);
        for pair in RANKS.windows(2) {
            assert!(pair[0].0 < pair[1].0, "ranks ascending");
            assert!(pair[0].1 < pair[1].1, "points ascending");
        }
    }

    #[test]
    fn rank_boundaries_round_down_to_the_reached_rank() {
        assert_eq!(rank_for_points(0), 1, "SUPUESTO floor below the first decoded rank");
        assert_eq!(rank_for_points(1799), 1);
        assert_eq!(rank_for_points(1800), 2);
        assert_eq!(rank_for_points(59999), 9);
        assert_eq!(rank_for_points(60000), 10);
        assert_eq!(rank_for_points(u32::MAX), 10, "the top rank holds");
        assert_eq!(next_rank_at(0), Some((2, 1800)));
        assert_eq!(next_rank_at(1800), Some((3, 3500)));
        assert_eq!(next_rank_at(60000), None);
    }

    /// The code table and the tracked sheet are the same data (the sheet carries the provenance:
    /// file and offset of each probed record).
    #[test]
    fn the_model_matches_the_tracked_sheet() {
        let sheet = include_str!("../../../../data/dex_ranks.tsv");
        let mut rows = Vec::new();
        for line in sheet.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.first() == Some(&"rank:u8") {
                continue; // typed header
            }
            assert_eq!(fields.len(), 5, "row shape: {line}");
            assert_eq!(fields[2], "PROBABLE", "every row keeps its confidence: {line}");
            assert_eq!(fields[3], "pokedex_rank_table.bin");
            rows.push((fields[0].parse::<u8>().unwrap(), fields[1].parse::<u32>().unwrap()));
        }
        assert_eq!(rows, RANKS);
    }

    // ------------------------------------------------------ research tasks

    fn task(kind: u8, move_target: u16, thresholds: [u32; 5], steps: u8) -> DexTask {
        DexTask {
            species: 25,
            slot: 0,
            kind,
            move_target,
            type_byte: 0,
            param5: 0,
            thresholds,
            steps,
        }
    }

    /// The tracked sheet parses and carries the measured shape: 1611 tasks over 242 species,
    /// every step count in 1..=5, every kind inside the range the game's code switches on.
    #[test]
    fn the_tracked_task_sheet_has_the_measured_shape() {
        let tasks = DexTasks::from_tsv(include_str!("../../../../data/dex_tasks.tsv")).unwrap();
        assert_eq!(tasks.entries.len(), 1611);
        assert_eq!(tasks.species().len(), 242);
        assert!(tasks.entries.iter().all(|t| (1..=5).contains(&t.steps)));
        assert!(tasks.entries.iter().all(|t| t.kind <= 27));
        // 240 species carry the default "caught" task; 393 tasks are "used move".
        assert_eq!(tasks.entries.iter().filter(|t| t.kind == KIND_CAUGHT).count(), 240);
        assert_eq!(tasks.entries.iter().filter(|t| t.kind == KIND_USED_MOVE).count(), 393);
        // Every move task names a move; the sheet keeps species in ascending runs.
        assert!(tasks
            .entries
            .iter()
            .filter(|t| t.kind == KIND_USED_MOVE)
            .all(|t| t.move_target > 0));
        assert!(tasks.entries.windows(2).all(|pair| pair[0].species <= pair[1].species));
        // Pikachu's first task (caught, 1/2/4/10/15) and its two move tasks, as decoded.
        let pikachu: Vec<&DexTask> = tasks.tasks_for(25).collect();
        assert_eq!(pikachu.len(), 8);
        assert_eq!(pikachu[0].kind, KIND_CAUGHT);
        assert_eq!(pikachu[0].thresholds, [1, 2, 4, 10, 15]);
        assert_eq!(pikachu[0].steps, 5);
        let move_tasks: Vec<u16> = pikachu
            .iter()
            .filter(|t| t.kind == KIND_USED_MOVE)
            .map(|t| t.move_target)
            .collect();
        assert_eq!(move_tasks, [84, 85], "thunder-shock and thunderbolt");
    }

    #[test]
    fn task_levels_follow_the_threshold_cascade() {
        let t = task(KIND_CAUGHT, 0, [1, 2, 4, 10, 15], 5);
        assert_eq!(t.level(0), 1, "nothing passed");
        assert_eq!(t.level(1), 2);
        assert_eq!(t.level(3), 3, "2 passed, 4 not yet");
        assert_eq!(t.level(15), 6, "all five passed");
        assert_eq!(t.level(u32::MAX), 6, "the level caps at steps + 1");
        assert!(t.complete(15));
        assert!(!t.complete(14));
        assert_eq!(t.points(0), 0);
        assert_eq!(t.points(15), 50, "five steps at ten points");
        // A one-step task caps at level 2 (1 step + base).
        let short = task(27, 0, [1, 0, 0, 0, 0], 1);
        assert_eq!(short.level(1), 2);
        assert_eq!(short.level(99), 2);
        assert_eq!(short.points(99), 10);
    }

    #[test]
    fn species_points_level_and_perfection() {
        let tasks = DexTasks {
            entries: vec![
                task(KIND_CAUGHT, 0, [1, 2, 4, 6, 10], 5),
                DexTask {
                    slot: 1,
                    kind: KIND_USED_MOVE,
                    move_target: 84,
                    thresholds: [3, 6, 12, 25, 0],
                    steps: 4,
                    ..task(KIND_USED_MOVE, 84, [0; 5], 4)
                },
            ],
        };
        let mut counters = ResearchCounters::new();
        assert!(species_points(&tasks.entries, &counters, 25) == 0);
        assert!(!is_registered(&counters, 25));
        counters.insert((25, KIND_CAUGHT, 0), 2); // caught 2 → two thresholds passed
        assert!(is_registered(&counters, 25));
        assert_eq!(species_points(&tasks.entries, &counters, 25), 20);
        counters.insert((25, KIND_USED_MOVE, 84), 12); // move used 12 times → three thresholds
        assert_eq!(species_points(&tasks.entries, &counters, 25), 50);
        assert_eq!(species_level(50), 6);
        assert_eq!(species_level(0), 1);
        assert_eq!(species_level(90), 10);
        assert_eq!(species_level(u32::MAX), 10);
        assert!(!species_perfect(&tasks.entries, &counters, 25), "the move task is not at 25 yet");
        counters.insert((25, KIND_CAUGHT, 0), 10); // caught at its last threshold
        counters.insert((25, KIND_USED_MOVE, 84), 25); // move at its last threshold
        assert!(species_perfect(&tasks.entries, &counters, 25));
        assert_eq!(species_rank_contribution(120, false), RANK_POINTS_CAP);
        assert_eq!(species_rank_contribution(120, true), RANK_POINTS_PERFECT);
        assert_eq!(species_rank_contribution(40, false), 40);
    }

    /// A sequence of catches climbs the species and the rank, and a full task list turns the
    /// species' contribution into the 200 the game's reader returns for a perfect entry.
    #[test]
    fn capture_sequence_climbs_points_and_rank() {
        let tasks = DexTasks {
            entries: vec![task(KIND_CAUGHT, 0, [1, 2, 4, 6, 10], 5)],
        };
        let mut counters = ResearchCounters::new();
        for catch in 1..=10u32 {
            counters.insert((25, KIND_CAUGHT, 0), catch);
            let points = species_points(&tasks.entries, &counters, 25);
            let expected = match catch {
                1 => 10,
                2 | 3 => 20,
                4 | 5 => 30,
                6..=9 => 40,
                _ => 50,
            };
            assert_eq!(points, expected, "catches {catch}");
            let contribution = if catch == 10 { RANK_POINTS_PERFECT } else { expected };
            assert_eq!(rank_points(&tasks, &counters), contribution, "catches {catch}");
        }
        assert!(species_perfect(&tasks.entries, &counters, 25));
        assert_eq!(rank_points(&tasks, &counters), RANK_POINTS_PERFECT);
        assert_eq!(rank_for_counters(&tasks, &counters), 1, "below rank 2's threshold");
    }
}
