//! Icons the original UI draws at run time (the layouts only hold placeholder panes for them).
//!
//! The exporter writes one PNG per icon under the UI root (never in the repository):
//! - Pokémon: `png/res_pokemon/icon_pokemon_s/pokeicon_s_<dex:4>_<form:3>_<gender:3>_n_00000000_fn_<n|r>.png`
//!   (128×128; `_l` = 512×512). The name fields are read from the file names: dex and form
//!   VERIFIED against the species shown, gender / `n|r` (normal / rare colour) PROBABLE.
//! - Items: `png/res_item/icon_item/item_<id:4>.png` (the game's item id, the same ids the port's
//!   bag uses; PROBABLE: checked on a few items only).
//! - Types: `png/icon_type/arc/type_dummy_tch/typeIcon_<NN>_<name>_00^t.png` (NN = the game's
//!   type order, Normal 00 ... Fairy 17, the same order as `battle::TypeChart`).

use std::collections::HashMap;
use std::path::Path;

use bevy::prelude::*;

use super::scene::ExternalImage;

const POKE_DIR: &str = "png/res_pokemon/icon_pokemon_s";
const ITEM_DIR: &str = "png/res_item/icon_item";
const TYPE_DIR: &str = "png/icon_type/arc/type_dummy_tch";

/// The game's type order (index = icon number).
pub const TYPE_ORDER: [&str; 18] = [
    "Normal", "Fighting", "Flying", "Poison", "Ground", "Rock", "Bug", "Ghost", "Steel", "Fire",
    "Water", "Grass", "Electric", "Psychic", "Ice", "Dragon", "Dark", "Fairy",
];

pub fn type_index(name: &str) -> Option<usize> {
    TYPE_ORDER.iter().position(|t| t.eq_ignore_ascii_case(name))
}

/// Icon files found under the UI root (scanned once).
#[derive(Resource, Debug, Default)]
pub struct IconIndex {
    scanned: bool,
    /// (dex, form) -> path relative to the UI root (gender 000 and normal colour preferred).
    pokemon: HashMap<(u16, u16), String>,
    items: HashMap<u16, String>,
    types: HashMap<usize, String>,
}

/// Parse `pokeicon_s_0025_001_000_n_00000000_fn_n.png` into (dex, form, gender, rare).
pub fn parse_pokeicon(name: &str) -> Option<(u16, u16, u16, bool)> {
    let stem = name.strip_suffix(".png")?;
    let parts: Vec<&str> = stem.split('_').collect();
    if parts.len() < 9 || parts[0] != "pokeicon" {
        return None;
    }
    let dex = parts[2].parse().ok()?;
    let form = parts[3].parse().ok()?;
    let gender = parts[4].parse().ok()?;
    let rare = parts.last() == Some(&"r");
    Some((dex, form, gender, rare))
}

impl IconIndex {
    /// Scan the icon folders of `root` (missing folders simply give no icons).
    pub fn scan(&mut self, root: &Path) {
        if self.scanned {
            return;
        }
        self.scanned = true;
        if let Ok(rd) = std::fs::read_dir(root.join(POKE_DIR)) {
            let mut best: HashMap<(u16, u16), (u16, String)> = HashMap::new();
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let Some((dex, form, gender, rare)) = parse_pokeicon(&name) else {
                    continue;
                };
                if rare {
                    continue;
                }
                let rel = format!("{POKE_DIR}/{name}");
                let entry = best.entry((dex, form)).or_insert((gender, rel.clone()));
                if gender < entry.0 {
                    *entry = (gender, rel);
                }
            }
            self.pokemon = best.into_iter().map(|(k, (_, p))| (k, p)).collect();
        }
        if let Ok(rd) = std::fs::read_dir(root.join(ITEM_DIR)) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if let Some(id) = name
                    .strip_prefix("item_")
                    .and_then(|s| s.strip_suffix(".png"))
                    .and_then(|s| s.parse().ok())
                {
                    self.items.insert(id, format!("{ITEM_DIR}/{name}"));
                }
            }
        }
        if let Ok(rd) = std::fs::read_dir(root.join(TYPE_DIR)) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if let Some(rest) = name.strip_prefix("typeIcon_") {
                    if let Some(n) = rest.get(..2).and_then(|s| s.parse::<usize>().ok()) {
                        self.types.insert(n, format!("{TYPE_DIR}/{name}"));
                    }
                }
            }
        }
        info!(
            "original UI icons: {} Pokémon, {} items, {} types",
            self.pokemon.len(),
            self.items.len(),
            self.types.len()
        );
    }

    /// Pokémon icon (form 0 when the form has no icon of its own).
    pub fn pokemon(&self, dex: u16, form: u16) -> Option<ExternalImage> {
        let p = self
            .pokemon
            .get(&(dex, form))
            .or_else(|| self.pokemon.get(&(dex, 0)))?;
        Some(ExternalImage {
            png: p.clone(),
            uv: [0.0, 0.0, 1.0, 1.0],
        })
    }

    pub fn item(&self, id: u16) -> Option<ExternalImage> {
        Some(ExternalImage {
            png: self.items.get(&id)?.clone(),
            uv: [0.0, 0.0, 1.0, 1.0],
        })
    }

    pub fn type_icon(&self, type_name: &str) -> Option<ExternalImage> {
        let p = self.types.get(&type_index(type_name)?)?;
        Some(ExternalImage {
            png: p.clone(),
            uv: [0.0, 0.0, 1.0, 1.0],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_icon_file_names() {
        assert_eq!(
            parse_pokeicon("pokeicon_s_0025_001_000_n_00000000_fn_n.png"),
            Some((25, 1, 0, false))
        );
        assert_eq!(
            parse_pokeicon("pokeicon_s_0722_000_000_n_00000000_fn_r.png"),
            Some((722, 0, 0, true))
        );
        assert_eq!(parse_pokeicon("item_0017.png"), None);
        assert_eq!(type_index("fire"), Some(9));
        assert_eq!(type_index("Fairy"), Some(17));
    }

    #[test]
    fn scan_of_a_synthetic_root_prefers_gender_0_and_normal_colour() {
        let root = std::env::temp_dir().join(format!("pla-icons-{}", std::process::id()));
        let poke = root.join(POKE_DIR);
        std::fs::create_dir_all(&poke).unwrap();
        std::fs::create_dir_all(root.join(ITEM_DIR)).unwrap();
        for f in [
            "pokeicon_s_0025_000_001_n_00000000_fn_n.png",
            "pokeicon_s_0025_000_000_n_00000000_fn_n.png",
            "pokeicon_s_0025_000_000_n_00000000_fn_r.png",
        ] {
            std::fs::write(poke.join(f), b"").unwrap();
        }
        std::fs::write(root.join(ITEM_DIR).join("item_0017.png"), b"").unwrap();
        let mut idx = IconIndex::default();
        idx.scan(&root);
        assert!(idx
            .pokemon(25, 0)
            .unwrap()
            .png
            .ends_with("pokeicon_s_0025_000_000_n_00000000_fn_n.png"));
        assert!(
            idx.pokemon(25, 3).is_some(),
            "unknown form falls back to form 0"
        );
        assert!(idx.pokemon(26, 0).is_none());
        assert!(idx.item(17).is_some());
        let _ = std::fs::remove_dir_all(&root);
    }
}
