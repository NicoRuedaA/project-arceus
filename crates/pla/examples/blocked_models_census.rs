//! Per-model TR layout census for the blocked-model classification step.
//!
//! Field mode walks the same packs as `export_field_gltf_batch` and writes one
//! diagnosis row per discovered model. Pokémon mode reports the assembly
//! diagnostics of every `pm*.gfpak` pack.
//!
//! Usage:
//!   blocked_models_census field <romfs-root> <output-tsv>
//!   blocked_models_census pokemon <archive/pokemon dir> <output-tsv>
//!   blocked_models_census dump <romfs-root> <pack path under romfs> <model> <out-dir>

use pla::assets::gfpak::GfPak;
use pla::assets::model_diagnosis::diagnose;
use pla::assets::tr::{TrMbf, TrShapeLayout};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("field") if args.len() == 3 => field_census(Path::new(&args[1]), Path::new(&args[2])),
        Some("pokemon") if args.len() == 3 => {
            pokemon_census(Path::new(&args[1]), Path::new(&args[2]))
        }
        Some("dump") if args.len() == 5 => {
            dump_model(Path::new(&args[1]), &args[2], &args[3], Path::new(&args[4]))
        }
        _ => {
            eprintln!(
                "usage: blocked_models_census field|pokemon <input> <output-tsv>\n\
                 \x20      blocked_models_census dump <romfs-root> <pack path under romfs> <model> <out-dir>"
            );
            std::process::exit(1);
        }
    }
}

/// Write the sibling `.trmsh`/`.trmbf`/`.trskl` entries of one model for byte-level inspection.
fn dump_model(
    romfs: &Path,
    pack_rel: &str,
    model: &str,
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(romfs.join(pack_rel))?;
    let gfpak = GfPak::parse(&bytes)?;
    let pack_stem = Path::new(pack_rel)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("pack path has no stem")?;
    let prefix = if pack_rel.contains("/terrain/") {
        format!("bin/field/terrain/{pack_stem}/terrain/")
    } else {
        "bin/field/model/pack/".to_string()
    };
    std::fs::create_dir_all(out_dir)?;
    for extension in ["trmsh", "trmbf", "trskl", "trmtr"] {
        let entry_path = format!("{prefix}{model}.{extension}");
        match read_entry(&gfpak, &bytes, &entry_path) {
            Some(data) => {
                let path = out_dir.join(format!("{model}.{extension}"));
                std::fs::write(&path, &data)?;
                eprintln!("wrote {} ({} bytes)", path.display(), data.len());
            }
            None => eprintln!("missing {entry_path}"),
        }
    }
    Ok(())
}

fn discover_models(bytes: &[u8], prefix: &str, gfpak: &GfPak) -> Vec<String> {
    let mut names = HashSet::new();
    let mut pos = 0;
    while pos + 6 <= bytes.len() {
        if &bytes[pos..pos + 3] == b".tr" {
            let mut start = pos;
            while start > 0 {
                let c = bytes[start - 1];
                if c.is_ascii_alphanumeric() || c == b'_' {
                    start -= 1;
                } else {
                    break;
                }
            }
            if pos - start >= 3 {
                if let Ok(s) = std::str::from_utf8(&bytes[start..pos]) {
                    names.insert(s.to_string());
                }
            }
        }
        pos += 1;
    }
    let mut found = Vec::new();
    for name in names {
        let mdl_path = format!("{prefix}{name}.trmdl");
        if matches!(gfpak.entry_index_by_path(&mdl_path), Ok(Some(_))) {
            found.push(name);
        }
    }
    found.sort();
    found
}

/// Which parse route the field exporter would take: the bounded legacy layout
/// (`legacy`), a FlatBuffer shape declaration (`shape`), or none.
fn parse_route(trmbf: &[u8], trmsh: &[u8]) -> &'static str {
    if TrMbf::parse_with_layout(trmbf, trmsh).is_ok() {
        return "legacy";
    }
    match TrShapeLayout::parse_all(trmsh) {
        Ok(shapes) => {
            for shape_index in 0..shapes.len() {
                if TrMbf::parse_with_shape_layout(trmbf, trmsh, shape_index).is_ok() {
                    return "shape";
                }
            }
            "none"
        }
        Err(_) => "none",
    }
}

fn read_entry(gfpak: &GfPak, bytes: &[u8], path: &str) -> Option<Vec<u8>> {
    let index = gfpak.entry_index_by_path(path).ok().flatten()?;
    gfpak.decoded_data(bytes, &gfpak.entries[index]).ok()
}

fn census_pack(
    writer: &mut BufWriter<File>,
    ctx: &(String, String),
    bytes: &[u8],
    gfpak: &GfPak,
) -> Result<usize, Box<dyn std::error::Error>> {
    let (pack_name, prefix) = ctx;
    let models = discover_models(bytes, prefix, gfpak);
    for model in &models {
        let trmsh = read_entry(gfpak, bytes, &format!("{prefix}{model}.trmsh"));
        let trmbf = read_entry(gfpak, bytes, &format!("{prefix}{model}.trmbf"));
        let trskl = read_entry(gfpak, bytes, &format!("{prefix}{model}.trskl"));
        let (present, diagnosis) = match trmbf.as_deref() {
            Some(trmbf) => (
                format!(
                    "trmsh={} trmbf={} trskl={}",
                    trmsh.is_some(),
                    true,
                    trskl.is_some()
                ),
                diagnose(trmbf, trmsh.as_deref(), trskl.as_deref()).summary(),
            ),
            None => (
                format!(
                    "trmsh={} trmbf=false trskl={}",
                    trmsh.is_some(),
                    trskl.is_some()
                ),
                "missing .trmbf".to_string(),
            ),
        };
        let route = trmbf
            .as_deref()
            .map(|trmbf| {
                trmsh
                    .as_deref()
                    .map_or("none", |trmsh| parse_route(trmbf, trmsh))
            })
            .unwrap_or("missing-buffer");
        writeln!(
            writer,
            "{pack_name}\t{model}\t{present}\t{route}\t{}",
            diagnosis.replace(['\t', '\n', '\r'], " ")
        )?;
    }
    Ok(models.len())
}

fn field_census(root: &Path, out_tsv: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = BufWriter::new(File::create(out_tsv)?);
    writeln!(writer, "source_pack\tmodel_name\tpresent\troute\tdiagnosis")?;
    let mut total = 0usize;
    let model_pack_dir = root.join("bin/archive/field/model/pack");
    let mut packs: Vec<PathBuf> = Vec::new();
    if model_pack_dir.exists() {
        for entry in std::fs::read_dir(&model_pack_dir)? {
            let path = entry?.path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if name.ends_with(".gfpak")
                && !name.contains("collision")
                && !name.contains("animation")
            {
                packs.push(path);
            }
        }
    }
    packs.sort();
    for pack_path in &packs {
        let pack_name = pack_path.file_name().unwrap().to_str().unwrap().to_string();
        eprintln!("field pack {pack_name}");
        let bytes = std::fs::read(pack_path)?;
        let Ok(gfpak) = GfPak::parse(&bytes) else {
            eprintln!("  cannot parse {pack_name}");
            continue;
        };
        total += census_pack(
            &mut writer,
            &(pack_name, "bin/field/model/pack/".to_string()),
            &bytes,
            &gfpak,
        )?;
    }
    let terrain_dir = root.join("bin/archive/field/terrain");
    let mut terrain_packs: Vec<PathBuf> = Vec::new();
    if terrain_dir.exists() {
        for entry in std::fs::read_dir(&terrain_dir)? {
            let path = entry?.path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if name.starts_with("ha_area") && name.ends_with(".gfpak") {
                terrain_packs.push(path);
            }
        }
    }
    terrain_packs.sort();
    for pack_path in &terrain_packs {
        let pack_name = pack_path.file_name().unwrap().to_str().unwrap().to_string();
        let area_stem = pack_path.file_stem().unwrap().to_str().unwrap().to_string();
        eprintln!("terrain pack {pack_name}");
        let bytes = std::fs::read(pack_path)?;
        let Ok(gfpak) = GfPak::parse(&bytes) else {
            eprintln!("  cannot parse {pack_name}");
            continue;
        };
        let prefix = format!("bin/field/terrain/{area_stem}/terrain/");
        let models = discover_models(&bytes, &prefix, &gfpak);
        let plants: Vec<String> = models
            .into_iter()
            .filter(|m| m.starts_with("plants_"))
            .collect();
        let mut count = 0usize;
        for model in &plants {
            let trmsh = read_entry(&gfpak, &bytes, &format!("{prefix}{model}.trmsh"));
            let trmbf = read_entry(&gfpak, &bytes, &format!("{prefix}{model}.trmbf"));
            let trskl = read_entry(&gfpak, &bytes, &format!("{prefix}{model}.trskl"));
            let (present, diagnosis) = match trmbf.as_deref() {
                Some(trmbf) => (
                    format!(
                        "trmsh={} trmbf=true trskl={}",
                        trmsh.is_some(),
                        trskl.is_some()
                    ),
                    diagnose(trmbf, trmsh.as_deref(), trskl.as_deref()).summary(),
                ),
                None => (
                    format!(
                        "trmsh={} trmbf=false trskl={}",
                        trmsh.is_some(),
                        trskl.is_some()
                    ),
                    "missing .trmbf".to_string(),
                ),
            };
            let route = trmbf
                .as_deref()
                .map(|trmbf| {
                    trmsh
                        .as_deref()
                        .map_or("none", |trmsh| parse_route(trmbf, trmsh))
                })
                .unwrap_or("missing-buffer");
            writeln!(
                writer,
                "{pack_name}\t{model}\t{present}\t{route}\t{}",
                diagnosis.replace(['\t', '\n', '\r'], " ")
            )?;
            count += 1;
        }
        total += count;
    }
    writer.flush()?;
    eprintln!("wrote {total} model diagnoses to {}", out_tsv.display());
    Ok(())
}

fn pokemon_census(dir: &Path, out_tsv: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use pla::assets::pokemon_model::assemble_pack;
    let mut packs: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "gfpak")
                && path
                    .file_stem()
                    .is_some_and(|stem| stem.to_string_lossy().starts_with("pm"))
        })
        .collect();
    packs.sort();
    let mut writer = BufWriter::new(File::create(out_tsv)?);
    writeln!(writer, "pack\tlod0_shapes\tdiagnostics")?;
    for pack in &packs {
        let stem = pack.file_stem().unwrap().to_string_lossy().to_string();
        let row = match assemble_pack(pack) {
            Ok(model) => {
                let lod0 = model
                    .meshes
                    .iter()
                    .filter(|mesh| mesh.lod == Some(0))
                    .count();
                let diagnostics = model
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.replace(['\t', '\n', '\r'], " "))
                    .collect::<Vec<_>>()
                    .join(" ;; ");
                format!("{lod0}\t{diagnostics}")
            }
            Err(error) => format!("0\tassemble: {}", error.replace(['\t', '\n', '\r'], " ")),
        };
        writeln!(writer, "{stem}\t{row}")?;
        eprintln!("{stem}");
    }
    writer.flush()?;
    Ok(())
}
