//! Crafting screen in the original look: the game's own `making` layout (with its `inventory_00`
//! recipe grid, `making_info_material_00` material rows and the `common_info_item` panel), bound
//! read-only to the existing crafting state.
//!
//! Keys, the bag copy and the craft call stay in `game::crafting_ui` (which is hidden while this
//! screen shows); this module only reads [`CraftingUi`], [`CraftingCatalog`] and [`CraftingBag`]
//! and fills the game's panes. Text is always the port's own English (item labels come from the
//! decoded item table; the game's text is never read). When the `making` layout is missing the
//! provisional screen stays (single warning from the library), so crafting never disappears.

use bevy::prelude::*;

use super::icons::IconIndex;
use super::menus::{fill_cell, slot_centres, translate_onto};
use super::{OrigScreen, OrigUiConfig, OrigUiSystems, ScreenId};
use crate::battle_engine::items::ItemTable;
use crate::game::bag_screen::BagItemTable;
use crate::game::battle_items::label;
use crate::game::crafting::{can_craft, Bag, Recipe};
use crate::game::crafting_ui::{CraftingBag, CraftingCatalog, CraftingRoot, CraftingUi};

/// Grid cells of the game's inventory layout (the recipe grid reuses it).
pub const CRAFT_CELLS: usize = 48;
/// Recipe cells the `making` layout shows at once (its own 5 x 4 slot circles).
pub const CRAFT_PAGE: usize = 20;
/// Material rows the layout holds (`L_info_material_00`..`04`).
pub const CRAFT_MATERIAL_ROWS: usize = 5;

/// Install the crafting binding: hide the provisional screen while the original one shows.
pub fn install(app: &mut App) {
    app.add_systems(Update, bind_crafting_screen.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<CraftingRoot>(app, ScreenId::Crafting);
}

/// Label of an item id: the decoded effect text, or `Item #id` for ids outside the table.
pub fn item_label(table: Option<&ItemTable>, id: u16) -> String {
    match table.and_then(|t| t.get(id)) {
        Some(item) => label(item),
        None => format!("Item #{id}"),
    }
}

/// Button label: the effect text without the trailing ` #<id>` (the grid cells are narrower than
/// the full label; the info panel and the material rows keep the id). The generic `Item #id` label
/// keeps its id: without it the name says nothing.
pub fn short_label(full: &str) -> &str {
    if let Some((head, tail)) = full.rsplit_once(" #") {
        if head != "Item"
            && !head.is_empty()
            && !tail.is_empty()
            && tail.chars().all(|c| c.is_ascii_digit())
        {
            return head;
        }
    }
    full
}

/// Detail line of the selected recipe for the info panel (port's own words).
pub fn recipe_detail(table: Option<&ItemTable>, recipe: &Recipe, bag: &Bag) -> String {
    format!(
        "Recipe {}: makes {} x{}. Condition: {}. {}",
        recipe.id,
        item_label(table, recipe.product),
        recipe.product_quantity,
        recipe.condition.display(),
        if can_craft(bag, recipe) {
            "Ready to craft."
        } else {
            "Missing materials or the recipe item."
        }
    )
}

/// Crafting (`making` layout): the recipe list on the game's own 5 x 4 slot circles (product icon
/// and label, selected cell highlighted), the selected recipe's material rows (icon, name,
/// required and owned counts, the game's "not enough" state), the heading and the port's detail
/// line in the info panel, and the message strip with the port's messages / key hint.
#[allow(clippy::too_many_arguments)]
pub fn bind_crafting_screen(
    config: Res<OrigUiConfig>,
    ui: Option<Res<CraftingUi>>,
    catalog: Option<Res<CraftingCatalog>>,
    bag: Option<Res<CraftingBag>>,
    table: Option<Res<BagItemTable>>,
    icons: Res<IconIndex>,
    mut screens: Query<&mut OrigScreen>,
) {
    if !config.enabled() {
        return;
    }
    let Some(mut s) = screens.iter_mut().find(|s| s.id == ScreenId::Crafting) else {
        return;
    };
    let open = ui.as_ref().is_some_and(|u| u.open);
    s.shown = open;
    let (Some(ui), Some(catalog), Some(bag)) = (ui.filter(|_| open), catalog, bag) else {
        return;
    };
    let item_table = table.as_deref().and_then(|t| t.0.as_ref());
    let entries = &catalog.0;
    let sel = ui.selected.min(entries.len().saturating_sub(1));
    let page = sel / CRAFT_PAGE;

    // Recipe buttons on the game's own slot circles (`P_deco_base_icon_item_kk`, 5 x 4).
    let slots = slot_centres(&s, "P_deco_base_icon_item_", CRAFT_PAGE);
    if let Some(grid) = s.scene.part_instance("L_inventory_parts_00") {
        for k in 0..CRAFT_CELLS {
            let idx = page * CRAFT_PAGE + k;
            let recipe = (k < CRAFT_PAGE).then(|| entries.get(idx)).flatten();
            let cell = recipe.map(|r| {
                let full = item_label(item_table, r.product);
                (
                    icons.item_or_dummy(r.product),
                    short_label(&full).to_string(),
                )
            });
            let shown = k < CRAFT_PAGE && cell.is_some() && slots.is_some();
            let pos = slots.as_ref().and_then(|c| {
                let node = s.scene.find_in(grid, &format!("L_btn_main_{k:02}"))?;
                translate_onto(&s, node, *c.get(k)?)
            });
            // An item without an exported icon shows its number on the button instead.
            let badge = recipe
                .filter(|_| cell.as_ref().is_some_and(|(i, _)| i.is_none()))
                .map(|r| format!("#{}", r.product));
            fill_cell(&mut s, grid, k, pos, 1.0, shown, cell, idx == sel, badge);
        }
    }
    // The game slides its neighbouring grids in from the sides: not shown at rest.
    for n in 0..s.scene.nodes.len() {
        if s.scene.nodes[n].pane.name.starts_with("N_sub_") {
            s.set_node(n, |o| o.visible = Some(false));
        }
    }

    // Material rows of the selected recipe (the game's five part rows; the plain base rows of the
    // unselected state are hidden instead).
    s.set_visible("N_recipe_unselected_00", false);
    let sel_recipe = entries.get(sel).filter(|_| !entries.is_empty());
    s.set_visible("N_recipe_unselected_01", sel_recipe.is_some());
    s.set_text("T_h_required_material_00", "Required materials");
    for k in 0..CRAFT_MATERIAL_ROWS {
        let part = format!("L_info_material_{k:02}");
        let Some(m) = sel_recipe.and_then(|r| r.materials.get(k)) else {
            s.set_visible(&part, false);
            continue;
        };
        s.set_visible(&part, true);
        let have = bag.0.get(&m.item_id).copied().unwrap_or(0);
        s.set_in(&part, "P_loader_icon_00", |o| {
            o.visible = Some(true);
            o.image = icons.item_or_dummy(m.item_id);
        });
        s.set_in(&part, "T_name_item_00", |o| {
            o.text = Some(item_label(item_table, m.item_id))
        });
        s.set_in(&part, "T_param_required_num_00", |o| {
            o.text = Some(m.quantity.to_string())
        });
        // The port keeps a single bag: the total count (`T_in_storage_all_00`) is what you hold;
        // the game's per-storage variants (bag / box) are hidden.
        s.set_in(&part, "T_in_storage_all_00", |o| {
            o.text = Some(have.to_string())
        });
        for hidden in ["N_storage_bag_00", "N_storage_ibox_00"] {
            s.set_in(&part, hidden, |o| o.visible = Some(false));
        }
        // The game's own "not enough" state on the row.
        if have < m.quantity {
            if let Some(inst) = s.scene.part_instance(&part) {
                s.freeze_end(inst, "item_not_enough", "not_enough");
            }
        } else if let Some(inst) = s.scene.part_instance(&part) {
            s.stop(inst, "not_enough");
        }
    }

    // Info panel of the selected recipe.
    let (heading, detail) = match sel_recipe {
        Some(r) => (
            item_label(item_table, r.product),
            recipe_detail(item_table, r, &bag.0),
        ),
        None => ("No recipes".to_string(), String::new()),
    };
    s.set_in("L_info_item_00", "T_info_heading_00", |o| {
        o.text = Some(heading)
    });
    s.set_in("L_info_item_00", "T_info_text_00", |o| {
        o.text = Some(detail)
    });
    for n in ["T_num_in_storage_00", "T_num_in_storage_01"] {
        s.set_in("L_info_item_00", n, |o| o.text = None);
    }

    // Message strip: the port's feedback, or the key hint when there is nothing to say.
    let hint = format!(
        "{} / {}   Up / Down: recipe   Enter: craft   Esc: close",
        if entries.is_empty() { 0 } else { sel + 1 },
        entries.len()
    );
    let msg = if ui.message.is_empty() {
        hint
    } else {
        ui.message.clone()
    };
    s.set("T_msg_guid_00", |o| o.text = Some(msg));
    s.set("N_msg_guid_00", |o| {
        o.visible = Some(true);
        o.alpha = Some(255.0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::crafting::{MaterialCost, RecipeCondition, RecipeName};

    fn table() -> ItemTable {
        crate::battle_engine::items::ItemTable::load(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap()
    }

    fn recipe() -> Recipe {
        Recipe {
            id: "1".into(),
            name: RecipeName::Unknown,
            product: 1710,
            product_quantity: 1,
            materials: vec![MaterialCost {
                item_id: 1613,
                quantity: 1,
            }],
            condition: RecipeCondition::OwnItem(1640),
        }
    }

    #[test]
    fn item_labels_fall_back_to_the_id() {
        let t = table();
        assert!(item_label(Some(&t), 17).contains("Heal 60 HP #17"));
        assert_eq!(item_label(None, 17), "Item #17");
        assert_eq!(item_label(Some(&t), 60000), "Item #60000");
    }

    #[test]
    fn short_labels_strip_only_the_trailing_id() {
        assert_eq!(short_label("Heal 60 HP #17"), "Heal 60 HP");
        assert_eq!(short_label("Regular Ball #1710"), "Regular Ball");
        assert_eq!(short_label("Item #1613"), "Item #1613", "a generic label keeps its id");
        assert_eq!(short_label("No id here"), "No id here");
        assert_eq!(short_label("#17"), "#17", "a bare id is kept");
        assert_eq!(short_label("Ratio #1.5"), "Ratio #1.5", "not a plain id");
    }

    #[test]
    fn recipe_detail_reports_condition_and_readiness() {
        let t = table();
        let r = recipe();
        let missing = recipe_detail(Some(&t), &r, &Bag::new());
        assert!(missing.contains("Recipe 1:"), "{missing}");
        assert!(missing.contains("Regular Ball #1710"), "{missing}");
        assert!(missing.contains("Requires held item 1640"), "{missing}");
        assert!(missing.contains("Missing materials"), "{missing}");
        let mut bag = Bag::from([(1613, 1), (1640, 1)]);
        let ready = recipe_detail(Some(&t), &r, &bag);
        assert!(ready.contains("Ready to craft."), "{ready}");
        // The recipe item survives a successful craft (model check, not UI).
        crate::game::crafting::craft(&mut bag, &r).unwrap();
        assert_eq!(bag.get(&1640), Some(&1));
    }
}
