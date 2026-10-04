//! sheetty CLI: `sheetty check <sheets-dir>`.
//!
//! Runs the L0-L3 preflight and reports findings. Exit code 0 when there are
//! no errors, 1 on errors, 2 on usage errors.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == "check" {
        let dir = PathBuf::from(&args[2]);
        let sheets = match sheetty::load_dir(&dir) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("sheetty: {e}");
                return ExitCode::from(1);
            }
        };
        let findings = sheetty::check(&sheets);
        let rows: usize = sheets.iter().map(|s| s.rows.len()).sum();
        println!(
            "PREFLIGHT {}  {} sheets, {} rows",
            dir.display(),
            sheets.len(),
            rows
        );
        let mut errors = 0usize;
        let mut warnings = 0usize;
        for f in &findings {
            match f.severity {
                sheetty::Severity::Error => errors += 1,
                sheetty::Severity::Warning => warnings += 1,
            }
            println!("  {:<22} {:<28} {}", f.code, f.location, f.message);
        }
        println!(
            "{}  {} errors, {} warnings",
            if errors == 0 { "OK" } else { "FAILED" },
            errors,
            warnings
        );
        return if errors == 0 {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        };
    }
    eprintln!("usage: sheetty check <sheets-dir>");
    ExitCode::from(2)
}
