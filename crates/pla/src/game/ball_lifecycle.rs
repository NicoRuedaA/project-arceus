//! Pure ballistics and life-cycle logic for Poké Balls.
//!
//! Provides deterministic state updates, decay timers, despawn conditions,
//! and bounce limits without Bevy dependencies.

use bevy::prelude::Vec3;

/// Ball state enum mirroring in-game ball trajectory and capture animation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BallLifecycleState {
    InFlight,
    RollingOnGround { roll_timer: f32 },
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

impl BallSimulation {
    pub const MAX_LIFETIME: f32 = 8.0;
    pub const MAX_DISTANCE_FROM_PLAYER: f32 = 150.0;
    pub const GROUND_ROLL_DESPAWN_TIME: f32 = 2.0;
    pub const CAPTURE_AURA_DESPAWN_TIME: f32 = 1.8;

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

    /// Advance simulation by `dt` seconds given terrain ground height at the current X/Z.
    /// Returns `true` if the ball should be despawned.
    pub fn tick(&mut self, dt: f32, ground_y: f32, player_pos: Vec3) -> bool {
        self.lifetime += dt;

        // Despawn if lifetime exceeds maximum or distance from player exceeds 150m
        if self.lifetime >= Self::MAX_LIFETIME {
            return true;
        }
        if (self.position - player_pos).length() > Self::MAX_DISTANCE_FROM_PLAYER {
            return true;
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
                false
            }
            BallLifecycleState::RollingOnGround { roll_timer } => {
                *roll_timer += dt;
                *roll_timer >= Self::GROUND_ROLL_DESPAWN_TIME
            }
            BallLifecycleState::Shaking {
                shake_count,
                timer,
                max_shakes,
                will_capture,
            } => {
                *timer += dt;
                if *timer >= 0.65 {
                    *timer = 0.0;
                    *shake_count += 1;
                    if *shake_count >= *max_shakes {
                        if *will_capture {
                            self.state = BallLifecycleState::Captured { despawn_timer: 0.0 };
                        } else {
                            self.state = BallLifecycleState::Breakout;
                            return true; // Despawn immediately on breakout
                        }
                    }
                }
                false
            }
            BallLifecycleState::Captured { despawn_timer } => {
                *despawn_timer += dt;
                *despawn_timer >= Self::CAPTURE_AURA_DESPAWN_TIME
            }
            BallLifecycleState::Breakout => true,
        }
    }
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
}
