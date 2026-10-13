//! Conversions between the save's `PartyMember` / the engine's `Combatant` and the pure
//! progression type. No Bevy, no side effects.
//!
//! The save format has no experience field yet, so a member loaded from it starts at the
//! exact threshold of its level (ASSUMED; extending the save is a follow-up for the wiring
//! phase). `PartyMember` is rebuilt without touching HP/status/nickname.

use super::save_game::{PartyMember, SavedMove, MAX_MOVES};
use crate::battle_engine::progression::{
    exp_for_level, ProgressMember, ProgressionTables,
};
use crate::battle_engine::Combatant;

impl ProgressMember {
    /// Build from a save entry; `exp` falls back to the level threshold.
    pub fn from_party(t: &ProgressionTables, p: &PartyMember, exp: Option<u32>) -> Self {
        let group = t.growth_group(p.species, p.form);
        let floor = exp_for_level(t, group, p.level);
        Self {
            species: p.species,
            form: p.form,
            level: p.level,
            exp: exp.map_or(floor, |e| e.max(floor)),
            moves: p.moves.iter().map(|m| m.move_id).collect(),
            effort: p.effort,
        }
    }

    /// Snapshot a live combatant (form is not tracked by the engine: pass it in).
    pub fn from_combatant(t: &ProgressionTables, c: &Combatant, form: u8, exp: Option<u32>) -> Self {
        let group = t.growth_group(c.species.dex, form);
        let floor = exp_for_level(t, group, c.level);
        Self {
            species: c.species.dex,
            form,
            level: c.level,
            exp: exp.map_or(floor, |e| e.max(floor)),
            moves: c.moves.iter().map(|m| m.data.id).collect(),
            effort: c.effort,
        }
    }

    /// Write species/form/level/moves/effort back into a save entry. Moves keep their
    /// saved PP when the id is unchanged; new moves get `default_pp(move_id)`.
    /// HP grows by `hp_gain` (the caller computes it from the combatant) up to nothing
    /// else; status and nickname are untouched.
    pub fn apply_to_party(&self, p: &mut PartyMember, hp_gain: u32, default_pp: impl Fn(u16) -> u8) {
        p.species = self.species;
        p.form = self.form;
        p.level = self.level;
        p.effort = self.effort;
        if p.hp > 0 {
            p.hp = p.hp.saturating_add(hp_gain);
        }
        let old = std::mem::take(&mut p.moves);
        p.moves = self
            .moves
            .iter()
            .take(MAX_MOVES)
            .map(|id| {
                old.iter()
                    .find(|m| m.move_id == *id)
                    .copied()
                    .unwrap_or(SavedMove { move_id: *id, pp: default_pp(*id) })
            })
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tables() -> ProgressionTables {
        ProgressionTables::load(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
    }

    #[test]
    fn roundtrip_keeps_pp_and_adds_new_move() {
        let t = tables();
        let mut p = PartyMember {
            species: 41,
            form: 0,
            level: 22,
            hp: 30,
            status: None,
            moves: vec![SavedMove { move_id: 33, pp: 7 }],
            effort: [1, 0, 0, 0, 0, 0],
            nickname: Some("zu".into()),
        };
        let mut m = ProgressMember::from_party(&t, &p, None);
        assert_eq!(m.exp, 10_648);
        m.moves.push(45);
        m.species = 42;
        m.apply_to_party(&mut p, 5, |_| 20);
        assert_eq!((p.species, p.hp), (42, 35));
        assert_eq!(p.moves, vec![SavedMove { move_id: 33, pp: 7 }, SavedMove { move_id: 45, pp: 20 }]);
        assert_eq!(p.nickname.as_deref(), Some("zu"));
    }
}
