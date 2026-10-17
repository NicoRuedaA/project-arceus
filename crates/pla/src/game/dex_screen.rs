//! Evidence-bounded Pokédex screen.
//!
//! The Hisui catalog is made only from active rows in the tracked `pml_personal.tsv` sheet.
//! Forms are collapsed to one deterministic entry per national species id; no species, localized
//! name, or sprite is invented when the sheet does not provide it.  The UI deliberately uses the
//! numeric id and the sheet's development label rather than claiming localization.
//!
//! A second, read-only mode lists the 1025 national species from `data/national/species.tsv`
//! (EXTERNAL/PROBABLE data layer, no sprites or research tasks yet).  It is opt-in: the screen
//! still opens on the Hisui list, and `Y` toggles between both lists (see `DexMode`).  The
//! national list shows the localized names from the sheet because it is a data listing, not a
//! play-state screen; seen/caught silhouettes stay a Hisui-mode behaviour only.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::dex::{
    rank_for_points, rank_points, species_level, species_points, DexTasks, ResearchCounters,
};
use super::input_map::{Action, ActionState, InputMapPlugin};
use super::pause_menu::{freeze_for_screen, DexOpenRequested, PauseMenuSuppressed};
use super::save_wire::PlayerProgress;
use super::state::Paused;

/// Number of catalog rows shown at once.
pub const VISIBLE_ROWS: usize = 12;

/// Lines of the selected species' research-task panel.
pub const TASK_LINES: usize = 3;

/// One distinct species in the Pokédex catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DexEntry {
    pub id: u16,
    /// Numeric id plus the tracked `species_name` development label, when present.
    pub label: String,
}

/// Which list the Pokédex screen shows.  `Hisui` is the default and keeps the pre-existing
/// behaviour (active species + research; the national data layer added `National`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DexMode {
    #[default]
    Hisui,
    National,
}

impl DexMode {
    pub fn toggled(self) -> Self {
        match self {
            Self::Hisui => Self::National,
            Self::National => Self::Hisui,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Hisui => "HISUI",
            Self::National => "NATIONAL",
        }
    }
}

/// Deterministic species catalog built from active `pml_personal.tsv` rows, plus the national
/// listing from `data/national/species.tsv` when that sheet is present.
#[derive(Debug, Clone, Default, PartialEq, Eq, Resource)]
pub struct DexCatalog {
    pub entries: Vec<DexEntry>,
    /// National listing (1..1025) from the external data layer; empty when the sheet is missing.
    pub national: Vec<DexEntry>,
}

impl DexCatalog {
    /// Parse only the personal sheet, keeping this screen independent from other data loaders.
    pub fn from_personal_tsv(text: &str) -> Result<Self, String> {
        Ok(Self {
            entries: Self::hisui_entries_from_tsv(text)?,
            national: Vec::new(),
        })
    }

    fn hisui_entries_from_tsv(text: &str) -> Result<Vec<DexEntry>, String> {
        let mut lines = text
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'));
        let headers: Vec<&str> = lines
            .next()
            .ok_or_else(|| "pml_personal: missing header".to_string())?
            .split('\t')
            .map(|field| field.split_once(':').map_or(field, |(name, _)| name))
            .collect();
        let column = |name: &str| {
            headers
                .iter()
                .position(|header| *header == name)
                .ok_or_else(|| format!("pml_personal: missing column {name}"))
        };
        let (id_column, name_column, form_column, active_column) = (
            column("dex_index_national")?,
            column("species_name")?,
            column("form")?,
            column("is_active")?,
        );

        let mut rows = Vec::new();
        for (line_index, line) in lines.enumerate() {
            let values: Vec<&str> = line.split('\t').collect();
            if values.len() != headers.len() {
                return Err(format!(
                    "pml_personal row {}: expected {} fields, got {}",
                    line_index + 2,
                    headers.len(),
                    values.len()
                ));
            }
            let active = match values[active_column] {
                "1" | "true" => true,
                "0" | "false" => false,
                _ => {
                    return Err(format!(
                        "pml_personal row {}: invalid is_active",
                        line_index + 2
                    ))
                }
            };
            if !active {
                continue;
            }
            let id = values[id_column]
                .parse::<u16>()
                .map_err(|_| format!("pml_personal row {}: invalid dex id", line_index + 2))?;
            let form = values[form_column]
                .parse::<u8>()
                .map_err(|_| format!("pml_personal row {}: invalid form", line_index + 2))?;
            rows.push((id, form, values[name_column].to_owned()));
        }

        // Form order is explicit so the result remains stable if the source sheet is reordered.
        rows.sort_by(|left, right| left.cmp(right));
        let mut labels = BTreeMap::<u16, String>::new();
        for (id, _form, name) in rows {
            labels
                .entry(id)
                .or_insert_with(|| development_label(id, &name));
        }
        Ok(labels
            .into_iter()
            .map(|(id, label)| DexEntry { id, label })
            .collect())
    }

    /// Parse the national species sheet into one entry per species (1..1025), using the English
    /// name with the kebab identifier as fallback.
    pub fn national_entries_from_tsv(text: &str) -> Result<Vec<DexEntry>, String> {
        let mut lines = text
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'));
        let headers: Vec<&str> = lines
            .next()
            .ok_or_else(|| "national species: missing header".to_string())?
            .split('\t')
            .map(|field| field.split_once(':').map_or(field, |(name, _)| name))
            .collect();
        let column = |name: &str| {
            headers
                .iter()
                .position(|header| *header == name)
                .ok_or_else(|| format!("national species: missing column {name}"))
        };
        let (id_column, name_en, name_es, identifier) = (
            column("national_number")?,
            column("name_en")?,
            column("name_es")?,
            column("identifier")?,
        );
        let mut entries = Vec::new();
        for (line_index, line) in lines.enumerate() {
            let values: Vec<&str> = line.split('\t').collect();
            if values.len() != headers.len() {
                return Err(format!(
                    "national species row {}: expected {} fields, got {}",
                    line_index + 2,
                    headers.len(),
                    values.len()
                ));
            }
            let id = values[id_column]
                .parse::<u16>()
                .map_err(|_| format!("national species row {}: invalid id", line_index + 2))?;
            let display = if !values[name_es].is_empty() {
                values[name_es]
            } else if !values[name_en].is_empty() {
                values[name_en]
            } else {
                values[identifier]
            };
            entries.push(DexEntry {
                id,
                label: format!("#{id:04} {display}"),
            });
        }
        entries.sort_by_key(|entry| entry.id);
        Ok(entries)
    }

    /// Load `pml_personal.tsv` (required) and `national/species.tsv` (optional) from a tracked
    /// data directory.
    pub fn load(data_dir: impl AsRef<Path>) -> Result<Self, String> {
        let dir = data_dir.as_ref();
        let personal = dir.join("pml_personal.tsv");
        let text = fs::read_to_string(&personal)
            .map_err(|error| format!("{}: {error}", personal.display()))?;
        let national = fs::read_to_string(dir.join("national/species.tsv"))
            .ok()
            .and_then(|text| {
                Self::national_entries_from_tsv(&text)
                    .map_err(|error| eprintln!("[dex] national species sheet rejected ({error})"))
                    .ok()
            })
            .unwrap_or_default();
        let mut catalog = Self::from_personal_tsv(&text)?;
        catalog.national = national;
        Ok(catalog)
    }

    pub fn distinct_species_count(&self) -> usize {
        self.entries.len()
    }

    pub fn national_count(&self) -> usize {
        self.national.len()
    }
}

/// Entries of the active list.
pub fn active_entries(catalog: &DexCatalog, mode: DexMode) -> &[DexEntry] {
    match mode {
        DexMode::Hisui => &catalog.entries,
        DexMode::National => &catalog.national,
    }
}

/// Species name of a catalog entry (the label without its "#NNNN " prefix). Labels that carry
/// only the numeric id ("#0905") are returned as they are.
pub fn entry_display_name(entry: &DexEntry) -> &str {
    entry
        .label
        .split_once(' ')
        .map_or(entry.label.as_str(), |(_, name)| name)
}

fn development_label(id: u16, name: &str) -> String {
    if name.is_empty() || name.starts_with("species_") {
        format!("#{id:04}")
    } else {
        format!("#{id:04} {name}")
    }
}

/// Information available for one catalog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DexVisibility {
    /// Neither the id nor a name is disclosed.
    Hidden,
    /// The entry has been seen but not caught; the UI renders a silhouette marker.
    SeenSilhouette,
    /// The entry is caught; the tracked development label is shown.
    Caught,
}

pub fn visibility(id: u16, seen: &BTreeSet<u16>, caught: &BTreeSet<u16>) -> DexVisibility {
    if caught.contains(&id) {
        DexVisibility::Caught
    } else if seen.contains(&id) {
        DexVisibility::SeenSilhouette
    } else {
        DexVisibility::Hidden
    }
}

/// Text for a row, with no name fabricated for hidden or silhouette entries.
pub fn row_label(entry: &DexEntry, state: DexVisibility) -> String {
    match state {
        DexVisibility::Hidden => format!("{}  ??????", numeric_id(entry.id)),
        DexVisibility::SeenSilhouette => format!("{}  SILHOUETTE", numeric_id(entry.id)),
        DexVisibility::Caught => entry.label.clone(),
    }
}

/// Research suffix of a row: the species' research level and points, only once the species is
/// registered (caught at least once). Empty for every other entry.
pub fn research_suffix(tasks: &DexTasks, counters: &ResearchCounters, species: u16) -> String {
    if !super::dex::is_registered(counters, species) {
        return String::new();
    }
    let points = species_points(&tasks.entries, counters, species);
    format!("  ·  lv{} {}p", species_level(points), points)
}

/// Status line: page, player rank (from the research counters and the decoded task catalog) and
/// the seen/caught counts. At the top rank the next threshold is omitted.
pub fn rank_line(tasks: &DexTasks, counters: &ResearchCounters) -> String {
    let points = rank_points(tasks, counters);
    let rank = rank_for_points(points);
    match super::dex::next_rank_at(points) {
        Some((_, needed)) => format!("Rank {rank}  |  {points} pts  |  next rank at {needed}"),
        None => format!("Rank {rank}  |  {points} pts  |  MAX"),
    }
}

/// Lines of the selected species' research tasks (one per task, capped at `max_lines`): kind,
/// progress against the task's last threshold, level and points. The first line names the
/// species' general state; an unregistered species (never caught) shows a single placeholder.
pub fn task_lines(
    tasks: &DexTasks,
    counters: &ResearchCounters,
    species: u16,
    max_lines: usize,
) -> Vec<String> {
    if !super::dex::is_registered(counters, species) {
        return vec!["Research: not registered (catch it first)".to_string()];
    }
    let points = species_points(&tasks.entries, counters, species);
    let head = format!(
        "Research: lv{}  |  {} pts{}",
        species_level(points),
        points,
        if super::dex::species_perfect(&tasks.entries, counters, species) {
            "  |  PERFECT"
        } else {
            ""
        }
    );
    let mut lines = vec![head];
    // Tasks with progress show first (they are the ones the player is working on), then the rest
    // in slot order.
    let mut ordered: Vec<(&crate::game::dex::DexTask, u32)> = tasks
        .tasks_for(species)
        .map(|task| {
            let count = counters
                .get(&(task.species, task.kind, task.counter_target()))
                .copied()
                .unwrap_or(0);
            (task, count)
        })
        .collect();
    ordered.sort_by_key(|(task, count)| (*count == 0, task.slot));
    let mut listed = 0;
    for (task, count) in &ordered {
        if listed + 1 >= max_lines {
            break;
        }
        let last = task
            .thresholds
            .iter()
            .take(usize::from(task.steps))
            .next_back()
            .copied()
            .unwrap_or(0);
        let label = if task.kind == super::dex::KIND_USED_MOVE {
            format!("kind {} (move {})", task.kind, task.move_target)
        } else {
            format!("kind {}", task.kind)
        };
        lines.push(format!(
            "  {label}: {count}/{last}  lv{}  {}p",
            task.level(*count),
            task.points(*count)
        ));
        listed += 1;
    }
    let rest = ordered.len().saturating_sub(listed);
    if rest > 0 {
        lines.push(format!("  +{rest} more task(s)"));
    }
    lines.truncate(max_lines);
    lines
}

fn numeric_id(id: u16) -> String {
    format!("#{id:04}")
}

/// Wrapping selection move. Empty catalogs remain at index zero.
pub fn move_selection(selected: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    ((selected.min(len - 1) as i32 + delta).rem_euclid(len as i32)) as usize
}

pub fn page_count(len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (len - 1) / VISIBLE_ROWS + 1
    }
}

pub fn page_for_selection(selected: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        selected.min(len - 1) / VISIBLE_ROWS
    }
}

/// Select an offset within a page, clamping to the last real entry.
pub fn select_page(page: usize, offset: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let last_page = page_count(len) - 1;
    let start = page.min(last_page) * VISIBLE_ROWS;
    (start + offset.min(VISIBLE_ROWS - 1)).min(len - 1)
}

/// Catalog slice for a page. Page indices outside the catalog return an empty slice.
pub fn page_entries(catalog: &DexCatalog, page: usize) -> &[DexEntry] {
    page_entries_of(&catalog.entries, page)
}

/// Page slice of any entry list (Hisui or national).
pub fn page_entries_of(entries: &[DexEntry], page: usize) -> &[DexEntry] {
    let start = page.saturating_mul(VISIBLE_ROWS);
    if start >= entries.len() {
        return &[];
    }
    let end = (start + VISIBLE_ROWS).min(entries.len());
    &entries[start..end]
}

/// Pure screen state. A caller opens it by setting `open` or, in the viewer, through the pause
/// menu's `Pokédex` row ([`DexOpenRequested`]), which also freezes the world until it closes.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct DexUi {
    pub open: bool,
    pub selected: usize,
    pub page: usize,
    /// Hisui (default, unchanged behaviour) or the national data listing.
    pub mode: DexMode,
}

impl Default for DexUi {
    fn default() -> Self {
        Self {
            open: false,
            selected: 0,
            page: 0,
            mode: DexMode::Hisui,
        }
    }
}

/// Root of the provisional Pokédex (hidden while the original one is shown, `orig_ui`).
#[derive(Component)]
pub struct DexRoot;

#[derive(Component, Clone, Copy)]
enum DexText {
    Title,
    Status,
    Row(usize),
    /// One line of the selected species' research-task panel.
    Task(usize),
    Hint,
}

#[derive(Component, Clone, Copy)]
struct DexRow(usize);

/// Bevy UI plugin for the independent Pokédex screen.
pub struct DexScreenPlugin {
    pub data_dir: Option<PathBuf>,
}

impl Default for DexScreenPlugin {
    fn default() -> Self {
        Self { data_dir: None }
    }
}

impl DexScreenPlugin {
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        Self {
            data_dir: Some(data_dir),
        }
    }
}

fn find_dex_data_dir() -> Option<PathBuf> {
    [
        PathBuf::from("data"),
        PathBuf::from("../../data"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
    ]
    .into_iter()
    .find(|path| path.join("pml_personal.tsv").is_file())
}

impl Plugin for DexScreenPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<InputMapPlugin>() {
            app.add_plugins(InputMapPlugin);
        }
        let data_dir = self.data_dir.clone().or_else(find_dex_data_dir);
        let catalog = data_dir.as_ref().and_then(|path| {
            DexCatalog::load(path)
                .map_err(|error| eprintln!("[dex] personal table unavailable ({error})"))
                .ok()
        });
        let tasks = data_dir.as_ref().and_then(|path| {
            DexTasks::load(path)
                .map_err(|error| eprintln!("[dex] task table unavailable ({error})"))
                .ok()
        });
        if let Some(catalog) = &catalog {
            println!(
                "[dex] catalog: {} distinct active species, {} national species",
                catalog.distinct_species_count(),
                catalog.national_count()
            );
        }
        if let Some(tasks) = &tasks {
            println!("[dex] research tasks: {} rows", tasks.entries.len());
        }
        app.insert_resource(catalog.unwrap_or_default())
            .insert_resource(tasks.unwrap_or_default())
            .init_resource::<DexUi>()
            .init_resource::<DexOpenRequested>()
            .add_systems(Startup, spawn_dex_ui)
            .add_systems(Update, (dex_open_requested, dex_input, sync_dex_ui).chain());
    }
}

fn spawn_dex_ui(mut commands: Commands) {
    commands
        .spawn((
            DexRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.76)),
            GlobalZIndex(125),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(820.0),
                    max_height: Val::Percent(88.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(20.0)),
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.13, 0.98)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    DexText::Title,
                    Text::new("POKÉDEX"),
                    TextFont::from_font_size(30.0),
                    TextColor(Color::srgb(0.95, 0.9, 0.6)),
                ));
                panel.spawn((
                    DexText::Status,
                    Text::new(""),
                    TextFont::from_font_size(17.0),
                    TextColor(Color::srgb(0.75, 0.85, 1.0)),
                ));
                for index in 0..VISIBLE_ROWS {
                    panel
                        .spawn((
                            DexRow(index),
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.10, 0.14, 0.20, 0.92)),
                        ))
                        .with_children(|row| {
                            row.spawn((
                                DexText::Row(index),
                                Text::new(""),
                                TextFont::from_font_size(19.0),
                                TextColor(Color::WHITE),
                            ));
                        });
                }
                panel.spawn((
                    DexText::Hint,
                    Text::new("Up/Down: select  |  Y: national/Hisui  |  Esc: close"),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
                ));
                panel.spawn((
                    Text::new("RESEARCH (selected)"),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::srgb(0.95, 0.9, 0.6)),
                ));
                for index in 0..TASK_LINES {
                    panel.spawn((
                        DexText::Task(index),
                        Text::new(""),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(0.8, 0.9, 0.85)),
                    ));
                }
            });
        });
}

fn dex_open_requested(
    mut request: ResMut<DexOpenRequested>,
    mut ui: ResMut<DexUi>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
) {
    if request.0 {
        request.0 = false;
        ui.open = true;
        freeze_for_screen(&mut paused, &mut suppressed, true);
    }
}

fn dex_input(
    mut actions: ResMut<ActionState>,
    catalog: Res<DexCatalog>,
    mut ui: ResMut<DexUi>,
    buttons: Query<(&DexRow, &Interaction), Changed<Interaction>>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
) {
    if !ui.open {
        return;
    }
    if actions.just_pressed(Action::Cancel) || actions.just_pressed(Action::Menu) {
        actions.consume(Action::Cancel);
        actions.consume(Action::Menu);
        ui.open = false;
        freeze_for_screen(&mut paused, &mut suppressed, false);
        return;
    }
    // Y: switch between the Hisui list and the national data listing; the selection restarts at
    // the top of the new list so page and selection always stay valid.
    if actions.just_pressed(Action::DexMode) {
        actions.consume(Action::DexMode);
        ui.mode = ui.mode.toggled();
        ui.selected = 0;
        ui.page = 0;
    }
    let len = active_entries(&catalog, ui.mode).len();
    let mut delta = 0;
    if actions.just_pressed(Action::Prev) {
        actions.consume(Action::Prev);
        delta -= 1;
    }
    if actions.just_pressed(Action::Next) {
        actions.consume(Action::Next);
        delta += 1;
    }
    for (row, interaction) in &buttons {
        match interaction {
            Interaction::Hovered => {
                ui.selected = select_page(ui.page, row.0, len);
            }
            Interaction::Pressed => {
                ui.selected = select_page(ui.page, row.0, len);
            }
            Interaction::None => {}
        }
    }
    if delta != 0 {
        ui.selected = move_selection(ui.selected, delta, len);
    }
}

fn sync_dex_ui(
    catalog: Res<DexCatalog>,
    tasks: Res<DexTasks>,
    progress: Option<Res<PlayerProgress>>,
    debug: Option<Res<super::debug_visual::DebugViewState>>,
    mut ui: ResMut<DexUi>,
    mut root: Query<&mut Visibility, With<DexRoot>>,
    mut texts: Query<(&DexText, &mut Text)>,
    mut rows: Query<(&DexRow, &mut BackgroundColor)>,
) {
    let entries = active_entries(&catalog, ui.mode);
    let len = entries.len();
    ui.selected = if len == 0 {
        0
    } else {
        ui.selected.min(len - 1)
    };
    ui.page = page_for_selection(ui.selected, len);
    let empty = BTreeSet::new();
    let empty_counters = ResearchCounters::new();
    let seen = progress.as_ref().map_or(&empty, |save| &save.seen);
    let caught = progress.as_ref().map_or(&empty, |save| &save.caught);
    let counters = progress
        .as_ref()
        .map_or(&empty_counters, |save| &save.research);
    // F6 (debug visual): every entry reads as seen + caught for the display; the save and the
    // research counters stay the player's real ones (this flag only changes what is drawn).
    let all = debug
        .as_ref()
        .is_some_and(|state| state.pokedex_all_active());
    let seen_count = if all {
        len
    } else {
        entries.iter().filter(|entry| seen.contains(&entry.id)).count()
    };
    let caught_count = if all {
        len
    } else {
        entries
            .iter()
            .filter(|entry| caught.contains(&entry.id))
            .count()
    };
    let page = page_entries_of(entries, ui.page);

    for mut visibility in &mut root {
        *visibility = if ui.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for (text, mut value) in &mut texts {
        let next = match (*text, ui.mode) {
            (DexText::Title, mode) => format!(
                "POKÉDEX  \u{2014}  {}  \u{2014}  {len}",
                match mode {
                    DexMode::Hisui => "HISUI",
                    DexMode::National => "NATIONAL",
                }
            ),
            (DexText::Status, DexMode::Hisui) => format!(
                "{}  |  Page {}/{}  |  seen {seen_count}  |  caught {caught_count}  |  total {len}{}",
                rank_line(&tasks, counters),
                ui.page + 1,
                page_count(len).max(1),
                if all { "  |  F6: TODAS (debug)" } else { "" },
            ),
            (DexText::Status, DexMode::National) => format!(
                "external data (no research yet)  |  Page {}/{}  |  seen {seen_count}  |  caught {caught_count}  |  total {len}{}",
                ui.page + 1,
                page_count(len).max(1),
                if all { "  |  F6: TODAS (debug)" } else { "" },
            ),
            (DexText::Hint, _) => {
                "Up/Down: select  |  Y: national/Hisui  |  Esc: close".to_string()
            }
            (DexText::Row(index), DexMode::Hisui) => {
                page.get(index).map_or_else(String::new, |entry| {
                    let state = if all {
                        DexVisibility::Caught
                    } else {
                        visibility(entry.id, seen, caught)
                    };
                    format!(
                        "{}{}",
                        row_label(entry, state),
                        research_suffix(&tasks, counters, entry.id)
                    )
                })
            }
            // The national list is a data listing: every name is visible, no silhouettes.
            (DexText::Row(index), DexMode::National) => {
                page.get(index).map_or_else(String::new, |entry| entry.label.clone())
            }
            (DexText::Task(index), DexMode::Hisui) => page
                .get(ui.selected % VISIBLE_ROWS)
                .map(|entry| {
                    task_lines(&tasks, counters, entry.id, TASK_LINES)
                        .get(index)
                        .cloned()
                        .unwrap_or_default()
                })
                .unwrap_or_default(),
            (DexText::Task(index), DexMode::National) => match index {
                0 => "National entry: stats/types/learnsets from data/national".to_string(),
                1 => "Models, icons and research tasks arrive in later phases".to_string(),
                _ => String::new(),
            },
        };
        if value.0 != next {
            value.0 = next;
        }
    }
    for (row, mut background) in &mut rows {
        let selected = row.0 == ui.selected % VISIBLE_ROWS;
        background.0 = if selected {
            Color::srgba(0.85, 0.6, 0.15, 0.95)
        } else {
            Color::srgba(0.10, 0.14, 0.20, 0.92)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "# comment\n\
dex_index_national:u16\tspecies_name:string\tform:u8\tis_active:bool\n\
25\tpikachu\t0\t1\n\
25\tpikachu\t1\t1\n\
1\tbulbasaur\t0\t0\n\
905\tspecies_905\t0\t1\n";

    const NATIONAL_FIXTURE: &str = "# comment\n\
national_number:u16\tidentifier:string\tname_en:string\tname_es:string\n\
1\tbulbasaur\tBulbasaur\tBulbasaur\n\
25\tpikachu\tPikachu\tPikachu\n\
1025\tpecharunt\tPecharunt\tPecharunt\n";

    fn press_once(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(key);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }

    fn row_text(app: &mut App, index: usize) -> String {
        let mut query = app.world_mut().query::<(&DexText, &Text)>();
        query
            .iter(app.world())
            .find_map(|(text, value)| matches!(text, DexText::Row(i) if *i == index).then(|| value.0.clone()))
            .unwrap_or_default()
    }

    #[test]
    fn catalog_is_distinct_deterministic_and_does_not_fabricate_species() {
        let catalog = DexCatalog::from_personal_tsv(FIXTURE).unwrap();
        assert_eq!(catalog.distinct_species_count(), 2);
        assert_eq!(
            catalog.entries[0],
            DexEntry {
                id: 25,
                label: "#0025 pikachu".into()
            }
        );
        assert_eq!(
            catalog.entries[1],
            DexEntry {
                id: 905,
                label: "#0905".into()
            }
        );
        assert_eq!(
            catalog
                .entries
                .iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            [25, 905]
        );
    }

    #[test]
    fn real_personal_sheet_has_the_measured_active_species_count() {
        let text = include_str!("../../../../data/pml_personal.tsv");
        let catalog = DexCatalog::from_personal_tsv(text).unwrap();
        assert_eq!(catalog.distinct_species_count(), 242);
        assert!(catalog
            .entries
            .windows(2)
            .all(|pair| pair[0].id < pair[1].id));
    }

    #[test]
    fn visibility_prefers_caught_and_marks_seen_uncaught_as_silhouette() {
        let seen = BTreeSet::from([25, 26]);
        let caught = BTreeSet::from([26]);
        assert_eq!(visibility(1, &seen, &caught), DexVisibility::Hidden);
        assert_eq!(
            visibility(25, &seen, &caught),
            DexVisibility::SeenSilhouette
        );
        assert_eq!(visibility(26, &seen, &caught), DexVisibility::Caught);
        let entry = DexEntry {
            id: 25,
            label: "#0025 pikachu".into(),
        };
        assert_eq!(
            row_label(&entry, DexVisibility::SeenSilhouette),
            "#0025  SILHOUETTE"
        );
        assert_eq!(row_label(&entry, DexVisibility::Hidden), "#0025  ??????");
    }

    #[test]
    fn selection_wraps_and_pages_are_stable() {
        assert_eq!(move_selection(0, -1, 25), 24);
        assert_eq!(move_selection(24, 1, 25), 0);
        assert_eq!(move_selection(0, 1, 0), 0);
        assert_eq!(page_count(0), 0);
        assert_eq!(page_count(25), 3);
        assert_eq!(page_for_selection(11, 25), 0);
        assert_eq!(page_for_selection(12, 25), 1);
        assert_eq!(select_page(1, 0, 25), 12);
        assert_eq!(select_page(2, 99, 25), 24);
        let catalog = DexCatalog {
            entries: (0..25)
                .map(|id| DexEntry {
                    id,
                    label: format!("#{id:04}"),
                })
                .collect(),
            national: Vec::new(),
        };
        assert_eq!(page_entries(&catalog, 1).len(), 12);
        assert_eq!(page_entries(&catalog, 2).len(), 1);
        assert!(page_entries(&catalog, 3).is_empty());
    }

    #[test]
    fn malformed_personal_rows_are_rejected_instead_of_filled() {
        assert!(DexCatalog::from_personal_tsv(
            "dex_index_national:u16\tspecies_name:string\tform:u8\tis_active:bool\n1\na\t0\t1\n"
        )
        .is_err());
        assert!(DexCatalog::from_personal_tsv(
            "dex_index_national:u16\tspecies_name:string\tform:u8\tis_active:bool\n1\na\t0\tmaybe\n"
        )
        .is_err());
    }

    /// The research suffix (level + points) appears only for registered species, and the status
    /// line reports the rank the counters reach.
    #[test]
    fn research_suffix_and_rank_line_follow_the_counters() {
        use super::super::dex::{DexTask, KIND_CAUGHT};

        let tasks = DexTasks {
            entries: vec![DexTask {
                species: 25,
                slot: 0,
                kind: KIND_CAUGHT,
                move_target: 0,
                type_byte: 0,
                param5: 0,
                field14: 10,
                field15: 0,
                thresholds: [1, 2, 4, 6, 10],
                steps: 5,
            }],
        };
        let mut counters = ResearchCounters::new();
        assert_eq!(research_suffix(&tasks, &counters, 25), "", "unregistered: no suffix");
        assert_eq!(rank_line(&tasks, &counters), "Rank 0  |  0 pts  |  next rank at 500");
        counters.insert((25, KIND_CAUGHT, 0), 4);
        assert_eq!(research_suffix(&tasks, &counters, 25), "  ·  lv4 30p");
        assert_eq!(rank_line(&tasks, &counters), "Rank 0  |  30 pts  |  next rank at 500");
    }

    /// The task panel lists the selected species' tasks with their counters and thresholds; an
    /// unregistered species shows the placeholder and the line count respects the cap.
    #[test]
    fn task_lines_show_counters_and_respect_the_cap() {
        use super::super::dex::{DexTask, KIND_CAUGHT, KIND_USED_MOVE};

        let tasks = DexTasks {
            entries: vec![
                DexTask {
                    species: 25,
                    slot: 0,
                    kind: KIND_CAUGHT,
                    move_target: 0,
                    type_byte: 0,
                    param5: 0,
                    field14: 10,
                    field15: 0,
                    thresholds: [1, 2, 4, 6, 10],
                    steps: 5,
                },
                DexTask {
                    species: 25,
                    slot: 1,
                    kind: KIND_USED_MOVE,
                    move_target: 84,
                    type_byte: 0,
                    param5: 0,
                    field14: 10,
                    field15: 0,
                    thresholds: [3, 6, 12, 25, 0],
                    steps: 4,
                },
            ],
        };
        let mut counters = ResearchCounters::new();
        assert_eq!(
            task_lines(&tasks, &counters, 25, TASK_LINES),
            vec!["Research: not registered (catch it first)"]
        );
        counters.insert((25, KIND_CAUGHT, 0), 4);
        counters.insert((25, KIND_USED_MOVE, 84), 6);
        let lines = task_lines(&tasks, &counters, 25, TASK_LINES);
        assert_eq!(lines[0], "Research: lv6  |  50 pts", "{lines:?}");
        assert_eq!(lines[1], "  kind 0: 4/10  lv4  30p");
        assert_eq!(lines[2], "  kind 1 (move 84): 6/25  lv3  20p");
        assert_eq!(task_lines(&tasks, &counters, 25, 1).len(), 1, "the cap holds");
        // Tasks with progress show before untouched ones.
        counters.remove(&(25, KIND_USED_MOVE, 84));
        counters.insert((25, KIND_CAUGHT, 0), 1);
        let lines = task_lines(&tasks, &counters, 25, TASK_LINES);
        assert_eq!(lines[1], "  kind 0: 1/10  lv2  10p");
        assert_eq!(lines[2], "  kind 1 (move 84): 0/25  lv1  0p");
    }

    #[test]
    fn entry_display_name_strips_the_number_prefix() {
        let entry = DexEntry {
            id: 1,
            label: "#0001 Bulbasaur".into(),
        };
        assert_eq!(entry_display_name(&entry), "Bulbasaur");
        let bare = DexEntry {
            id: 905,
            label: "#0905".into(),
        };
        assert_eq!(entry_display_name(&bare), "#0905");
    }

    #[test]
    fn national_listing_parses_names_and_keeps_national_order() {
        let entries = DexCatalog::national_entries_from_tsv(NATIONAL_FIXTURE).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries[0],
            DexEntry {
                id: 1,
                label: "#0001 Bulbasaur".into()
            }
        );
        assert!(entries.windows(2).all(|pair| pair[0].id < pair[1].id));
        assert!(DexCatalog::national_entries_from_tsv(
            "national_number:u16\tidentifier:string\tname_en:string\tname_es:string\nx\ta\tA\tA\n"
        )
        .is_err());
    }

    #[test]
    fn real_national_sheet_lists_all_1025_species() {
        let text = include_str!("../../../../data/national/species.tsv");
        let entries = DexCatalog::national_entries_from_tsv(text).unwrap();
        assert_eq!(entries.len(), 1025);
        assert_eq!(entries[0].label, "#0001 Bulbasaur");
        assert_eq!(entries[1024].id, 1025);
        assert!(entries.windows(2).all(|pair| pair[0].id < pair[1].id));
    }

    #[test]
    fn dex_mode_defaults_to_hisui_and_active_list_follows_it() {
        assert_eq!(DexUi::default().mode, DexMode::Hisui, "default unchanged");
        assert_eq!(DexMode::Hisui.toggled(), DexMode::National);
        assert_eq!(DexMode::National.toggled(), DexMode::Hisui);
        let catalog = DexCatalog {
            entries: vec![DexEntry {
                id: 25,
                label: "#0025 pikachu".into(),
            }],
            national: vec![DexEntry {
                id: 1,
                label: "#0001 Bulbasaur".into(),
            }],
        };
        assert_eq!(active_entries(&catalog, DexMode::Hisui).len(), 1);
        assert_eq!(
            active_entries(&catalog, DexMode::National)[0].label,
            "#0001 Bulbasaur"
        );
        assert_eq!(page_entries_of(&catalog.national, 0).len(), 1);
    }

    /// F6 (`game::debug_visual`) is a display-only override: every Hisui row reads as caught
    /// (name instead of `??????`/SILHOUETTE), the counters show the whole list, and the player's
    /// real `PlayerProgress` (the only source of the save) is byte-identical before and after.
    #[test]
    fn f6_shows_every_species_as_caught_and_never_writes_to_the_save() {
        use super::super::debug_visual::{DebugViewState, DebugVisualPlugin};

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.init_asset::<Mesh>().init_asset::<StandardMaterial>();
        app.add_plugins(DebugVisualPlugin::default());
        app.add_plugins(DexScreenPlugin::default());
        let mut progress = PlayerProgress::default();
        progress.seen.insert(25); // seen, never caught
        app.insert_resource(progress.clone());
        app.world_mut().resource_mut::<DexUi>().open = true;
        app.update();
        app.update();

        // Without F6 the seen-but-uncaught Pikachu row keeps its silhouette.
        assert_eq!(row_text(&mut app, 0), "#0025  SILHOUETTE");

        press_once(&mut app, KeyCode::F6);
        app.update();
        assert!(app.world().resource::<DebugViewState>().pokedex_all);
        let row = row_text(&mut app, 0);
        assert!(row.contains("pikachu"), "caught-like label: {row:?}");
        assert!(!row.contains("??????") && !row.contains("SILHOUETTE"), "{row:?}");
        assert_eq!(
            *app.world().resource::<PlayerProgress>(),
            progress,
            "F6 must not touch the saved seen/caught/research state"
        );

        press_once(&mut app, KeyCode::F6);
        app.update();
        assert!(!app.world().resource::<DebugViewState>().pokedex_all);
        assert_eq!(row_text(&mut app, 0), "#0025  SILHOUETTE", "off: the real state is back");
        assert_eq!(*app.world().resource::<PlayerProgress>(), progress);
    }

    /// End-to-end headless screen check with the real tracked data: `Y` switches to the national
    /// list, its first page starts at species 1 with the localized name, and `Y` switches back.
    #[test]
    fn y_toggles_the_national_list_in_a_headless_app() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_plugins(DexScreenPlugin::default());
        app.update();
        {
            let catalog = app.world().resource::<DexCatalog>();
            assert_eq!(catalog.entries.len(), 242, "Hisui list unchanged");
            assert_eq!(catalog.national.len(), 1025, "national sheet loaded");
        }
        app.world_mut().resource_mut::<DexUi>().open = true;
        press_once(&mut app, KeyCode::KeyY);
        assert_eq!(app.world().resource::<DexUi>().mode, DexMode::National);
        assert_eq!(row_text(&mut app, 0), "#0001 Bulbasaur");
        assert_eq!(row_text(&mut app, 1), "#0002 Ivysaur");
        press_once(&mut app, KeyCode::KeyY);
        assert_eq!(app.world().resource::<DexUi>().mode, DexMode::Hisui);
        // Hisui mode keeps the pre-existing silhouette behaviour for unseen species.
        let hisui_first = row_text(&mut app, 0);
        assert!(
            hisui_first.ends_with("??????"),
            "Hisui mode keeps silhouettes: {hisui_first:?}"
        );
        assert!(!hisui_first.contains("Bulbasaur"));
    }
}
