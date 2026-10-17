//! Generation-9 moves (ids 851..920) loaded from `data/gen9_moves.tsv`.
//!
//! The PLA move data stops at id 850, so these 70 moves come from an EXTERNAL source (see the
//! sheet manifest: Pokemon Showdown `moves.ts` cross-checked with WikiDex), not from the game
//! files: every value here is EXTERNAL/PROBABLE, never VERIFIED against the game. Only the rows
//! the sheet classifies `engine_class = a` are loaded (45 moves): moves whose whole behaviour
//! fits the columns the engine already reads from `data/pml_move_effects.tsv` (priority, crit
//! rank, hits, drain/heal, one paralysis/burn/poison/confusion roll, stat stages on a single
//! receiver) or a named hook in [`hooks`]. Rows classified `b` need new engine logic and are
//! skipped, never approximated.
//! Gigaton Hammer and Blood Moon load because the engine now honours their `cantusetwice` flag
//! through `MoveEffects::repeat_lock`; Last Respects, Rage Fist, Hard Press, Collision Course,
//! Electro Drift and Fickle Beam load through the derived `PowerRule` callbacks; Axe Kick and
//! Supercell Slam load through `crash_damage_pct`; Salt Cure and Syrup Bomb load through
//! `ResidualVolatile`; Hydro Steam and Matcha Gotcha load through `thaws_target`; Population
//! Bomb loads through `multi_accuracy`.
//! Family A (2026-10-17) loads through derived hooks: Flower Trick (`always_crit`), Ice Spinner
//! and Psyblade (plain damage: the terrain callbacks are omitted, Hisui has no terrain), Mortal
//! Spin (`clear_user_trap`; its poison is an ordinary secondary), Tidy Up (its `onHit` self
//! boosts are filled into the sheet's `self_boosts` column), Fillet Away (`half_hp_cost` plus
//! its boosts; the Generation-9 +2 collapses to +1 per PLA rank group) and Ruination
//! (`half_current_hp_damage`).
//! Family B (same day) adds the turn-memory rows: Comeuppance (`return_last_damage`), Alluring
//! Voice (`confuse_if_target_raised`), Double Shock (`fail_unless_user_has_move_type` +
//! `remove_user_type_after_hit`), Upper Hand (`require_target_priority`) and Thunderclap
//! (`require_target_attack`; both read the actions declared for the turn, so they are refused
//! in the single-action PLA queue flow) and Electro Shot (`charge` flag plus `charge_boost_spa`
//! and `charge_skip_in_rain`). Malignant Chain (919) loads with the sheet's `tox` secondary,
//! mapped to the synthetic engine ailment id 15 (badly poisoned; see `effects.rs` and
//! `reports/data-progress/sleep_toxic.md`).
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

use super::effects::{
    ailment_status, MoveEffects, MoveHooks, PowerRule, ResidualVolatile, Stat, StatChange,
};

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

/// Behaviour hooks of the sheet, DERIVED from the pinned Showdown source (the sheet's
/// `callbacks`/`other_props` columns name them; none of this is game data). PLA rows never
/// reach this function. Each hook's confidence is EXTERNAL/PROBABLE; the engine's meaning for
/// it is in `MoveHooks`.
fn hooks(name: &str) -> MoveHooks {
    match name {
        // willCrit=true -> the engine's guaranteed critical rank (6).
        "flower-trick" => MoveHooks {
            always_crit: true,
            ..MoveHooks::default()
        },
        // onAfterHit/onAfterSubDamage -> the user leaves partial trapping behind.
        "mortal-spin" => MoveHooks {
            clear_user_trap: true,
            ..MoveHooks::default()
        },
        // onTry/onTryHit/onHit -> half-max-HP cost; refuses at or below half HP.
        "fillet-away" => MoveHooks {
            half_hp_cost: true,
            ..MoveHooks::default()
        },
        // damageCallback -> fixed floor(target HP / 2), minimum 1, typeless.
        "ruination" => MoveHooks {
            half_current_hp_damage: true,
            ..MoveHooks::default()
        },
        // damageCallback + onTry -> 3/2 of the last hit taken this turn; refused without one.
        "comeuppance" => MoveHooks {
            return_last_damage: true,
            ..MoveHooks::default()
        },
        // onTry -> refused unless the target's chosen action has priority (`willMove`).
        "upper-hand" => MoveHooks {
            require_target_priority: true,
            ..MoveHooks::default()
        },
        // onTry -> refused unless the target's chosen action is an attack (`willMove`).
        "thunderclap" => MoveHooks {
            require_target_attack: true,
            ..MoveHooks::default()
        },
        // secondary.onHit -> confuses a target that raised a stat this turn.
        "alluring-voice" => MoveHooks {
            confuse_if_target_raised: true,
            ..MoveHooks::default()
        },
        // onTryMove (`hasType('Electric')`) + self.onHit (removes it after the hit).
        "double-shock" => MoveHooks {
            fail_unless_user_has_move_type: true,
            remove_user_type_after_hit: true,
            ..MoveHooks::default()
        },
        // onTryMove -> the charge turn boosts Sp. Atk one rank; rain skips the charge.
        "electro-shot" => MoveHooks {
            charge_boost_spa: true,
            charge_skip_in_rain: true,
            ..MoveHooks::default()
        },
        _ => MoveHooks::default(),
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
    let c_crash = col("crash_damage_pct")?;
    let c_thaw = col("thaws_target")?;
    let c_multi_acc = col("multi_accuracy")?;

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
        let hook = hooks(&name);

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

        // Confusion is not a major status: the status module handles ailment id 8 when the
        // volatile-status rules are on, exactly as it does for the PLA rows.
        let ailment_id: u16 = match (v[c_sec_status], v[c_sec_vol]) {
            ("par", _) => 1,
            ("brn", _) => 4,
            ("psn", _) => 5,
            // Synthetic id 15 = badly poisoned (the game's `0xf01` poison mark; see effects.rs).
            ("tox", _) => 15,
            ("", "confusion") => 8,
            ("", _) => 0,
            (other, _) => {
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
        let residual_volatile = match v[c_sec_vol] {
            "saltcure" => ResidualVolatile::SaltCure,
            "syrupbomb" => ResidualVolatile::SyrupBomb,
            _ => ResidualVolatile::None,
        };
        let crit = num(c_crit)?;
        // Flower Trick carries `willCrit=true` instead of a crit ratio: the engine's rank 6
        // means "always critical" (see `critical_hit`).
        let crit_rank = if hook.always_crit {
            6
        } else {
            crit.saturating_sub(1).max(0) as u8
        };
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
            crit_rank,
            flinch_chance,
            min_hits: num(c_minh)? as u8,
            max_hits: num(c_maxh)? as u8,
            drain_pct: (num(c_drain)? as i8).max(0),
            heal_pct: num(c_heal)? as i8,
            ailment: ailment_status(ailment_id),
            ailment_id,
            ailment_chance: if ailment_id != 0 || residual_volatile != ResidualVolatile::None {
                secondary_chance
            } else {
                0
            },
            ailment_turns: None,
            meta_category,
            target_raw: if self_target { 7 } else { 0 },
            stat_changes: changes,
            contact: flags.contains(&"contact"),
            effect_id: 0,
            // The sheet's `charge` flag (Electro Shot); `recharge` stays false: the rule was
            // REFUTED for PLA and no external row needs it.
            charge: flags.contains(&"charge"),
            recharge: false,
            protectable: flags.contains(&"protect"),
            repeat_lock: flags.contains(&"cantusetwice"),
            power_rule: power_rule(&name),
            crash_damage_pct: num(c_crash)? as u8,
            residual_volatile,
            thaws_target: num(c_thaw)? != 0,
            multi_accuracy: num(c_multi_acc)? != 0,
            hooks: hook,
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
    use crate::battle_engine::effects::{
        EffectEvent, MoveEffectsTable, PowerRule, ResidualVolatile,
    };
    use crate::battle_engine::statuses::{StatusEvent, StatusRules};
    use crate::battle_engine::{
        Action, Battle, BattleRng, BattleWeather, Combatant, Event, MajorStatus,
    };
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
        // 38 classic-column rows + family A (861, 866, 868, 870, 875, 877, 882), family B
        // (892, 894, 905, 909, 914, 918) and Malignant Chain (919).
        assert_eq!(moves.len(), 52);
        assert!(moves.iter().all(|m| (851..=920).contains(&m.id)));
        // Class b needs engine logic that does not exist: never approximated.
        for skipped in [
            "tera-blast",
            "psychic-noise",
            "revival-blessing",
            "temper-flare",
            "wicked-torque",
            "nihil-light",
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
        // Confusion secondaries use the status module's ailment id 8 (no major status).
        let magical = &find(&moves, "magical-torque").effects;
        assert_eq!(magical.ailment, None);
        assert_eq!(magical.ailment_id, 8);
        assert_eq!(magical.ailment_chance, 30);
        // Crash damage: half the user's max HP on a miss.
        assert_eq!(find(&moves, "axe-kick").effects.crash_damage_pct, 50);
        assert_eq!(find(&moves, "supercell-slam").effects.crash_damage_pct, 50);
        // Residual volatiles: Salt Cure (until switching) and Syrup Bomb (three drops).
        assert_eq!(
            find(&moves, "salt-cure").effects.residual_volatile,
            ResidualVolatile::SaltCure
        );
        assert_eq!(
            find(&moves, "syrup-bomb").effects.residual_volatile,
            ResidualVolatile::SyrupBomb
        );
        // Thawing hits.
        assert!(find(&moves, "hydro-steam").effects.thaws_target);
        assert!(find(&moves, "matcha-gotcha").effects.thaws_target);
        // Multi-hit accuracy.
        assert!(find(&moves, "population-bomb").effects.multi_accuracy);
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
    fn crash_damage_and_confusion_secondaries() {
        // Axe Kick miss: half the user's max HP is lost and a crash event is logged.
        let mut b = duel(("pikachu", &["axe-kick"], 50), ("snorlax", &["tackle"], 80));
        let before = b.combatants[0].hp;
        let ev = b.play_turn(&[act(0)], &mut Fixed(vec![95]));
        assert!(ev.iter().any(|e| matches!(e, Event::Missed { .. })));
        assert_eq!(b.combatants[0].hp, before - before / 2);
        assert!(b
            .take_effect_events()
            .iter()
            .any(|e| matches!(e, EffectEvent::CrashDamage { actor: 0, .. })));

        // Magical Torque's 30 % confusion runs through the status module.
        let mut b = duel(
            ("pikachu", &["magical-torque"], 50),
            ("snorlax", &["tackle"], 80),
        );
        b.set_status_rules(StatusRules {
            volatile_statuses: true,
            ..StatusRules::default()
        });
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0, 29, 0]));
        assert!(b.is_confused(1));
    }

    #[test]
    fn residual_volatiles_tick_at_end_of_turn() {
        // Salt Cure: 1/8 max HP per turn, persists until the target leaves the field.
        let mut b = duel(
            ("pikachu", &["salt-cure"], 60),
            ("snorlax", &["tackle"], 80),
        );
        b.set_status_rules(StatusRules {
            volatile_statuses: true,
            ..StatusRules::default()
        });
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        let max = b.combatants[1].max_hp;
        let events = b.take_status_events();
        assert!(events
            .iter()
            .any(|e| matches!(e, StatusEvent::SaltCure { target: 1 })));
        assert!(events.iter().any(
            |e| matches!(e, StatusEvent::HurtBySaltCure { target: 1, damage } if *damage == max / 8)
        ));
        b.play_turn(&[], &mut Fixed(vec![]));
        assert!(b
            .take_status_events()
            .iter()
            .any(|e| matches!(e, StatusEvent::HurtBySaltCure { target: 1, .. })));

        // Against Water or Steel the tick is 1/4.
        let mut b = duel(
            ("pikachu", &["salt-cure"], 80),
            ("magikarp", &["tackle"], 50),
        );
        b.set_status_rules(StatusRules {
            volatile_statuses: true,
            ..StatusRules::default()
        });
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        let max = b.combatants[1].max_hp;
        assert!(b.take_status_events().iter().any(
            |e| matches!(e, StatusEvent::HurtBySaltCure { target: 1, damage } if *damage == max / 4)
        ));

        // Syrup Bomb: one Speed drop at the end of each of three turns.
        let mut b = duel(
            ("pikachu", &["syrup-bomb"], 60),
            ("snorlax", &["tackle"], 80),
        );
        b.set_status_rules(StatusRules {
            volatile_statuses: true,
            ..StatusRules::default()
        });
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        assert!(b
            .take_status_events()
            .iter()
            .any(|e| matches!(e, StatusEvent::SyrupBomb { target: 1 })));
        assert_eq!(b.stages(1).speed, -1);
        b.play_turn(&[], &mut Fixed(vec![]));
        let events = b.take_status_events();
        assert!(!events
            .iter()
            .any(|e| matches!(e, StatusEvent::SyrupBombEnded { .. })));
        b.play_turn(&[], &mut Fixed(vec![]));
        assert!(b
            .take_status_events()
            .iter()
            .any(|e| matches!(e, StatusEvent::SyrupBombEnded { target: 1 })));
    }

    #[test]
    fn thawing_moves_clear_frostbite_and_matcha_gotcha_heals() {
        // Hydro Steam thaws a frostbitten target.
        let mut b = duel(
            ("pikachu", &["hydro-steam"], 60),
            ("snorlax", &["tackle"], 80),
        );
        b.combatants[1].status = Some(MajorStatus::Frostbite);
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        assert_eq!(b.combatants[1].status, None);
        assert!(b
            .take_status_events()
            .iter()
            .any(|e| matches!(e, StatusEvent::Thawed { target: 1 })));

        // Matcha Gotcha heals the user for half the damage dealt.
        let mut b = duel(
            ("pikachu", &["matcha-gotcha"], 60),
            ("snorlax", &["tackle"], 80),
        );
        b.combatants[0].hp = b.combatants[0].max_hp - 60;
        let before = b.combatants[0].hp;
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0, 99]));
        assert!(b.combatants[0].hp > before, "drain must heal the user");
    }

    #[test]
    fn population_bomb_rolls_accuracy_for_every_hit() {
        let hit_count = |b: &mut Battle| {
            b.take_effect_events().into_iter().find_map(|e| match e {
                EffectEvent::Hits { count, .. } => Some(count),
                _ => None,
            })
        };
        // Every one of the ten hits lands (accuracy, critical and damage roll each time).
        let mut b = duel(
            ("pikachu", &["population-bomb"], 20),
            ("snorlax", &["tackle"], 100),
        );
        let mut rolls = vec![0, 1, 0];
        for _ in 1..10 {
            rolls.extend([0, 1, 0]);
        }
        b.play_turn(&[act(0)], &mut Fixed(rolls));
        assert_eq!(hit_count(&mut b), Some(10));

        // A miss on the second hit stops the move.
        let mut b = duel(
            ("pikachu", &["population-bomb"], 20),
            ("snorlax", &["tackle"], 100),
        );
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0, 99]));
        assert_eq!(hit_count(&mut b), Some(1));
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

    /// Flower Trick (870): the hit is always critical (`willCrit`), mapped to the engine's
    /// guaranteed critical rank. EXTERNAL/PROBABLE.
    #[test]
    fn flower_trick_always_crits() {
        let moves = all();
        assert_eq!(find(&moves, "flower-trick").effects.crit_rank, 6);
        let mut b = duel(
            ("pikachu", &["flower-trick"], 40),
            ("snorlax", &["tackle"], 80),
        );
        // accuracy 101 never rolls; the crit rank 6 consumes no roll either (only the 85-100 %
        // damage roll is drawn).
        let ev = b.play_turn(&[act(0)], &mut Fixed(vec![0]));
        assert!(ev.iter().any(|e| matches!(
            e,
            Event::UsedMove {
                name,
                critical: true,
                ..
            } if name == "flower-trick"
        )));
    }

    /// Ice Spinner (861) and Psyblade (875): Hisui has no terrain, so only the plain damage
    /// runs (the Showdown terrain callbacks are omitted, noted in the sheet).
    #[test]
    fn ice_spinner_and_psyblade_are_plain_damage() {
        let moves = all();
        for name in ["ice-spinner", "psyblade"] {
            let fx = &find(&moves, name).effects;
            assert_eq!(fx.hooks, crate::battle_engine::effects::MoveHooks::default());
        }
        for name in ["ice-spinner", "psyblade"] {
            let mut b = duel(("pikachu", &[name], 40), ("snorlax", &["tackle"], 80));
            b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
            assert!(b.combatants[1].hp < b.combatants[1].max_hp, "{name} must hit");
        }
    }

    /// Mortal Spin (866): damage + 100 % poison + frees the user from a partial trap.
    #[test]
    fn mortal_spin_poisons_and_frees_the_user() {
        let mut b = duel(
            ("bidoof", &["mortal-spin"], 30),
            ("pikachu", &["bind"], 30),
        );
        b.set_status_rules(StatusRules {
            volatile_statuses: true,
            ..StatusRules::default()
        });
        b.combatants[0].max_hp = 500;
        b.combatants[0].hp = 500;
        // Pikachu binds Bidoof first: accuracy roll, crit, damage roll, then the 5..=6 duration.
        b.play_turn(&[act(1)], &mut Fixed(vec![0, 0, 0, 0]));
        assert!(b.is_trapped(0));

        // Mortal Spin: accuracy roll, crit, damage roll; the poison has a 100 % chance.
        let ev = b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        assert!(!b.is_trapped(0), "the user must be freed");
        assert_eq!(b.combatants[1].status, Some(MajorStatus::Poison));
        assert!(ev.iter().any(|e| matches!(
            e,
            Event::StatusApplied {
                target: 1,
                status: MajorStatus::Poison
            }
        )));
        assert!(b
            .take_status_events()
            .iter()
            .any(|e| matches!(e, StatusEvent::TrapEnded { target: 0 })));
    }

    /// Tidy Up (882): raises the user's Attack and Speed one state each (the hazards and
    /// Substitute part of the move is not modelled: no such concepts in the engine).
    #[test]
    fn tidy_up_raises_attack_and_speed() {
        let mut b = duel(("pikachu", &["tidy-up"], 30), ("snorlax", &["tackle"], 80));
        b.play_turn(&[act(0)], &mut Fixed(vec![]));
        assert_eq!(b.stages(0).attack, 1);
        assert_eq!(b.stages(0).speed, 1);
        assert_eq!(b.combatants[0].hp, b.combatants[0].max_hp);
    }

    /// Fillet Away (868): half the user's max HP, then Atk/SpA and Spe up. PLA has three rank
    /// states and Atk/SpA share one, so the Generation-9 +2 lands at +1 per group (documented
    /// consequence). At or below half HP the move fails and costs nothing.
    #[test]
    fn fillet_away_costs_half_and_collapses_the_boosts() {
        let mut b = duel(
            ("pikachu", &["fillet-away"], 30),
            ("snorlax", &["tackle"], 80),
        );
        let max = b.combatants[0].max_hp;
        b.play_turn(&[act(0)], &mut Fixed(vec![]));
        assert_eq!(b.combatants[0].hp, max - max / 2);
        assert_eq!(b.stages(0).attack, 1, "Atk and SpA share the first group");
        assert_eq!(b.stages(0).speed, 1);
        assert!(b.take_effect_events().iter().any(|e| matches!(
            e,
            EffectEvent::StageAlreadyAtLimit {
                target: 0,
                stat: Stat::SpecialAttack
            }
        )));

        // At exactly half HP the move is refused: no cost, no new boosts, a failed log line.
        b.combatants[0].hp = max / 2;
        let hp = b.combatants[0].hp;
        b.play_turn(&[act(0)], &mut Fixed(vec![]));
        assert_eq!(b.combatants[0].hp, hp);
        assert!(b.take_status_events().iter().any(|e| matches!(
            e,
            StatusEvent::MoveFailed { actor: 0, move_name } if move_name == "fillet-away"
        )));
    }

    /// Ruination (877): fixed damage, floor(target current HP / 2), minimum 1, typeless and
    /// with no damage roll.
    #[test]
    fn ruination_cuts_the_target_hp_in_half() {
        let mut b = duel(("pikachu", &["ruination"], 30), ("snorlax", &["tackle"], 80));
        b.combatants[1].hp = 201;
        // Only the 90 % accuracy roll is drawn; the damage is fixed.
        let ev = b.play_turn(&[act(0)], &mut Fixed(vec![0]));
        assert_eq!(b.combatants[1].hp, 201 - 100);
        assert!(ev.iter().any(|e| matches!(
            e,
            Event::UsedMove {
                name,
                damage: 100,
                critical: false,
                ..
            } if name == "ruination"
        )));
    }

    /// Comeuppance (894): returns 3/2 of the last hit the user took this turn (rounds down);
    /// refused when the user took no damage this turn.
    #[test]
    fn comeuppance_returns_the_last_hit_of_the_turn() {
        let mut b = duel(
            ("bidoof", &["comeuppance"], 20),
            ("pikachu", &["tackle"], 60),
        );
        assert!(b.combatants[0].speed < b.combatants[1].speed);
        let taken = {
            // Pikachu's tackle first (accuracy, crit, roll), then the answer.
            b.play_turn(&[act(0), act(1)], &mut Fixed(vec![0, 1, 0, 0]));
            b.combatants[0].max_hp - b.combatants[0].hp
        };
        assert!(taken > 0, "the tackle must land");
        assert_eq!(
            b.combatants[1].max_hp - b.combatants[1].hp,
            (taken * 3 / 2).max(1),
            "the hit comes back at 3/2"
        );

        // Next turn the user has taken no damage yet: the move is refused and deals nothing.
        let hp = b.combatants[1].hp;
        b.play_turn(&[act(0)], &mut Fixed(vec![]));
        assert_eq!(b.combatants[1].hp, hp);
        assert!(b.take_status_events().iter().any(|e| matches!(
            e,
            StatusEvent::MoveFailed { actor: 0, move_name } if move_name == "comeuppance"
        )));
    }

    /// Alluring Voice (914): confuses a target that raised a stat this turn (`secondary.onHit`).
    #[test]
    fn alluring_voice_confuses_only_a_booster() {
        // Pikachu (faster) dances first; the slower Bidoof answers with the voice.
        let mut b = duel(
            ("pikachu", &["swords-dance"], 50),
            ("bidoof", &["alluring-voice"], 30),
        );
        b.set_status_rules(StatusRules {
            volatile_statuses: true,
            ..StatusRules::default()
        });
        assert!(b.combatants[0].speed > b.combatants[1].speed);
        // Swords Dance (accuracy 101 never rolls) raises Attack; the voice hits (accuracy,
        // crit, roll, then the confusion duration).
        b.play_turn(&[act(0), act(1)], &mut Fixed(vec![0, 1, 0, 0]));
        assert_eq!(b.stages(0).attack, 1);
        assert!(b.is_confused(0), "a booster must be confused");

        // Against a target that did not raise anything this turn, nothing happens.
        let mut b = duel(
            ("snorlax", &["tackle"], 80),
            ("pikachu", &["alluring-voice"], 50),
        );
        b.set_status_rules(StatusRules {
            volatile_statuses: true,
            ..StatusRules::default()
        });
        b.play_turn(&[act(0), act(1)], &mut Fixed(vec![0, 1, 0, 0, 1, 0]));
        assert!(!b.is_confused(0));
    }

    /// Double Shock (892): refused unless the user has the move's own type; the hit removes it.
    #[test]
    fn double_shock_requires_and_consumes_the_electric_type() {
        let mut b = duel(
            ("pikachu", &["double-shock"], 50),
            ("snorlax", &["tackle"], 80),
        );
        assert!(b.combatants[0].species.types.iter().any(|t| t == "Electric"));
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        assert!(
            !b.combatants[0].species.types.iter().any(|t| t == "Electric"),
            "the type is gone after the hit"
        );
        assert!(b.take_status_events().iter().any(|e| matches!(
            e,
            StatusEvent::LostType { actor: 0, type_name } if type_name == "Electric"
        )));

        // A non-Electric user cannot even try: no damage, a failed log line.
        let mut b = duel(
            ("bidoof", &["double-shock"], 50),
            ("snorlax", &["tackle"], 80),
        );
        let hp = b.combatants[1].hp;
        b.play_turn(&[act(0)], &mut Fixed(vec![]));
        assert_eq!(b.combatants[1].hp, hp);
        assert!(b.take_status_events().iter().any(|e| matches!(
            e,
            StatusEvent::MoveFailed { actor: 0, move_name } if move_name == "double-shock"
        )));
    }

    /// Upper Hand (918): +3 priority + 100 % flinch, but only against a target whose chosen
    /// action uses a priority move.
    #[test]
    fn upper_hand_needs_a_priority_target() {
        // Pikachu (faster) chose Quick Attack (+1); Upper Hand is +3, so it acts first, lands
        // and the flinch eats the target's action.
        let mut b = duel(
            ("bidoof", &["upper-hand"], 30),
            ("pikachu", &["quick-attack"], 60),
        );
        let ev = b.play_turn(&[act(0), act(1)], &mut Fixed(vec![0, 1, 0]));
        assert_eq!(ev[1], Event::ActedFirst(0));
        assert!(b.combatants[1].hp < b.combatants[1].max_hp);
        assert_eq!(b.combatants[0].hp, b.combatants[0].max_hp, "quick attack was flinched");
        assert!(b
            .take_effect_events()
            .contains(&EffectEvent::FlinchedAndSkipped { actor: 1 }));

        // Against a plain Tackle (priority 0) the move is refused and the tackle lands.
        let mut b = duel(
            ("bidoof", &["upper-hand"], 30),
            ("pikachu", &["tackle"], 60),
        );
        b.play_turn(&[act(0), act(1)], &mut Fixed(vec![0, 1, 0]));
        assert!(b.combatants[0].hp < b.combatants[0].max_hp);
        assert!(b.take_status_events().iter().any(|e| matches!(
            e,
            StatusEvent::MoveFailed { actor: 0, move_name } if move_name == "upper-hand"
        )));
    }

    /// Thunderclap (909): +1 priority, but only against a target whose chosen action is an
    /// attack (category different from Status).
    #[test]
    fn thunderclap_needs_an_attack_target() {
        let mut b = duel(
            ("bidoof", &["thunderclap"], 30),
            ("pikachu", &["tackle"], 60),
        );
        let ev = b.play_turn(&[act(0), act(1)], &mut Fixed(vec![0, 1, 0]));
        assert_eq!(ev[1], Event::ActedFirst(0), "+1 priority goes first");
        assert!(b.combatants[1].hp < b.combatants[1].max_hp);

        // A status move (Growl) makes it fail.
        let mut b = duel(
            ("bidoof", &["thunderclap"], 30),
            ("pikachu", &["growl"], 60),
        );
        let hp = b.combatants[1].hp;
        b.play_turn(&[act(0), act(1)], &mut Fixed(vec![0]));
        assert_eq!(b.combatants[1].hp, hp);
        assert!(b.take_status_events().iter().any(|e| matches!(
            e,
            StatusEvent::MoveFailed { actor: 0, move_name } if move_name == "thunderclap"
        )));
    }

    /// Electro Shot (905): charges a turn raising Sp. Atk one rank, attacks the next; rain
    /// skips the charge and its boost.
    #[test]
    fn electro_shot_charges_boosts_and_skips_in_rain() {
        let rules = StatusRules {
            volatile_statuses: true,
            ..StatusRules::default()
        };
        let mut b = duel(
            ("pikachu", &["electro-shot"], 50),
            ("snorlax", &["tackle"], 80),
        );
        b.set_status_rules(rules);
        b.play_turn(&[act(0)], &mut Fixed(vec![]));
        assert!(b.is_charging(0));
        assert_eq!(b.stages(0).attack, 1, "Sp. Atk shares the first rank group");
        assert_eq!(b.combatants[1].hp, b.combatants[1].max_hp, "no damage on the charge turn");
        let pp = b.combatants[0].moves[0].pp;
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        assert!(!b.is_charging(0));
        assert!(b.combatants[1].hp < b.combatants[1].max_hp);
        assert_eq!(b.combatants[0].moves[0].pp, pp, "PP is only spent once");

        // Rain: one turn, no charge state and no boost.
        let mut b = duel(
            ("pikachu", &["electro-shot"], 50),
            ("snorlax", &["tackle"], 80),
        );
        b.set_status_rules(rules);
        b.weather = BattleWeather::Rain;
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0]));
        assert!(!b.is_charging(0));
        assert_eq!(b.stages(0).attack, 0);
        assert!(b.combatants[1].hp < b.combatants[1].max_hp);
    }

    /// Malignant Chain (919): Poison special whose secondary badly poisons 50 % of the time
    /// (the engine's state for the game's `0xf01` poison mark).
    #[test]
    fn malignant_chain_badly_poisons_half_the_time() {
        // accuracy, crit, damage roll, then the 50 % ailment roll.
        let mut b = duel(
            ("pikachu", &["malignant-chain"], 50),
            ("snorlax", &["tackle"], 80),
        );
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0, 49]));
        assert_eq!(b.combatants[1].status, Some(MajorStatus::BadlyPoisoned));

        let mut b = duel(
            ("pikachu", &["malignant-chain"], 50),
            ("snorlax", &["tackle"], 80),
        );
        b.play_turn(&[act(0)], &mut Fixed(vec![0, 1, 0, 50]));
        assert_eq!(b.combatants[1].status, None);
    }
}
