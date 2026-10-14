//! Battle screens in the original look, bound to the existing [`ActiveBattleSession`]
//! (read only: the battle logic, keys and flow are unchanged).
//!
//! | Original layout | Shows |
//! |---|---|
//! | `battle_top_02` | player plate (name, level, HP gauge and numbers), party balls, Y / + guides |
//! | `cmn_pokeplate_enemy_02` | opponent plate (name, level, HP gauge), placed at the top centre |
//! | `battle_tl_00` | action order (PLA turn flow): next 6 turns with names and Pokémon icons |
//! | `battle_command_03` | Fight (A) / Items + Pokémon (D-pad) / Run (B) diamond while the player chooses |
//! | `battle_command_waza_00` | the 4 rows of the port's grid: moves (type colour, type icon, PP), bag items or party |
//! | `msg_sys_00_s` | battle log (last two lines, the port's own English text) while a turn plays |
//!
//! The port's battle has no separate "choose Fight" step (its fight view shows the moves at
//! once), so the move rows and the command diamond are shown together; the diamond is lifted
//! above the rows and pulses the tab the port highlights.

use bevy::prelude::*;

use super::anim::PlayMode;
use super::icons::IconIndex;
use super::{OrigScreen, OrigUiConfig, OrigUiSystems, ScreenId};
use crate::game::battle_controller::GRID_SLOTS;
use crate::game::{ActiveBattleSession, BattleMenuState, BattleUiRoot, GameState};

pub fn install(app: &mut App) {
    app.init_resource::<BattleUiMemo>();
    app.add_systems(Update, bind_battle.in_set(OrigUiSystems::Bind));
    super::hide_when_shown::<BattleUiRoot>(app, ScreenId::BattleHud);
}

/// What the bindings showed last frame (to start one-shot animations on changes only).
#[derive(Resource, Debug, Default)]
pub struct BattleUiMemo {
    tab: Option<usize>,
}

/// "pikachu" / "PIKACHU" / "quick-attack" -> "Pikachu" / "Quick Attack" (the port's own
/// English names).
pub fn title_case(s: &str) -> String {
    s.split([' ', '-'])
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The player can pick an action now (menus are open).
pub fn awaiting_input(sess: &ActiveBattleSession) -> bool {
    !sess.battle.ended
        && matches!(
            sess.menu_state,
            BattleMenuState::MainMenu
                | BattleMenuState::MoveSelect
                | BattleMenuState::ItemSelect
                | BattleMenuState::SwitchSelect
        )
}

/// HP gauge part (see [`super::set_bar`]).
fn gauge(s: &mut OrigScreen, part: &str, hp: u32, max: u32) {
    let ratio = (hp as f32 / max.max(1) as f32).clamp(0.0, 1.0);
    super::set_bar(s, part, "P_color_00", ratio, hp > 0);
    // The darker "damage" trail follows the bar at once (no trailing animation yet).
    s.set_in(part, "P_color_01", |o| o.visible = Some(false));
    super::gauge_colour(s, part, ratio);
}

fn bind_hud(s: &mut OrigScreen, sess: &ActiveBattleSession) {
    let p = sess.active_player();
    let c = &sess.battle.combatants[p];
    s.set_text("T_pokename_00", title_case(&c.species.name));
    s.set_text("T_lv_00", format!("Lv. {}", c.level));
    s.set_text("T_hp_01", c.hp.to_string());
    s.set_text("T_hp_02", "/");
    s.set_text("T_hp_03", c.max_hp.to_string());
    gauge(s, "L_hp_gauge_00", c.hp, c.max_hp);
    // Not tracked by the port: gender, stat-stage chevrons, target picking.
    s.set_visible("L_seibetsu_00", false);
    s.set_visible("A_alignment_01", false);
    s.set_visible("N_poke_00", false);
    // Alpha (big red-eyed) marker: the port has no alpha Pokémon flag.
    s.set_visible("P_bspoke_00", false);
    let party = sess.battle.sides.first().map_or(0, |v| v.len());
    let foes = sess.battle.sides.get(1).map_or(0, |v| v.len());
    for k in 0..6 {
        let name = format!("L_icon_ball_0{k}");
        s.set_in("L_ball_00", &name, |o| o.visible = Some(k < party));
        s.set_in("L_ball_01", &name, |o| o.visible = Some(k < foes));
    }
    s.set_visible("L_ball_01", sess.trainer.is_some());
}

fn bind_enemy(s: &mut OrigScreen, sess: &ActiveBattleSession) {
    let w = sess.opponent();
    let c = &sess.battle.combatants[w];
    s.set_visible("A_alignment_02", true);
    s.set_text("T_pokename_00", title_case(&c.species.name));
    s.set_visible("N_lv_00", true);
    s.set_text("T_lv_00", format!("Lv. {}", c.level));
    gauge(s, "L_hp_gauge_01", c.hp, c.max_hp);
    s.set_visible("P_rare_00", false);
    s.set_visible("P_bspoke_00", false);
    s.set_visible("L_seibetsu_00", false);
    // The game floats this plate over the opponent; the port pins it at the top centre.
    s.set("all", |o| o.translate = Some([0.0, 330.0]));
}

fn bind_timeline(s: &mut OrigScreen, sess: &ActiveBattleSession, icons: &IconIndex) -> bool {
    if !sess.uses_queue() {
        return false;
    }
    let preview = sess.battle.turn_order_preview(6);
    for k in 0..6 {
        let part = format!("L_icon_0{k}");
        match preview.get(k) {
            Some(e) => {
                let c = &sess.battle.combatants[e.combatant];
                let name = title_case(&c.species.name);
                let ours = e.side == 0;
                s.set_visible(&part, true);
                s.set_in(&part, "T_pokename_00", |o| o.text = Some(name));
                s.set_in(&part, "N_color_00", |o| o.visible = Some(ours));
                s.set_in(&part, "N_color_01", |o| o.visible = Some(!ours));
                let icon = icons.pokemon(c.species.dex, 0);
                s.set_in(&part, "P_pokeicon_00", |o| {
                    o.visible = Some(icon.is_some());
                    o.alpha = Some(255.0);
                    o.size = Some([56.0, 56.0]);
                    o.image = icon;
                });
            }
            None => s.set_visible(&part, false),
        }
    }
    !preview.is_empty()
}

const COMMAND_PARTS: [&str; 4] = [
    "L_btn_attack_00",
    "L_btn_pokemon_00",
    "L_btn_bag_00",
    "L_btn_escape_00",
];

fn bind_command(
    s: &mut OrigScreen,
    sess: &ActiveBattleSession,
    memo: &mut BattleUiMemo,
    rows_shown: bool,
) {
    let tab = sess.highlighted_tab();
    let part = match tab {
        0 => COMMAND_PARTS[0],
        1 => COMMAND_PARTS[1],
        2 | 3 => COMMAND_PARTS[2],
        _ => COMMAND_PARTS[3],
    };
    if memo.tab != Some(tab) {
        memo.tab = Some(tab);
        s.play_part(part, "key_select", PlayMode::Hold, "select");
    }
    // Above the move rows when both are shown (see the module docs).
    let lift = if rows_shown { 330.0 } else { 0.0 };
    s.set("all", |o| o.translate = Some([0.0, lift]));
}

fn strip_slot_prefix(s: &str) -> String {
    match s.find("] ") {
        Some(i) if s.starts_with('[') => s[i + 2..].to_string(),
        _ => s.to_string(),
    }
}

fn bind_rows(s: &mut OrigScreen, sess: &ActiveBattleSession, icons: &IconIndex) {
    let slots = sess.slot_texts();
    let highlight = sess.highlight_slot();
    let moves_view = matches!(
        sess.menu_state,
        BattleMenuState::MainMenu | BattleMenuState::MoveSelect | BattleMenuState::LearnMove
    );
    let p = sess.active_player();
    let c = &sess.battle.combatants[p];
    for k in 0..GRID_SLOTS {
        let part = format!("L_waza_btn_0{k}");
        let Some(inst) = s.scene.part_instance(&part) else {
            continue;
        };
        let (title, detail) = &slots[k];
        let empty = detail == "Empty Slot";
        s.set_visible(&part, !empty);
        if empty {
            continue;
        }
        let name = title_case(&strip_slot_prefix(title));
        s.set_in(&part, "T_command_00", |o| o.text = Some(name));
        let mv = if moves_view && sess.menu_state != BattleMenuState::LearnMove {
            c.moves.get(k)
        } else {
            None
        };
        match mv {
            Some(m) => {
                let (pp, max) = (m.pp.to_string(), m.data.pp.to_string());
                s.set_in(&part, "N_pp_00", |o| o.visible = Some(true));
                s.set_in(&part, "T_pp_00", |o| o.text = Some(pp));
                s.set_in(&part, "T_pp_01", |o| o.text = Some("/".into()));
                s.set_in(&part, "T_pp_02", |o| o.text = Some(max));
                if let Some(t) = super::icons::type_index(&m.data.move_type) {
                    s.freeze(inst, "type", t as f32, "type");
                }
                let icon = icons.type_icon(&m.data.move_type);
                s.set_in(&part, "P_type_icon_00", |o| {
                    o.visible = Some(icon.is_some());
                    o.scale = Some([1.0, 1.0]);
                    o.size = Some([44.0, 44.0]);
                    o.image = icon;
                });
            }
            None => {
                // Items / party / move to forget: the detail goes where the PP numbers sit.
                let d = detail.clone();
                s.set_in(&part, "N_pp_00", |o| o.visible = Some(true));
                s.set_in(&part, "T_pp_00", |o| o.text = None);
                s.set_in(&part, "T_pp_01", |o| o.text = Some(d));
                s.set_in(&part, "T_pp_02", |o| o.text = None);
                // The type diamond shows the item / Pokemon icon instead (neutral colour).
                s.stop(inst, "type");
                let idx = sess.list_cursor / GRID_SLOTS * GRID_SLOTS + k;
                let icon = match sess.menu_state {
                    BattleMenuState::ItemSelect => sess
                        .bag_view()
                        .get(idx)
                        .and_then(|(e, _)| icons.item(e.item_id)),
                    BattleMenuState::SwitchSelect => sess
                        .battle
                        .sides
                        .first()
                        .and_then(|side| side.get(idx))
                        .and_then(|&ci| icons.pokemon(sess.battle.combatants[ci].species.dex, 0)),
                    _ => None,
                };
                s.set_in(&part, "P_type_icon_00", |o| {
                    o.visible = Some(icon.is_some());
                    o.scale = Some([1.0, 1.0]);
                    o.size = Some([56.0, 56.0]);
                    o.image = icon;
                });
            }
        }
        // Selected row: the game's select / unselect animations at rest.
        let tag = if k == highlight { "select" } else { "unselect" };
        s.freeze_end(inst, tag, "sel");
    }
    // Move info panel (left): the selected row's details, in the port's own words.
    let (_, detail) = &slots[highlight.min(GRID_SLOTS - 1)];
    let info = detail.clone();
    s.set_in("L_wazainfo_00", "T_info_00", |o| o.text = Some(info));
    match c.moves.get(highlight).filter(|_| moves_view) {
        Some(m) => {
            let (pow, acc) = (
                if m.data.power > 0 {
                    m.data.power.to_string()
                } else {
                    "---".into()
                },
                if m.data.accuracy > 0 {
                    m.data.accuracy.to_string()
                } else {
                    "---".into()
                },
            );
            s.set_in("L_wazainfo_00", "T_power_00", |o| {
                o.text = Some("Power".into())
            });
            s.set_in("L_wazainfo_00", "T_power_01", |o| o.text = Some(pow));
            s.set_in("L_wazainfo_00", "T_dex_00", |o| {
                o.text = Some("Accuracy".into())
            });
            s.set_in("L_wazainfo_00", "T_dex_01", |o| o.text = Some(acc));
        }
        None => {
            for t in ["T_power_00", "T_power_01", "T_dex_00", "T_dex_01"] {
                s.set_in("L_wazainfo_00", t, |o| o.text = None);
            }
        }
    }
    s.set_visible("L_wazamode_info_00", false);
    s.set_visible("L_waza_ef_00", false);
    let style_hint = if sess.uses_queue() {
        "Styles (Q / E)"
    } else {
        ""
    };
    s.set_text("T_option_01", style_hint);
    s.set_text("T_option_00", "Back (Backspace)");
}

fn bind_message(s: &mut OrigScreen, sess: &ActiveBattleSession) {
    let n = sess.log_lines.len();
    let a = if n >= 2 {
        sess.log_lines[n - 2].clone()
    } else {
        String::new()
    };
    let b = sess.log_lines.last().cloned().unwrap_or_default();
    s.set_text("T_00", a);
    s.set_text("T_01", b);
    s.set_visible("L_timer_00", false);
}

#[allow(clippy::too_many_arguments)]
pub fn bind_battle(
    config: Res<OrigUiConfig>,
    state: Res<State<GameState>>,
    session: Option<Res<ActiveBattleSession>>,
    paused: Option<Res<crate::game::state::Paused>>,
    icons: Res<IconIndex>,
    mut memo: ResMut<BattleUiMemo>,
    mut screens: Query<&mut OrigScreen>,
) {
    if !config.enabled() {
        return;
    }
    // The pause menu (drawn without a full-screen backdrop) replaces the battle screens.
    let paused = paused.is_some_and(|p| p.0);
    let sess = session
        .as_deref()
        .filter(|_| *state.get() == GameState::Battle && !paused);
    let Some(sess) = sess.filter(|s| s.battle.combatants.len() >= 2) else {
        for mut s in &mut screens {
            if matches!(
                s.id,
                ScreenId::BattleHud
                    | ScreenId::BattleEnemyPlate
                    | ScreenId::BattleTimeline
                    | ScreenId::BattleCommand
                    | ScreenId::BattleMoves
                    | ScreenId::BattleMessage
            ) {
                s.shown = false;
            }
        }
        memo.tab = None;
        return;
    };
    let choosing = awaiting_input(sess);
    let rows = choosing || sess.menu_state == BattleMenuState::LearnMove;
    for mut s in &mut screens {
        match s.id {
            ScreenId::BattleHud => {
                s.shown = true;
                bind_hud(&mut s, sess);
            }
            ScreenId::BattleEnemyPlate => {
                s.shown = true;
                bind_enemy(&mut s, sess);
            }
            ScreenId::BattleTimeline => {
                s.shown = bind_timeline(&mut s, sess, &icons);
            }
            ScreenId::BattleCommand => {
                s.shown = choosing;
                if choosing {
                    bind_command(&mut s, sess, &mut memo, rows);
                } else {
                    memo.tab = None;
                }
            }
            ScreenId::BattleMoves => {
                s.shown = rows;
                if rows {
                    bind_rows(&mut s, sess, &icons);
                }
            }
            ScreenId::BattleMessage => {
                s.shown = !rows && !sess.log_lines.is_empty();
                bind_message(&mut s, sess);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_prefixes() {
        assert_eq!(title_case("PIKACHU"), "Pikachu");
        assert_eq!(title_case("mr. mime"), "Mr. Mime");
        assert_eq!(title_case("quick-attack"), "Quick Attack");
        assert_eq!(strip_slot_prefix("[2] Thunder Shock"), "Thunder Shock");
        assert_eq!(strip_slot_prefix("Tackle"), "Tackle");
    }
}
