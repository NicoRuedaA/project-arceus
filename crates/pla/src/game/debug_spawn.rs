//! F7 debug spawn selector: search any national species and place its model next to the player.
//!
//! Developer aid, **off by default and zero-cost while closed**. F7 opens a panel with a search
//! box and the list of the 1025 national species (`data/national/names.tsv`: Spanish name,
//! English fallback). Typing filters the list as you write (case-insensitive, accents folded;
//! a number matches the national dex number), Up/Down move the selection, Left/Right change the
//! form of the selected species, Enter places ONE model of it next to the player (on the free
//! side of the camera's view and a step beyond the player, [`SPAWN_DISTANCE`]; the model faces
//! the camera) and Supr removes it. Esc closes the panel; the placed model stays until replaced
//! or removed. Picking another species replaces the previous model instead of piling them up.
//!
//! The model is resolved exactly like `--spawn-species` (`game::pokemon_resource::resolve_model`):
//! the game's own PLA catalog wins, the converted collection (`work/cache/pokemon_national`)
//! fills the rest; with no model the panel says `sin modelo para #N` (also in the toast) and
//! nothing is placed.
//!
//! While the panel is open it owns the keyboard: the world is frozen through the shared
//! screen mechanism ([`freeze_for_screen`]: `Paused` + `PauseMenuSuppressed`) and the frame's
//! input is neutralised for every other system (movement, riding, doors, pickups, the F5 save),
//! so typing a name never drives the player. The panel writes nothing to the save.
//!
//! Key map (the single source of truth for this feature):
//!
//! | Key | Action |
//! |---|---|
//! | `F7` | open / close the panel |
//! | letters, digits, `-`, `.`, space | append to the search text |
//! | `Backspace` | delete the last character |
//! | Up / Down | move the selection in the filtered list |
//! | PageUp / PageDown | jump one page ([`VISIBLE_ROWS`]) |
//! | Home / End | first / last match |
//! | Left / Right | cycle the forms that have a model |
//! | Enter / NumpadEnter | place the selected species + form next to the player |
//! | Supr (Delete) | remove the placed model |
//! | Esc | close the panel (the placed model stays) |

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::input_map::{Action, ActionState};
use super::pause_menu::{freeze_for_screen, PauseMenuSuppressed};
use super::pokemon_anim::PokemonAnimSource;
use super::pokemon_resource::{resolve_forms, resolve_model};
use super::save_wire::SaveToast;
use super::state::{GameState, Paused};

/// Rows of the list shown at once (the panel virtualizes the 1025 rows to this window).
pub const VISIBLE_ROWS: usize = 12;

/// Distance from the player where the selected model is placed (m, front-right).
pub const SPAWN_DISTANCE: f32 = 2.5;

/// The key that opens/closes the panel (F6 is the Pokédex override of `game::debug_visual`).
pub const PANEL_KEY: KeyCode = KeyCode::F7;

// ------------------------------------------------------------------ names sheet

/// One national species name (`data/national/names.tsv`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeciesName {
    pub id: u16,
    /// Display name (Spanish first).
    pub name: String,
    /// Folded search text: Spanish name, English name and identifier, so a species is found
    /// by any of them ("Iron Boulder" finds "Ferromole").
    pub search: String,
}

/// The 1025 national species names, loaded once from the tracked sheet.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct NationalNames {
    pub entries: Vec<SpeciesName>,
}

impl NationalNames {
    /// Parse `names.tsv`; the Spanish name wins, the English one then the identifier fill in.
    pub fn from_tsv(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'));
        let headers: Vec<&str> = lines
            .next()
            .ok_or_else(|| "national names: missing header".to_string())?
            .split('\t')
            .map(|field| field.split_once(':').map_or(field, |(name, _)| name))
            .collect();
        let column = |name: &str| {
            headers
                .iter()
                .position(|header| *header == name)
                .ok_or_else(|| format!("national names: missing column {name}"))
        };
        let (c_number, c_identifier, c_en, c_es) = (
            column("national_number")?,
            column("identifier")?,
            column("name_en")?,
            column("name_es")?,
        );
        let mut entries = Vec::new();
        for (line_index, line) in lines.enumerate() {
            let values: Vec<&str> = line.split('\t').collect();
            if values.len() != headers.len() {
                return Err(format!(
                    "national names row {}: expected {} fields, got {}",
                    line_index + 2,
                    headers.len(),
                    values.len()
                ));
            }
            let id = values[c_number]
                .parse::<u16>()
                .map_err(|_| format!("national names row {}: invalid number", line_index + 2))?;
            let name = if !values[c_es].trim().is_empty() {
                values[c_es].trim()
            } else if !values[c_en].trim().is_empty() {
                values[c_en].trim()
            } else {
                values[c_identifier].trim()
            };
            let search = [values[c_es], values[c_en], values[c_identifier]]
                .iter()
                .map(|text| fold(text))
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            entries.push(SpeciesName {
                id,
                name: name.to_string(),
                search,
            });
        }
        entries.sort_by_key(|entry| entry.id);
        Ok(Self { entries })
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        Self::from_tsv(&text)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Display name of a national id (empty when the sheet does not hold it).
    pub fn name_of(&self, id: u16) -> &str {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .map_or("", |entry| entry.name.as_str())
    }
}

/// Tracked data directory holding `national/names.tsv` (cwd first, then the repository's).
pub fn find_data_dir() -> Option<PathBuf> {
    [
        PathBuf::from("data"),
        PathBuf::from("../../data"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
    ]
    .into_iter()
    .find(|path| path.join("national/names.tsv").is_file())
}

// ------------------------------------------------------------------ pure search

/// Fold one character for searching: lowercase, accents removed, everything that is not a
/// letter or a digit dropped (spaces, dots, hyphens, apostrophes and the `♀`/`♂` symbols of
/// names like "Nidoran♀" disappear, so the plain word matches).
fn fold_char(c: char) -> Option<char> {
    match c {
        'a'..='z' | '0'..='9' => Some(c),
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => Some('a'),
        'é' | 'è' | 'ë' | 'ê' => Some('e'),
        'í' | 'ì' | 'ï' | 'î' => Some('i'),
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' => Some('o'),
        'ú' | 'ù' | 'ü' | 'û' => Some('u'),
        'ñ' => Some('n'),
        'ç' => Some('c'),
        _ => None,
    }
}

/// Search form of a text: lowercase, accents folded, non-alphanumeric dropped.
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(|c| c.to_lowercase())
        .filter_map(fold_char)
        .collect()
}

/// Indices of the entries matching `query` (all when the query is empty). Matching ignores
/// case, accents and punctuation, and a plain number (`6`, `06`, `#6`) also matches the
/// national dex number.
pub fn filter_indices(entries: &[SpeciesName], query: &str) -> Vec<usize> {
    let folded = fold(query);
    if folded.is_empty() {
        return (0..entries.len()).collect();
    }
    let numeric = query
        .trim()
        .trim_start_matches('#')
        .parse::<u16>()
        .ok()
        .filter(|n| *n > 0);
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            numeric == Some(entry.id)
                || fold(&entry.name).contains(&folded)
                || entry.search.split(' ').any(|alias| alias.contains(&folded))
        })
        .map(|(index, _)| index)
        .collect()
}

/// Wrapping move inside a list (`len == 0` stays at 0).
pub fn move_index(selected: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    ((selected.min(len - 1) as i32 + delta).rem_euclid(len as i32)) as usize
}

/// First row of the window that keeps `selected` centred (clamped to the list bounds).
pub fn window_start(selected: usize, len: usize, rows: usize) -> usize {
    if len == 0 || rows == 0 {
        return 0;
    }
    selected
        .saturating_sub(rows / 2)
        .min(len.saturating_sub(rows))
}

/// Wrapping cycle over the available forms.
pub fn cycle_form(forms: &[(u8, String)], current: usize, delta: i32) -> usize {
    if forms.is_empty() {
        return 0;
    }
    ((current.min(forms.len() - 1) as i32 + delta).rem_euclid(forms.len() as i32)) as usize
}

/// The character a typed key adds to the search text (physical positions, so the same keys as
/// a physical keyboard and as the automation `key` command; the names fold to ASCII, so no
/// accented key is needed).
pub fn char_for_key(key: KeyCode) -> Option<char> {
    const LETTERS: [(KeyCode, char); 26] = [
        (KeyCode::KeyA, 'a'),
        (KeyCode::KeyB, 'b'),
        (KeyCode::KeyC, 'c'),
        (KeyCode::KeyD, 'd'),
        (KeyCode::KeyE, 'e'),
        (KeyCode::KeyF, 'f'),
        (KeyCode::KeyG, 'g'),
        (KeyCode::KeyH, 'h'),
        (KeyCode::KeyI, 'i'),
        (KeyCode::KeyJ, 'j'),
        (KeyCode::KeyK, 'k'),
        (KeyCode::KeyL, 'l'),
        (KeyCode::KeyM, 'm'),
        (KeyCode::KeyN, 'n'),
        (KeyCode::KeyO, 'o'),
        (KeyCode::KeyP, 'p'),
        (KeyCode::KeyQ, 'q'),
        (KeyCode::KeyR, 'r'),
        (KeyCode::KeyS, 's'),
        (KeyCode::KeyT, 't'),
        (KeyCode::KeyU, 'u'),
        (KeyCode::KeyV, 'v'),
        (KeyCode::KeyW, 'w'),
        (KeyCode::KeyX, 'x'),
        (KeyCode::KeyY, 'y'),
        (KeyCode::KeyZ, 'z'),
    ];
    if let Some((_, ch)) = LETTERS.iter().find(|(code, _)| *code == key) {
        return Some(*ch);
    }
    match key {
        KeyCode::Digit0 => Some('0'),
        KeyCode::Digit1 => Some('1'),
        KeyCode::Digit2 => Some('2'),
        KeyCode::Digit3 => Some('3'),
        KeyCode::Digit4 => Some('4'),
        KeyCode::Digit5 => Some('5'),
        KeyCode::Digit6 => Some('6'),
        KeyCode::Digit7 => Some('7'),
        KeyCode::Digit8 => Some('8'),
        KeyCode::Digit9 => Some('9'),
        KeyCode::Numpad0 => Some('0'),
        KeyCode::Numpad1 => Some('1'),
        KeyCode::Numpad2 => Some('2'),
        KeyCode::Numpad3 => Some('3'),
        KeyCode::Numpad4 => Some('4'),
        KeyCode::Numpad5 => Some('5'),
        KeyCode::Numpad6 => Some('6'),
        KeyCode::Numpad7 => Some('7'),
        KeyCode::Numpad8 => Some('8'),
        KeyCode::Numpad9 => Some('9'),
        KeyCode::Space => Some(' '),
        KeyCode::Minus => Some('-'),
        KeyCode::NumpadSubtract => Some('-'),
        KeyCode::Period => Some('.'),
        KeyCode::NumpadDecimal => Some('.'),
        _ => None,
    }
}

// ------------------------------------------------------------------ resources

/// Marker of the model placed by the F7 panel (distinct from `--spawn-species`).
#[derive(Component, Debug, Clone, Copy)]
pub struct DebugSpawnedSpecies;

/// Panel state: open flag, search text, selection and the placed model. All in memory; the
/// save is never touched.
#[derive(Resource, Debug, Default)]
pub struct DebugSpawnPanel {
    pub open: bool,
    /// Search text as typed (matching folds it; the box shows it verbatim).
    pub query: String,
    /// Selection inside [`DebugSpawnPanel::filtered`].
    pub selected: usize,
    /// Index into [`DebugSpawnPanel::forms`].
    pub form_index: usize,
    /// Indices into [`NationalNames::entries`] matching the query (stable between edits).
    pub filtered: Vec<usize>,
    /// Forms with a model of the selected species, ascending.
    pub forms: Vec<(u8, String)>,
    /// Species the cached `forms` belong to (recomputed when the selection moves species).
    pub forms_species: Option<u16>,
    /// The live model placed by this panel: `(species, form, entity)`.
    pub spawned: Option<(u16, u8, Entity)>,
    /// Last action result shown in the panel.
    pub status: String,
    /// True while this panel froze the world (`freeze_for_screen`).
    frozen: bool,
}

impl DebugSpawnPanel {
    /// Selected species (national id), if any.
    pub fn selected_species(&self, names: &NationalNames) -> Option<u16> {
        self.filtered
            .get(self.selected)
            .and_then(|index| names.entries.get(*index))
            .map(|entry| entry.id)
    }

    /// Form that Enter would place (0 when the species has no listed form).
    pub fn selected_form(&self) -> u8 {
        self.forms.get(self.form_index).map_or(0, |(form, _)| *form)
    }

    /// Selected form's display name.
    pub fn selected_form_name(&self) -> &str {
        self.forms.get(self.form_index).map_or("", |(_, name)| name.as_str())
    }

    /// Recompute the filtered list after a text edit (selection restarts at the first match).
    fn apply_filter(&mut self, names: &NationalNames) {
        self.filtered = filter_indices(&names.entries, &self.query);
        self.selected = 0;
        self.sync_forms(names);
    }

    /// Rebuild the form list when the selection moved to another species.
    fn sync_forms(&mut self, names: &NationalNames) {
        let species = self.selected_species(names);
        if species != self.forms_species {
            self.forms_species = species;
            self.forms = species.map(resolve_forms).unwrap_or_default();
            self.form_index = 0;
        }
    }
}

// ------------------------------------------------------------------ spawn

/// Place one model of `(species, form)` at [`SPAWN_DISTANCE`] beside the player: on the free
/// side of the active camera's view (the camera's left, because the panel occupies the right)
/// and one step beyond the player, so the model never sits between the camera and the player
/// nor under the panel. The model faces the camera (front visible). Without a camera (headless
/// apps) the player's own front-right is used instead.
///
/// Resolution is the same as `--spawn-species` (game catalog first, converted collection
/// second). Returns the new entity and its label, or the reason nothing was placed.
fn place_species(
    commands: &mut Commands,
    asset_server: &AssetServer,
    player: &Transform,
    camera: Option<Vec3>,
    species: u16,
    form: u8,
) -> Result<(Entity, String), String> {
    let Some(model) = resolve_model(species, form) else {
        return Err(format!("sin modelo para #{species:04}"));
    };
    let player_xz = Vec2::new(player.translation.x, player.translation.z);
    // Camera-relative side: `to_player` runs from the camera to the player, `right` is the
    // camera's screen-right in world XZ (the model goes to its opposite, the free left side).
    let camera_side = camera.and_then(|camera| {
        let camera_xz = Vec2::new(camera.x, camera.z);
        (player_xz - camera_xz).try_normalize().map(|to_player| {
            let right = Vec2::new(-to_player.y, to_player.x);
            (right * -2.2 + to_player * 1.0, -to_player)
        })
    });
    let (direction, facing) = match camera_side {
        Some((direction, facing)) => (direction.normalize_or_zero(), facing),
        None => {
            // No camera: front-right of the player, looking the same way the player does
            // (like `--spawn-species`).
            let front = player.rotation * Vec3::Z;
            let forward = Vec2::new(front.x, front.z).try_normalize().unwrap_or(Vec2::Y);
            let right = Vec2::new(-forward.y, forward.x);
            ((forward + right * 0.8).normalize_or_zero(), forward)
        }
    };
    let direction = if direction == Vec2::ZERO { Vec2::Y } else { direction };
    let position = player_xz + direction * SPAWN_DISTANCE;
    let yaw = facing.x.atan2(facing.y);
    let source = PokemonAnimSource::from_resolved(asset_server, &model);
    let entity = commands
        .spawn((
            DebugSpawnedSpecies,
            source,
            Transform::from_xyz(position.x, player.translation.y, position.y)
                .with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .id();
    let label = format!("#{species:04} forma {form} - {}", model.rel);
    Ok((entity, label))
}

/// World position of the active 3D camera of the world (lowest order wins), when there is one.
fn active_camera_position(cameras: &Query<(&Camera, &GlobalTransform), With<Camera3d>>) -> Option<Vec3> {
    cameras
        .iter()
        .filter(|(camera, _)| camera.is_active)
        .min_by_key(|(camera, _)| camera.order)
        .map(|(_, transform)| transform.translation())
}

/// Despawn the panel's current model (no-op when there is none).
fn despawn_spawned(commands: &mut Commands, panel: &mut DebugSpawnPanel) {
    if let Some((_, _, entity)) = panel.spawned.take() {
        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.despawn();
        }
    }
}

// ------------------------------------------------------------------ systems

/// F7 opens/closes the panel; while open, the keys type/filter/navigate/place/remove (see the
/// module key map). Runs after the input mapping so the same frame's edges are visible.
#[allow(clippy::too_many_arguments)]
fn panel_input(
    mut commands: Commands,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    names: Res<NationalNames>,
    mut panel: ResMut<DebugSpawnPanel>,
    mut actions: Option<ResMut<ActionState>>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
    state: Option<Res<State<GameState>>>,
    asset_server: Option<Res<AssetServer>>,
    player: Query<&Transform, With<super::PlayerAvatar>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut toast: Option<ResMut<SaveToast>>,
) {
    if keys.just_pressed(PANEL_KEY) {
        if panel.open {
            close_panel(&mut panel, &mut paused, &mut suppressed);
        } else if can_open(&state, &paused) {
            panel.open = true;
            panel.status.clear();
            panel.apply_filter(&names);
            panel.frozen = true;
            freeze_for_screen(&mut paused, &mut suppressed, true);
            if let Some(toast) = toast.as_deref_mut() {
                toast.show("F7: colocador abierto. Escribe para filtrar; Enter coloca; Esc cierra.");
            }
        }
        return;
    }
    if !panel.open {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        // The same Esc must not also open the pause menu or clear anything else this frame.
        keys.clear_just_pressed(KeyCode::Escape);
        if let Some(actions) = actions.as_deref_mut() {
            actions.consume(Action::Menu);
            actions.consume(Action::Cancel);
        }
        close_panel(&mut panel, &mut paused, &mut suppressed);
        return;
    }

    // Typing: Backspace removes one character; every character key appends (any keystroke
    // re-filters the list and the selection restarts at the first match).
    let mut query_changed = false;
    if keys.just_pressed(KeyCode::Backspace) && panel.query.pop().is_some() {
        query_changed = true;
    }
    for code in keys.get_just_pressed().copied().collect::<Vec<_>>() {
        if let Some(ch) = char_for_key(code) {
            panel.query.push(ch);
            query_changed = true;
        }
    }
    if query_changed {
        panel.apply_filter(&names);
    }

    // Navigation: Up/Down step, PageUp/PageDown jump a window, Home/End go to the ends.
    let len = panel.filtered.len();
    let mut moved = false;
    if keys.just_pressed(KeyCode::ArrowUp) {
        panel.selected = move_index(panel.selected, -1, len);
        moved = true;
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        panel.selected = move_index(panel.selected, 1, len);
        moved = true;
    }
    if keys.just_pressed(KeyCode::PageUp) {
        panel.selected = move_index(panel.selected, -(VISIBLE_ROWS as i32), len);
        moved = true;
    }
    if keys.just_pressed(KeyCode::PageDown) {
        panel.selected = move_index(panel.selected, VISIBLE_ROWS as i32, len);
        moved = true;
    }
    if keys.just_pressed(KeyCode::Home) {
        panel.selected = 0;
        moved = true;
    }
    if keys.just_pressed(KeyCode::End) {
        panel.selected = len.saturating_sub(1);
        moved = true;
    }
    if moved {
        panel.sync_forms(&names);
    }

    // Left/Right cycle the forms that have a model of their own.
    if keys.just_pressed(KeyCode::ArrowLeft) {
        panel.form_index = cycle_form(&panel.forms, panel.form_index, -1);
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        panel.form_index = cycle_form(&panel.forms, panel.form_index, 1);
    }

    // Enter places (replacing the previous model); Supr removes the current one.
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        let Some(species) = panel.selected_species(&names) else {
            panel.status = "nada que colocar".to_string();
            return;
        };
        let form = panel.selected_form();
        despawn_spawned(&mut commands, &mut panel);
        let Some(asset_server) = asset_server.as_deref() else {
            panel.status = "sin servidor de recursos".to_string();
            return;
        };
        let Ok(player_tf) = player.single() else {
            panel.status = "jugador no disponible".to_string();
            return;
        };
        let camera = active_camera_position(&cameras);
        match place_species(&mut commands, asset_server, player_tf, camera, species, form) {
            Ok((entity, label)) => {
                panel.spawned = Some((species, form, entity));
                let text = format!("colocado {label}");
                panel.status = text.clone();
                if let Some(toast) = toast.as_deref_mut() {
                    toast.show(format!("F7: {text}"));
                }
            }
            Err(reason) => {
                panel.status = reason.clone();
                if let Some(toast) = toast.as_deref_mut() {
                    toast.show(format!("F7: {reason}"));
                }
            }
        }
    }
    if keys.just_pressed(KeyCode::Delete) {
        if panel.spawned.is_some() {
            despawn_spawned(&mut commands, &mut panel);
            panel.status = "modelo quitado".to_string();
            if let Some(toast) = toast.as_deref_mut() {
                toast.show("F7: modelo quitado");
            }
        } else {
            panel.status = "no hay modelo colocado".to_string();
        }
    }
}

/// The panel only opens in the world and when nothing else owns the pause (the pause menu or
/// another fullscreen screen).
fn can_open(state: &Option<Res<State<GameState>>>, paused: &Option<ResMut<Paused>>) -> bool {
    let in_world = state.as_ref().is_none_or(|s| *s.get() == GameState::World);
    let frozen = paused.as_ref().is_some_and(|p| p.0);
    in_world && !frozen
}

/// Close the panel and give the world back only when this panel froze it.
fn close_panel(
    panel: &mut DebugSpawnPanel,
    paused: &mut Option<ResMut<Paused>>,
    suppressed: &mut Option<ResMut<PauseMenuSuppressed>>,
) {
    panel.open = false;
    if panel.frozen {
        panel.frozen = false;
        freeze_for_screen(paused, suppressed, false);
    }
}

/// While the panel is open it owns the keyboard: the frame's action state is neutralised and
/// the raw key/mouse edges are cleared, so no gameplay system reacts to what was typed (the
/// panel's own input already ran this frame). Runs right after [`panel_input`].
fn block_game_input(
    panel: Res<DebugSpawnPanel>,
    mut actions: Option<ResMut<ActionState>>,
    mut keys: Option<ResMut<ButtonInput<KeyCode>>>,
    mut mouse: Option<ResMut<ButtonInput<MouseButton>>>,
) {
    if !panel.open {
        return;
    }
    if let Some(actions) = actions.as_deref_mut() {
        *actions = ActionState::default();
    }
    if let Some(keys) = keys.as_deref_mut() {
        keys.clear();
    }
    if let Some(mouse) = mouse.as_deref_mut() {
        mouse.clear();
    }
}

// ------------------------------------------------------------------ UI

/// Root of the F7 panel (hidden while closed).
#[derive(Component)]
pub struct DebugSpawnRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum PanelText {
    Title,
    Search,
    Count,
    Action,
    Form,
    Row(usize),
    Hint1,
    Hint2,
}

#[derive(Component, Clone, Copy)]
struct PanelRow(usize);

fn spawn_panel(mut commands: Commands) {
    commands
        .spawn((
            DebugSpawnRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(24.0),
                top: Val::Px(90.0),
                width: Val::Px(430.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(3.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.94)),
            // Above the dex screens (190/196) and the debug legend (1000 is far above; the
            // panel only shows while the world is frozen, never together with the legend).
            GlobalZIndex(300),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                PanelText::Title,
                Text::new("DEPURACION F7: COLOCAR POKEMON"),
                TextFont::from_font_size(16.0),
                TextColor(Color::srgb(0.95, 0.9, 0.6)),
            ));
            root.spawn((
                PanelText::Search,
                Text::new(""),
                TextFont::from_font_size(18.0),
                TextColor(Color::srgb(0.95, 0.95, 1.0)),
            ));
            root.spawn((
                PanelText::Count,
                Text::new(""),
                TextFont::from_font_size(13.0),
                TextColor(Color::srgb(0.75, 0.85, 1.0)),
            ));
            for index in 0..VISIBLE_ROWS {
                root.spawn((
                    PanelRow(index),
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.10, 0.14, 0.20, 0.0)),
                ))
                .with_children(|row| {
                    row.spawn((
                        PanelText::Row(index),
                        Text::new(""),
                        TextFont::from_font_size(15.0),
                        TextColor(Color::WHITE),
                    ));
                });
            }
            root.spawn((
                PanelText::Form,
                Text::new(""),
                TextFont::from_font_size(14.0),
                TextColor(Color::srgb(0.8, 0.95, 0.85)),
            ));
            root.spawn((
                PanelText::Action,
                Text::new(""),
                TextFont::from_font_size(13.0),
                TextColor(Color::srgb(0.95, 0.85, 0.7)),
            ));
            root.spawn((
                PanelText::Hint1,
                Text::new("Escribe: filtra | Backspace: borra | Flechas: mueve | Enter: coloca"),
                TextFont::from_font_size(12.0),
                TextColor(Color::srgb(0.7, 0.75, 0.8)),
            ));
            root.spawn((
                PanelText::Hint2,
                Text::new("Izq/Der: forma | RePag/AvPag: pagina | Supr: quita | Esc: cierra"),
                TextFont::from_font_size(12.0),
                TextColor(Color::srgb(0.7, 0.75, 0.8)),
            ));
        });
}

/// Show/hide the panel and keep every text in sync with the state (shown only while open).
fn sync_panel_ui(
    names: Res<NationalNames>,
    mut panel: ResMut<DebugSpawnPanel>,
    live: Query<(), With<DebugSpawnedSpecies>>,
    mut root: Query<&mut Visibility, With<DebugSpawnRoot>>,
    mut texts: Query<(&PanelText, &mut Text)>,
    mut rows: Query<(&PanelRow, &mut BackgroundColor)>,
) {
    // A placed model that is no longer in the world (despawned elsewhere) drops its record.
    if let Some((_, _, entity)) = panel.spawned {
        if live.get(entity).is_err() {
            panel.spawned = None;
        }
    }
    for mut visibility in &mut root {
        let want = if panel.open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
    }
    if !panel.open {
        return;
    }
    let len = panel.filtered.len();
    let selected_species = panel.selected_species(&names);
    let start = window_start(panel.selected, len, VISIBLE_ROWS);
    let species_label = selected_species.map_or_else(
        || "ninguna".to_string(),
        |species| {
            format!(
                "#{species:04} {} (forma {})",
                names.name_of(species),
                panel.selected_form()
            )
        },
    );
    let action = if len == 0 {
        "Nada que colocar".to_string()
    } else if panel.status.is_empty() {
        format!("Enter: coloca {species_label}")
    } else {
        format!("{}  |  {species_label}", panel.status)
    };
    for (text, mut value) in &mut texts {
        let next = match *text {
            PanelText::Title => "DEPURACION F7: COLOCAR POKEMON".to_string(),
            PanelText::Search => format!("Buscar: {}_", panel.query),
            PanelText::Count => {
                if names.is_empty() {
                    "sin nombres (data/national/names.tsv no encontrado)".to_string()
                } else {
                    format!("{} coincidencias de {}", len, names.len())
                }
            }
            PanelText::Action => action.clone(),
            PanelText::Form => {
                if panel.forms.is_empty() {
                    match selected_species {
                        Some(species) => format!("Sin modelo para #{species:04}"),
                        None => String::new(),
                    }
                } else {
                    let total = panel.forms.len();
                    let (form, name) = &panel.forms[panel.form_index.min(total - 1)];
                    if total > 1 {
                        format!("Forma {form}: {name}  ({total} formas - izq/der cambia)")
                    } else {
                        format!("Forma {form}: {name}")
                    }
                }
            }
            PanelText::Row(index) => panel
                .filtered
                .get(start + index)
                .and_then(|entry| names.entries.get(*entry))
                .map_or_else(String::new, |entry| {
                    format!("#{:04} {}", entry.id, entry.name)
                }),
            PanelText::Hint1 | PanelText::Hint2 => continue,
        };
        if value.0 != next {
            value.0 = next;
        }
    }
    for (row, mut background) in &mut rows {
        let selected = row.0 + start == panel.selected && row.0 < len;
        background.0 = if selected {
            Color::srgba(0.85, 0.6, 0.15, 0.95)
        } else {
            Color::srgba(0.10, 0.14, 0.20, 0.0)
        };
    }
}

// ------------------------------------------------------------------ plugin

/// The F7 species spawn selector (see the module key map). OFF by default.
#[derive(Debug, Clone, Default)]
pub struct DebugSpawnPlugin {
    /// Data directory holding `national/names.tsv`; discovered from the cwd by default.
    pub data_dir: Option<PathBuf>,
}

impl DebugSpawnPlugin {
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        Self {
            data_dir: Some(data_dir),
        }
    }
}

impl Plugin for DebugSpawnPlugin {
    fn build(&self, app: &mut App) {
        let data_dir = self.data_dir.clone().or_else(find_data_dir);
        let names = data_dir
            .as_ref()
            .and_then(|dir| {
                NationalNames::load(&dir.join("national/names.tsv"))
                    .map_err(|error| eprintln!("[debug-spawn] names sheet unavailable ({error})"))
                    .ok()
            })
            .unwrap_or_default();
        if !names.is_empty() {
            println!("[debug-spawn] names: {} national species", names.len());
        }
        app.insert_resource(names)
            .init_resource::<DebugSpawnPanel>()
            // Idempotent: the viewer's InputMapPlugin also initialises them; this keeps the
            // panel working in small apps and tests.
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Startup, spawn_panel)
            .add_systems(
                PreUpdate,
                (panel_input, block_game_input)
                    .chain()
                    .after(super::input_map::fill_action_state),
            )
            .add_systems(Update, sync_panel_ui);
    }
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "# comment\n\
national_number:u16\tidentifier:string\tname_en:string\tname_es:string\tname_ja_roma:string\n\
1\tbulbasaur\tBulbasaur\tBulbasaur\tFushigidane\n\
4\tcharmander\tCharmander\t\tHitokage\n\
5\tcharmeleon\tCharmeleon\t\tLizardo\n\
6\tcharizard\tCharizard\tCharizard\tLizardon\n\
25\tpikachu\tPikachu\tPikachu\tPikachu\n\
29\tnidoran-f\tNidoran♀\t\tNidoran♀\n\
83\tfarfetchd\tFarfetch’d\t\tKamonegi\n\
122\tmr-mime\tMr. Mime\t\tBarrierd\n\
233\tporygon2\tPorygon2\tPorygon2\tPorygon2\n\
669\tflabebe\tFlabebe\tFlabébé\tFlabebe\n";

    fn names() -> NationalNames {
        NationalNames::from_tsv(FIXTURE).unwrap()
    }

    fn hit_ids(query: &str) -> Vec<u16> {
        let names = names();
        filter_indices(&names.entries, query)
            .into_iter()
            .map(|index| names.entries[index].id)
            .collect()
    }

    #[test]
    fn sheet_parses_with_the_spanish_name_then_the_english_one() {
        let names = names();
        assert_eq!(names.len(), 10);
        assert_eq!(names.name_of(1), "Bulbasaur");
        assert_eq!(names.name_of(4), "Charmander", "empty Spanish falls back to English");
        assert_eq!(names.name_of(669), "Flabébé", "the Spanish name wins when present");
        assert_eq!(names.name_of(9999), "");
        assert!(names.entries.windows(2).all(|pair| pair[0].id <= pair[1].id));
    }

    #[test]
    fn filter_ignores_case_accents_and_punctuation() {
        assert_eq!(hit_ids(""), vec![1, 4, 5, 6, 25, 29, 83, 122, 233, 669]);
        assert_eq!(hit_ids("char"), vec![4, 5, 6], "the brief's example");
        assert_eq!(hit_ids("CHAR"), vec![4, 5, 6], "case-insensitive");
        assert_eq!(hit_ids("flabebe"), vec![669], "accents folded");
        assert_eq!(hit_ids("MR MIME"), vec![122], "space and dot folded");
        assert_eq!(hit_ids("mrmime"), vec![122]);
        assert_eq!(hit_ids("nidoran"), vec![29], "the gender symbol is dropped");
        assert_eq!(hit_ids("farfetchd"), vec![83], "the curly apostrophe is dropped");
        assert_eq!(hit_ids("zzz"), Vec::<u16>::new());
    }

    #[test]
    fn filter_accepts_the_national_number() {
        assert_eq!(hit_ids("6"), vec![6]);
        assert_eq!(hit_ids("06"), vec![6]);
        assert_eq!(hit_ids("#6"), vec![6]);
        assert_eq!(hit_ids("0006"), vec![6]);
        assert_eq!(hit_ids("0"), Vec::<u16>::new(), "0 is not a species");
        assert_eq!(hit_ids("122"), vec![122]);
        // Digits also match inside a name (Porygon2), so both rules coexist.
        assert_eq!(hit_ids("2"), vec![233]);
        assert_eq!(hit_ids("25"), vec![25]);
    }

    #[test]
    fn selection_wraps_and_the_window_follows_it() {
        assert_eq!(move_index(0, -1, 5), 4);
        assert_eq!(move_index(4, 1, 5), 0);
        assert_eq!(move_index(0, 1, 0), 0);
        assert_eq!(window_start(0, 1025, 12), 0);
        assert_eq!(window_start(11, 1025, 12), 5);
        assert_eq!(window_start(500, 1025, 12), 494);
        assert_eq!(window_start(1024, 1025, 12), 1013);
        assert_eq!(window_start(4, 5, 12), 0, "the window never runs past the list");
        assert_eq!(window_start(0, 0, 12), 0);
    }

    #[test]
    fn form_cycle_wraps_and_survives_empty_lists() {
        let forms = [(0, "base".to_string()), (1, "Mega X".to_string())];
        assert_eq!(cycle_form(&forms, 0, 1), 1);
        assert_eq!(cycle_form(&forms, 1, 1), 0, "wraps");
        assert_eq!(cycle_form(&forms, 0, -1), 1, "wraps backwards");
        assert_eq!(cycle_form(&[], 0, 1), 0);
    }

    #[test]
    fn typed_keys_map_to_characters() {
        assert_eq!(char_for_key(KeyCode::KeyC), Some('c'));
        assert_eq!(char_for_key(KeyCode::Digit6), Some('6'));
        assert_eq!(char_for_key(KeyCode::Numpad6), Some('6'));
        assert_eq!(char_for_key(KeyCode::Space), Some(' '));
        assert_eq!(char_for_key(KeyCode::Minus), Some('-'));
        assert_eq!(char_for_key(KeyCode::ArrowUp), None);
        assert_eq!(char_for_key(KeyCode::F7), None);
        assert_eq!(char_for_key(KeyCode::Delete), None);
    }

    // ---------------------------------------------------------- bevy glue

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release_all();
        app.update();
    }

    fn headless_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<bevy::gltf::Gltf>();
        app.add_plugins(DebugSpawnPlugin::with_data_dir(PathBuf::from(".")));
        // Replace the loaded (real) sheet with the fixture so the tests are deterministic.
        let fixture = NationalNames::from_tsv(FIXTURE).unwrap();
        app.insert_resource(fixture);
        app
    }

    #[test]
    fn f7_opens_freezes_and_closes_the_panel() {
        let mut app = headless_app();
        app.init_resource::<Paused>();
        app.init_resource::<PauseMenuSuppressed>();
        app.update();
        let actions = ActionState::default();
        app.insert_resource(actions);
        assert!(!app.world().resource::<DebugSpawnPanel>().open);

        press(&mut app, PANEL_KEY);
        {
            let panel = app.world().resource::<DebugSpawnPanel>();
            assert!(panel.open);
            assert_eq!(panel.filtered.len(), 10, "the fixture list is filtered in");
            assert!(app.world().resource::<Paused>().0, "the world frozen while open");
            assert!(app.world().resource::<PauseMenuSuppressed>().0);
        }

        // Typing filters; Backspace widens again; the selection follows the list.
        press(&mut app, KeyCode::KeyC);
        press(&mut app, KeyCode::KeyH);
        press(&mut app, KeyCode::KeyA);
        press(&mut app, KeyCode::KeyR);
        assert_eq!(app.world().resource::<DebugSpawnPanel>().query, "char");
        assert_eq!(app.world().resource::<DebugSpawnPanel>().filtered.len(), 3);
        press(&mut app, KeyCode::Backspace);
        assert_eq!(app.world().resource::<DebugSpawnPanel>().query, "cha");

        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(app.world().resource::<DebugSpawnPanel>().selected, 1);

        press(&mut app, KeyCode::Escape);
        let panel = app.world().resource::<DebugSpawnPanel>();
        assert!(!panel.open);
        assert!(!app.world().resource::<Paused>().0, "the world resumes on close");
        assert!(!app.world().resource::<PauseMenuSuppressed>().0);
    }

    #[test]
    fn f7_does_not_open_over_the_pause_menu_or_in_a_battle() {
        let mut app = headless_app();
        app.init_resource::<Paused>();
        app.init_resource::<PauseMenuSuppressed>();
        app.update();
        app.world_mut().resource_mut::<Paused>().0 = true;
        press(&mut app, PANEL_KEY);
        assert!(!app.world().resource::<DebugSpawnPanel>().open, "paused: no panel");

        app.world_mut().resource_mut::<Paused>().0 = false;
        app.insert_resource(State::new(GameState::Battle));
        press(&mut app, PANEL_KEY);
        assert!(!app.world().resource::<DebugSpawnPanel>().open, "battle: no panel");
        assert!(!app.world().resource::<Paused>().0, "and it did not freeze the battle");
    }

    #[test]
    fn enter_without_a_model_says_so_and_places_nothing() {
        let mut app = headless_app();
        app.init_resource::<Paused>();
        app.init_resource::<PauseMenuSuppressed>();
        app.world_mut()
            .spawn((super::super::PlayerAvatar, Transform::default()));
        app.update();
        press(&mut app, PANEL_KEY);
        // Every real species has a converted model now, so add a made-up species #9000 that no
        // model sheet knows.
        let invented = format!("{FIXTURE}9000\ttestmon\tTestmon\t\tTestmon\n");
        app.insert_resource(NationalNames::from_tsv(&invented).unwrap());
        for key in [KeyCode::Digit9, KeyCode::Digit0, KeyCode::Digit0, KeyCode::Digit0] {
            press(&mut app, key);
        }
        {
            let panel = app.world().resource::<DebugSpawnPanel>();
            assert_eq!(panel.query, "9000");
            assert_eq!(panel.selected_species(&app.world().resource::<NationalNames>()), Some(9000));
        }
        press(&mut app, KeyCode::Enter);
        let panel = app.world().resource::<DebugSpawnPanel>();
        assert!(panel.spawned.is_none(), "nothing placed");
        assert!(
            panel.status.starts_with("sin modelo para #9000"),
            "status: {}",
            panel.status
        );
    }

    #[test]
    fn enter_places_one_model_and_the_next_place_replaces_it() {
        let Some(mut app) = real_data_app_with_player() else {
            return;
        };
        press(&mut app, PANEL_KEY);
        // Pikachu (#25), which the game catalog holds as a batch model in the real cache.
        select_and_place_pikachu(&mut app);
        let (species, form, first) = {
            let panel = app.world().resource::<DebugSpawnPanel>();
            assert!(panel.status.starts_with("colocado"), "status: {}", panel.status);
            panel.spawned.expect("a model placed")
        };
        assert_eq!((species, form), (25, 0));
        assert!(app.world().get_entity(first).is_ok(), "the entity is in the world");
        let position = app
            .world()
            .get::<Transform>(first)
            .expect("transform")
            .translation;
        let distance = (position - Vec3::new(0.0, 1.0, 0.0)).length();
        assert!(
            (distance - SPAWN_DISTANCE).abs() < 0.01,
            "placed at {distance} m, expected {SPAWN_DISTANCE}"
        );
        assert!(position.z > 0.0 && position.x < 0.0, "front-right of the player: {position:?}");

        // Placing again replaces the previous model instead of piling up.
        press(&mut app, KeyCode::Enter);
        let second = app
            .world()
            .resource::<DebugSpawnPanel>()
            .spawned
            .expect("replaced model")
            .2;
        assert_ne!(first, second);
        assert!(app.world().get_entity(first).is_err(), "the old model is gone");
        assert!(app.world().get_entity(second).is_ok());

        // Supr removes it.
        press(&mut app, KeyCode::Delete);
        let panel = app.world().resource::<DebugSpawnPanel>();
        assert!(panel.spawned.is_none());
        assert_eq!(panel.status, "modelo quitado");
        assert!(app.world().get_entity(second).is_err());
    }

    /// With a camera the model goes to the free side of the view (the camera's left, opposite
    /// the panel) and a step beyond the player (never between the camera and the player) and
    /// faces the camera, so its front is visible.
    #[test]
    fn with_a_camera_the_model_goes_beside_the_view_and_faces_it() {
        let Some(mut app) = real_data_app_with_player() else {
            return;
        };
        // Camera at +Z looking towards -Z: screen-right is +X (where the panel sits), so the
        // free side is -X; "beyond the player" is -Z.
        app.world_mut().spawn((
            Camera3d::default(),
            Camera {
                is_active: true,
                ..default()
            },
            Transform::from_xyz(0.0, 2.0, 5.0),
            GlobalTransform::from_xyz(0.0, 2.0, 5.0),
        ));
        app.update();
        press(&mut app, PANEL_KEY);
        select_and_place_pikachu(&mut app);
        let entity = app
            .world()
            .resource::<DebugSpawnPanel>()
            .spawned
            .expect("a model placed")
            .2;
        let transform = *app.world().get::<Transform>(entity).expect("transform");
        let position = transform.translation;
        assert!(position.x < -1.0, "free (camera-left) side: {position:?}");
        assert!(position.z < 0.0, "one step beyond the player: {position:?}");
        let distance = position.xz().length();
        assert!(
            (distance - SPAWN_DISTANCE).abs() < 0.01,
            "placed at {distance} m, expected {SPAWN_DISTANCE}"
        );
        let facing = transform.rotation * Vec3::Z;
        assert!(facing.z > 0.9, "the model faces the camera: {facing:?}");
    }

    #[test]
    fn filter_finds_a_species_by_its_english_name_or_identifier() {
        // "Iron Boulder" is Ferromole in Spanish; the panel used to look at Spanish only.
        let sheet = "national_number:u16\tidentifier:string\tname_en:string\tname_es:string\tname_ja_roma:string\n\
1022\tiron-boulder\tIron Boulder\tFerromole\tTetsunoiwao\n";
        let names = NationalNames::from_tsv(sheet).unwrap();
        for query in ["Iron Boulder", "iron-boulder", "ironboulder", "Ferromole", "1022"] {
            assert_eq!(filter_indices(&names.entries, query), vec![0], "query {query}");
        }
        assert!(filter_indices(&names.entries, "bulbasaur").is_empty());
    }

    /// Every species 1..=1025 that `model_map.tsv` maps to a model must be listed by F7 with a
    /// resolvable model, found by its Spanish name, its English name and its number.
    #[test]
    fn every_mapped_species_is_listed_and_resolves_a_model() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/national");
        let Ok(names) = NationalNames::load(&data.join("names.tsv")) else {
            eprintln!("[skip] no names sheet");
            return;
        };
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../work/cache/pokemon_national");
        let have_cache = std::env::var("PLA_CACHE_ROOT")
            .map(|root| PathBuf::from(root).join("pokemon_national").is_dir())
            .unwrap_or(false)
            || cache.is_dir();
        if !have_cache {
            eprintln!("[skip] no pokemon_national cache");
            return;
        }
        let map = std::fs::read_to_string(data.join("model_map.tsv")).unwrap();
        let mut mapped = std::collections::BTreeSet::new();
        for line in map.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).skip(1) {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() > 5 && f[5] != "DESCONOCIDO" {
                if let Ok(id) = f[1].parse::<u16>() {
                    if (1..=1025).contains(&id) {
                        mapped.insert(id);
                    }
                }
            }
        }
        assert!(mapped.len() > 900, "model_map maps {} species", mapped.len());
        let english: std::collections::HashMap<u16, String> =
            std::fs::read_to_string(data.join("species.tsv"))
                .unwrap()
                .lines()
                .filter(|l| !l.starts_with('#'))
                .filter_map(|l| {
                    let f: Vec<&str> = l.split('\t').collect();
                    Some((f.first()?.parse().ok()?, f.get(17)?.to_string()))
                })
                .collect();
        let mut problems = Vec::new();
        for id in mapped {
            if resolve_model(id, 0).is_none() && resolve_forms(id).is_empty() {
                problems.push(format!("#{id} no model"));
            }
            let listed = |q: &str| {
                filter_indices(&names.entries, q)
                    .into_iter()
                    .any(|i| names.entries[i].id == id)
            };
            if !listed(names.name_of(id)) {
                problems.push(format!("#{id} not found by Spanish name"));
            }
            if let Some(en) = english.get(&id) {
                if !en.is_empty() && !listed(en) {
                    problems.push(format!("#{id} not found by English name {en}"));
                }
            }
            if !listed(&id.to_string()) {
                problems.push(format!("#{id} not found by number"));
            }
        }
        assert!(problems.is_empty(), "F7 gaps: {problems:#?}");
    }

    /// Zygarde's folders pm0770_11..15 and _51 are assigned to the official forms.
    #[test]
    fn zygarde_forms_are_all_listed() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../work/cache/pokemon_national");
        let have_cache = std::env::var("PLA_CACHE_ROOT")
            .map(|root| PathBuf::from(root).join("pokemon_national").is_dir())
            .unwrap_or(false)
            || cache.is_dir();
        if !have_cache {
            eprintln!("[skip] no pokemon_national cache");
            return;
        }
        let forms = resolve_forms(718);
        let names: Vec<(u8, &str)> = forms.iter().map(|(f, n)| (*f, n.as_str())).collect();
        for expected in [
            (0, "50% Forme"),
            (3, "Complete Forme"),
            (4, "10% Forme"),
            (5, "Mega Zygarde"),
            (6, "Zygarde Cell"),
            (7, "Zygarde Core"),
        ] {
            assert!(names.contains(&expected), "missing {expected:?} in {names:?}");
        }
    }

    /// App with the REAL names sheet, the real model cache and a player at the origin. `None`
    /// (and a `[skip]` note) when the converted collection is not installed.
    fn real_data_app_with_player() -> Option<App> {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../work/cache/pokemon_national");
        let cache = std::env::var("PLA_CACHE_ROOT")
            .map(|root| PathBuf::from(root).join("pokemon_national"))
            .ok()
            .filter(|path| path.is_dir())
            .unwrap_or(cache);
        if !cache.is_dir() {
            eprintln!("[skip] no pokemon_national cache at {}", cache.display());
            return None;
        }
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<bevy::gltf::Gltf>();
        app.add_plugins(DebugSpawnPlugin::with_data_dir(PathBuf::from(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )));
        app.init_resource::<Paused>();
        app.init_resource::<PauseMenuSuppressed>();
        // A player at the origin looking towards +Z (identity rotation).
        app.world_mut().spawn((
            super::super::PlayerAvatar,
            Transform::from_xyz(0.0, 1.0, 0.0),
        ));
        app.update();
        Some(app)
    }

    /// Open the panel (already open), type `25` and press Enter: Pikachu, which the game
    /// catalog holds as a batch model in the real cache.
    fn select_and_place_pikachu(app: &mut App) {
        for key in [KeyCode::Digit2, KeyCode::Digit5] {
            press(app, key);
        }
        press(app, KeyCode::Enter);
    }

    #[test]
    fn blocked_input_never_reaches_the_game() {
        let mut app = headless_app();
        app.init_resource::<Paused>();
        app.init_resource::<PauseMenuSuppressed>();
        app.insert_resource(ActionState::default());
        app.update();
        press(&mut app, PANEL_KEY);
        // A key press while the panel is open leaves no action edge for the game this frame.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        let actions = app.world().resource::<ActionState>();
        assert!(!actions.just_pressed(Action::Confirm), "Confirm stays blocked");
        assert_eq!(actions.move_axis(), Vec2::ZERO);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release_all();
        app.update();
    }
}
