//! Wander behaviour for overworld wild creatures (Idle <-> Walk inside the spawner radius).
//!
//! All decisions live in plain functions on [`WanderState`] (no Bevy types) so they are
//! unit-tested deterministically. The Bevy plugin at the bottom only feeds positions,
//! the navmesh and the player position into [`step`] and writes back the transform.
//! Verified by unit tests only; never checked visually.

use bevy::prelude::*;

use super::navigation::GameNavmeshResource;
use super::state::{GameState, Paused};
use super::wild::{hash_str, splitmix64, ActiveAreaSpawners, WildCreature};
use super::PlayerAvatar;

pub const MIN_RADIUS: f32 = 3.0;
pub const MAX_RADIUS: f32 = 25.0;
pub const IDLE_MIN_S: f32 = 1.5;
pub const IDLE_MAX_S: f32 = 5.0;
pub const SPEED_MIN: f32 = 0.6;
pub const SPEED_MAX: f32 = 1.2;
/// Distance at which a creature notices the player, turns to face them and stops.
pub const ALERT_RANGE: f32 = 6.0;
/// Yaw turn rate in radians per second.
pub const TURN_RATE: f32 = 4.0;
/// A creature only moves once its facing is within this angle of the heading.
pub const MOVE_ANGLE: f32 = 1.0;
const ARRIVE_DIST: f32 = 0.15;
const TARGET_TRIES: u32 = 8;

/// Global switch. Scripts / demos that need static wild positions set `enabled = false`.
#[derive(Resource, Debug, Clone)]
pub struct WanderConfig {
    pub enabled: bool,
}

impl Default for WanderConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Optional size hint (1.0 = average). Bigger creatures walk slower.
#[derive(Component, Debug, Clone, Copy)]
pub struct WanderSizeHint(pub f32);

/// Fixed parameters of one creature's wander area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WanderParams {
    pub home: (f32, f32),
    pub radius: f32,
    pub speed_scale: f32,
}

impl WanderParams {
    pub fn new(home: (f32, f32), spawner_radius: f32, size_hint: f32) -> Self {
        let size = if size_hint.is_finite() { size_hint.clamp(0.5, 2.0) } else { 1.0 };
        Self {
            home,
            radius: spawner_radius.clamp(MIN_RADIUS, MAX_RADIUS),
            speed_scale: 1.0 / size.sqrt(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    Idle { left: f32 },
    Walk { target: (f32, f32), speed: f32 },
}

/// Per-creature mutable state.
#[derive(Component, Debug, Clone)]
pub struct WanderState {
    pub rng: u64,
    pub phase: Phase,
    pub pos: (f32, f32),
    /// Yaw about +Y; 0 faces +Z (glTF front), see [`yaw_for`].
    pub yaw: f32,
    pub alert: bool,
    /// Planar distance walked since creation (diagnostics / tests).
    pub walked: f32,
}

/// Fixed params stored next to the state.
#[derive(Component, Debug, Clone, Copy)]
pub struct WanderHome {
    pub params: WanderParams,
    /// Height of the creature above the navmesh surface at spawn time.
    pub y_offset: Option<f32>,
}

/// Deterministic seed from spawner id and species.
pub fn wander_seed(spawner_id: &str, species_dex: u16) -> u64 {
    hash_str(spawner_id) ^ (species_dex as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

fn rand01(rng: &mut u64) -> f32 {
    (splitmix64(rng) >> 40) as f32 / (1u64 << 24) as f32
}

fn rand_range(rng: &mut u64, lo: f32, hi: f32) -> f32 {
    lo + (hi - lo) * rand01(rng)
}

/// Yaw that turns the model front (+Z) towards the planar direction `(dx, dz)`.
pub fn yaw_for(dx: f32, dz: f32) -> f32 {
    dx.atan2(dz)
}

/// Shortest signed angle from `a` to `b`, in (-PI, PI].
pub fn angle_diff(a: f32, b: f32) -> f32 {
    let mut d = (b - a) % std::f32::consts::TAU;
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    } else if d <= -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    d
}

/// Turns `yaw` towards `target` by at most `rate * dt` radians.
pub fn turn_towards(yaw: f32, target: f32, rate: f32, dt: f32) -> f32 {
    let d = angle_diff(yaw, target);
    let max = rate * dt;
    if d.abs() <= max { target } else { yaw + max * d.signum() }
}

impl WanderState {
    pub fn new(seed: u64, pos: (f32, f32)) -> Self {
        let mut rng = seed;
        let yaw = rand_range(&mut rng, -std::f32::consts::PI, std::f32::consts::PI);
        let left = rand_range(&mut rng, IDLE_MIN_S, IDLE_MAX_S);
        Self { rng, phase: Phase::Idle { left }, pos, yaw, alert: false, walked: 0.0 }
    }
}

fn pick_target(
    st: &mut WanderState,
    p: &WanderParams,
    walkable: &dyn Fn(f32, f32) -> bool,
) -> Option<(f32, f32)> {
    for _ in 0..TARGET_TRIES {
        let r = p.radius * rand01(&mut st.rng).sqrt();
        let a = rand_range(&mut st.rng, 0.0, std::f32::consts::TAU);
        let t = (p.home.0 + r * a.cos(), p.home.1 + r * a.sin());
        if walkable(t.0, t.1) && dist(t, st.pos) > 0.5 {
            return Some(t);
        }
    }
    None
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn start_idle(st: &mut WanderState) {
    st.phase = Phase::Idle { left: rand_range(&mut st.rng, IDLE_MIN_S, IDLE_MAX_S) };
}

/// Advances one creature by `dt` seconds. `player` is the planar player position, if any.
pub fn step(
    st: &mut WanderState,
    p: &WanderParams,
    dt: f32,
    player: Option<(f32, f32)>,
    walkable: &dyn Fn(f32, f32) -> bool,
) {
    // Alert: face the player and stop. Timers are frozen while alert.
    if let Some(pl) = player {
        if dist(pl, st.pos) <= ALERT_RANGE {
            st.alert = true;
            let (dx, dz) = (pl.0 - st.pos.0, pl.1 - st.pos.1);
            if dx.abs() + dz.abs() > 1e-4 {
                st.yaw = turn_towards(st.yaw, yaw_for(dx, dz), TURN_RATE, dt);
            }
            return;
        }
    }
    st.alert = false;

    match st.phase {
        Phase::Idle { left } => {
            let left = left - dt;
            if left > 0.0 {
                st.phase = Phase::Idle { left };
                return;
            }
            match pick_target(st, p, walkable) {
                Some(target) => {
                    let speed = rand_range(&mut st.rng, SPEED_MIN, SPEED_MAX) * p.speed_scale;
                    st.phase = Phase::Walk { target, speed };
                }
                None => start_idle(st),
            }
        }
        Phase::Walk { target, speed } => {
            let (dx, dz) = (target.0 - st.pos.0, target.1 - st.pos.1);
            let d = (dx * dx + dz * dz).sqrt();
            if d <= ARRIVE_DIST {
                start_idle(st);
                return;
            }
            st.yaw = turn_towards(st.yaw, yaw_for(dx, dz), TURN_RATE, dt);
            if angle_diff(st.yaw, yaw_for(dx, dz)).abs() > MOVE_ANGLE {
                return; // still turning on the spot
            }
            let adv = (speed * dt).min(d);
            let next = (st.pos.0 + dx / d * adv, st.pos.1 + dz / d * adv);
            if dist(next, p.home) > p.radius || !walkable(next.0, next.1) {
                start_idle(st);
                return;
            }
            st.walked += adv;
            st.pos = next;
            if adv >= d - 1e-6 {
                start_idle(st);
            }
        }
    }
}

/// Pure gating rule: wander only when enabled, in the overworld and not paused.
pub fn wander_allowed(enabled: bool, in_world: bool, paused: bool) -> bool {
    enabled && in_world && !paused
}

/// Run condition: wander only in the overworld, when enabled and while the pause menu is closed.
pub fn wander_active(
    config: Res<WanderConfig>,
    state: Option<Res<State<GameState>>>,
    paused: Option<Res<Paused>>,
) -> bool {
    wander_allowed(
        config.enabled,
        state.map_or(true, |s| *s.get() == GameState::World),
        paused.is_some_and(|p| p.0),
    )
}

fn attach_wander(
    mut commands: Commands,
    spawners: Option<Res<ActiveAreaSpawners>>,
    nav: Option<Res<GameNavmeshResource>>,
    q: Query<(Entity, &WildCreature, &Transform, Option<&WanderSizeHint>), Without<WanderState>>,
) {
    for (e, wild, tf, hint) in &q {
        let radius = spawners
            .as_ref()
            .and_then(|s| s.spawners.iter().find(|d| d.spawner_id == wild.spawner_id))
            .map_or(8.0, |d| d.radius);
        let home = (wild.spawn_pos.x, wild.spawn_pos.z);
        let params = WanderParams::new(home, radius, hint.map_or(1.0, |h| h.0));
        let y_offset = nav
            .as_ref()
            .and_then(|n| n.height_at(tf.translation.x, tf.translation.z))
            .map(|h| tf.translation.y - h);
        let seed = wander_seed(&wild.spawner_id, wild.national_dex);
        commands.entity(e).insert((
            WanderState::new(seed, (tf.translation.x, tf.translation.z)),
            WanderHome { params, y_offset },
        ));
    }
}

fn wander_system(
    time: Res<Time>,
    nav: Option<Res<GameNavmeshResource>>,
    wall_collision: Option<Res<super::world_collision::WorldCollision>>,
    player: Query<&Transform, (With<PlayerAvatar>, Without<WanderState>)>,
    mut q: Query<(&mut Transform, &mut WanderState, &WanderHome, &WildCreature)>,
) {
    // Without a navmesh walkability is unknown: stay put rather than risk walking off the map.
    let Some(nav) = nav else { return };
    let dt = time.delta_secs().min(0.1);
    let pl = player.single().ok().map(|t| (t.translation.x, t.translation.z));
    let walkable = |x: f32, z: f32| nav.is_walkable(x, z);
    for (mut tf, mut st, home, wild) in &mut q {
        if wild.in_battle || wild.defeated {
            continue;
        }
        step(&mut st, &home.params, dt, pl, &walkable);
        // Placed-object collision after the wander step, keeping the creature on the navmesh.
        if let Some(wc) = wall_collision.as_ref() {
            let pushed = wc.push_out_xz(st.pos.0, tf.translation.y + 0.6, st.pos.1, 0.4, 1.2);
            if nav.is_walkable(pushed.x, pushed.y) {
                st.pos = (pushed.x, pushed.y);
            }
        }
        tf.translation.x = st.pos.0;
        tf.translation.z = st.pos.1;
        if let (Some(off), Some(h)) = (home.y_offset, nav.height_at(st.pos.0, st.pos.1)) {
            tf.translation.y = h + off;
        }
        tf.rotation = Quat::from_rotation_y(st.yaw);
    }
}

/// Registers the wander systems. Add with `app.add_plugins(WildWanderPlugin)`.
pub struct WildWanderPlugin;

impl Plugin for WildWanderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WanderConfig>().add_systems(
            Update,
            (attach_wander, wander_system.run_if(wander_active)).chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 30.0;
    fn all(_: f32, _: f32) -> bool {
        true
    }
    fn params() -> WanderParams {
        WanderParams::new((100.0, 200.0), 8.0, 1.0)
    }
    fn fresh(id: &str) -> WanderState {
        WanderState::new(wander_seed(id, 25), (100.0, 200.0))
    }

    #[test]
    fn wander_is_gated_by_config_state_and_pause() {
        assert!(wander_allowed(true, true, false));
        assert!(!wander_allowed(false, true, false), "disabled by config");
        assert!(!wander_allowed(true, false, false), "not in the overworld");
        assert!(!wander_allowed(true, true, true), "paused");
    }

    #[test]
    fn radius_is_clamped() {
        assert_eq!(WanderParams::new((0.0, 0.0), 0.5, 1.0).radius, 3.0);
        assert_eq!(WanderParams::new((0.0, 0.0), 90.0, 1.0).radius, 25.0);
    }

    #[test]
    fn stays_inside_radius_10k_steps() {
        let p = params();
        let mut st = fresh("a");
        for _ in 0..10_000 {
            step(&mut st, &p, DT, None, &all);
            assert!(dist(st.pos, p.home) <= p.radius + 1e-3);
        }
        assert!(st.walked > 5.0, "creature should actually move");
    }

    #[test]
    fn idle_duration_and_speed_in_range() {
        let p = params();
        let mut st = fresh("b");
        let mut idle_steps = 0.0f32;
        let mut seen_walk = 0;
        let mut last_idle = true;
        for _ in 0..20_000 {
            let was_idle = matches!(st.phase, Phase::Idle { .. });
            step(&mut st, &p, DT, None, &all);
            match st.phase {
                Phase::Idle { .. } => {
                    idle_steps += DT;
                    last_idle = true;
                }
                Phase::Walk { speed, .. } => {
                    assert!((SPEED_MIN..=SPEED_MAX).contains(&speed), "speed {speed}");
                    if last_idle && was_idle {
                        // idle -> walk transition: idle lasted within range (+1 step slack)
                        assert!(idle_steps >= IDLE_MIN_S - DT && idle_steps <= IDLE_MAX_S + 2.0 * DT,
                            "idle {idle_steps}");
                        seen_walk += 1;
                    }
                    idle_steps = 0.0;
                    last_idle = false;
                }
            }
        }
        assert!(seen_walk >= 5);
    }

    #[test]
    fn size_hint_slows_big_creatures() {
        let small = WanderParams::new((0.0, 0.0), 8.0, 0.5);
        let big = WanderParams::new((0.0, 0.0), 8.0, 2.0);
        assert!(small.speed_scale > 1.0 && big.speed_scale < 1.0);
    }

    #[test]
    fn respects_walkability() {
        let p = params();
        // Half of the circle (x > home.x) is blocked.
        let walk = |x: f32, _z: f32| x <= 100.0;
        let mut st = fresh("c");
        st.pos = (99.0, 200.0);
        for _ in 0..10_000 {
            step(&mut st, &p, DT, None, &walk);
            assert!(st.pos.0 <= 100.0 + 1e-3, "entered blocked area: {:?}", st.pos);
        }
        assert!(st.walked > 1.0);
    }

    #[test]
    fn nothing_walkable_means_stays_idle() {
        let p = params();
        let mut st = fresh("d");
        for _ in 0..2_000 {
            step(&mut st, &p, DT, None, &|_, _| false);
        }
        assert_eq!(st.walked, 0.0);
    }

    #[test]
    fn facing_interpolates_smoothly() {
        let y = turn_towards(0.0, 3.0, TURN_RATE, DT);
        assert!((y - TURN_RATE * DT).abs() < 1e-6);
        assert_eq!(turn_towards(0.0, 0.01, TURN_RATE, DT), 0.01);
        // shortest way around the circle
        let y = turn_towards(3.0, -3.0, TURN_RATE, DT);
        assert!(y > 3.0, "should turn through PI, got {y}");
        assert!((angle_diff(3.0, -3.0) - (std::f32::consts::TAU - 6.0)).abs() < 1e-5);
        // yaw: +Z is 0, +X is PI/2
        assert!(yaw_for(0.0, 1.0).abs() < 1e-6);
        assert!((yaw_for(1.0, 0.0) - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn deterministic_for_same_seed() {
        let p = params();
        let run = |id: &str| {
            let mut st = fresh(id);
            let mut trace = Vec::new();
            for i in 0..3_000 {
                step(&mut st, &p, DT, None, &all);
                if i % 100 == 0 {
                    trace.push((st.pos, st.yaw));
                }
            }
            trace
        };
        assert_eq!(run("x"), run("x"));
        assert_ne!(run("x"), run("y"));
    }

    #[test]
    fn alert_faces_player_and_stops() {
        let p = params();
        let mut st = fresh("e");
        for _ in 0..600 {
            step(&mut st, &p, DT, None, &all);
        }
        let player = (st.pos.0 + 4.0, st.pos.1); // 4 m east
        let before = st.pos;
        for _ in 0..120 {
            step(&mut st, &p, DT, Some(player), &all);
        }
        assert!(st.alert);
        assert_eq!(st.pos, before, "must not move while alert");
        assert!((angle_diff(st.yaw, yaw_for(1.0, 0.0))).abs() < 1e-4);
        // player leaves: wandering resumes
        for _ in 0..2_000 {
            let far = (st.pos.0 + 50.0, st.pos.1);
            step(&mut st, &p, DT, Some(far), &all);
        }
        assert!(!st.alert);
        assert!(dist(st.pos, before) > 0.0);
    }

    #[test]
    fn far_player_does_not_alert() {
        let p = params();
        let mut st = fresh("f");
        let far = (st.pos.0 + ALERT_RANGE + 1.0, st.pos.1);
        step(&mut st, &p, DT, Some(far), &all);
        assert!(!st.alert);
    }
}
