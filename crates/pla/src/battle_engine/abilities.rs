//! The two abilities that have an effect in this game's battle code: Slow Start (112,
//! Regigigas) and Flower Gift (122, Cherrim, form change only). Opt-in through
//! [`Battle::set_abilities`]; without it the engine behaves exactly as before.
//!
//! Evidence (`reports/data-progress/abilities_held_items.md`, static reading of update
//! v262144): the game's ability handler table has exactly two entries. Slow Start halves a
//! Q12 value on the speed side and on physical moves while a per-Pokemon counter is below a
//! limit that is compared with 4; Flower Gift (species 421) only asks for the sunny form
//! change. The exact event meanings and the moment the counter grows are UNKNOWN here; the
//! 5-turn duration, the physical-power target and the end-of-turn growth are PROBABLE.

use super::{Battle, BattleWeather};

/// Ability ids with a live handler in this game's battle code (VERIFIED).
pub const SLOW_START: u16 = 112;
pub const FLOWER_GIFT: u16 = 122;
/// Species whose Flower Gift asks for the form change (VERIFIED).
pub const CHERRIM_DEX: u16 = 421;
/// Turns the halving lasts from the first turn the Pokemon is on the field
/// (PROBABLE: a counter compared with 4 fits five turns).
pub const SLOW_START_TURNS: u8 = 5;
/// Q12 multiplier for x0.5.
pub(crate) const HALF_Q12: u32 = 2048;

/// Opt-in switches. `Default` (everything off) leaves the engine unchanged.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AbilityRules {
    /// Regigigas halves its speed and its physical move power for the first turns.
    pub slow_start: bool,
    /// Cherrim asks for its sunny form while the weather is sun.
    pub flower_gift: bool,
}

impl AbilityRules {
    /// Both live abilities on.
    pub fn game_rules() -> Self {
        Self {
            slow_start: true,
            flower_gift: true,
        }
    }

    /// Comparison switch for verification captures: `PLA_ABILITIES=off` turns both rules off
    /// (anything else keeps the game rules). The engine itself stays opt-in; this only selects
    /// what a new battle session turns on.
    pub fn from_env() -> Self {
        match std::env::var("PLA_ABILITIES").ok().as_deref() {
            Some("off") | Some("legacy") | Some("0") | Some("false") => Self::default(),
            _ => Self::game_rules(),
        }
    }
}

/// Events produced by the ability rules, kept apart from `Event` (exhaustive matches
/// elsewhere).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbilityEvent {
    /// Slow Start stopped halving this combatant (its counter reached the limit).
    SlowStartEnded { combatant: usize },
    /// Form change the rules ask for; the caller plays the model change (Cherrim 421:
    /// form 1 with sun, form 0 otherwise). No stat change is applied (PROBABLE: the game
    /// code only requests the form).
    FormChangeRequested { combatant: usize, form: u8 },
}

/// Per-battle ability state.
#[derive(Debug, Clone, Default)]
pub(super) struct AbilityState {
    rules: AbilityRules,
    /// Turns elapsed per combatant while Slow Start is on the field (the game's counter).
    slow_start_counter: Vec<u8>,
    /// Last form requested per combatant (0 = base form).
    requested_form: Vec<u8>,
    events: Vec<AbilityEvent>,
}

impl Battle {
    /// Turn the two live ability rules on (or off with `AbilityRules::default()`). The Slow
    /// Start counter of every combatant restarts at zero.
    pub fn set_abilities(&mut self, rules: AbilityRules) {
        self.abilities.rules = rules;
        self.abilities.slow_start_counter = vec![0; self.combatants.len()];
        self.abilities.requested_form = vec![0; self.combatants.len()];
        self.ability_refresh_forms();
    }

    pub fn ability_rules(&self) -> AbilityRules {
        self.abilities.rules
    }

    /// Drain the events queued by the ability rules.
    pub fn take_ability_events(&mut self) -> Vec<AbilityEvent> {
        std::mem::take(&mut self.abilities.events)
    }

    /// Slow Start is halving this combatant's speed and physical move power right now.
    pub(super) fn slow_start_active(&self, combatant: usize) -> bool {
        self.abilities.rules.slow_start
            && self
                .combatants
                .get(combatant)
                .is_some_and(|c| c.ability == SLOW_START && c.is_alive())
            && self
                .abilities
                .slow_start_counter
                .get(combatant)
                .copied()
                .unwrap_or(SLOW_START_TURNS)
                < SLOW_START_TURNS
    }

    /// Start of turn: Flower Gift asks for the form that matches the weather.
    pub(super) fn ability_turn_start(&mut self) {
        self.ability_refresh_forms();
    }

    /// End of turn: the Slow Start counter grows for the combatants that were on the field
    /// (a benched Regigigas starts its countdown again when it is sent out; UNKNOWN in the
    /// game data, this is the assumption).
    pub(super) fn ability_end_of_turn(&mut self) {
        if !self.abilities.rules.slow_start {
            return;
        }
        for side in 0..self.sides.len() {
            let Some(c) = self.active.get(side).copied() else {
                continue;
            };
            if !self.slow_start_active(c) {
                continue;
            }
            if let Some(counter) = self.abilities.slow_start_counter.get_mut(c) {
                *counter = counter.saturating_add(1);
                if *counter >= SLOW_START_TURNS {
                    self.abilities.events.push(AbilityEvent::SlowStartEnded {
                        combatant: c,
                    });
                }
            }
        }
    }

    fn ability_refresh_forms(&mut self) {
        if !self.abilities.rules.flower_gift {
            return;
        }
        let sunny = self.weather == BattleWeather::Sun;
        for i in 0..self.combatants.len() {
            let c = &self.combatants[i];
            if c.ability != FLOWER_GIFT || c.species.dex != CHERRIM_DEX {
                continue;
            }
            let wanted = u8::from(sunny);
            if self.abilities.requested_form.get(i).copied().unwrap_or(0) != wanted {
                if let Some(slot) = self.abilities.requested_form.get_mut(i) {
                    *slot = wanted;
                }
                self.abilities
                    .events
                    .push(AbilityEvent::FormChangeRequested {
                        combatant: i,
                        form: wanted,
                    });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        calculate_damage, Action, BattleRng, BattleWeather, Combatant, Event, XorShift32,
    };
    use super::*;
    use crate::battle::BattleData;
    use std::path::PathBuf;

    struct Fixed(Vec<u32>);
    impl BattleRng for Fixed {
        fn next_u32(&mut self) -> u32 {
            if self.0.is_empty() {
                1
            } else {
                self.0.remove(0)
            }
        }
    }

    fn data() -> BattleData {
        BattleData::load(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
    }

    fn combatant(data: &BattleData, species: &str, moves: &[&str], level: u8) -> Combatant {
        let species = data.species_row(species).unwrap().unwrap();
        let moves = moves
            .iter()
            .map(|n| data.move_row(n).unwrap().unwrap())
            .collect();
        Combatant::from_base(species, level, moves)
    }

    fn pair(data: &BattleData, left: Combatant, right: Combatant) -> Battle {
        Battle::new(vec![left, right], vec![vec![0], vec![1]], data.type_chart().clone()).unwrap()
    }

    fn acted_first(events: &[Event]) -> usize {
        events
            .iter()
            .find_map(|e| match e {
                Event::ActedFirst(a) => Some(*a),
                _ => None,
            })
            .expect("turn had an ActedFirst event")
    }

    /// Acceptance data check: the sheet lists the 318 species present in the game with a
    /// non-zero first ability slot, and the two live abilities land on their species.
    #[test]
    fn abilities_sheet_covers_present_species_and_the_two_live_abilities() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/pml_personal_abilities.tsv");
        let text = std::fs::read_to_string(&path).expect("abilities sheet");
        let mut present_with_ability = 0;
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.first() == Some(&"dex_index_national:u16") {
                continue;
            }
            if cols.get(3) == Some(&"1") && cols.get(4).is_some_and(|v| *v != "0") {
                present_with_ability += 1;
            }
        }
        assert_eq!(present_with_ability, 318);
        let d = data();
        assert_eq!(
            d.species_row("regigigas").unwrap().unwrap().abilities,
            [SLOW_START; 3]
        );
        assert_eq!(
            d.species_row("cherrim").unwrap().unwrap().abilities,
            [FLOWER_GIFT; 3]
        );
        // The other 131 ability ids exist in the data but have no effect; anchors verified
        // against the franchise lists in reports/data-progress/abilities_held_items.md.
        assert_eq!(d.species_row("pikachu").unwrap().unwrap().abilities, [9, 9, 31]);
    }

    /// Acceptance: Regigigas is at half speed on turns 1-5 and back to normal on turn 6.
    #[test]
    fn slow_start_halves_speed_for_the_first_five_turns() {
        let d = data();
        let actions = [
            Action { actor: 0, move_slot: 0 },
            Action { actor: 1, move_slot: 0 },
        ];
        // Rules off (default): Regigigas (base 100 speed) moves first and nothing is queued.
        let regi = combatant(&d, "regigigas", &["growl"], 50);
        let pk = combatant(&d, "pikachu", &["growl"], 50);
        assert_eq!(regi.ability, SLOW_START);
        let mut rng = XorShift32(0x1234_5678);
        let mut off_battle = pair(&d, regi, pk);
        assert_eq!(acted_first(&off_battle.play_turn(&actions, &mut rng)), 0);
        assert!(off_battle.take_ability_events().is_empty(), "abilities are opt-in");
        // Rules on from turn 1: halved for five turns, normal on turn 6.
        let regi = combatant(&d, "regigigas", &["growl"], 50);
        let pk = combatant(&d, "pikachu", &["growl"], 50);
        let mut battle = pair(&d, regi, pk);
        battle.set_abilities(AbilityRules::game_rules());
        for turn in 1..=5 {
            let first = acted_first(&battle.play_turn(&actions, &mut rng));
            assert_eq!(first, 1, "halved speed on turn {turn}");
        }
        assert_eq!(
            battle.take_ability_events(),
            vec![AbilityEvent::SlowStartEnded { combatant: 0 }],
            "the counter reaches the limit at the end of turn 5"
        );
        assert_eq!(
            acted_first(&battle.play_turn(&actions, &mut rng)),
            0,
            "normal speed on turn 6"
        );
    }

    /// Acceptance: physical power is halved, special power is not.
    #[test]
    fn slow_start_halves_physical_power_only() {
        let d = data();
        let regi = combatant(&d, "regigigas", &["tackle"], 50);
        let pk = combatant(&d, "pikachu", &["tackle"], 50);
        let chart = d.type_chart();
        let roll = || Fixed(vec![5, 3]);
        let physical = |slow: bool| {
            calculate_damage(
                &regi, &pk, "Normal", "Physical", 40, chart, BattleWeather::Clear, 0, (0, 0),
                false, true, slow, false, &mut roll(),
            )
            .0
        };
        let special = |slow: bool| {
            calculate_damage(
                &regi, &pk, "Normal", "Special", 40, chart, BattleWeather::Clear, 0, (0, 0),
                false, true, slow, false, &mut roll(),
            )
            .0
        };
        let (on, off) = (physical(true), physical(false));
        assert!(on < off, "physical damage drops while Slow Start is active");
        let ratio = f64::from(off) / f64::from(on);
        assert!(
            (1.8..=2.2).contains(&ratio),
            "physical damage roughly halves: on {on}, off {off}"
        );
        assert_eq!(special(true), special(false), "special power is unchanged");
    }

    /// Acceptance: Cherrim asks for its sunny form only with sun.
    #[test]
    fn flower_gift_requests_cherrim_form_only_with_sun() {
        let d = data();
        let cherrim = combatant(&d, "cherrim", &["growl"], 30);
        let pk = combatant(&d, "pikachu", &["growl"], 30);
        assert_eq!(cherrim.ability, FLOWER_GIFT);
        let mut battle = pair(&d, cherrim, pk);
        battle.weather = BattleWeather::Sun;
        battle.set_abilities(AbilityRules::game_rules());
        assert_eq!(
            battle.take_ability_events(),
            vec![AbilityEvent::FormChangeRequested { combatant: 0, form: 1 }]
        );
        // Sun again: no duplicate.
        battle.ability_turn_start();
        assert!(battle.take_ability_events().is_empty());
        // Rain sends it back to the base form.
        battle.weather = BattleWeather::Rain;
        battle.ability_turn_start();
        assert_eq!(
            battle.take_ability_events(),
            vec![AbilityEvent::FormChangeRequested { combatant: 0, form: 0 }]
        );
        // Rules off: nothing is requested even with sun.
        battle.set_abilities(AbilityRules::default());
        battle.weather = BattleWeather::Sun;
        battle.ability_turn_start();
        assert!(battle.take_ability_events().is_empty());
    }
}
