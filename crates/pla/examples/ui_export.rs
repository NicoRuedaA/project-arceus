//! UI texture exporter for `bin/appli` (the game's 2D interface data).
//!
//! Decodes every texture the in-game screens use and writes one PNG per
//! texture plus a TSV index, so the layouts decoded by
//! `.tools/ui_layout_decode.py` can be redrawn and a human can browse them:
//!
//! - standalone `appli/<screen>/**/<name>.bntx` (icons, pictures, tips images)
//!   -> `<out>/png/<screen>/<subpath>/<name>.png` (`bin/` is dropped from the subpath);
//! - the texture atlas embedded in every layout archive
//!   (`appli/<screen>/**/<arc>.arc` -> `timg/*.bntx`, usually `__Combined.bntx`)
//!   -> `<out>/png/<screen>/arc/<arc>/<texture name>.png`.
//!
//! - the screen packs `bin/archive/appli/*.gfpak` (battle menu, HUD, pause menu,
//!   Pokédex, map): every entry is decoded (Oodle) and written to
//!   `<out>/unpacked/<pack>/<path>`; entry names are recovered by hashing the
//!   `bin/...` paths found inside the pack (FNV-1a 64), unknown ones stay
//!   `_unresolved/<hash>.<ext>`. Their textures are exported as screen
//!   `gfpak_<pack>`. Only the base and `_eng` language packs are unpacked.
//!
//! Only mip 0 / array layer 0 is exported. The BNTX component selector
//! (`channels`, the GPU swizzle) is applied so the PNG shows what the GPU
//! samples; the raw selector is recorded in the index.
//!
//! The PNGs are written with the PNG `Up` filter only, which the dependency-free
//! Python tooling in `.tools/ui_*.py` reads quickly.
//!
//! Output is game content: keep `<out>` under the git-ignored `work/`.
//!
//! Usage: `cargo run -p pla --example ui_export -- <bin-root> <out-dir>`
//! (`<bin-root>` contains `appli/` and `archive/appli/`).

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use pla::assets::bntx::{Bntx, BntxTexture, TextureData};
use pla::assets::gfpak::{fnv1a64, GfPak};
use pla::assets::sarc::Sarc;

/// Language suffixes of the screen packs; only the base and English ones are unpacked.
const LANG_SUFFIXES: [&str; 8] = [
    "_fre", "_ger", "_ita", "_kor", "_sch", "_spa", "_tch", "_jpn",
];

fn ext_for(blob: &[u8]) -> &'static str {
    match blob.get(0..4) {
        Some(b"SARC") => "arc",
        Some(b"BNTX") => "bntx",
        _ => "bin",
    }
}

/// `bin/...` ASCII paths embedded in a blob (FlatBuffers manifests, SARC names).
fn embedded_paths(blob: &[u8], out: &mut std::collections::BTreeSet<String>) {
    let mut i = 0;
    while let Some(pos) = blob[i..].windows(4).position(|w| w == b"bin/") {
        let start = i + pos;
        let mut end = start;
        while end < blob.len() && (0x21..0x7f).contains(&blob[end]) {
            end += 1;
        }
        if end - start > 4 {
            out.insert(String::from_utf8_lossy(&blob[start..end]).to_string());
        }
        i = end.max(start + 1);
        if i >= blob.len() {
            break;
        }
    }
}

/// Candidate full paths: every embedded path plus the manifest names the
/// screens use next to each archive (`<arc>_lyt.bin`, `uikit_<arc>.bin`).
fn candidate_paths(found: &std::collections::BTreeSet<String>) -> Vec<String> {
    let mut out: Vec<String> = found.iter().cloned().collect();
    for p in found {
        if let Some(stem) = p.strip_suffix(".arc") {
            out.push(format!("{stem}_lyt.bin"));
            if let Some((dir, name)) = stem.rsplit_once('/') {
                out.push(format!("{dir}/uikit_{name}.bin"));
            }
        }
    }
    out
}

/// Unpacks one screen pack; returns the number of entries and of resolved names.
fn unpack_gfpak(
    pack_path: &Path,
    out_dir: &Path,
    index: &mut impl Write,
) -> Result<(usize, usize), String> {
    let pack_name = pack_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let bytes = fs::read(pack_path).map_err(|e| e.to_string())?;
    let pak = GfPak::parse(&bytes)?;
    let mut blobs = Vec::with_capacity(pak.entries.len());
    let mut found = std::collections::BTreeSet::new();
    for entry in &pak.entries {
        let blob = pak.decoded_data(&bytes, entry);
        if let Ok(b) = &blob {
            embedded_paths(b, &mut found);
        }
        blobs.push(blob);
    }
    let mut by_hash = BTreeMap::new();
    for path in candidate_paths(&found) {
        by_hash.insert(fnv1a64(path.as_bytes()), path);
    }
    let mut resolved = 0;
    for (i, blob) in blobs.iter().enumerate() {
        let hash = pak.entry_path_hashes.get(i).copied().unwrap_or(0);
        match blob {
            Ok(blob) => {
                let rel = match by_hash.get(&hash) {
                    Some(p) => {
                        resolved += 1;
                        p.clone()
                    }
                    None => format!("_unresolved/{hash:016x}.{}", ext_for(blob)),
                };
                let target = out_dir.join(&pack_name).join(&rel);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::write(&target, blob).map_err(|e| e.to_string())?;
                writeln!(
                    index,
                    "{pack_name}\t{i}\t{hash:016x}\t{rel}\t{}\t{}\tok\t",
                    blob.len(),
                    ext_for(blob)
                )
                .map_err(|e| e.to_string())?;
            }
            Err(e) => {
                writeln!(index, "{pack_name}\t{i}\t{hash:016x}\t\t0\t\tfailed\t{e}")
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok((blobs.len(), resolved))
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "bntx" || e == "arc") {
            out.push(path);
        }
    }
    Ok(())
}

/// BNTX component selector value -> output byte for one texel.
/// 0 = zero, 1 = one, 2..=5 = R, G, B, A of the decoded texel.
fn select(sel: u8, texel: [u8; 4]) -> u8 {
    match sel {
        0 => 0,
        1 => 255,
        2..=5 => texel[(sel - 2) as usize],
        _ => texel[0],
    }
}

/// Applies the component selector unless it is the identity (R, G, B, A).
fn apply_channels(data: &mut TextureData, channels: [u8; 4]) -> bool {
    if channels == [2, 3, 4, 5] {
        return false;
    }
    for px in data.rgba.as_chunks_mut::<4>().0 {
        let t = *px;
        *px = channels.map(|sel| select(sel, t));
    }
    true
}

fn save_png_up(data: &TextureData, path: &Path) -> std::io::Result<u64> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), data.width, data.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_filter(png::Filter::Up);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&data.rgba)?;
    writer.finish()?;
    Ok(fs::metadata(path)?.len())
}

/// Keeps file names portable: path separators and control characters only.
fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c == '/' || c == '\\' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect()
}

struct Counts {
    ok: usize,
    failed: usize,
    by_format: BTreeMap<String, (usize, usize)>,
    by_reason: BTreeMap<String, usize>,
    by_screen: BTreeMap<String, (usize, usize)>,
}

#[allow(clippy::too_many_arguments)]
fn export_texture(
    w: &mut impl Write,
    counts: &mut Counts,
    screen: &str,
    kind: &str,
    source_rel: &str,
    bytes: &[u8],
    tex: &BntxTexture,
    out_root: &Path,
    png_rel: &str,
) -> std::io::Result<()> {
    let fmt = tex.format.name().to_string();
    let channels = format!(
        "{},{},{},{}",
        tex.channels[0], tex.channels[1], tex.channels[2], tex.channels[3]
    );
    let result = tex.decode(bytes).and_then(|mut d| {
        let swizzled = apply_channels(&mut d, tex.channels);
        save_png_up(&d, &out_root.join(png_rel))
            .map(|size| (size, swizzled))
            .map_err(|e| format!("save_png: {e}"))
    });
    let entry = counts.by_format.entry(fmt.clone()).or_default();
    let sc = counts.by_screen.entry(screen.to_string()).or_default();
    let (status, reason, png) = match result {
        Ok((_, swizzled)) => {
            counts.ok += 1;
            entry.0 += 1;
            sc.0 += 1;
            let reason = if swizzled { "channels_applied" } else { "" };
            ("ok", reason.to_string(), png_rel.to_string())
        }
        Err(e) => {
            counts.failed += 1;
            entry.1 += 1;
            sc.1 += 1;
            let key = e.split(':').next().unwrap_or("error").to_string();
            *counts.by_reason.entry(key).or_default() += 1;
            ("failed", e.replace(['\t', '\n'], " "), String::new())
        }
    };
    writeln!(
        w,
        "{screen}\t{kind}\t{source_rel}\t{}\t{}\t{}\t{fmt}\t{channels}\t{}\t{}\t{png}\t{status}\t{reason}",
        tex.name, tex.width, tex.height, tex.mip_count, tex.array_count
    )
}

struct Job {
    path: PathBuf,
    screen: String,
    /// Sub-directories below the screen folder (`bin` dropped).
    sub: Vec<String>,
    file_name: String,
    source_rel: String,
}

fn appli_jobs(appli: &Path) -> Result<Vec<Job>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    collect(appli, &mut files)?;
    files.sort();
    let mut jobs = Vec::new();
    for path in files {
        let rel = path
            .strip_prefix(appli)?
            .to_string_lossy()
            .replace('\\', "/");
        let mut parts: Vec<String> = rel.split('/').map(str::to_string).collect();
        let screen = parts.remove(0);
        let file_name = parts.pop().unwrap_or_default();
        let sub = parts.into_iter().filter(|p| p != "bin").collect();
        jobs.push(Job {
            path,
            screen,
            sub,
            file_name,
            source_rel: format!("bin/appli/{rel}"),
        });
    }
    Ok(jobs)
}

fn pack_jobs(unpacked: &Path, pack: &str) -> Result<Vec<Job>, Box<dyn std::error::Error>> {
    let root = unpacked.join(pack);
    let mut files = Vec::new();
    collect(&root, &mut files)?;
    files.sort();
    let mut jobs = Vec::new();
    for path in files {
        let rel = path
            .strip_prefix(&root)?
            .to_string_lossy()
            .replace('\\', "/");
        let inner = rel.strip_prefix("bin/appli/").unwrap_or(&rel);
        let mut parts: Vec<String> = inner.split('/').map(str::to_string).collect();
        let file_name = parts.pop().unwrap_or_default();
        let sub = parts.into_iter().filter(|p| p != "bin").collect();
        jobs.push(Job {
            path,
            screen: format!("gfpak_{pack}"),
            sub,
            file_name,
            source_rel: format!("bin/archive/appli/{pack}.gfpak:{rel}"),
        });
    }
    Ok(jobs)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: ui_export <bin-root> <out-dir>");
        std::process::exit(2);
    }
    let bin_root = Path::new(&args[1]).canonicalize()?;
    let appli = bin_root.join("appli");
    let out_root = PathBuf::from(&args[2]);
    fs::create_dir_all(&out_root)?;

    // 1. Unpack the screen packs.
    let unpacked = out_root.join("unpacked");
    fs::create_dir_all(&unpacked)?;
    let mut packs = Vec::new();
    let pack_dir = bin_root.join("archive").join("appli");
    let mut pack_index = BufWriter::new(File::create(unpacked.join("index.tsv"))?);
    writeln!(
        pack_index,
        "pack\tentry\thash\tpath\tsize\tkind\tstatus\treason"
    )?;
    if pack_dir.is_dir() {
        let mut pack_files: Vec<PathBuf> = fs::read_dir(&pack_dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "gfpak"))
            .collect();
        pack_files.sort();
        for pack in pack_files {
            let name = pack.file_stem().unwrap().to_string_lossy().to_string();
            if LANG_SUFFIXES.iter().any(|s| name.ends_with(s)) {
                println!("pack {name}: skipped (language variant)");
                continue;
            }
            match unpack_gfpak(&pack, &unpacked, &mut pack_index) {
                Ok((n, resolved)) => {
                    println!("pack {name}: {n} entries, {resolved} names resolved");
                    packs.push(name);
                }
                Err(e) => println!("pack {name}: FAILED {e}"),
            }
        }
    }
    pack_index.flush()?;

    // 2. Export the textures of appli/ and of every unpacked pack.
    let mut jobs = appli_jobs(&appli)?;
    for pack in &packs {
        jobs.extend(pack_jobs(&unpacked, pack)?);
    }

    let mut w = BufWriter::new(File::create(out_root.join("index.tsv"))?);
    writeln!(
        w,
        "screen\tkind\tsource\ttexture\twidth\theight\tformat\tchannels\tmips\tlayers\tpng\tstatus\treason"
    )?;
    let mut counts = Counts {
        ok: 0,
        failed: 0,
        by_format: BTreeMap::new(),
        by_reason: BTreeMap::new(),
        by_screen: BTreeMap::new(),
    };
    let mut archives = (0usize, 0usize);

    let total = jobs.len();
    for (i, job) in jobs.iter().enumerate() {
        let Job {
            path,
            screen,
            sub,
            file_name,
            source_rel,
        } = job;
        let (screen, file_name, source_rel) =
            (screen.clone(), file_name.clone(), source_rel.clone());
        let rel = file_name.clone();
        let bytes = fs::read(path)?;

        if rel.ends_with(".bntx") {
            let stem = file_name.trim_end_matches(".bntx");
            let mut dir = sub.join("/");
            if !dir.is_empty() {
                dir.push('/');
            }
            match Bntx::parse(&bytes) {
                Ok(bntx) => {
                    for (t, tex) in bntx.textures.iter().enumerate() {
                        let name = if t == 0 {
                            stem.to_string()
                        } else {
                            format!("{stem}__{}", safe_name(&tex.name))
                        };
                        let png = format!("png/{screen}/{dir}{name}.png");
                        export_texture(
                            &mut w,
                            &mut counts,
                            &screen,
                            "standalone",
                            &source_rel,
                            &bytes,
                            tex,
                            &out_root,
                            &png,
                        )?;
                    }
                }
                Err(e) => {
                    counts.failed += 1;
                    *counts.by_reason.entry("parse".into()).or_default() += 1;
                    counts.by_screen.entry(screen.clone()).or_default().1 += 1;
                    writeln!(
                        w,
                        "{screen}\tstandalone\t{source_rel}\t\t0\t0\tunknown\t\t0\t0\t\tfailed\tparse: {e}"
                    )?;
                }
            }
        } else {
            let arc_stem = file_name.trim_end_matches(".arc");
            let sarc = match Sarc::parse(&bytes) {
                Ok(s) => s,
                Err(e) => {
                    archives.1 += 1;
                    writeln!(
                        w,
                        "{screen}\tarc\t{source_rel}\t\t0\t0\tunknown\t\t0\t0\t\tfailed\tsarc: {e}"
                    )?;
                    continue;
                }
            };
            archives.0 += 1;
            for entry in &sarc.entries {
                let name = sarc.name(entry);
                if !name.ends_with(".bntx") {
                    continue;
                }
                let inner_rel = format!("{source_rel}:{name}");
                let data = match sarc.data(&bytes, entry) {
                    Ok(d) => d,
                    Err(e) => {
                        counts.failed += 1;
                        *counts.by_reason.entry("sarc_entry".into()).or_default() += 1;
                        writeln!(
                            w,
                            "{screen}\tarc\t{inner_rel}\t\t0\t0\tunknown\t\t0\t0\t\tfailed\tsarc_entry: {e}"
                        )?;
                        continue;
                    }
                };
                match Bntx::parse(data) {
                    Ok(bntx) => {
                        for tex in &bntx.textures {
                            let png =
                                format!("png/{screen}/arc/{arc_stem}/{}.png", safe_name(&tex.name));
                            export_texture(
                                &mut w,
                                &mut counts,
                                &screen,
                                "arc",
                                &inner_rel,
                                data,
                                tex,
                                &out_root,
                                &png,
                            )?;
                        }
                    }
                    Err(e) => {
                        counts.failed += 1;
                        *counts.by_reason.entry("parse".into()).or_default() += 1;
                        counts.by_screen.entry(screen.clone()).or_default().1 += 1;
                        writeln!(
                            w,
                            "{screen}\tarc\t{inner_rel}\t\t0\t0\tunknown\t\t0\t0\t\tfailed\tparse: {e}"
                        )?;
                    }
                }
            }
        }
        if (i + 1) % 200 == 0 || i + 1 == total {
            println!(
                "progress {}/{} files: {} textures ok, {} failed",
                i + 1,
                total,
                counts.ok,
                counts.failed
            );
        }
    }
    w.flush()?;

    println!("archives parsed: {} ok, {} failed", archives.0, archives.1);
    println!("textures: {} ok, {} failed", counts.ok, counts.failed);
    for (fmt, (ok, failed)) in &counts.by_format {
        println!("  format {fmt:<24} ok {ok:>5} failed {failed:>4}");
    }
    for (reason, n) in &counts.by_reason {
        println!("  failure {reason:<20} {n}");
    }
    for (screen, (ok, failed)) in &counts.by_screen {
        println!("  screen {screen:<22} ok {ok:>5} failed {failed:>4}");
    }
    Ok(())
}
