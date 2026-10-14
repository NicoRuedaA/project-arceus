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
use crate::game::battle_controller::BattleAssets;
use crate::game::party_screen::{PartyScreenRoot, PartyScreenState};
use crate::game::pause_menu::{
    menu_info_text, option_label, MenuOption, PauseMenu, PauseMenuSuppressed, PauseRoot, OPTIONS,
};
use crate::game::play_time::PlayTime;
use crate::game::save_game::PartyMember;
use crate::game::save_wire::PlayerProgress;
use crate::game::settings::UserSettings;
use crate::game::state::Paused;

pub fn install(app: &mut App) {
    app.add_systems(Update, bind_pause_menu.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<PauseRoot>(app, ScreenId::PauseMenu);
    app.add_systems(Update, bind_party_screen.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<PartyScreenRoot>(app, ScreenId::Party);
}

/// The game's menu icon (texture base name) the port uses for each option. Only icons the
/// archive really holds are used (several, e.g. `map`, `box`, `pokedex`, are 2×2 placeholders
/// in this archive and are drawn from other packs by the game).
pub fn option_icon(o: MenuOption) -> &'static str {
    match o {
        MenuOption::Resume | MenuOption::Quit => "help",
        MenuOption::Bag => "bag",
        MenuOption::Party => "pokelist",
        MenuOption::Boxes => "ibox",
        MenuOption::Pokedex | MenuOption::Save | MenuOption::Load => "report",
        MenuOption::Crafting => "making",
        MenuOption::Shop => "buy",
        MenuOption::InvertX | MenuOption::InvertY | MenuOption::Volume => "settings",
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
    let cream = [0.98, 0.96, 0.85, 1.0];
    let gold = [1.0, 0.82, 0.35, 1.0];
    let dim = [0.75, 0.75, 0.72, 1.0];
    let mut texts = Vec::with_capacity(OPTIONS.len() + 2);
    for (i, &o) in OPTIONS.iter().enumerate() {
        let (color, mark) = if i == sel {
            (gold, "> ")
        } else {
            (cream, "   ")
        };
        let y = 150.0 + i as f32 * 52.0;
        texts.push(free_text(
            90.0,
            y,
            760.0,
            48.0,
            32.0,
            color,
            TextAlign::Left,
            format!("{mark}{}", option_label(o, &settings)),
        ));
    }
    let info = menu_info_text(
        progress.as_ref().map_or(0, |p| p.money),
        play.as_ref().map_or(0, |p| p.seconds),
    );
    texts.push(free_text(
        90.0,
        150.0 + OPTIONS.len() as f32 * 52.0 + 20.0,
        900.0,
        44.0,
        26.0,
        dim,
        TextAlign::Left,
        info,
    ));
    texts.push(free_text(
        90.0,
        150.0 + OPTIONS.len() as f32 * 52.0 + 66.0,
        900.0,
        40.0,
        24.0,
        dim,
        TextAlign::Left,
        "Arrows / W S + Enter, Esc to resume",
    ));
    s.set_free_texts(texts);
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
    let open = state.is_some_and(|st| st.open);
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
            let tag = if i == 0 { "select" } else { "unselect" };
            s.freeze_end(inst, tag, "sel");
        }
    }
    // Right panel: the lead member.
    if let Some(lead) = progress.party.first() {
        let (name, _) = member_info(assets, lead);
        s.set("T_name_poke_00", |o| {
            o.text = Some(name);
            o.text_color = Some([0.16, 0.2, 0.26, 1.0]);
        });
        s.set_text("T_situation_00", "Party leader");
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
        "Party order is the stored order. HP 0 means fainted. Esc closes.",
    );
}

