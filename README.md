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
| ✅ **Done** | Both package dumps (base + update) available; the five reviewed executable roles extracted and version-qualified with NSO checks; the recorded outer RomFS sets extracted and overlaid; the effective base+update overlay measured (17,904 / 466 / 725 / 0); all 153,476 functions located by the fix2 `main` inventory have C or assembly exports; fix2 gameDB files and function rows are counted; auxiliary Ghidra detections and C exports are measured; the update auxiliary modules are byte-identical to base; selected RomFS formats parsed (base Lua 799/800, tables, texts); a Rust port scaffold that loads and runs real event scripts. |
| ✅ **Bounded P0** | **P0 closed only as a bounded inventory/provenance baseline** for base v0 + update v262144: the census, static inventory, overlay and provenance reports reconcile documentary gates 1–4, and Wave C passed local gate 5 within the fingerprint limit below. The current census has **26 records**, not a completeness denominator. NCA header/signature authenticity, update ContentMeta/CNMT semantics, NPDM cryptographic trust/runtime necessity, runtime loaded/reached state, and semantic file-to-code ownership are unverified and **Deferred outside P0**, not active P0 blockers. The overlay is **19,095** effective outer files (17,904 unchanged / 466 modified / 725 added / 0 removed); internal comparisons succeeded for **14/466** (SARC 10/10; GFLXPACK 4/10), while **452/466** lack successful comparisons. Parser/compile results overlap and do not add to that numerator. Added-file hashes are **33 exact base matches / 692 non-matches / 0 unknown** across 725 additions; non-match does not prove newness, and the [dated aggregate cross-tab](reports/function-progress/p0-wave8-added-provenance-cross-tab.md) reports group/extension partitions. Independent reproduction of that join now is **Unknown/Blocked** because per-entry flags were not retained. O9 found 118 matching string units, **0 exact-path xrefs and 97 normalized-only xrefs across 52 existing functions**; these are path candidates, not proof of opening, reading or delta ownership; semantic ownership remains **Unknown for 1,191/1,191** delta entries. Pseudocode is *not* understanding: in the fix2-located update-`main` inventory, **22/153,476 functions (~0.0143%)** are documented/analyzed, **8/153,476 (~0.0052%)** have partial implementations, and **0/153,476** have behavior or binary verification. These percentages use the located fix2 inventory only; the valid-function universe beyond its body coverage remains Unknown. |
| ❌ **Missing** | **3.44713 %** (1,830,644 bytes) remains an explicit **Unknown residual** outside existing update-`main` function bodies. Interpretation of the residual and validity of candidate functions are **Deferred beyond P0**; neither is a P0 task or exit gate. Ghidra listing/API metadata classifies 40,764 bytes as instructions and 1,789,880 as defined data; the generic datatype classifier groups all 1,789,328 data units as “other”, and all 20,627 operands are “other or unspecified” by the queried API flags. These are listing/API categories, not semantics. For 13,897 outside-body seeds, 77 are in defined instructions (308 bytes), 13,820 in defined data, and 0 undefined/unmapped; incoming refs: 91 targets / 179 edges (CALL 8/11; JUMP 64/66, conditionality combined; other flow 0/0; non-flow 19/102). The complete valid-function denominator remains Unknown. See [range reconciliation](reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), [successful semantic-query retry](reports/function-progress/p0-update-main-gap-semantic-retry.md), [corrected flow triage](reports/function-progress/p0-update-main-gap-flow-triage-correction.md), and [superseded failed attempt](reports/function-progress/p0-update-main-gap-semantic-triage.md). |

**P0 scope boundary (2026-10-07):** P0 is **closed only as a bounded
inventory/provenance baseline**; all five local gates passed within the
fingerprint-verification limit stated below. Its finite baseline is the pinned base v0
and update v262144 package identities and recorded member sets; six Program
ExeFS members per version (five NSOs plus NPDM); five named executable roles per
version with version-qualified NSO identities and original-NSO structural
imports/relocations; the effective outer RomFS set (base 18,370; effective
19,095 = 17,904 unchanged / 466 modified / 725 added / 0 removed); available
function detection/export/index metrics and body/gap measures (complete valid-
function denominators remain `Unknown` where not established); and the
per-format outer-file classes and parser outcomes recorded by the census and
sanitized package, ExeFS, module, and overlay reports. The 26-row census is a
status/evidence/limits/next-phase register, not a game-wide denominator. This
boundary is limited to those enumerated source sets and their cited metadata;
outer files and nested members are distinct. The update is a patch and the port
target remains base + update.

**Deferred outside P0; status remains `Unknown`/`Deferred`, not verified:** NCA
header signature authenticity; full update CNMT semantic parsing; NPDM
cryptographic trust and runtime necessity; loaded/reached runtime observations;
semantic function understanding, behavior, and matching; a complete
valid-function denominator; semantic file-to-code ownership; and nested
archive/member semantics. No update ContentMeta parse is claimed and no base
CNMT values are copied. The user-reported emulator use is context only, not an
instrumented trace. These are not P0 closure gates, and P0 does not claim whole-
game semantics or port parity.

The five local P0 gates are: (1) reconcile the finite package/member,
ExeFS, module-role, outer-RomFS and per-format sets above against the cited
sanitized manifests/reports and 26 census rows, using that reconciliation as the
stopping rule; (2) reconcile versioned identity/provenance and structural
import/inventory/export/index metrics for the declared module scope, leaving
valid-function denominators and body gaps `Unknown` where not established; (3)
reconcile overlay arithmetic and per-format states, including 14/466 successful
comparisons, 452/466 without success, and 33/692 added-file hash outcomes,
without treating parser results or non-matches as semantics or newness; (4)
classify every census dimension Known/Unknown/Deferred with direct evidence,
reason, limit, and next phase; (5) validate current fix2
profile/archive/inventory/ledger/Rust fingerprints and pass
`cargo run -p sheetty-cli -- check sheets`, `git diff --check`, and configured
CI. Require zero hidden unknowns, not zero unknowns. No NCA/CNMT cryptographic
verification, runtime trace, semantic file-to-code ownership, or complete
valid-function denominator is a P0 closure gate. The global C index is complete and the sanitized report lists the fix2
treemap as current. Wave C confirmed no drift in inventory, ledger or Rust
fingerprints; neither gameDB nor the treemap was regenerated.

**Wave C local verification:** `cargo run -p sheetty-cli -- check sheets`
finished with 29 sheets, 267,974 rows, 0 errors and 0 warnings;
`git diff --check` and `./.tools/ci.sh` both exited 0. Current SHA-256 values
for the inventory (`829d810c…abd2535`), evidence ledger
(`5e83e85a…6f8cb8`) and 49 Rust files (`e388b355…7f34bfb`) match the
[sanitized fix2 report](reports/function-progress/p0-fix2-treemap-profile.md).
The update archive size matches its reported 52,657,467-byte pin. **Limit:**
policy barred reading the JSON manifest and recomputing the update archive's
content hash. This is a bounded no-input-drift and report-consistency check,
**not** fresh direct manifest attestation or cryptographic authenticity.
Neither deferred item blocks this baseline. No further `gamedb index` ran, the
treemap was not regenerated, and no ledger state was promoted
(local ignored log `work/progress/p0-wave-c.log`).

**Reading the bounded inventory:** the base has six PFS0 root entries and
the update has seven reported root entries; the reviewed Program/ExeFS set has
six members per version and five NSO roles. The completed global gameDB index
processed **177,795 available C files** across six roots: **176,667 parsed
rows, 1,128 files without a row, and 87/87 selftests**. Update-`main` fix2 has
153,476 located functions and 51,275,676 body bytes; the remaining 1,830,644
bytes are not additional functions. The base-only SARC index contains 257
archives and 3,416 nested children, separate from the 19,095 effective outer
paths. Auxiliary range classification (2,477,224 instruction bytes and
5,160,284 defined-Data bytes) disagrees with the later `Data.isDefined()`
CodeUnit classification, which is predominantly Undefined-type Data; the
methods and units differ and establish neither functions nor semantics
([static inventory](reports/function-progress/p0-wave-a-static.md),
[census](reports/function-progress/p0-wave-a-census.md)).

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
| 2. Inventory (P0) | `main` update | Unknown | Unknown | 153,476 inventoried functions match the untouched Ghidra FunctionManager exactly; actual bodies have zero overlap and their union is 51,275,676 / 53,106,320 bytes (96.55287%). The complete valid-function denominator remains unknown; 1,830,644 bytes (3.44713%) are outside every existing body ([range reconciliation](reports/function-progress/p0-update-main-ghidra-range-reconciliation.md), [gap classification](reports/function-progress/p0-update-main-gap-classification.md)) | Direct fix2 project body-range reconciliation and listing-state classification |
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
| 6. Analysis (P2–P7) | Game code | ~0.0143 % | ~99.9857 % | 22/153,476 fix2-located functions documented/analyzed; denominator is the located inventory, not a complete valid-function universe. The 68,330 count is the legacy capped view. | Direct decompilation evidence and function-progress ledger; [current fix2 profile and treemap](reports/function-progress/p0-fix2-treemap-profile.md); broad analysis pending |
| 7. Implementation (P3–P7) | Rust port | ~0.0052 % partial ratio | Unknown | 8/153,476 fix2-located functions have partial implementation; partial is not complete. Denominator is the located inventory, not a complete valid-function universe. | Rust/Bevy implementation and function-progress ledger; [current fix2 profile and treemap](reports/function-progress/p0-fix2-treemap-profile.md) |
| 8. Behaviour verification (P3–P7, P9) | Fix2-located functions | 0% within fix2 | Unknown | 0/153,476 fix2-located functions behavior-verified; the whole function universe beyond body coverage is unknown. | No independent behavioral verification completed yet; [current fix2 profile and treemap](reports/function-progress/p0-fix2-treemap-profile.md) |
| 9. Binary matching (P8–P9) | Fix2-located functions | 0% within fix2 | Unknown | 0/153,476 fix2-located functions binary-matched; the whole function universe beyond body coverage is unknown. | No reproducible binary-matching run completed yet; [current fix2 profile and treemap](reports/function-progress/p0-fix2-treemap-profile.md) |
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
*understanding and porting* it. The current function-progress profile is
`update-v262144-fix2`; phase 6–9 function counts above are scoped to its
153,476-function located update-`main` inventory. The 68,330 denominator is the
earlier capped legacy view, and its artifacts remain historical. The untouched
fix2 project directly confirms all 153,476 inventory entries, with zero body
overlap and an exact 51,275,676-byte union (96.55287%). The remaining
1,830,644/53,106,320 bytes (3.44713%) are outside all existing function bodies;
this is a separate executable-byte gap metric, not an additional function row or
a denominator for the treemap's function counts. It remains an explicit Unknown
residual; semantic classification and candidate validity are deferred outside
P0 once the bounded residual is recorded. Listing state classifies
40,764 bytes as instructions and 1,789,880 as defined data, without establishing
semantics, valid new functions or reachability. Every inventoried function has an output (153,471 C + 5 ASM); C-body sum is 96.47728% and
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
validity. Semantic gap triage and candidate validity remain Unknown/Deferred,
not active P0 closure blockers. See the [correction evidence](reports/function-progress/p0-update-main-gap-flow-triage-correction.md).

The update's auxiliary modules (`rtld`, `sdk`, `subsdk0/1`) are extracted,
NSO-verified and byte-identical to the base's, with original-NSO structural
metadata and imports/relocations measured. Ghidra's export run detected 27,750
candidates and exported 20,327 C bodies totaling 7,902,740 bytes; its 50.75%
ratio against `.text` is summed body size, not coverage. A separate read-only
range reconciliation measured exact unions and gaps for existing detections;
entries. Internal comparisons succeeded for **14/466** modified entries, while
**452/466** remain without successful internal comparison; semantic ownership
is unknown for **1,191/1,191** entries. Static dependency evidence records three
`DT_NEEDED` entries and nine candidate name-overlap edges; loaded and reached
status remain unknown.

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
