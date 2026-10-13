//! Generate `data/field_collision_index.tsv`: every placed-object collision shape with the
//! full path that a GFLXPACK hash maps to it.
//!
//! Method (VERIFIED, reproducible): the `resident_release.gfpak` path manifest lists every
//! model file, including `bin/field/model/pack/<model>_col.trcol`. Hashing those strings with
//! FNV-1a 64 and looking the hash up in the two collision packs resolves the entry name. Shapes
//! whose path is not listed stay `unresolved` in the sheet (see the report).
//!
//! Usage: `collision_index [OUT_TSV]` (`PLA_BIN_ROOT` overrides the game root).

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use pla::game::world_collision::CollisionPacks;

fn bin_root() -> PathBuf {
    std::env::var_os("PLA_BIN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into())
}

/// Every printable `bin/...` run in a byte buffer (the manifest is FlatBuffers text data).
fn strings_with_prefix(bytes: &[u8], prefix: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + prefix.len() <= bytes.len() {
        if &bytes[i..i + prefix.len()] == prefix {
            let mut j = i;
            while j < bytes.len() && (0x20..0x7f).contains(&bytes[j]) {
                j += 1;
            }
            if j > i + prefix.len() {
                if let Ok(s) = std::str::from_utf8(&bytes[i..j]) {
                    out.push(s.to_string());
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

fn main() {
    let out_path = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "data/field_collision_index.tsv".into()),
    );
    let root = bin_root();

    let packs = CollisionPacks::load(&root).expect("collision packs");
    println!(
        "collision packs: {} ({} entries)",
        packs.packs.len(),
        packs.entry_count()
    );

    // Path manifest: decode every entry of resident_release.gfpak and keep the `.trcol` paths.
    let resident_path = root.join("archive/field/resident_release.gfpak");
    let mut trcol_paths: BTreeMap<u64, String> = BTreeMap::new();
    if resident_path.exists() {
        let bytes = std::fs::read(&resident_path).unwrap();
        let pak = pla::assets::gfpak::GfPak::parse(&bytes).expect("resident_release.gfpak");
        let mut seen: HashSet<String> = HashSet::new();
        for entry in &pak.entries {
            let Ok(data) = pak.decoded_data(&bytes, entry) else {
                continue;
            };
            for s in strings_with_prefix(&data, b"bin/") {
                if s.ends_with(".trcol") && seen.insert(s.clone()) {
                    trcol_paths.insert(pla::assets::gfpak::fnv1a64(s.as_bytes()), s);
                }
            }
        }
    }
    println!("manifest `.trcol` paths: {}", trcol_paths.len());

    // One row per collision entry of both packs.
    let mut rows: Vec<String> = Vec::new();
    let (mut resolved, mut unresolved, mut empty) = (0usize, 0usize, 0usize);
    let (mut total_tri, mut total_verts) = (0usize, 0usize);
    for pack in 0..packs.packs.len() {
        let name = packs.pack_name(pack).to_string();
        for entry in 0..packs.pack_entry_count(pack) {
            if !packs.is_collision_entry(pack, entry) {
                continue;
            }
            let hash = packs.entry_hashes(pack)[entry];
            let path = trcol_paths.get(&hash).cloned();
            let (model, status) = match &path {
                Some(p) => {
                    let base = p
                        .strip_prefix("bin/field/model/pack/")
                        .and_then(|s| s.strip_suffix("_col.trcol"))
                        .map(str::to_string);
                    match base {
                        Some(m) => (m, "ok"),
                        None => (String::new(), "unresolved"),
                    }
                }
                None => (String::new(), "unresolved"),
            };
            let mesh = packs.mesh(pack, entry).ok();
            let (verts, tris, secs) = mesh
                .as_ref()
                .map(|m| (m.vertices.len(), m.triangles.len(), m.section_count))
                .unwrap_or_default();
            if status == "ok" && tris > 0 {
                resolved += 1;
            } else if status == "ok" {
                empty += 1;
            } else {
                unresolved += 1;
            }
            total_tri += tris;
            total_verts += verts;
            rows.push(format!(
                "{name}\t{entry}\t{model}\t{}\t{status}\t{verts}\t{tris}\t{secs}",
                path.unwrap_or_default()
            ));
        }
    }

    let mut text = String::from(
        "# sheet: data/field_collision_index\n# version: 1\n# generator: crates/pla/examples/collision_index.rs\n# target: Pokémon Legends: Arceus v262144 collision shapes of placed field models (collision.gfpak, resident_bg_parts_model.gfpak)\n# note: `path` is the GFLXPACK entry name whose FNV-1a 64 hash equals the entry's stored hash; `model` is the visual model it belongs to.\n# note: `status` is `ok` (named + geometry), `empty` (named, no triangles) or `unresolved` (no manifest path).\npack:string*\tentry:u32\tmodel:string?\tpath:string?\tstatus:enum\tvertices:u32\ttriangles:u32\tsections:u32\n",
    );
    for r in &rows {
        text.push_str(r);
        text.push('\n');
    }
    std::fs::write(&out_path, text).unwrap();
    println!(
        "wrote {} rows to {}: {resolved} ok, {empty} named-empty, {unresolved} unresolved; {total_tri} triangles / {total_verts} vertices",
        rows.len(),
        out_path.display()
    );
}
