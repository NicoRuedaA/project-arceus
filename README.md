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

| | |
|---|---|
| ✅ **Done** | Both dumps (base + update) acquired; every executable module and all RomFS data extracted, NSO-verified and version-qualified; the effective base+update overlay measured (17,904 / 466 / 725 / 0); all 153,476 functions located by the fix2 `main` inventory have C or assembly exports; fix2 gameDB files and function rows are counted; auxiliary Ghidra detections and C exports are measured; the update auxiliary modules are byte-identical to base; RomFS data largely parsed (Lua 799/800, tables, texts); a Rust port scaffold that loads and runs real event scripts. |
| 🟡 **In progress** | **P0 is not complete**: base structural import/relocation metadata is inventoried for the five named NSOs, while loader binding, provider identity, full module exploration, NCA header/signature verification, update ContentMeta semantics, file-level ownership and runtime dependency resolution remain open or blocked. The effective overlay has **466 modified + 725 added** entries; internal analysis is **0/466**, and semantic ownership is **unknown for 1,191/1,191**. Pseudocode is *not* understanding: analysis, port and verification of the game code are ~**0.03 %** done. |
| ❌ **Missing** | **3.44713 %** (1,830,644 bytes) is outside existing update-`main` function bodies. Ghidra listing classifies 40,764 bytes as instructions and 1,789,880 as defined data; semantics, valid function boundaries and reachability remain unverified. Corrected read-only triage of 13,897 outside-body seeds found 77 addresses in defined instructions (308 bytes), 13,820 in defined data units (13,820 bytes), 0 undefined and 0 unmapped. Incoming references total 91 targets / 179 edges (CALL 8/11; JUMP class 64/66, conditionality combined; other flow 0/0; non-flow 19/102). Next: semantic triage of defined data/instruction gaps and candidate validity. Lua/data coverage has **0.1 %** remaining; behaviour verification and binary matching are **0 %**. Listing/reference metadata is not function-validity, reachability or semantic evidence. See [range reconciliation](reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), [gap classification](reports/function-progress/p0-update-main-gap-classification.md), [flow triage](reports/function-progress/p0-update-main-gap-flow-triage.md) and [correction evidence](reports/function-progress/p0-update-main-gap-flow-triage-correction.md). |

The detailed, phase-by-phase breakdown is the [Completion plan](#completion-plan)
below — the single status source. The canonical execution route and its evidence
gates live in [`odd/PLAN.md`](odd/PLAN.md).

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
counted from real files and functions; `—` means there is no known denominator
(not measured, so no percentage is invented) and `~` marks qualitative
estimates.

| Phase (PLAN.md) | Part | % done | % left | Note | Methods / tools |
|---|---|:--:|:--:|---|---|
| 1. Extract (P0) | Base: ExeFS + RomFS | 100 % | 0 % | | `.tools/` extraction pipeline; NSO parsing; SHA-256 and segment-hash checks |
| | Update: `main` + data | 100 % | 0 % | | `.tools/` extraction pipeline; NCZ/ExeFS/RomFS manifests; hash checks |
| | Update modules (`rtld`, `sdk`, `subsdk0/1`) | 100 % | 0 % | extracted + NSO-verified; byte-identical to the base's | NSO extraction plus byte/hash comparison against base modules |
| 2. Inventory (P0) | `main` update | — | — | 153,476 inventoried functions match the untouched Ghidra FunctionManager exactly; actual bodies have zero overlap and their union is 51,275,676 / 53,106,320 bytes (96.55287%). The complete valid-function denominator remains unknown; 1,830,644 bytes (3.44713%) are outside every existing body ([range reconciliation](reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), [gap classification](reports/function-progress/p0-update-main-gap-classification.md)) | Direct fix2 project body-range reconciliation and listing-state classification |
| | `main` base | 100 % | 0 % | 68,412 functions; provenance qualified (15/15 NSO segment hashes) | Direct NSO metadata parser; module IDs, SHA-256 and segment-hash verification |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | — | — | Ghidra detections: 27,750 candidates; complete function denominator unknown | Ghidra metadata detections; direct NSO import/relocation inventory; no completeness claim |
| 3. Pseudocode export (prep) | `main` update | 96.55 % | 3.45 % | All 153,476 inventoried functions have outputs (153,471 C + 5 ASM); separately, the exact body union is 96.55287% of executable bytes, with 3.44713% outside existing bodies. Corrected `getCodeUnitContaining` query classifies the 13,897 outside-body seeds as 77 in defined instructions (308 bytes), 13,820 in defined data (13,820 bytes), 0 undefined and 0 unmapped; 91 targets / 179 incoming edges (CALL 8/11; JUMP class 64/66, conditionality combined; other flow 0/0; non-flow 19/102). Listing/reference metadata does not establish semantics, reachability, function boundaries or function existence. The prior `getCodeUnitAt` result misclassified inside-unit addresses as undefined; the earlier jump subtype split is superseded/unconfirmed. Next: independently triage the semantic purpose of defined data/instruction gaps and candidate validity ([residual audit](reports/function-progress/p0-update-main-residual-export.md), [ranges](reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), [gap classification](reports/function-progress/p0-update-main-gap-classification.md), [flow triage](reports/function-progress/p0-update-main-gap-flow-triage.md), [correction evidence](reports/function-progress/p0-update-main-gap-flow-triage-correction.md)) | Fix2 Ghidra inventory/export; C/ASM fallback; direct body-range reconciliation and listing-state/reference-metadata triage |
| | `main` base | 5.8 % | 94.2 % | 3,997 of 68,412 | Ghidra export pipeline; bounded by the current base inventory |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | — | — | 20,327 C exports / 7,902,740 summed body bytes; 50.75% of 15,572,944 `.text` bytes is a measured body-size ratio, not coverage; complete function denominator unknown ([report](reports/function-progress/p0-auxiliary-module-export-inventory.md)) | Ghidra 12.1.2 C pseudocode export; body-size aggregation; not a coverage percentage |
| 4. Index (gameDB) (prep) | `main` update | ~100 % | ~0 % | 153,471 C files indexed / 153,471 C exports; 153,470 function rows (one C file has no parsed function row); 5 asm fallbacks outside index | gameDB SQLite index; read-only indexed-vs-export parity checks |
| | `main` base | 5.8 % | 94.2 % | 3,997 (bounded by export) | gameDB index bounded by the base pseudocode export |
| | `sdk` / `subsdk0` / `subsdk1` / `rtld` | — | — | Index completion/coverage denominator unknown | No auxiliary index completion measurement in current evidence |
| 5. Data (RomFS) (P0) | Lua scripts | 99.9 % | 0.1 % | 799 of 800 | RomFS extraction; Lua parsing and per-file validation |
| | Effective base + update overlay | 100 % | 0 % | Overlay arithmetic only: 19,095 virtual entries (17,904 unchanged / 466 modified / 725 added / 0 removed); internal analysis 0/466 modified; semantic ownership unknown for 1,191/1,191 delta entries; added-path labels 336 mapped / 389 unmapped are heuristic, not ownership | Hash-verified base+update manifest; path/size/hash comparison; heuristic path/SCC labels; no internal-container diff yet ([overlay audit](reports/function-progress/p0-overlay-ownership.md)) |
| | SARC packages | — | — | 264 indexed, no total | SARC parser/indexer; total denominator not established |
| | Strings / references | — | — | 13,370 + 29,162, no total | Extraction/reference scanners; total denominator not established |
| | Domain tables | — | — | 12 sheets, no total | TSV/sheet ingestion and `sheetty` preflight |
| | Base data | — | — | extracted, no clear index | RomFS extraction; complete base-data index pending |
| 6. Analysis (P2–P7) | Game code | 0.03 % | 99.97 % | 22 of 68,330 (legacy capped view) | Direct decompilation evidence and function-progress ledger; broad analysis pending |
| 7. Implementation (P3–P7) | Rust port | 0.01 % | 99.99 % | 8 partial | Rust/Bevy implementation, focused fixtures and CI checks |
| 8. Behaviour verification (P3–P7, P9) | — | 0 % | 100 % | 0 functions | No independent behavioural verification completed yet |
| 9. Binary matching (P8–P9) | — | 0 % | 100 % | 0 functions | No reproducible binary-matching run completed yet |
| 10. Playable port (P6–P7 → P9) | Container parsers (SARC, GFLXPACK, AHTB, BNTX, VFXB) | ~90 % | ~10 % | ASTC and unknown BNTX formats pending | Rust parsers, fixtures and focused parser tests |
| | Lua 5.3 layer | 100 % | 0 % | runs real event scripts | Rust Lua 5.3 host and event-script fixtures |
| | Host bindings | ~10 % | ~90 % | 2 real, rest stubs | Rust/Bevy bindings, event traces and stubs |
| | Save system | ~30 % | ~70 % | seeded from 450 event flags | Rust state model and seeded event-flag fixtures |
| | Visual subsystem | ~40 % | ~60 % | asset-level only; no full render | BNTX decoder, Bevy image/sprite path and headless tests |
| | `tr*` models/animations, ASTC, Havok→avian3d | 0 % | 100 % | pending | Not completed; no implementation evidence yet |

This table inventories *what exists* in each stretch of
[`odd/PLAN.md`](odd/PLAN.md); that plan is the canonical execution route and
fixes the order and evidence gates. The parenthesised labels carry the matching
PLAN.md phase: the preparation phases (1–5) belong to **P0**, and phases 6–10
belong to **P2–P9**, a dependency chain (`P3 → P4 → P5 → P6 → P7`) in which
parallelism exists only *inside* a phase. The port target is the game as it runs
with the update applied (the base + update overlay), never the update alone.

Phases 1–5 describe *having* the code and data; phases 6–9 describe
*understanding and porting* it and hold ~99.97 % of the remaining work. The
68,330 denominator in phases 6–7 is the earlier capped inventory. The untouched
fix2 project directly confirms all 153,476 inventory entries, with zero body
overlap and an exact 51,275,676-byte union (96.55287%). The remaining
1,830,644/53,106,320 bytes (3.44713%) are outside all existing function bodies;
listing state classifies 40,764 bytes as instructions and 1,789,880 as defined
data, without establishing semantics, valid new functions or reachability. Every inventoried
function has an output (153,471 C + 5 ASM); C-body sum is 96.47728% and
assembly-body sum is 0.07559% of executable bytes. A seed-format attempt yielded
zero additional exports. Its +1,425 manager-count discrepancy is specific to
the exploratory clone and remains unexplained by identity; it does not affect
the untouched source reconciliation. **Triage update (2026-10-06):** read-only reference/listing triage of
13,897 outside-body seeds was corrected after identifying a query bug: the prior
`getCodeUnitAt` exact-start lookup missed addresses inside defined units. Using
`Listing.getCodeUnitContaining(address)`, 77 seeds fall in defined instructions
(308 bytes), 13,820 in defined data units (13,820 bytes), and 0 are undefined or
unmapped. Incoming references are 91 targets / 179 edges: CALL 8/11, JUMP class
64/66 (conditionality combined), other flow 0/0, non-flow 19/102. The prior
fine jump subtype split is superseded/unconfirmed. These listing/reference records
do not establish semantics, reachability, function boundaries or function
validity. The next task is semantic triage of defined data/instruction gaps and
candidate validity. See the [correction evidence](reports/function-progress/p0-update-main-gap-flow-triage-correction.md).

The update's auxiliary modules (`rtld`, `sdk`, `subsdk0/1`) are extracted,
NSO-verified and byte-identical to the base's, with original-NSO structural
metadata and imports/relocations measured. Ghidra detected 27,750 candidates
and exported 20,327 C bodies totaling 7,902,740 bytes; the 50.75% ratio against
`.text` is summed body size, not coverage. Complete function denominators and
function-boundary reconciliation are unknown. Auxiliary index completion is
also unknown. **P0
remains in progress**: the base structural inventory is evidenced, but loader
binding/provider identity, complete semantic coverage, NCA header/signature
verification, update ContentMeta semantics, file-level ownership and runtime
dependency resolution remain open or blocked. The overlay ownership audit is
external classification only: **466 modified + 725 added**, **0/466** modified
entries have internal analysis, and semantic ownership is unknown for
**1,191/1,191** entries. Static dependency evidence records **3 `DT_NEEDED`**
entries for update `main`; loaded and reached status remain unknown.

Current P0 evidence is reconciled in [`p0-wave1-reconciliation.md`](reports/function-progress/p0-wave1-reconciliation.md), with direct base inventory, NCA/NPDM analysis, overlay ownership audit, static-only runtime dependency report and current export evidence linked there. The current subsystem-label split for added files is **336 mapped / 389 unmapped**; these are heuristic/path matches, not semantic ownership. The older 360 figure is stale historical wording.

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

![Update v262144 main implementation progress; partial is not completed and behavior/binary matching remain unknown](reports/function-progress/update-v262144/port.png)

![Update v262144 main analysis progress map](reports/function-progress/update-v262144/analysis.png)

[Interactive maps and per-function evidence](reports/function-progress/update-v262144/index.html) ·
[Analysis map](reports/function-progress/update-v262144/analysis.png) ·
[Latest evidence audit](reports/function-progress/update-v262144-evidence-audit.md)

Map area and percentages are weighted by **original native function-body bytes**
for update v262144 **main NSO only**, not whole-game completion, pseudocode
readability or Rust line count. Partial implementations are conditional ports;
whole-function behavioral verification and binary matching remain unknown.

## Legal

This is interoperability research on a game the collaborators own. Do **not**
commit or redistribute game content, extracted assets, decompiled output or
console keys. Decompiled output is evidence for writing sheet rows — never
source. See [`CONTRIBUTING.md`](CONTRIBUTING.md).
