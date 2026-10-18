//! Overworld follower: the lead party Pokémon walks behind the player.
//!
//! All decisions live in plain functions ([`step`], [`FollowerTrail`]) so they are
//! unit-tested deterministically; the Bevy systems at the bottom only feed the player
//! position and the navmesh into them, spawn/despawn the follower entity from
//! [`PlayerProgress::party`] and write the transform back. The walk clip, its playback
//! speed ([`crate::game::pokemon_anim::walk_playback_speed`]) and the idle/walk switch
//! come from `game::pokemon_anim`: the follower is one more creature of that pipeline.
//!
//! Separation (play-test 2026-10-15: "the follower walks through the player"):
//! - While moving, a step that would enter the player's disc slides along the separation
//!   circle by the same arc length (see [`slide_around_player`]): the follower steps aside
//!   smoothly instead of being pushed, so the player can never drag it.
//! - When the player stands still for [`STOP_REPOSITION_DELAY`] and the follower is in
//!   front of them, the aim point moves to the spot behind the player's facing: the
//!   follower walks around and returns to its place.
//! - [`MIN_SEPARATION`] remains the hard floor (the safety net when the circle is off the
//!   mesh), and the drive system runs after the frame's locomotion so the floor also holds
//!   at the frame boundary.
//!
//! Verified by unit tests and one headless Bevy test (a synthetic player walking a
//! circle); never checked visually. The model front axis (+Z, the
//! [`crate::game::wild_wander::yaw_for`] convention) is unverified, exactly as in
//! `wild_wander`.

use std::collections::VecDeque;

use bevy::prelude::*;

use super::terrain_collision::{ground_y, WalkContext};
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
/// The follower never stands closer than this to the player (planar, centre to centre). A
/// hard floor so the two models never overlap, whatever the player does (reverses, turns).
pub const MIN_SEPARATION: f32 = 1.4;
/// The aim point is kept at least this far from the player (so the follower stops at a
/// comfortable 1.5-2 m instead of pressing against the floor above).
pub const MIN_TARGET_DISTANCE: f32 = 1.8;
/// When the player has been standing still this long (seconds), a follower left in front of
/// them walks back to the spot behind the player. SUPUESTO: the game gives no decoded value
/// for this delay; 0.5 s avoids reacting to a one-frame stop.
pub const STOP_REPOSITION_DELAY: f32 = 0.5;
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
    /// Last player position seen by [`step`] (still-time detection).
    last_player: Option<Vec2>,
    /// Seconds the player has been standing still (within [`TRAIL_MIN_STEP`]).
    pub still: f32,
    /// While the player stands still: the follower is walking around to the spot behind them
    /// (set when the aim point is on the front side; cleared when the player moves).
    pub repositioning: bool,
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
            last_player: None,
            still: 0.0,
            repositioning: false,
        }
    }
}

/// The lead party Pokémon rendered as the overworld follower.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Follower {
    pub species: u16,
    pub form: u8,
}

/// Pushes `p` out of the disc of radius `radius` around `player`, along the line from the
/// player; `fallback` is the direction used when `p` sits exactly on the player.
fn push_out(p: Vec2, player: Vec2, radius: f32, fallback: Vec2) -> Vec2 {
    let off = p - player;
    if off.length() >= radius {
        return p;
    }
    let dir = off.try_normalize().or_else(|| fallback.try_normalize()).unwrap_or(Vec2::NEG_Y);
    player + dir * radius
}

/// Advances the follower by `dt` seconds towards the point [`FOLLOW_DISTANCE`] behind
/// `player` along the recorded path, never closer than [`MIN_SEPARATION`] to the player.
/// `facing` is the planar direction the player's model faces (used when the player stands
/// still, see [`STOP_REPOSITION_DELAY`]). `walkable` is the navmesh query; a caller without a
/// navmesh (flat fallback world) passes a predicate that is always true.
pub fn step(
    st: &mut FollowerState,
    player: Vec2,
    facing: Vec2,
    dt: f32,
    walkable: &dyn Fn(f32, f32) -> bool,
) {
    if dt <= 0.0 {
        return;
    }
    st.still = match st.last_player {
        Some(last) if last.distance(player) < TRAIL_MIN_STEP => st.still + dt,
        _ => 0.0,
    };
    st.last_player = Some(player);
    advance(st, player, facing, dt, walkable);
    // Hard floor, applied after every outcome (also when the follower was still turning or
    // stopped): when the player walks into it, it is pushed out, never overlapped.
    let here = Vec2::new(st.pos.0, st.pos.1);
    let trail_back = st.trail.point_behind(FOLLOW_DISTANCE).map_or(Vec2::NEG_Y, |t| t - player);
    let fixed = push_out(here, player, MIN_SEPARATION, trail_back);
    if fixed == here {
        return;
    }
    if walkable(fixed.x, fixed.y) || !walkable(here.x, here.y) {
        st.pos = (fixed.x, fixed.y);
        return;
    }
    // The radial spot is off the navmesh (cliff edge, wall): slide round the player to the
    // nearest walkable spot on the separation circle instead of staying inside the player.
    let base = fixed - player;
    for deg in [30.0_f32, -30.0, 60.0, -60.0, 90.0, -90.0, 120.0, -120.0, 150.0, -150.0, 180.0] {
        let spot = player + Vec2::from_angle(deg.to_radians()).rotate(base);
        if walkable(spot.x, spot.y) {
            st.pos = (spot.x, spot.y);
            return;
        }
    }
}

fn advance(
    st: &mut FollowerState,
    player: Vec2,
    facing: Vec2,
    dt: f32,
    walkable: &dyn Fn(f32, f32) -> bool,
) {
    st.trail.push(player);
    let here = Vec2::new(st.pos.0, st.pos.1);
    let mut target = st.trail.point_behind(FOLLOW_DISTANCE).unwrap_or(player);
    // The recorded path doubles back through the player after a U-turn: keep the aim point
    // outside the comfort radius, on the side where the follower already is.
    if target.distance(player) < MIN_TARGET_DISTANCE {
        target = push_out(target, player, MIN_TARGET_DISTANCE, here - player);
    }
    // Standing player: the recorded path can leave the aim point in front of them (they
    // walked backwards). A follower left on that side walks around to the spot behind the
    // player; the latch keeps the aim there until the player moves again, so the arc cannot
    // flip half-way (see the play-test fix 2026-10-15).
    if st.still < STOP_REPOSITION_DELAY {
        st.repositioning = false;
    } else {
        let facing = facing.try_normalize().unwrap_or(Vec2::Y);
        let behind = player - facing * FOLLOW_DISTANCE;
        if !st.repositioning && (target - player).dot(facing) > 0.0 && walkable(behind.x, behind.y) {
            st.repositioning = true;
        }
        if st.repositioning {
            target = behind;
        }
    }

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

    // Face the target; advance only once roughly facing it (same rule as wild_wander) --
    // except when the player blocks the way: the step aside is what keeps the models apart,
    // so it runs even while the body is still turning (its arc rate is capped below so the
    // turn can keep up).
    let heading = yaw_for(to.x, to.y);
    st.yaw = turn_towards(st.yaw, heading, TURN_RATE, dt);

    let advance = (st.speed * dt).min(d);
    let delta = to / d * advance;
    let direct = if walkable(here.x, here.y) {
        slide_move(here, delta, |p| walkable(p.x, p.y))
    } else {
        // Started off the mesh (e.g. spawned behind a wall): head straight for the target,
        // which lies on the player's walkable path.
        here + delta
    };
    let blocked = direct.distance(player) < MIN_SEPARATION;
    if !blocked && angle_diff(st.yaw, heading).abs() > MOVE_ANGLE {
        return;
    }
    let moved = slide_around_player(here, direct, player, to, facing, dt, walkable);
    st.walked += here.distance(moved);
    st.pos = (moved.x, moved.y);
}

/// Keeps the moving follower out of the player's disc: when the direct move would end inside
/// [`MIN_SEPARATION`], it slides along the separation circle by the same arc length, towards
/// the side that keeps closing on the aim direction `to` instead of crossing the player (a
/// smooth step aside, no teleport). Falls back to the direct move when both candidates are off
/// the navmesh; the hard floor in [`step`] is then the safety net.
fn slide_around_player(
    here: Vec2,
    moved: Vec2,
    player: Vec2,
    to: Vec2,
    facing: Vec2,
    dt: f32,
    walkable: &dyn Fn(f32, f32) -> bool,
) -> Vec2 {
    let off = moved - player;
    if off.length() >= MIN_SEPARATION {
        return moved;
    }
    let radial = here - player;
    let radial = if radial.length() > 1e-3 {
        radial.normalize()
    } else {
        (-to).try_normalize().or_else(|| (-facing).try_normalize()).unwrap_or(Vec2::NEG_Y)
    };
    // The arc never turns faster than the body: the yaw-towards-target can keep up, so the
    // movement gate above never throttles the step aside into a stop-and-go.
    let step_ang = (here.distance(moved) / MIN_SEPARATION).min(TURN_RATE * dt);
    let rotate = |v: Vec2, a: f32| Vec2::new(v.x * a.cos() - v.y * a.sin(), v.x * a.sin() + v.y * a.cos());
    let mut best: Option<(f32, Vec2)> = None;
    for sign in [1.0_f32, -1.0] {
        let cand = player + rotate(radial, sign * step_ang) * MIN_SEPARATION;
        if !walkable(cand.x, cand.y) {
            continue;
        }
        let score = (cand - here).dot(to);
        if best.is_none_or(|(b, _)| score > b) {
            best = Some((score, cand));
        }
    }
    best.map_or(moved, |(_, cand)| cand)
}

/// Model of `species`/`form` for the follower: the game's own catalog first, the converted
/// national models second (`game::pokemon_resource::resolve_model`); `None` = nobody follows.
/// Prefers the member's form and falls back to form 0.
fn resolve_batch_idle_rel(species: u16, form: u8) -> Option<crate::game::pokemon_resource::ResolvedModel> {
    let mut resolved = crate::game::pokemon_resource::resolve_model(species, form);
    if resolved.is_none() && form != 0 {
        resolved = crate::game::pokemon_resource::resolve_model(species, 0);
    }
    resolved
}

/// Spawns / respawns / despawns the follower entity so it shows the lead party member.
fn sync_follower(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    progress: Option<Res<PlayerProgress>>,
    player: Query<&Transform, With<PlayerAvatar>>,
    terrain: Option<Res<super::zone::TerrainResource>>,
    collision: Option<Res<super::world_collision::WorldCollision>>,
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
    let Some(model) = resolve_batch_idle_rel(lead.species, lead.form) else {
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
    let y = ground_y(
        terrain.as_deref(),
        collision.as_deref(),
        behind.x,
        behind.y,
        player_tf.translation.y,
    )
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
        crate::game::pokemon_anim::PokemonAnimSource::from_resolved(&asset_server, &model)
            .with_root_drop(GROUND_OFFSET),
        Transform::from_xyz(behind.x, y, behind.y).with_rotation(Quat::from_rotation_y(yaw)),
        Visibility::default(),
    ));
}

/// Moves the follower along the player's trail and grounds it on the terrain (or an interior's
/// collider floor); the walk rule is `game::terrain_collision` (no navmesh, user decision
/// 2026-10-17).
fn drive_follower(
    time: Res<Time>,
    terrain: Option<Res<super::zone::TerrainResource>>,
    collision: Option<Res<super::world_collision::WorldCollision>>,
    water: Option<Res<super::water::WaterBodies>>,
    player: Query<&Transform, (With<PlayerAvatar>, Without<Follower>)>,
    mut followers: Query<(&mut Transform, &mut FollowerState), Without<PlayerAvatar>>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let dt = time.delta_secs().min(0.1);
    let player = Vec2::new(player_tf.translation.x, player_tf.translation.z);
    let front = player_tf.rotation * Vec3::Z;
    let facing = Vec2::new(front.x, front.z).try_normalize().unwrap_or(Vec2::Y);
    for (mut tf, mut st) in &mut followers {
        // The walk rule is built per follower from its own feet: the ground probe and the
        // step rule must see the follower's surface, not the player's.
        let ctx = WalkContext::player(
            terrain.as_deref(),
            collision.as_deref(),
            water.as_deref(),
            false,
            tf.translation.y - GROUND_OFFSET,
        );
        let walkable = |x: f32, z: f32| ctx.is_walkable(x, z);
        step(&mut st, player, facing, dt, &walkable);
        tf.translation.x = st.pos.0;
        tf.translation.z = st.pos.1;
        if let Some(h) = ctx.ground_y(st.pos.0, st.pos.1) {
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
            // After the frame's locomotion (the viewer's horizontal move runs before
            // `PlayerMotionSet`): the follower then reacts to the player's final position of
            // the frame, so the separation floor holds at the sampled frame boundary too.
            (
                sync_follower,
                drive_follower
                    .run_if(not_paused)
                    .after(super::player_motion::PlayerMotionSet),
            )
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
    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    const DT: f32 = 1.0 / 60.0;
    const WALK_SPEED: f32 = 4.5;
    /// Player facing used by the tests that do not exercise the model yaw.
    const FACE: Vec2 = Vec2::Y;

    fn all(_: f32, _: f32) -> bool {
        true
    }

    fn dist(st: &FollowerState, p: Vec2) -> f32 {
        Vec2::new(st.pos.0, st.pos.1).distance(p)
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
            step(&mut st, player, FACE, DT, &all);
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
            step(&mut st, Vec2::ZERO, FACE, DT, &all);
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
        step(&mut st, Vec2::ZERO, FACE, DT, &all);
        assert_eq!(st.teleports, 1);
        assert!(
            dist(&st, Vec2::ZERO) <= FOLLOW_DISTANCE + 1e-3,
            "{:?}",
            st.pos
        );
        for _ in 0..120 {
            step(&mut st, Vec2::ZERO, FACE, DT, &all);
        }
        assert_eq!(st.teleports, 1, "no repeated teleport while close");
        assert!(dist(&st, Vec2::ZERO) <= FOLLOW_DISTANCE + ARRIVE_EPS + 0.05);
    }

    #[test]
    fn never_steps_off_the_test_corridor() {
        // Narrow corridor: |z| <= 3, the player path stays inside it.
        let walk = |x: f32, z: f32| (-120.0..=120.0).contains(&x) && (-3.0..=3.0).contains(&z);
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
            step(&mut st, player, FACE, DT, &walk);
            assert!(
                walk(st.pos.0, st.pos.1),
                "follower left the corridor at {:?}",
                st.pos
            );
        }
        assert!(st.walked > 40.0);
        assert_eq!(st.teleports, 0);
    }

    #[test]
    fn never_overlaps_the_player_when_it_walks_back_through_the_follower() {
        // Player walks +x for 5 s, then turns around and walks back (toward the camera /
        // the follower), then circles. The follower must never come closer than the floor and
        // must settle at 1.5-3 m when the player stands still.
        let mut st = FollowerState::new((-FOLLOW_DISTANCE, 0.0), std::f32::consts::FRAC_PI_2);
        st.trail.seed([Vec2::new(-FOLLOW_DISTANCE, 0.0), Vec2::ZERO]);
        let mut player = Vec2::ZERO;
        let mut min_d = f32::MAX;
        let mut check = |st: &FollowerState, player: Vec2, i: i32| {
            if i > 60 {
                min_d = min_d.min(dist(st, player));
            }
        };
        let mut i = 0;
        for _ in 0..300 {
            player.x += WALK_SPEED * DT;
            step(&mut st, player, FACE, DT, &all);
            check(&st, player, i);
            i += 1;
        }
        for _ in 0..600 {
            player.x -= WALK_SPEED * DT; // 180-degree reversal
            step(&mut st, player, FACE, DT, &all);
            check(&st, player, i);
            i += 1;
        }
        // Diagonal and sprint reversals too.
        for k in 0..600 {
            let a = k as f32 * 0.02;
            player += Vec2::new(a.cos(), a.sin() * 0.3) * (8.0 * DT) * if k % 200 < 100 { 1.0 } else { -1.0 };
            step(&mut st, player, FACE, DT, &all);
            check(&st, player, i);
            i += 1;
        }
        eprintln!("min separation over reversals: {min_d:.3} m");
        assert!(min_d >= MIN_SEPARATION - 1e-3, "overlap: {min_d}");
        for _ in 0..600 {
            step(&mut st, player, FACE, DT, &all);
        }
        let d = dist(&st, player);
        assert!((1.5..=3.0).contains(&d), "resting distance {d}");
    }

    #[test]
    fn slides_round_the_player_when_the_radial_spot_is_off_the_navmesh() {
        // Walkable only for z >= -0.5 (a cliff edge just behind the player).
        let walk = |_x: f32, z: f32| z >= -0.5;
        let player = Vec2::new(0.0, 0.0);
        let mut st = FollowerState::new((0.0, -0.3), 0.0); // behind the player, on the edge
        st.trail.seed([Vec2::new(0.0, -FOLLOW_DISTANCE), player]);
        step(&mut st, player, FACE, DT, &walk);
        let d = dist(&st, player);
        assert!(d >= MIN_SEPARATION - 1e-3, "still overlapping: {d}");
        assert!(walk(st.pos.0, st.pos.1), "ended off the navmesh: {:?}", st.pos);
    }

    #[test]
    fn standing_on_the_follower_pushes_it_out_instead_of_overlapping() {
        let mut st = FollowerState::new((0.0, 0.0), 0.0);
        st.trail.seed([Vec2::new(-FOLLOW_DISTANCE, 0.0), Vec2::ZERO]);
        step(&mut st, Vec2::ZERO, FACE, DT, &all);
        assert!(dist(&st, Vec2::ZERO) >= MIN_SEPARATION - 1e-4, "{:?}", st.pos);
    }

    #[test]
    fn walks_around_the_player_instead_of_being_pushed_through() {
        // The play-test case: the player turns around and walks straight at the follower
        // that stands in their way. The follower slides around the player instead of being
        // dragged in front of them, with no per-frame jump, and ends up behind.
        let mut st = FollowerState::new((0.0, MIN_SEPARATION + 0.1), 0.0); // in the player's way
        st.trail.seed([Vec2::new(0.0, -FOLLOW_DISTANCE), Vec2::ZERO]);
        let mut player = Vec2::ZERO;
        let mut prev = Vec2::new(st.pos.0, st.pos.1);
        let mut min_d = f32::MAX;
        let mut max_step = 0.0f32;
        let mut max_side = 0.0f32;
        for i in 0..240 {
            player.y += WALK_SPEED * DT;
            step(&mut st, player, FACE, DT, &all);
            let here = Vec2::new(st.pos.0, st.pos.1);
            max_step = max_step.max(prev.distance(here));
            min_d = min_d.min(here.distance(player));
            if i > 30 {
                max_side = max_side.max((here - player).x.abs());
            }
            prev = here;
        }
        eprintln!(
            "through: min {min_d:.3} m, max step {max_step:.3} m/frame, max side {max_side:.3} m"
        );
        assert!(min_d >= MIN_SEPARATION - 1e-3, "overlap {min_d}");
        assert!(max_step <= MAX_SPEED * DT + 0.05, "jerk {max_step} m/frame");
        assert!(max_side > 0.5, "went straight through (side {max_side})");
        let here = Vec2::new(st.pos.0, st.pos.1);
        assert!(
            (here - player).dot(FACE) < 0.0,
            "not behind after the pass: {here:?} vs {player:?}"
        );
        // The player stops: it settles behind at the follow distance.
        for _ in 0..600 {
            step(&mut st, player, FACE, DT, &all);
        }
        let d = dist(&st, player);
        let here = Vec2::new(st.pos.0, st.pos.1);
        assert!((1.5..=3.0).contains(&d), "resting distance {d}");
        assert!(here.y < player.y, "resting behind: {here:?}");
    }

    #[test]
    fn a_follower_left_in_front_walks_back_behind_the_stopped_player() {
        // The player walked backwards (facing +z) and stopped: the recorded path puts the
        // aim point in front of them, where the follower already stands. After the stop
        // delay the follower must walk around to the spot behind the player.
        let mut st = FollowerState::new((0.0, MIN_SEPARATION + 0.01), 0.0);
        // The path goes from +z (where the player came from) to the origin (player).
        st.trail.seed([Vec2::new(0.0, 4.0), Vec2::ZERO]);
        let mut prev = Vec2::new(st.pos.0, st.pos.1);
        let mut max_step = 0.0f32;
        let mut min_d = f32::MAX;
        let mut max_front = f32::MAX; // smallest distance seen while in front (never < floor)
        for i in 0..900 {
            step(&mut st, Vec2::ZERO, FACE, DT, &all);
            let here = Vec2::new(st.pos.0, st.pos.1);
            if i % 120 == 0 {
                eprintln!("i {i:4} pos ({:.3},{:.3}) ang {:.1}", here.x, here.y, here.to_angle().to_degrees());
            }
            max_step = max_step.max(prev.distance(here));
            let d = here.distance(Vec2::ZERO);
            min_d = min_d.min(d);
            if here.dot(FACE) > 0.0 {
                max_front = max_front.min(d);
            }
            prev = here;
        }
        let here = Vec2::new(st.pos.0, st.pos.1);
        eprintln!(
            "reposition: end {here:?}, min {min_d:.3} m, min while in front {max_front:.3} m, max step {max_step:.3} m/frame"
        );
        assert!(min_d >= MIN_SEPARATION - 1e-3, "overlap {min_d}");
        assert!(max_step <= MAX_SPEED * DT + 0.05, "jerk {max_step} m/frame");
        assert!(here.dot(FACE) < 0.0, "still in front: {here:?}");
        let d = here.distance(Vec2::ZERO);
        assert!((1.5..=3.0).contains(&d), "resting distance {d}");
        assert!(st.still > STOP_REPOSITION_DELAY, "still {}", st.still);
    }

    #[test]
    fn still_time_counts_only_standing_still() {
        let mut st = FollowerState::new((0.0, 2.0), 0.0);
        st.trail.seed([Vec2::new(-FOLLOW_DISTANCE, 0.0), Vec2::ZERO]);
        for _ in 0..60 {
            step(&mut st, Vec2::ZERO, FACE, DT, &all);
        }
        assert!(st.still > 0.9, "still {}", st.still);
        step(&mut st, Vec2::new(0.5, 0.0), FACE, DT, &all);
        assert_eq!(st.still, 0.0, "a player step resets the still timer");
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
        app.insert_resource(crate::game::terrain_collision::test_terrain(129, |_, _| 0.0));
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
            "grounded on the flat terrain (y {max_y})"
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
