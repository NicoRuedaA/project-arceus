//! Exporter regression guard for the model pipeline.
//!
//! The `pm0025` / `pm0026` packs (a fully skinned pair with clips) are the
//! canary for `assets::tr` and the glTF writers: whatever the blocked-model and
//! skeleton-less fallbacks change, these two must keep assembling, keep their
//! LOD0 meshes, keep their clips and export a skinned glTF with skin +
//! animation blocks. Fixtures are game content: the tests print `[skip]` and
//! pass when they are absent (see `tests/common/mod.rs`).
//!
//! The exported glTF goes to the system temp directory, never into the repo.

mod common;

use common::fixture;
use pla::assets::pokemon_model::{assemble_pack, write_skinned_lod_level, LayerBlendFormula};
use std::fs;

/// Assemble one pack and export its idle clip skinned. Returns early (skip)
/// when the fixture is not available.
fn skinned_export_guard(stem: &str) {
    let Some(bytes) = fixture(&format!("{stem}.gfpak")) else {
        return;
    };
    let dir = std::env::temp_dir().join("pla_model_export_guard");
    fs::create_dir_all(&dir).expect("temp guard dir");
    let pack_path = dir.join(format!("{stem}.gfpak"));
    fs::write(&pack_path, &bytes).expect("write fixture copy");
    let model = assemble_pack(&pack_path).expect("pack assembles");
    let lod0 = model
        .meshes
        .iter()
        .filter(|mesh| mesh.lod == Some(0))
        .count();
    assert!(
        lod0 >= 1,
        "{stem}: expected at least one LOD0 mesh, got {lod0}"
    );
    let clip = model
        .clips
        .iter()
        .find(|clip| clip.parsed.is_ok() && clip.name.contains("defaultwait"))
        .expect("a parsed idle clip");
    let out = dir.join(format!("{stem}_idle.gltf"));
    write_skinned_lod_level(
        &model,
        0,
        &clip.name,
        &out,
        Some(LayerBlendFormula::MaskLerp),
    )
    .expect("skinned LOD0 export");
    let gltf = fs::read_to_string(&out).expect("glTF is readable text");
    assert!(
        gltf.contains("\"skins\""),
        "{stem}: skinned glTF has no skin block"
    );
    assert!(
        gltf.contains("\"animations\""),
        "{stem}: skinned glTF has no animation block"
    );
    assert!(
        gltf.contains("\"POSITION\""),
        "{stem}: glTF has no positions"
    );
    let _ = fs::remove_file(&out);
    let _ = fs::remove_file(&pack_path);
}

#[test]
fn pm0025_skinned_export_regression_guard() {
    skinned_export_guard("pm0025_00_00");
}

#[test]
fn pm0026_skinned_export_regression_guard() {
    skinned_export_guard("pm0026_00_00");
}
