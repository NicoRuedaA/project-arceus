//! Animation level of detail for people and wild creatures.
//!
//! Bevy evaluates the bones of every animated model on every frame, however far away it is. A
//! village full of villagers plus wild creatures made that the heaviest CPU system (about a
//! quarter of the main thread in a profile of area 0). Far models are now frozen: the graph
//! handle is taken off their `AnimationPlayer`, so `animate_targets` bails out for their bones
//! at once, and it is put back when the player comes close again. A frozen model keeps its last
//! pose; the animation clock does not advance while it is frozen.

use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle};
use bevy::app::AnimationSystems;
use bevy::prelude::*;

use super::npc_model::NpcAnimState;
use super::pokemon_anim::{PokemonAnimState, PokemonBattleSlot};
use super::wild::WildCreature;
use super::PlayerAvatar;

/// Beyond this distance (metres) a model stops animating.
pub const FREEZE_DISTANCE: f32 = 70.0;
/// A frozen model thaws again closer than this (hysteresis: no flicker at the border).
pub const THAW_DISTANCE: f32 = 55.0;

/// The graph handle of a frozen `AnimationPlayer`, kept to give it back on thaw.
#[derive(Component)]
pub struct FrozenAnimation(pub Handle<AnimationGraph>);

pub struct AnimationLodPlugin;

impl Plugin for AnimationLodPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, animation_lod.before(AnimationSystems));
    }
}

/// Freeze or thaw one animation player according to its distance to the player.
fn apply(
    commands: &mut Commands,
    player: Entity,
    dist_sq: f32,
    graphs: &Query<&AnimationGraphHandle>,
    frozen: &Query<&FrozenAnimation>,
) {
    if let Ok(handle) = graphs.get(player) {
        if dist_sq > FREEZE_DISTANCE * FREEZE_DISTANCE {
            commands.entity(player).try_insert(FrozenAnimation(handle.0.clone())).try_remove::<AnimationGraphHandle>();
        }
    } else if let Ok(saved) = frozen.get(player) {
        if dist_sq < THAW_DISTANCE * THAW_DISTANCE {
            commands.entity(player).try_insert(AnimationGraphHandle(saved.0.clone())).try_remove::<FrozenAnimation>();
        }
    }
}

#[allow(clippy::type_complexity)]
fn animation_lod(
    mut commands: Commands,
    avatar: Query<&GlobalTransform, With<PlayerAvatar>>,
    people: Query<(&GlobalTransform, &NpcAnimState)>,
    wild: Query<(&GlobalTransform, &PokemonAnimState), (With<WildCreature>, Without<PokemonBattleSlot>)>,
    graphs: Query<&AnimationGraphHandle>,
    frozen: Query<&FrozenAnimation>,
) {
    let Ok(avatar) = avatar.single() else { return };
    let here = avatar.translation();
    for (tf, state) in &people {
        apply(&mut commands, state.player, tf.translation().distance_squared(here), &graphs, &frozen);
    }
    for (tf, state) in &wild {
        apply(&mut commands, state.player, tf.translation().distance_squared(here), &graphs, &frozen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::animation::AnimationPlayer;

    fn app_with_player_at_origin() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, animation_lod);
        app.world_mut().spawn((PlayerAvatar, GlobalTransform::default()));
        app
    }

    fn spawn_person(app: &mut App, x: f32) -> Entity {
        let player = app
            .world_mut()
            .spawn((AnimationPlayer::default(), AnimationGraphHandle(Handle::default())))
            .id();
        app.world_mut().spawn((
            GlobalTransform::from(Transform::from_xyz(x, 0.0, 0.0)),
            NpcAnimState { player, idle_node: Default::default() },
        ));
        player
    }

    #[test]
    fn a_far_person_freezes_and_thaws_when_the_player_comes_close() {
        let mut app = app_with_player_at_origin();
        let far = spawn_person(&mut app, FREEZE_DISTANCE + 10.0);
        let near = spawn_person(&mut app, 5.0);
        app.update();
        assert!(app.world().get::<AnimationGraphHandle>(far).is_none(), "far person frozen");
        assert!(app.world().get::<FrozenAnimation>(far).is_some());
        assert!(app.world().get::<AnimationGraphHandle>(near).is_some(), "near person keeps animating");

        // Inside the hysteresis band (between thaw and freeze) it stays frozen.
        let mid = (FREEZE_DISTANCE + THAW_DISTANCE) / 2.0;
        let mut q = app.world_mut().query::<(&mut GlobalTransform, &NpcAnimState)>();
        for (mut tf, st) in q.iter_mut(app.world_mut()) {
            if st.player == far {
                *tf = GlobalTransform::from(Transform::from_xyz(mid, 0.0, 0.0));
            }
        }
        app.update();
        assert!(app.world().get::<AnimationGraphHandle>(far).is_none(), "still frozen in the band");

        let mut q = app.world_mut().query::<(&mut GlobalTransform, &NpcAnimState)>();
        for (mut tf, st) in q.iter_mut(app.world_mut()) {
            if st.player == far {
                *tf = GlobalTransform::from(Transform::from_xyz(THAW_DISTANCE - 5.0, 0.0, 0.0));
            }
        }
        app.update();
        assert!(app.world().get::<AnimationGraphHandle>(far).is_some(), "thawed when close");
        assert!(app.world().get::<FrozenAnimation>(far).is_none());
    }
}
