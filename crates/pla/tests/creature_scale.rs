//! Creature scale guards: the exported glTF models are already in metres and
//! match the species heights, so a normal wild creature must be drawn at 1.0x
//! (S/L are uncommon), and the alpha size (LL) only for real alpha slots.
//! Model checks skip (with a message) when the git-ignored export cache is absent.

use std::path::{Path, PathBuf};

use pla::game::wild::{load_spawners_for_area, normal_size_scale, roll_spawner_variant};

fn work_dir() -> Option<PathBuf> {
    let mut c = vec![PathBuf::from("work"), PathBuf::from("../../work")];
    if let Ok(p) = std::env::var("PLA_CACHE_ROOT") {
        c.insert(0, PathBuf::from(p).join(".."));
    }
    c.push(PathBuf::from("/home/nico/work/decompilacion/work"));
    c.into_iter().find(|p| p.join("cache/pokemon_batch").is_dir())
}

fn spawn_table() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/field_spawn_table.tsv")
}

/// Y extent (metres) of the idle glTF: POSITION accessor boxes through node matrices.
fn gltf_height(path: &Path) -> f64 {
    let g: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let nodes = g["nodes"].as_array().unwrap();
    let mut parent = vec![usize::MAX; nodes.len()];
    for (i, n) in nodes.iter().enumerate() {
        for c in n["children"].as_array().into_iter().flatten() {
            parent[c.as_u64().unwrap() as usize] = i;
        }
    }
    let local = |n: &serde_json::Value| -> [[f64; 4]; 4] {
        let f = |k: &str, d: &[f64]| -> Vec<f64> {
            n[k].as_array()
                .map(|a| a.iter().map(|v| v.as_f64().unwrap()).collect())
                .unwrap_or_else(|| d.to_vec())
        };
        let mut m = [[0.0; 4]; 4];
        if let Some(a) = n["matrix"].as_array() {
            for (i, v) in a.iter().enumerate() {
                m[i % 4][i / 4] = v.as_f64().unwrap();
            }
            return m;
        }
        let (t, r, s) = (f("translation", &[0.0; 3]), f("rotation", &[0.0, 0.0, 0.0, 1.0]), f("scale", &[1.0; 3]));
        let (x, y, z, w) = (r[0], r[1], r[2], r[3]);
        let rot = [
            [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - z * w), 2.0 * (x * z + y * w)],
            [2.0 * (x * y + z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - x * w)],
            [2.0 * (x * z - y * w), 2.0 * (y * z + x * w), 1.0 - 2.0 * (x * x + y * y)],
        ];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] = rot[i][j] * s[j];
            }
            m[i][3] = t[i];
        }
        m[3][3] = 1.0;
        m
    };
    let mul = |a: [[f64; 4]; 4], b: [[f64; 4]; 4]| {
        let mut o = [[0.0; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                o[i][j] = (0..4).map(|k| a[i][k] * b[k][j]).sum();
            }
        }
        o
    };
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for (i, n) in nodes.iter().enumerate() {
        let Some(mesh) = n["mesh"].as_u64() else { continue };
        let mut w = local(n);
        let mut p = parent[i];
        while p != usize::MAX {
            w = mul(local(&nodes[p]), w);
            p = parent[p];
        }
        for prim in g["meshes"][mesh as usize]["primitives"].as_array().unwrap() {
            let a = &g["accessors"][prim["attributes"]["POSITION"].as_u64().unwrap() as usize];
            let (mn, mx) = (a["min"].as_array().unwrap(), a["max"].as_array().unwrap());
            for corner in 0..8 {
                let pick = |k: usize| mn[k].as_f64().unwrap() * f64::from(corner >> k & 1 == 0)
                    + mx[k].as_f64().unwrap() * f64::from(corner >> k & 1 == 1);
                let (px, py, pz) = (pick(0), pick(1), pick(2));
                let y = w[1][0] * px + w[1][1] * py + w[1][2] * pz + w[1][3];
                lo = lo.min(y);
                hi = hi.max(y);
            }
        }
    }
    hi - lo
}

#[test]
fn rendered_model_height_matches_species_height() {
    let Some(work) = work_dir() else {
        eprintln!("skip: work/cache/pokemon_batch not present");
        return;
    };
    // (stem, species, SUPUESTO reference height in metres: public Pokédex values;
    // the repository's personal table carries no height column).
    let species = [
        ("pm0025_00_00", "Pikachu", 0.4),
        ("pm0041_00_00", "Zubat", 0.8),
        ("pm0093_00_00", "Haunter", 1.6),
        ("pm0217_00_00", "Ursaring", 1.8),
        ("pm0487_11_00", "Giratina", 4.5),
        ("pm0399_00_00", "Bidoof", 0.5),
        ("pm0403_00_00", "Shinx", 0.5),
        ("pm0396_00_00", "Starly", 0.3),
        ("pm0035_00_00", "Clefairy", 0.6),
        ("pm0037_00_00", "Vulpix", 0.6),
        ("pm0143_00_00", "Snorlax", 2.1),
        ("pm0134_00_00", "Vaporeon", 1.0),
    ];
    let mut checked = 0;
    for (stem, name, data_h) in species {
        let p = work.join(format!("cache/pokemon_batch/{stem}/{stem}_idle.gltf"));
        if !p.is_file() {
            continue;
        }
        // A normal (M, 1.0x) creature is drawn at the model height unchanged.
        let rendered = gltf_height(&p) * f64::from(normal_size_scale(50));
        let ratio = rendered / data_h;
        assert!(
            (0.6..=1.4).contains(&ratio),
            "{name}: rendered {rendered:.2} m vs data {data_h} m (ratio {ratio:.2})"
        );
        checked += 1;
    }
    assert!(checked >= 10, "only {checked} species had an exported model");
}

#[test]
fn normal_creatures_are_mostly_medium() {
    let medium = (0..100u64).filter(|r| normal_size_scale(*r) == 1.0).count();
    assert!(medium >= 80, "medium share {medium}%");
    assert!((0..100u64).all(|r| normal_size_scale(r) <= 1.8));
}

#[test]
fn gated_alpha_slot_does_not_override_the_normal_slot() {
    // Regression guard of the playtest bug: a spawner that owns a normal slot (weight 100) plus
    // a gated random-alpha slot (weight 1-2 in the decoded tables) kept only one row per species,
    // so the alpha won and nearly every creature spawned alpha. The loader keeps both rows now
    // (the game rolls one slot by weight); on the decoded tables the alpha rows are never
    // heavier, so the common roll is the normal creature.
    let mut pairs = 0usize;
    for area in 1..=5u8 {
        let spawners = load_spawners_for_area(spawn_table(), area).expect("spawn table");
        for s in &spawners {
            let mut species: Vec<u16> = s.entries.iter().map(|e| e.species_id).collect();
            species.sort_unstable();
            species.dedup();
            for id in species {
                let rows: Vec<_> = s.entries.iter().filter(|e| e.species_id == id).collect();
                let alpha: Vec<u16> = rows.iter().filter(|e| e.alpha).map(|e| e.weight).collect();
                let normal: Vec<u16> =
                    rows.iter().filter(|e| !e.alpha).map(|e| e.weight).collect();
                if alpha.is_empty() || normal.is_empty() {
                    continue;
                }
                pairs += 1;
                assert!(
                    alpha.iter().max() <= normal.iter().min(),
                    "spawner {}: species {id} alpha weights {alpha:?} vs normal {normal:?}",
                    s.spawner_id
                );
            }
        }
    }
    assert!(pairs >= 100, "only {pairs} (spawner, species) pairs carry both rows");
}

#[test]
fn real_spawn_data_is_not_mostly_alpha() {
    // The loader keeps every slot of a species (a normal row plus its alpha rows), so the share
    // of alpha ENTRIES is large by design. What must stay small is the alpha share of the rolled
    // WEIGHT (`alpha weight / total weight`, the decoded ~1-4 %). Fixed-alpha spawners and
    // spawners whose every slot is an alpha are 100 % alpha by design and are skipped.
    for area in 1..=5u8 {
        let spawners = load_spawners_for_area(spawn_table(), area).expect("spawn table");
        let mut total = 0u64;
        let mut alpha = 0u64;
        for s in &spawners {
            if s.entries.iter().any(|e| e.unique_alpha) || !s.entries.iter().any(|e| !e.alpha) {
                continue;
            }
            for e in &s.entries {
                total += u64::from(e.weight);
                if e.alpha && !e.unique_alpha {
                    alpha += u64::from(e.weight);
                }
            }
        }
        assert!(total > 0, "area {area} has no ordinary spawner");
        assert!(
            alpha * 10 <= total,
            "area {area}: alpha weight {alpha}/{total}"
        );
    }
    // Area 05 Haunter spawners: the encounter shown in the playtest capture. The random alpha
    // Haunter exists with its decoded weight (~1 %): a rare alpha is legal now, a mostly-alpha
    // table is not.
    let spawners = load_spawners_for_area(spawn_table(), 5).unwrap();
    let mut rolls = 0u64;
    let mut alphas = 0u64;
    for s in spawners.iter().filter(|s| s.entries.iter().any(|e| e.species_id == 93)) {
        for seed in 0..20u64 {
            if let Some((93, _, _, v)) = roll_spawner_variant(s, seed) {
                rolls += 1;
                assert!(v.alpha || v.size_scale <= 1.8, "Haunter {} {:?}", s.spawner_id, v);
                if v.alpha {
                    alphas += 1;
                }
            }
        }
    }
    assert!(rolls > 0);
    assert!(
        alphas * 4 < rolls,
        "alpha Haunters {alphas}/{rolls} is not a rare roll"
    );
}
