//! F1 creature colliders: the capsule or push/interaction radius every nearby character really
//! uses, drawn on top of the world (`ENCIMA`) or alone (`SOLO`).
//!
//! The F1 collision view already draws walls, floors and navigation; this module adds the
//! **creatures**: player, follower, NPCs, trainers, wild creatures and the debug-placed models.
//! Each shape is the real radius/height read from the gameplay code (no invented values); where
//! an entity has no body capsule at all, the shape drawn is the push or interaction radius and
//! the legend says so.
//!
//! # Sources (one row per category; VERIFIED = read from the gameplay code)
//!
//! | Category | Drawn as | Value | Source |
//! |---|---|---|---|
//! | Player | capsule | r `0.35` m, h `1.8` m, base = root − 0.9 | `world_collision::{PLAYER_RADIUS, PLAYER_HEIGHT}` (`push_out_player`) |
//! | Follower | push ring | r `1.4` m | `follower::MIN_SEPARATION` (the disc the player can never be inside) |
//! | NPC | keep-out ring | r `1.6` m | `npc::MIN_NPC_SEPARATION` (`landing_clear_of_npcs`) |
//! | Trainer | range ring | r `3.0` m | `trainer_encounter::CHALLENGE_RANGE_M` (challenge trigger) |
//! | Wild creature | capsule | r `0.4` m, h `1.2` m, base = root | `wild_wander`'s `push_out_xz(.., 0.4, 1.2)` call (VERIFIED; see [`WILD_RADIUS`]) |
//! | Placed (F7 / `--spawn-species`) | capsule | r `0.4` m, h `1.2` m (reference) | no collider exists: same shape as a wild creature (SUPPOSED, marked in the guide) |
//!
//! Honest notes:
//!
//! * NPCs and the follower have **no movement collision** in the port (the player walks through
//!   them); their rings are the only distance rules the gameplay code enforces. Trainers use
//!   their challenge range; a trainer has no push either.
//! * The wild capsule is the one the wander system pushes the creature out of walls with, at the
//!   creature's root height (the same anchor `wild_wander` passes to `push_out_xz`). Alpha
//!   creatures are drawn with the same capsule: the push does NOT scale with the model.
//! * Capsules are the body; rings stand on the ground (a small cross marks the centre). Entities
//!   explicitly hidden (`Visibility::Hidden`, e.g. during a battle) are skipped, as are defeated
//!   wild creatures and everything farther than [`COLLIDER_RADIUS`] from the player.
//! * The line count is capped by [`MAX_COLLIDER_VERTICES`] like every other debug view; when
//!   clipped, the frame is logged (never silently lost).
//! * Read-only: nothing here touches gameplay state. The module is only active while F1 is on
//!   (`overlay` or `only`); with F1 off the system compares one resource and returns.

use bevy::prelude::*;

use super::debug_spawn::DebugSpawnedSpecies;
use super::debug_visual::{
    line_material, CollisionView, DebugViewState, DebugVisualEntity, LineSet,
};
use super::follower::{Follower, GROUND_OFFSET, MIN_SEPARATION};
use super::national_spawn::NationalSpawn;
use super::npc::{NpcInstance, MIN_NPC_SEPARATION, NPC_ROOT_ABOVE_GROUND};
use super::player_anim::AVATAR_ROOT_ABOVE_GROUND;
use super::trainer_encounter::{TrainerNpc, CHALLENGE_RANGE_M};
use super::wild::WildCreature;
use super::world_collision::{PLAYER_HEIGHT, PLAYER_RADIUS};
use super::PlayerAvatar;

// ------------------------------------------------------------------ tuning

/// Creature colliders are drawn within this distance of the player (m).
pub const COLLIDER_RADIUS: f32 = 40.0;
/// Vertex cap of the creature-collider line mesh (2 vertices per line segment).
pub const MAX_COLLIDER_VERTICES: usize = 60_000;
/// Ring segments of the push/range circles.
const RING_SEGMENTS: u32 = 24;
/// Ring centre cross size (m).
const CROSS_SIZE: f32 = 0.25;
/// Ground rings sit slightly above the surface so they do not z-fight with it (m).
const RING_LIFT: f32 = 0.05;

/// Radius of the wild creature push capsule (m). VERIFIED: the literal `0.4` of
/// `wild_wander::wander_system`'s `push_out_xz(x, y + 0.6, z, 0.4, 1.2)` call.
/// Keep in sync if that call changes.
pub const WILD_RADIUS: f32 = 0.4;
/// Height of the wild creature push capsule (m). VERIFIED: the literal `1.2` of the same call
/// (the capsule spans `[root.y, root.y + 1.2]` around the entity root).
pub const WILD_HEIGHT: f32 = 1.2;

/// Reference capsule of the debug-placed models: no collider exists for them (they are plain
/// models), so the wild creature's is drawn to compare sizes. SUPPOSED (see the module notes).
pub const PLACED_REFERENCE_RADIUS: f32 = WILD_RADIUS;
pub const PLACED_REFERENCE_HEIGHT: f32 = WILD_HEIGHT;

// ------------------------------------------------------------------ colours

/// One colour per creature category (see the module table); a single source of truth for the
/// legend entries this module contributes.
pub mod colors {
    use bevy::prelude::*;

    /// Player capsule: bright violet (no other debug source uses it).
    pub const PLAYER: Color = Color::srgb(0.72, 0.35, 1.00);
    /// Follower push ring: turquoise.
    pub const ALLY: Color = Color::srgb(0.10, 1.00, 0.75);
    /// NPC and trainer rings: the same white as the F3 NPC markers.
    pub const NPC: Color = crate::game::debug_visual::colors::NPC;
    /// Wild creature capsules: the same red as the F3 wild markers.
    pub const WILD: Color = crate::game::debug_visual::colors::WILD;
    /// Debug-placed models (F7 / `--spawn-species`) reference capsule: amber.
    pub const PLACED: Color = Color::srgb(1.00, 0.70, 0.20);
}

/// The colour list this module adds to the F1 legend (Spanish, no accents: the on-screen font
/// has none). Kept next to [`colors`] so labels and values cannot drift apart.
pub fn legend_entries() -> [&'static str; 5] {
    [
        "violeta=jugador",
        "turquesa=seguidor (separacion)",
        "blanco=PNJ/entrenador",
        "rojo=salvaje (capsula)",
        "ambar=colocado (referencia)",
    ]
}

// ------------------------------------------------------------------ pure geometry

/// Base of the player capsule: the avatar root sits [`AVATAR_ROOT_ABOVE_GROUND`] above the feet.
pub fn player_capsule_base(root: Vec3) -> Vec3 {
    root - Vec3::Y * AVATAR_ROOT_ABOVE_GROUND
}

/// Player collider: the exact capsule `world_collision::push_out_player` pushes out of walls.
pub fn add_player_collider(lines: &mut LineSet, root: Vec3) {
    lines.capsule(player_capsule_base(root), PLAYER_RADIUS, PLAYER_HEIGHT, colors::PLAYER);
}

/// Follower collider: the `MIN_SEPARATION` disc it is pushed out of the player with. The
/// follower has no other (the player can never get inside this ring; the follower keeps it too).
pub fn add_follower_collider(lines: &mut LineSet, root: Vec3) {
    let ground = root - Vec3::Y * GROUND_OFFSET;
    ring_with_centre(lines, ground, MIN_SEPARATION, colors::ALLY);
}

/// NPC keep-out ring: landings are moved out of this disc (`npc::MIN_NPC_SEPARATION`). NPCs have
/// no movement collision in the port, so this is their only real distance rule.
pub fn add_npc_collider(lines: &mut LineSet, root: Vec3) {
    let ground = root - Vec3::Y * NPC_ROOT_ABOVE_GROUND;
    ring_with_centre(lines, ground, MIN_NPC_SEPARATION, colors::NPC);
}

/// Trainer challenge range (`CHALLENGE_RANGE_M`): the distance at which the trainer starts the
/// battle prompt. Trainers have no push radius.
pub fn add_trainer_collider(lines: &mut LineSet, root: Vec3) {
    let ground = root - Vec3::Y * NPC_ROOT_ABOVE_GROUND;
    ring_with_centre(lines, ground, CHALLENGE_RANGE_M, colors::NPC);
}

/// Wild creature collider: the exact capsule `wild_wander` pushes the creature out of walls
/// with (`push_out_xz` radius [`WILD_RADIUS`], height [`WILD_HEIGHT`], base at the entity root).
pub fn add_wild_collider(lines: &mut LineSet, root: Vec3) {
    lines.capsule(root, WILD_RADIUS, WILD_HEIGHT, colors::WILD);
}

/// Debug-placed model (F7 panel / `--spawn-species`) reference capsule: these models have no
/// collider in the gameplay, so the wild creature's is drawn (SUPPOSED) anchored at the model
/// root, which is where the placement puts the model scene.
pub fn add_placed_collider(lines: &mut LineSet, root: Vec3) {
    lines.capsule(
        root,
        PLACED_REFERENCE_RADIUS,
        PLACED_REFERENCE_HEIGHT,
        colors::PLACED,
    );
}

/// Ground ring of `radius` plus a small centre cross (so the spot is readable in `SOLO`, where
/// the models are hidden).
fn ring_with_centre(lines: &mut LineSet, ground: Vec3, radius: f32, color: Color) {
    let centre = ground + Vec3::Y * RING_LIFT;
    lines.ring(centre, radius, color, RING_SEGMENTS);
    lines.cross(centre, CROSS_SIZE, color);
}

// ------------------------------------------------------------------ ECS

/// Per-category entity counts and the drawn line count of the last build (log on change only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct DrawnCounts {
    player: usize,
    follower: usize,
    npc: usize,
    trainer: usize,
    wild: usize,
    placed: usize,
    lines: usize,
    clipped: bool,
}

/// Debug-placed models: the F7 panel spawn and the `--spawn-species` national spawn.
type PlacedQuery<'w, 's> = Query<
    'w,
    's,
    (&'static GlobalTransform, Option<&'static Visibility>),
    Or<(With<DebugSpawnedSpecies>, With<NationalSpawn>)>,
>;

/// Bookkeeping of the creature-collider line mesh (one entity, rebuilt while F1 is active).
#[derive(Resource, Default)]
struct ColliderVisual {
    entity: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<StandardMaterial>>,
    last_counts: Option<DrawnCounts>,
}

/// True unless the entity is explicitly hidden (during a battle, for instance).
fn shown(visibility: Option<&Visibility>) -> bool {
    !matches!(visibility, Some(Visibility::Hidden))
}

/// Keep the creature-collider line mesh in sync with the F1 mode, the player and every nearby
/// character. Rebuilt every frame while F1 is on: the shapes follow moving characters and the
/// mesh is tiny (tens of lines per creature, capped by [`MAX_COLLIDER_VERTICES`]).
#[allow(clippy::too_many_arguments)]
fn sync_creature_colliders(
    mut commands: Commands,
    state: Res<DebugViewState>,
    mut visual: ResMut<ColliderVisual>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    player: Query<(&GlobalTransform, Option<&Visibility>), With<PlayerAvatar>>,
    followers: Query<(&GlobalTransform, Option<&Visibility>), With<Follower>>,
    npcs: Query<(&GlobalTransform, Option<&Visibility>), With<NpcInstance>>,
    trainers: Query<(&GlobalTransform, Option<&Visibility>), With<TrainerNpc>>,
    wild: Query<(&GlobalTransform, &WildCreature, Option<&Visibility>)>,
    placed: PlacedQuery,
) {
    if state.collisions == CollisionView::Off {
        if let Some(entity) = visual.entity.take() {
            commands.entity(entity).despawn();
        }
        visual.mesh = None;
        visual.material = None;
        visual.last_counts = None;
        return;
    }
    let (Some(meshes), Some(materials)) = (meshes.as_deref_mut(), materials.as_deref_mut()) else {
        return;
    };
    let player_pos = player
        .single()
        .map(|(t, _)| t.translation())
        .ok();
    let player_xz = player_pos.map(|p| p.xz()).unwrap_or(Vec2::ZERO);
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
    }
    let near = |p: Vec3| p.xz().distance(player_xz) <= COLLIDER_RADIUS;
    let mut lines = LineSet::new(MAX_COLLIDER_VERTICES);
    let mut counts = DrawnCounts::default();
    // Player.
    if let Ok((transform, visibility)) = player.single() {
        if shown(visibility) {
            add_player_collider(&mut lines, transform.translation());
            counts.player = 1;
        }
    }
    // Follower.
    for (transform, visibility) in &followers {
        if lines.clipped() {
            break;
        }
        if shown(visibility) && near(transform.translation()) {
            add_follower_collider(&mut lines, transform.translation());
            counts.follower += 1;
        }
    }
    // NPCs.
    for (transform, visibility) in &npcs {
        if lines.clipped() {
            break;
        }
        if shown(visibility) && near(transform.translation()) {
            add_npc_collider(&mut lines, transform.translation());
            counts.npc += 1;
        }
    }
    // Trainers.
    for (transform, visibility) in &trainers {
        if lines.clipped() {
            break;
        }
        if shown(visibility) && near(transform.translation()) {
            add_trainer_collider(&mut lines, transform.translation());
            counts.trainer += 1;
        }
    }
    // Wild creatures (defeated ones leave the world: skipped like the F3 markers do).
    for (transform, creature, visibility) in &wild {
        if lines.clipped() {
            break;
        }
        if !creature.defeated && shown(visibility) && near(transform.translation()) {
            add_wild_collider(&mut lines, transform.translation());
            counts.wild += 1;
        }
    }
    // Debug-placed models (F7 panel and `--spawn-species`).
    for (transform, visibility) in &placed {
        if lines.clipped() {
            break;
        }
        if shown(visibility) && near(transform.translation()) {
            add_placed_collider(&mut lines, transform.translation());
            counts.placed += 1;
        }
    }
    counts.lines = lines.len();
    counts.clipped = lines.clipped();
    if visual.last_counts != Some(counts) {
        visual.last_counts = Some(counts);
        info!(
            "debug view colliders: {} lines ({} jugador, {} aliado, {} PNJ, {} entrenador, {} salvajes, {} colocados){}",
            counts.lines,
            counts.player,
            counts.follower,
            counts.npc,
            counts.trainer,
            counts.wild,
            counts.placed,
            if counts.clipped { " [clipped]" } else { "" }
        );
    }
    if let Some(handle) = &visual.mesh {
        if let Some(mut mesh) = meshes.get_mut(handle) {
            *mesh = lines.into_mesh();
        }
    }
}

/// Registers the creature-collider overlay. Add it with the visual debug plugin (F1).
pub struct DebugCollidersPlugin;

impl Plugin for DebugCollidersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugViewState>()
            .init_resource::<ColliderVisual>()
            .add_systems(Update, sync_creature_colliders);
    }
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::debug_visual::DebugVisualPlugin;
    use bevy::asset::AssetPlugin;
    use bevy::mesh::VertexAttributeValues;

    /// Test NPC with placeholder names (the routes are UNKNOWN in the port anyway).
    fn npc_at(x: f32, y: f32, z: f32) -> (NpcInstance, Transform) {
        (
            NpcInstance {
                key: "npc-test".into(),
                chara_hash: "hash-test".into(),
                chara_id: None,
                idle_clip: None,
                route_confidence: "test",
            },
            Transform::from_xyz(x, y, z),
        )
    }

    fn headless_app() -> App {
        let mut app = App::new();
        // `TransformPlugin` so `GlobalTransform` follows `Transform` like in the viewer (the
        // systems read world positions; without it every entity would stay at the origin).
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::transform::TransformPlugin,
        ));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(DebugVisualPlugin::default());
        app
    }

    fn collider_positions(app: &App) -> Vec<[f32; 3]> {
        let visual = app.world().resource::<ColliderVisual>();
        let mesh = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&visual.mesh.clone().expect("collider mesh"))
            .expect("mesh asset");
        match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
            other => panic!("unexpected positions: {other:?}"),
        }
    }

    fn wild_at(x: f32, z: f32) -> (WildCreature, Transform) {
        (
            WildCreature {
                species_name: "starly".into(),
                national_dex: 396,
                level: 5,
                spawn_pos: Vec3::new(x, 0.0, z),
                aggro_range: 2.8,
                in_battle: false,
                defeated: false,
                spawner_id: "spawner-1".into(),
            },
            Transform::from_xyz(x, 0.0, z),
        )
    }

    /// Max distance of a vertex set to the capsule axis in XZ, and its Y span.
    fn axis_radius_and_span(positions: &[[f32; 3]], base: Vec3) -> (f32, f32, f32) {
        let mut radius: f32 = 0.0;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for p in positions {
            let dx = p[0] - base.x;
            let dz = p[2] - base.z;
            radius = radius.max((dx * dx + dz * dz).sqrt());
            min_y = min_y.min(p[1]);
            max_y = max_y.max(p[1]);
        }
        (radius, min_y, max_y)
    }

    // ---------------------------------------------------------- pure geometry

    #[test]
    fn player_collider_is_the_world_collision_capsule() {
        let base = Vec3::new(2.0, 10.0, -3.0);
        let root = base + Vec3::Y * AVATAR_ROOT_ABOVE_GROUND;
        let mut lines = LineSet::new(10_000);
        add_player_collider(&mut lines, root);
        assert!(!lines.is_empty() && !lines.clipped());
        // Read the raw vertices back (LineSet has no accessor: rebuild through into_mesh()).
        let mesh = lines.into_mesh();
        let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
            other => panic!("unexpected positions: {other:?}"),
        };
        let (radius, min_y, max_y) = axis_radius_and_span(&positions, base);
        assert!(
            (radius - PLAYER_RADIUS).abs() < 1e-4,
            "capsule radius {radius} != PLAYER_RADIUS {PLAYER_RADIUS}"
        );
        assert!(
            (min_y - base.y).abs() < 1e-4 && (max_y - (base.y + PLAYER_HEIGHT)).abs() < 1e-4,
            "capsule spans [{min_y}, {max_y}], expected [{}, {}]",
            base.y,
            base.y + PLAYER_HEIGHT
        );
    }

    #[test]
    fn wild_collider_spans_the_push_capsule_of_the_wander_system() {
        // The wander system pushes with `push_out_xz(x, y + 0.6, z, 0.4, 1.2)`; a capsule of
        // `radius 0.4` and `height 1.2` centred at `y + 0.6` spans exactly `[y, y + 1.2]`.
        let y = 1.25_f32;
        let centre = y + WILD_HEIGHT * 0.5; // `tf.translation.y + 0.6`
        let half = (WILD_HEIGHT * 0.5 - WILD_RADIUS).max(0.0); // push_out_capsule's `half`
        assert!((centre - half - WILD_RADIUS - y).abs() < 1e-6, "low end != root");
        assert!(
            (centre + half + WILD_RADIUS - (y + WILD_HEIGHT)).abs() < 1e-6,
            "high end != root + height"
        );
        let root = Vec3::new(5.0, y, 7.0);
        let mut lines = LineSet::new(10_000);
        add_wild_collider(&mut lines, root);
        let mesh = lines.into_mesh();
        let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
            other => panic!("unexpected positions: {other:?}"),
        };
        let (radius, min_y, max_y) = axis_radius_and_span(&positions, root);
        assert!((radius - 0.4).abs() < 1e-4, "wild radius {radius}");
        assert!(
            (min_y - y).abs() < 1e-4 && (max_y - (y + 1.2)).abs() < 1e-4,
            "wild capsule spans [{min_y}, {max_y}]"
        );
    }

    #[test]
    fn rings_use_the_documented_push_and_range_radii() {
        for (add, radius, lift) in [
            (
                add_follower_collider as fn(&mut LineSet, Vec3),
                MIN_SEPARATION,
                GROUND_OFFSET,
            ),
            (add_npc_collider, MIN_NPC_SEPARATION, NPC_ROOT_ABOVE_GROUND),
            (add_trainer_collider, CHALLENGE_RANGE_M, NPC_ROOT_ABOVE_GROUND),
        ] {
            let root = Vec3::new(-4.0, 6.0 + lift, 3.0);
            let mut lines = LineSet::new(10_000);
            add(&mut lines, root);
            let mesh = lines.into_mesh();
            let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
                other => panic!("unexpected positions: {other:?}"),
            };
            // Ring vertices sit exactly at `radius` from the entity, at the lifted ground.
            let ring_vertices: Vec<&[f32; 3]> = positions
                .iter()
                .filter(|p| {
                    let d = ((p[0] - root.x).powi(2) + (p[2] - root.z).powi(2)).sqrt();
                    (d - radius).abs() < 0.01
                })
                .collect();
            assert!(
                ring_vertices.len() >= RING_SEGMENTS as usize,
                "radius {radius}: {} ring vertices",
                ring_vertices.len()
            );
            let ground_y = root.y - lift + RING_LIFT;
            assert!(
                ring_vertices.iter().all(|p| (p[1] - ground_y).abs() < 1e-4),
                "radius {radius}: ring not at ground height {ground_y}"
            );
        }
    }

    #[test]
    fn placed_collider_is_the_documented_reference_capsule() {
        let root = Vec3::new(1.0, 2.0, 3.0);
        let mut lines = LineSet::new(10_000);
        add_placed_collider(&mut lines, root);
        let mesh = lines.into_mesh();
        let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
            other => panic!("unexpected positions: {other:?}"),
        };
        let (radius, min_y, max_y) = axis_radius_and_span(&positions, root);
        assert!((radius - PLACED_REFERENCE_RADIUS).abs() < 1e-4);
        assert!((min_y - root.y).abs() < 1e-4);
        assert!((max_y - (root.y + PLACED_REFERENCE_HEIGHT)).abs() < 1e-4);
    }

    #[test]
    fn every_category_draws_its_own_colour() {
        let mut lines = LineSet::new(10_000);
        let root = Vec3::new(0.0, 1.0, 0.0);
        add_player_collider(&mut lines, root);
        add_follower_collider(&mut lines, root);
        add_npc_collider(&mut lines, root);
        add_trainer_collider(&mut lines, root);
        add_wild_collider(&mut lines, root);
        add_placed_collider(&mut lines, root);
        let mesh = lines.into_mesh();
        let colors = match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
            Some(VertexAttributeValues::Float32x4(v)) => v.clone(),
            other => panic!("unexpected colours: {other:?}"),
        };
        let linear = |c: Color| {
            let l = c.to_linear();
            [l.red, l.green, l.blue, l.alpha]
        };
        for expected in [
            colors::PLAYER,
            colors::ALLY,
            colors::NPC,
            colors::WILD,
            colors::PLACED,
        ] {
            let want = linear(expected);
            assert!(
                colors.iter().any(|c| (0..4).all(|i| (c[i] - want[i]).abs() < 1e-5)),
                "colour {want:?} missing from the drawn set"
            );
        }
    }

    #[test]
    fn vertex_cap_clips_instead_of_growing_without_bound() {
        let mut lines = LineSet::new(80);
        for i in 0..100 {
            add_wild_collider(&mut lines, Vec3::new(i as f32, 0.0, 0.0));
        }
        assert!(lines.clipped(), "the cap was not reported");
        assert!(lines.len() * 2 <= 80, "{} vertices over the cap", lines.len() * 2);
    }

    #[test]
    fn legend_entries_cover_every_category_and_name_its_colour() {
        let entries = legend_entries();
        for needle in ["jugador", "seguidor", "PNJ/entrenador", "salvaje", "colocado"] {
            assert!(
                entries.iter().any(|e| e.contains(needle)),
                "missing legend entry for {needle}: {entries:?}"
            );
        }
        for entry in entries {
            assert!(entry.contains('='), "legend entry without a colour: {entry}");
            assert!(
                !entry.chars().any(|c| "\u{e1}\u{e9}\u{ed}\u{f3}\u{fa}".contains(c)),
                "accented character in {entry}: the on-screen font has none"
            );
        }
    }

    // ---------------------------------------------------------- ECS wiring

    #[test]
    fn f1_draws_the_player_and_nearby_characters_only() {
        let mut app = headless_app();
        app.update();
        app.world_mut()
            .spawn((PlayerAvatar, Transform::from_xyz(0.0, 0.9, 0.0)));
        app.world_mut().spawn((
            Follower { species: 25, form: 0 },
            Transform::from_xyz(1.5, 0.45, 0.0),
        ));
        app.world_mut().spawn(npc_at(5.0, 0.9, 0.0));
        app.world_mut().spawn((
            TrainerNpc { key: "t".into(), trainer_id: "t".into() },
            Transform::from_xyz(6.0, 0.9, 0.0),
        ));
        app.world_mut().spawn(wild_at(10.0, 0.0));
        // Far away: beyond COLLIDER_RADIUS.
        app.world_mut().spawn(wild_at(90.0, 0.0));
        app.world_mut()
            .spawn((DebugSpawnedSpecies, Transform::from_xyz(4.0, 0.9, 4.0)));
        app.world_mut().resource_mut::<DebugViewState>().collisions = CollisionView::Overlay;
        // First update propagates the fresh `Transform`s into `GlobalTransform`s (PostUpdate);
        // the second one builds the line mesh from those world positions.
        app.update();
        app.update();

        let positions = collider_positions(&app);
        let near = |x: f32, z: f32| {
            positions
                .iter()
                .any(|p| (p[0] - x).abs() < 0.6 && (p[2] - z).abs() < 0.6)
        };
        assert!(near(0.0, 0.0), "no player capsule at the origin");
        assert!(near(1.5, 0.0), "no follower ring");
        assert!(near(5.0, 0.0), "no NPC ring");
        assert!(near(6.0, 0.0), "no trainer ring");
        assert!(near(10.0, 0.0), "no wild capsule nearby");
        assert!(!near(90.0, 0.0), "the far wild creature was drawn");
        assert!(near(4.0, 4.0), "no placed reference capsule");
    }

    #[test]
    fn leaving_f1_despawns_the_collider_mesh() {
        let mut app = headless_app();
        app.update();
        app.world_mut()
            .spawn((PlayerAvatar, Transform::from_xyz(0.0, 0.9, 0.0)));
        app.world_mut().resource_mut::<DebugViewState>().collisions = CollisionView::Only;
        app.update();
        app.update();
        assert!(app.world().resource::<ColliderVisual>().entity.is_some());
        assert!(!collider_positions(&app).is_empty());

        app.world_mut().resource_mut::<DebugViewState>().collisions = CollisionView::Off;
        app.update();
        assert!(
            app.world().resource::<ColliderVisual>().entity.is_none(),
            "the mesh entity survived F1 off"
        );
    }

    #[test]
    fn hidden_and_defeated_characters_are_skipped() {
        let mut app = headless_app();
        app.update();
        app.world_mut()
            .spawn((PlayerAvatar, Transform::from_xyz(0.0, 0.9, 0.0)));
        app.world_mut().spawn((
            npc_at(5.0, 0.9, 0.0).0,
            Visibility::Hidden,
            Transform::from_xyz(5.0, 0.9, 0.0),
        ));
        let (mut creature, transform) = wild_at(10.0, 0.0);
        creature.defeated = true;
        app.world_mut().spawn((creature, transform));
        app.world_mut().resource_mut::<DebugViewState>().collisions = CollisionView::Overlay;
        app.update();
        app.update();

        let positions = collider_positions(&app);
        let near = |x: f32, z: f32| {
            positions
                .iter()
                .any(|p| (p[0] - x).abs() < 0.6 && (p[2] - z).abs() < 0.6)
        };
        assert!(near(0.0, 0.0), "the player capsule must still be drawn");
        assert!(!near(5.0, 0.0), "the hidden NPC was drawn");
        assert!(!near(10.0, 0.0), "the defeated creature was drawn");
    }
}
