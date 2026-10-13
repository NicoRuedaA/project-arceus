//! Play-time counter: whole seconds accumulated while the player is in the world or a battle,
//! never while paused. Persisted as `PlayerSave::play_seconds` (save format v4).
//!
//! The original game's clock and its wall-clock correlation are **unknown** (the decoded
//! time-of-day code in `reports/function-progress/n2-base-camps-and-travel.md` is a frame
//! accumulator that drives the lighting blend, not a real-time counter), so this port simply
//! counts real seconds. It does not advance the day/night slot; Brief 9 (clock/weather) owns
//! that and will take save format v5.

use bevy::prelude::*;

use super::state::{not_paused, GameState, Paused};

/// Seconds played since the current game started, plus the sub-second remainder.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct PlayTime {
    /// Whole seconds already accumulated (this is what the save stores).
    pub seconds: u64,
    /// Sub-second remainder; never negative and always < 1.
    fraction: f64,
}

impl PlayTime {
    /// A counter seeded from a loaded save.
    pub fn from_seconds(seconds: u64) -> Self {
        Self { seconds, fraction: 0.0 }
    }

    /// Accumulate one frame delta in seconds. NaN, infinite, zero and negative deltas are
    /// ignored, and the total saturates instead of overflowing.
    pub fn accumulate(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.fraction += f64::from(dt);
        let whole = self.fraction.trunc();
        if whole >= 1.0 {
            self.seconds = self.seconds.saturating_add(whole as u64);
            self.fraction -= whole;
        }
    }
}

/// `H:MM:SS` text for the pause menu (hours are not wrapped or padded).
pub fn format_play_time(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    format!("{h}:{m:02}:{s:02}")
}

/// Run condition: only `World` and `Battle` count, and never while paused. Missing resources
/// count as "running", so the counter still works in tiny test apps (see [`not_paused`]).
pub fn counts_play_time(state: Option<Res<State<GameState>>>, paused: Option<Res<Paused>>) -> bool {
    let in_game = state.is_none_or(|s| matches!(*s.get(), GameState::World | GameState::Battle));
    in_game && not_paused(paused)
}

fn tick_play_time(time: Res<Time>, mut play: ResMut<PlayTime>) {
    play.accumulate(time.delta_secs());
}

/// Installs the [`PlayTime`] resource (seeded from the save) and its tick system.
pub struct PlayTimePlugin;

impl PlayTimePlugin {
    pub fn install(app: &mut App, start_seconds: u64) {
        app.insert_resource(PlayTime::from_seconds(start_seconds))
            .add_systems(Update, tick_play_time.run_if(counts_play_time));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    #[test]
    fn accumulates_whole_seconds_and_keeps_the_fraction() {
        let mut p = PlayTime::default();
        for _ in 0..60 {
            p.accumulate(1.0 / 60.0);
        }
        assert_eq!(p.seconds, 1);
        p.accumulate(0.5);
        assert_eq!(p.seconds, 1, "half a second is not a whole second yet");
        p.accumulate(0.5);
        assert_eq!(p.seconds, 2);
    }

    #[test]
    fn ignores_bad_deltas_and_saturates() {
        let mut p = PlayTime::from_seconds(10);
        p.accumulate(f32::NAN);
        p.accumulate(f32::INFINITY);
        p.accumulate(-5.0);
        p.accumulate(0.0);
        assert_eq!(p.seconds, 10);
        let mut p = PlayTime::from_seconds(u64::MAX);
        p.accumulate(2.0);
        assert_eq!(p.seconds, u64::MAX);
    }

    #[test]
    fn formats_hours_minutes_seconds() {
        assert_eq!(format_play_time(0), "0:00:00");
        assert_eq!(format_play_time(59), "0:00:59");
        assert_eq!(format_play_time(60), "0:01:00");
        assert_eq!(format_play_time(3661), "1:01:01");
        assert_eq!(format_play_time(360_000), "100:00:00");
    }

    #[test]
    fn run_condition_counts_only_world_and_battle_outside_pause() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        assert!(app.world_mut().run_system_cached(counts_play_time).unwrap(), "World counts by default");
        app.insert_resource(Paused(true));
        assert!(!app.world_mut().run_system_cached(counts_play_time).unwrap(), "paused never counts");
        app.insert_resource(Paused(false));
        app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::BattleEnded);
        app.update();
        assert!(!app.world_mut().run_system_cached(counts_play_time).unwrap(), "BattleEnded does not count");
        app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Battle);
        app.update();
        assert!(app.world_mut().run_system_cached(counts_play_time).unwrap(), "Battle counts again");
    }

    /// Headless Bevy test of the real system: a fixed 250 ms delta per update (`Time<Virtual>`
    /// clamps its delta to 250 ms by default, so this is the largest clean step).
    #[test]
    fn ticks_in_world_and_battle_but_never_paused_or_post_battle() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<GameState>()
            .init_resource::<Paused>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(250)));
        PlayTimePlugin::install(&mut app, 0);

        app.update(); // warm-up frame: `Time<Real>` seeds its first instant, delta is 0
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<PlayTime>().seconds, 0, "0.75 s is still a fraction");
        app.update();
        assert_eq!(app.world().resource::<PlayTime>().seconds, 1, "a full second is counted");

        app.world_mut().resource_mut::<Paused>().0 = true;
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(app.world().resource::<PlayTime>().seconds, 1, "paused time never counts");

        app.world_mut().resource_mut::<Paused>().0 = false;
        app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::BattleEnded);
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(app.world().resource::<PlayTime>().seconds, 1, "BattleEnded never counts");

        app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Battle);
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(app.world().resource::<PlayTime>().seconds, 2, "Battle counts again");
    }
}
