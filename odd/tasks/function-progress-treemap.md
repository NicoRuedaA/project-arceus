# Function-Level Decompilation and Port Treemap

Status: deterministic registry and report generation are verified for update v262144, including focused tests, current outputs, PNG dimensions, CI/preflight, and HTML browser interactions. Overall work is complete. The update-only inventory has 68,330 rows; base v0 is excluded. See Completion evidence for exact commands, counts, and evidence limits. No commit was made.

## Subsystem membership and treemap assignment

- Source validation compares `sheets/domain/subsystems.tsv` with distinct function-address memberships per subsystem. One function may belong to multiple raw memberships, and repeated references to the same subsystem do not count the function more than once there.
- The measured raw membership counts are: `absl` 106, `appli` 38, `archive` 2, `chara` 5, `effect` 19, `event` 5, `font` 1, `grpc` 1,246, `grpc_core` 158, `havok` 430, `message` 2, `openssl` 536, `pml` 1, `pokemon` 2, `protobuf` 96, `re2` 73, and `webrtc` 17. These sum to 2,737 overlapping memberships and match the sheet.
- Treemap classification remains separate: one exclusive dominant group per qualifying function, `ambiguous` for tied winners, and `unknown` when no qualifying path reference exists. Its measured totals are 2,681 unique dominant, 7 tied, and 65,642 unknown; together they partition the 68,330-function inventory. Do not compare exclusive winner counts with source-sheet membership counts.
- At this correction-phase checkpoint, tests, the generator, CI/build, and browser checks had not run, and no report outputs had been created. The later final verification outcomes are recorded below.

## First verification attempt and corrections

- `python3 .tools/test_function_progress_treemap.py`: PASS (19 tests) before these corrections.
- The first generator run failed because parsed `function.name` values were treated as native addresses. Four distinct files named `thunk_FUN_032a2370` have entry addresses `032a2e00`, `032a3260`, `032a3880`, and `032a4530`; join and count from each parsed `files.path` suffix `_XXXXXXXX.c` instead. The acceptance rule is 9,767 parsed rows, exactly one parsed function row per parsed file, and 9,767 unique path-derived addresses. Names are non-authoritative metadata.
- CI sheet preflight rejected all eleven `update-v262144:XXXXXXXX` primary IDs with `E-L0-KEYCHARS`; use `update_v262144_XXXXXXXX` keys while keeping `build` and `function_id` as separate fields.
- An earlier generation attempt: `csv.reader(..., strict=True)` rejected physical line 46 of `re/exports/update-main/strrefs.tsv` with `'\t' expected after '"'`. The four-cell Ghidra record was `0378af9f<TAB>002fbb60<TAB>FUN_002fbb60<TAB>"false && \"len <= kMaxSize\""`; its text uses exporter/C-style escapes rather than RFC CSV quoting. The correction is a dedicated parser for this exact four-column export: it validates the exact header, four physical fields, and both source addresses; requires a well-formed outer-quoted text cell; decodes only `\\` and `\"`; preserves unrecognized escapes such as `\n`; and rejects malformed quoting without skipping rows. Generic sheet TSV parsing remains unchanged.
- At this parser-correction checkpoint, tests, the generator, CI/build, and browser checks had not rerun. The final verification outcomes are recorded below.

## Latest generator failure and parser correction

- After the focused suite passed 24 tests, a real generator run failed with `The update strrefs.tsv header/schema changed`. The actual `re/exports/update-main/strrefs.tsv` begins with `#` manifest metadata such as `# sheet: ...` and `# version: ...`, followed by the exact four-column header. `load_strrefs()` was selecting the first nonempty line as the header.
- `load_strrefs()` now locates the exact schema after skipping blank and leading `#` manifest-comment lines. It continues to parse data from the original lines so malformed-row errors retain physical file line numbers. Strict four-column validation, address checks, Ghidra string escape parsing, and generic `load_tsv()` behavior are unchanged.
- Regression coverage now parses a representative header/data row after leading comments and blank lines, checks preserved physical line numbering for a malformed data row, and retains existing malformed-header/row cases.
- At this parser-fix checkpoint, tests, generator, CI/build, browser, and final checks were pending. Their later outcomes are recorded below.

## Goal

Produce a reproducible, evidence-first function registry and two visualizations for one explicitly identified game build: (1) analysis/decompilation coverage, and (2) Rust/Bevy port progress. Include original Ghidra function byte sizes, per-state evidence, conservative unknowns, and explicit denominator/version metadata. Add a permanent regeneration rule to the repository-root `AGENTS.md`.

## Current evidence and constraints

- `pk1.nsz` is base v0; `pk2.nsz` is update v262144. The inspected update archive is 52,657,467 bytes with SHA-256 `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`.
- **Selected build: update v262144, `main` only.** `sheets/re/functions.tsv` has exactly 68,330 function rows and its complete data rows match `re/exports/update-main/functions.tsv`; the distinct base inventory has 68,412 rows and is not joined into this output. Update `main` function-body sizes sum to 19,029,816 bytes.
- `decompiled/update-main/` currently contains 12,303 uniquely address-matched `.c` exports: 12,207,980 original function bytes (64.1519%) and 18.005% of inventory rows. This proves pseudocode artifacts exist, not analysis or behavior. The shared gameDB has separate module labels: `ns.update.main` contains 10,019 indexed files and 9,767 parsed function rows; the base module is counted separately. Index parsing is not behavioral analysis.
- `re/exports/update-main/strrefs.tsv` plus `.tools/subsystem_cluster.py` supports a path-reference-derived subsystem heuristic, not a complete semantic ownership map. Raw per-subsystem memberships overlap; they match the `functions` counts in `sheets/domain/subsystems.tsv` at 2,737 total memberships. Treemap assignments remain exclusive: 2,681 unique dominant groups, 7 ties, and 65,642 functions without qualifying path references, partitioning the 68,330-function inventory.
- Existing decision records directly document 11 matching update addresses (20,852 original bytes), the documented-analysis total recorded below. The port implementation sheet has no function-address links; searched behavior tests do not name these native addresses, and no binary-match evidence exists. Therefore do not attribute work-unit Rust/test status to any function: port state and binary matching remain unknown.
- Current function inventory covers the update `main` NSO only. `re/base_update_diff.tsv` names `rtld`, `sdk`, `subsdk0`, and `subsdk1` but provides no function inventory for them. Other executable modules are not fully enumerated here. The chart denominator is not the whole game or all game binaries.
- A Ghidra inventory row with `status=identified` proves identification only. Pseudocode export, gameDB indexing, a Rust build, and unrelated tests do not prove analyzed/documented or native behavioral parity.
- Existing proprietary assets/binaries and decompiled C are ignored local inputs. Generated outputs must contain metadata/evidence references only, never copied game bytes or pseudocode.

## Generator hardening before first run

- The generator verifies pinned SHA-256 inputs for `sheets/re/functions.tsv` (`b1c93d898964d1b619b6f278855a3c1080c82d41b5f22b21d702e10c6312e163`), `re/exports/update-main/functions.tsv` (`8cefd2b2b7ce412653271ceec66be1325c0fcb0bc15b1d34bf8b3cdbfd5b7f8c`), `re/exports/update-main/strrefs.tsv` (`491c3e236823dcc57394409aceaa846134093d4f77d07f5787e8ee47defdc581`), `sheets/domain/subsystems.tsv` (`5677a306b35d491fc2f740707b74fdec6c2c218de8952549d5150c225bf7484f`), and `sheets/re/base_update_diff.tsv` (`c53edc0134c9b47da88325d89f9f8a07b9d6406deacc2ad5b141966ffc777b75`). The `pk2.nsz` archive's existing exact size/hash check remains. Any intentional input refresh requires explicit review and updating the corresponding pins.
- Evidence ledger primary IDs use safe `update_v262144_XXXXXXXX` keys; `build` and `function_id` remain separate fields. Ledger references must be repository-relative `path#fragment` citations that resolve to in-repository files and existing fragments. `analyzed_documented` additionally requires a matching `sheets/decisions.tsv#decNNN` record; implementation, behavior-verification, and binary-match states require valid direct citations. Other states remain unknown.
- gameDB acceptance requires 9,767 parsed rows in `ns.update.main`, exactly one parsed function row per parsed file, and 9,767 unique native addresses derived from each parsed `files.path` suffix `_XXXXXXXX.c`. Parsed symbol names are non-authoritative metadata and must not determine address joins or duplicate handling. `strrefs.tsv` must retain its exact four-column schema and every nonblank row must contain four fields; documented `subsystem_cluster.prefix_of()` behavior is unchanged.
- Output paths are fixed beneath the repository root, symlink escapes are refused, and files are written atomically. Prior report files are moved into an internal timestamped stale archive before generation. Failure/interruption must leave stale status and explicit HTML/PNG/TSV placeholders rather than old reports presented as current.
- Generated TSV cells beginning with `=`, `+`, `-`, or `@` are spreadsheet-safe; HTML/JSON escaping remains independent. The evidence ledger is manually curated source evidence read by the generator, not generated output.

## Work units

1. [x] Reconcile versioned base/update inventories and exports; audit Ghidra, gameDB, sheets, Rust mappings, tests, and task evidence; document missing modules and denominators. *(update-only inventory and denominator are recorded below; base v0 is excluded)*
2. [x] Define and generate one auditable per-function registry with independent analysis, decompilation, implementation, and verification states; preserve unknowns and one-to-many relations without duplicating progress. *(generator and focused tests pass; generated registry is current; unsupported port/behavior/binary-match states remain unknown)*
3. [x] Implement the deterministic cushion-treemap generator, tests, two PNGs, and searchable/zoomable HTML details view. *(26 focused tests passed; generation and both PNG dimensions verified; browser-interaction proof is tracked under item 5)*
4. [x] Add the root `AGENTS.md` maintenance instruction and link the script, registry, and outputs; preserve existing policy and no-publish rules. *(read-back confirmed; repository CI and sheet preflight pass)*
5. [x] Regenerate and verify row counts, weighted totals/percentages, state evidence, version identity, HTML interactions, PNG output, and repository publication hygiene. *(complete: report identity, counts, percentages, evidence, generated outputs, PNGs, CI/preflight, and browser interactions verified; no test changes to the repository)*

## Implementation paths

- `.tools/function_progress_treemap.py`
- `.tools/test_function_progress_treemap.py`
- `sheets/re/function_progress_evidence.tsv`
- `sheets/01-schema.tsv`, `sheets/02-plan.tsv`, and `sheets/03-impl.tsv`
- `AGENTS.md`
- Final verification generated the report outputs under `reports/function-progress/update-v262144/`; do not hand-edit them. The focused tests ran before generation, both PNGs were read back, and browser interactions were verified as recorded below. This documentation/projection update does not rerun tests or regenerate outputs.


## Completion evidence

Overall status: **complete**. Registry/report generation, repository checks, and HTML browser interactions passed. No commit was made.

### Commands and outputs

| Command/check | Observed result |
|---|---|
| `python3 .tools/test_function_progress_treemap.py` | PASS: 26 tests. |
| `python3 .tools/function_progress_treemap.py --build update-v262144` | SUCCESS; `reports/function-progress/update-v262144/generation-status.json` is current. Generated `reports/function-progress/update-v262144/function-progress.tsv`, `reports/function-progress/update-v262144/analysis.png`, `reports/function-progress/update-v262144/port.png`, `reports/function-progress/update-v262144/index.html`, `reports/function-progress/update-v262144/manifest.json`, and `reports/function-progress/update-v262144/generation-status.json`. |
| `./.tools/ci.sh` | PASS: `ci ok`; sheet preflight reported 0 errors and 0 warnings; build/tests passed, two tests ignored. |
| PNG readback | Both `analysis.png` and `port.png` were read successfully at 1440×900. |
| `file://` Playwright interaction check | PASS: Playwright 1.63.0 with Chromium 153.0.8010.12; page loaded with HTTP 200, search returned `00000190 — FUN_00000190`, analysis/port switching worked, function details opened, zoom changed scale 1.0→1.12, pan changed translation, canvas pixel hashes changed, and console/page errors were empty. Repository status was unchanged across the browser pass. |

### Build, denominators, and evidence limits

- Target: update v262144, `pk2.nsz`, SHA-256 `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`. Base v0 is excluded.
- Denominator: 68,330 update-main function rows and 19,029,816 original Ghidra body bytes. This is not whole-game coverage.
- Matching pseudocode exports: 12,303 functions / 12,207,980 body bytes = 18.005269% of rows / 64.151855% of bytes.
- Documented analysis: 11 functions / 20,852 bytes = 0.0161% of rows / 0.1096% of bytes.
- Update-only gameDB: 10,019 indexed files, 9,767 distinct parsed file-entry addresses, and 252 files without parsed functions.
- Subsystem evidence: 2,737 overlapping distinct-address memberships match the source sheet. The treemap's exclusive partition is 2,681 unique dominant, 7 tied, and 65,642 unknown.
- Port implementation, behavior verification, and binary matching remain unknown for all 68,330 functions; no direct evidence supports stronger claims.

### Browser interaction verification

The user authorized Playwright installation outside the repository. Playwright 1.63.0 ran from a temporary Python environment, and Chromium 153.0.8010.12 launched from the local browser cache. The standalone HTML loaded successfully; search for `00000190` returned `FUN_00000190`; the `analysis` and `port` views both selected; function details displayed the expected build and evidence fields; zoom changed the canvas transform scale from 1.0 to 1.12 and changed rendered pixels; pan changed canvas translation and rendered pixels; no console errors or uncaught page errors occurred. The repository status digest was identical before and after the focused browser pass. An initial harness attempt failed before browser launch due to an indentation error; the corrected checks above are the final observed result. No project dependencies or report artifacts were modified by browser verification.
