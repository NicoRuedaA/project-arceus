//! Shadow level of detail: small, far meshes stop casting shadows.
//!
//! Every shadow cascade draws every caster inside its volume, so with the 4 cascades and 400 m of
//! the ultra shadow quality a village or field of small objects (props, trees, rocks) is drawn
//! several times per frame. A prop that is far from the player and small casts a shadow of a few
//! pixels at most, so it is taken out of the shadow passes (`NotShadowCaster`) and put back when
//! the player gets close again. Big meshes (terrain chunks, buildings, cliffs) always keep
//! casting: their long shadows are visible from far away. Skinned meshes (people, creatures) are
//! left alone. Only meshes this module marked are ever unmarked.

use bevy::camera::primitives::Aabb;
use bevy::light::NotShadowCaster;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;

use super::PlayerAvatar;

/// A mesh stops casting once it is farther than its radius times this (it then spans under about
/// two degrees of the view, a handful of shadow pixels).
pub const DISTANCE_PER_RADIUS: f32 = 30.0;
/// No mesh is culled closer than this (metres), however small.
pub const MIN_CULL_DISTANCE: f32 = 30.0;
/// A culled mesh casts again at this fraction of its culling distance (hysteresis against flicker).
pub const RESTORE_FRACTION: f32 = 0.85;
/// Meshes whose bounding radius (metres) is at least this keep their shadow at any distance.
pub const BIG_RADIUS: f32 = 8.0;

/// Marks a mesh that this module took out of the shadow passes.
#[derive(Component)]
pub struct ShadowLodCulled;

pub struct ShadowLodPlugin;

impl Plugin for ShadowLodPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, shadow_lod.before(bevy::camera::visibility::VisibilitySystems::CheckVisibility));
    }
}

/// Bounding radius in world units of a mesh (largest scale axis times the box half diagonal).
fn world_radius(tf: &GlobalTransform, aabb: &Aabb) -> f32 {
    let (scale, _, _) = tf.to_scale_rotation_translation();
    Vec3::from(aabb.half_extents).length() * scale.max_element()
}

#[allow(clippy::type_complexity)]
fn shadow_lod(
    mut commands: Commands,
    avatar: Query<&GlobalTransform, With<PlayerAvatar>>,
    meshes: Query<
        (Entity, &GlobalTransform, &Aabb, Has<NotShadowCaster>, Has<ShadowLodCulled>),
        (With<Mesh3d>, Without<SkinnedMesh>),
    >,
) {
    let Ok(avatar) = avatar.single() else { return };
    let here = avatar.translation();
    for (entity, tf, aabb, no_shadow, culled) in &meshes {
        let dist_sq = tf.translation().distance_squared(here);
        let radius = world_radius(tf, aabb);
        let cull = (radius * DISTANCE_PER_RADIUS).max(MIN_CULL_DISTANCE);
        if culled {
            let restore = cull * RESTORE_FRACTION;
            if dist_sq < restore * restore {
                commands.entity(entity).try_remove::<(NotShadowCaster, ShadowLodCulled)>();
            }
        } else if !no_shadow
            && dist_sq > cull * cull
            && radius < BIG_RADIUS
        {
            commands.entity(entity).try_insert((NotShadowCaster, ShadowLodCulled));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, shadow_lod);
        app.world_mut().spawn((PlayerAvatar, GlobalTransform::default()));
        app
    }

    fn mesh_at(app: &mut App, x: f32, half: f32) -> Entity {
        app.world_mut()
            .spawn((
                Mesh3d(Handle::default()),
                GlobalTransform::from(Transform::from_xyz(x, 0.0, 0.0)),
                Aabb::from_min_max(Vec3::splat(-half), Vec3::splat(half)),
            ))
            .id()
    }

    #[test]
    fn small_far_meshes_stop_casting_and_come_back_when_close() {
        let mut app = app();
        let far_small = mesh_at(&mut app, 100.0, 1.0);
        let far_big = mesh_at(&mut app, 100.0, BIG_RADIUS);
        let near_small = mesh_at(&mut app, 10.0, 1.0);
        // A mesh somebody else excluded from shadows is never touched by this module.
        let foreign = app.world_mut().spawn((
            Mesh3d(Handle::default()),
            GlobalTransform::from(Transform::from_xyz(100.0, 0.0, 0.0)),
            Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
            NotShadowCaster,
        )).id();
        app.update();
        assert!(app.world().get::<NotShadowCaster>(far_small).is_some(), "far small culled");
        assert!(app.world().get::<NotShadowCaster>(far_big).is_none(), "big meshes keep casting");
        assert!(app.world().get::<NotShadowCaster>(near_small).is_none(), "near meshes keep casting");
        assert!(app.world().get::<ShadowLodCulled>(foreign).is_none(), "foreign exclusions are not claimed");

        // Inside the hysteresis band it stays culled; closer than the restore distance it casts again.
        app.world_mut().get_mut::<GlobalTransform>(far_small).map(|mut t| *t = Transform::from_xyz(50.0, 0.0, 0.0).into());
        app.update();
        assert!(app.world().get::<NotShadowCaster>(far_small).is_some(), "still culled in the band");
        app.world_mut().get_mut::<GlobalTransform>(far_small).map(|mut t| *t = Transform::from_xyz(20.0, 0.0, 0.0).into());
        app.update();
        assert!(app.world().get::<NotShadowCaster>(far_small).is_none(), "casts again when close");
        assert!(app.world().get::<NotShadowCaster>(foreign).is_some(), "foreign exclusion survives");
    }
}
