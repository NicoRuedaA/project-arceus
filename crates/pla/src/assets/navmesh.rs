//! Walkability data read from PLA `hkaiNavMesh` tagfiles (terrain packs).
//!
//! A navmesh is a set of convex polygons ("faces") over shared vertices, in the
//! terrain's local frame (x, z horizontal, y up, same frame as the heightmap).
//! Proven on the data: vertex bounds cover the 1025-unit area extent and vertex
//! heights agree with the heightmap (see `reports/data-progress/collision_navmesh.md`).
//! Not proven: the meaning of edge flags and of the per-face/per-edge data words.

use std::collections::HashMap;
use std::path::Path;

use super::gfpak::GfPak;
use super::havok_tag::TagFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavFace {
    pub start_edge: u32,
    pub num_edges: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavEdge {
    /// Vertex indices (start, end) of this polygon side.
    pub a: u32,
    pub b: u32,
    /// Neighbouring edge/face across this side; `u32::MAX` when there is none.
    pub opposite_edge: u32,
    pub opposite_face: u32,
    /// Raw `hkaiNavMesh::EdgeFlagBits` byte; individual bits are not decoded.
    pub flags: u8,
}

const CELL: f32 = 16.0;

#[derive(Debug, Clone)]
pub struct NavMesh {
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<NavFace>,
    pub edges: Vec<NavEdge>,
    /// One `i32` word per face (`faceDataStriding` words each); meaning unknown.
    pub face_data: Vec<i32>,
    pub face_data_stride: u32,
    pub edge_data: Vec<i32>,
    pub edge_data_stride: u32,
    grid: HashMap<(i32, i32), Vec<u32>>,
}

/// Result of a nearest-face query.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavHit {
    pub face: u32,
    /// Height of the face surface at the query point (or its closest point).
    pub y: f32,
    /// Horizontal distance from the query point to the face (0 when inside).
    pub distance: f32,
}

fn read_array<'a>(tag: &'a TagFile, object: usize, member: &str) -> Result<Option<(&'a [u8], usize)>, String> {
    match tag.member_item(object, member)? {
        None => Ok(None),
        Some(item) => {
            let count = tag.items[item].count as usize;
            Ok(Some((tag.item_bytes(item)?, count)))
        }
    }
}

fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

impl NavMesh {
    /// Parse every `hkaiNavMesh` object of a tagfile (main area packs hold two).
    pub fn parse_all(bytes: &[u8]) -> Result<Vec<NavMesh>, String> {
        let tag = TagFile::parse(bytes)?;
        tag.items_of_type("hkaiNavMesh")
            .into_iter()
            .map(|object| Self::from_object(&tag, object))
            .collect()
    }

    fn from_object(tag: &TagFile, object: usize) -> Result<NavMesh, String> {
        let face_t = tag.type_by_name("hkaiNavMesh::Face").ok_or("no Face type")?;
        let edge_t = tag.type_by_name("hkaiNavMesh::Edge").ok_or("no Edge type")?;
        let off = |t: u32, name: &str| -> Result<usize, String> {
            tag.field(t, name).map(|f| f.offset as usize).ok_or_else(|| format!("no member {name}"))
        };
        let (f_start, f_num) = (off(face_t, "startEdgeIndex")?, off(face_t, "numEdges")?);
        let (e_a, e_b, e_oe, e_of, e_fl) = (
            off(edge_t, "a")?,
            off(edge_t, "b")?,
            off(edge_t, "oppositeEdge")?,
            off(edge_t, "oppositeFace")?,
            off(edge_t, "flags")?,
        );
        let (fs, es) = (tag.type_size(face_t), tag.type_size(edge_t));

        let mut faces = Vec::new();
        if let Some((b, n)) = read_array(tag, object, "faces")? {
            for i in 0..n {
                let o = i * fs;
                faces.push(NavFace {
                    start_edge: le_u32(b, o + f_start),
                    num_edges: u16::from_le_bytes(b[o + f_num..o + f_num + 2].try_into().unwrap()),
                });
            }
        }
        let mut edges = Vec::new();
        if let Some((b, n)) = read_array(tag, object, "edges")? {
            for i in 0..n {
                let o = i * es;
                edges.push(NavEdge {
                    a: le_u32(b, o + e_a),
                    b: le_u32(b, o + e_b),
                    opposite_edge: le_u32(b, o + e_oe),
                    opposite_face: le_u32(b, o + e_of),
                    flags: b[o + e_fl],
                });
            }
        }
        let mut vertices = Vec::new();
        if let Some((b, n)) = read_array(tag, object, "vertices")? {
            for i in 0..n {
                let f = |k: usize| f32::from_le_bytes(b[i * 16 + 4 * k..i * 16 + 4 * k + 4].try_into().unwrap());
                vertices.push([f(0), f(1), f(2)]);
            }
        }
        let words = |member: &str| -> Result<Vec<i32>, String> {
            Ok(read_array(tag, object, member)?
                .map(|(b, n)| (0..n).map(|i| le_u32(b, i * 4) as i32).collect())
                .unwrap_or_default())
        };
        let mut mesh = NavMesh {
            vertices,
            faces,
            edges,
            face_data: words("faceData")?,
            face_data_stride: tag.member_u32(object, "faceDataStriding")?,
            edge_data: words("edgeData")?,
            edge_data_stride: tag.member_u32(object, "edgeDataStriding")?,
            grid: HashMap::new(),
        };
        mesh.validate()?;
        mesh.build_grid();
        Ok(mesh)
    }

    fn validate(&self) -> Result<(), String> {
        for (i, f) in self.faces.iter().enumerate() {
            let end = f.start_edge as usize + f.num_edges as usize;
            if end > self.edges.len() {
                return Err(format!("navmesh face {i} edges {}..{end} out of range", f.start_edge));
            }
        }
        for (i, e) in self.edges.iter().enumerate() {
            if e.a as usize >= self.vertices.len() || e.b as usize >= self.vertices.len() {
                return Err(format!("navmesh edge {i} vertex out of range"));
            }
        }
        Ok(())
    }

    /// Vertex positions of a face, in stored order.
    pub fn face_vertices(&self, face: u32) -> Vec<[f32; 3]> {
        let f = self.faces[face as usize];
        (f.start_edge..f.start_edge + f.num_edges as u32)
            .map(|e| self.vertices[self.edges[e as usize].a as usize])
            .collect()
    }

    fn build_grid(&mut self) {
        let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for fi in 0..self.faces.len() as u32 {
            let vs = self.face_vertices(fi);
            if vs.is_empty() {
                continue;
            }
            let (mut x0, mut x1, mut z0, mut z1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for v in &vs {
                x0 = x0.min(v[0]);
                x1 = x1.max(v[0]);
                z0 = z0.min(v[2]);
                z1 = z1.max(v[2]);
            }
            for cx in (x0 / CELL).floor() as i32..=(x1 / CELL).floor() as i32 {
                for cz in (z0 / CELL).floor() as i32..=(z1 / CELL).floor() as i32 {
                    grid.entry((cx, cz)).or_default().push(fi);
                }
            }
        }
        self.grid = grid;
    }

    /// Bounding box of all vertices: `(min, max)`.
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let first = *self.vertices.first()?;
        Some(self.vertices.iter().fold((first, first), |(mut lo, mut hi), v| {
            for k in 0..3 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
            (lo, hi)
        }))
    }

    /// Surface height of `face` at (x, z) if the point lies inside it.
    fn face_height(&self, face: u32, x: f32, z: f32) -> Option<f32> {
        let vs = self.face_vertices(face);
        (1..vs.len().saturating_sub(1)).find_map(|i| tri_height(vs[0], vs[i], vs[i + 1], x, z))
    }

    /// Height of the walkable surface at (x, z); `None` when not on the navmesh.
    /// Overlapping faces (bridges, caves) return the highest one.
    pub fn height_at(&self, x: f32, z: f32) -> Option<f32> {
        let cell = self.grid.get(&((x / CELL).floor() as i32, (z / CELL).floor() as i32))?;
        cell.iter().filter_map(|f| self.face_height(*f, x, z)).fold(None, |a, y| Some(a.map_or(y, |p: f32| p.max(y))))
    }

    pub fn is_walkable(&self, x: f32, z: f32) -> bool {
        self.height_at(x, z).is_some()
    }

    /// Closest face within `max_distance` horizontal units of (x, z).
    pub fn nearest(&self, x: f32, z: f32, max_distance: f32) -> Option<NavHit> {
        let r = (max_distance / CELL).ceil() as i32;
        let (cx, cz) = ((x / CELL).floor() as i32, (z / CELL).floor() as i32);
        let mut best: Option<NavHit> = None;
        for dx in -r..=r {
            for dz in -r..=r {
                let Some(cell) = self.grid.get(&(cx + dx, cz + dz)) else { continue };
                for &face in cell {
                    let vs = self.face_vertices(face);
                    for i in 1..vs.len().saturating_sub(1) {
                        let (d, y) = tri_distance(vs[0], vs[i], vs[i + 1], x, z);
                        if d <= max_distance && best.map_or(true, |b| d < b.distance) {
                            best = Some(NavHit { face, y, distance: d });
                        }
                    }
                }
            }
        }
        best
    }
}

/// Barycentric interpolation of y at (x, z) if inside the triangle (either winding).
fn tri_height(a: [f32; 3], b: [f32; 3], c: [f32; 3], x: f32, z: f32) -> Option<f32> {
    let d = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2]);
    if d.abs() < 1e-9 {
        return None;
    }
    let l1 = ((b[2] - c[2]) * (x - c[0]) + (c[0] - b[0]) * (z - c[2])) / d;
    let l2 = ((c[2] - a[2]) * (x - c[0]) + (a[0] - c[0]) * (z - c[2])) / d;
    let l3 = 1.0 - l1 - l2;
    const EPS: f32 = -1e-4;
    (l1 >= EPS && l2 >= EPS && l3 >= EPS).then(|| l1 * a[1] + l2 * b[1] + l3 * c[1])
}

/// (horizontal distance to triangle, y at the closest point approximated by the
/// interpolated height inside or the nearest-vertex-weighted edge height outside).
fn tri_distance(a: [f32; 3], b: [f32; 3], c: [f32; 3], x: f32, z: f32) -> (f32, f32) {
    if let Some(y) = tri_height(a, b, c, x, z) {
        return (0.0, y);
    }
    let mut best = (f32::MAX, a[1]);
    for (p, q) in [(a, b), (b, c), (c, a)] {
        let (dx, dz) = (q[0] - p[0], q[2] - p[2]);
        let len2 = dx * dx + dz * dz;
        let t = if len2 > 0.0 { (((x - p[0]) * dx + (z - p[2]) * dz) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let (px, pz) = (p[0] + t * dx, p[2] + t * dz);
        let d = ((x - px).powi(2) + (z - pz).powi(2)).sqrt();
        if d < best.0 {
            best = (d, p[1] + t * (q[1] - p[1]));
        }
    }
    best
}

/// Load every navmesh of one pack file (`ha_areaNN.gfpak` or a sub-area pack).
pub fn load_pack_navmeshes(path: impl AsRef<Path>) -> Result<Vec<NavMesh>, String> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let pack = GfPak::parse(&bytes)?;
    let mut out = Vec::new();
    for (i, entry) in pack.entries.iter().enumerate() {
        let data = pack.decoded_data(&bytes, entry).map_err(|e| format!("{} entry {i}: {e}", path.display()))?;
        if data.get(4..8) == Some(b"TAG0") && data.windows(11).any(|w| w == b"hkaiNavMesh") {
            out.extend(NavMesh::parse_all(&data).map_err(|e| format!("{} entry {i}: {e}", path.display()))?);
        }
    }
    Ok(out)
}

/// Navigation of one main area (`archive/field/terrain/ha_areaNN.gfpak`).
#[derive(Debug, Clone)]
pub struct AreaNavigation {
    pub area: u8,
    /// First mesh is the primary `NavMesh`; the pack stores a second one whose
    /// role is not established.
    pub meshes: Vec<NavMesh>,
}

impl AreaNavigation {
    pub fn load(bin_root: impl AsRef<Path>, area: u8) -> Result<Self, String> {
        let path = bin_root.as_ref().join("archive/field/terrain").join(format!("ha_area{area:02}.gfpak"));
        let meshes = load_pack_navmeshes(path)?;
        if meshes.is_empty() {
            return Err(format!("area {area}: no navmesh found"));
        }
        Ok(Self { area, meshes })
    }

    pub fn primary(&self) -> &NavMesh {
        &self.meshes[0]
    }

    pub fn is_walkable(&self, x: f32, z: f32) -> bool {
        self.primary().is_walkable(x, z)
    }

    pub fn height_at(&self, x: f32, z: f32) -> Option<f32> {
        self.primary().height_at(x, z)
    }

    pub fn nearest(&self, x: f32, z: f32, max_distance: f32) -> Option<NavHit> {
        self.primary().nearest(x, z, max_distance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_height_interpolates() {
        let h = tri_height([0.0, 0.0, 0.0], [10.0, 10.0, 0.0], [0.0, 0.0, 10.0], 5.0, 2.0).unwrap();
        assert!((h - 5.0).abs() < 1e-4);
        assert!(tri_height([0.0, 0.0, 0.0], [10.0, 10.0, 0.0], [0.0, 0.0, 10.0], 9.0, 9.0).is_none());
    }

    #[test]
    fn real_area_navmesh_if_present() {
        let root = std::env::var_os("PLA_BIN_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into());
        if !root.join("archive/field/terrain/ha_area00.gfpak").exists() {
            eprintln!("skip: game data not present");
            return;
        }
        let nav = AreaNavigation::load(&root, 0).unwrap();
        let m = nav.primary();
        assert_eq!(m.faces.len(), 24177);
        assert_eq!(m.edges.len(), 81513);
        assert_eq!(m.vertices.len(), 30968);
        let (lo, hi) = m.bounds().unwrap();
        assert!(lo[0] >= -1.0 && hi[0] <= 1026.0 && hi[2] <= 1026.0, "{lo:?} {hi:?}");
    }
}
