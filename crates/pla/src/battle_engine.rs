//! Headless, data-backed battle loop. This is a playable approximation, not a
//! claim that the listed mechanics reproduce Pokémon Legends: Arceus exactly.

use crate::battle::{BattleData, BattleMove, BattleSpecies, TypeChart};

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
}
impl MoveSlot {
    pub fn new(data: BattleMove) -> Self {
        let pp = data.pp;
        let priority = assumed_move_priority(&data.name);
        Self { data, pp, priority }
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
}

impl Combatant {
    /// Standard-series level/stat construction with fixed IV=15 and EV=0;
    /// individual PLA stat generation is not established here (assumed, unverified).
    pub fn from_base(species: BattleSpecies, level: u8, moves: Vec<BattleMove>) -> Self {
        let level = level.max(1);
        let stat = |base: u16| ((2 * u32::from(base) + 15) * u32::from(level) / 100 + 5).max(1);
        let max_hp =
            ((2 * u32::from(species.hp) + 15) * u32::from(level) / 100 + u32::from(level) + 10)
                .max(1);
        Self {
            attack: stat(species.attack),
            defense: stat(species.defense),
            special_attack: stat(species.special_attack),
            special_defense: stat(species.special_defense),
            speed: stat(species.speed),
            species,
            level,
            max_hp,
            hp: max_hp,
            moves: moves.into_iter().map(MoveSlot::new).collect(),
            status: None,
            caught: false,
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

    /// Choose the move with the highest estimated expected damage (no critical chance).
    pub fn best_damage_action(&self, actor: usize) -> Option<Action> {
        let target = self.target_for(actor)?;
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
                )
                .total_cmp(&expected_damage(
                    &self.combatants[actor],
                    &self.combatants[target],
                    &b.data,
                    &self.type_chart,
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
        let mut order = actions
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
        order.sort_by(|a, b| {
            let am = self.combatants[a.actor].moves.get(a.move_slot);
            let bm = self.combatants[b.actor].moves.get(b.move_slot);
            let ap = am.map_or(i8::MIN, |m| m.priority);
            let bp = bm.map_or(i8::MIN, |m| m.priority);
            bp.cmp(&ap).then_with(|| {
                self.combatants[b.actor]
                    .speed
                    .cmp(&self.combatants[a.actor].speed)
            })
        });
        if let Some(first) = order.first() {
            events.push(Event::ActedFirst(first.actor));
        }
        for action in order {
            if self.winner_side().is_some() {
                break;
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
            if movement.pp == 0 {
                continue;
            }
            let move_name = movement.data.name.clone();
            let category = movement.data.category.clone();
            let power = movement.data.power;
            let accuracy = movement.data.accuracy;
            let move_type = movement.data.move_type.clone();
            self.combatants[action.actor].moves[action.move_slot].pp -= 1;
            if let Some(status) = self.combatants[action.actor].status {
                if (status == MajorStatus::Drowsy && rng.below(2) == 0)
                    || (status == MajorStatus::Paralysis && rng.below(4) == 0)
                {
                    events.push(Event::UnableToAct {
                        actor: action.actor,
                        status,
                    });
                    continue;
                }
            }
            // Table values 0/101 are treated as guaranteed hit for this demo;
            // that convention is not established by the PLA evidence here.
            if accuracy != 0 && accuracy < 100 && rng.below(100) >= u32::from(accuracy) {
                events.push(Event::Missed {
                    actor: action.actor,
                    target,
                    name: move_name,
                });
                continue;
            }
            if power == 0 {
                if let Some(status) = status_move(&move_name) {
                    if self.combatants[target].status.is_none() {
                        self.combatants[target].status = Some(status);
                        events.push(Event::StatusApplied { target, status });
                    }
                }
                continue;
            }
            let (damage, critical, multiplier) = calculate_damage(
                &self.combatants[action.actor],
                &self.combatants[target],
                &move_type,
                &category,
                power,
                &self.type_chart,
                rng,
            );
            self.combatants[target].hp = self.combatants[target].hp.saturating_sub(damage);
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
                let damage = (self.combatants[i].max_hp / 8).max(1);
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

    /// Capture rate and combination order are not recovered. This playable
    /// estimate reuses recovered factors but its formula is assumed, unverified.
    pub fn attempt_capture(
        &mut self,
        data: &BattleData,
        target: usize,
        ball: &str,
        suitable_range: bool,
        player_rank: u8,
        pokemon_rank: u8,
        backstrike: bool,
        rng: &mut impl BattleRng,
    ) -> Result<bool, String> {
        let c = self
            .combatants
            .get(target)
            .ok_or("capture target out of range")?;
        if !c.is_alive() {
            return Ok(false);
        }
        let hp_factor = (1.0 - c.hp as f64 / c.max_hp as f64).powf(
            data.capture_constant("capture_index_hp_coef")?
                .unwrap_or(0.66),
        );
        let base = data
            .capture_constant("capture_index_basic_coef")?
            .unwrap_or(30.0)
            * data
                .capture_constant("capture_index_pokepara_coef")?
                .unwrap_or(0.75)
            * hp_factor;
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
        let rank = data
            .capture_level_rank_adjustment(player_rank, pokemon_rank)?
            .max(-20);
        let chance =
            (base * ball * status * context * (1.0 + f64::from(rank) / 100.0)).clamp(0.0, 100.0);
        let caught = f64::from(rng.below(10_000)) / 100.0 < chance;
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
    let mut seen: Vec<&String> = Vec::new();
    defenses
        .iter()
        .filter(|defense| {
            if seen.contains(defense) {
                false
            } else {
                seen.push(defense);
                true
            }
        })
        .fold(1.0, |acc, defense| {
            acc * chart.multiplier(attack, defense).unwrap_or(1.0)
        })
}

fn expected_damage(
    attacker: &Combatant,
    target: &Combatant,
    movement: &BattleMove,
    chart: &TypeChart,
) -> f32 {
    let (attack, defense) = if movement.category == "Physical" {
        (attacker.attack, target.defense)
    } else {
        (attacker.special_attack, target.special_defense)
    };
    let level_term = (2 * u32::from(attacker.level) / 5) + 2;
    let core = (level_term * u32::from(movement.power) * attack / defense.max(1) / 50 + 2).max(1);
    let stab = if attacker
        .species
        .types
        .iter()
        .any(|t| t == &movement.move_type)
    {
        1.5
    } else {
        1.0
    };
    let effectiveness = type_effectiveness(chart, &movement.move_type, &target.species.types);
    let accuracy = if movement.accuracy == 0 || movement.accuracy >= 100 {
        1.0
    } else {
        f32::from(movement.accuracy) / 100.0
    };
    core as f32 * stab * effectiveness * 0.925 * accuracy
}

fn calculate_damage(
    attacker: &Combatant,
    target: &Combatant,
    attack_type: &str,
    category: &str,
    power: u16,
    chart: &TypeChart,
    rng: &mut impl BattleRng,
) -> (u32, bool, f32) {
    // Conventional-series damage, crit chance and random range: assumed, unverified for PLA.
    let (attack, defense) = if category == "Physical" {
        (attacker.attack, target.defense)
    } else {
        (attacker.special_attack, target.special_defense)
    };
    let level_term = (2 * u32::from(attacker.level) / 5) + 2;
    let core = (level_term * u32::from(power) * attack / defense.max(1) / 50 + 2).max(1);
    let stab = if attacker.species.types.iter().any(|t| t == attack_type) {
        1.5
    } else {
        1.0
    };
    let effectiveness = type_effectiveness(chart, attack_type, &target.species.types);
    let critical = rng.below(16) == 0;
    let random = 85 + rng.below(16);
    let crit = if critical { 1.5 } else { 1.0 };
    let damage = (core as f32 * stab * effectiveness * crit * random as f32 / 100.0).floor() as u32;
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
/// Priority isn't included in the exported move TSV. These conventional-series
/// exceptions are a small, explicitly unverified fallback; callers can override.
fn assumed_move_priority(name: &str) -> i8 {
    match name {
        "extreme-speed" => 2,
        "quick-attack" | "aqua-jet" | "bullet-punch" | "ice-shard" | "shadow-sneak"
        | "mach-punch" | "vacuum-wave" | "water-shuriken" => 1,
        _ => 0,
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
            4.0
        );
        assert_eq!(
            type_effectiveness(d.type_chart(), "Electric", &["Ground".into()]),
            0.0
        );
        assert_eq!(d.type_chart().multiplier("Fire", "Grass").unwrap(), 2.0);
        assert_eq!(d.type_chart().multiplier("Normal", "Rock").unwrap(), 0.5);
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
            &mut Fixed(vec![1, 99, 1, 99]),
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
            before - (b.combatants[1].max_hp / 8).max(1)
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
        assert!(
            b.attempt_capture(&d, 1, "Gigaton", true, 0, 1, true, &mut Fixed(vec![0]))
                .unwrap()
        );
        assert!(b.combatants[1].caught);
    }
}
