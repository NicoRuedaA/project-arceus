//! Pure crafting data and atomic inventory operations.
//!
//! Recipe rows deliberately contain ids and quantities only. Names and unlock conditions are
//! data concerns owned by the recipe provider; this module never invents game recipes.

use std::collections::BTreeMap;

/// The largest stack accepted by the port's crafting inventory model.
pub const MAX_STACK: u16 = 999;

pub type ItemId = u16;
pub type Bag = BTreeMap<ItemId, u16>;

/// A display name that is absent from the currently available recipe data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeName {
    Known(String),
    Unknown,
}

impl RecipeName {
    pub fn display(&self) -> &str {
        match self {
            Self::Known(name) if !name.trim().is_empty() => name,
            Self::Known(_) | Self::Unknown => "Unknown recipe",
        }
    }
}

/// Unlock/availability condition from a recipe row.
///
/// `Known` means that a condition label was found, not that its game semantics are implemented.
/// Both non-`None` variants are therefore rejected by crafting until a condition evaluator exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeCondition {
    None,
    Known(String),
    Unknown,
}

impl RecipeCondition {
    pub fn display(&self) -> &str {
        match self {
            Self::None => "No condition",
            Self::Known(condition) if !condition.trim().is_empty() => condition,
            Self::Known(_) | Self::Unknown => "Unknown condition",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialCost {
    pub item_id: ItemId,
    pub quantity: u16,
}

/// Stable representation for one future `data/crafting_recipes.tsv` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub id: String,
    pub name: RecipeName,
    pub product: ItemId,
    pub product_quantity: u16,
    pub materials: Vec<MaterialCost>,
    pub condition: RecipeCondition,
}

/// A narrow seam for loading recipe rows from another data representation.
pub trait RecipeProvider {
    fn recipes(&self) -> Vec<Recipe>;
}

#[derive(Debug, Clone, Default)]
pub struct StaticRecipeProvider(pub Vec<Recipe>);

impl RecipeProvider for StaticRecipeProvider {
    fn recipes(&self) -> Vec<Recipe> {
        self.0.clone()
    }
}

#[derive(Debug, Clone, Default)]
pub struct RecipeCatalog(pub Vec<Recipe>);

impl RecipeCatalog {
    pub fn from_provider(provider: &impl RecipeProvider) -> Self {
        Self(provider.recipes())
    }
}

/// Rows of the tracked `data/crafting_recipes.tsv` sheet (decoded MakeList records).
///
/// Only columns of the sheet are used: `recipe_id`, `result_item_id`, `result_quantity`, the
/// `materials` list (`item:qty,item:qty`) and `condition`. The sheet has no recipe name column, so
/// every name is [`RecipeName::Unknown`]; its `condition` is `UNKNOWN` for every row (MakeList has
/// no unlock-condition field), which [`craft`] refuses by design.
#[derive(Debug, Clone, Default)]
pub struct TsvRecipeProvider(pub Vec<Recipe>);

impl TsvRecipeProvider {
    /// Parse the sheet text. Comment lines start with `#`; the first other line is the header.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'));
        let headers: Vec<&str> = lines
            .next()
            .ok_or_else(|| "crafting_recipes: missing header".to_string())?
            .split('\t')
            .map(|field| field.split_once(':').map_or(field, |(name, _)| name))
            .collect();
        let column = |name: &str| {
            headers
                .iter()
                .position(|header| *header == name)
                .ok_or_else(|| format!("crafting_recipes: missing column {name}"))
        };
        let (id_c, product_c, quantity_c, materials_c, condition_c) = (
            column("recipe_id")?,
            column("result_item_id")?,
            column("result_quantity")?,
            column("materials")?,
            column("condition")?,
        );
        let mut recipes = Vec::new();
        for (index, line) in lines.enumerate() {
            let row = index + 2;
            let values: Vec<&str> = line.split('\t').collect();
            if values.len() != headers.len() {
                return Err(format!(
                    "crafting_recipes row {row}: expected {} fields, got {}",
                    headers.len(),
                    values.len()
                ));
            }
            let number = |c: usize, what: &str| {
                values[c]
                    .parse::<u16>()
                    .map_err(|_| format!("crafting_recipes row {row}: invalid {what}"))
            };
            let mut materials = Vec::new();
            for pair in values[materials_c].split(',').filter(|p| !p.is_empty()) {
                let (item, quantity) = pair
                    .split_once(':')
                    .ok_or_else(|| format!("crafting_recipes row {row}: invalid material {pair}"))?;
                materials.push(MaterialCost {
                    item_id: item
                        .parse()
                        .map_err(|_| format!("crafting_recipes row {row}: invalid material id"))?,
                    quantity: quantity.parse().map_err(|_| {
                        format!("crafting_recipes row {row}: invalid material quantity")
                    })?,
                });
            }
            let condition = match values[condition_c].trim() {
                "" | "NONE" => RecipeCondition::None,
                "UNKNOWN" => RecipeCondition::Unknown,
                known => RecipeCondition::Known(known.to_string()),
            };
            recipes.push(Recipe {
                id: values[id_c].to_string(),
                name: RecipeName::Unknown,
                product: number(product_c, "result_item_id")?,
                product_quantity: number(quantity_c, "result_quantity")?,
                materials,
                condition,
            });
        }
        Ok(Self(recipes))
    }

    /// Load `crafting_recipes.tsv` from a tracked data directory.
    pub fn load(data_dir: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let path = data_dir.as_ref().join("crafting_recipes.tsv");
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        Self::parse(&text)
    }
}

impl RecipeProvider for TsvRecipeProvider {
    fn recipes(&self) -> Vec<Recipe> {
        self.0.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CraftError {
    InvalidRecipe,
    UnknownCondition,
    UnsupportedCondition(String),
    MissingMaterial {
        item_id: ItemId,
        required: u16,
        available: u16,
    },
    OutputStackFull {
        item_id: ItemId,
        current: u16,
        adding: u16,
    },
}

fn plan(bag: &Bag, recipe: &Recipe) -> Result<Bag, CraftError> {
    if recipe.id.trim().is_empty() || recipe.product_quantity == 0 {
        return Err(CraftError::InvalidRecipe);
    }
    match &recipe.condition {
        RecipeCondition::None => {}
        RecipeCondition::Unknown => return Err(CraftError::UnknownCondition),
        RecipeCondition::Known(condition) => {
            return Err(CraftError::UnsupportedCondition(condition.clone()))
        }
    }

    // Aggregate duplicate material ids before mutating anything. This also makes overflow and
    // missing-material checks deterministic for rows supplied by external data loaders.
    let mut required = BTreeMap::<ItemId, u16>::new();
    for material in &recipe.materials {
        if material.quantity == 0 {
            return Err(CraftError::InvalidRecipe);
        }
        let total = required
            .get(&material.item_id)
            .copied()
            .unwrap_or(0)
            .checked_add(material.quantity)
            .ok_or(CraftError::InvalidRecipe)?;
        required.insert(material.item_id, total);
    }

    let mut next = bag.clone();
    for (item_id, quantity) in required {
        let available = next.get(&item_id).copied().unwrap_or(0);
        if available < quantity {
            return Err(CraftError::MissingMaterial {
                item_id,
                required: quantity,
                available,
            });
        }
        let remaining = available - quantity;
        if remaining == 0 {
            next.remove(&item_id);
        } else {
            next.insert(item_id, remaining);
        }
    }

    let current = next.get(&recipe.product).copied().unwrap_or(0);
    let output =
        current
            .checked_add(recipe.product_quantity)
            .ok_or(CraftError::OutputStackFull {
                item_id: recipe.product,
                current,
                adding: recipe.product_quantity,
            })?;
    if output > MAX_STACK {
        return Err(CraftError::OutputStackFull {
            item_id: recipe.product,
            current,
            adding: recipe.product_quantity,
        });
    }
    next.insert(recipe.product, output);
    Ok(next)
}

/// Returns whether the recipe can be applied without changing the bag.
pub fn can_craft(bag: &Bag, recipe: &Recipe) -> bool {
    plan(bag, recipe).is_ok()
}

/// Applies a recipe atomically: the caller's bag is replaced only after every check succeeds.
pub fn craft(bag: &mut Bag, recipe: &Recipe) -> Result<(), CraftError> {
    let next = plan(bag, recipe)?;
    *bag = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(product: ItemId, product_quantity: u16, materials: &[(ItemId, u16)]) -> Recipe {
        Recipe {
            id: "1".into(),
            name: RecipeName::Known("Test item".into()),
            product,
            product_quantity,
            materials: materials
                .iter()
                .map(|&(item_id, quantity)| MaterialCost { item_id, quantity })
                .collect(),
            condition: RecipeCondition::None,
        }
    }

    #[test]
    fn can_craft_requires_every_material() {
        let bag = Bag::from([(10, 2), (11, 1)]);
        assert!(can_craft(&bag, &recipe(20, 1, &[(10, 2), (11, 1)])));
        assert!(!can_craft(&bag, &recipe(20, 1, &[(10, 3), (11, 1)])));
    }

    #[test]
    fn craft_consumes_materials_and_adds_output() {
        let mut bag = Bag::from([(10, 2), (11, 1)]);
        craft(&mut bag, &recipe(20, 1, &[(10, 2), (11, 1)])).unwrap();
        assert_eq!(bag, Bag::from([(20, 1)]));
    }

    #[test]
    fn missing_later_material_does_not_partially_consume_earlier_material() {
        let mut bag = Bag::from([(10, 2), (11, 0)]);
        let before = bag.clone();
        assert!(matches!(
            craft(&mut bag, &recipe(20, 1, &[(10, 2), (11, 1)])),
            Err(CraftError::MissingMaterial { item_id: 11, .. })
        ));
        assert_eq!(bag, before);
    }

    #[test]
    fn output_stack_limit_is_atomic() {
        let mut bag = Bag::from([(10, 1), (20, 999)]);
        let before = bag.clone();
        assert!(matches!(
            craft(&mut bag, &recipe(20, 1, &[(10, 1)])),
            Err(CraftError::OutputStackFull { item_id: 20, .. })
        ));
        assert_eq!(bag, before);
        let mut bag = Bag::from([(10, 1), (20, 998)]);
        craft(&mut bag, &recipe(20, 1, &[(10, 1)])).unwrap();
        assert_eq!(bag.get(&20), Some(&999));
    }

    #[test]
    fn duplicate_material_rows_are_aggregated() {
        let mut bag = Bag::from([(10, 3)]);
        craft(&mut bag, &recipe(20, 1, &[(10, 1), (10, 2)])).unwrap();
        assert_eq!(bag, Bag::from([(20, 1)]));
    }

    #[test]
    fn invalid_and_zero_quantity_rows_are_rejected_without_mutation() {
        let mut bag = Bag::from([(10, 1)]);
        let mut zero = recipe(20, 1, &[(10, 0)]);
        assert_eq!(craft(&mut bag, &zero), Err(CraftError::InvalidRecipe));
        zero.id.clear();
        assert_eq!(craft(&mut bag, &zero), Err(CraftError::InvalidRecipe));
        assert_eq!(bag, Bag::from([(10, 1)]));
    }

    #[test]
    fn unknown_name_is_displayed_but_does_not_change_material_semantics() {
        let mut r = recipe(20, 1, &[(10, 1)]);
        r.name = RecipeName::Unknown;
        assert_eq!(r.name.display(), "Unknown recipe");
        assert!(can_craft(&Bag::from([(10, 1)]), &r));
    }

    #[test]
    fn unknown_or_unimplemented_conditions_are_explicitly_refused() {
        let bag = Bag::from([(10, 1)]);
        let mut unknown = recipe(20, 1, &[(10, 1)]);
        unknown.condition = RecipeCondition::Unknown;
        assert!(!can_craft(&bag, &unknown));
        assert_eq!(unknown.condition.display(), "Unknown condition");
        let mut labelled = recipe(20, 1, &[(10, 1)]);
        labelled.condition = RecipeCondition::Known("story flag".into());
        assert_eq!(
            craft(&mut bag.clone(), &labelled),
            Err(CraftError::UnsupportedCondition("story flag".into()))
        );
    }

    #[test]
    fn tracked_recipe_sheet_loads_every_row_with_unknown_names_and_conditions() {
        let text = include_str!("../../../../data/crafting_recipes.tsv");
        let rows = TsvRecipeProvider::parse(text).unwrap().0;
        assert_eq!(rows.len(), 43);
        assert_eq!(rows[0].id, "1");
        assert_eq!((rows[0].product, rows[0].product_quantity), (1710, 1));
        assert_eq!(
            rows[0].materials,
            vec![
                MaterialCost { item_id: 1613, quantity: 1 },
                MaterialCost { item_id: 1628, quantity: 1 }
            ]
        );
        assert!(rows.iter().all(|r| r.name == RecipeName::Unknown));
        assert!(rows.iter().all(|r| r.condition == RecipeCondition::Unknown));
        // The sheet's unknown unlock condition keeps every recipe refused, even with materials.
        let mut bag = Bag::from([(1613, 9), (1628, 9)]);
        assert_eq!(craft(&mut bag, &rows[0]), Err(CraftError::UnknownCondition));
    }

    #[test]
    fn malformed_recipe_rows_are_rejected() {
        let header = "recipe_id:u8\tresult_item_id:u32\tresult_quantity:u32\tmaterials:string\tcondition:string\n";
        assert!(TsvRecipeProvider::parse(&format!("{header}1\t2\t1\t3:1\tNONE\n")).is_ok());
        assert!(TsvRecipeProvider::parse(&format!("{header}1\t2\t1\t3-1\tNONE\n")).is_err());
        assert!(TsvRecipeProvider::parse(&format!("{header}1\t2\t1\n")).is_err());
        assert!(TsvRecipeProvider::parse("").is_err());
    }

}
