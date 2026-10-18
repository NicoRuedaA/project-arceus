//! Original game UI (the default; `--ui debug` forces the provisional one): draws the game's own Ui2D layouts with `bevy_ui`.
//!
//! The layouts are NOT in the repository. They are decoded from the player's own dump by the
//! installer (`.tools/setup_cache.py`, step `ui`: `ui_export` + `ui_layout_decode.py`) into a
//! local UI root holding `layouts/<screen>.json` and `png/...`. At start-up this plugin looks for
//! that root (`--ui-root`, `PLA_UI_ROOT`, `<cache>/ui`, `<cache>/../ui`, `work/ui`); when it is
//! missing, or a screen's file is missing or broken, that screen keeps the provisional
//! `bevy_ui` screen (no crash).
//!
//! Pieces:
//! - [`layout`]: serde model of the decoded JSON;
//! - [`scene`]: pane tree with parts expanded, evaluated into screen-space quads and text slots;
//! - [`anim`]: `.bflan` curves and a small player (open / select / idle animations);
//! - [`material`]: one UI material per quad (`orig_ui.wgsl`) and the PNG cache;
//! - this file: the plugin, the screen entities and the sync from quads to `bevy_ui` nodes;
//! - [`battle`], [`field`], [`menus`]: bindings of each screen to the existing game state.
//!
//! Text is never taken from the game: text panes show the port's own (English) strings.
//! `--ui debug` keeps the provisional UI; when the local UI data is missing the original UI falls
//! back to the provisional one with a single warning.

pub mod anim;
pub mod battle;
pub mod crafting_screen;
pub mod dressup_screen;
pub mod field;
pub mod field_select;
pub mod icons;
pub mod layout;
pub mod material;
pub mod menus;
pub mod scene;
pub mod scrollbar;
pub mod shop_screen;
pub mod summary;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy::text::{FontSize, Justify, LineBreak, TextLayout};
use bevy::ui::UiTransform;
use bevy::ui_render::prelude::{MaterialNode, UiMaterialPlugin};

use anim::{AnimPlayer, AnimState, PlayMode};
use layout::ScreenDoc;
use material::{OrigUiMaterial, OrigUiParams, UiTextureCache};
use scene::{Frame, NodeOverride, Overrides, Scene, TextAlign};

/// Which UI the viewer draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiMode {
    /// The provisional `bevy_ui` screens (`--ui debug`, or the fallback when the data is missing).
    Debug,
    /// The game's own layouts, read from the local UI root (the default).
    #[default]
    Original,
}

impl UiMode {
    /// `--ui original|debug` (or `PLA_UI`); default `original`; a bad value is reported and the
    /// default is used.
    pub fn from_args(args: &[String]) -> Self {
        let env = std::env::var("PLA_UI").ok();
        let raw = crate::game::cli_flags::value_of(args, "--ui")
            .map(str::to_string)
            .or(env);
        match raw
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            None | Some("") => UiMode::default(),
            Some("debug") => UiMode::Debug,
            Some("original") | Some("orig") | Some("game") => UiMode::Original,
            Some(other) => {
                crate::game::cli_flags::report_bad_value(
                    "--ui",
                    other,
                    "original or debug",
                    "original",
                );
                UiMode::default()
            }
        }
    }
}

/// The `--ui-root` directory when given (and only that), else the first existing UI root among `PLA_UI_ROOT`, `<cache>/ui`, `<cache>/../ui`,
/// `work/ui` (a root is a directory with a `layouts/` folder).
pub fn resolve_ui_root(args: &[String], cache_root: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(v) = crate::game::cli_flags::value_of(args, "--ui-root") {
        // An explicit root is the only candidate: a wrong path means "no data" (provisional UI).
        let root = PathBuf::from(v);
        return root.join("layouts").is_dir().then_some(root);
    }
    if let Some(v) = std::env::var_os("PLA_UI_ROOT") {
        candidates.push(PathBuf::from(v));
    }
    candidates.push(cache_root.join("ui"));
    if let Some(parent) = cache_root.parent() {
        candidates.push(parent.join("ui"));
    }
    candidates.push(PathBuf::from("work/ui"));
    candidates.into_iter().find(|c| c.join("layouts").is_dir())
}

/// Mode and data root, decided at start-up.
#[derive(Resource, Debug, Clone)]
pub struct OrigUiConfig {
    pub mode: UiMode,
    pub root: Option<PathBuf>,
}

impl OrigUiConfig {
    /// The original UI is wanted and its data folder exists.
    pub fn enabled(&self) -> bool {
        self.mode == UiMode::Original && self.root.is_some()
    }
}

/// Lazily parsed screen documents (`None` = missing / broken, reported once).
#[derive(Resource, Debug, Default)]
pub struct OrigUiLibrary {
    pub root: Option<PathBuf>,
    docs: HashMap<String, Option<Arc<ScreenDoc>>>,
}

impl OrigUiLibrary {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self {
            root,
            docs: HashMap::new(),
        }
    }

    pub fn doc(&mut self, screen: &str) -> Option<Arc<ScreenDoc>> {
        if let Some(d) = self.docs.get(screen) {
            return d.clone();
        }
        let doc = match &self.root {
            Some(root) => match ScreenDoc::load(root, screen) {
                Ok(d) => Some(Arc::new(d)),
                Err(e) => {
                    warn!("original UI: screen '{screen}' unavailable ({e}); keeping the debug UI for it");
                    None
                }
            },
            None => None,
        };
        self.docs.insert(screen.to_string(), doc.clone());
        doc
    }

    /// Scene of `layout` in screen document `screen`.
    pub fn scene(&mut self, screen: &str, layout: &str) -> Option<Arc<Scene>> {
        let doc = self.doc(screen)?;
        match Scene::build(doc, layout) {
            Some(s) => Some(Arc::new(s)),
            None => {
                warn!("original UI: layout '{layout}' not found in '{screen}'; keeping the debug UI for it");
                None
            }
        }
    }
}

/// Every original screen the port draws: (id, screen document, layout, stacking order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenId {
    BattleHud,
    BattleEnemyPlate,
    BattleTimeline,
    BattleCommand,
    BattleMoves,
    BattleMessage,
    FieldHud,
    /// The field's item / Pokémon selector (`X` on foot): the game's `pokeget` wheel.
    FieldSelect,
    PauseMenu,
    Party,
    Summary,
    Bag,
    Box,
    Dex,
    Map,
    /// Graphics settings (Esc menu -> "Graphics..."), the game's `option_settings_00` list.
    Options,
    /// Crafting (Esc menu -> "Crafting"), the game's `making` screen.
    Crafting,
    /// Shop (Esc menu -> "Shop", or `E` at a shop trigger), the game's `shop_buy_00` screen.
    Shop,
    /// Customize (Esc menu -> "Customize"), the game's `dressup_00` screen.
    Customize,
}

impl ScreenId {
    pub const ALL: [ScreenId; 19] = [
        ScreenId::BattleHud,
        ScreenId::BattleEnemyPlate,
        ScreenId::BattleTimeline,
        ScreenId::BattleCommand,
        ScreenId::BattleMoves,
        ScreenId::BattleMessage,
        ScreenId::FieldHud,
        ScreenId::FieldSelect,
        ScreenId::PauseMenu,
        ScreenId::Party,
        ScreenId::Summary,
        ScreenId::Bag,
        ScreenId::Box,
        ScreenId::Dex,
        ScreenId::Map,
        ScreenId::Options,
        ScreenId::Crafting,
        ScreenId::Shop,
        ScreenId::Customize,
    ];

    /// (screen document id, layout name).
    pub fn source(self) -> (&'static str, &'static str) {
        match self {
            ScreenId::BattleHud => ("gfpak_battle_resident", "battle_top_02"),
            ScreenId::BattleEnemyPlate => ("gfpak_battle_resident", "cmn_pokeplate_enemy_02"),
            ScreenId::BattleTimeline => ("gfpak_battle_resident", "battle_tl_00"),
            ScreenId::BattleCommand => ("gfpak_battle_menu_eng", "battle_command_03"),
            ScreenId::BattleMoves => ("gfpak_battle_menu_eng", "battle_command_waza_00"),
            ScreenId::BattleMessage => ("gfpak_battle_resident", "msg_sys_00_s"),
            ScreenId::FieldHud => ("gfpak_resident_hud_arc01_eng", "fieldinfo_00"),
            // The field's item / Pokémon wheel (bottom right) with its `L` / `R` / `X` badges.
            ScreenId::FieldSelect => ("pokeget", "pokeget_item_change_00"),
            // The pause menu is the game's settings list (`option_settings_00`, the same
            // components as the Graphics screen) filled with the menu's rows: every option
            // readable at once, the selection on the game's own highlight, its scrollbar.
            ScreenId::PauseMenu => ("option", "option_settings_00"),
            ScreenId::Party => ("gfpak_battle_menu_eng", "pokelist_battle_00"),
            ScreenId::Summary => ("status", "status_top_00"),
            ScreenId::Bag => ("bag", "bag_top_00"),
            ScreenId::Box => ("box", "box_top_00"),
            ScreenId::Dex => ("gfpak_poke_dex", "pokedex_detail_00"),
            ScreenId::Map => ("gfpak_map", "map_top_00"),
            ScreenId::Options => ("option", "option_settings_00"),
            ScreenId::Crafting => ("making", "making"),
            ScreenId::Shop => ("shop", "shop_buy_00"),
            ScreenId::Customize => ("dressup", "dressup_00"),
        }
    }

    /// Full-screen backdrop behind the screen (the game blurs a capture of the scene instead).
    pub fn backdrop(self) -> Color {
        match self {
            ScreenId::PauseMenu => Color::srgba(0.02, 0.03, 0.05, 0.82),
            ScreenId::Bag
            | ScreenId::Box
            | ScreenId::Dex
            | ScreenId::Summary
            | ScreenId::Map
            | ScreenId::Options
            | ScreenId::Crafting
            | ScreenId::Shop
            | ScreenId::Party => Color::srgba(0.07, 0.07, 0.09, 0.97),
            _ => Color::NONE,
        }
    }

    /// Global stacking order (above the provisional UI's 0-100 range).
    pub fn z(self) -> i32 {
        match self {
            ScreenId::FieldHud => 150,
            ScreenId::FieldSelect => 155,
            ScreenId::BattleHud | ScreenId::BattleEnemyPlate | ScreenId::BattleTimeline => 160,
            ScreenId::BattleMessage => 165,
            ScreenId::BattleCommand | ScreenId::BattleMoves => 170,
            ScreenId::Map => 180,
            ScreenId::Bag
            | ScreenId::Box
            | ScreenId::Dex
            | ScreenId::Summary
            | ScreenId::Options
            | ScreenId::Crafting
            | ScreenId::Shop
            | ScreenId::Customize
            | ScreenId::Party => 190,
            ScreenId::PauseMenu => 200,
        }
    }
}

/// One original screen: its scene, animation player, run-time overrides and spawned nodes.
#[derive(Component)]
pub struct OrigScreen {
    pub id: ScreenId,
    pub scene: Arc<Scene>,
    pub anim: AnimPlayer,
    pub overrides: Overrides,
    /// Requested by the screen's binding system this frame.
    pub shown: bool,
    was_shown: bool,
    dirty: bool,
    quads: HashMap<u32, Entity>,
    texts: HashMap<usize, (Entity, Entity)>,
    /// Last evaluated frame (for bindings that place things relative to panes, and tests).
    pub last: Option<Frame>,
    /// Port text not tied to a text pane (lists the game fills at run time).
    free_texts: Vec<scene::TextSlot>,
    /// Images not tied to a picture pane (map markers, the summary's Pokémon picture), drawn
    /// after the layout's own quads.
    free_quads: Vec<scene::Quad>,
}

impl OrigScreen {
    pub fn new(id: ScreenId, scene: Arc<Scene>) -> Self {
        let mut s = Self {
            id,
            scene,
            anim: AnimPlayer::default(),
            overrides: Overrides::default(),
            shown: false,
            was_shown: false,
            dirty: true,
            quads: HashMap::new(),
            texts: HashMap::new(),
            last: None,
            free_texts: Vec::new(),
            free_quads: Vec::new(),
        };
        s.rest();
        s.opening(false);
        for n in s.scene.placeholder_icons() {
            s.overrides.node(n).visible = Some(false);
        }
        s
    }

    /// Opening animations of the main layout (`_wake`, `_in`, `_open`), before the resting ones
    /// (as the offline renderer's `--auto-anim`): from their first frame when the screen opens,
    /// at their last frame otherwise.
    pub fn opening(&mut self, from_start: bool) {
        self.anim.tracks.retain(|t| t.label != "open");
        let mut front = Vec::new();
        for tag in ["wake", "in", "open"] {
            if let Some(name) = self.scene.anim_name(0, tag) {
                let len = self.scene.archive().animations[&name].frame_size;
                front.push(anim::Track {
                    instance: 0,
                    anim: name,
                    frame: if from_start { 0.0 } else { len },
                    length: len,
                    mode: PlayMode::Hold,
                    label: "open",
                });
            }
        }
        front.append(&mut self.anim.tracks);
        self.anim.tracks = front;
        self.dirty = true;
    }

    /// Resting state: every instance's `_keep` animation (looping ones loop, others end held).
    fn rest(&mut self) {
        for inst in 0..self.scene.instances.len() {
            if let Some(name) = self.scene.anim_name(inst, "keep") {
                let a = &self.scene.archive().animations[&name];
                let mode = if a.looping {
                    PlayMode::Loop
                } else {
                    PlayMode::Frozen
                };
                let len = a.frame_size;
                self.anim.play(inst, &name, len, mode, "keep");
            }
        }
    }

    pub fn node(&self, name: &str) -> Option<usize> {
        self.scene.find(name)
    }

    /// Change the override of node `node` (marks the screen dirty only on a real change).
    pub fn set_node(&mut self, node: usize, f: impl FnOnce(&mut NodeOverride)) {
        if self.overrides.set(node, f) {
            self.dirty = true;
        }
    }

    /// Change the override of the first node named `name` (ignored when absent).
    pub fn set(&mut self, name: &str, f: impl FnOnce(&mut NodeOverride)) {
        if let Some(n) = self.node(name) {
            self.set_node(n, f);
        }
    }

    pub fn set_text(&mut self, name: &str, text: impl Into<String>) {
        let t = text.into();
        self.set(name, |o| o.text = Some(t));
    }

    pub fn set_visible(&mut self, name: &str, visible: bool) {
        self.set(name, |o| o.visible = Some(visible));
    }

    /// Play `<layout of instance>_<tag>`; false when that animation does not exist.
    pub fn play(
        &mut self,
        instance: usize,
        tag: &str,
        mode: PlayMode,
        label: &'static str,
    ) -> bool {
        let Some(name) = self.scene.anim_name(instance, tag) else {
            return false;
        };
        let len = self.scene.archive().animations[&name].frame_size;
        self.anim.play(instance, &name, len, mode, label);
        self.dirty = true;
        true
    }

    /// Node `name` inside the layout instance of part pane `part` (first part with that name).
    pub fn node_in(&self, part: &str, name: &str) -> Option<usize> {
        let inst = self.scene.part_instance(part)?;
        self.scene.find_in(inst, name)
    }

    /// Node `name` inside layout instance `instance`.
    pub fn node_in_instance(&self, instance: usize, name: &str) -> Option<usize> {
        self.scene.find_in(instance, name)
    }

    pub fn set_in(&mut self, part: &str, name: &str, f: impl FnOnce(&mut NodeOverride)) {
        if let Some(n) = self.node_in(part, name) {
            self.set_node(n, f);
        }
    }

    /// Show animation `tag` of part `part` at one fixed frame (no-op when already there).
    pub fn freeze_part(&mut self, part: &str, tag: &str, frame: f32, label: &'static str) {
        let Some(inst) = self.scene.part_instance(part) else {
            return;
        };
        self.freeze(inst, tag, frame, label);
    }

    /// Show animation `tag` of instance `inst` at one fixed frame (no-op when already there).
    pub fn freeze(&mut self, inst: usize, tag: &str, frame: f32, label: &'static str) {
        let Some(name) = self.scene.anim_name(inst, tag) else {
            return;
        };
        if self.anim.tracks.iter().any(|t| {
            t.instance == inst
                && t.label == label
                && t.anim == name
                && t.mode == PlayMode::Frozen
                && t.frame == frame
        }) {
            return;
        }
        self.anim.freeze(inst, &name, frame, label);
        self.dirty = true;
    }

    /// Replace the free texts (port strings drawn at layout positions without a text pane).
    pub fn set_free_texts(&mut self, mut texts: Vec<scene::TextSlot>) {
        for (i, t) in texts.iter_mut().enumerate() {
            t.node = FREE_TEXT_BASE + i;
        }
        if texts != self.free_texts {
            self.free_texts = texts;
            self.dirty = true;
        }
    }

    /// Replace the free images (drawn above the layout, in list order).
    pub fn set_free_quads(&mut self, mut quads: Vec<scene::Quad>) {
        for (i, q) in quads.iter_mut().enumerate() {
            q.key = FREE_QUAD_BASE + i as u32;
        }
        if quads != self.free_quads {
            self.free_quads = quads;
            self.dirty = true;
        }
    }

    /// Free images currently set (tests).
    pub fn free_quads(&self) -> &[scene::Quad] {
        &self.free_quads
    }

    /// Free texts currently set (tests).
    pub fn free_texts(&self) -> &[scene::TextSlot] {
        &self.free_texts
    }

    /// Length (frames) of `<instance layout>_<tag>`.
    pub fn anim_len(&self, inst: usize, tag: &str) -> Option<f32> {
        let name = self.scene.anim_name(inst, tag)?;
        Some(self.scene.archive().animations[&name].frame_size)
    }

    /// Show the last frame of `tag` on instance `inst` (resting state of a one-shot animation).
    pub fn freeze_end(&mut self, inst: usize, tag: &str, label: &'static str) {
        if let Some(len) = self.anim_len(inst, tag) {
            self.freeze(inst, tag, len, label);
        }
    }

    /// Play `tag` on the part pane named `part`.
    pub fn play_part(
        &mut self,
        part: &str,
        tag: &str,
        mode: PlayMode,
        label: &'static str,
    ) -> bool {
        match self.scene.part_instance(part) {
            Some(i) => self.play(i, tag, mode, label),
            None => false,
        }
    }

    pub fn stop(&mut self, instance: usize, label: &'static str) {
        if self.anim.has_label(instance, label) {
            self.anim.stop_label(instance, label);
            self.dirty = true;
        }
    }

    /// Evaluate the current state (also used by tests without any rendering).
    pub fn evaluate(&self) -> Frame {
        let mut state = AnimState::default();
        for t in &self.anim.tracks {
            if let Some(a) = self.scene.archive().animations.get(&t.anim) {
                state.apply(t.instance, a, t.frame);
            }
        }
        self.scene.evaluate(&state, &self.overrides)
    }
}

/// Glyph height used for a text pane's font size. The game's own fonts are not used (the port
/// draws its default font), so this is tuned by eye against the layouts.
pub const TEXT_SCALE: f32 = 0.68;

/// Text keys at or above this value are free texts, below are text panes.
pub const FREE_TEXT_BASE: usize = 1 << 24;

/// Quad keys at or above this value are free images (layout quads use node × 16 + piece).
pub const FREE_QUAD_BASE: u32 = 1 << 28;

/// A free image: PNG `png` (relative to the UI root, `uv` its source rectangle) centred at
/// layout pixel `center` (top-left origin) with size `size`, multiplied by `tint` (0-1 RGBA).
pub fn free_image(png: &str, uv: [f32; 4], center: Vec2, size: Vec2, tint: [f32; 4]) -> scene::Quad {
    let [u0, v0, u1, v1] = uv;
    scene::Quad {
        key: 0,
        node: 0,
        visible: true,
        center,
        size,
        angle: 0.0,
        uv: [
            Vec2::new(u0, v0),
            Vec2::new(u1, v0),
            Vec2::new(u0, v1),
            Vec2::new(u1, v1),
        ],
        vcol: [Vec4::from_array(tint); 4],
        texture: Some(png.to_string()),
        black: Vec4::ZERO,
        white: Vec4::ONE,
        srt: layout::TexSrt::default(),
        wrap: [0, 0],
        alpha: 1.0,
        additive: false,
        multiply: false,
    }
}

/// A free untextured rectangle of colour `rgba` (0-1).
pub fn free_rect(center: Vec2, size: Vec2, rgba: [f32; 4]) -> scene::Quad {
    let mut q = free_image("", [0.0, 0.0, 1.0, 1.0], center, size, rgba);
    q.texture = None;
    q
}

/// A free text at layout pixel `(x, y)` (top-left origin) of size `w`×`h`.
pub fn free_text(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    font_px: f32,
    color: [f32; 4],
    align: TextAlign,
    text: impl Into<String>,
) -> scene::TextSlot {
    scene::TextSlot {
        node: 0,
        visible: true,
        center: Vec2::new(x + w / 2.0, y + h / 2.0),
        size: Vec2::new(w, h),
        angle: 0.0,
        font_px,
        color: Vec4::from_array(color),
        align,
        text: text.into(),
    }
}

/// Spawned original screens by id.
#[derive(Resource, Debug, Default)]
pub struct OrigScreens {
    pub entities: HashMap<ScreenId, Entity>,
}

impl OrigScreens {
    pub fn has(&self, id: ScreenId) -> bool {
        self.entities.contains_key(&id)
    }
}

/// Marker of one quad node (child of a screen root).
#[derive(Component, Debug)]
pub struct OrigQuad {
    pub key: u32,
    params: OrigUiParams,
    texture: Option<String>,
}

/// Marker of a text box node (child of a screen root).
#[derive(Component, Debug)]
pub struct OrigTextBox;

/// Marker of the text inside a text box.
#[derive(Component, Debug)]
pub struct OrigText;

/// The original-UI plugin. Always added (it is inert in `debug` mode).
pub struct OrigUiPlugin {
    pub mode: UiMode,
    pub root: Option<PathBuf>,
}

impl OrigUiPlugin {
    pub fn from_args(args: &[String], cache_root: &Path) -> Self {
        let mode = UiMode::from_args(args);
        Self {
            mode,
            root: resolve_ui_root(args, cache_root),
        }
    }
}

/// System sets of the plugin, in order.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum OrigUiSystems {
    /// Screen bindings read the game state (Update).
    Bind,
    /// Animations advance and quads are synced to nodes (PostUpdate, before UI layout).
    Render,
}

impl Plugin for OrigUiPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "orig_ui.wgsl");
        if !app.is_plugin_added::<UiMaterialPlugin<OrigUiMaterial>>() {
            app.add_plugins(UiMaterialPlugin::<OrigUiMaterial>::default());
        }
        let config = OrigUiConfig {
            mode: self.mode,
            root: self.root.clone(),
        };
        match (&config.mode, &config.root) {
            (UiMode::Original, Some(r)) => info!("original UI: reading layouts from {}", r.display()),
            (UiMode::Original, None) => warn!(
                "original UI data not found: using the provisional UI (run `.tools/setup_cache.py` step ui, or set PLA_UI_ROOT; `--ui debug` silences this)"
            ),
            _ => {}
        }
        app.insert_resource(OrigUiLibrary::new(config.root.clone()))
            .insert_resource(UiTextureCache::new(config.root.clone().unwrap_or_default()))
            .insert_resource(config)
            .init_resource::<OrigScreens>()
            .init_resource::<icons::IconIndex>()
            .init_resource::<scrollbar::ScrollBars>()
            .init_resource::<scrollbar::ScrollDrag>()
            .init_resource::<scrollbar::VirtualPointer>()
            .configure_sets(Update, OrigUiSystems::Bind)
            .configure_sets(
                PostUpdate,
                OrigUiSystems::Render
                    .before(bevy::ui::UiSystems::Prepare)
                    .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
            )
            .init_resource::<OrigUiFont>()
            .add_systems(Startup, (load_ui_font, spawn_screens).chain())
            .add_systems(
                Update,
                scrollbar::scrollbar_input.after(OrigUiSystems::Bind),
            )
            .add_systems(
                PostUpdate,
                (render_screens, hide_provisional, minimap_off_under_screens)
                    .chain()
                    .in_set(OrigUiSystems::Render),
            );
        battle::install(app);
        field::install(app);
        field_select::install(app);
        menus::install(app);
        summary::install(app);
        crafting_screen::install(app);
        shop_screen::install(app);
        dressup_screen::install(app);
    }
}

/// Spawn (hidden) every original screen whose data is present.
pub fn spawn_screens(
    mut commands: Commands,
    config: Res<OrigUiConfig>,
    mut library: ResMut<OrigUiLibrary>,
    mut screens: ResMut<OrigScreens>,
    mut icons: ResMut<icons::IconIndex>,
) {
    if !config.enabled() {
        return;
    }
    if let Some(root) = &config.root {
        icons.scan(root);
    }
    for id in ScreenId::ALL {
        let (doc, layout) = id.source();
        let Some(scene) = library.scene(doc, layout) else {
            continue;
        };
        let e = spawn_screen(&mut commands, id, scene);
        screens.entities.insert(id, e);
    }
    info!(
        "original UI: {} of {} screens available",
        screens.entities.len(),
        ScreenId::ALL.len()
    );
}

/// Spawn one hidden screen root (its quad nodes are created on first show).
pub fn spawn_screen(commands: &mut Commands, id: ScreenId, scene: Arc<Scene>) -> Entity {
    commands
        .spawn((
            Name::new(format!("orig_ui {id:?}")),
            OrigScreen::new(id, scene),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            GlobalZIndex(id.z()),
            Visibility::Hidden,
        ))
        // The backdrop is the first child, so every quad (added later) draws over it.
        .with_child((
            OrigBackdrop,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(id.backdrop()),
        ))
        .id()
}

/// Full-screen backdrop node of a screen (first child of its root).
#[derive(Component, Debug)]
pub struct OrigBackdrop;

fn pct(v: f32, of: f32) -> Val {
    Val::Percent(v / of * 100.0)
}

/// Advance animations, evaluate dirty shown screens and sync their quads / texts to nodes.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn render_screens(
    mut commands: Commands,
    time: Res<Time>,
    mut screens: Query<(Entity, &mut OrigScreen, &mut Visibility)>,
    mut quads: Query<
        (
            &mut OrigQuad,
            &mut Node,
            &mut UiTransform,
            &MaterialNode<OrigUiMaterial>,
            &mut Visibility,
        ),
        (Without<OrigScreen>, Without<OrigTextBox>, Without<OrigText>),
    >,
    mut boxes: Query<
        (&mut Node, &mut UiTransform),
        (With<OrigTextBox>, Without<OrigQuad>, Without<OrigScreen>),
    >,
    mut texts: Query<(&mut Text, &mut TextFont, &mut TextColor, &mut TextLayout), With<OrigText>>,
    mut materials: ResMut<Assets<OrigUiMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut cache: ResMut<UiTextureCache>,
    ui_font: Option<Res<OrigUiFont>>,
) {
    let dt = time.delta_secs();
    let font_handle = ui_font.as_ref().and_then(|f| f.handle.clone());
    for (root, mut screen, mut vis) in &mut screens {
        let want = if screen.shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        vis.set_if_neq(want);
        if !screen.shown {
            screen.was_shown = false;
            continue;
        }
        if !screen.was_shown {
            screen.was_shown = true;
            screen.opening(true);
        }
        if screen.anim.advance(dt) {
            screen.dirty = true;
        }
        if !screen.dirty {
            continue;
        }
        screen.dirty = false;
        let frame = screen.evaluate();
        if let Ok(pat) = std::env::var("PLA_ORIG_UI_DEBUG") {
            for q in frame.quads.iter() {
                let path = &screen.scene.nodes[q.node].path;
                if path.contains(&pat) {
                    info!(
                        "orig_ui debug {:?}: {path} at {:?} visible={} alpha={:.2} add={} mul={} tex={:?} key={} entity={:?}",
                        screen.id,
                        q.center,
                        q.visible,
                        q.alpha,
                        q.additive,
                        q.multiply,
                        q.texture,
                        q.key,
                        screen.quads.get(&q.key)
                    );
                }
            }
        }
        let (w, h) = (screen.scene.width, screen.scene.height);
        // Quads: one node per stable key, spawned on first evaluation in draw order (free images
        // after the layout's quads, so they draw on top).
        let free_quads = screen.free_quads.clone();
        let mut seen_quads = Vec::with_capacity(frame.quads.len() + free_quads.len());
        for q in frame.quads.iter().chain(free_quads.iter()) {
            seen_quads.push(q.key);
            let tex_handle = q.texture.as_ref().and_then(|t| cache.get(&mut images, t));
            let params = OrigUiParams::from_quad(q, tex_handle.is_some());
            let node = Node {
                position_type: PositionType::Absolute,
                left: pct(q.center.x - q.size.x / 2.0, w),
                top: pct(q.center.y - q.size.y / 2.0, h),
                width: pct(q.size.x, w),
                height: pct(q.size.y, h),
                display: if q.visible {
                    Display::Flex
                } else {
                    Display::None
                },
                ..default()
            };
            let xf = UiTransform::from_rotation(Rot2::radians(q.angle));
            let quad_vis = if q.visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            match screen.quads.get(&q.key).copied() {
                Some(e) => {
                    if let Ok((mut oq, mut n, mut t, mat, mut v)) = quads.get_mut(e) {
                        // Hidden quads leave the layout (Display::None) and are not drawn (Visibility).
                        v.set_if_neq(quad_vis);
                        n.set_if_neq(node);
                        t.set_if_neq(xf);
                        if oq.params != params || oq.texture != q.texture {
                            if let Some(mut m) = materials.get_mut(&mat.0) {
                                m.params = params;
                                m.texture = tex_handle;
                            }
                            oq.params = params;
                            oq.texture = q.texture.clone();
                        }
                    }
                }
                None => {
                    let mat = materials.add(OrigUiMaterial {
                        params,
                        texture: tex_handle,
                    });
                    let e = commands
                        .spawn((
                            OrigQuad {
                                key: q.key,
                                params,
                                texture: q.texture.clone(),
                            },
                            node,
                            xf,
                            quad_vis,
                            MaterialNode(mat),
                        ))
                        .id();
                    commands.entity(root).add_child(e);
                    screen.quads.insert(q.key, e);
                }
            }
        }
        // Free images that were removed.
        let stale_quads: Vec<Entity> = screen
            .quads
            .iter()
            .filter(|(k, _)| **k >= FREE_QUAD_BASE && !seen_quads.contains(k))
            .map(|(_, e)| *e)
            .collect();
        for e in stale_quads {
            if let Ok((_, mut n, _, _, mut v)) = quads.get_mut(e) {
                v.set_if_neq(Visibility::Hidden);
                if n.display != Display::None {
                    n.display = Display::None;
                }
            }
        }
        // Text slots: a box at the pane rectangle with the port's own string inside.
        let mut seen = Vec::new();
        let free = screen.free_texts.clone();
        for t in frame.texts.iter().chain(free.iter()) {
            // Accented letters stay when a font that has them was found; the default font lacks
            // them, so they are folded to plain letters then.
            let shown_text = if font_handle.is_some() {
                t.text.clone()
            } else {
                fold_for_default_font(&t.text)
            };
            seen.push(t.node);
            let justify = match t.align {
                TextAlign::Left => (JustifyContent::FlexStart, Justify::Left),
                TextAlign::Center => (JustifyContent::Center, Justify::Center),
                TextAlign::Right => (JustifyContent::FlexEnd, Justify::Right),
            };
            let node = Node {
                position_type: PositionType::Absolute,
                left: pct(t.center.x - t.size.x / 2.0, w),
                top: pct(t.center.y - t.size.y / 2.0, h),
                width: pct(t.size.x, w),
                height: pct(t.size.y, h),
                display: if t.visible {
                    Display::Flex
                } else {
                    Display::None
                },
                align_items: AlignItems::Center,
                justify_content: justify.0,
                overflow: Overflow::visible(),
                ..default()
            };
            let xf = UiTransform::from_rotation(Rot2::radians(t.angle));
            // Font height as a share of the viewport height (resolution independent).
            let font = FontSize::Vh(t.font_px * TEXT_SCALE / h * 100.0);
            // Multi-line panes wrap at words; single-line ones never wrap.
            let wrap = if t.size.y > t.font_px * 1.8 {
                LineBreak::WordBoundary
            } else {
                LineBreak::NoWrap
            };
            let color = Color::srgba(t.color.x, t.color.y, t.color.z, t.color.w);
            match screen.texts.get(&t.node).copied() {
                Some((b, txt)) => {
                    if let Ok((mut n, mut x)) = boxes.get_mut(b) {
                        n.set_if_neq(node);
                        x.set_if_neq(xf);
                    }
                    if let Ok((mut text, mut tf, mut tc, mut tl)) = texts.get_mut(txt) {
                        if text.0 != shown_text {
                            text.0 = shown_text;
                        }
                        if let Some(handle) = font_handle.clone() {
                            let source = bevy::text::FontSource::Handle(handle);
                            if tf.font != source {
                                tf.font = source;
                            }
                        }
                        if tf.font_size != font {
                            tf.font_size = font;
                        }
                        tc.set_if_neq(TextColor(color));
                        let layout = TextLayout::new(justify.1, wrap);
                        if tl.justify != layout.justify || tl.linebreak != layout.linebreak {
                            *tl = layout;
                        }
                    }
                }
                None => {
                    let mut text_font = TextFont {
                        font_size: font,
                        ..default()
                    };
                    if let Some(handle) = font_handle.clone() {
                        text_font.font = bevy::text::FontSource::Handle(handle);
                    }
                    let txt = commands
                        .spawn((
                            OrigText,
                            Text::new(shown_text),
                            text_font,
                            TextColor(color),
                            TextLayout::new(justify.1, wrap),
                        ))
                        .id();
                    let b = commands.spawn((OrigTextBox, node, xf)).add_child(txt).id();
                    commands.entity(root).add_child(b);
                    screen.texts.insert(t.node, (b, txt));
                }
            }
        }
        // Texts whose string was cleared.
        let stale: Vec<(usize, Entity)> = screen
            .texts
            .iter()
            .filter(|(n, _)| !seen.contains(n))
            .map(|(n, (b, _))| (*n, *b))
            .collect();
        for (_, b) in stale {
            if let Ok((mut n, _)) = boxes.get_mut(b) {
                if n.display != Display::None {
                    n.display = Display::None;
                }
            }
        }
        screen.last = Some(frame);
    }
}

/// Existing Noto Sans system locations, in order. Noto Sans is distributed under the SIL Open
/// Font License 1.1; arbitrary paths are deliberately not accepted because the game's font and
/// game-derived font files must never enter the port.
pub const UI_FONT_CANDIDATES: [&str; 5] = [
    "/usr/share/fonts/noto/NotoSans-Medium.ttf",
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Medium.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
];

/// The font the original UI draws its text with (`None`: Bevy's default font, without accents).
#[derive(Resource, Debug, Default)]
pub struct OrigUiFont {
    pub handle: Option<Handle<Font>>,
    pub path: Option<PathBuf>,
}

/// First readable, pre-verified free font among [`UI_FONT_CANDIDATES`]. The argument is retained
/// for source compatibility with the earlier lookup API, but arbitrary environment paths are
/// ignored so a game-derived font cannot be selected accidentally.
pub fn find_ui_font(_env: Option<PathBuf>) -> Option<PathBuf> {
    UI_FONT_CANDIDATES
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

/// Load the UI font once at start-up (original UI only; missing or unreadable: default font).
pub fn load_ui_font(
    config: Res<OrigUiConfig>,
    fonts: Option<ResMut<Assets<Font>>>,
    mut ui_font: ResMut<OrigUiFont>,
) {
    let Some(mut fonts) = fonts.filter(|_| config.enabled()) else {
        return;
    };
    let Some(path) = find_ui_font(None) else {
        info!("original UI: no free font with accents found; accented letters are shown plain");
        return;
    };
    match std::fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| system_font_from_bytes(bytes).map_err(str::to_owned))
    {
        Ok(font) => {
            info!("original UI: text font {}", path.display());
            ui_font.handle = Some(fonts.add(font));
            ui_font.path = Some(path);
        }
        Err(e) => warn!("original UI: cannot read font {}: {e}", path.display()),
    }
}

/// Validate the container signature before handing bytes to Bevy's font asset.
///
/// Bevy 0.19's [`Font::from_bytes`] stores the bytes and returns `Font`; it does not parse a
/// typeface or return a `Result`. Face parsing happens when Bevy registers the asset, so this
/// helper rejects unrelated files without pretending to prove a valid glyph table here.
fn system_font_from_bytes(bytes: Vec<u8>) -> Result<Font, &'static str> {
    if bytes.len() < 12 {
        return Err("font file is shorter than its table directory header");
    }
    let sig = bytes.get(..4).ok_or("font file is shorter than its signature")?;
    if matches!(sig, [0, 1, 0, 0] | b"OTTO" | b"true") {
        Ok(Font::from_bytes(bytes))
    } else {
        Err("not a TrueType / OpenType font")
    }
}

/// The port draws text with Bevy's default font, which lacks accented letters: fold them so
/// "Pokémon" does not show a missing-glyph box.
pub fn fold_for_default_font(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'É' | 'È' => 'E',
            'á' | 'à' | 'â' | 'ä' => 'a',
            'í' | 'ï' => 'i',
            'ó' | 'ö' | 'ô' => 'o',
            'ú' | 'ü' => 'u',
            'ñ' => 'n',
            '·' => '-',
            c => c,
        })
        .collect()
}

/// Which provisional screen an original one replaces.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProvisionalUi(pub ScreenId);

/// Hide the provisional nodes whose original replacement is available and shown this frame
/// (or always, for the HUD-like ones the original screen fully covers).
#[allow(clippy::type_complexity)]
pub fn hide_provisional(
    config: Res<OrigUiConfig>,
    screens: Res<OrigScreens>,
    shown: Query<&OrigScreen>,
    mut provisional: Query<(&ProvisionalUi, &mut Visibility), Without<OrigScreen>>,
) {
    if !config.enabled() {
        return;
    }
    for (p, mut vis) in &mut provisional {
        let Some(e) = screens.entities.get(&p.0) else {
            continue;
        };
        if shown.get(*e).is_ok_and(|s| s.shown) {
            vis.set_if_neq(Visibility::Hidden);
        }
    }
}

/// Original screens the port's corner minimap must not cover: the battle HUD (the game shows no
/// map in battle) and every menu (the minimap sits on the pause list's footer and the menus'
/// bottom-left panels).
pub fn hides_minimap(id: ScreenId) -> bool {
    matches!(
        id,
        ScreenId::BattleHud
            | ScreenId::PauseMenu
            | ScreenId::Party
            | ScreenId::Summary
            | ScreenId::Bag
            | ScreenId::Box
            | ScreenId::Dex
            | ScreenId::Options
            | ScreenId::Crafting
            | ScreenId::Shop
            | ScreenId::Customize
    )
}

/// Hide the corner minimap while a screen of [`hides_minimap`] is shown, show it again after
/// (only on a change, so nothing else fights over its visibility).
pub fn minimap_off_under_screens(
    screens: Query<&OrigScreen>,
    mut hidden: Local<bool>,
    mut minimap: Query<
        &mut Visibility,
        (With<crate::game::minimap::MinimapRoot>, Without<OrigScreen>),
    >,
) {
    let covered = screens.iter().any(|s| s.shown && hides_minimap(s.id));
    if covered == *hidden {
        return;
    }
    *hidden = covered;
    for mut v in &mut minimap {
        *v = if covered {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

/// Register: hide every provisional node carrying marker `M` while original screen `id` is
/// shown (runs after the provisional screens' own visibility systems, before propagation).
pub fn hide_when_shown<M: Component>(app: &mut App, id: ScreenId) {
    app.add_systems(
        PostUpdate,
        (move |config: Res<OrigUiConfig>,
               screens: Res<OrigScreens>,
               shown: Query<&OrigScreen>,
               mut provisional: Query<&mut Visibility, (With<M>, Without<OrigScreen>)>| {
            if !config.enabled() {
                return;
            }
            let Some(e) = screens.entities.get(&id) else {
                return;
            };
            if shown.get(*e).is_ok_and(|s| s.shown) {
                for mut v in &mut provisional {
                    v.set_if_neq(Visibility::Hidden);
                }
            }
        })
        .after(render_screens)
        .in_set(OrigUiSystems::Render),
    );
}

/// Fill bar `pane` of part `part` to `ratio` (shrinks from the right). The game drives its
/// gauges with a texture-offset animation (`gauge_scale`) whose frame meaning is not certain
/// (UNKNOWN), so the port resizes the bar instead (same look).
pub fn set_bar(s: &mut OrigScreen, part: &str, pane: &str, ratio: f32, visible: bool) {
    let ratio = ratio.clamp(0.0, 1.0);
    if let Some(n) = s.node_in(part, pane) {
        let p = &s.scene.nodes[n].pane;
        let (w, h, tx, ty) = (p.size[0], p.size[1], p.translate[0], p.translate[1]);
        s.set_node(n, |o| {
            o.size = Some([w * ratio, h]);
            o.translate = Some([tx - w * (1.0 - ratio) / 2.0, ty]);
            o.visible = Some(visible);
        });
    }
}

/// The game's own green / yellow / red gauge colour animations (thresholds 50 % / 20 %).
pub fn gauge_colour(s: &mut OrigScreen, part: &str, ratio: f32) {
    let colour = if ratio > 0.5 {
        "color_green"
    } else if ratio > 0.2 {
        "color_yellow"
    } else {
        "color_red"
    };
    s.freeze_part(part, colour, 0.0, "gauge_color");
}

/// Fill a list of row slots: `rows[i]` gets `values[i]` or is hidden.
pub fn fill_rows(screen: &mut OrigScreen, rows: &[&str], values: &[String]) {
    for (i, row) in rows.iter().enumerate() {
        match values.get(i) {
            Some(v) => {
                let v = v.clone();
                screen.set(row, |o| {
                    o.text = Some(v);
                    o.visible = Some(true);
                });
            }
            None => screen.set(row, |o| {
                o.text = None;
            }),
        }
    }
}

#[cfg(test)]
mod tests;
