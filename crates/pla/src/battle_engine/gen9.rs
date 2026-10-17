//! Generation-9 moves (ids 851..920) loaded from `data/gen9_moves.tsv`.
//!
//! The PLA move data stops at id 850, so these 70 moves come from an EXTERNAL source (see the
//! sheet manifest: Pokemon Showdown `moves.ts` cross-checked with WikiDex), not from the game
//! files: every value here is EXTERNAL/PROBABLE, never VERIFIED against the game. Only the rows
//! the sheet classifies `engine_class = a` are loaded (30 moves): moves whose whole behaviour
//! fits the columns the engine already reads from `data/pml_move_effects.tsv` (priority, crit
//! rank, hits, drain/heal, one paralysis/burn/poison roll, stat stages on a single receiver).
//! Rows classified `b` need new engine logic and are skipped, never approximated. Gigaton
//! Hammer and Blood Moon load because the engine now honours their `cantusetwice` flag through
//! `MoveEffects::repeat_lock`; Last Respects, Rage Fist, Hard Press, Collision Course, Electro
//! Drift and Fickle Beam load through the derived `PowerRule` callbacks.
//!
//! Gen-9 moves have no agile/strong style rows and no effect timeline, so the style table falls
//! back to the normal style and the effects layer draws nothing for them.
//!
//! The mapping to [`MoveEffects`] follows the conventions of the PLA rows (checked against
//! Close Combat, Swords Dance, Acid Spray, Flamethrower, Thunder Wave and Slash):
//! `meta_category` 7 = damage + user stat change, 6 = damage + target stat change,
//! 4 = damage + ailment, 2 = status stat change, 0 = plain damage; `target_raw` 7 = self.
//! The numeric ids of the three supported ailments are those of [`ailment_status`]
//! (1 paralysis, 4 burn, 5 poison). The mapping is DERIVED, not data from the game. Stat stages
//! run on the PLA three-state rank groups, so a Generation-9 `-2`/`+2` change lands at `-1`/`+1`
//! (the game whose moves these are is not the game whose battle engine this is).

use std::fs;
use std::path::Path;

use super::effects::{ailment_status, MoveEffects, PowerRule, Stat, StatChange};

/// One loadable generation-9 move: the base row and its effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gen9Move {
    pub id: u16,
    /// Lower-kebab canonical name, built from the English name (`lumina-crash`), the same
    /// convention as `pml_moves.tsv`.
    pub name: String,
    pub move_type: String,
    pub category: String,
    pub power: u16,
    pub accuracy: u16,
    pub pp: u16,
    pub effects: MoveEffects,
}

/// Load `<data_root>/gen9_moves.tsv`. A missing file is not an error: the engine then behaves
/// as it did before the sheet existed.
pub fn load(data_root: impl AsRef<Path>) -> Result<Vec<Gen9Move>, String> {
    let path = data_root.as_ref().join("gen9_moves.tsv");
    match fs::read_to_string(&path) {
        Ok(text) => from_tsv(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

/// Canonical lower-kebab name: letters and digits lowercased, every other run becomes `-`.
pub fn kebab(english_name: &str) -> String {
    let mut out = String::new();
    for c in english_name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

fn stat_from_key(key: &str) -> Result<Stat, String> {
    Ok(match key {
        "atk" => Stat::Attack,
        "def" => Stat::Defense,
        "spa" => Stat::SpecialAttack,
        "spd" => Stat::SpecialDefense,
        "spe" => Stat::Speed,
        "accuracy" => Stat::Accuracy,
        "evasion" => Stat::Evasion,
        other => return Err(format!("gen9 moves: unknown stat key {other:?}")),
    })
}

/// Base-power callback of the sheet, DERIVED from Showdown (`basePowerCallback`/`onBasePower`).
/// Temper Flare is absent: it needs the previous move's failure state, which the engine does
/// not model yet, so it stays class b.
fn power_rule(name: &str) -> PowerRule {
    match name {
        "last-respects" => PowerRule::LastRespects,
        "rage-fist" => PowerRule::RageFist,
        "hard-press" => PowerRule::HardPress,
        "collision-course" | "electro-drift" => PowerRule::SuperEffectiveBoost,
        "fickle-beam" => PowerRule::FickleBeam,
        _ => PowerRule::None,
    }
}

/// Parse `atk:2,def:-2` into `(stat, stage)` pairs.
fn parse_boosts(text: &str) -> Result<Vec<(Stat, i8)>, String> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    text.split(',')
        .map(|pair| {
            let (key, stage) = pair
                .split_once(':')
                .ok_or_else(|| format!("gen9 moves: bad boost {pair:?}"))?;
            let stage = stage
                .parse::<i8>()
                .map_err(|_| format!("gen9 moves: bad stage in {pair:?}"))?;
            Ok((stat_from_key(key)?, stage))
        })
        .collect()
}

/// Parse the rows classified `a` of the sheet text.
pub fn from_tsv(text: &str) -> Result<Vec<Gen9Move>, String> {
    let mut lines = text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let header: Vec<&str> = lines
        .next()
        .ok_or("gen9 moves: missing header")?
        .split('\t')
        .map(|h| h.split(':').next().unwrap_or(h).trim_end_matches('*'))
        .collect();
    let col = |name: &str| -> Result<usize, String> {
        header
            .iter()
            .position(|h| *h == name)
            .ok_or_else(|| format!("gen9 moves: missing column {name}"))
    };
    let (c_move, c_en, c_type, c_cat) = (
        col("move")?,
        col("name_en")?,
        col("type")?,
        col("category")?,
    );
    let (c_pow, c_acc, c_pp, c_prio) = (
        col("power")?,
        col("accuracy")?,
        col("pp")?,
        col("priority")?,
    );
    let (c_target, c_flags, c_crit) = (col("target")?, col("flags")?, col("crit_ratio")?);
    let (c_minh, c_maxh, c_drain, c_heal) = (
        col("min_hits")?,
        col("max_hits")?,
        col("drain_pct")?,
        col("heal_pct")?,
    );
    let (c_boosts, c_self_boosts) = (col("boosts")?, col("self_boosts")?);
    let (c_sec_chance, c_sec_status, c_sec_vol) =
        (col("sec_chance")?, col("sec_status")?, col("sec_volatile")?);
    let (c_sec_boosts, c_sec_self) = (col("sec_boosts")?, col("sec_self_boosts")?);
    let c_class = col("engine_class")?;

    let mut out = Vec::new();
    for (n, line) in lines.enumerate() {
        let v: Vec<&str> = line.split('\t').collect();
        if v.len() != header.len() {
            return Err(format!("gen9 moves row {}: column count", n + 2));
        }
        if v[c_class] != "a" {
            continue;
        }
        let num = |i: usize| -> Result<i64, String> {
            if v[i].is_empty() {
                return Ok(0);
            }
            v[i].parse::<i64>()
                .map_err(|_| format!("gen9 moves row {}: bad number {:?}", n + 2, v[i]))
        };
        let name = kebab(v[c_en]);
        let damaging = v[c_cat] != "Status";
        let self_target = v[c_target] == "self";
        let flags: Vec<&str> = v[c_flags].split(';').collect();
        let secondary_chance = num(c_sec_chance)? as u8;

        // Stat changes with the receiver they imply. The sheet only marks as `a` the moves
        // whose changes all go to one receiver, so the first non-empty source decides it.
        let mut changes: Vec<StatChange> = Vec::new();
        let mut to_user = false;
        let mut push = |pairs: Vec<(Stat, i8)>, chance: u8| {
            for (stat, stage) in pairs {
                changes.push(StatChange {
                    stat,
                    stage,
                    chance,
                    turns: 0,
                });
            }
        };
        let status_boosts = parse_boosts(v[c_boosts])?;
        if !status_boosts.is_empty() {
            to_user = self_target;
            push(status_boosts, 0);
        }
        let self_boosts = parse_boosts(v[c_self_boosts])?;
        if !self_boosts.is_empty() {
            to_user = true;
            push(self_boosts, 100);
        }
        let sec_boosts = parse_boosts(v[c_sec_boosts])?;
        if !sec_boosts.is_empty() {
            push(sec_boosts, secondary_chance);
        }
        let sec_self = parse_boosts(v[c_sec_self])?;
        if !sec_self.is_empty() {
            to_user = true;
            push(sec_self, secondary_chance);
        }
        if changes.len() > 3 {
            return Err(format!("gen9 moves {name}: more than 3 stat changes"));
        }

        let ailment_id: u16 = match v[c_sec_status] {
            "par" => 1,
            "brn" => 4,
            "psn" => 5,
            "" => 0,
            other => {
                return Err(format!(
                    "gen9 moves {name}: ailment {other:?} is not class a"
                ))
            }
        };
        let flinch_chance = if v[c_sec_vol] == "flinch" {
            secondary_chance
        } else {
            0
        };
        let crit = num(c_crit)?;
        let meta_category = match (damaging, !changes.is_empty(), ailment_id != 0) {
            (true, true, _) if to_user => 7,
            (true, true, _) => 6,
            (true, false, true) => 4,
            (false, true, _) => 2,
            (false, false, true) => 1,
            _ => 0,
        };
        let effects = MoveEffects {
            name: name.clone(),
            priority: num(c_prio)? as i8,
            crit_rank: crit.saturating_sub(1).max(0) as u8,
            flinch_chance,
            min_hits: num(c_minh)? as u8,
            max_hits: num(c_maxh)? as u8,
            drain_pct: (num(c_drain)? as i8).max(0),
            heal_pct: num(c_heal)? as i8,
            ailment: ailment_status(ailment_id),
            ailment_id,
            ailment_chance: if ailment_id != 0 { secondary_chance } else { 0 },
            ailment_turns: None,
            meta_category,
            target_raw: if self_target { 7 } else { 0 },
            stat_changes: changes,
            contact: flags.contains(&"contact"),
            effect_id: 0,
            charge: false,
            recharge: false,
            protectable: flags.contains(&"protect"),
            repeat_lock: flags.contains(&"cantusetwice"),
            power_rule: power_rule(&name),
        };
        out.push(Gen9Move {
            id: num(c_move)? as u16,
            name,
            move_type: v[c_type].to_owned(),
            category: v[c_cat].to_owned(),
            power: num(c_pow)? as u16,
            accuracy: num(c_acc)? as u16,
            pp: num(c_pp)? as u16,
            effects,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::BattleData;
    use crate::battle_engine::effects::{EffectEvent, MoveEffectsTable, PowerRule};
    use crate::battle_engine::statuses::StatusEvent;
    use crate::battle_engine::{Action, Battle, BattleRng, Combatant, Event, MajorStatus};
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

    fn all() -> Vec<Gen9Move> {
        load(root()).unwrap()
    }

    fn find<'a>(moves: &'a [Gen9Move], name: &str) -> &'a Gen9Move {
        moves
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("{name} missing"))
    }

    #[test]
    fn kebab_names_follow_the_pml_convention() {
        assert_eq!(kebab("Lumina Crash"), "lumina-crash");
        assert_eq!(kebab("Make It Rain"), "make-it-rain");
        assert_eq!(kebab("Tachyon Cutter"), "tachyon-cutter");
    }

    #[test]
    fn only_the_class_a_rows_are_loaded() {
        let moves = all();
        assert_eq!(moves.len(), 30);
        assert!(moves.iter().all(|m| (851..=920).contains(&m.id)));
        // Class b needs engine logic that does not exist: never approximated.
        for skipped in [
            "tera-blast",
            "psychic-noise",
            "revival-blessing",
            "temper-flare",
            "salt-cure",
        ] {
            assert!(
                moves.iter().all(|m| m.name != skipped),
                "{skipped} must not load"
            );
        }
    }

    #[test]
    fn base_stats_match_the_sheet() {
        let moves = all();
        let lc = find(&moves, "lumina-crash");
        assert_eq!((lc.id, lc.power, lc.accuracy, lc.pp), (855, 80, 100, 10));
        assert_eq!(
            (lc.move_type.as_str(), lc.category.as_str()),
            ("Psychic", "Special")
        );
        let kc = find(&moves, "kowtow-cleave");
        assert_eq!(kc.accuracy, 101, "never-miss uses 101, as pml_moves does");
    }

    #[test]
    fn effects_map_to_the_engine_columns() {
        let moves = all();
        let jet = &find(&moves, "jet-punch").effects;
        assert_eq!(jet.priority, 1);
        let triple = &find(&moves, "triple-dive").effects;
        assert_eq!((triple.min_hits, triple.max_hits), (3, 3));
        let bitter = &find(&moves, "bitter-blade").effects;
        assert_eq!(bitter.drain_pct, 50);
        let cutter = &find(&moves, "aqua-cutter").effects;
        assert_eq!(
            cutter.crit_rank, 1,
            "Showdown critRatio 2 is one rank, like Slash"
        );
        let torque = &find(&moves, "combat-torque").effects;
        assert_eq!(torque.ailment, Some(MajorStatus::Paralysis));
        assert_eq!(torque.ailment_chance, 30);
        assert_eq!(
            find(&moves, "blazing-torque").effects.ailment,
            Some(MajorStatus::Burn)
        );
        assert_eq!(
            find(&moves, "noxious-torque").effects.ailment,
            Some(MajorStatus::Poison)
        );
    }

    #[test]
    fn stat_receivers_follow_the_pla_conventions() {
        use crate::battle_engine::effects::EffectTarget;
        let moves = all();
        // Target drop: damage + lower target stat (meta 6), like Acid Spray.
        let lumina = &find(&moves, "lumina-crash").effects;
        assert_eq!(lumina.meta_category, 6);
        assert_eq!(lumina.stat_target(), EffectTarget::Opponent);
        assert_eq!(
            (lumina.stat_changes[0].stat, lumina.stat_changes[0].stage),
            (Stat::SpecialDefense, -2)
        );
        // Self drop after damage (meta 7), like Close Combat.
        let rain = &find(&moves, "make-it-rain").effects;
        assert_eq!(rain.meta_category, 7);
        assert_eq!(rain.stat_target(), EffectTarget::User);
        assert_eq!(rain.stat_changes[0].chance, 100);
        // Self raise as a secondary effect.
        let song = &find(&moves, "torch-song").effects;
        assert_eq!(song.stat_target(), EffectTarget::User);
        assert_eq!(
            (song.stat_changes[0].stat, song.stat_changes[0].stage),
            (Stat::SpecialAttack, 1)
        );
        // Two drops on the user.
        let cannon = &find(&moves, "armor-cannon").effects;
        assert_eq!(cannon.stat_changes.len(), 2);
        assert_eq!(cannon.stat_target(), EffectTarget::User);
        // Status move on the opponent: +2 Atk, -2 Def, chance 0 = always.
        let spicy = &find(&moves, "spicy-extract").effects;
        assert_eq!(spicy.stat_target(), EffectTarget::Opponent);
        assert_eq!(spicy.stat_changes.len(), 2);
        assert!(spicy.stat_changes.iter().all(|s| s.chance == 0));
    }

    #[test]
    fn moves_reach_the_battle_data_and_the_effects_table() {
        let data = BattleData::load(root()).unwrap();
        let by_name = data.move_row("lumina-crash").unwrap().expect("by name");
        let by_id = data.move_row_by_id(855).unwrap().expect("by id");
        assert_eq!(by_name, by_id);
        assert_eq!(by_name.power, 80);
        assert!(data.move_row("tera-blast").unwrap().is_none());
        // The PLA rows keep their ids and values.
        assert_eq!(data.move_row("pound").unwrap().unwrap().power, 40);

        let table = MoveEffectsTable::load(root()).unwrap();
        assert_eq!(table.get("jet-punch").unwrap().priority, 1);
        assert_eq!(table.get("quick-attack").unwrap().priority, 1);
        assert!(table.get("tera-blast").is_none());
        // Repeat-lock rows load and carry their flag.
        assert!(table.get("gigaton-hammer").unwrap().repeat_lock);
        assert!(table.get("blood-moon").unwrap().repeat_lock);
    }

    fn mon(data: &BattleData, species: &str, moves: &[&str], level: u8) -> Combatant {
        let sp = data.species_row(species).unwrap().unwrap();
        let mv = moves
            .iter()
            .map(|n| data.move_row(n).unwrap().unwrap())
            .collect();
        Combatant::from_base(sp, level, mv)
    }

    fn duel(a: (&str, &[&str], u8), b: (&str, &[&str], u8)) -> Battle {
        let data = BattleData::load(root()).unwrap();
        let mut battle = Battle::new(
            vec![mon(&data, a.0, a.1, a.2), mon(&data, b.0, b.1, b.2)],
            vec![vec![0], vec![1]],
            data.type_chart().clone(),
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

    /// Windowless battle check for the class-a rows: priority, a 100 % self boost, a 50 %
    /// drain and an opponent drop, all through the normal engine turn.
    #[test]
    fn class_a_moves_play_a_headless_battle() {
        // Jet Punch (priority 1): the slower user acts first and spends one PP.
        let mut b = duel(("bidoof", &["jet-punch"], 20), ("starly", &["tackle"], 20));
        assert!(b.combatants[0].speed < b.combatants[1].speed);
        let ev = b.play_turn(&[act(0), act(1)], &mut Fixed(vec![]));
        assert_eq!(ev[1], Event::ActedFirst(0));
        assert!(b.combatants[1].hp < b.combatants[1].max_hp);
        assert_eq!(b.combatants[0].moves[0].pp, 14);
        assert!(ev
            .iter()
            .any(|e| matches!(e, Event::UsedMove { name, .. } if name == "jet-punch")));

        // Torch Song: 100 % self Special Attack +1 on PLA's shared attack rank.
        let mut b = duel(
            ("pikachu", &["torch-song"], 40),
            ("snorlax", &["tackle"], 80),
        );
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        assert_eq!(b.stages(0).attack, 1);

        // Bitter Blade: heals 50 % of the damage dealt.
        let mut b = duel(
            ("pikachu", &["bitter-blade"], 50),
            ("snorlax", &["tackle"], 80),
        );
        b.combatants[0].hp = b.combatants[0].max_hp - 40;
        let before = b.combatants[0].hp;
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        assert!(b.combatants[0].hp > before, "drain must heal the user");
        assert!(b
            .take_effect_events()
            .iter()
            .any(|e| matches!(e, EffectEvent::Drained { .. } | EffectEvent::Healed { .. })));

        // Lumina Crash: target Defense/SpD down. PLA has three rank states, so the
        // Generation-9 -2 lands at -1 (documented port adaptation).
        let mut b = duel(
            ("pikachu", &["lumina-crash"], 60),
            ("snorlax", &["tackle"], 80),
        );
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        assert_eq!(b.stages(1).defense, -1);
        assert!(b.take_effect_events().iter().any(|e| matches!(
            e,
            EffectEvent::StagesChanged {
                target: 1,
                stat: Stat::SpecialDefense,
                new_state: -1
            }
        )));
    }

    #[test]
    fn power_rules_follow_the_showdown_callbacks() {
        let moves = all();
        assert_eq!(
            find(&moves, "last-respects").effects.power_rule,
            PowerRule::LastRespects
        );
        assert_eq!(
            find(&moves, "rage-fist").effects.power_rule,
            PowerRule::RageFist
        );
        assert_eq!(
            find(&moves, "hard-press").effects.power_rule,
            PowerRule::HardPress
        );
        assert_eq!(
            find(&moves, "collision-course").effects.power_rule,
            PowerRule::SuperEffectiveBoost
        );
        assert_eq!(
            find(&moves, "fickle-beam").effects.power_rule,
            PowerRule::FickleBeam
        );

        // Last Respects: 50 + 50 per fainted member of the user's side.
        let data = BattleData::load(root()).unwrap();
        let mut deps = |names: &[&str]| {
            names
                .iter()
                .map(|n| mon(&data, n, &["tackle"], 20))
                .collect::<Vec<_>>()
        };
        let mut b = Battle::new(
            {
                let mut v = deps(&["pikachu", "bidoof", "starly", "snorlax"]);
                v[0] = mon(&data, "pikachu", &["last-respects"], 50);
                v
            },
            vec![vec![0, 1], vec![2, 3]],
            data.type_chart().clone(),
        )
        .unwrap();
        b.set_move_effects(MoveEffectsTable::load(root()).unwrap());
        assert_eq!(
            b.effective_power(
                0,
                2,
                50,
                PowerRule::LastRespects,
                "Ghost",
                &mut Fixed(vec![])
            ),
            50
        );
        b.combatants[1].hp = 0;
        assert_eq!(
            b.effective_power(
                0,
                2,
                50,
                PowerRule::LastRespects,
                "Ghost",
                &mut Fixed(vec![])
            ),
            100
        );

        // Hard Press: ~100 at full HP, ~50 at half, minimum 1.
        assert_eq!(
            b.effective_power(0, 3, 0, PowerRule::HardPress, "Steel", &mut Fixed(vec![])),
            100
        );
        b.combatants[3].hp = b.combatants[3].max_hp / 2;
        assert_eq!(
            b.effective_power(0, 3, 0, PowerRule::HardPress, "Steel", &mut Fixed(vec![])),
            50
        );

        // Collision Course/Electro Drift: x1.333 only when super effective (Electric vs Flying).
        assert_eq!(
            b.effective_power(
                0,
                2,
                100,
                PowerRule::SuperEffectiveBoost,
                "Electric",
                &mut Fixed(vec![])
            ),
            133
        );
        assert_eq!(
            b.effective_power(
                0,
                2,
                100,
                PowerRule::SuperEffectiveBoost,
                "Normal",
                &mut Fixed(vec![])
            ),
            100
        );

        // Fickle Beam: the 30 % roll doubles the power.
        assert_eq!(
            b.effective_power(
                0,
                2,
                80,
                PowerRule::FickleBeam,
                "Dragon",
                &mut Fixed(vec![29])
            ),
            160
        );
        assert_eq!(
            b.effective_power(
                0,
                2,
                80,
                PowerRule::FickleBeam,
                "Dragon",
                &mut Fixed(vec![30])
            ),
            80
        );

        // Rage Fist: 50 + 50 per hit taken (starly hits pikachu in the first slot).
        let mut b = duel(("pikachu", &["tackle"], 50), ("starly", &["tackle"], 20));
        assert_eq!(
            b.effective_power(0, 1, 50, PowerRule::RageFist, "Ghost", &mut Fixed(vec![])),
            50
        );
        b.play_turn(&[act(1)], &mut Fixed(vec![0, 1, 0]));
        assert_eq!(
            b.effective_power(0, 1, 50, PowerRule::RageFist, "Ghost", &mut Fixed(vec![])),
            100
        );
    }

    #[test]
    fn repeat_lock_moves_cannot_be_used_twice_in_a_row() {
        let mut b = duel(
            ("pikachu", &["gigaton-hammer", "tackle"], 50),
            ("snorlax", &["tackle"], 80),
        );
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        assert_eq!(b.combatants[0].moves[0].pp, 4);
        let hp = b.combatants[1].hp;

        // The very next turn the slot is locked: no PP spent and no damage dealt.
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        assert_eq!(b.combatants[0].moves[0].pp, 4);
        assert_eq!(b.combatants[1].hp, hp);
        assert!(b
            .take_status_events()
            .iter()
            .any(|e| matches!(e, StatusEvent::MoveLocked { actor: 0, .. })));

        // Another move replaces the last move, so the lock is gone on the following turn.
        b.play_turn(
            &[Action {
                actor: 0,
                move_slot: 1,
            }],
            &mut Fixed(vec![0, 1, 0]),
        );
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        assert_eq!(b.combatants[0].moves[0].pp, 3);
    }
}
