//! Measure the water planes of the main areas: where they are, how high their surface sits and how
//! deep the water is over the terrain/navmesh below.
//!
//! This is a DATA report, not a render: it decodes the same inputs the viewer feeds into
//! `game::water` — the shipped `field_object_placements.tsv` rows, the exported glTF meshes from
//! `field_model_export_index.tsv`, `TerrainArea` and the navmesh — and applies the same maths
//! (`water_body_from_mesh` + `WaterBodies::level_at`) to answer: is the water surface above the
//! ground, and is it deep enough for the chest (SWIM_ENTER_DEPTH) swim rule?
//!
//! The glTF triangle decode here mirrors what `world_objects::mesh_xz_triangles` does at runtime
//! with `Mesh::indices()`/`Mesh::ATTRIBUTE_POSITION`; the runtime path itself is not executed by
//! this tool (it needs the viewer's asset streamer).
//!
//! Usage: `water_report [--area N|all] [--bin-root DIR] [--cache-root DIR]`.
//! [skip] when the game data or an exported model is missing (no failure).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use pla::assets::terrain::TerrainArea;
use pla::game::water::{
    is_water_model, water_body_from_mesh, WaterBody, WaterMesh, SWIM_ENTER_DEPTH,
    SWIM_MAX_GROUND_OFFSET,
};
use pla::game::{world_objects, GameNavmeshResource};

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bin_root = arg_value(&args, "--bin-root")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("PLA_BIN_ROOT").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin"));
    let cache_root = arg_value(&args, "--cache-root")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("PLA_CACHE_ROOT").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(world_objects::DEFAULT_CACHE_ROOT));
    let areas: Vec<u8> = match arg_value(&args, "--area").as_deref() {
        None | Some("all") => (0..=5).collect(),
        Some(s) => s.parse::<u8>().map(|a| vec![a]).unwrap_or_else(|_| (0..=5).collect()),
    };
    if !bin_root.is_dir() {
        println!("[skip] game bin root not found: {}", bin_root.display());
        return;
    }
    let Some(sheet_path) = find_data_file("field_object_placements.tsv") else {
        println!("[skip] data/field_object_placements.tsv not found");
        return;
    };
    let Ok(sheet_text) = std::fs::read_to_string(&sheet_path) else {
        println!("[skip] cannot read {}", sheet_path.display());
        return;
    };
    let index_text = find_data_file("field_model_export_index.tsv").and_then(|p| std::fs::read_to_string(p).ok());

    for area in areas {
        report_area(area, &bin_root, &cache_root, &sheet_text, index_text.as_deref());
    }
}

fn report_area(
    area: u8,
    bin_root: &Path,
    cache_root: &Path,
    sheet_text: &str,
    index: Option<&str>,
) {
    let area_name = format!("ha_area{:02}", area);
    let placements: Vec<_> = world_objects::parse_placements(sheet_text, &area_name)
        .into_iter()
        .filter(|p| is_water_model(&p.model))
        .collect();
    println!("\n=== {area_name}: {} water placements (sheet, model_status=ok)", placements.len());
    if placements.is_empty() {
        return;
    }
    let paths = model_paths(cache_root, index);
    let Ok(terrain) = TerrainArea::load(bin_root, area) else {
        println!("[skip] {area_name}: terrain pack not available");
        return;
    };
    let navmesh = GameNavmeshResource::load(bin_root, area).ok();

    let (mut measured, mut swimmable, mut below_ground) = (0usize, 0usize, 0usize);
    for p in &placements {
        let Some(path) = paths.get(&p.model) else {
            println!("  {:<28} no exported glTF in the index", p.model);
            continue;
        };
        let Some(mesh) = gltf_water_mesh(path) else {
            println!("  {:<28} cannot decode the glTF mesh ({}, exists {})", p.model, path.display(), path.is_file());
            continue;
        };
        // The same transform the viewer places the model with (yaw about Y from rot_y).
        let tf = Transform {
            translation: Vec3::from(p.pos),
            rotation: world_objects::placement_rotation(p, world_objects::RotMode::YawY),
            scale: Vec3::from(p.scale),
        };
        let body: WaterBody = water_body_from_mesh(&mesh, &tf);
        measured += 1;

        // Sample a 24 x 24 grid over the bounding box and keep the points that are really water.
        let size = body.size();
        let mut depths = Vec::new();
        for ix in 0..24 {
            for iz in 0..24 {
                let x = body.min.x + size.x * (ix as f32 + 0.5) / 24.0;
                let z = body.min.y + size.y * (iz as f32 + 0.5) / 24.0;
                if !body.contains(Vec2::new(x, z)) {
                    continue;
                }
                let ground = navmesh
                    .as_ref()
                    .and_then(|n| n.height_at(x, z))
                    .or_else(|| terrain.sample_world_height(x, z));
                if let Some(g) = ground {
                    depths.push(body.level - g);
                }
            }
        }
        if depths.is_empty() {
            println!("  {:<28} no water sample over the terrain", p.model);
            continue;
        }
        let min_d = depths.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_d = depths.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let mean = depths.iter().sum::<f32>() / depths.len() as f32;
        // Points where the swim rule would actually start swimming: the surface sits on the ground
        // (within SWIM_MAX_GROUND_OFFSET) and the water is deeper than the chest.
        let swim_points = depths
            .iter()
            .filter(|d| d.abs() <= SWIM_MAX_GROUND_OFFSET && **d > SWIM_ENTER_DEPTH)
            .count();
        let deep = swim_points > 0;
        if deep {
            swimmable += 1;
        }
        if max_d < 0.0 {
            below_ground += 1;
        }
        println!(
            "  {:<28} {} tris, box {:7.1} x {:7.1} m at ({:7.1},{:7.1}), surface y {:8.2}, {} water samples, depth mean {:6.2} min {:6.2} max {:6.2} m, swim-trigger samples {}{}",
            p.model,
            body.triangles.len(),
            size.x,
            size.y,
            body.min.x + size.x / 2.0,
            body.min.y + size.y / 2.0,
            body.level,
            depths.len(),
            mean,
            min_d,
            max_d,
            swim_points,
            if deep { "  [swim reachable]" } else { "" },
        );
    }
    println!(
        "  {area_name}: {measured} bodies measured, {swimmable} deeper than the chest ({SWIM_ENTER_DEPTH:.2} m), {below_ground} entirely under the terrain at the sample points"
    );
}

/// Model name -> exported glTF path, from the export index (`model_status` == ok).
fn model_paths(cache_root: &Path, index: Option<&str>) -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    let Some(text) = index else { return map };
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() >= 4 && c[3] == "ok" {
            if let Some(rel) = c[2].strip_prefix("work/cache/") {
                map.insert(c[1].to_string(), cache_root.join(rel));
            }
        }
    }
    map
}

/// Local mesh bounds + XZ triangles of an exported glTF, mirroring the runtime
/// `Mesh::compute_aabb()` / `world_objects::mesh_xz_triangles` on the live `Mesh`.
fn gltf_water_mesh(path: &Path) -> Option<WaterMesh> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let dir = path.parent()?;
    let mut buffers = Vec::new();
    for b in v.get("buffers")?.as_array()? {
        let uri = b.get("uri")?.as_str()?;
        buffers.push(std::fs::read(dir.join(uri)).ok()?);
    }
    let mut mesh = WaterMesh::default();
    let mut first = true;
    for m in v.get("meshes")?.as_array()? {
        for prim in m.get("primitives")?.as_array()? {
            let positions = read_vec3(&v, &buffers, prim.get("attributes")?.get("POSITION")?.as_u64()? as usize)?;
            let mut lo = Vec3::splat(f32::INFINITY);
            let mut hi = Vec3::splat(f32::NEG_INFINITY);
            for p in &positions {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
            if first {
                mesh.local_min = lo;
                mesh.local_max = hi;
                first = false;
            } else {
                mesh.local_min = mesh.local_min.min(lo);
                mesh.local_max = mesh.local_max.max(hi);
            }
            let indices = match prim.get("indices") {
                Some(i) => read_indices(&v, &buffers, i.as_u64()? as usize)?,
                None => (0..positions.len()).collect(),
            };
            for tri in indices.chunks(3) {
                if let [i0, i1, i2] = tri {
                    let p = |i: usize| positions.get(i).map(|p| Vec2::new(p.x, p.z));
                    if let (Some(a), Some(b), Some(c)) = (p(*i0), p(*i1), p(*i2)) {
                        mesh.triangles.push([a, b, c]);
                    }
                }
            }
        }
    }
    (!mesh.triangles.is_empty()).then_some(mesh)
}

fn accessor_bytes<'a>(
    v: &serde_json::Value,
    buffers: &'a [Vec<u8>],
    accessor: usize,
) -> Option<(&'a [u8], usize, usize, usize)> {
    let acc = v.get("accessors")?.as_array()?.get(accessor)?;
    let view = v.get("bufferViews")?.as_array()?.get(acc.get("bufferView")?.as_u64()? as usize)?;
    let buf = buffers.get(view.get("buffer")?.as_u64()? as usize)?;
    let base = view.get("byteOffset").and_then(|b| b.as_u64()).unwrap_or(0)
        + acc.get("byteOffset").and_then(|b| b.as_u64()).unwrap_or(0);
    let count = acc.get("count")?.as_u64()? as usize;
    let component = acc.get("componentType")?.as_u64()? as usize;
    let stride = view
        .get("byteStride")
        .and_then(|b| b.as_u64())
        .map(|s| s as usize)
        .unwrap_or_else(|| match acc.get("type").and_then(|t| t.as_str()) {
            Some("VEC3") => 12,
            // Byte size of one component, NOT the componentType enum value.
            Some("SCALAR") => match component {
                5121 => 1,
                5123 => 2,
                _ => 4,
            },
            _ => 0,
        });
    if stride == 0 {
        return None;
    }
    Some((buf, base as usize, count, stride))
}

fn read_vec3(v: &serde_json::Value, buffers: &[Vec<u8>], accessor: usize) -> Option<Vec<Vec3>> {
    let (buf, base, count, stride) = accessor_bytes(v, buffers, accessor)?;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = base + i * stride;
        if o + 12 > buf.len() {
            return None;
        }
        let f = |k: usize| f32::from_le_bytes([buf[o + k * 4], buf[o + k * 4 + 1], buf[o + k * 4 + 2], buf[o + k * 4 + 3]]);
        out.push(Vec3::new(f(0), f(1), f(2)));
    }
    Some(out)
}

fn read_indices(v: &serde_json::Value, buffers: &[Vec<u8>], accessor: usize) -> Option<Vec<usize>> {
    let acc = v.get("accessors")?.as_array()?.get(accessor)?;
    let component = acc.get("componentType")?.as_u64()?;
    let (buf, base, count, stride) = accessor_bytes(v, buffers, accessor)?;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = base + i * stride;
        let idx = match component {
            5123 if o + 2 <= buf.len() => u16::from_le_bytes([buf[o], buf[o + 1]]) as usize,
            5125 if o + 4 <= buf.len() => u32::from_le_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]) as usize,
            _ => return None,
        };
        out.push(idx);
    }
    Some(out)
}

fn find_data_file(name: &str) -> Option<PathBuf> {
    [PathBuf::from("data"), PathBuf::from("../../data"), PathBuf::from("/home/nico/work/decompilacion/data")]
        .into_iter()
        .map(|d| d.join(name))
        .find(|p| p.exists())
}
