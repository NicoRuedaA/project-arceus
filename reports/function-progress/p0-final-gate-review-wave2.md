# P0 bounded closure gate review — wave 2

**Verdict: FAIL — do not close P0 yet.** The bounded metadata/inventory scope
is accepted; this is not a request to resolve its explicitly deferred Unknowns.
Gates 1–3 pass. Gate 4 has one conflicting active-looking summary; Gate 5 lacks
post-documentation-fix evidence for the required local checks.

## Gate results

| Gate | Result | Evidence and finding |
|---|---|---|
| 1. Scope boundary, discovery, and inclusion/exclusion rules | **PASS** | The current PLAN addendum (lines 432–468) declares a finite boundary: pinned base/update package roots and recorded members, six Program/ExeFS members per version, five version-qualified NSO roles and original-NSO structural metadata, the effective outer RomFS set, and already-recorded outer-file classes/parser and module inventory metrics. It states the stopping rule and that unenumerated categories/nested members remain Unknown, not excluded. The 26-row census and final bounded-scope matrix provide evidence references and do not claim an exhaustive game-wide discovery universe. |
| 2. Static module/inventory/export/index metrics and denominator Unknowns | **PASS** | The census and both READMEs distinguish identity and structural inventories from partial detections/exports and available-C indexing, retain Unknown complete valid-function denominators, and avoid treating pseudocode/index rows as semantics. The final successful index addendum reports 177,795 C files, 176,667 parsed function rows, 1,128 files without a parsed row, 83,373 strings, 176,667 symbols, 48,991,899 edges, and selftest 87/87; it bounds those counts to the six available C-export roots and excludes five ASM fallbacks. Fix2 remains scoped to 153,476 located update-main functions and separately records the 1,830,644-byte residual. |
| 3. Overlay arithmetic and per-format state | **PASS** | The active documents agree on 19,095 effective outer files = 17,904 unchanged + 466 modified + 725 added + 0 removed; 14/466 successful internal comparisons and 452/466 without success; and 33 exact added-file matches plus 692 non-matches (not proof of newness). Parser/compile cohorts remain separate and overlapping. Unsupported/rejected and untested cases are labeled, while semantic ownership (1,191/1,191), nested-member scope, and effective per-file script/asset denominators remain Unknown rather than being inferred from parser results. |
| 4. No hidden Unknowns in the declared universe | **FAIL** | The PLAN, census, and closure matrix explicitly assign reasons/limits/next-phase treatment to the important Unknowns and state that Unknown is not exclusion. However, the README completion-plan row for “3. Pseudocode export (prep) — main update” still says the next step is independent semantic gap/candidate triage. The matching Spanish row says the same. This conflicts with the latest bounded-closure decision in PLAN and both README scope sections, which explicitly defer semantic gap classification and candidate validity outside P0. The stale wording is not labeled historical/superseded and leaves an active-looking P0 task in the declared status source. |
| 5. Current fix2/index fingerprints and sheetty/CI checks | **FAIL** | Fix2 `generation-status.json` says `current`; its manifest identifies the pinned archive/inventory, evidence-ledger digest, Rust-source fingerprint, and available gameDB snapshot. The global-index report’s final addendum records successful indexing and its scoped results. But the latest closure matrix explicitly says sheetty, CI, and fresh fingerprint recomputation were not run; the inspected CI log only records an earlier exit code 0, and earlier sheetty/diff-check logs predate the latest documentation synchronization. The prior matrix’s `git diff --check` covered tracked changes only. Therefore the required current-check evidence is not established for this review. No checks were run because the request was read-only. |

## Remaining local gates

1. **Gate 4:** reconcile or clearly mark as superseded the README and README.es.md
   export-row “Next” sentence that makes semantic gap/candidate triage look like
   active P0 work. No semantic triage itself is required for bounded P0 closure.
2. **Gate 5:** after documentation is settled, record successful current
   `cargo run -p sheetty-cli -- check sheets`, `git diff --check`, and configured
   CI results; validate the fix2/index fingerprints against their recorded
   inventories. This review did not execute those checks or recompute hashes.

## Explicitly not blockers

NCA signature authenticity and update CNMT semantics; NPDM cryptographic trust
or runtime necessity; loaded/reached state; semantic file-to-code ownership;
whole-game semantics; complete valid-function denominator; nested-member
semantics; and unresolved overlay comparisons/provenance remain explicit
Unknown/Deferred items under the user-approved bounded definition. The reported
emulator use is not a build-qualified runtime trace.

## Review limits

Read-only review of the current bounded PLAN addendum, README.md/README.es.md,
the 26-data-row census, the global-index report opening and final successful
addendum, the fix2 manifest/status, the closure matrix, and documented CI/local
check records. No package manifests, keys, payloads, Ghidra/gameDB commands,
sudo, or canonical-file edits were used. Only this report and its progress log
were written.
