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

/// Distance per second above which a creature plays its walk clip.
pub const WALK_SPEED_THRESHOLD: f32 = 0.25;

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
                commands.entity(e).insert(PokemonAnimState {
                    player: Entity::PLACEHOLDER,
                    idle: AnimationNodeIndex::new(0),
                    walk: None,
                    walking: false,
                });
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
            commands.entity(e).insert(PokemonAnimState {
                player: player_e,
                idle: AnimationNodeIndex::new(0),
                walk: None,
                walking: false,
            });
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
        commands.entity(e).insert(PokemonAnimState {
            player: player_e,
            idle: idle_node,
            walk: walk_node,
            walking: false,
        });
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
        app.add_systems(Update, (spawn_scene, wire_players, track_motion, switch_clips).chain());
    }
}
