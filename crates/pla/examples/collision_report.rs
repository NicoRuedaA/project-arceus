//! Parse every collision pack, print totals and the material-name histogram.
//! Usage: collision_report [OUT_DIR]   (PLA_BIN_ROOT overrides the game root)

use std::collections::BTreeMap;
use std::path::PathBuf;

use pla::assets::collision::load_collision_pack;

fn main() {
    let root = std::env::var_os("PLA_BIN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into());
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "work/cache/collision".into()));
    std::fs::create_dir_all(&out).unwrap();
    let mut materials: BTreeMap<String, usize> = BTreeMap::new();
    for pack in ["collision", "resident_bg_parts_model"] {
        let path = root.join(format!("archive/field/model/pack/{pack}.gfpak"));
        let meshes = load_collision_pack(&path).unwrap();
        let (mut verts, mut tris, mut quads, mut empty, mut secs) = (0, 0, 0, 0, 0);
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for (i, m) in &meshes {
            verts += m.vertices.len();
            tris += m.triangles.len();
            quads += m.quad_count;
            secs += m.section_count;
            empty += m.triangles.is_empty() as usize;
            if let Some((l, h)) = m.bounds() {
                for k in 0..3 {
                    lo[k] = lo[k].min(l[k]);
                    hi[k] = hi[k].max(h[k]);
                }
            }
            for n in &m.material_names {
                *materials.entry(n.clone()).or_default() += 1;
            }
            if *i == 0 && pack == "collision" {
                let mut obj = String::new();
                for v in &m.vertices {
                    obj += &format!("v {} {} {}\n", v[0], v[1], v[2]);
                }
                for t in &m.triangles {
                    obj += &format!("f {} {} {}\n", t[0] + 1, t[1] + 1, t[2] + 1);
                }
                std::fs::write(out.join("collision_entry0.obj"), obj).unwrap();
            }
        }
        println!("{pack}: shapes={} empty={empty} sections={secs} vertices={verts} triangles={tris} quads={quads} bounds lo={lo:?} hi={hi:?}", meshes.len());
    }
    let mut top: Vec<_> = materials.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1));
    println!("distinct material names: {}", top.len());
    for (n, c) in top.iter().take(25) {
        println!("  {c:5} {n}");
    }
}
