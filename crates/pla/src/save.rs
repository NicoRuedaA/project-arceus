//! Save system: the event-flag store the scripts mutate.
//!
//! The port seeds the store from the generated flag table (`domain/event_flags`,
//! 450 flags from the update build's `event_flags.tbl`) and exposes it to Lua
//! through the real host bindings (`Global.GetSaveSystem().EventWork()`).

use std::collections::BTreeMap;

use crate::generated;

/// One event flag: the engine's opaque u64 id plus the known name, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventFlag {
    pub id: u64,
    pub set: bool,
}

#[derive(Debug, Default)]
pub struct EventWork {
    /// Flag id -> set state.
    flags: BTreeMap<u64, bool>,
    /// Flag id -> name (from the generated table).
    names: BTreeMap<u64, &'static str>,
    /// Number of SetEventFlag calls observed.
    pub writes: usize,
}

#[derive(Debug, Default)]
pub struct SaveSystem {
    pub event_work: EventWork,
}

impl EventWork {
    /// Seed the known flags from the generated sheet (name + opaque id).
    pub fn seed_from_sheets(&mut self) -> usize {
        let mut n = 0;
        for row in generated::domain_event_flags::ROWS {
            let id = u64::from_str_radix(row.flag_id, 16).unwrap_or(0);
            self.flags.entry(id).or_insert(false);
            self.names.insert(id, row.name);
            n += 1;
        }
        n
    }

    pub fn set(&mut self, id: u64, value: bool) {
        self.flags.insert(id, value);
        self.writes += 1;
    }

    pub fn get(&self, id: u64) -> bool {
        self.flags.get(&id).copied().unwrap_or(false)
    }

    pub fn name(&self, id: u64) -> Option<&'static str> {
        self.names.get(&id).copied()
    }

    pub fn known(&self) -> usize {
        self.flags.len()
    }

    pub fn set_flags(&self) -> Vec<u64> {
        self.flags
            .iter()
            .filter(|(_, &v)| v)
            .map(|(&k, _)| k)
            .collect()
    }
}

impl SaveSystem {
    pub fn seeded() -> Self {
        let mut s = SaveSystem::default();
        s.event_work.seed_from_sheets();
        s
    }
}

/// The engine's `FnvHash64` as the port implements it (FNV-1a, 64-bit).
///
/// Note: the ids stored in the game's tables do **not** match FNV-1a 64 of
/// their names (all common variants tested), so the engine's exact hash variant
/// is still unidentified; the port uses FNV-1a 64 for its own flag identities.
pub fn fnv1a64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn fnv1a64_str(s: &str) -> u64 {
    fnv1a64(s.as_bytes())
}
