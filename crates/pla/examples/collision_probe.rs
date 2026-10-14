//! Headless probe for `game::world_collision`: coverage, rotation-convention validation,
//! capsule stop tests and ray pass-through counts against the real game data.
//!
//! No window is ever opened. Usage:
//!
//! ```sh
//! cargo run -p pla --example collision_probe -- [--area N] [--all-areas] [--rays K]
//! ```
//!
//! Everything reads the committed sheets and the game packs under `PLA_BIN_ROOT`
//! (default: the main checkout's `work/pla/pk2/romfs_patched/bin`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use bevy::math::EulerRot;
use bevy::prelude::*;
use pla::assets::navmesh::AreaNavigation;
use pla::assets::terrain::TerrainArea;
use pla::game::world_collision::{
    placement_transform, transformed_bounds, CollisionGrid, CollisionPacks, PLAYER_HEIGHT,
    PLAYER_RADIUS,
};
use pla::game::world_objects::{
    find_data_file, normalize_export_path, parse_placements, Placement, RotMode,
};

fn bin_root() -> PathBuf {
    std::env::var_os("PLA_BIN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into())
}

fn cache_root() -> PathBuf {
    std::env::var_os("PLA_CACHE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/home/nico/work/decompilacion/work/cache".into())
}

fn placements_for(area: u8) -> Vec<Placement> {
    let text =
        std::fs::read_to_string(find_data_file("field_object_placements.tsv").unwrap()).unwrap();
    parse_placements(&text, &format!("ha_area{area:02}"))
}

/// glTF `path` -> (model path, export status) from `data/field_model_export_index.tsv`.
fn gltf_lookup() -> std::collections::HashMap<String, (PathBuf, String)> {
    let mut out = std::collections::HashMap::new();
    if let Some(text) =
        find_data_file("field_model_export_index.tsv").and_then(|p| std::fs::read_to_string(p).ok())
    {
        for line in text.lines().filter(|l| !l.starts_with('#')) {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() >= 4 {
                out.insert(c[1].to_string(), (PathBuf::from(c[2]), c[3].to_string()));
            }
        }
    }
    out
}

/// Local AABB of every POSITION accessor referenced by the glTF meshes.
fn gltf_local_bounds(path: &Path) -> Option<(Vec3, Vec3)> {
    let text = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let accessors = json.get("accessors")?.as_array()?;
    let mut indices = BTreeSet::new();
    for mesh in json.get("meshes")?.as_array()? {
        for prim in mesh.get("primitives")?.as_array()? {
            if let Some(i) = prim.get("attributes")?.get("POSITION")?.as_u64() {
                indices.insert(i as usize);
            }
        }
    }
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for i in indices {
        let a = accessors.get(i)?;
        let amin = a.get("min")?.as_array()?;
        let amax = a.get("max")?.as_array()?;
        for k in 0..3 {
            lo[k] = lo[k].min(amin[k].as_f64()? as f32);
            hi[k] = hi[k].max(amax[k].as_f64()? as f32);
        }
    }
    (lo.x <= hi.x).then_some((lo, hi))
}

fn aabb_iou(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> f32 {
    let inter = (a.1.min(b.1) - a.0.max(b.0)).max(Vec3::ZERO);
    let vi = inter.x * inter.y * inter.z;
    let va = (a.1 - a.0).x * (a.1 - a.0).y * (a.1 - a.0).z;
    let vb = (b.1 - b.0).x * (b.1 - b.0).y * (b.1 - b.0).z;
    if va + vb - vi <= 0.0 {
        0.0
    } else {
        vi / (va + vb - vi)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RotCand {
    YawY,
    YawYNeg,
    YawYFlip,
    EulerXyz,
    EulerZyx,
    EulerYxz,
}

impl RotCand {
    const ALL: [RotCand; 6] = [
        RotCand::YawY,
        RotCand::YawYNeg,
        RotCand::YawYFlip,
        RotCand::EulerXyz,
        RotCand::EulerZyx,
        RotCand::EulerYxz,
    ];

    fn name(self) -> &'static str {
        match self {
            RotCand::YawY => "yaw_y",
            RotCand::YawYNeg => "yaw_y_neg",
            RotCand::YawYFlip => "yaw_y_flip",
            RotCand::EulerXyz => "euler_xyz_deg",
            RotCand::EulerZyx => "euler_zyx_deg",
            RotCand::EulerYxz => "euler_yxz_deg",
        }
    }

    fn matrix(self, p: &Placement) -> Mat4 {
        match self {
            RotCand::YawY => placement_transform(p, RotMode::YawY),
            RotCand::YawYNeg => placement_transform(p, RotMode::YawYNeg),
            RotCand::YawYFlip => placement_transform(p, RotMode::YawYFlip),
            RotCand::EulerXyz | RotCand::EulerZyx | RotCand::EulerYxz => {
                let r = [
                    p.rot_deg[0].to_radians(),
                    p.rot_deg[1].to_radians(),
                    p.rot_deg[2].to_radians(),
                ];
                let order = match self {
                    RotCand::EulerXyz => EulerRot::XYZ,
                    RotCand::EulerZyx => EulerRot::ZYX,
                    _ => EulerRot::YXZ,
                };
                Mat4::from_scale_rotation_translation(
                    Vec3::from(p.scale),
                    Quat::from_euler(order, r[0], r[1], r[2]),
                    Vec3::from(p.pos),
                )
            }
        }
    }
}

struct Rng(u64);
impl Rng {
    fn next_f32(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32) / ((1u64 << 24) as f32)
    }
}

/// Ground truth the capsule walk follows: collision grid, terrain heights and navmesh.
struct Scene<'a> {
    nav: Option<&'a AreaNavigation>,
    terrain: Option<&'a TerrainArea>,
}

impl Scene<'_> {
    fn height(&self, x: f32, z: f32) -> Option<f32> {
        self.terrain.and_then(|t| t.sample_world_height(x, z))
    }

    fn walkable(&self, x: f32, z: f32) -> bool {
        self.nav.is_none_or(|n| n.is_walkable(x, z))
    }
}

/// One-direction capsule walk: returns (final_xz, resolved_penetration, final_walkable).
fn walk_capsule(
    grid: &CollisionGrid,
    scene: &Scene,
    start: Vec2,
    y0: f32,
    dir: Vec2,
    steps: usize,
    step: f32,
) -> (Vec2, f32, bool) {
    let mut p = Vec3::new(start.x, y0, start.y);
    let mut worst = 0.0f32;
    for _ in 0..steps {
        p.x += dir.x * step;
        p.z += dir.y * step;
        if let Some(t) = scene.height(p.x, p.z) {
            p.y = t + 0.9;
        }
        let out = grid.push_out_capsule(p, PLAYER_RADIUS, PLAYER_HEIGHT);
        let again = grid.push_out_capsule(out, PLAYER_RADIUS, PLAYER_HEIGHT);
        worst = worst.max((again - out).length());
        p = out;
    }
    (Vec2::new(p.x, p.z), worst, scene.walkable(p.x, p.z))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let area: u8 = args
        .iter()
        .position(|a| a == "--area")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let rays: usize = args
        .iter()
        .position(|a| a == "--rays")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let all_areas = args.iter().any(|a| a == "--all-areas");

    let root = bin_root();
    let packs = CollisionPacks::load(&root).expect("collision packs");
    println!(
        "== collision_probe == packs={} entries={}",
        packs.packs.len(),
        packs.entry_count()
    );

    // ---- coverage of every committed placement row (all areas or one) ----
    let areas: Vec<u8> = if all_areas {
        (0..=5).collect()
    } else {
        vec![area]
    };
    let (mut placed, mut covered, mut missing_models) = (0usize, 0usize, BTreeSet::new());
    for a in &areas {
        let ps = placements_for(*a);
        let mut with = 0;
        for p in &ps {
            placed += 1;
            match packs.mesh_for_model(&p.model) {
                Ok(_) => {
                    covered += 1;
                    with += 1;
                }
                Err(_) => {
                    missing_models.insert(p.model.clone());
                }
            }
        }
        println!(
            "area {a:02}: {} placements, {with} with a collision shape",
            ps.len()
        );
    }
    println!(
        "coverage: {covered}/{placed} placements, {} uniques without a shape {missing_models:?}",
        missing_models.len()
    );

    let nav = AreaNavigation::load(&root, area).ok();
    let terrain = TerrainArea::load(&root, area).ok();
    let ps = placements_for(area);

    // ---- optional single-model inspection ----
    if let Some(model) = args
        .iter()
        .position(|a| a == "--inspect")
        .and_then(|i| args.get(i + 1))
    {
        let Ok(mesh) = packs.mesh_for_model(model) else {
            println!("inspect: {model} has no collision shape");
            return;
        };
        let placement = ps.iter().find(|p| p.model == *model);
        let tf = placement.map_or(Mat4::IDENTITY, |p| placement_transform(p, RotMode::YawY));
        let (lo, hi) = transformed_bounds(&mesh, &tf);
        let walls = mesh
            .triangles
            .iter()
            .filter(|t| {
                let [a, b, c] =
                    t.map(|i| tf.transform_point3(Vec3::from(mesh.vertices[i as usize])));
                let n = (b - a).cross(c - a).normalize_or_zero();
                n.y.abs() <= 0.7
            })
            .count();
        println!(
            "inspect {model}: tf={} verts={} tris={} wall_tris={} aabb=({:.1},{:.1},{:.1})..({:.1},{:.1},{:.1})",
            if placement.is_some() { "placed" } else { "identity" },
            mesh.vertices.len(),
            mesh.triangles.len(),
            walls,
            lo.x, lo.y, lo.z, hi.x, hi.y, hi.z
        );
        let mut grid = CollisionGrid::new(4.0);
        grid.insert(std::sync::Arc::new(mesh), tf);
        let cz = (lo.z + hi.z) * 0.5;
        let base = terrain
            .as_ref()
            .and_then(|t| t.sample_world_height(lo.x - 2.0, cz))
            .unwrap_or(0.0);
        for (label, dy) in [("chest +1.0", 1.0f32), ("high +2.2", 2.2)] {
            let cy = base + dy;
            print!("  {label}: ");
            for dz in -4..=4 {
                let z = cz + dz as f32 * (hi.z - lo.z) / 8.0;
                let origin = Vec3::new(lo.x - 2.0, cy, z);
                let hit = grid.raycast(origin, Vec3::X, (hi.x - lo.x) + 5.0);
                print!("{}", if hit.is_some() { '#' } else { '.' });
            }
            println!("  (rays +x across z)");
        }
        return;
    }

    // ---- local-frame alignment vs the exported visual glTF ----
    let gltfs = gltf_lookup();
    let (mut checked, mut aligned) = (0usize, 0usize);
    let mut iou_sum = 0.0f32;
    for p in ps.iter().filter(|p| p.model.contains("house")) {
        let Some((out, status)) = gltfs.get(&p.model) else {
            continue;
        };
        if status != "ok" {
            continue;
        }
        // The index stores absolute (current exporter) or legacy `work/cache/`-relative rows;
        // normalize both against the cache root (fallback: the stored path as-is).
        let rel = normalize_export_path(&cache_root(), &out.to_string_lossy())
            .map(|(path, _)| path)
            .unwrap_or_else(|| out.clone());
        let Some(visual) = gltf_local_bounds(&rel) else {
            continue;
        };
        let Ok(mesh) = packs.mesh_for_model(&p.model) else {
            continue;
        };
        let Some((clo, chi)) = mesh.bounds() else {
            continue;
        };
        let iou = aabb_iou((Vec3::from(clo), Vec3::from(chi)), visual);
        checked += 1;
        iou_sum += iou;
        if iou > 0.5 {
            aligned += 1;
        }
    }
    println!(
        "local frame: {aligned}/{checked} houses have IoU>0.5 between the collision mesh and the visual glTF local AABB (mean IoU {:.3})",
        if checked > 0 { iou_sum / checked as f32 } else { 0.0 }
    );

    // ---- rotation convention validated against the navmesh holes and the terrain ----
    if let (Some(nav), Some(terrain)) = (nav.as_ref(), terrain.as_ref()) {
        for (label, sel) in [("houses", 0usize), ("tilted props", 1usize)] {
            let group: Vec<&Placement> = ps
                .iter()
                .filter(|p| {
                    if sel == 0 {
                        p.model.contains("house")
                    } else {
                        (p.rot_deg[0].abs() > 1.0 || p.rot_deg[2].abs() > 1.0)
                            && packs.mesh_for_model(&p.model).is_ok()
                    }
                })
                .take(60)
                .collect();
            if group.is_empty() {
                continue;
            }
            println!(
                "rotation check on {} {label} of area {area:02}:",
                group.len()
            );
            println!(
                "{:<14} {:>10} {:>10} {:>8}",
                "candidate", "walk_frac", "base_dy", "good"
            );
            for cand in RotCand::ALL {
                let (mut wf, mut dy, mut good, mut n) = (0.0f32, 0.0f32, 0usize, 0usize);
                for p in &group {
                    let Ok(mesh) = packs.mesh_for_model(&p.model) else {
                        continue;
                    };
                    let tf = cand.matrix(p);
                    let pts: Vec<Vec3> = mesh
                        .vertices
                        .iter()
                        .map(|v| tf.transform_point3(Vec3::from(*v)))
                        .collect();
                    if pts.is_empty() {
                        continue;
                    }
                    let ymin = pts.iter().map(|v| v.y).fold(f32::MAX, f32::min);
                    let (mut w, mut d, mut dc) = (0usize, 0.0f32, 0usize);
                    for v in &pts {
                        if nav.is_walkable(v.x, v.z) {
                            w += 1;
                        }
                        if v.y < ymin + 0.5 {
                            if let Some(h) = terrain.sample_world_height(v.x, v.z) {
                                d += (v.y - h).abs();
                                dc += 1;
                            }
                        }
                    }
                    let frac = w as f32 / pts.len() as f32;
                    wf += frac;
                    dy += if dc > 0 { d / dc as f32 } else { 0.0 };
                    // Houses should sit in navmesh holes (low walk_frac); props on walkable ground (high).
                    good += if sel == 0 {
                        (frac < 0.25) as usize
                    } else {
                        (frac > 0.75) as usize
                    };
                    n += 1;
                }
                let n = n.max(1) as f32;
                println!(
                    "{:<14} {:>10.3} {:>10.3} {:>8}",
                    cand.name(),
                    wf / n,
                    dy / n,
                    good
                );
            }
        }
        println!("(walk_frac = share of collision vertices landing on walkable navmesh: near 0 for houses, near 1 for props; base_dy = mean |collision base - terrain height| m)");
    }

    // ---- capsule stop test against each house facade (four approach directions) ----
    let (mut tested, mut stopped, mut crossed, mut off_mesh, mut start_off) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut worst_pen = 0.0f32;
    let mut crossed_cases: Vec<String> = Vec::new();
    for p in ps.iter().filter(|p| p.model.contains("house")) {
        let Ok(mesh) = packs.mesh_for_model(&p.model) else {
            continue;
        };
        let tf = placement_transform(p, RotMode::YawY);
        let (lo, hi) = transformed_bounds(&mesh, &tf);
        let mut grid = CollisionGrid::new(4.0);
        grid.insert(std::sync::Arc::new(mesh), tf);
        let center = Vec2::new((lo.x + hi.x) * 0.5, (lo.z + hi.z) * 0.5);
        let scene = Scene {
            nav: nav.as_ref(),
            terrain: terrain.as_ref(),
        };
        for (name, dir) in [
            ("+x", Vec2::X),
            ("-x", Vec2::NEG_X),
            ("+z", Vec2::Y),
            ("-z", Vec2::NEG_Y),
        ] {
            let half = Vec2::new(hi.x - lo.x, hi.z - lo.z) * 0.5;
            let reach = half.dot(dir.abs());
            let start = center - dir * (reach + 3.0);
            let y0 = scene.height(start.x, start.y).unwrap_or(0.0) + 0.9;
            if !scene.walkable(start.x, start.y) {
                start_off += 1;
            }
            let (fin, pen, walk) = walk_capsule(&grid, &scene, start, y0, dir, 120, 0.1);
            tested += 1;
            worst_pen = worst_pen.max(pen);
            let fy = scene.height(fin.x, fin.y).map_or(y0, |h| h + 0.9);
            let inside = grid.point_inside_shell(Vec3::new(fin.x, fy, fin.y), 40.0);
            let past = (fin - center).dot(dir) > 0.0;
            if inside || past {
                crossed += 1;
                crossed_cases.push(format!(
                    "{}({name}{})",
                    p.model,
                    if inside { ",inside" } else { "" }
                ));
            } else {
                stopped += 1;
            }
            if !walk {
                off_mesh += 1;
            }
        }
    }
    println!(
        "capsule facade test: {stopped}/{tested} stopped, {crossed} crossed, worst unresolved penetration {worst_pen:.3} m; starts off-navmesh {start_off}, ends off-navmesh {off_mesh}"
    );
    if !crossed_cases.is_empty() {
        println!("crossed cases: {}", crossed_cases.join(", "));
    }

    // ---- rays through each house: how many cross with zero triangle hits ----
    let mut rng = Rng(0x5eed);
    let (mut ray_total, mut ray_miss) = (0usize, 0usize);
    let mut per_house: Vec<(String, usize, usize)> = Vec::new();
    for p in ps.iter().filter(|p| p.model.contains("house")) {
        let Ok(mesh) = packs.mesh_for_model(&p.model) else {
            continue;
        };
        let tf = placement_transform(p, RotMode::YawY);
        let (lo, hi) = pla::game::world_collision::transformed_bounds(&mesh, &tf);
        let mut grid = CollisionGrid::new(4.0);
        grid.insert(std::sync::Arc::new(mesh), tf);
        let half = Vec2::new((hi.x - lo.x) * 0.5, (hi.z - lo.z) * 0.5);
        // Aim at the vertex centroid (the AABB centre can fall outside a rotated footprint).
        let mesh = match packs.mesh_for_model(&p.model) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mut centroid = Vec2::ZERO;
        for v in &mesh.vertices {
            centroid += Vec2::new(v[0], v[2]);
        }
        centroid /= mesh.vertices.len().max(1) as f32;
        let centroid = (tf * Vec4::new(centroid.x, 0.0, centroid.y, 1.0)).truncate();
        let (mut local_miss, mut local_total) = (0usize, 0usize);
        let mut side_miss = [0usize; 4];
        for k in 0..rays {
            let side = k % 4;
            let dir = [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y][side];
            let target = Vec2::new(
                centroid.x + (rng.next_f32() - 0.5) * 1.2,
                centroid.z + (rng.next_f32() - 0.5) * 1.2,
            );
            let start = Vec2::new(target.x, target.y) - dir * (half.dot(dir.abs()) + 6.0);
            let y = terrain
                .as_ref()
                .and_then(|t| t.sample_world_height(start.x, start.y))
                .unwrap_or(0.0)
                + 1.0;
            let origin = Vec3::new(start.x, y, start.y);
            let to = Vec3::new(target.x, y, target.y) - origin;
            let dist = to.length();
            ray_total += 1;
            local_total += 1;
            if grid.raycast(origin, to / dist, dist + 2.0).is_none() {
                ray_miss += 1;
                local_miss += 1;
                side_miss[side] += 1;
            }
        }
        if local_miss > 0 {
            per_house.push((
                format!("{} sides{:?}", p.model, side_miss),
                local_miss,
                local_total,
            ));
        }
    }
    println!("rays through houses: {ray_miss}/{ray_total} crossed with no triangle hit");
    per_house.sort_by_key(|(_, m, _)| std::cmp::Reverse(*m));
    if !per_house.is_empty() {
        println!(
            "houses with misses: {:?}{}",
            &per_house[..per_house.len().min(6)],
            if per_house.len() > 6 { " ..." } else { "" }
        );
    }
}
