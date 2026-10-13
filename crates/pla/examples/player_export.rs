//! Assemble the player character (Rei/Akari variants `p1` and `p2`) into ONE skinned glTF
//! per variant: every part of the outfit (face, eyebrows, hair, clothes, shoes, bag...) skins
//! to a single shared biped skeleton, plus a handful of field animation clips.
//!
//! Layout (all evidenced by FNV-1a 64 path hashes, see reports/data-progress/player_model.md):
//!   part pack  `<chara data>/pc/<v>/model/<category>/<stem>.gfpak`, files at
//!              `bin/chara/data/pc/<v>/model/<category>/<stem>/<stem>.<ext>` (flat, no mdl/ dir)
//!   skeleton   `p2_base0001_00_default.trskl` in `pc/p2/model/base/` (no other pack carries one)
//!   clips      `pc.gfpak`: `<dir>/pc0002_00_<nnnnn>_<name>.tranm`; the directory text is unknown,
//!              but its FNV state is recovered from the pack hashes (ANIM_DIR_STATE below).
//!
//! Usage: player_export <archive/chara/data dir> <out dir> [--variant p1|p2] [--parts stem,stem,..]
//!        [--name <output stem>] [--index <tsv>]
//!        [--hair-color R,G,B|name] [--eye-color R,G,B|name] [--skin-tone N] [--no-hair-eye-color]

use pla::assets::{
    gfpak::GfPak,
    player_model::{build_player_skeleton, read_skin_color_table, write_player_gltf, PlayerExportOptions, PlayerSkeleton},
    pokemon_model::{assemble_pack_with, LayerBlendFormula, PackLayout, PokemonModel},
    tr,
};
use std::{env, fs, path::PathBuf};

/// FNV-1a 64 state after hashing the (unknown) directory text of the animations in pc.gfpak.
/// Derived by inverting the hashes of 423 listed names: all agree on this one state.
const ANIM_DIR_STATE: u64 = 7_618_479_347_468_000_054;
const FNV_PRIME: u64 = 0x100000001b3;

/// (clip name inside pc.gfpak without the `pc0002_00_` prefix, exported glTF animation name)
const CLIPS: &[(&str, &str)] = &[
    ("02001_wait01_loop", "idle"),
    ("02012_wait02_loop", "idle2"),
    ("02101_walk01_loop", "walk"),
    ("02121_run01_loop", "run"),
    ("02140_dash01_loop", "dash"),
    ("02061_jump01_start", "jump_start"),
    ("02064_jump01_fallloop", "jump_fall"),
    ("02081_land01", "land"),
    ("02093_swim01_loop", "swim"),
    // Brief 22: dodge roll (four directions), crouch idle / walk / enter / exit. Names are
    // exact `.tranm` names from the pack (see reports/data-progress/player_skills.md); that
    // `rolling01_*` is the dodge and `sq*` the crouch is a reading of the names (PROBABLE).
    ("05011_rolling01_f", "roll_f"),
    ("05012_rolling01_b", "roll_b"),
    ("05013_rolling01_l", "roll_l"),
    ("05014_rolling01_r", "roll_r"),
    ("02031_squat01_l_loop", "crouch"),
    ("02105_sqwalk01_loop", "crouch_walk"),
    ("02033_sqwait01_l_start", "crouch_start"),
    ("02034_sqwait01_l_end", "crouch_end"),
];

fn category_dir(stem: &str) -> Result<&'static str, String> {
    let code = stem.split('_').nth(1).ok_or("part stem has no code")?;
    let letters: String = code.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    Ok(match letters.as_str() {
        "base" => "base",
        "face" => "face",
        "eyb" => "eyebrow",
        "hrs" => "hair",
        "drs" => "dress",
        "tps" => "tops",
        "btm" => "bottoms",
        "ftw" => "shoes",
        "bag" => "bag",
        "hdw" => "hat",
        "eyew" => "eyewear",
        other => return Err(format!("unknown part category code {other}")),
    })
}

/// The 15 hair colour names of `appli/table/hair/hair_color_table.bin` (ids hair_color_01..15,
/// names only: the RGB values are NOT in the game data found). RGB below is ASSUMED, picked to
/// match the names; used by `--hair-color <name>`.
const HAIR_PALETTE: &[(&str, [u8; 3])] = &[
    ("darkbrown", [72, 48, 34]),
    ("darknavy", [28, 34, 62]),
    ("platinumbrown", [150, 112, 82]),
    ("black", [30, 28, 30]),
    ("white", [235, 232, 226]),
    ("red", [170, 48, 40]),
    ("navy", [48, 70, 130]),
    ("turquoise", [60, 170, 170]),
    ("violet", [130, 80, 160]),
    ("orange", [220, 130, 50]),
    ("peach", [240, 170, 150]),
    ("milk", [220, 190, 160]),
    ("skyblue", [120, 180, 230]),
    ("gray", [150, 150, 155]),
    ("blond", [225, 190, 100]),
];

/// Eye colour names seen in the dressup table face variants (`p?_face0001_00_default_<name>`),
/// RGB ASSUMED. The first listed variant after the plain one is `white`; the plain default has no colour name.
/// Default iris colour: blue-grey like the real-game reference screenshot (ASSUMED RGB; the data has names only).
const DEFAULT_EYE_RGB: [u8; 3] = [88, 118, 150];

const EYE_PALETTE: &[(&str, [u8; 3])] = &[
    ("brown", [90, 58, 36]),
    ("hazel", [130, 95, 50]),
    ("black", [30, 26, 26]),
    ("gray", [130, 135, 140]),
    ("blue", [60, 100, 190]),
    ("ultramarine", [40, 60, 170]),
    ("green", [60, 150, 80]),
    ("yellowgreen", [150, 185, 60]),
    ("yellow", [225, 200, 60]),
    ("red", [190, 50, 50]),
    ("poppyred", [215, 70, 60]),
    ("purple", [130, 70, 160]),
    ("lavender", [175, 150, 215]),
    ("white", [235, 235, 235]),
];

fn parse_color(spec: &str, palette: &[(&str, [u8; 3])]) -> Result<[u8; 3], String> {
    if let Some((_, rgb)) = palette.iter().find(|(n, _)| *n == spec) {
        return Ok(*rgb);
    }
    let v: Vec<u8> = spec.split(',').map(|c| c.trim().parse::<u8>()).collect::<Result<_, _>>().map_err(|_| format!("bad colour {spec:?}: use R,G,B (0-255) or a palette name"))?;
    <[u8; 3]>::try_from(v).map_err(|_| format!("bad colour {spec:?}: need exactly 3 components"))
}

fn default_parts(v: &str) -> Vec<String> {
    let hair = if v == "p1" { "hrs0001_00_default" } else { "hrs0002_00_default" };
    [
        "face0001_00_default",
        "eyb0001_00_default",
        hair,
        "drs0001_00_default",
        "ftw0002_00_default",
        "bag0001_00_default",
    ]
    .iter()
    .map(|s| format!("{v}_{s}"))
    .collect()
}

fn fnv_continue(mut state: u64, bytes: &[u8]) -> u64 {
    for b in bytes {
        state = (state ^ u64::from(*b)).wrapping_mul(FNV_PRIME);
    }
    state
}

fn bbox(parts: &[PokemonModel], skip_hidden: bool) -> Option<([f32; 3], [f32; 3])> {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    let mut any = false;
    for part in parts {
        for mesh in part.meshes.iter().filter(|m| m.lod == Some(0) && !(skip_hidden && m.name.starts_with("hide_"))) {
            for i in 0..mesh.mesh.vertex_count {
                if let Some(p) = mesh.mesh.position(&mesh.buffer_bytes, i) {
                    any = true;
                    for a in 0..3 {
                        min[a] = min[a].min(p[a]);
                        max[a] = max[a].max(p[a]);
                    }
                }
            }
        }
    }
    any.then_some((min, max))
}

struct Row {
    model: String,
    variant: String,
    status: &'static str,
    reason: String,
    parts: usize,
    primitives: usize,
    clips: usize,
    height_m: Option<f32>,
}

#[allow(clippy::too_many_lines)]
fn export_variant(
    chara: &PathBuf,
    out_root: &PathBuf,
    v: &str,
    parts_override: Option<&Vec<String>>,
    name: Option<&str>,
    skeleton_bytes: &[u8],
    verbose: bool,
    options: PlayerExportOptions,
    no_colors: bool,
    clips: &[(String, tr::TrAnmClip)],
) -> Result<Row, String> {
    let mut options = options;
    // ASSUMED defaults (no RGB in the data): franchise protagonists, dark brown (p1) / light brown (p2).
    if options.hair_rgb.is_none() && !no_colors {
        options.hair_rgb = Some(if v == "p1" { HAIR_PALETTE[0].1 } else { HAIR_PALETTE[2].1 });
    }
    if options.eye_rgb.is_none() && !no_colors {
        options.eye_rgb = Some(DEFAULT_EYE_RGB);
    }
    let stems = parts_override.cloned().unwrap_or_else(|| default_parts(v));
    let out_name = name.map_or_else(|| format!("{v}_default"), str::to_owned);
    let mut models = Vec::new();
    let mut exts: Vec<(String, Vec<u8>)> = Vec::new();
    let mut notes = Vec::new();
    for stem in &stems {
        let cat = category_dir(stem)?;
        let variant = stem.split('_').next().unwrap_or(v);
        let pack = chara.join(format!("pc/{variant}/model/{cat}/{stem}.gfpak"));
        let layout = PackLayout {
            base: format!("bin/chara/data/pc/{variant}/model/{cat}/{stem}"),
            mdl_dir: "",
            anm_dir: "",
            skeleton: Some(skeleton_bytes),
            allow_ik: true,
        };
        let model = assemble_pack_with(&pack, &layout).map_err(|e| format!("{stem}: {e}"))?;
        // Extension skeleton (face/hair/clothes bones), when the pack has one.
        let pack_bytes = fs::read(&pack).map_err(|e| e.to_string())?;
        let gp = GfPak::parse(&pack_bytes)?;
        if let Some(i) = gp.entry_index_by_path(&format!("{}/{stem}.trskl", layout.base))? {
            exts.push((stem.clone(), gp.decoded_data(&pack_bytes, &gp.entries[i])?));
        }
        let lod0 = model.meshes.iter().filter(|m| m.lod == Some(0)).count();
        if lod0 == 0 {
            notes.push(format!("{stem}: no mesh ({})", model.diagnostics.first().cloned().unwrap_or_default()));
            continue;
        }
        if verbose {
            for mesh in model.meshes.iter().filter(|m| m.lod == Some(0)) {
                let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
                for i in 0..mesh.mesh.vertex_count {
                    if let Some(p) = mesh.mesh.position(&mesh.buffer_bytes, i) {
                        for a in 0..3 {
                            mn[a] = mn[a].min(p[a]);
                            mx[a] = mx[a].max(p[a]);
                        }
                    }
                }
                println!("  mesh {} verts={} tris={} bbox {:.2?}..{:.2?}", mesh.name, mesh.mesh.vertex_count, mesh.mesh.triangle_count(), mn, mx);
            }
            for m in &model.materials {
                for b in pla::assets::pokemon_material::parse_blocks(&m.raw_bytes) {
                    let srgb = |c: f32| (c.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8;
                    let layers: Vec<String> = (1..=4)
                        .filter_map(|i| b.vec4.get(&format!("BaseColorLayer{i}")).map(|v| format!("L{i}=({},{},{})", srgb(v[0]), srgb(v[1]), srgb(v[2]))))
                        .collect();
                    let em = b.vec4.get("EmissionColor").map(|v| format!("({:.2},{:.2},{:.2})", v[0], v[1], v[2]));
                    println!("  material {stem}/{} shader={} tex={} {} emission={:?}", b.name, b.shader, b.textures.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join(","), layers.join(" "), em);
                }
            }
        }
        for d in model.diagnostics.iter().filter(|d| d.starts_with("mesh ") || d.starts_with("material ")) {
            notes.push(format!("{stem}: {d}"));
        }
        models.push(model);
    }
    let refs: Vec<&PokemonModel> = models.iter().collect();
    let skel: PlayerSkeleton = build_player_skeleton(skeleton_bytes, &exts)?;
    for n in &skel.notes {
        println!("  skeleton {n}");
    }
    let mut usable = Vec::new();
    for (clip_name, clip) in clips {
        match pla::assets::gltf_export::animation_tracks_from_tr(&skel.skeleton, clip) {
            Ok(_) => usable.push((clip_name.clone(), clip.clone())),
            Err(e) => notes.push(format!("clip {clip_name}: {e}")),
        }
    }
    let clips = &usable[..];
    let dir = out_root.join(&out_name);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let gltf = dir.join("player.gltf");
    write_player_gltf(&refs, &skel, clips, &gltf, Some(LayerBlendFormula::MaskLerp), options)?;
    let bb = bbox(&models, options.skip_hidden);
    let height = bb.map(|(mn, mx)| mx[1] - mn[1]);
    if let Some((mn, mx)) = bb {
        println!("{out_name}: bbox min {mn:.3?} max {mx:.3?} (rest pose, metres)");
    }
    let primitives: usize = models.iter().map(|m| m.meshes.iter().filter(|x| x.lod == Some(0)).count()).sum();
    for n in &notes {
        println!("  note {n}");
    }
    Ok(Row {
        model: out_name,
        variant: v.to_owned(),
        status: "ok",
        reason: notes.join(" | ").replace(['\t', '\n'], " ").chars().take(220).collect(),
        parts: models.len(),
        primitives,
        clips: clips.len(),
        height_m: height,
    })
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 2 {
        return Err("usage: player_export <archive/chara/data> <out dir> [--variant p1|p2] [--parts a,b] [--name n] [--index tsv]".into());
    }
    let chara = PathBuf::from(&args[0]);
    let out_root = PathBuf::from(&args[1]);
    let opt = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let variants: Vec<String> = opt("--variant").map_or_else(|| vec!["p1".into(), "p2".into()], |v| vec![v]);
    let parts_override = opt("--parts").map(|p| p.split(',').map(str::to_owned).collect::<Vec<_>>());
    let name = opt("--name");
    let index = opt("--index");
    let verbose = args.iter().any(|a| a == "--materials");
    // Skin tones: chara/table/skin_color_table.bin holds 8 rows (two groups of the same 4 RGB tones).
    let table_path = chara.join("../../../chara/table/skin_color_table.bin");
    let skin_rows: Vec<[u8; 3]> = fs::read(&table_path)
        .map_err(|e| format!("{}: {e}", table_path.display()))
        .and_then(|b| read_skin_color_table(&b))?
        .into_iter()
        .map(|(_, rgb)| rgb)
        .collect();
    println!("skin_color_table rows: {skin_rows:?}");
    let no_colors = args.iter().any(|a| a == "--no-hair-eye-color");
    let hair_rgb = opt("--hair-color").map(|c| parse_color(&c, HAIR_PALETTE)).transpose()?;
    let eye_rgb = opt("--eye-color").map(|c| parse_color(&c, EYE_PALETTE)).transpose()?;
    let skin_index = opt("--skin-tone").or_else(|| opt("--skin")).map_or(Some(0), |s| s.parse::<usize>().ok());
    let options = PlayerExportOptions {
        skip_hidden: args.iter().any(|a| a == "--skip-hide"),
        skin_rgb: if args.iter().any(|a| a == "--no-skin-tone") { None } else { skin_index.and_then(|i| skin_rows.get(i).copied()) },
        hair_rgb,
        eye_rgb,
    };

    // Shared skeleton: the p2 base pack is the only one that carries a trskl.
    let base_pack = chara.join("pc/p2/model/base/p2_base0001_00_default.gfpak");
    let base_bytes = fs::read(&base_pack).map_err(|e| format!("{}: {e}", base_pack.display()))?;
    let base = GfPak::parse(&base_bytes)?;
    let skl_path = "bin/chara/data/pc/p2/model/base/p2_base0001_00_default/p2_base0001_00_default.trskl";
    let skl_index = base.entry_index_by_path(skl_path)?.ok_or("skeleton entry not found")?;
    let skeleton_bytes = base.decoded_data(&base_bytes, &base.entries[skl_index])?;

    // Clips from pc.gfpak, resolved through the recovered directory hash state.
    let pc_bytes = fs::read(chara.join("pc.gfpak")).map_err(|e| e.to_string())?;
    let pc = GfPak::parse(&pc_bytes)?;
    let mut clips = Vec::new();
    for (file, export_name) in CLIPS {
        let full = format!("/pc0002_00_{file}.tranm");
        let hash = fnv_continue(ANIM_DIR_STATE, full.as_bytes());
        let Some(i) = pc.entry_path_hashes.iter().position(|h| *h == hash) else {
            println!("clip {file}: hash not in pc.gfpak");
            continue;
        };
        let bytes = pc.decoded_data(&pc_bytes, &pc.entries[i])?;
        match tr::TrAnm::parse_tracks(&bytes) {
            Ok(mut clip) => {
                // `chara_root_exp` is the only track that names no skeleton bone (the base
                // skeleton has `chara_root`); semantics unproven, so it is dropped, not applied.
                if let Some(t) = clip.tracks.iter().find(|t| t.name == "chara_root_exp") {
                    let k: Vec<_> = t.translation.keys.iter().map(|k| k.value).collect();
                    println!("  chara_root_exp in {file}: {} translation keys, first {:?} last {:?}", k.len(), k.first(), k.last());
                }
                clip.tracks.retain(|t| t.name != "chara_root_exp");
                println!("clip {file} -> {export_name}: {} frames @ {} fps, {} tracks", clip.frame_count, clip.frame_rate, clip.tracks.len());
                // Root motion of the `origin` bone (the exporter keeps it; the game pins it):
                // first/last translation, used to size the dodge roll and the crouch walk.
                if let Some(t) = clip.tracks.iter().find(|t| t.name == "origin") {
                    if let (Some(a), Some(b)) = (t.translation.keys.first(), t.translation.keys.last()) {
                        let d = [b.value[0] - a.value[0], b.value[1] - a.value[1], b.value[2] - a.value[2]];
                        println!(
                            "  origin root motion {export_name}: dx {:.3} dy {:.3} dz {:.3} m over {:.3} s",
                            d[0], d[1], d[2], clip.frame_count as f32 / clip.frame_rate.max(1) as f32
                        );
                    }
                }
                clips.push(((*export_name).to_owned(), clip));
            }
            Err(e) => println!("clip {file}: {e}"),
        }
    }

    let mut rows = Vec::new();
    for v in &variants {
        match export_variant(&chara, &out_root, v, parts_override.as_ref(), name.as_deref(), &skeleton_bytes, verbose, options, no_colors, &clips) {
            Ok(row) => rows.push(row),
            Err(e) => {
                println!("{v}: BLOCKED {e}");
                rows.push(Row {
                    model: name.clone().unwrap_or_else(|| format!("{v}_default")),
                    variant: v.clone(),
                    status: "blocked",
                    reason: e.replace(['\t', '\n'], " ").chars().take(220).collect(),
                    parts: 0,
                    primitives: 0,
                    clips: 0,
                    height_m: None,
                });
            }
        }
    }
    if let Some(index) = index {
        let mut tsv = String::from(
            "# sheet: data/player_model_export_index\n# version: 1\n# generator: crates/pla/examples/player_export.rs\n# target: Pokémon Legends: Arceus v262144 player character (default outfit parts merged onto the shared biped skeleton)\nmodel:string*\tvariant:string\tstatus:enum\treason:string?\tpart_count:u8\tprimitive_count:u16\tclip_count:u8\theight_m:f32\n",
        );
        for r in &rows {
            tsv.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                r.model,
                r.variant,
                r.status,
                r.reason,
                r.parts,
                r.primitives,
                r.clips,
                r.height_m.map_or_else(|| "0".to_owned(), |h| format!("{h:.3}"))
            ));
        }
        fs::write(index, tsv).map_err(|e| e.to_string())?;
    }
    Ok(())
}
