# gamedb Indexing (SQLite reference for the Rust/Bevy port)

Upstream: **https://github.com/smileybaal/gamedb** (MIT). A single native CLI that
indexes decompiled source into SQLite for search, call-graph, and module queries.
Read the upstream README/`--help` before running anything.

## Build / install

- Linux/macOS: `cargo build --release` → `target/release/gamedb`
- Windows (upstream example): `cargo build --release --target x86_64-pc-windows-msvc`
  → `target\x86_64-pc-windows-msvc\release\gamedb.exe`
- Zero crates: it binds the system SQLite (`libsqlite3` on Unix, `winsqlite3.dll`
  on Windows 10 1809+). Drop the binary on `PATH` or call it by path.
- Builds only where a Rust toolchain and a C linker exist.

## Commands

```
gamedb index    -r SRC [-v0] [--dry-run] [--force] [--rules FILE|derive]   build/refresh index
gamedb search   -r SRC QUERY [--limit N]             find functions by name substring
gamedb strings  -r SRC QUERY [--limit N]             find string literals
gamedb read     -r SRC NAME [--path SUBSTR] [--out F] [--force]   print one function body
gamedb stats    -r SRC                               counts per table
gamedb modules  -r SRC [--rules FILE|derive]         which subsystem each file belongs to
gamedb set-module -r SRC --module ID [--state S] [--verified|--unverified]
                    [--verified-by WHO] [--remaining TEXT]        track rewrite progress
gamedb graph    -r SRC NAME [--path SUBSTR] [--direction both|callers|callees]   call graph
gamedb sql      -r SRC --sql Q [--param V]...        raw SQLite escape hatch
gamedb selftest                                      built-in checks
```

Global flags: `-r/--root SRC` (default `.`), `--db PATH`, `--json`, `-q`,
`--verbose=N`, `--rules FILE|derive`, `-h`, `-V`.

Index location: **`<root>/.gamedb/index.sqlite`** (Windows: `<root>\.gamedb\index.sqlite`).
Delete `.gamedb/` to discard it.

## Schema (source of truth: `src/db.rs`)

| Table | Contents |
|---|---|
| `files` | path, mtime, size, module |
| `functions` | name, params, signature, line range, owning file |
| `symbols` | namespace / type / method / property / field declarations |
| `strings` | string literals, 4+ chars |
| `edges` | caller → callee, with line and hit count |
| `module_status` | per-module rewrite state and sign-off |

## Phase 1 — Index into SQLite

1. Inspect the repo's CLI usage first (README / `--help`); confirm the exact indexing
   commands, flags, and output paths **before** running anything.
2. Index **all** decompiled source files — do not stop at a subset.
3. Track which files/paths were indexed and flag any the tool skipped or errored on.
4. The schema must serve a port: source path, file type/module, original symbol names,
   function/struct/enum definitions, cross-references between symbols, and any
   dependency/call-graph info the CLI exposes.
5. Keep the raw file contents available in the DB (or a referenced blob/path) so the
   Rust rewrite and later checks can diff against them.
6. Make the step **idempotent / re-runnable** so it survives source changes
   (`index` is already incremental; `--force` re-parses, `--dry-run` reports).

## Phase 2 — Parity check

- **Define parity concretely first**: every indexed source artifact is accounted for,
  no missing or truncated records, and content round-trips back to the decompiled
  source (symbol counts plus a content hash/diff against the originals).
- Report anything missing, duplicated, malformed, or broken with the exact file paths
  and symbols involved.
- Summarize as: total artifacts indexed vs. expected, discrepancies found, and the
  gaps that must be resolved before the Rust port can rely on the database.
- Seed the baseline with `gamedb stats -r <decompiled>` and `gamedb selftest`.

## Behaviour notes (from the README)

- `index` is incremental (mtime + size); an unchanged tree writes nothing.
- The whole pass is one transaction; an interrupted run leaves the previous index intact.
- Unreadable files are **named**, not silently skipped (bad UTF-8 / UTF-16 without BOM).
- Search is a case-sensitive substring with SQL wildcards escaped.
- `read` needs the exact function name; `--path` disambiguates duplicates.
- Call edges resolve by **name only** — no type inference. Treat the graph as a map,
  not as proof.
- Exit codes: `0` success, `1` runtime error, `2` usage error.

## Irreversible changes

Ask before overwriting an existing database or deleting `.gamedb/`.
