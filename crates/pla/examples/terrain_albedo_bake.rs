//! Dump decoded terrain textures (raw RGBA8 / f32 + sizes) for one area.
//! Usage: terrain_albedo_bake --bin-root BIN --area N --out DIR   (raw dump)
//!        terrain_albedo_bake --bin-root BIN --area N|all --bake WORK_ROOT [--size 2048] [--mean]
use pla::assets::terrain::TerrainArea;
use pla::assets::terrain_albedo::{albedo_cache_path, bake_albedo};
use std::{env, fs, path::PathBuf};

fn main() {
    let a: Vec<String> = env::args().collect();
    let get = |k: &str| a.iter().position(|x| x == k).map(|i| a[i + 1].clone());
    let bin = get("--bin-root").expect("--bin-root");
    let area: u8 = if a.iter().any(|x| x == "all") { 0 } else { area_arg(&a) };
    if let Some(work) = get("--bake") {
        // --bake WORK_ROOT [--size 2048] [--mean]  (areas via --area N or --area all)
        let size: u32 = get("--size").map(|v| v.parse().unwrap()).unwrap_or(2048);
        let areas: Vec<u8> = if a.iter().any(|x| x == "all") { (0..=6).collect() } else { vec![area_arg(&a)] };
        for ar in areas {
            let t = TerrainArea::load(&bin, ar).expect("load");
            let img = bake_albedo(&t, size, !a.iter().any(|x| x == "--mean")).expect("bake");
            let p = albedo_cache_path(&work, ar);
            img.save_png(&p).unwrap();
            eprintln!("wrote {}", p.display());
        }
        return;
    }
    let out = PathBuf::from(get("--out").expect("--out"));
    fs::create_dir_all(out.join("ground")).unwrap();
    {
        use pla::assets::{bntx::Bntx, gfpak::GfPak};
        let pk = fs::read(format!("{bin}/archive/field/terrain/ha_area{area:02}.gfpak")).unwrap();
        let g = GfPak::parse(&pk).unwrap();
        let tile = if area == 6 { "terrain_1_1".to_string() } else { format!("area{area:02}_0_0") };
        let path = format!("bin/field/terrain/ha_area{area:02}/terrain/{tile}_normalmap.bntx");
        let e = g.entry_by_path(&path).unwrap().unwrap();
        let d = g.decoded_data(&pk, e).unwrap();
        let b = Bntx::parse(&d).unwrap();
        let f = b.textures[0].decode_f32(&d).unwrap();
        let raw: Vec<u8> = f.samples.iter().flat_map(|v| ((v * 65535.0).round() as u16).to_le_bytes()).collect();
        fs::write(out.join("normal16.u16"), raw).unwrap();
    }
    let t = TerrainArea::load(&bin, area).expect("load");
    let w = |name: &str, tex: &pla::assets::bntx::TextureData| {
        fs::write(out.join(format!("{name}.rgba")), &tex.rgba).unwrap();
        fs::write(out.join(format!("{name}.txt")), format!("{} {}\n", tex.width, tex.height)).unwrap();
    };
    w("splat", &t.splat_alpha);
    if let Some(s1) = &t.splat_alpha1 {
        w("splat1", s1);
    }
    w("normal", &t.normal_map);
    for (n, tex) in &t.ground_textures {
        w(&format!("ground/{n}"), tex);
    }
    let h: Vec<u8> = t.heightmap.samples.iter().flat_map(|v| v.to_le_bytes()).collect();
    fs::write(out.join("height.f32"), h).unwrap();
    fs::write(out.join("height.txt"), format!("{} {} {}\n", t.heightmap.width, t.heightmap.height, t.world_height)).unwrap();
    // instances
    let mut s = String::new();
    for i in &t.instances {
        s += &format!("{}\t{}\t{}\t{}\n", i.model_name, i.matrix[3][0], i.matrix[3][1], i.matrix[3][2]);
    }
    fs::write(out.join("instances.tsv"), s).unwrap();
    if let Some(m) = &t.material {
        eprintln!("{:#?}", m.slots.textures);
        eprintln!("unresolved: {:?}", t.unresolved_ground_textures);
    }
}

fn area_arg(a: &[String]) -> u8 {
    a.iter().position(|x| x == "--area").map(|i| a[i + 1].parse().unwrap()).expect("--area")
}
