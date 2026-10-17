//! Data-driven move effects (priority, secondary status, stat stages, drain/recoil,
//! multi-hit, healing, flinch) loaded from `data/pml_move_effects.tsv`.
//!
//! Everything here is additive: a `Battle` without an effects table behaves exactly as
//! before. Confidence labels follow `reports/data-progress/battle_engine.md`:
//! the decoded columns are VERIFIED/LIKELY (see the report); the way they are applied to a
//! battle is a simulation of the recovered data, not a claim of recovered engine behavior.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::{Battle, BattleRng, Combatant, MajorStatus};

/// Stat ids used by the move data (LIKELY): 1 Atk, 2 Def, 3 SpA, 4 SpD, 5 Spe, 6 Acc, 7 Eva, 8 all five.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stat {
    Attack,
    Defense,
    SpecialAttack,
    SpecialDefense,
    Speed,
    Accuracy,
    Evasion,
    AllBattleStats,
}

impl Stat {
    pub fn from_id(id: u8) -> Option<Self> {
        Some(match id {
            1 => Self::Attack,
            2 => Self::Defense,
            3 => Self::SpecialAttack,
            4 => Self::SpecialDefense,
            5 => Self::Speed,
            6 => Self::Accuracy,
            7 => Self::Evasion,
            8 => Self::AllBattleStats,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatChange {
    pub stat: Stat,
    /// Signed stage delta from the data (e.g. +2 Agility, -1 Growl).
    pub stage: i8,
    /// Percent chance; the data stores 0 for "always" on pure status moves.
    pub chance: u8,
    /// Normal-style duration in turns (`raw_f31..raw_f33`, VERIFIED reader); 0 = no data.
    pub turns: u8,
}

/// Which combatant receives stat changes / healing (derived from meta category and target id; LIKELY).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectTarget {
    User,
    Opponent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveEffects {
    pub name: String,
    pub priority: i8,
    pub crit_rank: u8,
    pub flinch_chance: u8,
    pub min_hits: u8,
    pub max_hits: u8,
    /// >0: percent of damage dealt healed; <0: percent of damage dealt taken as recoil.
    pub drain_pct: i8,
    /// >0: percent of max HP healed; <0: percent of max HP lost by the user.
    pub heal_pct: i8,
    pub ailment: Option<MajorStatus>,
    pub ailment_id: u16,
    pub ailment_chance: u8,
    /// Ailment duration `(kind, min, max)` from `raw_f13..raw_f15` (VERIFIED reader); kinds
    /// 2 and 4 are turn-limited. `None` when the columns are absent.
    pub ailment_turns: Option<(u8, u8, u8)>,
    pub meta_category: u8,
    pub target_raw: u8,
    pub stat_changes: Vec<StatChange>,
    pub contact: bool,
    /// Raw `effect_id` column (RAW, used only to recognise Protect-style moves).
    pub effect_id: u16,
    pub charge: bool,
    pub recharge: bool,
    pub protectable: bool,
    /// The move cannot be used on the turn right after it was used (`cantusetwice`).
    pub repeat_lock: bool,
}

impl MoveEffects {
    /// Receiver of the stat changes: damage+raise (7) and self targets affect the user.
    pub fn stat_target(&self) -> EffectTarget {
        if self.meta_category == 7 || self.target_raw == 7 {
            EffectTarget::User
        } else {
            EffectTarget::Opponent
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MoveEffectsTable {
    by_name: HashMap<String, MoveEffects>,
}

impl MoveEffectsTable {
    /// Load `<data_root>/pml_move_effects.tsv`.
    pub fn load(data_root: impl AsRef<Path>) -> Result<Self, String> {
        let path = data_root.as_ref().join("pml_move_effects.tsv");
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut table = Self::from_tsv(&text)?;
        // Generation-9 moves (external sheet, class `a` only); the PLA rows always win by name.
        for mv in super::gen9::load(data_root.as_ref())? {
            table.by_name.entry(mv.name).or_insert(mv.effects);
        }
        Ok(table)
    }

    pub fn from_tsv(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'));
        let header: Vec<&str> = lines
            .next()
            .ok_or("move effects: missing header")?
            .split('\t')
            .map(|h| h.split(':').next().unwrap_or(h))
            .collect();
        let col = |name: &str| -> Result<usize, String> {
            header
                .iter()
                .position(|h| *h == name)
                .ok_or_else(|| format!("move effects: missing column {name}"))
        };
        let (c_name, c_prio, c_crit, c_flinch) = (
            col("move_name")?,
            col("priority")?,
            col("crit_rank")?,
            col("flinch_chance")?,
        );
        let (c_minh, c_maxh, c_drain, c_heal) = (
            col("min_hits")?,
            col("max_hits")?,
            col("drain_pct")?,
            col("heal_pct")?,
        );
        let (c_aid, c_ach, c_meta, c_tgt, c_contact) = (
            col("ailment_id")?,
            col("ailment_chance")?,
            col("meta_category")?,
            col("target_raw")?,
            col("flag_contact")?,
        );
        let (c_eid, c_charge, c_recharge, c_protectable) = (
            col("effect_id")?,
            col("flag_charge")?,
            col("flag_recharge")?,
            col("flag_protectable")?,
        );
        let stat_cols = [
            (col("stat1_id")?, col("stat1_stage")?, col("stat1_chance")?, col("raw_f31").ok()),
            (col("stat2_id")?, col("stat2_stage")?, col("stat2_chance")?, col("raw_f32").ok()),
            (col("stat3_id")?, col("stat3_stage")?, col("stat3_chance")?, col("raw_f33").ok()),
        ];
        let turn_cols = (col("raw_f13").ok(), col("raw_f14").ok(), col("raw_f15").ok());
        let mut by_name = HashMap::new();
        for (n, line) in lines.enumerate() {
            let v: Vec<&str> = line.split('\t').collect();
            if v.len() != header.len() {
                return Err(format!("move effects row {}: column count", n + 2));
            }
            let num = |i: usize| -> Result<i64, String> {
                v[i].parse::<i64>()
                    .map_err(|_| format!("move effects row {}: bad number {:?}", n + 2, v[i]))
            };
            let ailment_id = num(c_aid)? as u16;
            let mut stat_changes = Vec::new();
            let opt = |c: Option<usize>| -> Result<u8, String> {
                c.map_or(Ok(0), |i| num(i).map(|x| x as u8))
            };
            for (si, sg, sc, st) in stat_cols {
                if let Some(stat) = Stat::from_id(num(si)? as u8) {
                    let stage = num(sg)? as i8;
                    if stage != 0 {
                        stat_changes.push(StatChange {
                            stat,
                            stage,
                            chance: num(sc)? as u8,
                            turns: opt(st)?,
                        });
                    }
                }
            }
            let fx = MoveEffects {
                name: v[c_name].to_owned(),
                priority: num(c_prio)? as i8,
                crit_rank: num(c_crit)? as u8,
                flinch_chance: num(c_flinch)? as u8,
                min_hits: num(c_minh)? as u8,
                max_hits: num(c_maxh)? as u8,
                drain_pct: num(c_drain)? as i8,
                heal_pct: num(c_heal)? as i8,
                ailment: ailment_status(ailment_id),
                ailment_id,
                ailment_chance: num(c_ach)? as u8,
                ailment_turns: match turn_cols {
                    (Some(k), Some(lo), Some(hi)) => {
                        Some((num(k)? as u8, num(lo)? as u8, num(hi)? as u8))
                    }
                    _ => None,
                },
                meta_category: num(c_meta)? as u8,
                target_raw: num(c_tgt)? as u8,
                stat_changes,
                contact: num(c_contact)? != 0,
                effect_id: num(c_eid)? as u16,
                charge: num(c_charge)? != 0,
                recharge: num(c_recharge)? != 0,
                protectable: num(c_protectable)? != 0,
                // Only the external generation-9 sheet carries this flag.
                repeat_lock: false,
            };
            // Source has a duplicate move id 1 (two files); first occurrence wins by name.
            by_name.entry(fx.name.clone()).or_insert(fx);
        }
        Ok(Self { by_name })
    }

    pub fn get(&self, name: &str) -> Option<&MoveEffects> {
        self.by_name.get(name)
    }

    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    /// Copy real priority and critical rank into a combatant's move slots.
    pub fn apply_to(&self, combatant: &mut Combatant) {
        for slot in &mut combatant.moves {
            if let Some(fx) = self.get(&slot.data.name) {
                slot.priority = fx.priority;
                slot.critical_rank = fx.crit_rank;
            }
        }
    }
}

/// Ailment ids 2 and 6 are both sleep-like in the data (distinct meaning UNKNOWN);
/// 3 (freeze-like), 8 (confusion), 9, 10 and 65535 are not modeled.
pub fn ailment_status(id: u16) -> Option<MajorStatus> {
    match id {
        1 => Some(MajorStatus::Paralysis),
        2 | 6 => Some(MajorStatus::Drowsy),
        4 => Some(MajorStatus::Burn),
        5 => Some(MajorStatus::Poison),
        7 => Some(MajorStatus::Frostbite),
        _ => None,
    }
}

/// PLA stat ranks have only three states per group (VERIFIED): -1 down, 0 neutral, +1 up.
/// Attack/Special Attack share one state, Defense/Special Defense share one, and Speed,
/// accuracy and evasion share the third (`speed`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stages {
    pub attack: i8,
    pub defense: i8,
    pub speed: i8,
}

/// Effect log kept beside the stable `Event` stream (new variants would break callers'
/// exhaustive matches). Drain with `Battle::take_effect_events`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectEvent {
    Hits {
        actor: usize,
        count: u32,
    },
    StagesChanged {
        target: usize,
        stat: Stat,
        new_state: i8,
    },
    StageAlreadyAtLimit {
        target: usize,
        stat: Stat,
    },
    Healed {
        target: usize,
        amount: u32,
    },
    Drained {
        actor: usize,
        amount: u32,
    },
    RecoilDamage {
        actor: usize,
        amount: u32,
    },
    HpCost {
        actor: usize,
        amount: u32,
    },
    Flinched {
        target: usize,
    },
    FlinchedAndSkipped {
        actor: usize,
    },
}

/// Q12 forms of the rank coefficients. The game applies them as floats to the base damage
/// (see `rules::apply_rank_states`); these constants are kept for API compatibility.
pub const STAGE_UP_Q12: u32 = 6144; // 1.5 (VERIFIED coefficient)
pub const STAGE_DOWN_Q12: u32 = 2703; // 0.66 (VERIFIED coefficient)

impl Battle {
    /// Attach the effects table and copy real priority / critical rank into every move slot.
    pub fn set_move_effects(&mut self, table: MoveEffectsTable) {
        for c in &mut self.combatants {
            table.apply_to(c);
        }
        self.effects = Some(table);
    }

    pub fn move_effects(&self) -> Option<&MoveEffectsTable> {
        self.effects.as_ref()
    }

    /// Apply the 3-state rank multipliers to the base damage (VERIFIED placement, see
    /// `rules::apply_rank_states`) and the end-of-turn rank countdown. Off by default to keep
    /// old callers unchanged; `StatusRules::game_rules()` turns it on.
    pub fn set_stage_multipliers_enabled(&mut self, on: bool) {
        self.stage_multipliers = on;
    }

    pub fn stages(&self, combatant: usize) -> Stages {
        self.stages.get(combatant).copied().unwrap_or_default()
    }

    pub fn take_effect_events(&mut self) -> Vec<EffectEvent> {
        std::mem::take(&mut self.effect_events)
    }

    pub(super) fn stages_mut(&mut self, combatant: usize) -> &mut Stages {
        if self.stages.len() <= combatant {
            self.stages.resize(combatant + 1, Stages::default());
        }
        &mut self.stages[combatant]
    }

    /// Apply one stat change; each rank group moves within -1..=1.
    pub(super) fn change_stat(&mut self, target: usize, stat: Stat, delta: i8) {
        if stat == Stat::AllBattleStats {
            for s in [
                Stat::Attack,
                Stat::Defense,
                Stat::SpecialAttack,
                Stat::SpecialDefense,
                Stat::Speed,
            ] {
                self.change_stat(target, s, delta);
            }
            return;
        }
        // VERIFIED: Speed, accuracy and evasion changes all move the shared third group.
        let (old, new) = {
            let st = self.stages_mut(target);
            let slot = match stat {
                Stat::Attack | Stat::SpecialAttack => &mut st.attack,
                Stat::Defense | Stat::SpecialDefense => &mut st.defense,
                _ => &mut st.speed,
            };
            let old = *slot;
            *slot = (old + delta).clamp(-1, 1);
            (old, *slot)
        };
        if old == new {
            self.effect_events
                .push(EffectEvent::StageAlreadyAtLimit { target, stat });
        } else {
            self.effect_events.push(EffectEvent::StagesChanged {
                target,
                stat,
                new_state: new,
            });
        }
    }

    pub(super) fn heal(&mut self, target: usize, amount: u32) {
        let c = &mut self.combatants[target];
        let healed = amount.min(c.max_hp - c.hp);
        c.hp += healed;
        self.effect_events.push(EffectEvent::Healed {
            target,
            amount: healed,
        });
    }

    /// Number of hits (1 without data). Uniform inside [min, max] is a placeholder: the
    /// game's multi-hit distribution is UNKNOWN. No RNG is consumed for fixed counts.
    pub(super) fn roll_hits(fx: &MoveEffects, rng: &mut impl BattleRng) -> u32 {
        let (lo, hi) = (fx.min_hits.max(1), fx.max_hits.max(fx.min_hits).max(1));
        if lo == hi {
            u32::from(lo)
        } else {
            u32::from(lo) + rng.below(u32::from(hi - lo) + 1)
        }
    }

    fn chance_passes(chance: u8, always_if_zero: bool, rng: &mut impl BattleRng) -> bool {
        let chance = if chance == 0 && always_if_zero {
            100
        } else {
            chance
        };
        match chance {
            0 => false,
            100.. => true,
            c => rng.below(100) < u32::from(c),
        }
    }

    /// Secondary/status effects after a move connected. `damage_dealt` is HP actually removed.
    pub(super) fn apply_move_effects(
        &mut self,
        actor: usize,
        target: usize,
        fx: &MoveEffects,
        damaging: bool,
        damage_dealt: u32,
        flinch_targets: &mut Vec<usize>,
        rng: &mut impl BattleRng,
    ) -> Vec<usize> {
        let mut knocked_out = Vec::new();
        // Drain / recoil of damage dealt.
        if damaging && damage_dealt > 0 && fx.drain_pct != 0 {
            let amount = (damage_dealt * u32::from(fx.drain_pct.unsigned_abs()) / 100).max(1);
            if fx.drain_pct > 0 {
                self.heal(actor, amount);
                self.effect_events
                    .push(EffectEvent::Drained { actor, amount });
            } else {
                let c = &mut self.combatants[actor];
                let lost = amount.min(c.hp);
                c.hp -= lost;
                self.effect_events.push(EffectEvent::RecoilDamage {
                    actor,
                    amount: lost,
                });
                if self.combatants[actor].hp == 0 {
                    knocked_out.push(actor);
                }
            }
        }
        // Healing / HP cost as a share of the user's max HP.
        if fx.heal_pct > 0 {
            let amount = self.combatants[actor].max_hp * u32::from(fx.heal_pct as u8) / 100;
            self.heal(actor, amount);
        } else if fx.heal_pct < 0 {
            let c = &mut self.combatants[actor];
            let lost = (c.max_hp * u32::from(fx.heal_pct.unsigned_abs()) / 100).min(c.hp);
            c.hp -= lost;
            self.effect_events.push(EffectEvent::HpCost {
                actor,
                amount: lost,
            });
            if self.combatants[actor].hp == 0 && !knocked_out.contains(&actor) {
                knocked_out.push(actor);
            }
        }
        // Stat stage changes.
        let receiver = match fx.stat_target() {
            EffectTarget::User => actor,
            EffectTarget::Opponent => target,
        };
        let alive_receiver = self.combatants[receiver].is_alive();
        for change in &fx.stat_changes {
            if alive_receiver && Self::chance_passes(change.chance, !damaging, rng) {
                self.change_stat(receiver, change.stat, change.stage);
                self.note_stage_turns(receiver, change.stat, change.turns);
            }
        }
        // Major status.
        if let Some(status) = fx.ailment {
            if self.combatants[target].is_alive()
                && self.combatants[target].status.is_none()
                && Self::chance_passes(fx.ailment_chance, !damaging, rng)
            {
                self.combatants[target].status = Some(status);
                self.start_status_turns(target, fx, rng);
                self.pending_status.push((target, status));
            }
        }
        // Flinch: only matters if the target has not acted yet (checked by play_turn).
        if damaging
            && fx.flinch_chance > 0
            && self.combatants[target].is_alive()
            && Self::chance_passes(fx.flinch_chance, false, rng)
        {
            flinch_targets.push(target);
            self.effect_events.push(EffectEvent::Flinched { target });
        }
        knocked_out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::BattleData;
    use crate::battle_engine::{Action, Combatant, Event};
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

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    fn mon(d: &BattleData, species: &str, moves: &[&str], level: u8) -> Combatant {
        let sp = d.species_row(species).unwrap().unwrap();
        let mv = moves
            .iter()
            .map(|n| d.move_row(n).unwrap().unwrap())
            .collect();
        Combatant::from_base(sp, level, mv)
    }

    fn duel(a: (&str, &[&str], u8), b: (&str, &[&str], u8)) -> Battle {
        let d = BattleData::load(root()).unwrap();
        let mut battle = Battle::new(
            vec![mon(&d, a.0, a.1, a.2), mon(&d, b.0, b.1, b.2)],
            vec![vec![0], vec![1]],
            d.type_chart().clone(),
        )
        .unwrap();
        battle.set_move_effects(MoveEffectsTable::load(root()).unwrap());
        battle
    }

    fn act(actor: usize) -> Action {
        Action {
            actor,
            move_slot: 0,
        }
    }

    #[test]
    fn parses_checked_in_effects_sheet() {
        let t = MoveEffectsTable::load(root()).unwrap();
        assert!(t.len() > 800);
        assert_eq!(t.get("quick-attack").unwrap().priority, 1);
        assert_eq!(
            t.get("thunder-wave").unwrap().ailment,
            Some(MajorStatus::Paralysis)
        );
        assert_eq!(t.get("recover").unwrap().heal_pct, 50);
        assert_eq!(t.get("double-edge").unwrap().drain_pct, -33);
        assert_eq!(t.get("fury-swipes").unwrap().max_hits, 5);
        assert_eq!(t.get("growl").unwrap().stat_changes[0].stage, -1);
    }

    #[test]
    fn real_priority_lets_a_slow_quick_attack_user_move_first() {
        let mut b = duel(
            ("bidoof", &["quick-attack"], 20),
            ("starly", &["tackle"], 20),
        );
        assert!(b.combatants[0].speed < b.combatants[1].speed);
        assert_eq!(b.combatants[0].moves[0].priority, 1);
        let ev = b.play_turn(&[act(0), act(1)], &mut Fixed(vec![]));
        assert_eq!(ev[1], Event::ActedFirst(0));
    }

    #[test]
    fn thunder_wave_always_paralyses_after_its_accuracy_roll() {
        let mut b = duel(
            ("pikachu", &["thunder-wave"], 30),
            ("starly", &["tackle"], 5),
        );
        let ev = b.play_turn(&[act(0)], &mut Fixed(vec![0]));
        assert!(ev.contains(&Event::StatusApplied {
            target: 1,
            status: MajorStatus::Paralysis
        }));
        // accuracy 90: a roll of 95 misses and nothing is applied
        let mut b = duel(
            ("pikachu", &["thunder-wave"], 30),
            ("starly", &["tackle"], 5),
        );
        let ev = b.play_turn(&[act(0)], &mut Fixed(vec![95]));
        assert!(b.combatants[1].status.is_none());
        assert!(matches!(ev[2], Event::Missed { .. }));
    }

    #[test]
    fn flamethrower_burn_uses_its_secondary_chance() {
        // rolls: accuracy, critical, damage roll, burn chance (data: 20%)
        let mut b = duel(
            ("cyndaquil", &["flamethrower"], 30),
            ("snorlax", &["tackle"], 60),
        );
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0, 19]));
        assert_eq!(b.combatants[1].status, Some(MajorStatus::Burn));
        let mut b = duel(
            ("cyndaquil", &["flamethrower"], 30),
            ("snorlax", &["tackle"], 60),
        );
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0, 20]));
        assert_eq!(b.combatants[1].status, None);
    }

    #[test]
    fn stat_stages_use_three_states_and_report_limits() {
        let mut b = duel(
            ("pikachu", &["swords-dance"], 30),
            ("starly", &["growl"], 5),
        );
        b.play_turn(&[act(0), act(1)], &mut Fixed(vec![0, 0, 0, 0]));
        // swords-dance raises the user's attack (data: +1); growl lowers the user's attack (-1 on foe)
        assert_eq!(b.stages(0).attack, 0);
        let mut b = duel(
            ("pikachu", &["swords-dance"], 30),
            ("starly", &["tackle"], 5),
        );
        b.play_turn(&[act(0)], &mut Fixed(vec![]));
        assert_eq!(b.stages(0).attack, 1);
        b.play_turn(&[act(0)], &mut Fixed(vec![]));
        assert_eq!(b.stages(0).attack, 1);
        assert!(b
            .take_effect_events()
            .contains(&EffectEvent::StageAlreadyAtLimit {
                target: 0,
                stat: Stat::Attack
            }));
        let mut b = duel(("pikachu", &["growl"], 30), ("starly", &["tackle"], 5));
        b.play_turn(&[act(0)], &mut Fixed(vec![]));
        assert_eq!(b.stages(1).attack, -1);
        let mut b = duel(("pikachu", &["agility"], 30), ("starly", &["tackle"], 5));
        b.play_turn(&[act(0)], &mut Fixed(vec![]));
        assert_eq!(b.stages(0).speed, 1);
    }

    #[test]
    fn stage_multiplier_option_changes_damage_only_when_enabled() {
        let run = |enabled: bool| {
            let mut b = duel(
                ("pikachu", &["thunderbolt"], 30),
                ("starly", &["tackle"], 5),
            );
            b.set_stage_multipliers_enabled(enabled);
            b.stages_mut(0).attack = 1;
            let mut rng = Fixed(vec![0, 1, 0, 99]);
            let ev = b.play_turn(&[act(0)], &mut rng);
            ev.iter()
                .find_map(|e| match e {
                    Event::UsedMove { damage, .. } => Some(*damage),
                    _ => None,
                })
                .unwrap()
        };
        assert!(run(true) > run(false));
    }

    #[test]
    fn drain_and_recoil_follow_damage_dealt() {
        let mut b = duel(("rowlet", &["giga-drain"], 40), ("starly", &["tackle"], 5));
        b.combatants[0].hp = 1;
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        let healed = b
            .take_effect_events()
            .into_iter()
            .find_map(|e| match e {
                EffectEvent::Drained { amount, .. } => Some(amount),
                _ => None,
            })
            .unwrap();
        let dealt = b.combatants[1].max_hp - b.combatants[1].hp;
        assert_eq!(healed, (dealt / 2).max(1));
        assert_eq!(
            b.combatants[0].hp,
            1 + healed.min(b.combatants[0].max_hp - 1)
        );

        let mut b = duel(
            ("bidoof", &["double-edge"], 40),
            ("pikachu", &["tackle"], 40),
        );
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        let dealt = b.combatants[1].max_hp - b.combatants[1].hp;
        let lost = b.combatants[0].max_hp - b.combatants[0].hp;
        assert_eq!(lost, (dealt * 33 / 100).max(1));
    }

    #[test]
    fn fixed_multi_hit_moves_hit_the_exact_count() {
        let mut b = duel(
            ("bidoof", &["double-kick"], 40),
            ("snorlax", &["tackle"], 60),
        );
        let ev = b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0, 1, 0]));
        assert!(b
            .take_effect_events()
            .contains(&EffectEvent::Hits { actor: 0, count: 2 }));
        let total = ev
            .iter()
            .find_map(|e| match e {
                Event::UsedMove { damage, .. } => Some(*damage),
                _ => None,
            })
            .unwrap();
        assert_eq!(total, b.combatants[1].max_hp - b.combatants[1].hp);
    }

    #[test]
    fn variable_multi_hit_uses_data_range_with_injected_roll() {
        let t = MoveEffectsTable::load(root()).unwrap();
        let fx = t.get("fury-swipes").unwrap();
        assert_eq!((fx.min_hits, fx.max_hits), (2, 5));
        assert_eq!(Battle::roll_hits(fx, &mut Fixed(vec![0])), 2);
        assert_eq!(Battle::roll_hits(fx, &mut Fixed(vec![3])), 5);
    }

    #[test]
    fn recover_heals_half_of_max_hp() {
        let mut b = duel(("pikachu", &["recover"], 30), ("starly", &["tackle"], 5));
        let max = b.combatants[0].max_hp;
        b.combatants[0].hp = 1;
        b.play_turn(&[act(0)], &mut Fixed(vec![]));
        assert_eq!(b.combatants[0].hp, 1 + max / 2);
    }

    #[test]
    fn fake_out_flinch_makes_a_slower_target_lose_its_action() {
        let mut b = duel(("bidoof", &["fake-out"], 40), ("starly", &["tackle"], 40));
        let ev = b.play_turn(&[act(0), act(1)], &mut Fixed(vec![0, 1, 0]));
        assert_eq!(ev[1], Event::ActedFirst(0));
        assert!(b
            .take_effect_events()
            .contains(&EffectEvent::FlinchedAndSkipped { actor: 1 }));
        assert_eq!(b.combatants[0].hp, b.combatants[0].max_hp);
    }

    #[test]
    fn battle_without_effects_table_is_unchanged() {
        let d = BattleData::load(root()).unwrap();
        let mut b = Battle::new(
            vec![
                mon(&d, "pikachu", &["tackle"], 20),
                mon(&d, "starly", &["tackle"], 20),
            ],
            vec![vec![0], vec![1]],
            d.type_chart().clone(),
        )
        .unwrap();
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        assert!(b.take_effect_events().is_empty());
        assert_eq!(b.stages(0), Stages::default());
    }
}
