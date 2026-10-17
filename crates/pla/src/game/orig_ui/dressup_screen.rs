//! Customize screen in the original look: the game's own `dressup_00` layout (its category tab
//! strip, seven list rows with the piece name and colour, and the equipment strip), bound
//! read-only to the existing customize state.
//!
//! Keys and the look model stay in `game::customize_screen` (the provisional screen is hidden
//! while this one shows); this module only reads [`CustomizeScreenState`], [`PlayerLookState`]
//! and [`LookCatalog`] and fills the game's panes, so changes keep applying to the model live and
//! the look keeps being stored by the save (`look_*`, v10). Text is the port's own English (the
//! same row and value labels the provisional screen shows). When the `dressup` layout is missing
//! the provisional screen stays (single warning from the library), so customize never disappears.
//!
//! The game's dress-up screen has a fitting step (preview before confirming) and a buy side; the
//! port applies every change immediately and does not sell clothes here, so the fitting strip and
//! the whole buy branch stay hidden and the equipment strip shows the selected row's value.
//! Mapping the port's 13 rows onto the game's 7 category tabs is port-side (SUPUESTO).

use bevy::prelude::*;

use super::scene::TextAlign;
use super::{free_text, OrigScreen, OrigUiConfig, OrigUiSystems, ScreenId};
use crate::game::customize_screen::{
    row_label, value_label, CustomizeScreenRoot, CustomizeScreenState, Row, ROWS,
};
use crate::game::dressup::{LookCatalog, PlayerLookState};

/// List row parts the `dressup_00` layout holds (`L_btn_list_00`..`06`).
pub const DRESSUP_ROWS: usize = 7;
/// Row slots the layout shows: five (`L_btn_list_01`..`05`, over the five `P_btn_base_empty`
/// plates). Parts `00` and `06` are the scroller's off-box rows (the game clips them with its
/// scissor, which the port does not draw), so they stay hidden.
pub const DRESSUP_SLOTS: usize = 5;
/// First visible row part (`L_btn_list_01`).
pub const DRESSUP_FIRST_SLOT: usize = 1;
/// Category tabs of the layout (`L_tab_00`..`06`).
pub const DRESSUP_TABS: usize = 7;

/// Install the customize binding: the provisional screen is hidden while this one shows.
pub fn install(app: &mut App) {
    app.add_systems(Update, bind_dressup_screen.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<CustomizeScreenRoot>(app, ScreenId::Customize);
}

/// First row shown so `selected` stays inside a `visible`-row window (stateless; the caller keeps
/// the returned window while the selection stays inside it). Port-side windowing: the game lists
/// the pieces of one category, the port lists its thirteen rows.
pub fn rows_scroll(selected: usize, scroll: usize, total: usize, visible: usize) -> usize {
    let max = total.saturating_sub(visible);
    let mut s = scroll.min(max);
    if selected < s {
        s = selected;
    } else if visible > 0 && selected >= s + visible {
        s = selected + 1 - visible;
    }
    s.min(max)
}

/// Category tab of a port row (port-side mapping, SUPUESTO): protagonist, appearance (hair style
/// and colours), skin tone, clothing, accessories, shoes and bag.
pub fn row_tab(row: Row) -> usize {
    match row {
        Row::Variant => 0,
        Row::Hair | Row::HairColor | Row::EyebrowColor | Row::EyeColor => 1,
        Row::SkinTone => 2,
        Row::Dress | Row::Top | Row::Bottom => 3,
        Row::Hat | Row::Glasses => 4,
        Row::Shoes => 5,
        Row::Bag => 6,
    }
}

/// Customize (`dressup_00` layout): the port's thirteen rows in the game's seven row slots (row
/// name and current value, selected row highlighted, the window follows the selection), the
/// category tab of the selected row marked, the selected value in the equipment strip, and the
/// port's key hint. The buy branch, the fitting strip and the game's key guide are hidden.
#[allow(clippy::too_many_arguments)]
pub fn bind_dressup_screen(
    config: Res<OrigUiConfig>,
    mut state: Option<ResMut<CustomizeScreenState>>,
    look_state: Option<Res<PlayerLookState>>,
    catalog: Option<Res<LookCatalog>>,
    mut screens: Query<&mut OrigScreen>,
) {
    if !config.enabled() {
        return;
    }
    let Some(mut s) = screens.iter_mut().find(|s| s.id == ScreenId::Customize) else {
        return;
    };
    let open = state.as_ref().is_some_and(|st| st.open);
    s.shown = open;
    let (Some(state), Some(look_state), Some(catalog)) = (state.as_deref_mut(), look_state, catalog)
    else {
        return;
    };
    if !open {
        return;
    }
    // The world keeps rendering (the viewer's customize camera shows the model up close).
    s.set_visible("N_buy_00", false);
    s.set_visible("L_guide_00", false);
    for name in [
        "N_dressup_not_equip_00",
        "N_dressup_noitem00",
        "N_fitting_00",
    ] {
        s.set_visible(name, false);
    }
    // The tab strip cursors (the game shows them while paging): not used by the port.
    for name in ["L_cursor_l_00", "L_cursor_r_00"] {
        s.set_visible(name, false);
    }

    let selected = state.selected.min(ROWS.len().saturating_sub(1));
    state.selected = selected;
    state.scroll = rows_scroll(selected, state.scroll, ROWS.len(), DRESSUP_SLOTS);
    let scroll = state.scroll;

    // One game row per port row inside the window (the five on-box slots).
    for k in 0..DRESSUP_ROWS {
        let part = format!("L_btn_list_{k:02}");
        let Some(row) = (DRESSUP_FIRST_SLOT..DRESSUP_FIRST_SLOT + DRESSUP_SLOTS)
            .contains(&k)
            .then(|| ROWS.get(scroll + (k - DRESSUP_FIRST_SLOT)).copied())
            .flatten()
        else {
            s.set_visible(&part, false);
            continue;
        };
        s.set_visible(&part, true);
        s.set_in(&part, "T_name_item_00", |o| {
            o.text = Some(row_label(row).to_string())
        });
        s.set_in(&part, "T_name_color_00", |o| {
            o.text = Some(value_label(row, &look_state.look, &catalog))
        });
        let tag = if scroll + (k - DRESSUP_FIRST_SLOT) == selected {
            "select"
        } else {
            "unselect"
        };
        if let Some(inst) = s.scene.part_instance(&part) {
            s.freeze_end(inst, tag, "sel");
        }
    }

    // The category tab of the selected row is the game's selected tab.
    let tab = row_tab(ROWS[selected]);
    for k in 0..DRESSUP_TABS {
        let part = format!("L_tab_{k:02}");
        let tag = if k == tab { "select" } else { "unselect" };
        if let Some(inst) = s.scene.part_instance(&part) {
            s.freeze_end(inst, tag, "sel");
        }
    }

    // Equipment strip: the selected row's value (the port applies every change immediately, so
    // there is no separate fitting state).
    let row = ROWS[selected];
    s.set_in("L_info_equipment_00", "T_name_item_epuip_00", |o| {
        o.text = Some(value_label(row, &look_state.look, &catalog))
    });
    s.set_in("L_info_equipment_00", "T_name_color_equip_00", |o| {
        o.text = Some(row_label(row).to_string())
    });
    if let Some(inst) = s.scene.part_instance("L_info_equipment_00") {
        s.freeze_end(inst, "equipment_state", "state");
    }

    // The port's own key hint (the game's key-guide layout uses other keys and is hidden).
    s.set_free_texts(vec![free_text(
        60.0,
        1020.0,
        1800.0,
        44.0,
        26.0,
        [0.92, 0.9, 0.82, 1.0],
        TextAlign::Left,
        format!(
            "{} / {}   Up / Down: row   Left / Right or Enter: change   Esc: back",
            selected + 1,
            ROWS.len()
        ),
    )]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_follows_the_selection() {
        assert_eq!(rows_scroll(0, 0, 13, 7), 0);
        assert_eq!(rows_scroll(6, 0, 13, 7), 0, "last slot of the first window");
        assert_eq!(rows_scroll(7, 0, 13, 7), 1, "one step down");
        assert_eq!(rows_scroll(12, 0, 13, 7), 6, "last row");
        assert_eq!(rows_scroll(3, 6, 13, 7), 3, "moving up pulls the window");
        assert_eq!(rows_scroll(0, 99, 13, 7), 0, "clamped");
    }

    #[test]
    fn every_row_maps_to_a_tab() {
        for row in ROWS {
            assert!(row_tab(row) < DRESSUP_TABS, "{row:?}");
        }
        assert_eq!(row_tab(Row::Variant), 0);
        assert_eq!(row_tab(Row::EyeColor), 1);
        assert_eq!(row_tab(Row::SkinTone), 2);
        assert_eq!(row_tab(Row::Bottom), 3);
        assert_eq!(row_tab(Row::Glasses), 4);
        assert_eq!(row_tab(Row::Shoes), 5);
        assert_eq!(row_tab(Row::Bag), 6);
    }
}
