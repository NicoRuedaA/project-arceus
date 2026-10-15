//! Drop items: the materials a Pokemon leaves when it is defeated or caught, plus the
//! battle grit table (Grit Dust/Gravel/Pebble/Rock). Data comes from
//! `data/poke_drop_items.tsv`, decoded from `bin/pokemon/data/poke_drop_item.bin` and
//! `poke_drop_item_battle.bin` with the public pkNX Arceus FlatBuffers schemas
//! (`.tools/decode_drop_items.py`, layout VERIFIED byte by byte).
//!
//! Confidence of each part (see `reports/data-progress/drop_items.md`):
//!   * Raw layout (hash, item ids, probabilities, brackets): VERIFIED.
//!   * Item identities (from the game's own item-name messages, read locally): VERIFIED
//!     for the ids checked, e.g. 1759 Grit Dust, 1760 Grit Gravel, 155 Oran Berry.
//!   * Field key -> species/form: UNKNOWN (the 64-bit key has not been resolved).
//!   * Battle bracket meaning: PROBABLE (15/30/45/60/75/90/100 read as level thresholds;
//!     the game's own trigger is not decoded).
//!   * Quantity: UNKNOWN (the tables carry no count field; one item per successful roll).

use super::BattleRng;
use crate::battle::Table;

/// One drop slot: an item and its probability in percent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropSlot {
    pub item: u16,
    pub probability: u8,
}

/// One row of the field table: regular and rare slot for one (unresolved) key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDropRow {
    /// The game's 64-bit key; `species`/`form` are UNKNOWN, so lookups stay by hash.
    pub key_hash: u64,
    pub regular: Option<DropSlot>,
    pub rare: Option<DropSlot>,
}

/// One row of the battle table: a bracket with up to four grit slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleDropRow {
    /// Bracket threshold (15, 30, 45, 60, 75, 90, 100); meaning PROBABLE (level).
    pub bracket: u16,
    pub slots: Vec<DropSlot>,
}

/// Both decoded drop tables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DropTable {
    pub field: Vec<FieldDropRow>,
    pub battle: Vec<BattleDropRow>,
}

impl DropTable {
    /// Load `data/poke_drop_items.tsv`.
    pub fn load(data_root: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let table = Table::load(&data_root.as_ref().join("poke_drop_items.tsv"), None)?;
        let mut drops = DropTable::default();
        for row in &table.rows {
            let scope = Table::value(row, "scope:enum")?;
            let slot = Table::value(row, "slot:enum")?.to_owned();
            let item: u16 = Table::value(row, "item_id:u16")?.parse().map_err(|_| "bad item_id")?;
            let probability: u8 =
                Table::value(row, "probability:u8")?.parse().map_err(|_| "bad probability")?;
            let drop = DropSlot { item, probability };
            match scope {
                "field" => {
                    let key_hash = u64::from_str_radix(Table::value(row, "key_hash:string")?, 16)
                        .map_err(|_| "bad key_hash")?;
                    let entry = match drops.field.iter_mut().find(|r| r.key_hash == key_hash) {
                        Some(entry) => entry,
                        None => {
                            drops.field.push(FieldDropRow {
                                key_hash,
                                regular: None,
                                rare: None,
                            });
                            drops.field.last_mut().expect("just pushed")
                        }
                    };
                    match slot.as_str() {
                        "regular" => entry.regular = Some(drop),
                        "rare" => entry.rare = Some(drop),
                        other => return Err(format!("unknown field slot {other}")),
                    }
                }
                "battle" => {
                    let bracket: u16 = Table::value(row, "bracket:u16")?.parse().map_err(|_| "bad bracket")?;
                    let entry = match drops.battle.iter_mut().find(|r| r.bracket == bracket) {
                        Some(entry) => entry,
                        None => {
                            drops.battle.push(BattleDropRow {
                                bracket,
                                slots: Vec::new(),
                            });
                            drops.battle.last_mut().expect("just pushed")
                        }
                    };
                    entry.slots.push(drop);
                }
                other => return Err(format!("unknown drop scope {other}")),
            }
        }
        drops.field.sort_by_key(|r| r.key_hash);
        drops.battle.sort_by_key(|r| r.bracket);
        Ok(drops)
    }

    /// Field table entry by its game key hash (species mapping UNKNOWN).
    pub fn field_by_hash(&self, key_hash: u64) -> Option<&FieldDropRow> {
        self.field.iter().find(|r| r.key_hash == key_hash)
    }

    /// Battle row by exact bracket value.
    pub fn battle_by_bracket(&self, bracket: u16) -> Option<&BattleDropRow> {
        self.battle.iter().find(|r| r.bracket == bracket)
    }

    /// Battle row for a Pokemon level: the greatest bracket at or below it (PROBABLE: the
    /// brackets read as level thresholds; below the first one the first row is used).
    pub fn battle_for_level(&self, level: u8) -> Option<&BattleDropRow> {
        let level = u16::from(level);
        self.battle
            .iter()
            .filter(|r| r.bracket <= level)
            .max_by_key(|r| r.bracket)
            .or_else(|| self.battle.first())
    }

    /// Roll one slot: `probability` percent chance of the item (0 never, 100 always).
    pub fn roll_slot(slot: &DropSlot, rng: &mut impl BattleRng) -> Option<u16> {
        if slot.probability == 0 {
            return None;
        }
        if slot.probability >= 100 || rng.below(100) < u32::from(slot.probability) {
            Some(slot.item)
        } else {
            None
        }
    }

    /// Roll a field row: the regular and the rare slot, in that order. One item per
    /// successful slot (the tables carry no count; UNKNOWN in the game data).
    pub fn roll_field(row: &FieldDropRow, rng: &mut impl BattleRng) -> Vec<u16> {
        let mut out = Vec::new();
        if let Some(slot) = &row.regular {
            out.extend(Self::roll_slot(slot, rng));
        }
        if let Some(slot) = &row.rare {
            out.extend(Self::roll_slot(slot, rng));
        }
        out
    }

    /// Roll a battle row (the grit slots), in order.
    pub fn roll_battle(row: &BattleDropRow, rng: &mut impl BattleRng) -> Vec<u16> {
        row.slots
            .iter()
            .filter_map(|slot| Self::roll_slot(slot, rng))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle_engine::XorShift32;
    use std::path::PathBuf;

    fn table() -> DropTable {
        DropTable::load(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
    }

    #[test]
    fn loads_both_decoded_tables() {
        let t = table();
        // The file has 48 rows; the first carries only its key (no drops), so the sheet
        // lists the 47 rows that actually drop something.
        assert_eq!(t.field.len(), 47);
        assert_eq!(t.battle.len(), 7, "poke_drop_item_battle.bin has 7 brackets");
        let regular = t.field.iter().filter(|r| r.regular.is_some()).count();
        let rare = t.field.iter().filter(|r| r.rare.is_some()).count();
        assert_eq!((regular, rare), (47, 43), "one key-only row, 47 regular and 43 rare slots");
        let battle_slots: usize = t.battle.iter().map(|r| r.slots.len()).sum();
        assert_eq!(battle_slots, 19);
        // Anchors cross-checked against the game's own item-name messages (read locally).
        let lowest = t.battle_by_bracket(15).unwrap();
        assert_eq!(
            lowest.slots,
            vec![DropSlot { item: 1759, probability: 10 }],
            "bracket 15: Grit Dust at 10 %"
        );
        let highest = t.battle_by_bracket(100).unwrap();
        assert_eq!(
            highest.slots,
            vec![
                DropSlot { item: 1759, probability: 100 },
                DropSlot { item: 1760, probability: 70 },
                DropSlot { item: 1761, probability: 30 },
                DropSlot { item: 1762, probability: 10 },
            ],
            "bracket 100: the four grit items"
        );
    }

    #[test]
    fn battle_bracket_follows_the_level_thresholds() {
        let t = table();
        assert_eq!(t.battle_for_level(1).unwrap().bracket, 15, "below the first bracket");
        assert_eq!(t.battle_for_level(15).unwrap().bracket, 15);
        assert_eq!(t.battle_for_level(29).unwrap().bracket, 15);
        assert_eq!(t.battle_for_level(30).unwrap().bracket, 30);
        assert_eq!(t.battle_for_level(100).unwrap().bracket, 100);
        assert_eq!(t.battle_for_level(255).unwrap().bracket, 100);
    }

    #[test]
    fn slot_roll_respects_the_probability_and_is_seeded() {
        let always = DropSlot { item: 7, probability: 100 };
        let never = DropSlot { item: 7, probability: 0 };
        let mut rng = XorShift32(1);
        for _ in 0..20 {
            assert_eq!(DropTable::roll_slot(&never, &mut rng), None);
            assert_eq!(DropTable::roll_slot(&always, &mut rng), Some(7));
        }
        // A 10 % slot over 10000 rolls lands near 1000 (seeded, deterministic).
        let ten = DropSlot { item: 9, probability: 10 };
        let mut rng = XorShift32(0x00d1_0003);
        let hits = (0..10_000).filter(|_| DropTable::roll_slot(&ten, &mut rng).is_some()).count();
        assert!((800..1200).contains(&hits), "roughly 10 %: {hits}");
    }

    #[test]
    fn field_and_battle_rolls_are_deterministic_for_a_seed() {
        let t = table();
        let row = t.field_by_hash(0x4424_cc09_7145_649f).expect("berry row");
        assert_eq!(row.regular, Some(DropSlot { item: 1613, probability: 40 }));
        assert_eq!(row.rare, Some(DropSlot { item: 1688, probability: 15 }));
        let battle = t.battle_for_level(90).unwrap();
        let roll = |seed: u32| {
            let mut rng = XorShift32(seed);
            (
                DropTable::roll_field(row, &mut rng),
                DropTable::roll_battle(battle, &mut rng),
            )
        };
        assert_eq!(roll(7), roll(7), "same seed, same drops");
        let (field, grit) = roll(7);
        assert!(field.iter().all(|i| [1613, 1688].contains(i)));
        assert!(grit.iter().all(|i| [1759, 1760, 1761, 1762].contains(i)));
        assert!(grit.contains(&1759), "bracket 90 always drops Grit Dust");
    }
}
