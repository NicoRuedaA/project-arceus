//! Navmesh helpers for the REPORT examples only (no gameplay code reads this).
//!
//! Movement and placement use the port's own terrain rule (`game::terrain_collision`); since
//! the user decision of 2026-10-17 the decoded navmesh is kept for the interior minimap
//! (`game::minimap`) and for the report examples that measure the decoded data
//! (`spawn_report`, `terrain_splat_report`, `terrain_layers_export`). Nothing here is called
//! by a Bevy system of the game.

use bevy::math::Vec2;

use crate::assets::navmesh::NavMesh;
use crate::game::spawn_point::SpawnConfig;

/// Connected components of walkable faces (faces joined across shared edges).
#[derive(Debug, Clone)]
pub struct Components {
    /// Component id per face, ids sorted by descending horizontal area (0 = largest).
    pub of_face: Vec<u32>,
    pub face_count: Vec<usize>,
    pub area_m2: Vec<f64>,
}

impl Components {
    pub fn len(&self) -> usize {
        self.face_count.len()
    }
    pub fn is_empty(&self) -> bool {
        self.face_count.is_empty()
    }
}

/// Horizontal (x,z) area of a face, shoelace formula.
pub fn face_area_xz(mesh: &NavMesh, face: u32) -> f32 {
    let vs = mesh.face_vertices(face);
    let mut s = 0.0;
    for i in 0..vs.len() {
        let (a, b) = (vs[i], vs[(i + 1) % vs.len()]);
        s += a[0] * b[2] - b[0] * a[2];
    }
    s.abs() * 0.5
}

/// Slope of a face in degrees (0 = flat), from its Newell normal.
pub fn face_slope_deg(mesh: &NavMesh, face: u32) -> f32 {
    let vs = mesh.face_vertices(face);
    let (mut nx, mut ny, mut nz) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..vs.len() {
        let (a, b) = (vs[i], vs[(i + 1) % vs.len()]);
        nx += (a[1] - b[1]) * (a[2] + b[2]);
        ny += (a[2] - b[2]) * (a[0] + b[0]);
        nz += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if len < 1e-12 {
        return 90.0;
    }
    (ny.abs() / len).clamp(0.0, 1.0).acos().to_degrees()
}

/// Mean of the face's vertices (x, y, z).
pub fn face_centroid(mesh: &NavMesh, face: u32) -> [f32; 3] {
    let vs = mesh.face_vertices(face);
    let n = vs.len().max(1) as f32;
    let mut c = [0.0; 3];
    for v in &vs {
        for k in 0..3 {
            c[k] += v[k] / n;
        }
    }
    c
}

/// Connected components over `opposite_face` adjacency (symmetrised).
pub fn components(mesh: &NavMesh) -> Components {
    let n = mesh.faces.len();
    let mut parent: Vec<u32> = (0..n as u32).collect();
    fn find(p: &mut [u32], mut x: u32) -> u32 {
        while p[x as usize] != x {
            p[x as usize] = p[p[x as usize] as usize];
            x = p[x as usize];
        }
        x
    }
    for (fi, f) in mesh.faces.iter().enumerate() {
        for e in f.start_edge..f.start_edge + f.num_edges as u32 {
            let o = mesh.edges[e as usize].opposite_face;
            if (o as usize) < n {
                let (a, b) = (find(&mut parent, fi as u32), find(&mut parent, o));
                if a != b {
                    parent[a as usize] = b;
                }
            }
        }
    }
    let mut root_idx = std::collections::HashMap::new();
    let mut roots = Vec::new();
    let mut raw = vec![0usize; n];
    for i in 0..n {
        let r = find(&mut parent, i as u32);
        let k = *root_idx.entry(r).or_insert_with(|| {
            roots.push(r);
            roots.len() - 1
        });
        raw[i] = k;
    }
    let mut count = vec![0usize; roots.len()];
    let mut area = vec![0.0f64; roots.len()];
    for i in 0..n {
        count[raw[i]] += 1;
        area[raw[i]] += face_area_xz(mesh, i as u32) as f64;
    }
    let mut order: Vec<usize> = (0..roots.len()).collect();
    order.sort_by(|&a, &b| area[b].partial_cmp(&area[a]).unwrap().then(count[b].cmp(&count[a])));
    let mut rank = vec![0u32; roots.len()];
    for (new, &old) in order.iter().enumerate() {
        rank[old] = new as u32;
    }
    Components {
        of_face: raw.iter().map(|&k| rank[k]).collect(),
        face_count: order.iter().map(|&o| count[o]).collect(),
        area_m2: order.iter().map(|&o| area[o]).collect(),
    }
}

/// Where a point stands relative to the walkable components.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointStatus {
    NotWalkable,
    /// `component` 0 is the largest.
    Walkable { component: u32, face: u32, y: f32 },
}

pub fn classify_point(mesh: &NavMesh, comps: &Components, x: f32, z: f32) -> PointStatus {
    match mesh.face_at(x, z) {
        None => PointStatus::NotWalkable,
        Some((face, y)) => PointStatus::Walkable { component: comps.of_face[face as usize], face, y },
    }
}

/// True if the point is walkable and inside the largest component.
pub fn in_main_component(mesh: &NavMesh, comps: &Components, x: f32, z: f32) -> bool {
    matches!(classify_point(mesh, comps, x, z), PointStatus::Walkable { component: 0, .. })
}

/// The chosen safe start (navmesh report form).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SafeSpawn {
    pub xz: Vec2,
    /// Navmesh surface height at `xz`.
    pub y: f32,
    pub face: u32,
    pub slope_deg: f32,
}

/// Whether five 24-point rings (r/5 .. r) around (x,z) are walkable, in component 0 and within `dy`.
fn has_clearance(mesh: &NavMesh, comps: &Components, x: f32, z: f32, y: f32, cfg: &SpawnConfig) -> bool {
    for ring in [0.2, 0.4, 0.6, 0.8, 1.0] {
        let r = cfg.clearance * ring;
        for k in 0..24 {
            let a = k as f32 * std::f32::consts::TAU / 24.0;
            match classify_point(mesh, comps, x + r * a.cos(), z + r * a.sin()) {
                PointStatus::Walkable { component: 0, y: py, .. } if (py - y).abs() <= cfg.max_clearance_dy => {}
                _ => return false,
            }
        }
    }
    true
}

/// Landmarks that sit on (or within 15 m of) the largest component, moved onto it. Objects such
/// as houses stand on non-walkable footprints, so a miss snaps to the nearest face in component 0.
pub fn usable_landmarks(mesh: &NavMesh, comps: &Components, landmarks: &[Vec2]) -> Vec<Vec2> {
    landmarks
        .iter()
        .filter_map(|p| {
            if in_main_component(mesh, comps, p.x, p.y) {
                return Some(*p);
            }
            let hit = mesh.nearest(p.x, p.y, 15.0)?;
            (comps.of_face[hit.face as usize] == 0).then(|| {
                let c = face_centroid(mesh, hit.face);
                Vec2::new(c[0], c[2])
            })
        })
        .take(24)
        .collect()
}

/// Pick the safe start. `landmarks` are preferred anchor points (x,z) in priority order; those
/// that lie in (or within 15 m of) the largest component are averaged (first 24). With none, the component centroid
/// is the anchor. The result is the qualifying face centre closest to the anchor.
pub fn choose_safe_spawn(mesh: &NavMesh, comps: &Components, landmarks: &[Vec2], cfg: &SpawnConfig) -> Option<SafeSpawn> {
    if comps.is_empty() {
        return None;
    }
    let usable = usable_landmarks(mesh, comps, landmarks);
    let anchor = if usable.is_empty() {
        let (mut sum, mut w) = (Vec2::ZERO, 0.0f32);
        for (fi, &c) in comps.of_face.iter().enumerate() {
            if c == 0 {
                let a = face_area_xz(mesh, fi as u32);
                let cen = face_centroid(mesh, fi as u32);
                sum += Vec2::new(cen[0], cen[2]) * a;
                w += a;
            }
        }
        if w <= 0.0 {
            return None;
        }
        sum / w
    } else {
        usable.iter().copied().sum::<Vec2>() / usable.len() as f32
    };
    let mut cands: Vec<(f32, u32)> = comps
        .of_face
        .iter()
        .enumerate()
        .filter(|(_, &c)| c == 0)
        .map(|(fi, _)| {
            let c = face_centroid(mesh, fi as u32);
            (Vec2::new(c[0], c[2]).distance_squared(anchor), fi as u32)
        })
        .collect();
    cands.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    for (_, face) in cands {
        let slope = face_slope_deg(mesh, face);
        if slope > cfg.max_slope_deg {
            continue;
        }
        let c = face_centroid(mesh, face);
        // The player's height comes from the highest face at (x,z): require it to be this one.
        match mesh.face_at(c[0], c[2]) {
            Some((f, y)) if f == face && (y - c[1]).abs() < 0.5 => {
                if has_clearance(mesh, comps, c[0], c[2], y, cfg) {
                    return Some(SafeSpawn { xz: Vec2::new(c[0], c[2]), y, face, slope_deg: slope });
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::navmesh::{NavEdge, NavFace};

    /// Append a `nx` x `nz` grid of unit quads at (x0,z0); height = `slope` * x (+ `y0`).
    fn add_grid(
        vs: &mut Vec<[f32; 3]>,
        fs: &mut Vec<NavFace>,
        es: &mut Vec<NavEdge>,
        x0: f32,
        z0: f32,
        nx: usize,
        nz: usize,
        y0: f32,
        slope: f32,
    ) {
        let base = vs.len() as u32;
        for j in 0..=nz {
            for i in 0..=nx {
                let x = x0 + i as f32;
                vs.push([x, y0 + slope * (x - x0), z0 + j as f32]);
            }
        }
        let vi = |i: usize, j: usize| base + (j * (nx + 1) + i) as u32;
        let face_base = fs.len() as u32;
        let fid = |i: usize, j: usize| face_base + (j * nx + i) as u32;
        for j in 0..nz {
            for i in 0..nx {
                let start_edge = es.len() as u32;
                // CCW sides: bottom (z-), right (x+), top (z+), left (x-).
                let sides = [
                    (vi(i, j), vi(i + 1, j), (j > 0).then(|| fid(i, j - 1))),
                    (vi(i + 1, j), vi(i + 1, j + 1), (i + 1 < nx).then(|| fid(i + 1, j))),
                    (vi(i + 1, j + 1), vi(i, j + 1), (j + 1 < nz).then(|| fid(i, j + 1))),
                    (vi(i, j + 1), vi(i, j), (i > 0).then(|| fid(i - 1, j))),
                ];
                for (a, b, opp) in sides {
                    es.push(NavEdge { a, b, opposite_edge: u32::MAX, opposite_face: opp.unwrap_or(u32::MAX), flags: 0 });
                }
                fs.push(NavFace { start_edge, num_edges: 4 });
            }
        }
    }

    fn two_islands() -> NavMesh {
        let (mut v, mut f, mut e) = (Vec::new(), Vec::new(), Vec::new());
        // Big island 60x60 at origin; small 8x8 island far away, same height, NOT connected.
        add_grid(&mut v, &mut f, &mut e, 0.0, 0.0, 60, 60, 0.0, 0.0);
        add_grid(&mut v, &mut f, &mut e, 200.0, 200.0, 8, 8, 0.0, 0.0);
        NavMesh::from_parts(v, f, e).unwrap()
    }

    #[test]
    fn components_split_islands_and_rank_by_area() {
        let m = two_islands();
        let c = components(&m);
        assert_eq!(c.len(), 2);
        assert_eq!(c.face_count, vec![3600, 64]);
        assert!((c.area_m2[0] - 3600.0).abs() < 1e-3 && (c.area_m2[1] - 64.0).abs() < 1e-3);
        assert_eq!(classify_point(&m, &c, 30.5, 30.5), PointStatus::Walkable { component: 0, face: 30 * 60 + 30, y: 0.0 });
        assert!(matches!(classify_point(&m, &c, 203.5, 203.5), PointStatus::Walkable { component: 1, .. }));
        assert_eq!(classify_point(&m, &c, 100.0, 100.0), PointStatus::NotWalkable);
    }

    #[test]
    fn picks_larger_island_even_when_landmark_is_on_the_small_one() {
        let m = two_islands();
        let c = components(&m);
        let s = choose_safe_spawn(&m, &c, &[Vec2::new(204.0, 204.0)], &SpawnConfig::default()).unwrap();
        assert!(in_main_component(&m, &c, s.xz.x, s.xz.y));
        // No usable landmark -> component centroid (30,30).
        assert!(s.xz.distance(Vec2::new(30.0, 30.0)) < 1.5, "{:?}", s.xz);
    }

    #[test]
    fn landmark_in_main_component_is_preferred() {
        let m = two_islands();
        let c = components(&m);
        let s = choose_safe_spawn(&m, &c, &[Vec2::new(20.5, 40.5)], &SpawnConfig::default()).unwrap();
        assert!(s.xz.distance(Vec2::new(20.5, 40.5)) < 1.5, "{:?}", s.xz);
    }

    #[test]
    fn small_island_alone_has_no_clearance_but_main_is_chosen() {
        let m = two_islands();
        let c = components(&m);
        // The 8x8 island cannot give 10 m clearance; anchoring there still returns main-island point.
        let s = choose_safe_spawn(&m, &c, &[], &SpawnConfig { clearance: 10.0, ..Default::default() }).unwrap();
        assert_eq!(c.of_face[s.face as usize], 0);
        // Clearance is honoured: >= 10 m from the main island's edge.
        assert!(s.xz.x >= 10.0 && s.xz.x <= 50.0 && s.xz.y >= 10.0 && s.xz.y <= 50.0, "{:?}", s.xz);
    }

    #[test]
    fn slope_filter_skips_steep_faces() {
        // Left half steep (slope 1.0 = 45 deg), right half flat, connected into one component.
        let (mut v, mut f, mut e) = (Vec::new(), Vec::new(), Vec::new());
        add_grid(&mut v, &mut f, &mut e, 0.0, 0.0, 40, 40, 0.0, 1.0);
        let m = NavMesh::from_parts(v.clone(), f.clone(), e.clone()).unwrap();
        let c = components(&m);
        assert!(face_slope_deg(&m, 0) > 40.0);
        // Everything steep: no spawn at all.
        assert!(choose_safe_spawn(&m, &c, &[], &SpawnConfig::default()).is_none());
        // A permissive limit finds one.
        assert!(choose_safe_spawn(&m, &c, &[], &SpawnConfig { max_slope_deg: 50.0, max_clearance_dy: 20.0, ..Default::default() }).is_some());
        // Flat mesh: first candidate passes.
        let (mut v, mut f, mut e) = (Vec::new(), Vec::new(), Vec::new());
        add_grid(&mut v, &mut f, &mut e, 0.0, 0.0, 40, 40, 0.0, 0.0);
        let m = NavMesh::from_parts(v, f, e).unwrap();
        let c = components(&m);
        let s = choose_safe_spawn(&m, &c, &[], &SpawnConfig::default()).unwrap();
        assert!(s.slope_deg < 1.0);
    }

    #[test]
    fn mesh_too_small_for_clearance_returns_none() {
        let (mut v, mut f, mut e) = (Vec::new(), Vec::new(), Vec::new());
        add_grid(&mut v, &mut f, &mut e, 0.0, 0.0, 15, 15, 0.0, 0.0);
        let m = NavMesh::from_parts(v, f, e).unwrap();
        let c = components(&m);
        assert!(choose_safe_spawn(&m, &c, &[], &SpawnConfig::default()).is_none());
        assert!(choose_safe_spawn(&m, &c, &[], &SpawnConfig { clearance: 4.0, ..Default::default() }).is_some());
    }
}
