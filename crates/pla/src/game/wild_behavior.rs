//! Deterministic wild-Pokémon reaction state machine.
//!
//! The state machine is deliberately independent from Bevy and from the battle engine. Numeric
//! defaults in [`BehaviorParams::assumed`] are port policy, not decoded game data; the evidence
//! ledger in `data/wild_behavior.tsv` keeps that distinction explicit.
//!
//! Sleep: the game keeps a sleeping state for wild Pokémon (capture ×4 while sleeping in
//! `common_capture_config.bin`, a dedicated player face reaction keyed `wild_poke_sleep_base`
//! and native AI states 4/7 in `FUN_024e26b4`), but no hour window or probability lives in the
//! 104 app-config tables. The port follows the external notes (PROBABLE): a creature that
//! finishes a patrol leg may fall asleep for 30–60 s; only the species the game gives the
//! "caught while they were sleeping" research task (task kind 11, see [`SLEEP_TASK_SPECIES`])
//! are eligible. All the timing constants are declared next to [`SLEEP_CYCLE_CHANCE`].

use bevy::prelude::Component;

/// Coarse archetypes used by the port until species-specific behavior data is recovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Archetype {
    Passive,
    Timid,
    Aggressive,
}

/// Pure wild reaction states. The names are port-facing labels, not a claim that every native
/// state ID has been decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Wander,
    Sleep,
    Alert,
    Flee,
    Chase,
    Attack,
}

/// Player noise supplied by a caller that has locomotion information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Noise {
    Silent,
    Walk,
    Run,
}

/// Inputs to one deterministic perception tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Perception {
    pub distance: f32,
    /// Absolute angle between the creature's forward direction and the player, in radians.
    pub angle: f32,
    pub noise: Noise,
    /// True means the player is concealed and cannot be perceived by this policy.
    pub player_hidden: bool,
    /// Optional 24-hour clock for the (unset-by-default) hour-window sleep policy. The patrol
    /// sleep does not need it.
    pub hour: Option<f32>,
}

impl Perception {
    pub const fn quiet(distance: f32) -> Self {
        Self {
            distance,
            angle: 0.0,
            noise: Noise::Silent,
            player_hidden: false,
            hour: None,
        }
    }
}

/// PROBABLE (external notes, not present in the game's config tables): chance that a creature
/// falling out of a completed patrol leg (Wander → Idle) dozes off. The notes give 10–15 %;
/// the midpoint is used.
pub const SLEEP_CYCLE_CHANCE: f32 = 0.125;
/// PROBABLE (external notes): sleep lasts 30–60 s.
pub const SLEEP_DURATION_MIN_S: f32 = 30.0;
pub const SLEEP_DURATION_MAX_S: f32 = 60.0;

/// Species with the game's "Number you've caught while they were sleeping" research task
/// (`data/dex_tasks.tsv` rows with kind 11; cross-checked against the public task lists).
/// These are the only per-species sleep evidence in the game tables, so the port limits the
/// random patrol sleep to them. Sorted.
pub const SLEEP_TASK_SPECIES: [u16; 11] =
    [37, 58, 108, 113, 143, 172, 173, 175, 239, 240, 417];

/// Whether a species may fall asleep during patrol (see [`SLEEP_TASK_SPECIES`]).
pub fn sleep_capable(species: u16) -> bool {
    SLEEP_TASK_SPECIES.contains(&species)
}

/// Tunable state-machine parameters. Values from [`assumed`](Self::assumed) are provisional.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BehaviorParams {
    pub archetype: Archetype,
    pub vision_distance: f32,
    pub vision_half_angle: f32,
    pub hearing_distance: f32,
    pub alert_duration_s: f32,
    pub flee_duration_s: f32,
    pub chase_distance: f32,
    pub attack_distance: f32,
    pub sleep_window: Option<(f32, f32)>,
    /// Species eligibility for the patrol sleep (see [`sleep_capable`]).
    pub can_sleep: bool,
    /// Chance to fall asleep when a patrol leg completes (0 disables the roll).
    pub sleep_chance: f32,
    /// Sleep duration draw bounds, in seconds.
    pub sleep_min_s: f32,
    pub sleep_max_s: f32,
}

impl BehaviorParams {
    /// Port defaults used only when no species-specific behavior record is available.
    pub const fn assumed(archetype: Archetype) -> Self {
        match archetype {
            Archetype::Passive => Self {
                archetype,
                vision_distance: 10.0,
                vision_half_angle: std::f32::consts::FRAC_PI_3,
                hearing_distance: 6.0,
                alert_duration_s: 2.0,
                flee_duration_s: 2.0,
                chase_distance: 0.0,
                attack_distance: 0.0,
                sleep_window: None,
                can_sleep: false,
                sleep_chance: SLEEP_CYCLE_CHANCE,
                sleep_min_s: SLEEP_DURATION_MIN_S,
                sleep_max_s: SLEEP_DURATION_MAX_S,
            },
            Archetype::Timid => Self {
                archetype,
                vision_distance: 12.0,
                vision_half_angle: 1.570_796_4,
                hearing_distance: 10.0,
                alert_duration_s: 0.75,
                flee_duration_s: 4.0,
                chase_distance: 0.0,
                attack_distance: 0.0,
                sleep_window: None,
                can_sleep: false,
                sleep_chance: SLEEP_CYCLE_CHANCE,
                sleep_min_s: SLEEP_DURATION_MIN_S,
                sleep_max_s: SLEEP_DURATION_MAX_S,
            },
            Archetype::Aggressive => Self {
                archetype,
                vision_distance: 14.0,
                vision_half_angle: 1.570_796_4,
                hearing_distance: 10.0,
                alert_duration_s: 0.5,
                flee_duration_s: 0.0,
                chase_distance: 18.0,
                attack_distance: 2.5,
                sleep_window: None,
                can_sleep: false,
                sleep_chance: SLEEP_CYCLE_CHANCE,
                sleep_min_s: SLEEP_DURATION_MIN_S,
                sleep_max_s: SLEEP_DURATION_MAX_S,
            },
        }
    }
}

/// Result of one state-machine tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepResult {
    pub previous: State,
    pub current: State,
    pub changed: bool,
}

/// Mutable deterministic state. The seed drives the evidence-backed patrol-sleep roll.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Machine {
    pub seed: u64,
    pub state: State,
    pub state_time_s: f32,
    /// Remaining patrol sleep in seconds (0 = not a timed sleep).
    pub sleep_remaining_s: f32,
}

impl Machine {
    pub const fn new(seed: u64) -> Self {
        Self {
            seed,
            state: State::Idle,
            state_time_s: 0.0,
            sleep_remaining_s: 0.0,
        }
    }

    /// Deterministic uniform draw in `[0, 1)` from the machine seed (LCG step).
    fn next_unit(&mut self) -> f64 {
        self.seed = self
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.seed >> 33) as f64 / (1u64 << 31) as f64
    }

    /// Draw the patrol sleep: eligible species only, one roll per completed patrol leg.
    fn try_fall_asleep(&mut self, params: BehaviorParams) {
        if !params.can_sleep || !params.sleep_chance.is_finite() || params.sleep_chance <= 0.0 {
            return;
        }
        let roll = self.next_unit();
        if roll >= f64::from(params.sleep_chance.clamp(0.0, 1.0)) {
            return;
        }
        let lo = params.sleep_min_s.max(0.0);
        let hi = params.sleep_max_s.max(lo);
        let draw = lo + ((hi - lo) as f64 * self.next_unit()) as f32;
        self.sleep_remaining_s = draw;
        self.set_state(State::Sleep);
    }

    /// Advance one tick. Non-finite or negative deltas are treated as zero.
    pub fn step(&mut self, dt: f32, params: BehaviorParams, perception: Perception) -> StepResult {
        let previous = self.state;
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.state_time_s = (self.state_time_s + dt).min(86_400.0);
        let noticed = perceived(&perception, params);

        if matches!(self.state, State::Idle | State::Wander)
            && sleeping_at(perception.hour, params.sleep_window)
        {
            self.set_state(State::Sleep);
        }

        match self.state {
            State::Idle | State::Wander => {
                if noticed {
                    self.set_state(reaction_state(params, perception));
                }
            }
            State::Sleep => {
                if noticed {
                    self.set_state(State::Alert);
                } else if params.sleep_window.is_some()
                    && !sleeping_at(perception.hour, params.sleep_window)
                {
                    self.set_state(State::Idle);
                } else if self.sleep_remaining_s > 0.0 {
                    self.sleep_remaining_s = (self.sleep_remaining_s - dt).max(0.0);
                    if self.sleep_remaining_s == 0.0 {
                        self.set_state(State::Idle);
                    }
                } else if params.sleep_window.is_none() {
                    // Timed sleep with an exhausted timer cannot persist.
                    self.set_state(State::Idle);
                }
            }
            State::Alert => {
                if noticed {
                    match params.archetype {
                        Archetype::Aggressive
                            if valid_distance(perception.distance) <= params.attack_distance =>
                        {
                            self.set_state(State::Attack);
                        }
                        Archetype::Aggressive => self.set_state(State::Chase),
                        Archetype::Timid => self.set_state(State::Flee),
                        Archetype::Passive if self.state_time_s >= params.alert_duration_s => {
                            self.set_state(State::Idle);
                        }
                        Archetype::Passive => {}
                    }
                } else if self.state_time_s >= params.alert_duration_s {
                    self.set_state(State::Idle);
                }
            }
            State::Flee => {
                if self.state_time_s >= params.flee_duration_s {
                    self.set_state(State::Idle);
                }
            }
            State::Chase => {
                if !noticed || valid_distance(perception.distance) > params.chase_distance {
                    self.set_state(State::Idle);
                } else if valid_distance(perception.distance) <= params.attack_distance {
                    self.set_state(State::Attack);
                }
            }
            State::Attack => {
                if !noticed {
                    self.set_state(State::Idle);
                } else if valid_distance(perception.distance) > params.attack_distance {
                    self.set_state(State::Chase);
                }
            }
        }

        StepResult {
            previous,
            current: self.state,
            changed: previous != self.state,
        }
    }

    /// Mirrors the already-existing wander loop without overwriting reaction states. A
    /// Wander → Idle edge is a completed patrol leg and rolls the patrol sleep once.
    pub fn sync_wander_phase(&mut self, walking: bool, params: BehaviorParams) {
        let target = if walking { State::Wander } else { State::Idle };
        if matches!(self.state, State::Idle | State::Wander) && self.state != target {
            let completed_patrol = self.state == State::Wander && target == State::Idle;
            self.set_state(target);
            if completed_patrol {
                self.try_fall_asleep(params);
            }
        }
    }

    fn set_state(&mut self, state: State) {
        if self.state != state {
            self.state = state;
            self.state_time_s = 0.0;
            if state != State::Sleep {
                self.sleep_remaining_s = 0.0;
            }
        }
    }
}

/// Component attached when an existing wild creature receives wander state.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct WildBehavior {
    pub machine: Machine,
    pub params: BehaviorParams,
}

impl WildBehavior {
    pub const fn assumed_passive(seed: u64) -> Self {
        Self {
            machine: Machine::new(seed),
            params: BehaviorParams::assumed(Archetype::Passive),
        }
    }
}

/// Pure encounter-facing projection. Wiring this into the viewer requires a real stealth/noise
/// input path; it is intentionally not called by the contested viewer encounter system yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncounterIntent {
    Ignore,
    Engage { surprise: bool },
}

pub fn encounter_intent(
    state: State,
    player_hidden: bool,
    distance: f32,
    attack_distance: f32,
) -> EncounterIntent {
    if player_hidden || !distance.is_finite() || distance < 0.0 {
        return EncounterIntent::Ignore;
    }
    match state {
        State::Attack if distance <= attack_distance.max(0.0) => {
            EncounterIntent::Engage { surprise: true }
        }
        State::Alert | State::Chase | State::Attack => EncounterIntent::Engage { surprise: false },
        State::Idle | State::Wander | State::Sleep | State::Flee => EncounterIntent::Ignore,
    }
}

fn valid_distance(distance: f32) -> f32 {
    if distance.is_finite() {
        distance.max(0.0)
    } else {
        f32::INFINITY
    }
}

fn perceived(perception: &Perception, params: BehaviorParams) -> bool {
    if perception.player_hidden {
        return false;
    }
    let distance = valid_distance(perception.distance);
    let angle = if perception.angle.is_finite() {
        perception.angle.abs()
    } else {
        f32::INFINITY
    };
    let visible = distance <= params.vision_distance.max(0.0)
        && angle <= params.vision_half_angle.clamp(0.0, std::f32::consts::PI);
    let heard = perception.noise != Noise::Silent && distance <= params.hearing_distance.max(0.0);
    visible || heard
}

fn sleeping_at(hour: Option<f32>, window: Option<(f32, f32)>) -> bool {
    let Some(hour) = hour.filter(|hour| hour.is_finite()) else {
        return false;
    };
    let Some((start, end)) = window.filter(|(start, end)| start.is_finite() && end.is_finite())
    else {
        return false;
    };
    let hour = hour.rem_euclid(24.0);
    let start = start.rem_euclid(24.0);
    let end = end.rem_euclid(24.0);
    if start <= end {
        hour >= start && hour < end
    } else {
        hour >= start || hour < end
    }
}

fn reaction_state(params: BehaviorParams, perception: Perception) -> State {
    match params.archetype {
        Archetype::Passive => State::Alert,
        Archetype::Timid => State::Flee,
        Archetype::Aggressive if valid_distance(perception.distance) <= params.attack_distance => {
            State::Attack
        }
        Archetype::Aggressive => State::Alert,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_player_is_not_perceived() {
        let params = BehaviorParams::assumed(Archetype::Aggressive);
        let mut machine = Machine::new(1);
        let mut perception = Perception::quiet(1.0);
        perception.player_hidden = true;
        let result = machine.step(1.0, params, perception);
        assert_eq!(result.current, State::Idle);
    }

    #[test]
    fn angle_and_distance_gate_vision() {
        let params = BehaviorParams::assumed(Archetype::Passive);
        let mut machine = Machine::new(2);
        let mut perception = Perception::quiet(5.0);
        perception.angle = std::f32::consts::PI;
        assert_eq!(machine.step(0.1, params, perception).current, State::Idle);
        perception.angle = 0.0;
        assert_eq!(machine.step(0.1, params, perception).current, State::Alert);
    }

    #[test]
    fn noise_reaches_a_creature_outside_vision_cone() {
        let params = BehaviorParams::assumed(Archetype::Passive);
        let mut machine = Machine::new(3);
        let perception = Perception {
            distance: 5.0,
            angle: std::f32::consts::PI,
            noise: Noise::Run,
            player_hidden: false,
            hour: None,
        };
        assert_eq!(machine.step(0.1, params, perception).current, State::Alert);
    }

    #[test]
    fn archetypes_react_differently() {
        let perception = Perception::quiet(2.0);
        let mut passive = Machine::new(4);
        let mut timid = Machine::new(4);
        let mut aggressive = Machine::new(4);
        assert_eq!(
            passive
                .step(0.1, BehaviorParams::assumed(Archetype::Passive), perception)
                .current,
            State::Alert
        );
        assert_eq!(
            timid
                .step(0.1, BehaviorParams::assumed(Archetype::Timid), perception)
                .current,
            State::Flee
        );
        assert_eq!(
            aggressive
                .step(
                    0.1,
                    BehaviorParams::assumed(Archetype::Aggressive),
                    perception
                )
                .current,
            State::Attack
        );
    }

    #[test]
    fn sleep_window_wraps_midnight_and_wakes() {
        let mut machine = Machine::new(5);
        let params = BehaviorParams {
            sleep_window: Some((22.0, 6.0)),
            ..BehaviorParams::assumed(Archetype::Passive)
        };
        assert_eq!(
            machine
                .step(
                    0.1,
                    params,
                    Perception {
                        hour: Some(23.0),
                        ..Perception::quiet(50.0)
                    }
                )
                .current,
            State::Sleep
        );
        assert_eq!(
            machine
                .step(
                    0.1,
                    params,
                    Perception {
                        hour: Some(12.0),
                        ..Perception::quiet(50.0)
                    }
                )
                .current,
            State::Idle
        );
    }

    #[test]
    fn alert_times_out_without_new_perception() {
        let params = BehaviorParams::assumed(Archetype::Passive);
        let mut machine = Machine::new(6);
        machine.step(0.1, params, Perception::quiet(1.0));
        assert_eq!(
            machine.step(2.0, params, Perception::quiet(50.0)).current,
            State::Idle
        );
    }

    #[test]
    fn aggressive_chases_then_attacks() {
        let params = BehaviorParams::assumed(Archetype::Aggressive);
        let mut machine = Machine::new(7);
        let far = Perception::quiet(10.0);
        assert_eq!(machine.step(0.1, params, far).current, State::Alert);
        assert_eq!(machine.step(0.1, params, far).current, State::Chase);
        assert_eq!(
            machine.step(0.1, params, Perception::quiet(2.0)).current,
            State::Attack
        );
    }

    #[test]
    fn same_seed_and_inputs_are_reproducible() {
        let params = BehaviorParams::assumed(Archetype::Timid);
        let inputs = [
            Perception::quiet(20.0),
            Perception::quiet(2.0),
            Perception::quiet(20.0),
        ];
        let run = || {
            let mut machine = Machine::new(0x1234);
            inputs.map(|input| machine.step(0.25, params, input).current)
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn sleep_species_match_the_game_task_table() {
        let mut species: Vec<u16> = include_str!("../../../../data/dex_tasks.tsv")
            .lines()
            .filter(|line| !line.starts_with('#') && !line.starts_with("species:"))
            .filter_map(|line| {
                let mut fields = line.split('\t');
                let species = fields.next()?.parse::<u16>().ok()?;
                let _slot = fields.next()?;
                let kind = fields.next()?.parse::<u8>().ok()?;
                (kind == 11).then_some(species)
            })
            .collect();
        species.sort_unstable();
        species.dedup();
        assert_eq!(species, SLEEP_TASK_SPECIES.to_vec());
        assert!(sleep_capable(143)); // Snorlax
        assert!(!sleep_capable(399)); // Bidoof
    }

    #[test]
    fn completed_patrol_sleeps_a_capable_creature_until_the_timer_runs_out() {
        let params = BehaviorParams {
            can_sleep: true,
            sleep_chance: 1.0,
            sleep_min_s: 30.0,
            sleep_max_s: 60.0,
            ..BehaviorParams::assumed(Archetype::Passive)
        };
        let mut machine = Machine::new(11);
        machine.sync_wander_phase(true, params);
        assert_eq!(machine.state, State::Wander);
        machine.sync_wander_phase(false, params);
        assert_eq!(machine.state, State::Sleep);
        assert!((30.0..=60.0).contains(&machine.sleep_remaining_s));
        // A quiet, distant player does not wake it; the timer does.
        assert_eq!(
            machine.step(1.0, params, Perception::quiet(50.0)).current,
            State::Sleep
        );
        assert_eq!(
            machine
                .step(120.0, params, Perception::quiet(50.0))
                .current,
            State::Idle
        );
        assert_eq!(machine.sleep_remaining_s, 0.0);
    }

    #[test]
    fn sleep_roll_is_deterministic_and_gated_by_eligibility() {
        let eligible = BehaviorParams {
            can_sleep: true,
            sleep_chance: 0.125,
            ..BehaviorParams::assumed(Archetype::Passive)
        };
        let run = || {
            let mut machine = Machine::new(0x51ee);
            machine.sync_wander_phase(true, eligible);
            machine.sync_wander_phase(false, eligible);
            machine.state
        };
        assert_eq!(run(), run(), "same seed must reproduce the roll");

        let ineligible = BehaviorParams {
            can_sleep: false,
            ..eligible
        };
        let mut machine = Machine::new(11);
        machine.sync_wander_phase(true, ineligible);
        machine.sync_wander_phase(false, ineligible);
        assert_eq!(machine.state, State::Idle);

        let disabled = BehaviorParams {
            sleep_chance: 0.0,
            ..eligible
        };
        let mut machine = Machine::new(11);
        machine.sync_wander_phase(true, disabled);
        machine.sync_wander_phase(false, disabled);
        assert_eq!(machine.state, State::Idle);
    }

    #[test]
    fn noticed_player_wakes_a_sleeping_creature() {
        let params = BehaviorParams {
            can_sleep: true,
            sleep_chance: 1.0,
            ..BehaviorParams::assumed(Archetype::Passive)
        };
        let mut machine = Machine::new(12);
        machine.sync_wander_phase(true, params);
        machine.sync_wander_phase(false, params);
        assert_eq!(machine.state, State::Sleep);
        assert_eq!(
            machine.step(0.1, params, Perception::quiet(1.0)).current,
            State::Alert
        );
    }

    #[test]
    fn encounter_intent_keeps_stealth_out_of_battle_start() {
        assert_eq!(
            encounter_intent(State::Attack, true, 1.0, 2.5),
            EncounterIntent::Ignore
        );
        assert_eq!(
            encounter_intent(State::Attack, false, 1.0, 2.5),
            EncounterIntent::Engage { surprise: true }
        );
        assert_eq!(
            encounter_intent(State::Wander, false, 1.0, 2.5),
            EncounterIntent::Ignore
        );
    }
}
