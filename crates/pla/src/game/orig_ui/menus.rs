//! Menu screens in the original look, bound to the existing menu state (read only: keys,
//! actions and data stay those of the provisional screens, which are hidden while these show).
//!
//! | Screen | Original layout | Filled with |
//! |---|---|---|
//! | Pause menu (Esc) | `slidelist` / `slidemenu_menulist_01` | header = selected option, 4-icon tab strip paging the options, option list fitted above the bottom edge + money / play time as port text |
//! | Party | `battle_menu` / `pokelist_battle_00` | 6 rows (icon, name, level, HP numbers + gauge), cursor row's moves on the right; Enter opens the summary (`summary.rs`) |
//! | Bag | `bag` / `bag_top_00` + `inventory_00` | current pocket as item buttons (item / ball icon + count) on the game's own 5 x 4 slot circles, port pocket names, item label and description, money |
//! | Boxes | `box` / `box_top_00` + `inventory_00` | box of 30 Pokémon (icon + level, the game's 6 x 5 grid from its anchor), party buttons and rows, held / selected state, messages in the game's strip |
//! | Pokedex | `poke_dex` / `pokedex_detail_00` | list plates (number, name once caught, icon / silhouette), notebook page of the selected species with type badges and base stats, seen / caught counters |
//! | Map (M) | `map` / `map_top_00` | the game's Hisui map with every area's tint and the current area highlighted, beside the port's area map with the game's player arrow and fast-travel points |
//!
//! Text is always the port's own (English); the game's text is never read.

use bevy::prelude::*;

use super::battle::title_case;
use super::icons::IconIndex;
use super::scene::{ExternalImage, NodeOverride, TextAlign};
use super::{free_text, OrigScreen, OrigUiConfig, OrigUiSystems, ScreenId};
use crate::battle_engine::Combatant;
use crate::game::bag_screen::{
    pocket_codes, pocket_entries, screen_entries, BagItemTable, BagMode, BagRoot,
    BagUi, ShopCatalog,
};
use crate::game::battle_controller::BattleAssets;
use crate::game::battle_items::label;
use crate::game::box_screen::{
    BoxFocus, BoxScreenRoot, BoxScreenState, BoxSelection, GRID_COLUMNS, SLOTS_PER_BOX,
};
use crate::game::dex_screen::{visibility, DexCatalog, DexRoot, DexUi, DexVisibility};
use crate::game::graphics_screen::{
    self, GraphicsRuntime, GraphicsScreenState, GraphicsSettings,
};
use crate::game::minimap::{FullMapRoot, MinimapState};
use crate::game::party_screen::{PartyCursor, PartyOpsState, PartyScreenRoot, PartyScreenState};
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
    app.add_systems(Update, bind_options_screen.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<crate::game::graphics_screen::GraphicsScreenRoot>(
        app,
        ScreenId::Options,
    );
    app.add_systems(Update, bind_party_screen.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<PartyScreenRoot>(app, ScreenId::Party);
    app.add_systems(Update, bind_bag_screen.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<BagRoot>(app, ScreenId::Bag);
    app.add_systems(Update, bind_box_screen.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<BoxScreenRoot>(app, ScreenId::Box);
    app.add_systems(Update, bind_dex_screen.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<DexRoot>(app, ScreenId::Dex);
    app.add_systems(
        Update,
        (bind_map_screen, sync_map_markers).in_set(OrigUiSystems::Bind),
    );
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
    ops: Option<Res<PartyOpsState>>,
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
    // Operations submenu (Enter on a member): the port's own popup over the move panel.
    let ops_open = ops.as_ref().is_some_and(|o| o.open);
    let mut lines = Vec::new();
    if ops_open {
        let ops = ops.as_deref().expect("checked above");
        let who = progress
            .party
            .get(sel)
            .map(|m| member_info(assets, m).0)
            .unwrap_or_default();
        lines.push(free_text(
            1260.0,
            380.0,
            620.0,
            46.0,
            26.0,
            [0.98, 0.96, 0.85, 1.0],
            TextAlign::Left,
            format!("Actions: {who}"),
        ));
        for (i, op) in crate::game::party_screen::OPS.iter().enumerate() {
            let selected = ops.selected == i;
            lines.push(free_text(
                1260.0,
                432.0 + i as f32 * 46.0,
                620.0,
                44.0,
                24.0,
                if selected {
                    [1.0, 0.82, 0.35, 1.0]
                } else {
                    [0.98, 0.96, 0.85, 1.0]
                },
                TextAlign::Left,
                format!("{}{op}", if selected { "> " } else { "   " }),
            ));
        }
        lines.push(free_text(
            1260.0,
            716.0,
            620.0,
            44.0,
            22.0,
            [0.75, 0.75, 0.72, 1.0],
            TextAlign::Left,
            if ops.message.is_empty() {
                "Enter: choose   Esc: back".to_string()
            } else {
                ops.message.clone()
            },
        ));
        s.set_free_quads(vec![super::free_rect(
            Vec2::new(1560.0, 580.0),
            Vec2::new(660.0, 420.0),
            [0.03, 0.04, 0.06, 0.85],
        )]);
    } else {
        s.set_free_quads(Vec::new());
    }
    // The game's own info strip (`T_info_00`) is not visible in this layout's resting state, so
    // the port draws its own line at the bottom (same as the bag and box screens).
    lines.push(free_text(
        60.0,
        1020.0,
        1800.0,
        44.0,
        26.0,
        [0.92, 0.9, 0.82, 1.0],
        TextAlign::Left,
        if ops_open {
            "Enter: choose   Esc: back"
        } else {
            "Up / Down: choose   Enter: actions   Esc: close"
        },
    ));
    s.set_free_texts(lines);
}

// ----------------------------------------------------------------------------------- options

/// Row parts of the game's settings list (`option_settings_00`): `L_settings_list_00`..`13`. The
/// port shows the first nine of them (the game's list box clips the ninth against its edge; the
/// port's renderer has no scissor, so it drops the five off-box parts instead).
pub const OPTION_ROW_PARTS: usize = 14;
/// Rows shown at once (the port's window inside the fourteen parts).
pub const OPTION_FULL_ROWS: usize = 9;

/// First row shown so the selection stays inside [`OPTION_FULL_ROWS`] slots. Stateless: the
/// caller keeps the returned window while the selection stays inside it.
pub fn options_scroll(selected: usize, scroll: usize, total: usize) -> usize {
    let max = total.saturating_sub(1);
    let mut s = scroll.min(max);
    if selected < s {
        s = selected;
    } else if selected >= s + OPTION_FULL_ROWS {
        s = selected + 1 - OPTION_FULL_ROWS;
    }
    s.min(max)
}

/// The Graphics screen (Esc menu -> "Graphics...") in the game's own settings list: one
/// `L_settings_list_XX` row per visible entry of the current page, the game's selection state on
/// the cursor row and its slider gauge on slider rows. Read-only: keys stay in
/// `game::graphics_screen` (the provisional screen owns the input and is hidden while this one
/// shows). When the `option` layout is missing the provisional screen stays (single warning from
/// the orig-UI library), so the screen never disappears.
#[allow(clippy::too_many_arguments)]
pub fn bind_options_screen(
    config: Res<OrigUiConfig>,
    state: Option<Res<GraphicsScreenState>>,
    settings: Option<Res<GraphicsSettings>>,
    runtime: Option<Res<GraphicsRuntime>>,
    mut window: Local<(usize, usize)>,
    mut screens: Query<&mut OrigScreen>,
) {
    if !config.enabled() {
        return;
    }
    let Some(mut s) = screens.iter_mut().find(|s| s.id == ScreenId::Options) else {
        return;
    };
    let open = state.as_ref().is_some_and(|st| st.open);
    s.shown = open;
    let (Some(state), Some(settings), Some(runtime)) = (state, settings, runtime) else {
        return;
    };
    if !open {
        return;
    }
    let pages = graphics_screen::visible_pages();
    let page_index = state.page.min(pages.len() - 1);
    let page = pages[page_index];
    let rows = graphics_screen::rows_of(page);
    // A page change starts at its top; moving the cursor keeps the window until the selection
    // leaves the eight slots.
    if window.0 != page_index {
        *window = (page_index, 0);
    }
    window.1 = options_scroll(state.selected, window.1, rows.len());
    let scroll = window.1;
    for slot in 0..OPTION_ROW_PARTS {
        let part = format!("L_settings_list_{slot:02}");
        let Some(row) = rows.get(scroll + slot).copied().filter(|_| slot < OPTION_FULL_ROWS) else {
            s.set_visible(&part, false);
            continue;
        };
        s.set_visible(&part, true);
        let selected = scroll + slot == state.selected;
        let (name, value) = graphics_screen::row_name_value(
            row,
            page,
            pages.len(),
            page_index,
            &settings,
            &runtime.startup,
            &runtime.overridden,
        );
        s.set_in(&part, "T_heading_00", |o| o.text = Some(name));
        let Some(inst) = s.scene.part_instance(&part) else { continue };
        // The game's selection state: heading dark on the highlighted row, boxes otherwise.
        s.freeze(
            inst,
            if selected { "select_state" } else { "unselect_state" },
            0.0,
            "sel",
        );
        if let Some(ratio) = graphics_screen::slider_ratio(row, &settings) {
            s.set_in(&part, "N_type_select_00", |o| o.visible = Some(false));
            s.set_in(&part, "N_switch_slider_00", |o| o.visible = Some(true));
            for k in 0..3 {
                s.set_in(&part, &format!("N_switch_3_0{k}"), |o| {
                    o.visible = Some(false)
                });
            }
            // The value box is the row's own pane; the gauge is a nested part.
            s.set_in(&part, "T_slider_current_param", |o| o.text = Some(value.clone()));
            if let Some(node) = s.node_in_instance(inst, "L_option_slider_00") {
                if let Some(slider) = s.scene.instance_of_node(node) {
                    // `control_slider` frame 0 is the full bar and each step drops a tenth;
                    // the port maps the value's ratio onto that (one step of headroom at the
                    // full end; the art cannot empty completely).
                    let len = s.anim_len(slider, "control_slider").unwrap_or(10.0);
                    let frame = (((1.0 - ratio) * (len - 1.0)).round() + 1.0)
                        .clamp(0.0, (len - 1.0).max(0.0));
                    s.freeze(slider, "control_slider", frame, "gauge");
                }
            }
        } else {
            s.set_in(&part, "N_type_select_00", |o| o.visible = Some(true));
            s.set_in(&part, "N_switch_slider_00", |o| o.visible = Some(false));
            s.set_in(&part, "N_switch_3_00", |o| o.visible = Some(true));
            s.set_in(&part, "N_switch_3_01", |o| o.visible = Some(false));
            s.set_in(&part, "N_switch_3_02", |o| o.visible = Some(false));
            // All three text-colour variants carry the value; the visible one is always 00
            // (the framed box), so switching variants never blanks the text.
            for v in 0..3 {
                s.set_in(&part, &format!("T_option_heading_0{v}"), |o| {
                    o.text = Some(value.clone())
                });
            }
        }
    }
    // Footer: the selected option's help and the keys, in the port's own words (the game's own
    // key guide is a separate layout and is not drawn).
    let about = match rows.get(state.selected).copied() {
        Some(graphics_screen::Row::Option(i)) => graphics_screen::OPTION_DEFS[i].help,
        Some(graphics_screen::Row::Preset) => {
            "Presets set every quality option at once; the look, time, weather, v-sync and FPS cap stay yours."
        }
        _ => "",
    };
    let more_up = if scroll > 0 { "▲ more above   " } else { "" };
    let more_down = if scroll + OPTION_FULL_ROWS < rows.len() {
        "▼ more below   "
    } else {
        ""
    };
    let info = format!(
        "Page {}/{}   {more_up}{more_down}Up/Down select, Left/Right change, Tab page, Esc back",
        page_index + 1,
        pages.len()
    );
    s.set_free_texts(vec![
        free_text(
            223.0,
            790.0,
            1474.0,
            64.0,
            26.0,
            [0.95, 0.93, 0.85, 1.0],
            TextAlign::Left,
            about,
        ),
        free_text(
            223.0,
            856.0,
            1474.0,
            40.0,
            22.0,
            [0.72, 0.75, 0.78, 1.0],
            TextAlign::Left,
            info,
        ),
    ]);
}

// --------------------------------------------------------------------------------------- bag

/// Grid cells of the game's inventory layout (6 × 8 buttons the game places at run time).
pub const BAG_CELLS: usize = 48;
/// Cells the bag shows per page: the 5 × 4 slot circles of `bag_top_00`.
pub const BAG_PAGE: usize = 20;

/// Port names of the bag pockets by the item table's pocket code (LIKELY: the codes group balls
/// (2), medicine (6) and berries (7) in the decoded table; the other names are the port's
/// reading of their contents).
pub fn pocket_name(code: u8) -> &'static str {
    match code {
        0 => "Items",
        1 => "Other items",
        2 => "Poké Balls",
        4 => "Boosters",
        6 => "Medicine",
        7 => "Berries",
        _ => "Pocket",
    }
}

/// Description of the selected item: its effect, stack and prices (the port's own words).
pub fn bag_description(item: &crate::battle_engine::items::ItemEffect, count: u16) -> String {
    let price = if item.price == 0 {
        "Not sold in shops.".to_string()
    } else {
        format!(
            "Price {} (sells for {}).",
            item.price,
            crate::game::shop::Shop::sell_unit_price(item)
        )
    };
    format!(
        "{}. You have {count} (up to {}). {price}",
        label(item),
        item.max_quantity
    )
}

/// Translation that puts the centre of `node` on screen point `target` (layout pixels), from
/// the last evaluated frame: the parent's centre is the anchor of a centre-anchored child
/// without rotation or scale (the case of the grid buttons). `None` before the first frame.
pub fn translate_onto(s: &OrigScreen, node: usize, target: Vec2) -> Option<[f32; 2]> {
    let last = s.last.as_ref()?;
    let parent = s.scene.nodes[node].parent?;
    let pc = last.geoms.get(parent)?.center;
    Some([target.x - pc.x, pc.y - target.y])
}

/// Screen centres (layout pixels) of the panes named `{prefix}{k:02}` for k in 0..n, from the
/// last evaluated frame (the slot circles the game draws under its buttons).
pub fn slot_centres(s: &OrigScreen, prefix: &str, n: usize) -> Option<Vec<Vec2>> {
    let last = s.last.as_ref()?;
    (0..n)
        .map(|k| {
            let node = s.scene.find(&format!("{prefix}{k:02}"))?;
            Some(last.geoms.get(node)?.center)
        })
        .collect()
}

/// Fill grid cell `k` (`L_btn_main_kk` of the inventory grid instance `grid`): place it at `pos`
/// (layout pixels, the grid's own placement is run-time data), show `content` (icon, label) or
/// an empty slot, and the game's select / unselect state.
#[allow(clippy::too_many_arguments)]
fn fill_cell(
    s: &mut OrigScreen,
    grid: usize,
    k: usize,
    pos: Option<[f32; 2]>,
    scale: f32,
    shown: bool,
    content: Option<(Option<ExternalImage>, String)>,
    selected: bool,
    badge: Option<String>,
) {
    let Some(node) = s.scene.find_in(grid, &format!("L_btn_main_{k:02}")) else {
        return;
    };
    let Some(inst) = s.scene.instance_of_node(node) else {
        return;
    };
    s.set_node(node, |o| {
        o.visible = Some(shown);
        if pos.is_some() {
            o.translate = pos;
        }
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
    let has_badge = badge.is_some();
    set(s, "T_00", &|o| {
        o.visible = Some(has_badge);
        o.text = badge.clone();
    });
    for hidden in [
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
    let label = label.filter(|l| !l.is_empty());
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
    // Buttons sit on the game's own slot circles (`P_deoc_item_blank_kk`, 5 x 4).
    let slots = slot_centres(&s, "P_deoc_item_blank_", BAG_PAGE);
    if let Some(grid) = s.scene.part_instance("L_inventory_parts_00") {
        for k in 0..BAG_CELLS {
            let idx = page * BAG_PAGE + k;
            let cell = (k < BAG_PAGE)
                .then(|| entries.get(idx))
                .flatten()
                .map(|&(id, count)| (icons.item_or_dummy(id), format!("x{count}")));
            let shown = k < BAG_PAGE && cell.is_some() && slots.is_some();
            let pos = slots.as_ref().and_then(|c| {
                let node = s.scene.find_in(grid, &format!("L_btn_main_{k:02}"))?;
                translate_onto(&s, node, *c.get(k)?)
            });
            // An item without an exported icon shows its number on the button instead.
            let badge = cell
                .as_ref()
                .filter(|(i, _)| i.is_none())
                .and_then(|_| entries.get(idx))
                .map(|(id, _)| format!("#{id}"));
            fill_cell(&mut s, grid, k, pos, 1.0, shown, cell, idx == ui.selected, badge);
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
        Some((item, count)) => (label(item), bag_description(item, count)),
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
        o.text = Some(pocket_name(pocket).to_string())
    });
    s.set_in("L_tab_disp_01", "T_text_00", |o| {
        o.text = Some(format!(
            "{} / {}  ({n})",
            ui.pocket.min(pockets.len().saturating_sub(1)) + 1,
            pockets.len()
        ))
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

/// Pitch of the box grid buttons (layout pixels): the party rows' authored pitch, also used by
/// the game's grid anchors (`N_inventory_00` / `N_inventory_01`) — PROBABLE.
pub const BOX_PITCH: f32 = 142.0;
/// First inventory-grid cell used for the party column (after the 30 box cells).
const PARTY_CELL0: usize = 36;

/// Pokémon boxes (`box_top_00`): the current box's 30 Pokémon as the game's grid buttons
/// (6 x 5 from the grid anchor `N_inventory_00`, icon + level), the party as a column of buttons
/// on the anchor `N_inventory_01` beside the game's party rows (name + level), the selected
/// button highlighted, a held Pokémon (Enter) dimmed, page and messages in the game's message
/// strip. Neighbouring grids, the all-box overview, the trade tab and the status panel are
/// hidden.
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
    let anchor = |s: &OrigScreen, name: &str| -> Option<Vec2> {
        let n = s.scene.find_in(0, name)?;
        Some(s.last.as_ref()?.geoms.get(n)?.center)
    };
    let box_anchor = anchor(&s, "N_inventory_00");
    let party_anchor = anchor(&s, "N_inventory_01");
    if let Some(grid) = s.scene.part_instance("L_inventory_parts_00") {
        for k in 0..BAG_CELLS {
            let node = s.scene.find_in(grid, &format!("L_btn_main_{k:02}"));
            if k < SLOTS_PER_BOX {
                let (col, row) = ((k % GRID_COLUMNS) as f32, (k / GRID_COLUMNS) as f32);
                let index = base + k;
                let cell = progress.box_list.get(index).map(|m| {
                    (
                        icons.pokemon(m.species, m.form as u16),
                        format!("Lv{}", m.level),
                    )
                });
                let pos = box_anchor.zip(node).and_then(|(a, n)| {
                    translate_onto(&s, n, a + Vec2::new(col, row) * BOX_PITCH)
                });
                let selected = st.focus == BoxFocus::Box && k == st.selected_slot;
                let held = st.held == Some(BoxSelection::Box(index));
                fill_cell(&mut s, grid, k, pos, 0.92, pos.is_some(), cell, selected, None);
                dim_held(&mut s, grid, k, held);
            } else if (PARTY_CELL0..PARTY_CELL0 + 6).contains(&k) {
                let i = k - PARTY_CELL0;
                let cell = progress
                    .party
                    .get(i)
                    .map(|m| (icons.pokemon(m.species, m.form as u16), String::new()));
                let shown = cell.is_some();
                let pos = party_anchor.zip(node).and_then(|(a, n)| {
                    translate_onto(&s, n, a + Vec2::new(0.0, i as f32 * BOX_PITCH))
                });
                let selected = st.focus == BoxFocus::Party && i == st.selected_party;
                let held = st.held == Some(BoxSelection::Party(i));
                fill_cell(&mut s, grid, k, pos, 0.92, shown && pos.is_some(), cell, selected, None);
                dim_held(&mut s, grid, k, held);
            } else {
                fill_cell(&mut s, grid, k, None, 1.0, false, None, false, None);
            }
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
    let msg = if st.message.is_empty() {
        match st.held {
            Some(_) => "Choose where to put it and press Enter.".to_string(),
            None => format!(
                "Box {} / {}   ({} stored)",
                st.page + 1,
                crate::game::box_screen::page_count(),
                progress.box_list.len()
            ),
        }
    } else {
        st.message.clone()
    };
    s.set_text("T_msg_guide_00", msg);
    s.set("N_msg_guide_00", |o| {
        o.visible = Some(true);
        o.alpha = Some(255.0);
    });
    let lines = vec![
        // The box name sits in the game's name strip between L and R.
        free_text(
            660.0,
            70.0,
            200.0,
            44.0,
            32.0,
            [0.95, 0.93, 0.85, 1.0],
            TextAlign::Center,
            format!("Box {}", st.page + 1),
        ),
        free_text(
            60.0,
            1020.0,
            1800.0,
            44.0,
            26.0,
            [0.92, 0.9, 0.82, 1.0],
            TextAlign::Left,
            "Arrows: move   Tab: party / box   Enter: pick / place   D: store   W: withdraw   X X: release   PgUp / PgDn: box   Esc: close",
        ),
    ];
    s.set_free_texts(lines);
}

/// A held button (picked with Enter, waiting for its destination) is drawn half transparent.
fn dim_held(s: &mut OrigScreen, grid: usize, k: usize, held: bool) {
    let Some(node) = s.scene.find_in(grid, &format!("L_btn_main_{k:02}")) else {
        return;
    };
    s.set_node(node, |o| o.alpha = held.then_some(110.0));
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
            "T_param_like_00",
        ] {
            set0(&mut s, pane, None);
        }
        // The port's own data once seen: type badges and base stats (in the game's item list).
        let species = (state != DexVisibility::Hidden)
            .then(|| assets.and_then(|a| a.data.species_row_by_dex_form(e.id, 0).ok().flatten()))
            .flatten();
        let mut types: Vec<String> = Vec::new();
        if let Some(sp) = &species {
            for t in &sp.types {
                let t = title_case(t);
                if !t.is_empty() && !types.contains(&t) {
                    types.push(t);
                }
            }
        }
        for (k, pane) in ["P_icon_type_00", "P_icon_type_01"].iter().enumerate() {
            let img = types.get(k).and_then(|t| icons.type_text(t));
            if let Some(n) = s.node_in_instance(0, pane) {
                s.set_node(n, |o| {
                    o.visible = Some(img.is_some());
                    o.image = img;
                });
            }
        }
        let stats: Vec<String> = match &species {
            Some(sp) => vec![
                format!("HP {}   Atk {}", sp.hp, sp.attack),
                format!("Def {}   Sp.A {}", sp.defense, sp.special_attack),
                format!("Sp.D {}   Spe {}", sp.special_defense, sp.speed),
                format!(
                    "Total {}",
                    u32::from(sp.hp)
                        + u32::from(sp.attack)
                        + u32::from(sp.defense)
                        + u32::from(sp.special_attack)
                        + u32::from(sp.special_defense)
                        + u32::from(sp.speed)
                ),
            ],
            None => Vec::new(),
        };
        set0(
            &mut s,
            "T_h_item_00",
            (!stats.is_empty()).then(|| "Base stats".to_string()),
        );
        for k in 0..5 {
            set0(&mut s, &format!("T_param_item_0{k}"), stats.get(k).cloned());
        }
        for pane in [
            "P_icon_gender_m",
            "P_icon_gender_f",
            "P_icon_rare_01",
            "P_icon_bspoke_01",
            "P_btn_icon_rare_00",
            "P_btn_icon_rare_01",
        ] {
            if let Some(n) = s.node_in_instance(0, pane) {
                s.set_node(n, |o| o.visible = Some(false));
            }
        }
    }
    // Counters at the top: seen, caught and completed entries (the port has no research
    // levels, so "completed" is 0) drawn with the game's own digit textures.
    let counts = [progress.seen.len(), progress.caught.len(), 0];
    for (part, value) in ["L_p_ur_00", "L_p_ur_01", "L_p_cmp_00"].iter().zip(counts) {
        let Some(inst) = s.scene.part_instance(part) else {
            continue;
        };
        let value = value.min(999);
        for (digit_part, d) in [
            ("L_num_b_35pt_100", value / 100),
            ("L_num_b_35pt_010", value / 10 % 10),
            ("L_num_b_35pt_000", value % 10),
        ] {
            let Some(dn) = s.node_in_instance(inst, digit_part) else {
                continue;
            };
            let Some(di) = s.scene.instance_of_node(dn) else {
                continue;
            };
            let img = archive_image(&s, &format!("num_35pt_b_{d}^t"));
            if let Some(n) = s.node_in_instance(di, "P_num_b_35pt_00") {
                s.set_node(n, |o| o.image = img);
            }
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

/// Highlight slot (`L_area_vfx_kk` / `L_icon_kk` of `map_top_00`) of a main area. The slots sit
/// over the Hisui map in the areas' places: Obsidian Fieldlands (area 1) south, Crimson
/// Mirelands (2) south-east, Cobalt Coastlands (3) east, Coronet Highlands (4) centre, Alabaster
/// Icelands (5) north, Jubilife Village (0) west — read from the slot positions on the map
/// picture: PROBABLE. Slot 6 (between the centre and the east coast) is not mapped.
/// Authored x (layout pixels from the centre) of each highlight slot.
pub const MAP_SLOT_X: [f32; 7] = [-220.0, 402.0, 685.0, 23.0, -150.0, -369.0, 258.0];

pub fn map_area_slot(area: u8) -> Option<usize> {
    match area {
        1..=5 => Some(usize::from(area) - 1),
        0 => Some(5),
        _ => None,
    }
}

/// Full-screen map (`M`): the game's Hisui map (`map_top_00`) is drawn as the background with
/// the current area's own highlight (the game's shape tinted by its multiply blend and the teal
/// additive outline) and its place tag; the port's own area map with the player marker stays in
/// front, without its dark backdrop. Quest icons and the other areas' highlights are hidden.
pub fn bind_map_screen(
    config: Res<OrigUiConfig>,
    state: Res<State<GameState>>,
    minimap: Option<Res<MinimapState>>,
    mut screens: Query<&mut OrigScreen>,
    mut overlay: Query<(&mut BackgroundColor, &mut Node), With<FullMapRoot>>,
) {
    if !config.enabled() {
        return;
    }
    let Some(mut s) = screens.iter_mut().find(|s| s.id == ScreenId::Map) else {
        return;
    };
    let open = minimap.as_ref().is_some_and(|m| m.open) && *state.get() == GameState::World;
    s.shown = open;
    let current = minimap
        .as_ref()
        .and_then(|m| m.area)
        .and_then(map_area_slot);
    // The port's area map moves to the side away from the current area's highlight, so the
    // highlight on the Hisui map stays visible (centred again when the map closes).
    let side = match current.map(|k| MAP_SLOT_X[k]) {
        Some(x) if open && x > 0.0 => AlignItems::FlexStart,
        Some(_) if open => AlignItems::FlexEnd,
        _ => AlignItems::Center,
    };
    for (mut bg, mut node) in &mut overlay {
        let want = if open {
            Color::NONE
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.72)
        };
        if bg.0 != want {
            bg.0 = want;
        }
        let pad = if side == AlignItems::Center {
            UiRect::default()
        } else {
            UiRect::horizontal(Val::Vw(4.0))
        };
        if node.align_items != side || node.padding != pad {
            node.align_items = side;
            node.padding = pad;
        }
    }
    if !open {
        return;
    }
    for name in ["N_icon_qicon_vfx_00", "N_deco_name_00", "P_fade_00"] {
        s.set_visible(name, false);
    }
    s.set_visible("N_area_vfx_00", current.is_some());
    s.set_visible("N_icon_00", current.is_some());
    // Every area keeps the game's sepia multiply tint ("unselect"); the current one is lighter
    // and its teal outline pulses ("select" then the looping "select_state"), as when the
    // game's cursor rests on an area.
    for k in 0..7 {
        let here = Some(k) == current;
        s.set_visible(&format!("L_area_vfx_0{k}"), true);
        s.set_visible(&format!("L_icon_0{k}"), here);
        let Some(inst) = s.scene.part_instance(&format!("L_area_vfx_0{k}")) else {
            continue;
        };
        if here {
            s.freeze_end(inst, "select", "sel");
            // The loop's brightest frame, held (the game pulses it).
            s.freeze(inst, "select_state", 0.0, "pulse");

        } else {
            s.freeze_end(inst, "unselect", "sel");
            s.stop(inst, "pulse");
        }
    }
}

/// The game's player arrow on the port's area map (the port's own marker never moves off the
/// map centre) and one marker per fast-travel point of the area (`data/warp_links.tsv`,
/// `ha_areaNN_FTxx` rows).
/// `Some(k)`: fast-travel point k; `None`: the player arrow.
#[derive(Component, Debug)]
pub struct OrigMapMarker(pub Option<usize>);

const MAP_ARROW: &str = "png/gfpak_map/arc/map_app_icon_00/map_icon_location_arrow_00^q.png";
const MAP_POINT: &str = "png/gfpak_map/arc/map_app_icon_00/map_icon_spmap_00^q.png";

/// Percent position of world `(x, z)` on the port's full area map.
pub fn map_percent(x: f32, z: f32, extent: Vec2) -> Vec2 {
    let uv = crate::game::minimap::world_to_uv(x, z, extent);
    (uv * 100.0).clamp(Vec2::ZERO, Vec2::splat(100.0))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_map_markers(
    mut commands: Commands,
    config: Res<OrigUiConfig>,
    minimap: Option<Res<MinimapState>>,
    links: Option<Res<crate::game::zone::ZoneLinks>>,
    player: Query<&Transform, With<crate::game::PlayerAvatar>>,
    panel: Query<&ChildOf, With<crate::game::minimap::FullMapImage>>,
    mut port_marker: Query<
        &mut Visibility,
        (With<crate::game::minimap::FullMapMarker>, Without<OrigMapMarker>),
    >,
    mut markers: Query<(Entity, &OrigMapMarker, &mut Node, &mut UiTransform, &mut Visibility)>,
    mut images: ResMut<Assets<Image>>,
    mut cache: ResMut<super::material::UiTextureCache>,
    mut spawned: Local<Option<(u8, usize)>>,
) {
    if !config.enabled() {
        return;
    }
    let Some(minimap) = minimap.filter(|m| m.open && m.extent.x > 0.0) else {
        return;
    };
    let Some(area) = minimap.area else {
        return;
    };
    for mut v in &mut port_marker {
        v.set_if_neq(Visibility::Hidden);
    }
    let points: Vec<Vec2> = links
        .as_ref()
        .map(|l| {
            let name = format!("ha_area{area:02}");
            l.all()
                .iter()
                .filter(|z| z.kind == crate::game::zone::LinkKind::FastTravel && z.from_area == name)
                .map(|z| Vec2::new(z.pos.x, z.pos.z))
                .collect()
        })
        .unwrap_or_default();
    // (Re)spawn when the area or the number of points changed: one arrow + one per point.
    if *spawned != Some((area, points.len())) {
        let Ok(parent) = panel.single().map(|c| c.parent()) else {
            return;
        };
        for (e, ..) in &markers {
            commands.entity(e).despawn();
        }
        let arrow = cache.get(&mut images, MAP_ARROW);
        let point = cache.get(&mut images, MAP_POINT);
        let mut spawn = |which: Option<usize>, img: Option<Handle<Image>>, size: f32, color: Color| {
            let mut e = commands.spawn((
                OrigMapMarker(which),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Vh(size),
                    height: Val::Vh(size),
                    margin: UiRect {
                        left: Val::Vh(-size / 2.0),
                        top: Val::Vh(-size / 2.0),
                        ..default()
                    },
                    ..default()
                },
                UiTransform::default(),
                Visibility::Inherited,
            ));
            match img {
                Some(h) => {
                    e.insert(ImageNode::new(h).with_color(color));
                }
                None => {
                    e.insert(BackgroundColor(color));
                }
            }
            let id = e.id();
            commands.entity(parent).add_child(id);
        };
        for k in 0..points.len() {
            spawn(Some(k), point.clone(), 2.6, Color::srgb(1.0, 0.85, 0.45));
        }
        spawn(None, arrow, 4.4, Color::WHITE);
        *spawned = Some((area, points.len()));
        return;
    }
    let Ok(t) = player.single() else {
        return;
    };
    let yaw = t.rotation.to_euler(EulerRot::YXZ).0;
    let facing = crate::game::minimap::facing_from_yaw(yaw);
    let rot = crate::game::minimap::marker_rotation_deg(facing.x, facing.y);
    for (_, which, mut node, mut ui, mut vis) in &mut markers {
        let (pos, angle) = match which.0 {
            None => (map_percent(t.translation.x, t.translation.z, minimap.extent), rot),
            Some(i) => match points.get(i) {
                Some(p) => (map_percent(p.x, p.y, minimap.extent), 0.0),
                None => continue,
            },
        };
        let (left, top) = (Val::Percent(pos.x), Val::Percent(pos.y));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        ui.set_if_neq(UiTransform::from_rotation(Rot2::degrees(angle)));
        vis.set_if_neq(Visibility::Inherited);
    }
}
