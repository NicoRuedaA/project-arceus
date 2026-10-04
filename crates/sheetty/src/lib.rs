//! sheetty: the Spreadsheet Method machinery.
//!
//! - `parse`: canonical TSV sheets (manifest block, header row, data rows).
//! - `check`: preflight layers L0-L3 (structural, type/domain, references,
//!   coverage).
//! - `emit`: one generated Rust module per sheet plus a registry, gated on
//!   content hashes so no-op writes do not dirty the build.

pub mod check;
pub mod emit;
pub mod parse;

pub use check::{check, Finding, Severity};
pub use emit::{run, Config};
pub use parse::{load_dir, Column, Row, Sheet};
