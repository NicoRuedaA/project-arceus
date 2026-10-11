//! Typed readers and evidence-bounded PML mechanics for the PLA data tables.
//!
//! This module preserves table values as decoded and does not infer behavior
//! from descriptive labels or incomplete native evidence. In particular,
//! non-HP stat intermediates and the nature matrix are not reconstructed.

use std::{fs, path::Path};

pub const EXPECTED_PERSONAL_ROWS: usize = 1_276;
pub const EXPECTED_GROWTH_ROWS: usize = 600;
pub const EXPECTED_EVOLUTION_ROWS: usize = 146;
pub const EXPECTED_MOVE_ROWS: usize = 851;
pub const EXPECTED_LEARNSET_ROWS: usize = 26_160;
pub const GANBA_K: [u8; 11] = [0, 2, 3, 4, 7, 8, 9, 14, 15, 16, 25];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenderRatio {
    AllMale,
    AllFemale,
    Genderless,
    Equal,
    /// Preserve unclassified native encodings instead of assigning semantics.
    Unknown(u8),
}

impl GenderRatio {
    fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::AllMale,
            127 => Self::Equal,
            254 => Self::AllFemale,
            255 => Self::Genderless,
            value => Self::Unknown(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonalEntry {
    pub national_dex: u16,
    pub species_name: String,
    pub form: u8,
    pub is_active: bool,
    /// HP, Attack, Defense, Special Attack, Special Defense, Speed.
    pub base_stats: [u8; 6],
    pub base_stat_total: u16,
    pub type1: u8,
    pub type1_name: String,
    pub type2: u8,
    pub type2_name: String,
    pub gender_ratio: GenderRatio,
    pub base_exp_yield: u16,
    pub growth_group: u8,
    pub growth_group_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrowthThreshold {
    pub group: u8,
    pub group_name: String,
    pub level: u8,
    pub cumulative_exp: u32,
    pub delta_to_next: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvolutionEntry {
    pub species: u16,
    pub species_name: String,
    pub form: u8,
    pub evolution_index: u8,
    /// Native method number. Methods 48–54 are recognized PLA trigger kinds.
    pub method: u8,
    pub trigger_name: String,
    pub argument: u16,
    pub condition_detail: Option<String>,
    pub target_species: u16,
    pub target_species_name: String,
    pub target_form: u8,
    pub level: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaEvolutionMethod {
    ItemDay,
    ItemNight,
    ItemMoonPhase,
    AgileMoveUses,
    StrongMoveUses,
    RecoilDamageMale,
    RecoilDamageFemale,
}

impl EvolutionEntry {
    pub fn pla_method(&self) -> Option<PlaEvolutionMethod> {
        Some(match self.method {
            48 => PlaEvolutionMethod::ItemDay,
            49 => PlaEvolutionMethod::ItemNight,
            50 => PlaEvolutionMethod::ItemMoonPhase,
            51 => PlaEvolutionMethod::AgileMoveUses,
            52 => PlaEvolutionMethod::StrongMoveUses,
            53 => PlaEvolutionMethod::RecoilDamageMale,
            54 => PlaEvolutionMethod::RecoilDamageFemale,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveEntry {
    pub source_file: String,
    pub move_id: u16,
    pub move_name: String,
    pub move_type: u8,
    pub move_type_name: String,
    pub category: u8,
    pub category_name: String,
    pub power: u8,
    pub accuracy: u8,
    pub pp: u8,
    pub agile_power: u8,
    pub strong_power: u8,
    pub strong_accuracy: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearnsetEntry {
    pub species: u16,
    pub species_name: String,
    pub form: u8,
    pub learn_method: String,
    pub level: u8,
    pub mastery_level: u8,
    pub move_id: u16,
    pub move_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmlDatabase {
    pub personal: Vec<PersonalEntry>,
    pub growth: Vec<GrowthThreshold>,
    pub evolutions: Vec<EvolutionEntry>,
    pub moves: Vec<MoveEntry>,
    pub learnsets: Vec<LearnsetEntry>,
}

impl PmlDatabase {
    pub fn load_from_dir(directory: impl AsRef<Path>) -> Result<Self, String> {
        let dir = directory.as_ref();
        let personal = read_table(dir.join("pml_personal.tsv"), parse_personal)?;
        let growth = read_table(dir.join("pml_grow_table.tsv"), parse_growth)?;
        let evolutions = read_table(dir.join("pml_evolution.tsv"), parse_evolution)?;
        let moves = read_table(dir.join("pml_moves.tsv"), parse_move)?;
        let learnsets = read_table(dir.join("pml_learnsets.tsv"), parse_learnset)?;
        Ok(Self {
            personal,
            growth,
            evolutions,
            moves,
            learnsets,
        })
    }

    /// Cumulative experience threshold for levels 1–100. As in the native
    /// lookup, levels above 100 clamp to 100; level zero has no evidenced row.
    pub fn experience_at_level(&self, group: u8, level: u8) -> Option<u32> {
        if level == 0 {
            return None;
        }
        let level = level.min(100);
        self.growth
            .iter()
            .find(|row| row.group == group && row.level == level)
            .map(|row| row.cumulative_exp)
    }

    /// The native experience-to-level comparison boundary is not stated in
    /// the available evidence, so this module intentionally does not infer it.
    pub const fn level_for_experience_is_unknown(&self) -> bool {
        true
    }
}

/// The constructor-observed subset of PokemonParam, retaining raw signed IVs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PokemonParam {
    pub pid: u32,
    pub flags: u64,
    pub encryption_constant: u32,
    pub level: u16,
    /// HP, Attack, Defense, Speed, Special Attack, Special Defense; raw i16 slots.
    pub iv_slots: [i16; 6],
    pub guaranteed_perfect_ivs: u8,
}

impl PokemonParam {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 0x65 {
            return None;
        }
        let u16_at = |offset| {
            Some(u16::from_le_bytes(
                bytes.get(offset..offset + 2)?.try_into().ok()?,
            ))
        };
        let u32_at = |offset| {
            Some(u32::from_le_bytes(
                bytes.get(offset..offset + 4)?.try_into().ok()?,
            ))
        };
        let i16_at = |offset| {
            Some(i16::from_le_bytes(
                bytes.get(offset..offset + 2)?.try_into().ok()?,
            ))
        };
        Some(Self {
            pid: u32_at(0x10)?,
            flags: u64::from_le_bytes(bytes[0x18..0x20].try_into().ok()?),
            encryption_constant: u32_at(0x20)?,
            level: u16_at(0x30)?.min(100),
            iv_slots: [
                i16_at(0x3a)?,
                i16_at(0x3c)?,
                i16_at(0x3e)?,
                i16_at(0x40)?,
                i16_at(0x42)?,
                i16_at(0x44)?,
            ],
            guaranteed_perfect_ivs: bytes[0x64],
        })
    }
}

/// Computes the documented HP expression. `shedinja_predicate` is an input
/// because its native predicate is known but species/form resolution is not
/// implemented here. `None` means that predicate evidence was not supplied.
pub fn calculate_hp(
    base: u8,
    level: u16,
    ganba: u8,
    shedinja_predicate: Option<bool>,
) -> Option<u32> {
    let shedinja = shedinja_predicate?;
    if shedinja {
        return Some(1);
    }
    let k = *GANBA_K.get(usize::from(ganba))? as f64;
    let level = level.min(100) as f64;
    let base = f64::from(base);
    let ganba_term = ((base.sqrt() * k + level) / 2.5).floor();
    let level_term = (((level / 100.0) + 1.0) * base + level).floor();
    Some((ganba_term + level_term) as u32)
}

/// Computes the evidenced non-HP expression only when both unresolved inputs
/// are supplied from independent evidence; neither is reconstructed here.
pub fn calculate_non_hp_stat(
    base: u8,
    level: u16,
    ganba: u8,
    intermediate: Option<f64>,
    nature_modifier: Option<f64>,
) -> Option<u32> {
    let intermediate = intermediate?;
    let nature_modifier = nature_modifier?;
    if !intermediate.is_finite()
        || !nature_modifier.is_finite()
        || intermediate < 0.0
        || nature_modifier < 0.0
    {
        return None;
    }
    let k = *GANBA_K.get(usize::from(ganba))? as f64;
    let level = f64::from(level.min(100));
    let ganba_term = ((f64::from(base).sqrt() * k + level) / 2.5).floor();
    Some((intermediate * nature_modifier).floor().max(0.0) as u32 + ganba_term as u32)
}

fn read_table<T>(
    path: impl AsRef<Path>,
    parse: fn(&[(&str, &str)]) -> Result<T, String>,
) -> Result<Vec<T>, String> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut lines = text
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'));
    let header_line = lines
        .next()
        .ok_or_else(|| format!("{}: missing typed header", path.display()))?;
    let headers: Vec<String> = header_line
        .split('\t')
        .map(|field| {
            let field = field.trim_end_matches(['*', '?']);
            field
                .split_once(':')
                .map_or(field, |(name, _)| name)
                .to_owned()
        })
        .collect();
    if headers.is_empty() {
        return Err(format!("{}: empty header", path.display()));
    }
    let mut records = Vec::new();
    for (line_index, line) in lines.enumerate() {
        if line.starts_with("//") {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != headers.len() {
            return Err(format!(
                "{}:{}: expected {} fields, got {}",
                path.display(),
                line_index + 1,
                headers.len(),
                fields.len()
            ));
        }
        let pairs: Vec<(&str, &str)> = headers.iter().map(String::as_str).zip(fields).collect();
        records.push(
            parse(&pairs)
                .map_err(|error| format!("{}:{}: {error}", path.display(), line_index + 1))?,
        );
    }
    Ok(records)
}

fn field<'a>(row: &'a [(&str, &str)], name: &str) -> Result<&'a str, String> {
    row.iter()
        .find_map(|(key, value)| (*key == name).then_some(*value))
        .ok_or_else(|| format!("missing column {name}"))
}
fn number<T: std::str::FromStr>(row: &[(&str, &str)], name: &str) -> Result<T, String> {
    field(row, name)?
        .parse()
        .map_err(|_| format!("invalid {name}"))
}
fn string(row: &[(&str, &str)], name: &str) -> Result<String, String> {
    Ok(field(row, name)?.to_owned())
}
fn optional_string(row: &[(&str, &str)], name: &str) -> Result<Option<String>, String> {
    let value = field(row, name)?;
    Ok((!value.is_empty()).then(|| value.to_owned()))
}
fn parse_personal(r: &[(&str, &str)]) -> Result<PersonalEntry, String> {
    Ok(PersonalEntry {
        national_dex: number(r, "dex_index_national")?,
        species_name: string(r, "species_name")?,
        form: number(r, "form")?,
        is_active: match field(r, "is_active")? {
            "1" | "true" => true,
            "0" | "false" => false,
            _ => return Err("invalid is_active".into()),
        },
        base_stats: [
            number(r, "stat_hp")?,
            number(r, "stat_atk")?,
            number(r, "stat_def")?,
            number(r, "stat_spa")?,
            number(r, "stat_spd")?,
            number(r, "stat_spe")?,
        ],
        base_stat_total: number(r, "base_stat_total")?,
        type1: number(r, "type1")?,
        type1_name: string(r, "type_1")?,
        type2: number(r, "type2")?,
        type2_name: string(r, "type_2")?,
        gender_ratio: GenderRatio::from_raw(number(r, "gender")?),
        base_exp_yield: number(r, "base_exp")?,
        growth_group: number(r, "exp_growth")?,
        growth_group_name: string(r, "growth_group")?,
    })
}
fn parse_growth(r: &[(&str, &str)]) -> Result<GrowthThreshold, String> {
    Ok(GrowthThreshold {
        group: number(r, "exp_group")?,
        group_name: string(r, "exp_group_name")?,
        level: number(r, "level")?,
        cumulative_exp: number(r, "exp_threshold")?,
        delta_to_next: number(r, "exp_delta_to_next")?,
    })
}
fn parse_evolution(r: &[(&str, &str)]) -> Result<EvolutionEntry, String> {
    Ok(EvolutionEntry {
        species: number(r, "species")?,
        species_name: string(r, "species_name")?,
        form: number(r, "form")?,
        evolution_index: number(r, "evolution_index")?,
        method: number(r, "method")?,
        trigger_name: string(r, "trigger_name")?,
        argument: number(r, "argument")?,
        condition_detail: optional_string(r, "condition_detail")?,
        target_species: number(r, "target_species")?,
        target_species_name: string(r, "target_species_name")?,
        target_form: number(r, "target_form")?,
        level: number(r, "level")?,
    })
}
fn parse_move(r: &[(&str, &str)]) -> Result<MoveEntry, String> {
    Ok(MoveEntry {
        source_file: string(r, "source_file")?,
        move_id: number(r, "move")?,
        move_name: string(r, "move_name")?,
        move_type: number(r, "type")?,
        move_type_name: string(r, "type_name")?,
        category: number(r, "category")?,
        category_name: string(r, "category_name")?,
        power: number(r, "power")?,
        accuracy: number(r, "accuracy")?,
        pp: number(r, "pp")?,
        agile_power: number(r, "agile_power")?,
        strong_power: number(r, "strong_power")?,
        strong_accuracy: number(r, "strong_accuracy")?,
    })
}
fn parse_learnset(r: &[(&str, &str)]) -> Result<LearnsetEntry, String> {
    Ok(LearnsetEntry {
        species: number(r, "species")?,
        species_name: string(r, "species_name")?,
        form: number(r, "form")?,
        learn_method: string(r, "learn_method")?,
        level: number(r, "level")?,
        mastery_level: number(r, "level_master")?,
        move_id: number(r, "move")?,
        move_name: string(r, "move_name")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn loads_verified_tables_from_repository_data() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let db = PmlDatabase::load_from_dir(root).unwrap();
        assert_eq!(db.personal.len(), EXPECTED_PERSONAL_ROWS);
        assert_eq!(db.growth.len(), EXPECTED_GROWTH_ROWS);
        assert_eq!(db.evolutions.len(), EXPECTED_EVOLUTION_ROWS);
        assert_eq!(db.moves.len(), EXPECTED_MOVE_ROWS);
        assert_eq!(db.learnsets.len(), EXPECTED_LEARNSET_ROWS);
        let pikachu = db
            .personal
            .iter()
            .find(|row| row.national_dex == 25 && row.form == 0)
            .unwrap();
        assert_eq!(pikachu.base_stats, [35, 55, 40, 50, 50, 90]);
        assert_eq!(pikachu.gender_ratio, GenderRatio::Equal);
        let arceus = db
            .personal
            .iter()
            .find(|row| row.national_dex == 493 && row.form == 0)
            .unwrap();
        assert_eq!(arceus.gender_ratio, GenderRatio::Genderless);
        assert_eq!(arceus.base_stats, [120; 6]);
        assert_eq!(
            db.moves
                .iter()
                .find(|row| row.move_name == "ice-punch")
                .map(|row| (
                    row.power,
                    row.agile_power,
                    row.strong_power,
                    row.strong_accuracy
                )),
            Some((75, 60, 90, 100))
        );
    }

    #[test]
    fn growth_curves_are_complete_monotonic_and_known_caps_match() {
        let db =
            PmlDatabase::load_from_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
                .unwrap();
        let mut groups: BTreeMap<u8, Vec<&GrowthThreshold>> = BTreeMap::new();
        for row in &db.growth {
            groups.entry(row.group).or_default().push(row);
        }
        assert_eq!(groups.len(), 6);
        for (group, rows) in groups {
            assert_eq!(rows.len(), 100);
            assert_eq!(
                rows.iter().map(|row| row.level).collect::<Vec<_>>(),
                (1..=100).collect::<Vec<u8>>()
            );
            assert!(
                rows.windows(2)
                    .all(|pair| pair[0].cumulative_exp <= pair[1].cumulative_exp),
                "group {group}"
            );
        }
        assert_eq!(db.experience_at_level(0, 100), Some(1_000_000));
        assert_eq!(db.experience_at_level(5, 100), Some(1_250_000));
        assert_eq!(db.experience_at_level(0, 101), Some(1_000_000));
        assert_eq!(db.experience_at_level(0, 0), None);
        assert!(db.level_for_experience_is_unknown());
    }

    #[test]
    fn hp_formula_matches_hand_computed_n2_examples() {
        assert_eq!(calculate_hp(100, 50, 0, Some(false)), Some(220));
        assert_eq!(calculate_hp(100, 50, 1, Some(false)), Some(228));
        assert_eq!(calculate_hp(100, 50, 10, Some(false)), Some(320));
        assert_eq!(calculate_hp(100, 50, 10, Some(true)), Some(1));
        assert_eq!(calculate_hp(100, 50, 11, Some(false)), None);
        assert_eq!(calculate_hp(100, 50, 0, None), None);
    }

    #[test]
    fn non_hp_formula_requires_unresolved_inputs() {
        assert_eq!(calculate_non_hp_stat(100, 50, 1, None, Some(1.1)), None);
        assert_eq!(calculate_non_hp_stat(100, 50, 1, Some(200.0), None), None);
        assert_eq!(
            calculate_non_hp_stat(100, 50, 1, Some(200.0), Some(1.1)),
            Some(248)
        );
    }

    #[test]
    fn pokemon_param_layout_clamps_level_and_keeps_raw_iv_order() {
        let mut bytes = vec![0; 0x70];
        bytes[0x10..0x14].copy_from_slice(&0x1234_u32.to_le_bytes());
        bytes[0x30..0x32].copy_from_slice(&150_u16.to_le_bytes());
        for (i, offset) in [0x3a, 0x3c, 0x3e, 0x40, 0x42, 0x44].into_iter().enumerate() {
            bytes[offset..offset + 2].copy_from_slice(&(i as i16).to_le_bytes());
        }
        bytes[0x64] = 3;
        let param = PokemonParam::from_bytes(&bytes).unwrap();
        assert_eq!(param.pid, 0x1234);
        assert_eq!(param.level, 100);
        assert_eq!(param.iv_slots, [0, 1, 2, 3, 4, 5]);
        assert_eq!(param.guaranteed_perfect_ivs, 3);
        assert!(PokemonParam::from_bytes(&bytes[..0x64]).is_none());
    }

    #[test]
    fn evolution_methods_preserve_numbers_without_claiming_predicates() {
        let db =
            PmlDatabase::load_from_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
                .unwrap();
        let method = db
            .evolutions
            .iter()
            .find(|entry| entry.method == 51)
            .unwrap();
        assert_eq!(method.pla_method(), Some(PlaEvolutionMethod::AgileMoveUses));
        assert!(db
            .evolutions
            .iter()
            .filter(|entry| (48..=54).contains(&entry.method))
            .all(|entry| entry.pla_method().is_some()));
    }
}
