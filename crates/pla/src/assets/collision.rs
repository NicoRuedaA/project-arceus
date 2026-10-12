//! Static collision geometry from PLA `hknpMeshShape` tagfiles
//! (`archive/field/model/pack/collision.gfpak` and `resident_bg_parts_model.gfpak`).
//!
//! Decoded layout (derived from the files, verified on 712 of 712 shapes):
//! the mesh is split into geometry sections; each section has 16-bit quantized
//! vertices `(x, y, z)` and primitives of four 8-bit vertex ids `(a, b, c, d)`.
//! A primitive is a triangle `(a, b, c)` when `c == d`, otherwise a quad ring
//! `a-b-c-d` that is split into `(a, b, c)` and `(a, c, d)`. A vertex position
//! is `(section_offset[i] + q[i]) * bitScale16Inv[i]` (bitScale16Inv = 1/512 in
//! every file read so far). The material names come from the palette; the link
//! from a primitive to its material (shape tags) is not decoded.

use std::path::Path;

use super::gfpak::GfPak;
use super::havok_tag::TagFile;

#[derive(Debug, Clone, Default)]
pub struct CollisionMesh {
    pub vertices: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    /// Number of quad primitives (each already split into two triangles).
    pub quad_count: usize,
    pub section_count: usize,
    /// Names from `hknpMaterialDescriptor`, e.g. `attribute_rock`; palette order.
    pub material_names: Vec<String>,
}

fn f32_at(t: &TagFile, at: usize) -> Result<f32, String> {
    Ok(f32::from_bits(t.u32_at(at)?))
}

impl CollisionMesh {
    pub fn parse(bytes: &[u8]) -> Result<CollisionMesh, String> {
        let t = TagFile::parse(bytes)?;
        let mesh_data = *t
            .items_of_type("hknpMeshShapeData")
            .first()
            .ok_or("collision: no hknpMeshShapeData")?;
        let md = t.items[mesh_data];
        let util = t.field(md.type_index, "vertexConversionUtil").ok_or("no vertexConversionUtil")?;
        let util_type = util.type_index;
        let inv_off = t.field(util_type, "bitScale16Inv").ok_or("no bitScale16Inv")?.offset as usize;
        let inv_at = md.offset as usize + util.offset as usize + inv_off;
        let inv = [f32_at(&t, inv_at)?, f32_at(&t, inv_at + 4)?, f32_at(&t, inv_at + 8)?];

        // A null reference means the shape has no sections (an empty mesh).
        let Some(sec_item) = t.member_item(mesh_data, "geometrySections")? else {
            return Ok(CollisionMesh::default());
        };
        let sec = t.items[sec_item];
        let sec_size = t.type_size(sec.type_index);
        let f = |name: &str| -> Result<usize, String> {
            t.field(sec.type_index, name).map(|f| f.offset as usize).ok_or_else(|| format!("no member {name}"))
        };
        let (f_prim, f_vert, f_off) = (f("primitives")?, f("quantizedVertices")?, f("sectionOffset")?);

        let mut out = CollisionMesh { section_count: sec.count as usize, ..Default::default() };
        for k in 0..sec.count as usize {
            let base = sec.offset as usize + k * sec_size;
            let prim_item = t.u32_at(base + f_prim)? as usize;
            let vert_item = t.u32_at(base + f_vert)? as usize;
            let offset = [
                t.u32_at(base + f_off)? as i32,
                t.u32_at(base + f_off + 4)? as i32,
                t.u32_at(base + f_off + 8)? as i32,
            ];
            let verts = if vert_item == 0 { &[][..] } else { t.item_bytes(vert_item)? };
            let prims = if prim_item == 0 { &[][..] } else { t.item_bytes(prim_item)? };
            let first = out.vertices.len() as u32;
            for v in verts.chunks_exact(6) {
                let q = |i: usize| u16::from_le_bytes([v[2 * i], v[2 * i + 1]]) as i32;
                out.vertices.push([
                    (offset[0] + q(0)) as f32 * inv[0],
                    (offset[1] + q(1)) as f32 * inv[1],
                    (offset[2] + q(2)) as f32 * inv[2],
                ]);
            }
            let n = (verts.len() / 6) as u32;
            for p in prims.chunks_exact(4) {
                if p.iter().any(|i| *i as u32 >= n) {
                    return Err("collision: primitive vertex id out of range".into());
                }
                let [a, b, c, d] = [p[0], p[1], p[2], p[3]].map(|i| first + i as u32);
                out.triangles.push([a, b, c]);
                if p[2] != p[3] {
                    out.triangles.push([a, c, d]);
                    out.quad_count += 1;
                }
            }
        }
        for item in t.items_of_type("hknpMaterialDescriptor") {
            let it = t.items[item];
            let size = t.type_size(it.type_index);
            let name_off = t.field(it.type_index, "name").map(|f| f.offset as usize).unwrap_or(0);
            for k in 0..it.count as usize {
                let name_item = t.u32_at(it.offset as usize + k * size + name_off)? as usize;
                if name_item != 0 {
                    let raw = t.item_bytes(name_item)?;
                    out.material_names.push(String::from_utf8_lossy(raw).trim_end_matches('\0').to_owned());
                }
            }
        }
        Ok(out)
    }

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

    /// Highest triangle surface at (x, z), or `None` when no triangle covers it.
    pub fn height_at(&self, x: f32, z: f32) -> Option<f32> {
        let mut best: Option<f32> = None;
        for tri in &self.triangles {
            let [a, b, c] = tri.map(|i| self.vertices[i as usize]);
            let d = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2]);
            if d.abs() < 1e-9 {
                continue;
            }
            let l1 = ((b[2] - c[2]) * (x - c[0]) + (c[0] - b[0]) * (z - c[2])) / d;
            let l2 = ((c[2] - a[2]) * (x - c[0]) + (a[0] - c[0]) * (z - c[2])) / d;
            let l3 = 1.0 - l1 - l2;
            if l1 >= -1e-5 && l2 >= -1e-5 && l3 >= -1e-5 {
                let y = l1 * a[1] + l2 * b[1] + l3 * c[1];
                best = Some(best.map_or(y, |p| p.max(y)));
            }
        }
        best
    }
}

/// Parse every `hknpMeshShape` tagfile of a pack; the result keeps entry order
/// (`(entry index, mesh)`). Entry names are not resolved (they are path hashes).
pub fn load_collision_pack(path: impl AsRef<Path>) -> Result<Vec<(usize, CollisionMesh)>, String> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let pack = GfPak::parse(&bytes)?;
    let mut out = Vec::new();
    for (i, entry) in pack.entries.iter().enumerate() {
        let data = pack.decoded_data(&bytes, entry).map_err(|e| format!("{} entry {i}: {e}", path.display()))?;
        if data.get(4..8) == Some(b"TAG0") && data.windows(14).any(|w| w == b"hknpMeshShape\0" || &w[..13] == b"hknpMeshShape") {
            out.push((i, CollisionMesh::parse(&data).map_err(|e| format!("{} entry {i}: {e}", path.display()))?));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_collision_shape_is_a_box_if_present() {
        let root = std::env::var_os("PLA_BIN_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into());
        let pack = root.join("archive/field/model/pack/collision.gfpak");
        if !pack.exists() {
            eprintln!("skip: game data not present");
            return;
        }
        let meshes = load_collision_pack(&pack).unwrap();
        assert_eq!(meshes.len(), 655);
        let (_, m) = &meshes[0];
        assert_eq!((m.vertices.len(), m.triangles.len(), m.section_count), (12, 10, 2));
        let (lo, hi) = m.bounds().unwrap();
        assert!((hi[1] - lo[1] - 2.721).abs() < 0.01, "{lo:?} {hi:?}");
        assert!(m.height_at(0.0, 0.0).is_some());
    }
}
