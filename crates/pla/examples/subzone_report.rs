//! Headless report (no window): checks the sub-area frames and the door/warp links decoded by
//! `.tools/subzone_decode.py` (`data/subzone_index.tsv`, `data/warp_links.tsv`) against the game's
//! Havok navmeshes and the exported room models.
//!
//! Usage: subzone_report   (run from the repo root; PLA_BIN_ROOT overrides the game root,
//! PLA_CACHE_ROOT the glTF cache root). Areas whose pack or glTF is missing are reported as `[skip]`.
//!
//! Checks
//! 1. Frame: per area, share of navmesh vertices inside the record's bounds box (fields 52/53, +-2 m),
//!    share of bg_parts placements inside it, share inside the placed room-model box (+-2 m; glTF bounds),
//!    and share of vertices that also sit on the PARENT area's navmesh at the same x,z (+-2 m height).
//! 2. Links: every door position lies on/near its own area's navmesh, and the arrival point (exit door +
//!    exit-door offset rotated by the exit-door yaw) lies on the destination navmesh at matching height.
//!    Both yaw conventions are scored; the one the data supports is reported.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pla::assets::navmesh::{load_pack_navmeshes, NavMesh};

type Row = HashMap<String, String>;

fn read_sheet(path: &str) -> Vec<Row> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let header: Vec<String> =
        lines.next().unwrap_or_default().split('\t').map(|h| h.split(':').next().unwrap_or("").to_string()).collect();
    lines.map(|l| header.iter().cloned().zip(l.split('\t').map(str::to_string)).collect()).collect()
}

fn num(r: &Row, k: &str) -> Option<f32> {
    r.get(k).and_then(|v| v.parse().ok())
}

fn v3(r: &Row, a: &str, b: &str, c: &str) -> Option<[f32; 3]> {
    Some([num(r, a)?, num(r, b)?, num(r, c)?])
}

/// Yaw rotation about +y in degrees. `sign = 1` is Bevy's `Quat::from_rotation_y`
/// (x' = x cos + z sin, z' = -x sin + z cos); `sign = -1` is the mirrored convention.
fn rot_y(v: [f32; 3], yaw_deg: f32, sign: f32) -> [f32; 3] {
    let (s, c) = (sign * yaw_deg.to_radians()).sin_cos();
    [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c]
}

struct Navs {
    root: PathBuf,
    cache: HashMap<String, Option<NavMesh>>,
}

impl Navs {
    fn get(&mut self, area: &str) -> Option<&NavMesh> {
        let root = self.root.clone();
        self.cache
            .entry(area.to_string())
            .or_insert_with(|| {
                let p = root.join("archive/field/terrain").join(format!("{area}.gfpak"));
                load_pack_navmeshes(&p).ok().and_then(|mut v| (!v.is_empty()).then(|| v.swap_remove(0)))
            })
            .as_ref()
    }
}

/// Union of POSITION accessor bounds of a glTF file (model-local frame; nodes have no transform).
fn gltf_bounds(path: &Path) -> Option<([f32; 3], [f32; 3])> {
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let acc = json.get("accessors")?.as_array()?;
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for mesh in json.get("meshes")?.as_array()? {
        for prim in mesh.get("primitives")?.as_array()? {
            let a = &acc[prim.get("attributes")?.get("POSITION")?.as_u64()? as usize];
            for k in 0..3 {
                lo[k] = lo[k].min(a.get("min")?.get(k)?.as_f64()? as f32);
                hi[k] = hi[k].max(a.get("max")?.get(k)?.as_f64()? as f32);
            }
        }
    }
    (lo[0] <= hi[0]).then_some((lo, hi))
}

fn inside(p: [f32; 3], lo: [f32; 3], hi: [f32; 3], margin: f32) -> bool {
    (0..3).all(|k| p[k] >= lo[k] - margin && p[k] <= hi[k] + margin)
}

fn pct(n: usize, d: usize) -> String {
    if d == 0 {
        "-".into()
    } else {
        format!("{:.1}%", 100.0 * n as f64 / d as f64)
    }
}

/// Is `p` on the navmesh at matching height? Returns (on, |dy| or horizontal miss distance).
fn on_mesh(m: &NavMesh, p: [f32; 3], tol_y: f32) -> (bool, f32) {
    match m.height_at(p[0], p[2]) {
        Some(y) => ((y - p[1]).abs() <= tol_y, (y - p[1]).abs()),
        None => match m.nearest(p[0], p[2], 1.0) {
            Some(h) => ((h.y - p[1]).abs() <= tol_y, h.distance.max((h.y - p[1]).abs())),
            None => (false, f32::INFINITY),
        },
    }
}

fn main() {
    let root = std::env::var_os("PLA_BIN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into());
    let cache_root = std::env::var_os("PLA_CACHE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/home/nico/work/decompilacion/work/cache".into());
    if !root.join("archive/field/terrain").is_dir() {
        println!("[skip] game data not found under {}", root.display());
        return;
    }
    let index = read_sheet("data/subzone_index.tsv");
    let links = read_sheet("data/warp_links.tsv");
    let places = read_sheet("data/field_object_placements.tsv");
    let mut navs = Navs { root, cache: HashMap::new() };

    // ---------------------------------------------------------------- 1. frames
    println!("== frames: navmesh (first mesh) vs record bounds, bg_parts, room-model box, parent navmesh");
    println!("area\tverts\tin_bounds\tbg_in_bounds\tin_model_box\tmodel_box_src\ton_parent_nav");
    let (mut sub_n, mut sub_bounds_ok, mut sub_model_ok, mut sub_model_n, mut sub_parent_hits) = (0, 0, 0, 0, 0);
    for r in &index {
        let area = r["area"].clone();
        let parent = r["parent"].clone();
        let (Some(lo), Some(hi)) = (
            v3(r, "bounds_min_x", "bounds_min_y", "bounds_min_z"),
            v3(r, "bounds_max_x", "bounds_max_y", "bounds_max_z"),
        ) else {
            continue;
        };
        let whole_map = lo[0] < 0.0; // -1 sentinel = whole 1025 map
        let bg: Vec<&Row> = places.iter().filter(|p| p["area"] == area).collect();
        let bg_in = bg.iter().filter(|p| v3(p, "x", "y", "z").is_some_and(|q| inside(q, lo, hi, 2.0))).count();
        // placed room-model box from glTF bounds (rotation about y only; x/z rotation must be absent)
        let mut mlo = [f32::MAX; 3];
        let mut mhi = [f32::MIN; 3];
        let mut used = Vec::new();
        // the shell model of a sub-area: its main room model(s); for areas without one, its ground models.
        // furniture (irori, chair, screens) is not a shell. A shell without an exported glTF skips the check.
        let is_room = |n: &str| n.starts_with("room_") && !n.contains("_irori") && !n.contains("_chair") && !n.contains("_screen");
        let has_room = bg.iter().any(|p| is_room(&p["model_name"]));
        let shells: Vec<&&Row> =
            bg.iter().filter(|p| if has_room { is_room(&p["model_name"]) } else { p["model_name"].starts_with("ground_") }).collect();
        let mut missing = None;
        for p in shells.iter().filter(|_| parent != area) {
            let name = &p["model_name"];
            let Some((a, b)) = gltf_bounds(&cache_root.join("gltf_field").join(&area).join(format!("{name}.gltf"))) else {
                missing = Some(name.clone());
                continue;
            };
            if num(p, "rot_x").unwrap_or(0.0) != 0.0 || num(p, "rot_z").unwrap_or(0.0) != 0.0 {
                continue;
            }
            let pos = v3(p, "x", "y", "z").unwrap_or([0.0; 3]);
            let sc = v3(p, "scale_x", "scale_y", "scale_z").unwrap_or([1.0; 3]);
            let yaw = num(p, "rot_y").unwrap_or(0.0);
            for i in 0..8 {
                let c = [
                    if i & 1 == 0 { a[0] } else { b[0] } * sc[0],
                    if i & 2 == 0 { a[1] } else { b[1] } * sc[1],
                    if i & 4 == 0 { a[2] } else { b[2] } * sc[2],
                ];
                let w = rot_y(c, yaw, 1.0);
                for k in 0..3 {
                    mlo[k] = mlo[k].min(w[k] + pos[k]);
                    mhi[k] = mhi[k].max(w[k] + pos[k]);
                }
            }
            used.push(name.clone());
        }
        let parent_mesh_heights: Option<Vec<[f32; 3]>> = if parent != area {
            navs.get(&area).map(|m| m.vertices.clone())
        } else {
            None
        };
        let on_parent = parent_mesh_heights.as_ref().and_then(|vs| {
            let pm = navs.get(&parent)?;
            Some(vs.iter().filter(|v| pm.height_at(v[0], v[2]).is_some_and(|y| (y - v[1]).abs() <= 2.0)).count())
        });
        let Some(m) = navs.get(&area) else {
            println!("{area}\t[skip] no navmesh pack");
            continue;
        };
        let n = m.vertices.len();
        let in_b = m.vertices.iter().filter(|v| inside(**v, lo, hi, 2.0)).count();
        if missing.is_some() {
            used.clear();
        }
        used.sort();
        used.dedup();
        let in_m = (!used.is_empty()).then(|| m.vertices.iter().filter(|v| inside(**v, mlo, mhi, 2.0)).count());
        println!(
            "{area}\t{n}\t{}\t{}\t{}\t{}\t{}",
            if whole_map { "whole-map".into() } else { pct(in_b, n) },
            pct(bg_in, bg.len()),
            in_m.map_or("-".into(), |k| pct(k, n)),
            match (&missing, used.is_empty()) {
                (Some(n), _) => format!("[skip] {n} has no glTF"),
                (None, true) if parent == area => "main area".to_string(),
                (None, true) => "[skip] no shell model".to_string(),
                (None, false) => used.iter().take(4).cloned().collect::<Vec<_>>().join(","),
            },
            on_parent.map_or("-".into(), |k| pct(k, n))
        );
        if parent != area {
            sub_n += 1;
            if whole_map || in_b * 10 >= n * 9 {
                sub_bounds_ok += 1;
            }
            if let Some(k) = in_m {
                sub_model_n += 1;
                if k * 10 >= n * 9 {
                    sub_model_ok += 1;
                }
            }
            if on_parent.is_some_and(|k| k * 2 > n) {
                sub_parent_hits += 1;
            }
        }
    }
    println!(
        "summary: {sub_n} sub-areas; >=90% navmesh vertices inside record bounds (+-2 m): {sub_bounds_ok}; \
         inside placed room-model box: {sub_model_ok}/{sub_model_n} with glTF; >50% vertices on the parent navmesh: {sub_parent_hits}"
    );

    // ---------------------------------------------------------------- 2. links
    println!("\n== links: door on own navmesh, arrival on destination navmesh (tolerance 1.5 m in y)");
    let by_door: HashMap<String, &Row> = links.iter().map(|r| (r["door"].clone(), r)).collect();
    let (mut door_ok, mut arr_a, mut arr_b, mut total) = (0, 0, 0, 0);
    let mut pass: Vec<String> = Vec::new();
    let mut lines = Vec::new();
    for r in &links {
        total += 1;
        let from = r["from_area"].clone();
        let to = r["to_area"].clone();
        let p = v3(r, "from_x", "from_y", "from_z").unwrap_or([0.0; 3]);
        let door_on = navs.get(&from).map(|m| match m.nearest(p[0], p[2], 3.0) {
            Some(h) => (h.y - p[1]).abs() <= 2.0,
            None => false,
        });
        let Some(exit) = by_door.get(&r["to_door"]) else { continue };
        let ep = v3(exit, "from_x", "from_y", "from_z").unwrap_or([0.0; 3]);
        let eyaw = num(exit, "from_yaw").unwrap_or(0.0);
        let off = v3(exit, "offset_x", "offset_y", "offset_z").unwrap_or([0.0; 3]);
        let arrive = |sign: f32| {
            let o = rot_y(off, eyaw, sign);
            [ep[0] + o[0], ep[1] + o[1], ep[2] + o[2]]
        };
        let (a, b) = (arrive(1.0), arrive(-1.0));
        let Some(m) = navs.get(&to) else {
            lines.push(format!("{}\t[skip] no navmesh for {to}", r["door"]));
            continue;
        };
        let (oa, da) = on_mesh(m, a, 1.5);
        let (ob, db) = on_mesh(m, b, 1.5);
        door_ok += door_on.unwrap_or(false) as usize;
        arr_a += oa as usize;
        arr_b += ob as usize;
        // nearest bg_parts object to the door (only meaningful in its own area)
        let near = places
            .iter()
            .filter(|q| q["area"] == from)
            .filter_map(|q| {
                let v = v3(q, "x", "y", "z")?;
                Some((((v[0] - p[0]).powi(2) + (v[2] - p[2]).powi(2)).sqrt(), q["model_name"].clone()))
            })
            .min_by(|x, y| x.0.total_cmp(&y.0));
        if door_on == Some(true) && oa {
            pass.push(r["door"].clone());
        }
        lines.push(format!(
            "{}\t{from}->{to}\tdoor_on_nav={}\tarrival=({:.1},{:.1},{:.1}) {} dy={:.2}\tmirrored {} \tnearest_object={}",
            r["door"],
            door_on.map_or("skip".into(), |b| b.to_string()),
            a[0],
            a[1],
            a[2],
            if oa { "ON" } else { "off" },
            da,
            if ob { "ON" } else { "off" },
            near.map_or("-".into(), |(d, n)| format!("{n} @{d:.1} m"))
        ));
        let _ = db;
    }
    for l in &lines {
        println!("{l}");
    }
    // a pair is validated end to end when both directions pass (door on its navmesh, arrival on the other)
    let pairs: Vec<(&String, &String)> = pass
        .iter()
        .filter_map(|d| by_door.get(d).map(|r| (d, &r["to_door"])))
        .filter(|(d, back)| d < back && pass.contains(back))
        .collect();
    println!(
        "summary: {total} records; door on own navmesh {door_ok}; arrival on destination navmesh: \
         yaw convention A (x'=x cos+z sin) {arr_a}, mirrored B {arr_b}; records passing both checks {}; \
         door pairs passing in both directions {}",
        pass.len(),
        pairs.len()
    );
    for (a, b) in pairs.iter().take(12) {
        println!("  pair OK: {a} <-> {b}");
    }
}
