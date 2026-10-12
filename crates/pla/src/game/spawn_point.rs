//! Safe default start position and anti-stuck helpers (pure logic, no window, no Bevy ECS).
//!
//! Problem: a fixed start (e.g. the terrain centre) can land on a small disconnected walkable
//! island or on steep ground, where the navmesh sliding collision traps the player.
//!
//! Method: faces are joined into connected components through `NavEdge::opposite_face`
//! (the navmesh stores adjacency explicitly). The safe start is a face of the LARGEST component
//! (by horizontal area), gently sloped, with walkable clearance around it, closest to a landmark
//! (village objects / first spawners) or, failing that, to the component's centroid.

use bevy::math::Vec2;
use bevy::prelude::Resource;

use crate::assets::navmesh::NavMesh;

/// Tunables of the safe-start search.
#[derive(Debug, Clone, Copy)]
pub struct SpawnConfig {
    /// Maximum face slope (angle of the face normal from vertical), degrees.
    pub max_slope_deg: f32,
    /// Required walkable radius around the start (metres).
    pub clearance: f32,
    /// Maximum height difference between the start and any clearance sample (metres).
    pub max_clearance_dy: f32,
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self { max_slope_deg: 15.0, clearance: 10.0, max_clearance_dy: 5.0 }
    }
}

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

/// The chosen safe start.
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

/// The safe start of the current area (teleport target of the pause menu and the stuck escape).
#[derive(Resource, Debug, Clone, Copy)]
pub struct SafeSpawnPoint(pub Vec2);

/// Outcome of the start-position decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StartChoice {
    /// `--player-pos`: honoured as given (the caller may still snap it onto the navmesh).
    Explicit(Vec2),
    /// Saved position that is walkable and in the largest component.
    Saved(Vec2),
    /// No requested position: the safe start.
    Safe(Vec2),
    /// The saved position was unwalkable or on a non-largest island: replaced by the safe start.
    SafeReplacingSaved(Vec2),
    /// No safe start could be computed: the caller keeps its old behaviour.
    NoSafe,
}

/// Decide the start: explicit CLI position > valid saved position > safe start.
pub fn choose_start(
    mesh: &NavMesh,
    comps: &Components,
    explicit: Option<Vec2>,
    saved: Option<Vec2>,
    safe: Option<SafeSpawn>,
) -> StartChoice {
    if let Some(p) = explicit {
        return StartChoice::Explicit(p);
    }
    if let Some(p) = saved {
        if in_main_component(mesh, comps, p.x, p.y) {
            return StartChoice::Saved(p);
        }
        return safe.map_or(StartChoice::Saved(p), |s| StartChoice::SafeReplacingSaved(s.xz));
    }
    safe.map_or(StartChoice::NoSafe, |s| StartChoice::Safe(s.xz))
}

// ------------------------------------------------------------------ landmarks

/// Column index of `name` in the TSV header (`name:type...`).
fn col(header: &[&str], name: &str) -> Option<usize> {
    header.iter().position(|h| h.split(':').next() == Some(name))
}

/// Landmark anchors for an area from the two committed tables.
/// Area 0: the placed `objects_area00_house*` models (village centre); other areas, and area 0
/// without houses: the spawners of exactly `ha_areaNN`, in table order.
pub fn landmarks_for_area(area: u8, spawn_tsv: &str, placements_tsv: &str) -> Vec<Vec2> {
    let name = format!("ha_area{area:02}");
    let rows = |text: &str, key: &str, filter: &dyn Fn(&[&str], &[&str]) -> bool| -> Vec<Vec2> {
        let mut header: Option<Vec<&str>> = None;
        let mut out = Vec::new();
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
            let f: Vec<&str> = line.split('\t').collect();
            let Some(h) = &header else {
                header = Some(f);
                continue;
            };
            let (Some(ia), Some(ix), Some(iz)) = (col(h, key), col(h, "x"), col(h, "z")) else { return out };
            if f.get(ia) != Some(&name.as_str()) || !filter(h, &f) {
                continue;
            }
            if let (Some(x), Some(z)) = (f.get(ix).and_then(|v| v.parse().ok()), f.get(iz).and_then(|v| v.parse().ok())) {
                out.push(Vec2::new(x, z));
            }
        }
        out
    };
    if area == 0 {
        let houses = rows(placements_tsv, "area", &|h, f| {
            col(h, "model_name").and_then(|i| f.get(i)).is_some_and(|m| m.contains("house"))
        });
        if !houses.is_empty() {
            return houses;
        }
    }
    rows(spawn_tsv, "area", &|_, _| true)
}

// ------------------------------------------------------------------ anti-stuck

/// Collision step: the full move, then axis-projected slides, then the move rotated by
/// +-30/60/90 deg (so a path narrower than an axis-aligned step, e.g. a diagonal ridge trail,
/// still lets the player follow it). Returns `curr` when every candidate is blocked.
pub fn slide_move(curr: Vec2, delta: Vec2, walkable: impl Fn(Vec2) -> bool) -> Vec2 {
    let target = curr + delta;
    if walkable(target) {
        return target;
    }
    if delta.x.abs() > 1e-4 && walkable(Vec2::new(target.x, curr.y)) {
        return Vec2::new(target.x, curr.y);
    }
    if delta.y.abs() > 1e-4 && walkable(Vec2::new(curr.x, target.y)) {
        return Vec2::new(curr.x, target.y);
    }
    for deg in [30.0f32, -30.0, 60.0, -60.0, 90.0, -90.0] {
        let (s, c) = deg.to_radians().sin_cos();
        let cand = curr + Vec2::new(delta.x * c - delta.y * s, delta.x * s + delta.y * c);
        if walkable(cand) {
            return cand;
        }
    }
    curr
}

/// When the current position is not walkable, move toward the nearest walkable point by at most
/// the length of `delta` (at least 0.1 m so a zero-length input still escapes while held).
/// Returns `None` when the position is walkable (normal collision applies) or nothing is near.
pub fn escape_step(
    curr: Vec2,
    delta: Vec2,
    walkable: impl Fn(Vec2) -> bool,
    nearest_walkable: impl Fn(Vec2) -> Option<Vec2>,
) -> Option<Vec2> {
    if walkable(curr) {
        return None;
    }
    let target = nearest_walkable(curr)?;
    let to = target - curr;
    let dist = to.length();
    let step = delta.length().max(0.1);
    Some(if dist <= step { target } else { curr + to / dist * step })
}

/// True when the player cannot step in any of 8 directions (`step` metres) from `curr`.
pub fn boxed_in(curr: Vec2, step: f32, walkable: impl Fn(Vec2) -> bool) -> bool {
    (0..8).all(|k| {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        !walkable(curr + Vec2::new(a.cos(), a.sin()) * step)
    })
}

/// Accumulates time spent boxed in while movement input is held.
#[derive(Debug, Default, Clone, Copy)]
pub struct StuckTracker {
    pub blocked_secs: f32,
}

impl StuckTracker {
    /// Returns true once the player has been input-held and boxed in for more than 1 s.
    pub fn update(&mut self, dt: f32, input_held: bool, boxed_in: bool) -> bool {
        if input_held && boxed_in {
            self.blocked_secs += dt;
        } else {
            self.blocked_secs = 0.0;
        }
        if self.blocked_secs > 1.0 {
            self.blocked_secs = 0.0;
            true
        } else {
            false
        }
    }
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

    #[test]
    fn landmarks_parse_houses_for_area0_and_spawners_otherwise() {
        let spawn = "# c\narea:string*\tspawner_id:string*\tx:f32\ty:f32\tz:f32\nha_area01\ta\t1.0\t0\t2.0\nha_area01_s01\tb\t9\t0\t9\nha_area01\tc\t3.0\t0\t4.0\nha_area00\td\t7\t0\t8\n";
        let place = "area:string*\tmodel_name:string?\tx:f32\ty:f32\tz:f32\nha_area00\tobjects_area00_house01\t10\t0\t20\nha_area00\tobjects_area00_obj01\t99\t0\t99\nha_area00\tobjects_area00_house02\t30\t0\t40\n";
        assert_eq!(landmarks_for_area(0, spawn, place), vec![Vec2::new(10.0, 20.0), Vec2::new(30.0, 40.0)]);
        assert_eq!(landmarks_for_area(1, spawn, place), vec![Vec2::new(1.0, 2.0), Vec2::new(3.0, 4.0)]);
        assert_eq!(landmarks_for_area(0, spawn, "area:string*\tx:f32\tz:f32\n"), vec![Vec2::new(7.0, 8.0)]);
    }

    #[test]
    fn escape_step_moves_toward_nearest_walkable_only_when_outside() {
        let walk = |p: Vec2| p.x >= 10.0;
        let near = |p: Vec2| Some(Vec2::new(10.0, p.y));
        // Walkable: normal collision applies.
        assert_eq!(escape_step(Vec2::new(11.0, 0.0), Vec2::new(0.1, 0.0), walk, near), None);
        // Outside, even if the input points away, it heads to the walkable point.
        let p = escape_step(Vec2::new(0.0, 0.0), Vec2::new(-0.1, 0.0), walk, near).unwrap();
        assert!((p.x - 0.1).abs() < 1e-5 && p.y == 0.0);
        // Close enough: snaps onto it.
        assert_eq!(escape_step(Vec2::new(9.95, 3.0), Vec2::new(0.5, 0.0), walk, near), Some(Vec2::new(10.0, 3.0)));
        // Nothing nearby: no escape.
        assert_eq!(escape_step(Vec2::ZERO, Vec2::X, walk, |_| None), None);
        // Repeated steps eventually reach walkable ground.
        let mut p = Vec2::ZERO;
        for _ in 0..200 {
            if let Some(n) = escape_step(p, Vec2::new(0.0, 0.2), walk, near) {
                p = n;
            }
        }
        assert!(walk(p));
    }

    #[test]
    fn start_choice_priority() {
        let m = two_islands();
        let c = components(&m);
        let safe = choose_safe_spawn(&m, &c, &[], &SpawnConfig::default());
        let (good, small, off) = (Vec2::new(10.5, 10.5), Vec2::new(204.0, 204.0), Vec2::new(100.0, 100.0));
        assert_eq!(choose_start(&m, &c, Some(small), None, safe), StartChoice::Explicit(small));
        assert_eq!(choose_start(&m, &c, None, Some(good), safe), StartChoice::Saved(good));
        let s = safe.unwrap().xz;
        assert_eq!(choose_start(&m, &c, None, Some(small), safe), StartChoice::SafeReplacingSaved(s));
        assert_eq!(choose_start(&m, &c, None, Some(off), safe), StartChoice::SafeReplacingSaved(s));
        assert_eq!(choose_start(&m, &c, None, None, safe), StartChoice::Safe(s));
        assert_eq!(choose_start(&m, &c, None, None, None), StartChoice::NoSafe);
    }

    #[test]
    fn slide_follows_a_diagonal_trail_that_axis_slides_cannot() {
        // Walkable only within 0.2 m of the line x == z.
        let trail = |p: Vec2| (p.x - p.y).abs() < 0.2;
        let curr = Vec2::new(5.0, 5.0);
        // Heading mostly +x: full step and both axis projections leave the trail.
        let delta = Vec2::new(0.3, 0.0);
        assert!(!trail(curr + delta) && !trail(Vec2::new(curr.x + 0.3, curr.y)));
        let moved = slide_move(curr, delta, trail);
        assert!(trail(moved) && moved != curr, "{moved:?}");
        // Fully enclosed: stays.
        assert_eq!(slide_move(curr, delta, |_| false), curr);
        // Open ground: plain move.
        assert_eq!(slide_move(curr, delta, |_| true), curr + delta);
    }

    #[test]
    fn boxed_in_and_stuck_timer() {
        let only_here = |p: Vec2| p.length() < 0.05;
        assert!(boxed_in(Vec2::ZERO, 0.3, only_here));
        assert!(!boxed_in(Vec2::ZERO, 0.3, |p: Vec2| p.x > 0.2));
        let mut t = StuckTracker::default();
        assert!(!t.update(0.6, true, true));
        assert!(t.update(0.6, true, true)); // 1.2 s
        assert!(!t.update(0.6, true, true)); // reset after firing
        assert!(!t.update(0.6, false, true)); // input released resets
        assert!(!t.update(0.6, true, false)); // not boxed in resets
        assert!(!t.update(0.6, true, true));
    }
}
