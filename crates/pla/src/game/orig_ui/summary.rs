//! Pokémon summary in the original look (`status` / `status_top_00`), bound to
//! [`SummaryState`] and the live party (read only: keys and data stay those of
//! `game::summary_screen`, whose provisional panel is hidden while this one shows).
//!
//! | Page (game tab) | Filled with |
//! |---|---|
//! | Info (`pt_status_scr_basic_00`) | species name, type badges, dex number, nature / ability placeholders, status, experience and the experience bar |
//! | Moves (`pt_status_scr_waza_00`) | four move rows (name, type badge, PP), details of the highlighted move (power, accuracy, category, port text) |
//! | Stats (`pt_status_scr_param_00`) | HP and the five stats with the effort level dial of each |
//! | Notes (`pt_status_scr_memo_00`) | the port's own note text |
//!
//! Name plate: name and level. The Pokémon's 512 px picture stands where the game shows the 3D
//! model. Gender, shiny / alpha marks, language, markings and the stat-polygon are not shown
//! (the port does not track them). The hexagon order of the stats (HP top, then Attack, Defense,
//! Speed, Sp. Def, Sp. Atk clockwise) is the main series convention: PROBABLE.

use bevy::prelude::*;

use super::icons::IconIndex;
use super::scene::{NodeOverride, TextAlign};
use super::{free_image, free_text, OrigScreen, OrigUiConfig, OrigUiSystems, ScreenId};
use crate::game::battle_controller::BattleAssets;
use crate::game::save_wire::PlayerProgress;
use crate::game::summary_screen::{
    memo_text, summary_info, SummaryPage, SummaryRoot, SummaryState, ABILITY_TEXT, NATURE_TEXT,
    STAT_NAMES,
};

pub fn install(app: &mut App) {
    app.add_systems(Update, bind_summary.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<SummaryRoot>(app, ScreenId::Summary);
    // Opened from the party list: the provisional list underneath stays hidden too.
    super::hide_when_shown::<crate::game::party_screen::PartyScreenRoot>(app, ScreenId::Summary);
}
/// Hexagon slot (`N_param_0k`, clockwise from the top) -> index into the stat array.
pub const HEX_STAT: [usize; 6] = [0, 1, 2, 5, 4, 3];

/// Centre of the Pokémon picture (`N_pokemon_picture`, layout pixels, top-left origin).
const PICTURE_CENTER: Vec2 = Vec2::new(1425.0, 520.0);
const PICTURE_SIZE: f32 = 640.0;

fn set_in_inst(s: &mut OrigScreen, inst: usize, pane: &str, f: impl FnOnce(&mut NodeOverride)) {
    if let Some(n) = s.node_in_instance(inst, pane) {
        s.set_node(n, f);
    }
}

fn text_in(s: &mut OrigScreen, inst: usize, pane: &str, text: Option<String>) {
    set_in_inst(s, inst, pane, |o| {
        o.text = text;
        o.visible = Some(true);
    });
}

/// Instance of the first part named `part` inside instance `outer`.
fn sub_part(s: &OrigScreen, outer: usize, part: &str) -> Option<usize> {
    let n = s.node_in_instance(outer, part)?;
    s.scene.instance_of_node(n)
}

#[allow(clippy::too_many_arguments)]
pub fn bind_summary(
    config: Res<OrigUiConfig>,
    state: Option<Res<SummaryState>>,
    progress: Option<Res<PlayerProgress>>,
    assets: Option<Res<BattleAssets>>,
    icons: Res<IconIndex>,
    mut screens: Query<&mut OrigScreen>,
) {
    if !config.enabled() {
        return;
    }
    let Some(mut s) = screens.iter_mut().find(|s| s.id == ScreenId::Summary) else {
        return;
    };
    let member = state.as_ref().filter(|st| st.open).and_then(|st| {
        progress
            .as_ref()
            .and_then(|p| p.party.get(st.member).cloned())
    });
    s.shown = member.is_some();
    let (Some(m), Some(st)) = (member, state) else {
        return;
    };
    let party_len = progress.as_ref().map_or(1, |p| p.party.len());
    let info = summary_info(assets.as_deref(), &m);
    let page = st.page;

    // Page: the game's own keep animations fade one page group in and the others out.
    s.freeze(0, &format!("keep_scr_0{}", page.index() + 1), 0.0, "page");
    if let Some(slide) = s.scene.part_instance("L_pt_slide") {
        for k in 0..4 {
            if let Some(i) = sub_part(&s, slide, &format!("L_sel_icon_0{k}")) {
                let tag = if k == page.index() { "select" } else { "unselect" };
                s.freeze_end(i, tag, "sel");
            }
        }
    }
    // Only the current page's part is shown (its group fade alone leaves the texts visible).
    for (k, part) in ["L_scr_01_basic", "L_scr_01_waza", "L_scr_03_param", "L_scr_04_memo"]
        .iter()
        .enumerate()
    {
        s.set_visible(part, k == page.index());
    }
    s.set_visible("L_pt_mark_00", false);
    s.set_visible("N_pt_mark_window", false);

    // Name plate.
    if let Some(p) = s.scene.part_instance("L_pt_poke_name_00") {
        text_in(&mut s, p, "T_pokemon_name_00", Some(info.name.clone()));
        text_in(&mut s, p, "T_pokemon_level_00", Some(format!("Lv. {}", info.level)));
        for hidden in ["N_gender_00", "N_sign_rare_anim_00", "N_bspoke_00", "N_icon_lang_00"] {
            set_in_inst(&mut s, p, hidden, |o| o.visible = Some(false));
        }
        let more = party_len > 1;
        for b in ["L_btn_01_up", "L_btn_01_down"] {
            set_in_inst(&mut s, p, b, |o| o.visible = Some(more));
        }
    }

    // Info page.
    if let Some(b) = s.scene.part_instance("L_scr_01_basic") {
        if let Some(head) = sub_part(&s, b, "L_pt_bg_basic_00") {
            let species = assets
                .as_deref()
                .and_then(|a| a.data.species_row_by_dex_form(m.species, m.form).ok().flatten())
                .map(|sp| super::battle::title_case(&sp.name))
                .unwrap_or_else(|| info.name.clone());
            text_in(&mut s, head, "T_pokemon_name_03", Some(species));
            for (k, pane) in ["P_icon_poke_type_00", "P_icon_poke_type_01"].iter().enumerate() {
                let img = info.types.get(k).and_then(|t| icons.type_text(t));
                set_in_inst(&mut s, head, pane, |o| {
                    o.visible = Some(img.is_some());
                    o.image = img;
                });
            }
        }
        // Rows from the top: the layout lists them bottom-up.
        let rows = [
            ("T_weight_03", "T_status_weight_04", "Pokédex No.".to_string(), format!("{:03}", info.dex)),
            ("T_weight_01", "T_status_weight_02", "Nature".into(), NATURE_TEXT.into()),
            ("T_parent_00", "T_parent_01", "Ability".into(), ABILITY_TEXT.into()),
            ("T_id_00", "T_id_01", "Condition".into(), info.status.into()),
        ];
        for (label, value, l, v) in rows {
            text_in(&mut s, b, label, Some(l));
            text_in(&mut s, b, value, Some(v));
        }
        text_in(&mut s, b, "T_now_exp_00", Some("Exp. Points".into()));
        text_in(&mut s, b, "T_next_exp_00", Some("To next level".into()));
        text_in(&mut s, b, "T_next_exp_01", None);
        let (now, next) = match (info.exp, info.exp_to_next) {
            (Some(e), Some(n)) => (e.to_string(), n.to_string()),
            _ => ("---".into(), "---".into()),
        };
        text_in(&mut s, b, "T_now_exp_01", Some(now));
        text_in(&mut s, b, "T_next_exp_03", Some(next));
        if let Some(bar) = sub_part(&s, b, "L_pt_exp_01") {
            // The game's own bar animation, one frame per percent (PROBABLE).
            s.freeze(bar, "gauge_scale", (info.exp_ratio * 100.0).round(), "gauge");
        }
    }

    // Moves page.
    if let Some(w) = s.scene.part_instance("L_scr_01_waza") {
        let cursor = st.move_cursor.min(info.moves.len().saturating_sub(1));
        for k in 0..5 {
            let Some(row) = sub_part(&s, w, &format!("L_pt_waza_info_0{k}")) else {
                continue;
            };
            let mv = info.moves.get(k).filter(|_| k < 4);
            let part_node = s.scene.instances[row].part_node;
            if let Some(n) = part_node {
                s.set_node(n, |o| o.visible = Some(mv.is_some()));
            }
            let Some(mv) = mv else {
                continue;
            };
            text_in(&mut s, row, "T_waza_name_00", Some(mv.name.clone()));
            text_in(&mut s, row, "T_waza_name_01", None);
            let pp = match mv.max_pp {
                Some(x) => format!("{}/{x}", mv.pp),
                None => mv.pp.to_string(),
            };
            text_in(&mut s, row, "T_ui_pokelist_pp", Some(pp));
            let badge = icons.type_text(&mv.move_type);
            set_in_inst(&mut s, row, "P_icon_poke_type_00", |o| {
                o.visible = Some(badge.is_some());
                o.image = badge;
            });
            set_in_inst(&mut s, row, "N_kdw_00", |o| o.visible = Some(false));
            let selected = page == SummaryPage::Moves && k == cursor;
            s.freeze_end(row, if selected { "select" } else { "unselect" }, "sel");
        }
        let (power, accuracy, category, text) = match info.moves.get(cursor) {
            Some(mv) => (
                if mv.power > 0 { mv.power.to_string() } else { "---".into() },
                if mv.accuracy > 0 { mv.accuracy.to_string() } else { "---".into() },
                mv.category.clone(),
                format!(
                    "{}: {} move, {} category.",
                    mv.name,
                    if mv.move_type.is_empty() { "unknown" } else { &mv.move_type },
                    if mv.category.is_empty() { "unknown" } else { &mv.category },
                ),
            ),
            None => ("---".into(), "---".into(), String::new(), "No moves.".into()),
        };
        text_in(&mut s, w, "T_skill_category_00", Some("Category".into()));
        text_in(&mut s, w, "T_skill_power_00", Some("Power".into()));
        text_in(&mut s, w, "T_skill_hit_00", Some("Accuracy".into()));
        text_in(&mut s, w, "T_skill_power_01", Some(power));
        text_in(&mut s, w, "T_skill_power_01_2", Some(accuracy));
        text_in(&mut s, w, "T_skill_hit_02", Some(text));
        if let Some(cat) = sub_part(&s, w, "L_icon_bunrui") {
            // Physical / special / status icon frames of the game's `change` animation (PROBABLE).
            let frame = match category.as_str() {
                "Physical" => 0.0,
                "Special" => 1.0,
                _ => 2.0,
            };
            s.freeze(cat, "change", frame, "category");
        }
        // The game slides the details panel in when a move is picked; the port shows it at rest.
        s.freeze_end(w, "detaile_in", "detail");
        for g in ["N_guide_00", "N_guide_01"] {
            set_in_inst(&mut s, w, g, |o| o.visible = Some(false));
        }
    }

    // Stats page.
    if let Some(p) = s.scene.part_instance("L_scr_03_param") {
        if let Some(graph) = sub_part(&s, p, "L_pt_graph") {
            for (slot, &stat) in HEX_STAT.iter().enumerate() {
                if let Some(title) = sub_part(&s, graph, &format!("L_param_0{slot}")) {
                    text_in(&mut s, title, "T_param_name", Some(STAT_NAMES[stat].into()));
                    set_in_inst(&mut s, title, "L_graph_sign", |o| o.visible = Some(false));
                    if let Some(dial) = sub_part(&s, title, "L_lv_gauge") {
                        let effort = info.effort[stat];
                        text_in(&mut s, dial, "T_lv_numb", Some(effort.to_string()));
                        // Effort levels 0-10 on the dial's 0-100 animation (PROBABLE).
                        s.freeze(dial, "gauge", f32::from(effort.min(10)) * 10.0, "gauge");
                    }
                }
                let value = info.stats.map(|v| v[stat]);
                if stat == 0 {
                    let max = value.map_or("---".into(), |v| v.to_string());
                    text_in(&mut s, graph, "T_param_00_numb_0", Some(info.hp.to_string()));
                    text_in(&mut s, graph, "T_param_00_numb_1", Some("/".into()));
                    text_in(&mut s, graph, "T_param_00_numb_2", Some(max));
                } else {
                    text_in(
                        &mut s,
                        graph,
                        &format!("T_param_0{slot}_numb"),
                        Some(value.map_or("---".into(), |v| v.to_string())),
                    );
                }
            }
        }
    }

    // Notes page.
    if let Some(n) = s.scene.part_instance("L_scr_04_memo") {
        text_in(&mut s, n, "T_trmemo_01_01", Some("Notes".into()));
        text_in(&mut s, n, "T_trmemo_03_01_00", Some(memo_text(&info)));
    }

    // The Pokémon picture where the game shows the model.
    let pic = icons.pokemon_large(m.species, u16::from(m.form));
    s.set_free_quads(
        pic.map(|img| {
            free_image(
                &img.png,
                img.uv,
                PICTURE_CENTER,
                Vec2::splat(PICTURE_SIZE),
                [1.0; 4],
            )
        })
        .into_iter()
        .collect(),
    );
    let hint = if page == SummaryPage::Moves {
        "Left / Right: page   Up / Down: move   PgUp / PgDn: Pokémon   Esc: back"
    } else {
        "Left / Right: page   Up / Down: Pokémon   Esc: back"
    };
    s.set_free_texts(vec![
        free_text(
            100.0,
            70.0,
            700.0,
            44.0,
            30.0,
            [0.95, 0.93, 0.85, 1.0],
            TextAlign::Left,
            format!("{}   ({} / {party_len})", page.title(), st.member + 1),
        ),
        free_text(
            60.0,
            1020.0,
            1800.0,
            44.0,
            26.0,
            [0.92, 0.9, 0.82, 1.0],
            TextAlign::Left,
            hint,
        ),
    ]);
}
