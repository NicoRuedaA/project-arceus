//! Interior (sub-area) scenes: plan, load helpers and the missing-data fallback (Brief 37).
//!
//! Data ([`data/subzone_index.tsv`], see `reports/data-progress/subareas.md`): the 55 sub-areas
//! (`ha_areaNN_sXX[_y]`) are independent local scenes entered through the paired door records of
//! `data/warp_links.tsv`. Every sub-area has its own navmesh pack
//! (`bin/archive/field/terrain/<zone>.gfpak`), its own row set in
//! `data/field_object_placements.tsv` (the room shells and props) and an `env_set` naming the
//! decoded lighting rows that apply to it (`ha_area00_s00` for the Jubilife houses).
//!
//! This module is the data side of the interior load: parse the sub-zone index, decide what a
//! sub-area scene contains (rooms, navmesh, environment) and build the visible SUPUESTO fallback
//! when no room shell can be streamed (6 of 55 sub-areas; 12 have no `room_*.gltf` in their own
//! export folder and the rest are covered by the shared export index). The runtime swap itself
//! lives in [`crate::game::zone::apply_zone_change`], which calls into this module.
//!
//! Claims: the sub-zone index rows, pack paths, navmesh packs and placement rows are **VERIFIED**
//! against the decoded sheets; the room-shell placement transform is **PROBABLE** (rot_y degrees,
//! like the rest of the placement sheet); the fallback room dimensions come from the navmesh
//! bounds and are **SUPUESTO** port scaffolding, labelled as such in the HUD notice.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use crate::assets::bntx::{FloatTextureData, TextureData};
use crate::assets::terrain::{TerrainArea, TerrainMeshSettings};
use crate::game::world_objects::{
    find_data_file, model_paths_for_pack, model_paths_from_text, parse_placements_for_spawn,
    Placement,
};
use crate::game::zone::TerrainResource;

/// Decoded sub-zone index (one row per area record: 8 main + 55 sub-areas).
pub const SUBZONE_INDEX_SHEET: &str = "subzone_index.tsv";

/// Generated shell-coverage sheet (`data/subzone_shells.tsv`, `.tools/subzone_shells.py`):
/// per sub-area, whether the exported placed geometry covers its navmesh.
pub const SUBZONE_SHELLS_SHEET: &str = "subzone_shells.tsv";

/// Sentinel `TerrainArea::area` of the empty terrain installed while an interior is active: it
/// makes the `.trins` tree spawner drop the previous area's trees (its step reacts to the change)
/// and gives the height fallbacks nothing to sample, without removing the resource other systems
/// hold as `Option<Res<_>>`.
pub const INTERIOR_TERRAIN_AREA: u8 = 255;

/// One sub-zone index row (the columns this module uses).
#[derive(Debug, Clone, PartialEq)]
pub struct SubAreaRow {
    pub area: String,
    pub parent: String,
    /// Lighting environment this sub-area uses (`ha_area00_s00` for the Jubilife houses).
    pub env_set: String,
    pub spawn: Vec3,
    /// `own_terrain=1`: the sub-area carries its own heightmap (terrain naming not decoded).
    pub own_terrain: bool,
    pub model_pack: String,
}

/// Parse `data/subzone_index.tsv`, keeping the sub-area rows (parent != area).
pub fn parse_subzone_index(text: &str) -> Vec<SubAreaRow> {
    let mut cols: HashMap<&str, usize> = HashMap::new();
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let c: Vec<&str> = line.split('\t').collect();
        if cols.is_empty() {
            for (i, h) in c.iter().enumerate() {
                cols.insert(h.split(':').next().unwrap_or(h), i);
            }
            continue;
        }
        let get = |n: &str| cols.get(n).and_then(|&i| c.get(i)).copied().unwrap_or("");
        if get("area").is_empty() || get("parent").is_empty() || get("area") == get("parent") {
            continue;
        }
        let f = |n: &str, d: f32| get(n).parse::<f32>().unwrap_or(d);
        out.push(SubAreaRow {
            area: get("area").to_string(),
            parent: get("parent").to_string(),
            env_set: get("env_set").to_string(),
            spawn: Vec3::new(f("spawn_x", 0.0), f("spawn_y", 0.0), f("spawn_z", 0.0)),
            own_terrain: get("own_terrain") == "1",
            model_pack: get("model_pack").to_string(),
        });
    }
    out
}

/// Read and parse the real sub-zone index. Empty without the sheet.
pub fn load_subzone_index() -> Vec<SubAreaRow> {
    find_data_file(SUBZONE_INDEX_SHEET)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| parse_subzone_index(&t))
        .unwrap_or_default()
}

/// Shell coverage of one sub-area (row of [`SUBZONE_SHELLS_SHEET`]).
///
/// `covered` means the union of the placed exported geometry covers the sub-area navmesh box
/// (>= 95 % of each horizontal axis and reaching the navmesh's y band within 0.5 m), so the
/// scene has real walls/floor and needs no placeholder. `partial` = some geometry exists but it
/// does not cover the navmesh. `none` = no placed geometry at all. PROBABLE (box test; `rot_y`
/// is LIKELY sheet data), see `.tools/subzone_shells.py`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellStatus {
    Covered,
    Partial,
    None,
}

/// One parsed shell-coverage row.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellRow {
    pub area: String,
    pub models_ok: u16,
    pub coverage_x: f32,
    pub coverage_z: f32,
    pub shell: ShellStatus,
}

/// Parse `subzone_shells.tsv` (header comments skipped; unknown `shell` values are dropped).
pub fn parse_subzone_shells(text: &str) -> HashMap<String, ShellRow> {
    let mut cols: HashMap<&str, usize> = HashMap::new();
    let mut out = HashMap::new();
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let c: Vec<&str> = line.split('\t').collect();
        if cols.is_empty() {
            for (i, h) in c.iter().enumerate() {
                cols.insert(h.split(':').next().unwrap_or(h), i);
            }
            continue;
        }
        let get = |n: &str| cols.get(n).and_then(|&i| c.get(i)).copied().unwrap_or("");
        let area = get("area");
        if area.is_empty() {
            continue;
        }
        let shell = match get("shell") {
            "covered" => ShellStatus::Covered,
            "partial" => ShellStatus::Partial,
            "none" => ShellStatus::None,
            _ => continue,
        };
        out.insert(
            area.to_string(),
            ShellRow {
                area: area.to_string(),
                models_ok: get("models_ok").parse().unwrap_or(0),
                coverage_x: get("coverage_x").parse().unwrap_or(0.0),
                coverage_z: get("coverage_z").parse().unwrap_or(0.0),
                shell,
            },
        );
    }
    out
}

/// Read and parse the real shell-coverage sheet. Empty without the sheet.
pub fn load_subzone_shells() -> HashMap<String, ShellRow> {
    find_data_file(SUBZONE_SHELLS_SHEET)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| parse_subzone_shells(&t))
        .unwrap_or_default()
}

/// Export cache root (`$PLA_CACHE_ROOT`, else the main checkout's `work/cache`).
pub fn cache_root() -> PathBuf {
    std::env::var_os("PLA_CACHE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(crate::game::world_objects::DEFAULT_CACHE_ROOT))
}

/// True for an independent sub-area scene id (`ha_area00_s03_a`), false for main areas.
pub fn is_sub_area(zone: &str) -> bool {
    crate::game::zone::main_area_of(zone) != zone
}

/// Row of `zone` in a parsed index.
pub fn row_for<'a>(rows: &'a [SubAreaRow], zone: &str) -> Option<&'a SubAreaRow> {
    rows.iter().find(|r| r.area == zone)
}

/// What an interior scene load needs to know; every field is evidence-backed or explicitly
/// labelled in its doc.
#[derive(Debug, Clone, PartialEq)]
pub struct InteriorPlan {
    pub zone: String,
    pub parent: String,
    /// Zone whose lighting rows apply (the sub-area's own rows when present, else its env_set).
    pub env_zone: String,
    /// Decoded spawn of the sub-area record (fallback when the door arrival is off-mesh).
    pub spawn: Option<Vec3>,
    /// Absolute path of the sub-area navmesh pack, when present on disk.
    pub navmesh_pack: Option<PathBuf>,
    /// Placement models of this sub-area with an exported glTF on disk (rooms and props).
    pub geometry_models: Vec<String>,
    /// True when the scene has a real shell: the shell sheet marks it `covered`, or a
    /// streamable model is a `room_*` shell (the rule the sheet cannot replace for scenes whose
    /// room sits inside a bigger navmesh, e.g. the tents). False only when the visible SUPUESTO
    /// fallback is needed.
    pub has_room: bool,
    pub own_terrain: bool,
    /// Why the visible fallback room is needed (`None` = real geometry available).
    pub fallback: Option<String>,
    /// Data warning shown even when rooms load (e.g. `own_terrain` undecoded).
    pub notice: Option<String>,
}

/// Build the plan from the sheets (pure; the caller reads the files). `cache_root` resolves the
/// export-index paths, `bin_root` locates the navmesh pack, `shells` is the parsed
/// [`SUBZONE_SHELLS_SHEET`] (empty map = sheet unavailable, falls back to the `room_*` rule).
/// `None` without a matching index row.
pub fn interior_plan_from_sheets(
    zone: &str,
    rows: &[SubAreaRow],
    placements_text: &str,
    export_index_text: &str,
    shells: &HashMap<String, ShellRow>,
    cache_root: &Path,
    bin_root: &Path,
) -> Option<InteriorPlan> {
    let row = row_for(rows, zone)?;
    let env_zone = if row.env_set.is_empty() { zone.to_string() } else { row.env_set.clone() };
    let pack = bin_root.join("archive/field/terrain").join(format!("{zone}.gfpak"));
    let navmesh_pack = pack.is_file().then_some(pack);
    // Models this sub-area can draw: per-pack index rows first (each sub-area has its own
    // exported folder), global rows as fallback for shared models.
    let mut paths = model_paths_for_pack(export_index_text, cache_root, &format!("{zone}.gfpak"));
    for (k, v) in model_paths_from_text(export_index_text, cache_root) {
        paths.entry(k).or_insert(v);
    }
    let placements: Vec<Placement> = parse_placements_for_spawn(placements_text, zone)
        .into_iter()
        .filter(|p| paths.get(&p.model).is_some_and(|(f, _)| f.exists()))
        .collect();
    let mut geometry_models: Vec<String> = placements.iter().map(|p| p.model.clone()).collect();
    geometry_models.sort();
    geometry_models.dedup();
    let has_room_prefix = geometry_models.iter().any(|m| m.starts_with("room_"));
    let shell = shells.get(zone).map(|s| s.shell);
    let has_room = has_room_prefix || shell == Some(ShellStatus::Covered);
    let fallback = (!has_room).then(|| {
        match shell {
            Some(ShellStatus::None) | None => {
                "no placed geometry for this sub-area (its own terrain is the scene)".to_string()
            }
            Some(ShellStatus::Partial) if !geometry_models.is_empty() => {
                "streamable geometry does not cover the sub-area navmesh".to_string()
            }
            Some(ShellStatus::Partial) => "no streamable room shell for this sub-area".to_string(),
            Some(ShellStatus::Covered) => unreachable!("covered implies has_room"),
        }
    });
    let notice = row.own_terrain.then(|| {
        "sub-area carries its own terrain; the port loads its placed models, not the terrain \
         (tile naming decoded, loader not wired)"
            .to_string()
    });
    Some(InteriorPlan {
        zone: zone.to_string(),
        parent: row.parent.clone(),
        env_zone,
        spawn: Some(row.spawn),
        navmesh_pack,
        geometry_models,
        has_room,
        own_terrain: row.own_terrain,
        fallback,
        notice,
    })
}

fn read_export_index() -> Result<String, ()> {
    find_data_file("field_model_export_index.tsv")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .ok_or(())
}

/// [`interior_plan_from_sheets`] over the real sheets. `None` without a matching row.
pub fn interior_plan(zone: &str, cache_root: &Path, bin_root: &Path) -> Option<InteriorPlan> {
    let rows = load_subzone_index();
    let placements_text = find_data_file("field_object_placements.tsv")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    let export_index_text = read_export_index().unwrap_or_default();
    let shells = load_subzone_shells();
    interior_plan_from_sheets(
        zone,
        &rows,
        &placements_text,
        &export_index_text,
        &shells,
        cache_root,
        bin_root,
    )
}

/// Empty terrain installed while an interior is active (see [`INTERIOR_TERRAIN_AREA`]).
///
/// The heightmap is deliberately non-samplable (1x1: `sample_height` refuses it), so no height
/// fallback can use the previous area's ground while the interior navmesh is the only surface.
pub fn placeholder_terrain() -> TerrainResource {
    let empty_tex = |w: u32, h: u32| TextureData { width: w, height: h, rgba: Vec::new() };
    TerrainResource {
        area: std::sync::Arc::new(TerrainArea {
            area: INTERIOR_TERRAIN_AREA,
            source_path: PathBuf::new(),
            world_extent: [0.0, 0.0],
            world_height: 0.0,
            heightmap_brti_name: String::new(),
            navmesh_bounds: None,
            heightmap: FloatTextureData { width: 1, height: 1, samples: vec![0.0] },
            normal_map: empty_tex(0, 0),
            splat_alpha: empty_tex(0, 0),
            splat_alpha1: None,
            material: None,
            ground_textures: HashMap::new(),
            unresolved_ground_textures: Vec::new(),
            masks: Vec::new(),
            instances: Vec::new(),
        }),
        chunks: Vec::new(),
        blended_albedo: None,
        settings: TerrainMeshSettings { horizontal_cell_size: 1.0, vertical_scale: 100.0, chunk_cells: 64 },
    }
}

/// Marker of the visible SUPUESTO fallback room spawned when a sub-area has no exported shell.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct InteriorFallback;

/// Kind of missing data the fallback covers (drives the notice text and the placeholder shape).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackKind {
    /// No streamable geometry at all: floor slab and, when the scene is small, three walls too.
    EmptyScene,
    /// Props (ground pieces, furniture) but no room shell: a provisional floor slab only.
    NoRoomShell,
    /// The sub-area has its own terrain and that pack naming is not decoded: notice only.
    OwnTerrain,
}

/// Spawn the visible provisional floor of a fallback interior, sized to `bounds` (`min`, `max` of
/// the sub-area navmesh) and marked [`InteriorFallback`] so any later zone change despawns it.
/// Small scenes also get three walls so the placeholder reads as a room. All dimensions are
/// **SUPUESTO** scaffolding, not game data. No-op when the app has no mesh/material stores.
pub fn spawn_fallback_room(world: &mut World, bounds: ([f32; 3], [f32; 3]), kind: FallbackKind) {
    if kind == FallbackKind::OwnTerrain {
        return;
    }
    let (min, max) = bounds;
    // A whole-field navmesh (1024 m) gets no absurd wall ring: only the floor slab.
    let w = (max[0] - min[0]).abs().clamp(4.0, 256.0);
    let d = (max[2] - min[2]).abs().clamp(4.0, 256.0);
    let with_walls = kind == FallbackKind::EmptyScene && w <= 40.0 && d <= 40.0;
    let h = 3.0_f32;
    let cx = (min[0] + max[0]) * 0.5;
    let cz = (min[2] + max[2]) * 0.5;
    let y = min[1];
    let color = match kind {
        FallbackKind::EmptyScene => Color::srgb(0.42, 0.40, 0.36),
        FallbackKind::NoRoomShell | FallbackKind::OwnTerrain => Color::srgb(0.35, 0.42, 0.38),
    };
    let Some(mut materials) = world.get_resource_mut::<Assets<StandardMaterial>>() else {
        return;
    };
    let material = materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.9,
        cull_mode: None,
        ..default()
    });
    let Some(mut meshes) = world.get_resource_mut::<Assets<Mesh>>() else {
        return;
    };
    let mut parts: Vec<(Vec3, Vec3)> = Vec::new();
    // Floor (thin slab just under the navmesh), then three walls around it (door side open).
    parts.push((Vec3::new(w + 0.6, 0.2, d + 0.6), Vec3::new(cx, y - 0.1, cz)));
    if with_walls {
        parts.push((Vec3::new(w + 0.6, h, 0.3), Vec3::new(cx, y + h * 0.5, min[2] - 0.15)));
        parts.push((Vec3::new(w + 0.6, h, 0.3), Vec3::new(cx, y + h * 0.5, max[2] + 0.15)));
        parts.push((Vec3::new(0.3, h, d + 0.6), Vec3::new(min[0] - 0.15, y + h * 0.5, cz)));
    }
    let handles: Vec<(Handle<Mesh>, Vec3)> =
        parts.iter().map(|(size, pos)| (meshes.add(Cuboid::new(size.x, size.y, size.z)), *pos)).collect();
    drop(meshes);
    for (mesh, pos) in handles {
        world.spawn((
            InteriorFallback,
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(pos),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real_index() -> String {
        find_data_file(SUBZONE_INDEX_SHEET)
            .and_then(|p| std::fs::read_to_string(p).ok())
            .expect("data/subzone_index.tsv")
    }

    #[test]
    fn sub_area_ids() {
        assert!(is_sub_area("ha_area00_s03_a"));
        assert!(is_sub_area("ha_area05_s02"));
        assert!(!is_sub_area("ha_area00"));
        assert!(!is_sub_area("ha_area06"));
        assert!(!is_sub_area("nonsense"));
    }

    #[test]
    fn real_index_has_the_expected_rows() {
        let rows = parse_subzone_index(&real_index());
        assert_eq!(rows.len(), 55, "55 sub-area records");
        assert_eq!(row_for(&rows, "ha_area00_s03_a").unwrap().parent, "ha_area00");
        let s09 = row_for(&rows, "ha_area00_s09").unwrap();
        assert_eq!(s09.env_set, "ha_area00_s09");
        let s03 = row_for(&rows, "ha_area00_s03_a").unwrap();
        assert_eq!(s03.env_set, "ha_area00_s00");
        let s02 = row_for(&rows, "ha_area00_s02").unwrap();
        assert!(s02.own_terrain, "ha_area00_s02 carries its own terrain");
        assert!(!s03.own_terrain);
    }

    #[test]
    fn plan_uses_pack_geometry_and_env_set() {
        let cache_root = std::env::var_os("PLA_CACHE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/home/nico/work/decompilacion/work/cache"));
        let bin_root = PathBuf::from("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin");
        if !bin_root.join("archive/field/terrain/ha_area00_s03_a.gfpak").is_file() {
            eprintln!("[skip] game data not found");
            return;
        }
        let plan = interior_plan("ha_area00_s03_a", &cache_root, &bin_root).expect("a plan");
        assert_eq!(plan.parent, "ha_area00");
        assert_eq!(plan.env_zone, "ha_area00_s00");
        assert!(plan.navmesh_pack.is_some(), "sub-area navmesh pack present");
        assert!(plan.has_room, "room_area00_s03.gltf is exported for s03_a");
        assert!(plan.fallback.is_none());
        assert!(plan.geometry_models.iter().any(|m| m == "room_area00_s03"));
    }

    #[test]
    fn plan_flags_the_missing_shell_fallback() {
        let cache_root = std::env::var_os("PLA_CACHE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/home/nico/work/decompilacion/work/cache"));
        let bin_root = PathBuf::from("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin");
        if !bin_root.join("archive/field/terrain/ha_area05_s02.gfpak").is_file() {
            eprintln!("[skip] game data not found");
            return;
        }
        // s02 of area 00 has its own terrain but rooms are exported (notice only, no placeholder);
        // s02 of area 03 places a room model exported from the main area pack (room streams);
        // s02 of area 05 has no placements at all (empty-scene fallback);
        // s02 of area 04 is a cave whose exported shell (`ground_area04_cave03`) covers the
        // navmesh, so the shell sheet marks it `covered` and no placeholder is drawn.
        let own = interior_plan("ha_area00_s02", &cache_root, &bin_root).expect("plan s02");
        assert!(own.own_terrain);
        assert!(own.has_room, "s02 exports its rooms");
        assert!(own.notice.is_some(), "own-terrain sub-area carries a warning");
        assert!(own.fallback.is_none(), "rooms exist: no placeholder room");

        let shared = interior_plan("ha_area03_s02", &cache_root, &bin_root).expect("plan a03_s02");
        assert!(
            shared.has_room,
            "room_area03_s03 is exported from the main area pack"
        );
        assert!(shared.fallback.is_none());

        let empty = interior_plan("ha_area05_s02", &cache_root, &bin_root).expect("plan a05_s02");
        assert!(empty.geometry_models.is_empty());
        assert!(empty.fallback.is_some(), "no geometry => empty-scene fallback");
        assert!(empty.navmesh_pack.is_some(), "the navmesh still loads");

        let cave = interior_plan("ha_area04_s02", &cache_root, &bin_root).expect("plan a04_s02");
        assert!(!cave.geometry_models.is_empty(), "the cave shell and props stream");
        assert!(cave.has_room, "the exported cave shell covers the navmesh");
        assert!(cave.fallback.is_none(), "covered shell => no floor placeholder");
    }

    #[test]
    fn parse_shell_rows_and_pick_the_status() {
        let text = "# sheet: x\narea:string*\tparent:string\tmodels_ok:u16\tcoverage_x:f32\tcoverage_z:f32\ty_overlap:f32\tshell:enum\tconfidence:enum\n\
                    ha_area00_s03_a\tha_area00\t2\t1\t1\t1\tcovered\tPROBABLE\n\
                    ha_area04_s01\tha_area04\t1\t0.5\t0.5\t1\tpartial\tPROBABLE\n\
                    ha_area05_s02\tha_area05\t0\t\t\t\tnone\tVERIFIED\n\
                    ha_bogus\tx\t0\t\t\t\tweird\tASSUMED\n";
        let shells = parse_subzone_shells(text);
        assert_eq!(shells.len(), 3, "the unknown status is dropped");
        assert_eq!(shells["ha_area00_s03_a"].shell, ShellStatus::Covered);
        assert_eq!(shells["ha_area04_s01"].shell, ShellStatus::Partial);
        assert_eq!(shells["ha_area05_s02"].shell, ShellStatus::None);
        assert_eq!(shells["ha_area00_s03_a"].models_ok, 2);
        assert!((shells["ha_area04_s01"].coverage_x - 0.5).abs() < 1e-6);
    }

    #[test]
    fn real_shells_sheet_covers_the_six_fallback_interiors() {
        let text = find_data_file(SUBZONE_SHELLS_SHEET)
            .and_then(|p| std::fs::read_to_string(p).ok())
            .expect("data/subzone_shells.tsv");
        let shells = parse_subzone_shells(&text);
        assert_eq!(shells.len(), 55, "one row per sub-area");
        // The five interiors whose shell was wrongly reported missing: their exported
        // geometry covers the navmesh (see reports/data-progress/subzone_shells.md).
        for zone in [
            "ha_area04_s01",
            "ha_area04_s02",
            "ha_area04_s03",
            "ha_area04_s04",
            "ha_area05_s01",
        ] {
            assert_eq!(
                shells[zone].shell,
                ShellStatus::Covered,
                "{zone} has a real exported shell"
            );
        }
        // The one sub-area the game ships empty of placed geometry: its own terrain is the scene.
        assert_eq!(shells["ha_area05_s02"].shell, ShellStatus::None);
        assert_eq!(shells["ha_area05_s02"].models_ok, 0);
    }

    #[test]
    fn placeholder_terrain_is_not_samplable() {
        let t = placeholder_terrain();
        assert_eq!(t.area.area, INTERIOR_TERRAIN_AREA);
        assert!(t.area.instances.is_empty(), "no .trins trees in the placeholder");
        assert!(t.area.sample_world_height(10.0, 10.0).is_none());
        assert!(t.chunks.is_empty());
    }
}
