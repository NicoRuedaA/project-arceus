//! Graphics settings screen (Esc menu -> "Graphics..."): every visual option in one data-driven
//! table, changed while playing and kept between sessions.
//!
//! Design (so that many more quality options can be added later):
//! - [`OPTION_DEFS`] is the option table: id (the key in the file), label, [`Page`], [`Kind`]
//!   (cycle / on-off / slider), default, [`Restart`] rule and the command-line flag that overrides
//!   it. Adding an option is one table entry plus, if it must act on the running game, one entry
//!   in [`APPLIERS`] (the ids it depends on + a function that pushes the new value into the game).
//! - [`GraphicsSettings`] stores one integer per table entry (cycle: index, on-off: 0/1, slider:
//!   the value). Typed accessors turn it into the game's own settings types.
//! - [`PRESETS`] is the quality preset table (only "default" for now; the row shows "custom" when
//!   the values match no preset).
//! - The file `graphics.toml` (in the settings directory, next to `settings.txt`; never in the
//!   save game) starts with `version = 1`; unknown keys are ignored and written back unchanged, so
//!   options added later never break an older file.
//! - Start-up order: file first, then command-line flags override (and are announced once through
//!   [`super::cli_flags::report_note`]). A flag never rewrites the file; changing that option in
//!   the screen replaces the flag's value for the rest of the session.
//! - The screen has pages (rows `Page`, `Quality preset`, the page's options, `Reset to defaults`)
//!   and scrolls when a page has more rows than fit at 720p.
//!
//! Input: Up/Down (W/S, D-pad) select, Left/Right (A/D) or Enter/A change, Tab / PageUp / PageDown
//! switch page, mouse click changes, Esc goes back to the pause menu. Gamepad D-pad left/right
//! also move the selection (the shared action layer cannot tell them from up/down); Confirm
//! changes the value.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::art_style::{ArtStyle, StyleSettings};
use super::cli_flags;
use super::clock::{slot_centre_hours, GameClock};
use super::effects::sim::Quality as EffectsQuality;
use super::input_map::{Action, ActionState};
use super::pause_menu::{GraphicsOpenRequested, PauseMenuSuppressed};
use super::render_look::{Look, LookSettings};
use super::save_wire::SaveToast;
use super::state::Paused;
use super::weather::{weather_id_from_arg, WeatherState, WeatherVisual};
use crate::assets::vegetation::GrassQuality;

pub const SETTINGS_VERSION: u32 = 1;
pub const FILE_NAME: &str = "graphics.toml";

// ------------------------------------------------------------------ option table

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Look,
    Quality,
    Distances,
    PostProcessing,
}

impl Page {
    pub const ALL: [Page; 4] = [Page::Look, Page::Quality, Page::Distances, Page::PostProcessing];
    pub fn label(self) -> &'static str {
        match self {
            Page::Look => "Look",
            Page::Quality => "Quality",
            Page::Distances => "Distances",
            Page::PostProcessing => "Post-processing",
        }
    }
}

/// How an option's value is chosen.
#[derive(Debug, Clone, Copy)]
pub enum Kind {
    /// Value = index into `(file key, shown label)` pairs.
    Cycle(&'static [(&'static str, &'static str)]),
    /// Value 0 = off, 1 = on.
    OnOff,
    /// Value = the number itself, clamped to `min..=max`, moved by `step`.
    Slider { min: i32, max: i32, step: i32, unit: &'static str },
}

/// When a change needs a restart of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restart {
    /// Applied at once.
    Never,
    /// Applied on the next start (shows "(restart)" as soon as it differs from the start-up value).
    Always,
    /// Applied at once, except when the game started with this value (the systems behind it were
    /// never installed).
    FromValue(i32),
}

/// One row of the option table.
pub struct OptionDef {
    pub id: &'static str,
    pub label: &'static str,
    pub page: Page,
    pub kind: Kind,
    pub default: i32,
    pub restart: Restart,
    /// The command-line flag (or environment variable) that overrides this option, as a value.
    pub cli: fn(&[String]) -> Option<i32>,
}

pub mod id {
    pub const LOOK: &str = "look";
    pub const STYLE: &str = "style";
    pub const OUTLINES: &str = "outlines";
    pub const SHADOWS: &str = "shadows";
    pub const SSAO: &str = "ssao";
    pub const BLOOM: &str = "bloom";
    pub const GRASS: &str = "grass";
    pub const EFFECTS: &str = "effects";
    pub const TIME: &str = "time_of_day";
    pub const WEATHER: &str = "weather";
}

const LOOKS: &[(&str, &str)] = &[("vivid", "vivid"), ("classic", "classic")];
const STYLES: &[(&str, &str)] = &[("plain", "plain"), ("original", "original")];
const QUALITY3: &[(&str, &str)] = &[("off", "off"), ("low", "low"), ("high", "high")];
const TIMES: &[(&str, &str)] = &[
    ("auto", "auto (clock)"),
    ("0", "slot 0 (09:00)"),
    ("1", "slot 1 (15:00)"),
    ("2", "slot 2 (21:00)"),
    ("3", "slot 3 (03:00)"),
];
// Index i >= 1 is weather id i + 1 (2 sunny .. 9 snowstorm), the decoded ids of game::weather.
const WEATHERS: &[(&str, &str)] = &[
    ("auto", "auto (lottery)"),
    ("sunny", "sunny (clear)"),
    ("cloudy", "cloudy"),
    ("rain", "rain"),
    ("snow", "snow"),
    ("drought", "drought"),
    ("fog", "fog"),
    ("rainstorm", "rainstorm"),
    ("snowstorm", "snowstorm"),
];

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag || a.starts_with(&format!("{flag}=")))
}

fn look_is_classic(args: &[String]) -> bool {
    cli_flags::value_of(args, "--look").is_some_and(|v| v.eq_ignore_ascii_case("classic"))
}

fn cli_look(args: &[String]) -> Option<i32> {
    cli_flags::value_of(args, "--look")?;
    Some(i32::from(look_is_classic(args)))
}

fn cli_style(args: &[String]) -> Option<i32> {
    let toon = cli_flags::value_of(args, "--look").is_some_and(|v| v.eq_ignore_ascii_case("toon"));
    if !(has_flag(args, "--style") || toon) {
        return None;
    }
    Some(i32::from(StyleSettings::from_args(args).style == ArtStyle::Original))
}

fn cli_outlines(args: &[String]) -> Option<i32> {
    has_flag(args, "--no-outlines").then_some(0)
}

fn cli_shadows(args: &[String]) -> Option<i32> {
    has_flag(args, "--no-shadows").then_some(0)
}

/// `--ssao` / `--no-ssao`, else off with `--look classic` (the classic look has no post-processing
/// unless a flag asks for it).
fn cli_post(args: &[String], on: &str, off: &str) -> Option<i32> {
    if has_flag(args, on) {
        Some(1)
    } else if has_flag(args, off) || look_is_classic(args) {
        Some(0)
    } else {
        None
    }
}

fn cli_ssao(args: &[String]) -> Option<i32> {
    cli_post(args, "--ssao", "--no-ssao")
}

fn cli_bloom(args: &[String]) -> Option<i32> {
    cli_post(args, "--bloom", "--no-bloom")
}

fn cli_grass(args: &[String]) -> Option<i32> {
    has_flag(args, "--grass").then(|| match GrassQuality::from_args(args) {
        GrassQuality::Off => 0,
        GrassQuality::Low => 1,
        GrassQuality::High => 2,
    })
}

fn effects_index(q: EffectsQuality) -> i32 {
    match q {
        EffectsQuality::Off => 0,
        EffectsQuality::Low => 1,
        EffectsQuality::High => 2,
    }
}

fn cli_effects(args: &[String]) -> Option<i32> {
    if has_flag(args, "--effects") {
        return Some(effects_index(super::effects::sim::quality_from(args, None, None)));
    }
    std::env::var("PLA_EFFECTS").ok().and_then(|v| EffectsQuality::parse(&v)).map(effects_index)
}

fn cli_time(args: &[String]) -> Option<i32> {
    has_flag(args, "--time-slot")
        .then(|| super::clock::time_slot_arg(args))
        .flatten()
        .map(|s| s as i32 + 1)
}

fn cli_weather(args: &[String]) -> Option<i32> {
    let raw = cli_flags::value_of(args, "--weather")?;
    weather_id_from_arg(raw).map(|id| i32::from(id) - 1)
}

/// The option table, in display order inside each page.
pub const OPTION_DEFS: &[OptionDef] = &[
    OptionDef { id: id::LOOK, label: "Look", page: Page::Look, kind: Kind::Cycle(LOOKS), default: 0, restart: Restart::Never, cli: cli_look },
    OptionDef { id: id::STYLE, label: "Character style", page: Page::Look, kind: Kind::Cycle(STYLES), default: 0, restart: Restart::Never, cli: cli_style },
    OptionDef { id: id::OUTLINES, label: "Outlines", page: Page::Look, kind: Kind::OnOff, default: 1, restart: Restart::Never, cli: cli_outlines },
    OptionDef { id: id::TIME, label: "Time of day", page: Page::Look, kind: Kind::Cycle(TIMES), default: 0, restart: Restart::Never, cli: cli_time },
    OptionDef { id: id::WEATHER, label: "Weather", page: Page::Look, kind: Kind::Cycle(WEATHERS), default: 0, restart: Restart::Never, cli: cli_weather },
    OptionDef { id: id::SHADOWS, label: "Shadows", page: Page::Quality, kind: Kind::OnOff, default: 1, restart: Restart::Never, cli: cli_shadows },
    OptionDef { id: id::GRASS, label: "Grass", page: Page::Quality, kind: Kind::Cycle(QUALITY3), default: 1, restart: Restart::Never, cli: cli_grass },
    OptionDef { id: id::EFFECTS, label: "Battle effects", page: Page::Quality, kind: Kind::Cycle(QUALITY3), default: 1, restart: Restart::FromValue(0), cli: cli_effects },
    OptionDef { id: id::SSAO, label: "SSAO (ambient occlusion)", page: Page::PostProcessing, kind: Kind::OnOff, default: 1, restart: Restart::Never, cli: cli_ssao },
    OptionDef { id: id::BLOOM, label: "Bloom", page: Page::PostProcessing, kind: Kind::OnOff, default: 1, restart: Restart::Never, cli: cli_bloom },
];

pub fn def_index(id: &str) -> Option<usize> {
    OPTION_DEFS.iter().position(|d| d.id == id)
}

/// Pages that have at least one option (an empty page is hidden until options are added to it).
pub fn visible_pages() -> Vec<Page> {
    Page::ALL.into_iter().filter(|p| OPTION_DEFS.iter().any(|d| d.page == *p)).collect()
}

// ------------------------------------------------------------------ presets

pub struct Preset {
    pub name: &'static str,
    /// `(option id, value)`; options not listed take their default.
    pub overrides: &'static [(&'static str, i32)],
}

/// Quality presets. Only the default exists for now; add `Low`, `Medium`, ... here.
pub const PRESETS: &[Preset] = &[Preset { name: "default", overrides: &[] }];

// ------------------------------------------------------------------ settings

/// The values of every option (one integer per [`OPTION_DEFS`] entry) plus the file lines this
/// build does not know (kept so a newer file survives being rewritten).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct GraphicsSettings {
    values: Vec<i32>,
    extra: Vec<(String, String)>,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self { values: OPTION_DEFS.iter().map(|d| d.default).collect(), extra: Vec::new() }
    }
}

fn clamp_value(def: &OptionDef, v: i32) -> i32 {
    match def.kind {
        Kind::Cycle(items) => v.clamp(0, items.len() as i32 - 1),
        Kind::OnOff => v.clamp(0, 1),
        Kind::Slider { min, max, .. } => v.clamp(min, max),
    }
}

impl GraphicsSettings {
    pub fn get(&self, id: &str) -> i32 {
        def_index(id).map_or(0, |i| self.values[i])
    }

    pub fn set(&mut self, id: &str, v: i32) {
        if let Some(i) = def_index(id) {
            self.values[i] = clamp_value(&OPTION_DEFS[i], v);
        }
    }

    pub fn value_at(&self, def: usize) -> i32 {
        self.values[def]
    }

    pub fn is_default(&self) -> bool {
        self.values.iter().zip(OPTION_DEFS).all(|(v, d)| *v == d.default)
    }

    pub fn reset(&mut self) {
        for (v, d) in self.values.iter_mut().zip(OPTION_DEFS) {
            *v = d.default;
        }
    }

    /// Move option `def` by `dir` (-1 / +1): cycles wrap, sliders clamp. Returns whether it changed.
    pub fn step(&mut self, def: usize, dir: i32) -> bool {
        let d = &OPTION_DEFS[def];
        let old = self.values[def];
        let new = match d.kind {
            Kind::Cycle(items) => (old + dir).rem_euclid(items.len() as i32),
            Kind::OnOff => (old + dir).rem_euclid(2),
            Kind::Slider { min, max, step, .. } => (old + dir * step).clamp(min, max),
        };
        self.values[def] = new;
        new != old
    }

    /// Text shown for the value of option `def`.
    pub fn value_label(&self, def: usize) -> String {
        let d = &OPTION_DEFS[def];
        let v = self.values[def];
        match d.kind {
            Kind::Cycle(items) => items[v as usize].1.to_string(),
            Kind::OnOff => if v != 0 { "on" } else { "off" }.to_string(),
            Kind::Slider { unit, .. } => format!("{v}{unit}"),
        }
    }

    /// Index of the preset whose values equal the current ones (`None` = custom).
    pub fn preset(&self) -> Option<usize> {
        (0..PRESETS.len()).find(|i| {
            let mut p = GraphicsSettings::default();
            p.apply_preset(*i);
            p.values == self.values
        })
    }

    pub fn apply_preset(&mut self, preset: usize) {
        let Some(p) = PRESETS.get(preset) else { return };
        self.reset();
        for (id, v) in p.overrides {
            self.set(id, *v);
        }
    }

    // ---- file

    /// Parse `graphics.toml` text. Unknown keys are kept in `extra`, bad values keep the default.
    pub fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim().trim_matches('"'));
            if k == "version" {
                continue;
            }
            match def_index(k) {
                Some(i) => {
                    if let Some(val) = parse_value(&OPTION_DEFS[i], v) {
                        s.values[i] = val;
                    }
                }
                None => s.extra.push((k.to_string(), v.to_string())),
            }
        }
        s
    }

    pub fn to_text(&self) -> String {
        let mut out = format!(
            "# Graphics settings of the Pokemon Legends: Arceus Bevy port (written by the Graphics screen).\n\
             # Unknown keys are kept; command-line flags override these values for one session.\n\
             version = {SETTINGS_VERSION}\n"
        );
        for (i, d) in OPTION_DEFS.iter().enumerate() {
            let v = self.values[i];
            let text = match d.kind {
                Kind::Cycle(items) => format!("\"{}\"", items[v as usize].0),
                Kind::OnOff => (v != 0).to_string(),
                Kind::Slider { .. } => v.to_string(),
            };
            out.push_str(&format!("{} = {}\n", d.id, text));
        }
        for (k, v) in &self.extra {
            out.push_str(&format!("{k} = \"{v}\"\n"));
        }
        out
    }

    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(dir.join(FILE_NAME)).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join(FILE_NAME), self.to_text())
    }

    // ---- typed views used to build and drive the game

    /// `ssao_quality` (not an option here) comes from `--ssao-quality`.
    pub fn look_settings(&self, base: LookSettings) -> LookSettings {
        LookSettings {
            look: if self.get(id::LOOK) == 1 { Look::Classic } else { Look::Vivid },
            shadows: self.get(id::SHADOWS) != 0,
            ssao: self.get(id::SSAO) != 0,
            bloom: self.get(id::BLOOM) != 0,
            ..base
        }
    }

    pub fn style_settings(&self) -> StyleSettings {
        StyleSettings {
            style: if self.get(id::STYLE) == 1 { ArtStyle::Original } else { ArtStyle::Plain },
            outlines: self.get(id::OUTLINES) != 0,
        }
    }

    pub fn grass_quality(&self) -> GrassQuality {
        match self.get(id::GRASS) {
            0 => GrassQuality::Off,
            2 => GrassQuality::High,
            _ => GrassQuality::Low,
        }
    }

    pub fn effects_quality(&self) -> EffectsQuality {
        match self.get(id::EFFECTS) {
            0 => EffectsQuality::Off,
            2 => EffectsQuality::High,
            _ => EffectsQuality::Low,
        }
    }

    /// Pinned time slot, `None` = follow the clock.
    pub fn time_slot(&self) -> Option<usize> {
        let v = self.get(id::TIME);
        (v > 0).then(|| v as usize - 1)
    }

    /// Forced weather id (2..=9), `None` = the lottery.
    pub fn weather_id(&self) -> Option<u8> {
        let v = self.get(id::WEATHER);
        (v > 0).then(|| (v + 1) as u8)
    }

    /// `cli` plus `--time-slot` / `--weather` for a saved choice the command line did not give, so
    /// the clock and weather plugins (which read arguments) start from the saved value.
    pub fn startup_args(&self, cli: &[String]) -> Vec<String> {
        let mut args = cli.to_vec();
        if let (Some(slot), false) = (self.time_slot(), has_flag(cli, "--time-slot")) {
            args.extend(["--time-slot".to_string(), slot.to_string()]);
        }
        if let (Some(id), false) = (self.weather_id(), has_flag(cli, "--weather")) {
            args.extend(["--weather".to_string(), id.to_string()]);
        }
        args
    }
}

fn parse_value(def: &OptionDef, text: &str) -> Option<i32> {
    match def.kind {
        Kind::Cycle(items) => items.iter().position(|(k, _)| k.eq_ignore_ascii_case(text)).map(|i| i as i32),
        Kind::OnOff => match text.to_ascii_lowercase().as_str() {
            "true" | "on" | "1" | "yes" => Some(1),
            "false" | "off" | "0" | "no" => Some(0),
            _ => None,
        },
        Kind::Slider { .. } => text.parse::<i32>().ok().map(|v| clamp_value(def, v)),
    }
}

// ------------------------------------------------------------------ start-up resolution

/// What the file and the command line gave at start-up.
#[derive(Debug, Clone)]
pub struct GraphicsStartup {
    /// Values in use (file, then command-line overrides).
    pub settings: GraphicsSettings,
    /// What the file holds (rewritten when an option changes; never gets a command-line value).
    pub saved: GraphicsSettings,
    /// Option ids a command-line flag overrides this session.
    pub overridden: Vec<&'static str>,
    pub dir: PathBuf,
}

impl GraphicsStartup {
    /// File from `dir` first, then the flags in `args` override and are announced once.
    pub fn resolve(dir: &Path, args: &[String]) -> Self {
        let file = std::fs::read_to_string(dir.join(FILE_NAME)).ok();
        let mut saved = file.as_deref().map(GraphicsSettings::parse).unwrap_or_default();
        // The older `effects=` line of settings.txt still counts when graphics.toml says nothing.
        let has_effects_key = file.as_deref().is_some_and(|t| {
            t.lines().any(|l| l.split_once('=').is_some_and(|(k, _)| k.trim() == id::EFFECTS))
        });
        if !has_effects_key {
            let txt = std::fs::read_to_string(dir.join(super::settings::UserSettings::FILE_NAME)).ok();
            if let Some(txt) = txt {
                if txt.lines().any(|l| l.split_once('=').is_some_and(|(k, _)| k.trim() == "effects")) {
                    saved.set(id::EFFECTS, effects_index(super::effects::sim::quality_from(&[], None, Some(&txt))));
                }
            }
        }
        let mut settings = saved.clone();
        let mut overridden = Vec::new();
        for d in OPTION_DEFS {
            if let Some(v) = (d.cli)(args) {
                let v = clamp_value(d, v);
                if v != settings.get(d.id) {
                    cli_flags::report_note(format!(
                        "Graphics: a command-line flag overrides the saved '{}' setting for this session.",
                        d.label
                    ));
                }
                settings.set(d.id, v);
                overridden.push(d.id);
            }
        }
        Self { settings, saved, overridden, dir: dir.to_path_buf() }
    }
}

// ------------------------------------------------------------------ rows

/// One selectable row of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Page,
    Preset,
    Option(usize),
    Reset,
}

/// Rows of `page`: page switch, preset, the page's options, reset.
pub fn rows_of(page: Page) -> Vec<Row> {
    let mut rows = vec![Row::Page, Row::Preset];
    rows.extend(OPTION_DEFS.iter().enumerate().filter(|(_, d)| d.page == page).map(|(i, _)| Row::Option(i)));
    rows.push(Row::Reset);
    rows
}

/// Rows shown at once (a longer page scrolls).
pub const VISIBLE_ROWS: usize = 14;

/// First visible row so that `selected` is shown.
pub fn scroll_for(selected: usize, scroll: usize, total: usize) -> usize {
    let max_scroll = total.saturating_sub(VISIBLE_ROWS);
    let mut s = scroll.min(max_scroll);
    if selected < s {
        s = selected;
    } else if selected >= s + VISIBLE_ROWS {
        s = selected + 1 - VISIBLE_ROWS;
    }
    s.min(max_scroll)
}

/// Does a change of option `def` still need a restart, given the value the game started with?
pub fn needs_restart(def: usize, startup: &GraphicsSettings, now: &GraphicsSettings) -> bool {
    let (s, n) = (startup.value_at(def), now.value_at(def));
    match OPTION_DEFS[def].restart {
        Restart::Never => false,
        Restart::Always => s != n,
        Restart::FromValue(v) => s == v && n != v,
    }
}

/// Text of a row.
pub fn row_label(
    row: Row,
    page: Page,
    page_count: usize,
    page_index: usize,
    s: &GraphicsSettings,
    startup: &GraphicsSettings,
    overridden: &[&str],
) -> String {
    match row {
        Row::Page => format!("Page: < {} >  ({}/{})", page.label(), page_index + 1, page_count),
        Row::Preset => format!(
            "Quality preset: {}",
            s.preset().map_or("custom", |i| PRESETS[i].name)
        ),
        Row::Reset => "Reset to defaults".to_string(),
        Row::Option(i) => {
            let d = &OPTION_DEFS[i];
            let mut t = format!("{}: {}", d.label, s.value_label(i));
            if needs_restart(i, startup, s) {
                t.push_str(" (restart)");
            }
            if d.id == id::SHADOWS && s.get(id::LOOK) == 1 {
                t.push_str(" (vivid look only)");
            }
            if overridden.contains(&d.id) {
                t.push_str(" [command line]");
            }
            t
        }
    }
}

// ------------------------------------------------------------------ runtime glue

/// Bookkeeping of the running screen.
#[derive(Resource, Debug, Clone)]
pub struct GraphicsRuntime {
    /// What the file holds.
    pub saved: GraphicsSettings,
    /// Values at start-up (for the "(restart)" rule).
    pub startup: GraphicsSettings,
    pub overridden: Vec<&'static str>,
    pub dir: PathBuf,
}

#[derive(Resource, Debug, Default)]
pub struct GraphicsScreenState {
    pub open: bool,
    pub selected: usize,
    pub page: usize,
    pub scroll: usize,
}

#[derive(Component)]
pub struct GraphicsScreenRoot;
#[derive(Component)]
struct GfxRowButton(usize);
#[derive(Component)]
struct GfxRowText(usize);
#[derive(Component)]
struct GfxHint;

pub struct GraphicsScreenPlugin {
    pub startup: GraphicsStartup,
}

impl GraphicsScreenPlugin {
    /// Defaults, nothing overridden, file in `dir` (tests).
    pub fn with_dir(dir: PathBuf) -> Self {
        let s = GraphicsSettings::default();
        Self {
            startup: GraphicsStartup { settings: s.clone(), saved: s, overridden: Vec::new(), dir },
        }
    }
}

impl Plugin for GraphicsScreenPlugin {
    fn build(&self, app: &mut App) {
        let st = &self.startup;
        app.insert_resource(st.settings.clone())
            .insert_resource(GraphicsRuntime {
                saved: st.saved.clone(),
                startup: st.settings.clone(),
                overridden: st.overridden.clone(),
                dir: st.dir.clone(),
            })
            .init_resource::<GraphicsScreenState>()
            .init_resource::<GraphicsOpenRequested>()
            .init_resource::<ActionState>()
            .add_systems(Startup, spawn_graphics_screen)
            .add_systems(Update, (graphics_input, apply_graphics_changes, sync_graphics_screen).chain());
    }
}

fn spawn_graphics_screen(mut commands: Commands) {
    commands
        .spawn((
            GraphicsScreenRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.62)),
            GlobalZIndex(130),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(14.0)),
                    row_gap: Val::Px(3.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.95)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("GRAPHICS"),
                    TextFont::from_font_size(26.0),
                    TextColor(Color::srgb(0.95, 0.9, 0.6)),
                ));
                for i in 0..VISIBLE_ROWS {
                    panel
                        .spawn((
                            Button,
                            GfxRowButton(i),
                            Node {
                                width: Val::Px(560.0),
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(3.0)),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.15, 0.2, 0.28, 0.9)),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                GfxRowText(i),
                                Text::new(""),
                                TextFont::from_font_size(18.0),
                                TextColor(Color::WHITE),
                            ));
                        });
                }
                panel.spawn((
                    GfxHint,
                    Text::new(""),
                    TextFont::from_font_size(13.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
                ));
            });
        });
}

fn page_list() -> Vec<Page> {
    visible_pages()
}

#[allow(clippy::too_many_arguments)]
fn graphics_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut actions: ResMut<ActionState>,
    mut state: ResMut<GraphicsScreenState>,
    mut settings: ResMut<GraphicsSettings>,
    mut runtime: ResMut<GraphicsRuntime>,
    mut request: ResMut<GraphicsOpenRequested>,
    mut paused: Option<ResMut<Paused>>,
    mut suppressed: Option<ResMut<PauseMenuSuppressed>>,
    mut toast: Option<ResMut<SaveToast>>,
    buttons: Query<(&GfxRowButton, &Interaction), Changed<Interaction>>,
) {
    if request.0 {
        request.0 = false;
        state.open = true;
        state.selected = 0;
        state.page = 0;
        state.scroll = 0;
        super::pause_menu::freeze_for_screen(&mut paused, &mut suppressed, true);
        return;
    }
    if !state.open {
        return;
    }
    if actions.just_pressed(Action::Cancel) || actions.just_pressed(Action::Menu) {
        actions.consume(Action::Cancel);
        actions.consume(Action::Menu);
        state.open = false;
        // Back to the pause menu: still frozen, but the menu owns the input again.
        if let Some(p) = paused.as_deref_mut() {
            p.0 = true;
        }
        if let Some(s) = suppressed.as_deref_mut() {
            s.0 = false;
        }
        return;
    }

    let pages = page_list();
    state.page = state.page.min(pages.len() - 1);
    let page = pages[state.page];
    let rows = rows_of(page);
    state.selected = state.selected.min(rows.len() - 1);

    let any = |keys: &ButtonInput<KeyCode>, list: &[KeyCode]| keys.any_just_pressed(list.iter().copied());
    let left = any(&keys, &[KeyCode::ArrowLeft, KeyCode::KeyA]);
    let right = any(&keys, &[KeyCode::ArrowRight, KeyCode::KeyD]);
    let up = any(&keys, &[KeyCode::ArrowUp, KeyCode::KeyW]);
    let down = any(&keys, &[KeyCode::ArrowDown, KeyCode::KeyS]);
    let mut dir = 0i32;
    let mut select = 0i32;
    let mut page_move = 0i32;
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
            select -= 1;
        }
        if actions.just_pressed(Action::Next) {
            actions.consume(Action::Next);
            select += 1;
        }
    }
    if any(&keys, &[KeyCode::Tab, KeyCode::PageDown]) {
        page_move = 1;
        // Tab is also the aim toggle of the action layer: do not let it leak to the world.
        actions.consume(Action::Aim);
    }
    if any(&keys, &[KeyCode::PageUp]) {
        page_move = -1;
    }
    if actions.just_pressed(Action::Confirm) {
        actions.consume(Action::Confirm);
        dir = 1;
    }
    for (b, interaction) in &buttons {
        let idx = state.scroll + b.0;
        if idx >= rows.len() {
            continue;
        }
        match interaction {
            Interaction::Hovered => state.selected = idx,
            Interaction::Pressed => {
                state.selected = idx;
                dir = 1;
            }
            Interaction::None => {}
        }
    }
    for key in [
        KeyCode::ArrowLeft, KeyCode::ArrowRight, KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::KeyA,
        KeyCode::KeyD, KeyCode::KeyW, KeyCode::KeyS, KeyCode::Enter, KeyCode::NumpadEnter,
        KeyCode::Space, KeyCode::Tab,
    ] {
        keys.clear_just_pressed(key);
    }

    if select != 0 {
        state.selected = (state.selected as i32 + select.signum()).rem_euclid(rows.len() as i32) as usize;
    }
    let mut new_page = state.page;
    if page_move != 0 {
        new_page = (state.page as i32 + page_move).rem_euclid(pages.len() as i32) as usize;
    }

    if dir != 0 {
        match rows[state.selected] {
            Row::Page => new_page = (state.page as i32 + dir).rem_euclid(pages.len() as i32) as usize,
            Row::Preset => {
                let cur = settings.preset().map_or(-1, |i| i as i32);
                let next = (cur + dir).rem_euclid(PRESETS.len() as i32) as usize;
                let mut s = settings.clone();
                s.apply_preset(next);
                commit(&mut settings, &mut runtime, toast.as_deref_mut(), s);
            }
            Row::Option(i) => {
                let mut s = settings.clone();
                if s.step(i, dir) {
                    commit(&mut settings, &mut runtime, toast.as_deref_mut(), s);
                }
            }
            Row::Reset => {
                let mut s = settings.clone();
                s.reset();
                commit(&mut settings, &mut runtime, toast.as_deref_mut(), s);
            }
        }
    }
    if new_page != state.page {
        state.page = new_page;
        state.selected = state.selected.min(rows_of(pages[new_page]).len() - 1);
        state.scroll = 0;
    }
    let total = rows_of(pages[state.page]).len();
    state.scroll = scroll_for(state.selected, state.scroll, total);
}

/// Store `new` as the live settings, copy the changed options to the file copy, drop their
/// command-line override mark and write the file.
fn commit(
    settings: &mut ResMut<GraphicsSettings>,
    runtime: &mut ResMut<GraphicsRuntime>,
    toast: Option<&mut SaveToast>,
    new: GraphicsSettings,
) {
    if **settings == new {
        return;
    }
    for (i, d) in OPTION_DEFS.iter().enumerate() {
        if settings.value_at(i) != new.value_at(i) {
            runtime.saved.values[i] = new.value_at(i);
            runtime.overridden.retain(|o| *o != d.id);
        }
    }
    **settings = new;
    if let Err(e) = runtime.saved.save(&runtime.dir) {
        if let Some(t) = toast {
            t.show(format!("Could not save graphics settings: {e}"));
        }
    }
}

// ---- applying changes to the running game

struct Applier {
    /// Option ids this applier reacts to.
    ids: &'static [&'static str],
    run: fn(&mut World, &GraphicsSettings),
}

/// How each group of options reaches the running game. Add one entry per new live option.
const APPLIERS: &[Applier] = &[
    Applier { ids: &[id::LOOK, id::SHADOWS, id::SSAO, id::BLOOM], run: apply_look },
    Applier { ids: &[id::STYLE, id::OUTLINES], run: apply_style },
    Applier { ids: &[id::GRASS], run: apply_grass },
    Applier { ids: &[id::EFFECTS], run: apply_effects },
    Applier { ids: &[id::TIME], run: apply_time },
    Applier { ids: &[id::WEATHER], run: apply_weather },
];

fn apply_look(world: &mut World, s: &GraphicsSettings) {
    let base = world.get_resource::<LookSettings>().copied().unwrap_or_default();
    world.insert_resource(s.look_settings(base));
    super::lighting::reapply_look(world);
}

fn apply_style(world: &mut World, s: &GraphicsSettings) {
    // The art-style systems revert / re-convert the character meshes when this resource changes.
    world.insert_resource(s.style_settings());
}

fn apply_grass(world: &mut World, s: &GraphicsSettings) {
    if let Some(mut cfg) = world.get_resource_mut::<super::vegetation::VegetationConfig>() {
        cfg.quality = s.grass_quality();
    }
}

fn apply_effects(world: &mut World, s: &GraphicsSettings) {
    if let Some(mut e) = world.get_resource_mut::<super::effects::EffectsSettings>() {
        e.quality = s.effects_quality();
    }
}

fn apply_time(world: &mut World, s: &GraphicsSettings) {
    if let Some(mut clock) = world.get_resource_mut::<GameClock>() {
        match s.time_slot() {
            Some(slot) => {
                clock.t_hours = slot_centre_hours(slot);
                clock.pinned = true;
            }
            None => clock.pinned = false,
        }
    }
}

fn apply_weather(world: &mut World, s: &GraphicsSettings) {
    let Some(mut ws) = world.get_resource_mut::<WeatherState>() else { return };
    ws.force = s.weather_id();
    if let Some(id) = ws.force {
        ws.machine.id = id;
        world.insert_resource(WeatherVisual::for_id(id));
    }
}

fn apply_graphics_changes(world: &mut World, mut last: Local<Option<GraphicsSettings>>) {
    let Some(cur) = world.get_resource::<GraphicsSettings>().cloned() else { return };
    let Some(prev) = last.clone() else {
        *last = Some(cur);
        return;
    };
    if prev == cur {
        return;
    }
    for a in APPLIERS {
        if a.ids.iter().any(|id| prev.get(id) != cur.get(id)) {
            (a.run)(world, &cur);
        }
    }
    *last = Some(cur);
}

#[allow(clippy::too_many_arguments)]
fn sync_graphics_screen(
    state: Res<GraphicsScreenState>,
    settings: Res<GraphicsSettings>,
    runtime: Res<GraphicsRuntime>,
    mut root: Query<&mut Visibility, With<GraphicsScreenRoot>>,
    mut buttons: Query<(&GfxRowButton, &mut BackgroundColor, &mut Node)>,
    mut texts: Query<(&GfxRowText, &mut Text), Without<GfxHint>>,
    mut hint: Query<&mut Text, With<GfxHint>>,
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
    let pages = page_list();
    let page_index = state.page.min(pages.len() - 1);
    let page = pages[page_index];
    let rows = rows_of(page);
    for (b, mut bg, mut node) in &mut buttons {
        let idx = state.scroll + b.0;
        let display = if idx < rows.len() { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        bg.0 = if idx == state.selected {
            Color::srgba(0.85, 0.6, 0.15, 0.95)
        } else {
            Color::srgba(0.15, 0.2, 0.28, 0.9)
        };
    }
    for (t, mut text) in &mut texts {
        let label = rows
            .get(state.scroll + t.0)
            .map(|row| row_label(*row, page, pages.len(), page_index, &settings, &runtime.startup, &runtime.overridden))
            .unwrap_or_default();
        if text.0 != label {
            text.0 = label;
        }
    }
    let more_up = if state.scroll > 0 { "^ more above  " } else { "" };
    let more_down = if state.scroll + VISIBLE_ROWS < rows.len() { "v more below  " } else { "" };
    let help = format!(
        "{more_up}{more_down}Up/Down select, Left/Right or Enter change, Tab page, Esc back. Changes apply now and are saved to {FILE_NAME}."
    );
    for mut h in &mut hint {
        if h.0 != help {
            h.0 = help.clone();
        }
    }
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::pause_menu::{PauseMenu, PauseMenuPlugin};
    use crate::game::settings::UserSettings;

    fn a(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pla_gfx_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn table_is_consistent_and_ids_unique() {
        for (i, d) in OPTION_DEFS.iter().enumerate() {
            assert_eq!(def_index(d.id), Some(i), "duplicate id {}", d.id);
            assert_eq!(clamp_value(d, d.default), d.default, "{} default out of range", d.id);
        }
        assert_eq!(visible_pages(), vec![Page::Look, Page::Quality, Page::PostProcessing]);
    }

    #[test]
    fn defaults_match_the_viewer_defaults() {
        let s = GraphicsSettings::default();
        assert!(s.is_default());
        assert_eq!(s.look_settings(LookSettings::default()), LookSettings::default());
        assert_eq!(s.style_settings(), StyleSettings::default());
        assert_eq!(s.grass_quality(), GrassQuality::default());
        assert_eq!(s.effects_quality(), EffectsQuality::default());
        assert_eq!((s.time_slot(), s.weather_id()), (None, None));
        assert_eq!(s.preset(), Some(0));
    }

    #[test]
    fn text_round_trip_keeps_every_value_and_unknown_keys() {
        let mut s = GraphicsSettings::default();
        s.set(id::LOOK, 1);
        s.set(id::STYLE, 1);
        s.set(id::OUTLINES, 0);
        s.set(id::GRASS, 2);
        s.set(id::TIME, 3);
        s.set(id::WEATHER, 7);
        s.extra.push(("render_scale".into(), "150".into()));
        let text = s.to_text();
        assert!(text.contains("version = 1"));
        assert!(text.contains("look = \"classic\""));
        assert!(text.contains("outlines = false"));
        assert_eq!(GraphicsSettings::parse(&text), s);
        assert_eq!(s.time_slot(), Some(2));
        assert_eq!(s.weather_id(), Some(8));
    }

    #[test]
    fn parse_ignores_garbage_and_keeps_defaults() {
        let s = GraphicsSettings::parse("version = 9\nlook = \"nope\"\nbloom = maybe\nfuture = 3\n# x\njunk\nshadows = off\n");
        assert_eq!(s.get(id::LOOK), 0);
        assert_eq!(s.get(id::BLOOM), 1);
        assert_eq!(s.get(id::SHADOWS), 0);
        assert_eq!(s.extra, vec![("future".to_string(), "3".to_string())]);
    }

    #[test]
    fn cycling_wraps_both_ways_and_sliders_clamp() {
        let mut s = GraphicsSettings::default();
        let look = def_index(id::LOOK).unwrap();
        assert!(s.step(look, 1));
        assert_eq!(s.value_label(look), "classic");
        assert!(s.step(look, 1));
        assert_eq!(s.value_label(look), "vivid");
        s.step(look, -1);
        assert_eq!(s.value_label(look), "classic");
        let grass = def_index(id::GRASS).unwrap();
        let seen: Vec<String> = (0..4).map(|_| { s.step(grass, 1); s.value_label(grass) }).collect();
        assert_eq!(seen, ["high", "off", "low", "high"]);
        // Sliders clamp (no slider exists yet; check the rule on a local definition).
        let slider = OptionDef { id: "t", label: "t", page: Page::Quality, kind: Kind::Slider { min: 0, max: 20, step: 10, unit: "m" }, default: 10, restart: Restart::Never, cli: |_| None };
        assert_eq!(clamp_value(&slider, 99), 20);
        assert_eq!(parse_value(&slider, "-5"), Some(0));
    }

    #[test]
    fn preset_row_shows_custom_and_reset_returns_to_default() {
        let mut s = GraphicsSettings::default();
        s.set(id::BLOOM, 0);
        assert_eq!(s.preset(), None);
        let label = row_label(Row::Preset, Page::Look, 3, 0, &s, &GraphicsSettings::default(), &[]);
        assert_eq!(label, "Quality preset: custom");
        s.apply_preset(0);
        assert!(s.is_default());
    }

    #[test]
    fn rows_scroll_keeps_the_selection_visible() {
        assert_eq!(rows_of(Page::Look).len(), 2 + 5 + 1);
        assert_eq!(scroll_for(3, 0, 8), 0);
        assert_eq!(scroll_for(29, 0, 30), 30 - VISIBLE_ROWS);
        assert_eq!(scroll_for(2, 16, 30), 2);
        assert_eq!(scroll_for(20, 0, 30), 20 + 1 - VISIBLE_ROWS);
    }

    #[test]
    fn restart_marker_follows_the_rule() {
        let effects = def_index(id::EFFECTS).unwrap();
        let mut started_off = GraphicsSettings::default();
        started_off.set(id::EFFECTS, 0);
        let mut now = started_off.clone();
        assert!(!needs_restart(effects, &started_off, &now));
        now.set(id::EFFECTS, 2);
        assert!(needs_restart(effects, &started_off, &now));
        let started_low = GraphicsSettings::default();
        let mut off = started_low.clone();
        off.set(id::EFFECTS, 0);
        assert!(!needs_restart(effects, &started_low, &off), "low -> off is live");
        let l = row_label(Row::Option(effects), Page::Quality, 3, 1, &now, &started_off, &[id::EFFECTS]);
        assert_eq!(l, "Battle effects: high (restart) [command line]");
    }

    #[test]
    fn command_line_overrides_the_file_and_never_rewrites_it() {
        let dir = tmp("cli");
        let mut saved = GraphicsSettings::default();
        saved.set(id::BLOOM, 0);
        saved.set(id::GRASS, 2);
        saved.save(&dir).unwrap();
        let st = GraphicsStartup::resolve(&dir, &a("viewer --look classic --grass off --no-shadows --time-slot 2 --weather rain"));
        assert_eq!(st.settings.get(id::LOOK), 1);
        assert_eq!(st.settings.get(id::GRASS), 0);
        assert_eq!(st.settings.get(id::SHADOWS), 0);
        assert_eq!(st.settings.time_slot(), Some(2));
        assert_eq!(st.settings.weather_id(), Some(4));
        // classic without post flags means no SSAO / bloom, like LookSettings::from_args.
        assert_eq!((st.settings.get(id::SSAO), st.settings.get(id::BLOOM)), (0, 0));
        assert!(st.overridden.contains(&id::LOOK) && st.overridden.contains(&id::GRASS));
        assert_eq!(st.saved.get(id::GRASS), 2, "the file copy keeps the saved value");
        assert!(cli_flags::warnings().iter().any(|w| w.contains("overrides the saved 'Grass'")));
        let text = std::fs::read_to_string(dir.join(FILE_NAME)).unwrap();
        assert!(text.contains("grass = \"high\""), "{text}");
        // No flags: the file is used as is.
        let st = GraphicsStartup::resolve(&dir, &a("viewer --area 1"));
        assert_eq!(st.settings, st.saved);
        assert!(st.overridden.is_empty());
        assert_eq!(st.settings.get(id::BLOOM), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saved_time_and_weather_become_start_arguments_unless_the_cli_gave_them() {
        let mut s = GraphicsSettings::default();
        s.set(id::TIME, 2);
        s.set(id::WEATHER, 3);
        let args = s.startup_args(&a("viewer"));
        assert_eq!(cli_flags::value_of(&args, "--time-slot"), Some("1"));
        assert_eq!(cli_flags::value_of(&args, "--weather"), Some("4"));
        let args = s.startup_args(&a("viewer --time-slot 0 --weather=fog"));
        assert_eq!(args, a("viewer --time-slot 0 --weather=fog"));
    }

    #[test]
    fn style_flags_map_to_the_style_option() {
        assert_eq!(cli_style(&a("v")), None);
        assert_eq!(cli_style(&a("v --style original")), Some(1));
        assert_eq!(cli_style(&a("v --look toon")), Some(1));
        assert_eq!(cli_style(&a("v --style plain")), Some(0));
    }

    // ---- headless Bevy

    fn step(app: &mut App, down: &[KeyCode], up: &[KeyCode]) {
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.clear();
            for k in down {
                keys.press(*k);
            }
            for k in up {
                keys.release(*k);
            }
        }
        app.update();
    }

    fn gfx_app(tag: &str) -> (App, PathBuf) {
        let dir = tmp(tag);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<ButtonInput<MouseButton>>();
        app.add_plugins(crate::game::input_map::InputMapPlugin);
        PauseMenuPlugin::install(&mut app, UserSettings::default(), dir.clone());
        app.insert_resource(LookSettings::default());
        app.add_plugins(GraphicsScreenPlugin::with_dir(dir.clone()));
        step(&mut app, &[], &[]);
        (app, dir)
    }

    fn press(app: &mut App, key: KeyCode) {
        step(app, &[key], &[]);
        step(app, &[], &[key]);
    }

    #[test]
    fn headless_open_change_option_updates_resources_and_file_then_esc_returns_to_the_menu() {
        let (mut app, dir) = gfx_app("flow");
        press(&mut app, KeyCode::Escape);
        assert!(app.world().resource::<Paused>().0);
        app.world_mut().resource_mut::<GraphicsOpenRequested>().0 = true;
        step(&mut app, &[], &[]);
        assert!(app.world().resource::<GraphicsScreenState>().open);
        assert!(app.world().resource::<PauseMenuSuppressed>().0, "the screen owns the input");

        // Page row -> preset row -> Look option row.
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(app.world().resource::<GraphicsScreenState>().selected, 2);
        press(&mut app, KeyCode::ArrowRight);
        assert_eq!(app.world().resource::<GraphicsSettings>().get(id::LOOK), 1);
        step(&mut app, &[], &[]);
        assert_eq!(app.world().resource::<LookSettings>().look, Look::Classic, "applied to the live resource");
        let file = std::fs::read_to_string(dir.join(FILE_NAME)).unwrap();
        assert!(file.contains("look = \"classic\""), "{file}");

        // Left changes back; Enter also changes forward.
        press(&mut app, KeyCode::ArrowLeft);
        assert_eq!(app.world().resource::<GraphicsSettings>().get(id::LOOK), 0);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.world().resource::<GraphicsSettings>().get(id::LOOK), 1);
        assert_eq!(app.world().resource::<GraphicsScreenState>().selected, 2, "left/right did not move the row");

        // Reset row.
        press(&mut app, KeyCode::ArrowUp);
        press(&mut app, KeyCode::ArrowUp);
        press(&mut app, KeyCode::ArrowUp); // wraps to Reset
        press(&mut app, KeyCode::Enter);
        assert!(app.world().resource::<GraphicsSettings>().is_default());
        step(&mut app, &[], &[]);
        assert_eq!(app.world().resource::<LookSettings>().look, Look::Vivid);
        assert!(std::fs::read_to_string(dir.join(FILE_NAME)).unwrap().contains("look = \"vivid\""));

        // Esc goes back to the pause menu, not to the world.
        press(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<GraphicsScreenState>().open);
        assert!(app.world().resource::<Paused>().0, "pause menu still open");
        assert!(!app.world().resource::<PauseMenuSuppressed>().0);
        press(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<Paused>().0, "second Esc resumes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn headless_pages_switch_and_each_option_updates_its_resource() {
        let (mut app, dir) = gfx_app("pages");
        app.world_mut().resource_mut::<GraphicsOpenRequested>().0 = true;
        step(&mut app, &[], &[]);
        step(&mut app, &[KeyCode::Tab], &[]);
        assert!(
            !app.world().resource::<ActionState>().just_pressed(Action::Aim),
            "Tab (page switch) must not leak as the aim toggle"
        );
        step(&mut app, &[], &[KeyCode::Tab]);
        assert_eq!(app.world().resource::<GraphicsScreenState>().page, 1);
        // Quality page: Preset, Shadows, Grass, Effects.
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::ArrowRight); // shadows off
        assert_eq!(app.world().resource::<GraphicsSettings>().get(id::SHADOWS), 0);
        step(&mut app, &[], &[]);
        assert!(!app.world().resource::<LookSettings>().shadows);
        // Page row: left/right switches pages too.
        press(&mut app, KeyCode::ArrowUp);
        press(&mut app, KeyCode::ArrowUp);
        press(&mut app, KeyCode::ArrowRight);
        assert_eq!(app.world().resource::<GraphicsScreenState>().page, 2);
        let reloaded = GraphicsSettings::load(&dir);
        assert_eq!(reloaded.get(id::SHADOWS), 0, "survives a restart");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn headless_clock_and_weather_apply_live() {
        let (mut app, dir) = gfx_app("clock");
        app.insert_resource(GameClock::default());
        app.world_mut().resource_mut::<GraphicsSettings>().set(id::TIME, 4);
        step(&mut app, &[], &[]);
        step(&mut app, &[], &[]);
        let c = *app.world().resource::<GameClock>();
        assert!(c.pinned);
        assert_eq!(c.slot(), 3);
        app.world_mut().resource_mut::<GraphicsSettings>().set(id::TIME, 0);
        step(&mut app, &[], &[]);
        step(&mut app, &[], &[]);
        assert!(!app.world().resource::<GameClock>().pinned);
        let _ = dir;
    }

    #[test]
    fn pause_menu_row_opens_the_graphics_screen() {
        let (mut app, dir) = gfx_app("menu");
        press(&mut app, KeyCode::Escape);
        app.world_mut().resource_mut::<PauseMenu>().selected = crate::game::pause_menu::OPTIONS
            .iter()
            .position(|o| *o == crate::game::pause_menu::MenuOption::Graphics)
            .unwrap();
        step(&mut app, &[KeyCode::Enter], &[KeyCode::Escape]);
        assert!(app.world().resource::<GraphicsScreenState>().open, "Graphics row must open the screen");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
