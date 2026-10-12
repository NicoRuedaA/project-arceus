//! Pokémon skeletal animation (idle / walk) on top of Bevy's glTF scene loader.
//!
//! The batch exporter writes one skinned glTF per clip (`*_idle.gltf`,
//! `*_walk.gltf`) that share the same skeleton. We spawn the idle file's
//! scene (so Bevy builds `SkinnedMesh` + joints + `AnimationPlayer` itself),
//! take the walk clip from the sibling file, and drive both through an
//! `AnimationGraph` + `AnimationTransitions`. Models without clips stay static.

use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle, AnimationNodeIndex};
use bevy::animation::transition::AnimationTransitions;
use bevy::animation::AnimationPlayer;
use bevy::asset::LoadState;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use std::time::Duration;

use super::player_anim::strip_root_motion;

/// Distance per second above which a creature plays its walk clip.
pub const WALK_SPEED_THRESHOLD: f32 = 0.25;

/// Name of the bone that carries the clips' root motion (381 of 382 walk clips of the
/// exported packs move `origin` along +Z and nothing else horizontally at root level).
pub const ROOT_MOTION_BONE: &str = "origin";

/// Playback-speed limits of the walk clip when it is matched to the world speed.
pub const WALK_SPEED_SCALE_MIN: f32 = 0.5;
pub const WALK_SPEED_SCALE_MAX: f32 = 2.0;

/// Clip time that must be measured before the root speed estimate is trusted.
const METER_MIN_TIME: f32 = 0.5;
/// Seconds after switching to walk before the root motion is measured (the cross-fade
/// mixes in the idle pose and would under-report it).
const METER_SETTLE_S: f32 = 0.3;

/// Playback speed that makes the walk clip's own root motion match the speed the entity
/// really moves at: `entity_speed / clip_root_speed`, clamped to 0.5..2. Unknown clip
/// speed (or a clip without root motion) plays at normal speed.
pub fn walk_playback_speed(entity_speed: f32, clip_root_speed: Option<f32>) -> f32 {
    match clip_root_speed {
        Some(c) if c > 0.05 && entity_speed.is_finite() && entity_speed > 0.0 => {
            (entity_speed / c).clamp(WALK_SPEED_SCALE_MIN, WALK_SPEED_SCALE_MAX)
        }
        _ => 1.0,
    }
}

/// Measures the root-motion speed of a clip from the raw (unpinned) root bone positions.
/// Only forward (+Z) steps count; a backwards step is the clip wrapping around.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RootMotionMeter {
    distance: f32,
    time: f32,
}

impl RootMotionMeter {
    /// One frame: the root bone moved `dz` along +Z during `clip_dt` seconds of clip time.
    pub fn observe(&mut self, dz: f32, clip_dt: f32) {
        if clip_dt > 0.0 && dz >= 0.0 {
            self.distance += dz;
            self.time += clip_dt;
        }
    }

    /// Clip root speed in metres per clip-second, once enough time was observed.
    pub fn speed(&self) -> Option<f32> {
        (self.time >= METER_MIN_TIME).then(|| self.distance / self.time)
    }
}

/// Request: render this creature from glTF files with idle (+ optional walk) clips.
#[derive(Component)]
pub struct PokemonAnimSource {
    pub idle: Handle<Gltf>,
    pub walk: Option<Handle<Gltf>>,
}

impl PokemonAnimSource {
    /// `idle_rel` is an asset path such as `cache/pokemon_batch/pm0025_00_00/pm0025_00_00_idle.gltf`;
    /// the walk file is the sibling `_walk.gltf` (absent file simply leaves the model idle-only).
    pub fn from_idle_path(asset_server: &AssetServer, idle_rel: &str) -> Self {
        let walk = idle_rel
            .strip_suffix("_idle.gltf")
            .map(|stem| asset_server.load::<Gltf>(format!("{stem}_walk.gltf")));
        Self { idle: asset_server.load(idle_rel.to_string()), walk }
    }
}

/// Marks that the scene child has been spawned.
#[derive(Component)]
pub struct PokemonSceneSpawned;

/// Runtime animation state once the player has been wired.
#[derive(Component)]
pub struct PokemonAnimState {
    pub player: Entity,
    pub idle: AnimationNodeIndex,
    pub walk: Option<AnimationNodeIndex>,
    pub walking: bool,
    /// Bone whose horizontal translation is pinned (root motion), if found.
    pub root_bone: Option<Entity>,
    /// Rest translation of the root bone (x/z are what it is pinned to).
    pub rest: Vec3,
    /// Measured root motion of the walk clip.
    pub root_meter: RootMotionMeter,
    /// Seconds since the walk clip became active.
    pub walk_time: f32,
    /// Raw (unpinned) root bone z of the previous frame.
    last_raw_z: Option<f32>,
    /// Current playback speed of the walk clip.
    pub walk_speed: f32,
}

impl PokemonAnimState {
    fn new(player: Entity, idle: AnimationNodeIndex, walk: Option<AnimationNodeIndex>) -> Self {
        Self {
            player,
            idle,
            walk,
            walking: false,
            root_bone: None,
            rest: Vec3::ZERO,
            root_meter: RootMotionMeter::default(),
            walk_time: 0.0,
            last_raw_z: None,
            walk_speed: 1.0,
        }
    }
}

/// Tracks displacement of the root so walk/idle can follow the world code.
#[derive(Component, Default)]
pub struct PokemonMotion {
    last: Option<Vec3>,
    pub speed: f32,
}

/// Pick a clip by name prefix (`pm0025_00_00_00000_defaultwait01_loop` contains `defaultwait01`).
pub fn find_clip(gltf: &Gltf, needles: &[&str]) -> Option<Handle<AnimationClip>> {
    let mut names: Vec<&Box<str>> = gltf.named_animations.keys().collect();
    names.sort();
    for needle in needles {
        if let Some(n) = names.iter().find(|n| n.contains(needle)) {
            return gltf.named_animations.get(*n).cloned();
        }
    }
    None
}

fn spawn_scene(
    mut commands: Commands,
    gltfs: Res<Assets<Gltf>>,
    q: Query<(Entity, &PokemonAnimSource), Without<PokemonSceneSpawned>>,
) {
    for (e, src) in &q {
        let Some(gltf) = gltfs.get(&src.idle) else { continue };
        let Some(scene) = gltf.default_scene.clone().or_else(|| gltf.scenes.first().cloned()) else {
            continue;
        };
        commands.entity(e).insert((PokemonSceneSpawned, PokemonMotion::default())).with_children(|p| {
            p.spawn((WorldAssetRoot(scene), Transform::IDENTITY, Visibility::default()));
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn wire_players(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q: Query<(Entity, &PokemonAnimSource), (With<PokemonSceneSpawned>, Without<PokemonAnimState>)>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    mut mats: Query<&mut MeshMaterial3d<StandardMaterial>>,
    names: Query<(&Name, &Transform)>,
) {
    for (e, src) in &q {
        let Some(idle_gltf) = gltfs.get(&src.idle) else { continue };
        // Wait for the walk file unless it failed to load.
        let walk_gltf = match &src.walk {
            Some(h) => match gltfs.get(h) {
                Some(g) => Some(g),
                None if matches!(asset_server.load_state(h), LoadState::Failed(_)) => None,
                None => continue,
            },
            None => None,
        };
        // Scene instance must have produced the player.
        let Some(player_e) = children
            .iter_descendants(e)
            .find(|d| players.get(*d).is_ok())
        else {
            // No player: either the instance is not ready yet or the model has no clips.
            if idle_gltf.animations.is_empty() {
                commands.entity(e).insert(PokemonAnimState::new(
                    Entity::PLACEHOLDER,
                    AnimationNodeIndex::new(0),
                    None,
                ));
            }
            continue;
        };

        // Double-sided materials (thin tail/ear sheets), as the static path did.
        for d in children.iter_descendants(e) {
            if let Ok(mut m) = mats.get_mut(d) {
                if let Some(orig) = materials.get(&m.0) {
                    let mut c = orig.clone();
                    c.cull_mode = None;
                    c.double_sided = true;
                    m.0 = materials.add(c);
                }
            }
        }

        let idle_clip = find_clip(idle_gltf, &["defaultwait01", "defaultwait"])
            .or_else(|| idle_gltf.animations.first().cloned());
        let walk_clip = walk_gltf.and_then(|g| find_clip(g, &["walk01", "walk"]));
        let Some(idle_clip) = idle_clip else {
            commands.entity(e).insert(PokemonAnimState::new(player_e, AnimationNodeIndex::new(0), None));
            continue;
        };
        let (mut graph, idle_node) = AnimationGraph::from_clip(idle_clip);
        let walk_node = walk_clip.map(|c| graph.add_clip(c, 1.0, graph.root));
        let graph_h = graphs.add(graph);

        let mut transitions = AnimationTransitions::new();
        if let Ok(mut player) = players.get_mut(player_e) {
            transitions.play(&mut player, idle_node, Duration::ZERO).repeat();
        }
        commands.entity(player_e).insert((AnimationGraphHandle(graph_h), transitions));
        let root_bone = children
            .iter_descendants(e)
            .find(|d| names.get(*d).is_ok_and(|(n, _)| n.as_str() == ROOT_MOTION_BONE));
        let mut state = PokemonAnimState::new(player_e, idle_node, walk_node);
        state.root_bone = root_bone;
        state.rest = root_bone.and_then(|b| names.get(b).ok()).map_or(Vec3::ZERO, |(_, t)| t.translation);
        commands.entity(e).insert(state);
    }
}

fn track_motion(time: Res<Time>, mut q: Query<(&GlobalTransform, &mut PokemonMotion)>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (gt, mut m) in &mut q {
        let p = gt.translation();
        if let Some(last) = m.last {
            let d = p - last;
            let planar = Vec2::new(d.x, d.z).length();
            // light smoothing so single-frame jitter does not flip the clip
            m.speed = m.speed * 0.8 + (planar / dt) * 0.2;
        }
        m.last = Some(p);
    }
}

fn switch_clips(
    mut q: Query<(&mut PokemonAnimState, &PokemonMotion)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (mut st, motion) in &mut q {
        let Some(walk) = st.walk else { continue };
        let want_walk = motion.speed > WALK_SPEED_THRESHOLD;
        if want_walk == st.walking {
            continue;
        }
        if let Ok((mut player, mut tr)) = players.get_mut(st.player) {
            let node = if want_walk { walk } else { st.idle };
            tr.play(&mut player, node, Duration::from_millis(200)).repeat();
            st.walking = want_walk;
            st.walk_time = 0.0;
        }
    }
}

/// Matches the walk clip's playback speed to the speed the entity really moves at.
fn scale_walk_speed(
    mut q: Query<(&mut PokemonAnimState, &PokemonMotion)>,
    mut players: Query<&mut AnimationPlayer>,
) {
    for (mut st, motion) in &mut q {
        let Some(walk) = st.walk else { continue };
        let want = if st.walking { walk_playback_speed(motion.speed, st.root_meter.speed()) } else { 1.0 };
        if (want - st.walk_speed).abs() < 0.02 {
            continue;
        }
        if let Ok(mut player) = players.get_mut(st.player) {
            if let Some(active) = player.animation_mut(walk) {
                active.set_speed(want);
                st.walk_speed = want;
            }
        }
    }
}

/// Runs after the animation system wrote the bone transforms and before they propagate:
/// measures the clip's root motion, then removes its horizontal part so the model stays
/// on its entity (the wander system moves the entity; the clip must not slide the model).
fn pin_root_motion(time: Res<Time>, mut q: Query<&mut PokemonAnimState>, mut bones: Query<&mut Transform>) {
    let dt = time.delta_secs();
    for mut st in &mut q {
        let Some(bone) = st.root_bone else { continue };
        let Ok(mut t) = bones.get_mut(bone) else { continue };
        let raw_z = t.translation.z;
        if st.walking {
            st.walk_time += dt;
            if st.walk_time > METER_SETTLE_S {
                if let Some(last) = st.last_raw_z {
                    let clip_dt = dt * st.walk_speed;
                    st.root_meter.observe(raw_z - last, clip_dt);
                }
            }
        }
        st.last_raw_z = Some(raw_z);
        let fixed = strip_root_motion(t.translation, st.rest);
        if fixed != t.translation {
            t.translation = fixed;
        }
    }
}

/// Adds the idle/walk animation systems.
pub struct PokemonAnimPlugin;

impl Plugin for PokemonAnimPlugin {
    fn build(&self, app: &mut App) {
        // glTF scenes are spawned through the reflection registry. Bevy only fills it
        // automatically with the `reflect_auto_register` feature, which `pla` does not
        // enable, so register every derived type here (idempotent) to stay self-sufficient.
        app.world()
            .resource::<AppTypeRegistry>()
            .write()
            .register_derived_types();
        app.add_systems(
            Update,
            (spawn_scene, wire_players, track_motion, switch_clips, scale_walk_speed).chain(),
        );
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

    #[test]
    fn playback_speed_matches_entity_to_clip() {
        // Entity walks 0.9 m/s, clip covers 0.6 m/s: play 1.5x.
        assert!((walk_playback_speed(0.9, Some(0.6)) - 1.5).abs() < 1e-6);
        assert_eq!(walk_playback_speed(0.6, Some(0.6)), 1.0);
    }

    #[test]
    fn playback_speed_is_clamped_and_safe() {
        assert_eq!(walk_playback_speed(5.0, Some(0.6)), WALK_SPEED_SCALE_MAX);
        assert_eq!(walk_playback_speed(0.1, Some(0.6)), WALK_SPEED_SCALE_MIN);
        assert_eq!(walk_playback_speed(1.0, None), 1.0, "unknown clip speed");
        assert_eq!(walk_playback_speed(1.0, Some(0.0)), 1.0, "clip without root motion");
        assert_eq!(walk_playback_speed(f32::NAN, Some(0.6)), 1.0);
        assert_eq!(walk_playback_speed(0.0, Some(0.6)), 1.0);
    }

    #[test]
    fn meter_measures_a_looping_ramp() {
        // Pikachu-like walk: z goes 0 -> 0.6 in 1.02 s, then wraps back to 0.
        let (cycle, dist, fps) = (1.02_f32, 0.6_f32, 60.0_f32);
        let mut m = RootMotionMeter::default();
        assert_eq!(m.speed(), None);
        let (mut z, mut t) = (0.0_f32, 0.0_f32);
        for _ in 0..(fps * 3.0) as usize {
            let dt = 1.0 / fps;
            t += dt;
            let new_z = (t % cycle) / cycle * dist;
            m.observe(new_z - z, dt);
            z = new_z;
        }
        let v = m.speed().unwrap();
        assert!((v - dist / cycle).abs() < 0.02, "got {v}");
    }

    #[test]
    fn meter_ignores_backwards_steps_and_zero_time() {
        let mut m = RootMotionMeter::default();
        m.observe(-0.5, 0.1);
        m.observe(0.1, 0.0);
        assert_eq!(m, RootMotionMeter::default());
    }

    #[test]
    fn pinned_root_keeps_vertical_bob() {
        let rest = Vec3::new(0.0, 0.4, 0.0);
        assert_eq!(strip_root_motion(Vec3::new(0.0, 0.45, 0.6), rest), Vec3::new(0.0, 0.45, 0.0));
    }
}
