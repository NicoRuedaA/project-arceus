//! Icons the original UI draws at run time (the layouts only hold placeholder panes for them).
//!
//! The exporter writes one PNG per icon under the UI root (never in the repository):
//! - Pokémon: `png/res_pokemon/icon_pokemon_s/pokeicon_s_<dex:4>_<form:3>_<gender:3>_n_00000000_fn_<n|r>.png`
//!   (128×128; `_l` = 512×512). The name fields are read from the file names: dex and form
//!   VERIFIED against the species shown, gender / `n|r` (normal / rare colour) PROBABLE.
//! - Items: `png/res_item/icon_item/item_<id:4>.png` (the game's item id, the same ids the port's
//!   bag uses; PROBABLE: checked on a few items only); Poké Balls `png/icon_ball/icon_ball_<id:4>.png`;
//!   items without either use the game's `item_dummy` placeholder.
//! - Type badges with the name: `png/icon_type/arc/type_dummy_eng/typeIconText_<NN>_<name>_00^u.png`.
//! - Types: `png/icon_type/arc/type_dummy_tch/typeIcon_<NN>_<name>_00^t.png` (NN = the game's
//!   type order, Normal 00 ... Fairy 17, the same order as `battle::TypeChart`).

use std::collections::HashMap;
use std::path::Path;

use bevy::prelude::*;

use super::scene::ExternalImage;

const POKE_DIR: &str = "png/res_pokemon/icon_pokemon_s";
const POKE_L_DIR: &str = "png/res_pokemon/icon_pokemon_l";
const ITEM_DIR: &str = "png/res_item/icon_item";
/// Poké Ball icons by item id: `icon_ball_<id:4>.png` (the bag's ball pocket).
const BALL_DIR: &str = "png/icon_ball";
/// The game's placeholder for an item without its own icon.
const ITEM_DUMMY: &str = "png/res_item/icon_item_dummy/item_dummy.png";
const TYPE_DIR: &str = "png/icon_type/arc/type_dummy_tch";
/// Type badges with the type name drawn in (English pack): `typeIconText_<NN>_<name>_00^u.png`.
const TYPE_TEXT_DIR: &str = "png/icon_type/arc/type_dummy_eng";

/// The game's type order (index = icon number).
pub const TYPE_ORDER: [&str; 18] = [
    "Normal", "Fighting", "Flying", "Poison", "Ground", "Rock", "Bug", "Ghost", "Steel", "Fire",
    "Water", "Grass", "Electric", "Psychic", "Ice", "Dragon", "Dark", "Fairy",
];

/// Tint per type for panes the game colours at run time from data the port does not have
/// (the port's own approximation of the usual type colours, 0-1 RGBA).
pub fn type_tint(name: &str) -> [f32; 4] {
    const T: [[f32; 3]; 18] = [
        [0.66, 0.65, 0.48], // Normal
        [0.76, 0.19, 0.16], // Fighting
        [0.66, 0.56, 0.95], // Flying
        [0.64, 0.24, 0.63], // Poison
        [0.89, 0.75, 0.40], // Ground
        [0.71, 0.63, 0.21], // Rock
        [0.65, 0.73, 0.10], // Bug
        [0.45, 0.34, 0.59], // Ghost
        [0.72, 0.72, 0.81], // Steel
        [0.93, 0.51, 0.19], // Fire
        [0.39, 0.56, 0.94], // Water
        [0.48, 0.78, 0.30], // Grass
        [0.97, 0.82, 0.17], // Electric
        [0.98, 0.33, 0.53], // Psychic
        [0.59, 0.85, 0.84], // Ice
        [0.44, 0.21, 0.99], // Dragon
        [0.44, 0.34, 0.27], // Dark
        [0.84, 0.52, 0.68], // Fairy
    ];
    let c = type_index(name).map_or([0.4, 0.45, 0.5], |i| T[i]);
    [c[0], c[1], c[2], 1.0]
}

pub fn type_index(name: &str) -> Option<usize> {
    TYPE_ORDER.iter().position(|t| t.eq_ignore_ascii_case(name))
}

/// Icon files found under the UI root (scanned once).
#[derive(Resource, Debug, Default)]
pub struct IconIndex {
    scanned: bool,
    /// (dex, form) -> path relative to the UI root (gender 000 and normal colour preferred).
    pokemon: HashMap<(u16, u16), String>,
    /// The same for the 512 x 512 pictures.
    pokemon_large: HashMap<(u16, u16), String>,
    items: HashMap<u16, String>,
    types: HashMap<usize, String>,
    type_texts: HashMap<usize, String>,
    item_dummy: Option<String>,
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

/// (dex, form) -> icon path in `dir` (gender 000 and the normal colour preferred).
fn scan_pokemon(root: &Path, dir: &str) -> HashMap<(u16, u16), String> {
    let Ok(rd) = std::fs::read_dir(root.join(dir)) else {
        return HashMap::new();
    };
    let mut best: HashMap<(u16, u16), (u16, String)> = HashMap::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some((dex, form, gender, rare)) = parse_pokeicon(&name) else {
            continue;
        };
        if rare {
            continue;
        }
        let rel = format!("{dir}/{name}");
        let entry = best.entry((dex, form)).or_insert((gender, rel.clone()));
        if gender < entry.0 {
            *entry = (gender, rel);
        }
    }
    best.into_iter().map(|(k, (_, p))| (k, p)).collect()
}

impl IconIndex {
    /// Scan the icon folders of `root` (missing folders simply give no icons).
    pub fn scan(&mut self, root: &Path) {
        if self.scanned {
            return;
        }
        self.scanned = true;
        self.pokemon = scan_pokemon(root, POKE_DIR);
        self.pokemon_large = scan_pokemon(root, POKE_L_DIR);
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
        if let Ok(rd) = std::fs::read_dir(root.join(BALL_DIR)) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if let Some(id) = name
                    .strip_prefix("icon_ball_")
                    .and_then(|s| s.strip_suffix(".png"))
                    .and_then(|s| s.parse().ok())
                {
                    self.items
                        .entry(id)
                        .or_insert_with(|| format!("{BALL_DIR}/{name}"));
                }
            }
        }
        // The exported placeholder is a blank 2 x 2 texture in some dumps: only a real picture
        // is used.
        self.item_dummy = std::fs::metadata(root.join(ITEM_DUMMY))
            .is_ok_and(|m| m.len() > 256)
            .then(|| ITEM_DUMMY.to_string());
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
        if let Ok(rd) = std::fs::read_dir(root.join(TYPE_TEXT_DIR)) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if let Some(rest) = name.strip_prefix("typeIconText_") {
                    if let Some(n) = rest.get(..2).and_then(|s| s.parse::<usize>().ok()) {
                        self.type_texts.insert(n, format!("{TYPE_TEXT_DIR}/{name}"));
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

    /// 512 x 512 Pokemon picture (form 0 fallback).
    pub fn pokemon_large(&self, dex: u16, form: u16) -> Option<ExternalImage> {
        let p = self
            .pokemon_large
            .get(&(dex, form))
            .or_else(|| self.pokemon_large.get(&(dex, 0)))?;
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

    /// Type badge with the type's name (falls back to the plain type icon).
    /// Item icon, else the game's placeholder item icon.
    pub fn item_or_dummy(&self, id: u16) -> Option<ExternalImage> {
        self.item(id).or_else(|| {
            Some(ExternalImage {
                png: self.item_dummy.clone()?,
                uv: [0.0, 0.0, 1.0, 1.0],
            })
        })
    }

    pub fn type_text(&self, type_name: &str) -> Option<ExternalImage> {
        match self.type_texts.get(&type_index(type_name)?) {
            Some(p) => Some(ExternalImage {
                png: p.clone(),
                uv: [0.0, 0.0, 1.0, 1.0],
            }),
            None => self.type_icon(type_name),
        }
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
