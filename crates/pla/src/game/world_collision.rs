//! Wall collision for placed scenery: houses, fences, rocks, logs and water edges.
//!
//! The visual placements (`data/field_object_placements.tsv`) are shared with
//! [`super::world_objects`], and so is the streamable set: collision parses the same
//! export-authoritative rows ([`super::world_objects::parse_placements_for_spawn`]) and applies
//! the same export-index filter ([`super::world_objects::keep_streamable_placements`]). A model
//! the index does not export must stay out even when a `<model>_col.trcol` entry exists (the 7
//! blocked rows do have shapes): its collision would be an invisible wall.
//! The collision shapes live in the field model collision packs
//! (`archive/field/model/pack/collision.gfpak` and `resident_bg_parts_model.gfpak`) and are
//! addressed by the same full-path FNV-1a 64 scheme as every other GFLXPACK entry:
//!
//! ```text
//! bin/field/model/pack/<visual model>_col.trcol
//! ```
//!
//! This formula is VERIFIED here: 622 of the 655 `collision.gfpak` entries and 57 of the 57
//! shapes in `resident_bg_parts_model.gfpak` resolve through the `_col.trcol` paths listed in
//! the `resident_release.gfpak` path manifest, and all 461 distinct models placed by
//! `field_object_placements.tsv` resolve through it. The unproven entries are listed in
//! `data/field_collision_index.tsv` (see `reports/data-progress/world_collision.md`).
//!
//! The query is a pure geometric push-out of a vertical capsule against the transformed
//! triangles (no physics engine; `avian3d` is not available offline). Only wall-like triangles
//! are kept: a triangle whose normal is nearly vertical is a walkable floor/roof and is ignored.
//! The placement transform reuses [`super::world_objects::placement_rotation`], so collision and
//! visuals move together under every `--obj-rot` mode.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::prelude::*;

use super::world_objects::{
    find_data_file, keep_streamable_placements, model_paths, parse_placements_for_spawn,
    placement_rotation, should_be_alive, Placement, RotMode, LOAD_RADIUS, UNLOAD_RADIUS,
};
use super::PlayerAvatar;
use crate::assets::collision::CollisionMesh;
use crate::assets::gfpak::{fnv1a64, GfPak};

/// Side of one uniform grid cell, in metres.
pub const CELL: f32 = 8.0;
/// Player capsule used by the viewer integration.
pub const PLAYER_RADIUS: f32 = 0.35;
pub const PLAYER_HEIGHT: f32 = 1.8;
/// A triangle with |normal.y| above this is a floor/roof, not a wall.
pub const WALL_NORMAL_Y: f32 = 0.7;
/// Push-out iterations per query.
pub const MAX_ITER: usize = 4;

/// World-space collision shape of one placed model.
#[derive(Debug, Clone)]
pub struct CollisionInstance {
    pub mesh: Arc<CollisionMesh>,
    /// World transform (`scale * rotation * translation`) from the placement row.
    pub transform: Mat4,
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
}

/// Uniform 2D grid of transformed collision meshes (pure logic, no Bevy ECS).
#[derive(Debug, Default)]
pub struct CollisionGrid {
    cell: f32,
    cells: HashMap<(i32, i32), Vec<u32>>,
    instances: Vec<CollisionInstance>,
}

impl CollisionGrid {
    pub fn new(cell: f32) -> Self {
        Self {
            cell: cell.max(0.5),
            cells: HashMap::new(),
            instances: Vec::new(),
        }
    }

    pub fn cell_size(&self) -> f32 {
        self.cell
    }

    pub fn len(&self) -> usize {
        self.instances.len()
    }

    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    pub fn clear(&mut self) {
        self.cells.clear();
        self.instances.clear();
    }

    pub fn cell_of(&self, x: f32, z: f32) -> (i32, i32) {
        (
            (x / self.cell).floor() as i32,
            (z / self.cell).floor() as i32,
        )
    }

    /// Insert one transformed mesh; returns its instance id.
    pub fn insert(&mut self, mesh: Arc<CollisionMesh>, transform: Mat4) -> u32 {
        let (aabb_min, aabb_max) = transformed_bounds(&mesh, &transform);
        let id = self.instances.len() as u32;
        self.instances.push(CollisionInstance {
            mesh,
            transform,
            aabb_min,
            aabb_max,
        });
        let (x0, z0) = self.cell_of(aabb_min.x, aabb_min.z);
        let (x1, z1) = self.cell_of(aabb_max.x, aabb_max.z);
        for cx in x0..=x1 {
            for cz in z0..=z1 {
                self.cells.entry((cx, cz)).or_default().push(id);
            }
        }
        id
    }

    /// Instance ids whose cell overlaps the axis-aligned box around `center` ± `radius` in XZ.
    fn candidates(&self, center: Vec3, radius: f32) -> Vec<u32> {
        let (x0, z0) = self.cell_of(center.x - radius, center.z - radius);
        let (x1, z1) = self.cell_of(center.x + radius, center.z + radius);
        let mut out = Vec::new();
        for cx in x0..=x1 {
            for cz in z0..=z1 {
                if let Some(ids) = self.cells.get(&(cx, cz)) {
                    out.extend_from_slice(ids);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Push a vertical capsule (`center` at its middle, `radius`, `height`) out of every wall.
    /// Floor-like triangles (|normal.y| > [`WALL_NORMAL_Y`]) are ignored: the terrain and the
    /// navmesh own the vertical direction. Returns the corrected center.
    pub fn push_out_capsule(&self, center: Vec3, radius: f32, height: f32) -> Vec3 {
        let half = (height * 0.5 - radius).max(0.0);
        let mut p = center;
        for _ in 0..MAX_ITER {
            let a = p - Vec3::Y * half;
            let b = p + Vec3::Y * half;
            let mut best: Option<(f32, Vec3)> = None;
            for id in self.candidates(p, radius) {
                let inst = &self.instances[id as usize];
                if !instance_near_capsule(inst, p, radius, half) {
                    continue;
                }
                for tri in &inst.mesh.triangles {
                    let [t0, t1, t2] = tri.map(|i| {
                        let v = inst.mesh.vertices[i as usize];
                        inst.transform.transform_point3(Vec3::from(v))
                    });
                    let n = (t1 - t0).cross(t2 - t0);
                    let len = n.length();
                    if len < 1e-9 {
                        continue;
                    }
                    let n = n / len;
                    if n.y.abs() > WALL_NORMAL_Y {
                        continue; // walkable floor/roof
                    }
                    let (sp, tp, d) = closest_segment_triangle(a, b, t0, t1, t2);
                    if d >= radius {
                        continue;
                    }
                    let penetration = radius - d;
                    let dir = if d > 1e-5 {
                        (sp - tp) / d
                    } else {
                        let mut h = Vec3::new(n.x, 0.0, n.z);
                        if h.length() < 1e-5 {
                            continue;
                        }
                        h = h.normalize();
                        if h.dot(p - tp) < 0.0 {
                            h = -h;
                        }
                        h
                    };
                    if best.is_none_or(|(bp, _)| penetration > bp) {
                        best = Some((penetration, dir));
                    }
                }
            }
            match best {
                None => return p,
                Some((pen, dir)) => p += dir * (pen + 1e-3),
            }
        }
        p
    }

    /// Nearest triangle hit along a ray. Returns the distance, or `None` within `max_dist`.
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<f32> {
        self.ray_hits(origin, dir, max_dist)
            .into_iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Every triangle hit along a ray, as ray parameters.
    pub fn ray_hits(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Vec<f32> {
        let dir = dir.normalize_or_zero();
        if dir == Vec3::ZERO || max_dist <= 0.0 {
            return Vec::new();
        }
        let mut ids = Vec::new();
        let steps = (max_dist / (self.cell * 0.5)).ceil() as usize + 2;
        for k in 0..=steps {
            let s = origin + dir * (max_dist * k as f32 / steps as f32);
            for id in self.candidates(s, 0.0) {
                ids.push(id);
            }
        }
        ids.sort_unstable();
        ids.dedup();
        let mut out = Vec::new();
        for id in ids {
            let inst = &self.instances[id as usize];
            if !ray_hits_aabb(origin, dir, max_dist, inst.aabb_min, inst.aabb_max) {
                continue;
            }
            for tri in &inst.mesh.triangles {
                let [t0, t1, t2] = tri.map(|i| {
                    let v = inst.mesh.vertices[i as usize];
                    inst.transform.transform_point3(Vec3::from(v))
                });
                if let Some(t) = ray_triangle(origin, dir, t0, t1, t2) {
                    if t <= max_dist {
                        out.push(t);
                    }
                }
            }
        }
        // Two triangles sharing an edge both report a hit on that edge; keep one.
        out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        out.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
        out
    }

    /// Inside/outside test by odd parity along three axes (majority vote).
    /// A closed shell reports inside for points within it; open shells are not reliable.
    pub fn point_inside_shell(&self, p: Vec3, max_dist: f32) -> bool {
        let axes = [Vec3::X, Vec3::Y, Vec3::Z];
        let odd = axes
            .iter()
            .filter(|d| self.ray_hits(p, **d, max_dist).len() % 2 == 1)
            .count();
        odd >= 2
    }
}

/// World AABB of a mesh under an affine transform (all eight local AABB corners).
pub fn transformed_bounds(mesh: &CollisionMesh, transform: &Mat4) -> (Vec3, Vec3) {
    let Some((lo, hi)) = mesh.bounds() else {
        return (Vec3::ZERO, Vec3::ZERO);
    };
    let (lo, hi) = (Vec3::from(lo), Vec3::from(hi));
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for k in 0..8 {
        let c = Vec3::new(
            if k & 1 == 0 { lo.x } else { hi.x },
            if k & 2 == 0 { lo.y } else { hi.y },
            if k & 4 == 0 { lo.z } else { hi.z },
        );
        let p = transform.transform_point3(c);
        min = min.min(p);
        max = max.max(p);
    }
    (min, max)
}

/// Placement transform: same convention as the visual `world_objects` spawner.
pub fn placement_transform(p: &Placement, mode: RotMode) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from(p.scale),
        placement_rotation(p, mode),
        Vec3::from(p.pos),
    )
}

/// True when the instance AABB overlaps the capsule's XZ square and Y span.
pub fn instance_near_capsule(
    inst: &CollisionInstance,
    center: Vec3,
    radius: f32,
    half: f32,
) -> bool {
    center.x + radius >= inst.aabb_min.x
        && center.x - radius <= inst.aabb_max.x
        && center.z + radius >= inst.aabb_min.z
        && center.z - radius <= inst.aabb_max.z
        && center.y + half + radius >= inst.aabb_min.y
        && center.y - half - radius <= inst.aabb_max.y
}

/// `t` of the ray/AABB slab test, or `None`.
pub fn ray_hits_aabb(origin: Vec3, dir: Vec3, max_dist: f32, min: Vec3, max: Vec3) -> bool {
    let mut t0 = 0.0f32;
    let mut t1 = max_dist;
    for k in 0..3 {
        let (o, d, lo, hi) = (origin[k], dir[k], min[k], max[k]);
        if d.abs() < 1e-9 {
            if o < lo || o > hi {
                return false;
            }
            continue;
        }
        let (mut a, mut b) = ((lo - o) / d, (hi - o) / d);
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        t0 = t0.max(a);
        t1 = t1.min(b);
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// Möller–Trumbore; returns the ray parameter `t` when it hits inside the triangle.
pub fn ray_triangle(origin: Vec3, dir: Vec3, t0: Vec3, t1: Vec3, t2: Vec3) -> Option<f32> {
    let e1 = t1 - t0;
    let e2 = t2 - t0;
    let pv = dir.cross(e2);
    let det = e1.dot(pv);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let tv = origin - t0;
    let u = tv.dot(pv) * inv;
    if !(-1e-6..=1.0 + 1e-6).contains(&u) {
        return None;
    }
    let qv = tv.cross(e1);
    let v = dir.dot(qv) * inv;
    if v < -1e-6 || u + v > 1.0 + 1e-6 {
        return None;
    }
    let t = e2.dot(qv) * inv;
    (t >= 0.0).then_some(t)
}

// ------------------------------------------------------------------ closest points

/// Closest point on segment `[a, b]` to point `p`.
pub fn closest_point_on_segment(p: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 < 1e-12 {
        return a;
    }
    a + ab * ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
}

/// Closest points between segments `[p1, q1]` and `[p2, q2]`.
pub fn closest_points_on_segments(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (Vec3, Vec3) {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.length_squared();
    let e = d2.length_squared();
    let f = d2.dot(r);
    let (mut s, mut t);
    if a < 1e-12 && e < 1e-12 {
        return (p1, p2);
    }
    if a < 1e-12 {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e < 1e-12 {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            s = if denom > 1e-12 {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
        }
    }
    (p1 + d1 * s, p2 + d2 * t)
}

/// Closest point on a triangle to `p` (Ericson), including the face interior.
pub fn closest_point_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// Closest pair (point on segment `[a, b]`, point on triangle) and their distance.
/// Exact for convex features: segment endpoints against the whole triangle surface
/// (including its interior), plus all three triangle edges against the segment.
pub fn closest_segment_triangle(
    a: Vec3,
    b: Vec3,
    t0: Vec3,
    t1: Vec3,
    t2: Vec3,
) -> (Vec3, Vec3, f32) {
    let mut best: Option<(Vec3, Vec3, f32)> = None;
    let mut consider = |sp: Vec3, tp: Vec3| {
        let d = (sp - tp).length();
        if best.is_none_or(|(_, _, bd)| d < bd) {
            best = Some((sp, tp, d));
        }
    };
    for s in [a, b] {
        consider(s, closest_point_on_triangle(s, t0, t1, t2));
    }
    for (u, w) in [(t0, t1), (t1, t2), (t2, t0)] {
        let (sp, tp) = closest_points_on_segments(a, b, u, w);
        consider(sp, tp);
    }
    if let Some(p) = segment_triangle_hit(a, b, t0, t1, t2) {
        consider(p, p);
    }
    best.unwrap_or((a, t0, (a - t0).length()))
}

/// Intersection of a segment with a triangle surface (both sides), if any.
pub fn segment_triangle_hit(a: Vec3, b: Vec3, t0: Vec3, t1: Vec3, t2: Vec3) -> Option<Vec3> {
    let dir = b - a;
    let len = dir.length();
    if len < 1e-9 {
        return None;
    }
    let t = ray_triangle(a, dir / len, t0, t1, t2)?;
    (t <= len).then(|| a + dir * (t / len))
}

// ------------------------------------------------------------------ collision packs

/// One loaded GFLXPACK collision pack (entry hashes + payload bytes kept for lazy parsing).
pub struct CollisionPack {
    pub name: String,
    pub entry_count: usize,
    pak: GfPak,
    bytes: Vec<u8>,
}

/// The two field collision packs, loaded once, meshes parsed on demand.
pub struct CollisionPacks {
    pub packs: Vec<CollisionPack>,
}

impl CollisionPacks {
    /// Pack file stems, in lookup order.
    pub const PACK_FILES: [&'static str; 2] = ["collision", "resident_bg_parts_model"];

    /// Load the packs of a game root (`.../romfs_patched/bin`). A pack that is missing or
    /// malformed is skipped with the reason kept in the error list; both failing is an error.
    pub fn load(bin_root: impl AsRef<Path>) -> Result<Self, String> {
        let root = bin_root.as_ref();
        let mut packs = Vec::new();
        let mut errors = Vec::new();
        for name in Self::PACK_FILES {
            let path = root
                .join("archive/field/model/pack")
                .join(format!("{name}.gfpak"));
            match std::fs::read(&path) {
                Ok(bytes) => match GfPak::parse(&bytes) {
                    Ok(pak) => packs.push(CollisionPack {
                        name: name.to_string(),
                        entry_count: pak.entry_count as usize,
                        pak,
                        bytes,
                    }),
                    Err(e) => errors.push(format!("{}: {e}", path.display())),
                },
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
        if packs.is_empty() {
            return Err(format!("no collision pack loaded ({})", errors.join("; ")));
        }
        if !errors.is_empty() {
            eprintln!("World collision: {}", errors.join("; "));
        }
        Ok(Self { packs })
    }

    /// Verified GFLXPACK path of the collision shape of a placed model.
    pub fn collision_path(model: &str) -> String {
        format!("bin/field/model/pack/{model}_col.trcol")
    }

    /// `(pack index, entry index)` whose stored path hash equals the hash of `path`.
    /// Packs are searched in [`Self::PACK_FILES`] order (`collision.gfpak` first: a model may
    /// ship the same shape in both packs). A duplicate hash inside one pack is ambiguous.
    pub fn entry_by_path(&self, path: &str) -> Option<(usize, usize)> {
        let hash = fnv1a64(path.as_bytes());
        for (i, pack) in self.packs.iter().enumerate() {
            let mut found = None;
            for (e, h) in pack.pak.entry_path_hashes.iter().enumerate() {
                if *h == hash {
                    if found.is_some() {
                        return None; // ambiguous inside one pack
                    }
                    found = Some((i, e));
                }
            }
            if found.is_some() {
                return found;
            }
        }
        None
    }

    /// Parse the mesh of a resolved entry.
    pub fn mesh(&self, pack: usize, entry: usize) -> Result<CollisionMesh, String> {
        let p = &self.packs[pack];
        let data = p.pak.decoded_data(&p.bytes, &p.pak.entries[entry])?;
        CollisionMesh::parse(&data)
    }

    /// Pack file stem by index.
    pub fn pack_name(&self, pack: usize) -> &str {
        &self.packs[pack].name
    }

    /// Stored full-path hashes of one pack, indexed like its entries.
    pub fn entry_hashes(&self, pack: usize) -> &[u64] {
        &self.packs[pack].pak.entry_path_hashes
    }

    /// Number of entries in one pack.
    pub fn pack_entry_count(&self, pack: usize) -> usize {
        self.packs[pack].entry_count
    }

    /// True when an entry's decoded payload is a Havok TAG0 tagfile carrying `hknpMeshShape`.
    pub fn is_collision_entry(&self, pack: usize, entry: usize) -> bool {
        let p = &self.packs[pack];
        let Ok(data) = p.pak.decoded_data(&p.bytes, &p.pak.entries[entry]) else {
            return false;
        };
        data.get(4..8) == Some(b"TAG0")
            && data
                .windows(14)
                .any(|w| w == b"hknpMeshShape\0" || &w[..13] == b"hknpMeshShape")
    }

    /// Parse the collision mesh of a placed model, if its pack entry exists.
    pub fn mesh_for_model(&self, model: &str) -> Result<CollisionMesh, String> {
        let path = Self::collision_path(model);
        let (pack, entry) = self
            .entry_by_path(&path)
            .ok_or_else(|| format!("no collision entry for {path}"))?;
        self.mesh(pack, entry)
    }

    /// Whether a placed model has a collision entry (no parsing).
    pub fn has_collision_entry(&self, model: &str) -> bool {
        self.entry_by_path(&Self::collision_path(model)).is_some()
    }

    /// Total parsed-entry count over all packs (for reports).
    pub fn entry_count(&self) -> usize {
        self.packs.iter().map(|p| p.entry_count).sum()
    }
}

// ------------------------------------------------------------------ Bevy integration

pub const STREAM_LOAD: f32 = LOAD_RADIUS;
pub const STREAM_UNLOAD: f32 = UNLOAD_RADIUS;

/// Streaming collision world of one main area.
#[derive(Resource)]
pub struct WorldCollision {
    grid: CollisionGrid,
    packs: CollisionPacks,
    placements: Vec<Placement>,
    /// Export-index table of the spawnable models (see [`model_paths`]): zone changes filter the
    /// new area's rows with it, exactly like the visual objects do.
    paths: HashMap<String, (PathBuf, String)>,
    /// Export-index cache root; a zone change re-resolves the paths per zone pack (the interior
    /// scenes of Brief 37 each have their own exported folder).
    cache_root: PathBuf,
    /// Per-model parsed mesh (`None` = the model has no collision entry).
    models: HashMap<String, Option<Arc<CollisionMesh>>>,
    alive: Vec<bool>,
    rot_mode: RotMode,
    /// Diagnostics.
    pub placements_total: usize,
    pub placements_with_collision: usize,
    pub meshes_parsed: usize,
    pub triangles: usize,
}

impl WorldCollision {
    /// Push a capsule out of every wall near its XZ position (y = capsule middle).
    pub fn push_out_xz(&self, x: f32, y: f32, z: f32, radius: f32, height: f32) -> Vec2 {
        let out = self
            .grid
            .push_out_capsule(Vec3::new(x, y, z), radius, height);
        Vec2::new(out.x, out.z)
    }

    /// Push the player capsule out of walls (`PLAYER_RADIUS` × `PLAYER_HEIGHT`).
    pub fn push_player_out(&self, xz: Vec2, y: f32) -> Vec2 {
        self.push_out_xz(xz.x, y, xz.y, PLAYER_RADIUS, PLAYER_HEIGHT)
    }

    /// Number of live collision instances (for tests/probes).
    pub fn instance_count(&self) -> usize {
        self.grid.len()
    }

    /// Live instances within `radius` of a point (for tests/probes).
    pub fn placement_count(&self) -> usize {
        self.placements.len()
    }

    fn mesh_for(&mut self, model: &str) -> Option<Arc<CollisionMesh>> {
        if let Some(cached) = self.models.get(model) {
            return cached.clone();
        }
        let mesh = match self.packs.mesh_for_model(model) {
            Ok(m) => {
                self.meshes_parsed += 1;
                self.triangles += m.triangles.len();
                Some(Arc::new(m))
            }
            Err(_) => None,
        };
        self.models.insert(model.to_string(), mesh.clone());
        mesh
    }

    /// Rebuild the grid from the live placement set.
    fn rebuild(&mut self) {
        self.grid.clear();
        for i in 0..self.placements.len() {
            if !self.alive[i] {
                continue;
            }
            let model = self.placements[i].model.clone();
            let Some(mesh) = self.mesh_for(&model) else {
                continue;
            };
            let tf = placement_transform(&self.placements[i], self.rot_mode);
            self.grid.insert(mesh, tf);
        }
    }

    /// Move the streaming radius to the player: insert placements entering the load radius and
    /// drop those leaving the unload radius (same hysteresis as the visual objects).
    pub fn stream_to(&mut self, player: Vec2) {
        self.stream_to_scaled(player, 1.0);
    }

    /// [`Self::stream_to`] with the Graphics screen "Collision range" multiplier (1.0 = 200/230 m).
    pub fn stream_to_scaled(&mut self, player: Vec2, scale: f32) {
        let (load, unload) = (STREAM_LOAD * scale, STREAM_UNLOAD * scale);
        let mut changed = false;
        for i in 0..self.placements.len() {
            let p = &self.placements[i];
            let want = should_be_alive(
                player,
                Vec2::new(p.pos[0], p.pos[2]),
                self.alive[i],
                load,
                unload,
            );
            if want != self.alive[i] {
                self.alive[i] = want;
                changed = true;
            }
        }
        if changed {
            self.rebuild();
        }
    }

    /// Install a new placement set (zone change): the old area's instances leave the grid at
    /// once and the new ones stream in around the player on the next [`Self::stream_to`].
    /// The parsed-mesh cache is kept: it is keyed by model name, not by area.
    pub fn replace_placements(&mut self, placements: Vec<Placement>) {
        self.placements_total = placements.len();
        self.placements_with_collision = placements
            .iter()
            .filter(|p| self.packs.has_collision_entry(&p.model))
            .count();
        self.alive = vec![false; placements.len()];
        self.placements = placements;
        self.rebuild();
    }

    /// Switch to main area `area` (`ha_areaNN`) reusing the loaded packs. Areas above 5 and an
    /// unreadable placement sheet leave NO placed walls (an empty set), never the old area's
    /// walls, which live in another area's frame.
    pub fn switch_area(&mut self, area: u8) -> Result<(), String> {
        if area > 5 {
            self.replace_placements(Vec::new());
            return Err(format!("ha_area{area:02} has no placed-object collision"));
        }
        self.switch_zone(&format!("ha_area{area:02}"))
    }

    /// [`Self::switch_area`] for any zone id (main area or interior sub-area, Brief 37): the
    /// streamable-table filter is re-resolved against the zone's own export pack first, so the
    /// room shells of an interior keep their walls.
    pub fn switch_zone(&mut self, zone: &str) -> Result<(), String> {
        let Some(text) = find_data_file("field_object_placements.tsv")
            .and_then(|p| std::fs::read_to_string(p).ok())
        else {
            self.replace_placements(Vec::new());
            return Err("data/field_object_placements.tsv not found".into());
        };
        let paths = if self.cache_root.as_os_str().is_empty() {
            self.paths.clone()
        } else {
            super::world_objects::paths_for_zone(zone, &self.cache_root)
        };
        let placements = Self::streamable_placements(&text, zone, &paths);
        self.paths = paths;
        self.replace_placements(placements);
        Ok(())
    }

    /// The placements collision covers: the spawn-authoritative parse of `area` restricted to
    /// the models the export index can draw (the same parse + filter
    /// [`super::world_objects::WorldObjectsPlugin`] streams, so walls never outlive the visual
    /// model; `exclude_models` is not applied here because the spawner's `.trins`-tree exclusion
    /// is a visual-duplicate rule, not a collision rule).
    fn streamable_placements(
        text: &str,
        area: &str,
        paths: &HashMap<String, (PathBuf, String)>,
    ) -> Vec<Placement> {
        keep_streamable_placements(
            parse_placements_for_spawn(text, area),
            &HashSet::new(),
            paths,
        )
    }

    /// Build the resource for one area from the committed placement sheet and the game packs.
    /// `cache_root` resolves the export index (`PLA_CACHE_ROOT` in the viewer).
    pub fn load(
        bin_root: impl AsRef<Path>,
        area: u8,
        rot_mode: RotMode,
        cache_root: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let packs = CollisionPacks::load(bin_root)?;
        let Some(text) = find_data_file("field_object_placements.tsv")
            .and_then(|p| std::fs::read_to_string(p).ok())
        else {
            return Err("data/field_object_placements.tsv not found".into());
        };
        let area_name = format!("ha_area{area:02}");
        let paths = model_paths(cache_root.as_ref());
        let placements = Self::streamable_placements(&text, &area_name, &paths);
        let placements_total = placements.len();
        // Coverage is counted from the verified path hashes; meshes parse lazily on first use.
        let placements_with_collision = placements
            .iter()
            .filter(|p| packs.has_collision_entry(&p.model))
            .count();
        let mut wc = Self {
            grid: CollisionGrid::new(CELL),
            packs,
            placements,
            paths,
            cache_root: cache_root.as_ref().to_path_buf(),
            models: HashMap::new(),
            alive: vec![false; placements_total],
            rot_mode,
            placements_total,
            placements_with_collision,
            meshes_parsed: 0,
            triangles: 0,
        };
        wc.rebuild();
        Ok(wc)
    }
}

fn stream_collision(
    mut wc: ResMut<WorldCollision>,
    player: Query<&Transform, With<PlayerAvatar>>,
    dist: Option<Res<super::quality::StreamDistances>>,
) {
    let Some(t) = player.iter().next() else {
        return;
    };
    let p = Vec2::new(t.translation.x, t.translation.z);
    wc.stream_to_scaled(p, super::quality::scale_of(dist.as_deref(), |d| d.collision));
}

/// Streams the collision shapes of placed objects around the player.
pub struct WorldCollisionPlugin {
    pub area: u8,
    pub bin_root: PathBuf,
    /// Export-index cache root for the streamable-model filter (`PLA_CACHE_ROOT` in the viewer).
    pub cache_root: PathBuf,
    pub rot_mode: RotMode,
}

impl Plugin for WorldCollisionPlugin {
    fn build(&self, app: &mut App) {
        if self.area > 5 {
            return;
        }
        match WorldCollision::load(&self.bin_root, self.area, self.rot_mode, &self.cache_root) {
            Ok(wc) => {
                println!(
                    "World collision area {:02}: {} placements, {} with a collision shape; streaming within {} m.",
                    self.area, wc.placements_total, wc.placements_with_collision, STREAM_LOAD
                );
                app.insert_resource(wc);
                app.add_systems(Update, stream_collision);
            }
            Err(e) => println!("World collision: {e}; skipping."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall_mesh() -> CollisionMesh {
        // Vertical quad on the x = 0 plane, z in [-5, 5], y in [0, 3], normal -x.
        CollisionMesh {
            vertices: vec![
                [0.0, 0.0, -5.0],
                [0.0, 3.0, -5.0],
                [0.0, 3.0, 5.0],
                [0.0, 0.0, 5.0],
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            quad_count: 1,
            section_count: 1,
            material_names: vec!["wall".into()],
        }
    }

    fn floor_mesh() -> CollisionMesh {
        // Horizontal quad at y = 0 under the capsule (normal +y) must be ignored.
        CollisionMesh {
            vertices: vec![
                [-5.0, 0.0, -5.0],
                [-5.0, 0.0, 5.0],
                [5.0, 0.0, 5.0],
                [5.0, 0.0, -5.0],
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            quad_count: 1,
            section_count: 1,
            material_names: vec!["floor".into()],
        }
    }

    fn l_wall_mesh() -> CollisionMesh {
        // Two perpendicular walls: x = 0 (z >= -5) and z = 0 (x <= 5), corner at the origin.
        CollisionMesh {
            vertices: vec![
                [0.0, 0.0, 0.0],
                [0.0, 3.0, 0.0],
                [0.0, 3.0, 5.0],
                [0.0, 0.0, 5.0],
                [5.0, 0.0, 0.0],
                [5.0, 3.0, 0.0],
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3], [0, 4, 5]],
            quad_count: 0,
            section_count: 2,
            material_names: vec!["wall".into()],
        }
    }

    fn box_shell_mesh() -> CollisionMesh {
        // Closed box [-1, 1]^3 as 12 wall triangles (8 mm-sized corners, 2 m tall).
        let v = [
            [-1.0, 0.0, -1.0],
            [1.0, 0.0, -1.0],
            [1.0, 0.0, 1.0],
            [-1.0, 0.0, 1.0],
            [-1.0, 2.0, -1.0],
            [1.0, 2.0, -1.0],
            [1.0, 2.0, 1.0],
            [-1.0, 2.0, 1.0],
        ];
        let quads = [
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
            [4, 5, 6, 7], // roof
            [0, 1, 2, 3], // floor
        ];
        let mut triangles = Vec::new();
        for [a, b, c, d] in quads {
            triangles.push([a, b, c]);
            triangles.push([a, c, d]);
        }
        CollisionMesh {
            vertices: v.to_vec(),
            triangles,
            quad_count: 6,
            section_count: 1,
            material_names: vec!["box".into()],
        }
    }

    #[test]
    fn replacing_placements_drops_the_old_area_walls() {
        let mut wc = WorldCollision {
            grid: CollisionGrid::new(CELL),
            packs: CollisionPacks { packs: Vec::new() },
            placements: Vec::new(),
            paths: HashMap::new(),
            cache_root: PathBuf::new(),
            models: HashMap::new(),
            alive: Vec::new(),
            rot_mode: RotMode::YawY,
            placements_total: 0,
            placements_with_collision: 0,
            meshes_parsed: 0,
            triangles: 0,
        };
        wc.grid.insert(Arc::new(wall_mesh()), Mat4::IDENTITY);
        let start = Vec2::new(0.1, 0.0);
        assert_ne!(wc.push_player_out(start, 0.9), start, "the wall pushes before the switch");
        wc.replace_placements(Vec::new());
        assert_eq!(wc.instance_count(), 0);
        assert_eq!(wc.push_player_out(start, 0.9), start, "no wall after the zone change");
        assert!(wc.switch_area(6).is_err());
        assert_eq!(wc.instance_count(), 0);
    }

    #[test]
    fn parity_detects_inside_a_closed_box() {
        let mut grid = CollisionGrid::new(4.0);
        grid.insert(Arc::new(box_shell_mesh()), Mat4::IDENTITY);
        assert!(grid.point_inside_shell(Vec3::new(0.0, 1.0, 0.0), 20.0));
        assert!(grid.point_inside_shell(Vec3::new(0.13, 1.1, 0.27), 20.0));
        assert!(!grid.point_inside_shell(Vec3::new(3.0, 1.0, 0.0), 20.0));
        // A ray through the box centre crosses both wall triangles on their shared diagonal;
        // the near-duplicate dedupe must count it once.
        assert_eq!(
            grid.ray_hits(Vec3::new(0.0, 1.0, 0.0), Vec3::X, 20.0).len(),
            1
        );
        assert_eq!(
            grid.ray_hits(Vec3::new(-3.0, 1.0, 0.0), Vec3::X, 20.0)
                .len(),
            2
        );
    }

    #[test]
    fn parallel_segment_uses_the_triangle_face_not_the_diagonal() {
        // Vertical segment parallel to the wall plane, in front of the quad centre. The old
        // vertex/edge enumeration returned the diagonal edge (slightly farther), which left a
        // residual penetration; the face interior must win.
        let (sp, tp, d) = closest_segment_triangle(
            Vec3::new(-0.3, 0.35, 0.0),
            Vec3::new(-0.3, 1.45, 0.0),
            [0.0, 0.0, -5.0].into(),
            [0.0, 0.0, 5.0].into(),
            [0.0, 3.0, 5.0].into(),
        );
        assert!((d - 0.3).abs() < 1e-6, "distance {d}");
        assert!(
            (sp.x - -0.3).abs() < 1e-6 && (tp.x).abs() < 1e-6,
            "points {sp:?} {tp:?}"
        );
    }

    #[test]
    fn capsule_walks_into_a_wall_and_stops() {
        let mut grid = CollisionGrid::new(4.0);
        grid.insert(Arc::new(wall_mesh()), Mat4::IDENTITY);
        let mut p = Vec3::new(-2.0, 0.9, 0.0);
        for _ in 0..80 {
            p.x += 0.1;
            p = grid.push_out_capsule(p, 0.35, 1.8);
            assert!(p.x <= -0.35 + 5e-3, "penetrated the wall: {p:?}");
        }
        assert!(p.x > -0.5, "should rest against the wall, got {p:?}");
    }

    #[test]
    fn floors_are_ignored_but_ceiling_like_is_too() {
        let mut grid = CollisionGrid::new(4.0);
        grid.insert(Arc::new(floor_mesh()), Mat4::IDENTITY);
        let p = Vec3::new(0.0, 0.9, 0.0);
        assert_eq!(
            grid.push_out_capsule(p, 0.35, 1.8),
            p,
            "floor must not push"
        );
        // Walking across the floor mesh is free everywhere.
        for x in -4..=4 {
            let q = Vec3::new(x as f32, 0.9, 0.0);
            assert_eq!(grid.push_out_capsule(q, 0.35, 1.8), q);
        }
    }

    #[test]
    fn corner_resolves_out_of_both_walls() {
        let mut grid = CollisionGrid::new(4.0);
        grid.insert(Arc::new(l_wall_mesh()), Mat4::IDENTITY);
        let p = Vec3::new(-0.4, 0.9, -0.4);
        let out = grid.push_out_capsule(p, 0.35, 1.8);
        assert!(out.x <= -0.35 + 5e-3, "still inside the x wall: {out:?}");
        assert!(out.z <= -0.35 + 5e-3, "still inside the z wall: {out:?}");
    }

    #[test]
    fn rotation_moves_the_wall() {
        let mut grid = CollisionGrid::new(4.0);
        // Rotate the x = 0 wall by 90 deg: it becomes the z = 0 wall.
        grid.insert(
            Arc::new(wall_mesh()),
            Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2),
        );
        let p = Vec3::new(0.0, 0.9, -1.0);
        let out = grid.push_out_capsule(p, 0.35, 1.8);
        assert!(
            out.z <= -0.35 + 5e-3,
            "not pushed away from the rotated wall: {out:?}"
        );
    }

    #[test]
    fn far_instances_do_not_push() {
        let mut grid = CollisionGrid::new(4.0);
        grid.insert(
            Arc::new(wall_mesh()),
            Mat4::from_translation(Vec3::new(100.0, 0.0, 0.0)),
        );
        let p = Vec3::new(0.0, 0.9, 0.0);
        assert_eq!(grid.push_out_capsule(p, 0.35, 1.8), p);
        assert_eq!(grid.len(), 1);
        grid.clear();
        assert!(grid.is_empty());
    }

    #[test]
    fn raycast_hits_the_wall_at_the_expected_distance() {
        let mut grid = CollisionGrid::new(4.0);
        grid.insert(Arc::new(wall_mesh()), Mat4::IDENTITY);
        let hit = grid
            .raycast(Vec3::new(-2.0, 1.0, 0.0), Vec3::X, 10.0)
            .unwrap();
        assert!((hit - 2.0).abs() < 1e-3, "hit at {hit}");
        assert!(grid
            .raycast(Vec3::new(-2.0, 1.0, 20.0), Vec3::X, 10.0)
            .is_none());
    }

    #[test]
    fn placement_transform_matches_world_objects() {
        let p = Placement {
            model: "objects_area00_house17_b".into(),
            pos: [10.0, 2.0, 30.0],
            rot_deg: [0.0, 90.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        };
        for mode in [RotMode::YawY, RotMode::YawYNeg, RotMode::YawYFlip] {
            let mine = placement_transform(&p, mode);
            let visual = Mat4::from_scale_rotation_translation(
                Vec3::from(p.scale),
                placement_rotation(&p, mode),
                Vec3::from(p.pos),
            );
            assert_eq!(
                mine, visual,
                "collision and visuals must share the transform"
            );
        }
    }

    #[test]
    fn collision_path_formula_is_stable() {
        assert_eq!(
            CollisionPacks::collision_path("objects_area00_house17_b"),
            "bin/field/model/pack/objects_area00_house17_b_col.trcol"
        );
        assert_eq!(
            fnv1a64(CollisionPacks::collision_path("gimmick_area03_tree01").as_bytes()),
            0x408b8f76354e9697,
            "entry 0 of collision.gfpak is gimmick_area03_tree01_col.trcol"
        );
    }

    fn bin_root() -> PathBuf {
        std::env::var_os("PLA_BIN_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into()
            })
    }

    fn cache_root() -> PathBuf {
        std::env::var_os("PLA_CACHE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(super::super::world_objects::DEFAULT_CACHE_ROOT))
    }

    /// The game packs and the exported glTF cache, or `None` (test skipped).
    fn game_data() -> Option<(PathBuf, PathBuf)> {
        let bin = bin_root();
        let cache = cache_root();
        (bin.join("archive/field/model/pack/collision.gfpak")
            .exists()
            && cache.is_dir())
        .then_some((bin, cache))
    }

    #[test]
    fn real_resource_streams_and_pushes() {
        let Some((root, cache)) = game_data() else {
            eprintln!("skip: game data not present");
            return;
        };
        let mut wc = WorldCollision::load(&root, 0, RotMode::YawY, &cache).unwrap();
        // The export-authoritative streamable set (was 195 under the stale sheet snapshot; the
        // graficos2 fix adds the blocked-but-exported houses).
        assert_eq!(wc.placements_total, 236);
        assert_eq!(wc.placements_with_collision, 236);
        assert_eq!(
            wc.instance_count(),
            0,
            "nothing streams before a player position"
        );
        // Safe start of ha_area00 (see collision_navmesh.md); houses are nearby.
        wc.stream_to(Vec2::new(445.2, 493.4));
        assert!(
            wc.instance_count() > 10,
            "expected wall instances, got {}",
            wc.instance_count()
        );
        assert!(wc.meshes_parsed > 0);
        // Far away: every instance leaves the unload radius.
        wc.stream_to(Vec2::new(100_000.0, 100_000.0));
        assert_eq!(wc.instance_count(), 0);
    }

    #[test]
    fn real_pack_resolves_placed_models() {
        let Some((root, cache)) = game_data() else {
            eprintln!("skip: game data not present");
            return;
        };
        let packs = CollisionPacks::load(&root).unwrap();
        assert!(!packs.packs.is_empty());
        let path = CollisionPacks::collision_path("gimmick_area03_tree01");
        let (pack, entry) = packs.entry_by_path(&path).expect("verified manifest entry");
        assert_eq!((pack, entry), (0, 0));
        let mesh = packs.mesh(pack, entry).unwrap();
        assert!(!mesh.triangles.is_empty(), "entry 0 must carry geometry");

        // Every model the spawn path can stream in area 00 must resolve to a collision shape.
        let text = std::fs::read_to_string(find_data_file("field_object_placements.tsv").unwrap())
            .unwrap();
        let placements =
            WorldCollision::streamable_placements(&text, "ha_area00", &model_paths(&cache));
        assert!(!placements.is_empty());
        let mut missing = Vec::new();
        for p in &placements {
            if packs.mesh_for_model(&p.model).is_err() {
                missing.push(p.model.clone());
            }
        }
        assert!(
            missing.is_empty(),
            "area00 models without collision: {missing:?}"
        );
    }

    /// The graficos2 follow-up: the houses the spawn path started streaming (`blocked` in the
    /// sheet, exported `ok`) were missing from the collision set; they must resolve their `_col`
    /// shapes, and the resource totals must match the streamable set.
    #[test]
    fn real_spawnable_houses_resolve_collision_entries() {
        let Some((root, cache)) = game_data() else {
            eprintln!("skip: game data not present");
            return;
        };
        let text = std::fs::read_to_string(find_data_file("field_object_placements.tsv").unwrap())
            .unwrap();
        let placements =
            WorldCollision::streamable_placements(&text, "ha_area00", &model_paths(&cache));
        assert!(
            placements
                .iter()
                .any(|p| p.model.starts_with("objects_area00_house")),
            "the export-authoritative parse keeps the house placements"
        );
        let packs = CollisionPacks::load(&root).unwrap();
        for p in &placements {
            assert!(
                packs.has_collision_entry(&p.model),
                "{} is streamable but has no collision entry",
                p.model
            );
        }
        let wc = WorldCollision::load(&root, 0, RotMode::YawY, &cache).unwrap();
        assert_eq!(wc.placements_total, placements.len());
        assert_eq!(wc.placements_with_collision, wc.placements_total);
    }
}
