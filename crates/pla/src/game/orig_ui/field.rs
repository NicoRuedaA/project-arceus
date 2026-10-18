//! Field HUD in the original look (`fieldinfo_00` of `resident_hud_arc01_eng`), bound to the
//! existing world state (read only).
//!
//! | Part of the layout | Shows (port data, port text) |
//! |---|---|
//! | `L_zninfo_00` (top-left research bar) | nearest wild Pokémon: icon and name (else the zone name) |
//! | `L_item_00` (ball shortcut, right) | selected ball name and count in the bag |
//! | `L_command_00` (button guides, bottom left) | the game's menu / Pokédex / map guide icons, moved right of the port's minimap |
//!
//! While the field selector is open (`X`, see [`super::field_select`]) the resting ball shortcut
//! is hidden — the wheel at the bottom right shows the same item larger, and the game does not
//! draw the shortcut twice (PROBABLE: the captures of the original show only the wheel). The
//! bottom-centre line names that key while the wheel is closed (the port's own hint text).
//!
//! Hidden because the port has no such data yet: requests / missions (`L_mssn_00`), the noble
//! gauge and timer (`L_gauge_nspoke_01`, `L_ns_time_00`), the alpha / outbreak notices
//! (`L_as_00`, `L_wnhd_00`), the Pokémon-throw panel (`L_item_pokemon_00`) and the second / third
//! item slots. The provisional debug cards (zone / position, nearby radar) are hidden while this
//! HUD is shown; the port's minimap and compass stay.

use std::collections::HashMap;

use bevy::prelude::*;

use super::battle::title_case;
use super::field_select::FieldSelector;
use super::icons::IconIndex;
use super::scene::TextAlign;
use super::{free_text, OrigScreen, OrigUiConfig, OrigUiSystems, ScreenId};
use crate::game::battle_controller::BattleAssets;
use crate::game::save_wire::PlayerProgress;
use crate::game::state::Paused;
use crate::game::throwing::BallInventory;
use crate::game::{
    ActiveZoneInfo, GameState, PlayerAvatar, WildCreature, WorldRadarText, WorldZoneText,
};

pub fn install(app: &mut App) {
    app.add_systems(Update, bind_field_hud.in_set(OrigUiSystems::Bind));
    app.add_systems(
        PostUpdate,
        hide_debug_cards
            .after(super::render_screens)
            .in_set(OrigUiSystems::Render),
    );
}

/// Port text dark enough for the light research bar.
const BAR_TEXT: [f32; 4] = [0.16, 0.2, 0.26, 1.0];

/// Zone name without the internal id (`"Obsidian Fieldlands (ha_area01)"` -> `"Obsidian Fieldlands"`).
pub fn zone_label(name: &str) -> String {
    match name.rfind(" (") {
        Some(i) if name.ends_with(')') => name[..i].to_string(),
        _ => name.to_string(),
    }
}

/// The provisional world HUD's debug cards (zone / position, nearby radar) are hidden while the
/// original field HUD is shown; the port's minimap (also under the world HUD root) stays.
#[allow(clippy::type_complexity)]
fn hide_debug_cards(
    screens: Query<&OrigScreen>,
    texts: Query<&ChildOf, Or<(With<WorldZoneText>, With<WorldRadarText>)>>,
    mut vis: Query<&mut Visibility, Without<OrigScreen>>,
) {
    if !screens
        .iter()
        .any(|s| s.id == ScreenId::FieldHud && s.shown)
    {
        return;
    }
    for parent in &texts {
        if let Ok(mut v) = vis.get_mut(parent.parent()) {
            v.set_if_neq(Visibility::Hidden);
        }
    }
}

/// Species display names by dex (looked up once each from the battle data).
#[derive(Default)]
pub struct NameCache(HashMap<(u16, u8), String>);

impl NameCache {
    pub fn get(&mut self, assets: Option<&BattleAssets>, dex: u16, form: u8) -> String {
        self.0
            .entry((dex, form))
            .or_insert_with(|| {
                assets
                    .and_then(|a| a.data.species_row_by_dex_form(dex, form).ok().flatten())
                    .map(|s| title_case(&s.name))
                    .unwrap_or_else(|| format!("#{dex:04}"))
            })
            .clone()
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn bind_field_hud(
    config: Res<OrigUiConfig>,
    state: Res<State<GameState>>,
    paused: Option<Res<Paused>>,
    zone: Option<Res<ActiveZoneInfo>>,
    progress: Option<Res<PlayerProgress>>,
    balls: Option<Res<BallInventory>>,
    selector: Option<Res<FieldSelector>>,
    icons: Res<IconIndex>,
    player: Query<&Transform, With<PlayerAvatar>>,
    wild: Query<(&Transform, &WildCreature), Without<PlayerAvatar>>,
    mut screens: Query<&mut OrigScreen>,
) {
    if !config.enabled() {
        return;
    }
    let Some(mut s) = screens.iter_mut().find(|s| s.id == ScreenId::FieldHud) else {
        return;
    };
    let show = *state.get() == GameState::World && !paused.as_ref().is_some_and(|p| p.0);
    s.shown = show;
    if !show {
        return;
    }
    // Research bar: the nearest wild Pokémon within 60 m (the provisional radar's data).
    let nearest = player.single().ok().and_then(|p| {
        wild.iter()
            .filter(|(_, w)| !w.defeated)
            .map(|(t, w)| (t.translation.distance(p.translation), w))
            .filter(|(d, _)| *d < 60.0)
            .min_by(|a, b| a.0.total_cmp(&b.0))
    });
    let (header, icon) = match nearest {
        Some((_, w)) => (
            title_case(&w.species_name),
            icons.pokemon(w.national_dex, 0),
        ),
        None => (
            zone.as_ref()
                .map(|z| zone_label(&z.name))
                .unwrap_or_default(),
            None,
        ),
    };
    s.set_in("L_zninfo_00", "T_header_zninfo_00", |o| {
        o.text = Some(header);
        o.text_color = Some(BAR_TEXT);
    });
    let has_icon = icon.is_some();
    s.set_in("L_zninfo_00", "P_pokemon_00", |o| {
        o.visible = Some(has_icon);
        o.image = icon;
    });
    for n in ["P_item_bg_00", "P_item_bg_line_00"] {
        s.set_in("L_zninfo_00", n, |o| o.visible = Some(has_icon));
    }
    // Research counters: no data in the port.
    for n in [
        "N_count_uncomplete_00",
        "N_count_complete_00",
        "N_multiple_00",
        "P_overlay_00",
    ] {
        s.set_in("L_zninfo_00", n, |o| o.visible = Some(false));
    }
    if let Some(b) = &balls {
        let id = b.selected.item_id();
        let count = progress
            .as_ref()
            .and_then(|p| p.bag.get(&id).copied())
            .unwrap_or(0);
        let label = b.selected.display_name().to_string();
        s.set_in("L_item_00", "T_itemname_00", |o| o.text = Some(label));
        s.set_in("L_item_00", "T_count_00", |o| o.text = Some("x".into()));
        s.set_in("L_item_00", "T_count_01", |o| {
            o.text = Some(count.to_string())
        });
    }
    // Button guides right of the port's minimap (bottom-left corner, about 230 px wide).
    s.set("L_command_00", |o| o.translate = Some([230.0, 0.0]));
    for hidden in [
        "L_item_01",
        "L_item_02",
        "L_mssn_00",
        "L_gauge_nspoke_01",
        "L_ns_time_00",
        "L_item_pokemon_00",
        "L_as_00",
        "L_wnhd_00",
    ] {
        s.set_visible(hidden, false);
    }
    // The field selector (X) shows the same ball larger, so the shortcut is hidden while it is
    // open; a bottom-centre line names that key while it is closed (the wheel brings its own
    // key line).
    let selector_open = selector.as_ref().is_some_and(|s| s.open);
    s.set_visible("L_item_00", !selector_open);
    s.set_free_texts(if selector_open {
        Vec::new()
    } else {
        vec![free_text(
            610.0,
            1032.0,
            700.0,
            34.0,
            24.0,
            [0.95, 0.95, 0.9, 0.55],
            TextAlign::Center,
            "X: open the item / Pokémon selector",
        )]
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_label_drops_the_internal_id() {
        assert_eq!(
            zone_label("Obsidian Fieldlands (ha_area01)"),
            "Obsidian Fieldlands"
        );
        assert_eq!(zone_label("Jubilife Village"), "Jubilife Village");
    }
}
