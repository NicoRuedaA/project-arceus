//! Battle rules re-derived from a static reading of the game's battle code (brief 28).
//!
//! Every constant below carries its confidence tag. VERIFIED means: the rule's structure was
//! read from the update `main` code and its numbers come from the game's own config/data
//! tables, then checked against a hand-computed example in the tests. Nothing here was run
//! in the game. Function names and addresses are listed only in
//! `reports/data-progress/battle_rules_verified.md`; no game code is reproduced here.

/// VERIFIED: additive attack term of the base damage (`damage_atk_power_coef`).
pub const DAMAGE_ATK_POWER_COEF: f32 = 100.0;
/// VERIFIED: level multiplier of the base damage (`damage_atk_level_coef`).
pub const DAMAGE_ATK_LEVEL_COEF: f32 = 15.0;
/// VERIFIED: additive defense term of the base damage (`damage_def_power_coef`).
pub const DAMAGE_DEF_POWER_COEF: f32 = 50.0;
/// VERIFIED: final divisor of the base damage (`damage_denominator`).
pub const DAMAGE_DENOMINATOR: f32 = 5.0;
/// VERIFIED: attacker Atk/SpA group "up" multiplier on the base damage.
pub const ATK_RANK_UP: f32 = 1.5;
/// VERIFIED: attacker Atk/SpA group "down" multiplier on the base damage.
pub const ATK_RANK_DOWN: f32 = 0.66;
/// VERIFIED: defender Def/SpD group "up" multiplier on the base damage.
pub const DEF_RANK_UP: f32 = 0.66;
/// VERIFIED: defender Def/SpD group "down" multiplier on the base damage.
pub const DEF_RANK_DOWN: f32 = 1.5;

/// VERIFIED: percent fail chance of a paralysed combatant's action (`mahi_WazaFailPer`).
pub const PARALYSIS_FAIL_PERCENT: u32 = 33;
/// VERIFIED: percent fail chance of a drowsy combatant's action (`nemuke_WazaFailPer`); doubled in snow.
pub const DROWSY_FAIL_PERCENT: u32 = 33;
/// VERIFIED: percent damage taken by a drowsy defender (`nemuke_WazaDamageRatio`).
pub const DROWSY_DAMAGE_TAKEN_PERCENT: u32 = 133;
/// VERIFIED: percent damage dealt by a burned physical / frostbitten special attacker.
pub const BURN_FROSTBITE_DAMAGE_PERCENT: u32 = 50;
/// VERIFIED: slip damage denominators (poison, burn, frostbite), rounded up.
pub const POISON_SLIP_DENOM: u32 = 6;
pub const BURN_SLIP_DENOM: u32 = 12;
pub const FROSTBITE_SLIP_DENOM: u32 = 12;
/// VERIFIED: badly poisoned (the game's poison record 5 with the 0xf01 mark): end-of-turn
/// damage `ceil(max_hp / 16) x counter`; the counter grows +1 per end of turn, capped at 15,
/// and the state never expires (`FUN_02850cf0` case 5, `FUN_02851d74`). The counter is
/// incremented before the damage of the same end of turn is computed.
pub const TOXIC_SLIP_DENOM: u32 = 16;
pub const TOXIC_COUNTER_CAP: u8 = 15;
/// PROBABLE: the badly-poisoned counter resets when the combatant leaves the field. Series
/// convention; the game's switch flow was not located (report `sleep_toxic.md`, UNKNOWN).
pub const TOXIC_COUNTER_RESETS_ON_SWITCH: bool = true;
/// VERIFIED: partial-trap damage denominator per end of turn, rounded up.
pub const TRAP_SLIP_DENOM: u32 = 8;
/// VERIFIED: confusion self-hit chance (hard-coded, not a config value).
pub const CONFUSION_SELF_HIT_PERCENT: u32 = 33;
/// VERIFIED: power of the confusion self-hit.
pub const CONFUSION_SELF_HIT_POWER: u32 = 40;
/// VERIFIED: agility cap and the 13-bit field it is packed into in the turn-order key.
pub const AGILITY_CAP: u32 = 10_000;
pub const AGILITY_KEY_MASK: u32 = 0x1fff;

/// VERIFIED PLA base damage:
/// `floor(floor(floor(P * (A + 100 + 15 L)) / (D + 50)) / 5)`, computed in single precision
/// with truncation after each step, as the game does. There is no `+2` term.
pub fn pla_base_damage(power: u32, attack: u32, level: u32, defense: u32) -> u32 {
    let attack_term =
        (DAMAGE_ATK_POWER_COEF + attack as f32) + DAMAGE_ATK_LEVEL_COEF * level as f32;
    let scaled = (attack_term * power as f32) as u32;
    let divided = (scaled as f32 / (DAMAGE_DEF_POWER_COEF + defense as f32)) as u32;
    (divided as f32 / DAMAGE_DENOMINATOR) as u32
}

/// Main-series base damage `((P*A*(floor(2L/5)+2))/D)/50+2`. REFUTED for PLA: it is only the
/// game's fallback when no damage callback is configured, and the shipped config installs one.
/// Kept as the engine default so existing callers stay unchanged; `notes_rates` (game rules)
/// or `Battle::set_pla_damage_formula(true)` selects [`pla_base_damage`].
pub fn legacy_base_damage(power: u32, attack: u32, level: u32, defense: u32) -> u32 {
    (((power * attack * ((2 * level / 5) + 2)) / defense.max(1)) / 50) + 2
}

/// Base damage with the formula selected by `pla`.
pub fn base_damage(pla: bool, power: u32, attack: u32, level: u32, defense: u32) -> u32 {
    if pla {
        pla_base_damage(power, attack, level, defense)
    } else {
        legacy_base_damage(power, attack, level, defense)
    }
}

/// VERIFIED rank multiplier on the base damage. Stages use the engine's -1/0/+1 encoding of
/// the game's three rank states. Equal attacker/defender states skip the multiplier entirely
/// (so up/up and down/down give x1, not x0.99). Single precision, truncated.
pub fn apply_rank_states(base: u32, attacker_atk_stage: i8, defender_def_stage: i8) -> u32 {
    if attacker_atk_stage == defender_def_stage {
        return base;
    }
    let atk = match attacker_atk_stage {
        s if s > 0 => ATK_RANK_UP,
        s if s < 0 => ATK_RANK_DOWN,
        _ => 1.0,
    };
    let def = match defender_def_stage {
        s if s > 0 => DEF_RANK_UP,
        s if s < 0 => DEF_RANK_DOWN,
        _ => 1.0,
    };
    ((base as f32 * atk) * def) as u32
}

/// VERIFIED fraction-of-max-HP damage: `ceil(max_hp / denom)`, at least 1.
pub fn ceil_fraction(max_hp: u32, denom: u32) -> u32 {
    max_hp.div_ceil(denom.max(1)).max(1)
}

/// VERIFIED accuracy table indexed by `clamp(acc + 1 - eva, 0, 2)` (numerator, denominator).
pub const ACCURACY_TABLE: [(u32, u32); 3] = [(70, 100), (100, 100), (130, 100)];

/// VERIFIED duration roll used for turn-limited conditions: uniform in `[min, max]`
/// (inclusive, bounds swapped when reversed). No randomness is consumed when min == max.
pub fn roll_turns(min: u8, max: u8, rng: &mut impl BattleRng) -> u8 {
    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
    if lo == hi {
        lo
    } else {
        lo + rng.below(u32::from(hi - lo) + 1) as u8
    }
}

/// Rule notifications that do not fit the stable `Event`/`EffectEvent`/`StatusEvent` enums
/// (those are matched exhaustively by game code). Drain with `Battle::take_rule_events`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleEvent {
    /// A turn-limited major status ran out at the end of a turn (VERIFIED countdown).
    StatusExpired { target: usize, status: MajorStatus },
}

use super::effects::{EffectEvent, MoveEffects, Stat};
use super::{Battle, BattleRng, MajorStatus};

/// Rank group of a stat id: 0 Atk/SpA, 1 Def/SpD, 2 Spe/Acc/Eva (VERIFIED shared groups).
fn stage_group(stat: Stat) -> Option<usize> {
    match stat {
        Stat::Attack | Stat::SpecialAttack => Some(0),
        Stat::Defense | Stat::SpecialDefense => Some(1),
        Stat::Speed | Stat::Accuracy | Stat::Evasion => Some(2),
        Stat::AllBattleStats => None,
    }
}

impl Battle {
    /// Select the VERIFIED PLA base damage formula (also selected by `notes_rates`).
    pub fn set_pla_damage_formula(&mut self, on: bool) {
        self.pla_damage = on;
    }

    pub fn pla_damage_formula(&self) -> bool {
        self.pla_damage
    }

    /// Drain rule notifications (status expiry). See [`RuleEvent`].
    pub fn take_rule_events(&mut self) -> Vec<RuleEvent> {
        std::mem::take(&mut self.rule_events)
    }

    /// Remaining turn-limited duration of a major status: `(turns, elapsed)`.
    pub fn status_duration(&self, i: usize) -> Option<(u8, u8)> {
        self.status_turns.get(i).copied().flatten()
    }

    /// Remaining turns of each rank group (Atk/SpA, Def/SpD, Spe/Acc/Eva).
    pub fn stage_durations(&self, i: usize) -> [u8; 3] {
        self.stage_turns.get(i).copied().unwrap_or_default()
    }

    /// Slip damage per end of turn: VERIFIED `ceil(max_hp / denom)` with the game's
    /// denominators when `notes_rates` is on; the legacy `floor(max_hp / 8)` otherwise.
    pub(super) fn residual_damage(&self, i: usize, status: MajorStatus) -> u32 {
        let max_hp = self.combatants[i].max_hp;
        if !self.status_rules().notes_rates {
            return (max_hp / 8).max(1);
        }
        let denom = match status {
            MajorStatus::Poison => POISON_SLIP_DENOM,
            MajorStatus::Burn => BURN_SLIP_DENOM,
            _ => FROSTBITE_SLIP_DENOM,
        };
        ceil_fraction(max_hp, denom)
    }

    /// Estimated damage of the badly-poisoned state at this moment, without touching the
    /// counter (the counter starts at 0, so a fresh state already deals one step).
    pub fn toxic_damage_estimate(&self, i: usize) -> u32 {
        let max_hp = self.combatants[i].max_hp;
        if !self.status_rules().notes_rates {
            return (max_hp / 8).max(1);
        }
        let counter = self.toxic_counter.get(i).copied().unwrap_or(0).max(1);
        ceil_fraction(max_hp, TOXIC_SLIP_DENOM) * u32::from(counter)
    }

    /// VERIFIED end-of-turn badly-poisoned update: the counter grows +1 (capped at 15) and the
    /// same end of turn deals `ceil(max_hp / 16) x counter` with the updated value.
    pub(super) fn toxic_slip_damage(&mut self, i: usize) -> u32 {
        if self.toxic_counter.len() <= i {
            self.toxic_counter.resize(i + 1, 0);
        }
        if self.toxic_counter[i] < TOXIC_COUNTER_CAP {
            self.toxic_counter[i] += 1;
        }
        self.toxic_damage_estimate(i)
    }

    /// Counter of the badly-poisoned state (0 = absent or just reset).
    pub fn toxic_counter(&self, i: usize) -> u8 {
        self.toxic_counter.get(i).copied().unwrap_or(0)
    }

    /// Record the data-driven duration of a freshly applied major status (move columns
    /// `raw_f13` kind, `raw_f14` min, `raw_f15` max; kinds 2 and 4 are turn-limited).
    pub(super) fn start_status_turns(
        &mut self,
        target: usize,
        fx: &MoveEffects,
        rng: &mut impl BattleRng,
    ) {
        if self.status_turns.len() <= target {
            self.status_turns.resize(target + 1, None);
        }
        self.status_turns[target] = match fx.ailment_turns {
            Some((kind, min, max)) if matches!(kind, 2 | 4) && max.max(min) > 0 => {
                Some((roll_turns(min, max, rng), 0))
            }
            _ => None,
        };
    }

    /// VERIFIED end-of-turn countdown of turn-limited major statuses (after slip damage):
    /// the elapsed counter grows by one per end of turn and the status ends once it reaches
    /// the rolled duration. Only active combatants count down. Requires `notes_rates`.
    pub(super) fn status_turns_end_of_turn(&mut self) {
        if !self.status_rules().notes_rates {
            return;
        }
        for i in 0..self.status_turns.len() {
            let Some((turns, elapsed)) = self.status_turns[i] else {
                continue;
            };
            let Some(status) = self.combatants[i].status else {
                self.status_turns[i] = None;
                continue;
            };
            if !self.combatants[i].is_alive() || !self.active.contains(&i) {
                continue;
            }
            let elapsed = elapsed + 1;
            if elapsed >= turns {
                self.status_turns[i] = None;
                self.combatants[i].status = None;
                self.rule_events
                    .push(RuleEvent::StatusExpired { target: i, status });
            } else {
                self.status_turns[i] = Some((turns, elapsed));
            }
        }
    }

    /// Store a rank change's duration (VERIFIED: the longer of the current and new duration;
    /// returning to neutral clears it). A zero duration (no data) never expires (ASSUMED).
    pub(super) fn note_stage_turns(&mut self, target: usize, stat: Stat, turns: u8) {
        let groups: &[usize] = match stage_group(stat) {
            Some(0) => &[0],
            Some(1) => &[1],
            Some(_) => &[2],
            None => &[0, 1, 2],
        };
        if self.stage_turns.len() <= target {
            self.stage_turns.resize(target + 1, [0; 3]);
        }
        let stages = self.stages(target);
        for &g in groups {
            let state = [stages.attack, stages.defense, stages.speed][g];
            let slot = &mut self.stage_turns[target][g];
            *slot = if state == 0 { 0 } else { (*slot).max(turns) };
        }
    }

    /// VERIFIED end-of-turn rank countdown: each group's counter drops by one; a non-neutral
    /// group whose counter reaches zero returns to neutral. Requires `stage_multipliers`.
    pub(super) fn stage_turns_end_of_turn(&mut self) {
        if !self.stage_multipliers {
            return;
        }
        for i in 0..self.stage_turns.len() {
            if !self.combatants[i].is_alive() || !self.active.contains(&i) {
                continue;
            }
            for g in 0..3 {
                let left = self.stage_turns[i][g];
                if left == 0 {
                    continue;
                }
                self.stage_turns[i][g] = left - 1;
                if left - 1 > 0 {
                    continue;
                }
                let st = self.stages_mut(i);
                let (slot, stat) = match g {
                    0 => (&mut st.attack, Stat::Attack),
                    1 => (&mut st.defense, Stat::Defense),
                    _ => (&mut st.speed, Stat::Speed),
                };
                if *slot != 0 {
                    *slot = 0;
                    self.effect_events.push(EffectEvent::StagesChanged {
                        target: i,
                        stat,
                        new_state: 0,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(Vec<u32>);
    impl super::super::BattleRng for Fixed {
        fn next_u32(&mut self) -> u32 {
            if self.0.is_empty() {
                0
            } else {
                self.0.remove(0)
            }
        }
    }

    #[test]
    fn base_damage_matches_hand_calculation() {
        // P=40, A=50, L=10, D=50: 40*(50+100+150)=12000; /100=120; /5=24.
        assert_eq!(pla_base_damage(40, 50, 10, 50), 24);
        // P=90, A=73, L=25, D=61: 90*(73+100+375)=49320; /111=444 (444.32); /5=88 (88.8).
        assert_eq!(pla_base_damage(90, 73, 25, 61), 88);
        // Old main-series shape would give 6 for the first case; the game formula has no +2.
        assert_eq!(pla_base_damage(0, 50, 10, 50), 0);
    }

    #[test]
    fn rank_states_multiply_base_and_cancel_when_equal() {
        assert_eq!(apply_rank_states(100, 0, 0), 100);
        assert_eq!(apply_rank_states(100, 1, 0), 150);
        assert_eq!(apply_rank_states(100, -1, 0), 66);
        assert_eq!(apply_rank_states(100, 0, 1), 66);
        assert_eq!(apply_rank_states(100, 0, -1), 150);
        // up vs up and down vs down skip the multiply (x1), not 1.5*0.66.
        assert_eq!(apply_rank_states(100, 1, 1), 100);
        assert_eq!(apply_rank_states(100, -1, -1), 100);
        // up vs down: 100*1.5*1.5 = 225.
        assert_eq!(apply_rank_states(100, 1, -1), 225);
    }

    #[test]
    fn fraction_damage_rounds_up() {
        assert_eq!(ceil_fraction(100, 12), 9); // 8.33 -> 9
        assert_eq!(ceil_fraction(100, 6), 17); // 16.67 -> 17
        assert_eq!(ceil_fraction(96, 8), 12);
        assert_eq!(ceil_fraction(1, 12), 1);
    }

    #[test]
    fn turn_roll_is_inclusive_and_free_when_fixed() {
        assert_eq!(roll_turns(5, 5, &mut Fixed(vec![])), 5);
        assert_eq!(roll_turns(2, 5, &mut Fixed(vec![0])), 2);
        assert_eq!(roll_turns(2, 5, &mut Fixed(vec![3])), 5);
        assert_eq!(roll_turns(6, 5, &mut Fixed(vec![1])), 6);
    }

    mod in_battle {
        use super::Fixed;
        use crate::battle::BattleData;
        use crate::battle_engine::effects::{EffectEvent, MoveEffectsTable, Stat};
        use crate::battle_engine::rules::RuleEvent;
        use crate::battle_engine::statuses::StatusRules;
        use crate::battle_engine::*;
        use std::path::PathBuf;

        fn root() -> PathBuf {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
        }

        fn duel(a: &[&str], b: &[&str]) -> Battle {
            let d = BattleData::load(root()).unwrap();
            let mon = |species: &str, moves: &[&str]| {
                let sp = d.species_row(species).unwrap().unwrap();
                let mv = moves
                    .iter()
                    .map(|n| d.move_row(n).unwrap().unwrap())
                    .collect();
                Combatant::from_base(sp, 30, mv)
            };
            let mut battle = Battle::new(
                vec![mon("bidoof", a), mon("pikachu", b)],
                vec![vec![0], vec![1]],
                d.type_chart().clone(),
            )
            .unwrap();
            battle.set_move_effects(MoveEffectsTable::load(root()).unwrap());
            battle
        }

        /// Synthetic Special hit: SpA 73, L25, SpD 61, power 90, neutral, no STAB.
        fn hit(b: &Battle, ranks: (i8, i8), drowsy: bool) -> u32 {
            let mut attacker = b.combatants[0].clone();
            let mut target = b.combatants[1].clone();
            attacker.special_attack = 73;
            attacker.level = 25;
            target.special_defense = 61;
            target.species.types = ["Normal".into(), "Normal".into()];
            if drowsy {
                target.status = Some(MajorStatus::Drowsy);
            }
            // rng: crit check 1 (no crit), roll 15 (100 %).
            calculate_damage(
                &attacker,
                &target,
                "Water",
                "Special",
                90,
                &b.type_chart,
                BattleWeather::Clear,
                0,
                ranks,
                true,
                true,
                false,
                &mut Fixed(vec![1, 15]),
            )
            .0
        }

        #[test]
        fn damage_pipeline_uses_base_ranks_and_drowsy_bonus() {
            let b = duel(&["tackle"], &["tackle"]);
            // base 88 (see base_damage_matches_hand_calculation).
            assert_eq!(hit(&b, (0, 0), false), 88);
            assert_eq!(hit(&b, (1, 0), false), 132); // 88 * 1.5
            assert_eq!(hit(&b, (0, 1), false), 58); // 88 * 0.66 = 58.08
            assert_eq!(hit(&b, (1, 1), false), 88); // equal states cancel
            assert_eq!(hit(&b, (0, 0), true), 117); // 88 * 133 / 100 = 117.04
        }

        #[test]
        fn agility_is_capped_reversed_and_packed_in_13_bits() {
            let b = duel(&["tackle"], &["tackle"]);
            let mut c = b.combatants[0].clone();
            c.speed = 9000;
            assert_eq!(
                turn_order_key(&c, 0, BattleWeather::Clear, false, false).2,
                9000 & 0x1fff
            );
            c.speed = 12_000; // capped at 10000, then packed: 10000 & 8191 = 1808
            assert_eq!(turn_order_key(&c, 0, BattleWeather::Clear, false, false).2, 1808);
            c.speed = 100; // reversed: 9900 & 8191 = 1708
            assert_eq!(turn_order_key(&c, 0, BattleWeather::Clear, true, false).2, 1708);
            c.speed = 300; // sun does not boost a non-Grass type; paralysis halves: 150
            c.status = Some(MajorStatus::Paralysis);
            assert_eq!(turn_order_key(&c, 0, BattleWeather::Sun, false, false).2, 150);
        }

        #[test]
        fn accuracy_and_evasion_share_the_speed_rank_group() {
            let mut b = duel(&["tackle"], &["tackle"]);
            b.change_stat(1, Stat::Evasion, 1);
            assert_eq!(b.stages(1).speed, 1);
            b.change_stat(1, Stat::Speed, -1);
            assert_eq!(b.stages(1).speed, 0, "speed down cancels the evasion up");
            b.change_stat(1, Stat::Accuracy, 1);
            assert_eq!(b.stages(1).speed, 1);
            // Tackle (100 %) into an "up" defender: 100 * 0.7 = 70; roll 70 misses.
            b.combatants[0].moves[0].data.accuracy = 100;
            let ev = b.play_turn(
                &[Action {
                    actor: 0,
                    move_slot: 0,
                }],
                &mut Fixed(vec![70]),
            );
            assert!(ev.iter().any(|e| matches!(e, Event::Missed { .. })));
        }

        #[test]
        fn statuses_from_moves_expire_after_their_turns() {
            let mut b = duel(&["tackle"], &["thunder-wave"]);
            b.set_status_rules(StatusRules {
                notes_rates: true,
                ..StatusRules::default()
            });
            b.combatants[0].max_hp = 5000;
            b.combatants[0].hp = 5000;
            // Thunder Wave: data kind 2, 5..=5 turns; accuracy roll 0 hits.
            b.play_turn(
                &[Action {
                    actor: 1,
                    move_slot: 0,
                }],
                &mut Fixed(vec![0]),
            );
            assert_eq!(b.combatants[0].status, Some(MajorStatus::Paralysis));
            assert_eq!(b.status_duration(0), Some((5, 1)));
            for _ in 0..3 {
                b.play_turn(&[], &mut Fixed(vec![]));
            }
            assert_eq!(b.status_duration(0), Some((5, 4)));
            assert!(b.take_rule_events().is_empty());
            b.play_turn(&[], &mut Fixed(vec![]));
            assert_eq!(b.combatants[0].status, None);
            assert_eq!(
                b.take_rule_events(),
                vec![RuleEvent::StatusExpired {
                    target: 0,
                    status: MajorStatus::Paralysis
                }]
            );
        }

        /// VERIFIED badly-poisoned formula and order: the counter grows first (+1, capped at
        /// 15) and the same end of turn deals ceil(max_hp / 16) x counter; the state never
        /// expires and the counter restarts when the combatant leaves the field (PROBABLE).
        #[test]
        fn badly_poisoned_counter_grows_caps_and_resets_on_switch() {
            let d = BattleData::load(root()).unwrap();
            let mon = |species: &str, moves: &[&str]| {
                let sp = d.species_row(species).unwrap().unwrap();
                let mv = moves
                    .iter()
                    .map(|n| d.move_row(n).unwrap().unwrap())
                    .collect();
                Combatant::from_base(sp, 30, mv)
            };
            let mut b = Battle::new(
                vec![
                    mon("bidoof", &["tackle"]),
                    mon("pikachu", &["toxic"]),
                    mon("bidoof", &["tackle"]),
                ],
                vec![vec![0, 2], vec![1]],
                d.type_chart().clone(),
            )
            .unwrap();
            b.set_move_effects(MoveEffectsTable::load(root()).unwrap());
            b.set_status_rules(StatusRules {
                notes_rates: true,
                ..StatusRules::default()
            });
            for c in &mut b.combatants {
                c.max_hp = 1600;
                c.hp = 1600;
            }
            // Toxic lands (accuracy 90, roll 0) and starts the counter at 0; the same end of
            // turn grows it to 1 and deals ceil(1600/16) = 100.
            b.play_turn(
                &[Action {
                    actor: 1,
                    move_slot: 0,
                }],
                &mut Fixed(vec![0]),
            );
            assert_eq!(b.combatants[0].status, Some(MajorStatus::BadlyPoisoned));
            assert_eq!(b.toxic_counter(0), 1);
            assert_eq!(b.combatants[0].hp, 1500);
            // Next turn: 2 x 100.
            b.play_turn(&[], &mut Fixed(vec![]));
            assert_eq!(b.toxic_counter(0), 2);
            assert_eq!(b.combatants[0].hp, 1300);
            // Never expires: no rule event after more turns.
            for _ in 0..3 {
                b.play_turn(&[], &mut Fixed(vec![]));
            }
            assert_eq!(b.combatants[0].status, Some(MajorStatus::BadlyPoisoned));
            assert_eq!(b.toxic_counter(0), 5);
            assert!(b.take_rule_events().is_empty());
            // Switching restarts the counter (PROBABLE constant).
            b.combatants[0].hp = 1600;
            b.switch(0, 2).unwrap();
            assert_eq!(b.toxic_counter(0), 0);
            b.switch(0, 0).unwrap();
            b.play_turn(&[], &mut Fixed(vec![]));
            assert_eq!(b.toxic_counter(0), 1);
        }

        #[test]
        fn badly_poisoned_formula_caps_the_counter_at_fifteen() {
            let mut b = duel(&["tackle"], &["tackle"]);
            b.set_status_rules(StatusRules {
                notes_rates: true,
                ..StatusRules::default()
            });
            b.combatants[1].max_hp = 160;
            b.combatants[1].hp = 160;
            // ceil(160/16) = 10; the counter grows before each damage step.
            assert_eq!(b.toxic_slip_damage(1), 10);
            assert_eq!(b.toxic_counter(1), 1);
            assert_eq!(b.toxic_slip_damage(1), 20);
            for _ in 0..20 {
                b.toxic_slip_damage(1);
            }
            assert_eq!(b.toxic_counter(1), 15, "capped at the duration field (15)");
            assert_eq!(b.toxic_damage_estimate(1), 150);
        }

        #[test]
        fn rank_changes_expire_after_their_turns() {
            let mut b = duel(&["swords-dance"], &["tackle"]);
            b.set_status_rules(StatusRules {
                stage_multipliers: true,
                ..StatusRules::default()
            });
            // Swords Dance: raw_f31 = 5 turns; the use turn's end already counts.
            b.play_turn(
                &[Action {
                    actor: 0,
                    move_slot: 0,
                }],
                &mut Fixed(vec![0]),
            );
            assert_eq!(b.stages(0).attack, 1);
            assert_eq!(b.stage_durations(0), [4, 0, 0]);
            b.take_effect_events();
            for _ in 0..3 {
                b.play_turn(&[], &mut Fixed(vec![]));
            }
            assert_eq!(b.stages(0).attack, 1);
            b.play_turn(&[], &mut Fixed(vec![]));
            assert_eq!(b.stages(0).attack, 0);
            assert!(b
                .take_effect_events()
                .contains(&EffectEvent::StagesChanged {
                    target: 0,
                    stat: Stat::Attack,
                    new_state: 0
                }));
        }
    }
}
