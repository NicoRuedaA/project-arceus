//! Noble (frenzied boss) battle: the field phase (Brief 30 gap "fase de campo noble").
//!
//! This module is the rules model of the action part around the turn-based fight. It only
//! implements what the recovered evidence supports, and labels everything else SUPUESTO:
//!
//! - **Gauge**: `gauge_max` per setting is decoded (`data/boss_battles.tsv`, event-encounter
//!   slot 16; the capacity reading is LIKELY, not proven by code). Rematch settings have no
//!   capacity: no field phase ([`NobleField::from_def`] returns `None`).
//! - **Checkpoints**: crossing the 75 %, 50 % and 25 % quarter steps locks gauge progress and
//!   triggers the checkpoint feedback; the 0 % step dispatches the final pacification
//!   (`FUN_013326b8`, read in `reports/function-progress/n2-noble-boss-battles.md`; the
//!   `result_nushi` event name is VERIFIED there). While locked, the gauge holds at the
//!   crossed threshold (SUPUESTO: the report says "locks progress"; the exact hold value is
//!   not decoded).
//! - **Phases**: the director tracks a phase counter and the aura coordinator has a calm, a
//!   frenzy (yellow) and an enraged (orange/red) mode (VERIFIED modes). Which gauge value
//!   flips to phase 2 is not decoded: this model places it at the 50 % checkpoint (SUPUESTO).
//! - **Field attacks**: per setting (`data/boss_field_attacks.tsv`: kind, knock strength,
//!   status). Names carrying an `h` segment are ASSUMED to be the phase-2 (harder) variants.
//! - **Sequence (LIKELY, from strings and the N2 notes)**: field phase → checkpoint stagger
//!   window (`NUSHI_NO_SUKI`) → throwing the player's Pokémon during the window opens the
//!   turn-based battle → winning stuns the Noble → more balms → gauge 0 → pacification.
//!   [`NobleField::open_turn_battle`] models the bridge into the existing
//!   `ActiveBattleSession::start_noble`.
//! - **UNKNOWN, kept as named parameters**: balm damage per hit (native value not in any
//!   decoded table) and the stagger window length. The defaults are SUPUESTO placeholder
//!   values, never claimed as game data.
//!
//! The in-world Bevy system (Noble roaming, telegraphed attacks, balm ballistics) belongs to
//! Brief D5 and stays pending; this module is its rule core.

use std::collections::BTreeMap;

use super::trainer_data::NobleDef;

/// Gauge damage of one balm hit. UNKNOWN in the recovered data: SUPUESTO placeholder.
pub const BALM_DAMAGE_ASSUMED: u32 = 100;
/// Length of the stagger window opened by a checkpoint. UNKNOWN: SUPUESTO placeholder.
pub const STAGGER_SECONDS_ASSUMED: f32 = 8.0;

/// Quarter checkpoints (percent) that lock gauge progress; 0 is the pacification instead.
pub const CHECKPOINT_PERCENTS: [u32; 3] = [75, 50, 25];

/// Field phase of a Noble setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoblePhase {
    One,
    /// Enraged phase: aura mode B in the recovered code; its start point is SUPUESTO (50 %).
    Two,
}

/// One field attack of `data/boss_field_attacks.tsv`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NobleAttack {
    pub id: String,
    pub ns_key: String,
    pub kind: String,
    pub knock: Option<String>,
    pub status: Option<String>,
    /// Name carries the `h` segment: ASSUMED phase-2 (harder) variant.
    pub phase_two: bool,
}

/// What a gauge update did, for the caller (log line, audio, in-world system).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldEvent {
    /// The gauge crossed `percent`; progress is locked until the stagger window ends.
    Checkpoint { percent: u32 },
    /// The stagger window opened (checkpoint feedback; LIKELY reading of the sequence).
    Staggered,
    /// The stagger window ended: the gauge is unlocked again.
    StaggerEnded,
    /// Phase 2 (enraged) started.
    PhaseTwo,
    /// The gauge reached 0: `result_nushi` / pacification.
    Pacified,
}

/// The field phase state of one Noble.
#[derive(Debug, Clone, PartialEq)]
pub struct NobleField {
    pub ns_key: String,
    pub gauge_max: u32,
    pub gauge: u32,
    pub phase: NoblePhase,
    /// Frozen at a crossed checkpoint until the stagger window ends.
    pub locked: bool,
    pub staggered: bool,
    pub stagger_left: f32,
    /// Balm hits received (diagnostics).
    pub balms: u32,
    pub pacified: bool,
}

impl NobleField {
    pub fn new(ns_key: impl Into<String>, gauge_max: u32) -> Self {
        let gauge_max = gauge_max.max(1);
        Self {
            ns_key: ns_key.into(),
            gauge_max,
            gauge: gauge_max,
            phase: NoblePhase::One,
            locked: false,
            staggered: false,
            stagger_left: 0.0,
            balms: 0,
            pacified: false,
        }
    }

    /// Field phase of a setting: `None` when the setting has no gauge capacity (rematches,
    /// LIKELY reading).
    pub fn from_def(def: &NobleDef) -> Option<Self> {
        def.gauge_max.map(|max| Self::new(def.ns_key.clone(), max))
    }

    /// Gauge as a whole percent of its capacity (the script's cached percentage).
    pub fn percent(&self) -> u32 {
        self.gauge.saturating_mul(100) / self.gauge_max
    }

    /// A balm hit. Ignored while the gauge is locked (the script holds progress at a
    /// checkpoint) or after pacification.
    pub fn apply_balm(&mut self, damage: u32) -> Vec<FieldEvent> {
        if self.pacified || self.locked {
            return Vec::new();
        }
        let before = self.percent();
        self.gauge = self.gauge.saturating_sub(damage);
        self.balms += 1;
        let after = self.percent();
        let mut events = Vec::new();
        if after == 0 && before > 0 {
            self.pacified = true;
            events.push(FieldEvent::Pacified);
            return events;
        }
        for checkpoint in CHECKPOINT_PERCENTS {
            if before > checkpoint && after <= checkpoint {
                // Progress locked at the quarter step (SUPUESTO hold value).
                let threshold = checkpoint * self.gauge_max / 100;
                self.gauge = self.gauge.max(threshold);
                self.locked = true;
                self.staggered = true;
                self.stagger_left = STAGGER_SECONDS_ASSUMED;
                events.push(FieldEvent::Checkpoint { percent: checkpoint });
                events.push(FieldEvent::Staggered);
                if checkpoint == 50 && self.phase == NoblePhase::One {
                    self.phase = NoblePhase::Two;
                    events.push(FieldEvent::PhaseTwo);
                }
                break;
            }
        }
        events
    }

    /// Advance the stagger window; ends it (and unlocks the gauge) when it expires.
    pub fn tick(&mut self, dt: f32) -> Option<FieldEvent> {
        if !self.staggered {
            return None;
        }
        self.stagger_left -= dt.max(0.0);
        if self.stagger_left > 0.0 {
            return None;
        }
        self.staggered = false;
        self.locked = false;
        Some(FieldEvent::StaggerEnded)
    }

    /// While the Noble is staggered, throwing the player's Pokémon opens the turn-based
    /// battle (the bridge to `ActiveBattleSession::start_noble`).
    pub fn open_turn_battle(&mut self) -> Option<FieldEvent> {
        if self.staggered && !self.pacified {
            self.staggered = false;
            self.locked = false;
            return Some(FieldEvent::StaggerEnded);
        }
        None
    }

    /// Attacks available in the current phase. Phase 1 uses the plain attacks; phase 2 adds
    /// the `h` variants (ASSUMED reading of the `h` segment).
    pub fn attacks_for_phase<'a>(&self, attacks: &'a [NobleAttack]) -> Vec<&'a NobleAttack> {
        attacks
            .iter()
            .filter(|a| self.phase == NoblePhase::Two || !a.phase_two)
            .collect()
    }
}

/// Parse `data/boss_field_attacks.tsv` into attacks grouped by `ns_key`.
pub fn parse_field_attacks(text: &str) -> BTreeMap<String, Vec<NobleAttack>> {
    let mut map: BTreeMap<String, Vec<NobleAttack>> = BTreeMap::new();
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let Some(header) = lines.next() else { return map };
    let idx: BTreeMap<&str, usize> = header
        .split('\t')
        .enumerate()
        .map(|(i, h)| (h.split(':').next().unwrap_or(h), i))
        .collect();
    let cell = |row: &[&str], name: &str| -> String {
        idx.get(name).and_then(|i| row.get(*i)).copied().unwrap_or("").to_string()
    };
    for line in lines {
        let row: Vec<&str> = line.split('\t').collect();
        let id = cell(&row, "id");
        if id.is_empty() {
            continue;
        }
        let ns_key = cell(&row, "ns_key");
        map.entry(ns_key.clone()).or_default().push(NobleAttack {
            id,
            ns_key,
            kind: cell(&row, "kind"),
            knock: Some(cell(&row, "knock")).filter(|v| !v.is_empty()),
            status: Some(cell(&row, "status_effect")).filter(|v| !v.is_empty()),
            phase_two: cell(&row, "h_variant") == "1",
        });
    }
    map
}

/// Load the field attacks from the repo data directory (None when the sheet is missing).
pub fn load_field_attacks() -> Option<BTreeMap<String, Vec<NobleAttack>>> {
    let path = super::world_objects::find_data_file("boss_field_attacks.tsv")?;
    std::fs::read_to_string(path).ok().map(|t| parse_field_attacks(&t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attacks() -> Vec<NobleAttack> {
        vec![
            NobleAttack {
                id: "ns06_sp01".into(),
                ns_key: "ns06".into(),
                kind: "bombing".into(),
                knock: Some("down_strong".into()),
                status: None,
                phase_two: false,
            },
            NobleAttack {
                id: "ns06_sp02h".into(),
                ns_key: "ns06".into(),
                kind: "charge_attack".into(),
                knock: Some("down_weak".into()),
                status: None,
                phase_two: true,
            },
        ]
    }

    #[test]
    fn balm_hits_cross_checkpoints_once_and_lock_progress() {
        let mut field = NobleField::new("ns02", 1000);
        assert_eq!(field.percent(), 100);
        // Hits of 100 (10 %): 100 -> 90 -> 80 -> 70 crosses 75 and locks at the threshold.
        assert!(field.apply_balm(BALM_DAMAGE_ASSUMED).is_empty());
        assert!(field.apply_balm(BALM_DAMAGE_ASSUMED).is_empty());
        assert_eq!(
            field.apply_balm(BALM_DAMAGE_ASSUMED),
            vec![
                FieldEvent::Checkpoint { percent: 75 },
                FieldEvent::Staggered
            ]
        );
        assert_eq!(field.percent(), 75, "held at the crossed checkpoint");
        // Locked: further hits do nothing until the stagger window ends.
        let held = field.gauge;
        assert!(field.locked && field.staggered);
        assert!(field.apply_balm(BALM_DAMAGE_ASSUMED).is_empty());
        assert_eq!(field.gauge, held);
        assert_eq!(field.tick(STAGGER_SECONDS_ASSUMED), Some(FieldEvent::StaggerEnded));
        assert!(!field.locked && !field.staggered);
        // On to the next checkpoint: 75 -> 65 -> 55 -> 45 crosses 50 and starts phase 2.
        assert!(field.apply_balm(BALM_DAMAGE_ASSUMED).is_empty());
        assert!(field.apply_balm(BALM_DAMAGE_ASSUMED).is_empty());
        assert_eq!(
            field.apply_balm(BALM_DAMAGE_ASSUMED),
            vec![
                FieldEvent::Checkpoint { percent: 50 },
                FieldEvent::Staggered,
                FieldEvent::PhaseTwo
            ]
        );
        assert_eq!(field.percent(), 50);
        assert_eq!(field.phase, NoblePhase::Two);
    }

    #[test]
    fn a_single_big_hit_locks_at_the_first_crossed_checkpoint() {
        // One huge hit from 100 % to 20 %: the script locks progress at the quarter step it
        // crossed first (SUPUESTO hold value: the threshold itself).
        let mut field = NobleField::new("ns06", 1000);
        let events = field.apply_balm(800);
        assert_eq!(
            events,
            vec![
                FieldEvent::Checkpoint { percent: 75 },
                FieldEvent::Staggered
            ]
        );
        assert_eq!(field.percent(), 75);
        assert_eq!(field.phase, NoblePhase::One, "the 50 % step was not reached");
        field.tick(STAGGER_SECONDS_ASSUMED);
        assert_eq!(
            field.apply_balm(300),
            vec![
                FieldEvent::Checkpoint { percent: 50 },
                FieldEvent::Staggered,
                FieldEvent::PhaseTwo
            ]
        );
    }

    #[test]
    fn turn_battle_opens_only_during_the_stagger_window() {
        let mut field = NobleField::new("ns02", 300);
        assert_eq!(field.open_turn_battle(), None, "no window before a checkpoint");
        field.apply_balm(100); // 66 %
        assert!(field.staggered);
        assert_eq!(field.open_turn_battle(), Some(FieldEvent::StaggerEnded));
        assert!(!field.locked, "the battle opens the gauge again");
        assert_eq!(field.open_turn_battle(), None, "the window is consumed once");
    }

    #[test]
    fn phase_two_adds_the_h_variants() {
        let mut field = NobleField::new("ns06", 3000);
        let all = attacks();
        let one: Vec<&str> = field.attacks_for_phase(&all).iter().map(|a| a.id.as_str()).collect();
        assert_eq!(one, vec!["ns06_sp01"]);
        field.apply_balm(1600); // crosses 75, locks
        field.tick(STAGGER_SECONDS_ASSUMED);
        field.apply_balm(1600); // crosses 50 -> phase 2 (and 25, chained by the loop above)
        assert_eq!(field.phase, NoblePhase::Two);
        let two: Vec<&str> = field.attacks_for_phase(&all).iter().map(|a| a.id.as_str()).collect();
        assert_eq!(two, vec!["ns06_sp01", "ns06_sp02h"]);
    }

    #[test]
    fn rematches_have_no_field_phase() {
        let def = NobleDef {
            id: "nsi_ex_01_r".into(),
            rematch: true,
            ns_key: "ns02".into(),
            bgm: String::new(),
            species_id: 900,
            species: "kleavor".into(),
            form: 1,
            level: 70,
            nature_raw: None,
            gauge_max: None,
            balm_item: 1678,
            moves: Vec::new(),
            field_attacks: 0,
            field_attacks_h: 0,
        };
        assert_eq!(NobleField::from_def(&def), None);
        let first = NobleDef { gauge_max: Some(1000), rematch: false, ..def };
        assert_eq!(NobleField::from_def(&first).map(|f| f.gauge_max), Some(1000));
    }

    #[test]
    fn real_field_attack_sheet_parses_and_matches_the_boss_rows() {
        let Some(map) = load_field_attacks() else {
            eprintln!("[skip] data/boss_field_attacks.tsv not found");
            return;
        };
        let total: usize = map.values().map(Vec::len).sum();
        assert_eq!(total, 109, "sheet data rows");
        let ns06 = map.get("ns06").expect("ns06 attacks");
        assert_eq!(ns06.len(), 11, "arcanine field_attack_count of boss_battles");
        assert_eq!(
            ns06.iter().filter(|a| a.phase_two).count(),
            6,
            "arcanine field_attack_h_count of boss_battles"
        );
        let ns02 = map.get("ns02").expect("ns02 attacks");
        assert_eq!(ns02.len(), 4, "kleavor field_attack_count of boss_battles");
        assert_eq!(ns02.iter().filter(|a| a.phase_two).count(), 1, "kleavor h variants");
    }
}
