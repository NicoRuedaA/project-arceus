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
    // The collapse bug (fixed 2026-10-14, slots redesigned 2026-10-15): the weight-1 gated
    // random-alpha row replaced the weight-100 normal row of the same species by file order,
    // so most wild creatures spawned as 2.5x alphas. Since the redesign both rows survive and
    // the weighted roll (`roll_spawner_variant`) decides: a spawner owning a normal row and a
    // random-alpha row of the same species rolls the alpha rarely, never most of the time.
    let spawners = load_spawners_for_area(spawn_table(), 1).expect("spawn table");
    let mut checked = 0;
    for s in &spawners {
        // Fixed alphas legitimately own their spawner at weight 100: skip those.
        if s.entries.iter().any(|e| e.unique_alpha) {
            continue;
        }
        let owns_pair = s
            .entries
            .iter()
            .filter(|e| e.alpha && !e.unique_alpha)
            .any(|a| s.entries.iter().any(|n| !n.alpha && n.species_id == a.species_id));
        if !owns_pair {
            continue;
        }
        let rolls = 200u64;
        let alpha = (0..rolls)
            .filter(|seed| {
                roll_spawner_variant(s, *seed).is_some_and(|(_, _, _, v)| v.alpha)
            })
            .count();
        // The decoded random-alpha weights are 1-2 next to 100-weight normals (~1 %-2 %).
        assert!(
            alpha * 10 <= rolls as usize,
            "spawner {}: {alpha}/{rolls} alphas (the gated slot overrides the normal one)",
            s.spawner_id
        );
        checked += 1;
    }
    assert!(checked >= 10, "only {checked} spawners with a normal+random-alpha pair");
}

#[test]
fn real_spawn_data_is_not_mostly_alpha() {
    // Since the 2026-10-15 redesign the loader keeps one row per slot (normal and light alpha
    // coexist), so raw row counts overstate alpha; the weighted roll is what the player sees.
    // Roll a few seeds per spawner and count outcomes. Fixed alphas (unique_alpha, weight 100)
    // are rare and roll alpha by design; they sit inside the bounds below.
    for area in 1..=5u8 {
        let spawners = load_spawners_for_area(spawn_table(), area).expect("spawn table");
        let mut rolls = 0usize;
        let mut alpha = 0usize;
        for s in &spawners {
            for seed in 0..4u64 {
                if let Some((_, _, _, v)) = roll_spawner_variant(s, seed) {
                    rolls += 1;
                    alpha += usize::from(v.alpha);
                }
            }
        }
        assert!(rolls > 100, "area {area}: only {rolls} rolls");
        assert!(alpha * 10 <= rolls, "area {area}: {alpha}/{rolls} rolled alphas");
    }
    // Area 05 Haunter spawners: the encounter shown in the playtest capture. Its decoded
    // random-alpha slot (weight 1 next to 100-weight normals) keeps its light weight, so a
    // Haunter roll is almost always a normal-size creature; an alpha roll is the 2.5x one.
    let spawners = load_spawners_for_area(spawn_table(), 5).unwrap();
    let mut seen = 0;
    let mut alphas = 0;
    for s in spawners.iter().filter(|s| s.entries.iter().any(|e| e.species_id == 93)) {
        for seed in 0..20u64 {
            if let Some((93, _, _, v)) = roll_spawner_variant(s, seed) {
                seen += 1;
                alphas += usize::from(v.alpha);
                if v.alpha {
                    assert!(
                        (v.size_scale - 2.5).abs() < 1e-6,
                        "alpha Haunter must be 2.5x: {} {:?}",
                        s.spawner_id,
                        v
                    );
                } else {
                    assert!(v.size_scale <= 1.8, "Haunter {} {:?}", s.spawner_id, v);
                }
            }
        }
    }
    assert!(seen > 0);
    assert!(alphas * 10 <= seen, "Haunter alphas {alphas}/{seen}");
}
