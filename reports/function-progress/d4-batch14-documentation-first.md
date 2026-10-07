# D4 batch 14 — documentation-first continuation

**47,651/153,476 located update-main functions have static documentation markers (31.05%).** This round adds 28 complete-body analyst records and 140 independently gated EXACT-copy records; 0 previous marker(s) were withdrawn. Net gain: **168**. The game target remains the base + update overlay; this is not whole-game coverage, typed recovery or runtime verification.

## Evidence gates and scope

- User priority is [documenting directly observed operations first](d4-documentation-queue.md), not guessing game purpose. Low copy yield is not a stop condition.
- Read-only Luna authors and separate Luna auditors bind exact summary text to complete fresh live bodies. The writer validates imports/constants, completeness, unresolved calls and the exact audited subject. No names or signatures are proposed or applied.
- EXACT identity was recomputed for every member of the selected groups, preserving local layout and absolute external call/data targets. Representative live bytes match the original ELF. No SHAPE propagation or export-only promotion.
- Consistent pre-mutation working-project backup: `PLA-update-work-preR14-astra`. The writer saved and closed only the working project, copied its `.gpr`/`.rep` pair without overwrite, verified every file digest, then reopened the same project. Reference project untouched; one MCP service, no parallel indexing.
- 168 comments were written and read back exactly, then saved. Ledger analysis states changed only for admitted documentation or explicit withdrawal; implementation, behavior and binary-match columns remain unchanged. N2 register untouched.

## Measured result

| Category | Current markers |
|---|---:|
| Individually read | 1,368 |
| Audited EXACT copies | 17,174 |
| Deterministic mechanical | 28,308 |
| Import stubs | 801 |
| Total / still without marker | **47,651 / 105,825** |

Decision: `sheets/decisions.tsv#dec139`. Private evidence: `work/d4/r14/gates.json`, `exact-gate.json`, `apply-plan.json`, `backup.log`, `apply.log` and the round's proposal/audit/validation files. These retain operation descriptions privately; public artifacts publish only metadata and evidence references.

## Withheld evidence

4 examined candidates remain unknown: `00d234d0`, `026653b0`, `00566708`, `00573284`. Their audit/validation records explain the unresolved evidence; no copies are admitted from these candidates. See `work/d4/r14/withheld.json`.

One candidate (`00d234d0`) needs a corrected branch condition independently checked. Three otherwise audited candidates (`026653b0`, `00566708`, `00573284`) failed the writer's import-completeness gate; their omitted imports were not silently inferred or repaired. All four stay unknown.

## Verification and continuation

- `cargo run -p sheetty-cli -- check sheets`: exit 0; 30 sheets, 315,734 rows, zero errors/warnings.
- Ledger comparison: 168 new markers (28 reads, 140 copies), four additional unknown records, no prior status change. All non-analysis columns and the N2 register are unchanged; all 28 exact gate subjects match their audited hashes.
- Fix2 regenerated successfully; pinned archive identity, exact inventory, current 49-file Rust fingerprint and all ledger/output/status digests verified; state `current`. No fresh archive cryptographic audit.
- Markdown structural readback and `git diff --check -- "*.md"` passed. Logs: `work/d4/r14/sheet-check.log`, `check.json`, `manifest-check.log`, `regenerate.log`.
- Round 2 of four is complete. R15 readers and R16 read-ahead overlap these checks; one new reader is kept active while the other audits preceding work.

Runtime harness: N/A, static documentation only. Rollback is this round's ledger/decision/report/README/plan/projections plus restoration of its verified separate working-project backup if comments must be reverted. No Rust, types, game-system mapping, boundary, reference-project, push or RDD changes.
