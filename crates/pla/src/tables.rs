//! Evidence-bounded loaders for curated item, encounter, outbreak, and weather TSVs.
//!
//! This module does not promote raw field positions to semantic names without
//! direct supporting evidence. Shop and crafting binary inputs are not decoded
//! into local TSVs and are intentionally outside the loader API.

use std::{collections::BTreeSet, path::Path};

use crate::pml::{PmlDatabase, read_table};

const ITEM_INDEX_ROWS: usize = 1_830;
const WILD_ENCOUNTER_ROWS: usize = 322;
const OUTBREAK_AREA_ROWS: usize = 5;
const OUTBREAK_LOTTERY_ROWS: usize = 5;
const WEATHER_TRANSITION_ROWS: usize = 38;
const WEATHER_LOTTERY_ROWS: usize = 8;

/// Parsed curated TSV tables whose source coverage is described in
/// `docs/table-loaders.md`. Binary-only tables are deliberately absent.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaTableDatabase {
    pub item_hash_index: Vec<ItemHashIndexRow>,
    pub wild_encounter_modifiers: Vec<WildEncounterModifier>,
    pub outbreak_areas: Vec<RawOutbreakRow>,
    pub outbreak_lottery: Vec<RawOutbreakRow>,
    pub weather_dry_transitions: Vec<WeatherDryTransition>,
    pub weather_lottery: Vec<WeatherLotteryRow>,
}

impl PlaTableDatabase {
    /// Load the checked-in data TSVs and validate supported PML joins.
    pub fn load_from_dir(directory: impl AsRef<Path>, pml: &PmlDatabase) -> Result<Self, String> {
        let dir = directory.as_ref();
        let item_hash_index =
            read_table(dir.join("items/item_hash_to_index.tsv"), parse_item_index)?;
        let wild_encounter_modifiers = read_table(
            dir.join("field/wild_encounter_modifiers.tsv"),
            parse_wild_encounter,
        )?;
        let outbreak_areas = read_raw_outbreak_table(
            dir.join("field/huge_outbreak_areas.tsv"),
            OUTBREAK_AREA_ROWS,
        )?;
        let outbreak_lottery = read_raw_outbreak_table(
            dir.join("field/huge_outbreak_lottery.tsv"),
            OUTBREAK_LOTTERY_ROWS,
        )?;
        let weather_dry_transitions = read_table(
            dir.join("weather/weather_dry_transitions.tsv"),
            parse_weather_transition,
        )?;
        let weather_lottery = read_table(
            dir.join("weather/weather_lottery_matrix.tsv"),
            parse_weather_lottery,
        )?;

        require_count("item hash index", item_hash_index.len(), ITEM_INDEX_ROWS)?;
        require_count(
            "wild encounter modifiers",
            wild_encounter_modifiers.len(),
            WILD_ENCOUNTER_ROWS,
        )?;
        require_count(
            "weather dry transitions",
            weather_dry_transitions.len(),
            WEATHER_TRANSITION_ROWS,
        )?;
        require_count(
            "weather lottery",
            weather_lottery.len(),
            WEATHER_LOTTERY_ROWS,
        )?;
        validate_item_index(&item_hash_index)?;
        validate_wild_encounters(&wild_encounter_modifiers, pml)?;
        validate_weather(&weather_dry_transitions, &weather_lottery)?;

        Ok(Self {
            item_hash_index,
            wild_encounter_modifiers,
            outbreak_areas,
            outbreak_lottery,
            weather_dry_transitions,
            weather_lottery,
        })
    }
}

/// Raw item hash-index row. The field positions and local wire widths are
/// retained; the meaning of `campo_08` is unresolved and its width conflicts
/// with the referenced community parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemHashIndexRow {
    pub record_index: u32,
    pub campo_00: u64,
    pub campo_08: u64,
    pub evidence: String,
}

/// Wild encounter modifier row. Only `species_id` has a supported PML-domain
/// join; the remaining values retain raw field indices rather than proposed
/// meanings.
#[derive(Debug, Clone, PartialEq)]
pub struct WildEncounterModifier {
    pub species_id: u32,
    pub campo_01: Option<u32>,
    pub campo_02_to_13: [Option<f32>; 12],
    pub evidence: String,
}

/// One schema-unresolved outbreak row. Field labels are normalized to
/// `campo_NN`; values remain their original TSV text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawOutbreakRow {
    pub record_index: u8,
    pub fields: Vec<RawField>,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawField {
    pub name: String,
    pub value: Option<String>,
}

/// Curated weather transition row from the app-config export.
#[derive(Debug, Clone, PartialEq)]
pub struct WeatherDryTransition {
    pub time_of_day: String,
    pub weather_condition: String,
    pub target_surface: String,
    pub rate: f64,
    pub description: String,
}

/// Curated weather lottery row. The IDs/names are data labels, not behavioral
/// conclusions about eligibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeatherLotteryRow {
    pub weather_id: u8,
    pub weather_name: String,
    pub internal_code: String,
    pub disable_system_flag: String,
    pub default_weight: u8,
    pub description: String,
}

fn parse_item_index(row: &[(&str, &str)]) -> Result<ItemHashIndexRow, String> {
    Ok(ItemHashIndexRow {
        record_index: number(row, "record_index")?,
        campo_00: number(row, "f00_u64")?,
        campo_08: number(row, "f08_u64")?,
        evidence: string(row, "evidence")?,
    })
}

fn parse_wild_encounter(row: &[(&str, &str)]) -> Result<WildEncounterModifier, String> {
    let mut fields = [None; 12];
    for (index, value) in fields.iter_mut().enumerate() {
        let column = format!("f{:02}_f32", index + 2);
        *value = optional_number(row, &column)?;
    }
    Ok(WildEncounterModifier {
        species_id: number(row, "f00_u32")?,
        campo_01: optional_number(row, "f01_u32")?,
        campo_02_to_13: fields,
        evidence: string(row, "evidence")?,
    })
}

fn read_raw_outbreak_table(
    path: impl AsRef<Path>,
    expected_rows: usize,
) -> Result<Vec<RawOutbreakRow>, String> {
    let path = path.as_ref();
    let rows = read_table(path, |row| {
        let record_index = number(row, "record_index")?;
        let evidence = string(row, "evidence")?;
        let mut fields = Vec::new();
        for (column, value) in row {
            if *column == "record_index" || *column == "evidence" {
                continue;
            }
            let raw_index = column
                .strip_prefix('f')
                .and_then(|rest| rest.get(..2))
                .and_then(|digits| digits.parse::<u8>().ok())
                .ok_or_else(|| format!("unexpected raw field column {column}"))?;
            fields.push(RawField {
                name: format!("campo_{raw_index:02}"),
                value: (!value.is_empty()).then(|| (*value).to_owned()),
            });
        }
        Ok(RawOutbreakRow {
            record_index,
            fields,
            evidence,
        })
    })?;
    require_count(
        &format!("{} raw outbreak", path.display()),
        rows.len(),
        expected_rows,
    )?;
    for (expected_index, row) in rows.iter().enumerate() {
        if usize::from(row.record_index) != expected_index {
            return Err(format!(
                "{}: expected record_index {expected_index}, got {}",
                path.display(),
                row.record_index
            ));
        }
    }
    Ok(rows)
}

fn parse_weather_transition(row: &[(&str, &str)]) -> Result<WeatherDryTransition, String> {
    Ok(WeatherDryTransition {
        time_of_day: string(row, "time_of_day")?,
        weather_condition: string(row, "weather_condition")?,
        target_surface: string(row, "target_surface")?,
        rate: number(row, "rate")?,
        description: string(row, "description")?,
    })
}

fn parse_weather_lottery(row: &[(&str, &str)]) -> Result<WeatherLotteryRow, String> {
    Ok(WeatherLotteryRow {
        weather_id: number(row, "weather_id")?,
        weather_name: string(row, "weather_name")?,
        internal_code: string(row, "internal_code")?,
        disable_system_flag: string(row, "disable_system_flag")?,
        default_weight: number(row, "default_weight")?,
        description: string(row, "description")?,
    })
}

fn validate_item_index(rows: &[ItemHashIndexRow]) -> Result<(), String> {
    for (index, row) in rows.iter().enumerate() {
        if row.record_index as usize != index {
            return Err(format!(
                "item hash index: expected record_index {index}, got {}",
                row.record_index
            ));
        }
    }
    Ok(())
}

fn validate_wild_encounters(
    rows: &[WildEncounterModifier],
    pml: &PmlDatabase,
) -> Result<(), String> {
    let active_species: BTreeSet<u32> = pml
        .personal
        .iter()
        .filter(|row| row.is_active)
        .map(|row| u32::from(row.national_dex))
        .collect();
    for (index, row) in rows.iter().enumerate() {
        if !active_species.contains(&row.species_id) {
            return Err(format!(
                "wild encounter row {index}: species_id {} does not join active PML rows",
                row.species_id
            ));
        }
        if row
            .campo_02_to_13
            .iter()
            .flatten()
            .any(|value| !value.is_finite() || !(-1.0..=1.5).contains(value))
        {
            return Err(format!(
                "wild encounter row {index}: raw float field outside observed range"
            ));
        }
    }
    Ok(())
}

fn validate_weather(
    transitions: &[WeatherDryTransition],
    lottery: &[WeatherLotteryRow],
) -> Result<(), String> {
    for (index, row) in transitions.iter().enumerate() {
        if !row.rate.is_finite() || !(0.0..=1.0).contains(&row.rate) {
            return Err(format!(
                "weather transition row {index}: rate outside [0, 1]"
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for row in lottery {
        if !(2..=9).contains(&row.weather_id) || !ids.insert(row.weather_id) {
            return Err(format!(
                "weather lottery: duplicate or out-of-domain ID {}",
                row.weather_id
            ));
        }
    }
    Ok(())
}

fn require_count(name: &str, actual: usize, expected: usize) -> Result<(), String> {
    if actual != expected {
        return Err(format!("{name}: expected {expected} rows, got {actual}"));
    }
    Ok(())
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

fn optional_number<T: std::str::FromStr>(
    row: &[(&str, &str)],
    name: &str,
) -> Result<Option<T>, String> {
    let value = field(row, name)?;
    if value.is_empty() {
        return Ok(None);
    }
    value
        .parse()
        .map(Some)
        .map_err(|_| format!("invalid {name}"))
}

fn string(row: &[(&str, &str)], name: &str) -> Result<String, String> {
    Ok(field(row, name)?.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_data() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    #[test]
    fn loads_supported_tables_with_pinned_counts_and_pml_join() {
        let data = repository_data();
        let pml = PmlDatabase::load_from_dir(&data).unwrap();
        let tables = PlaTableDatabase::load_from_dir(&data, &pml).unwrap();

        assert_eq!(tables.item_hash_index.len(), ITEM_INDEX_ROWS);
        assert_eq!(tables.wild_encounter_modifiers.len(), WILD_ENCOUNTER_ROWS);
        assert_eq!(tables.outbreak_areas.len(), OUTBREAK_AREA_ROWS);
        assert_eq!(tables.outbreak_lottery.len(), OUTBREAK_LOTTERY_ROWS);
        assert_eq!(
            tables.weather_dry_transitions.len(),
            WEATHER_TRANSITION_ROWS
        );
        assert_eq!(tables.weather_lottery.len(), WEATHER_LOTTERY_ROWS);

        let active_species: BTreeSet<u32> = pml
            .personal
            .iter()
            .filter(|row| row.is_active)
            .map(|row| u32::from(row.national_dex))
            .collect();
        let encountered: BTreeSet<u32> = tables
            .wild_encounter_modifiers
            .iter()
            .map(|row| row.species_id)
            .collect();
        assert_eq!(encountered.len(), 242);
        assert_eq!(*encountered.first().unwrap(), 25);
        assert_eq!(*encountered.last().unwrap(), 905);
        assert!(encountered.is_subset(&active_species));
        assert_eq!(
            tables.wild_encounter_modifiers.len() - encountered.len(),
            80
        );
        assert_eq!(
            tables
                .wild_encounter_modifiers
                .iter()
                .filter(|row| row.campo_01.is_some())
                .count(),
            96
        );
        assert_eq!(
            tables
                .wild_encounter_modifiers
                .iter()
                .filter(|row| row.campo_01.is_none())
                .count(),
            226
        );
        let raw_float_values: Vec<f32> = tables
            .wild_encounter_modifiers
            .iter()
            .flat_map(|row| row.campo_02_to_13.iter().flatten())
            .copied()
            .collect();
        assert!(
            raw_float_values
                .iter()
                .all(|value| (-1.0..=1.5).contains(value))
        );
        assert_eq!(
            raw_float_values.iter().copied().reduce(f32::min),
            Some(-1.0)
        );
        assert_eq!(raw_float_values.iter().copied().reduce(f32::max), Some(1.5));
        assert_eq!(tables.item_hash_index.first().unwrap().record_index, 0);
        assert_eq!(tables.item_hash_index.last().unwrap().record_index, 1829);
        assert_eq!(tables.outbreak_areas[0].fields[0].name, "campo_00");
        assert_eq!(tables.outbreak_lottery[4].record_index, 4);
        assert!(
            tables
                .weather_dry_transitions
                .iter()
                .all(|row| row.rate == 0.05)
        );
        assert_eq!(
            tables
                .weather_lottery
                .iter()
                .map(|row| row.weather_id)
                .collect::<BTreeSet<_>>(),
            (2..=9).collect()
        );
        assert_eq!(
            tables
                .weather_lottery
                .iter()
                .map(|row| row.default_weight)
                .min(),
            Some(2)
        );
        assert_eq!(
            tables
                .weather_lottery
                .iter()
                .map(|row| row.default_weight)
                .max(),
            Some(40)
        );
    }

    #[test]
    fn raw_outbreak_and_item_index_fields_are_not_given_domain_names() {
        let data = repository_data();
        let pml = PmlDatabase::load_from_dir(&data).unwrap();
        let tables = PlaTableDatabase::load_from_dir(&data, &pml).unwrap();
        assert_ne!(tables.item_hash_index[0].campo_00, 0);
        assert_eq!(tables.item_hash_index[0].campo_08, 0);
        assert!(
            tables.outbreak_lottery[0]
                .fields
                .iter()
                .all(|field| field.name.starts_with("campo_"))
        );
    }
}
