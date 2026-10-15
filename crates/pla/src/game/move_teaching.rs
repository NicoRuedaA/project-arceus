//! Move teaching (Brief 23): the move shop and the move reminder over the `move_tutor` learnsets.
//!
//! PLA has no TM/disc items. `.tools/decode_wazashop.py` audits the item records for a
//! move-teaching link and finds none (the tracked `data/item_teach_moves.tsv` documents the
//! negative result); the game teaches moves through two paths instead:
//!
//! * the **move shop** (`bin/appli/wazaremember/bin/wazashop_table.bin` -> `data/move_shop.tsv`):
//!   61 moves with money prices (PROBABLE);
//! * the **move reminder** over the species' `move_tutor` learnset rows
//!   (`data/pml_learnsets.tsv`, 19,283 rows).
//!
//! `can_learn` accepts every move of the species' learned pool (tutor rows plus level-up rows:
//! the dojo teaches the whole pool; the union is LIKELY, the exact per-path split is not
//! recovered). Shop moves cost their decoded price; every other learnable move is taught free
//! (the reminder fee, if any, is UNKNOWN and the port charges none).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;

use crate::battle_engine::progression::{ProgressMember, MAX_KNOWN_MOVES};

/// The tables the teaching rules read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TutorTables {
    /// `(species, form, move id)` the species can learn (tutor + level-up rows).
    learnable: BTreeSet<(u16, u8, u16)>,
    /// Move id -> shop price in money (the 61 decoded rows).
    shop: BTreeMap<u16, u32>,
}

/// Why a teach was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeachError {
    /// The species/form cannot learn that move.
    NotLearnable,
    /// The member already knows the move.
    AlreadyKnown,
    /// No room: the member knows four moves and the slot is out of range.
    NoSlot,
    /// The member cannot pay the shop price.
    CannotAfford,
}

impl fmt::Display for TeachError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::NotLearnable => "that Pokemon cannot learn this move",
            Self::AlreadyKnown => "that move is already known",
            Self::NoSlot => "no room: forget a move first",
            Self::CannotAfford => "not enough money",
        };
        f.write_str(s)
    }
}

impl TutorTables {
    /// Parse the two tracked sheets: `pml_learnsets` rows and `move_shop` rows.
    pub fn from_tsvs(learnsets: &str, shop: &str) -> Result<Self, String> {
        // --- learnsets: species, form, learn_method, move (column positions by name)
        let mut lines = learnsets
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
        let header: Vec<&str> = lines
            .next()
            .ok_or_else(|| "pml_learnsets: missing header".to_string())?
            .split('\t')
            .map(|f| f.split_once(':').map_or(f, |(n, _)| n))
            .collect();
        let col = |name: &str| {
            header
                .iter()
                .position(|h| *h == name)
                .ok_or_else(|| format!("pml_learnsets: missing column {name}"))
        };
        let (c_species, c_form, c_method, c_move) =
            (col("species")?, col("form")?, col("learn_method")?, col("move")?);
        let mut learnable = BTreeSet::new();
        for (n, line) in lines.enumerate() {
            let v: Vec<&str> = line.split('\t').collect();
            if v.len() != header.len() {
                return Err(format!("pml_learnsets row {}: column count", n + 2));
            }
            if v[c_method] != "move_tutor" && v[c_method] != "level_up" {
                continue;
            }
            let species: u16 = v[c_species].parse().map_err(|_| format!("pml_learnsets row {}: species", n + 2))?;
            let form: u8 = v[c_form].parse().map_err(|_| format!("pml_learnsets row {}: form", n + 2))?;
            let move_id: u16 = v[c_move].parse().map_err(|_| format!("pml_learnsets row {}: move", n + 2))?;
            learnable.insert((species, form, move_id));
        }

        // --- move shop: entry, move, price
        let mut lines = shop
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
        let header: Vec<&str> = lines
            .next()
            .ok_or_else(|| "move_shop: missing header".to_string())?
            .split('\t')
            .map(|f| f.split_once(':').map_or(f, |(n, _)| n))
            .collect();
        let col = |name: &str| {
            header
                .iter()
                .position(|h| *h == name)
                .ok_or_else(|| format!("move_shop: missing column {name}"))
        };
        let (c_move, c_price) = (col("move")?, col("price")?);
        let mut shop_prices = BTreeMap::new();
        for (n, line) in lines.enumerate() {
            let v: Vec<&str> = line.split('\t').collect();
            if v.len() != header.len() {
                return Err(format!("move_shop row {}: column count", n + 2));
            }
            let move_id: u16 = v[c_move].parse().map_err(|_| format!("move_shop row {}: move", n + 2))?;
            let price: u32 = v[c_price].parse().map_err(|_| format!("move_shop row {}: price", n + 2))?;
            if price == 0 || shop_prices.insert(move_id, price).is_some() {
                return Err(format!("move_shop row {}: zero price or duplicate move", n + 2));
            }
        }
        Ok(Self { learnable, shop: shop_prices })
    }

    /// Load `<data_root>/pml_learnsets.tsv` and `<data_root>/move_shop.tsv`.
    pub fn load(data_root: impl AsRef<Path>) -> Result<Self, String> {
        let root = data_root.as_ref();
        let learnsets = fs::read_to_string(root.join("pml_learnsets.tsv"))
            .map_err(|e| format!("{}/pml_learnsets.tsv: {e}", root.display()))?;
        let shop = fs::read_to_string(root.join("move_shop.tsv"))
            .map_err(|e| format!("{}/move_shop.tsv: {e}", root.display()))?;
        Self::from_tsvs(&learnsets, &shop)
    }

    /// The species/form can learn the move (tutor or level-up row).
    pub fn can_learn(&self, species: u16, form: u8, move_id: u16) -> bool {
        self.learnable.contains(&(species, form, move_id))
            || (form != 0 && self.learnable.contains(&(species, 0, move_id)))
    }

    /// Shop price of the move, `None` when it is not one of the 61 sold moves.
    pub fn shop_price(&self, move_id: u16) -> Option<u32> {
        self.shop.get(&move_id).copied()
    }

    /// Money the teaching costs: the shop price, or 0 for the free reminder (UNKNOWN fee).
    pub fn price_to_teach(&self, species: u16, form: u8, move_id: u16) -> Option<u32> {
        self.can_learn(species, form, move_id)
            .then(|| self.shop_price(move_id).unwrap_or(0))
    }

    pub fn learnable_count(&self) -> usize {
        self.learnable.len()
    }
}

/// Teach `move_id` to `member` into `slot` (a slot beyond the known moves appends when there is
/// room). Charges the shop price from `money` only on success and returns the forgotten move, if
/// any. Refusals: the species cannot learn the move, it is already known, there is no slot, or
/// the player cannot pay.
pub fn teach_move(
    tables: &TutorTables,
    member: &mut ProgressMember,
    money: &mut u32,
    slot: usize,
    move_id: u16,
) -> Result<Option<u16>, TeachError> {
    if !tables.can_learn(member.species, member.form, move_id) {
        return Err(TeachError::NotLearnable);
    }
    if member.moves.contains(&move_id) {
        return Err(TeachError::AlreadyKnown);
    }
    let price = tables.price_to_teach(member.species, member.form, move_id).unwrap_or(0);
    if *money < price {
        return Err(TeachError::CannotAfford);
    }
    let forgotten = if slot < member.moves.len() {
        Some(std::mem::replace(&mut member.moves[slot], move_id))
    } else if member.moves.len() < MAX_KNOWN_MOVES {
        member.moves.push(move_id);
        None
    } else {
        return Err(TeachError::NoSlot);
    };
    *money -= price;
    Ok(forgotten)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    fn tables() -> TutorTables {
        TutorTables::load(root()).unwrap()
    }

    fn pikachu(level: u8, moves: &[u16]) -> ProgressMember {
        ProgressMember {
            species: 25,
            form: 0,
            level,
            exp: 0,
            moves: moves.to_vec(),
            effort: [0; 6],
        }
    }

    /// The tracked sheets carry the measured shape: the 61 shop moves with prices, and Pikachu's
    /// thunder-shock (84) is teachable (it is a tutor/level-up move of species 25).
    #[test]
    fn the_sheets_carry_the_measured_shape() {
        let t = tables();
        assert_eq!(t.shop.len(), 61, "decoded waza shop rows");
        assert!(t.shop_price(206).unwrap() > 0);
        assert_eq!(t.shop_price(9999), None);
        assert_eq!(t.learnable_count(), 19_654, "distinct tutor+level-up (species, form, move) rows");
        assert!(t.can_learn(25, 0, 84), "Pikachu can learn thunder-shock");
        assert!(!t.can_learn(25, 0, 52), "Pikachu cannot learn ember (Vulpix's move)");
        assert_eq!(t.price_to_teach(25, 0, 84), Some(t.shop_price(84).unwrap_or(0)));
    }

    #[test]
    fn teach_refuses_wrong_species_duplicates_and_no_slot() {
        let t = tables();
        // Real Pikachu moves: 84 thunder-shock, 85 thunderbolt, 86 thunder-wave, 87 thunder.
        let mut p = pikachu(10, &[84, 85, 86, 87]);
        let mut money = 10_000;
        assert_eq!(teach_move(&t, &mut p, &mut money, 0, 52), Err(TeachError::NotLearnable));
        assert_eq!(teach_move(&t, &mut p, &mut money, 0, 84), Err(TeachError::AlreadyKnown));
        // Four moves known and a slot outside them: no room (98 quick-attack is learnable).
        assert_eq!(teach_move(&t, &mut p, &mut money, 4, 98), Err(TeachError::NoSlot));
        // A replaced move comes back and the shop price (231 iron-tail) is charged once.
        let before = money;
        let price = t.price_to_teach(25, 0, 231).expect("iron-tail is sold and teachable");
        assert_eq!(price, 2000);
        let forgotten = teach_move(&t, &mut p, &mut money, 2, 231).unwrap();
        assert_eq!(forgotten, Some(86));
        assert_eq!(p.moves, vec![84, 85, 231, 87]);
        assert_eq!(money, before - price);
    }

    #[test]
    fn teach_appends_into_a_free_slot_and_checks_money() {
        let t = tables();
        // Thunderbolt (85) is a Pikachu move with a decoded shop price (3000); pay it.
        let mut p = pikachu(10, &[84]);
        let price = t.price_to_teach(25, 0, 85).expect("thunderbolt is teachable");
        assert_eq!(price, 3000);
        let mut money = price;
        assert_eq!(teach_move(&t, &mut p, &mut money, 9, 85).unwrap(), None);
        assert_eq!(p.moves, vec![84, 85], "a free slot appends");
        assert_eq!(money, 0, "the price is paid");
        // Without money the same teach is refused and nothing changes.
        let mut p2 = pikachu(10, &[84]);
        let mut broke = 0;
        assert_eq!(teach_move(&t, &mut p2, &mut broke, 9, 85), Err(TeachError::CannotAfford));
        assert_eq!(p2.moves, vec![84], "a refused teach changes nothing");
        // The reminder path is free (84 is teachable but not sold).
        let mut p3 = pikachu(10, &[]);
        let mut none = 0;
        assert_eq!(t.price_to_teach(25, 0, 84), Some(0));
        assert_eq!(teach_move(&t, &mut p3, &mut none, 0, 84).unwrap(), None);
        assert_eq!(p3.moves, vec![84]);
    }
}
