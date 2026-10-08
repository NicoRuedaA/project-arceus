# Pokémon Legends: Arceus — reverse-engineering workspace & Rust/Bevy port

**English** · [Español](README.es.md)

A **The Spreadsheet Method** workspace for reverse-engineering a legally owned
copy of *Pokémon Legends: Arceus* (Nintendo Switch) and driving a Rust/Bevy port
from a versioned sheet book.

> **This repository contains no game content.** No ROMs/NSZ, no extracted
> assets, no decompiled code, no keys. It ships the *method*, the *tooling* and
> the *derived knowledge* (sheets). Every collaborator supplies their own copy of
> the game and their own `prod.keys`, and regenerates the heavy artifacts
> locally — see [`REPRODUCE.md`](REPRODUCE.md).

## Where the project stands

Figures refer to the update v262144 `main` (153,476 located functions). The phase-by-phase detail is in the [Completion plan](#completion-plan). Last updated: 2026-10-07.

### 📊 Progress by phase

<!-- progress-bars:start -->
```text
 1 Extraction                       ████████████████████    100 %   enumerated files and members
 2 Inventory of main                ███████████████████░   98.4 %   executable bytes inside functions
 3 Pseudocode export                ████████████████████    100 %   of the 153,476 located functions
 4 gameDB index                     ████████████████████    100 %   of the exported C files
 5 Data (RomFS)                     ████████████████████    100 %   extracted and overlaid
     · interpretation               ░░░░░░░░░░░░░░░░░░░░    3.0 %   14/466 modified files compared inside
 6 Analysis: read by analysts       ░░░░░░░░░░░░░░░░░░░░    2.5 %   3,810 of 153,476 functions
     · classified by program        ███░░░░░░░░░░░░░░░░░   18.4 %   28,308: tiny functions; says what they do, not what they are for
     · copied from identical        ██░░░░░░░░░░░░░░░░░░   14.1 %   21,631: exact copies of an audited function
     · import stubs                 ░░░░░░░░░░░░░░░░░░░░    0.5 %   801: identified from link data
 7 Implementation (port)            ░░░░░░░░░░░░░░░░░░░░   <0.1 %   8 of 153,476 with a partial implementation
 8 Behaviour verification           ░░░░░░░░░░░░░░░░░░░░      0 %   0 of 153,476
 9 Binary matching                  ░░░░░░░░░░░░░░░░░░░░      0 %   0 of 153,476
10 Playable port                    █████████░░░░░░░░░░░    ~45 %   average of qualitative estimates
```

*Bars show what is actually quantified in each phase; several phases have an unknown full denominator (see the Completion plan). Phase 2 measures executable bytes covered by functions, not a function count; in phase 6 only the first line is functions read and understood; the other three are counted separately.*
<!-- progress-bars:end -->

### ✅ Done

- **Extracted material:** base (v0) and update (v262144) available; the five executables (`main`, `rtld`, `sdk`, `subsdk0`, `subsdk1`) extracted and hash-verified; RomFS extracted and overlaid. The update's auxiliary modules are byte-identical to the base's.
- **Game data:** the base + update overlay has 19,095 files (17,904 unchanged, 466 modified, 725 added, 0 removed). Interpreted so far: the base Lua scripts (799 of 800), plus selected tables and texts.
- **`main` functions:** 153,476 located functions, all with exported pseudocode (153,471 in C and 5 in assembly) and indexed in gameDB. After fixing six wrong "never returns" flags, their bodies cover **98.40 %** of the executable code (52,255,468 of 53,106,320 bytes). The remaining 850,852 bytes are classified (padding, tables, data and possible code) but not interpreted ([report](reports/function-progress/d1-main-gap-closure.md)).
- **Third-party code separated:** 36,021 functions (23.47 %) are libraries linked into the program (networking, Havok, Wwise, Nintendo SDK, Lua, Oodle). Game code is at most 117,455 functions ([report](reports/function-progress/d2-library-ownership.md)).
- **Technical findings:** the engine's name hash is 64-bit FNV-1a with its own offset basis (it reproduces 450 of 450 event-flag ids); all 816 library calls resolved; 205,023 missing data pointers applied.
- **Documented functions: 54,550 of 153,476 (35.54 %), in four separate categories:** 801 import stubs, 3,810 complete-body analyst records, 21,631 copies of independently audited EXACT functions and 28,308 deterministic mechanical classifications. These are static operation descriptions, not game-purpose interpretation, types (N2), runtime verification or binary matching. [Latest documentation round](reports/function-progress/d4-batch123-documentation-first.md); [repeatable queue](reports/function-progress/d4-documentation-queue.md).
- **Identical functions grouped:** 109,550 distinct functions out of 153,476; 53,912 sit in groups of identical copies.
- **Port:** a Rust skeleton that loads and runs real event scripts.

### 🔄 In progress

- Reading the remaining game functions, in order of importance.

### ❌ Missing

- **Understanding the game code:** only 0.87 % has been read by analysts; the rest of what is documented is stubs, copies or tiny functions.
- **Fixing wrongly delimited function boundaries.** This would change the 153,476 total and awaits a decision.
- **Regenerating the exported pseudocode,** which is out of date after the fixes.
- **Data:** 452 of the 466 modified files still lack a successful internal comparison, and it is not known which code each data file belongs to.
- **Behaviour verification and binary matching:** 0 of 153,476 functions.
- **Port:** partial (8 functions with a partial implementation; visual, save and system bindings very incomplete).

## The game: base + update

The game ships in two parts:

- **Base** (`pk1.nsz`, v0) — the complete title: every executable module
  (`main`, `rtld`, `sdk`, `subsdk0`, `subsdk1`) and almost all data (the RomFS,
  ~2.3 GB).
- **Update** (`pk2.nsz`, v1.1.1 / v262144) — a **patch**, not a standalone
  program. It carries a complete new `main` plus only the data files it changes
  (an AesCtrEx/BKTR delta).

**The update does not run on its own.** Playing v1.1.1 needs the base **and** the
update; the running game is the **overlay of the two**, and the program code that
executes is the update's `main`, which replaces the base's. The base is therefore
**mandatory** — it supplies everything the update does not ship.

**Port target:** the game as it runs with the update applied (the base + update
overlay) — never "the update", which is not a program. The base's own `main` (v0)
is an older build of the same program and is not ported; it is recorded for
provenance only.

## Completion plan

Phase-by-phase status from the raw game dumps to a playable port. Percentages are
counted from real files and functions within each stated inventory; extraction
percentages apply to the enumerated member/path sets, not an unproven
whole-game denominator. `Unknown` means the complete denominator
for that phase/module is unknown, so no completion percentage is invented; use
`N/A` only when the part does not apply. Known staged-file counts do not
establish complete-module coverage. `~` marks qualitative estimates.

| Phase (PLAN.md) | Part | % done | % left | Note | Methods / tools |
|---|---|:--:|:--:|---|---|
| 1. Extract (P0) | Base: ExeFS + RomFS | 100 % | 0 % | | `.tools/` extraction pipeline; NSO parsing; SHA-256 and segment-hash checks |
| | Update: `main` + data | 100 % | 0 % | | `.tools/` extraction pipeline; NCZ/ExeFS/RomFS manifests; hash checks |
| | Update modules (`rtld`, `sdk`, `subsdk0/1`) | 100 % | 0 % | extracted + NSO-verified; byte-identical to the base's | NSO extraction plus byte/hash comparison against base modules |
| 2. Inventory (P0) | `main` update | Unknown | Unknown | The 153,476 inventoried functions match the Ghidra project's FunctionManager exactly. In the working copy, after fixing six wrong "never returns" flags (D1), the body union is 52,255,468 / 53,106,320 bytes (98.40 %); the original fix2 reference was 51,275,676 (96.55287 %). 850,852 bytes remain outside every body and are classified in the [D1 report](reports/function-progress/d1-main-gap-closure.md). The complete valid-function denominator remains unknown ([range reconciliation](reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), [gap classification](reports/function-progress/p0-update-main-gap-classification.md)) | Ghidra body-range reconciliation, listing-state classification and correction of "never returns" flags |
| | `main` base | Unknown | Unknown | 68,412 historical capped detections, not a complete denominator. Provenance qualified (15/15 NSO segment hashes); base `main` is port provenance only. | Direct NSO metadata parser; module IDs, SHA-256 and segment-hash verification |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | Unknown | Unknown | Exact existing-body unions / `.text` bytes / gaps: `rtld` 5,220/6,240; gap 1,020/13 ranges; `sdk` 1,167,456/5,822,640; gap 4,655,184/11,529; `subsdk0` 1,746,372/3,445,104; gap 1,698,732/2,268; `subsdk1` 5,016,388/6,298,960; gap 1,282,572/3,210. These reconcile existing detections only, not complete function coverage. Inventory completeness and complete export denominators are unknown; the gameDB index of the available C corpus is complete ([range reconciliation](reports/function-progress/p0-auxiliary-range-reconciliation.md)) | Direct Ghidra body-range unions and gap listing classes; inventory entry-ID check; completeness not established |
| 3. Pseudocode export (prep) | `main` update | 100% of located inventory | 0% of located inventory | All 153,476 inventoried functions have outputs (153,471 C + 5 ASM); separately, the exact body union is 96.55287% of executable bytes, with 3.44713% outside existing bodies. Corrected `getCodeUnitContaining` query classifies the 13,897 outside-body seeds as 77 in defined instructions (308 bytes), 13,820 in defined data (13,820 bytes), 0 undefined and 0 unmapped; 91 targets / 179 incoming edges (CALL 8/11; JUMP class 64/66, conditionality combined; other flow 0/0; non-flow 19/102). Listing/reference metadata does not establish semantics, reachability, function boundaries or function existence. The prior `getCodeUnitAt` result misclassified inside-unit addresses as undefined; the earlier jump subtype split is superseded/unconfirmed. Interpretation of the residual and candidate-function validity are **Deferred beyond P0**, not P0 tasks or exit gates ([residual audit](reports/function-progress/p0-update-main-residual-export.md), [ranges](reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), [gap classification](reports/function-progress/p0-update-main-gap-classification.md), [flow triage](reports/function-progress/p0-update-main-gap-flow-triage.md), [correction evidence](reports/function-progress/p0-update-main-gap-flow-triage-correction.md)) | Fix2 Ghidra inventory/export; C/ASM fallback; direct body-range reconciliation and listing-state/reference-metadata triage |
| | `main` base | 5.8 % | 94.2 % | 3,997 of 68,412 | Ghidra export pipeline; bounded by the current base inventory |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | Unknown | Unknown | 20,327 C exports / 7,902,740 summed body bytes; 50.75% of 15,572,944 `.text` bytes is a measured body-size ratio, not coverage; complete function denominator unknown ([report](reports/function-progress/p0-auxiliary-module-export-inventory.md)) | Ghidra 12.1.2 C pseudocode export; body-size aggregation; not a coverage percentage |
| 4. Index (gameDB) (prep) | `main` update | 100 % | 0 % | All 153,471 available staged C files indexed; 152,634 parsed function rows, 837 files without a parsed function row; 5 ASM fallbacks are outside this C-only corpus. This is staged-export indexing, not semantic coverage ([global index report](reports/function-progress/p0-global-gamedb-index.md)) | gameDB SQLite index; read-only indexed-vs-export parity checks |
| | `main` base | 5.8 % | 94.2 % | Current partial export/index: 3,997 files, 3,917 parsed function rows, 80 without a parsed row; percentage remains bounded by 3,997 / 68,412 inventoried functions ([global index report](reports/function-progress/p0-global-gamedb-index.md)) | gameDB SQLite index bounded by the partial base pseudocode export |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | Unknown | Unknown | All currently available staged C files indexed: 20,327 files / 20,116 parsed function rows / 211 without parsed rows (`rtld` 31/31/0; `sdk` 9,063/9,002/61; `subsdk0` 2,959/2,901/58; `subsdk1` 8,274/8,182/92; each triplet is files/rows/no-row files). Complete module function denominators are unknown, so module-completion percentages remain Unknown ([global index report](reports/function-progress/p0-global-gamedb-index.md)) | gameDB SQLite index; current-export counts only, not complete-module coverage |
| 5. Data (RomFS) (P0) | Base Lua scripts | 99.9 % | 0.1 % | 799 of 800 | RomFS extraction; Lua parsing and per-file validation |
| | Effective base + update overlay | 100 % | 0 % | **Overlay arithmetic only:** 19,095 virtual entries (17,904 unchanged / 466 modified / 725 added / 0 removed). Structural comparisons: 14/466 modified successful (SARC 10/10; GFLXPACK 4/10); 452/466 still lack successful internal comparison. Message: 376 changed + 320 added files; 189 complete pairs, 179 fully accepted / 10 partial; `.dat` 378/378 and `.tbl` 368/378 accepted. `.blua` compile: 96/108 accepted, 12 rejected (all 108 headers Lua 5.3; chunks not called). Event-progress `.bin`: 68/90 accepted, 22 unsupported; parser has no magic signature. Added hashes: 33/725 exact base matches, 692/725 no exact match; nonmatches do not prove newness. **Semantic ownership unknown for 1,191/1,191 delta entries.** | Hash-verified manifest; format-specific structural parsers only; aggregate file hashing; see [overlay audit/addendum](reports/function-progress/p0-overlay-ownership.md), [message tables](reports/function-progress/p0-message-table-overlay-audit.md), [AHTB](reports/function-progress/p0-ahtb-overlay-reinspection.md), [GFLXPACK](reports/function-progress/p0-gfpak-variant-followup.md), [Lua](reports/function-progress/p0-blua-overlay-compile-audit.md), [event tables](reports/function-progress/p0-event-table-overlay-audit.md), and [local internal audit](reports/function-progress/p0-overlay-local-internal-audit.md) |
| | SARC packages | Unknown | Unknown | Base-only index: 257 SARC archives and 3,416 nested children; the effective base+update denominator and semantics are unknown. Nested children are not added to the 19,095 outer paths. | SARC parser/indexer; total denominator not established |
| | Strings / references | Unknown | Unknown | 13,370 + 29,162, no total | Extraction/reference scanners; total denominator not established |
| | Domain tables | Unknown | Unknown | 12 sheets, no total | TSV/sheet ingestion and `sheetty` preflight |
| | Base data | 100% of enumerated outer paths | 0% of enumerated outer paths | 18,370 base outer paths; not a complete semantic inventory and excludes nested archive children. | RomFS extraction; complete base-data index pending |
| 6. Analysis (P2–P7) | Game code | ~35.54 % (2.48 % read) | ~64.46 % | 54,550/153,476 documented: 801 import stubs, 3,810 individually read, 21,631 audited EXACT copies, 28,308 mechanical classifications. 98,926 located functions still lack markers. Priority: document observed operations for every located function; game-purpose interpretation and N2 come later. The located inventory is not the complete valid-function universe. | [Latest D4 round](reports/function-progress/d4-batch123-documentation-first.md); [documentation queue](reports/function-progress/d4-documentation-queue.md); [current fix2 profile](reports/function-progress/p0-fix2-treemap-profile.md) |
| 7. Implementation (P3–P7) | Rust port | ~0.0052 % partial ratio | Unknown | 8/153,476 fix2-located functions have partial implementation; partial is not complete. Denominator is the located inventory, not a complete valid-function universe. | Rust/Bevy implementation and function-progress ledger; [current fix2 profile and treemap](reports/function-progress/p0-fix2-treemap-profile.md) |
| 8. Behaviour verification (P3–P7, P9) | Fix2-located functions | 0% within fix2 | Unknown | 0/153,476 fix2-located functions behavior-verified; the whole function universe beyond body coverage is unknown. | No independent behavioral verification completed yet; [current fix2 profile and treemap](reports/function-progress/p0-fix2-treemap-profile.md) |
| 9. Binary matching (P8–P9) | Fix2-located functions | 0% within fix2 | Unknown | 0/153,476 fix2-located functions binary-matched; the whole function universe beyond body coverage is unknown. | No reproducible binary-matching run completed yet; [current fix2 profile and treemap](reports/function-progress/p0-fix2-treemap-profile.md) |
| 10. Playable port (P6–P7 → P9) | Container parsers (SARC, GFLXPACK, AHTB, BNTX, VFXB) | ~90 % | ~10 % | ASTC and unknown BNTX formats pending | Rust parsers, fixtures and focused parser tests |
| | Lua 5.3 layer | 100 % | 0 % | runs real event scripts | Rust Lua 5.3 host and event-script fixtures |
| | Host bindings | ~10 % | ~90 % | 2 real, rest stubs | Rust/Bevy bindings, event traces and stubs |
| | Save system | ~30 % | ~70 % | seeded from 450 event flags | Rust state model and seeded event-flag fixtures |
| | Visual subsystem | ~40 % | ~60 % | asset-level only; no full render | BNTX decoder, Bevy image/sprite path and headless tests |
| | `tr*` models/animations, ASTC, Havok→avian3d | 0 % | 100 % | pending | Not completed; no implementation evidence yet |

This table inventories *what exists* at each stretch of
[`odd/PLAN.md`](odd/PLAN.md).

Phases 1–5 describe *having* the code and data.
Phases 6–9 describe *understanding and porting it*.

## What is here

| Path | What it is |
|---|---|
| `sheets/` | **The sheet book** (the source of truth): doctrine, schema, plan/impl, decisions, domain sheets and the RE evidence sheets. |
| `crates/sheetty` | Canonical TSV parser, L0–L3 preflight, per-sheet emitter (PHF registry, dense index, generated Rust modules). |
| `crates/sheetty-cli` | `sheetty check <sheets-dir>` — the preflight CLI. |
| `crates/pla` | The port crate: container/asset parsers, Lua 5.3 script host, save system, sheet-driven ECS/visual scaffold. |
| `gamedb/` | **Git submodule** — [smileybaal/gamedb](https://github.com/smileybaal/gamedb) (MIT), the decompiled-source indexer (SQLite). |
| `.tools/` | The extraction pipeline (Python) and the Ghidra export scripts. |
| `odd/` | Organic Driven Development task documents (per-feature work log and the execution plan). |
| `THE-SPREADSHEET-METHOD.json` | The normative method (master design document) this workspace implements. |

## Quickstart

```bash
# 0. Clone with the submodule (or: git submodule update --init)
git clone --recurse-submodules <repo-url> && cd <repo>

# 1. Sheet book: preflight + generated modules
cargo run -p sheetty-cli -- check sheets     # 0 errors expected
cargo build -p pla                           # build.rs runs the preflight + emitter
cargo build --manifest-path gamedb/Cargo.toml --release   # the indexer

# 2. Tests (game fixtures are optional: missing ones skip)
cargo test
```

### Running the fixture-based tests

The parser/event tests use real game files as fixtures. Point `PLA_FIXTURES` at
your own extraction (see `REPRODUCE.md` for the fixture list), or copy them into
`crates/pla/tests/fixtures/`:

```bash
PLA_FIXTURES=/path/to/your/extraction cargo test
```

Without fixtures the repository still compiles and every test passes (the
fixture-based ones print `[skip]` and return).

## Function-level progress

![Fix2-located update v262144 main implementation progress; partial is not completed and behavior/binary matching remain unknown](reports/function-progress/update-v262144-fix2/port.png)

![Fix2-located update v262144 main analysis progress map](reports/function-progress/update-v262144-fix2/analysis.png)

[Current fix2 per-function evidence table](reports/function-progress/update-v262144-fix2/function-progress.tsv) ·
[Current fix2 analysis map](reports/function-progress/update-v262144-fix2/analysis.png) ·
[Current fix2 profile report](reports/function-progress/p0-fix2-treemap-profile.md) ·
[Latest evidence audit](reports/function-progress/update-v262144-evidence-audit.md)

Map area and percentages are weighted by **original native function-body bytes**
for the fix2-located update v262144 **main NSO inventory only**, not whole-game
completion, pseudocode readability or Rust line count. The valid-function
universe beyond existing body coverage remains unknown. The `update-v262144`
profile is a historical, capped 68,330-function view. Partial implementations
are conditional ports; whole-function behavioral verification and binary
matching remain unknown.

## Legal

This is interoperability research on a game the collaborators own. Do **not**
commit or redistribute game content, extracted assets, decompiled output or
console keys. Decompiled output is evidence for writing sheet rows — never
source. See [`CONTRIBUTING.md`](CONTRIBUTING.md).
