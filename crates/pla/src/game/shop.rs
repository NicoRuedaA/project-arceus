//! Shop model (pure, no Bevy): real stock + buy/sell rules.
//!
//! Stock comes from `data/shop_stock.tsv`, decoded from
//! `bin/appli/shop/bin/ha_shop_data.bin` with `.tools/decode_shop_data.py`. The file follows
//! the upstream `HaShopTable` FlatBuffer (`Field_00:ulong, Field_01:ulong, ItemID:int`;
//! schema mapping in `data/schemas/shop_data_fields.tsv`). `Field_00` groups the rows into
//! 8 tables and `slot` keeps the file order; what merchant each hash is stays UNKNOWN, and
//! the `Field_01` value is kept raw as `entry_hash`.
//!
//! Prices are taken from `data/item_effects.tsv` (`price`, LIKELY). The sell price (half the
//! buy price, rounded down) is ASSUMED, and selling is allowed in any shop.

use std::fmt;
use std::fs;
use std::path::Path;

use crate::battle_engine::items::ItemTable;
use crate::game::save_wire::PlayerProgress;

/// One decoded shop table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shop {
    /// Raw `Field_00` of its rows; the merchant identity is UNKNOWN.
    pub hash: u64,
    /// Item ids in file order (verified unique within a shop by the decoder and the loader).
    pub stock: Vec<u16>,
}

/// Why a buy or a sell was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopError {
    /// No row for that item in `item_effects.tsv`.
    UnknownItem,
    /// The item is not sold by this shop.
    NotInStock,
    /// Price 0 ("not sold", LIKELY) or an item with no price.
    NotSellable,
    /// Quantity 0.
    BadQuantity,
    /// The player cannot pay the total.
    InsufficientMoney,
    /// The bag does not hold that many.
    NotEnoughItems,
    /// Buying would pass the item's stack ceiling (all-or-nothing).
    StorageLimit,
}

impl fmt::Display for ShopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::UnknownItem => "unknown item",
            Self::NotInStock => "not sold here",
            Self::NotSellable => "this item cannot be sold",
            Self::BadQuantity => "quantity must be at least 1",
            Self::InsufficientMoney => "not enough money",
            Self::NotEnoughItems => "not enough items",
            Self::StorageLimit => "would pass the stack limit",
        };
        f.write_str(s)
    }
}

impl std::error::Error for ShopError {}

impl Shop {
    /// Parse `data/shop_stock.tsv` (sheetized: `#` manifest lines are skipped, `name:type`
    /// headers are reduced to `name`). Rows must be contiguous per `shop_hash` and unique
    /// per `(shop_hash, item_id)`.
    pub fn from_sheet(text: &str) -> Result<Vec<Shop>, String> {
        let mut lines = text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
        let header: Vec<&str> = lines
            .next()
            .ok_or("shop stock: missing header")?
            .split('\t')
            .map(|h| h.split(':').next().unwrap_or(h))
            .collect();
        let col = |name: &str| -> Result<usize, String> {
            header
                .iter()
                .position(|h| *h == name)
                .ok_or_else(|| format!("shop stock: missing column {name}"))
        };
        let c_hash = col("shop_hash")?;
        let c_slot = col("slot")?;
        let c_item = col("item_id")?;
        let mut shops: Vec<Shop> = Vec::new();
        let mut last_slot: Option<u16> = None;
        for (n, line) in lines.enumerate() {
            let v: Vec<&str> = line.split('\t').collect();
            if v.len() != header.len() {
                return Err(format!("shop stock row {}: column count", n + 2));
            }
            let num = |i: usize| -> Result<u64, String> {
                v[i].parse::<u64>()
                    .map_err(|_| format!("shop stock row {}: bad number {:?}", n + 2, v[i]))
            };
            let hash = num(c_hash)?;
            let slot = u16::try_from(num(c_slot)?)
                .map_err(|_| format!("slot out of range: {}", v[c_slot]))?;
            let item_id = u16::try_from(num(c_item)?)
                .map_err(|_| format!("item id out of range: {}", v[c_item]))?;
            match shops.last_mut() {
                Some(shop) if shop.hash == hash => {
                    if shop.stock.contains(&item_id) {
                        return Err(format!(
                            "shop stock row {}: duplicate item {item_id}",
                            n + 2
                        ));
                    }
                    if slot <= last_slot.unwrap_or(0) {
                        return Err(format!("shop stock row {}: slot out of order", n + 2));
                    }
                    shop.stock.push(item_id);
                }
                _ => {
                    // A new shop restarts the slot counter.
                    if slot != 1 {
                        return Err(format!("shop stock row {}: first slot should be 1", n + 2));
                    }
                    shops.push(Shop {
                        hash,
                        stock: vec![item_id],
                    });
                }
            }
            last_slot = Some(slot);
        }
        if shops.is_empty() {
            return Err("shop stock: no rows".into());
        }
        Ok(shops)
    }

    /// Load `<data_root>/shop_stock.tsv`.
    pub fn load(data_root: impl AsRef<Path>) -> Result<Vec<Shop>, String> {
        let path = data_root.as_ref().join("shop_stock.tsv");
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_sheet(&text)
    }

    /// Declared-provisional shop used only when `shop_stock.tsv` is missing: the same starter
    /// selection the battle item menu uses. ASSUMED, never presented as decoded data.
    pub fn provisional(table: &ItemTable) -> Shop {
        Shop {
            hash: 0,
            stock: crate::game::battle_items::starter_selection(table)
                .into_iter()
                .map(|(id, _)| id)
                .collect(),
        }
    }

    pub fn has(&self, item_id: u16) -> bool {
        self.stock.contains(&item_id)
    }

    /// Sell price of one unit: half the buy price, rounded down (ASSUMED).
    pub fn sell_unit_price(item: &crate::battle_engine::items::ItemEffect) -> u32 {
        item.price / 2
    }

    /// Buy `qty` of `item_id`. All-or-nothing: money, stock and the stack ceiling must all
    /// allow it. Returns the total paid.
    pub fn buy(
        &self,
        progress: &mut PlayerProgress,
        table: &ItemTable,
        item_id: u16,
        qty: u16,
    ) -> Result<u32, ShopError> {
        if qty == 0 {
            return Err(ShopError::BadQuantity);
        }
        if !self.has(item_id) {
            return Err(ShopError::NotInStock);
        }
        let item = table.get(item_id).ok_or(ShopError::UnknownItem)?;
        let cost = u64::from(item.price) * u64::from(qty);
        if cost > u64::from(progress.money) {
            return Err(ShopError::InsufficientMoney);
        }
        let have = progress.bag.get(&item_id).copied().unwrap_or(0);
        if u32::from(have) + u32::from(qty) > u32::from(item.max_quantity) {
            return Err(ShopError::StorageLimit);
        }
        progress.money -= cost as u32;
        progress.bag.insert(item_id, have + qty);
        Ok(cost as u32)
    }

    /// Sell `qty` of `item_id` (any priced item, also out of this shop's stock). Returns the
    /// money gained.
    pub fn sell(
        &self,
        progress: &mut PlayerProgress,
        table: &ItemTable,
        item_id: u16,
        qty: u16,
    ) -> Result<u32, ShopError> {
        if qty == 0 {
            return Err(ShopError::BadQuantity);
        }
        let item = table.get(item_id).ok_or(ShopError::UnknownItem)?;
        if item.price == 0 {
            return Err(ShopError::NotSellable);
        }
        let have = progress.bag.get(&item_id).copied().unwrap_or(0);
        if have < qty {
            return Err(ShopError::NotEnoughItems);
        }
        let gained = Self::sell_unit_price(item).saturating_mul(u32::from(qty));
        let left = have - qty;
        if left == 0 {
            progress.bag.remove(&item_id);
        } else {
            progress.bag.insert(item_id, left);
        }
        progress.add_money(gained);
        Ok(gained)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle_engine::items::{HpHeal, ItemEffect};
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    fn fx(id: u16, price: u32, pocket: u8) -> ItemEffect {
        ItemEffect {
            item_id: id,
            price,
            max_quantity: 999,
            effect_kind: 1,
            pocket_code: pocket,
            ball_category: None,
            hp_heal: HpHeal::Fixed(20),
            cures: vec![],
            cures_confusion: false,
            revives: false,
            pp_restore_one: false,
            pp_restore_all: false,
            pp_amount: 0,
            level_up: false,
        }
    }

    fn sheet() -> String {
        let mut s = String::from("# sheet: data/shop_stock\n# version: 1\n");
        s.push_str("shop_hash:u64*\tslot:u16*\tentry_hash:u64\titem_id:u16\n");
        s.push_str("100\t1\t7\t17\n100\t2\t8\t26\n");
        s.push_str("200\t1\t7\t4\n");
        s
    }

    #[test]
    fn parses_shops_from_the_sheet() {
        let shops = Shop::from_sheet(&sheet()).unwrap();
        assert_eq!(shops.len(), 2);
        assert_eq!(shops[0].hash, 100);
        assert_eq!(shops[0].stock, vec![17, 26]);
        assert_eq!(shops[1].stock, vec![4]);
    }

    #[test]
    fn refuses_duplicate_items_and_bad_slots() {
        let dup = sheet().replace("100\t2\t8\t26", "100\t2\t8\t17");
        assert!(Shop::from_sheet(&dup).unwrap_err().contains("duplicate"));
        let unordered = sheet().replace("200\t1", "200\t2");
        assert!(Shop::from_sheet(&unordered)
            .unwrap_err()
            .contains("first slot"));
        let no_header = Shop::from_sheet("# only comments\n").unwrap_err();
        assert!(no_header.contains("header"));
    }

    fn progress_with(bag: &[(u16, u16)], money: u32) -> PlayerProgress {
        PlayerProgress {
            bag: bag.iter().copied().collect(),
            money,
            ..Default::default()
        }
    }

    fn shop() -> Shop {
        Shop {
            hash: 1,
            stock: vec![17, 26],
        }
    }

    fn table() -> ItemTable {
        ItemTable::from_items([fx(17, 200, 6), fx(26, 400, 6), fx(4, 0, 2)])
    }

    #[test]
    fn buy_checks_stock_money_and_stack_limit() {
        let t = table();
        let s = shop();
        let mut p = progress_with(&[], 1000);
        assert_eq!(s.buy(&mut p, &t, 17, 2).unwrap(), 400);
        assert_eq!(p.money, 600);
        assert_eq!(p.bag.get(&17), Some(&2));
        // not in this shop
        assert_eq!(s.buy(&mut p, &t, 4, 1), Err(ShopError::NotInStock));
        // price 0 item would be free, but it is not in stock either
        // not enough money
        assert_eq!(s.buy(&mut p, &t, 26, 2), Err(ShopError::InsufficientMoney));
        // quantity 0
        assert_eq!(s.buy(&mut p, &t, 17, 0), Err(ShopError::BadQuantity));
        // unknown item
        assert_eq!(s.buy(&mut p, &t, 999, 1), Err(ShopError::NotInStock));
    }

    #[test]
    fn buy_stops_at_the_stack_ceiling_without_charging() {
        let mut small = fx(17, 200, 6);
        small.max_quantity = 3;
        let t = ItemTable::from_items([small]);
        let s = shop();
        let mut p = progress_with(&[(17, 2)], 10_000);
        assert_eq!(s.buy(&mut p, &t, 17, 2), Err(ShopError::StorageLimit));
        assert_eq!(p.money, 10_000, "nothing is charged on a refused buy");
        assert_eq!(p.bag.get(&17), Some(&2));
        assert_eq!(s.buy(&mut p, &t, 17, 1).unwrap(), 200);
        assert_eq!(p.bag.get(&17), Some(&3));
    }

    #[test]
    fn buy_overflows_like_the_engine_stack_limit_says() {
        let mut big = fx(17, 100, 6);
        big.max_quantity = u16::MAX;
        let t = ItemTable::from_items([big]);
        let s = shop();
        let mut p = progress_with(&[(17, 999)], 100_000_000);
        assert_eq!(s.buy(&mut p, &t, 17, 999).unwrap(), 99_900);
        assert_eq!(p.bag.get(&17), Some(&1998));
    }

    #[test]
    fn sell_pays_half_and_never_goes_negative() {
        let t = table();
        let s = shop();
        let mut p = progress_with(&[(17, 3)], 100);
        assert_eq!(s.sell(&mut p, &t, 17, 2).unwrap(), 200);
        assert_eq!(p.money, 300);
        assert_eq!(p.bag.get(&17), Some(&1));
        assert_eq!(s.sell(&mut p, &t, 26, 2), Err(ShopError::NotEnoughItems));
        // price 0 -> not sellable even if held
        let mut p0 = progress_with(&[(4, 1)], 0);
        assert_eq!(s.sell(&mut p0, &t, 4, 1), Err(ShopError::NotSellable));
        let mut p0 = progress_with(&[], 0);
        assert_eq!(s.sell(&mut p0, &t, 4, 1), Err(ShopError::NotSellable));
        // the last unit removes the bag entry entirely
        assert_eq!(s.sell(&mut p, &t, 17, 1).unwrap(), 100);
        assert!(!p.bag.contains_key(&17));
    }

    #[test]
    fn real_sheet_decodes_eight_shops_with_143_rows() {
        let shops = Shop::load(root()).unwrap();
        assert_eq!(shops.len(), 8, "decoded shop tables");
        let rows: usize = shops.iter().map(|s| s.stock.len()).sum();
        assert_eq!(rows, 143, "rows in ha_shop_data.bin");
        // Every stocked id must exist in the real item table.
        let table = ItemTable::load(root()).unwrap();
        let missing: Vec<u16> = shops
            .iter()
            .flat_map(|s| s.stock.iter().copied())
            .filter(|id| table.get(*id).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "stock ids missing from item_effects: {missing:?}"
        );
    }

    #[test]
    fn real_shop_zero_is_the_general_store() {
        // Shop 0's decoded stock includes balls and the basic medicines; the exact list is
        // checked against the sheet so the test fails if the decode changes.
        let shops = Shop::load(root()).unwrap();
        let general = &shops[0];
        assert_eq!(general.hash, 0x149c_0319_263e_1eb2);
        for id in [
            1710, 1711, 1712, 1748, 1749, 1713, 1746, 17, 26, 25, 24, 23, 27, 28, 29,
        ] {
            assert!(general.has(id), "shop 0 should sell #{id}");
        }
    }

    #[test]
    fn provisional_shop_is_declared_assumed() {
        let t = ItemTable::from_items([fx(17, 200, 6)]);
        let p = Shop::provisional(&t);
        assert_eq!(p.hash, 0);
        assert!(p.stock.contains(&17));
    }
}
