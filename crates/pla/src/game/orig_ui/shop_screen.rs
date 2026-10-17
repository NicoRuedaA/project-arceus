//! Shop screen in the original look: the game's own `shop_buy_00` layout (its item grid on the
//! layout's slot circles, `cmn_shop_money_00` money plate and `common_info_item` panel), bound
//! read-only to the existing bag/shop state.
//!
//! Keys, the buy/sell calls and `[ ]` quantity stay in `game::bag_screen` (the provisional screen
//! is hidden while this one shows); this module only reads [`BagUi`], [`ShopCatalog`],
//! [`BagItemTable`] and [`PlayerProgress`] and fills the game's panes. The shop's raw id/hash is
//! debug data: it is never drawn here (only the provisional `--ui debug` view shows it). When the
//! `shop` layout is missing the provisional screen stays (single warning from the library), so
//! the shop never disappears.

use bevy::prelude::*;

use super::icons::IconIndex;
use super::menus::{fill_cell, item_label, short_label, slot_centres, translate_onto};
use super::{free_text, OrigScreen, OrigUiConfig, OrigUiSystems, ScreenId};
use crate::battle_engine::items::ItemEffect;
use crate::game::bag_screen::{
    screen_entries, BagItemTable, BagMode, BagRoot, BagUi, ShopCatalog, ShopTab,
};
use crate::game::save_wire::PlayerProgress;
use crate::game::shop::Shop;

/// Grid cells of the game's inventory layout (the shop reuses it for its buttons).
pub const SHOP_CELLS: usize = 48;
/// Item cells the `shop_buy_00` layout shows at once (its own 7 x 3 slot circles).
pub const SHOP_PAGE: usize = 21;

/// Install the shop binding: the provisional bag/shop screen is hidden while this one shows.
pub fn install(app: &mut App) {
    app.add_systems(Update, bind_shop_screen.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<BagRoot>(app, ScreenId::Shop);
}

/// Detail line of the selected item for the info panel: what it is, how many you hold and what
/// the trade would cost / pay (the port's own words; the sell price is the model's half, ASSUMED).
pub fn shop_detail(label: &str, item: &ItemEffect, owned: u16, qty: u16, tab: ShopTab) -> String {
    let trade = match tab {
        ShopTab::Buy => {
            if item.price == 0 {
                "Not sold in shops.".to_string()
            } else {
                format!(
                    "Price {} (sells for {}). Buying {qty} costs {}.",
                    item.price,
                    Shop::sell_unit_price(item),
                    u64::from(item.price) * u64::from(qty)
                )
            }
        }
        ShopTab::Sell => format!(
            "Sells for {} each; selling {qty} pays {}.",
            Shop::sell_unit_price(item),
            u64::from(Shop::sell_unit_price(item)) * u64::from(qty)
        ),
    };
    format!(
        "{label}. You have {owned} (up to {}). {trade}",
        item.max_quantity
    )
}

/// Shop (`shop_buy_00` layout): the current tab's items on the game's own 7 x 3 slot circles
/// (item icon and label, selected cell highlighted, pages follow the selection), the Buy / Sell
/// heading, the money plate, the selected item's detail with price, owned count and the trade
/// total, and the port's message / key-hint line.
#[allow(clippy::too_many_arguments)]
pub fn bind_shop_screen(
    config: Res<OrigUiConfig>,
    ui: Option<Res<BagUi>>,
    table: Option<Res<BagItemTable>>,
    shops: Option<Res<ShopCatalog>>,
    progress: Option<Res<PlayerProgress>>,
    icons: Res<IconIndex>,
    mut screens: Query<&mut OrigScreen>,
) {
    if !config.enabled() {
        return;
    }
    let Some(mut s) = screens.iter_mut().find(|s| s.id == ScreenId::Shop) else {
        return;
    };
    let open = ui
        .as_ref()
        .is_some_and(|u| u.open && u.mode == BagMode::Shop);
    s.shown = open;
    let (Some(ui), Some(table), Some(shops)) = (ui.filter(|_| open), table, shops) else {
        return;
    };
    let Some(table) = table.0.as_ref() else {
        return;
    };
    let empty = std::collections::BTreeMap::new();
    let bag = progress.as_ref().map(|p| &p.bag).unwrap_or(&empty);
    let entries = screen_entries(&ui, table, bag, &shops.0);
    let base = (ui.selected / SHOP_PAGE) * SHOP_PAGE;

    // Item buttons on the game's own slot circles (`P_deco_base_item_kk`, 7 x 3).
    let slots = slot_centres(&s, "P_deco_base_item_", SHOP_PAGE);
    if let Some(grid) = s.scene.part_instance("L_inventory_parts_00") {
        for k in 0..SHOP_CELLS {
            let idx = base + k;
            let entry = (k < SHOP_PAGE).then(|| entries.get(idx)).flatten();
            let cell = entry.map(|&(id, count)| {
                let label = item_label(Some(table), id);
                (
                    icons.item_or_dummy(id),
                    format!("{}  x{count}", short_label(&label)),
                )
            });
            let shown = k < SHOP_PAGE && cell.is_some() && slots.is_some();
            let pos = slots.as_ref().and_then(|c| {
                let node = s.scene.find_in(grid, &format!("L_btn_main_{k:02}"))?;
                translate_onto(&s, node, *c.get(k)?)
            });
            // An item without an exported icon shows its number on the button instead.
            let badge = entry
                .filter(|_| cell.as_ref().is_some_and(|(i, _)| i.is_none()))
                .map(|(id, _)| format!("#{id}"));
            fill_cell(&mut s, grid, k, pos, 1.0, shown, cell, idx == ui.selected, badge);
        }
    }
    // The game slides its neighbouring grids in from the sides: not shown at rest.
    for n in 0..s.scene.nodes.len() {
        if s.scene.nodes[n].pane.name.starts_with("N_sub_") {
            s.set_node(n, |o| o.visible = Some(false));
        }
    }

    // Buy / Sell heading and the money plate.
    s.set_text(
        "T_heading_pt_00",
        match ui.tab {
            ShopTab::Buy => "Buy",
            ShopTab::Sell => "Sell",
        },
    );
    let money = progress.as_ref().map_or(0, |p| p.money);
    s.set_in("L_money_00", "T_param_money_00", |o| {
        o.text = Some(money.to_string())
    });
    for n in ["T_param_add_00", "T_param_sub_00"] {
        s.set_in("L_money_00", n, |o| o.text = None);
    }

    // Info panel of the selected item.
    let (heading, detail, owned) = match entries
        .get(ui.selected)
        .and_then(|(id, count)| table.get(*id).map(|item| (item, *count)))
    {
        Some((item, count)) => {
            let label = item_label(Some(table), item.item_id);
            let detail = shop_detail(&label, item, count, ui.quantity, ui.tab);
            (label, detail, count)
        }
        None => ("Nothing".to_string(), String::new(), 0),
    };
    s.set_in("L_info_item_00", "T_info_heading_00", |o| {
        o.text = Some(heading)
    });
    s.set_in("L_info_item_00", "T_info_text_00", |o| {
        o.text = Some(detail)
    });
    // The port keeps a single bag: the first storage counter is "how many you hold"; the
    // game's second storage variant (bag / box) is hidden.
    s.set_in("L_info_item_00", "T_num_in_storage_00", |o| {
        o.text = Some(owned.to_string())
    });
    s.set_in("L_info_item_00", "N_storage_01", |o| {
        o.visible = Some(false)
    });

    // The port's own message or the key hint (same words as the provisional screen).
    let msg = if ui.message.is_empty() {
        "I/Esc close - W/S select - A/D Buy/Sell - [ ] quantity - Enter: trade".to_string()
    } else {
        ui.message.clone()
    };
    s.set_free_texts(vec![free_text(
        60.0,
        1020.0,
        1800.0,
        44.0,
        26.0,
        [0.92, 0.9, 0.82, 1.0],
        super::scene::TextAlign::Left,
        msg,
    )]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle_engine::items::HpHeal;

    fn fx(id: u16, price: u32) -> ItemEffect {
        ItemEffect {
            item_id: id,
            price,
            max_quantity: 999,
            effect_kind: 1,
            pocket_code: 6,
            storage_class: 0,
            ball_category: None,
            hp_heal: HpHeal::Fixed(60),
            cures: vec![],
            cures_confusion: false,
            revives: false,
            pp_restore_one: false,
            pp_restore_all: false,
            pp_amount: 0,
            level_up: false,
        }
    }

    #[test]
    fn detail_names_the_trade_total_per_tab() {
        let item = fx(17, 200);
        let buy = shop_detail("Heal 60 HP #17", &item, 3, 2, ShopTab::Buy);
        assert!(buy.contains("You have 3"), "{buy}");
        assert!(buy.contains("sells for 100"), "{buy}");
        assert!(buy.contains("costs 400"), "{buy}");
        let sell = shop_detail("Heal 60 HP #17", &item, 3, 2, ShopTab::Sell);
        assert!(sell.contains("selling 2 pays 200"), "{sell}");
        let free = fx(4, 0);
        let buy_free = shop_detail("Regular Ball #4", &free, 0, 1, ShopTab::Buy);
        assert!(buy_free.contains("Not sold in shops."), "{buy_free}");
    }
}
