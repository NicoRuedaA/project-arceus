//! Batch BNTX Texture to PNG Exporter
//!
//! Converts BNTX texture containers from the game RomFS to standard PNG files:
//! - Formats handled: BC7 (UNORM/sRGB), BC4, BC5 (with Z normal reconstruction),
//!   RGBA8, BC3, BC6 (float), R8, RG32F, RGBA32F.
//! - Bounded streaming: decodes one texture at a time, writes PNG to `work/cache/textures/`.
//! - Generates sheetty-compatible index `data/texture_index.tsv`.
//!
//! Usage:
//! `cargo run -p pla --example texture_batch_exporter -- <romfs-root> <cache-output-dir> <index-tsv-path>`

use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: texture_batch_exporter <romfs-root> <cache-output-dir> <index-tsv-path>");
        std::process::exit(1);
    }

    let romfs_root = Path::new(&args[1]);
    let cache_dir = Path::new(&args[2]);
    let index_tsv = Path::new(&args[3]);

    println!(
        "Skeleton: batch exporting BNTX from {:?} to {:?} (index at {:?})",
        romfs_root, cache_dir, index_tsv
    );
    Ok(())
}
