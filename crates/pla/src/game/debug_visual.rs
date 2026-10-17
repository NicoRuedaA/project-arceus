//! Visual debug modes (developer aid): collisions, models and data overlays.
//!
//! OFF by default and zero-cost while off: with every mode off the systems only compare a state
//! resource and return, so nothing is iterated, spawned or drawn (the whole feature exists to
//! inspect the port, not to change the normal game).
//!
//! # Key map (the single source of truth for this feature)
//!
//! | Key | Mode | Cycle |
//! |---|---|---|
//! | `F1` | Collisions | off -> **overlay** (collision lines over the world) -> **only** (world models hidden, only collisions + navigation) -> off |
//! | `F2` | Models | normal -> **flat** (plain grey material, no textures) -> **wireframe** (Bevy wireframe pass) -> normal |
//! | `F3` | Data | off -> **player** (position, zone, time, speed, state, FPS) -> **player + markers** (coloured interest zones) -> off |
//! | `F5` | Save game | already used by `game::save_wire`; this module does not touch it |
//! | `F4`, `F6`..`F12` | free | reserved; not used by this module |
//!
//! Every press advances that mode's cycle and wraps back to its first state. The three modes are
//! independent: F1 can be at `only` while F2 stays `normal`. A legend with the current state of
//! the three keys and the colour list stays on screen while any mode is active.
//!
//! # Command line (for windowless captures)
//!
//! `--debug-view` takes a comma-separated list, so the automated captures do not depend on the
//! keyboard: `--debug-view collisions` (= `collisions=overlay`), `collisions=only`,
//! `models` (= `models=flat`), `wireframe` (= `models=wireframe`), `data` (= `data=player`),
//! `markers` (= `data=markers`), or the explicit `collisions=off|overlay|only`,
//! `models=normal|flat|wireframe`, `data=off|player|markers`. A malformed value is reported
//! through `game::cli_flags` (stderr + start-up toast), never silently ignored.
//!
//! # Sources and colours (F1 and F3)
//!
//! | Colour | What it marks | Source |
//! |---|---|---|
//! | cyan | placed-object walls (building/rock collision triangles, non-floor) | `game::world_collision` (`CollisionGrid`) |
//! | dim grey | floor/roof collision triangles of the same instances | `game::world_collision` |
//! | green | walkable navigation faces | `assets::navmesh::AreaNavigation` |
//! | dark red | navigation cells that are NOT walkable (**sampled** on a 8 m grid, drawn in `only` mode) | `AreaNavigation::is_walkable` |
//! | yellow | doors AND fast-travel points of the active area: trigger box + reach circle + beacon | `game::zone::{ZoneLinks, DoorReachIndex}` |
//! | orange | climbable cliff surfaces (marker column + grasp radius) | `game::climb_surfaces::ClimbSurfaces` |
//! | blue | water body footprints at their surface height | `game::water::WaterBodies` |
//! | lime | wild spawner points (circle + marker) and the saved safe spawn point | `game::wild::ActiveAreaSpawners`, `game::spawn_point::SafeSpawnPoint` |
//! | magenta | alive ground pickups / resource nodes | `game::pickups::SpawnMarker` |
//! | white | NPC capsules | `game::npc::NpcInstance` |
//! | red | wild creature capsules + their aggro radius ring | `game::wild::WildCreature` |
//!
//! Interiors (sub-areas, Brief 37) load their walls into the same `WorldCollision` grid on a zone
//! change, so F1 shows them without extra code; the fallback rooms of `game::interior` are plain
//! meshes, not collision entries (they are the visual stand-in when a room shell is missing).
//!
//! # Wiring notes
//!
//! * F1 `overlay` draws the collision triangles within 70 m of the player; `only` additionally
//!   hides every `Mesh3d` entity that is not a debug-visual entity (models, terrain, water,
//!   vegetation) by saving its previous `Visibility` and restoring it exactly when the mode
//!   leaves `only`.
//! * F2 `flat` swaps the material of every mesh for a shared grey `StandardMaterial` and restores
//!   the exact original handles when it leaves `flat`. Four material types are covered:
//!   `StandardMaterial` (world, pickups, fallback meshes...), the art-style `ToonMaterial` that
//!   characters carry when the "original" style is on (the Graphics default), the
//!   `TerrainSplatMaterial` of the real-time terrain (`--terrain splat`, the default) and the
//!   `WaterMaterial` of the rivers and the sea.
//! * F2 `wireframe` uses Bevy's `bevy::pbr::wireframe::WireframePlugin` (`global` toggled on
//!   demand). If the GPU lacks `POLYGON_MODE_LINE`/`IMMEDIATES` Bevy logs a warning and the mode
//!   simply draws nothing (the rest of the viewer is unaffected).
//! * Data markers are rebuilt at most every 0.5 s while `data = markers`, only within 160 m of
//!   the player, and every list is capped (see `MAX_*`) so a huge area cannot blow up the frame.
//! * F3 touches no gameplay state: it only reads.

use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;

use crate::assets::navmesh::{AreaNavigation, NavMesh};
use crate::game::climb_surfaces::ClimbSurfaces;
use crate::game::clock::GameClock;
use crate::game::interact::Climbable;
use crate::game::npc::NpcInstance;
use crate::game::pickups::SpawnMarker;
use crate::game::player_motion::{MotionMode, PlayerMotionState};
use crate::game::ride::RideState;
use crate::game::spawn_point::SafeSpawnPoint;
use crate::game::water::{PlayerSwim, SwimState, WaterBodies};
use crate::game::wild::ActiveAreaSpawners;
use crate::game::world_collision::{CollisionInstance, WorldCollision, WALL_NORMAL_Y};
use crate::game::zone::{DoorReachIndex, TerrainResource, ZoneLinks};
use crate::game::{ActiveZoneInfo, GameNavmeshResource, PlayerAvatar, WildCreature};

// ------------------------------------------------------------------ tuning

/// Collision lines are drawn within this distance of the player (m).
pub const COLLISION_RADIUS: f32 = 70.0;
/// Navigation lines are drawn within this distance of the player (m).
pub const NAV_RADIUS: f32 = 64.0;
/// Data markers are drawn within this distance of the player (m).
pub const MARKER_RADIUS: f32 = 160.0;
/// Cell side of the sampled "not walkable" grid drawn in `collisions = only` (m).
pub const NAV_SAMPLE_STEP: f32 = 8.0;
/// Vertex cap of the collision and navigation line meshes (2 vertices per line segment).
pub const MAX_VIEW_VERTICES: usize = 400_000;
/// Vertex cap of the data marker line mesh.
pub const MAX_MARKER_VERTICES: usize = 300_000;
/// Rebuild the collision/nav lines when the player has moved this far since the last build (m).
pub const REBUILD_MOVE: f32 = 8.0;
/// Rebuild the data markers at most this often while active (s).
pub const MARKER_INTERVAL: f32 = 0.5;
/// Interaction-reach circle segments.
const REACH_SEGMENTS: u32 = 48;
/// Aggro-radius ring segments.
const RING_SEGMENTS: u32 = 32;

// ------------------------------------------------------------------ colours

/// One colour per debug source (see the module table); a single source of truth for the legend.
pub mod colors {
    use bevy::prelude::*;

    pub const WALL: Color = Color::srgb(0.10, 0.85, 1.00);
    pub const FLOOR: Color = Color::srgb(0.45, 0.48, 0.52);
    pub const NAV_WALK: Color = Color::srgb(0.20, 1.00, 0.35);
    // Brighter than the first version: dark red vanished on the near-black `only` backdrop.
    pub const NAV_BLOCKED: Color = Color::srgb(0.80, 0.10, 0.10);
    pub const DOOR: Color = Color::srgb(1.00, 0.85, 0.10);
    pub const CLIMB: Color = Color::srgb(1.00, 0.55, 0.10);
    pub const WATER: Color = Color::srgb(0.25, 0.55, 1.00);
    pub const SPAWN: Color = Color::srgb(0.65, 1.00, 0.20);
    pub const PICKUP: Color = Color::srgb(1.00, 0.35, 0.85);
    pub const NPC: Color = Color::srgb(1.00, 1.00, 1.00);
    pub const WILD: Color = Color::srgb(1.00, 0.15, 0.15);
}

// ------------------------------------------------------------------ pure modes

/// F1: how collisions are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollisionView {
    /// Nothing drawn.
    #[default]
    Off,
    /// Collision lines over the world.
    Overlay,
    /// Only collisions + navigation: the world models and terrain are hidden.
    Only,
}

impl CollisionView {
    /// Next state of the F1 cycle (wraps back to `Off`).
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Overlay,
            Self::Overlay => Self::Only,
            Self::Only => Self::Off,
        }
    }

    /// Legend label (Spanish, like the rest of the on-screen port copy).
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "APAGADO",
            Self::Overlay => "ENCIMA",
            Self::Only => "SOLO",
        }
    }
}

/// F2: how models are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModelView {
    /// Untouched.
    #[default]
    Normal,
    /// Every `StandardMaterial` mesh with a shared plain grey material (no textures).
    Flat,
    /// Bevy wireframe pass over every mesh.
    Wireframe,
}

impl ModelView {
    /// Next state of the F2 cycle (wraps back to `Normal`).
    pub fn next(self) -> Self {
        match self {
            Self::Normal => Self::Flat,
            Self::Flat => Self::Wireframe,
            Self::Wireframe => Self::Normal,
        }
    }

    /// Legend label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Flat => "SIN TEXTURA",
            Self::Wireframe => "ALAMBRE",
        }
    }
}

/// F3: which data overlays are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataView {
    /// Nothing drawn.
    #[default]
    Off,
    /// Player info line only.
    Player,
    /// Player info line plus the coloured interest markers.
    Markers,
}

impl DataView {
    /// Next state of the F3 cycle (wraps back to `Off`).
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Player,
            Self::Player => Self::Markers,
            Self::Markers => Self::Off,
        }
    }

    /// Legend label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "APAGADO",
            Self::Player => "JUGADOR",
            Self::Markers => "JUGADOR+MARCAS",
        }
    }
}

/// Current state of the three debug modes (one resource, always present, trivial to compare).
#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub struct DebugViewState {
    pub collisions: CollisionView,
    pub models: ModelView,
    pub data: DataView,
}

impl DebugViewState {
    /// True while at least one mode is active (the legend is shown only then).
    pub fn any_active(&self) -> bool {
        self.collisions != CollisionView::Off
            || self.models != ModelView::Normal
            || self.data != DataView::Off
    }

    /// The `F1 ... | F2 ... | F3 ...` line of the on-screen legend.
    pub fn legend_line(&self) -> String {
        format!(
            "F1 colisiones: {} | F2 modelos: {} | F3 datos: {}",
            self.collisions.label(),
            self.models.label(),
            self.data.label()
        )
    }

    /// The colour list of the active modes (empty when nothing draws with colours).
    pub fn colors_line(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        match self.collisions {
            CollisionView::Overlay => {
                parts.push("cian=muros");
                parts.push("gris oscuro=suelos");
            }
            CollisionView::Only => {
                parts.push("cian=muros");
                parts.push("verde=navegacion");
                parts.push("rojo=no caminable");
            }
            CollisionView::Off => {}
        }
        if self.data == DataView::Markers {
            parts.push("amarillo=puertas+viaje");
            parts.push("naranja=trepables");
            parts.push("azul=agua");
            parts.push("lima=spawns");
            parts.push("magenta=recogibles");
            parts.push("blanco=PNJ");
            parts.push("rojo=salvajes+radio");
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("colores: {}", parts.join("  "))
        }
    }
}

// ------------------------------------------------------------------ CLI

/// Parse one `--debug-view` spec part into `state` (case-insensitive; see the module list).
///
/// Returns the unknown/invalid text on error; the caller reports it through `game::cli_flags`.
pub fn apply_spec_part(state: &mut DebugViewState, part: &str) -> Result<(), String> {
    let lower = part.trim().to_ascii_lowercase();
    let (key, value) = match lower.split_once('=') {
        Some((k, v)) => (k.trim(), Some(v.trim())),
        None => (lower.as_str(), None),
    };
    match (key, value) {
        ("collisions", None) | ("collisions", Some("overlay")) | ("colisiones", None) => {
            state.collisions = CollisionView::Overlay;
            Ok(())
        }
        ("collisions", Some("off")) => {
            state.collisions = CollisionView::Off;
            Ok(())
        }
        ("collisions", Some("only")) | ("colisiones", Some("solo")) => {
            state.collisions = CollisionView::Only;
            Ok(())
        }
        ("models", None) | ("models", Some("flat")) | ("modelos", None) => {
            state.models = ModelView::Flat;
            Ok(())
        }
        ("models", Some("normal")) | ("models", Some("off")) => {
            state.models = ModelView::Normal;
            Ok(())
        }
        ("models", Some("wireframe"))
        | ("models", Some("wire"))
        | ("wireframe", None)
        | ("alambre", None) => {
            state.models = ModelView::Wireframe;
            Ok(())
        }
        ("data", None) | ("data", Some("player")) | ("datos", None) => {
            state.data = DataView::Player;
            Ok(())
        }
        ("data", Some("off")) => {
            state.data = DataView::Off;
            Ok(())
        }
        ("data", Some("markers")) | ("markers", None) | ("marcas", None) => {
            state.data = DataView::Markers;
            Ok(())
        }
        _ => Err(part.trim().to_string()),
    }
}

/// Build the initial state from `--debug-view` (absent = everything off).
pub fn state_from_args(args: &[String]) -> DebugViewState {
    let mut state = DebugViewState::default();
    let Some(raw) = crate::game::cli_flags::value_of(args, "--debug-view") else {
        return state;
    };
    for part in raw
        .split([',', ';'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        if let Err(bad) = apply_spec_part(&mut state, part) {
            crate::game::cli_flags::report_bad_value(
                "--debug-view",
                &bad,
                "collisions[=off|overlay|only], models[=normal|flat|wireframe], data[=off|player|markers]",
                format!("{:?}", DebugViewState::default()),
            );
        }
    }
    state
}

/// Key pressed -> the mode it cycles.
pub fn key_of(action: DebugKey) -> KeyCode {
    match action {
        DebugKey::Collisions => KeyCode::F1,
        DebugKey::Models => KeyCode::F2,
        DebugKey::Data => KeyCode::F3,
    }
}

/// The three debug keys (kept in one enum so the key map and its tests cannot drift apart).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugKey {
    Collisions,
    Models,
    Data,
}

/// Next state after pressing the key of `action` (pure; the input system applies it).
pub fn next_for(state: DebugViewState, action: DebugKey) -> DebugViewState {
    let mut next = state;
    match action {
        DebugKey::Collisions => next.collisions = next.collisions.next(),
        DebugKey::Models => next.models = next.models.next(),
        DebugKey::Data => next.data = next.data.next(),
    }
    next
}

// ------------------------------------------------------------------ line builder (pure)

/// Accumulates coloured line segments (2 vertices per line) for one `LineList` mesh.
#[derive(Debug, Default)]
pub struct LineSet {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    cap: usize,
    clipped: bool,
}

impl LineSet {
    pub fn new(cap: usize) -> Self {
        Self { cap, ..default() }
    }

    /// True when a segment was dropped because the vertex cap was reached.
    pub fn clipped(&self) -> bool {
        self.clipped
    }

    pub fn len(&self) -> usize {
        self.positions.len() / 2
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    pub fn line(&mut self, a: Vec3, b: Vec3, color: Color) {
        if self.positions.len() + 2 > self.cap {
            self.clipped = true;
            return;
        }
        let c = linear(color);
        self.positions.push([a.x, a.y, a.z]);
        self.colors.push(c);
        self.positions.push([b.x, b.y, b.z]);
        self.colors.push(c);
    }

    pub fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, color: Color) {
        self.line(a, b, color);
        self.line(b, c, color);
        self.line(c, a, color);
    }

    /// Circle of `segments` lines on the XZ plane at `center.y`.
    pub fn ring(&mut self, center: Vec3, radius: f32, color: Color, segments: u32) {
        let segments = segments.max(8);
        let mut previous = center + Vec3::new(radius, 0.0, 0.0);
        for i in 1..=segments {
            let a = i as f32 / segments as f32 * std::f32::consts::TAU;
            let point = center + Vec3::new(a.cos() * radius, 0.0, a.sin() * radius);
            self.line(previous, point, color);
            previous = point;
        }
    }

    /// Wire box of half extents `half` around `center`, rotated `yaw` about +Y.
    pub fn box_wire(&mut self, center: Vec3, half: Vec3, yaw: f32, color: Color) {
        let (s, c) = yaw.sin_cos();
        let corner = |x: f32, y: f32, z: f32| center + Vec3::new(x * c + z * s, y, -x * s + z * c);
        let corners = [
            corner(-half.x, -half.y, -half.z),
            corner(half.x, -half.y, -half.z),
            corner(half.x, -half.y, half.z),
            corner(-half.x, -half.y, half.z),
            corner(-half.x, half.y, -half.z),
            corner(half.x, half.y, -half.z),
            corner(half.x, half.y, half.z),
            corner(-half.x, half.y, half.z),
        ];
        for (a, b) in [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0), // bottom
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4), // top
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7), // verticals
        ] {
            self.line(corners[a], corners[b], color);
        }
    }

    /// Wire capsule standing on `base`: two rings plus four verticals.
    pub fn capsule(&mut self, base: Vec3, radius: f32, height: f32, color: Color) {
        let top = base + Vec3::Y * height;
        self.ring(base, radius, color, 16);
        self.ring(top, radius, color, 16);
        for i in 0..4 {
            let a = i as f32 / 4.0 * std::f32::consts::TAU;
            let offset = Vec3::new(a.cos() * radius, 0.0, a.sin() * radius);
            self.line(base + offset, top + offset, color);
        }
    }

    /// Small 3-axis cross at `center`.
    pub fn cross(&mut self, center: Vec3, size: f32, color: Color) {
        self.line(center - Vec3::X * size, center + Vec3::X * size, color);
        self.line(center - Vec3::Z * size, center + Vec3::Z * size, color);
        self.line(center, center + Vec3::Y * size * 1.5, color);
    }

    /// Build the `LineList` mesh; an empty set gets one degenerate segment so the mesh exists.
    pub fn into_mesh(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::LineList,
            bevy::asset::RenderAssetUsages::default(),
        );
        if self.positions.is_empty() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0_f32; 3]; 2]);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0_f32; 4]; 2]);
            return mesh;
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh
    }
}

/// Linear RGBA of a colour for the `ATTRIBUTE_COLOR` vertex data.
fn linear(color: Color) -> [f32; 4] {
    let l = color.to_linear();
    [l.red, l.green, l.blue, l.alpha]
}

/// Add the collision triangles of `instances` (already filtered near the player) to `lines`.
///
/// Wall triangles (|normal.y| <= `WALL_NORMAL_Y`) get the wall colour; floors/roofs the dim one.
pub fn add_collision_lines<'a>(
    lines: &mut LineSet,
    instances: impl Iterator<Item = &'a CollisionInstance>,
) {
    for inst in instances {
        if lines.clipped() {
            return;
        }
        for tri in &inst.mesh.triangles {
            let [t0, t1, t2] = tri.map(|i| {
                let v = inst.mesh.vertices[i as usize];
                inst.transform.transform_point3(Vec3::from(v))
            });
            let normal = (t1 - t0).cross(t2 - t0);
            let color = if normal.length_squared() < 1e-12 {
                colors::FLOOR
            } else if (normal.normalize().y).abs() > WALL_NORMAL_Y {
                colors::FLOOR
            } else {
                colors::WALL
            };
            lines.triangle(t0, t1, t2, color);
        }
    }
}

/// Add the walkable navigation faces near `player` to `lines` (green), and when `sampled_blocked`
/// also the NOT-walkable sampled grid cells (dark red). `ground` supplies a height for the
/// sampled marks (terrain or navmesh).
pub fn add_nav_lines(
    lines: &mut LineSet,
    nav: &NavMesh,
    player: Vec2,
    radius: f32,
    sampled_blocked: bool,
    ground: impl Fn(f32, f32) -> Option<f32>,
) {
    let radius2 = radius * radius;
    for face in &nav.faces {
        if lines.clipped() {
            return;
        }
        let start = face.start_edge;
        let end = start + face.num_edges as u32;
        if start >= nav.edges.len() as u32 || end > nav.edges.len() as u32 {
            continue;
        }
        // Cheap reject: the face's first vertex decides (faces are small convex islands).
        let first = nav.vertices[nav.edges[start as usize].a as usize];
        let center = Vec2::new(first[0], first[2]);
        if center.distance_squared(player) > radius2 {
            continue;
        }
        for e in start..end {
            let edge = nav.edges[e as usize];
            let a = nav.vertices[edge.a as usize];
            let b = nav.vertices[edge.b as usize];
            lines.line(Vec3::from(a), Vec3::from(b), colors::NAV_WALK);
        }
    }
    if !sampled_blocked {
        return;
    }
    let step = NAV_SAMPLE_STEP;
    let half = (radius / step).ceil() as i32;
    let (cx0, cz0) = (
        (player.x / step).floor() as i32,
        (player.y / step).floor() as i32,
    );
    for dx in -half..=half {
        for dz in -half..=half {
            if lines.clipped() {
                return;
            }
            let (cx, cz) = (cx0 + dx, cz0 + dz);
            let (x, z) = ((cx as f32 + 0.5) * step, (cz as f32 + 0.5) * step);
            if Vec2::new(x, z).distance_squared(player) > radius2 {
                continue;
            }
            if nav.is_walkable(x, z) {
                continue;
            }
            let y = ground(x, z).unwrap_or(0.0);
            let center = Vec3::new(x, y + 0.05, z);
            let s = step * 0.25;
            lines.line(
                center - Vec3::X * s,
                center + Vec3::X * s,
                colors::NAV_BLOCKED,
            );
            lines.line(
                center - Vec3::Z * s,
                center + Vec3::Z * s,
                colors::NAV_BLOCKED,
            );
        }
    }
}

// ------------------------------------------------------------------ Bevy resources

/// Marker on every entity spawned by this module (excluded from hiding/material swaps).
#[derive(Component)]
pub struct DebugVisualEntity;

/// Marker of the legend root node.
#[derive(Component)]
struct DebugLegendRoot;
/// Marker of the legend text.
#[derive(Component)]
struct DebugLegendText;
/// Marker of the player-info text.
#[derive(Component)]
struct DebugInfoText;

/// Collision-overlay entity bookkeeping.
#[derive(Resource, Default)]
struct CollisionVisual {
    entity: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<StandardMaterial>>,
    last_player: Vec2,
    last_revision: u64,
    last_mode: Option<CollisionView>,
}

/// Navigation-overlay entity bookkeeping.
#[derive(Resource, Default)]
struct NavVisual {
    entity: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<StandardMaterial>>,
    last_player: Vec2,
    /// Pointer to the loaded `AreaNavigation` the last mesh was built from (zone-change detect).
    last_nav: usize,
}

/// Screen background of `collisions = only`: nearly black, so the thin cyan/green/red lines read
/// well (the game's own clear colour is the sky colour of the current time of day).
const ONLY_BACKDROP: Color = Color::srgb(0.02, 0.025, 0.04);

/// The game clear colour saved while `collisions = only` paints [`ONLY_BACKDROP`].
#[derive(Resource, Default)]
struct BackdropSaved(Option<Color>);

/// World-hiding bookkeeping of `collisions = only` (previous visibility per entity).
#[derive(Resource, Default)]
struct WorldHide {
    active: bool,
    saved: HashMap<Entity, Visibility>,
}

/// Flat-material bookkeeping of `models = flat` (original handle per entity, either material type:
/// characters carry the art-style `ToonMaterial` when the "original" style is on, which is the
/// default in the Graphics settings).
#[derive(Resource, Default)]
struct FlatShade {
    active: bool,
    material: Option<Handle<StandardMaterial>>,
    saved: HashMap<Entity, FlatSaved>,
}

/// The original material of one mesh when `models = flat` turned it grey.
#[derive(Debug, Clone)]
enum FlatSaved {
    Std(Handle<StandardMaterial>),
    Toon(Handle<crate::game::art_style::ToonMaterial>),
    Splat(Handle<crate::game::terrain_splat::TerrainSplatMaterial>),
    Water(Handle<crate::game::water::WaterMaterial>),
}

/// Data-marker entity bookkeeping.
#[derive(Resource, Default)]
struct DataMarkers {
    entity: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<StandardMaterial>>,
    built: bool,
    timer: f32,
    last_zone: String,
    /// `(lines, water bodies, wild, npcs, pickups)` of the last build (log on change only).
    last_counts: (usize, usize, usize, usize, usize),
}

/// Player-info probe (speed/FPS need the previous sample).
#[derive(Resource, Default)]
struct PlayerProbe {
    last_pos: Vec3,
    has_last: bool,
    speed: f32,
    fps: f32,
    visible: bool,
    text: String,
}

/// Optional world resources the data markers read (grouped to stay inside the system param limit).
#[derive(SystemParam)]
struct MarkerWorld<'w> {
    water: Option<Res<'w, WaterBodies>>,
    climb: Option<Res<'w, ClimbSurfaces>>,
    links: Option<Res<'w, ZoneLinks>>,
    reach: Option<Res<'w, DoorReachIndex>>,
    zone: Option<Res<'w, ActiveZoneInfo>>,
    spawners: Option<Res<'w, ActiveAreaSpawners>>,
    nav: Option<Res<'w, GameNavmeshResource>>,
    terrain: Option<Res<'w, TerrainResource>>,
    safe_spawn: Option<Res<'w, SafeSpawnPoint>>,
}

/// Live entity markers (wild creatures, NPCs, pickups).
#[derive(SystemParam)]
struct LiveMarkers<'w, 's> {
    wild: Query<'w, 's, (&'static Transform, &'static WildCreature)>,
    npcs: Query<'w, 's, (&'static Transform, &'static NpcInstance)>,
    pickups: Query<'w, 's, &'static Transform, With<SpawnMarker>>,
}

/// Player-info sources (grouped; all optional so tests do not need the whole game).
#[derive(SystemParam)]
struct InfoSources<'w> {
    clock: Option<Res<'w, GameClock>>,
    zone: Option<Res<'w, ActiveZoneInfo>>,
    ride: Option<Res<'w, RideState>>,
}

/// Ground height under `(x, z)`: navmesh first, terrain second, `fallback` when neither knows.
fn ground_height(world: &MarkerWorld, x: f32, z: f32, fallback: f32) -> f32 {
    world
        .nav
        .as_ref()
        .and_then(|n| n.height_at(x, z))
        .or_else(|| {
            world
                .terrain
                .as_ref()
                .and_then(|t| t.area.sample_world_height(x, z))
        })
        .unwrap_or(fallback)
}

// ------------------------------------------------------------------ systems

/// F1/F2/F3 edges advance their mode cycle (raw `ButtonInput`, like the F5 save hotkey).
fn cycle_modes(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<DebugViewState>) {
    let mut changed = false;
    if keys.just_pressed(key_of(DebugKey::Collisions)) {
        state.collisions = state.collisions.next();
        changed = true;
    }
    if keys.just_pressed(key_of(DebugKey::Models)) {
        state.models = state.models.next();
        changed = true;
    }
    if keys.just_pressed(key_of(DebugKey::Data)) {
        state.data = state.data.next();
        changed = true;
    }
    if changed {
        let line = state.legend_line();
        info!("debug view: {line}");
    }
}

/// Keep the collision-overlay line mesh in sync with the mode, the player and the streamed grid.
#[allow(clippy::too_many_arguments)]
fn sync_collision_lines(
    mut commands: Commands,
    state: Res<DebugViewState>,
    mut visual: ResMut<CollisionVisual>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    collision: Option<Res<WorldCollision>>,
    player: Query<&GlobalTransform, With<PlayerAvatar>>,
) {
    if state.collisions == CollisionView::Off {
        if let Some(entity) = visual.entity.take() {
            commands.entity(entity).despawn();
        }
        visual.mesh = None;
        visual.material = None;
        visual.last_mode = None;
        return;
    }
    let (Some(meshes), Some(materials)) = (meshes.as_deref_mut(), materials.as_deref_mut()) else {
        return;
    };
    let player_pos = player
        .single()
        .map(|t| t.translation())
        .unwrap_or(Vec3::ZERO)
        .xz();
    if visual.entity.is_none() {
        let material = materials.add(line_material());
        let mesh = meshes.add(LineSet::new(2).into_mesh());
        let entity = commands
            .spawn((
                DebugVisualEntity,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::default(),
            ))
            .id();
        visual.entity = Some(entity);
        visual.mesh = Some(mesh);
        visual.material = Some(material);
        visual.last_mode = None;
    }
    let revision = collision.as_ref().map_or(0, |c| c.collision_revision());
    let moved = visual.last_player.distance(player_pos) > REBUILD_MOVE;
    let mode_changed = visual.last_mode != Some(state.collisions);
    if !moved && !mode_changed && revision == visual.last_revision {
        return;
    }
    visual.last_player = player_pos;
    visual.last_revision = revision;
    visual.last_mode = Some(state.collisions);
    let mut lines = LineSet::new(MAX_VIEW_VERTICES);
    if let Some(collision) = collision.as_deref() {
        add_collision_lines(
            &mut lines,
            collision
                .instances_near(player_pos, COLLISION_RADIUS)
                .into_iter(),
        );
    }
    if let Some(handle) = &visual.mesh {
        if let Some(mut mesh) = meshes.get_mut(handle) {
            *mesh = lines.into_mesh();
        }
    }
}

/// Keep the navigation overlay in sync (drawn in `collisions = only`, with the sampled
/// not-walkable grid).
fn sync_nav_lines(
    mut commands: Commands,
    state: Res<DebugViewState>,
    mut visual: ResMut<NavVisual>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    nav: Option<Res<GameNavmeshResource>>,
    terrain: Option<Res<TerrainResource>>,
    player: Query<&GlobalTransform, With<PlayerAvatar>>,
) {
    let want = state.collisions == CollisionView::Only;
    if !want {
        if let Some(entity) = visual.entity.take() {
            commands.entity(entity).despawn();
        }
        visual.mesh = None;
        visual.material = None;
        return;
    }
    let (Some(meshes), Some(materials)) = (meshes.as_deref_mut(), materials.as_deref_mut()) else {
        return;
    };
    let Some(nav) = nav else {
        return;
    };
    let player_pos = player
        .single()
        .map(|t| t.translation())
        .unwrap_or(Vec3::ZERO)
        .xz();
    if visual.entity.is_none() {
        let material = materials.add(line_material());
        let mesh = meshes.add(LineSet::new(2).into_mesh());
        let entity = commands
            .spawn((
                DebugVisualEntity,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::default(),
            ))
            .id();
        visual.entity = Some(entity);
        visual.mesh = Some(mesh);
        visual.material = Some(material);
        visual.last_nav = 0;
    }
    let nav_ptr = std::sync::Arc::as_ptr(&nav.nav) as *const AreaNavigation as usize;
    let moved = visual.last_player.distance(player_pos) > REBUILD_MOVE * 2.0;
    if !moved && nav_ptr == visual.last_nav {
        return;
    }
    visual.last_player = player_pos;
    visual.last_nav = nav_ptr;
    let mut lines = LineSet::new(MAX_VIEW_VERTICES);
    add_nav_lines(
        &mut lines,
        nav.nav.primary(),
        player_pos,
        NAV_RADIUS,
        true,
        |x, z| {
            nav.height_at(x, z).or_else(|| {
                terrain
                    .as_ref()
                    .and_then(|t| t.area.sample_world_height(x, z))
            })
        },
    );
    if lines.clipped() {
        debug!("debug view: nav lines clipped at {} lines", lines.len());
    }
    if let Some(handle) = &visual.mesh {
        if let Some(mut mesh) = meshes.get_mut(handle) {
            *mesh = lines.into_mesh();
        }
    }
}

/// `collisions = only`: hide every non-debug `Mesh3d` entity, restoring the exact previous
/// `Visibility` when the mode leaves `only` (and each frame catching newly streamed models).
fn sync_world_hiding(
    state: Res<DebugViewState>,
    mut hide: ResMut<WorldHide>,
    mut meshes: Query<(Entity, &mut Visibility), (With<Mesh3d>, Without<DebugVisualEntity>)>,
) {
    let want = state.collisions == CollisionView::Only;
    if !want {
        if !hide.active {
            return;
        }
        let saved: Vec<(Entity, Visibility)> = hide.saved.drain().collect();
        for (entity, visibility) in saved {
            if let Ok((_, mut current)) = meshes.get_mut(entity) {
                *current = visibility;
            }
        }
        hide.active = false;
        return;
    }
    hide.active = true;
    for (entity, mut visibility) in &mut meshes {
        if hide.saved.contains_key(&entity) {
            continue;
        }
        hide.saved.insert(entity, *visibility);
        *visibility = Visibility::Hidden;
    }
}

/// `collisions = only`: paint a dark screen background so the lines stand out, and give the game
/// colour back when the mode leaves `only`.
///
/// The sky dome and clouds are already hidden by [`sync_world_hiding`]; what is left behind is the
/// `ClearColor`, which the lighting module rewrites whenever the time of day or weather blends to a
/// new key. Running in `Last` (right before extraction) keeps the dark colour on screen even then,
/// and a colour the game wrote in between is the one that is saved and restored.
fn sync_dark_backdrop(
    state: Res<DebugViewState>,
    clear: Option<ResMut<ClearColor>>,
    mut saved: ResMut<BackdropSaved>,
) {
    // Headless test apps (no camera plugin) have no `ClearColor`: nothing to paint.
    let Some(mut clear) = clear else {
        return;
    };
    if state.collisions == CollisionView::Only {
        if clear.0 != ONLY_BACKDROP {
            saved.0 = Some(clear.0);
            clear.0 = ONLY_BACKDROP;
        }
    } else if let Some(game) = saved.0.take() {
        if clear.0 == ONLY_BACKDROP {
            clear.0 = game;
        }
    }
}

/// `models = flat`: swap every mesh material to a shared grey material; restore the original
/// handles when the mode leaves `flat`.
///
/// Four material types are covered: `StandardMaterial` (world, pickups, fallback meshes...), the
/// art-style `ToonMaterial` (characters when the "original" style is on, the Graphics default),
/// the `TerrainSplatMaterial` of the real-time terrain (`--terrain splat`, the default) and the
/// `WaterMaterial` of the rivers/sea. A non-standard mesh is switched by removing its own material
/// component and inserting the grey `StandardMaterial` (having two material components would
/// double-draw the mesh); the saved handle restores the exact original later. The terrain-splat and
/// water appliers only touch meshes without their own marker component, so a swap while they run
/// stays put; if one of them re-applies anyway, the next frame swaps it again.
fn sync_flat_materials(
    mut commands: Commands,
    state: Res<DebugViewState>,
    mut flat: ResMut<FlatShade>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut std_meshes: Query<
        (Entity, &mut MeshMaterial3d<StandardMaterial>),
        Without<DebugVisualEntity>,
    >,
    toon_meshes: Query<
        (
            Entity,
            &MeshMaterial3d<crate::game::art_style::ToonMaterial>,
        ),
        Without<DebugVisualEntity>,
    >,
    splat_meshes: Query<
        (
            Entity,
            &MeshMaterial3d<crate::game::terrain_splat::TerrainSplatMaterial>,
        ),
        Without<DebugVisualEntity>,
    >,
    water_meshes: Query<
        (Entity, &MeshMaterial3d<crate::game::water::WaterMaterial>),
        Without<DebugVisualEntity>,
    >,
) {
    let want = state.models == ModelView::Flat;
    if !want {
        if !flat.active {
            return;
        }
        let saved: Vec<(Entity, FlatSaved)> = flat.saved.drain().collect();
        for (entity, original) in saved {
            match original {
                FlatSaved::Std(handle) => {
                    if let Ok((_, mut current)) = std_meshes.get_mut(entity) {
                        current.0 = handle;
                    } else {
                        commands.entity(entity).insert(MeshMaterial3d(handle));
                    }
                }
                FlatSaved::Toon(handle) => {
                    commands
                        .entity(entity)
                        .remove::<MeshMaterial3d<StandardMaterial>>()
                        .insert(MeshMaterial3d(handle));
                }
                FlatSaved::Splat(handle) => {
                    commands
                        .entity(entity)
                        .remove::<MeshMaterial3d<StandardMaterial>>()
                        .insert(MeshMaterial3d(handle));
                }
                FlatSaved::Water(handle) => {
                    commands
                        .entity(entity)
                        .remove::<MeshMaterial3d<StandardMaterial>>()
                        .insert(MeshMaterial3d(handle));
                }
            }
        }
        flat.active = false;
        return;
    }
    flat.active = true;
    let Some(materials) = materials.as_deref_mut() else {
        return;
    };
    let handle = flat
        .material
        .get_or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.55, 0.55, 0.55),
                perceptual_roughness: 0.9,
                metallic: 0.0,
                ..default()
            })
        })
        .clone();
    for (entity, mut material) in &mut std_meshes {
        if material.0 == handle {
            continue;
        }
        flat.saved
            .entry(entity)
            .or_insert_with(|| FlatSaved::Std(material.0.clone()));
        material.0 = handle.clone();
    }
    for (entity, toon) in &toon_meshes {
        // The art style may convert a freshly streamed character out of the grey material again:
        // re-apply every frame it still carries the toon component (the saved original stays).
        flat.saved
            .entry(entity)
            .or_insert_with(|| FlatSaved::Toon(toon.0.clone()));
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<crate::game::art_style::ToonMaterial>>()
            .insert(MeshMaterial3d(handle.clone()));
    }
    for (entity, splat) in &splat_meshes {
        flat.saved
            .entry(entity)
            .or_insert_with(|| FlatSaved::Splat(splat.0.clone()));
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<crate::game::terrain_splat::TerrainSplatMaterial>>()
            .insert(MeshMaterial3d(handle.clone()));
    }
    for (entity, water) in &water_meshes {
        flat.saved
            .entry(entity)
            .or_insert_with(|| FlatSaved::Water(water.0.clone()));
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<crate::game::water::WaterMaterial>>()
            .insert(MeshMaterial3d(handle.clone()));
    }
}

/// `models = wireframe`: toggle Bevy's global wireframe config.
fn sync_wireframe(
    state: Res<DebugViewState>,
    config: Option<ResMut<bevy::pbr::wireframe::WireframeConfig>>,
) {
    let want = state.models == ModelView::Wireframe;
    if let Some(mut config) = config {
        if config.global != want {
            config.global = want;
            info!("debug view: wireframe global = {want}");
        }
    }
}

/// `data = markers`: rebuild the coloured interest markers (interval-based while active).
#[allow(clippy::too_many_arguments)]
fn sync_data_markers(
    mut commands: Commands,
    state: Res<DebugViewState>,
    mut markers: ResMut<DataMarkers>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    time: Res<Time>,
    player: Query<&GlobalTransform, With<PlayerAvatar>>,
    world: MarkerWorld,
    live: LiveMarkers,
) {
    if state.data != DataView::Markers {
        if let Some(entity) = markers.entity.take() {
            commands.entity(entity).despawn();
        }
        markers.mesh = None;
        markers.material = None;
        markers.built = false;
        markers.last_counts = (0, 0, 0, 0, 0);
        return;
    }
    let (Some(meshes), Some(materials)) = (meshes.as_deref_mut(), materials.as_deref_mut()) else {
        return;
    };
    markers.timer += time.delta_secs();
    let zone_id = world
        .zone
        .as_ref()
        .map(|z| z.area.clone())
        .unwrap_or_default();
    let zone_changed = markers.built && zone_id != markers.last_zone;
    if markers.built && !zone_changed && markers.timer < MARKER_INTERVAL {
        return;
    }
    markers.timer = 0.0;
    markers.last_zone = zone_id;
    if markers.entity.is_none() {
        let material = materials.add(line_material());
        let mesh = meshes.add(LineSet::new(2).into_mesh());
        let entity = commands
            .spawn((
                DebugVisualEntity,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::default(),
            ))
            .id();
        markers.entity = Some(entity);
        markers.mesh = Some(mesh);
        markers.material = Some(material);
    }
    let player_pos = player
        .single()
        .map(|t| t.translation())
        .unwrap_or(Vec3::ZERO)
        .xz();
    let mut lines = LineSet::new(MAX_MARKER_VERTICES);
    add_data_markers(&mut lines, &world, &live, player_pos);
    let clipped = lines.clipped();
    let counts = (
        lines.len(),
        world.water.as_ref().map_or(0, |w| w.bodies.len()),
        live.wild.iter().count(),
        live.npcs.iter().count(),
        live.pickups.iter().count(),
    );
    if counts != markers.last_counts {
        markers.last_counts = counts;
        let doors = match (world.links.as_ref(), world.zone.as_ref()) {
            (Some(links), Some(zone)) => links.doors_in(&zone.area).count(),
            _ => 0,
        };
        let fts = match (world.links.as_ref(), world.zone.as_ref()) {
            (Some(links), Some(zone)) => links.fast_travel_in(&zone.area).count(),
            _ => 0,
        };
        info!(
            "debug view markers: {} lines ({} water bodies, {} wild, {} npcs, {} pickups, {} doors + {} fast travel){}",
            counts.0,
            counts.1,
            counts.2,
            counts.3,
            counts.4,
            doors,
            fts,
            if clipped { " [clipped]" } else { "" }
        );
    }
    markers.built = true;
    if let Some(handle) = &markers.mesh {
        if let Some(mut mesh) = meshes.get_mut(handle) {
            *mesh = lines.into_mesh();
        }
    }
}

/// The pure marker builder: every source of the F3 `markers` state (see the module colour table).
fn add_data_markers(lines: &mut LineSet, world: &MarkerWorld, live: &LiveMarkers, player: Vec2) {
    let near = |p: Vec3| p.xz().distance(player) <= MARKER_RADIUS;

    // Doors (and fast-travel points) of the active area, with their interaction reach.
    if let (Some(links), Some(zone)) = (world.links.as_ref(), world.zone.as_ref()) {
        let reach_index = world.reach.as_deref();
        for link in links
            .doors_in(&zone.area)
            .chain(links.fast_travel_in(&zone.area))
        {
            if lines.clipped() {
                return;
            }
            if !link.is_usable() || !near(link.pos) {
                continue;
            }
            let y = ground_height(world, link.pos.x, link.pos.z, link.pos.y);
            let center = Vec3::new(link.pos.x, y + link.trigger_h * 0.5, link.pos.z);
            let half = Vec3::new(
                (link.trigger.x * 0.5).max(0.5),
                (link.trigger_h * 0.5).max(0.5),
                (link.trigger.y * 0.5).max(0.5),
            );
            lines.box_wire(center, half, link.yaw_deg.to_radians(), colors::DOOR);
            // A tall vertical beacon: most doors sit inside their own building, so the marker only
            // becomes visible from outside as a pin above the roof.
            lines.line(
                Vec3::new(link.pos.x, y, link.pos.z),
                Vec3::new(link.pos.x, y + 6.0, link.pos.z),
                colors::DOOR,
            );
            let reach = reach_index.map_or(0.75, |r| r.of(link));
            lines.ring(
                Vec3::new(link.pos.x, y + 0.08, link.pos.z),
                reach,
                colors::DOOR,
                REACH_SEGMENTS,
            );
        }
    }

    // Climbable cliff surfaces (column marker + grasp radius); empty in interiors.
    if let Some(climb) = world.climb.as_ref() {
        for c in climb.patches() {
            if lines.clipped() {
                return;
            }
            if !near(c.bottom) {
                continue;
            }
            let Climbable {
                bottom,
                top,
                radius,
                ..
            } = *c;
            let mid = (bottom + top) * 0.5;
            let half = Vec3::new(
                radius.max(1.0),
                ((top.y - bottom.y) * 0.5).max(0.5),
                radius.max(1.0),
            );
            lines.box_wire(mid, half, 0.0, colors::CLIMB);
            lines.ring(
                Vec3::new(bottom.x, bottom.y + 0.08, bottom.z),
                radius.max(0.5),
                colors::CLIMB,
                REACH_SEGMENTS,
            );
        }
    }

    // Water footprints at their surface height.
    if let Some(water) = world.water.as_ref() {
        for body in &water.bodies {
            if lines.clipped() {
                return;
            }
            // Cheap body reject on the AABB centre before walking its triangles.
            let centre = Vec2::new(
                (body.min.x + body.max.x) * 0.5,
                (body.min.y + body.max.y) * 0.5,
            );
            let extent = (body.max - body.min).length() * 0.5;
            if centre.distance(player) > MARKER_RADIUS + extent {
                continue;
            }
            let y = body.level + 0.05;
            for tri in &body.triangles {
                if lines.clipped() {
                    return;
                }
                lines.triangle(
                    Vec3::new(tri[0].x, y, tri[0].y),
                    Vec3::new(tri[1].x, y, tri[1].y),
                    Vec3::new(tri[2].x, y, tri[2].y),
                    colors::WATER,
                );
            }
        }
    }

    // Wild spawners (circle + marker) and the saved safe spawn point.
    if let Some(spawners) = world.spawners.as_ref() {
        for spawner in &spawners.spawners {
            if lines.clipped() {
                return;
            }
            if !near(spawner.pos) {
                continue;
            }
            let y = ground_height(world, spawner.pos.x, spawner.pos.z, spawner.pos.y);
            lines.cross(
                Vec3::new(spawner.pos.x, y + 0.2, spawner.pos.z),
                0.7,
                colors::SPAWN,
            );
            lines.ring(
                Vec3::new(spawner.pos.x, y + 0.08, spawner.pos.z),
                spawner.radius.max(0.5),
                colors::SPAWN,
                REACH_SEGMENTS,
            );
        }
    }
    if let Some(safe) = world.safe_spawn.as_ref() {
        let y = ground_height(world, safe.0.x, safe.0.y, 0.0);
        lines.cross(Vec3::new(safe.0.x, y + 0.2, safe.0.y), 0.9, colors::SPAWN);
    }

    // Alive pickups / resource nodes.
    for transform in &live.pickups {
        if lines.clipped() {
            return;
        }
        let pos = transform.translation;
        if !near(pos) {
            continue;
        }
        lines.box_wire(pos + Vec3::Y * 0.3, Vec3::splat(0.45), 0.0, colors::PICKUP);
    }

    // NPC capsules.
    for (transform, _) in &live.npcs {
        if lines.clipped() {
            return;
        }
        let pos = transform.translation;
        if !near(pos) {
            continue;
        }
        lines.capsule(pos, 0.4, 1.7, colors::NPC);
    }

    // Wild creature capsules + their aggro radius ring.
    for (transform, creature) in &live.wild {
        if lines.clipped() {
            return;
        }
        if creature.defeated || !near(transform.translation) {
            continue;
        }
        let pos = transform.translation;
        let scale = transform.scale.y.max(0.1);
        lines.capsule(
            pos,
            0.45 * transform.scale.x.max(0.1),
            1.1 * scale,
            colors::WILD,
        );
        let y = ground_height(world, pos.x, pos.z, pos.y);
        lines.ring(
            Vec3::new(pos.x, y + 0.06, pos.z),
            creature.aggro_range.max(1.0),
            colors::WILD,
            RING_SEGMENTS,
        );
    }
}

/// F3 >= `player`: the info line (position, speed, state, zone, sub-area, clock, FPS).
fn update_player_info(
    state: Res<DebugViewState>,
    mut probe: ResMut<PlayerProbe>,
    mut info: Query<(&mut Text, &mut Visibility), With<DebugInfoText>>,
    player: Query<
        (
            &GlobalTransform,
            Option<&PlayerMotionState>,
            Option<&PlayerSwim>,
        ),
        With<PlayerAvatar>,
    >,
    time: Res<Time>,
    sources: InfoSources,
) {
    let want = state.data != DataView::Off;
    if !want && !probe.visible {
        return;
    }
    let dt = time.delta_secs();
    if dt > 0.0 {
        let instant = 1.0 / dt;
        probe.fps = if probe.fps <= 0.0 {
            instant
        } else {
            probe.fps * 0.9 + instant * 0.1
        };
    }
    if let Ok((transform, motion, swim)) = player.single() {
        let pos = transform.translation();
        if probe.has_last && dt > 0.0 {
            let instant = pos.distance(probe.last_pos) / dt;
            probe.speed = if probe.speed <= 0.0 && instant > 0.0 {
                instant.min(60.0)
            } else {
                (probe.speed * 0.85 + instant.min(60.0) * 0.15).min(60.0)
            };
        }
        probe.last_pos = pos;
        probe.has_last = true;
        let mut state_words: Vec<String> = Vec::new();
        state_words.push(match motion.map(|m| m.mode) {
            Some(MotionMode::Airborne) => "en el aire".to_string(),
            Some(MotionMode::Climbing) => "trepando".to_string(),
            _ => "suelo".to_string(),
        });
        if swim.is_some_and(|s| s.state == SwimState::Swimming) {
            state_words.push("nadando".to_string());
        }
        if let Some(ride) = sources.ride.as_ref() {
            if let Some(kind) = ride.mounted {
                state_words.push(format!("montado en {kind:?}"));
            }
        }
        let (area, name) = sources
            .zone
            .as_ref()
            .map(|z| (z.area.clone(), z.name.clone()))
            .unwrap_or_else(|| ("?".to_string(), "?".to_string()));
        let subarea = area
            .split_once("_s")
            .map(|(_, rest)| format!("s{rest}"))
            .unwrap_or_else(|| "-".to_string());
        let clock = sources
            .clock
            .as_ref()
            .map(|c| c.hhmm())
            .unwrap_or_else(|| "--:--".to_string());
        probe.text = format!(
            "JUGADOR pos {:.1}, {:.1}, {:.1} | vel {:.1} m/s | {} | zona {} ({}) | subarea {} | hora {} | FPS {:.0}",
            pos.x,
            pos.y,
            pos.z,
            probe.speed,
            state_words.join(" + "),
            area,
            name,
            subarea,
            clock,
            probe.fps,
        );
    }
    for (mut text, mut visibility) in &mut info {
        if want {
            if text.0 != probe.text {
                text.0 = probe.text.clone();
            }
            if *visibility != Visibility::Visible {
                *visibility = Visibility::Visible;
            }
        } else if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
    }
    probe.visible = want;
}

/// Show/hide the legend (root) and keep its two lines in sync with the state.
fn update_legend(
    state: Res<DebugViewState>,
    mut last: Local<Option<DebugViewState>>,
    mut root: Query<&mut Visibility, With<DebugLegendRoot>>,
    mut legend: Query<&mut Text, With<DebugLegendText>>,
) {
    if last.as_ref() == Some(&*state) {
        return;
    }
    *last = Some(state.clone());
    let active = state.any_active();
    for mut visibility in &mut root {
        let want = if active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
    }
    if !active {
        return;
    }
    let header = state.legend_line();
    let colors = state.colors_line();
    let text = if colors.is_empty() {
        format!("DEBUG VISUAL (F1/F2/F3 ciclan)\n{header}")
    } else {
        format!("DEBUG VISUAL (F1/F2/F3 ciclan)\n{header}\n{colors}")
    };
    for mut legend in &mut legend {
        if legend.0 != text {
            legend.0 = text.clone();
        }
    }
}

/// The shared material of every debug line mesh (unlit, vertex colours, alpha blend).
///
/// No `depth_bias`: wgpu rejects a non-zero depth bias on `LineList` topology pipelines
/// (`Depth bias is not compatible with non-triangle topology`), which killed the viewer while
/// drawing the first overlay frame. The debug geometry offsets itself by a few centimetres
/// instead (`+0.05`/`+0.08` in the builders).
fn line_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    }
}

/// The legend and player-info UI: top-left, below the game's zone box, dark background, hidden
/// until a mode is active. `ZIndex` keeps it above other UI roots regardless of spawn order.
fn spawn_debug_ui(mut commands: Commands) {
    commands
        .spawn((
            DebugLegendRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(160.0),
                left: Val::Px(8.0),
                padding: UiRect::all(Val::Px(6.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            ZIndex(1000),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                DebugLegendText,
                Text::new(""),
                TextFont::from_font_size(14.0),
                TextColor(Color::WHITE),
            ));
            root.spawn((
                DebugInfoText,
                Text::new(""),
                TextFont::from_font_size(13.0),
                TextColor(Color::srgb(0.75, 0.95, 1.0)),
                Visibility::Hidden,
            ));
        });
}

// ------------------------------------------------------------------ plugin

/// Developer visual-debug aid: F1 collisions, F2 models, F3 data overlays. OFF by default.
///
/// Add after `InputMapPlugin` in the viewer; `from_args` reads `--debug-view` so windowless
/// captures can start with a mode already active.
#[derive(Debug, Clone, Default)]
pub struct DebugVisualPlugin {
    /// Initial state (from `--debug-view`); all modes off by default.
    pub initial: DebugViewState,
}

impl DebugVisualPlugin {
    /// Build from the process argument list (`--debug-view ...`).
    pub fn from_args(args: &[String]) -> Self {
        Self {
            initial: state_from_args(args),
        }
    }
}

impl Plugin for DebugVisualPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.initial.clone())
            // Idempotent: `InputMapPlugin` also initialises it in the viewer; this keeps the
            // F1/F2/F3 reader valid when the plugin is used on its own (tests, small examples).
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<CollisionVisual>()
            .init_resource::<NavVisual>()
            .init_resource::<WorldHide>()
            .init_resource::<BackdropSaved>()
            .init_resource::<FlatShade>()
            .init_resource::<DataMarkers>()
            .init_resource::<PlayerProbe>();
        // The wireframe pass the F2 `wireframe` mode toggles (inert while `global = false`; on
        // GPUs without POLYGON_MODE_LINE/IMMEDIATES Bevy logs a warning and the mode draws
        // nothing instead of failing).
        app.add_plugins(bevy::pbr::wireframe::WireframePlugin::default());
        app.add_systems(Startup, spawn_debug_ui);
        // `ClearColor` is what the renderer extracts after `Last`: override it there.
        app.add_systems(Last, sync_dark_backdrop);
        app.add_systems(
            Update,
            (
                cycle_modes,
                sync_collision_lines,
                sync_nav_lines,
                sync_world_hiding,
                sync_flat_materials,
                sync_wireframe,
                sync_data_markers,
                update_player_info,
                update_legend,
            ),
        );
    }
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use bevy::mesh::VertexAttributeValues;

    /// Marker positions of the current `data = markers` mesh.
    fn marker_positions(app: &App) -> Vec<[f32; 3]> {
        let markers = app.world().resource::<DataMarkers>();
        let mesh = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&markers.mesh.clone().expect("marker mesh"))
            .expect("mesh asset");
        match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
            other => panic!("unexpected positions: {other:?}"),
        }
    }

    #[test]
    fn data_markers_draw_wild_capsule_and_aggro_ring() {
        let mut app = headless_app();
        app.update();
        app.world_mut()
            .spawn((PlayerAvatar, Transform::from_xyz(0.0, 0.0, 0.0)));
        app.world_mut().spawn((
            WildCreature {
                species_name: "starly".into(),
                national_dex: 396,
                level: 5,
                spawn_pos: Vec3::new(10.0, 0.0, 0.0),
                aggro_range: 2.8,
                in_battle: false,
                defeated: false,
                spawner_id: "spawner-1".into(),
            },
            Transform::from_xyz(10.0, 0.0, 0.0),
        ));
        app.world_mut().resource_mut::<DebugViewState>().data = DataView::Markers;
        app.update();

        let positions = marker_positions(&app);
        assert!(
            positions.len() > 2,
            "only the placeholder mesh: {} vertices",
            positions.len()
        );
        // The capsule stands on the creature: some vertex above it and near it in XZ.
        assert!(
            positions
                .iter()
                .any(|p| (p[0] - 10.0).abs() < 0.6 && p[2].abs() < 0.6 && p[1] > 0.5),
            "no capsule vertex near the wild creature"
        );
        // The aggro ring lies at `aggro_range` from the creature in XZ.
        assert!(
            positions.iter().any(|p| {
                let r = ((p[0] - 10.0).powi(2) + p[2].powi(2)).sqrt();
                (r - 2.8).abs() < 0.3
            }),
            "no aggro-ring vertex at 2.8 m"
        );
    }

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
    }

    fn release_all(app: &mut App) {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release_all();
        keys.clear();
    }

    fn headless_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(DebugVisualPlugin::default());
        app
    }

    // ---------------------------------------------------------- pure cycling

    #[test]
    fn collision_cycle_wraps_off_overlay_only() {
        assert_eq!(CollisionView::Off.next(), CollisionView::Overlay);
        assert_eq!(CollisionView::Overlay.next(), CollisionView::Only);
        assert_eq!(CollisionView::Only.next(), CollisionView::Off);
        assert_eq!(CollisionView::default(), CollisionView::Off);
    }

    #[test]
    fn model_cycle_wraps_normal_flat_wireframe() {
        assert_eq!(ModelView::Normal.next(), ModelView::Flat);
        assert_eq!(ModelView::Flat.next(), ModelView::Wireframe);
        assert_eq!(ModelView::Wireframe.next(), ModelView::Normal);
        assert_eq!(ModelView::default(), ModelView::Normal);
    }

    #[test]
    fn data_cycle_wraps_off_player_markers() {
        assert_eq!(DataView::Off.next(), DataView::Player);
        assert_eq!(DataView::Player.next(), DataView::Markers);
        assert_eq!(DataView::Markers.next(), DataView::Off);
        assert_eq!(DataView::default(), DataView::Off);
    }

    #[test]
    fn keys_are_f1_f2_f3_and_f5_is_not_touched() {
        assert_eq!(key_of(DebugKey::Collisions), KeyCode::F1);
        assert_eq!(key_of(DebugKey::Models), KeyCode::F2);
        assert_eq!(key_of(DebugKey::Data), KeyCode::F3);
        // The save hotkey F5 must stay out of this module's map.
        assert!(key_of(DebugKey::Collisions) != KeyCode::F5);
        assert!(key_of(DebugKey::Models) != KeyCode::F5);
        assert!(key_of(DebugKey::Data) != KeyCode::F5);
    }

    #[test]
    fn modes_cycle_independently_after_full_loops() {
        let mut state = DebugViewState::default();
        state = next_for(state, DebugKey::Collisions);
        state = next_for(state, DebugKey::Collisions);
        assert_eq!(state.collisions, CollisionView::Only);
        assert_eq!(state.models, ModelView::Normal);
        assert_eq!(state.data, DataView::Off);
        for _ in 0..3 {
            state = next_for(state, DebugKey::Collisions);
        }
        assert_eq!(
            state.collisions,
            CollisionView::Only,
            "three more presses wrap to where it was"
        );
    }

    #[test]
    fn legend_names_keys_modes_and_colours() {
        let mut state = DebugViewState::default();
        assert!(!state.any_active());
        assert!(state.colors_line().is_empty());
        state.collisions = CollisionView::Only;
        state.models = ModelView::Flat;
        state.data = DataView::Markers;
        let line = state.legend_line();
        assert!(line.contains("F1 colisiones: SOLO"), "{line}");
        assert!(line.contains("F2 modelos: SIN TEXTURA"), "{line}");
        assert!(line.contains("F3 datos: JUGADOR+MARCAS"), "{line}");
        let colors = state.colors_line();
        for needle in [
            "cian=muros",
            "verde=navegacion",
            "rojo=no caminable",
            "amarillo=puertas+viaje",
            "naranja=trepables",
            "azul=agua",
            "lima=spawns",
            "magenta=recogibles",
            "blanco=PNJ",
            "rojo=salvajes+radio",
        ] {
            assert!(colors.contains(needle), "missing {needle}: {colors}");
        }
        assert!(state.any_active());
    }

    #[test]
    fn cli_spec_parses_short_and_explicit_forms() {
        let args = |s: &str| s.split_whitespace().map(str::to_string).collect::<Vec<_>>();
        assert_eq!(
            state_from_args(&args(
                "viewer --debug-view collisions=only,models=wireframe,data=markers"
            )),
            DebugViewState {
                collisions: CollisionView::Only,
                models: ModelView::Wireframe,
                data: DataView::Markers,
            }
        );
        assert_eq!(
            state_from_args(&args("viewer --debug-view collisions")).collisions,
            CollisionView::Overlay
        );
        assert_eq!(
            state_from_args(&args("viewer --debug-view wireframe,data")).models,
            ModelView::Wireframe
        );
        assert_eq!(
            state_from_args(&args("viewer --debug-view markers")).data,
            DataView::Markers
        );
        assert_eq!(
            state_from_args(&args("viewer --area 1")),
            DebugViewState::default()
        );
        assert_eq!(
            state_from_args(&args("viewer --debug-view collisions=off")).collisions,
            CollisionView::Off
        );
    }

    // ---------------------------------------------------------- line builder

    fn square(x: f32, z: f32) -> Vec3 {
        Vec3::new(x, 0.0, z)
    }

    #[test]
    fn line_set_caps_segments_and_reports_clipping() {
        let mut set = LineSet::new(4);
        set.line(square(0.0, 0.0), square(1.0, 0.0), colors::WALL);
        set.line(square(0.0, 1.0), square(1.0, 1.0), colors::WALL);
        assert_eq!(set.len(), 2);
        assert!(!set.clipped());
        set.line(square(0.0, 2.0), square(1.0, 2.0), colors::WALL);
        assert_eq!(set.len(), 2, "over the cap nothing is added");
        assert!(set.clipped());
    }

    #[test]
    fn ring_and_box_and_capsule_produce_lines() {
        let mut set = LineSet::new(10_000);
        set.ring(Vec3::ZERO, 5.0, colors::DOOR, 8);
        assert_eq!(set.len(), 8);
        let mut set = LineSet::new(10_000);
        set.box_wire(Vec3::ZERO, Vec3::ONE, 0.0, colors::DOOR);
        assert_eq!(set.len(), 12);
        let mut set = LineSet::new(10_000);
        set.capsule(Vec3::ZERO, 0.4, 1.7, colors::NPC);
        assert_eq!(set.len(), 32 + 4);
    }

    // ---------------------------------------------------------- bevy glue

    #[test]
    fn f_keys_cycle_the_modes_in_a_headless_app() {
        let mut app = headless_app();
        app.update();
        assert_eq!(
            *app.world().resource::<DebugViewState>(),
            DebugViewState::default()
        );

        press(&mut app, KeyCode::F1);
        app.update();
        assert_eq!(
            app.world().resource::<DebugViewState>().collisions,
            CollisionView::Overlay
        );
        release_all(&mut app);
        app.update();

        press(&mut app, KeyCode::F1);
        app.update();
        assert_eq!(
            app.world().resource::<DebugViewState>().collisions,
            CollisionView::Only
        );
        release_all(&mut app);
        app.update();

        press(&mut app, KeyCode::F1);
        app.update();
        assert_eq!(
            app.world().resource::<DebugViewState>().collisions,
            CollisionView::Off
        );
        release_all(&mut app);
        app.update();

        press(&mut app, KeyCode::F2);
        app.update();
        assert_eq!(
            app.world().resource::<DebugViewState>().models,
            ModelView::Flat
        );
        release_all(&mut app);
        app.update();

        press(&mut app, KeyCode::F3);
        app.update();
        assert_eq!(
            app.world().resource::<DebugViewState>().data,
            DataView::Player
        );
    }

    #[test]
    fn off_state_spawns_no_debug_entities_and_touches_nothing() {
        let mut app = headless_app();
        app.update();
        // A sentinel world mesh with its own material and visibility.
        let original_material = {
            let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
            materials.add(StandardMaterial::default())
        };
        let sentinel = {
            let mesh = app
                .world_mut()
                .resource_mut::<Assets<Mesh>>()
                .add(Mesh::new(
                    PrimitiveTopology::TriangleList,
                    bevy::asset::RenderAssetUsages::default(),
                ));
            app.world_mut()
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(original_material.clone()),
                    Transform::default(),
                    Visibility::Inherited,
                ))
                .id()
        };
        for _ in 0..3 {
            app.update();
        }
        // Nothing of the debug view exists...
        let mut debug_entities = app
            .world_mut()
            .query_filtered::<Entity, With<DebugVisualEntity>>();
        assert_eq!(debug_entities.iter(app.world()).count(), 0);
        let mut root = app
            .world_mut()
            .query_filtered::<&Visibility, With<DebugLegendRoot>>();
        assert_eq!(
            root.iter(app.world()).next(),
            Some(&Visibility::Hidden),
            "legend hidden while off"
        );
        // ...and the sentinel was not touched.
        assert_eq!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(sentinel)
                .unwrap()
                .0,
            original_material
        );
        assert_eq!(
            *app.world().get::<Visibility>(sentinel).unwrap(),
            Visibility::Inherited
        );
        assert!(
            app.world().resource::<WorldHide>().saved.is_empty(),
            "nothing was hidden"
        );
        assert!(app.world().resource::<FlatShade>().saved.is_empty());
    }

    #[test]
    fn only_mode_paints_a_dark_backdrop_and_gives_the_game_colour_back() {
        let mut app = headless_app();
        let sky = Color::linear_rgb(0.40, 0.62, 0.95);
        app.insert_resource(ClearColor(sky));
        app.update();
        assert_eq!(app.world().resource::<ClearColor>().0, sky, "off: untouched");

        app.world_mut().resource_mut::<DebugViewState>().collisions = CollisionView::Only;
        app.update();
        assert_eq!(app.world().resource::<ClearColor>().0, ONLY_BACKDROP, "only: dark");

        // The lighting module rewrites the clear colour when the time of day blends to a new key:
        // the dark backdrop is painted again and the new game colour is the one that comes back.
        let dusk = Color::linear_rgb(0.80, 0.45, 0.30);
        app.world_mut().resource_mut::<ClearColor>().0 = dusk;
        app.update();
        assert_eq!(app.world().resource::<ClearColor>().0, ONLY_BACKDROP);

        app.world_mut().resource_mut::<DebugViewState>().collisions = CollisionView::Off;
        app.update();
        assert_eq!(app.world().resource::<ClearColor>().0, dusk, "leaving only restores the game colour");
        assert!(app.world().resource::<BackdropSaved>().0.is_none());
    }

    #[test]
    fn only_mode_hides_world_meshes_and_restores_them_exactly() {
        let mut app = headless_app();
        app.update();
        let sentinel = app
            .world_mut()
            .spawn((
                Mesh3d(Handle::<Mesh>::default()),
                Transform::default(),
                Visibility::Inherited,
            ))
            .id();
        let already_hidden = app
            .world_mut()
            .spawn((
                Mesh3d(Handle::<Mesh>::default()),
                Transform::default(),
                Visibility::Hidden,
            ))
            .id();

        app.world_mut().resource_mut::<DebugViewState>().collisions = CollisionView::Only;
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(sentinel).unwrap(),
            Visibility::Hidden,
            "world mesh hidden in only mode"
        );
        assert_eq!(
            *app.world().get::<Visibility>(already_hidden).unwrap(),
            Visibility::Hidden
        );

        app.world_mut().resource_mut::<DebugViewState>().collisions = CollisionView::Off;
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(sentinel).unwrap(),
            Visibility::Inherited,
            "the exact previous visibility comes back"
        );
        assert_eq!(
            *app.world().get::<Visibility>(already_hidden).unwrap(),
            Visibility::Hidden,
            "an entity that was already hidden stays hidden"
        );
        assert!(app.world().resource::<WorldHide>().saved.is_empty());
    }

    #[test]
    fn flat_mode_swaps_standard_materials_and_restores_them() {
        let mut app = headless_app();
        app.update();
        let original = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::srgb(0.1, 0.2, 0.3),
                ..default()
            });
        let sentinel = app
            .world_mut()
            .spawn((
                Mesh3d(Handle::<Mesh>::default()),
                MeshMaterial3d(original.clone()),
                Transform::default(),
            ))
            .id();

        app.world_mut().resource_mut::<DebugViewState>().models = ModelView::Flat;
        app.update();
        let flat = app
            .world()
            .get::<MeshMaterial3d<StandardMaterial>>(sentinel)
            .unwrap()
            .0
            .clone();
        assert_ne!(flat, original, "the material was swapped");
        assert!(app
            .world()
            .resource::<FlatShade>()
            .saved
            .contains_key(&sentinel));

        app.world_mut().resource_mut::<DebugViewState>().models = ModelView::Normal;
        app.update();
        assert_eq!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(sentinel)
                .unwrap()
                .0,
            original,
            "the exact original handle comes back"
        );
        assert!(app.world().resource::<FlatShade>().saved.is_empty());
    }

    #[test]
    fn flat_mode_swaps_toon_materials_of_characters_and_restores_them() {
        // Characters carry the art-style `ToonMaterial` when the "original" style is on (the
        // Graphics default): flat mode must cover them too (the farm-NPC bug of 2026-10-17).
        let mut app = headless_app();
        app.init_asset::<crate::game::art_style::ToonMaterial>();
        app.update();
        let original: Handle<crate::game::art_style::ToonMaterial> = Handle::default();
        let sentinel = app
            .world_mut()
            .spawn((
                Mesh3d(Handle::<Mesh>::default()),
                MeshMaterial3d(original.clone()),
                Transform::default(),
            ))
            .id();

        app.world_mut().resource_mut::<DebugViewState>().models = ModelView::Flat;
        app.update();
        app.update(); // the material swap uses deferred commands
        assert!(
            app.world()
                .get::<MeshMaterial3d<crate::game::art_style::ToonMaterial>>(sentinel)
                .is_none(),
            "the toon material was removed"
        );
        assert!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(sentinel)
                .is_some(),
            "the grey material was inserted"
        );
        assert!(app
            .world()
            .resource::<FlatShade>()
            .saved
            .contains_key(&sentinel));

        app.world_mut().resource_mut::<DebugViewState>().models = ModelView::Normal;
        app.update();
        app.update();
        assert!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(sentinel)
                .is_none(),
            "the grey material is gone"
        );
        let back = app
            .world()
            .get::<MeshMaterial3d<crate::game::art_style::ToonMaterial>>(sentinel)
            .expect("the toon material is back");
        assert_eq!(back.0, original);
        assert!(app.world().resource::<FlatShade>().saved.is_empty());
    }

    #[test]
    fn flat_mode_swaps_splat_terrain_and_water_and_restores_them() {
        // The real-time terrain (`TerrainSplatMaterial`) and the water (`WaterMaterial`) carry
        // their own material components: flat mode must cover half the ground scene too (the
        // review defect of 2026-10-17: grass, paths and the river kept their look).
        let mut app = headless_app();
        app.init_asset::<crate::game::terrain_splat::TerrainSplatMaterial>();
        app.init_asset::<crate::game::water::WaterMaterial>();
        app.update();
        let splat: Handle<crate::game::terrain_splat::TerrainSplatMaterial> = Handle::default();
        let water: Handle<crate::game::water::WaterMaterial> = Handle::default();
        let terrain = app
            .world_mut()
            .spawn((
                Mesh3d(Handle::<Mesh>::default()),
                MeshMaterial3d(splat.clone()),
                Transform::default(),
            ))
            .id();
        let river = app
            .world_mut()
            .spawn((
                Mesh3d(Handle::<Mesh>::default()),
                MeshMaterial3d(water.clone()),
                Transform::default(),
            ))
            .id();

        app.world_mut().resource_mut::<DebugViewState>().models = ModelView::Flat;
        app.update();
        app.update(); // the material swap uses deferred commands
        for entity in [terrain, river] {
            assert!(
                app.world()
                    .get::<MeshMaterial3d<StandardMaterial>>(entity)
                    .is_some(),
                "the grey material was inserted"
            );
            assert!(
                app.world()
                    .resource::<FlatShade>()
                    .saved
                    .contains_key(&entity),
                "the original handle was saved"
            );
        }
        assert!(
            app.world()
                .get::<MeshMaterial3d<crate::game::terrain_splat::TerrainSplatMaterial>>(terrain)
                .is_none(),
            "the splat material was removed"
        );
        assert!(
            app.world()
                .get::<MeshMaterial3d<crate::game::water::WaterMaterial>>(river)
                .is_none(),
            "the water material was removed"
        );

        app.world_mut().resource_mut::<DebugViewState>().models = ModelView::Normal;
        app.update();
        app.update();
        let back_terrain = app
            .world()
            .get::<MeshMaterial3d<crate::game::terrain_splat::TerrainSplatMaterial>>(terrain)
            .expect("the splat material is back");
        assert_eq!(back_terrain.0, splat);
        let back_water = app
            .world()
            .get::<MeshMaterial3d<crate::game::water::WaterMaterial>>(river)
            .expect("the water material is back");
        assert_eq!(back_water.0, water);
        assert!(app.world().resource::<FlatShade>().saved.is_empty());
    }

    #[test]
    fn collision_lines_split_walls_from_floors() {
        use crate::assets::collision::CollisionMesh;
        use std::sync::Arc;
        // One vertical wall quad (x = 0) and one floor quad (y = 0).
        let mesh = CollisionMesh {
            vertices: vec![
                [0.0, 0.0, -1.0],
                [0.0, 2.0, -1.0],
                [0.0, 2.0, 1.0],
                [0.0, 0.0, 1.0],
                [-1.0, 0.0, -1.0],
                [1.0, 0.0, -1.0],
                [1.0, 0.0, 1.0],
                [-1.0, 0.0, 1.0],
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]],
            quad_count: 2,
            section_count: 1,
            material_names: vec!["test".into()],
        };
        let instance = CollisionInstance {
            mesh: Arc::new(mesh),
            transform: Mat4::IDENTITY,
            aabb_min: Vec3::new(-1.0, 0.0, -1.0),
            aabb_max: Vec3::new(1.0, 2.0, 1.0),
        };
        let mut set = LineSet::new(10_000);
        add_collision_lines(&mut set, std::iter::once(&instance));
        assert_eq!(set.len(), 12, "four triangles, three lines each");
        let wall = linear(colors::WALL);
        let floor = linear(colors::FLOOR);
        assert!(set.colors.iter().any(|c| *c == wall), "wall colour used");
        assert!(set.colors.iter().any(|c| *c == floor), "floor colour used");
        assert_ne!(wall, floor, "the two collision kinds are distinguishable");
    }

    #[test]
    fn legend_ui_starts_hidden_and_appears_with_a_mode() {
        let mut app = headless_app();
        // The UI spawns in Startup; run it without a render/UI plugin (components still spawn).
        app.update();
        let mut roots = app
            .world_mut()
            .query_filtered::<&Visibility, With<DebugLegendRoot>>();
        assert_eq!(
            roots.iter(app.world()).next(),
            Some(&Visibility::Hidden),
            "startup legend is hidden"
        );
        app.world_mut().resource_mut::<DebugViewState>().data = DataView::Player;
        app.update();
        let mut roots = app
            .world_mut()
            .query_filtered::<&Visibility, With<DebugLegendRoot>>();
        assert_eq!(
            roots.iter(app.world()).next(),
            Some(&Visibility::Visible),
            "legend visible with a mode active"
        );
        let mut legend = app
            .world_mut()
            .query_filtered::<&Text, With<DebugLegendText>>();
        let text = legend.iter(app.world()).next().unwrap().0.clone();
        assert!(text.contains("F1 colisiones: APAGADO"), "{text}");
        assert!(text.contains("F3 datos: JUGADOR"), "{text}");
        assert!(text.contains("F1/F2/F3"), "{text}");
    }
}
