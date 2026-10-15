//! Headless, data-backed battle rules. Recovered mechanics carry explicit
//! confidence and unknown inputs stay caller-configurable.

use crate::battle::{BattleData, BattleMove, BattleSpecies, TypeChart};

pub mod abilities;
pub mod effects;
pub mod items;
pub mod progression;
pub mod rules;
pub mod statuses;
pub mod styles;
use abilities::AbilityState;
use effects::{EffectEvent, MoveEffectsTable, Stages};
use items::ItemTable;

pub trait BattleRng {
    fn next_u32(&mut self) -> u32;
    fn below(&mut self, upper: u32) -> u32 {
        self.next_u32() % upper.max(1)
    }
}

#[derive(Debug, Clone)]
pub struct XorShift32(pub u32);
impl BattleRng for XorShift32 {
    fn next_u32(&mut self) -> u32 {
        let mut x = if self.0 == 0 { 0x9e37_79b9 } else { self.0 };
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MajorStatus {
    Drowsy,
    Paralysis,
    Poison,
    Burn,
    Frostbite,
}

#[derive(Debug, Clone)]
pub struct MoveSlot {
    pub data: BattleMove,
    pub pp: u8,
    pub priority: i8,
    /// Critical rank is absent from the exported move table; supply it only
    /// when recovered move data is available.
    pub critical_rank: u8,
}
impl MoveSlot {
    pub fn new(data: BattleMove) -> Self {
        let pp = data.pp;
        Self {
            data,
            pp,
            priority: 0,
            critical_rank: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Combatant {
    pub species: BattleSpecies,
    pub level: u8,
    pub max_hp: u32,
    pub hp: u32,
    pub attack: u32,
    pub defense: u32,
    pub special_attack: u32,
    pub special_defense: u32,
    pub speed: u32,
    pub moves: Vec<MoveSlot>,
    pub status: Option<MajorStatus>,
    pub caught: bool,
    /// Active ability id (0 = none). Taken from the species' first slot; only Slow Start
    /// (112) and Flower Gift (122) have an effect, and only with `Battle::set_abilities`.
    pub ability: u16,
    pub effort: [u8; 6],
    /// Nature percentages in HP/Atk/Def/SpA/SpD/Spe order. The nature table
    /// mapping is not in the checked-in personal table, so callers provide it.
    pub nature_percent: [u16; 6],
    pub action_class: u8,
    pub speed_modifier_q12: u16,
    pub damage_modifier_q12: u16,
    pub power_modifier_q12: u16,
    pub attack_modifier_q12: u16,
    pub defense_modifier_q12: u16,
    pub crit_stage: u8,
    pub accuracy_stage: i8,
    pub evasion_stage: i8,
}

impl Combatant {
    /// PLA stat formula, neutral nature and effort level zero unless explicitly configured.
    pub fn from_base(species: BattleSpecies, level: u8, moves: Vec<BattleMove>) -> Self {
        Self::with_effort_nature(species, level, moves, [0; 6], [100; 6])
    }
    pub fn with_effort_nature(
        species: BattleSpecies,
        level: u8,
        moves: Vec<BattleMove>,
        effort: [u8; 6],
        nature_percent: [u16; 6],
    ) -> Self {
        let level = level.max(1);
        let values = [
            species.hp,
            species.attack,
            species.defense,
            species.special_attack,
            species.special_defense,
            species.speed,
        ];
        let stats = std::array::from_fn::<_, 6, _>(|i| {
            calculate_stat(
                u32::from(values[i]),
                level,
                effort[i],
                nature_percent[i],
                i == 0,
                species.dex == 292,
            )
        });
        let max_hp = stats[0];
        let ability = species.abilities[0];
        Self {
            attack: stats[1],
            defense: stats[2],
            special_attack: stats[3],
            special_defense: stats[4],
            speed: stats[5],
            species,
            level,
            max_hp,
            hp: max_hp,
            moves: moves.into_iter().map(MoveSlot::new).collect(),
            status: None,
            caught: false,
            ability,
            effort,
            nature_percent,
            action_class: 0,
            speed_modifier_q12: 4096,
            damage_modifier_q12: 4096,
            power_modifier_q12: 4096,
            attack_modifier_q12: 4096,
            defense_modifier_q12: 4096,
            crit_stage: 0,
            accuracy_stage: 0,
            evasion_stage: 0,
        }
    }
    pub fn is_alive(&self) -> bool {
        self.hp > 0 && !self.caught
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    pub actor: usize,
    pub move_slot: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealingItem {
    pub name: String,
    pub healing: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct CaptureModifiers {
    pub outbreak: bool,
    pub ai: f64,
    pub bait: f64,
}
impl Default for CaptureModifiers {
    fn default() -> Self {
        Self {
            outbreak: false,
            ai: 1.0,
            bait: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    TurnStarted(u32),
    ActedFirst(usize),
    UsedMove {
        actor: usize,
        target: usize,
        name: String,
        damage: u32,
        critical: bool,
        effectiveness: String,
    },
    Missed {
        actor: usize,
        target: usize,
        name: String,
    },
    StatusApplied {
        target: usize,
        status: MajorStatus,
    },
    UnableToAct {
        actor: usize,
        status: MajorStatus,
    },
    Recoil {
        target: usize,
        status: MajorStatus,
        damage: u32,
    },
    KnockedOut(usize),
    Switched {
        side: usize,
        from: usize,
        to: usize,
    },
    UsedHealingItem {
        combatant: usize,
        item: String,
        healed: u32,
    },
    BattleEnded {
        winner_side: Option<usize>,
    },
}

#[derive(Debug, Clone)]
pub struct Battle {
    pub combatants: Vec<Combatant>,
    pub sides: Vec<Vec<usize>>,
    pub turn: u32,
    pub ended: bool,
    active: Vec<usize>,
    type_chart: TypeChart,
    pub trick_room: bool,
    pub weather: BattleWeather,
    effects: Option<MoveEffectsTable>,
    items: Option<ItemTable>,
    stages: Vec<Stages>,
    effect_events: Vec<EffectEvent>,
    pending_status: Vec<(usize, MajorStatus)>,
    stage_multipliers: bool,
    status: statuses::StatusState,
    /// Turn-limited major status per combatant: (turns, elapsed end-of-turns). VERIFIED rule.
    status_turns: Vec<Option<(u8, u8)>>,
    /// Remaining turns per rank group (Atk/SpA, Def/SpD, Spe/Acc/Eva). VERIFIED rule.
    stage_turns: Vec<[u8; 3]>,
    rule_events: Vec<rules::RuleEvent>,
    /// VERIFIED PLA base damage formula on/off (off = legacy main-series shape).
    pla_damage: bool,
    /// Agile/Strong styles, the PLA action-order queue and Fixated (all opt-in; see styles.rs).
    styles: styles::StyleState,
    /// The two live abilities, Slow Start and Flower Gift (opt-in; see abilities.rs).
    abilities: AbilityState,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BattleWeather {
    #[default]
    Unknown,
    Clear,
    Rain,
    Sun,
    Snow,
}

impl Battle {
    pub fn new(
        combatants: Vec<Combatant>,
        sides: Vec<Vec<usize>>,
        type_chart: TypeChart,
    ) -> Result<Self, String> {
        if sides.len() < 2 || sides.iter().any(Vec::is_empty) {
            return Err("battle needs at least two non-empty sides".into());
        }
        for side in &sides {
            for &index in side {
                if index >= combatants.len() {
                    return Err(format!("combatant index {index} out of range"));
                }
            }
            if !side.iter().any(|&index| combatants[index].is_alive()) {
                return Err("each battle side needs a living combatant".into());
            }
        }
        let active = sides
            .iter()
            .map(|side| {
                side.iter()
                    .copied()
                    .find(|&index| combatants[index].is_alive())
                    .unwrap()
            })
            .collect();
        Ok(Self {
            combatants,
            sides,
            turn: 0,
            ended: false,
            active,
            type_chart,
            trick_room: false,
            weather: BattleWeather::Unknown,
            effects: None,
            items: None,
            stages: Vec::new(),
            effect_events: Vec::new(),
            pending_status: Vec::new(),
            stage_multipliers: false,
            status: statuses::StatusState::default(),
            status_turns: Vec::new(),
            stage_turns: Vec::new(),
            rule_events: Vec::new(),
            pla_damage: false,
            styles: styles::StyleState::default(),
            abilities: AbilityState::default(),
        })
    }

    pub fn active_combatant(&self, side: usize) -> Option<usize> {
        self.active.get(side).copied()
    }

    /// Switch the active team member. Switch order and action cost are assumed.
    pub fn switch(&mut self, side: usize, replacement: usize) -> Result<Event, String> {
        let roster = self.sides.get(side).ok_or("battle side out of range")?;
        if !roster.contains(&replacement) {
            return Err("replacement is not on this side".into());
        }
        if !self.combatants[replacement].is_alive() {
            return Err("replacement is not able to battle".into());
        }
        let from = self.active[side];
        if !self.switch_allowed(side, from) {
            return Err("trapped: cannot switch out".into());
        }
        self.clear_volatile_on_leave(from);
        self.active[side] = replacement;
        Ok(Event::Switched {
            side,
            from,
            to: replacement,
        })
    }

    /// Use a caller-supplied healing item; inventory and exact game effects are unknown.
    pub fn use_healing_item(&mut self, side: usize, item: &HealingItem) -> Result<Event, String> {
        let combatant = *self.active.get(side).ok_or("battle side out of range")?;
        if !self.combatants[combatant].is_alive() {
            return Err("active Pokémon is unable to battle".into());
        }
        let before = self.combatants[combatant].hp;
        self.combatants[combatant].hp = before
            .saturating_add(item.healing)
            .min(self.combatants[combatant].max_hp);
        Ok(Event::UsedHealingItem {
            combatant,
            item: item.name.clone(),
            healed: self.combatants[combatant].hp - before,
        })
    }

    /// Estimated damage of one move slot of `actor` against `target`, with the same estimator
    /// [`Battle::best_damage_action`] uses (no critical chance, deterministic roll).
    pub fn damage_estimate(&self, actor: usize, move_slot: usize, target: usize) -> Option<f32> {
        let a = self.combatants.get(actor)?;
        let t = self.combatants.get(target)?;
        let m = a.moves.get(move_slot)?;
        Some(expected_damage(
            a,
            t,
            &m.data,
            &self.type_chart,
            self.pla_damage,
            self.slow_start_active(actor),
        ))
    }

    /// Choose the move with the highest estimated expected damage (no critical chance).
    pub fn best_damage_action(&self, actor: usize) -> Option<Action> {
        let target = self.target_for(actor)?;
        let slow_start = self.slow_start_active(actor);
        self.combatants
            .get(actor)?
            .moves
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.pp > 0 && slot.data.power > 0)
            .max_by(|(_, a), (_, b)| {
                expected_damage(
                    &self.combatants[actor],
                    &self.combatants[target],
                    &a.data,
                    &self.type_chart,
                    self.pla_damage,
                    slow_start,
                )
                .total_cmp(&expected_damage(
                    &self.combatants[actor],
                    &self.combatants[target],
                    &b.data,
                    &self.type_chart,
                    self.pla_damage,
                    slow_start,
                ))
            })
            .map(|(move_slot, _)| Action { actor, move_slot })
    }

    pub fn winner_side(&self) -> Option<Option<usize>> {
        let living = self
            .sides
            .iter()
            .enumerate()
            .filter(|(_, side)| side.iter().any(|&i| self.combatants[i].is_alive()))
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        if living.len() == self.sides.len() {
            None
        } else if living.len() == 1 {
            Some(Some(living[0]))
        } else {
            Some(None)
        }
    }

    pub fn play_turn(&mut self, actions: &[Action], rng: &mut impl BattleRng) -> Vec<Event> {
        let mut events = Vec::new();
        if self.ended {
            return events;
        }
        self.turn += 1;
        events.push(Event::TurnStarted(self.turn));
        self.ability_turn_start();
        let order = actions
            .iter()
            .copied()
            .filter(|a| {
                a.actor < self.combatants.len()
                    && self.combatants[a.actor].is_alive()
                    && self.active.iter().enumerate().any(|(side, active)| {
                        *active == a.actor && self.sides[side].contains(&a.actor)
                    })
            })
            .collect::<Vec<_>>();
        let mut keyed = order
            .into_iter()
            .map(|action| {
                let slot = self.combatants[action.actor].moves.get(action.move_slot);
                let priority = slot.map_or(i8::MIN, |m| m.priority);
                (
                    action,
                    turn_order_key(
                        &self.combatants[action.actor],
                        priority,
                        self.weather,
                        self.trick_room,
                        self.slow_start_active(action.actor),
                    ),
                )
            })
            .collect::<Vec<_>>();
        keyed.sort_by(|(_, ka), (_, kb)| kb.cmp(ka));
        let mut start = 0;
        while start < keyed.len() {
            let mut end = start + 1;
            while end < keyed.len() && keyed[end].1 == keyed[start].1 {
                end += 1;
            }
            for i in (start + 1..end).rev() {
                let j = start + rng.below((i - start + 1) as u32) as usize;
                keyed.swap(i, j);
            }
            start = end;
        }
        let order = keyed
            .into_iter()
            .map(|(action, _)| action)
            .collect::<Vec<_>>();
        let mut flinched: Vec<usize> = Vec::new();
        if let Some(first) = order.first() {
            events.push(Event::ActedFirst(first.actor));
        }
        for mut action in order {
            if self.winner_side().is_some() {
                break;
            }
            if !self.combatants[action.actor].is_alive() {
                continue;
            }
            if flinched.contains(&action.actor) {
                self.effect_events.push(EffectEvent::FlinchedAndSkipped {
                    actor: action.actor,
                });
                continue;
            }
            let mut releasing = false;
            match self.status_pre_action(action.actor, rng, &mut events) {
                statuses::PreAction::Skip => continue,
                statuses::PreAction::Act(Some(slot)) => {
                    action.move_slot = slot;
                    releasing = true;
                }
                statuses::PreAction::Act(None) => {}
            }
            if !self.combatants[action.actor].is_alive() {
                continue;
            }
            let Some(target) = self.target_for(action.actor) else {
                continue;
            };
            let Some(movement) = self.combatants[action.actor].moves.get(action.move_slot) else {
                continue;
            };
            if movement.pp == 0 && !releasing {
                continue;
            }
            let move_name = movement.data.name.clone();
            let category = movement.data.category.clone();
            let power = movement.data.power;
            let accuracy = movement.data.accuracy;
            let critical_rank = movement.critical_rank;
            let move_type = movement.data.move_type.clone();
            if !releasing {
                self.combatants[action.actor].moves[action.move_slot].pp -= 1;
            }
            let rules = self.status_rules();
            if let Some(status) = self.combatants[action.actor].status {
                if (status == MajorStatus::Drowsy && !rules.sleep_duration && rng.below(2) == 0)
                    || (status == MajorStatus::Paralysis && !rules.notes_rates && rng.below(4) == 0)
                {
                    events.push(Event::UnableToAct {
                        actor: action.actor,
                        status,
                    });
                    continue;
                }
            }
            if self.status_pre_move(
                action.actor,
                target,
                action.move_slot,
                &move_name,
                releasing,
                rng,
            ) {
                continue;
            }
            // VERIFIED: accuracy and evasion read the shared Spe/Acc/Eva rank group.
            let acc_stage = (self.combatants[action.actor].accuracy_stage
                + self.stages(action.actor).speed)
                .clamp(-1, 1);
            let eva_stage =
                (self.combatants[target].evasion_stage + self.stages(target).speed).clamp(-1, 1);
            if !accuracy_hits(accuracy, acc_stage, eva_stage, 4096, rng) {
                events.push(Event::Missed {
                    actor: action.actor,
                    target,
                    name: move_name,
                });
                continue;
            }
            let fx = self
                .effects
                .as_ref()
                .and_then(|t| t.get(&move_name))
                .cloned();
            if power == 0 {
                if let Some(fx) = &fx {
                    let ko = self.apply_move_effects(
                        action.actor,
                        target,
                        fx,
                        false,
                        0,
                        &mut flinched,
                        rng,
                    );
                    self.status_post_effects(action.actor, target, fx, false, rng);
                    for (t, status) in std::mem::take(&mut self.pending_status) {
                        events.push(Event::StatusApplied { target: t, status });
                    }
                    for k in ko {
                        events.push(Event::KnockedOut(k));
                        self.advance_active_for(k);
                    }
                    continue;
                }
                if let Some(status) = status_move(&move_name) {
                    if self.combatants[target].status.is_none() {
                        self.combatants[target].status = Some(status);
                        events.push(Event::StatusApplied { target, status });
                    }
                }
                continue;
            }
            let hits = fx.as_ref().map_or(1, |f| Self::roll_hits(f, rng));
            // VERIFIED: rank states multiply the base damage (not the stats); see rules.rs.
            let ranks = if self.stage_multipliers {
                (
                    self.stages(action.actor).attack,
                    self.stages(target).defense,
                )
            } else {
                (0, 0)
            };
            let drowsy_bonus = self.status_rules().notes_rates;
            let pla_formula = self.pla_damage;
            let mut damage = 0;
            let mut dealt = 0;
            let mut critical = false;
            let mut multiplier = 1.0;
            let mut landed = 0;
            let slow_start = self.slow_start_active(action.actor);
            for _ in 0..hits {
                let (d, c, m) = calculate_damage(
                    &self.combatants[action.actor],
                    &self.combatants[target],
                    &move_type,
                    &category,
                    power,
                    &self.type_chart,
                    self.weather,
                    critical_rank,
                    ranks,
                    drowsy_bonus,
                    pla_formula,
                    slow_start,
                    rng,
                );
                // Opt-in Fixated multipliers (no-op unless `set_fixated` was used).
                let d = self.fixated_damage(action.actor, target, &move_name, d);
                dealt += d.min(self.combatants[target].hp);
                self.combatants[target].hp = self.combatants[target].hp.saturating_sub(d);
                damage += d;
                critical |= c;
                multiplier = m;
                landed += 1;
                if self.combatants[target].hp == 0 {
                    break;
                }
            }
            if hits > 1 {
                self.effect_events.push(EffectEvent::Hits {
                    actor: action.actor,
                    count: landed,
                });
            }
            let move_name_for_hook = move_name.clone();
            events.push(Event::UsedMove {
                actor: action.actor,
                target,
                name: move_name,
                damage,
                critical,
                effectiveness: effectiveness_label(multiplier).into(),
            });
            if self.combatants[target].hp == 0 {
                events.push(Event::KnockedOut(target));
                self.advance_active_for(target);
            }
            self.status_post_hit(action.actor, &move_name_for_hook);
            if let Some(fx) = &fx {
                let ko = self.apply_move_effects(
                    action.actor,
                    target,
                    fx,
                    true,
                    dealt,
                    &mut flinched,
                    rng,
                );
                self.status_post_effects(action.actor, target, fx, true, rng);
                for (t, status) in std::mem::take(&mut self.pending_status) {
                    events.push(Event::StatusApplied { target: t, status });
                }
                for k in ko {
                    events.push(Event::KnockedOut(k));
                    self.advance_active_for(k);
                }
            }
        }
        // End-of-round status damage and status timing are conventional-series assumptions.
        let mut status_knockouts = Vec::new();
        for side in &self.sides {
            for &i in side {
                if !self.combatants[i].is_alive() {
                    continue;
                }
                let Some(
                    status @ (MajorStatus::Poison | MajorStatus::Burn | MajorStatus::Frostbite),
                ) = self.combatants[i].status
                else {
                    continue;
                };
                let damage = self.residual_damage(i, status);
                self.combatants[i].hp = self.combatants[i].hp.saturating_sub(damage);
                events.push(Event::Recoil {
                    target: i,
                    status,
                    damage,
                });
                if self.combatants[i].hp == 0 {
                    events.push(Event::KnockedOut(i));
                    status_knockouts.push(i);
                }
            }
        }
        for index in status_knockouts {
            self.advance_active_for(index);
        }
        self.status_turns_end_of_turn();
        self.stage_turns_end_of_turn();
        self.status_end_of_turn(&mut events);
        self.ability_end_of_turn();
        if let Some(winner) = self.winner_side() {
            self.ended = true;
            events.push(Event::BattleEnded {
                winner_side: winner,
            });
        }
        events
    }

    fn target_for(&self, actor: usize) -> Option<usize> {
        let side = self
            .sides
            .iter()
            .position(|indices| indices.contains(&actor))?;
        self.sides
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != side)
            .filter_map(|(enemy_side, _)| self.active.get(enemy_side).copied())
            .find(|&i| self.combatants[i].is_alive())
    }

    fn advance_active_for(&mut self, knocked_out: usize) {
        if let Some(side) = self
            .sides
            .iter()
            .position(|roster| roster.contains(&knocked_out))
        {
            if self.active[side] == knocked_out {
                if let Some(next) = self.sides[side]
                    .iter()
                    .copied()
                    .find(|&index| self.combatants[index].is_alive())
                {
                    self.active[side] = next;
                }
            }
        }
    }

    /// Capture index and shake formula are LIKELY. Catch rate and AI/bait factors
    /// are explicit inputs because the current typed species table omits them.
    pub fn attempt_capture(
        &mut self,
        data: &BattleData,
        target: usize,
        ball: &str,
        suitable_range: bool,
        player_rank: u8,
        pokemon_rank: u8,
        catch_rate: u8,
        backstrike: bool,
        modifiers: CaptureModifiers,
        rng: &mut impl BattleRng,
    ) -> Result<bool, String> {
        let c = self
            .combatants
            .get(target)
            .ok_or("capture target out of range")?;
        if !c.is_alive() {
            return Ok(false);
        }
        let ball = data.capture_ball_multiplier(ball, suitable_range)?;
        let status = match c.status {
            None => data.capture_status_multiplier("None")?,
            Some(MajorStatus::Paralysis) => data.capture_status_multiplier("Paralysis")?,
            Some(MajorStatus::Poison) => data.capture_status_multiplier("Poison")?,
            Some(MajorStatus::Burn) => data.capture_status_multiplier("Burn")?,
            Some(MajorStatus::Drowsy) => data.capture_status_multiplier("Drowsiness / Sleep")?,
            Some(MajorStatus::Frostbite) => data.capture_status_multiplier("Frostbite / Freeze")?,
        };
        let context = data
            .capture_factor(if backstrike {
                "capture_index_backstrike_success_factor"
            } else {
                "capture_index_backstrike_failure_factor"
            })?
            .unwrap_or(1.0);
        let rank = data.capture_level_rank_adjustment(player_rank, pokemon_rank)?;
        let ratio = c.hp as f64 / c.max_hp as f64;
        let index = capture_index(catch_rate, ratio, rank, status, ball, context, modifiers);
        let probability = capture_probability(index);
        let caught = two_shake_capture(probability, rng);
        if caught {
            self.combatants[target].caught = true;
            self.advance_active_for(target);
        }
        if self.winner_side().is_some() {
            self.ended = true;
        }
        Ok(caught)
    }
}

/// Read the executable-derived chart. Unknown names remain neutral as in the prior fallback.
pub fn type_effectiveness(chart: &TypeChart, attack: &str, defenses: &[String]) -> f32 {
    let mut exponent = 0_i32;
    let mut seen: Vec<&String> = Vec::new();
    for defense in defenses {
        if seen.contains(&defense) {
            continue;
        }
        seen.push(defense);
        match chart.multiplier(attack, defense).unwrap_or(1.0) {
            0.0 => return 0.0,
            0.5 => exponent -= 1,
            2.0 => exponent += 1,
            _ => {}
        }
    }
    effectiveness_from_exponent(exponent)
}

/// VERIFIED PLA type categories: 4x is capped to 2.5 and 0.25x to 0.4.
fn effectiveness_from_exponent(exponent: i32) -> f32 {
    match exponent {
        i32::MIN..=-2 => 0.4,
        -1 => 0.5,
        0 => 1.0,
        1 => 2.0,
        _ => 2.5,
    }
}

/// LIKELY capture-index reconstruction; inputs sourced from the recovered tables.
pub fn capture_index(
    catch_rate: u8,
    current_hp_ratio: f64,
    rank_adjustment: i32,
    sick: f64,
    ball: f64,
    backstrike: f64,
    modifiers: CaptureModifiers,
) -> f64 {
    let hp_term = 30.0
        * (1.0 - 0.66 * current_hp_ratio.clamp(0.0, 1.0))
        * sick
        * ball
        * modifiers.ai
        * backstrike
        * modifiers.bait;
    0.75 * f64::from(catch_rate)
        + if modifiers.outbreak { 10.0 } else { 0.0 }
        + 30.0 * f64::from(rank_adjustment)
        + hp_term
}

pub fn capture_probability(index: f64) -> f64 {
    (index / 255.0).clamp(0.0, 1.0)
}

/// LIKELY two-check capture test: each check fails with sqrt(1-p).
pub fn two_shake_capture(probability: f64, rng: &mut impl BattleRng) -> bool {
    let failure = (1.0 - probability.clamp(0.0, 1.0)).sqrt();
    let fail_1 = f64::from(rng.below(65_536)) / 65_536.0 < failure;
    let fail_2 = f64::from(rng.below(65_536)) / 65_536.0 < failure;
    !fail_1 && !fail_2
}

const EFFORT_K: [u32; 11] = [0, 2, 3, 4, 7, 8, 9, 14, 15, 16, 25];
/// VERIFIED PLA stat formula; nature percentage is caller-supplied because its mapping is unknown.
fn calculate_stat(
    base: u32,
    level: u8,
    effort: u8,
    nature_percent: u16,
    hp: bool,
    shedinja: bool,
) -> u32 {
    if hp && shedinja {
        return 1;
    }
    let l = f64::from(level.max(1));
    let b = f64::from(base);
    let k = f64::from(EFFORT_K[usize::from(effort.min(10))]);
    let effort_term = ((b.sqrt() * k + l) / 2.5).floor();
    if hp {
        (effort_term + (((l / 100.0 + 1.0) * b + l).floor())).max(1.0) as u32
    } else {
        let core = (((l / 50.0 + 1.0) * b / 1.5).floor() as u32 * u32::from(nature_percent)
            & 0xfffe)
            / 100;
        (f64::from(core) + effort_term).max(1.0) as u32
    }
}

/// Q12 multiply used by recovered damage modifiers; ties round down.
fn q12_mul(value: u32, modifier: u32) -> u32 {
    let product = u64::from(value) * u64::from(modifier);
    let quotient = product / 4096;
    let remainder = product % 4096;
    (quotient + u64::from(remainder > 2048)).min(u64::from(u32::MAX)) as u32
}

/// VERIFIED critical-rank probabilities. The rank input is clamped to 3.
pub fn critical_hit(move_rank: u8, stage: u8, rng: &mut impl BattleRng) -> bool {
    if move_rank == 6 {
        return true;
    }
    let rank = usize::from(move_rank.saturating_add(stage).min(3));
    let denominators = [24, 8, 2, 1];
    rng.below(denominators[rank]) == 0
}

/// VERIFIED PLA accuracy: three rank states x0.7/x1.0/x1.3, cap 100, Q12 event modifier with
/// round-half-down, roll in [0, 99]. Accuracy above 100 (101 in data) never misses.
pub fn accuracy_hits(
    accuracy: u8,
    accuracy_stage: i8,
    evasion_stage: i8,
    event_q12: u32,
    rng: &mut impl BattleRng,
) -> bool {
    if accuracy > 100 {
        return true;
    }
    let index = (accuracy_stage + 1 - evasion_stage).clamp(0, 2) as usize;
    let (numerator, denominator) = rules::ACCURACY_TABLE[index];
    let chance = q12_mul((u32::from(accuracy) * numerator / denominator).min(100), event_q12).min(100);
    rng.below(100) < chance
}

/// VERIFIED agility: speed x Q12 event modifier (round half down), x133/100 for Grass in sun or
/// Ice in snow, x50/100 when paralysed, capped at 10000, optionally reversed (10000 - v), then
/// packed into a 13-bit field. Speed rank states are NOT part of it (they share the accuracy /
/// evasion group). Ordering is action class, then move priority, then agility; the exact class
/// numbering stays a caller input (default attack class).
fn turn_order_key(
    actor: &Combatant,
    priority: i8,
    weather: BattleWeather,
    trick_room: bool,
    slow_start: bool,
) -> (u8, i8, u32) {
    let speed = agility(actor, weather, trick_room, slow_start);
    (actor.action_class, priority, speed & rules::AGILITY_KEY_MASK)
}

/// VERIFIED agility value (see `turn_order_key`) before it is packed into the 13-bit key.
/// The PLA action-order queue reads the same value without the reversal (`reverse = false`).
/// `slow_start` applies Slow Start's Q12 x0.5 as the last modifier (PROBABLE order; see
/// abilities.rs).
pub(crate) fn agility(
    actor: &Combatant,
    weather: BattleWeather,
    reverse: bool,
    slow_start: bool,
) -> u32 {
    let mut speed = q12_mul(actor.speed, u32::from(actor.speed_modifier_q12));
    if (weather == BattleWeather::Sun && actor.species.types.iter().any(|t| t == "Grass"))
        || (weather == BattleWeather::Snow && actor.species.types.iter().any(|t| t == "Ice"))
    {
        speed = speed * 133 / 100;
    }
    if actor.status == Some(MajorStatus::Paralysis) {
        speed /= 2;
    }
    if slow_start {
        speed = q12_mul(speed, abilities::HALF_Q12);
    }
    speed = speed.min(rules::AGILITY_CAP);
    if reverse {
        speed = rules::AGILITY_CAP - speed;
    }
    speed
}

fn expected_damage(
    attacker: &Combatant,
    target: &Combatant,
    movement: &BattleMove,
    chart: &TypeChart,
    pla: bool,
    slow_start: bool,
) -> f32 {
    let (attack, defense) = if movement.category == "Physical" {
        (attacker.attack, target.defense)
    } else {
        (attacker.special_attack, target.special_defense)
    };
    let power = if slow_start && movement.category == "Physical" {
        q12_mul(u32::from(movement.power), abilities::HALF_Q12)
    } else {
        u32::from(movement.power)
    };
    let base = rules::base_damage(pla, power, attack, u32::from(attacker.level), defense);
    let stab = attacker
        .species
        .types
        .iter()
        .any(|t| t == &movement.move_type);
    let effectiveness = type_effectiveness(chart, &movement.move_type, &target.species.types);
    base as f32 * if stab { 1.25 } else { 1.0 } * effectiveness * 0.925
}

/// VERIFIED damage pipeline: PLA base formula (when `pla_formula`) with rank states, Q12 modifier, rain/Fire x0.75,
/// critical x1.5, 85-100 % roll, STAB x1.25, type category, burn/physical and
/// frostbite/special x50 %, drowsy defender x133 % (when `drowsy_bonus`), minimum 1.
/// `slow_start` halves the power of physical moves (PROBABLE target of the game's halving).
#[allow(clippy::too_many_arguments)]
fn calculate_damage(
    attacker: &Combatant,
    target: &Combatant,
    attack_type: &str,
    category: &str,
    power: u16,
    chart: &TypeChart,
    weather: BattleWeather,
    critical_rank: u8,
    ranks: (i8, i8),
    drowsy_bonus: bool,
    pla_formula: bool,
    slow_start: bool,
    rng: &mut impl BattleRng,
) -> (u32, bool, f32) {
    let (attack, defense) = if category == "Physical" {
        (attacker.attack, target.defense)
    } else {
        (attacker.special_attack, target.special_defense)
    };
    let mut power = q12_mul(u32::from(power), u32::from(attacker.power_modifier_q12));
    if slow_start && category == "Physical" {
        power = q12_mul(power, abilities::HALF_Q12);
    }
    let attack = q12_mul(attack, u32::from(attacker.attack_modifier_q12));
    let defense = q12_mul(defense, u32::from(target.defense_modifier_q12));
    let base = rules::base_damage(pla_formula, power, attack, u32::from(attacker.level), defense);
    let base = rules::apply_rank_states(base, ranks.0, ranks.1);
    let effectiveness = type_effectiveness(chart, attack_type, &target.species.types);
    let mut damage = q12_mul(base, u32::from(attacker.damage_modifier_q12));
    if weather == BattleWeather::Rain && attack_type == "Fire" {
        damage = q12_mul(damage, 3072);
    }
    let critical = critical_hit(critical_rank, attacker.crit_stage, rng);
    if critical {
        damage = damage * 15 / 10;
    }
    damage = damage * (85 + rng.below(16)) / 100;
    if attacker.species.types.iter().any(|t| t == attack_type) {
        damage = q12_mul(damage, 0x1400);
    }
    damage = (f64::from(damage) * f64::from(effectiveness)).floor() as u32;
    if (attacker.status == Some(MajorStatus::Burn) && category == "Physical")
        || (attacker.status == Some(MajorStatus::Frostbite) && category == "Special")
    {
        damage = damage * rules::BURN_FROSTBITE_DAMAGE_PERCENT / 100;
    }
    if drowsy_bonus && target.status == Some(MajorStatus::Drowsy) {
        damage = damage * rules::DROWSY_DAMAGE_TAKEN_PERCENT / 100;
    }
    (
        if effectiveness == 0.0 {
            0
        } else {
            damage.max(1)
        },
        critical,
        effectiveness,
    )
}
fn status_move(name: &str) -> Option<MajorStatus> {
    match name {
        "thunder-wave" => Some(MajorStatus::Paralysis),
        "poison-powder" | "toxic" => Some(MajorStatus::Poison),
        "will-o-wisp" => Some(MajorStatus::Burn),
        "sleep-powder" | "hypnosis" => Some(MajorStatus::Drowsy),
        "ice-beam" => None,
        _ => None,
    }
}
fn effectiveness_label(multiplier: f32) -> &'static str {
    if multiplier == 0.0 {
        "no effect"
    } else if multiplier > 1.0 {
        "super effective"
    } else if multiplier < 1.0 {
        "not very effective"
    } else {
        "normal"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn loads_real_species_and_move_tables() {
        let d = data();
        let pikachu = d.species_row("pikachu").unwrap().unwrap();
        assert_eq!((pikachu.hp, pikachu.speed), (35, 90));
        let quick = d.move_row("quick-attack").unwrap().unwrap();
        assert_eq!(
            (quick.power, quick.pp, quick.move_type.as_str()),
            (40, 20, "Normal")
        );
    }
    #[test]
    fn type_chart_supports_dual_types_and_immunity() {
        let d = data();
        assert_eq!(d.type_chart().cell_count(), 324);
        assert_eq!(
            type_effectiveness(
                d.type_chart(),
                "Electric",
                &["Water".into(), "Flying".into()]
            ),
            2.5
        );
        assert_eq!(
            type_effectiveness(d.type_chart(), "Electric", &["Ground".into()]),
            0.0
        );
        assert_eq!(d.type_chart().multiplier("Fire", "Grass").unwrap(), 2.0);
        assert_eq!(d.type_chart().multiplier("Normal", "Rock").unwrap(), 0.5);
        assert_eq!(effectiveness_from_exponent(2), 2.5);
        assert_eq!(effectiveness_from_exponent(-2), 0.4);
    }

    #[test]
    fn recovered_stat_and_integer_damage_examples_match_hand_calculation() {
        assert_eq!(calculate_stat(45, 100, 10, 100, true, false), 297);
        assert_eq!(calculate_stat(50, 100, 10, 110, false, false), 220);
        assert_eq!(calculate_stat(1, 50, 10, 110, true, true), 1);
        // PLA base: 40*(50+100+15*20)=18000; /(40+50)=200; /5=40.
        assert_eq!(rules::pla_base_damage(40, 50, 20, 40), 40);
        assert_eq!(q12_mul(100, 0x1400), 125);
    }

    #[test]
    fn critical_accuracy_and_capture_formulas_use_injected_randomness() {
        assert!(critical_hit(6, 0, &mut Fixed(vec![])));
        assert!(critical_hit(2, 0, &mut Fixed(vec![0]))); // one of two
        assert!(!critical_hit(1, 0, &mut Fixed(vec![1]))); // one of eight
        assert!(accuracy_hits(50, -1, 0, 4096, &mut Fixed(vec![34]))); // 35%
        assert!(!accuracy_hits(50, -1, 0, 4096, &mut Fixed(vec![35])));
        let index = capture_index(255, 1.0, 0, 1.0, 0.75, 1.0, CaptureModifiers::default());
        assert!((index - 198.9).abs() < 0.001);
        assert!((capture_probability(index) - 0.78).abs() < 0.001);
        assert!(two_shake_capture(0.78, &mut Fixed(vec![65_535, 65_535])));
        assert!(!two_shake_capture(0.78, &mut Fixed(vec![0, 0])));
    }

    #[test]
    fn player_can_switch_and_use_healing_item_while_ai_picks_best_damage() {
        let d = data();
        let mut b = Battle::new(
            vec![
                combatant(&d, "pikachu", &["thunder-shock", "quick-attack"], 20),
                combatant(&d, "eevee", &["tackle"], 20),
                combatant(&d, "staraptor", &["wing-attack"], 20),
            ],
            vec![vec![0, 1], vec![2]],
            d.type_chart().clone(),
        )
        .unwrap();
        let action = b.best_damage_action(2).unwrap();
        assert_eq!(action.move_slot, 0);
        b.combatants[0].hp -= 10;
        let item = HealingItem {
            name: "Potion".into(),
            healing: 20,
        };
        assert_eq!(
            b.use_healing_item(0, &item).unwrap(),
            Event::UsedHealingItem {
                combatant: 0,
                item: "Potion".into(),
                healed: 10,
            }
        );
        assert_eq!(b.combatants[0].hp, b.combatants[0].max_hp);
        assert_eq!(
            b.switch(0, 1).unwrap(),
            Event::Switched {
                side: 0,
                from: 0,
                to: 1
            }
        );
        assert_eq!(b.active_combatant(0), Some(1));
        assert!(b.switch(0, 2).is_err());
    }
    #[test]
    fn damage_includes_stab_effectiveness_critical_and_roll() {
        let d = data();
        let attacker = combatant(&d, "pikachu", &["thunder-shock"], 20);
        let mut target = combatant(&d, "staraptor", &["tackle"], 20);
        let (critical_damage, critical, multiplier) = calculate_damage(
            &attacker,
            &target,
            "Electric",
            "Special",
            40,
            d.type_chart(),
            BattleWeather::Clear,
            0,
            (0, 0),
            false,
            true,
            false,
            &mut Fixed(vec![0, 0]),
        );
        assert!(critical);
        assert_eq!(multiplier, 2.0);
        let (ordinary_damage, critical, _) = calculate_damage(
            &attacker,
            &target,
            "Electric",
            "Special",
            40,
            d.type_chart(),
            BattleWeather::Clear,
            0,
            (0, 0),
            false,
            true,
            false,
            &mut Fixed(vec![1, 15]),
        );
        assert!(!critical);
        assert!(critical_damage > ordinary_damage);
        target.species.types = ["Ground".into(), "Ground".into()];
        let (immune_damage, _, multiplier) = calculate_damage(
            &attacker,
            &target,
            "Electric",
            "Special",
            40,
            d.type_chart(),
            BattleWeather::Clear,
            0,
            (0, 0),
            false,
            true,
            false,
            &mut Fixed(vec![1, 15]),
        );
        assert_eq!(multiplier, 0.0);
        assert_eq!(immune_damage, 0);
    }
    #[test]
    fn speed_orders_turn_then_knockout_ends_battle() {
        let d = data();
        let mut b = Battle::new(
            vec![
                combatant(&d, "pikachu", &["thunder-shock"], 30),
                combatant(&d, "starly", &["tackle"], 5),
            ],
            vec![vec![0], vec![1]],
            d.type_chart().clone(),
        )
        .unwrap();
        b.combatants[1].hp = 1;
        let events = b.play_turn(
            &[
                Action {
                    actor: 0,
                    move_slot: 0,
                },
                Action {
                    actor: 1,
                    move_slot: 0,
                },
            ],
            &mut Fixed(vec![1, 99, 1, 99]),
        );
        assert_eq!(events[1], Event::ActedFirst(0));
        assert!(events.contains(&Event::KnockedOut(1)));
        assert!(b.ended);
        assert_eq!(b.combatants[0].moves[0].pp, 24);
    }
    #[test]
    fn explicit_priority_can_outweigh_speed_and_status_moves_apply() {
        let d = data();
        let mut b = Battle::new(
            vec![
                combatant(&d, "pikachu", &["thunder-wave"], 30),
                combatant(&d, "starly", &["tackle"], 5),
            ],
            vec![vec![0], vec![1]],
            d.type_chart().clone(),
        )
        .unwrap();
        b.combatants[0].moves[0].priority = -1;
        b.combatants[1].moves[0].priority = 1;
        let events = b.play_turn(
            &[
                Action {
                    actor: 0,
                    move_slot: 0,
                },
                Action {
                    actor: 1,
                    move_slot: 0,
                },
            ],
            &mut Fixed(vec![0, 99, 0, 0]),
        );
        assert_eq!(events[1], Event::ActedFirst(1));
        assert_eq!(b.combatants[1].status, Some(MajorStatus::Paralysis));
    }

    #[test]
    fn accuracy_can_miss_and_status_damage_runs_at_round_end() {
        let d = data();
        let mut b = Battle::new(
            vec![
                combatant(&d, "pikachu", &["tackle"], 20),
                combatant(&d, "starly", &["tackle"], 5),
            ],
            vec![vec![0], vec![1]],
            d.type_chart().clone(),
        )
        .unwrap();
        b.combatants[0].moves[0].data.accuracy = 1;
        b.combatants[1].hp = 10;
        b.combatants[1].status = Some(MajorStatus::Poison);
        let before = b.combatants[1].hp;
        let events = b.play_turn(
            &[Action {
                actor: 0,
                move_slot: 0,
            }],
            &mut Fixed(vec![99]),
        );
        assert!(events.contains(&Event::Missed {
            actor: 0,
            target: 1,
            name: "tackle".into()
        }));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Recoil {
                target: 1,
                status: MajorStatus::Poison,
                ..
            }
        )));
        assert_eq!(
            b.combatants[1].hp,
            before - (b.combatants[1].max_hp / 8).max(1),
            "legacy rate when notes_rates is off"
        );
    }
    #[test]
    fn capture_uses_recovered_ball_and_status_factors_with_injected_rng() {
        let d = data();
        let mut b = Battle::new(
            vec![
                combatant(&d, "pikachu", &["tackle"], 20),
                combatant(&d, "starly", &["tackle"], 5),
            ],
            vec![vec![0], vec![1]],
            d.type_chart().clone(),
        )
        .unwrap();
        b.combatants[1].hp = 1;
        b.combatants[1].status = Some(MajorStatus::Paralysis);
        assert!(b
            .attempt_capture(
                &d,
                1,
                "Gigaton",
                true,
                0,
                1,
                255,
                true,
                CaptureModifiers::default(),
                &mut Fixed(vec![0, 0])
            )
            .unwrap());
        assert!(b.combatants[1].caught);
    }
}
