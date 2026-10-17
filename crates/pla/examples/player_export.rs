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
//!
//! Dress-up catalogue (Brief 36): with `--catalog <tsv> --parts-out <dir> --parts-index <tsv>`
//! the exporter writes ONE glTF per catalogue piece (`data/player_parts_catalog.tsv`, read from
//! `chara/table/dressup_table_p*.bin`) into `<parts-out>/<variant>/<stem>/piece.gltf`, each over
//! the shared biped base + its own extension skeleton, with every body clip plus the REAL face
//! clips of `pc/dressup_motion.gfpak` (`00011..00021_faceNN_loop`, `00201_eye_close01`) that the
//! player face rig can resolve. Hair/eyebrow/eye colour variants are baked as PNGs and listed in
//! `<piece>/colors.tsv`; skin tones are left to the glTF `baseColorFactor`.

use pla::assets::{
    gfpak::GfPak,
    player_model::{
        build_player_skeleton, read_skin_color_table, write_player_gltf, PlayerExportOptions, PlayerSkeleton,
        DEFAULT_EYE_RGB, EYE_COLOR_PALETTE, HAIR_COLOR_PALETTE,
    },
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
    // Brief 38 follow-up (feat/monturas2): the mounted-rider pose while a ride Pokémon is
    // active. Names are exact `.tranm` names from the pack's animation controller dump
    // (`work/cache/player/dump/pc/0006.bin`); the controller groups them under the mount's
    // own blend set (`ods_*` Wyrdeer, `hmg_*` Ursaluna, `bsr_*` Basculegion, `wgl_*`
    // Braviary; the lone `03602_wait01_loop` is the fifth set, read as Sneasler: PROBABLE).
    // Idle = the seated wait; move = the mount's run/walk loop (the rider's gait).
    ("03001_wait01_loop", "ride_wyrdeer_idle"),
    ("03021_run01_loop", "ride_wyrdeer_move"),
    ("03202_wait01_loop", "ride_ursaluna_idle"),
    ("03221_run01_loop", "ride_ursaluna_move"),
    ("03402_wait01_loop", "ride_basculegion_idle"),
    ("03421_run01_loop", "ride_basculegion_move"),
    ("03602_wait01_loop", "ride_sneasler_idle"),
    ("03801_wait01_loop", "ride_braviary_idle"),
    // Braviary has no ground gait in the set (it flies): `walk01` is its airborne loop.
    ("03811_walk01_loop", "ride_braviary_move"),
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

/// Face clips of `pc/dressup_motion.gfpak` (VERIFIED names, from the animation controller
/// `pc/0006.bin`-style listing of that pack): 11 expression/mouth-shape loops and the blink.
/// `DRESSUP_MOTION_STATE` is the FNV-1a 64 state of their (unknown) directory text, recovered by
/// inverting the hashes of six known names; a second, smaller copy of the same names exists in
/// the pack (NPC face rig: no eyeball/upper-eyelid tracks) and is NOT used.
const DRESSUP_MOTION_STATE: u64 = 3_548_500_725_770_503_767;
const FACE_CLIPS: &[(&str, &str)] = &[
    ("00011_face01_loop", "face_01"),
    ("00012_face02_loop", "face_02"),
    ("00013_face03_loop", "face_03"),
    ("00014_face04_loop", "face_04"),
    ("00015_face05_loop", "face_05"),
    ("00016_face06_loop", "face_06"),
    ("00017_face07_loop", "face_07"),
    ("00018_face08_loop", "face_08"),
    ("00019_face09_loop", "face_09"),
    ("00020_face10_loop", "face_10"),
    ("00021_face11_loop", "face_11"),
    ("00201_eye_close01", "eye_close"),
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
        options.hair_rgb = Some(if v == "p1" { HAIR_COLOR_PALETTE[0].1 } else { HAIR_COLOR_PALETTE[2].1 });
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

/// The face clips of `pc/dressup_motion.gfpak` (expression loops + blink), parsed and with the
/// `chara_root_exp` bookkeeping track dropped like the body clips. Missing pack or entries are
/// reported and skipped (an older dump must not fail the export).
fn load_face_clips(chara: &PathBuf) -> Result<Vec<(String, tr::TrAnmClip)>, String> {
    let pack_path = chara.join("pc/dressup_motion.gfpak");
    let bytes = fs::read(&pack_path).map_err(|e| format!("{}: {e}", pack_path.display()))?;
    let pack = GfPak::parse(&bytes)?;
    let mut clips = Vec::new();
    for (file, export_name) in FACE_CLIPS {
        let full = format!("/{file}.tranm");
        let hash = fnv_continue(DRESSUP_MOTION_STATE, full.as_bytes());
        let Some(i) = pack.entry_path_hashes.iter().position(|h| *h == hash) else {
            println!("face clip {file}: hash not in dressup_motion.gfpak");
            continue;
        };
        let data = pack.decoded_data(&bytes, &pack.entries[i])?;
        match tr::TrAnm::parse_tracks(&data) {
            Ok(mut clip) => {
                // `chara_root_exp` and `head_dummy` are helper tracks that name no bone of the
                // player skeleton (the base has `chara_root` and `head`); dropped, not applied.
                clip.tracks.retain(|t| t.name != "chara_root_exp" && t.name != "head_dummy");
                println!(
                    "face clip {file} -> {export_name}: {} frames @ {} fps, {} tracks",
                    clip.frame_count,
                    clip.frame_rate,
                    clip.tracks.len()
                );
                clips.push(((*export_name).to_owned(), clip));
            }
            Err(e) => println!("face clip {file}: {e}"),
        }
    }
    Ok(clips)
}

struct PieceRow {
    variant: String,
    slot: String,
    stem: String,
    gltf: String,
    status: &'static str,
    reason: String,
    primitives: usize,
    clips: usize,
    color_variants: usize,
    height_m: Option<f32>,
}

fn read_catalog(path: &str) -> Result<Vec<(String, String, String, usize)>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 5 || cols[0] == "variant:string*" {
            continue;
        }
        let count = cols[3].parse::<usize>().unwrap_or(0);
        out.push((cols[0].to_owned(), cols[1].to_owned(), cols[2].to_owned(), count));
    }
    if out.is_empty() {
        return Err(format!("{path}: no catalogue rows"));
    }
    Ok(out)
}

/// One glTF per catalogue piece (Brief 36). Each piece is self-contained: the shared biped base
/// plus its own extension skeleton, every body clip and the face clips it can resolve, its own
/// textures (default colours baked; hair/eyebrow/eye palettes written as variants + colors.tsv).
#[allow(clippy::too_many_arguments)]
fn export_catalog(
    chara: &PathBuf,
    parts_out: &PathBuf,
    parts_index: &str,
    catalog_path: &str,
    skeleton_bytes: &[u8],
    skin_rows: &[[u8; 3]],
    body_clips: &[(String, tr::TrAnmClip)],
    face_clips: &[(String, tr::TrAnmClip)],
    variants: &[String],
) -> Result<(), String> {
    let catalog = read_catalog(catalog_path)?;
    let mut rows = Vec::new();
    for v in variants {
        for (variant, slot, stem, colour_count) in catalog.iter().filter(|(variant, _, _, _)| variant == v) {
            let path = parts_out.join(variant).join(stem).join("piece.gltf");
            let row = match export_piece(chara, &path, variant, slot, stem, skeleton_bytes, skin_rows, body_clips, face_clips) {
                Ok((primitives, clips, colors, height)) => PieceRow {
                    variant: variant.clone(),
                    slot: slot.clone(),
                    stem: stem.clone(),
                    gltf: index_path(parts_out, variant, stem),
                    status: "ok",
                    reason: format!("catalogue rows: {colour_count}"),
                    primitives,
                    clips,
                    color_variants: colors,
                    height_m: height,
                },
                Err(e) => {
                    println!("{stem}: BLOCKED {e}");
                    PieceRow {
                        variant: variant.clone(),
                        slot: slot.clone(),
                        stem: stem.clone(),
                        gltf: index_path(parts_out, variant, stem),
                        status: "blocked",
                        reason: e.replace(['\t', '\n'], " ").chars().take(200).collect(),
                        primitives: 0,
                        clips: 0,
                        color_variants: 0,
                        height_m: None,
                    }
                }
            };
            println!(
                "{} {}: {} parts={} prims={} clips={} colors={} h={:?}",
                row.variant, row.stem, row.status, 1, row.primitives, row.clips, row.color_variants, row.height_m
            );
            rows.push(row);
        }
    }
    let ok = rows.iter().filter(|r| r.status == "ok").count();
    let mut tsv = String::from(
        "# sheet: data/player_parts_index\n# version: 1\n# generator: crates/pla/examples/player_export.rs\n# target: Pokémon Legends: Arceus v262144 dress-up pieces (one glTF per catalogue stem, shared biped base)\n# note: `gltf` is relative to the cache root when the export ran inside it; `colour_variants` counts the baked hair/eyebrow/eye PNGs listed in the piece's colors.tsv.\nvariant:string*\tslot:enum*\tstem:string*\tstatus:enum\treason:string?\tgltf:string\tprimitive_count:u16\tclip_count:u8\tcolour_variants:u16\theight_m:f32\n",
    );
    for r in &rows {
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.variant,
            r.slot,
            r.stem,
            r.status,
            r.reason,
            r.gltf,
            r.primitives,
            r.clips,
            r.color_variants,
            r.height_m.map_or_else(|| "0".to_owned(), |h| format!("{h:.3}"))
        ));
    }
    fs::write(parts_index, tsv).map_err(|e| e.to_string())?;
    println!("{parts_index}: {} pieces, {ok} ok", rows.len());
    Ok(())
}

/// Path written into the index: relative to the cache root when the export runs inside it
/// (`.../cache/player/parts/...`), else absolute.
fn index_path(parts_out: &PathBuf, variant: &str, stem: &str) -> String {
    let rel = PathBuf::from("player").join("parts").join(variant).join(stem).join("piece.gltf");
    if parts_out.ends_with(PathBuf::from("player").join("parts")) {
        rel.to_string_lossy().replace('\\', "/")
    } else {
        parts_out.join(variant).join(stem).join("piece.gltf").to_string_lossy().replace('\\', "/")
    }
}

/// The `.trskl` extension skeleton of a pack, when it has one.
fn read_piece_ext(chara: &PathBuf, variant: &str, cat: &str, stem: &str) -> Result<Option<(String, Vec<u8>)>, String> {
    let pack = chara.join(format!("pc/{variant}/model/{cat}/{stem}.gfpak"));
    if !pack.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(&pack).map_err(|e| e.to_string())?;
    let gp = GfPak::parse(&bytes)?;
    let base = format!("bin/chara/data/pc/{variant}/model/{cat}/{stem}");
    if let Some(i) = gp.entry_index_by_path(&format!("{base}/{stem}.trskl"))? {
        return Ok(Some((stem.to_owned(), gp.decoded_data(&bytes, &gp.entries[i])?)));
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn export_piece(
    chara: &PathBuf,
    gltf: &PathBuf,
    variant: &str,
    slot: &str,
    stem: &str,
    skeleton_bytes: &[u8],
    skin_rows: &[[u8; 3]],
    body_clips: &[(String, tr::TrAnmClip)],
    face_clips: &[(String, tr::TrAnmClip)],
) -> Result<(usize, usize, usize, Option<f32>), String> {
    let cat = category_dir(stem)?;
    let pack = chara.join(format!("pc/{variant}/model/{cat}/{stem}.gfpak"));
    let layout = PackLayout {
        base: format!("bin/chara/data/pc/{variant}/model/{cat}/{stem}"),
        mdl_dir: "",
        anm_dir: "",
        skeleton: Some(skeleton_bytes),
        allow_ik: true,
    };
    let model = assemble_pack_with(&pack, &layout).map_err(|e| format!("{stem}: {e}"))?;
    // The piece's own extension skeleton, plus the canonical extensions of the other rig ranges
    // its vertices may reference (the eyebrow and some hats skin to the FACE rig; a piece of one
    // slot replaces that slot's canonical extension, ranges are per slot: face 74, hair 125,
    // clothes 147+, see reports/data-progress/player_model.md).
    let own = read_piece_ext(chara, variant, cat, stem)?;
    let face_default = format!("{variant}_face0001_00_default");
    let hair_default = if variant == "p1" { format!("{variant}_hrs0001_00_default") } else { format!("{variant}_hrs0002_00_default") };
    let dress_default = format!("{variant}_drs0001_00_default");
    let face_canonical = read_piece_ext(chara, variant, "face", &face_default)?;
    let hair_canonical = read_piece_ext(chara, variant, "hair", &hair_default)?;
    let dress_canonical = read_piece_ext(chara, variant, "dress", &dress_default)?;
    let missing = |what: &str| format!("no {what} extension skeleton");
    let mut exts: Vec<(String, Vec<u8>)> = Vec::new();
    match slot {
        "face" => exts.push(own.or(face_canonical).ok_or_else(|| missing("face"))?),
        "hrs" => {
            exts.push(face_canonical.ok_or_else(|| missing("face"))?);
            // Some short hairstyles carry no extension at all (base head bones only).
            exts.extend(own.or(hair_canonical));
            exts.extend(dress_canonical);
        }
        "drs" | "tps" | "btm" => {
            exts.push(face_canonical.ok_or_else(|| missing("face"))?);
            exts.extend(hair_canonical);
            exts.push(own.or(dress_canonical).ok_or_else(|| missing("dress"))?);
        }
        _ => {
            exts.push(face_canonical.ok_or_else(|| missing("face"))?);
            exts.extend(hair_canonical);
            exts.push(own.or(dress_canonical).ok_or_else(|| missing("clothes"))?);
        }
    }
    let skel: PlayerSkeleton = build_player_skeleton(skeleton_bytes, &exts)?;
    // Every clip whose tracks all name bones of THIS piece's skeleton (the face piece carries the
    // face rig and therefore the face clips; body pieces get the body clips).
    let mut usable: Vec<(String, tr::TrAnmClip)> = Vec::new();
    for (name, clip) in body_clips.iter().chain(face_clips.iter()) {
        match pla::assets::gltf_export::animation_tracks_from_tr(&skel.skeleton, clip) {
            Ok(_) => usable.push((name.clone(), clip.clone())),
            Err(e) => {
                if slot == "face" && face_clips.iter().any(|(n, _)| n == name) {
                    println!("  face piece: clip {name} not usable: {e}");
                }
            }
        }
    }
    let face_piece = slot == "face";
    let options = PlayerExportOptions {
        skip_hidden: false,
        skin_rgb: skin_rows.first().copied(),
        hair_rgb: Some(if variant == "p1" { HAIR_COLOR_PALETTE[0].1 } else { HAIR_COLOR_PALETTE[2].1 }),
        eye_rgb: Some(DEFAULT_EYE_RGB),
        color_variants: matches!(slot, "hrs" | "eyb" | "face"),
        skin_unbaked: true,
    };
    let dir = gltf.parent().ok_or("piece gltf has no parent")?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let report = write_player_gltf(&[&model], &skel, &usable, gltf, Some(LayerBlendFormula::MaskLerp), options)?;
    if !report.color_files.is_empty() {
        let mut tsv = String::from(
            "# sheet: dress-up colour variants for this piece\n# set:enum*\tname:string*\tfile:string\n",
        );
        for (set, name, file) in &report.color_files {
            tsv.push_str(&format!("{set}\t{name}\t{file}\n"));
        }
        fs::write(dir.join("colors.tsv"), tsv).map_err(|e| e.to_string())?;
    }
    let primitives = model.meshes.iter().filter(|m| m.lod == Some(0)).count();
    let height = bbox(std::slice::from_ref(&model), false).map(|(mn, mx)| mx[1] - mn[1]);
    let _ = face_piece;
    Ok((primitives, usable.len(), report.color_files.len(), height))
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
    let hair_rgb = opt("--hair-color").map(|c| parse_color(&c, HAIR_COLOR_PALETTE)).transpose()?;
    let eye_rgb = opt("--eye-color").map(|c| parse_color(&c, EYE_COLOR_PALETTE)).transpose()?;
    let skin_index = opt("--skin-tone").or_else(|| opt("--skin")).map_or(Some(0), |s| s.parse::<usize>().ok());
    let options = PlayerExportOptions {
        skip_hidden: args.iter().any(|a| a == "--skip-hide"),
        skin_rgb: if args.iter().any(|a| a == "--no-skin-tone") { None } else { skin_index.and_then(|i| skin_rows.get(i).copied()) },
        hair_rgb,
        eye_rgb,
        color_variants: false,
        skin_unbaked: false,
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

    // Dress-up catalogue mode (Brief 36): one glTF per piece, over the shared biped base.
    if let Some(catalog_path) = opt("--catalog") {
        let parts_out = opt("--parts-out").map(PathBuf::from).ok_or("--catalog needs --parts-out <dir>")?;
        let parts_index = opt("--parts-index").ok_or("--catalog needs --parts-index <tsv>")?;
        let face_clips = load_face_clips(&chara)?;
        println!("face clips: {} parsed", face_clips.len());
        return export_catalog(&chara, &parts_out, &parts_index, &catalog_path, &skeleton_bytes, &skin_rows, &clips, &face_clips, &variants);
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
