//! Usage: field_submesh_dump <romfs> <pack.gfpak> <model>
use pla::assets::field_material::trmsh_submeshes;
use pla::assets::gfpak::GfPak;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let bytes = std::fs::read(format!("{}/bin/archive/field/model/pack/{}", a[0], a[1])).unwrap();
    let g = GfPak::parse(&bytes).unwrap();
    let i = g.entry_index_by_path(&format!("bin/field/model/pack/{}.trmsh", a[2])).unwrap().unwrap();
    let sh = g.decoded_data(&bytes, &g.entries[i]).unwrap();
    for s in trmsh_submeshes(&sh) { println!("{:?}", s); }
}
