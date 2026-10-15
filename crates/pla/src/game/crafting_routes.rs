//! Recipe acquisition routes: how each recipe item reaches the bag.
//!
//! Decoded from the game's own scripts and tables (`.tools/decode_recipe_routes.py`):
//! Galaxy Team rank rewards (the rank-up ceremony arrays in `rankupevent.blua`,
//! rank 1 from the rank-01 ceremony in `ev_main_015200`), the Jubilife craftworks
//! shop stock (`ha_shop_data.bin`), story/event grants (`ItemCommand.ItemGetMessage`)
//! and seven still-unresolved items (story/requests, out of current scope).
//!
//! The sheet lists one row per (recipe item, route); prices come from the item
//! table. Nothing here changes the save format: persisting a `galaxy_rank` value
//! (and wiring a rank-up flow) is pending and would need a save version bump.

use std::collections::BTreeMap;

use super::crafting::Bag;

/// Where a recipe item comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeRoute {
    /// Galaxy Team rank reward (rank value 1..=10 as stored by the game).
    Rank(u8),
    /// Sold by a shop table; `price` is the item table buy price.
    Shop { hash: u64, price: u32 },
    /// Granted by a story event or request (out of current gameplay scope).
    Story,
    /// No route found yet.
    Unknown,
}

/// One decoded acquisition row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeRouteRow {
    pub recipe_item_id: u16,
    pub recipe_id: u8,
    pub product: u32,
    pub route: RecipeRoute,
    pub source: String,
    pub status: String,
}

#[derive(Debug, Clone, Default)]
pub struct RecipeRoutes(pub Vec<RecipeRouteRow>);

impl RecipeRoutes {
    /// Parse the sheet text. Comment lines start with `#`; the first other line is the header.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'));
        let headers: Vec<&str> = lines
            .next()
            .ok_or_else(|| "crafting_recipe_routes: missing header".to_string())?
            .split('\t')
            .map(|field| field.split_once(':').map_or(field, |(name, _)| name))
            .collect();
        let column = |name: &str| {
            headers
                .iter()
                .position(|header| *header == name)
                .ok_or_else(|| format!("crafting_recipe_routes: missing column {name}"))
        };
        let (item_c, recipe_c, product_c, route_c, rank_c, shop_c, price_c, source_c, status_c) = (
            column("recipe_item_id")?,
            column("recipe_id")?,
            column("product_item_id")?,
            column("route")?,
            column("rank")?,
            column("shop_hash")?,
            column("price")?,
            column("source")?,
            column("status")?,
        );
        let mut rows = Vec::new();
        for (index, line) in lines.enumerate() {
            let row = index + 2;
            let values: Vec<&str> = line.split('\t').collect();
            if values.len() != headers.len() {
                return Err(format!(
                    "crafting_recipe_routes row {row}: expected {} fields, got {}",
                    headers.len(),
                    values.len()
                ));
            }
            let number = |c: usize, what: &str| -> Result<u64, String> {
                let raw = values[c].trim();
                if raw.is_empty() {
                    return Ok(0);
                }
                raw.parse::<u64>()
                    .map_err(|_| format!("crafting_recipe_routes row {row}: invalid {what}"))
            };
            let route = match values[route_c].trim() {
                "rank" => RecipeRoute::Rank(number(rank_c, "rank")? as u8),
                "shop" => RecipeRoute::Shop {
                    hash: number(shop_c, "shop_hash")?,
                    price: number(price_c, "price")? as u32,
                },
                "story" => RecipeRoute::Story,
                "unknown" => RecipeRoute::Unknown,
                other => {
                    return Err(format!(
                        "crafting_recipe_routes row {row}: unknown route {other:?}"
                    ));
                }
            };
            rows.push(RecipeRouteRow {
                recipe_item_id: number(item_c, "recipe_item_id")? as u16,
                recipe_id: number(recipe_c, "recipe_id")? as u8,
                product: number(product_c, "product_item_id")? as u32,
                route,
                source: values[source_c].trim().to_string(),
                status: values[status_c].trim().to_string(),
            });
        }
        Ok(Self(rows))
    }

    /// Load `crafting_recipe_routes.tsv` from a tracked data directory.
    pub fn load(data_dir: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let path = data_dir.as_ref().join("crafting_recipe_routes.tsv");
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        Self::parse(&text)
    }

    /// Recipe items awarded by a Galaxy-rank promotion, ascending and deduplicated.
    pub fn rank_recipe_items(&self, rank: u8) -> Vec<u16> {
        let mut items: Vec<u16> = self
            .0
            .iter()
            .filter(|row| row.route == RecipeRoute::Rank(rank))
            .map(|row| row.recipe_item_id)
            .collect();
        items.sort_unstable();
        items.dedup();
        items
    }

    /// Add the recipe items awarded at `rank` to the bag (absent items only) and
    /// return the granted ids. A recipe item stacks to one, so an already-held
    /// item is never duplicated.
    pub fn grant_rank_recipe_items(&self, bag: &mut Bag, rank: u8) -> Vec<u16> {
        let granted: Vec<u16> = self
            .rank_recipe_items(rank)
            .into_iter()
            .filter(|item| bag.get(item).copied().unwrap_or(0) == 0)
            .collect();
        for item in &granted {
            bag.insert(*item, 1);
        }
        granted
    }

    /// Distinct recipe items listed with a shop route, ascending.
    pub fn shop_recipe_items(&self) -> Vec<u16> {
        let mut items: Vec<u16> = self
            .0
            .iter()
            .filter(|row| matches!(row.route, RecipeRoute::Shop { .. }))
            .map(|row| row.recipe_item_id)
            .collect();
        items.sort_unstable();
        items.dedup();
        items
    }

    /// Total distinct recipe items touched by any known route.
    pub fn known_recipe_items(&self) -> Vec<u16> {
        let mut items: Vec<u16> = self
            .0
            .iter()
            .filter(|row| row.route != RecipeRoute::Unknown)
            .map(|row| row.recipe_item_id)
            .collect();
        items.sort_unstable();
        items.dedup();
        items
    }
}

/// Shop hash of the Jubilife craftworks recipe stock (from `data/shop_stock.tsv`).
pub const CRAFTWORKS_SHOP_HASH: u64 = 70894275551592854;

/// A recipe item granted at each rank, in rank order, as (rank, item ids).
pub fn rank_reward_table(routes: &RecipeRoutes) -> BTreeMap<u8, Vec<u16>> {
    let mut table = BTreeMap::new();
    for rank in 1..=10u8 {
        let items = routes.rank_recipe_items(rank);
        if !items.is_empty() {
            table.insert(rank, items);
        }
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::crafting::{can_craft, craft, TsvRecipeProvider};

    fn tracked() -> RecipeRoutes {
        let text = include_str!("../../../../data/crafting_recipe_routes.tsv");
        RecipeRoutes::parse(text).unwrap()
    }

    #[test]
    fn tracked_sheet_covers_every_recipe_with_the_expected_routes() {
        let routes = tracked();
        let expected = [
            ("rank", 13usize),
            ("shop", 18),
            ("story", 5),
            ("unknown", 7),
        ];
        for (name, count) in expected {
            let actual = routes
                .0
                .iter()
                .filter(|row| match (&row.route, name) {
                    (RecipeRoute::Rank(_), "rank")
                    | (RecipeRoute::Shop { .. }, "shop")
                    | (RecipeRoute::Story, "story")
                    | (RecipeRoute::Unknown, "unknown") => true,
                    _ => false,
                })
                .count();
            assert_eq!(actual, count, "route {name}");
        }
        // Every route row references a recipe item that the recipe sheet carries.
        let recipes = TsvRecipeProvider::parse(include_str!(
            "../../../../data/crafting_recipes.tsv"
        ))
        .unwrap()
        .0;
        let known: std::collections::BTreeSet<u16> =
            recipes.iter().filter_map(|r| match r.condition {
                crate::game::crafting::RecipeCondition::OwnItem(id) => Some(id),
                _ => None,
            }).collect();
        for row in &routes.0 {
            assert!(
                known.contains(&row.recipe_item_id),
                "route row {} not a recipe item",
                row.recipe_item_id
            );
            assert_eq!(row.status == "unknown", row.route == RecipeRoute::Unknown);
        }
    }

    #[test]
    fn rank_tables_match_the_game_scripts_and_grant_once() {
        let routes = tracked();
        assert_eq!(routes.rank_recipe_items(1), vec![1667, 1753]); // Revive, Heavy Ball
        assert_eq!(routes.rank_recipe_items(2), vec![1643, 1657]); // Feather Ball, Super Potion
        assert_eq!(routes.rank_recipe_items(3), vec![1641]); // Great Ball
        assert_eq!(routes.rank_recipe_items(4), vec![1658, 1783]); // Hyper Potion, Leaden Ball
        assert_eq!(routes.rank_recipe_items(5), vec![1751]); // Wing Ball
        assert_eq!(routes.rank_recipe_items(6), vec![1642, 1659]); // Ultra Ball, Max Potion
        assert_eq!(routes.rank_recipe_items(7), vec![1784]); // Gigaton Ball
        assert_eq!(routes.rank_recipe_items(8), vec![1660, 1752]); // Full Restore, Jet Ball
        assert!(routes.rank_recipe_items(9).is_empty()); // rank 9/10 award no recipes

        let table = rank_reward_table(&routes);
        assert_eq!(table.len(), 8);
        assert_eq!(table.values().map(Vec::len).sum::<usize>(), 13);

        let mut bag = Bag::default();
        assert_eq!(routes.grant_rank_recipe_items(&mut bag, 2), vec![1643, 1657]);
        assert_eq!(bag.get(&1643), Some(&1));
        // Re-granting never duplicates the one-copy recipe item.
        assert!(routes.grant_rank_recipe_items(&mut bag, 2).is_empty());
        assert_eq!(bag.get(&1643), Some(&1));
    }

    #[test]
    fn rank_rewards_unlock_the_matching_craftable_recipe() {
        let routes = tracked();
        let recipes = TsvRecipeProvider::parse(include_str!(
            "../../../../data/crafting_recipes.tsv"
        ))
        .unwrap()
        .0;
        // Recipe 7 turns Apricorn + Sky Tumblestone into a Feather Ball (1713).
        let feather = recipes
            .iter()
            .find(|r| r.product == 1713)
            .expect("Feather Ball recipe");
        let mut bag = Bag::from([(1613, 2), (1682, 2)]);
        assert!(!can_craft(&bag, feather), "locked without the recipe item");
        assert_eq!(routes.grant_rank_recipe_items(&mut bag, 2), vec![1643, 1657]);
        assert!(can_craft(&bag, feather));
        craft(&mut bag, feather).unwrap();
        assert_eq!(bag.get(&1713), Some(&1));
        assert_eq!(bag.get(&1643), Some(&1), "recipe item survives the craft");
    }

    #[test]
    fn shop_routes_list_the_craftworks_stock_only() {
        let routes = tracked();
        let shop_items = routes.shop_recipe_items();
        assert_eq!(shop_items.len(), 18);
        assert!(routes.0.iter().filter(|r| matches!(r.route, RecipeRoute::Shop { .. })).all(
            |r| matches!(r.route, RecipeRoute::Shop { hash, .. } if hash == CRAFTWORKS_SHOP_HASH)
        ));
        let priced: Vec<(u16, u32)> = routes
            .0
            .iter()
            .filter_map(|row| match row.route {
                RecipeRoute::Shop { price, .. } => Some((row.recipe_item_id, price)),
                _ => None,
            })
            .collect();
        assert!(priced.iter().all(|(_, price)| *price > 0));
        // Spot anchors straight from the item table.
        assert!(priced.contains(&(1673, 1500))); // Aux Power recipe
        assert!(priced.contains(&(1649, 10000))); // Star Piece recipe
    }

    #[test]
    fn malformed_route_rows_are_rejected() {
        let header = "recipe_item_id:u16\trecipe_id:u8\tproduct_item_id:u32\troute:string\trank:u8?\tshop_hash:u64?\tprice:u32?\tsource:string\tstatus:string\n";
        assert!(RecipeRoutes::parse(&format!("{header}1\t1\t2\trank\t1\t\t\ts\tverified\n")).is_ok());
        assert!(RecipeRoutes::parse(&format!("{header}1\t1\t2\tteleport\t\t\t\ts\tx\n")).is_err());
        assert!(RecipeRoutes::parse("").is_err());
    }
}
