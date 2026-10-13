//! Headless census of the game's visual effects (no window).
//!
//! * counts every file under `bin/effect` by folder and extension, with sizes and
//!   leading magic bytes;
//! * decodes every `.ptcl` with `pla::assets::ptcl` (emitters, textures, shapes,
//!   blend modes) and every battle sequence with `pla::assets::effect_sequence`
//!   (which particles each sequence plays);
//! * with `--packs`, also scans every `.gfpak` under `bin` for embedded `VFXB`
//!   entries (decodes all pack entries; slower);
//! * writes `data/effect_index.tsv` (one row per `.ptcl`) when `--write` is given.
//!
//! Usage (from the repo root): `effect_census [--packs] [--write]`
//! `PLA_BIN_ROOT` overrides the game data root.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pla::assets::effect_sequence::EffectSequence;
use pla::assets::gfpak::{fnv1a64, GfPak};
use pla::assets::ptcl::{BlendType, PtclFile};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn ext(p: &Path) -> String {
    p.extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn magic(p: &Path) -> String {
    use std::io::Read;
    let mut buf = [0u8; 4];
    let n = std::fs::File::open(p)
        .and_then(|mut f| f.read(&mut buf))
        .unwrap_or(0);
    buf[..n]
        .iter()
        .map(|&c| {
            if c.is_ascii_graphic() {
                (c as char).to_string()
            } else {
                format!("\\x{c:02x}")
            }
        })
        .collect()
}

#[derive(Default)]
struct Totals {
    files: usize,
    bytes: u64,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let packs = args.iter().any(|a| a == "--packs");
    let write = args.iter().any(|a| a == "--write");
    let root = std::env::var_os("PLA_BIN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into());
    let effect = root.join("effect");
    if !effect.is_dir() {
        println!("[skip] no effect folder under {}", root.display());
        return;
    }

    // 1. Files by folder and extension.
    let mut files = Vec::new();
    walk(&effect, &mut files);
    let mut by_group: BTreeMap<(String, String), Totals> = BTreeMap::new();
    let mut magics: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut total = Totals::default();
    for f in &files {
        let rel = f.strip_prefix(&effect).unwrap();
        let mut comps = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned());
        let top = comps.next().unwrap_or_default();
        let second = comps
            .next()
            .filter(|_| rel.components().count() > 2)
            .unwrap_or_default();
        let group = if second.is_empty() {
            top
        } else {
            format!("{top}/{second}")
        };
        let size = f.metadata().map(|m| m.len()).unwrap_or(0);
        let e = by_group.entry((group, ext(f))).or_default();
        e.files += 1;
        e.bytes += size;
        total.files += 1;
        total.bytes += size;
        *magics.entry((ext(f), magic(f))).or_default() += 1;
    }
    println!(
        "== bin/effect: {} files, {:.1} MiB",
        total.files,
        total.bytes as f64 / 1048576.0
    );
    for ((g, e), t) in &by_group {
        println!(
            "  {g:<24} .{e:<6} {:>5} files {:>9.1} MiB",
            t.files,
            t.bytes as f64 / 1048576.0
        );
    }
    println!("== leading magic by extension");
    for ((e, m), n) in &magics {
        println!("  .{e:<6} {m:<20} {n}");
    }

    // 2. Sequences: which particle files each one plays.
    let particle_dir = effect.join("battle/particle");
    let mut referenced: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let (mut seq_ok, mut seq_err, mut plays, mut kinds) = (0, 0, 0, BTreeMap::<u8, usize>::new());
    for f in files.iter().filter(|f| ext(f) == "wbin") {
        let name = f.file_stem().unwrap().to_string_lossy().into_owned();
        match std::fs::read(f)
            .map_err(|e| e.to_string())
            .and_then(|b| EffectSequence::parse(&b))
        {
            Ok(s) => {
                seq_ok += 1;
                for c in s.tracks.iter().flatten() {
                    *kinds.entry(c.kind).or_default() += 1;
                }
                for c in s.particles() {
                    plays += 1;
                    referenced
                        .entry(c.particle.clone().unwrap())
                        .or_default()
                        .insert(name.clone());
                }
            }
            Err(e) => {
                seq_err += 1;
                println!("  sequence {name}: {e}");
            }
        }
    }
    let missing: Vec<_> = referenced
        .keys()
        .filter(|p| !particle_dir.join(p).exists())
        .collect();
    let move_seqs = files
        .iter()
        .filter(|f| ext(f) == "wbin")
        .filter(|f| f.file_stem().unwrap().to_string_lossy().starts_with("ew"))
        .count();
    println!(
        "== sequences: {seq_ok} parsed, {seq_err} failed, {move_seqs} move (ew*) files, {} command types, {plays} particle plays, {} distinct particle paths, missing: {missing:?}",
        kinds.len(),
        referenced.len()
    );

    // 3. Particle files.
    let mut rows = Vec::new();
    let (mut ok, mut err) = (0usize, 0usize);
    let (mut emitters, mut children, mut textures, mut meshes, mut stripes) = (0, 0, 0, 0, 0);
    let mut shapes: BTreeMap<&str, usize> = BTreeMap::new();
    let mut blends: BTreeMap<String, usize> = BTreeMap::new();
    let mut attrs: BTreeMap<String, usize> = BTreeMap::new();
    let mut bntx_ok = 0usize;
    for f in files.iter().filter(|f| ext(f) == "ptcl") {
        let rel = f
            .strip_prefix(&effect)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let bytes = std::fs::read(f).unwrap();
        let p = match PtclFile::parse(&bytes) {
            Ok(p) => p,
            Err(e) => {
                err += 1;
                println!("  ptcl {rel}: {e}");
                continue;
            }
        };
        ok += 1;
        if matches!(p.bntx(&bytes), Some(Ok(_))) {
            bntx_ok += 1;
        }
        emitters += p.emitters.len();
        children += p.emitters.iter().filter(|e| e.parent.is_some()).count();
        textures += p.texture_names.len();
        let mut file_shapes: BTreeMap<&str, usize> = BTreeMap::new();
        for e in &p.emitters {
            *shapes.entry(e.shape()).or_default() += 1;
            *file_shapes.entry(e.shape()).or_default() += 1;
            *blends.entry(format!("{:?}", e.blend)).or_default() += 1;
            for a in &e.attributes {
                *attrs.entry(a.clone()).or_default() += 1;
            }
            meshes += usize::from(e.primitive_id.is_some());
            stripes += usize::from(e.is_stripe());
        }
        let seq_key = rel.strip_prefix("battle/particle/").unwrap_or("");
        let used_by = referenced.get(seq_key).map(|s| s.len()).unwrap_or(0);
        rows.push(format!(
            "{rel}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            bytes.len(),
            p.emitter_set_name,
            p.emitters.len(),
            p.emitters.iter().filter(|e| e.parent.is_some()).count(),
            p.texture_names.len(),
            p.emitters
                .iter()
                .filter(|e| e.blend == BlendType::Add)
                .count(),
            p.emitters.iter().map(|e| e.life_frames).max().unwrap_or(0),
            p.emitters
                .iter()
                .filter(|e| e.primitive_id.is_some())
                .count(),
            p.emitters.iter().filter(|e| e.is_stripe()).count(),
            file_shapes
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(","),
            used_by
        ));
    }
    println!(
        "== particles: {ok} parsed, {err} failed, {bntx_ok} with a parsable embedded BNTX; {emitters} emitters ({children} child), {textures} texture names, {meshes} mesh emitters, {stripes} stripe emitters"
    );
    println!("  shapes: {shapes:?}");
    println!("  blend:  {blends:?}");
    println!("  attributes: {attrs:?}");
    let battle_particles = files
        .iter()
        .filter(|f| ext(f) == "ptcl" && f.starts_with(&particle_dir))
        .count();
    println!(
        "  battle particle files referenced by a sequence: {} of {battle_particles}",
        referenced
            .keys()
            .filter(|p| particle_dir.join(p).exists())
            .count()
    );

    // 4. Optional: VFXB entries inside packs.
    if packs {
        let mut pack_files = Vec::new();
        walk(&root, &mut pack_files);
        let (mut n_packs, mut n_entries, mut n_vfxb, mut n_vfxb_ok) = (0, 0, 0, 0);
        let mut effect_dirs: BTreeSet<String> = files
            .iter()
            .filter_map(|f| f.parent()?.strip_prefix(&effect).ok())
            .map(|d| d.to_string_lossy().into_owned())
            .collect();
        // Extra folders named by `bin/effect/...` strings inside the main executable
        // (read only for names; `PLA_MAIN_ELF` overrides the default location).
        let elf = std::env::var_os("PLA_MAIN_ELF")
            .map(PathBuf::from)
            .unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/main.elf".into());
        if let Ok(bytes) = std::fs::read(&elf) {
            let needle = b"bin/effect/";
            let mut i = 0;
            while let Some(pos) = bytes[i..].windows(needle.len()).position(|w| w == needle) {
                let start = i + pos;
                let end = bytes[start..]
                    .iter()
                    .position(|&c| c == 0 || !c.is_ascii_graphic())
                    .map(|n| start + n)
                    .unwrap_or(bytes.len());
                let text = String::from_utf8_lossy(&bytes[start + needle.len()..end]).into_owned();
                if let Some((dir, _)) = text.rsplit_once('/') {
                    if !dir.contains('%') {
                        effect_dirs.insert(dir.to_string());
                    }
                }
                i = end.max(start + 1);
            }
        }
        for pf in pack_files.iter().filter(|p| ext(p) == "gfpak") {
            let Ok(bytes) = std::fs::read(pf) else {
                continue;
            };
            let Ok(pak) = GfPak::parse(&bytes) else {
                continue;
            };
            n_packs += 1;
            let mut here = 0;
            for (i, e) in pak.entries.iter().enumerate() {
                n_entries += 1;
                let Ok(data) = pak.decoded_data(&bytes, e) else {
                    continue;
                };
                if data.starts_with(b"VFXB") {
                    here += 1;
                    n_vfxb += 1;
                    if let Ok(p) = PtclFile::parse(&data) {
                        n_vfxb_ok += 1;
                        // Try to name the entry: FNV-1a 64 of candidate full paths built
                        // from the emitter-set name and every real effect folder.
                        let hash = pak.entry_path_hashes.get(i).copied();
                        let path = effect_dirs
                            .iter()
                            .map(|d| format!("bin/effect/{d}/{}.ptcl", p.emitter_set_name))
                            .find(|c| Some(fnv1a64(c.as_bytes())) == hash);
                        println!(
                            "  pack {}: VFXB set '{}' with {} emitters, path {}",
                            pf.strip_prefix(&root).unwrap().display(),
                            p.emitter_set_name,
                            p.emitters.len(),
                            path.unwrap_or_else(|| format!(
                                "unresolved (hash {:#018x})",
                                hash.unwrap_or(0)
                            ))
                        );
                    }
                }
            }
            if here > 0 {
                println!(
                    "  pack {} holds {here} VFXB",
                    pf.strip_prefix(&root).unwrap().display()
                );
            }
        }
        println!("== packs: {n_packs} gfpak, {n_entries} entries, {n_vfxb} VFXB entries ({n_vfxb_ok} parsed by ptcl.rs)");
    }

    if write {
        let header = [
            "# sheet: data/effect_index",
            "# version: 1",
            "# generator: crates/pla/examples/effect_census.rs --write",
            "# target: Pokémon Legends: Arceus update v262144 particle files (bin/effect/**/*.ptcl, VFXB 0x16)",
            "# note: counts come from pla::assets::ptcl. add_emitters uses the LIKELY blend byte; max_life_frames the LIKELY life field.",
            "# note: shapes: billboard/streak/polygon from the LIKELY display byte, mesh = primitive id set, ribbon = EP03 attribute (LIKELY).",
            "# note: used_by_sequences counts battle sequences that play the file (0 for field/demo/poke files and unused ones).",
            "path:string*\tbytes:u32\temitter_set:string\temitters:u16\tchild_emitters:u16\ttextures:u16\tadd_emitters:u16\tmax_life_frames:i32\tmesh_emitters:u16\tstripe_emitters:u16\tshapes:string\tused_by_sequences:u16",
        ];
        let mut text = header.join("\n");
        text.push('\n');
        for r in &rows {
            text.push_str(r);
            text.push('\n');
        }
        std::fs::write("data/effect_index.tsv", text).expect("write data/effect_index.tsv");
        println!("wrote data/effect_index.tsv ({} rows)", rows.len());
    }
}
