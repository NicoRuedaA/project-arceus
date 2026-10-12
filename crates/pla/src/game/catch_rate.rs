//! Per-species capture (catch) rate from `data/pml_personal_extra.tsv`.
//!
//! The value is personal-table field 17 of the game's data (Bidoof 255, Arceus 3), decoded by
//! `.tools/decode_pml_tables.py`. How the capture formula uses it is the engine's LIKELY model.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Fallback used only when the sheet is missing or the species is not in it.
pub const CAPTURE_CATCH_RATE_PLACEHOLDER: u8 = 45;

#[derive(Debug, Clone, Default)]
pub struct CatchRateTable {
    /// Base form (form 0) by species name.
    by_name: HashMap<String, u8>,
}

/// Where a catch rate came from, shown in the battle log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatchRateSource {
    GameData,
    Placeholder,
}

impl CatchRateTable {
    pub fn load(data_root: impl AsRef<Path>) -> Result<Self, String> {
        let path = data_root.as_ref().join("pml_personal_extra.tsv");
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_tsv(&text)
    }

    pub fn from_tsv(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'));
        let header: Vec<&str> = lines
            .next()
            .ok_or("personal extra: missing header")?
            .split('\t')
            .map(|h| h.split(':').next().unwrap_or(h))
            .collect();
        let col = |name: &str| {
            header
                .iter()
                .position(|h| *h == name)
                .ok_or_else(|| format!("personal extra: missing column {name}"))
        };
        let (c_name, c_form, c_rate) = (col("species_name")?, col("form")?, col("catch_rate")?);
        let mut by_name = HashMap::new();
        for (n, line) in lines.enumerate() {
            let v: Vec<&str> = line.split('\t').collect();
            if v.len() != header.len() {
                return Err(format!("personal extra row {}: column count", n + 2));
            }
            if v[c_form] != "0" {
                continue;
            }
            let rate: u8 = v[c_rate]
                .parse()
                .map_err(|_| format!("personal extra row {}: bad catch rate", n + 2))?;
            by_name.insert(v[c_name].to_owned(), rate);
        }
        Ok(Self { by_name })
    }

    /// Catch rate of the species, or the placeholder (and its source) when unknown or zero.
    /// A zero rate marks a species with no record (the sheet keeps empty stubs at 0).
    pub fn lookup(&self, species: &str) -> (u8, CatchRateSource) {
        match self.by_name.get(species) {
            Some(&r) if r > 0 => (r, CatchRateSource::GameData),
            _ => (CATCH_PLACEHOLDER, CatchRateSource::Placeholder),
        }
    }

    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

const CATCH_PLACEHOLDER: u8 = CAPTURE_CATCH_RATE_PLACEHOLDER;

#[cfg(test)]
mod tests {
    use super::*;

    const SHEET: &str = "# sheet: x\ndex_index_national:u16\tform:u8\tspecies_name:string\tcatch_rate:u8\n399\t0\tbidoof\t255\n493\t0\tarceus\t3\n493\t1\tarceus\t99\n1\t0\tbulbasaur\t0\n";

    #[test]
    fn reads_real_rates_and_ignores_other_forms() {
        let t = CatchRateTable::from_tsv(SHEET).unwrap();
        assert_eq!(t.lookup("bidoof"), (255, CatchRateSource::GameData));
        assert_eq!(t.lookup("arceus"), (3, CatchRateSource::GameData));
    }

    #[test]
    fn unknown_or_zero_falls_back_to_placeholder() {
        let t = CatchRateTable::from_tsv(SHEET).unwrap();
        assert_eq!(t.lookup("bulbasaur"), (45, CatchRateSource::Placeholder));
        assert_eq!(t.lookup("nobody"), (45, CatchRateSource::Placeholder));
    }

    #[test]
    fn rejects_malformed_rows() {
        assert!(CatchRateTable::from_tsv("a\tb\n1\t2\n").is_err());
    }
}
