//! Compare the port's walk rule (`pla::game::terrain_collision`, user decision 2026-10-17) with
//! the decoded navmesh over the main areas: how many m2 each rule allows and where they disagree.
//!
//! This is a DATA report, not a render. For every sample point of the terrain extent it asks:
//!
//! * navmesh: `NavMesh::is_walkable` (what the movement used before the change);
//! * terrain rule: ground exists (heightmap), the local slope is <=
//!   `terrain_collision::MAX_WALK_SLOPE_DEG`, the water sheet (decoded from the water placement
//!   models, like `water_report.rs`) is not deeper than `SWIM_ENTER_DEPTH` on foot, and the
//!   player capsule is not pushed out by placed-object collision (`WorldCollision`, streamed in
//!   tiles so the whole area is covered).
//!
//! Static map reading: in play the slope term blocks a wall ahead of the walker's feet and a
//! drop is always walkable; this report counts a steep point as blocked (a walker cannot stand
//! there), which is the conservative reading of the rule WITHOUT a walker.
//!
//! Usage: `terrain_walk_report [--area 0,1|all] [--step 4] [--map-step 8] [--tile 100]
//!         [--bin-root DIR] [--cache-root DIR]`.
//! The glTF triangle decode mirrors `water_report.rs` (same exported cache, same placement
//! transform); the runtime path itself is not executed by this tool.
//! [skip] without game data (no failure).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::math::Vec2;
use bevy::prelude::{Transform, Vec3};

use pla::assets::navmesh::AreaNavigation;
use pla::assets::terrain::{TerrainArea, TerrainScaleOverride};
use pla::game::terrain_collision::{terrain_slope_deg, MAX_WALK_SLOPE_DEG};
use pla::game::water::{
    is_water_model, water_body_from_mesh, WaterBodies, WaterMesh, SWIM_ENTER_DEPTH,
};
use pla::game::world_collision::{WorldCollision, PLAYER_HEIGHT, PLAYER_RADIUS};
use pla::game::world_objects::{self, RotMode};
use pla::game::zone::TerrainResource;

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bin_root = arg_value(&args, "--bin-root")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("PLA_BIN_ROOT").map(PathBuf::from))
        .unwrap_or_else(|| {
            PathBuf::from("/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin")
        });
    let cache_root = arg_value(&args, "--cache-root")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("PLA_CACHE_ROOT").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(world_objects::DEFAULT_CACHE_ROOT));
    let step: f32 = arg_value(&args, "--step")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(4.0)
        .max(0.5);
    let map_step: f32 = arg_value(&args, "--map-step")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(8.0)
        .max(step);
    let tile: f32 = arg_value(&args, "--tile")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(100.0)
        .max(20.0);
    let areas = match arg_value(&args, "--area").as_deref() {
        None => vec![0u8, 1],
        Some("all") => (0..=5).collect(),
        Some(s) => s
            .split(',')
            .filter_map(|a| a.trim().parse::<u8>().ok())
            .collect(),
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
    let index_text = find_data_file("field_model_export_index.tsv")
        .and_then(|p| std::fs::read_to_string(p).ok());

    let mut world = WorldTotals::default();
    for area in areas {
        match report_area(area, &bin_root, &cache_root, &sheet_text, index_text.as_deref(), step, map_step, tile) {
            Some(t) => world.add(t),
            None => println!("[skip] ha_area{area:02}: terrain pack not available"),
        }
    }
    let unit = step * step;
    let m2 = |cells: usize| fmt_area(cells as f32 * unit);
    if world.samples > 0 {
        println!("\n=== total sampled {} of terrain", m2(world.samples));
        println!(
            "    walkable by navmesh: {}; by the terrain rule: {}",
            m2(world.both + world.only_nav),
            m2(world.both + world.only_rule)
        );
        println!(
            "    both: {}; only navmesh: {}; only terrain rule: {}; neither: {}",
            m2(world.both),
            m2(world.only_nav),
            m2(world.only_rule),
            m2(world.neither)
        );
    }
}

#[derive(Default, Clone, Copy)]
struct WorldTotals {
    samples: usize,
    both: usize,
    only_nav: usize,
    only_rule: usize,
    neither: usize,
}

impl WorldTotals {
    fn add(&mut self, o: WorldTotals) {
        self.samples += o.samples;
        self.both += o.both;
        self.only_nav += o.only_nav;
        self.only_rule += o.only_rule;
        self.neither += o.neither;
    }
}

fn fmt_area(m2: f32) -> String {
    if m2 >= 10_000.0 {
        format!("{:.2} ha ({:.0} m2)", m2 / 10_000.0, m2)
    } else {
        format!("{m2:.0} m2")
    }
}

/// Cell class of one sample point (`Both`, `OnlyNav`, `OnlyRule`, `Neither`).
#[derive(Clone, Copy, PartialEq)]
enum Class {
    Both,
    OnlyNav,
    OnlyRule,
    Neither,
}

#[allow(clippy::too_many_arguments)]
fn report_area(
    area_idx: u8,
    bin_root: &Path,
    cache_root: &Path,
    sheet_text: &str,
    index: Option<&str>,
    step: f32,
    map_step: f32,
    tile: f32,
) -> Option<WorldTotals> {
    let area_name = format!("ha_area{area_idx:02}");
    let Ok(terrain_area) = TerrainArea::load(bin_root, area_idx) else {
        return None;
    };
    let extent = terrain_area.world_extent;
    let settings = terrain_area
        .mesh_settings(TerrainScaleOverride::default(), 64)
        .expect("mesh settings");
    let terrain = TerrainResource {
        area: Arc::new(terrain_area),
        chunks: Vec::new(),
        blended_albedo: None,
        settings,
    };
    let Ok(nav) = AreaNavigation::load(bin_root, area_idx) else {
        println!("\n=== {area_name}: no navmesh pack; skipped");
        return None;
    };
    let water = load_water_bodies(&area_name, sheet_text, index, cache_root);
    let mut wc = match WorldCollision::load(bin_root, area_idx, RotMode::YawY, cache_root) {
        Ok(w) => {
            println!(
                "\n=== {area_name}: extent {:.0} x {:.0} m, {} water bodies kept, collision placements {}/{} with a shape",
                extent[0], extent[1], water.bodies.len(), w.placements_total, w.placements_with_collision
            );
            Some(w)
        }
        Err(e) => {
            println!(
                "\n=== {area_name}: extent {:.0} x {:.0} m, {} water bodies kept; collision unavailable ({e})",
                extent[0], extent[1], water.bodies.len()
            );
            None
        }
    };

    let mut totals = WorldTotals::default();
    let mut lost_slope = 0usize;
    let mut lost_water = 0usize;
    let mut lost_collider = 0usize;
    let mut lost_multi = 0usize;
    // Coarse class map for the printed picture.
    let mw = (extent[0] / map_step).ceil() as usize;
    let mh = (extent[1] / map_step).ceil() as usize;
    let mut map_counts: Vec<[usize; 4]> = vec![[0; 4]; mw * mh];
    let class_slot = |c: Class| match c {
        Class::Both => 0,
        Class::OnlyNav => 1,
        Class::OnlyRule => 2,
        Class::Neither => 3,
    };

    let tiles_x = (extent[0] / tile).ceil() as usize;
    let tiles_z = (extent[1] / tile).ceil() as usize;
    for tz in 0..tiles_z {
        for tx in 0..tiles_x {
            let centre = Vec2::new((tx as f32 + 0.5) * tile, (tz as f32 + 0.5) * tile);
            if let Some(w) = wc.as_mut() {
                w.stream_to_scaled(centre, 1.0);
            }
            let z0 = tz as f32 * tile;
            let z1 = (z0 + tile).min(extent[1]);
            let x0 = tx as f32 * tile;
            let x1 = (x0 + tile).min(extent[0]);
            let mut z = z0 + step * 0.5;
            while z < z1 {
                let mut x = x0 + step * 0.5;
                while x < x1 {
                    let ground = terrain.area.sample_world_height(x, z);
                    let slope_ok = match ground {
                        Some(_) => terrain_slope_deg(Some(&terrain), x, z)
                            .is_some_and(|s| s <= MAX_WALK_SLOPE_DEG),
                        None => false,
                    };
                    let deep = match ground {
                        Some(g) => water
                            .level_at(Vec2::new(x, z))
                            .is_some_and(|level| level - g > SWIM_ENTER_DEPTH),
                        None => false,
                    };
                    let collider_blocked = match (wc.as_ref(), ground) {
                        (Some(w), Some(g)) => {
                            let out = w.push_out_capsule(
                                Vec3::new(x, g + PLAYER_HEIGHT * 0.5, z),
                                PLAYER_RADIUS,
                                PLAYER_HEIGHT,
                            );
                            Vec2::new(out.x - x, out.z - z).length() > 0.01
                        }
                        _ => false,
                    };
                    let rule_walk = slope_ok && !deep && !collider_blocked;
                    let nav_walk = nav.is_walkable(x, z);
                    let class = match (nav_walk, rule_walk) {
                        (true, true) => Class::Both,
                        (true, false) => Class::OnlyNav,
                        (false, true) => Class::OnlyRule,
                        (false, false) => Class::Neither,
                    };
                    match class {
                        Class::Both => totals.both += 1,
                        Class::OnlyNav => {
                            totals.only_nav += 1;
                            let blocks = [(!slope_ok) as usize, deep as usize, collider_blocked as usize]
                                .iter()
                                .sum::<usize>();
                            if blocks > 1 {
                                lost_multi += 1;
                            } else if !slope_ok {
                                lost_slope += 1;
                            } else if deep {
                                lost_water += 1;
                            } else if collider_blocked {
                                lost_collider += 1;
                            }
                        }
                        Class::OnlyRule => totals.only_rule += 1,
                        Class::Neither => totals.neither += 1,
                    }
                    totals.samples += 1;
                    let mx = (x / map_step) as usize;
                    let mz = (z / map_step) as usize;
                    if mx < mw && mz < mh {
                        map_counts[mz * mw + mx][class_slot(class)] += 1;
                    }
                    x += step;
                }
                z += step;
            }
        }
    }

    let unit = step * step;
    println!(
        "    samples {} ({} grid, {:.1} m): both {}, only navmesh {}, only terrain rule {}, neither {}",
        totals.samples,
        fmt_area(totals.samples as f32 * unit),
        step,
        fmt_area(totals.both as f32 * unit),
        fmt_area(totals.only_nav as f32 * unit),
        fmt_area(totals.only_rule as f32 * unit),
        fmt_area(totals.neither as f32 * unit)
    );
    let nav_total = (totals.both + totals.only_nav) as f32 * unit;
    let rule_total = (totals.both + totals.only_rule) as f32 * unit;
    if nav_total > 0.0 {
        println!(
            "    navmesh walkable area kept by the rule: {:.1} % ({} lost)",
            100.0 * totals.both as f32 / (totals.both + totals.only_nav) as f32,
            fmt_area(totals.only_nav as f32 * unit)
        );
    }
    if rule_total > 0.0 {
        println!(
            "    terrain rule walkable area the navmesh did not allow: {} (+{:.1} % over the navmesh)",
            fmt_area(totals.only_rule as f32 * unit),
            100.0 * totals.only_rule as f32 / (totals.both + totals.only_nav).max(1) as f32
        );
    }
    println!(
        "    only-navmesh cells blocked by: slope {lost_slope}, deep water {lost_water}, placed colliders {lost_collider}, several {lost_multi}"
    );
    // Coarse map.
    println!("    map ({map_step:.0} m/char): # both, N only navmesh, T only terrain rule, . neither");
    for mz in 0..mh {
        let mut line = String::with_capacity(mw);
        for mx in 0..mw {
            let counts = map_counts[mz * mw + mx];
            let total: usize = counts.iter().sum();
            let ch = if total == 0 {
                ' '
            } else {
                // Differences first (they are what the report is about), then majority.
                let diff = |a: usize, b: usize| a + b > total / 4 && a + b > 0;
                let nav_extra = counts[1];
                let rule_extra = counts[2];
                if diff(nav_extra, rule_extra) {
                    if rule_extra >= nav_extra {
                        'T'
                    } else {
                        'N'
                    }
                } else if counts[0] * 2 >= total {
                    '#'
                } else {
                    '.'
                }
            };
            line.push(ch);
        }
        println!("    {line}");
    }
    Some(totals)
}

/// Water bodies of the area (same decode as `water_report.rs`): the placement sheet rows whose
/// model is a water sheet, their exported glTF triangles moved by the placement transform.
fn load_water_bodies(
    area_name: &str,
    sheet_text: &str,
    index: Option<&str>,
    cache_root: &Path,
) -> WaterBodies {
    let mut bodies = WaterBodies::default();
    let paths = export_paths(cache_root, index);
    let placements: Vec<_> = world_objects::parse_placements(sheet_text, area_name)
        .into_iter()
        .filter(|p| is_water_model(&p.model))
        .collect();
    let mut missing = 0usize;
    for p in &placements {
        let Some(path) = paths.get(&p.model) else {
            missing += 1;
            continue;
        };
        let Some(mesh) = gltf_water_mesh(path) else {
            missing += 1;
            continue;
        };
        let tf = Transform {
            translation: Vec3::from(p.pos),
            rotation: world_objects::placement_rotation(p, RotMode::YawY),
            scale: Vec3::from(p.scale),
        };
        bodies.insert(water_body_from_mesh(&mesh, &tf));
    }
    if missing > 0 {
        println!(
            "    ({area_name}: {missing} of {} water placements without a decodable glTF were skipped)",
            placements.len()
        );
    }
    bodies
}

/// Model name -> exported glTF path, from the export index (`model_status` == ok); mirrors
/// `water_report.rs`.
fn export_paths(cache_root: &Path, index: Option<&str>) -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    let Some(text) = index else { return map };
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() >= 4 && c[3] == "ok" {
            if let Some((path, _)) = world_objects::normalize_export_path(cache_root, c[2]) {
                map.insert(c[1].to_string(), path);
            }
        }
    }
    map
}

// ------------------------------------------------------------------ glTF (copied from water_report.rs)

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
            let positions = read_vec3(
                &v,
                &buffers,
                prim.get("attributes")?.get("POSITION")?.as_u64()? as usize,
            )?;
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
    let view = v
        .get("bufferViews")?
        .as_array()?
        .get(acc.get("bufferView")?.as_u64()? as usize)?;
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
        let f = |k: usize| {
            f32::from_le_bytes([
                buf[o + k * 4],
                buf[o + k * 4 + 1],
                buf[o + k * 4 + 2],
                buf[o + k * 4 + 3],
            ])
        };
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
            5125 if o + 4 <= buf.len() => {
                u32::from_le_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]) as usize
            }
            _ => return None,
        };
        out.push(idx);
    }
    Some(out)
}

fn find_data_file(name: &str) -> Option<PathBuf> {
    [
        PathBuf::from("data"),
        PathBuf::from("../../data"),
        PathBuf::from("/home/nico/work/decompilacion/data"),
    ]
    .into_iter()
    .map(|d| d.join(name))
    .find(|p| p.exists())
}
