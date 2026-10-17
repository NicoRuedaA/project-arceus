//! National Pokédex data layer (EXTERNAL/PROBABLE) merged with the VERIFIED PLA tables.
//!
//! `data/national/*.tsv` is built by `.tools/build_national_dex.py` from public external sources
//! (PokeAPI CSVs + Pokemon Showdown's Legends: Z-A learnsets; see the sheet headers and
//! `reports/data-progress/national_dex.md`).  Every row is EXTERNAL/PROBABLE and never overrides
//! the game files: for any species/form with an active row in `data/pml_personal.tsv` the PLA
//! sheets win at load time (see `BattleData::species_row*`), and this layer only fills what the
//! game does not have.
//!
//! What this phase deliberately does not include: models, icons, cries, world spawns, research
//! tasks, PLA-specific stats (effort/styles) and abilities for the new species.  Form mechanics
//! (Mega, Gigantamax, Terastal) are data entries only.
//!
//! Rules encoded by the generator (see `.tools/build_national_dex.py`):
//! * one form entry per PokeAPI pokemon row (1025 defaults + 326 alternatives) plus 228 extra
//!   cosmetic form rows (Unown letters, Alcremie flavors, ...); `form_index` 0 is the default
//!   form and the rest keep a stable order;
//! * learnsets follow the chain Z-A > Scarlet/Violet > Sword/Shield > most recent older version;
//!   an entry with no list anywhere inherits its species' default form (variants always inherit
//!   their parent row), and `inherited`/`inherited_from` record that;
//! * moves the engine cannot load stay in `learnsets.tsv` with `in_engine = 0` and are summarised
//!   in `moves_not_in_engine.tsv` (never dropped silently).

use std::collections::BTreeMap;
use std::path::Path;

use crate::pml::read_table;

pub const EXPECTED_SPECIES: usize = 1_025;
/// Form entries with a PokeAPI pokemon_id (1025 defaults + 326 alternative forms).
pub const EXPECTED_FORMS: usize = 1_351;
/// Extra cosmetic form entries that are not separate pokemon rows.
pub const EXPECTED_VARIANTS: usize = 228;
pub const EXPECTED_EVOLUTIONS: usize = 676;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NationalSpecies {
    pub national_number: u16,
    pub identifier: String,
    pub generation: u8,
    pub is_legendary: bool,
    pub is_mythical: bool,
    pub is_baby: bool,
    pub capture_rate: u8,
    /// PokeAPI female eighths: -1 genderless, 0 all male, 8 all female, else n/8 female.
    pub gender_rate: i8,
    /// Engine-style byte derived from `gender_rate` (255 genderless, 0 all male, 254 all female).
    pub gender_raw: u8,
    pub hatch_counter: u16,
    pub growth_rate_id: u8,
    /// PLA exp group id (see `data/pml_grow_table.tsv`).
    pub growth_group: u8,
    pub growth_name: String,
    pub base_happiness: u8,
    pub base_experience: u16,
    pub evolution_chain: u16,
    pub entry_count: u16,
    pub name_en: String,
    pub name_es: String,
    pub name_ja_roma: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NationalForm {
    pub pokemon_id: u32,
    pub national_number: u16,
    pub form_index: u8,
    pub identifier: String,
    pub form_identifier: String,
    pub is_default: bool,
    /// base | regional | mega | gmax | form.
    pub form_kind: String,
    pub is_mega: bool,
    pub is_gmax: bool,
    pub is_regional: bool,
    pub is_battle_only: bool,
    pub introduced_version_group: u16,
    pub form_order: u8,
    pub height_dm: u16,
    pub weight_hg: u16,
    pub base_experience: u16,
    pub name_en: String,
    pub name_es: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NationalVariant {
    pub national_number: u16,
    pub form_index: u8,
    pub parent_pokemon_id: u32,
    pub identifier: String,
    pub form_identifier: String,
    pub form_order: u8,
    pub name_en: String,
    pub name_es: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NationalStats {
    pub pokemon_id: u32,
    pub national_number: u16,
    pub form_index: u8,
    /// HP, Attack, Defense, Special Attack, Special Defense, Speed.
    pub stats: [u8; 6],
    pub total: u16,
    /// True when the row copies the parent pokemon row (cosmetic variant fill-in).
    pub inherited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NationalTypes {
    pub pokemon_id: u32,
    pub national_number: u16,
    pub form_index: u8,
    pub type1: String,
    pub type2: String,
    pub inherited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NationalEvolution {
    pub evolution_chain: u16,
    pub from_species: u16,
    pub from_form: u16,
    pub to_species: u16,
    pub to_form: u16,
    pub trigger: String,
    pub trigger_id: u8,
    pub min_level: u8,
    pub item: String,
    pub held_item: String,
    pub time_of_day: String,
    pub known_move: String,
    pub known_move_type: String,
    pub min_happiness: u16,
    pub condition: String,
    pub version_group: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NationalLearnset {
    pub national_number: u16,
    pub form_index: u8,
    pub pokemon_id: u32,
    /// za | sv | swsh | lgpe | bdsp | pla | genN.
    pub source: String,
    pub inherited: bool,
    pub inherited_from: u32,
    /// level_up | machine | egg | tutor | form_change | special.
    pub method: String,
    pub method_id: u8,
    pub level: u8,
    pub move_id: u16,
    pub engine_move_id: u16,
    pub move_name: String,
    pub in_engine: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NationalMissingMove {
    pub move_name: String,
    pub sources: String,
    pub rows_count: u16,
    pub species_count: u16,
    pub pokemon_count: u16,
    pub example_species: String,
    pub first_pokemon_id: u32,
    pub note: String,
}

/// The national data layer.  An empty deck (no `data/national/` directory) keeps the engine
/// behaving exactly as before the layer existed.
#[derive(Debug, Clone, Default)]
pub struct NationalDex {
    pub species: Vec<NationalSpecies>,
    pub forms: Vec<NationalForm>,
    pub variants: Vec<NationalVariant>,
    pub stats: Vec<NationalStats>,
    pub types: Vec<NationalTypes>,
    pub evolutions: Vec<NationalEvolution>,
    pub learnsets: Vec<NationalLearnset>,
    pub missing_moves: Vec<NationalMissingMove>,
    species_index: BTreeMap<u16, usize>,
    form_index: BTreeMap<(u16, u8), usize>,
    stats_index: BTreeMap<(u16, u8), usize>,
    types_index: BTreeMap<(u16, u8), usize>,
    default_form_index: BTreeMap<u16, usize>,
    identifier_index: BTreeMap<String, usize>,
}

impl NationalDex {
    /// Load `<data_root>/national/*.tsv`.  A missing directory is not an error: the returned deck
    /// is empty and the engine behaves as before the layer existed.
    pub fn load(directory: impl AsRef<Path>) -> Result<Self, String> {
        let dir = directory.as_ref();
        if !dir.join("species.tsv").is_file() {
            return Ok(Self::default());
        }
        let mut dex = Self {
            species: read_table(dir.join("species.tsv"), parse_species)?,
            forms: read_table(dir.join("forms.tsv"), parse_form)?,
            variants: read_table(dir.join("form_variants.tsv"), parse_variant)?,
            stats: read_table(dir.join("base_stats.tsv"), parse_stats)?,
            types: read_table(dir.join("types.tsv"), parse_types)?,
            evolutions: read_table(dir.join("evolutions.tsv"), parse_evolution)?,
            learnsets: read_table(dir.join("learnsets.tsv"), parse_learnset)?,
            missing_moves: read_table(dir.join("moves_not_in_engine.tsv"), parse_missing_move)?,
            ..Self::default()
        };
        dex.rebuild_indexes();
        Ok(dex)
    }

    fn rebuild_indexes(&mut self) {
        self.species_index.clear();
        self.form_index.clear();
        self.stats_index.clear();
        self.types_index.clear();
        self.default_form_index.clear();
        self.identifier_index.clear();
        for (index, species) in self.species.iter().enumerate() {
            self.species_index.insert(species.national_number, index);
        }
        for (index, form) in self.forms.iter().enumerate() {
            self.form_index
                .insert((form.national_number, form.form_index), index);
            self.identifier_index
                .entry(form.identifier.clone())
                .or_insert(index);
            if form.is_default {
                self.default_form_index
                    .entry(form.national_number)
                    .or_insert(index);
            }
        }
        for (index, stats) in self.stats.iter().enumerate() {
            self.stats_index
                .insert((stats.national_number, stats.form_index), index);
        }
        for (index, types) in self.types.iter().enumerate() {
            self.types_index
                .insert((types.national_number, types.form_index), index);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.species.is_empty()
    }

    pub fn species(&self, national: u16) -> Option<&NationalSpecies> {
        self.species_index.get(&national).map(|&i| &self.species[i])
    }

    /// Exact form entry by (national number, national form index).
    pub fn form(&self, national: u16, form_index: u8) -> Option<&NationalForm> {
        self.form_index
            .get(&(national, form_index))
            .map(|&i| &self.forms[i])
    }

    pub fn default_form(&self, national: u16) -> Option<&NationalForm> {
        self.default_form_index
            .get(&national)
            .map(|&i| &self.forms[i])
    }

    pub fn form_by_identifier(&self, identifier: &str) -> Option<&NationalForm> {
        self.identifier_index.get(identifier).map(|&i| &self.forms[i])
    }

    pub fn default_form_by_identifier(&self, identifier: &str) -> Option<&NationalForm> {
        self.default_form_index.values().find_map(|&i| {
            let form = &self.forms[i];
            (form.identifier == identifier).then_some(form)
        })
    }

    pub fn stats_of(&self, national: u16, form_index: u8) -> Option<&NationalStats> {
        self.stats_index
            .get(&(national, form_index))
            .map(|&i| &self.stats[i])
    }

    pub fn types_of(&self, national: u16, form_index: u8) -> Option<&NationalTypes> {
        self.types_index
            .get(&(national, form_index))
            .map(|&i| &self.types[i])
    }

    pub fn variant(&self, national: u16, form_index: u8) -> Option<&NationalVariant> {
        self.variants
            .iter()
            .find(|v| v.national_number == national && v.form_index == form_index)
    }

    pub fn learnsets_of(&self, national: u16, form_index: u8) -> Vec<&NationalLearnset> {
        self.learnsets
            .iter()
            .filter(|row| row.national_number == national && row.form_index == form_index)
            .collect()
    }

    /// Learnset rows of one entry whose move exists in the engine's move table.
    pub fn engine_learnsets_of(&self, national: u16, form_index: u8) -> Vec<&NationalLearnset> {
        self.learnsets
            .iter()
            .filter(|row| {
                row.national_number == national && row.form_index == form_index && row.in_engine
            })
            .collect()
    }

    pub fn evolutions_to(&self, species: u16) -> Vec<&NationalEvolution> {
        self.evolutions
            .iter()
            .filter(|row| row.to_species == species)
            .collect()
    }
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

fn text(row: &[(&str, &str)], name: &str) -> Result<String, String> {
    Ok(field(row, name)?.to_owned())
}

fn flag(row: &[(&str, &str)], name: &str) -> Result<bool, String> {
    match field(row, name)? {
        "1" | "true" => Ok(true),
        "0" | "false" => Ok(false),
        other => Err(format!("invalid {name} {other:?}")),
    }
}

fn parse_species(row: &[(&str, &str)]) -> Result<NationalSpecies, String> {
    Ok(NationalSpecies {
        national_number: number(row, "national_number")?,
        identifier: text(row, "identifier")?,
        generation: number(row, "generation")?,
        is_legendary: flag(row, "is_legendary")?,
        is_mythical: flag(row, "is_mythical")?,
        is_baby: flag(row, "is_baby")?,
        capture_rate: number(row, "capture_rate")?,
        gender_rate: number(row, "gender_rate")?,
        gender_raw: number(row, "gender_raw")?,
        hatch_counter: number(row, "hatch_counter")?,
        growth_rate_id: number(row, "growth_rate_id")?,
        growth_group: number(row, "growth_group")?,
        growth_name: text(row, "growth_name")?,
        base_happiness: number(row, "base_happiness")?,
        base_experience: number(row, "base_experience")?,
        evolution_chain: number(row, "evolution_chain")?,
        entry_count: number(row, "entry_count")?,
        name_en: text(row, "name_en")?,
        name_es: text(row, "name_es")?,
        name_ja_roma: text(row, "name_ja_roma")?,
    })
}

fn parse_form(row: &[(&str, &str)]) -> Result<NationalForm, String> {
    Ok(NationalForm {
        pokemon_id: number(row, "pokemon_id")?,
        national_number: number(row, "national_number")?,
        form_index: number(row, "form_index")?,
        identifier: text(row, "identifier")?,
        form_identifier: text(row, "form_identifier")?,
        is_default: flag(row, "is_default")?,
        form_kind: text(row, "form_kind")?,
        is_mega: flag(row, "is_mega")?,
        is_gmax: flag(row, "is_gmax")?,
        is_regional: flag(row, "is_regional")?,
        is_battle_only: flag(row, "is_battle_only")?,
        introduced_version_group: number(row, "introduced_version_group")?,
        form_order: number(row, "form_order")?,
        height_dm: number(row, "height_dm")?,
        weight_hg: number(row, "weight_hg")?,
        base_experience: number(row, "base_experience")?,
        name_en: text(row, "name_en")?,
        name_es: text(row, "name_es")?,
    })
}

fn parse_variant(row: &[(&str, &str)]) -> Result<NationalVariant, String> {
    Ok(NationalVariant {
        national_number: number(row, "national_number")?,
        form_index: number(row, "form_index")?,
        parent_pokemon_id: number(row, "parent_pokemon_id")?,
        identifier: text(row, "identifier")?,
        form_identifier: text(row, "form_identifier")?,
        form_order: number(row, "form_order")?,
        name_en: text(row, "name_en")?,
        name_es: text(row, "name_es")?,
    })
}

fn parse_stats(row: &[(&str, &str)]) -> Result<NationalStats, String> {
    Ok(NationalStats {
        pokemon_id: number(row, "pokemon_id")?,
        national_number: number(row, "national_number")?,
        form_index: number(row, "form_index")?,
        stats: [
            number(row, "hp")?,
            number(row, "atk")?,
            number(row, "def")?,
            number(row, "spa")?,
            number(row, "spd")?,
            number(row, "spe")?,
        ],
        total: number(row, "total")?,
        inherited: flag(row, "inherited")?,
    })
}

fn parse_types(row: &[(&str, &str)]) -> Result<NationalTypes, String> {
    Ok(NationalTypes {
        pokemon_id: number(row, "pokemon_id")?,
        national_number: number(row, "national_number")?,
        form_index: number(row, "form_index")?,
        type1: text(row, "type1")?,
        type2: text(row, "type2")?,
        inherited: flag(row, "inherited")?,
    })
}

fn parse_evolution(row: &[(&str, &str)]) -> Result<NationalEvolution, String> {
    Ok(NationalEvolution {
        evolution_chain: number(row, "evolution_chain")?,
        from_species: number(row, "from_species")?,
        from_form: number(row, "from_form")?,
        to_species: number(row, "to_species")?,
        to_form: number(row, "to_form")?,
        trigger: text(row, "trigger")?,
        trigger_id: number(row, "trigger_id")?,
        min_level: number(row, "min_level")?,
        item: text(row, "item")?,
        held_item: text(row, "held_item")?,
        time_of_day: text(row, "time_of_day")?,
        known_move: text(row, "known_move")?,
        known_move_type: text(row, "known_move_type")?,
        min_happiness: number(row, "min_happiness")?,
        condition: text(row, "condition")?,
        version_group: number(row, "version_group")?,
    })
}

fn parse_learnset(row: &[(&str, &str)]) -> Result<NationalLearnset, String> {
    Ok(NationalLearnset {
        national_number: number(row, "national_number")?,
        form_index: number(row, "form_index")?,
        pokemon_id: number(row, "pokemon_id")?,
        source: text(row, "source")?,
        inherited: flag(row, "inherited")?,
        inherited_from: number(row, "inherited_from")?,
        method: text(row, "method")?,
        method_id: number(row, "method_id")?,
        level: number(row, "level")?,
        move_id: number(row, "move_id")?,
        engine_move_id: number(row, "engine_move_id")?,
        move_name: text(row, "move_name")?,
        in_engine: flag(row, "in_engine")?,
    })
}

fn parse_missing_move(row: &[(&str, &str)]) -> Result<NationalMissingMove, String> {
    Ok(NationalMissingMove {
        move_name: text(row, "move_name")?,
        sources: text(row, "sources")?,
        rows_count: number(row, "rows_count")?,
        species_count: number(row, "species_count")?,
        pokemon_count: number(row, "pokemon_count")?,
        example_species: text(row, "example_species")?,
        first_pokemon_id: number(row, "first_pokemon_id")?,
        note: text(row, "note")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::BattleData;
    use crate::battle_engine::{Action, Battle, Combatant, XorShift32};
    use crate::pml::PmlDatabase;

    fn data_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    fn national_dex() -> NationalDex {
        NationalDex::load(data_root().join("national")).unwrap()
    }

    #[test]
    fn loads_the_national_sheets_from_repository_data() {
        let dex = national_dex();
        assert!(!dex.is_empty());
        assert_eq!(dex.species.len(), EXPECTED_SPECIES);
        assert_eq!(dex.forms.len(), EXPECTED_FORMS);
        assert_eq!(dex.variants.len(), EXPECTED_VARIANTS);
        assert_eq!(dex.stats.len(), EXPECTED_FORMS + EXPECTED_VARIANTS);
        assert_eq!(dex.types.len(), EXPECTED_FORMS + EXPECTED_VARIANTS);
        assert_eq!(dex.evolutions.len(), EXPECTED_EVOLUTIONS);
        assert!(
            dex.learnsets.len() > 80_000,
            "learnsets: {}",
            dex.learnsets.len()
        );
        assert_eq!(dex.missing_moves.len(), 14);
        // one species row per national number, no gaps and no duplicates
        assert_eq!(dex.species.first().unwrap().national_number, 1);
        assert_eq!(dex.species.last().unwrap().national_number, 1025);
        let mut sorted: Vec<u16> = dex.species.iter().map(|s| s.national_number).collect();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), EXPECTED_SPECIES);
    }

    #[test]
    fn charizard_entry_has_stats_types_names_and_forms() {
        let dex = national_dex();
        let charizard = dex.form(6, 0).expect("charizard default form");
        assert_eq!(charizard.identifier, "charizard");
        assert_eq!(charizard.name_en, "Charizard");
        assert_eq!(charizard.form_kind, "base");
        assert_eq!((charizard.height_dm, charizard.weight_hg), (17, 905));
        assert_eq!(
            dex.stats_of(6, 0).unwrap().stats,
            [78, 84, 78, 109, 85, 100]
        );
        let types = dex.types_of(6, 0).unwrap();
        assert_eq!((types.type1.as_str(), types.type2.as_str()), ("Fire", "Flying"));
        let mega = dex
            .form_by_identifier("charizard-mega-x")
            .expect("charizard-mega-x");
        assert_eq!(mega.form_kind, "mega");
        assert!(mega.is_mega);
        let gmax = dex
            .form_by_identifier("charizard-gmax")
            .expect("charizard-gmax");
        assert_eq!(gmax.form_kind, "gmax");
        assert!(gmax.is_gmax);
        let species = dex.species(6).unwrap();
        assert_eq!(species.identifier, "charizard");
        assert_eq!(species.name_es, "Charizard");
        assert!(!species.name_ja_roma.is_empty());
        assert!(species.entry_count >= 4);
    }

    #[test]
    fn hisuian_growlithe_matches_the_verified_pla_row() {
        let dex = national_dex();
        let pml = PmlDatabase::load_from_dir(data_root()).unwrap();
        let pla = pml
            .personal
            .iter()
            .find(|row| row.national_dex == 58 && row.form == 1 && row.is_active)
            .expect("verified Hisuian Growlithe row");
        let form = dex.form_by_identifier("growlithe-hisui").unwrap();
        let stats = dex.stats_of(form.national_number, form.form_index).unwrap();
        assert_eq!(stats.stats, pla.base_stats);
        let types = dex.types_of(form.national_number, form.form_index).unwrap();
        assert_eq!(types.type1, pla.type1_name);
        assert_eq!(types.type2, pla.type2_name);
        assert!(!stats.inherited);
    }

    #[test]
    fn learnsets_follow_the_chain_and_forms_inherit() {
        let dex = national_dex();
        let charizard = dex.learnsets_of(6, 0);
        assert!(!charizard.is_empty());
        assert!(charizard.iter().all(|row| row.source == "za"));
        // Z-A lists the same move under both codes (9M machine + 9L30 level-up).
        let flamethrowers: Vec<&NationalLearnset> = charizard
            .iter()
            .filter(|row| row.move_name == "flamethrower")
            .copied()
            .collect();
        assert_eq!(flamethrowers.len(), 2);
        assert!(flamethrowers.iter().any(|row| row.method == "machine"));
        assert!(flamethrowers
            .iter()
            .any(|row| row.method == "level_up" && row.level == 30));
        assert!(flamethrowers
            .iter()
            .all(|row| row.in_engine && row.engine_move_id == 53));
        // Charizard's Mega X has its own PokeAPI list (an older version group); the Gigantamax
        // form has no list anywhere, so it inherits the default form's list.
        let mega_index = dex.form_by_identifier("charizard-mega-x").unwrap().form_index;
        let mega = dex.learnsets_of(6, mega_index);
        assert!(!mega.is_empty());
        assert!(mega.iter().all(|row| !row.inherited));
        let gmax_index = dex.form_by_identifier("charizard-gmax").unwrap().form_index;
        let gmax = dex.learnsets_of(6, gmax_index);
        assert!(!gmax.is_empty());
        assert!(gmax.iter().all(|row| row.inherited));
        assert!(gmax.iter().all(|row| row.inherited_from == 6));
        // Cosmetic variants inherit from their parent pokemon row.
        let alcremie_variant = dex.variant(869, 2).expect("alcremie variant at index 2");
        assert_eq!(alcremie_variant.parent_pokemon_id, 869);
        let variant_rows = dex.learnsets_of(869, 2);
        assert!(!variant_rows.is_empty());
        assert!(variant_rows.iter().all(|row| row.inherited));
    }

    #[test]
    fn moves_missing_from_the_engine_are_listed() {
        let dex = national_dex();
        let tera_blast = dex
            .missing_moves
            .iter()
            .find(|row| row.move_name == "tera-blast")
            .expect("tera-blast listed");
        assert!(tera_blast.rows_count > 0);
        assert!(tera_blast.sources.contains("sv"));
        let rows: Vec<&NationalLearnset> = dex
            .learnsets
            .iter()
            .filter(|row| row.move_name == "tera-blast")
            .collect();
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|row| !row.in_engine && row.engine_move_id == 0));
    }

    #[test]
    fn battle_data_prefers_pla_and_falls_back_to_the_national_layer() {
        let data = BattleData::load(data_root()).unwrap();
        // PLA active row wins for Hisuian Growlithe.
        let hisuian = data.species_row_by_dex_form(58, 1).unwrap().unwrap();
        assert!(!hisuian.external);
        assert_eq!(hisuian.types, ["Fire".to_owned(), "Rock".to_owned()]);
        // The dex-level fallback for (58, 0) stays the first active PLA form, exactly as before.
        let fallback = data.species_row_by_dex_form(58, 0).unwrap().unwrap();
        assert!(!fallback.external);
        assert_eq!(fallback.types, ["Fire".to_owned(), "Rock".to_owned()]);
        // The explicit national lookup returns the external Kanto Growlithe instead.
        let kanto = data.national_species_row(58, 0).unwrap().unwrap();
        assert!(kanto.external);
        assert_eq!(kanto.types, ["Fire".to_owned(), "Fire".to_owned()]);
        // Species and forms the game does not have come from the national layer.
        let charizard = data.species_row("charizard").unwrap().unwrap();
        assert!(charizard.external);
        assert_eq!(charizard.dex, 6);
        let pecharunt = data.species_row_by_dex(1025).unwrap().unwrap();
        assert!(pecharunt.external);
        // A known PLA species keeps its verified row through the old lookups.
        let pikachu = data.species_row("pikachu").unwrap().unwrap();
        assert!(!pikachu.external);
        assert_eq!(pikachu.abilities, [9, 9, 31]);
    }

    fn combatant(data: &BattleData, dex: u16, form: u8, level: u8, moves: &[&str]) -> Combatant {
        let species = data
            .species_row_by_dex_form(dex, form)
            .unwrap()
            .expect("species row");
        let moves = moves
            .iter()
            .map(|name| data.move_row(name).unwrap().expect("move row"))
            .collect();
        Combatant::from_base(species, level, moves)
    }

    #[test]
    fn national_species_battles_a_hisui_species_headless() {
        let data = BattleData::load(data_root()).unwrap();
        // Charizard (EXTERNAL national entry) vs Hisuian Decidueye (VERIFIED PLA form).
        let charizard = data.species_row("charizard").unwrap().unwrap();
        assert!(charizard.external);
        assert_eq!(charizard.dex, 6);
        let decidueye = data.species_row_by_dex_form(724, 1).unwrap().unwrap();
        assert!(!decidueye.external);
        assert_eq!(decidueye.types, ["Grass".to_owned(), "Fighting".to_owned()]);
        // Dragapult is NOT in the game, so it must come from the national layer; Garchomp IS in
        // Legends: Arceus (Alpha Garchomp), so its verified row must keep winning.
        let dragapult = data.species_row("dragapult").unwrap().unwrap();
        assert!(dragapult.external);
        assert_eq!(dragapult.dex, 887);
        let garchomp = data.species_row("garchomp").unwrap().unwrap();
        assert!(!garchomp.external);
        assert_eq!(garchomp.dex, 445);

        let mut battle = Battle::new(
            vec![
                combatant(&data, 6, 0, 50, &["air-slash", "flamethrower"]),
                combatant(&data, 724, 1, 50, &["leaf-blade"]),
            ],
            vec![vec![0], vec![1]],
            data.type_chart().clone(),
        )
        .unwrap();
        battle.set_pla_damage_formula(true);
        let mut rng = XorShift32(0x4e41_54);
        let events = battle.play_turn(
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
            &mut rng,
        );
        let used = events
            .iter()
            .find_map(|event| match event {
                crate::battle_engine::Event::UsedMove {
                    actor,
                    target,
                    name,
                    damage,
                    effectiveness,
                    ..
                } if *actor == 0 => Some((*target, name.clone(), *damage, effectiveness.clone())),
                _ => None,
            })
            .expect("charizard's move resolves");
        assert_eq!(used.1, "air-slash");
        // Flying vs Grass/Fighting is 4x: the engine's labels cap it at "super effective".
        assert_eq!(used.3, "super effective");
        assert!(used.2 > 0, "damage must be positive");
        assert!(
            battle.combatants[1].hp < battle.combatants[1].max_hp,
            "Hisuian Decidueye must lose HP"
        );
        // Both entries keep their data provenance.
        assert!(battle.combatants[0].species.external);
        assert!(!battle.combatants[1].species.external);
    }
}
