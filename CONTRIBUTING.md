# Contributing

## Hard rules

1. **Never commit game content.** No NSZ/NSP/NCA, no extracted assets, no
   decompiled C, no decompiled Lua, no keys. All of it is regenerable (see
   `REPRODUCE.md`) and all of it is copyrighted.
2. **Never commit keys.** `ProdKeys*/` and `*.keys` are gitignored; each
   collaborator uses their own.
3. **Decompiled output is evidence, never source.** Read it to write a sheet
   row; never paste it into Rust and patch it (method §8.6).
4. **`sheets/**` is the source of truth.** Generated Rust is a projection;
   never hand-edit generated modules.

## Repository layout notes

- `gamedb/` is a **git submodule** (MIT, upstream `smileybaal/gamedb`). Clone with
  `--recurse-submodules`; never edit files inside it — upstream changes go there.

## Adding a host subsystem (the parallel extension point)

The Lua host is **frozen** in `crates/pla/src/host/mod.rs`; contributions go in
`crates/pla/src/host/subsystems/`:

1. Create `subsystems/<name>.rs` with
   `pub fn install(lua: &Lua, save: &SaveHandle) -> Result<()>`. Build the real
   tables and call `crate::host::stub_fallback(lua, &table, "Global")` for the
   methods that are not ported yet.
2. Add one line to `subsystems::SUBSYSTEMS`.

`subsystems/save.rs` is the reference implementation. Nothing else changes, so
several subsystems can be developed in parallel without touching shared code.

## Workflow

- Run the local gate before pushing:

  ```bash
  .tools/ci.sh          # fmt + clippy + sheet preflight + build + tests
  .tools/ci.sh --full   # also builds gamedb and runs its selftest
  ```

- Track substantial work in `odd/tasks/<feature>.md` (one document per feature)
  and keep `sheets/02-plan.tsv` / `sheets/03-impl.tsv` in sync.
- Add or change a sheet: update `sheets/01-schema.tsv` first, then the data,
  then run the preflight:

  ```bash
  cargo run -p sheetty-cli -- check sheets
  ```

- Emitters/parsers: keep the Rust tests green (`cargo test`) and prefer parity
  tests against the Python reference tools.
- Fixture-based tests must **skip** when the fixture is absent (see
  `crates/pla/tests/common/mod.rs`), so a clean clone stays green.
- `.tools/ci.sh` is the contract: `cargo fmt --check`, clippy with
  `-D warnings`, the sheet preflight, the workspace build and the tests must
  all pass.

## Commits

Conventional commits (`feat:`, `fix:`, `docs:`, `chore:`), one reviewable work
unit each: behavior + tests + docs together. No AI attribution in commit
messages.
