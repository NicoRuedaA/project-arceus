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

/// Form value for a Mega Evolution model (port extension, not a game form): the export
/// `pokemon_batch/pmNNNN_51_00/pmNNNN_51_00_idle.gltf`, converted locally from another
/// game's model. Absent unless the user converted it; the battle screen then shows a cube.
pub const MEGA_FORM: u16 = 0xFF51;

/// Stem of the local Mega Evolution export of `dex`.
pub fn mega_stem(dex: u16) -> String {
    format!("pm{dex:04}_51_00")
}

/// A Mega Evolution model of `dex` was converted into the cache.
pub fn mega_model_available(dex: u16) -> bool {
    batch_gltf_rel_for_stem(&mega_stem(dex), &["idle"]).is_some()
}

/// Form value for a Gigantamax model (port extension, not a game form): the export
/// `pokemon_batch/pmNNNN_81_00/pmNNNN_81_00_idle.gltf`, converted locally from another game's
/// model (`.tools/convert_mega_models.py --form 81`). Shown while such a Pokemon is Dynamaxed.
pub const GIGANTAMAX_FORM: u16 = 0xFF81;

/// Stem of the local Gigantamax export of `dex`.
pub fn gigantamax_stem(dex: u16) -> String {
    format!("pm{dex:04}_81_00")
}

/// A Gigantamax model of `dex` was converted into the cache.
pub fn gigantamax_model_available(dex: u16) -> bool {
    batch_gltf_rel_for_stem(&gigantamax_stem(dex), &["idle"]).is_some()
}

/// Exact model of `(dex, form, rare)`: never substitutes another form or palette.
pub fn resolve_batch_gltf_rel(dex: u16, form: u16, rare: bool) -> Option<String> {
    if form == MEGA_FORM {
        return batch_gltf_rel_for_stem(&mega_stem(dex), &["idle"]);
    }
    if form == GIGANTAMAX_FORM {
        return batch_gltf_rel_for_stem(&gigantamax_stem(dex), &["idle"]);
    }
    let stem = global_table().pack_stem(dex, form)?;
    let variants: &[&str] = if rare { &["rare_idle", "rare_static"] } else { &["idle", "static"] };
    batch_gltf_rel_for_stem(&stem, variants)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_table() -> PokemonResourceTable {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/pokemon_resource_table.tsv");
        PokemonResourceTable::load(&p).expect("data/pokemon_resource_table.tsv")
    }

    #[test]
    fn mega_form_uses_its_own_stem_never_a_game_form() {
        assert_eq!(mega_stem(94), "pm0094_51_00");
        assert_eq!(mega_stem(448), "pm0448_51_00");
        assert!(repo_table().pack_stem(94, MEGA_FORM).is_none(), "no game form collides with it");
        assert_eq!(gigantamax_stem(25), "pm0025_81_00");
        assert!(repo_table().pack_stem(25, GIGANTAMAX_FORM).is_none());
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
