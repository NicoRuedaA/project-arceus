# P0 final bounded-closure gate review

**Verdict: READY TO CLOSE BOUNDED P0.** The five local gates pass for the
user-approved metadata/inventory baseline, with the explicit freshness and
attestation limits below. This is not a claim of authenticity, runtime tracing,
semantic ownership, whole-game semantics, or a complete valid-function
denominator.

## Gate results

| Gate | Result | Independent read-only finding |
|---|---|---|
| 1. Finite scope boundary, discovery, and inclusion/exclusion | **PASS** | The current PLAN addendum declares the stopping boundary over the pinned base/update package identities and recorded roots, six reviewed Program/ExeFS members per version, five version-qualified NSO roles and their original-NSO structural metadata, the effective outer RomFS set, and the already recorded outer-file format/parser and module-inventory metrics. The stopping rule reconciles those finite sets and records each census dimension with evidence, reason, limit, and next phase. The 26 census records and Wave 9's 14-dimension checklist are not treated as completeness denominators. Nested members and categories outside the declared finite set remain Unknown, not excluded. |
| 2. Static identities/imports/inventory/export/index | **PASS** | The current reports distinguish structural identities/imports from function detections, exports, C-index files, parsed rows, and body-byte measures. The global index reports 177,795 available C files, 176,667 parsed function rows, 1,128 files without a parsed row, and 87/87 selftests across six roots; five update-main ASM fallbacks are outside that C-only corpus. Fix2 covers 153,476 located update-main functions and 51,275,676 body bytes, with 1,830,644 bytes outside existing bodies. Complete valid-function denominators remain Unknown where not established; no inventory, export, index, or parser result is presented as semantic coverage. |
| 3. Overlay arithmetic and per-format parser states | **PASS** | The current PLAN, READMEs, census, and overlay matrix reconcile 19,095 effective outer entries = 17,904 unchanged + 466 modified + 725 added + 0 removed. They preserve 14/466 successful internal comparisons and 452/466 without a successful comparison, and distinguish 33 exact base-hash matches from 692 non-matches among additions. Parser/compile cohorts are separate, overlapping, and have their own denominators and limits; no parser outcome or hash non-match is promoted to semantic ownership or proof of newness. |
| 4. No hidden Unknowns within the declared boundary | **PASS** | The census, current PLAN, and both READMEs keep residual dimensions status-labeled and bounded, including effective script/asset denominators, nested members, auxiliary inventory/method variance, the main-body residual, function semantics, and ownership. Each is retained as Unknown/Deferred with a cited reason, evidence limit, and later-phase treatment rather than silently treated as zero, absent, or excluded. Historical broad work queues in the bounded-closure audit, final scope matrix, and Wave 9 matrix are superseded by the later PLAN addendum and Wave B/C closure decision; they are not current P0 tasks. |
| 5. Current fix2/global-index status, fingerprints, sheetty, diff-check, CI | **PASS WITH DOCUMENTED LIMIT** | The newest fix2 generation status says `current` and its manifest SHA-256 matches the status record. Manifest metadata agrees with the sanitized profile on the pinned archive name/hash/size, inventory SHA, ledger SHA, and 49-file Rust fingerprint; the global-index count and fix2-scoped gameDB snapshot identity are consistent with their reports. The latest recorded Wave C run reports sheetty 29 sheets / 267,974 rows / 0 errors / 0 warnings, `git diff --check` exit 0, and configured `./.tools/ci.sh` exit 0. Its final post-edit readback records matching permitted input fingerprints and no required regeneration. Per the stated policy, this review did not recompute hashes, inspect package manifests, or re-run checks; the archive SHA and manifest contents are not freshly attested by this review. |

## Limits retained at closure

- P0 closes only as a bounded metadata/inventory/provenance baseline for base
  v0 plus update v262144; the update is a patch and the port target remains the
  base-plus-update overlay.
- NCA signature authenticity, update CNMT semantics, NPDM cryptographic trust
  and runtime necessity, loaded/reached state, semantic ownership, whole-game
  semantics, nested-member semantics, and the complete valid-function
  denominator remain Unknown/Deferred, not verified and not P0 gates.
- User-reported emulator use is context only; no instrumented runtime trace is
  claimed. The 452 comparisons without success and 692 added-file hash
  non-matches remain explicitly limited results, not reasons to expand this
  bounded closure.
- No Ghidra/gameDB command, generation command, or verification command was run
  for this review. No canonical source, README, PLAN, or census was changed.

## Evidence reviewed

Current bounded-scope addendum and status in `odd/PLAN.md`; `README.md`,
`README.es.md`; the 26-record `sheets/re/p0_scope_census.tsv`; bounded closure
audit, final bounded-scope matrix, prior wave-2 gate review, final static
inventory gate, fix2 profile, global index report, Wave 8 overlay matrix, and
Wave 9 inclusion matrix; fix2 `manifest.json` metadata and
`generation-status.json`; and `work/progress/p0-wave-c.log` plus the previous
review progress log. No package-manifest JSON, keys, credentials, payloads, or
configuration files were inspected.
