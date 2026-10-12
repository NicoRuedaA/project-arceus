//! Pure, evidence-bounded battle mechanics and data-table access.
//!
//! This module deliberately does not depend on Bevy. Incomplete source evidence
//! is represented as an unsupported outcome or an explicit unknown parameter;
//! it is never replaced with a guessed formula.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Table {
    headers: Vec<String>,
    rows: Vec<BTreeMap<String, String>>,
}

impl Table {
    fn load(path: &Path, key_column: Option<&str>) -> Result<Self, String> {
        let text =
            fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let mut lines = text
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'));
        let headers: Vec<String> = lines
            .next()
            .ok_or_else(|| format!("{}: missing header", path.display()))?
            .split('\t')
            .map(str::to_owned)
            .collect();
        let mut rows: Vec<BTreeMap<String, String>> = Vec::new();
        for (row_number, line) in lines.enumerate() {
            let values: Vec<&str> = line.split('\t').collect();
            if values.len() != headers.len() {
                return Err(format!(
                    "{}:{}: expected {} columns, found {}",
                    path.display(),
                    row_number + 2,
                    headers.len(),
                    values.len()
                ));
            }
            rows.push(
                headers
                    .iter()
                    .cloned()
                    .zip(values.into_iter().map(str::to_owned))
                    .collect(),
            );
        }
        if let Some(key) = key_column {
            let mut keys = std::collections::BTreeSet::new();
            for row in &rows {
                let value = row
                    .get(key)
                    .ok_or_else(|| format!("{}: missing {key}", path.display()))?;
                if !keys.insert(value) {
                    return Err(format!("{}: duplicate {key}={value}", path.display()));
                }
            }
        }
        Ok(Self { headers, rows })
    }

    fn row(&self, column: &str, value: &str) -> Option<&BTreeMap<String, String>> {
        self.rows
            .iter()
            .find(|row| row.get(column).is_some_and(|v| v == value))
    }

    fn value<'a>(row: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
        row.get(key)
            .map(String::as_str)
            .ok_or_else(|| format!("missing field {key}"))
    }
}

/// Data loaded from the checked-in `data/ai` and `data/capture` TSV tables.
#[derive(Debug, Clone)]
pub struct BattleData {
    score_rules: Table,
    expert_sequences: Table,
    capture_bait: Table,
    capture_balls: Table,
    capture_constants: Table,
    capture_context: Table,
    capture_level_ranks: Table,
    capture_status: Table,
    throwable_parameters: Table,
}

impl BattleData {
    /// Load all battle tables below a repository/data root (for example `data`).
    pub fn load(data_root: impl AsRef<Path>) -> Result<Self, String> {
        let root = data_root.as_ref();
        let ai = root.join("ai");
        let capture = root.join("capture");
        Ok(Self {
            score_rules: Table::load(&ai.join("score_rules.tsv"), Some("rule_id:string*"))?,
            expert_sequences: Table::load(
                &ai.join("expert_sequences.tsv"),
                Some("handler_id:string*"),
            )?,
            capture_bait: Table::load(&capture.join("bait_parameters.tsv"), None)?,
            capture_balls: Table::load(
                &capture.join("capture_ball_multipliers.tsv"),
                Some("ball_category:string*"),
            )?,
            capture_constants: Table::load(
                &capture.join("capture_constants.tsv"),
                Some("constant_key:string*"),
            )?,
            capture_context: Table::load(
                &capture.join("capture_context_modifiers.tsv"),
                Some("factor_key:string*"),
            )?,
            capture_level_ranks: Table::load(
                &capture.join("capture_level_rank_matrix.tsv"),
                Some("player_galaxy_rank:u8*"),
            )?,
            capture_status: Table::load(
                &capture.join("capture_status_multipliers.tsv"),
                Some("status_condition:string*"),
            )?,
            throwable_parameters: Table::load(
                &capture.join("throwable_parameters.tsv"),
                Some("f00_raw_u32:u32*"),
            )?,
        })
    }

    pub fn score_rule_count(&self) -> usize {
        self.score_rules.rows.len()
    }
    pub fn expert_sequence_count(&self) -> usize {
        self.expert_sequences.rows.len()
    }
    pub fn bait_parameter_count(&self) -> usize {
        self.capture_bait.rows.len()
    }
    pub fn throwable_parameter_count(&self) -> usize {
        self.throwable_parameters.rows.len()
    }

    /// Score a selected subset of source rules. Partial, unknown, composite,
    /// and otherwise unmapped rules fail closed instead of being guessed.
    pub fn score_rules(&self, rule_ids: &[&str], facts: &AiFacts) -> Result<i32, String> {
        let mut total = 0_i32;
        for id in rule_ids {
            match self.evaluate_rule(id, facts)? {
                RuleOutcome::Applied(delta) => {
                    total = total.checked_add(delta).ok_or("AI score overflow")?;
                }
                RuleOutcome::NotTriggered => {}
                RuleOutcome::Unsupported(reason) => return Err(format!("{id}: {reason}")),
            }
        }
        Ok(total)
    }

    pub fn evaluate_rule(&self, id: &str, facts: &AiFacts) -> Result<RuleOutcome, String> {
        let row = self
            .score_rules
            .row("rule_id:string*", id)
            .ok_or_else(|| format!("unknown AI rule {id}"))?;
        let status = Table::value(row, "evidence_status:enum")?;
        if !matches!(status, "observed_lua" | "cross_report") {
            return Ok(RuleOutcome::Unsupported(format!(
                "evidence status is {status}"
            )));
        }
        let delta = || -> Result<i32, String> {
            Table::value(row, "score_delta:i32?")?
                .parse()
                .map_err(|_| format!("{id}: score delta not recovered"))
        };
        let chance = || -> Result<u8, String> {
            Table::value(row, "chance_pct:u8?")?
                .parse()
                .map_err(|_| format!("{id}: chance not recovered"))
        };
        let gated = |chance: u8| -> Result<RuleOutcome, String> {
            match facts.probability_gate_passed {
                Some(true) => Ok(RuleOutcome::Applied(delta()?)),
                Some(false) => Ok(RuleOutcome::NotTriggered),
                None => Ok(RuleOutcome::Unsupported(format!(
                    "requires the observed {chance}% random gate outcome"
                ))),
            }
        };
        let active = facts.damaging_move;
        let outcome = match id {
            "basic_affinity_ineffective"
                if active && facts.affinity == Some(0) && !facts.basic_affinity_exception =>
            {
                RuleOutcome::Applied(delta()?)
            }
            "basic_affinity_ineffective" => RuleOutcome::NotTriggered,
            "strong_friendly_fire" | "expert_friendly_fire"
                if active && facts.double_battle && facts.friendly_target =>
            {
                RuleOutcome::Applied(delta()?)
            }
            "strong_friendly_fire" | "expert_friendly_fire" => RuleOutcome::NotTriggered,
            "strong_non_max_damage" if active && !facts.candidate_is_max_damage => {
                RuleOutcome::Applied(delta()?)
            }
            "strong_non_max_damage" => RuleOutcome::NotTriggered,
            "strong_high_affinity_bonus"
                if active && facts.affinity.is_some_and(|affinity| affinity >= 9) =>
            {
                gated(chance()?)?
            }
            "strong_high_affinity_bonus" => RuleOutcome::NotTriggered,
            "allowance_cat_tease" if facts.can_neko_damasi => RuleOutcome::Applied(delta()?),
            "allowance_cat_tease" => RuleOutcome::NotTriggered,
            "allowance_hp_under_20"
                if active && facts.target_hp_percent.is_some_and(|hp| hp < 20) =>
            {
                gated(chance()?)?
            }
            "allowance_hp_under_20" => RuleOutcome::NotTriggered,
            "allowance_hp_under_40"
                if active
                    && facts
                        .target_hp_percent
                        .is_some_and(|hp| (20..40).contains(&hp)) =>
            {
                gated(chance()?)?
            }
            "allowance_hp_under_40" => RuleOutcome::NotTriggered,
            _ => RuleOutcome::Unsupported(
                "no complete predicate mapping is implemented for this observed rule".into(),
            ),
        };
        Ok(outcome)
    }

    /// Return the recovered rank adjustment for a player's galaxy rank and a
    /// Pokémon rank (1–10). This is a table lookup, not a capture probability.
    pub fn capture_level_rank_adjustment(
        &self,
        player_rank: u8,
        pokemon_rank: u8,
    ) -> Result<i32, String> {
        let row = self
            .capture_level_ranks
            .row("player_galaxy_rank:u8*", &player_rank.to_string())
            .ok_or_else(|| format!("unknown player galaxy rank {player_rank}"))?;
        if !(1..=10).contains(&pokemon_rank) {
            return Err(format!("pokemon rank {pokemon_rank} outside 1..=10"));
        }
        Table::value(
            row,
            &format!(
                "pokemon_rank_{pokemon_rank}:{}",
                if pokemon_rank == 1 { "u8*" } else { "i32" }
            ),
        )?
        .parse()
        .map_err(|_| "invalid capture level/rank cell".into())
    }

    pub fn capture_constant(&self, key: &str) -> Result<Option<f64>, String> {
        let Some(row) = self.capture_constants.row("constant_key:string*", key) else {
            return Ok(None);
        };
        Ok(Some(
            Table::value(row, "value:f64")?
                .parse()
                .map_err(|_| format!("invalid capture constant {key}"))?,
        ))
    }

    /// Resolve a recovered table multiplier. These factors are returned as
    /// source parameters; the report does not prove their final-rate ordering.
    pub fn capture_factor(&self, key: &str) -> Result<Option<f64>, String> {
        if let Some(row) = self.capture_context.row("factor_key:string*", key) {
            return Ok(Some(
                Table::value(row, "multiplier:f64")?
                    .parse()
                    .map_err(|_| format!("invalid multiplier {key}"))?,
            ));
        }
        if let Some(row) = self.capture_balls.row("multiplier_key:string", key) {
            return Ok(Some(
                Table::value(row, "base_multiplier:f64")?
                    .parse()
                    .map_err(|_| format!("invalid multiplier {key}"))?,
            ));
        }
        if let Some(row) = self.capture_status.rows.iter().find(|row| {
            row.get("multiplier_key:string")
                .is_some_and(|value| value == key)
        }) {
            return Ok(Some(
                Table::value(row, "multiplier:u8")?
                    .parse::<u8>()
                    .map_err(|_| format!("invalid multiplier {key}"))? as f64,
            ));
        }
        Ok(None)
    }

    pub fn capture_ball_multiplier(
        &self,
        category: &str,
        suitable_range: bool,
    ) -> Result<f64, String> {
        let row = self
            .capture_balls
            .row("ball_category:string*", category)
            .ok_or_else(|| format!("unknown capture ball category {category}"))?;
        let field = if suitable_range {
            "suitable_multiplier:f64"
        } else {
            "base_multiplier:f64"
        };
        Table::value(row, field)?
            .parse()
            .map_err(|_| format!("invalid multiplier for {category}"))
    }

    pub fn capture_status_multiplier(&self, status: &str) -> Result<f64, String> {
        let row = self
            .capture_status
            .row("status_condition:string*", status)
            .ok_or_else(|| format!("unknown capture status {status}"))?;
        Table::value(row, "multiplier:u8")?
            .parse::<u8>()
            .map(|v| v as f64)
            .map_err(|_| format!("invalid status multiplier for {status}"))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AiFacts {
    pub damaging_move: bool,
    pub double_battle: bool,
    pub friendly_target: bool,
    pub candidate_is_max_damage: bool,
    pub affinity: Option<i32>,
    pub basic_affinity_exception: bool,
    pub target_hp_percent: Option<u8>,
    pub can_neko_damasi: bool,
    /// The caller supplies the outcome of the game's percentage gate; this
    /// module does not claim to reproduce the game's PRNG.
    pub probability_gate_passed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleOutcome {
    Applied(i32),
    NotTriggered,
    Unsupported(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureUnknownParameter {
    HpCurveShape,
    ShakeThresholds,
    ShakeCountRelationship,
    FinalRateComposition,
    ThrowableNumericFieldMeaning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageUnknownModifier {
    RandomRoll,
    AbilityAndItemModifiers,
    CriticalHit,
    SameTypeAndTypeEffectiveness,
    FinalRoundingOrder,
}

pub const CAPTURE_UNKNOWN_PARAMETERS: &[CaptureUnknownParameter] = &[
    CaptureUnknownParameter::HpCurveShape,
    CaptureUnknownParameter::ShakeThresholds,
    CaptureUnknownParameter::ShakeCountRelationship,
    CaptureUnknownParameter::FinalRateComposition,
    CaptureUnknownParameter::ThrowableNumericFieldMeaning,
];

pub const DAMAGE_UNKNOWN_MODIFIERS: &[DamageUnknownModifier] = &[
    DamageUnknownModifier::RandomRoll,
    DamageUnknownModifier::AbilityAndItemModifiers,
    DamageUnknownModifier::CriticalHit,
    DamageUnknownModifier::SameTypeAndTypeEffectiveness,
    DamageUnknownModifier::FinalRoundingOrder,
];

/// Inputs to the recovered integer base-damage expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaseDamageInput {
    pub power: u64,
    pub level: u64,
    pub attack: u64,
    pub defense: u64,
}

/// Compute the reported integer damage core using widened integer arithmetic.
pub fn integer_base_damage(input: BaseDamageInput) -> Result<u64, String> {
    if input.defense == 0 {
        return Err("defense must be nonzero".into());
    }
    let level_term = input.level.checked_mul(2).ok_or("level overflow")? / 5 + 2;
    let numerator = u128::from(input.power)
        .checked_mul(u128::from(input.attack))
        .and_then(|value| value.checked_mul(u128::from(level_term)))
        .ok_or("base damage intermediate overflow")?;
    let denominator = u128::from(input.defense) * 50;
    u64::try_from(numerator / denominator + 2).map_err(|_| "base damage overflow".into())
}

/// Apply a Q12 fixed-point scale with integer truncation: floor(value*scale/4096).
pub fn apply_q12(value: u64, scale_q12: u32) -> Result<u64, String> {
    let scaled = u128::from(value) * u128::from(scale_q12) / 4096;
    u64::try_from(scaled).map_err(|_| "Q12-scaled damage overflow".into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageCategoryMultiplier {
    Zero,
    TwoFifths,
    OneHalf,
    Double,
    TwoAndHalf,
}

/// Apply one of the recovered post-roll category multipliers. Mapping a native
/// selector value to these categories is intentionally left to a proven caller.
pub fn apply_damage_category(
    value: u64,
    multiplier: DamageCategoryMultiplier,
) -> Result<u64, String> {
    let (numerator, denominator) = match multiplier {
        DamageCategoryMultiplier::Zero => (0_u128, 1_u128),
        DamageCategoryMultiplier::TwoFifths => (2, 5),
        DamageCategoryMultiplier::OneHalf => (1, 2),
        DamageCategoryMultiplier::Double => (2, 1),
        DamageCategoryMultiplier::TwoAndHalf => (5, 2),
    };
    u64::try_from(u128::from(value) * numerator / denominator)
        .map_err(|_| "category-scaled damage overflow".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn data() -> BattleData {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        BattleData::load(root).unwrap()
    }

    #[test]
    fn loads_checked_in_battle_tables() {
        let data = data();
        assert_eq!(data.score_rule_count(), 30);
        assert_eq!(data.expert_sequence_count(), 43);
        assert!(data.bait_parameter_count() > 0);
        assert!(data.throwable_parameter_count() > 0);
    }

    #[test]
    fn scores_only_fully_mapped_lua_rules() {
        let data = data();
        let facts = AiFacts {
            damaging_move: true,
            double_battle: true,
            friendly_target: true,
            ..Default::default()
        };
        assert_eq!(
            data.score_rules(&["strong_friendly_fire"], &facts).unwrap(),
            -100
        );
        assert!(data
            .score_rules(&["expert_sequence_dispatch"], &facts)
            .unwrap_err()
            .contains("evidence status is unknown"));
        let partial = data
            .evaluate_rule(
                "strong_high_affinity_bonus",
                &AiFacts {
                    damaging_move: true,
                    affinity: Some(9),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(matches!(partial, RuleOutcome::Unsupported(_)));
    }

    #[test]
    fn capture_queries_expose_tables_without_claiming_a_final_rate() {
        let data = data();
        assert_eq!(
            data.capture_ball_multiplier("Regular", false).unwrap(),
            0.75
        );
        assert_eq!(data.capture_ball_multiplier("Feather", true).unwrap(), 1.25);
        assert_eq!(data.capture_status_multiplier("Paralysis").unwrap(), 2.0);
        assert_eq!(data.capture_level_rank_adjustment(0, 3).unwrap(), -2);
        assert_eq!(
            data.capture_constant("capture_index_hp_coef").unwrap(),
            Some(0.66)
        );
        assert_eq!(
            data.capture_factor("capture_index_backstrike_success_factor")
                .unwrap(),
            Some(1.75)
        );
    }

    #[test]
    fn damage_arithmetic_uses_the_reported_integer_steps() {
        let core = integer_base_damage(BaseDamageInput {
            power: 50,
            level: 50,
            attack: 100,
            defense: 50,
        })
        .unwrap();
        assert_eq!(core, 46); // floor(50*100*(floor(100/5)+2)/(50*50))+2
        assert_eq!(apply_q12(core, 2048).unwrap(), 23);
        assert_eq!(
            apply_damage_category(core, DamageCategoryMultiplier::TwoFifths).unwrap(),
            18
        );
        assert_eq!(
            apply_damage_category(core, DamageCategoryMultiplier::TwoAndHalf).unwrap(),
            115
        );
    }
}
