//! Dump a BNTX texture's mip 0 as raw RGBA8 for visual inspection.
//!
//! Usage: cargo run -p pla --example dump_bntx -- <file.bntx> <out.rgba> [texture-index]

use std::process::ExitCode;

use pla::assets::bntx::Bntx;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: dump_bntx <file.bntx> <out.rgba> [texture-index]");
        return ExitCode::from(2);
    }
    let bytes = match std::fs::read(&args[1]) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("read {}: {e}", args[1]);
            return ExitCode::from(1);
        }
    };
    let bntx = match Bntx::parse(&bytes) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("parse: {e}");
            return ExitCode::from(1);
        }
    };
    let index: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let Some(t) = bntx.textures.get(index) else {
        eprintln!(
            "texture {index} out of range ({} textures)",
            bntx.texture_count
        );
        return ExitCode::from(1);
    };
    println!(
        "name={} {}x{} depth={} mips={} array={} fmt={} ({:#x}) image_size={} tile={}",
        t.name,
        t.width,
        t.height,
        t.depth,
        t.mip_count,
        t.array_count,
        t.format.name(),
        t.format.code(),
        t.image_size,
        t.tile_mode
    );
    let data = match t.decode(&bytes) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("decode: {e}");
            return ExitCode::from(1);
        }
    };
    if let Err(e) = std::fs::write(&args[2], &data.rgba) {
        eprintln!("write {}: {e}", args[2]);
        return ExitCode::from(1);
    }
    println!(
        "wrote {} ({}x{}, {} bytes)",
        args[2],
        data.width,
        data.height,
        data.rgba.len()
    );
    ExitCode::SUCCESS
}
