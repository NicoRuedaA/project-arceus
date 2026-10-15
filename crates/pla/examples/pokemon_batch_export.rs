//! Resumable batch export of every Pokémon model pack to glTF (one pack at a time).
//!
//! Safe baseline material: plain base `alb` texture + normal map only; the unproven
//! layer-mask / AO mixing is deliberately skipped.
//!
//! Besides `*_idle.gltf` / `*_walk.gltf` it writes `*_battle.gltf` with the battle clips
//! (see `BattleClipSlot`: battle idle, attack, ranged attack, damage, down, appear).
//!
//! Usage: pokemon_batch_export <archive/pokemon dir> <output-dir> <output-index.tsv> [--force] [--only <stem>]

use pla::assets::pokemon_model::{
    assemble_pack, battle_clip_names, export_bind_records, write_skinned_lod_level_clips_variant,
    write_skinned_lod_level_variant, write_static_lod_level_baked_variant, LayerBlendFormula,
    POKEMON_BIND_EPSILON,
};
use std::{
    env, fs,
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
};

struct Outcome {
    status: &'static str,
    reason: String,
    clips: usize,
    textures: usize,
}

fn count_pngs(dir: &Path, rare: bool) -> usize {
    fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter(|e| {
                    e.path().extension().is_some_and(|x| x == "png")
                        && e.file_name().to_string_lossy().contains("_rare") == rare
                })
                .count()
        })
        .unwrap_or(0)
}

fn export_pack(pack: &Path, out: &Path, stem: &str, rare: bool) -> Outcome {
    let blocked = |reason: String| Outcome {
        status: "blocked",
        reason,
        clips: 0,
        textures: 0,
    };
    let model = match assemble_pack(pack) {
        Ok(m) => m,
        Err(e) => return blocked(format!("assemble: {e}")),
    };
    if !model.meshes.iter().any(|m| m.lod == Some(0)) {
        let first = model
            .diagnostics
            .iter()
            .find(|d| d.starts_with("mesh "))
            .cloned()
            .unwrap_or_default();
        return blocked(format!("no LOD0 mesh parsed ({first})"));
    }
    if let Err(e) = fs::create_dir_all(out) {
        return blocked(format!("mkdir: {e}"));
    }
    let pick = |needle: &str| {
        model
            .clips
            .iter()
            .filter(|c| c.parsed.is_ok() && c.name.contains(needle))
            .min_by_key(|c| (!c.name.contains("01_loop"), c.name.clone()))
    };
    let mut clips = 0;
    let mut notes: Vec<String> = Vec::new();
    if model.skeleton.is_some() && !model.bind_records.is_empty() {
        if let Ok((_, rebuilt)) = export_bind_records(&model, POKEMON_BIND_EPSILON) {
            if rebuilt > 0 {
                notes.push(format!(
                    "bind: {rebuilt}/{} records rebuilt from the reference rest pose",
                    model.bind_records.len()
                ));
            }
        }
    }
    let prefix = if rare {
        format!("{stem}_rare")
    } else {
        stem.to_owned()
    };
    for (label, needle) in [("idle", "defaultwait"), ("walk", "walk")] {
        if let Some(clip) = pick(needle) {
            let file = out.join(format!("{prefix}_{label}.gltf"));
            match write_skinned_lod_level_variant(
                &model,
                0,
                &clip.name,
                &file,
                Some(LayerBlendFormula::MaskLerp),
                rare,
            ) {
                Ok(()) => clips += 1,
                Err(e) => notes.push(format!("{label}: {e}")),
            }
        } else {
            notes.push(format!("{label}: no parsed clip"));
        }
    }
    if clips > 0 {
        // Battle clips (attack, hit, faint, ...) of the same skeleton in one extra file.
        let parsed: Vec<&str> = model
            .clips
            .iter()
            .filter(|c| c.parsed.is_ok())
            .map(|c| c.name.as_str())
            .collect();
        let wanted = battle_clip_names(&parsed);
        if wanted.is_empty() {
            notes.push("battle: no clips".into());
        } else {
            let file = out.join(format!("{prefix}_battle.gltf"));
            match write_skinned_lod_level_clips_variant(
                &model,
                0,
                &wanted,
                &file,
                Some(LayerBlendFormula::MaskLerp),
                rare,
            ) {
                Ok(written) => {
                    clips += written.len();
                    if written.len() < wanted.len() {
                        notes.push(format!("battle: {}/{} clips", written.len(), wanted.len()));
                    }
                }
                Err(e) => notes.push(format!("battle: {e}")),
            }
        }
    }
    if clips == 0 {
        // Fall back to the static mesh so the model is still usable.
        let file = out.join(format!("{prefix}_static.gltf"));
        return match write_static_lod_level_baked_variant(
            &model,
            0,
            &file,
            LayerBlendFormula::MaskLerp,
            rare,
        ) {
            Ok(()) => Outcome {
                status: "ok",
                reason: format!("static only; {}", notes.join(" | ")),
                clips: 0,
                textures: count_pngs(out, rare),
            },
            Err(e) => blocked(format!("static export: {e}; {}", notes.join(" | "))),
        };
    }
    Outcome {
        status: "ok",
        reason: if notes.is_empty() {
            String::new()
        } else {
            notes.join(" | ")
        },
        clips,
        textures: count_pngs(out, rare),
    }
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
        .chars()
        .take(220)
        .collect()
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 3 {
        return Err("usage: pokemon_batch_export <pokemon dir> <out dir> <index.tsv> [--force] [--rare] [--only stem]".into());
    }
    let (src, out_root, index) = (
        PathBuf::from(&args[0]),
        PathBuf::from(&args[1]),
        PathBuf::from(&args[2]),
    );
    let force = args.iter().any(|a| a == "--force");
    let rare = args.iter().any(|a| a == "--rare");
    let only = args
        .iter()
        .position(|a| a == "--only")
        .and_then(|i| args.get(i + 1))
        .cloned();
    fs::create_dir_all(&out_root).map_err(|e| e.to_string())?;
    let mut packs: Vec<PathBuf> = fs::read_dir(&src)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "gfpak")
                && p.file_stem()
                    .is_some_and(|s| s.to_string_lossy().starts_with("pm"))
        })
        .collect();
    packs.sort();
    // Silence panic backtraces; they are recorded as blocked reasons.
    std::panic::set_hook(Box::new(|_| {}));
    for pack in &packs {
        let stem = pack.file_stem().unwrap().to_string_lossy().to_string();
        if only
            .as_ref()
            .is_some_and(|o| !o.split(',').any(|x| x == stem))
        {
            continue;
        }
        let out = out_root.join(&stem);
        let status_file = if rare {
            out_root.join(format!("{stem}.rare.status"))
        } else {
            out_root.join(format!("{stem}.status"))
        };
        if !force && status_file.exists() {
            continue;
        }
        if !rare {
            let _ = fs::remove_dir_all(&out);
        }
        let result = catch_unwind(AssertUnwindSafe(|| export_pack(pack, &out, &stem, rare)));
        let o = result.unwrap_or_else(|p| Outcome {
            status: "blocked",
            reason: format!(
                "panic: {}",
                p.downcast_ref::<String>()
                    .cloned()
                    .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default()
            ),
            clips: 0,
            textures: 0,
        });
        let o = if o.status == "blocked" {
            Outcome { textures: 0, ..o }
        } else {
            o
        };
        fs::write(
            &status_file,
            format!(
                "{}\t{}\t{}\t{}\n",
                o.status,
                clean(&o.reason),
                o.clips,
                o.textures
            ),
        )
        .map_err(|e| e.to_string())?;
        println!(
            "{stem} {} clips={} tex={} {}",
            o.status,
            o.clips,
            o.textures,
            clean(&o.reason)
        );
    }
    // Rebuild the index from per-pack status files.
    let rare_source = fs::read_to_string("data/pokemon_pack_index.tsv")
        .ok()
        .and_then(|text| {
            let header = text.lines().find(|line| !line.starts_with('#'))?;
            let columns: Vec<&str> = header.split('\t').collect();
            let file_col = columns.iter().position(|c| *c == "file:string*")?;
            let rare_col = columns
                .iter()
                .position(|c| *c == "has_rare_shiny_material_name:bool")?;
            Some(
                text.lines()
                    .filter(|line| !line.starts_with('#'))
                    .skip(1)
                    .filter_map(|line| {
                        let fields: Vec<&str> = line.split('\t').collect();
                        Some((
                            fields.get(file_col)?.to_string(),
                            *fields.get(rare_col)? == "1",
                        ))
                    })
                    .collect::<std::collections::HashMap<_, _>>(),
            )
        })
        .unwrap_or_default();
    let mut tsv = String::from(
        "# sheet: data/pokemon_model_export_index\n# version: 2\n# generator: crates/pla/examples/pokemon_batch_export.rs\n# target: Pokémon Legends: Arceus v262144 Pokémon model packs (glTF export with colours baked from TRMTR layer masks)\npack:string*\tform:string\tstatus:enum\treason:string?\tclip_count:u8\ttexture_count:u8\trare_status:enum\n",
    );
    for pack in &packs {
        let stem = pack.file_stem().unwrap().to_string_lossy().to_string();
        let Ok(s) = fs::read_to_string(out_root.join(format!("{stem}.status"))) else {
            continue;
        };
        let f: Vec<&str> = s.trim_end_matches('\n').split('\t').collect();
        if f.len() != 4 {
            continue;
        }
        let form = stem
            .strip_prefix("pm")
            .and_then(|r| r.split_once('_'))
            .map_or("", |x| x.1);
        let rare_status = fs::read_to_string(out_root.join(format!("{stem}.rare.status")))
            .ok()
            .and_then(|status| status.split('\t').next().map(str::to_owned))
            .or_else(|| {
                rare_source.get(&format!("{stem}.gfpak")).map(|available| {
                    if *available {
                        "available".into()
                    } else {
                        "unavailable".into()
                    }
                })
            })
            .unwrap_or_else(|| "unknown".into());
        tsv.push_str(&format!(
            "{stem}.gfpak\t{form}\t{}\t{}\t{}\t{}\t{}\n",
            f[0], f[1], f[2], f[3], rare_status
        ));
    }
    fs::write(&index, tsv).map_err(|e| e.to_string())?;
    Ok(())
}
