//! Volatile battle conditions and turn-state rules: confusion, sleep duration, partial
//! trapping, two-turn (charge) moves, recharge turns, Protect-style moves, speed stages.
//!
//! Everything is additive and opt-in through [`StatusRules`]; the default value changes
//! nothing, so existing demos and tests behave exactly as before. Results are reported in a
//! separate [`StatusEvent`] list (`Battle::take_status_events`) because the stable `Event`
//! and `EffectEvent` enums are matched exhaustively by other code.
//!
//! Confidence tags (VERIFIED = read from the game's battle code, see
//! `reports/data-progress/battle_rules_verified.md`; LIKELY = data column meaning inferred;
//! ASSUMED = franchise convention that no recovered code or data contradicts):
//!
//! * Which moves do what (data columns): confusion = `ailment_id` 8 (LIKELY), trapping =
//!   `ailment_id` 10 (LIKELY), charge/recharge/protectable flags (LIKELY), Protect-style =
//!   `meta_category` 13 + `target_raw` 7 + `priority` +4 (priority VERIFIED, rest LIKELY) with
//!   `effect_id` in {111, 355, 361, 383, 441} (protect/detect/max-guard, king's-shield,
//!   spiky-shield, baneful-bunker, obstruct); endure (116), magic-coat (183) and snatch (195)
//!   share the +4 priority but are not blocking moves and are excluded.
//! * Confusion (VERIFIED): duration rolled in the move's `[raw_f14, raw_f15]` range (2..=5 for
//!   Confuse Ray), counted per action; the action that reaches it snaps out and acts normally;
//!   otherwise 33 % self-hit: 40-power hit through the PLA base formula on the user's own
//!   Atk/Def, rank states ignored, no crit/STAB/type, 85-100 % roll, minimum 1.
//! * Drowsy (VERIFIED, `sleep_duration` switch): no forced sleep; each action fails 33 %
//!   (66 % in snow); it lasts the move's turn count (see `rules.rs`). The franchise model of
//!   1..=3 skipped actions was REFUTED.
//! * Paralysis action failure 33 % (VERIFIED, `notes_rates`); poison 1/6, burn 1/12, frostbite
//!   1/12 of max HP per end of turn, rounded up (VERIFIED). Off keeps the old 25 % / 1/8.
//! * Order of the pre-action checks (VERIFIED): confusion, then paralysis, then drowsiness.
//! * Trapping: duration rolled in the move's `[raw_f14, raw_f15]` range (5..=6 for Wrap,
//!   VERIFIED), ceil(1/8) max HP per end of turn (VERIFIED); no switching while the trapper
//!   stays active and alive (ASSUMED).
//! * Charge: turn 1 only charges, turn 2 attacks with the stored move (ASSUMED; no move usable
//!   in PLA carries `flag_charge`, so the rule is not exercised by real PLA moves).
//! * Recharge: REFUTED as a column meaning. `flag_recharge` is also set on Dire Claw and
//!   Psyshield Bash, which never recharge, so the engine no longer skips the next action.
//! * Protect: blocks protectable moves aimed at the protector for the round; success
//!   chance 1/3^n after n consecutive successes (ASSUMED; no Protect-style move is usable in
//!   PLA's move set, and no chain code was found).
//! * Speed stages: the game's agility does not read rank states (VERIFIED), so the switch no
//!   longer changes turn order; the speed rank group drives accuracy/evasion instead.

use super::effects::MoveEffects;
use super::rules;
use super::{Battle, BattleRng, BattleWeather, Event, MajorStatus};

/// Opt-in switches. `Default` (everything off) leaves the engine unchanged.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusRules {
    /// Confusion, trapping, charge turns, recharge turns, Protect-style moves.
    pub volatile_statuses: bool,
    /// Drowsy follows the game's rule (33 % fail, 66 % in snow) instead of a 50 % coin flip.
    pub sleep_duration: bool,
    /// Game rates: paralysis 33 %, slip damage 1/6, 1/12, 1/12 rounded up, drowsy defender
    /// x1.33 damage taken, and turn-limited major statuses.
    pub notes_rates: bool,
    /// Kept for compatibility: rank states do not change turn order in the game (VERIFIED).
    pub speed_stages: bool,
    /// Rank states multiply the base damage, and rank changes expire after their turns.
    pub stage_multipliers: bool,
}

impl StatusRules {
    /// Everything on: the engine's closest approximation of the game's rules.
    pub fn game_rules() -> Self {
        Self {
            volatile_statuses: true,
            sleep_duration: true,
            notes_rates: true,
            speed_stages: true,
            stage_multipliers: true,
        }
    }
}

/// New events, kept apart from `Event`/`EffectEvent` (exhaustive matches elsewhere).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusEvent {
    Confused { target: usize },
    SnappedOutOfConfusion { target: usize },
    HurtByConfusion { actor: usize, damage: u32 },
    WokeUp { target: usize },
    Trapped { target: usize, by: usize },
    HurtByTrap { target: usize, damage: u32 },
    TrapEnded { target: usize },
    CantSwitch { side: usize, combatant: usize },
    Charging { actor: usize, move_name: String },
    MustRecharge { actor: usize },
    Recharging { actor: usize },
    ProtectUp { actor: usize },
    ProtectFailed { actor: usize },
    ProtectedFrom { target: usize, attacker: usize, move_name: String },
}

#[derive(Debug, Clone, Copy)]
struct Trap {
    by: usize,
    turns: u8,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Volatile {
    confusion: u8,
    trap: Option<Trap>,
    charging: Option<usize>,
    recharge: bool,
    protecting: bool,
    protect_used_now: bool,
    protect_chain: u8,
}

#[derive(Debug, Clone, Default)]
pub(super) struct StatusState {
    rules: StatusRules,
    vol: Vec<Volatile>,
    events: Vec<StatusEvent>,
}

pub(super) enum PreAction {
    Skip,
    /// Proceed; `Some(slot)` = release a stored charge using that move slot.
    Act(Option<usize>),
}

const PROTECT_EFFECT_IDS: [u16; 5] = [111, 355, 361, 383, 441];
pub const CONFUSION_SELF_HIT_PERCENT: u32 = rules::CONFUSION_SELF_HIT_PERCENT;
pub const CONFUSION_SELF_HIT_POWER: u64 = rules::CONFUSION_SELF_HIT_POWER as u64;
pub const PARALYSIS_FAIL_PERCENT: u32 = rules::PARALYSIS_FAIL_PERCENT;

/// Does this move's data row describe a Protect-style blocking move?
pub fn is_protect_move(fx: &MoveEffects) -> bool {
    fx.priority == 4
        && fx.meta_category == 13
        && fx.target_raw == 7
        && PROTECT_EFFECT_IDS.contains(&fx.effect_id)
}

impl Battle {
    pub fn set_status_rules(&mut self, rules: StatusRules) {
        self.status.rules = rules;
        if rules.stage_multipliers {
            self.stage_multipliers = true;
        }
        if rules.notes_rates {
            self.pla_damage = true;
        }
    }

    pub fn status_rules(&self) -> StatusRules {
        self.status.rules
    }

    pub fn take_status_events(&mut self) -> Vec<StatusEvent> {
        std::mem::take(&mut self.status.events)
    }

    fn vol(&mut self, i: usize) -> &mut Volatile {
        if self.status.vol.len() <= i {
            self.status.vol.resize(i + 1, Volatile::default());
        }
        &mut self.status.vol[i]
    }

    fn vol_ref(&self, i: usize) -> Option<&Volatile> {
        self.status.vol.get(i)
    }

    pub fn is_confused(&self, i: usize) -> bool {
        self.vol_ref(i).is_some_and(|v| v.confusion > 0)
    }

    pub fn is_charging(&self, i: usize) -> bool {
        self.vol_ref(i).is_some_and(|v| v.charging.is_some())
    }

    pub fn must_recharge(&self, i: usize) -> bool {
        self.vol_ref(i).is_some_and(|v| v.recharge)
    }

    /// Trapped = a living, still-active trapper holds the combatant.
    pub fn is_trapped(&self, i: usize) -> bool {
        let Some(trap) = self.vol_ref(i).and_then(|v| v.trap) else {
            return false;
        };
        self.combatants[trap.by].is_alive() && self.active.contains(&trap.by)
    }

    /// Switch gate; returns false (and logs) when a trap forbids leaving.
    pub(super) fn switch_allowed(&mut self, side: usize, current: usize) -> bool {
        if self.status.rules.volatile_statuses && self.is_trapped(current) {
            self.status.events.push(StatusEvent::CantSwitch {
                side,
                combatant: current,
            });
            return false;
        }
        true
    }

    /// Leaving the field drops volatile conditions and any trap involving the combatant.
    pub(super) fn clear_volatile_on_leave(&mut self, i: usize) {
        if self.status.vol.is_empty() {
            return;
        }
        *self.vol(i) = Volatile::default();
        for v in &mut self.status.vol {
            if v.trap.is_some_and(|t| t.by == i) {
                v.trap = None;
            }
        }
    }

    /// Checks that happen when a combatant is about to act (before PP is spent).
    /// VERIFIED order: confusion, paralysis, drowsiness.
    pub(super) fn status_pre_action(
        &mut self,
        actor: usize,
        rng: &mut impl BattleRng,
        events: &mut Vec<Event>,
    ) -> PreAction {
        let rules = self.status.rules;
        if rules.volatile_statuses && self.vol(actor).recharge {
            self.vol(actor).recharge = false;
            self.status.events.push(StatusEvent::Recharging { actor });
            return PreAction::Skip;
        }
        if rules.volatile_statuses && self.vol(actor).confusion > 0 {
            self.vol(actor).confusion -= 1;
            if self.vol(actor).confusion == 0 {
                self.status
                    .events
                    .push(StatusEvent::SnappedOutOfConfusion { target: actor });
            } else if rng.below(100) < CONFUSION_SELF_HIT_PERCENT {
                let damage = self.confusion_damage(actor, rng);
                let c = &mut self.combatants[actor];
                let dealt = damage.min(c.hp);
                c.hp -= dealt;
                self.status
                    .events
                    .push(StatusEvent::HurtByConfusion { actor, damage: dealt });
                if self.combatants[actor].hp == 0 {
                    events.push(Event::KnockedOut(actor));
                    self.advance_active_for(actor);
                }
                return PreAction::Skip;
            }
        }
        if rules.notes_rates
            && self.combatants[actor].status == Some(MajorStatus::Paralysis)
            && rng.below(100) < PARALYSIS_FAIL_PERCENT
        {
            events.push(Event::UnableToAct {
                actor,
                status: MajorStatus::Paralysis,
            });
            return PreAction::Skip;
        }
        if rules.sleep_duration && self.combatants[actor].status == Some(MajorStatus::Drowsy) {
            let fail = if self.weather == BattleWeather::Snow {
                rules::DROWSY_FAIL_PERCENT * 2
            } else {
                rules::DROWSY_FAIL_PERCENT
            };
            if rng.below(100) < fail {
                events.push(Event::UnableToAct {
                    actor,
                    status: MajorStatus::Drowsy,
                });
                return PreAction::Skip;
            }
        }
        if rules.volatile_statuses {
            if let Some(slot) = self.vol(actor).charging.take() {
                return PreAction::Act(Some(slot));
            }
        }
        PreAction::Act(None)
    }

    /// VERIFIED confusion self-hit: 40 power through the PLA base formula on the user's own
    /// Atk and Def (no event modifiers, rank states ignored), 85-100 % roll, minimum 1.
    fn confusion_damage(&self, actor: usize, rng: &mut impl BattleRng) -> u32 {
        let c = &self.combatants[actor];
        let base = rules::base_damage(
            self.pla_damage,
            rules::CONFUSION_SELF_HIT_POWER,
            c.attack,
            u32::from(c.level),
            c.defense,
        );
        (base * (85 + rng.below(16)) / 100).max(1)
    }

    /// Charge / Protect handling once the move is chosen. Returns true when the action ends here.
    pub(super) fn status_pre_move(
        &mut self,
        actor: usize,
        target: usize,
        slot: usize,
        move_name: &str,
        releasing: bool,
        rng: &mut impl BattleRng,
    ) -> bool {
        if !self.status.rules.volatile_statuses {
            return false;
        }
        let Some(fx) = self.effects.as_ref().and_then(|t| t.get(move_name)).cloned() else {
            return false;
        };
        if self.effects_flag(move_name, Flag::Charge) && !releasing {
            self.vol(actor).charging = Some(slot);
            self.status.events.push(StatusEvent::Charging {
                actor,
                move_name: move_name.to_owned(),
            });
            return true;
        }
        if is_protect_move(&fx) {
            let chain = self.vol(actor).protect_chain.min(6);
            let ok = chain == 0 || rng.below(3u32.pow(u32::from(chain))) == 0;
            if ok {
                let v = self.vol(actor);
                v.protecting = true;
                v.protect_used_now = true;
                v.protect_chain = chain + 1;
                self.status.events.push(StatusEvent::ProtectUp { actor });
            } else {
                self.vol(actor).protect_chain = 0;
                self.status.events.push(StatusEvent::ProtectFailed { actor });
            }
            return true;
        }
        let aimed_at_target = !matches!(fx.target_raw, 7 | 10 | 12 | 13);
        if aimed_at_target
            && self.effects_flag(move_name, Flag::Protectable)
            && self.vol_ref(target).is_some_and(|v| v.protecting)
        {
            self.status.events.push(StatusEvent::ProtectedFrom {
                target,
                attacker: actor,
                move_name: move_name.to_owned(),
            });
            return true;
        }
        false
    }

    fn effects_flag(&self, move_name: &str, flag: Flag) -> bool {
        self.effects
            .as_ref()
            .and_then(|t| t.get(move_name))
            .is_some_and(|f| match flag {
                Flag::Charge => f.charge,
                Flag::Protectable => f.protectable,
            })
    }

    /// After a damaging move connected. The old "recharge flag skips the next action" rule
    /// was REFUTED (the column is set on Dire Claw and Psyshield Bash too), so nothing happens.
    /// How PLA delays the user after Hyper Beam-like moves is UNKNOWN (action-order system).
    pub(super) fn status_post_hit(&mut self, _actor: usize, _move_name: &str) {}

    /// Confusion (ailment 8) and trapping (ailment 10) from a move that connected.
    pub(super) fn status_post_effects(
        &mut self,
        actor: usize,
        target: usize,
        fx: &MoveEffects,
        damaging: bool,
        rng: &mut impl BattleRng,
    ) {
        if !self.status.rules.volatile_statuses
            || !matches!(fx.ailment_id, 8 | 10)
            || !self.combatants[target].is_alive()
        {
            return;
        }
        let chance = if fx.ailment_chance == 0 && !damaging {
            100
        } else {
            u32::from(fx.ailment_chance)
        };
        if chance == 0 || (chance < 100 && rng.below(100) >= chance) {
            return;
        }
        if fx.ailment_id == 8 {
            if self.vol(target).confusion == 0 {
                let (lo, hi) = data_turns(fx, (2, 5));
                self.vol(target).confusion = rules::roll_turns(lo, hi, rng);
                self.status
                    .events
                    .push(StatusEvent::Confused { target });
            }
        } else if !self.is_trapped(target) {
            let (lo, hi) = data_turns(fx, (5, 6));
            self.vol(target).trap = Some(Trap {
                by: actor,
                turns: rules::roll_turns(lo, hi, rng),
            });
            self.status
                .events
                .push(StatusEvent::Trapped { target, by: actor });
        }
    }

    /// End-of-round: trap damage, Protect bookkeeping, cleanup of fainted combatants.
    pub(super) fn status_end_of_turn(&mut self, events: &mut Vec<Event>) {
        if !self.status.rules.volatile_statuses {
            return;
        }
        let mut knocked_out = Vec::new();
        for i in 0..self.combatants.len() {
            if !self.combatants[i].is_alive() {
                if i < self.status.vol.len() {
                    self.clear_volatile_on_leave(i);
                }
                continue;
            }
            let Some(trap) = self.vol_ref(i).and_then(|v| v.trap) else {
                continue;
            };
            if !self.is_trapped(i) {
                self.vol(i).trap = None;
                self.status.events.push(StatusEvent::TrapEnded { target: i });
                continue;
            }
            let damage = rules::ceil_fraction(self.combatants[i].max_hp, rules::TRAP_SLIP_DENOM)
                .min(self.combatants[i].hp);
            self.combatants[i].hp -= damage;
            self.status
                .events
                .push(StatusEvent::HurtByTrap { target: i, damage });
            if self.combatants[i].hp == 0 {
                events.push(Event::KnockedOut(i));
                knocked_out.push(i);
            }
            let left = trap.turns.saturating_sub(1);
            if left == 0 {
                self.vol(i).trap = None;
                self.status.events.push(StatusEvent::TrapEnded { target: i });
            } else if let Some(t) = &mut self.vol(i).trap {
                t.turns = left;
            }
        }
        for v in &mut self.status.vol {
            if !v.protect_used_now {
                v.protect_chain = 0;
            }
            v.protect_used_now = false;
            v.protecting = false;
        }
        for k in knocked_out {
            self.advance_active_for(k);
        }
    }
}

#[derive(Clone, Copy)]
enum Flag {
    Charge,
    Protectable,
}

/// Duration range from the move's ailment columns, or the fallback when absent.
fn data_turns(fx: &MoveEffects, fallback: (u8, u8)) -> (u8, u8) {
    match fx.ailment_turns {
        Some((_, lo, hi)) if hi.max(lo) > 0 => (lo, hi),
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::BattleData;
    use crate::battle_engine::effects::MoveEffectsTable;
    use crate::battle_engine::{Action, Combatant};
    use std::path::PathBuf;

    /// Returns the queued values, then `rest` forever.
    struct Seq(Vec<u32>, u32);
    impl BattleRng for Seq {
        fn next_u32(&mut self) -> u32 {
            if self.0.is_empty() {
                self.1
            } else {
                self.0.remove(0)
            }
        }
    }

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    fn duel(a: &[&str], b: &[&str], rules: StatusRules) -> Battle {
        let d = BattleData::load(root()).unwrap();
        let mon = |species: &str, moves: &[&str]| {
            let sp = d.species_row(species).unwrap().unwrap();
            let mv = moves.iter().map(|n| d.move_row(n).unwrap().unwrap()).collect();
            Combatant::from_base(sp, 30, mv)
        };
        let mut battle = Battle::new(
            vec![mon("bidoof", a), mon("pikachu", b)],
            vec![vec![0], vec![1]],
            d.type_chart().clone(),
        )
        .unwrap();
        battle.set_move_effects(MoveEffectsTable::load(root()).unwrap());
        battle.set_status_rules(rules);
        battle
    }

    fn vol_rules() -> StatusRules {
        StatusRules {
            volatile_statuses: true,
            ..StatusRules::default()
        }
    }

    fn act(actor: usize) -> Action {
        Action { actor, move_slot: 0 }
    }

    fn pass(b: &mut Battle, a0: bool, a1: bool, rng: &mut impl BattleRng) -> Vec<Event> {
        let mut acts = vec![];
        if a0 {
            acts.push(act(0));
        }
        if a1 {
            acts.push(act(1));
        }
        b.play_turn(&acts, rng)
    }

    #[test]
    fn data_columns_identify_protect_moves() {
        let t = MoveEffectsTable::load(root()).unwrap();
        for n in ["protect", "detect", "kings-shield", "spiky-shield", "baneful-bunker", "obstruct", "max-guard"] {
            assert!(is_protect_move(t.get(n).unwrap()), "{n}");
        }
        for n in ["endure", "magic-coat", "snatch", "tackle", "counter"] {
            assert!(!is_protect_move(t.get(n).unwrap()), "{n}");
        }
    }

    #[test]
    fn default_rules_change_nothing() {
        let mut b = duel(&["confuse-ray"], &["tackle"], StatusRules::default());
        let mut rng = Seq(vec![], 0);
        pass(&mut b, true, false, &mut rng);
        assert!(!b.is_confused(1));
        assert!(b.take_status_events().is_empty());
    }

    #[test]
    fn confusion_is_applied_lasts_and_self_hits() {
        let mut b = duel(&["confuse-ray"], &["tackle"], vol_rules());
        b.combatants[0].max_hp = 5000;
        b.combatants[0].hp = 5000;
        // accuracy roll 0 hits; status move chance 0 = always; duration 2 + 3 = 5 (data 2..=5).
        let mut rng = Seq(vec![0, 3], 50);
        pass(&mut b, true, false, &mut rng);
        assert!(b.is_confused(1));
        assert!(b.take_status_events().contains(&StatusEvent::Confused { target: 1 }));
        // Victim acts: counter 5 -> 4, roll 10 < 33 -> hits itself and loses the action.
        let hp = b.combatants[1].hp;
        let mut rng = Seq(vec![10, 0], 50);
        pass(&mut b, false, true, &mut rng);
        let ev = b.take_status_events();
        assert!(ev.iter().any(|e| matches!(e, StatusEvent::HurtByConfusion { actor: 1, damage } if *damage > 0)));
        assert!(b.combatants[1].hp < hp);
        assert_eq!(b.combatants[0].hp, b.combatants[0].max_hp, "no damage to the foe");
        // Roll 90 >= 33: acts normally (tackle lands on bidoof).
        let mut rng = Seq(vec![90, 0], 50);
        pass(&mut b, false, true, &mut rng);
        assert!(b.combatants[0].hp < b.combatants[0].max_hp);
        // Two more attempts bring the counter 3 -> 2 -> 1, then 1 -> 0 snaps out.
        let mut rng = Seq(vec![], 99);
        for _ in 0..3 {
            pass(&mut b, false, true, &mut rng);
        }
        assert!(!b.is_confused(1));
        assert!(b.take_status_events().contains(&StatusEvent::SnappedOutOfConfusion { target: 1 }));
    }

    #[test]
    fn confusion_damage_formula_is_typeless_forty_power() {
        let mut b = duel(&["tackle"], &["tackle"], vol_rules());
        b.set_pla_damage_formula(true);
        let c = &b.combatants[1];
        // PLA base formula: floor(floor(floor(40*(A+100+15L))/(D+50))/5).
        let l = u32::from(c.level);
        let base = 40 * (c.attack + 100 + 15 * l) / (c.defense + 50) / 5;
        assert_eq!(base, rules::pla_base_damage(40, c.attack, l, c.defense));
        let mut rng = Seq(vec![15], 0); // roll 100 %
        assert_eq!(b.confusion_damage(1, &mut rng), base);
        let mut rng = Seq(vec![0], 0); // roll 85 %
        assert_eq!(b.confusion_damage(1, &mut rng), (base * 85 / 100).max(1));
    }

    #[test]
    fn drowsy_fails_a_third_of_actions_and_two_thirds_in_snow() {
        let rules = StatusRules {
            sleep_duration: true,
            ..StatusRules::default()
        };
        let mut b = duel(&["tackle"], &["tackle"], rules);
        b.combatants[1].status = Some(MajorStatus::Drowsy);
        let unable = |ev: &[Event]| ev.iter().any(|e| matches!(e, Event::UnableToAct { actor: 1, .. }));
        // roll 32 < 33 fails; roll 33 acts (no forced multi-turn sleep, REFUTED).
        assert!(unable(&pass(&mut b, false, true, &mut Seq(vec![32], 99))));
        assert!(!unable(&pass(&mut b, false, true, &mut Seq(vec![33], 99))));
        assert_eq!(b.combatants[1].status, Some(MajorStatus::Drowsy), "no wake-up roll");
        // Snow doubles the chance: roll 65 < 66 fails, 66 acts.
        b.weather = BattleWeather::Snow;
        assert!(unable(&pass(&mut b, false, true, &mut Seq(vec![65], 99))));
        assert!(!unable(&pass(&mut b, false, true, &mut Seq(vec![66], 99))));
    }

    #[test]
    fn trapping_blocks_switch_hurts_each_round_and_expires() {
        let d = BattleData::load(root()).unwrap();
        let mon = |s: &str, m: &[&str]| {
            let sp = d.species_row(s).unwrap().unwrap();
            let mv = m.iter().map(|n| d.move_row(n).unwrap().unwrap()).collect();
            Combatant::from_base(sp, 30, mv)
        };
        let mut b = Battle::new(
            vec![mon("bidoof", &["bind"]), mon("pikachu", &["tackle"]), mon("bidoof", &["tackle"])],
            vec![vec![0], vec![1, 2]],
            d.type_chart().clone(),
        )
        .unwrap();
        b.set_move_effects(MoveEffectsTable::load(root()).unwrap());
        b.set_status_rules(vol_rules());
        for c in &mut b.combatants {
            c.max_hp = 400;
            c.hp = 400;
        }
        // accuracy roll 0, crit/roll draws, then duration 5 + 0 (bind data: 5..=6).
        let mut rng = Seq(vec![0, 0, 0], 0);
        b.play_turn(&[act(0)], &mut rng);
        assert!(b.is_trapped(1));
        let hp_after_hit = b.combatants[1].hp;
        assert!(b.switch(1, 2).is_err(), "cannot leave while trapped");
        assert!(b.take_status_events().iter().any(|e| matches!(e, StatusEvent::CantSwitch { .. })));
        // Residual damage is ceil(400/8) = 50 per round; bind itself also damages on its own turn.
        let mut rng = Seq(vec![], 99); // bidoof's later binds miss, only trap damage happens
        let mut total = 0;
        let mut rounds = 0;
        while b.is_trapped(1) && rounds < 10 {
            let before = b.combatants[1].hp;
            b.play_turn(&[], &mut rng);
            total += before - b.combatants[1].hp;
            rounds += 1;
        }
        assert!(hp_after_hit > 0);
        assert_eq!(rounds, 4, "5 rounds of trap in total, one already spent at the first end of turn");
        assert_eq!(total, 4 * 50);
        assert!(b.switch(1, 2).is_ok());
    }

    #[test]
    fn trap_ends_when_trapper_leaves() {
        let mut b = duel(&["bind"], &["tackle"], vol_rules());
        let mut rng = Seq(vec![0, 0, 1], 0);
        b.play_turn(&[act(0)], &mut rng);
        assert!(b.is_trapped(1));
        b.clear_volatile_on_leave(0);
        assert!(!b.is_trapped(1));
    }

    #[test]
    fn charge_move_spends_turn_one_and_attacks_on_turn_two() {
        // solar-beam: flag_charge; bidoof is the attacker.
        let mut b = duel(&["solar-beam"], &["growl"], vol_rules());
        let pp0 = b.combatants[0].moves[0].pp;
        let mut rng = Seq(vec![], 0);
        let ev = b.play_turn(&[act(0)], &mut rng);
        assert!(b.is_charging(0));
        assert_eq!(b.combatants[1].hp, b.combatants[1].max_hp, "no damage on the charge turn");
        assert!(!ev.iter().any(|e| matches!(e, Event::UsedMove { .. })));
        assert!(b.take_status_events().iter().any(|e| matches!(e, StatusEvent::Charging { actor: 0, .. })));
        assert_eq!(b.combatants[0].moves[0].pp, pp0 - 1);
        // Turn two: even if the player picked another slot/none, the stored move fires.
        let ev = b.play_turn(&[act(0)], &mut rng);
        assert!(!b.is_charging(0));
        assert!(ev.iter().any(|e| matches!(e, Event::UsedMove { actor: 0, damage, .. } if *damage > 0)));
        assert_eq!(b.combatants[0].moves[0].pp, pp0 - 1, "PP is spent once for the two turns");
    }

    #[test]
    fn recharge_flag_no_longer_skips_the_next_action() {
        // REFUTED: the column is also set on dire-claw and psyshield-bash, which never recharge.
        let t = MoveEffectsTable::load(root()).unwrap();
        assert!(t.get("psyshield-bash").unwrap().recharge);
        let mut b = duel(&["hyper-beam"], &["tackle"], vol_rules());
        b.combatants[1].max_hp = 5000;
        b.combatants[1].hp = 5000;
        let mut rng = Seq(vec![], 0);
        b.play_turn(&[act(0)], &mut rng);
        assert!(!b.must_recharge(0));
        let hp = b.combatants[1].hp;
        b.play_turn(&[act(0)], &mut rng);
        assert!(b.combatants[1].hp < hp, "acts again on the next turn");
    }

    #[test]
    fn protect_blocks_protectable_moves_and_chain_halves_odds() {
        // Pikachu protects (priority +4 acts first); bidoof tackles.
        let mut b = duel(&["tackle"], &["protect"], vol_rules());
        let mut rng = Seq(vec![], 0);
        let ev = b.play_turn(&[act(0), act(1)], &mut rng);
        assert_eq!(b.combatants[1].hp, b.combatants[1].max_hp);
        assert!(!ev.iter().any(|e| matches!(e, Event::UsedMove { actor: 0, .. })));
        let se = b.take_status_events();
        assert!(se.contains(&StatusEvent::ProtectUp { actor: 1 }));
        assert!(se.iter().any(|e| matches!(e, StatusEvent::ProtectedFrom { target: 1, attacker: 0, .. })));
        // Second consecutive use needs a 1-in-3 roll: roll 1 fails and resets the chain.
        let mut rng = Seq(vec![1], 0);
        b.play_turn(&[act(0), act(1)], &mut rng);
        let se = b.take_status_events();
        assert!(se.contains(&StatusEvent::ProtectFailed { actor: 1 }));
        assert!(b.combatants[1].hp < b.combatants[1].max_hp, "tackle landed after the failure");
        // Chain was reset: next use is guaranteed again.
        let mut rng = Seq(vec![], 7);
        b.play_turn(&[act(0), act(1)], &mut rng);
        assert!(b.take_status_events().contains(&StatusEvent::ProtectUp { actor: 1 }));
        // Roll 0 on the following consecutive use succeeds (odds 1/3).
        let mut rng = Seq(vec![0], 0);
        b.play_turn(&[act(0), act(1)], &mut rng);
        assert!(b.take_status_events().contains(&StatusEvent::ProtectUp { actor: 1 }));
    }

    #[test]
    fn protect_does_not_stop_moves_that_are_not_protectable() {
        let t = MoveEffectsTable::load(root()).unwrap();
        // The flag column decides: a move without it passes through.
        assert!(t.get("tackle").unwrap().protectable);
        assert!(!t.get("swords-dance").unwrap().protectable);
    }

    #[test]
    fn speed_rank_does_not_reorder_turns() {
        // REFUTED: the game's agility ignores rank states, with or without the switch.
        for enabled in [false, true] {
            let rules = StatusRules {
                speed_stages: enabled,
                ..StatusRules::default()
            };
            let mut b = duel(&["tackle"], &["tackle"], rules);
            b.combatants[0].speed = 100;
            b.combatants[1].speed = 120;
            b.stages_mut(0).speed = 1;
            let mut rng = Seq(vec![], 99);
            let ev = b.play_turn(&[act(0), act(1)], &mut rng);
            assert!(ev.contains(&Event::ActedFirst(1)), "enabled={enabled}");
        }
    }

    #[test]
    fn stage_multipliers_follow_the_config_flag() {
        let off = StatusRules::default();
        let on = StatusRules {
            stage_multipliers: true,
            ..StatusRules::default()
        };
        let dmg = |rules: StatusRules| {
            let mut b = duel(&["tackle"], &["growl"], rules);
            b.stages_mut(0).attack = 1;
            let mut rng = Seq(vec![], 0);
            b.play_turn(&[act(0)], &mut rng);
            b.combatants[1].max_hp - b.combatants[1].hp
        };
        assert!(dmg(on) > dmg(off));
    }

    #[test]
    fn notes_rates_use_recovered_slip_denominators() {
        let mut b = duel(&["tackle"], &["tackle"], StatusRules::default());
        b.combatants[1].max_hp = 100;
        assert_eq!(b.residual_damage(1, MajorStatus::Poison), 12, "legacy floor(1/8)");
        b.set_status_rules(StatusRules {
            notes_rates: true,
            ..StatusRules::default()
        });
        // VERIFIED ceil(max/denom): 100/6 -> 17, 100/12 -> 9.
        assert_eq!(b.residual_damage(1, MajorStatus::Poison), 17);
        assert_eq!(b.residual_damage(1, MajorStatus::Burn), 9);
        assert_eq!(b.residual_damage(1, MajorStatus::Frostbite), 9);
        b.combatants[1].max_hp = 120;
        b.combatants[1].hp = 120;
        b.combatants[1].status = Some(MajorStatus::Poison);
        let mut rng = Seq(vec![], 0);
        b.play_turn(&[], &mut rng);
        assert_eq!(b.combatants[1].hp, 100);
    }
}
