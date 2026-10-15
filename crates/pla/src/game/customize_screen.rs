//! Customization screen (Brief 36): choose the protagonist's look.
//!
//! This is the port's PROVISIONAL screen (the same `bevy_ui` style as the graphics screen). The
//! original game's dress-up layout (`bin/appli/dressup`, decoded into `work/ui/layouts/dressup.json`
//! with its PNGs) exists, but binding it (category tabs, the 7-row list, the info panel) is a
//! larger job than this pass; the look model, the catalogue and the live preview here are the
//! same data the original screen would drive.
//!
//! While the screen is open the world keeps rendering and the camera is moved to a front
//! close-up of the player (see the viewer's customize camera override), so every change is
//! visible immediately: pieces are respawned by `game::dressup`.

use bevy::prelude::*;

use super::dressup::{LookCatalog, Outfit, PlayerLook, PlayerLookState, SKIN_TONES};
use super::input_map::{Action, ActionState};
use super::pause_menu::{freeze_for_screen, PauseMenuSuppressed};
use super::state::Paused;
use crate::assets::player_model::{EYE_COLOR_PALETTE, HAIR_COLOR_PALETTE};

/// Set by the pause menu's `Customize` row; the screen consumes it and opens.
#[derive(Resource, Debug, Default)]
pub struct CustomizeOpenRequested(pub bool);

/// Open state and cursor.
#[derive(Resource, Debug, Default)]
pub struct CustomizeScreenState {
    pub open: bool,
    pub selected: usize,
    pub scroll: usize,
}

/// Screen rows, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Variant,
    Hair,
    HairColor,
    EyebrowColor,
    EyeColor,
    SkinTone,
    Dress,
    Top,
    Bottom,
    Hat,
    Glasses,
    Shoes,
    Bag,
}

pub const ROWS: [Row; 13] = [
    Row::Variant,
    Row::Hair,
    Row::HairColor,
    Row::EyebrowColor,
    Row::EyeColor,
    Row::SkinTone,
    Row::Dress,
    Row::Top,
    Row::Bottom,
    Row::Hat,
    Row::Glasses,
    Row::Shoes,
    Row::Bag,
];

/// Rows the panel shows at once.
pub const VISIBLE_ROWS: usize = 13;

pub fn row_label(row: Row) -> &'static str {
    match row {
        Row::Variant => "Protagonist",
        Row::Hair => "Hair style",
        Row::HairColor => "Hair colour",
        Row::EyebrowColor => "Eyebrow colour",
        Row::EyeColor => "Eye colour",
        Row::SkinTone => "Skin tone",
        Row::Dress => "Outfit (set)",
        Row::Top => "Top",
        Row::Bottom => "Bottom",
        Row::Hat => "Hat",
        Row::Glasses => "Glasses",
        Row::Shoes => "Shoes",
        Row::Bag => "Bag",
    }
}

/// The catalogue slot a row cycles through, when it is a piece row.
fn slot_of(row: Row) -> Option<&'static str> {
    match row {
        Row::Hair => Some("hrs"),
        Row::Dress => Some("drs"),
        Row::Top => Some("tps"),
        Row::Bottom => Some("btm"),
        Row::Hat => Some("hdw"),
        Row::Glasses => Some("eyew"),
        Row::Shoes => Some("ftw"),
        Row::Bag => Some("bag"),
        _ => None,
    }
}

/// A short label for a stem: the part after the category code (`p1_hrs0002_00_ponytail` ->
/// `ponytail`). The catalogue ids are the game's own internal names, not shop text.
pub fn stem_short(stem: &str) -> &str {
    let mut parts = stem.split('_');
    let _variant = parts.next();
    let _code = parts.next();
    let _nn = parts.next();
    parts.next().unwrap_or(stem)
}

/// The value shown for a row.
pub fn value_label(row: Row, look: &PlayerLook, catalog: &LookCatalog) -> String {
    let _ = catalog;
    let v = look.variant.tag();
    match row {
        Row::Variant => format!("{} ({})", v.to_uppercase(), if v == "p1" { "Rei" } else { "Akari" }),
        Row::Hair => stem_short(&look.hair).to_owned(),
        Row::HairColor => look.hair_color.clone(),
        Row::EyebrowColor => look.eyebrow_color.clone(),
        Row::EyeColor => {
            if look.eye_color.is_empty() {
                "default".into()
            } else {
                look.eye_color.clone()
            }
        }
        Row::SkinTone => {
            let rgb = SKIN_TONES[look.skin_tone as usize % SKIN_TONES.len()];
            format!("{} ({},{},{})", look.skin_tone + 1, rgb[0], rgb[1], rgb[2])
        }
        Row::Dress => match &look.outfit {
            Outfit::Set(stem) => format!("{} *", stem_short(stem)),
            Outfit::TopBottom { .. } => "(wearing top + bottom)".into(),
        },
        Row::Top => match &look.outfit {
            Outfit::Set(_) => "(wearing a set)".into(),
            Outfit::TopBottom { top, .. } => stem_short(top).to_owned(),
        },
        Row::Bottom => match &look.outfit {
            Outfit::Set(_) => "(wearing a set)".into(),
            Outfit::TopBottom { bottom, .. } => stem_short(bottom).to_owned(),
        },
        Row::Hat => if look.hat.is_empty() { "none".into() } else { stem_short(&look.hat).to_owned() },
        Row::Glasses => if look.glasses.is_empty() { "none".into() } else { stem_short(&look.glasses).to_owned() },
        Row::Shoes => stem_short(&look.shoes).to_owned(),
        Row::Bag => stem_short(&look.bag).to_owned(),
    }
}

/// Cycle the value of `row` by `dir` (+1/-1). Returns `true` when the look changed. Pure,
/// unit-tested.
pub fn cycle(row: Row, dir: i32, look: &mut PlayerLook, catalog: &LookCatalog) -> bool {
    let v = look.variant.tag();
    let before = look.clone();
    match row {
        Row::Variant => {
            let other = look.variant.flip();
            let mut next = PlayerLook::default_for(other);
            // Keep the colour choices across variants; the stems reset to that variant's defaults.
            next.hair_color = look.hair_color.clone();
            next.eyebrow_color = look.eyebrow_color.clone();
            next.eye_color = look.eye_color.clone();
            next.skin_tone = look.skin_tone;
            next.sanitize(catalog);
            *look = next;
        }
        Row::HairColor => {
            look.hair_color = cycle_name(&HAIR_COLOR_PALETTE, &look.hair_color, dir);
        }
        Row::EyebrowColor => {
            look.eyebrow_color = cycle_name(&HAIR_COLOR_PALETTE, &look.eyebrow_color, dir);
        }
        Row::EyeColor => {
            let mut names: Vec<String> = vec![String::new()];
            names.extend(EYE_COLOR_PALETTE.iter().map(|(n, _)| (*n).to_owned()));
            look.eye_color = cycle_list(&names, &look.eye_color, dir);
        }
        Row::SkinTone => {
            let n = SKIN_TONES.len() as i32;
            look.skin_tone = ((i32::from(look.skin_tone) + dir).rem_euclid(n)) as u8;
        }
        Row::Dress => {
            let stems = catalog.stems(v, "drs");
            let current = match &look.outfit {
                Outfit::Set(s) => s.clone(),
                Outfit::TopBottom { .. } => String::new(),
            };
            let next = cycle_list_owned(&stems, &current, dir);
            look.outfit = Outfit::Set(next);
        }
        Row::Top | Row::Bottom => {
            let slot = if row == Row::Top { "tps" } else { "btm" };
            let stems = catalog.stems(v, slot);
            let (mut top, mut bottom) = match &look.outfit {
                Outfit::TopBottom { top, bottom } => (top.clone(), bottom.clone()),
                Outfit::Set(_) => (
                    catalog.default_stem(v, "tps").unwrap_or_default(),
                    catalog.default_stem(v, "btm").unwrap_or_default(),
                ),
            };
            let current = if row == Row::Top { top.clone() } else { bottom.clone() };
            let next = cycle_list_owned(&stems, &current, dir);
            if row == Row::Top {
                top = next;
            } else {
                bottom = next;
            }
            look.outfit = Outfit::TopBottom { top, bottom };
        }
        Row::Hat | Row::Glasses => {
            let slot = if row == Row::Hat { "hdw" } else { "eyew" };
            let mut stems: Vec<String> = vec![String::new()];
            stems.extend(catalog.stems(v, slot).into_iter().map(str::to_owned));
            let current = if row == Row::Hat { look.hat.clone() } else { look.glasses.clone() };
            let next = cycle_list(&stems, &current, dir);
            if row == Row::Hat {
                look.hat = next;
            } else {
                look.glasses = next;
            }
        }
        _ => {
            if let Some(slot) = slot_of(row) {
                let stems = catalog.stems(v, slot);
                let current = match row {
                    Row::Hair => look.hair.clone(),
                    Row::Shoes => look.shoes.clone(),
                    Row::Bag => look.bag.clone(),
                    _ => String::new(),
                };
                let next = cycle_list_owned(&stems, &current, dir);
                match row {
                    Row::Hair => look.hair = next,
                    Row::Shoes => look.shoes = next,
                    Row::Bag => look.bag = next,
                    _ => {}
                }
            }
        }
    }
    *look != before
}

fn cycle_name(palette: &[(&str, [u8; 3])], current: &str, dir: i32) -> String {
    let names: Vec<String> = palette.iter().map(|(n, _)| (*n).to_owned()).collect();
    cycle_list(&names, current, dir)
}

fn cycle_list(list: &[String], current: &str, dir: i32) -> String {
    if list.is_empty() {
        return current.to_owned();
    }
    let idx = list.iter().position(|s| s == current).map_or(0, |i| i as i32);
    let next = (idx + dir).rem_euclid(list.len() as i32) as usize;
    list[next].clone()
}

fn cycle_list_owned(list: &[&str], current: &str, dir: i32) -> String {
    let owned: Vec<String> = list.iter().map(|s| (*s).to_owned()).collect();
    cycle_list(&owned, current, dir)
}

// ---------------------------------------------------------------- systems

#[derive(Component)]
pub struct CustomizeScreenRoot;
#[derive(Component)]
pub struct CustomizeRowButton(pub usize);
#[derive(Component)]
pub struct CustomizeRowText(pub usize);
#[derive(Component)]
pub struct CustomizeHint;

fn spawn_customize_screen(mut commands: Commands) {
    commands
        .spawn((
            CustomizeScreenRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                padding: UiRect::left(Val::Px(24.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            GlobalZIndex(130),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexStart,
                    padding: UiRect::all(Val::Px(14.0)),
                    row_gap: Val::Px(3.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.88)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("CUSTOMIZE"),
                    TextFont::from_font_size(26.0),
                    TextColor(Color::srgb(0.95, 0.9, 0.6)),
                ));
                for i in 0..VISIBLE_ROWS {
                    panel
                        .spawn((
                            Button,
                            CustomizeRowButton(i),
                            Node {
                                width: Val::Px(430.0),
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(3.0)),
                                justify_content: JustifyContent::SpaceBetween,
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.15, 0.2, 0.28, 0.9)),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                CustomizeRowText(i),
                                Text::new(""),
                                TextFont::from_font_size(17.0),
                                TextColor(Color::WHITE),
                            ));
                        });
                }
                panel.spawn((
                    CustomizeHint,
                    Node { max_width: Val::Px(470.0), ..default() },
                    Text::new(""),
                    TextFont::from_font_size(13.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
                ));
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn customize_input(
    keys: ResMut<ButtonInput<KeyCode>>,
    mut actions: ResMut<ActionState>,
    mut state: ResMut<CustomizeScreenState>,
    mut look_state: ResMut<PlayerLookState>,
    catalog: Res<LookCatalog>,
    mut request: ResMut<CustomizeOpenRequested>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
    buttons: Query<(&CustomizeRowButton, &Interaction), Changed<Interaction>>,
) {
    if request.0 {
        request.0 = false;
        state.open = true;
        state.selected = 0;
        state.scroll = 0;
        freeze_for_screen(&mut paused, &mut suppressed, true);
        return;
    }
    if !state.open {
        return;
    }
    if actions.just_pressed(Action::Cancel) || actions.just_pressed(Action::Menu) {
        actions.consume(Action::Cancel);
        actions.consume(Action::Menu);
        state.open = false;
        if let Some(p) = paused.as_deref_mut() {
            p.0 = true;
        }
        if let Some(s) = suppressed.as_deref_mut() {
            s.0 = false;
        }
        return;
    }

    let any = |keys: &ButtonInput<KeyCode>, list: &[KeyCode]| keys.any_just_pressed(list.iter().copied());
    let left = any(&keys, &[KeyCode::ArrowLeft, KeyCode::KeyA]);
    let right = any(&keys, &[KeyCode::ArrowRight, KeyCode::KeyD]);
    let up = any(&keys, &[KeyCode::ArrowUp, KeyCode::KeyW]);
    let down = any(&keys, &[KeyCode::ArrowDown, KeyCode::KeyS]);
    let mut dir = 0i32;
    let mut select = 0i32;
    if left {
        dir = -1;
        actions.consume(Action::Prev);
    }
    if right {
        dir = 1;
        actions.consume(Action::Next);
    }
    if up {
        select -= 1;
        actions.consume(Action::Prev);
    }
    if down {
        select += 1;
        actions.consume(Action::Next);
    }
    if !(left || right || up || down) {
        if actions.just_pressed(Action::Prev) {
            actions.consume(Action::Prev);
            dir = -1;
        }
        if actions.just_pressed(Action::Next) {
            actions.consume(Action::Next);
            dir = 1;
        }
    }
    if actions.just_pressed(Action::Confirm) {
        actions.consume(Action::Confirm);
        // Enter cycles the row forward, like Right.
        dir = 1;
    }
    for (b, interaction) in &buttons {
        if *interaction == Interaction::Pressed {
            if b.0 == state.selected {
                dir = 1;
            } else {
                state.selected = b.0;
            }
        }
    }

    state.selected = (state.selected as i32 + select).rem_euclid(ROWS.len() as i32) as usize;
    if dir != 0 {
        let mut look = look_state.look.clone();
        if cycle(ROWS[state.selected], dir, &mut look, &catalog) {
            look_state.look = look;
        }
    }
    // Keep the selected row visible (the panel shows every row, so only the cursor moves).
    state.scroll = 0;
}

fn sync_customize_screen(
    state: Res<CustomizeScreenState>,
    look_state: Res<PlayerLookState>,
    catalog: Res<LookCatalog>,
    mut root: Query<&mut Visibility, With<CustomizeScreenRoot>>,
    mut buttons: Query<(&CustomizeRowButton, &mut BackgroundColor)>,
    mut texts: Query<(&CustomizeRowText, &mut Text), Without<CustomizeHint>>,
    mut hint: Query<&mut Text, With<CustomizeHint>>,
) {
    for mut v in &mut root {
        let want = if state.open { Visibility::Visible } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
    }
    if !state.open {
        return;
    }
    for (b, mut bg) in &mut buttons {
        bg.0 = if b.0 == state.selected {
            Color::srgba(0.85, 0.6, 0.15, 0.95)
        } else {
            Color::srgba(0.15, 0.2, 0.28, 0.9)
        };
    }
    for (t, mut text) in &mut texts {
        let Some(row) = ROWS.get(t.0) else { continue };
        let label = format!("{}   < {} >", row_label(*row), value_label(*row, &look_state.look, &catalog));
        if text.0 != label {
            text.0 = label;
        }
    }
    let help = "Up/Down select, Left/Right or Enter change, Esc back. \
                Changes apply to the model immediately; the look is stored in the save file.\n\
                Hair/eyebrow/eye colours and skin tones are the game's own options (the RGB of the \
                hair and eye names is a stand-in: the data has names only). Clothing patterns are \
                not applied (not decoded).";
    for mut h in &mut hint {
        if h.0 != help {
            h.0 = help.to_owned();
        }
    }
}

/// Installs the screen. The pause menu's `Customize` row raises [`CustomizeOpenRequested`].
pub struct CustomizeScreenPlugin;

impl Plugin for CustomizeScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CustomizeScreenState>()
            .init_resource::<CustomizeOpenRequested>()
            .add_systems(Startup, spawn_customize_screen)
            .add_systems(Update, (customize_input, sync_customize_screen).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::dressup::LookVariant;

    fn catalog() -> LookCatalog {
        LookCatalog::builtin()
    }

    #[test]
    fn rows_cover_every_slot_and_are_stable() {
        assert_eq!(ROWS.len(), VISIBLE_ROWS);
        for slot in ["hrs", "drs", "tps", "btm", "hdw", "eyew", "ftw", "bag"] {
            assert!(ROWS.iter().any(|r| slot_of(*r) == Some(slot)), "slot {slot} has no row");
        }
        let mut labels: Vec<&str> = ROWS.iter().map(|r| row_label(*r)).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), ROWS.len());
    }

    #[test]
    fn cycling_a_piece_row_lands_on_catalogue_stems() {
        let catalog = catalog();
        let mut look = PlayerLook::default_for(LookVariant::P1);
        let first = look.hair.clone();
        assert!(cycle(Row::Hair, 1, &mut look, &catalog));
        assert_ne!(look.hair, first);
        assert!(catalog.has("p1", "hrs", &look.hair));
        // Full wrap returns to the first stem (one step was taken above already).
        for _ in 1..catalog.stems("p1", "hrs").len() {
            cycle(Row::Hair, 1, &mut look, &catalog);
        }
        assert_eq!(look.hair, first);
    }

    #[test]
    fn hat_and_glasses_can_be_removed_again() {
        let catalog = catalog();
        let mut look = PlayerLook::default_for(LookVariant::P1);
        assert!(look.hat.is_empty());
        assert!(cycle(Row::Hat, 1, &mut look, &catalog));
        assert!(!look.hat.is_empty());
        assert!(catalog.has("p1", "hdw", &look.hat));
        // The "none" entry is the first of the cycle: wrapping forward from the last lands there.
        for _ in 1..=catalog.stems("p1", "hdw").len() {
            cycle(Row::Hat, 1, &mut look, &catalog);
        }
        assert!(look.hat.is_empty());
    }

    #[test]
    fn colours_cycle_through_the_palettes() {
        let catalog = catalog();
        let mut look = PlayerLook::default_for(LookVariant::P1);
        cycle(Row::HairColor, 1, &mut look, &catalog);
        assert!(HAIR_COLOR_PALETTE.iter().any(|(n, _)| *n == look.hair_color));
        assert!(look.eye_color.is_empty(), "the default eye colour comes first");
        cycle(Row::EyeColor, -1, &mut look, &catalog);
        assert!(EYE_COLOR_PALETTE.iter().any(|(n, _)| *n == look.eye_color));
        cycle(Row::SkinTone, 1, &mut look, &catalog);
        assert_eq!(look.skin_tone, 1);
        cycle(Row::SkinTone, -1, &mut look, &catalog);
        cycle(Row::SkinTone, -1, &mut look, &catalog);
        assert_eq!(look.skin_tone, 3, "wraps to the last tone");
    }

    #[test]
    fn switching_protagonist_resets_stems_and_keeps_colours() {
        let catalog = catalog();
        let mut look = PlayerLook::default_for(LookVariant::P1);
        look.hair_color = "blond".into();
        look.skin_tone = 2;
        assert!(cycle(Row::Variant, 1, &mut look, &catalog));
        assert_eq!(look.variant, LookVariant::P2);
        assert_eq!(look.hair_color, "blond");
        assert_eq!(look.skin_tone, 2);
        for (slot, stem) in look.pieces() {
            assert!(catalog.has("p2", slot, &stem), "{slot} {stem}");
        }
    }

    #[test]
    fn top_and_bottom_rows_switch_to_a_pair() {
        let catalog = catalog();
        let mut look = PlayerLook::default_for(LookVariant::P1);
        assert!(matches!(look.outfit, Outfit::Set(_)));
        assert!(cycle(Row::Top, 1, &mut look, &catalog));
        match &look.outfit {
            Outfit::TopBottom { top, bottom } => {
                assert!(catalog.has("p1", "tps", top));
                assert!(catalog.has("p1", "btm", bottom));
            }
            other => panic!("expected a pair, got {other:?}"),
        }
        // The dress row goes back to a set.
        assert!(cycle(Row::Dress, 1, &mut look, &catalog));
        assert!(matches!(look.outfit, Outfit::Set(_)));
    }

    #[test]
    fn stem_short_strips_the_catalogue_prefix() {
        assert_eq!(stem_short("p1_hrs0002_00_ponytail"), "ponytail");
        assert_eq!(stem_short("p2_drs0001_00_default"), "default");
    }
}
