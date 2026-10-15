//! Ball kinds, throw ballistics and the bag-backed throw inventory (Brief 18, part a).
//!
//! Extracted from the viewer (`port_viewer.rs`) so the throwing rules are testable without a
//! window. Ball identities are data-driven:
//! - `data/item_effects.tsv` links the Hisui ball item ids to capture-table categories
//!   (`ball_category`; ids 1710-1750, decoded rows).
//! - `data/capture/throwable_parameters.tsv` gives each throwable row's speed `f06_f32`
//!   (balls: 30 or 40 in the decoded rows).
//!
//! Gravity, colours and the suitable-range conditions are ASSUMED: they are not in the decoded
//! sheets. The capture multiplier itself comes from `capture_ball_multipliers.tsv` at roll time
//! (see `capture_world.rs`); nothing here duplicates the capture formula.
//!
//! All logic is pure except [`BallVisuals`] / [`spawn_ball`], which only build meshes.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use bevy::prelude::*;

use crate::battle_engine::items::ItemTable;

use super::save_wire::PlayerProgress;

/// The nine decoded ball lines (Hisui ball items 1710-1750).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BallKind {
    PokeBall,
    GreatBall,
    UltraBall,
    FeatherBall,
    WingBall,
    JetBall,
    HeavyBall,
    LeadenBall,
    GigatonBall,
}

/// Selection / HUD order.
pub const ALL_BALL_KINDS: [BallKind; 9] = [
    BallKind::PokeBall,
    BallKind::GreatBall,
    BallKind::UltraBall,
    BallKind::FeatherBall,
    BallKind::WingBall,
    BallKind::JetBall,
    BallKind::HeavyBall,
    BallKind::LeadenBall,
    BallKind::GigatonBall,
];

impl BallKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::PokeBall => "Poké Ball",
            Self::GreatBall => "Great Ball",
            Self::UltraBall => "Ultra Ball",
            Self::FeatherBall => "Feather Ball",
            Self::WingBall => "Wing Ball",
            Self::JetBall => "Jet Ball",
            Self::HeavyBall => "Heavy Ball",
            Self::LeadenBall => "Leaden Ball",
            Self::GigatonBall => "Gigaton Ball",
        }
    }

    /// Capture-table category (keys of `capture_ball_multipliers.tsv`).
    pub fn category(&self) -> &'static str {
        match self {
            Self::PokeBall => "Regular",
            Self::GreatBall => "Great",
            Self::UltraBall => "Ultra",
            Self::FeatherBall => "Feather",
            Self::WingBall => "Wing",
            Self::JetBall => "Jet",
            Self::HeavyBall => "Heavy",
            Self::LeadenBall => "Leaden",
            Self::GigatonBall => "Gigaton",
        }
    }

    /// Hisui ball item id, as decoded in `item_effects.tsv` (`ball_category` rows) and present
    /// in `capture/throwable_parameters.tsv` (speed `f06_f32`).
    pub fn item_id(&self) -> u16 {
        match self {
            Self::PokeBall => 1710,
            Self::GreatBall => 1711,
            Self::UltraBall => 1712,
            Self::FeatherBall => 1713,
            Self::WingBall => 1746,
            Self::JetBall => 1747,
            Self::HeavyBall => 1748,
            Self::LeadenBall => 1749,
            Self::GigatonBall => 1750,
        }
    }

    pub fn from_category(category: &str) -> Option<Self> {
        ALL_BALL_KINDS
            .iter()
            .copied()
            .find(|k| k.category() == category)
    }

    pub fn from_item_id(item_id: u16) -> Option<Self> {
        ALL_BALL_KINDS
            .iter()
            .copied()
            .find(|k| k.item_id() == item_id)
    }

    /// Upper-hemisphere colour. ASSUMED: the sheets carry no ball colours.
    pub fn top_color(&self) -> Color {
        match self {
            Self::PokeBall => Color::srgb(0.85, 0.15, 0.15),
            Self::GreatBall => Color::srgb(0.15, 0.35, 0.85),
            Self::UltraBall => Color::srgb(0.2, 0.2, 0.2),
            Self::FeatherBall => Color::srgb(0.3, 0.75, 0.85),
            Self::WingBall => Color::srgb(0.25, 0.55, 0.8),
            Self::JetBall => Color::srgb(0.2, 0.4, 0.9),
            Self::HeavyBall => Color::srgb(0.35, 0.35, 0.38),
            Self::LeadenBall => Color::srgb(0.3, 0.3, 0.45),
            Self::GigatonBall => Color::srgb(0.24, 0.24, 0.32),
        }
    }

    /// ASSUMED gravity per weight line (winged line floats, heavy line drops): not decoded.
    /// Keeps the values the viewer shipped before the extraction.
    pub fn gravity(&self) -> f32 {
        match self {
            Self::FeatherBall | Self::WingBall | Self::JetBall => -8.0,
            Self::HeavyBall | Self::LeadenBall | Self::GigatonBall => -24.0,
            _ => -16.0,
        }
    }

    /// Winged line: designed for long throws (ASSUMED condition, see `capture_world`).
    pub fn prefers_long_range(&self) -> bool {
        matches!(self, Self::FeatherBall | Self::WingBall | Self::JetBall)
    }

    /// Heavy line: designed for unaware / backstrike hits (ASSUMED condition).
    pub fn prefers_backstrike(&self) -> bool {
        matches!(self, Self::HeavyBall | Self::LeadenBall | Self::GigatonBall)
    }

    /// Fallback throw speed when `throwable_parameters.tsv` is unavailable. ASSUMED but matches
    /// the decoded rows (winged line 40, the rest 30).
    pub fn default_speed(&self) -> f32 {
        if self.prefers_long_range() {
            40.0
        } else {
            30.0
        }
    }
}

/// One row of `data/capture/throwable_parameters.tsv`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThrowableRow {
    pub item_id: u16,
    pub f05_raw: u32,
    /// `f06_f32`: throw speed (empty in some rows).
    pub speed: Option<f32>,
    /// `f07_string`: throwable class (`ball`, `bait`, `fatigue`, ...).
    pub kind_tag: String,
    /// `f08_string`: selection cue (`warning`, `check`).
    pub cue: String,
    /// `f09_f32`: residual range hint (semantics unknown).
    pub range_hint: Option<f32>,
}

/// Decoded throwable parameter rows, keyed by item id.
#[derive(Debug, Clone, Default, Resource)]
pub struct ThrowableTable {
    rows: Vec<ThrowableRow>,
    by_id: HashMap<u16, usize>,
}

impl ThrowableTable {
    /// Load `<data_root>/capture/throwable_parameters.tsv`.
    pub fn load(data_root: impl AsRef<Path>) -> Result<Self, String> {
        let path = data_root.as_ref().join("capture/throwable_parameters.tsv");
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_tsv(&text)
    }

    pub fn from_tsv(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'));
        let header: Vec<&str> = lines
            .next()
            .ok_or("throwable parameters: missing header")?
            .split('\t')
            .map(|h| h.split(':').next().unwrap_or(h))
            .collect();
        let col = |name: &str| {
            header
                .iter()
                .position(|h| *h == name)
                .ok_or_else(|| format!("throwable parameters: missing column {name}"))
        };
        let (c_id, c_f05, c_speed, c_kind, c_cue, c_range) = (
            col("f00_raw_u32")?,
            col("f05_raw_u32")?,
            col("f06_f32")?,
            col("f07_string")?,
            col("f08_string")?,
            col("f09_f32")?,
        );
        let mut table = Self::default();
        for (n, line) in lines.enumerate() {
            let v: Vec<&str> = line.split('\t').collect();
            if v.len() != header.len() {
                return Err(format!("throwable row {}: column count", n + 2));
            }
            let item_id: u16 = v[c_id]
                .parse()
                .map_err(|_| format!("throwable row {}: bad item id", n + 2))?;
            let row = ThrowableRow {
                item_id,
                f05_raw: v[c_f05].parse().unwrap_or(0),
                speed: v[c_speed].trim().parse().ok(),
                kind_tag: v[c_kind].to_owned(),
                cue: v[c_cue].to_owned(),
                range_hint: v[c_range].trim().parse().ok(),
            };
            table.by_id.insert(item_id, table.rows.len());
            table.rows.push(row);
        }
        Ok(table)
    }

    pub fn get(&self, item_id: u16) -> Option<&ThrowableRow> {
        self.by_id.get(&item_id).map(|i| &self.rows[*i])
    }

    pub fn speed_for(&self, item_id: u16) -> Option<f32> {
        self.get(item_id).and_then(|r| r.speed)
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// Speed + gravity actually used for one throw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThrowParams {
    pub speed: f32,
    pub gravity: f32,
}

impl ThrowParams {
    /// Ball throw parameters: speed from `throwable_parameters.tsv` when loaded, gravity from
    /// the ASSUMED per-line constants.
    pub fn for_ball(kind: BallKind, table: Option<&ThrowableTable>) -> Self {
        let speed = table
            .and_then(|t| t.speed_for(kind.item_id()))
            .unwrap_or_else(|| kind.default_speed());
        Self {
            speed,
            gravity: kind.gravity(),
        }
    }

    /// Bait / fatigue throwable: speed from the table row, generic ASSUMED gravity.
    pub fn for_throwable(item_id: u16, table: Option<&ThrowableTable>) -> Self {
        let speed = table
            .and_then(|t| t.speed_for(item_id))
            .unwrap_or(DEFAULT_BAIT_SPEED);
        Self {
            speed,
            gravity: DEFAULT_BAIT_GRAVITY,
        }
    }
}

/// ASSUMED fallback bait speed (decoded bait rows are 15 or 30).
pub const DEFAULT_BAIT_SPEED: f32 = 15.0;
/// ASSUMED bait gravity (not decoded).
pub const DEFAULT_BAIT_GRAVITY: f32 = -16.0;

/// Velocity that lobs a projectile from `spawn` onto `target` under `gravity`, compensating the
/// flight arc (the arc-loft formula the viewer shipped, now pure and tested).
pub fn launch_velocity(spawn: Vec3, target: Vec3, speed: f32, gravity: f32) -> Vec3 {
    let to_target = target - spawn;
    let target_dist = to_target.length().max(1.0);
    let flight_time = target_dist / speed.max(0.1);
    let arc_loft = 0.5 * gravity.abs() * flight_time * flight_time;
    let compensated_target = target + Vec3::new(0.0, arc_loft, 0.0);
    (compensated_target - spawn).normalize() * speed
}

/// Radius around the camera ray within which a wild target is locked (viewer value).
pub const AIM_LOCK_RADIUS: f32 = 1.0;
/// Ground-ray fallback distance cap (viewer value).
pub const AIM_GROUND_MAX_T: f32 = 60.0;

/// Aim convergence point: nearest wild target on the camera ray, else the ground intersection,
/// else a point ahead of the player / camera. Pure port of the viewer's throw targeting.
pub fn pick_aim_point(
    cam_pos: Vec3,
    cam_forward: Vec3,
    player_pos: Vec3,
    player_forward_horiz: Vec3,
    targets: &[Vec3],
) -> Vec3 {
    let mut best_target: Option<Vec3> = None;
    let mut closest_dist = f32::INFINITY;
    for pkmn_pos in targets {
        let to_pkmn = *pkmn_pos - cam_pos;
        let proj = to_pkmn.dot(cam_forward);
        if proj > 0.0 {
            let closest_on_ray = cam_pos + cam_forward * proj;
            let ray_dist = (pkmn_pos - closest_on_ray).length();
            if ray_dist < AIM_LOCK_RADIUS && proj < closest_dist {
                closest_dist = proj;
                best_target = Some(*pkmn_pos);
            }
        }
    }
    if let Some(target) = best_target {
        return target;
    }
    if cam_forward.y < -0.01 {
        let t_ground = (-cam_pos.y) / cam_forward.y;
        if t_ground > 0.0 && t_ground < AIM_GROUND_MAX_T {
            let ground_pt = cam_pos + cam_forward * t_ground;
            if (ground_pt - player_pos).dot(player_forward_horiz) > 0.5 {
                return ground_pt;
            }
            return player_pos + player_forward_horiz * 8.0 + Vec3::new(0.0, 0.5, 0.0);
        }
        return cam_pos + cam_forward * 15.0;
    }
    cam_pos + cam_forward * 20.0
}

/// Item ids in `progress.bag` that the item table marks as balls of `category` (lowest first:
/// `BTreeMap` iteration is sorted).
pub fn bag_ball_ids(progress: &PlayerProgress, table: &ItemTable, category: &str) -> Vec<u16> {
    progress
        .bag
        .iter()
        .filter(|(_, n)| **n > 0)
        .filter(|(id, _)| {
            table
                .get(**id)
                .and_then(|i| i.ball_category.as_deref())
                .is_some_and(|c| c == category)
        })
        .map(|(id, _)| *id)
        .collect()
}

/// Remaining balls of one kind across every bag id of its category (legacy and Hisui ids).
pub fn ball_count(progress: &PlayerProgress, table: &ItemTable, kind: BallKind) -> u16 {
    bag_ball_ids(progress, table, kind.category())
        .iter()
        .map(|id| progress.bag.get(id).copied().unwrap_or(0))
        .sum()
}

/// Remove one item from the bag. False when the item is absent or at zero.
pub fn consume_item(progress: &mut PlayerProgress, item_id: u16) -> bool {
    match progress.bag.get_mut(&item_id) {
        Some(n) if *n > 0 => {
            *n -= 1;
            true
        }
        _ => false,
    }
}

/// Consume one ball of `kind`, lowest bag id first. False when the bag has none left.
pub fn consume_ball(progress: &mut PlayerProgress, table: &ItemTable, kind: BallKind) -> bool {
    for id in bag_ball_ids(progress, table, kind.category()) {
        if consume_item(progress, id) {
            return true;
        }
    }
    false
}

/// Viewer-side data of a thrown ball. The motion and lifecycle live in `BallBody`
/// (`pla::game::ball_lifecycle`).
#[derive(Component)]
pub struct PokeBall {
    pub kind: BallKind,
    /// Wild Pokémon entity the ball is holding (hidden while it shakes).
    pub target: Option<Entity>,
}

/// Bag-backed throw state: selection, counters and the newest capture status line.
#[derive(Resource)]
pub struct BallInventory {
    pub selected: BallKind,
    pub balls_thrown: u32,
    pub last_capture_status: Option<String>,
    /// Bait item id selected for the bait throw (`None` = none selected yet).
    pub selected_bait: Option<u16>,
}

impl Default for BallInventory {
    fn default() -> Self {
        Self {
            selected: BallKind::PokeBall,
            balls_thrown: 0,
            last_capture_status: None,
            selected_bait: None,
        }
    }
}

impl BallInventory {
    /// Next ball in selection order (wraps).
    pub fn next_ball(&mut self) -> BallKind {
        let idx = ALL_BALL_KINDS
            .iter()
            .position(|k| *k == self.selected)
            .unwrap_or(0);
        self.selected = ALL_BALL_KINDS[(idx + 1) % ALL_BALL_KINDS.len()];
        self.selected
    }

    /// Cycle the selected bait through `available` (bag ids); starts at `None`, wraps.
    pub fn cycle_bait(&mut self, available: &[u16]) -> Option<u16> {
        if available.is_empty() {
            self.selected_bait = None;
            return None;
        }
        self.selected_bait = match self.selected_bait {
            None => Some(available[0]),
            Some(current) => {
                let next = available
                    .iter()
                    .position(|id| *id == current)
                    .map(|i| i + 1);
                match next {
                    Some(i) if i < available.len() => Some(available[i]),
                    _ => None,
                }
            }
        };
        self.selected_bait
    }
}

/// Mesh and material handles shared by every thrown ball (built once, never per throw).
pub struct BallVisuals {
    pub upper: Handle<Mesh>,
    pub lower: Handle<Mesh>,
    pub latch: Handle<Mesh>,
    tops: Vec<Handle<StandardMaterial>>,
    pub bottom: Handle<StandardMaterial>,
    pub latch_mat: Handle<StandardMaterial>,
}

impl BallVisuals {
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        Self {
            upper: meshes.add(Mesh::from(Sphere::new(0.12))),
            lower: meshes.add(Mesh::from(Sphere::new(0.115))),
            latch: meshes.add(Mesh::from(Cylinder::new(0.03, 0.02))),
            tops: ALL_BALL_KINDS
                .iter()
                .map(|k| {
                    materials.add(StandardMaterial {
                        base_color: k.top_color(),
                        perceptual_roughness: 0.4,
                        ..default()
                    })
                })
                .collect(),
            // Hisui wood/cream lower hemisphere
            bottom: materials.add(StandardMaterial {
                base_color: Color::srgb(0.9, 0.88, 0.82),
                perceptual_roughness: 0.7,
                ..default()
            }),
            // Bronze latch
            latch_mat: materials.add(StandardMaterial {
                base_color: Color::srgb(0.7, 0.6, 0.3),
                metallic: 0.8,
                perceptual_roughness: 0.3,
                ..default()
            }),
        }
    }

    pub fn top_material(&self, kind: BallKind) -> Handle<StandardMaterial> {
        let idx = ALL_BALL_KINDS.iter().position(|k| *k == kind).unwrap_or(0);
        self.tops[idx].clone()
    }
}

/// Spawn one ball entity (logical component + simulation + the three visual children).
pub fn spawn_ball(
    commands: &mut Commands,
    visuals: &BallVisuals,
    kind: BallKind,
    position: Vec3,
    velocity: Vec3,
    gravity: f32,
) -> Entity {
    let top_mat = visuals.top_material(kind);
    commands
        .spawn((
            PokeBall { kind, target: None },
            super::ball_lifecycle::BallBody(super::ball_lifecycle::BallSimulation::new(
                position, velocity, gravity,
            )),
            Transform::from_translation(position),
            Visibility::default(),
        ))
        .with_children(|parent| {
            // Ball upper hemisphere
            parent.spawn((
                Mesh3d(visuals.upper.clone()),
                MeshMaterial3d(top_mat),
                Transform::from_xyz(0.0, 0.04, 0.0),
            ));
            // Ball lower hemisphere
            parent.spawn((
                Mesh3d(visuals.lower.clone()),
                MeshMaterial3d(visuals.bottom.clone()),
                Transform::from_xyz(0.0, -0.04, 0.0),
            ));
            // Center latch / button
            parent.spawn((
                Mesh3d(visuals.latch.clone()),
                MeshMaterial3d(visuals.latch_mat.clone()),
                Transform::from_xyz(0.0, 0.0, 0.11)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            ));
        })
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle_engine::items::{HpHeal, ItemEffect};
    use crate::game::save_wire::PlayerProgress;

    const THROWABLE_TSV: &str = "\
f00_raw_u32:u32*\tf05_raw_u32:u32\tf06_f32:f32?\tf07_string:string\tf08_string:string\tf09_f32:f32?
1710\t10\t30.0\tball\twarning\t5.0
1713\t10\t40.0\tball\twarning\t5.0
1746\t10\t40.0\tball\twarning\t5.0
1748\t10\t30.0\tball\twarning\t
1724\t300\t\tbait\tcheck\t30.0
149\t300\t15.0\tbait\tcheck\t15.0
";

    fn fx(id: u16, category: Option<&str>) -> ItemEffect {
        ItemEffect {
            item_id: id,
            price: 0,
            max_quantity: 999,
            effect_kind: if category.is_some() { 0 } else { 1 },
            pocket_code: if category.is_some() { 2 } else { 0 },
            ball_category: category.map(str::to_owned),
            storage_class: 0,
            hp_heal: HpHeal::None,
            cures: vec![],
            cures_confusion: false,
            revives: false,
            pp_restore_one: false,
            pp_restore_all: false,
            pp_amount: 0,
            level_up: false,
        }
    }

    #[test]
    fn ball_ids_and_categories_cover_the_decoded_ball_rows() {
        for kind in ALL_BALL_KINDS {
            assert_eq!(BallKind::from_category(kind.category()), Some(kind));
            assert_eq!(BallKind::from_item_id(kind.item_id()), Some(kind));
        }
        assert_eq!(
            BallKind::from_category("Master"),
            None,
            "undecoded category"
        );
    }

    #[test]
    fn throw_table_reads_speeds_by_item_id() {
        let t = ThrowableTable::from_tsv(THROWABLE_TSV).unwrap();
        assert_eq!(t.len(), 6);
        assert_eq!(t.speed_for(1710), Some(30.0));
        assert_eq!(t.speed_for(1713), Some(40.0));
        assert_eq!(t.speed_for(1724), None, "empty f06 stays unknown");
        assert_eq!(t.get(149).unwrap().kind_tag, "bait");
        let p = ThrowParams::for_ball(BallKind::PokeBall, Some(&t));
        assert_eq!((p.speed, p.gravity), (30.0, -16.0));
        let f = ThrowParams::for_ball(BallKind::FeatherBall, Some(&t));
        assert_eq!((f.speed, f.gravity), (40.0, -8.0));
        let heavy = ThrowParams::for_ball(BallKind::GigatonBall, Some(&t));
        assert_eq!(
            (heavy.speed, heavy.gravity),
            (30.0, -24.0),
            "row + heavy gravity"
        );
    }

    #[test]
    fn throw_table_falls_back_when_absent_or_unknown() {
        let p = ThrowParams::for_ball(BallKind::PokeBall, None);
        assert_eq!((p.speed, p.gravity), (30.0, -16.0));
        let f = ThrowParams::for_ball(BallKind::JetBall, None);
        assert_eq!(f.speed, 40.0, "winged line default");
        let bait = ThrowParams::for_throwable(1724, None);
        assert_eq!(bait.speed, DEFAULT_BAIT_SPEED);
        assert_eq!(bait.gravity, DEFAULT_BAIT_GRAVITY);
    }

    #[test]
    fn launch_velocity_lands_near_the_target() {
        let spawn = Vec3::new(0.0, 1.0, 0.0);
        let target = Vec3::new(0.0, 0.2, 20.0);
        let speed = 30.0;
        let gravity = -16.0;
        let v = launch_velocity(spawn, target, speed, gravity);
        assert!((v.length() - speed).abs() < 1e-4, "speed preserved");
        let dt = 1.0 / 240.0;
        let mut pos = spawn;
        let mut vel = v;
        let mut closest = f32::INFINITY;
        for _ in 0..2000 {
            vel.y += gravity * dt;
            pos += vel * dt;
            closest = closest.min(pos.distance(target));
            if pos.y < 0.0 {
                break;
            }
        }
        assert!(closest < 0.25, "closest approach {closest} m");
    }

    #[test]
    fn aim_point_prefers_a_target_on_the_ray_then_the_ground() {
        let cam = Vec3::new(0.0, 0.5, 0.0);
        let fwd = Vec3::new(0.0, 0.0, -1.0);
        let player = Vec3::ZERO;
        let ahead = Vec3::new(0.0, 0.0, 1.0);
        // Nearest of two Pokemon on the ray wins.
        let p = pick_aim_point(
            cam,
            fwd,
            player,
            ahead,
            &[Vec3::new(0.1, 0.5, -9.0), Vec3::new(0.0, 0.5, -16.0)],
        );
        assert_eq!(p, Vec3::new(0.1, 0.5, -9.0));
        // Down-looking ray aims ahead of the player: it hits the ground plane y = 0.
        let down = Vec3::new(0.0, -1.0, -1.0).normalize();
        let p = pick_aim_point(Vec3::new(0.0, 2.0, 0.0), down, player, -ahead, &[]);
        assert!(p.y.abs() < 1e-3 && p.z < 0.0, "ground point {p:?}");
        // Horizontal-ish ray: point 20 m along the camera ray.
        let flat = Vec3::new(0.0, 0.0, -1.0);
        let p = pick_aim_point(cam, flat, player, ahead, &[]);
        assert_eq!(p, cam + flat * 20.0);
    }

    fn progress_with_bag(entries: &[(u16, u16)]) -> PlayerProgress {
        PlayerProgress {
            bag: entries.iter().copied().collect(),
            ..PlayerProgress::default()
        }
    }

    fn ball_table() -> ItemTable {
        ItemTable::from_items([
            fx(4, Some("Regular")),
            fx(3, Some("Great")),
            fx(1711, Some("Great")),
            fx(17, None),
        ])
    }

    #[test]
    fn bag_counts_and_consumption_use_the_real_bag() {
        let t = ball_table();
        let mut p = progress_with_bag(&[(4, 10), (3, 2), (1711, 1), (17, 5)]);
        assert_eq!(ball_count(&p, &t, BallKind::PokeBall), 10);
        assert_eq!(
            ball_count(&p, &t, BallKind::GreatBall),
            3,
            "legacy + Hisui ids"
        );
        assert_eq!(
            bag_ball_ids(&p, &t, "Great"),
            vec![3, 1711],
            "lowest id first"
        );
        assert!(consume_ball(&mut p, &t, BallKind::GreatBall));
        assert_eq!(ball_count(&p, &t, BallKind::GreatBall), 2);
        assert_eq!(p.bag.get(&3), Some(&1), "legacy id consumed first");
        assert!(consume_ball(&mut p, &t, BallKind::GreatBall));
        assert_eq!(p.bag.get(&3), Some(&0));
        assert!(consume_ball(&mut p, &t, BallKind::GreatBall));
        assert_eq!(ball_count(&p, &t, BallKind::GreatBall), 0);
        assert!(
            !consume_ball(&mut p, &t, BallKind::GreatBall),
            "empty bag refuses"
        );
        assert!(
            !consume_ball(&mut p, &t, BallKind::UltraBall),
            "absent category refuses"
        );
    }

    #[test]
    fn every_ball_kind_matches_the_checked_in_sheets() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let items = ItemTable::load(&root).unwrap();
        let throwables = ThrowableTable::load(&root).unwrap();
        for kind in ALL_BALL_KINDS {
            let row = items
                .get(kind.item_id())
                .unwrap_or_else(|| panic!("item {} missing from the item sheet", kind.item_id()));
            assert!(row.is_ball(), "{} is not a ball row", kind.item_id());
            assert_eq!(
                row.ball_category.as_deref(),
                Some(kind.category()),
                "category of item {}",
                kind.item_id()
            );
            let speed = throwables
                .speed_for(kind.item_id())
                .unwrap_or_else(|| panic!("no decoded speed for {}", kind.item_id()));
            assert_eq!(
                speed,
                kind.default_speed(),
                "decoded speed of item {}",
                kind.item_id()
            );
        }
        for bait in [149u16, 164, 1724] {
            assert_eq!(
                throwables.get(bait).map(|r| r.kind_tag.as_str()),
                Some("bait"),
                "bait row {bait}"
            );
        }
    }

    #[test]
    fn bait_cycle_walks_the_available_ids_and_returns_to_none() {
        let mut inv = BallInventory::default();
        assert_eq!(inv.selected_bait, None);
        let ids = [149, 164];
        assert_eq!(inv.cycle_bait(&ids), Some(149));
        assert_eq!(inv.cycle_bait(&ids), Some(164));
        assert_eq!(inv.cycle_bait(&ids), None);
        assert_eq!(inv.cycle_bait(&[]), None);
    }

    #[test]
    fn selection_cycles_through_every_kind() {
        let mut inv = BallInventory::default();
        assert_eq!(inv.selected, BallKind::PokeBall);
        for _ in 0..ALL_BALL_KINDS.len() {
            inv.next_ball();
        }
        assert_eq!(inv.selected, BallKind::PokeBall, "wraps");
    }
}
