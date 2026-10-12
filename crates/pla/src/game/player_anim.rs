//! The player character as a real skinned glTF model with idle / walk / run clips.
//!
//! The exporter (`player_export`) writes one glTF per outfit variant
//! (`<cache>/player/p1_default/player.gltf`) with nine clips. This module spawns that
//! scene as a child of the `PlayerAvatar` entity, builds an `AnimationGraph` with the
//! idle / walk / run / dash clips and cross-fades between them from the measured planar
//! speed of the avatar. Locomotion (movement, facing, terrain height) stays in the
//! viewer; the model only follows the avatar entity.
//!
//! Root motion: the walk / run / dash clips translate the `origin` bone forward
//! (0 -> 1.7 / 2.85 / 3.3 m per loop). The entity is moved by the locomotion code, so
//! that translation would make the model slide away from, and snap back to, the
//! avatar. Bevy stores curves as opaque trait objects, so the clip cannot be edited
//! per axis; instead the horizontal translation of the `origin` bone is pinned to its
//! rest value every frame after the animation system has written it.

use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle, AnimationNodeIndex};
use bevy::animation::transition::AnimationTransitions;
use bevy::animation::AnimationPlayer;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::PlayerAvatar;

/// Height of the avatar root above the ground (the old capsule was 1.8 m, centred).
pub const AVATAR_ROOT_ABOVE_GROUND: f32 = 0.9;
/// Standing height of the exported model (p1 1.49 m, p2 1.46 m).
pub const MODEL_HEIGHT: f32 = 1.5;
/// Name of the bone that carries the clips' forward root motion.
pub const ROOT_MOTION_BONE: &str = "origin";

/// Cross-fade length between clips.
pub const FADE: Duration = Duration::from_millis(250);

/// Speed (m/s) tiers with hysteresis: (enter from below, leave back down).
const WALK_UP: f32 = 0.35;
const WALK_DOWN: f32 = 0.2;
const RUN_UP: f32 = 4.2;
const RUN_DOWN: f32 = 3.6;
const DASH_UP: f32 = 6.5;
const DASH_DOWN: f32 = 5.8;

/// Locomotion clip chosen from the player speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerClip {
    Idle,
    Walk,
    Run,
    Dash,
}

impl PlayerClip {
    /// Clip name inside the exported glTF.
    pub fn gltf_name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Walk => "walk",
            Self::Run => "run",
            Self::Dash => "dash",
        }
    }
}

/// Picks the clip for `speed` (m/s), keeping `current` while the speed stays inside its
/// hysteresis band so a speed hovering at a threshold does not flicker between clips.
pub fn select_clip(current: PlayerClip, speed: f32) -> PlayerClip {
    use PlayerClip::*;
    let mut c = current;
    // At most three steps are ever needed (idle <-> dash).
    for _ in 0..3 {
        let next = match c {
            Idle if speed > WALK_UP => Walk,
            Walk if speed > RUN_UP => Run,
            Walk if speed < WALK_DOWN => Idle,
            Run if speed > DASH_UP => Dash,
            Run if speed < RUN_DOWN => Walk,
            Dash if speed < DASH_DOWN => Run,
            other => other,
        };
        if next == c {
            break;
        }
        c = next;
    }
    c
}

/// Returns `translation` with the horizontal root motion removed: x/z pinned to the rest
/// pose, the vertical component (jumps, bobbing) kept.
pub fn strip_root_motion(translation: Vec3, rest: Vec3) -> Vec3 {
    Vec3::new(rest.x, translation.y, rest.z)
}

/// One locomotion facing step: turns `current` towards the planar movement direction
/// `(x, z)` (model front is +Z) at a rate of 15 per second, as the capsule avatar did.
/// A zero direction keeps the current facing.
pub fn step_facing(current: Quat, move_xz: Vec2, dt: f32) -> Quat {
    if move_xz.length_squared() <= f32::EPSILON {
        return current;
    }
    let target = Quat::from_rotation_y(move_xz.x.atan2(move_xz.y));
    current.slerp(target, (15.0 * dt).min(1.0))
}

/// Camera heights relative to the avatar root for a model standing `height` metres tall:
/// `(focal_offset_y, shoulder_y)`. The root sits `AVATAR_ROOT_ABOVE_GROUND` above the
/// ground; the orbit focus is on the upper chest (80 %), the aim camera at the
/// shoulders (85 %).
pub fn camera_offsets(height: f32) -> (f32, f32) {
    (0.8 * height - AVATAR_ROOT_ABOVE_GROUND, 0.85 * height - AVATAR_ROOT_ABOVE_GROUND)
}

/// Location of the exported player glTF for `variant` ("p1" | "p2") under `cache_root`,
/// or `None` if the variant is unknown or the file is missing.
pub fn resolve_player_model(cache_root: &Path, variant: &str) -> Option<PathBuf> {
    if !matches!(variant, "p1" | "p2") {
        return None;
    }
    let p = cache_root.join("player").join(format!("{variant}_default")).join("player.gltf");
    p.is_file().then_some(p)
}

/// Asset-server path for `file`: relative to the asset root `work_root` when inside it
/// (as for the Pokémon models), otherwise the absolute path.
pub fn asset_path(file: &Path, work_root: &Path) -> String {
    file.strip_prefix(work_root).unwrap_or(file).to_string_lossy().replace('\\', "/")
}

/// Request: render the player from this glTF (placed on a `PlayerAvatar` entity).
#[derive(Component)]
pub struct PlayerModelSource {
    pub gltf: Handle<Gltf>,
}

/// Marks that the scene child has been spawned.
#[derive(Component)]
pub struct PlayerSceneSpawned;

/// Smoothed planar speed of the avatar (m/s), measured from its transform.
#[derive(Component, Default)]
pub struct PlayerMotion {
    last: Option<Vec3>,
    pub speed: f32,
}

/// Runtime animation state once the model has been wired.
#[derive(Component)]
pub struct PlayerAnimState {
    pub player: Entity,
    pub nodes: [Option<AnimationNodeIndex>; 4],
    pub current: PlayerClip,
    /// Bone whose horizontal translation is pinned (root motion), if found.
    pub root_bone: Option<Entity>,
    pub rest: Vec3,
}

fn slot(c: PlayerClip) -> usize {
    c as usize
}

fn spawn_scene(
    mut commands: Commands,
    gltfs: Res<Assets<Gltf>>,
    q: Query<(Entity, &PlayerModelSource), (With<PlayerAvatar>, Without<PlayerSceneSpawned>)>,
) {
    for (e, src) in &q {
        let Some(gltf) = gltfs.get(&src.gltf) else { continue };
        let Some(scene) = gltf.default_scene.clone().or_else(|| gltf.scenes.first().cloned()) else {
            continue;
        };
        commands.entity(e).insert((PlayerSceneSpawned, PlayerMotion::default())).with_children(|p| {
            // The glTF origin is at the feet; the avatar root is AVATAR_ROOT_ABOVE_GROUND higher.
            p.spawn((
                WorldAssetRoot(scene),
                Transform::from_xyz(0.0, -AVATAR_ROOT_ABOVE_GROUND, 0.0),
                Visibility::default(),
            ));
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn wire_player(
    mut commands: Commands,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    q: Query<(Entity, &PlayerModelSource), (With<PlayerSceneSpawned>, Without<PlayerAnimState>)>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    names: Query<(&Name, &Transform)>,
) {
    for (e, src) in &q {
        let Some(gltf) = gltfs.get(&src.gltf) else { continue };
        // The scene instance must have produced the AnimationPlayer first.
        let Some(player_e) = children.iter_descendants(e).find(|d| players.get(*d).is_ok()) else {
            continue;
        };
        let clip = |c: PlayerClip| gltf.named_animations.get(c.gltf_name()).cloned();
        let Some(idle) = clip(PlayerClip::Idle) else { continue };
        let (mut graph, idle_node) = AnimationGraph::from_clip(idle);
        let mut nodes = [None; 4];
        nodes[slot(PlayerClip::Idle)] = Some(idle_node);
        for c in [PlayerClip::Walk, PlayerClip::Run, PlayerClip::Dash] {
            nodes[slot(c)] = clip(c).map(|h| graph.add_clip(h, 1.0, graph.root));
        }
        let graph_h = graphs.add(graph);

        let mut transitions = AnimationTransitions::new();
        if let Ok(mut player) = players.get_mut(player_e) {
            transitions.play(&mut player, idle_node, Duration::ZERO).repeat();
        }
        commands.entity(player_e).insert((AnimationGraphHandle(graph_h), transitions));

        let root_bone = children
            .iter_descendants(e)
            .find(|d| names.get(*d).is_ok_and(|(n, _)| n.as_str() == ROOT_MOTION_BONE));
        let rest = root_bone.and_then(|b| names.get(b).ok()).map_or(Vec3::ZERO, |(_, t)| t.translation);
        commands.entity(e).insert(PlayerAnimState {
            player: player_e,
            nodes,
            current: PlayerClip::Idle,
            root_bone,
            rest,
        });
    }
}

fn track_motion(time: Res<Time>, mut q: Query<(&Transform, &mut PlayerMotion), With<PlayerAvatar>>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (xf, mut m) in &mut q {
        let p = xf.translation;
        if let Some(last) = m.last {
            let planar = Vec2::new(p.x - last.x, p.z - last.z).length() / dt;
            // A jump this large is a teleport (battle arena, save load), not running.
            let planar = if planar > 25.0 { 0.0 } else { planar };
            m.speed = m.speed * 0.8 + planar * 0.2;
        }
        m.last = Some(p);
    }
}

fn switch_clips(
    mut q: Query<(&mut PlayerAnimState, &PlayerMotion)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (mut st, motion) in &mut q {
        let mut want = select_clip(st.current, motion.speed);
        // A clip missing from the file falls back to the next slower one.
        while st.nodes[slot(want)].is_none() && want != PlayerClip::Idle {
            want = match want {
                PlayerClip::Dash => PlayerClip::Run,
                PlayerClip::Run => PlayerClip::Walk,
                _ => PlayerClip::Idle,
            };
        }
        if want == st.current {
            continue;
        }
        let Some(node) = st.nodes[slot(want)] else { continue };
        if let Ok((mut player, mut tr)) = players.get_mut(st.player) {
            tr.play(&mut player, node, FADE).repeat();
            st.current = want;
        }
    }
}

/// Runs after the animation system wrote the bone transforms and before they propagate.
fn pin_root_motion(q: Query<&PlayerAnimState>, mut bones: Query<&mut Transform>) {
    for st in &q {
        let Some(bone) = st.root_bone else { continue };
        if let Ok(mut t) = bones.get_mut(bone) {
            let fixed = strip_root_motion(t.translation, st.rest);
            if fixed != t.translation {
                t.translation = fixed;
            }
        }
    }
}

/// Adds the player model animation systems. Requires the same reflect-type registration
/// as `PokemonAnimPlugin` (idempotent), so it works with or without that plugin.
pub struct PlayerAnimPlugin;

impl Plugin for PlayerAnimPlugin {
    fn build(&self, app: &mut App) {
        app.world().resource::<AppTypeRegistry>().write().register_derived_types();
        app.add_systems(Update, (spawn_scene, wire_player, track_motion, switch_clips).chain());
        app.add_systems(
            PostUpdate,
            pin_root_motion
                .after(bevy::app::AnimationSystems)
                .before(bevy::transform::TransformSystems::Propagate),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PlayerClip::*;

    #[test]
    fn tiers_from_rest() {
        assert_eq!(select_clip(Idle, 0.0), Idle);
        assert_eq!(select_clip(Idle, 0.3), Idle);
        assert_eq!(select_clip(Idle, 1.5), Walk);
        assert_eq!(select_clip(Idle, 4.5), Run);
        assert_eq!(select_clip(Idle, 8.0), Dash);
        assert_eq!(select_clip(Dash, 0.0), Idle);
    }

    #[test]
    fn hysteresis_holds_inside_band() {
        // Walk stays walk down to 0.2 even though 0.3 < WALK_UP.
        assert_eq!(select_clip(Walk, 0.25), Walk);
        assert_eq!(select_clip(Walk, 0.15), Idle);
        assert_eq!(select_clip(Idle, 0.25), Idle);
        // Run <-> walk around 4 m/s.
        assert_eq!(select_clip(Run, 3.9), Run);
        assert_eq!(select_clip(Run, 3.5), Walk);
        assert_eq!(select_clip(Walk, 4.0), Walk);
        assert_eq!(select_clip(Walk, 4.3), Run);
        // Sprint band.
        assert_eq!(select_clip(Dash, 6.0), Dash);
        assert_eq!(select_clip(Dash, 5.5), Run);
        assert_eq!(select_clip(Run, 6.2), Run);
        assert_eq!(select_clip(Run, 6.6), Dash);
    }

    #[test]
    fn jittering_speed_does_not_flicker() {
        let mut c = Walk;
        let mut changes = 0;
        for i in 0..200 {
            let s = 4.2 + if i % 2 == 0 { 0.15 } else { -0.15 };
            let n = select_clip(c, s);
            if n != c {
                changes += 1;
                c = n;
            }
        }
        assert_eq!(changes, 1, "one switch to Run, then stable");
        assert_eq!(c, Run);
    }

    #[test]
    fn root_motion_is_stripped_on_a_synthetic_walk_clip() {
        // Keyframes like the exported walk clip: origin goes 0 -> 1.714 m forward (+Z).
        let rest = Vec3::ZERO;
        let keys: Vec<Vec3> = (0..=68).map(|i| Vec3::new(0.0, 0.0, 1.714 * i as f32 / 68.0)).collect();
        assert!(keys.last().unwrap().z > 1.7);
        for k in &keys {
            assert_eq!(strip_root_motion(*k, rest), Vec3::ZERO);
        }
        // Sideways drift and a non-zero rest pose are handled; vertical motion is kept.
        let rest = Vec3::new(0.1, 0.0, -0.2);
        assert_eq!(strip_root_motion(Vec3::new(0.5, 0.3, 2.0), rest), Vec3::new(0.1, 0.3, -0.2));
    }

    #[test]
    fn facing_turns_toward_movement_and_is_stable() {
        let start = Quat::IDENTITY; // facing +Z
        // Moving +X: target yaw +90 degrees.
        let mut q = start;
        for _ in 0..120 {
            q = step_facing(q, Vec2::new(1.0, 0.0), 1.0 / 60.0);
        }
        let fwd = q * Vec3::Z;
        assert!((fwd - Vec3::X).length() < 1e-3, "{fwd:?}");
        // Zero input keeps the facing.
        assert_eq!(step_facing(q, Vec2::ZERO, 0.016), q);
        // One short step only partly turns.
        let one = step_facing(start, Vec2::new(1.0, 0.0), 1.0 / 60.0);
        let angle = (one * Vec3::Z).angle_between(Vec3::Z);
        assert!(angle > 0.0 && angle < std::f32::consts::FRAC_PI_2);
        // A huge dt clamps to the target instead of overshooting.
        let snap = step_facing(start, Vec2::new(0.0, -1.0), 10.0);
        assert!(((snap * Vec3::Z) - Vec3::NEG_Z).length() < 1e-3);
    }

    #[test]
    fn camera_heights_follow_the_model() {
        let (focal, shoulder) = camera_offsets(MODEL_HEIGHT);
        // Focus ~1.2 m above the ground, shoulders ~1.28 m (root is 0.9 above ground).
        assert!((focal + AVATAR_ROOT_ABOVE_GROUND - 1.2).abs() < 1e-5);
        assert!((shoulder + AVATAR_ROOT_ABOVE_GROUND - 1.275).abs() < 1e-5);
        assert!(focal < 1.2, "lower than the capsule focus");
    }

    #[test]
    fn model_path_resolution() {
        let dir = std::env::temp_dir().join(format!("pla-player-anim-{}", std::process::id()));
        let model = dir.join("player/p2_default");
        std::fs::create_dir_all(&model).unwrap();
        std::fs::write(model.join("player.gltf"), "{}").unwrap();
        assert_eq!(resolve_player_model(&dir, "p2"), Some(model.join("player.gltf")));
        assert_eq!(resolve_player_model(&dir, "p1"), None, "missing file");
        assert_eq!(resolve_player_model(&dir, "p3"), None, "unknown variant");
        let rel = asset_path(&model.join("player.gltf"), &dir);
        assert_eq!(rel, "player/p2_default/player.gltf");
        assert_eq!(asset_path(Path::new("/x/y.gltf"), &dir), "/x/y.gltf");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
