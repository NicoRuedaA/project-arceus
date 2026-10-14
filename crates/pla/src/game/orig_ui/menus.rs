//! Menu screens in the original look, bound to the existing menu state (read only: keys,
//! actions and data stay those of the provisional screens, which are hidden while these show).
//!
//! | Screen | Original layout | Filled with |
//! |---|---|---|
//! | Pause menu (Esc) | `slidelist` / `slidemenu_menulist_01` | header = selected option, 4-icon tab strip paging the 15 options, option list + money / play time as port text |
//! | Party | `battle_menu` / `pokelist_battle_00` | 6 rows (icon, name, level, HP numbers + gauge), lead member's moves on the right |
//! | Bag | `bag` / `bag_top_00` + `inventory_00` | current pocket as item buttons (icon + count) on the game's 5 x 4 slots, item name and port detail line, money |
//! | Boxes | `box` / `box_top_00` + `inventory_00` | page of 32 Pokemon (icon + level, 8 x 4), party list, page / status line |
//! | Pokedex | `poke_dex` / `pokedex_detail_00` | list plates (number, name once caught, icon / silhouette), notebook page of the selected species |
//! | Map (M) | `map` / `map_top_00` | the game's Hisui map behind the port's own area map |
//!
//! Text is always the port's own (English); the game's text is never read.

use bevy::prelude::*;

use super::battle::title_case;
use super::icons::IconIndex;
use super::scene::{ExternalImage, NodeOverride, TextAlign};
use super::{free_text, OrigScreen, OrigUiConfig, OrigUiSystems, ScreenId};
use crate::battle_engine::Combatant;
use crate::game::bag_screen::{
    detail_for, pocket_codes, pocket_entries, screen_entries, BagItemTable, BagMode, BagRoot,
    BagUi, ShopCatalog,
};
use crate::game::battle_controller::BattleAssets;
use crate::game::battle_items::label;
use crate::game::box_screen::{
    BoxFocus, BoxScreenRoot, BoxScreenState, GRID_COLUMNS, SLOTS_PER_BOX,
};
use crate::game::dex_screen::{visibility, DexCatalog, DexRoot, DexUi, DexVisibility};
use crate::game::minimap::{FullMapRoot, MinimapState};
use crate::game::party_screen::{PartyCursor, PartyScreenRoot, PartyScreenState};
use crate::game::summary_screen::SummaryState;
use crate::game::pause_menu::{
    menu_info_text, option_label, MenuOption, PauseMenu, PauseMenuSuppressed, PauseRoot, OPTIONS,
};
use crate::game::play_time::PlayTime;
use crate::game::save_game::PartyMember;
use crate::game::save_wire::PlayerProgress;
use crate::game::settings::UserSettings;
use crate::game::state::Paused;
use crate::game::GameState;

pub fn install(app: &mut App) {
    app.add_systems(Update, bind_pause_menu.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<PauseRoot>(app, ScreenId::PauseMenu);
    app.add_systems(Update, bind_party_screen.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<PartyScreenRoot>(app, ScreenId::Party);
    app.add_systems(Update, bind_bag_screen.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<BagRoot>(app, ScreenId::Bag);
    app.add_systems(Update, bind_box_screen.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<BoxScreenRoot>(app, ScreenId::Box);
    app.add_systems(Update, bind_dex_screen.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<DexRoot>(app, ScreenId::Dex);
    app.add_systems(Update, bind_map_screen.in_set(OrigUiSystems::Bind));
}

/// The game's menu icon (texture base name) the port uses for each option. Only icons the
/// archive really holds are used (several, e.g. `map`, `box`, `pokedex`, are 2×2 placeholders
/// in this archive and are drawn from other packs by the game).
pub fn option_icon(o: MenuOption) -> &'static str {
    match o {
        MenuOption::Resume | MenuOption::Quit => "help",
        MenuOption::Bag => "bag",
        MenuOption::Party | MenuOption::Summary => "pokelist",
        MenuOption::Boxes => "ibox",
        MenuOption::Pokedex | MenuOption::Save | MenuOption::Load => "report",
        MenuOption::Crafting => "making",
        MenuOption::Shop => "buy",
        MenuOption::InvertX | MenuOption::InvertY | MenuOption::Volume | MenuOption::Graphics => "settings",
        MenuOption::SafeSpawn => "net",
        MenuOption::Rest => "passive",
    }
}

/// PNG of texture `name` of the screen's archive, as an override image (`None` for a missing or
/// placeholder texture).
fn archive_image(s: &OrigScreen, name: &str) -> Option<ExternalImage> {
    let info = s.scene.archive().textures.get(name)?;
    if info.width <= 2 || info.height <= 2 {
        return None; // placeholder texture
    }
    let png = info.png.clone()?;
    Some(ExternalImage {
        png,
        uv: [0.0, 0.0, 1.0, 1.0],
    })
}

const TABS: usize = 4;

#[allow(clippy::too_many_arguments)]
pub fn bind_pause_menu(
    config: Res<OrigUiConfig>,
    paused: Option<Res<Paused>>,
    suppressed: Option<Res<PauseMenuSuppressed>>,
    menu: Option<Res<PauseMenu>>,
    settings: Option<Res<UserSettings>>,
    progress: Option<Res<PlayerProgress>>,
    play: Option<Res<PlayTime>>,
    mut screens: Query<&mut OrigScreen>,
) {
    if !config.enabled() {
        return;
    }
    let Some(mut s) = screens.iter_mut().find(|s| s.id == ScreenId::PauseMenu) else {
        return;
    };
    let open = paused.as_ref().is_some_and(|p| p.0) && !suppressed.as_ref().is_some_and(|p| p.0);
    s.shown = open;
    let (Some(menu), Some(settings)) = (menu, settings) else {
        return;
    };
    if !open {
        return;
    }
    let sel = menu.selected.min(OPTIONS.len() - 1);
    s.set_text("T_title_00", option_label(OPTIONS[sel], &settings));
    let page = sel / TABS;
    for k in 0..TABS {
        let part = format!("L_menu_0{k}");
        let Some(inst) = s.scene.part_instance(&part) else {
            continue;
        };
        let idx = page * TABS + k;
        let Some(&opt) = OPTIONS.get(idx) else {
            s.set_visible(&part, false);
            continue;
        };
        s.set_visible(&part, true);
        let icon = option_icon(opt);
        let main = archive_image(&s, &format!("sm_menu_icon_{icon}_00^t"));
        let shadow = archive_image(&s, &format!("sm_menu_icon_{icon}_ds_00^s"));
        for pane in ["P_icon_menu_00", "P_icon_menu_01", "P_icon_menu_03"] {
            let img = main.clone();
            s.set_in(&part, pane, |o| o.image = img);
        }
        for pane in [
            "P_icon_menu_00_ds_00",
            "P_icon_menu_01_ds_00",
            "P_icon_menu_03_ds_00",
        ] {
            let img = shadow.clone();
            s.set_in(&part, pane, |o| o.image = img);
        }
        let tag = if idx == sel { "select" } else { "unselect" };
        s.freeze_end(inst, tag, "sel");
    }
    // The whole option list and the summary line: the port's own text on the dimmed panel.
    let money = progress.as_ref().map_or(0, |p| p.money);
    let seconds = play.as_ref().map_or(0, |p| p.seconds);
    let labels: Vec<String> = OPTIONS.iter().map(|&o| option_label(o, &settings)).collect();
    let texts = pause_list_texts(&labels, sel, &menu_info_text(money, seconds));
    // A dark panel behind the list keeps it readable over bright scenery (the game blurs a
    // capture of the scene instead; port approximation).
    let bottom = texts
        .iter()
        .map(|t| t.center.y + t.size.y / 2.0)
        .fold(PAUSE_LIST_TOP, f32::max);
    let (top, h) = (PAUSE_LIST_TOP - 16.0, bottom - PAUSE_LIST_TOP + 32.0);
    s.set_free_quads(vec![super::free_rect(
        Vec2::new(40.0 + 430.0, top + h / 2.0),
        Vec2::new(860.0, h),
        [0.03, 0.04, 0.06, 0.62],
    )]);
    s.set_free_texts(texts);
}

/// Top of the option list (layout pixels, below the game's title bar and tab strip).
pub const PAUSE_LIST_TOP: f32 = 118.0;
/// Lowest layout pixel the pause texts may reach (a margin above the 1080 px bottom edge).
pub const PAUSE_LIST_BOTTOM: f32 = 1036.0;
/// Height of the two footer lines (money / play time and the key hint) below the rows.
const PAUSE_FOOTER: f32 = 104.0;

/// Row pitch of the pause list: the authored 52 px when it fits, tighter for long lists so the
/// rows and the footer always end above [`PAUSE_LIST_BOTTOM`] (the layout is stretched to the
/// viewport, so this holds at every 16:9 resolution).
pub fn pause_row_pitch(rows: usize) -> f32 {
    let room = PAUSE_LIST_BOTTOM - PAUSE_FOOTER - PAUSE_LIST_TOP;
    (room / rows.max(1) as f32).min(52.0)
}

/// Free texts of the pause list: one row per label (`sel` highlighted) and the footer lines.
pub fn pause_list_texts(labels: &[String], sel: usize, info: &str) -> Vec<super::scene::TextSlot> {
    let cream = [0.98, 0.96, 0.85, 1.0];
    let gold = [1.0, 0.82, 0.35, 1.0];
    let dim = [0.75, 0.75, 0.72, 1.0];
    let pitch = pause_row_pitch(labels.len());
    // The glyphs shrink with the pitch so neighbouring rows never touch.
    let font = (pitch * 0.62).min(32.0);
    let mut texts = Vec::with_capacity(labels.len() + 2);
    for (i, label) in labels.iter().enumerate() {
        let (color, mark) = if i == sel {
            (gold, "> ")
        } else {
            (cream, "   ")
        };
        texts.push(free_text(
            90.0,
            PAUSE_LIST_TOP + i as f32 * pitch,
            760.0,
            pitch - 2.0,
            font,
            color,
            TextAlign::Left,
            format!("{mark}{label}"),
        ));
    }
    let footer = PAUSE_LIST_TOP + labels.len() as f32 * pitch + 12.0;
    texts.push(free_text(
        90.0,
        footer,
        900.0,
        44.0,
        26.0,
        dim,
        TextAlign::Left,
        info,
    ));
    texts.push(free_text(
        90.0,
        footer + 46.0,
        900.0,
        40.0,
        24.0,
        dim,
        TextAlign::Left,
        "Arrows / W S + Enter, Esc to resume",
    ));
    texts
}

// ------------------------------------------------------------------------------------- party

/// Display name and maximum HP of a party member (from the battle data; `None` when missing).
pub fn member_info(assets: Option<&BattleAssets>, m: &PartyMember) -> (String, Option<u32>) {
    let species = assets.and_then(|a| {
        a.data
            .species_row_by_dex_form(m.species, m.form)
            .ok()
            .flatten()
    });
    let name = m
        .nickname
        .clone()
        .or_else(|| species.as_ref().map(|s| title_case(&s.name)))
        .unwrap_or_else(|| format!("#{:04}", m.species));
    let max = species.map(|s| Combatant::from_base(s, m.level, Vec::new()).max_hp);
    (name, max)
}

/// Move name, PP maximum and type (from the battle data).
fn move_info(assets: Option<&BattleAssets>, id: u16) -> (String, Option<u8>, String) {
    match assets.and_then(|a| a.data.move_row_by_id(id).ok().flatten()) {
        Some(m) => (title_case(&m.name), Some(m.pp), m.move_type),
        None => (format!("Move #{id}"), None, String::new()),
    }
}

/// Party screen (`pokelist_battle_00`, the game's Pokémon list): six rows with icon, name,
/// level, HP numbers and gauge; the right panel shows the lead member and its moves (the port's
/// party screen has no cursor). Experience gauges, gender and status icons are hidden (the save
/// holds no gender; experience to next level is not shown by the port yet).
#[allow(clippy::too_many_arguments)]
pub fn bind_party_screen(
    config: Res<OrigUiConfig>,
    state: Option<Res<PartyScreenState>>,
    cursor: Option<Res<PartyCursor>>,
    summary: Option<Res<SummaryState>>,
    progress: Option<Res<PlayerProgress>>,
    assets: Option<Res<BattleAssets>>,
    icons: Res<IconIndex>,
    mut screens: Query<&mut OrigScreen>,
) {
    if !config.enabled() {
        return;
    }
    let Some(mut s) = screens.iter_mut().find(|s| s.id == ScreenId::Party) else {
        return;
    };
    // The summary opened from this list replaces it while open.
    let open = state.is_some_and(|st| st.open) && !summary.is_some_and(|x| x.open);
    let sel = cursor.map_or(0, |c| c.0);
    s.shown = open;
    let Some(progress) = progress.filter(|_| open) else {
        return;
    };
    let assets = assets.as_deref();
    for i in 0..6 {
        let part = format!("L_list_base_0{i}");
        let Some(m) = progress.party.get(i) else {
            s.set_visible(&part, false);
            continue;
        };
        s.set_visible(&part, true);
        let (name, max) = member_info(assets, m);
        let max = max.unwrap_or(m.hp).max(1);
        s.set_in(&part, "T_name_01", |o| o.text = Some(name));
        s.set_in(&part, "T_lv_00", |o| {
            o.text = Some(format!("Lv. {}", m.level))
        });
        s.set_in(&part, "T_hp_heading_00", |o| o.text = Some("HP".into()));
        s.set_in(&part, "T_hp_numerator", |o| o.text = Some(m.hp.to_string()));
        s.set_in(&part, "T_hp_slash", |o| o.text = Some("/".into()));
        s.set_in(&part, "T_hp_denominator", |o| {
            o.text = Some(max.to_string())
        });
        if let Some(inst) = s.scene.part_instance(&part) {
            // The HP gauge is a nested part of this row.
            if let Some(gauge_node) = s.node_in_instance(inst, "L_gauge_hp_00") {
                if let Some(g) = s.scene.instance_of_node(gauge_node) {
                    let ratio = m.hp as f32 / max as f32;
                    if let Some(n) = s.node_in_instance(g, "P_color_00") {
                        let p = &s.scene.nodes[n].pane;
                        let (w, h, tx, ty) = (p.size[0], p.size[1], p.translate[0], p.translate[1]);
                        s.set_node(n, |o| {
                            o.size = Some([w * ratio, h]);
                            o.translate = Some([tx - w * (1.0 - ratio) / 2.0, ty]);
                            o.visible = Some(m.hp > 0);
                        });
                    }
                    let colour = if ratio > 0.5 {
                        "color_green"
                    } else if ratio > 0.2 {
                        "color_yellow"
                    } else {
                        "color_red"
                    };
                    s.freeze(g, colour, 0.0, "gauge_color");
                }
            }
            for hidden in [
                "L_gauge_exp_00",
                "T_exp_00",
                "P_icon_next_exp_00",
                "P_fx_bospoke_00",
                "L_icon_seibetsu_00",
                "P_sick_00",
            ] {
                if let Some(n) = s.node_in_instance(inst, hidden) {
                    s.set_node(n, |o| o.visible = Some(false));
                }
            }
            let icon = icons.pokemon(m.species, m.form as u16);
            if let Some(n) = s.node_in_instance(inst, "P_icon_monbo_00") {
                s.set_node(n, |o| {
                    o.visible = Some(icon.is_some());
                    o.image = icon;
                });
            }
            let tag = if i == sel { "select" } else { "unselect" };
            s.freeze_end(inst, tag, "sel");
        }
    }
    // Right panel: the highlighted member.
    if let Some(lead) = progress.party.get(sel).or(progress.party.first()) {
        let (name, _) = member_info(assets, lead);
        s.set("T_name_poke_00", |o| {
            o.text = Some(name);
            o.text_color = Some([0.16, 0.2, 0.26, 1.0]);
        });
        s.set_text(
            "T_situation_00",
            if sel == 0 { "Party leader" } else { "Party member" },
        );
        for k in 0..4 {
            let part = format!("L_status_button_skill_0{k}");
            match lead.moves.get(k) {
                Some(mv) => {
                    let (mname, max, mtype) = move_info(assets, mv.move_id);
                    let pp = match max {
                        Some(mx) => format!("{}/{mx}", mv.pp),
                        None => mv.pp.to_string(),
                    };
                    s.set_visible(&part, true);
                    s.set_in(&part, "T_skill_name_00", |o| o.text = Some(mname));
                    s.set_in(&part, "T_pp_param_00", |o| o.text = Some(pp));
                    s.set_in(&part, "P_type_affinity_icon_00", |o| {
                        o.visible = Some(false)
                    });
                    let tint = super::icons::type_tint(&mtype);
                    s.set_in(&part, "P_icon_poke_type_00", |o| o.tint = Some(tint));
                }
                None => s.set_visible(&part, false),
            }
        }
    }
    s.set_text(
        "T_info_00",
        "Up / Down: choose   Enter: summary   Esc: close",
    );
}

// --------------------------------------------------------------------------------------- bag

/// Grid cells of the game's inventory layout (6 × 8 buttons the game places at run time).
pub const BAG_CELLS: usize = 48;
/// Cells the bag shows per page: the 5 × 4 slot circles of `bag_top_00`.
pub const BAG_PAGE: usize = 20;

/// Fill grid cell `k` (`L_btn_main_kk` of the inventory grid instance `grid`): place it at `pos`
/// (layout pixels, the grid's own placement is run-time data), show `content` (icon, label) or
/// an empty slot, and the game's select / unselect state.
#[allow(clippy::too_many_arguments)]
fn fill_cell(
    s: &mut OrigScreen,
    grid: usize,
    k: usize,
    pos: [f32; 2],
    scale: f32,
    shown: bool,
    content: Option<(Option<ExternalImage>, String)>,
    selected: bool,
) {
    let Some(node) = s.scene.find_in(grid, &format!("L_btn_main_{k:02}")) else {
        return;
    };
    let Some(inst) = s.scene.instance_of_node(node) else {
        return;
    };
    s.set_node(node, |o| {
        o.visible = Some(shown);
        o.translate = Some(pos);
        o.scale = Some([scale, scale]);
    });
    if !shown {
        return;
    }
    let set = |s: &mut OrigScreen, name: &str, f: &dyn Fn(&mut NodeOverride)| {
        if let Some(n) = s.scene.find_in(inst, name) {
            s.set_node(n, |o| f(o));
        }
    };
    for hidden in [
        "T_00",
        "P_mask_price_00",
        "P_base_price_00",
        "P_icon_check_00",
        "P_icon_favo_00",
        "Pi_icon_use_field_00",
        "P_blost_00",
    ] {
        set(s, hidden, &|o| o.visible = Some(false));
    }
    let (icon, label) = content.map_or((None, None), |(i, l)| (i, Some(l)));
    let has_icon = icon.is_some();
    set(s, "P_icon_00", &|o| {
        o.visible = Some(has_icon);
        o.image = icon.clone();
    });
    let has_label = label.is_some();
    set(s, "T_01", &|o| {
        o.visible = Some(has_label);
        o.text = label.clone();
    });
    set(s, "P_base_catch_num_00", &|o| {
        o.alpha = Some(if has_label { 255.0 } else { 0.0 })
    });
    s.freeze_end(inst, if selected { "select" } else { "unselect" }, "sel");
}

/// Bag (`bag_top_00` with its `inventory_00` grid): the current pocket's items as the game's
/// item buttons (item icon + count), the selected cell highlighted, the item name and the port's
/// detail line in the info panel, the pocket in the tab strip and the money. The neighbouring
/// grids the game slides in from the sides are hidden. Shop mode keeps the provisional screen.
#[allow(clippy::too_many_arguments)]
pub fn bind_bag_screen(
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
    let Some(mut s) = screens.iter_mut().find(|s| s.id == ScreenId::Bag) else {
        return;
    };
    let open = ui
        .as_ref()
        .is_some_and(|u| u.open && u.mode == BagMode::Bag);
    s.shown = open;
    let (Some(ui), Some(table), Some(shops)) = (ui, table, shops) else {
        return;
    };
    let Some(table) = table.0.as_ref().filter(|_| open) else {
        return;
    };
    let empty = std::collections::BTreeMap::new();
    let bag = progress.as_ref().map(|p| &p.bag).unwrap_or(&empty);
    let entries = screen_entries(&ui, table, bag, &shops.0);
    let page = ui.selected / BAG_PAGE;
    if let Some(grid) = s.scene.part_instance("L_inventory_parts_00") {
        for k in 0..BAG_CELLS {
            let idx = page * BAG_PAGE + k;
            let cell = (k < BAG_PAGE)
                .then(|| entries.get(idx))
                .flatten()
                .map(|&(id, count)| (icons.item(id), format!("x{count}")));
            let (col, row) = ((k % 5) as f32, (k / 5) as f32);
            let shown = k < BAG_PAGE && cell.is_some();
            fill_cell(
                &mut s,
                grid,
                k,
                [-788.0 + col * 156.0, 305.0 - row * 146.0],
                1.0,
                shown,
                cell,
                idx == ui.selected,
            );
        }
    }
    // The game slides the neighbouring pockets in from the sides: not shown at rest.
    for n in 0..s.scene.nodes.len() {
        if s.scene.nodes[n].pane.name.starts_with("N_sub_") {
            s.set_node(n, |o| o.visible = Some(false));
        }
    }
    let (heading, detail) = match entries
        .get(ui.selected)
        .and_then(|(id, c)| table.get(*id).map(|i| (i, *c)))
    {
        Some((item, count)) => (label(item), detail_for(item, count)),
        None => ("Empty pocket".to_string(), String::new()),
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
    let pockets = pocket_codes(table, bag);
    let pocket = pockets.get(ui.pocket).copied().unwrap_or(0);
    let n = pocket_entries(table, bag, pocket).len();
    s.set_in("L_tab_disp_00", "T_text_00", |o| {
        o.text = Some(format!("Pocket {pocket}"))
    });
    s.set_in("L_tab_disp_01", "T_text_00", |o| {
        o.text = Some(format!("{n} items"))
    });
    let money = progress.as_ref().map_or(0, |p| p.money);
    s.set_in("L_money_00", "T_param_money_00", |o| {
        o.text = Some(money.to_string())
    });
    for n in ["T_param_add_00", "T_param_sub_00"] {
        s.set_in("L_money_00", n, |o| o.text = None);
    }
    let msg = if ui.message.is_empty() {
        "Left / Right: pocket   Up / Down: item   Enter: use   Esc: close".to_string()
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
        TextAlign::Left,
        msg,
    )]);
}

// --------------------------------------------------------------------------------------- box

/// Pokémon boxes (`box_top_00`): the current page's Pokémon as the game's grid buttons
/// (Pokémon icon + level), the party in the game's left list (name + level), the selected slot
/// or party row highlighted, page and status line as the port's text. The port's box page holds
/// 32 Pokémon in 8 × 4; the game's 6-wide grid cells are re-spaced to that shape. Neighbouring grids, the all-box
/// overview and the trade tab are hidden.
#[allow(clippy::too_many_arguments)]
pub fn bind_box_screen(
    config: Res<OrigUiConfig>,
    state: Option<Res<BoxScreenState>>,
    progress: Option<Res<PlayerProgress>>,
    assets: Option<Res<BattleAssets>>,
    icons: Res<IconIndex>,
    mut screens: Query<&mut OrigScreen>,
) {
    if !config.enabled() {
        return;
    }
    let Some(mut s) = screens.iter_mut().find(|s| s.id == ScreenId::Box) else {
        return;
    };
    let open = state.as_ref().is_some_and(|st| st.open);
    s.shown = open;
    let (Some(st), Some(progress)) = (state.filter(|_| open), progress) else {
        return;
    };
    let assets = assets.as_deref();
    let base = st.page * SLOTS_PER_BOX;
    if let Some(grid) = s.scene.part_instance("L_inventory_parts_00") {
        for k in 0..BAG_CELLS {
            // The port's page is 8 x 4: the game's cells are re-spaced (and shrunk) to that shape.
            let (col, row) = ((k % GRID_COLUMNS) as f32, (k / GRID_COLUMNS) as f32);
            let cell = (k < SLOTS_PER_BOX)
                .then(|| progress.box_list.get(base + k))
                .flatten()
                .map(|m| {
                    (
                        icons.pokemon(m.species, m.form as u16),
                        format!("Lv{}", m.level),
                    )
                });
            let selected = st.focus == BoxFocus::Box && k == st.selected_slot;
            fill_cell(
                &mut s,
                grid,
                k,
                [-430.0 + col * 110.0, 304.0 - row * 142.0],
                0.8,
                k < SLOTS_PER_BOX,
                cell,
                selected,
            );
        }
    }
    for n in 0..s.scene.nodes.len() {
        let name = &s.scene.nodes[n].pane.name;
        if name.starts_with("N_sub_")
            || matches!(
                name.as_str(),
                "N_list_allbox_00" | "L_mode_trade_00" | "N_icon_base_00" | "N_info_status"
            )
        {
            s.set_node(n, |o| o.visible = Some(false));
        }
    }
    for i in 0..6 {
        let part = format!("L_list_temochi_base_0{i}");
        let Some(m) = progress.party.get(i) else {
            s.set_visible(&part, false);
            continue;
        };
        s.set_visible(&part, true);
        let (name, _) = member_info(assets, m);
        s.set_in(&part, "T_nickname_00", |o| o.text = Some(name));
        s.set_in(&part, "T_lv_00", |o| {
            o.text = Some(format!("Lv. {}", m.level))
        });
        s.set_in(&part, "L_icon_sick_00", |o| o.visible = Some(false));
        if let Some(inst) = s.scene.part_instance(&part) {
            let tag = if st.focus == BoxFocus::Party && i == st.selected_party {
                "select"
            } else {
                "unselect"
            };
            s.freeze_end(inst, tag, "sel");
        }
    }
    let pages = progress.box_list.len().div_ceil(SLOTS_PER_BOX).max(1);
    let mut lines = vec![free_text(
        640.0,
        30.0,
        900.0,
        44.0,
        30.0,
        [0.95, 0.93, 0.85, 1.0],
        TextAlign::Left,
        format!(
            "Box {} / {}   ({} Pokémon stored)",
            st.page + 1,
            pages.max(st.page + 1),
            progress.box_list.len()
        ),
    )];
    let msg = if st.message.is_empty() {
        "Arrows: move   Tab: party / box   Enter: pick / place   PgUp / PgDn: page   Esc: close"
            .to_string()
    } else {
        st.message.clone()
    };
    lines.push(free_text(
        60.0,
        1020.0,
        1800.0,
        44.0,
        26.0,
        [0.92, 0.9, 0.82, 1.0],
        TextAlign::Left,
        msg,
    ));
    s.set_free_texts(lines);
}

// ------------------------------------------------------------------------------------ pokédex

/// Plates of the game's list shown at once (the layout holds 9; the last two sit off screen).
const DEX_PLATES: usize = 7;

/// Pokédex (`pokedex_detail_00`): the game's notebook page for the selected species (number,
/// name once caught, seen / caught state and the port's counters as text) and the game's list
/// plates on the right (Pokémon icon once seen, black silhouette when only seen, the game's
/// "undiscovered" icon otherwise). Research tasks, forms, sizes and the description of the
/// game are not used (no research data in the port; game text is out of scope).
#[allow(clippy::too_many_arguments)]
pub fn bind_dex_screen(
    config: Res<OrigUiConfig>,
    ui: Option<Res<DexUi>>,
    catalog: Option<Res<DexCatalog>>,
    progress: Option<Res<PlayerProgress>>,
    assets: Option<Res<BattleAssets>>,
    icons: Res<IconIndex>,
    mut names: Local<crate::game::orig_ui::field::NameCache>,
    mut screens: Query<&mut OrigScreen>,
) {
    if !config.enabled() {
        return;
    }
    let Some(mut s) = screens.iter_mut().find(|s| s.id == ScreenId::Dex) else {
        return;
    };
    let open = ui.as_ref().is_some_and(|u| u.open);
    s.shown = open;
    let (Some(ui), Some(catalog), Some(progress)) = (ui.filter(|_| open), catalog, progress) else {
        return;
    };
    let assets = assets.as_deref();
    let entries = &catalog.entries;
    let sel = ui.selected.min(entries.len().saturating_sub(1));
    let start = sel
        .saturating_sub(DEX_PLATES / 2)
        .min(entries.len().saturating_sub(DEX_PLATES));
    for k in 0..9 {
        let part = format!("L_list_poke_0{k}");
        let Some(inst) = s.scene.part_instance(&part) else {
            continue;
        };
        let Some(e) = entries.get(start + k).filter(|_| k < DEX_PLATES) else {
            s.set_visible(&part, false);
            continue;
        };
        s.set_visible(&part, true);
        let state = visibility(e.id, &progress.seen, &progress.caught);
        let name = match state {
            DexVisibility::Caught => names.get(assets, e.id, 0),
            _ => "??????".to_string(),
        };
        let icon = (state != DexVisibility::Hidden)
            .then(|| icons.pokemon(e.id, 0))
            .flatten();
        let silhouette = state == DexVisibility::SeenSilhouette;
        let num = format!("{:03}", e.id);
        let set = |s: &mut OrigScreen, pane: &str, f: &dyn Fn(&mut NodeOverride)| {
            if let Some(n) = s.node_in_instance(inst, pane) {
                s.set_node(n, |o| f(o));
            }
        };
        set(&mut s, "T_dexnum_00", &|o| o.text = Some(num.clone()));
        set(&mut s, "T_pokerace_00", &|o| o.text = Some(name.clone()));
        let has_icon = icon.is_some();
        set(&mut s, "P_loader_icon_poke_00", &|o| {
            o.visible = Some(has_icon);
            o.image = icon.clone();
            o.tint = silhouette.then_some([0.0, 0.0, 0.0, 1.0]);
        });
        set(&mut s, "N_icon_undiscovered_00", &|o| {
            o.visible = Some(!has_icon)
        });
        for hidden in ["P_icon_complete_00", "P_icon_perfect_00"] {
            set(&mut s, hidden, &|o| o.visible = Some(false));
        }
        let tag = if start + k == sel {
            "select"
        } else {
            "unselect"
        };
        s.freeze_end(inst, tag, "sel");
    }
    // Notebook page of the selected species (main layout instance 0).
    if let Some(e) = entries.get(sel) {
        let state = visibility(e.id, &progress.seen, &progress.caught);
        let name = match state {
            DexVisibility::Caught => names.get(assets, e.id, 0),
            _ => "??????".to_string(),
        };
        let status = match state {
            DexVisibility::Caught => "Caught",
            DexVisibility::SeenSilhouette => "Seen, not caught yet",
            DexVisibility::Hidden => "Not seen yet",
        };
        let set0 = |s: &mut OrigScreen, pane: &str, text: Option<String>| {
            if let Some(n) = s.node_in_instance(0, pane) {
                s.set_node(n, |o| o.text = text);
            }
        };
        set0(&mut s, "T_pokerace_00", Some(name));
        set0(&mut s, "T_dexnum_00", Some(format!("No. {:03}", e.id)));
        set0(
            &mut s,
            "T_description_00",
            Some(format!(
                "{status}.  Seen {}, caught {} of {} species.",
                progress.seen.len(),
                progress.caught.len(),
                catalog.distinct_species_count()
            )),
        );
        let pic = (state != DexVisibility::Hidden)
            .then(|| icons.pokemon_large(e.id, 0))
            .flatten();
        let has_pic = pic.is_some();
        let dark = state == DexVisibility::SeenSilhouette;
        if let Some(n) = s.node_in_instance(0, "P_loader_poke_00") {
            s.set_node(n, |o| {
                o.visible = Some(has_pic);
                o.image = pic;
                o.tint = dark.then_some([0.0, 0.0, 0.0, 1.0]);
            });
        }
        if let Some(n) = s.node_in_instance(0, "N_discovered_00") {
            s.set_node(n, |o| o.visible = Some(!has_pic));
        }
        if let Some(n) = s.node_in_instance(0, "N_gender_00") {
            s.set_node(n, |o| o.visible = Some(false));
        }
        for pane in [
            "T_classification_00",
            "T_form_name_00",
            "T_h_form_num_00",
            "T_param_form_num_00",
            "T_h_like_00",
            "T_h_item_00",
        ] {
            set0(&mut s, pane, None);
        }
    }
    s.set_free_texts(vec![free_text(
        60.0,
        1020.0,
        1800.0,
        44.0,
        26.0,
        [0.92, 0.9, 0.82, 1.0],
        TextAlign::Left,
        "Up / Down: select   Esc: close",
    )]);
}

// --------------------------------------------------------------------------------------- map

/// Full-screen map (`M`): the game's Hisui map (`map_top_00`) is drawn as the background and the
/// port's own area map (with the player marker) stays in front, without its dark backdrop. The
/// game's area highlights need a second (mask) texture the renderer does not combine yet, its
/// location tags and quest icons are run-time content: all hidden. The fog clouds stay.
pub fn bind_map_screen(
    config: Res<OrigUiConfig>,
    state: Res<State<GameState>>,
    minimap: Option<Res<MinimapState>>,
    mut screens: Query<&mut OrigScreen>,
    mut overlay: Query<&mut BackgroundColor, With<FullMapRoot>>,
) {
    if !config.enabled() {
        return;
    }
    let Some(mut s) = screens.iter_mut().find(|s| s.id == ScreenId::Map) else {
        return;
    };
    let open = minimap.is_some_and(|m| m.open) && *state.get() == GameState::World;
    s.shown = open;
    for mut bg in &mut overlay {
        let want = if open {
            Color::NONE
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.72)
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
    if !open {
        return;
    }
    for name in [
        "N_area_vfx_00",
        "N_icon_00",
        "N_icon_qicon_vfx_00",
        "N_deco_name_00",
        "P_fade_00",
    ] {
        s.set_visible(name, false);
    }
}
