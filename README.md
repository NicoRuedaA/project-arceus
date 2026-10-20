# Pokémon Legends: Arceus — Rust/Bevy port and reverse-engineering workspace

**English** · [Español](README.es.md)

This project is an attempt to rebuild *Pokémon Legends: Arceus* (Nintendo Switch)
as a new game in Rust, using the Bevy engine. To get there, it studies the
original program step by step (reverse engineering) and writes down what it learns
in a versioned book of tables (the "sheet book", following **The Spreadsheet
Method**). The port is then driven from those tables.

The target is the game as it really runs: the **base game (v0) plus the update
v1.1.1 (v262144) applied on top**. The update is only a patch and cannot run
alone, so the base is always required.

> **This repository contains no game content.** No ROMs/NSZ, no extracted
> assets, no decompiled code, no keys. The derived tables and reports it tracks
> contain game-derived ids, numbers and some name strings (audited in
> [`docs/legal-audit.md`](docs/legal-audit.md)). It ships the *method*, the
> *tooling* and the *derived knowledge* (sheets). Every collaborator supplies
> their own copy of the game and their own `prod.keys`, and regenerates the heavy
> artifacts locally — see [`REPRODUCE.md`](REPRODUCE.md).

## Where the project stands

The reading of the game's program is far along; the playable port is still early.
Figures refer to the update v262144 `main` program (153,476 located functions).
Last updated: 2026-10-17. Full detail: [`docs/PROGRESS.md`](docs/PROGRESS.md).

<!-- progress-bars:start -->
```text
 1 Extraction                       ████████████████████    100 %   enumerated files and members
 2 Inventory of main                ███████████████████░   98.4 %   executable bytes inside functions
 3 Pseudocode export                ████████████████████    100 %   of the 153,476 located functions
 4 gameDB index                     ████████████████████    100 %   of the exported C files
 5 Data (RomFS)                     ████████████████████    100 %   extracted and overlaid
     · interpretation               ░░░░░░░░░░░░░░░░░░░░    3.0 %   14/466 modified files compared inside
 6 Analysis: read by analysts       ███████████████░░░░░   76.4 %   117,228 of 153,476 functions
     · classified by program        █░░░░░░░░░░░░░░░░░░░    8.5 %   13,055: tiny functions; says what they do, not what they are for
     · copied from identical        ██░░░░░░░░░░░░░░░░░░   14.6 %   22,392: exact copies of an audited function
     · import stubs                 ░░░░░░░░░░░░░░░░░░░░    0.5 %   801: identified from link data
 7 Implementation (port)            ░░░░░░░░░░░░░░░░░░░░   <0.1 %   38 of 153,476 with a partial implementation
 8 Behaviour verification           ░░░░░░░░░░░░░░░░░░░░      0 %   0 of 153,476
 9 Binary matching                  ░░░░░░░░░░░░░░░░░░░░      0 %   0 of 153,476
10 Playable port                    █████████░░░░░░░░░░░    ~45 %   average of qualitative estimates
```

*Bars show what is actually quantified in each phase; several phases have an unknown full denominator (see the Completion plan in docs/PROGRESS.md). Phase 2 measures executable bytes covered by functions, not a function count; in phase 6 only the first line is functions read and understood; the other three are counted separately.*
<!-- progress-bars:end -->

## Install and play

Bring your own dumps of the base game (`pk1.nsz`), the update v1.1.1 (`pk2.nsz`) and your
console's `prod.keys`, then:

```bash
./install.sh --base /path/to/pk1.nsz --update /path/to/pk2.nsz --keys /path/to/prod.keys
./play.sh --area 1
```

`./install.sh --check` shows every step without changing anything. Requirements, disk
space, options and troubleshooting: [`docs/INSTALL.md`](docs/INSTALL.md). A play-testing
checklist for non-programmers (in Spanish): [`docs/GUIA-DE-PRUEBAS.md`](docs/GUIA-DE-PRUEBAS.md).
Developer aids inside the viewer (F1/F2/F3/F6/F7): [`docs/DEBUG-VIEW.md`](docs/DEBUG-VIEW.md).

## Build and test

```bash
git clone --recurse-submodules <repo-url> && cd <repo>   # or: git submodule update --init

cargo run -p sheetty-cli -- check sheets     # check the sheet book (0 errors expected)
cargo build -p pla                           # the port crate
cargo test                                   # game fixtures are optional: missing ones skip
```

Some tests use real game files. Point `PLA_FIXTURES` at your own extraction (see
[`REPRODUCE.md`](REPRODUCE.md)); without them the tests still pass and print `[skip]`.

## What is here

| Path | What it is |
|---|---|
| `sheets/` | The sheet book (source of truth): rules, schema, plan, decisions, domain and evidence sheets. |
| `crates/pla` | The port: file readers, Lua script host, save system, game scaffold. |
| `crates/sheetty`, `crates/sheetty-cli` | The sheet-book reader/checker and its command line. |
| `gamedb/` | Git submodule: the decompiled-code indexer ([smileybaal/gamedb](https://github.com/smileybaal/gamedb), MIT). |
| `.tools/` | The extraction pipeline (Python) and the Ghidra export scripts. |
| `odd/` | Task documents and the execution plan. |
| `reports/`, `docs/` | Evidence reports and project documentation. |

## More documentation

- [`docs/PROGRESS.md`](docs/PROGRESS.md) — detailed status, the phase-by-phase completion plan and per-function progress.
- [`docs/INSTALL.md`](docs/INSTALL.md) — install and run.
- [`docs/DEBUG-VIEW.md`](docs/DEBUG-VIEW.md) — in-viewer debug views and the Pokémon spawn selector.
- [`REPRODUCE.md`](REPRODUCE.md) — regenerate the heavy artifacts from your own game copy.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — how to contribute and the legal rules.
- [`odd/PLAN.md`](odd/PLAN.md) — the execution plan; [`THE-SPREADSHEET-METHOD.json`](THE-SPREADSHEET-METHOD.json) — the method.

## Legal

This is interoperability research on a game the collaborators own. Do **not**
commit or redistribute game content, extracted assets, decompiled output or
console keys. Decompiled output is evidence for writing sheet rows — never
source. See [`CONTRIBUTING.md`](CONTRIBUTING.md).
