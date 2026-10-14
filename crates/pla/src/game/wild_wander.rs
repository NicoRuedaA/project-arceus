//! Wander behaviour for overworld wild creatures (Idle <-> Walk inside the spawner radius).
//!
//! All decisions live in plain functions on [`WanderState`] (no Bevy types) so they are
//! unit-tested deterministically. The Bevy plugin at the bottom only feeds positions,
//! the navmesh and the player position into [`step`] and writes back the transform.
//! Verified by unit tests only; never checked visually.
//!
//! Reactions ([`WildBehaviorSettings`], viewer flag `--wild-behavior`): when enabled (the
//! default) every creature's [`WildBehavior`] state machine is stepped each frame from the
//! player's distance, the angle to the creature's facing and the player's noise (still / walking
//! / running). Idle and Wander keep the wander loop above; Alert and Attack stop and face the
//! player, Flee walks away, Chase walks towards the player (the existing proximity encounter of
//! the viewer then starts the battle), Sleep stands still. Every movement stays inside the
//! spawner radius and on the navmesh. With `--wild-behavior off` the plain wander loop (stop and
//! face within [`ALERT_RANGE`]) runs exactly as before. Archetype per creature, flee/chase
//! speeds and noise thresholds are SUPPOSED port policy (no species data is decoded).

use bevy::prelude::*;

use super::navigation::GameNavmeshResource;
use super::state::{GameState, Paused};
use super::wild::{hash_str, splitmix64, ActiveAreaSpawners, WildCreature};
use super::wild_behavior::{
    Archetype, BehaviorParams, Machine, Noise, Perception, State as ReactionState, WildBehavior,
};
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

/// Which archetypes drive the wild reaction state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchetypeChoice {
    /// Deterministic per-creature pick among passive / timid / aggressive (SUPPOSED policy).
    Mixed,
    /// Every creature uses this archetype (testing aid).
    All(Archetype),
}

/// `--wild-behavior on|off|passive|timid|aggressive` (default `on` = mixed archetypes).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WildBehaviorSettings {
    /// `false` = plain wander loop only (the pre-wiring behaviour).
    pub enabled: bool,
    pub archetypes: ArchetypeChoice,
}

impl Default for WildBehaviorSettings {
    fn default() -> Self {
        Self { enabled: true, archetypes: ArchetypeChoice::Mixed }
    }
}

impl WildBehaviorSettings {
    /// Parse `--wild-behavior VALUE` (or `--wild-behavior=VALUE`); unknown values keep the
    /// default and print a warning.
    pub fn from_args(args: &[String]) -> Self {
        let value = args.iter().enumerate().find_map(|(i, a)| {
            if a == "--wild-behavior" {
                args.get(i + 1).cloned()
            } else {
                a.strip_prefix("--wild-behavior=").map(str::to_string)
            }
        });
        let Some(value) = value else { return Self::default() };
        match Self::parse(&value) {
            Some(settings) => settings,
            None => {
                super::cli_flags::report_bad_value(
                    "--wild-behavior",
                    &value,
                    "on|off|passive|timid|aggressive",
                    "on",
                );
                Self::default()
            }
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        let all = |a| Some(Self { enabled: true, archetypes: ArchetypeChoice::All(a) });
        match value.to_ascii_lowercase().as_str() {
            "on" | "mixed" => Some(Self::default()),
            "off" => Some(Self { enabled: false, ..Self::default() }),
            "passive" => all(Archetype::Passive),
            "timid" => all(Archetype::Timid),
            "aggressive" => all(Archetype::Aggressive),
            _ => None,
        }
    }

    /// Archetype of a creature with wander seed `seed`.
    pub fn archetype_for(&self, seed: u64) -> Archetype {
        match self.archetypes {
            ArchetypeChoice::All(a) => a,
            ArchetypeChoice::Mixed => assumed_archetype(seed),
        }
    }
}

/// SUPPOSED port policy: no species-to-archetype data is decoded, so each creature gets one of
/// the three archetypes from its deterministic wander seed (about one third each).
pub fn assumed_archetype(seed: u64) -> Archetype {
    let mut rng = seed ^ 0xa5a5_5a5a_c3c3_3c3c;
    match splitmix64(&mut rng) % 3 {
        0 => Archetype::Passive,
        1 => Archetype::Timid,
        _ => Archetype::Aggressive,
    }
}

/// SUPPOSED flee speed (m/s, before the size scale); faster than the wander walk.
pub const FLEE_SPEED: f32 = 3.5;
/// SUPPOSED chase speed (m/s, before the size scale); slower than the player's walk (4.5 m/s).
pub const CHASE_SPEED: f32 = 3.0;
/// A chasing creature stops this far inside its attack distance (m).
const CHASE_STOP_MARGIN: f32 = 0.05;
/// SUPPOSED player speed (m/s) above which the player counts as running (walk 4.5, sprint 8).
pub const RUN_NOISE_SPEED: f32 = 6.0;
/// SUPPOSED player speed (m/s) above which the player makes walking noise.
pub const WALK_NOISE_SPEED: f32 = 0.5;

/// Noise class of the player from its planar speed.
pub fn noise_from_speed(speed: f32) -> Noise {
    if !speed.is_finite() || speed <= WALK_NOISE_SPEED {
        Noise::Silent
    } else if speed <= RUN_NOISE_SPEED {
        Noise::Walk
    } else {
        Noise::Run
    }
}

/// What a creature perceives of the player this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerSense {
    pub pos: (f32, f32),
    pub noise: Noise,
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

/// Moves `st` along `heading` by up to `max_adv` metres after turning towards it; refuses
/// (stays put) when the next point leaves the spawner radius or the navmesh. Returns the
/// distance moved.
fn move_heading(
    st: &mut WanderState,
    p: &WanderParams,
    heading: f32,
    max_adv: f32,
    dt: f32,
    walkable: &dyn Fn(f32, f32) -> bool,
) -> f32 {
    st.yaw = turn_towards(st.yaw, heading, TURN_RATE, dt);
    if angle_diff(st.yaw, heading).abs() > MOVE_ANGLE || max_adv <= 0.0 {
        return 0.0;
    }
    let next = (st.pos.0 + heading.sin() * max_adv, st.pos.1 + heading.cos() * max_adv);
    if dist(next, p.home) > p.radius || !walkable(next.0, next.1) {
        return 0.0;
    }
    st.pos = next;
    st.walked += max_adv;
    max_adv
}

/// Perception of `player` by a creature at `st` (a hidden or absent player is not perceived).
pub fn perception_of(st: &WanderState, player: Option<PlayerSense>) -> Perception {
    match player {
        Some(pl) => {
            let (dx, dz) = (pl.pos.0 - st.pos.0, pl.pos.1 - st.pos.1);
            let angle = if dx.abs() + dz.abs() > 1e-4 {
                angle_diff(st.yaw, yaw_for(dx, dz)).abs()
            } else {
                0.0
            };
            Perception {
                distance: (dx * dx + dz * dz).sqrt(),
                angle,
                noise: pl.noise,
                player_hidden: false,
                hour: None,
            }
        }
        None => Perception { player_hidden: true, ..Perception::quiet(f32::INFINITY) },
    }
}

/// One frame of a creature driven by its reaction state machine (`--wild-behavior on`).
///
/// The machine is stepped first; Idle/Wander then run the plain wander loop without its own
/// player alert (the machine owns noticing), and every other state moves or faces the player as
/// described in the module docs.
pub fn behavior_step(
    st: &mut WanderState,
    p: &WanderParams,
    machine: &mut Machine,
    params: BehaviorParams,
    dt: f32,
    player: Option<PlayerSense>,
    walkable: &dyn Fn(f32, f32) -> bool,
) {
    let perception = perception_of(st, player);
    machine.step(dt, params, perception);
    let to_player = player.map(|pl| (pl.pos.0 - st.pos.0, pl.pos.1 - st.pos.1));
    match machine.state {
        ReactionState::Idle | ReactionState::Wander => {
            st.alert = false;
            step(st, p, dt, None, walkable);
            machine.sync_wander_phase(matches!(st.phase, Phase::Walk { .. }));
        }
        ReactionState::Sleep => st.alert = false,
        ReactionState::Alert | ReactionState::Attack => {
            st.alert = true;
            if let Some((dx, dz)) = to_player.filter(|(dx, dz)| dx.abs() + dz.abs() > 1e-4) {
                st.yaw = turn_towards(st.yaw, yaw_for(dx, dz), TURN_RATE, dt);
            }
        }
        ReactionState::Flee => {
            st.alert = true;
            if let Some((dx, dz)) = to_player.filter(|(dx, dz)| dx.abs() + dz.abs() > 1e-4) {
                let speed = FLEE_SPEED * p.speed_scale;
                move_heading(st, p, yaw_for(-dx, -dz), speed * dt, dt, walkable);
            }
        }
        ReactionState::Chase => {
            st.alert = true;
            if let Some((dx, dz)) = to_player.filter(|(dx, dz)| dx.abs() + dz.abs() > 1e-4) {
                // Stop just inside the attack distance so float noise cannot flip Attack/Chase.
                let gap = (dx * dx + dz * dz).sqrt() - (params.attack_distance - CHASE_STOP_MARGIN);
                let adv = (CHASE_SPEED * p.speed_scale * dt).min(gap.max(0.0));
                move_heading(st, p, yaw_for(dx, dz), adv, dt, walkable);
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
    settings: Option<Res<WildBehaviorSettings>>,
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
        let behavior = match settings.as_deref() {
            Some(s) if s.enabled => WildBehavior {
                machine: Machine::new(seed),
                params: BehaviorParams::assumed(s.archetype_for(seed)),
            },
            _ => WildBehavior::assumed_passive(seed),
        };
        commands.entity(e).try_insert((
            WanderState::new(seed, (tf.translation.x, tf.translation.z)),
            WanderHome { params, y_offset },
            behavior,
        ));
    }
}

/// The player position creatures can react to: none while the player is hidden.
pub fn noticeable_player(player: Option<(f32, f32)>, hidden: bool) -> Option<(f32, f32)> {
    player.filter(|_| !hidden)
}

#[allow(clippy::too_many_arguments)]
fn wander_system(
    time: Res<Time>,
    settings: Option<Res<WildBehaviorSettings>>,
    mut last_player: Local<Option<(f32, f32)>>,
    nav: Option<Res<GameNavmeshResource>>,
    wall_collision: Option<Res<super::world_collision::WorldCollision>>,
    player: Query<&Transform, (With<PlayerAvatar>, Without<WanderState>)>,
    stealth: Option<Res<super::player_skills::PlayerStealth>>,
    safety: Option<Res<super::wild::EncounterSafety>>,
    mut q: Query<(
        &mut Transform,
        &mut WanderState,
        &WanderHome,
        &WildCreature,
        Option<&mut WildBehavior>,
    )>,
) {
    // Without a navmesh walkability is unknown: stay put rather than risk walking off the map.
    let Some(nav) = nav else { return };
    let dt = time.delta_secs().min(0.1);
    // A hidden player (tall grass, or crouching in a stealth volume) is not noticed: the
    // creatures wander as if nobody were there (game::player_skills::PlayerStealth).
    let hidden = stealth.is_some_and(|s| s.hidden);
    let raw_player = player.single().ok().map(|t| (t.translation.x, t.translation.z));
    // During the arrival grace period nobody notices the player: no alert, no aggressive chase.
    let grace = safety.is_some_and(|s| s.grace_active());
    let pl = noticeable_player(raw_player, hidden || grace);
    // Player noise from its planar speed since the last frame (a teleport reads as one frame of
    // running noise at most).
    let player_speed = match (raw_player, *last_player) {
        (Some(now), Some(before)) if dt > 0.0 => dist(now, before) / dt,
        _ => 0.0,
    };
    *last_player = raw_player;
    let sense = pl.map(|pos| PlayerSense { pos, noise: noise_from_speed(player_speed) });
    let reactions = settings.is_none_or(|s| s.enabled);
    let walkable = |x: f32, z: f32| nav.is_walkable(x, z);
    for (mut tf, mut st, home, wild, behavior) in &mut q {
        if wild.in_battle || wild.defeated {
            continue;
        }
        match behavior {
            Some(mut behavior) if reactions => {
                let WildBehavior { machine, params } = &mut *behavior;
                behavior_step(&mut st, &home.params, machine, *params, dt, sense, &walkable);
            }
            behavior => {
                step(&mut st, &home.params, dt, pl, &walkable);
                if let Some(mut behavior) = behavior {
                    behavior.machine.sync_wander_phase(matches!(st.phase, Phase::Walk { .. }));
                }
            }
        }
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
        app.init_resource::<WanderConfig>()
            .init_resource::<WildBehaviorSettings>()
            .add_systems(
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
    fn a_hidden_player_is_not_noticed() {
        assert_eq!(noticeable_player(Some((1.0, 2.0)), false), Some((1.0, 2.0)));
        assert_eq!(noticeable_player(Some((1.0, 2.0)), true), None);
        assert_eq!(noticeable_player(None, false), None);
        // A creature that alerts to a visible player 4 m away ignores a hidden one.
        let p = params();
        for hidden in [false, true] {
            let mut st = fresh("e");
            for _ in 0..600 {
                step(&mut st, &p, DT, None, &all);
            }
            let player = (st.pos.0 + 4.0, st.pos.1);
            for _ in 0..120 {
                step(&mut st, &p, DT, noticeable_player(Some(player), hidden), &all);
            }
            assert_eq!(st.alert, !hidden, "hidden = {hidden}");
        }
    }

    #[test]
    fn headless_three_creatures_receive_behavior_fallbacks() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, WildWanderPlugin));
        app.world_mut().spawn((PlayerAvatar, Transform::default()));
        for i in 0..3 {
            app.world_mut().spawn((
                WildCreature {
                    species_name: format!("synthetic-{i}"),
                    national_dex: 25 + i as u16,
                    level: 5,
                    spawn_pos: Vec3::new(i as f32, 0.0, i as f32),
                    aggro_range: 2.8,
                    in_battle: false,
                    defeated: false,
                    spawner_id: format!("synthetic-{i}"),
                },
                Transform::default(),
            ));
        }
        app.update();
        let mut query = app.world_mut().query::<(&WanderState, &WildBehavior)>();
        assert_eq!(query.iter(app.world()).count(), 3);
    }

    fn wide() -> WanderParams {
        WanderParams::new((100.0, 200.0), 25.0, 1.0)
    }

    /// Creature at the home point facing +Z, player `ahead` metres in front of it.
    fn facing_setup(id: &str, ahead: f32) -> (WanderState, PlayerSense) {
        let mut st = fresh(id);
        st.yaw = 0.0;
        let player = PlayerSense { pos: (100.0, 200.0 + ahead), noise: Noise::Silent };
        (st, player)
    }

    fn reaction(archetype: Archetype) -> (Machine, BehaviorParams) {
        (Machine::new(9), BehaviorParams::assumed(archetype))
    }

    #[test]
    fn wild_behavior_flag_parses_and_defaults_to_on() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(WildBehaviorSettings::from_args(&args(&["viewer"])), WildBehaviorSettings::default());
        assert!(WildBehaviorSettings::default().enabled);
        assert!(!WildBehaviorSettings::from_args(&args(&["v", "--wild-behavior", "off"])).enabled);
        assert!(WildBehaviorSettings::from_args(&args(&["v", "--wild-behavior=on"])).enabled);
        assert_eq!(
            WildBehaviorSettings::from_args(&args(&["v", "--wild-behavior", "timid"])).archetypes,
            ArchetypeChoice::All(Archetype::Timid)
        );
        assert_eq!(WildBehaviorSettings::parse("bogus"), None);
        assert_eq!(
            WildBehaviorSettings::from_args(&args(&["v", "--wild-behavior", "bogus"])),
            WildBehaviorSettings::default()
        );
    }

    #[test]
    fn mixed_archetypes_are_deterministic_and_use_all_three() {
        let picks: Vec<Archetype> = (0..300u64).map(|i| assumed_archetype(wander_seed(&format!("s{i}"), 25))).collect();
        for a in [Archetype::Passive, Archetype::Timid, Archetype::Aggressive] {
            let n = picks.iter().filter(|p| **p == a).count();
            assert!((60..=140).contains(&n), "{a:?}: {n} of 300");
        }
        assert_eq!(assumed_archetype(42), assumed_archetype(42));
        let forced = WildBehaviorSettings::parse("aggressive").unwrap();
        assert_eq!(forced.archetype_for(1), Archetype::Aggressive);
    }

    #[test]
    fn noise_follows_player_speed() {
        assert_eq!(noise_from_speed(0.0), Noise::Silent);
        assert_eq!(noise_from_speed(4.5), Noise::Walk);
        assert_eq!(noise_from_speed(8.0), Noise::Run);
        assert_eq!(noise_from_speed(f32::NAN), Noise::Silent);
    }

    #[test]
    fn passive_creature_alerts_faces_and_stays_put() {
        let p = wide();
        let (mut st, _) = facing_setup("pa", 0.0);
        let (mut m, params) = reaction(Archetype::Passive);
        // Player 5 m to the east, inside the 10 m vision only once the creature looks; walking
        // noise (6 m hearing) makes it notice from any angle.
        let player = PlayerSense { pos: (105.0, 200.0), noise: Noise::Walk };
        let before = st.pos;
        for _ in 0..30 {
            behavior_step(&mut st, &p, &mut m, params, DT, Some(player), &all);
        }
        assert_eq!(m.state, ReactionState::Alert);
        assert!(st.alert);
        assert_eq!(st.pos, before, "an alert creature does not walk");
        assert!(angle_diff(st.yaw, yaw_for(1.0, 0.0)).abs() < 1e-3, "faces the player");
    }

    #[test]
    fn timid_creature_flees_away_inside_its_radius() {
        let p = wide();
        let (mut st, player) = facing_setup("ti", 3.0);
        let (mut m, params) = reaction(Archetype::Timid);
        let start = dist(st.pos, player.pos);
        let mut fled = false;
        for _ in 0..90 {
            behavior_step(&mut st, &p, &mut m, params, DT, Some(player), &all);
            fled |= m.state == ReactionState::Flee;
            assert!(dist(st.pos, p.home) <= p.radius + 1e-3);
        }
        assert!(fled, "timid reaction is Flee");
        assert!(dist(st.pos, player.pos) > start + 2.0, "moved away from the player");
    }

    #[test]
    fn aggressive_creature_chases_then_stops_at_attack_distance() {
        let p = wide();
        let (mut st, player) = facing_setup("ag", 8.0);
        let (mut m, params) = reaction(Archetype::Aggressive);
        for _ in 0..300 {
            behavior_step(&mut st, &p, &mut m, params, DT, Some(player), &all);
        }
        assert_eq!(m.state, ReactionState::Attack);
        let d = dist(st.pos, player.pos);
        assert!(d <= params.attack_distance + 1e-3 && d >= params.attack_distance - 0.2, "d = {d}");
        // Within the viewer's 2.8 m proximity encounter, so the existing battle start fires.
        assert!(d < 2.8);
    }

    #[test]
    fn chase_and_flee_respect_walkability() {
        let p = wide();
        let blocked = |_: f32, _: f32| false;
        for archetype in [Archetype::Timid, Archetype::Aggressive] {
            let (mut st, player) = facing_setup("wb", 6.0);
            let (mut m, params) = reaction(archetype);
            let before = st.pos;
            for _ in 0..120 {
                behavior_step(&mut st, &p, &mut m, params, DT, Some(player), &blocked);
            }
            assert_eq!(st.pos, before, "{archetype:?} must not leave the navmesh");
        }
    }

    #[test]
    fn hidden_or_absent_player_leaves_the_wander_loop_in_charge() {
        let p = params();
        let (mut st, _) = facing_setup("hid", 0.0);
        let (mut m, params_b) = reaction(Archetype::Aggressive);
        for _ in 0..3_000 {
            behavior_step(&mut st, &p, &mut m, params_b, DT, None, &all);
            assert!(matches!(m.state, ReactionState::Idle | ReactionState::Wander));
        }
        assert!(st.walked > 1.0, "still wanders");
        assert!(perception_of(&st, None).player_hidden);
    }

    #[test]
    fn headless_settings_choose_the_attached_archetype() {
        for (value, expected) in [
            ("timid", Archetype::Timid),
            ("aggressive", Archetype::Aggressive),
            ("off", Archetype::Passive),
        ] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, WildWanderPlugin));
            app.insert_resource(WildBehaviorSettings::parse(value).unwrap());
            app.world_mut().spawn((
                WildCreature {
                    species_name: "synthetic".into(),
                    national_dex: 399,
                    level: 5,
                    spawn_pos: Vec3::ZERO,
                    aggro_range: 2.8,
                    in_battle: false,
                    defeated: false,
                    spawner_id: "synthetic".into(),
                },
                Transform::default(),
            ));
            app.update();
            let mut q = app.world_mut().query::<&WildBehavior>();
            let b = q.single(app.world()).unwrap();
            assert_eq!(b.params.archetype, expected, "--wild-behavior {value}");
        }
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
