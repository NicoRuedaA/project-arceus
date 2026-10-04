#!/usr/bin/env bash
# Local CI — the gate every collaborator runs before pushing.
#
#   .tools/ci.sh           fmt + clippy + sheet preflight + build + tests
#   .tools/ci.sh --full    also builds gamedb and runs its selftest
#
# Game content is not required: fixture-based tests print [skip] and pass on a
# clean clone (see REPRODUCE.md for how each collaborator regenerates them).
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

step() { printf '\n== %s ==\n' "$1"; }

step "rustfmt"
cargo fmt --check

step "clippy (warnings are errors)"
cargo clippy --workspace --all-targets -- -D warnings

step "sheet book preflight"
cargo run --quiet -p sheetty-cli -- check sheets

step "workspace build (all targets)"
cargo build --workspace --all-targets

step "workspace tests"
cargo test --workspace

if [[ "${1:-}" == "--full" ]]; then
  if [[ -f gamedb/Cargo.toml ]]; then
    step "gamedb build (submodule)"
    cargo build --manifest-path gamedb/Cargo.toml --release
    step "gamedb selftest"
    ./gamedb/target/release/gamedb selftest
  else
    printf '\n[skip] gamedb submodule not initialized (git submodule update --init)\n'
  fi
fi

step "ci ok"
