//! Menu screens in the original look, bound to the existing menu state (read only).
//!
//! Pause menu (`slidelist` → `slidemenu_menulist_01`): the game's header bar and its 4-icon tab
//! strip. The port's 15 options are shown 4 per page in the strip (the game's own menu icons,
//! one chosen per option by the port), the highlighted option is the selected tab and its label
//! is the header title; the full option list (with the current settings) and the money / play
//! time line are drawn as the port's own text over the game's dimmed panel. Keys and actions are
//! the existing pause menu's (`pause_menu.rs`).

use bevy::prelude::*;

use super::scene::{ExternalImage, TextAlign};
use super::{free_text, OrigScreen, OrigUiConfig, OrigUiSystems, ScreenId};
use crate::game::pause_menu::{
    menu_info_text, option_label, MenuOption, PauseMenu, PauseMenuSuppressed, PauseRoot, OPTIONS,
};
use crate::game::play_time::PlayTime;
use crate::game::save_wire::PlayerProgress;
use crate::game::settings::UserSettings;
use crate::game::state::Paused;

pub fn install(app: &mut App) {
    app.add_systems(Update, bind_pause_menu.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<PauseRoot>(app, ScreenId::PauseMenu);
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
