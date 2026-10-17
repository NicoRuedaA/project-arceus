//! Species/form -> exported Pokémon model path.
//!
//! The game's own model catalog (`bin/pokemon/table/poke_resource_table.trpmcatalog`, decoded
//! by `.tools/decode_poke_resource_table.py` into `data/pokemon_resource_table.tsv`) maps a
//! national dex number and form to a model pack such as `pm0058_00_41` (Hisuian Growlithe) or
//! `pm0841_00_00` (Rowlet). The pack name is NOT `pm{dex}_{form}_00`: regional forms use the
//! `_41` suffix, multi-form species start at `_11`, and species after #649 use developer model
//! numbers (Rowlet #722 is `pm0841`). Building the name from the dex number left those wild
//! creatures as grey placeholder spheres forever (play-test 2026-10-14).
//!
//! Raw catalog layout: VERIFIED by readback. Female-model flag: PROBABLE.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// `(dex, form)` -> pack stem for the default (non-female) model, plus female packs.
#[derive(Debug, Default, Clone)]
pub struct PokemonResourceTable {
    packs: HashMap<(u16, u16), String>,
    female_packs: HashMap<(u16, u16), String>,
}

impl PokemonResourceTable {
    /// Parses `data/pokemon_resource_table.tsv` (`species form female pack`).
    pub fn parse(text: &str) -> Self {
        let mut table = Self::default();
        for line in text.lines() {
            if line.starts_with('#') || line.starts_with("species:") || line.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 4 {
                continue;
            }
            let (Ok(dex), Ok(form)) = (f[0].parse::<u16>(), f[1].parse::<u16>()) else {
                continue;
            };
            let map = if f[2] == "1" { &mut table.female_packs } else { &mut table.packs };
            map.entry((dex, form)).or_insert_with(|| f[3].trim().to_owned());
        }
        table
    }

    pub fn load(path: &Path) -> Option<Self> {
        std::fs::read_to_string(path).ok().map(|t| Self::parse(&t))
    }

    pub fn len(&self) -> usize {
        self.packs.len() + self.female_packs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Pack stem of `(dex, form)`. Without a catalog row, form 0 falls back to the historical
    /// `pm{dex:04}_00_00` naming (true for every species up to #649); other forms have none.
    pub fn pack_stem(&self, dex: u16, form: u16) -> Option<String> {
        if let Some(p) = self.packs.get(&(dex, form)) {
            return Some(p.clone());
        }
        if let Some(p) = self.female_packs.get(&(dex, form)) {
            return Some(p.clone());
        }
        (form == 0 && dex <= 649).then(|| format!("pm{dex:04}_00_00"))
    }
}

/// The catalog, read once from `data/` (cwd, then the repository's `data/`).
pub fn global_table() -> &'static PokemonResourceTable {
    static TABLE: OnceLock<PokemonResourceTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        let candidates = [
            PathBuf::from("data/pokemon_resource_table.tsv"),
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/pokemon_resource_table.tsv"),
        ];
        for c in &candidates {
            if let Some(t) = PokemonResourceTable::load(c) {
                if !t.is_empty() {
                    return t;
                }
            }
        }
        eprintln!(
            "warning: data/pokemon_resource_table.tsv not found; Pokémon models use the \
             pm<dex>_00_00 naming only (regional forms and species after #649 stay placeholders)"
        );
        PokemonResourceTable::default()
    })
}

/// Cache roots holding `pokemon_batch/` (the asset server reads `work/cache`).
fn cache_roots() -> Vec<PathBuf> {
    [
        std::env::var("PLA_CACHE_DIR").map(PathBuf::from).ok(),
        Some(PathBuf::from("work/cache")),
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../work/cache")),
        Some(PathBuf::from("/home/nico/work/decompilacion/work/cache")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Asset path (relative to the `work/` asset root) of an exported pack's glTF, trying the
/// given clip variants in order (`idle`, `static`, `rare_idle`, ...).
pub fn batch_gltf_rel_for_stem(stem: &str, variants: &[&str]) -> Option<String> {
    for root in cache_roots() {
        for v in variants {
            if root.join(format!("pokemon_batch/{stem}/{stem}_{v}.gltf")).exists() {
                return Some(format!("cache/pokemon_batch/{stem}/{stem}_{v}.gltf"));
            }
        }
    }
    None
}

/// Exact model of `(dex, form, rare)`: never substitutes another form or palette.
pub fn resolve_batch_gltf_rel(dex: u16, form: u16, rare: bool) -> Option<String> {
    let stem = global_table().pack_stem(dex, form)?;
    let variants: &[&str] = if rare { &["rare_idle", "rare_static"] } else { &["idle", "static"] };
    batch_gltf_rel_for_stem(&stem, variants)
}

/// One row of `data/national/model_map.tsv` (converted user models).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NationalModelRow {
    pub model: String,
    pub species: u16,
    pub form: u8,
    pub kind: String,
    pub confidence: String,
}

/// `data/national/model_map.tsv`: national species/form -> converted model pack.
///
/// The sheet is produced by `.tools/build_model_map.py` from the user's model
/// collection (read-only) and the in-game PLA pack catalog; rows without an
/// identification (`DESCONOCIDO`) never resolve.
#[derive(Debug, Default, Clone)]
pub struct NationalModelMap {
    rows: Vec<NationalModelRow>,
}

impl NationalModelMap {
    pub fn parse(text: &str) -> Self {
        let mut rows = Vec::new();
        for line in text.lines() {
            if line.starts_with('#') || line.starts_with("model:") || line.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 6 {
                continue;
            }
            let (Ok(species), Ok(form)) = (f[1].parse::<u16>(), f[2].parse::<u8>()) else {
                continue;
            };
            rows.push(NationalModelRow {
                model: f[0].to_string(),
                species,
                form,
                kind: f[4].to_string(),
                confidence: f[5].to_string(),
            });
        }
        Self { rows }
    }

    pub fn load(path: &Path) -> Option<Self> {
        std::fs::read_to_string(path).ok().map(|t| Self::parse(&t))
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Converted model pack of `(species, form)`: an exact identified row, preferring a
    /// plain base model over colour variants. `DESCONOCIDO` rows never match.
    pub fn pack_for(&self, species: u16, form: u8) -> Option<&NationalModelRow> {
        let mut best: Option<&NationalModelRow> = None;
        for row in &self.rows {
            if row.species != species || row.form != form || row.confidence == "DESCONOCIDO" {
                continue;
            }
            let better = match best {
                None => true,
                Some(b) => {
                    let score = |k: &str| match k {
                        "base" => 3,
                        "regional" | "mega" | "gigamax" => 2,
                        "cosmetica" | "otro" => 1,
                        _ => 0,
                    };
                    score(&row.kind) > score(&b.kind)
                }
            };
            if better {
                best = Some(row);
            }
        }
        best
    }
}

/// The national model map, read once from `data/` (cwd, then the repository's `data/`).
pub fn global_national_map() -> &'static NationalModelMap {
    static MAP: OnceLock<NationalModelMap> = OnceLock::new();
    MAP.get_or_init(|| {
        let candidates = [
            PathBuf::from("data/national/model_map.tsv"),
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/national/model_map.tsv"),
        ];
        for c in &candidates {
            if let Some(m) = NationalModelMap::load(c) {
                if !m.is_empty() {
                    return m;
                }
            }
        }
        NationalModelMap::default()
    })
}

/// Asset path (relative to the `work/` asset root) of a converted national model: one glTF
/// that carries the skin, the textures and every clip (`cache/pokemon_national/<pack>/<pack>.gltf`).
pub fn national_gltf_rel(species: u16, form: u8) -> Option<String> {
    let row = global_national_map().pack_for(species, form)?;
    let rel = format!("cache/pokemon_national/{0}/{0}.gltf", row.model);
    for root in cache_roots() {
        if root.join(format!("pokemon_national/{0}/{0}.gltf", row.model)).exists() {
            return Some(rel);
        }
    }
    None
}

/// A model resolved for gameplay: the asset path plus its on-disk layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModel {
    /// Path relative to the `work/` asset root.
    pub rel: String,
    /// `true` for one-file national models (all clips inside); `false` for the PLA batch
    /// layout (`_idle` / `_walk` / `_battle` siblings).
    pub single_file: bool,
}

/// Game model for `(species, form)`: the game's own catalog (PLA) always wins; the converted
/// national models from the user collection fill the rest. `None` = grey placeholder.
pub fn resolve_model(species: u16, form: u8) -> Option<ResolvedModel> {
    let table = global_table();
    for f in [u16::from(form), 0] {
        if let Some(stem) = table.pack_stem(species, f) {
            if let Some(rel) = batch_gltf_rel_for_stem(&stem, &["idle", "static"]) {
                return Some(ResolvedModel { rel, single_file: false });
            }
        }
    }
    national_gltf_rel(species, form).map(|rel| ResolvedModel { rel, single_file: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn national_map_parses_and_prefers_base_rows() {
        let sheet = "# sheet: x\nmodel:string*\tspecies\tform_index\tform_name\tkind\tconfidence\treason\n\
             pm1015_00_00\t911\t0\tSkeledirge\tbase\tPROBABLE\ticono\n\
             pm1015_00_01\t911\t0\tSkeledirge\tvariante-color\tVERIFICADO\tbrillante\n\
             pm0099_00_00\t0\t0\t\totro\tDESCONOCIDO\tsin pista\n";
        let map = NationalModelMap::parse(sheet);
        assert_eq!(map.len(), 3);
        let row = map.pack_for(911, 0).expect("species 911");
        assert_eq!(row.model, "pm1015_00_00");
        assert_eq!(row.kind, "base");
        assert!(map.pack_for(0, 0).is_none(), "DESCONOCIDO never resolves");
        assert!(map.pack_for(911, 3).is_none(), "unknown form has no model");
    }

    #[test]
    fn national_map_keeps_unidentified_rows_in_the_sheet() {
        let map = NationalModelMap::parse("pm9999_00_00\t0\t0\t\totro\tDESCONOCIDO\tx\n");
        assert_eq!(map.len(), 1);
        assert!(map.pack_for(9999, 0).is_none());
    }

    fn repo_table() -> PokemonResourceTable {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/pokemon_resource_table.tsv");
        PokemonResourceTable::load(&p).expect("data/pokemon_resource_table.tsv")
    }

    #[test]
    fn catalog_maps_regional_forms_and_late_species() {
        let t = repo_table();
        assert_eq!(t.len(), 376);
        assert_eq!(t.pack_stem(25, 0).as_deref(), Some("pm0025_00_00"));
        assert_eq!(t.pack_stem(58, 1).as_deref(), Some("pm0058_00_41")); // Hisuian Growlithe
        assert_eq!(t.pack_stem(211, 1).as_deref(), Some("pm0211_00_41")); // Hisuian Qwilfish
        assert_eq!(t.pack_stem(422, 0).as_deref(), Some("pm0422_11_00")); // Shellos west
        assert_eq!(t.pack_stem(550, 2).as_deref(), Some("pm0550_13_41")); // white-striped Basculin
        assert_eq!(t.pack_stem(722, 0).as_deref(), Some("pm0841_00_00")); // Rowlet
        assert_eq!(t.pack_stem(704, 0).as_deref(), Some("pm0763_00_00")); // Goomy
        assert_eq!(t.pack_stem(712, 0).as_deref(), Some("pm0749_00_00")); // Bergmite
    }

    #[test]
    fn missing_rows_fall_back_only_for_old_species_form_zero() {
        let t = PokemonResourceTable::parse("species:u16*\tform\tfemale\tpack\n");
        assert_eq!(t.pack_stem(41, 0).as_deref(), Some("pm0041_00_00"));
        assert_eq!(t.pack_stem(41, 1), None);
        assert_eq!(t.pack_stem(722, 0), None);
    }

    /// Every wild spawn slot of the five wild areas resolves to an exported model in the real
    /// cache (skipped when the cache is not installed). Before the catalog, 22 rows of area 01
    /// (Hisuian Qwilfish, Shellos, Gastrodon, Shaymin, Landorus) had no model.
    #[test]
    fn every_wild_spawn_slot_has_an_exported_model() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../work/cache/pokemon_batch");
        let cache = std::env::var("PLA_CACHE_ROOT")
            .map(|r| PathBuf::from(r).join("pokemon_batch"))
            .ok()
            .filter(|p| p.is_dir())
            .unwrap_or(cache);
        if !cache.is_dir() {
            eprintln!("[skip] no pokemon_batch cache at {}", cache.display());
            return;
        }
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/field_spawn_table.tsv");
        let mut missing = Vec::new();
        let mut checked = 0;
        for area in 1..=5u8 {
            let spawners = crate::game::wild::load_spawners_for_area(&data, area).expect("spawn table");
            for s in &spawners {
                for e in &s.entries {
                    checked += 1;
                    let stem = global_table().pack_stem(e.species_id, e.form);
                    let ok = stem.as_ref().is_some_and(|stem| {
                        ["idle", "static"]
                            .iter()
                            .any(|v| cache.join(format!("{stem}/{stem}_{v}.gltf")).exists())
                    });
                    if !ok {
                        missing.push(format!(
                            "area {area:02} {} #{} form {} -> {stem:?}",
                            e.species_name, e.species_id, e.form
                        ));
                    }
                }
            }
        }
        assert!(checked > 1000, "only {checked} spawn slots read");
        assert!(missing.is_empty(), "{} slots without a model:\n{}", missing.len(), missing.join("\n"));
    }
}
