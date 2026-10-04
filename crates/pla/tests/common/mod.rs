//! Test fixture loading.
//!
//! Game fixtures are **not** part of the repository (they are copyrighted game
//! content). Each collaborator regenerates them from their own copy of the game
//! (see `REPRODUCE.md`) into a directory of their choice and points
//! `PLA_FIXTURES` at it, or copies them into `crates/pla/tests/fixtures/`.
//!
//! Tests whose fixtures are missing print a `[skip]` line and pass, so a clean
//! clone compiles and stays green.

use std::path::PathBuf;

/// Directory holding the game fixtures.
pub fn fixtures_dir() -> PathBuf {
    std::env::var("PLA_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("tests/fixtures"))
}

/// Load a binary fixture, or `None` when it is absent (skip the test).
pub fn fixture(name: &str) -> Option<Vec<u8>> {
    let path = fixtures_dir().join(name);
    match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(_) => {
            eprintln!(
                "[skip] fixture {} not found — set PLA_FIXTURES to your extraction",
                path.display()
            );
            None
        }
    }
}

/// Load a text fixture (expected-value files live next to the binaries).
#[allow(dead_code)] // used by some test targets only
pub fn fixture_text(name: &str) -> Option<String> {
    let path = fixtures_dir().join(name);
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(_) => {
            eprintln!(
                "[skip] fixture {} not found — set PLA_FIXTURES to your extraction",
                path.display()
            );
            None
        }
    }
}
