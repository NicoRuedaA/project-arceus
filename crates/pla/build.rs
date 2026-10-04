//! Thin build shim: preflight the sheet book, then emit one Rust module per
//! sheet into `$OUT_DIR/sheets` (The Spreadsheet Method, section 5.6).

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
    }
    out
}

fn main() {
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let sheets_dir = manifest_dir.join("../../sheets");
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // Per-sheet change tracking: only a change to a sheet rebuilds this crate.
    for entry in walk(&sheets_dir) {
        println!("cargo:rerun-if-changed={}", entry.display());
    }
    println!("cargo:rerun-if-changed=build.rs");

    sheetty::run(sheetty::Config {
        sheets_dir,
        out_dir,
        fail_on_preflight_error: true, // D5: not configurable at runtime
        emit_docs: false,
    })
    .unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
}
