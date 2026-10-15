//! Evidence-bounded Pokédex screen.
//!
//! The catalog is made only from active rows in the tracked `pml_personal.tsv` sheet.  Forms are
//! collapsed to one deterministic entry per national species id; no species, localized name, or
//! sprite is invented when the sheet does not provide it.  The UI deliberately uses the numeric
//! id and the sheet's development label rather than claiming localization.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::input_map::{Action, ActionState, InputMapPlugin};
use super::pause_menu::{freeze_for_screen, DexOpenRequested, PauseMenuSuppressed};
use super::save_wire::PlayerProgress;
use super::state::Paused;

/// Number of catalog rows shown at once.
pub const VISIBLE_ROWS: usize = 12;

/// One distinct species in the Pokédex catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DexEntry {
    pub id: u16,
    /// Numeric id plus the tracked `species_name` development label, when present.
    pub label: String,
}

/// Deterministic species catalog built from active `pml_personal.tsv` rows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Resource)]
pub struct DexCatalog {
    pub entries: Vec<DexEntry>,
}

impl DexCatalog {
    /// Parse only the personal sheet, keeping this screen independent from other data loaders.
    pub fn from_personal_tsv(text: &str) -> Result<Self, String> {
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
        Ok(Self {
            entries: labels
                .into_iter()
                .map(|(id, label)| DexEntry { id, label })
                .collect(),
        })
    }

    /// Load `pml_personal.tsv` from a tracked data directory.
    pub fn load(data_dir: impl AsRef<Path>) -> Result<Self, String> {
        let path = data_dir.as_ref().join("pml_personal.tsv");
        let text =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        Self::from_personal_tsv(&text)
    }

    pub fn distinct_species_count(&self) -> usize {
        self.entries.len()
    }
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
    let start = page.saturating_mul(VISIBLE_ROWS);
    if start >= catalog.entries.len() {
        return &[];
    }
    let end = (start + VISIBLE_ROWS).min(catalog.entries.len());
    &catalog.entries[start..end]
}

/// Pure screen state. A caller opens it by setting `open` or, in the viewer, through the pause
/// menu's `Pokédex` row ([`DexOpenRequested`]), which also freezes the world until it closes.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct DexUi {
    pub open: bool,
    pub selected: usize,
    pub page: usize,
}

impl Default for DexUi {
    fn default() -> Self {
        Self {
            open: false,
            selected: 0,
            page: 0,
        }
    }
}

/// Root of the provisional Pokédex (hidden while the original one is shown, `orig_ui`).
#[derive(Component)]
pub struct DexRoot;

#[derive(Component, Clone, Copy)]
enum DexText {
    Status,
    Row(usize),
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
        if let Some(catalog) = &catalog {
            println!(
                "[dex] catalog: {} distinct active species",
                catalog.distinct_species_count()
            );
        }
        app.insert_resource(catalog.unwrap_or_default())
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
                    Text::new("Up/Down: select  |  Esc: close"),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
                ));
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
                ui.selected = select_page(ui.page, row.0, catalog.entries.len());
            }
            Interaction::Pressed => {
                ui.selected = select_page(ui.page, row.0, catalog.entries.len());
            }
            Interaction::None => {}
        }
    }
    if delta != 0 {
        ui.selected = move_selection(ui.selected, delta, catalog.entries.len());
    }
}

fn sync_dex_ui(
    catalog: Res<DexCatalog>,
    progress: Option<Res<PlayerProgress>>,
    mut ui: ResMut<DexUi>,
    mut root: Query<&mut Visibility, With<DexRoot>>,
    mut texts: Query<(&DexText, &mut Text)>,
    mut rows: Query<(&DexRow, &mut BackgroundColor)>,
) {
    let len = catalog.entries.len();
    ui.selected = if len == 0 {
        0
    } else {
        ui.selected.min(len - 1)
    };
    ui.page = page_for_selection(ui.selected, len);
    let empty = BTreeSet::new();
    let seen = progress.as_ref().map_or(&empty, |save| &save.seen);
    let caught = progress.as_ref().map_or(&empty, |save| &save.caught);
    let seen_count = catalog
        .entries
        .iter()
        .filter(|entry| seen.contains(&entry.id))
        .count();
    let caught_count = catalog
        .entries
        .iter()
        .filter(|entry| caught.contains(&entry.id))
        .count();

    for mut visibility in &mut root {
        *visibility = if ui.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for (text, mut value) in &mut texts {
        let next = match *text {
            DexText::Status => format!(
                "Page {}/{}  |  seen {seen_count}  |  caught {caught_count}  |  total {len}",
                ui.page + 1,
                page_count(len).max(1),
            ),
            DexText::Hint => "Up/Down: select  |  Esc: close".to_string(),
            DexText::Row(index) => page_entries(&catalog, ui.page)
                .get(index)
                .map_or_else(String::new, |entry| {
                    row_label(entry, visibility(entry.id, seen, caught))
                }),
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
}
