//! Battle effect playback coverage and texture cache (no window).
//!
//! For every move timeline in `data/move_effect_map.tsv` (177 default + 15 species
//! overrides) and the shared battle timelines (critical, type effectiveness, status), loads
//! the particle files with the same code the game plugin uses (`game::effects`), counts which
//! emitters the player draws and which it skips (mesh, ribbon, child, unknown display), and
//! decodes the textures the drawn emitters need into the PNG cache
//! (`$PLA_CACHE_ROOT/effects/textures/*.png`, index `effects/texture_index.tsv`).
//!
//! Usage: `effect_coverage [--no-cache]`. Prints a summary; per-move rows go to
//! `$PLA_CACHE_ROOT/effects/move_coverage.tsv`.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;

use pla::assets::bntx::Bntx;
use pla::assets::ptcl::PtclFile;
use pla::assets::ptcl_textures::{default_bin_root, effects_cache_dir, load_textures, TextureSource};
use pla::game::effects::library::load_timeline;
use pla::game::effects::sim::{blend_mode, DrawShape, EffectDef, Skip};
use pla::game::effects::timeline::{
    status_sequence, CRITICAL_SEQUENCE, NOT_VERY_EFFECTIVE_SEQUENCE, SUPER_EFFECTIVE_SEQUENCE,
};

fn pct(v: &mut [f32], p: f32) -> f32 {
    if v.is_empty() {
        return f32::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f32 * p) as usize]
}

struct FileInfo {
    def: EffectDef,
    textures_ok: usize,
    textures_failed: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let use_cache = !args.iter().any(|a| a == "--no-cache");
    let root = default_bin_root();
    let cache = effects_cache_dir();
    let map_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/move_effect_map.tsv");
    let Ok(text) = std::fs::read_to_string(&map_path) else {
        println!("[skip] {} missing", map_path.display());
        return;
    };
    if !root.join("effect/battle/sequence").is_dir() {
        println!("[skip] game data not present under {}", root.display());
        return;
    }
    // (move, name, sequence, selector)
    let mut rows: Vec<(u16, String, String, String)> = Vec::new();
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let header: Vec<&str> = lines.next().unwrap().split('\t').collect();
    let col = |n: &str| header.iter().position(|h| h.split(':').next() == Some(n)).unwrap();
    let (cm, cn, cs, csel) = (col("move"), col("move_name"), col("sequence"), col("selector"));
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        rows.push((f[cm].parse().unwrap(), f[cn].into(), f[cs].into(), f[csel].into()));
    }

    let mut files: HashMap<String, FileInfo> = HashMap::new();
    let mut texture_index: BTreeMap<String, (u32, u32, String, [u8; 4], String, &'static str)> = BTreeMap::new();
    let mut load = |path: &str, files: &mut HashMap<String, FileInfo>| {
        if files.contains_key(path) {
            return;
        }
        let full = root.join("effect/battle/particle").join(path);
        let Ok(bytes) = std::fs::read(&full) else {
            files.insert(path.into(), FileInfo { def: EffectDef::default(), textures_ok: 0, textures_failed: 1 });
            return;
        };
        let file = PtclFile::parse(&bytes).expect("ptcl parses");
        let def = EffectDef::from_ptcl(path, &file);
        let wanted = def.texture_names();
        let (tex, errors) = load_textures(&bytes, &file, Some(&wanted), use_cache.then_some(cache.as_path()));
        if let Some(Ok((bntx, _))) = file.bntx(&bytes) {
            let bntx: Bntx = bntx;
            for t in &bntx.textures {
                if let Some(d) = tex.get(&t.name) {
                    texture_index.entry(t.name.clone()).or_insert((
                        t.width,
                        t.height,
                        t.format.name().to_string(),
                        t.channels,
                        path.to_string(),
                        if d.source == TextureSource::Cache { "cache" } else { "decoded" },
                    ));
                }
            }
        }
        files.insert(path.into(), FileInfo { def, textures_ok: tex.len(), textures_failed: errors.len() });
    };

    let mut out_rows = Vec::new();
    let mut playable_default = 0;
    let mut default_total = 0;
    let mut playable_all = 0;
    for (mv, name, seq, sel) in &rows {
        let tl = load_timeline(&root, seq);
        let (plays, drawn_plays, emit_ok, emit_skip) = match &tl {
            Ok(t) => {
                let mut drawn = 0;
                let (mut ok, mut skip) = (0, 0);
                for p in &t.plays {
                    load(&p.path, &mut files);
                    let fi = &files[&p.path];
                    ok += fi.def.emitters.len();
                    skip += fi.def.skipped.len();
                    if fi.def.is_playable() {
                        drawn += 1;
                    }
                }
                (t.plays.len(), drawn, ok, skip)
            }
            Err(_) => (0, 0, 0, 0),
        };
        let playable = drawn_plays > 0;
        if sel == "default" {
            default_total += 1;
            playable_default += playable as usize;
        }
        playable_all += playable as usize;
        out_rows.push(format!("{mv}\t{name}\t{seq}\t{sel}\t{plays}\t{drawn_plays}\t{emit_ok}\t{emit_skip}\t{playable}"));
    }
    let mut shared = vec![CRITICAL_SEQUENCE.to_string(), SUPER_EFFECTIVE_SEQUENCE.into(), NOT_VERY_EFFECTIVE_SEQUENCE.into()];
    for s in [
        pla::battle_engine::MajorStatus::Drowsy,
        pla::battle_engine::MajorStatus::Poison,
        pla::battle_engine::MajorStatus::Burn,
        pla::battle_engine::MajorStatus::Frostbite,
        pla::battle_engine::MajorStatus::Paralysis,
    ] {
        shared.push(status_sequence(s).into());
    }
    println!("shared battle timelines:");
    for s in &shared {
        match load_timeline(&root, s) {
            Ok(t) => {
                let mut desc = Vec::new();
                for p in &t.plays {
                    load(&p.path, &mut files);
                    let fi = &files[&p.path];
                    desc.push(format!(
                        "{}@{}-{}:{:?} ({} drawn/{} skipped)",
                        p.path,
                        p.start,
                        p.end,
                        p.side,
                        fi.def.emitters.len(),
                        fi.def.skipped.len()
                    ));
                }
                println!("  {s}: {} frames, plays: {}", t.length_frames, if desc.is_empty() { "none".into() } else { desc.join(", ") });
            }
            Err(e) => println!("  {s}: missing ({e})"),
        }
    }

    // Emitter statistics over all loaded files.
    let mut shapes: BTreeMap<String, usize> = BTreeMap::new();
    let mut skips: BTreeMap<String, usize> = BTreeMap::new();
    let (mut sizes, mut speeds, mut lives, mut grav) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut tex_ok, mut tex_fail, mut no_tex, mut slot1) = (0, 0, 0, 0);
    let (mut add_by_name, mut mirrored, mut blends) = (0, 0, BTreeMap::<String, usize>::new());
    for fi in files.values() {
        tex_ok += fi.textures_ok;
        tex_fail += fi.textures_failed;
        for e in &fi.def.emitters {
            *shapes
                .entry(match e.shape {
                    DrawShape::Billboard => "billboard",
                    DrawShape::YBillboard => "y_billboard",
                    DrawShape::Streak => "streak",
                    DrawShape::PolygonXY => "polygon_xy",
                    DrawShape::PolygonXZ => "polygon_xz",
                }
                .to_string())
                .or_default() += 1;
            sizes.push(e.base_size()[0]);
            speeds.push(e.emitter.omni_speed.abs());
            lives.push(e.emitter.life_frames as f32);
            grav.push((e.emitter.gravity[3]).abs());
            if e.texture.is_none() {
                no_tex += 1;
            }
            if e.slot == 1 {
                slot1 += 1;
            }
            if e.blend != blend_mode(e.emitter.blend) {
                add_by_name += 1;
            }
            if e.wrap().contains(&0) && e.atlas() == [1, 1] {
                mirrored += 1;
            }
            *blends.entry(format!("{:?}", e.blend)).or_default() += 1;
        }
        for (_, s) in &fi.def.skipped {
            *skips.entry(Skip::label(*s)).or_default() += 1;
        }
    }
    let drawn: usize = shapes.values().sum();
    let skipped: usize = skips.values().sum();
    println!(
        "moves with a playable effect: {playable_default} of {default_total} default timelines ({playable_all} of {} incl. species overrides)",
        rows.len()
    );
    println!("particle files loaded: {}", files.len());
    println!("emitters drawn: {drawn} {shapes:?}; skipped: {skipped} {skips:?}; drawn as a soft dot (white placeholder texture, no second sampler): {no_tex}; drawing sampler 1: {slot1}");
    println!("drawn blends {blends:?}; additive by name only: {add_by_name}; mirrored single-cell textures: {mirrored}");
    println!("textures decoded/cached: {tex_ok}, failed: {tex_fail}, distinct names: {}", texture_index.len());
    println!(
        "base size p10/p50/p90: {:.3}/{:.3}/{:.3}; omni speed p50/p90: {:.4}/{:.4}; life p50: {}; gravity p50/p90: {:.4}/{:.4}",
        pct(&mut sizes.clone(), 0.1),
        pct(&mut sizes.clone(), 0.5),
        pct(&mut sizes, 0.9),
        pct(&mut speeds.clone(), 0.5),
        pct(&mut speeds, 0.9),
        pct(&mut lives, 0.5),
        pct(&mut grav.clone(), 0.5),
        pct(&mut grav, 0.9)
    );
    let mut formats: BTreeMap<(String, [u8; 4]), usize> = BTreeMap::new();
    for (_, (_, _, f, ch, _, _)) in &texture_index {
        *formats.entry((f.clone(), *ch)).or_default() += 1;
    }
    println!("texture formats x channel mapping: {formats:?}");

    std::fs::create_dir_all(&cache).unwrap();
    let mut f = std::fs::File::create(cache.join("move_coverage.tsv")).unwrap();
    writeln!(f, "move\tmove_name\tsequence\tselector\tparticle_plays\tdrawn_plays\temitters_drawn\temitters_skipped\tplayable").unwrap();
    for r in &out_rows {
        writeln!(f, "{r}").unwrap();
    }
    if use_cache {
        let mut f = std::fs::File::create(cache.join("texture_index.tsv")).unwrap();
        writeln!(f, "name\twidth\theight\tformat\tchannels\tfirst_file\tpng").unwrap();
        for (name, (w, h, fmt, ch, src, _)) in &texture_index {
            writeln!(f, "{name}\t{w}\t{h}\t{fmt}\t{ch:?}\t{src}\ttextures/{name}.png").unwrap();
        }
    }
    println!("wrote {}/move_coverage.tsv and texture_index.tsv", cache.display());
}
