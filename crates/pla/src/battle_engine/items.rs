//! Data-driven item use (healing, status cures, revives, PP restore, capture balls) loaded
//! from `data/item_effects.tsv`.
//!
//! Additive like `effects`: a `Battle` without an item table behaves exactly as before and
//! `use_healing_item` / `attempt_capture` keep their signatures. Column confidence is listed in
//! `reports/data-progress/items.md`. The way effects are applied (e.g. what an item does to a
//! fainted Pokemon) is a simulation of the recovered data, not recovered engine behavior.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::{Battle, BattleRng, CaptureModifiers, MajorStatus};
use crate::battle::BattleData;

/// How much HP an item restores (VERIFIED encoding: 255 full, 254 half, 252 a third,
/// otherwise a literal number of HP; checked against the game's item descriptions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HpHeal {
    None,
    Fixed(u32),
    /// `numerator / denominator` of max HP, rounded down, at least 1.
    Fraction(u32, u32),
    Full,
}

impl HpHeal {
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            0 => Self::None,
            255 => Self::Full,
            254 => Self::Fraction(1, 2),
            252 => Self::Fraction(1, 3),
            n => Self::Fixed(n),
        }
    }

    pub fn amount(self, max_hp: u32) -> u32 {
        match self {
            Self::None => 0,
            Self::Fixed(n) => n,
            Self::Fraction(n, d) => (max_hp * n / d).max(1),
            Self::Full => max_hp,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemEffect {
    pub item_id: u16,
    /// LIKELY (buy price; 0 = not sold).
    pub price: u32,
    /// VERIFIED stack ceiling.
    pub max_quantity: u16,
    /// VERIFIED: 1 selects the medicine-style parameter block.
    pub effect_kind: u8,
    /// LIKELY pocket code (raw field; 2 = balls, 6 = medicine, 7 = berries in the decoded
    /// shop stock, 0 = misc/other). Only used to group the bag screen.
    pub pocket_code: u8,
    /// Key-item class: low nibble of the raw `+0x11` byte. `1` is the key-item class (Brief 23):
    /// all 85 records are key items by name (Plates, Old Verses, Red Chain, Gracidea, Charms,
    /// quest items) and none appears in `data/shop_stock.tsv`; 0 is the ordinary class and 2 is
    /// UNKNOWN (45 unnamed ids). See [`ItemEffect::is_key_item`].
    pub storage_class: u8,
    /// Capture table category (`Regular`, `Great`, `Ultra`, `Feather`, ...); `None` when
    /// the ball has no verified capture row.
    pub ball_category: Option<String>,
    pub hp_heal: HpHeal,
    /// Statuses cured (sleep, poison, burn, freeze, paralysis); confusion is not modeled.
    pub cures: Vec<MajorStatus>,
    pub cures_confusion: bool,
    /// LIKELY: set on the three revive items together with a non-zero HP amount.
    pub revives: bool,
    pub pp_restore_one: bool,
    pub pp_restore_all: bool,
    /// PP restored; 127 means all PP (VERIFIED against Ether/Max Ether).
    pub pp_amount: u8,
    pub level_up: bool,
}

impl ItemEffect {
    pub fn is_ball(&self) -> bool {
        self.effect_kind == 0 && self.ball_category.is_some()
    }

    /// Key item: cannot be sold and is not stocked by any shop (PROBABLE, Brief 23). The class is
    /// `storage_class == 1`; the evidence is name-level (85/85 decoded records are key items) and
    /// the empty intersection with the decoded shop stock.
    pub fn is_key_item(&self) -> bool {
        self.storage_class == 1
    }
}

#[derive(Debug, Clone, Default)]
pub struct ItemTable {
    by_id: HashMap<u16, ItemEffect>,
}

impl ItemTable {
    /// Load `<data_root>/item_effects.tsv`.
    pub fn load(data_root: impl AsRef<Path>) -> Result<Self, String> {
        let path = data_root.as_ref().join("item_effects.tsv");
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_tsv(&text)
    }

    pub fn from_tsv(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'));
        let header: Vec<&str> = lines
            .next()
            .ok_or("item effects: missing header")?
            .split('\t')
            .map(|h| h.split(':').next().unwrap_or(h))
            .collect();
        let col = |name: &str| -> Result<usize, String> {
            header
                .iter()
                .position(|h| *h == name)
                .ok_or_else(|| format!("item effects: missing column {name}"))
        };
        let c_id = col("item_id")?;
        let c_price = col("price")?;
        let c_max = col("max_quantity")?;
        let c_kind = col("effect_kind")?;
        let c_pocket = col("pocket_code")?;
        let c_storage = col("storage_class")?;
        let c_ball = col("ball_category")?;
        let c_heal = col("hp_heal")?;
        let c_cures = [
            (col("cure_sleep")?, MajorStatus::Drowsy),
            (col("cure_poison")?, MajorStatus::Poison),
            (col("cure_burn")?, MajorStatus::Burn),
            (col("cure_freeze")?, MajorStatus::Frostbite),
            (col("cure_paralysis")?, MajorStatus::Paralysis),
        ];
        let c_conf = col("cure_confusion")?;
        let c_rev = col("flag_26")?;
        let c_ppone = col("pp_restore_one")?;
        let c_ppall = col("pp_restore_all")?;
        let c_ppamt = col("pp_amount")?;
        let c_lvl = col("level_up")?;
        let mut by_id = HashMap::new();
        for (n, line) in lines.enumerate() {
            let v: Vec<&str> = line.split('\t').collect();
            if v.len() != header.len() {
                return Err(format!("item effects row {}: column count", n + 2));
            }
            let num = |i: usize| -> Result<i64, String> {
                v[i].parse::<i64>()
                    .map_err(|_| format!("item effects row {}: bad number {:?}", n + 2, v[i]))
            };
            let hp_raw = num(c_heal)? as u32;
            let item = ItemEffect {
                item_id: num(c_id)? as u16,
                price: num(c_price)? as u32,
                max_quantity: num(c_max)? as u16,
                effect_kind: num(c_kind)? as u8,
                pocket_code: num(c_pocket)? as u8,
                storage_class: num(c_storage)? as u8,
                ball_category: match v[c_ball] {
                    "none" | "" => None,
                    s => Some(s.to_owned()),
                },
                hp_heal: HpHeal::from_raw(hp_raw),
                cures: {
                    let mut cures: Vec<MajorStatus> = Vec::new();
                    for &(c, s) in &c_cures {
                        if num(c)? != 0 {
                            cures.push(s);
                            // A sleep cure clears both sleep-like states: drowsiness (id 6)
                            // and the real sleep (id 2).
                            if s == MajorStatus::Drowsy {
                                cures.push(MajorStatus::Sleep);
                            }
                        }
                    }
                    cures
                },
                cures_confusion: num(c_conf)? != 0,
                revives: num(c_rev)? != 0 && hp_raw > 0,
                pp_restore_one: num(c_ppone)? != 0,
                pp_restore_all: num(c_ppall)? != 0,
                pp_amount: num(c_ppamt)? as u8,
                level_up: num(c_lvl)? != 0,
            };
            by_id.insert(item.item_id, item);
        }
        Ok(Self { by_id })
    }

    pub fn from_items(items: impl IntoIterator<Item = ItemEffect>) -> Self {
        Self {
            by_id: items.into_iter().map(|i| (i.item_id, i)).collect(),
        }
    }

    pub fn get(&self, item_id: u16) -> Option<&ItemEffect> {
        self.by_id.get(&item_id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// Result of `Battle::use_item`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemUse {
    pub item_id: u16,
    pub combatant: usize,
    pub hp_restored: u32,
    pub status_cured: Option<MajorStatus>,
    pub revived: bool,
    pub pp_restored: u32,
}

/// Minimal stack-limited bag (stack ceiling VERIFIED; pockets and ordering not modeled).
#[derive(Debug, Clone, Default)]
pub struct ItemBag {
    counts: HashMap<u16, u16>,
}

impl ItemBag {
    /// Add up to `max_quantity`; returns how many were stored.
    pub fn add(&mut self, table: &ItemTable, item_id: u16, qty: u16) -> Result<u16, String> {
        let max = table
            .get(item_id)
            .ok_or_else(|| format!("unknown item {item_id}"))?
            .max_quantity;
        let slot = self.counts.entry(item_id).or_insert(0);
        let stored = qty.min(max.saturating_sub(*slot));
        *slot += stored;
        Ok(stored)
    }

    pub fn count(&self, item_id: u16) -> u16 {
        self.counts.get(&item_id).copied().unwrap_or(0)
    }

    pub fn take(&mut self, item_id: u16) -> bool {
        match self.counts.get_mut(&item_id) {
            Some(n) if *n > 0 => {
                *n -= 1;
                true
            }
            _ => false,
        }
    }
}

impl Battle {
    /// Attach the item table. Without it `use_item` and `throw_ball_item` return an error.
    pub fn set_item_table(&mut self, table: ItemTable) {
        self.items = Some(table);
    }

    pub fn item_table(&self) -> Option<&ItemTable> {
        self.items.as_ref()
    }

    /// Use a medicine on the side's active combatant.
    pub fn use_item(&mut self, side: usize, item_id: u16) -> Result<ItemUse, String> {
        let target = *self.active.get(side).ok_or("battle side out of range")?;
        self.use_item_on(side, item_id, target, None)
    }

    /// Use a medicine on any combatant of `side`; `move_slot` selects the move for single-move
    /// PP restoration. An item with no possible effect is refused (nothing is consumed by the
    /// caller), as the game refuses such uses.
    pub fn use_item_on(
        &mut self,
        side: usize,
        item_id: u16,
        target: usize,
        move_slot: Option<usize>,
    ) -> Result<ItemUse, String> {
        let item = self
            .items
            .as_ref()
            .ok_or("no item table attached")?
            .get(item_id)
            .ok_or_else(|| format!("unknown item {item_id}"))?
            .clone();
        if item.effect_kind != 1 {
            return Err(format!("item {item_id} is not a usable medicine"));
        }
        if !self
            .sides
            .get(side)
            .ok_or("battle side out of range")?
            .contains(&target)
        {
            return Err("target is not on this side".into());
        }
        let c = &mut self.combatants[target];
        if c.caught {
            return Err("target is not able to be treated".into());
        }
        let fainted = c.hp == 0;
        let mut out = ItemUse {
            item_id,
            combatant: target,
            hp_restored: 0,
            status_cured: None,
            revived: false,
            pp_restored: 0,
        };
        if fainted {
            if !item.revives {
                return Err("target has fainted".into());
            }
            c.hp = item.hp_heal.amount(c.max_hp).min(c.max_hp);
            out.hp_restored = c.hp;
            out.revived = true;
            c.status = None;
            // A revive clears the real-sleep attempt counter too (field-level access: the
            // combatant borrow above only covers `self.combatants`).
            if target < self.sleep.len() {
                self.sleep[target] = None;
            }
            return Ok(out);
        }
        if item.revives {
            return Err("target has not fainted".into());
        }
        let mut changed = false;
        if item.hp_heal != HpHeal::None && c.hp < c.max_hp {
            let before = c.hp;
            c.hp = (c.hp + item.hp_heal.amount(c.max_hp)).min(c.max_hp);
            out.hp_restored = c.hp - before;
            changed = true;
        }
        if let Some(status) = c.status {
            // Badly poisoned is the poison record with the 0xf01 mark: poison cures clear it
            // too (the game has one cure flag per record).
            if item.cures.contains(&status)
                || (status == MajorStatus::BadlyPoisoned
                    && item.cures.contains(&MajorStatus::Poison))
            {
                c.status = None;
                out.status_cured = Some(status);
                changed = true;
            }
        }
        if item.pp_restore_all || item.pp_restore_one {
            let slots: Vec<usize> = if item.pp_restore_all {
                (0..c.moves.len()).collect()
            } else {
                vec![move_slot.ok_or("this item needs a move slot")?]
            };
            for s in slots {
                let slot = c.moves.get_mut(s).ok_or("move slot out of range")?;
                let room = slot.data.pp.saturating_sub(slot.pp);
                let add = if item.pp_amount >= 127 {
                    room
                } else {
                    room.min(item.pp_amount)
                };
                slot.pp += add;
                out.pp_restored += u32::from(add);
            }
            changed |= out.pp_restored > 0;
        }
        if !changed {
            return Err("the item would have no effect".into());
        }
        if out.status_cured.is_some() && target < self.sleep.len() {
            // A sleep state cleared by an item drops its attempt counter.
            self.sleep[target] = None;
        }
        Ok(out)
    }

    /// Throw a ball item at `target`: the item id picks the capture-table category and the
    /// existing `attempt_capture` does the rest. Balls without a verified category are refused.
    #[allow(clippy::too_many_arguments)]
    pub fn throw_ball_item(
        &mut self,
        data: &BattleData,
        item_id: u16,
        target: usize,
        suitable_range: bool,
        player_rank: u8,
        pokemon_rank: u8,
        catch_rate: u8,
        backstrike: bool,
        modifiers: CaptureModifiers,
        rng: &mut impl BattleRng,
    ) -> Result<bool, String> {
        let category = {
            let item = self
                .items
                .as_ref()
                .ok_or("no item table attached")?
                .get(item_id)
                .ok_or_else(|| format!("unknown item {item_id}"))?;
            if !item.is_ball() {
                return Err(format!(
                    "item {item_id} is not a ball with a capture category"
                ));
            }
            item.ball_category.clone().unwrap()
        };
        self.attempt_capture(
            data,
            target,
            &category,
            suitable_range,
            player_rank,
            pokemon_rank,
            catch_rate,
            backstrike,
            modifiers,
            rng,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle_engine::Combatant;
    use std::path::PathBuf;

    struct Fixed(Vec<u32>);
    impl BattleRng for Fixed {
        fn next_u32(&mut self) -> u32 {
            if self.0.is_empty() {
                1
            } else {
                self.0.remove(0)
            }
        }
    }

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    fn mon(d: &BattleData, species: &str, moves: &[&str], level: u8) -> Combatant {
        let sp = d.species_row(species).unwrap().unwrap();
        let mv = moves
            .iter()
            .map(|n| d.move_row(n).unwrap().unwrap())
            .collect();
        Combatant::from_base(sp, level, mv)
    }

    fn battle() -> (Battle, BattleData) {
        let d = BattleData::load(root()).unwrap();
        let mut b = Battle::new(
            vec![
                mon(&d, "pikachu", &["thunderbolt", "tackle"], 30),
                mon(&d, "bidoof", &["tackle"], 30),
                mon(&d, "starly", &["tackle"], 5),
            ],
            vec![vec![0, 1], vec![2]],
            d.type_chart().clone(),
        )
        .unwrap();
        b.set_item_table(ItemTable::load(root()).unwrap());
        (b, d)
    }

    // Real item ids (VERIFIED against the sheet): 17 Potion (60 HP), 26 Super Potion (100),
    // 24 Max Potion, 18 Antidote, 21 Awakening, 28 Revive, 29 Max Revive, 38 Ether, 40 Elixir,
    // 4 Poke Ball, 3 Great Ball, 2 Ultra Ball, 1713 Feather Ball.
    #[test]
    fn parses_checked_in_item_sheet() {
        let t = ItemTable::load(root()).unwrap();
        assert!(t.len() > 800);
        assert_eq!(t.get(17).unwrap().hp_heal, HpHeal::Fixed(60));
        assert_eq!(t.get(26).unwrap().hp_heal, HpHeal::Fixed(100));
        assert_eq!(t.get(24).unwrap().hp_heal, HpHeal::Full);
        assert_eq!(t.get(18).unwrap().cures, vec![MajorStatus::Poison]);
        assert!(t.get(28).unwrap().revives);
        assert_eq!(t.get(28).unwrap().hp_heal, HpHeal::Fraction(1, 2));
        assert_eq!(t.get(17).unwrap().max_quantity, 999);
        assert_eq!(t.get(3).unwrap().ball_category.as_deref(), Some("Great"));
        assert_eq!(
            t.get(1713).unwrap().ball_category.as_deref(),
            Some("Feather")
        );
    }

    /// Brief 23: the key-item class is `storage_class == 1`; the checked-in sheet has 85 of them
    /// (all key items by name: Plates, Old Verses, Red Chain, Gracidea, quest items), none is a
    /// ball, medicine or berry, and none is stocked by any shop.
    #[test]
    fn key_items_are_the_storage_class_one_rows() {
        let t = ItemTable::load(root()).unwrap();
        let key: Vec<u16> = (0..2000)
            .filter_map(|id| t.get(id as u16).map(|i| (id as u16, i)))
            .filter(|(_, i)| i.is_key_item())
            .map(|(id, _)| id)
            .collect();
        assert_eq!(key.len(), 85, "key-item rows in the decoded sheet");
        for id in [111, 298, 441, 455, 466, 632, 1787, 1828] {
            assert!(key.contains(&id), "item {id} is a key item");
        }
        assert!(key.iter().all(|id| {
            let item = t.get(*id).unwrap();
            !item.is_ball() && item.cures.is_empty() && item.pocket_code != 7
        }));
        // Ordinary consumables keep the class 0 (Potion, Poké Ball, Oran Berry).
        for id in [17, 4, 155] {
            let item = t.get(id).unwrap();
            assert!(!item.is_key_item(), "item {id} is not a key item");
            assert_eq!(item.storage_class, 0);
        }
    }

    #[test]
    fn potion_heals_sixty_capped_at_max_hp() {
        let (mut b, _) = battle();
        let max = b.combatants[0].max_hp;
        b.combatants[0].hp = 1;
        let r = b.use_item(0, 17).unwrap();
        assert_eq!(r.hp_restored, 60.min(max - 1));
        assert_eq!(b.combatants[0].hp, (1 + 60).min(max));
        b.combatants[0].hp = max - 5;
        assert_eq!(b.use_item(0, 17).unwrap().hp_restored, 5);
    }

    #[test]
    fn full_hp_target_refuses_a_potion() {
        let (mut b, _) = battle();
        assert!(b.use_item(0, 17).is_err());
    }

    #[test]
    fn max_potion_restores_everything() {
        let (mut b, _) = battle();
        b.combatants[0].hp = 3;
        let max = b.combatants[0].max_hp;
        assert_eq!(b.use_item(0, 24).unwrap().hp_restored, max - 3);
    }

    #[test]
    fn antidote_cures_only_poison() {
        let (mut b, _) = battle();
        b.combatants[0].status = Some(MajorStatus::Burn);
        assert!(b.use_item(0, 18).is_err());
        b.combatants[0].status = Some(MajorStatus::Poison);
        let r = b.use_item(0, 18).unwrap();
        assert_eq!(r.status_cured, Some(MajorStatus::Poison));
        assert_eq!(b.combatants[0].status, None);
    }

    #[test]
    fn antidote_also_cures_the_badly_poisoned_state() {
        let (mut b, _) = battle();
        b.combatants[0].status = Some(MajorStatus::BadlyPoisoned);
        let r = b.use_item(0, 18).unwrap();
        assert_eq!(r.status_cured, Some(MajorStatus::BadlyPoisoned));
        assert_eq!(b.combatants[0].status, None);
    }

    #[test]
    fn awakening_cures_drowsy_and_real_sleep() {
        let (mut b, _) = battle();
        b.combatants[0].status = Some(MajorStatus::Drowsy);
        assert_eq!(
            b.use_item(0, 21).unwrap().status_cured,
            Some(MajorStatus::Drowsy)
        );
        let (mut b, _) = battle();
        b.combatants[0].status = Some(MajorStatus::Sleep);
        assert_eq!(
            b.use_item(0, 21).unwrap().status_cured,
            Some(MajorStatus::Sleep)
        );
        assert_eq!(b.combatants[0].status, None);
    }

    #[test]
    fn revive_brings_back_half_hp_and_max_revive_all() {
        let (mut b, _) = battle();
        let max = b.combatants[1].max_hp;
        b.combatants[1].hp = 0;
        assert!(
            b.use_item_on(0, 17, 1, None).is_err(),
            "a potion cannot revive"
        );
        let r = b.use_item_on(0, 28, 1, None).unwrap();
        assert!(r.revived);
        assert_eq!(b.combatants[1].hp, max / 2);
        b.combatants[1].hp = 0;
        b.use_item_on(0, 29, 1, None).unwrap();
        assert_eq!(b.combatants[1].hp, max);
        // a living target cannot be revived
        assert!(b.use_item_on(0, 28, 1, None).is_err());
    }

    #[test]
    fn ether_restores_ten_pp_to_one_move_and_elixir_to_all() {
        let (mut b, _) = battle();
        let full0 = b.combatants[0].moves[0].data.pp;
        let full1 = b.combatants[0].moves[1].data.pp;
        b.combatants[0].moves[0].pp = 0;
        b.combatants[0].moves[1].pp = 0;
        assert!(b.use_item(0, 38).is_err(), "Ether needs a move slot");
        let r = b.use_item_on(0, 38, 0, Some(0)).unwrap();
        assert_eq!(r.pp_restored, 10.min(u32::from(full0)));
        assert_eq!(b.combatants[0].moves[1].pp, 0);
        let r = b.use_item(0, 40).unwrap();
        assert_eq!(b.combatants[0].moves[1].pp, 10.min(full1));
        assert!(r.pp_restored > 0);
    }

    #[test]
    fn caught_or_foreign_targets_are_refused() {
        let (mut b, _) = battle();
        b.combatants[1].hp = 0;
        assert!(
            b.use_item_on(0, 28, 2, None).is_err(),
            "target on the other side"
        );
        b.combatants[1].caught = true;
        assert!(b.use_item_on(0, 28, 1, None).is_err());
    }

    #[test]
    fn ball_items_use_the_verified_capture_multiplier() {
        let (mut b, d) = battle();
        // Great Ball (item 3) is the "Great" row (1.5); compare with the explicit API.
        let mods = CaptureModifiers::default();
        let mut direct = b.clone();
        let want = direct
            .attempt_capture(
                &d,
                2,
                "Great",
                false,
                5,
                5,
                255,
                false,
                mods,
                &mut Fixed(vec![65_535, 65_535]),
            )
            .unwrap();
        let got = b
            .throw_ball_item(
                &d,
                3,
                2,
                false,
                5,
                5,
                255,
                false,
                mods,
                &mut Fixed(vec![65_535, 65_535]),
            )
            .unwrap();
        assert_eq!(want, got);
        assert!(got);
        assert!(b.combatants[2].caught);
        // Master Ball (id 1) and mainline specialty balls have no verified row: refused.
        let (mut b, d) = battle();
        assert!(
            b.throw_ball_item(&d, 1, 2, false, 5, 5, 255, false, mods, &mut Fixed(vec![]))
                .is_err()
        );
        // a medicine is not a ball
        assert!(
            b.throw_ball_item(&d, 17, 2, false, 5, 5, 255, false, mods, &mut Fixed(vec![]))
                .is_err()
        );
    }

    #[test]
    fn suitable_range_matters_for_feather_ball_only() {
        let (mut b, d) = battle();
        let mods = CaptureModifiers::default();
        // Feather: base 0.5 vs suitable 1.25; Poke Ball: 0.75 in both.
        let mut near = b.clone();
        let mut far = b.clone();
        // A fixed high roll means failure unless probability is large, so compare the multipliers.
        assert_eq!(d.capture_ball_multiplier("Feather", true).unwrap(), 1.25);
        assert_eq!(d.capture_ball_multiplier("Feather", false).unwrap(), 0.5);
        assert_eq!(d.capture_ball_multiplier("Regular", true).unwrap(), 0.75);
        near.throw_ball_item(
            &d,
            1713,
            2,
            true,
            5,
            5,
            3,
            false,
            mods,
            &mut Fixed(vec![0, 0]),
        )
        .unwrap();
        far.throw_ball_item(
            &d,
            1713,
            2,
            false,
            5,
            5,
            3,
            false,
            mods,
            &mut Fixed(vec![0, 0]),
        )
        .unwrap();
        let _ = &mut b;
    }

    #[test]
    fn bag_respects_stack_ceiling() {
        let t = ItemTable::load(root()).unwrap();
        let mut bag = ItemBag::default();
        assert_eq!(bag.add(&t, 17, 990).unwrap(), 990);
        assert_eq!(bag.add(&t, 17, 20).unwrap(), 9);
        assert_eq!(bag.count(17), 999);
        assert!(bag.take(17));
        assert_eq!(bag.count(17), 998);
        assert!(bag.add(&t, 60000, 1).is_err());
    }

    #[test]
    fn without_item_table_nothing_changes() {
        let (mut b, d) = battle();
        b.items = None;
        assert!(b.use_item(0, 17).is_err());
        assert!(
            b.throw_ball_item(
                &d,
                3,
                2,
                false,
                5,
                5,
                255,
                false,
                CaptureModifiers::default(),
                &mut Fixed(vec![])
            )
            .is_err()
        );
        // old demo healing path untouched
        b.combatants[0].hp = 1;
        b.use_healing_item(
            0,
            &crate::battle_engine::HealingItem {
                name: "Potion".into(),
                healing: 10,
            },
        )
        .unwrap();
        assert_eq!(b.combatants[0].hp, 11);
    }
}
