//! Batch BNTX Texture to PNG Exporter
//!
//! Converts BNTX texture containers from the game RomFS to standard PNG files:
//! - Formats handled: BC7 (UNORM/sRGB), BC4, BC5 (raw decoded R/G channels),
//!   RGBA8, BC3, BC6 (float), R8, RG32F, RGBA32F.
//! - Bounded streaming: decodes one texture at a time, writes PNG under the supplied cache root.
//! - Generates sheetty-compatible index `data/texture_index.tsv`.
//!
//! Usage:
//! `cargo run -p pla --example texture_batch_exporter -- <romfs-root> <cache-output-dir> <index-tsv-path>`

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use pla::assets::bntx::Bntx;

fn collect_bntx_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_bntx_files(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "bntx") {
            out.push(path);
        }
    }
    Ok(())
}

fn determine_category(rel_path: &str) -> &'static str {
    let p = rel_path.strip_prefix("bin/").unwrap_or(rel_path);
    if p.starts_with("chara/") {
        "chara"
    } else if p.starts_with("field/") {
        "field"
    } else if p.starts_with("demo/") {
        "demo"
    } else if p.starts_with("appli/") {
        "appli"
    } else if p.starts_with("system_resource") || rel_path.starts_with("system_resource") {
        "system"
    } else {
        "other"
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: texture_batch_exporter <romfs-root> <cache-output-dir> <index-tsv-path>");
        std::process::exit(1);
    }

    let romfs_root = Path::new(&args[1]).canonicalize()?;
    let cache_dir = Path::new(&args[2]);
    let index_tsv_path = Path::new(&args[3]);

    if let Some(parent) = index_tsv_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir_all(cache_dir)?;

    println!("Scanning for BNTX files in {}...", romfs_root.display());
    let mut bntx_files = Vec::new();
    collect_bntx_files(&romfs_root, &mut bntx_files)?;
    bntx_files.sort();
    let total_files = bntx_files.len();
    println!("Found {} BNTX files to process.", total_files);

    let tsv_file = File::create(index_tsv_path)?;
    let mut writer = BufWriter::new(tsv_file);

    // Sheetty header lines
    writeln!(writer, "# sheet: data/texture_index")?;
    writeln!(writer, "# version: 1")?;
    writeln!(writer, "# generator: crates/pla/examples/texture_batch_exporter.rs")?;
    writeln!(
        writer,
        "# target: Pokémon Legends: Arceus update v262144 BNTX texture export index"
    )?;
    writeln!(writer, "# requires: bin/**/*.bntx")?;
    writeln!(
        writer,
        "source_rel_path:string*\toutput_png_rel:string\tcategory:string\ttexture_name:string\tformat_code:u32\tformat_name:string\twidth:u32\theight:u32\tmip_count:u16\tbntx_size_bytes:u32\tpng_size_bytes:u32?\tstatus:string\treason:string?"
    )?;

    let mut successful_exports = 0usize;
    let mut failed_exports = 0usize;
    let mut format_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut category_counts = std::collections::BTreeMap::<&'static str, (usize, usize)>::new();

    let max_file_size = 512 * 1024 * 1024; // 512 MiB limit per file

    for (idx, bntx_path) in bntx_files.iter().enumerate() {
        let rel_path = match bntx_path.strip_prefix(&romfs_root) {
            Ok(p) => p.to_string_lossy().replace('\\', "/"),
            Err(_) => bntx_path.to_string_lossy().replace('\\', "/"),
        };
        let category = determine_category(&rel_path);

        let file_size = match fs::metadata(bntx_path) {
            Ok(m) => m.len(),
            Err(e) => {
                failed_exports += 1;
                writeln!(
                    writer,
                    "{}\t\t{}\t\t0\tunknown\t0\t0\t0\t0\t\tfailed\t{}",
                    rel_path, category, e
                )?;
                continue;
            }
        };

        if file_size > max_file_size {
            failed_exports += 1;
            writeln!(
                writer,
                "{}\t\t{}\t\t0\tunknown\t0\t0\t0\t{}\t\tfailed\tfile_over_512mib",
                rel_path, category, file_size
            )?;
            continue;
        }

        let bytes = match fs::read(bntx_path) {
            Ok(b) => b,
            Err(e) => {
                failed_exports += 1;
                writeln!(
                    writer,
                    "{}\t\t{}\t\t0\tunknown\t0\t0\t0\t{}\t\tfailed\t{}",
                    rel_path, category, file_size, e
                )?;
                continue;
            }
        };

        let bntx = match Bntx::parse(&bytes) {
            Ok(b) => b,
            Err(e) => {
                failed_exports += 1;
                writeln!(
                    writer,
                    "{}\t\t{}\t\t0\tunknown\t0\t0\t0\t{}\t\tfailed\tparse: {}",
                    rel_path, category, file_size, e
                )?;
                continue;
            }
        };

        let Some(texture) = bntx.textures.first() else {
            failed_exports += 1;
            writeln!(
                writer,
                "{}\t\t{}\t\t0\tunknown\t0\t0\t0\t{}\t\tfailed\tno_texture_in_container",
                rel_path, category, file_size
            )?;
            continue;
        };

        let format_name = texture.format.name();
        *format_counts.entry(format_name.to_string()).or_default() += 1;

        // Output PNG relative path
        let png_rel_path = format!(
            "work/cache/png/{}.png",
            rel_path.strip_suffix(".bntx").unwrap_or(&rel_path)
        );
        let out_png_path = cache_dir.join(format!(
            "{}.png",
            rel_path.strip_suffix(".bntx").unwrap_or(&rel_path)
        ));

        match texture.decode(&bytes) {
            Ok(decoded) => {
                match decoded.save_png(&out_png_path) {
                    Ok(()) => {
                        let png_size = fs::metadata(&out_png_path).map(|m| m.len()).unwrap_or(0);
                        successful_exports += 1;
                        let cat_entry = category_counts.entry(category).or_insert((0, 0));
                        cat_entry.0 += 1;
                        let reason = if format_name.starts_with("BC5") {
                            "BC5 exported as decoded R/G with B=0; no Z reconstruction"
                        } else {
                            ""
                        };

                        writeln!(
                            writer,
                            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tok\t{}",
                            rel_path,
                            png_rel_path,
                            category,
                            texture.name,
                            texture.format.code(),
                            format_name,
                            texture.width,
                            texture.height,
                            texture.mip_count,
                            file_size,
                            png_size,
                            reason
                        )?;
                    }
                    Err(e) => {
                        failed_exports += 1;
                        let cat_entry = category_counts.entry(category).or_insert((0, 0));
                        cat_entry.1 += 1;

                        writeln!(
                            writer,
                            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\tfailed\tsave_png: {}",
                            rel_path,
                            png_rel_path,
                            category,
                            texture.name,
                            texture.format.code(),
                            format_name,
                            texture.width,
                            texture.height,
                            texture.mip_count,
                            file_size,
                            e
                        )?;
                    }
                }
            }
            Err(e) => {
                failed_exports += 1;
                let cat_entry = category_counts.entry(category).or_insert((0, 0));
                cat_entry.1 += 1;

                writeln!(
                    writer,
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\tfailed\tdecode: {}",
                    rel_path,
                    png_rel_path,
                    category,
                    texture.name,
                    texture.format.code(),
                    format_name,
                    texture.width,
                    texture.height,
                    texture.mip_count,
                    file_size,
                    e
                )?;
            }
        }

        if (idx + 1) % 500 == 0 || idx + 1 == total_files {
            println!(
                "Progress: {}/{} processed ({:.1}%, {} ok, {} failed)",
                idx + 1,
                total_files,
                (idx + 1) as f32 / total_files as f32 * 100.0,
                successful_exports,
                failed_exports
            );
        }
    }

    writer.flush()?;

    let elapsed = start_time.elapsed();
    println!("============================================================");
    println!("Texture Batch Export Complete in {:.2?}", elapsed);
    println!(
        "Total: {} | Success: {} ({:.1}%) | Failed: {} ({:.1}%)",
        total_files,
        successful_exports,
        successful_exports as f32 / total_files as f32 * 100.0,
        failed_exports,
        failed_exports as f32 / total_files as f32 * 100.0,
    );
    println!("Breakdown by category (ok / failed):");
    for (cat, (ok, fail)) in &category_counts {
        println!("  {:<10}: {} ok, {} failed", cat, ok, fail);
    }
    println!("Breakdown by texture format:");
    for (fmt, count) in &format_counts {
        println!("  {:<24}: {}", fmt, count);
    }
    println!("Index written to: {}", index_tsv_path.display());
    println!("============================================================");

    Ok(())
}
