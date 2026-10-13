//! Pure ballistics and life-cycle logic for Poké Balls.
//!
//! Provides deterministic state updates, decay timers, despawn conditions,
//! and bounce limits without Bevy dependencies.

use bevy::prelude::*;

/// Ball state enum mirroring in-game ball trajectory and capture animation.
///
/// A ball that hits a wild Pokémon goes `Absorbing` (energy draw) → `Dropping` (the ball falls
/// to the ground carrying it) → `Shaking` → `Captured` / `Breakout`. The absorb and drop
/// timings are ASSUMED presentation values; the shake count comes from the capture roll
/// (`capture_world::WorldCaptureRoll`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BallLifecycleState {
    InFlight,
    RollingOnGround { roll_timer: f32 },
    /// Hit landed: the Pokémon is drawn into the ball.
    Absorbing {
        timer: f32,
        ground_y: f32,
        max_shakes: u8,
        will_capture: bool,
    },
    /// The ball falls from the impact point back to `ground_y`.
    Dropping {
        timer: f32,
        ground_y: f32,
        max_shakes: u8,
        will_capture: bool,
    },
    Shaking {
        shake_count: u8,
        timer: f32,
        max_shakes: u8,
        will_capture: bool,
    },
    Captured { despawn_timer: f32 },
    Breakout,
}

/// Physical parameters and lifecycle tracker for a thrown Poké Ball.
#[derive(Debug, Clone, PartialEq)]
pub struct BallSimulation {
    pub position: Vec3,
    pub velocity: Vec3,
    pub gravity: f32,
    pub lifetime: f32,
    pub bounces: u8,
    pub state: BallLifecycleState,
}

/// Why a ball left the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DespawnReason {
    /// Lived longer than [`BallSimulation::MAX_LIFETIME`].
    Lifetime,
    /// Flew or rolled beyond [`BallSimulation::MAX_DISTANCE_FROM_PLAYER`].
    TooFar,
    /// Touched the ground and rolled for [`BallSimulation::GROUND_ROLL_DESPAWN_TIME`].
    RolledOut,
    /// The capture result was shown (aura time elapsed).
    CaptureShown,
    /// The Pokemon broke free of the ball.
    BrokeFree,
}

/// Something worth telling the player during one tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BallEvent {
    /// The Pokemon was drawn into the ball (absorb finished).
    Absorbed,
    /// The ball finished falling to the ground, shakes start next.
    Dropped,
    /// The n-th shake finished (1-based).
    Shake(u8),
    /// The last shake finished and the Pokemon is caught.
    Captured,
    /// The last shake finished and the Pokemon escaped.
    BrokeFree,
}

/// Result of one tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TickOutcome {
    pub despawn: Option<DespawnReason>,
    pub event: Option<BallEvent>,
}

impl BallSimulation {
    pub const MAX_LIFETIME: f32 = 8.0;
    pub const MAX_DISTANCE_FROM_PLAYER: f32 = 150.0;
    pub const GROUND_ROLL_DESPAWN_TIME: f32 = 2.0;
    pub const CAPTURE_AURA_DESPAWN_TIME: f32 = 1.8;
    /// Time the Pokemon takes to be drawn into the ball. ASSUMED (no decoded clip timing).
    pub const ABSORB_TIME: f32 = 0.35;
    /// Safeguard so the drop always ends even with gravity 0. ASSUMED.
    pub const DROP_MAX_TIME: f32 = 1.5;
    /// Time between shakes. ASSUMED (viewer value kept).
    pub const SHAKE_INTERVAL: f32 = 0.65;

    pub fn new(position: Vec3, velocity: Vec3, gravity: f32) -> Self {
        Self {
            position,
            velocity,
            gravity,
            lifetime: 0.0,
            bounces: 0,
            state: BallLifecycleState::InFlight,
        }
    }

    /// Put the ball on a wild Pokemon: it absorbs it at the impact point, drops to `ground_y`,
    /// shakes `max_shakes` times, then captures or breaks free.
    pub fn hit_pokemon(&mut self, at: Vec3, ground_y: f32, max_shakes: u8, will_capture: bool) {
        self.position = at;
        self.velocity = Vec3::ZERO;
        self.state = BallLifecycleState::Absorbing {
            timer: 0.0,
            ground_y,
            max_shakes: max_shakes.max(1),
            will_capture,
        };
    }

    /// True while the ball can still hit a Pokemon.
    pub fn in_flight(&self) -> bool {
        matches!(self.state, BallLifecycleState::InFlight)
    }

    /// Advance simulation by `dt` seconds given terrain ground height at the current X/Z.
    /// Returns `true` if the ball should be despawned.
    pub fn tick(&mut self, dt: f32, ground_y: f32, player_pos: Vec3) -> bool {
        self.tick_events(dt, ground_y, player_pos).despawn.is_some()
    }

    /// Same as [`tick`](Self::tick) but reports the reason to despawn and capture events.
    pub fn tick_events(&mut self, dt: f32, ground_y: f32, player_pos: Vec3) -> TickOutcome {
        let mut out = TickOutcome::default();
        self.lifetime += dt;

        if self.lifetime >= Self::MAX_LIFETIME {
            out.despawn = Some(DespawnReason::Lifetime);
            return out;
        }
        if (self.position - player_pos).length() > Self::MAX_DISTANCE_FROM_PLAYER {
            out.despawn = Some(DespawnReason::TooFar);
            return out;
        }

        match &mut self.state {
            BallLifecycleState::InFlight => {
                self.position += self.velocity * dt;
                self.velocity.y += self.gravity * dt;

                let ball_bottom = ground_y + 0.12;
                if self.position.y <= ball_bottom {
                    self.position.y = ball_bottom;
                    if self.velocity.y.abs() > 2.0 && self.bounces < 3 {
                        self.velocity.y = -self.velocity.y * 0.42;
                        self.velocity.x *= 0.75;
                        self.velocity.z *= 0.75;
                        self.bounces += 1;
                    } else {
                        self.velocity = Vec3::ZERO;
                        self.state = BallLifecycleState::RollingOnGround { roll_timer: 0.0 };
                    }
                }
            }
            BallLifecycleState::RollingOnGround { roll_timer } => {
                *roll_timer += dt;
                if *roll_timer >= Self::GROUND_ROLL_DESPAWN_TIME {
                    out.despawn = Some(DespawnReason::RolledOut);
                }
            }
            BallLifecycleState::Absorbing {
                timer,
                ground_y,
                max_shakes,
                will_capture,
            } => {
                *timer += dt;
                if *timer >= Self::ABSORB_TIME {
                    out.event = Some(BallEvent::Absorbed);
                    let (ground_y, max_shakes, will_capture) = (*ground_y, *max_shakes, *will_capture);
                    self.state = BallLifecycleState::Dropping {
                        timer: 0.0,
                        ground_y,
                        max_shakes,
                        will_capture,
                    };
                }
            }
            BallLifecycleState::Dropping {
                timer,
                ground_y,
                max_shakes,
                will_capture,
            } => {
                *timer += dt;
                self.velocity.y += self.gravity * dt;
                self.position += self.velocity * dt;
                let floor = *ground_y + 0.12;
                if self.position.y <= floor || *timer >= Self::DROP_MAX_TIME {
                    self.position.y = floor;
                    self.velocity = Vec3::ZERO;
                    out.event = Some(BallEvent::Dropped);
                    self.state = BallLifecycleState::Shaking {
                        shake_count: 0,
                        timer: 0.0,
                        max_shakes: *max_shakes,
                        will_capture: *will_capture,
                    };
                }
            }
            BallLifecycleState::Shaking { shake_count, timer, max_shakes, will_capture } => {
                *timer += dt;
                if *timer >= Self::SHAKE_INTERVAL {
                    *timer = 0.0;
                    *shake_count += 1;
                    out.event = Some(BallEvent::Shake(*shake_count));
                    if *shake_count >= *max_shakes {
                        if *will_capture {
                            out.event = Some(BallEvent::Captured);
                            self.state = BallLifecycleState::Captured { despawn_timer: 0.0 };
                        } else {
                            out.event = Some(BallEvent::BrokeFree);
                            self.state = BallLifecycleState::Breakout;
                            out.despawn = Some(DespawnReason::BrokeFree);
                        }
                    }
                }
            }
            BallLifecycleState::Captured { despawn_timer } => {
                *despawn_timer += dt;
                if *despawn_timer >= Self::CAPTURE_AURA_DESPAWN_TIME {
                    out.despawn = Some(DespawnReason::CaptureShown);
                }
            }
            BallLifecycleState::Breakout => out.despawn = Some(DespawnReason::BrokeFree),
        }
        out
    }
}

/// Bevy component wrapping the simulation of one thrown ball.
#[derive(Component, Debug, Clone)]
pub struct BallBody(pub BallSimulation);

/// Advance one ball entity: tick the simulation, mirror the position into `xf` and despawn the
/// entity (children included) when the lifecycle ends. The viewer calls this for every ball
/// each frame, so no ball can outlive its lifecycle.
pub fn step_ball(
    commands: &mut Commands,
    entity: Entity,
    xf: &mut Transform,
    body: &mut BallBody,
    dt: f32,
    ground_y: f32,
    player_pos: Vec3,
) -> TickOutcome {
    let out = body.0.tick_events(dt, ground_y, player_pos);
    xf.translation = body.0.position;
    if out.despawn.is_some() {
        commands.entity(entity).despawn();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ball_despawns_after_max_lifetime() {
        let mut ball = BallSimulation::new(Vec3::new(0.0, 10.0, 0.0), Vec3::new(0.0, 0.0, 0.0), 0.0);
        let player = Vec3::ZERO;
        let mut despawned = false;
        // Step in 0.1s increments without hitting ground
        for _ in 0..85 {
            if ball.tick(0.1, -1000.0, player) {
                despawned = true;
                break;
            }
        }
        assert!(despawned);
        assert!(ball.lifetime >= 8.0);
    }

    #[test]
    fn test_ball_despawns_after_distance_limit() {
        let mut ball = BallSimulation::new(Vec3::new(151.0, 1.0, 0.0), Vec3::ZERO, -9.8);
        let player = Vec3::ZERO;
        let despawn = ball.tick(0.016, 0.0, player);
        assert!(despawn);
    }

    #[test]
    fn test_ball_despawns_after_ground_roll() {
        let mut ball = BallSimulation::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.0), -9.8);
        let player = Vec3::ZERO;
        // First tick puts it on ground into RollingOnGround
        assert!(!ball.tick(0.1, 0.0, player));
        assert!(matches!(ball.state, BallLifecycleState::RollingOnGround { .. }));

        // Advance roll timer to 2.1s
        let mut despawned = false;
        for _ in 0..25 {
            if ball.tick(0.1, 0.0, player) {
                despawned = true;
                break;
            }
        }
        assert!(despawned);
    }

    #[test]
    fn test_ball_breakout_despawns_immediately() {
        let mut ball = BallSimulation::new(Vec3::ZERO, Vec3::ZERO, 0.0);
        ball.state = BallLifecycleState::Shaking {
            shake_count: 2,
            timer: 0.64,
            max_shakes: 3,
            will_capture: false,
        };
        let despawn = ball.tick(0.02, 0.0, Vec3::ZERO);
        assert!(despawn);
    }

    #[test]
    fn test_ball_captured_despawns_after_aura() {
        let mut ball = BallSimulation::new(Vec3::ZERO, Vec3::ZERO, 0.0);
        ball.state = BallLifecycleState::Captured { despawn_timer: 0.0 };
        // Tick up to 1.7s -> not yet
        assert!(!ball.tick(1.7, 0.0, Vec3::ZERO));
        // Tick 0.2s more (total 1.9s >= 1.8s) -> despawn!
        assert!(ball.tick(0.2, 0.0, Vec3::ZERO));
    }

    #[test]
    fn test_simulation_pool_no_leaks() {
        // Simulate throwing 50 balls with random trajectories and verify all eventually despawn
        let mut balls: Vec<BallSimulation> = (0..50)
            .map(|i| {
                BallSimulation::new(
                    Vec3::new(0.0, 1.5, 0.0),
                    Vec3::new((i as f32).sin() * 15.0, 5.0, (i as f32).cos() * 15.0),
                    -16.0,
                )
            })
            .collect();

        let dt = 0.05;
        let mut total_time = 0.0;
        while !balls.is_empty() && total_time < 12.0 {
            balls.retain_mut(|b| !b.tick(dt, 0.0, Vec3::ZERO));
            total_time += dt;
        }

        // Must have 0 remaining balls
        assert_eq!(balls.len(), 0, "All balls must despawn without entity leaks");
    }

    #[test]
    fn test_hit_then_capture_shows_result_then_despawns() {
        let mut ball = BallSimulation::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), -16.0);
        ball.hit_pokemon(Vec3::new(3.0, 0.9, 0.0), 0.0, 3, true);
        assert!(!ball.in_flight());
        let mut events = Vec::new();
        let mut reason = None;
        for _ in 0..400 {
            let o = ball.tick_events(0.016, 0.0, Vec3::ZERO);
            events.extend(o.event);
            if o.despawn.is_some() {
                reason = o.despawn;
                break;
            }
        }
        assert_eq!(
            events,
            vec![
                BallEvent::Absorbed,
                BallEvent::Dropped,
                BallEvent::Shake(1),
                BallEvent::Shake(2),
                // The 3rd shake interval is the one that ends in the click.
                BallEvent::Captured
            ]
        );
        assert_eq!(reason, Some(DespawnReason::CaptureShown));
    }

    #[test]
    fn test_absorb_and_drop_timings_and_landing() {
        let mut ball = BallSimulation::new(Vec3::ZERO, Vec3::ZERO, -16.0);
        ball.hit_pokemon(Vec3::new(0.0, 1.2, 0.0), 0.0, 3, false);
        // Absorb takes ABSORB_TIME (ASSUMED), then the ball starts to drop.
        let mut t = 0.0;
        let mut saw_absorb = false;
        while !saw_absorb {
            let o = ball.tick_events(0.01, 0.0, Vec3::ZERO);
            t += 0.01;
            saw_absorb = o.event == Some(BallEvent::Absorbed);
        }
        assert!((t - BallSimulation::ABSORB_TIME).abs() < 0.02, "absorb lasted {t}");
        // Dropping: ball falls and lands at ground + 0.12 where the shakes start.
        let mut saw_drop = false;
        while !saw_drop {
            let o = ball.tick_events(0.01, 0.0, Vec3::ZERO);
            saw_drop = o.event == Some(BallEvent::Dropped);
        }
        assert!((ball.position.y - 0.12).abs() < 1e-3, "landed at {}", ball.position.y);
        assert!(matches!(ball.state, BallLifecycleState::Shaking { .. }));
    }

    #[test]
    fn test_hit_then_failure_breaks_free_and_despawns() {
        let mut ball = BallSimulation::new(Vec3::ZERO, Vec3::ZERO, -16.0);
        // A failed roll with one passed check breaks free after two shakes.
        ball.hit_pokemon(Vec3::new(3.0, 0.9, 0.0), 0.0, 2, false);
        let mut events = Vec::new();
        let mut last = TickOutcome::default();
        for _ in 0..400 {
            last = ball.tick_events(0.016, 0.0, Vec3::ZERO);
            events.extend(last.event);
            if last.despawn.is_some() {
                break;
            }
        }
        assert_eq!(
            events,
            vec![
                BallEvent::Absorbed,
                BallEvent::Dropped,
                BallEvent::Shake(1),
                // The 2nd shake interval is the one that breaks free.
                BallEvent::BrokeFree
            ]
        );
        assert_eq!(last.despawn, Some(DespawnReason::BrokeFree));
    }

    #[test]
    fn test_ground_roll_reason_and_time_window() {
        let mut ball = BallSimulation::new(Vec3::new(0.0, 0.12, 0.0), Vec3::new(0.0, -1.0, 0.0), -16.0);
        let mut t = 0.0;
        let mut reason = None;
        while t < 10.0 {
            let o = ball.tick_events(0.05, 0.0, Vec3::ZERO);
            t += 0.05;
            if o.despawn.is_some() {
                reason = o.despawn;
                break;
            }
        }
        assert_eq!(reason, Some(DespawnReason::RolledOut));
        assert!((1.5..=3.0).contains(&t), "rolled for {t}s");
    }

    /// Entity-level proof: throw many balls (flying off, grounded, captured, broken out, too far)
    /// and check that no `BallBody` entity survives the simulation.
    #[test]
    fn test_no_ball_entities_remain_after_many_throws() {
        fn step_all(mut commands: Commands, mut q: Query<(Entity, &mut Transform, &mut BallBody)>) {
            for (e, mut xf, mut body) in &mut q {
                step_ball(&mut commands, e, &mut xf, &mut body, 0.05, 0.0, Vec3::ZERO);
            }
        }
        let mut app = App::new();
        app.add_systems(Update, step_all);
        for i in 0..60u32 {
            let ang = i as f32 * 0.7;
            let mut sim = BallSimulation::new(
                Vec3::new(0.0, 1.5, 0.0),
                Vec3::new(ang.sin() * 30.0, 6.0, ang.cos() * 30.0),
                -16.0,
            );
            match i % 4 {
                1 => sim.hit_pokemon(Vec3::new(2.0, 0.9, 2.0), 0.0, 3, true),
                2 => sim.hit_pokemon(Vec3::new(2.0, 0.9, 2.0), 0.0, 2, false),
                3 => sim.velocity = Vec3::new(400.0, 0.0, 0.0),
                _ => {}
            }
            app.world_mut().spawn((Transform::from_translation(sim.position), BallBody(sim)));
        }
        let count = |app: &mut App| app.world_mut().query::<&BallBody>().iter(app.world()).count();
        assert_eq!(count(&mut app), 60);
        for _ in 0..400 {
            app.update();
        }
        assert_eq!(count(&mut app), 0, "balls leaked");
    }
}
