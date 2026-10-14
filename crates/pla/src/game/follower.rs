//! Overworld follower: the lead party Pokémon walks behind the player.
//!
//! All decisions live in plain functions ([`step`], [`FollowerTrail`]) so they are
//! unit-tested deterministically; the Bevy systems at the bottom only feed the player
//! position and the navmesh into them, spawn/despawn the follower entity from
//! [`PlayerProgress::party`] and write the transform back. The walk clip, its playback
//! speed ([`crate::game::pokemon_anim::walk_playback_speed`]) and the idle/walk switch
//! come from `game::pokemon_anim`: the follower is one more creature of that pipeline.
//!
//! Verified by unit tests and one headless Bevy test (a synthetic player walking a
//! circle); never checked visually. The model front axis (+Z, the
//! [`crate::game::wild_wander::yaw_for`] convention) is unverified, exactly as in
//! `wild_wander`.

use std::collections::VecDeque;

use bevy::prelude::*;

use super::navigation::GameNavmeshResource;
use super::save_wire::PlayerProgress;
use super::spawn_point::slide_move;
use super::state::{not_paused, GameState};
use super::wild_wander::{angle_diff, turn_towards, yaw_for};
use super::PlayerAvatar;

/// How far behind the player (measured along its recorded path) the follower aims to stand.
pub const FOLLOW_DISTANCE: f32 = 2.5;
/// Close enough: the follower stops (slack that absorbs the player's per-frame step).
pub const ARRIVE_EPS: f32 = 0.35;
/// Distance from the player beyond which the follower teleports onto its target.
/// A zone change repositions the player far away, so that case is covered too.
pub const TELEPORT_DISTANCE: f32 = 15.0;
/// Speed cap while closing a gap (the player sprints at 8 m/s).
pub const MAX_SPEED: f32 = 12.0;
/// Target speed = distance to the target * this gain, capped by [`MAX_SPEED`].
pub const SPEED_GAIN: f32 = 10.0;
/// Acceleration towards the target speed, in m/s^2.
pub const ACCEL: f32 = 16.0;
/// Yaw turn rate in radians per second (same rule as `wild_wander`).
pub const TURN_RATE: f32 = 6.0;
/// The follower only advances once facing its target within this angle.
pub const MOVE_ANGLE: f32 = 1.0;
/// Model root height above the navmesh surface (same offset the wild creatures use).
pub const GROUND_OFFSET: f32 = 0.45;
/// Trail steps shorter than this are not recorded (the player is standing still).
const TRAIL_MIN_STEP: f32 = 0.02;
/// Upper bound of recorded trail positions (bounded memory).
const TRAIL_MAX_POINTS: usize = 512;

/// Queue of recent player positions: the follower's path memory.
#[derive(Debug, Clone, Default)]
pub struct FollowerTrail {
    points: VecDeque<Vec2>,
}

impl FollowerTrail {
    /// Record `pos`, dropping steps shorter than [`TRAIL_MIN_STEP`] and the oldest points.
    pub fn push(&mut self, pos: Vec2) {
        if self
            .points
            .back()
            .is_some_and(|last| last.distance(pos) < TRAIL_MIN_STEP)
        {
            return;
        }
        if self.points.len() >= TRAIL_MAX_POINTS {
            self.points.pop_front();
        }
        self.points.push_back(pos);
    }

    /// Point `distance` metres back from the newest position, measured along the recorded
    /// path (linear interpolation inside a segment). `None` when the trail is empty; when
    /// the whole trail is shorter than `distance`, the oldest recorded point.
    pub fn point_behind(&self, distance: f32) -> Option<Vec2> {
        let mut iter = self.points.iter().rev();
        let mut cur = *iter.next()?;
        let mut acc = 0.0;
        for older in iter {
            let seg = cur.distance(*older);
            if acc + seg >= distance {
                let t = if seg > 0.0 {
                    ((distance - acc) / seg).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                return Some(cur.lerp(*older, t));
            }
            acc += seg;
            cur = *older;
        }
        Some(cur)
    }

    /// Replace the trail with `points` (spawn seeding: the behind point, then the player).
    pub fn seed(&mut self, points: impl IntoIterator<Item = Vec2>) {
        self.points.clear();
        for p in points {
            self.push(p);
        }
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

/// Per-follower mutable state.
#[derive(Component, Debug, Clone)]
pub struct FollowerState {
    /// Planar position (x, z) in the terrain frame.
    pub pos: (f32, f32),
    /// Yaw about +Y; 0 faces +Z (glTF front), see [`yaw_for`].
    pub yaw: f32,
    /// Current planar speed in metres per second.
    pub speed: f32,
    /// Recorded player path; the target lives on it.
    pub trail: FollowerTrail,
    /// Planar distance walked since spawn (diagnostics / tests).
    pub walked: f32,
    /// How many times the follower had to teleport (diagnostics / tests).
    pub teleports: u32,
}

impl FollowerState {
    pub fn new(pos: (f32, f32), yaw: f32) -> Self {
        Self {
            pos,
            yaw,
            speed: 0.0,
            trail: FollowerTrail::default(),
            walked: 0.0,
            teleports: 0,
        }
    }
}

/// The lead party Pokémon rendered as the overworld follower.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Follower {
    pub species: u16,
    pub form: u8,
}

/// Advances the follower by `dt` seconds towards the point [`FOLLOW_DISTANCE`] behind
/// `player` along the recorded path. `walkable` is the navmesh query; a caller without a
/// navmesh (flat fallback world) passes a predicate that is always true.
pub fn step(st: &mut FollowerState, player: Vec2, dt: f32, walkable: &dyn Fn(f32, f32) -> bool) {
    if dt <= 0.0 {
        return;
    }
    st.trail.push(player);
    let target = st.trail.point_behind(FOLLOW_DISTANCE).unwrap_or(player);
    let here = Vec2::new(st.pos.0, st.pos.1);

    // Left behind (or the zone changed and the player was repositioned): teleport onto the
    // target, or onto the player when only that spot is walkable.
    if here.distance(player) > TELEPORT_DISTANCE {
        let spot = if walkable(target.x, target.y) {
            target
        } else if walkable(player.x, player.y) {
            player
        } else {
            target
        };
        st.pos = (spot.x, spot.y);
        st.speed = 0.0;
        st.teleports += 1;
        return;
    }

    let to = target - here;
    let d = to.length();
    let desired = if d <= ARRIVE_EPS {
        0.0
    } else {
        (d * SPEED_GAIN).min(MAX_SPEED)
    };
    let dv = ACCEL * dt;
    st.speed = if desired > st.speed {
        (st.speed + dv).min(desired)
    } else {
        (st.speed - dv).max(desired)
    };
    if st.speed <= 0.0 || d <= 1e-6 {
        return;
    }

    // Face the target; advance only once roughly facing it (same rule as wild_wander).
    let heading = yaw_for(to.x, to.y);
    st.yaw = turn_towards(st.yaw, heading, TURN_RATE, dt);
    if angle_diff(st.yaw, heading).abs() > MOVE_ANGLE {
        return;
    }

    let advance = (st.speed * dt).min(d);
    let delta = to / d * advance;
    let moved = if walkable(here.x, here.y) {
        slide_move(here, delta, |p| walkable(p.x, p.y))
    } else {
        // Started off the mesh (e.g. spawned behind a wall): head straight for the target,
        // which lies on the player's walkable path.
        here + delta
    };
    st.walked += here.distance(moved);
    st.pos = (moved.x, moved.y);
}

/// Asset-server path of the exported batch model for `species`/`form`
/// (`cache/pokemon_batch/<pack>/<pack>_idle.gltf`, pack from the game's model catalog, see
/// `game::pokemon_resource`), or `None` when it was not exported. Prefers the member's form
/// and falls back to form 0.
fn resolve_batch_idle_rel(species: u16, form: u8) -> Option<String> {
    use crate::game::pokemon_resource::{batch_gltf_rel_for_stem, global_table};
    let table = global_table();
    let mut stems = Vec::new();
    for f in [u16::from(form), 0] {
        if let Some(stem) = table.pack_stem(species, f) {
            if !stems.contains(&stem) {
                stems.push(stem);
            }
        }
    }
    stems
        .iter()
        .find_map(|stem| batch_gltf_rel_for_stem(stem, &["idle"]))
}

/// Spawns / respawns / despawns the follower entity so it shows the lead party member.
fn sync_follower(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    progress: Option<Res<PlayerProgress>>,
    player: Query<&Transform, With<PlayerAvatar>>,
    nav: Option<Res<GameNavmeshResource>>,
    existing: Query<(Entity, &Follower)>,
    mut warned: Local<Option<u16>>,
) {
    let Some(lead) = progress.as_ref().and_then(|p| p.party.first()) else {
        for (e, _) in &existing {
            commands.entity(e).despawn();
        }
        return;
    };
    if existing
        .iter()
        .any(|(_, f)| f.species == lead.species && f.form == lead.form)
    {
        return;
    }
    for (e, _) in &existing {
        commands.entity(e).despawn();
    }
    let Some(rel) = resolve_batch_idle_rel(lead.species, lead.form) else {
        if *warned != Some(lead.species) {
            eprintln!(
                "Follower: no exported model for #{:04} form {}; nobody follows",
                lead.species, lead.form
            );
            *warned = Some(lead.species);
        }
        return;
    };
    let Ok(player_tf) = player.single() else {
        return;
    };
    let front = player_tf.rotation * Vec3::Z;
    let fwd = Vec2::new(front.x, front.z)
        .try_normalize()
        .unwrap_or(Vec2::Y);
    let p = Vec2::new(player_tf.translation.x, player_tf.translation.z);
    let behind = p - fwd * FOLLOW_DISTANCE;
    let y = nav
        .as_ref()
        .and_then(|n| n.height_at(behind.x, behind.y))
        .map_or(player_tf.translation.y, |h| h + GROUND_OFFSET);
    let yaw = yaw_for(fwd.x, fwd.y);
    let mut state = FollowerState::new((behind.x, behind.y), yaw);
    state.trail.seed([behind, p]);
    commands.spawn((
        Follower {
            species: lead.species,
            form: lead.form,
        },
        state,
        crate::game::pokemon_anim::PokemonAnimSource::from_idle_path(&asset_server, &rel),
        Transform::from_xyz(behind.x, y, behind.y).with_rotation(Quat::from_rotation_y(yaw)),
        Visibility::default(),
    ));
}

/// Moves the follower along the player's trail and grounds it on the navmesh.
fn drive_follower(
    time: Res<Time>,
    nav: Option<Res<GameNavmeshResource>>,
    player: Query<&Transform, (With<PlayerAvatar>, Without<Follower>)>,
    mut followers: Query<(&mut Transform, &mut FollowerState), Without<PlayerAvatar>>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let dt = time.delta_secs().min(0.1);
    // Without a navmesh (flat fallback world) every spot is walkable and no height is known.
    let walkable = |x: f32, z: f32| nav.as_ref().is_none_or(|n| n.is_walkable(x, z));
    let player = Vec2::new(player_tf.translation.x, player_tf.translation.z);
    for (mut tf, mut st) in &mut followers {
        step(&mut st, player, dt, &walkable);
        tf.translation.x = st.pos.0;
        tf.translation.z = st.pos.1;
        if let Some(h) = nav.as_ref().and_then(|n| n.height_at(st.pos.0, st.pos.1)) {
            tf.translation.y = h + GROUND_OFFSET;
        }
        tf.rotation = Quat::from_rotation_y(st.yaw);
    }
}

/// Hides the follower while a battle runs (the player avatar is hidden there too).
fn hide_follower_in_battle(mut q: Query<&mut Visibility, With<Follower>>) {
    for mut vis in &mut q {
        *vis = Visibility::Hidden;
    }
}

/// Shows it again when the battle ends.
fn show_follower_after_battle(mut q: Query<&mut Visibility, With<Follower>>) {
    for mut vis in &mut q {
        *vis = Visibility::Inherited;
    }
}

/// Registers the follower systems. Add with `app.add_plugins(FollowerPlugin)`.
pub struct FollowerPlugin;

impl Plugin for FollowerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (sync_follower, drive_follower.run_if(not_paused))
                .chain()
                .run_if(in_state(GameState::World)),
        );
        app.add_systems(OnEnter(GameState::Battle), hide_follower_in_battle);
        app.add_systems(OnExit(GameState::Battle), show_follower_after_battle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::navmesh::{AreaNavigation, NavEdge, NavFace, NavMesh};
    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;
    use std::sync::Arc;
    use std::time::Duration;

    const DT: f32 = 1.0 / 60.0;
    const WALK_SPEED: f32 = 4.5;

    fn all(_: f32, _: f32) -> bool {
        true
    }

    fn dist(st: &FollowerState, p: Vec2) -> f32 {
        Vec2::new(st.pos.0, st.pos.1).distance(p)
    }

    /// Flat quad navmesh spanning `half_x` x `half_z` around the origin.
    fn quad(half_x: f32, half_z: f32) -> NavMesh {
        let v = vec![
            [-half_x, 0.0, -half_z],
            [half_x, 0.0, -half_z],
            [half_x, 0.0, half_z],
            [-half_x, 0.0, half_z],
        ];
        let e = |a, b| NavEdge {
            a,
            b,
            opposite_edge: u32::MAX,
            opposite_face: u32::MAX,
            flags: 0,
        };
        NavMesh::from_parts(
            v,
            vec![NavFace {
                start_edge: 0,
                num_edges: 4,
            }],
            vec![e(0, 1), e(1, 2), e(2, 3), e(3, 0)],
        )
        .expect("test navmesh")
    }

    // ---------------------------------------------------------------- trail

    #[test]
    fn trail_point_behind_walks_the_recorded_path() {
        let mut t = FollowerTrail::default();
        assert_eq!(t.point_behind(2.5), None, "empty trail");
        // L-shaped path: 10 m along +x, then 10 m along +z.
        t.seed([Vec2::ZERO]);
        for i in 1..=10 {
            t.push(Vec2::new(i as f32, 0.0));
        }
        for i in 1..=10 {
            t.push(Vec2::new(10.0, i as f32));
        }
        // 2.5 m back from (10, 10) along the path: (10, 7.5), not the straight-line point.
        let p = t.point_behind(2.5).unwrap();
        assert!((p - Vec2::new(10.0, 7.5)).length() < 1e-4, "{p:?}");
        // 12.5 m back: 2.5 m before the corner, on the x leg.
        let p = t.point_behind(12.5).unwrap();
        assert!((p - Vec2::new(7.5, 0.0)).length() < 1e-4, "{p:?}");
        // Longest possible: the oldest point.
        let p = t.point_behind(1000.0).unwrap();
        assert!((p - Vec2::ZERO).length() < 1e-4, "{p:?}");
    }

    #[test]
    fn trail_ignores_standing_still_and_drops_old_points() {
        let mut t = FollowerTrail::default();
        t.push(Vec2::ZERO);
        for _ in 0..60 {
            t.push(Vec2::new(0.001, 0.0)); // jitter below TRAIL_MIN_STEP
        }
        assert_eq!(t.len(), 1);
        for i in 1..=TRAIL_MAX_POINTS as i32 * 2 {
            t.push(Vec2::new(i as f32, 0.0));
        }
        assert_eq!(t.len(), TRAIL_MAX_POINTS);
        assert!(
            t.point_behind(FOLLOW_DISTANCE).unwrap().x > 0.0,
            "bounded memory keeps the newest path"
        );
        assert!(!t.is_empty());
    }

    // ---------------------------------------------------------------- following

    #[test]
    fn follower_converges_to_the_follow_distance() {
        let mut st = FollowerState::new((-FOLLOW_DISTANCE, 0.0), std::f32::consts::FRAC_PI_2);
        st.trail
            .seed([Vec2::new(-FOLLOW_DISTANCE, 0.0), Vec2::ZERO]);
        let mut player = Vec2::ZERO;
        let mut dists = Vec::new();
        for i in 0..1200 {
            player.x += WALK_SPEED * DT;
            step(&mut st, player, DT, &all);
            if i > 300 {
                dists.push(dist(&st, player));
            }
        }
        let mean = dists.iter().sum::<f32>() / dists.len() as f32;
        let max = dists.iter().copied().fold(0.0f32, f32::max);
        eprintln!(
            "straight walk: mean {mean:.3} m, max {max:.3} m over {} steps",
            dists.len()
        );
        assert!((2.3..=3.4).contains(&mean), "mean follow distance {mean}");
        assert!(max < 4.0, "max follow distance {max}");
        assert!(st.walked > 40.0, "must really walk ({} m)", st.walked);
        assert_eq!(st.teleports, 0);
    }

    #[test]
    fn follower_stops_once_it_reaches_the_target() {
        let mut st = FollowerState::new((-6.0, 0.0), std::f32::consts::FRAC_PI_2);
        st.trail
            .seed([Vec2::new(-FOLLOW_DISTANCE, 0.0), Vec2::ZERO]);
        for _ in 0..600 {
            step(&mut st, Vec2::ZERO, DT, &all);
        }
        let d = dist(&st, Vec2::ZERO);
        eprintln!("standing player: {d:.3} m away");
        assert!(
            (FOLLOW_DISTANCE..=FOLLOW_DISTANCE + ARRIVE_EPS + 0.05).contains(&d),
            "{d}"
        );
        assert!(st.speed < 1e-3, "must be stopped, speed {}", st.speed);
        assert_eq!(st.teleports, 0);
    }

    #[test]
    fn teleports_when_left_far_behind_and_only_once() {
        let mut st = FollowerState::new((40.0, 0.0), 0.0);
        st.trail
            .seed([Vec2::new(-FOLLOW_DISTANCE, 0.0), Vec2::ZERO]);
        step(&mut st, Vec2::ZERO, DT, &all);
        assert_eq!(st.teleports, 1);
        assert!(
            dist(&st, Vec2::ZERO) <= FOLLOW_DISTANCE + 1e-3,
            "{:?}",
            st.pos
        );
        for _ in 0..120 {
            step(&mut st, Vec2::ZERO, DT, &all);
        }
        assert_eq!(st.teleports, 1, "no repeated teleport while close");
        assert!(dist(&st, Vec2::ZERO) <= FOLLOW_DISTANCE + ARRIVE_EPS + 0.05);
    }

    #[test]
    fn never_steps_off_the_test_navmesh() {
        let nav = quad(120.0, 3.0); // narrow corridor, player path stays inside
        let walk = |x: f32, z: f32| nav.is_walkable(x, z);
        assert!(walk(-2.5, 0.0) && walk(0.0, 0.0));
        let mut st = FollowerState::new((-FOLLOW_DISTANCE, 0.0), std::f32::consts::FRAC_PI_2);
        st.trail
            .seed([Vec2::new(-FOLLOW_DISTANCE, 0.0), Vec2::ZERO]);
        for i in 0..1200 {
            let t = i as f32 * DT;
            let player = Vec2::new(t * WALK_SPEED, 2.5 * (t * 0.7).sin());
            assert!(
                walk(player.x, player.y),
                "player left the corridor at {player:?}"
            );
            step(&mut st, player, DT, &walk);
            assert!(
                walk(st.pos.0, st.pos.1),
                "follower left the navmesh at {:?}",
                st.pos
            );
        }
        assert!(st.walked > 40.0);
        assert_eq!(st.teleports, 0);
    }

    // ---------------------------------------------------------------- headless Bevy

    #[test]
    fn headless_bevy_player_circle_keeps_the_follower_close() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            DT,
        )));
        app.init_state::<GameState>();
        app.insert_resource(GameNavmeshResource {
            nav: Arc::new(AreaNavigation {
                area: 0,
                meshes: vec![quad(60.0, 60.0)],
            }),
        });
        let player = app
            .world_mut()
            .spawn((PlayerAvatar, Transform::from_xyz(0.0, 0.0, 0.0)))
            .id();
        let mut state = FollowerState::new((-FOLLOW_DISTANCE, 0.0), std::f32::consts::FRAC_PI_2);
        state
            .trail
            .seed([Vec2::new(-FOLLOW_DISTANCE, 0.0), Vec2::ZERO]);
        let follower = app
            .world_mut()
            .spawn((
                Follower {
                    species: 25,
                    form: 0,
                },
                state,
                Transform::from_xyz(-FOLLOW_DISTANCE, 0.0, 0.0),
            ))
            .id();
        app.add_systems(Update, drive_follower);

        // Player on a 6 m circle at 4.5 m/s (0.75 rad/s), 2400 ticks = 40 s.
        let mut dists = Vec::new();
        let mut max_y = 0.0f32;
        for i in 0..2400 {
            let t = i as f32 * DT;
            let (x, z) = (6.0 * (t * 0.75).cos(), 6.0 * (t * 0.75).sin());
            app.world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation = Vec3::new(x, 0.0, z);
            app.update();
            if i > 120 {
                let ftf = app.world().get::<Transform>(follower).unwrap();
                let ptf = app.world().get::<Transform>(player).unwrap();
                dists.push(
                    Vec3::new(ftf.translation.x, 0.0, ftf.translation.z).distance(Vec3::new(
                        ptf.translation.x,
                        0.0,
                        ptf.translation.z,
                    )),
                );
                max_y = max_y.max(ftf.translation.y.abs());
            }
        }
        let mean = dists.iter().sum::<f32>() / dists.len() as f32;
        let max = dists.iter().copied().fold(0.0f32, f32::max);
        let st = app.world().get::<FollowerState>(follower).unwrap();
        eprintln!(
            "circle: mean {mean:.3} m, max {max:.3} m, walked {:.1} m, teleports {}",
            st.walked, st.teleports
        );
        assert!((2.0..=3.5).contains(&mean), "mean {mean}");
        assert!(max < 4.5, "max {max}");
        assert!(st.walked > 150.0, "followed the circle ({} m)", st.walked);
        assert_eq!(st.teleports, 0);
        assert!(
            (max_y - GROUND_OFFSET).abs() < 1e-5,
            "grounded on the flat navmesh (y {max_y})"
        );
    }

    #[test]
    fn battle_hides_the_follower_and_world_restores_it() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.add_systems(OnEnter(GameState::Battle), hide_follower_in_battle);
        app.add_systems(OnExit(GameState::Battle), show_follower_after_battle);
        let follower = app
            .world_mut()
            .spawn((
                Follower {
                    species: 25,
                    form: 0,
                },
                Visibility::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Battle);
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(follower).unwrap(),
            Visibility::Hidden
        );
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::World);
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(follower).unwrap(),
            Visibility::Inherited
        );
    }
}
