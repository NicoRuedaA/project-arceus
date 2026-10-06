# P0 Census and Evidence-Ledger Reconciliation Audit

Audit date: 2026-10-06. Status: **read-only; no canonical source was edited**.
Scope: the P0 scope census, function-progress evidence ledger, Wave 1/2 reports,
and current README status tables for base v0 + update v262144. This report contains
metadata and evidence references only. No Ghidra process or `gamedb index` was run.

## Executive result

The canonical P0 census contains 17 records and its file-level citation paths
resolve. The function-progress evidence ledger contains 22 update-`main` rows;
its SHA-256 matches the hash recorded by the generated progress manifest. Those
facts do not make the generated map current for the newer fix2 inventory: that
manifest and its artifacts still use the capped 68,330-function / 19,029,816-byte
denominator, while the fix2 report records 153,476 located functions and 53,106,320
executable bytes. No P0-wide progress states are encoded by the per-function
ledger, and no function-state promotion is justified by the inventory/export
reports.

**Discrepancies recorded: 7**, grouped by distinct issue (including one internal
report count mismatch). The P0 closure gate remains **open**. Exact proposed
integration edits are in the table below; none were applied.

## Files checked

- `.opencode/skills/decompilacion/SKILL.md`
- `THE-SPREADSHEET-METHOD.json`
- `odd/PLAN.md`
- `sheets/re/p0_scope_census.tsv`
- `sheets/re/function_progress_evidence.tsv` (the current function-level evidence
  ledger; `sheets/re/p0_evidence_ledger.tsv` does not exist)
- `README.md`, `README.es.md`
- Wave 1 and reconciliation: `p0-wave1-reconciliation.md`,
  `p0-unexplored-modules.md`, `p0-base-import-relocation-inventory.md`,
  `p0-nca-npdm-analysis.md`, `p0-overlay-ownership.md`,
  `p0-sheet-plan-reconciliation.md`
- Other current P0 evidence: `p0-base-v0-module-provenance.md`,
  `p0-base-update-overlay.md`, `p0-ownership-and-npdm.md`,
  `p0-update-main-npdm-extraction.md`, `p0-update-extraction-attempt.md`,
  `p0-update-main-dynamic-metadata.md`, `p0-update-aux-ghidra-inventory.md`,
  `p0-base-aux-ghidra-inventory.md`, `p0-module-import-relocation-inventory.md`,
  `p0-update-changed-content-classification.md`, `p0-aux-module-search.md`,
  `p0-runtime-dependency-resolution.md`
- Current export and generated status: `update-main-full-pseudocode-export.md`,
  `reports/function-progress/update-v262144/manifest.json`,
  `reports/function-progress/update-v262144/generation-status.json`,
  `update-v262144-evidence-audit.md`

Read-only validation confirmed 17 census records, 22 function-ledger rows, and
zero missing citation *paths* in their populated evidence columns. The manifest's
recorded hashes for `sheets/re/function_progress_evidence.tsv` and
`sheets/re/functions.tsv` match the current files. Path existence does not imply
that a referenced fragment points to the relevant current evidence.

## Discrepancies and direct evidence

| ID | Finding | Direct evidence and consequence |
|---|---|---|
| D1 | **Broken reconciliation-table fragments in the census.** `#C3` occurs 7 times, `#C2` once, and `#O4` once on `p0-wave1-reconciliation.md`; those are table row labels, not section/HTML anchors. File paths exist, but these nine citations do not navigate to the claimed row. | `p0-wave1-reconciliation.md` §§“Closed evidence” and “Still-open P0 items” use C1–C5/O1–O10 as table-cell IDs and define no HTML anchors for those IDs. Direct replacement targets: `p0-base-import-relocation-inventory.md#per-module-counts`, `p0-base-v0-module-provenance.md#3-version-qualified-base-module-identity`, and `p0-nca-npdm-analysis.md#nca-header-signature-verification-attempt`. |
| D2 | **Fix2 facts in the update-main census row cite a capped manifest.** `update_main_identity` states 153,476 functions / 153,471 C + 5 assembly / 153,471 indexed files / 153,470 function rows, but also cites `update-v262144/manifest.json#inventory`, whose actual denominator is 68,330 functions and 19,029,816 bytes. That generated manifest is current only for its capped recorded inputs, not the fix2 inventory. | `update-main-full-pseudocode-export.md` §“Coverage — update-main NSO, denominator 53,106,320 executable bytes” (153,476 located; 153,471 C; 5 assembly; 1,830,644 bytes / 3.44713% not located), §“gameDB index” (153,471 files / 153,470 function rows), and §“Verification”; compare `manifest.json` `denominator`, `inventory`, and `gamedb` objects. The manifest status says `current` for its own recorded source fingerprints, but its source inventory hash/count are the capped generic inventory. |
| D3 | **The READMEs overstate why the remaining 3.45% is uncovered.** The completion table says the last 3.45% “needs emulator tracing.” Current fix2 evidence says 3.44713% of executable bytes are **not located** by that static inventory; it does not establish emulator tracing as the required or only next method. | `update-main-full-pseudocode-export.md` lines 112–123 explicitly distinguish located/exported/fallback/not-located byte shares. The fix2 report does not say the unlocated bytes require emulator tracing. Preserve the exact 3.44713% as not located in static inventory; leave the method/next work unclaimed until evidenced. |
| D4 | **Auxiliary export/index percentages imply known denominators.** README rows assign auxiliary modules 0% exported / 100% remaining and 0% indexed / 100% remaining, although complete per-module function denominators and export/index coverage are not established. The existence of Ghidra *detection* metadata is not a pseudocode export or a complete inventory. | `p0-update-aux-ghidra-inventory.md` §“Exact update module inputs and metadata detections” reports detections (33/13,531/5,523/8,262), explicitly not semantic/completeness; `p0-unexplored-modules.md` §4 says no complete reconciled function inventory/index, especially for `rtld`; `p0-aux-module-search.md` §§1–2 is a bounded historical repository search, not a global proof of absence. Use `—` for percentage/denominator unless an inventory denominator is directly qualified. |
| D5 | **The current changed-content classification report retains a contradictory 360 count.** Census/README say 336 mapped / 389 unmapped additions, but `p0-update-changed-content-classification.md` §3.1 says “360 mapped” while the table and per-directory counts sum to 336. | `p0-overlay-ownership.md` §4.3 independently reconciles 336 + 389 = 725, identifies the 360 statement as stale, and repeats the 336 total in `p0-unexplored-modules.md` §6 and `p0-wave1-reconciliation.md` §“Count and path checks”. Do not treat the 360 prose as current. |
| D6 | **Wave 1 summary closure count does not match its closure table.** The summary reports six closed bounded claims, but the “Closed evidence” table lists only C1–C5. | `p0-wave1-reconciliation.md` lines 11–17 say six; its table lines 88–95 contains exactly C1 through C5. Either add the sixth scoped claim and direct evidence, or change the summary count to five. This is a report-internal count discrepancy, not evidence that another P0 gate closed. |
| D7 | **Several older P0 reports still make already-superseded statements.** Their historical limitations are useful, but unqualified reuse would conflict with the current census/README/PLAN. | `p0-base-aux-ghidra-inventory.md` §§1–3 still calls module mappings/candidates and later auxiliary binaries unresolved; `p0-base-update-overlay.md` §§1, 4 still calls update `main.npdm` unknown and base provenance unresolved; `p0-update-main-dynamic-metadata.md` §“Method and limits” still says base provenance/overlay unresolved; `p0-update-changed-content-classification.md` §3.1 has the D5 count. Superseding evidence: `p0-base-v0-module-provenance.md` §§1–6, `p0-ownership-and-npdm.md` §2, `p0-base-import-relocation-inventory.md` §§1–5, `p0-overlay-ownership.md` §§2, 4.3, and `p0-nca-npdm-analysis.md` §§3, 5–6. Preserve history; point to the superseding result. |

## Reconciled current facts (not discrepancies)

- **Build/overlay:** the target remains base v0 + update v262144. Update is a
  patch, not a standalone executable. Effective RomFS is 17,904 unchanged / 466
  changed (465 size-different + 1 same-size) / 725 added / 0 removed; the 1,191
  delta entries are externally classified only.
- **Added subsystem label split:** 336 mapped / 389 unmapped additions. These are
  heuristic name/path matches, not semantic ownership. Semantic ownership and
  runtime use remain unknown for all 1,191 changed/added entries; internal
  analysis is 0/466 modified entries.
- **Auxiliary modules:** `rtld`, `sdk`, `subsdk0`, and `subsdk1` are extracted,
  NSO-verified, and byte-identical between base and update. Original-NSO
  structural imports/relocations are directly inventoried for all five base
  modules and all five update modules. Ghidra metadata detections exist for the
  four update auxiliaries, but are not complete semantic inventories. Pseudocode
  export and gameDB index percentages for the auxiliaries have no established
  denominators in the reviewed evidence.
- **Update `main`:** fix2 locates 153,476 functions (153,471 C exports + 5
  assembly fallbacks), covering 96.55287% of the 53,106,320 executable bytes;
  3.44713% is not located. The export covers 96.47728% of bytes in C and 0.07559%
  in assembly fallback. 153,471 files / 153,470 function rows are in the fix2
  gameDB. These are export/index measures, not understanding or semantic coverage.
- **Function ledger:** the 22 rows remain scoped to selected update-`main`
  functions. `analysis_status`, implementation, behavior verification, and
  binary matching remain independent; most states are unknown, and partial cache
  evidence is bounded to its declared scenarios. Its counts must not be
  generalized to P0 scope closure.

## Exact proposed integration edits (not applied)

| Target | Proposed edit |
|---|---|
| `sheets/re/p0_scope_census.tsv`, `base_main_identity` evidence field | Replace the non-navigable `reports/function-progress/p0-wave1-reconciliation.md#C3` with `reports/function-progress/p0-base-import-relocation-inventory.md#per-module-counts` (the census already has this direct report link; remove the broken extra citation or use the reconciliation report path without `#C3`). Keep all residual runtime/semantic unknowns. |
| `sheets/re/p0_scope_census.tsv`, `base_main_candidate_pin_check` evidence field | Replace `p0-wave1-reconciliation.md#C2` with `p0-base-v0-module-provenance.md#3-version-qualified-base-module-identity` (already present), or remove the unusable row-label fragment. Keep the non-reproducible historical mismatch explicitly superseded. |
| `sheets/re/p0_scope_census.tsv`, `base_package_contentmeta` evidence field | Replace `p0-wave1-reconciliation.md#O4` with the direct file reference `reports/function-progress/p0-nca-npdm-analysis.md`; keep NCA authenticity and update ContentMeta/CNMT semantics unknown/blocked. |
| `sheets/re/p0_scope_census.tsv`, `update_main_identity` evidence field | Add direct fix2 citations to `reports/function-progress/update-main-full-pseudocode-export.md` (coverage, gameDB index, and verification sections). Do not use the capped generated manifest as the citation for current fix2 counts; retain it only as an explicit historical/capped-view reference. |
| `README.md` and `README.es.md`, Completion plan row “main update” under pseudocode export | Replace “3.45% ... needs emulator tracing” with “3.44713% of executable bytes not located in fix2 inventory; export coverage 96.47728% C + 0.07559% assembly fallback.” Keep 100% inventory functions exported as a separate, accurately scoped metric only if phrased as 153,471 C + 5 assembly of 153,476 located. |
| `README.md` and `README.es.md`, auxiliary rows under pseudocode export and gameDB index | Change `0% / 100%` to `— / —`; note “No qualified complete per-module denominator; no pseudocode-export/index coverage established.” Keep the separate inventory row's detection counts explicitly as detections, not complete function inventory. |
| `reports/function-progress/p0-update-changed-content-classification.md` §3.1 | Correct “360 added” to “336 added”; add an erratum/superseding note if preserving audit history is preferred. Its table, overlay-ownership report, census, and README already support 336. |
| `reports/function-progress/p0-wave1-reconciliation.md` status summary | Reconcile six-vs-five: either document a sixth scoped closed claim with direct evidence, or change the summary to five. Do not alter the 10 open categories or P0-open verdict without new evidence. |
| Older P0 reports cited in D7 | Add short dated superseding pointers to the current reports; do not erase the old observation, claim runtime/semantic closure, or re-extract `main.npdm`. |

No change is proposed to the 22 function-progress evidence rows solely because
P0 metadata/inventory reports advanced. The report does not find evidence to
promote those function states.

## P0 closure gate assessment

**P0 is not closed.** Scoped evidence is closed for package/build identity and
bounded module universe; base-v0 provenance and the five-base-NSO structural
import/relocation inventory; effective overlay arithmetic/classification; and
the update `main.npdm` disposition plus bounded static field comparison. The
following remain open or blocked: loader binding/provider identity and loaded or
reached status; complete semantic/function inventories and broader scope;
NCA header/signature verification and update ContentMeta/CNMT semantics; internal
analysis of 466 modified entries; provenance of 725 additions; file-to-code
ownership/runtime access; asset/script denominator reconciliation; and current
fix2 regeneration of the progress map. The reports explicitly state no current
runtime trace. Export, index, name matches, or metadata counts cannot close these
criteria.

## Blockers and next bounded task

**Blockers:** current Wave 2 runtime report says guest execution was blocked
before launch because the launchable Suyu executable, loader-ready NSP pair, and
trace instrumentation were unavailable. NCA verification separately lacks an
authorized header-decryption/verifier configuration; update ContentMeta/CNMT
semantics remain unknown. No Ghidra or index operation was run for this audit.

**Next bounded task:** have the single integration writer apply only the
metadata/documentation corrections listed above, validating every link and
keeping README translations synchronized. Keep P0 open. Separately schedule a
bounded fix2 progress-artifact regeneration against the fix2 inventory and
current ledger, with the required manifest/source-fingerprint verification;
that generator task is not part of the canonical TSV/README integration pass.

## Work boundary

This task wrote only this audit report among report outputs and appended only
`work/progress/p0-ledger-audit.log`. A separate untracked file,
`reports/function-progress/p0-update-main-residual-export.md`, appeared in the
final status check; it is outside this task and was not read, edited, or removed.
Canonical census, function ledger, plan, READMEs, and other reports were not
edited. No Ghidra process, `gamedb index`, commit, or push was performed.

## Superseding addendum — current fix2 progress-map regeneration (2026-10-06)

This addendum supersedes only the current-map regeneration status recorded in
O3 and the associated blocker/next-task wording in this audit. The separate
`update-v262144-fix2` profile has now been regenerated successfully. Its
`generation-status.json` is current, and its manifest matches the exact pinned
update archive and fix2 inventory as well as the current Rust-source
fingerprint; see [the profile report](p0-fix2-treemap-profile.md) and
[manifest](update-v262144-fix2/manifest.json). The profile covers the 153,476
functions located in the fix2 update-`main` inventory only, not whole-game
coverage or a complete valid-function denominator. It records 22 documented/
analyzed, 8 partial implementations, 0 behavior-verified, and 0 binary-matched
functions. No function state was promoted by generation.

All other audit findings, historical counts, and P0 closure gates remain as
previously recorded: semantic/function-scope completeness, runtime/binding,
NCA/ContentMeta, overlay comparisons/provenance, ownership, and the other
identified work remain open or blocked. This addendum does not claim P0 closure.

## Superseding addendum — Wave A execution and next local queue (2026-10-06)

This addendum supersedes the older next-task wording above that proposed
regenerating the fix2 profile; that profile is current and its manifest matches
the pinned archive, fix2 inventory, ledger, and Rust-source fingerprint. It also
records that the successful global gameDB index covers the currently available
six-root C corpus; do not run another index command for this queue.

- Update-main reference triage used executable PT_LOAD `[0,0x32a5690)` minus
  exact existing function bodies, yielding 23,597 ranges / 1,830,644 bytes.
  The scan visited 2,830,748 reference records; complement-connected edges
  totaled 44,832 inbound and 2,667 outbound. Stored references do not prove
  reachability, valid functions, semantics, behavior, or ownership. See
  [PT_LOAD strata](p0-update-main-gap-reference-strata.md).
- Auxiliary read-only scans found a listing-method discrepancy: many Data
  CodeUnits had `Data.isDefined() == false`, unlike the prior range report's
  classification of all gap Data as defined. This is unresolved API-method
  reconciliation, not evidence that body unions changed. Direct CALL/JUMP
  references identify candidates only. Fifteen sequential Ghidra invocations
  (peak concurrency 1, including failed script compiles) left source hashes
  stable. See [auxiliary triage](p0-auxiliary-gap-reference-triage.md).
- AHTB's ten rejects all overrun at entry 26 (276 declared, 173 remaining,
  103-byte overrun); no alternate format rule is evidenced, so strict parsing
  remains. GFLXPACK had no fresh direct validation because exact virtual inputs
  were unreadable; prior 6/10 rejects remain. See the [AHTB](p0-ahtb-variant-reassessment.md)
  and [GFLXPACK](p0-gfpak-variant-reassessment.md) reports.
- The census has 21 records. O3 (bounded fix2 artifact generation) is resolved;
  O1, O2, and O4–O10 remain open. Five ready local lanes are O2 scope/inventory
  reconciliation, O7 modified-overlay reconciliation, O8 added-path provenance,
  O9 static ownership candidates, and O10 scope/census audit. O4 NCA, O5 CNMT,
  and O1/O6 runtime/NPDM-necessity proof remain blocked on prerequisites. The
  [Wave 7 audit](p0-wave7-scope-and-ownership-plan.md) is the queue source.

No function-level evidence rows or states were changed. Reference/Ghidra
metadata, parser acceptance, and static candidates are not semantic function,
ownership, or behavior evidence; P0 remains open.

## Superseding addendum — Wave 7 findings integrated into the active queue (2026-10-06)

This addendum records newer reference-strata and scope/ownership follow-up
evidence. It does not change the 21-record census, the function evidence ledger,
or any P0 closure gate.

- The corrected update-main complement reference scan explicitly used executable
  PT_LOAD `[0,0x32a5690)` minus exact current bodies and reproduced 23,597 ranges /
  1,830,644 bytes. It counted 44,832 inbound and 2,667 outbound edges connected
  to the complement. These are stored reference edges, not reachability or
  semantic evidence; use [the reference-strata report](p0-update-main-gap-reference-strata.md)
  alongside, not instead of, the semantic-query/listing retry.
- The auxiliary listing pass separately checked `Data.isDefined()` and found
  most gap Data CodeUnits reported as undefined-type, in conflict with the
  earlier all-defined Data/Instructions classification in
  [range reconciliation](p0-auxiliary-range-reconciliation.md). Preserve both
  results as an unresolved method/classification discrepancy. The pass confirms
  existing body unions, not semantic meaning; see [auxiliary reference triage](p0-auxiliary-gap-reference-triage.md).
- AHTB remains 10/10 rejected at entry 26 with a 103-byte overrun and no safe
  alternate rule. GFLXPACK had no readable fresh virtual inputs; prior 6/10
  rejections remain. Neither parser changed ([AHTB](p0-ahtb-variant-reassessment.md),
  [GFLXPACK](p0-gfpak-variant-reassessment.md)).
- The active local queue is O2/O7/O8/O9/O10. O8's group/extension cross-tab is
  not reproducible (0/11 groups, 0/5 extensions); O9's Ghidra query returned no
  counters, so its result is Unknown, not zero. O10 found 21 census records but
  no complete scope/method/exclusion accounting. O4/O5 and O1/O6 remain
  prerequisite-gated. See [overlay matrix](p0-wave8-overlay-coverage-matrix.md),
  [added provenance cross-tab](p0-wave8-added-provenance-cross-tab.md),
  [static candidates](p0-wave8-static-file-code-candidates.md),
  [census audit](p0-wave8-census-audit.md), and
  [full-scope audit](p0-wave8-full-scope-audit.md).

P0 remains open. No function-level evidence state is advanced; current fix2
generation and global available-C indexing do not establish semantic or scope
closure.

## Superseding addendum — Wave 9 local-lane integration (2026-10-06)

This addendum updates the earlier O9 no-counter outcome and records the current
local-lane references. The canonical census remains at **26 records**; no
function evidence rows or states, Rust sources, parsers, fix2 artifacts, or
global index were changed.

- **O9:** one read-only, `-noanalysis` Ghidra run matched **118 string data
  units** against the effective inventory. It produced **0 exact-path xrefs**
  and **97 normalized-only direct xrefs across 52 distinct existing functions**.
  These are static candidates only: no file-open call, delta membership,
  semantic ownership, runtime access, or reachability was established. This
  successful report supersedes the older Wave 8 attempt's unavailable counters
  for the current query; retain that report as history. See
  [Wave 9 triage](p0-wave9-static-file-code-triage.md) and
  [Wave 8 attempt](p0-wave8-static-file-code-candidates.md).
- **Active local lanes:** O2 cites the [function-scope matrix](p0-function-scope-matrix.md)
  and both gap-reference reports; O7 cites the [overlay coverage matrix](p0-wave8-overlay-coverage-matrix.md)
  and strict AHTB/GFLXPACK reassessments; O8 cites the [added-provenance
  cross-tab](p0-wave8-added-provenance-cross-tab.md), whose group/extension
  cross-tab remains unavailable; O9 uses the Wave 9 report above; O10 uses the
  [Wave 9 inclusion matrix](p0-wave9-scope-inclusion-matrix.md) alongside the
  prior census audits. O2/O7/O8/O9/O10 remain local/next lanes, not closed P0
  gates.
- The auxiliary `Data.isDefined()` classification discrepancy remains
  unresolved. No safe AHTB/GFLXPACK variant decision exists; parser bounds stay
  unchanged. B1/B2 and O1/O6 remain blocked on their stated prerequisites.
- The fix2 treemap remains `current` for its pinned update-main located-function
  inventory only, and the global gameDB index remains current only for available
  exports; neither is semantic or whole-game coverage.

P0 remains open. No function-level evidence state is advanced by this
integration.

## Superseding addendum — user-directed bounded P0 scope (2026-10-06)

This addendum supersedes prior active-gate wording in this audit that made NCA
signature/authenticity, update ContentMeta/CNMT semantics, runtime binding or
NPDM necessity prerequisites for P0. It records a scope decision, not new
cryptographic or runtime evidence. **P0 remains in progress** until the local
baseline gates below pass.

### P0 closure claim boundary

P0 may close only as a bounded metadata/inventory baseline for declared base v0
+ update v262144: package and Program ExeFS identities; executable-module roles
with structural import inventory; effective outer overlay accounting; and
bounded file-group/parser states. The closure will not claim NCA authenticity,
complete update CNMT semantics, full semantic understanding, a complete valid-
function denominator, file-to-code ownership, runtime behavior, whole-game
completeness, or port parity. The update remains a patch; the required target is
base plus update, not the update alone.

### Verified state versus deferral

- Update `main.npdm` is already extracted, hash-verified, and field-compared in
  bounded static scope. Do not re-extract it. Its cryptographic trust and runtime
  necessity remain Unknown/Deferred; no authenticity claim follows from the
  field comparison.
- NCA header/signature authenticity is **not verified** and is Deferred outside
  P0. Update ContentMeta/CNMT semantics remain **not verified**; no update
  ContentMeta parse is claimed and no base CNMT values are substituted.
- Loaded/reached dependency state and runtime behavior are Deferred to P2. No
  runtime trace is claimed. User-reported emulator use, if mentioned elsewhere,
  is not independent trace evidence and is not part of the closure case.
- Overlay remains 19,095 effective outer entries (17,904 unchanged, 466
  modified, 725 added, 0 removed). Internal structural comparisons are 14/466
  successful and 452/466 without successful comparison; added-file hashes are
  33 exact base matches and 692 non-matches, which do not prove newness.
  Semantic ownership is Unknown/Deferred for 1,191/1,191 delta entries.
- The 1,830,644-byte update-main residual and auxiliary gaps remain separate
  scoped unknowns, not invented function rows. Valid-function denominators remain
  Unknown. The current fix2 profile and global available-C index need no update
  here; no gameDB reindex is planned and no treemap change is required unless the
  ledger or Rust sources change.
- The auxiliary `Data.isDefined()` discrepancy is unresolved listing-method
  variance, not a P0 stop while body ranges/import metadata remain stable.

### Final local P0 gates

1. Complete the census discovery boundary, inclusion/exclusion rationale and
   stopping rule across package roots, Program ExeFS members, effective outer
   RomFS files, module roles, scripts/assets/other data, and nested-member limits.
2. Reconcile static module/dependency/export/index evidence; preserve Unknown
   function denominators and keep byte residuals/gaps separate from function
   counts.
3. Reconcile effective overlay and per-format/parser states without treating
   parser success as semantics or summing overlapping parser cohorts.
4. Assign every in-scope row Known, Unknown, or Deferred with evidence, reason,
   limit, and later phase; keep deferred validation explicitly outside P0.
5. Pass `cargo run -p sheetty-cli -- check sheets` and `git diff --check`.

Only after all five gates pass may P0 close as this bounded baseline. Until then
P0 remains in progress. No function-ledger states, Rust sources, gameDB index,
or treemap artifacts are changed by this scope integration.
