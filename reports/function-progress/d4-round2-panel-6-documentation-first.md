# D4 round-2 panel 6 — documentation-first batch integration

**127,683/153,476 located update-main functions now carry static documentation markers (83.20%).** Panel 6 integrates 204 batch manifests covering its full assigned range (`016a1f08`–`01ef1790`, 9,740 queue functions, ~48 per batch): 8,229 complete-body records admitted as `analyzed_documented`, 1,308 withheld candidates explicitly recorded as `unknown`, and 183 addresses skipped because the evidence ledger already carried a row for them. This is static operation documentation only — not game-purpose interpretation, types (N2), implementation, behavior verification or binary matching.

## Integration evidence gates

- Admission required all three manifest gates (`accepted=true`, `audit_status=accepted`, `validation_status=passed`) with queue corroboration in `work/d4/tasks/panel-6-queue.json`. Corroboration: queue `documented` agrees with the manifests for all 8,229 admitted records; the queue's 1,416 `assigned` entries are a stale pre-work snapshot, so the manifest gates decided admissibility for the 1,308 records recorded as `unknown`, recorded explicitly as a queue staleness reconciliation and not as a silent substitution. Every withheld panel-6 record was audit-accepted but failed writer validation.
- Manifest gap recorded: 20 queue-assigned addresses (`017d49c8`, `017efc50`, `017f6878`, `0180c338`, `01877a4c`, `018aced8`, `018ee1e0`, `019e54b8`, `01a2b778`, `01b21ff0`, `01c1a47c`, `01c348b0`, `01c56008`, `01c69054`, `01ccf1a4`, `01ccf3bc`, `01cf1c08`, `01d28e10`, `01ddde80`, `01e2c388`) have no round-2 manifest record. Nothing was invented for them; the gap is recorded and their documentation remains unknown.
- 183 manifest addresses already carried earlier ledger rows (95 inside the manifest-accepted set, 88 withheld) and were skipped, so they carry no new marker. All prior rows are round-1 `unknown` states (decisions dec140–dec289, including round-1 panel-3-region work in the overlapping address region).
- Per-address hashes were joined to stored scratch bytes in `work/d4/scratch/panel-6/R{addr}-auto-01/attempt-01`: `raw_sha256` against the raw response bytes, embedded audit `fresh_raw_sha256`/`proposal_sha256` fields against both manifest hashes, and proposal reserialization hash against `proposal_sha256`. All 9,537 appended records verified on that core join. Zero records passed the ancillary `audit_sha256` reserialization check: panel-6's manifest audit hashes do not bind to the stored `audit.json` reserialization (the audit files still carry the audit decisions; the record binding is the embedded-field join). Per the round rule this ancillary non-binding is recorded, not treated as a rejection; 0 not joinable; 0 raw or embedded-field mismatch. No verified count was faked.
- Analysis status/evidence columns changed only; implementation, verification and binary-match columns remain `unknown`. No N2 register changes.

## Measured result

| Category | Current markers |
|---|---:|
| Complete-body analyst records | 58,262 + 8,229 (round-2 total across panels 2, 4, 5, 6, 7, 8, 9, 10) |
| Audited EXACT copies | 22,392 (unchanged; no propagation in any round-2 panel) |
| Deterministic mechanical | 28,308 (unchanged) |
| Import stubs | 801 (unchanged) |
| Total / still without marker | **127,683 / 25,793** |

Decision: `sheets/decisions.tsv#dec297`. Census and join details (metadata only): `work/d4/orchestrator/round2-panel-6-census.json`; integrator: `work/d4/orchestrator/integrate_round2.py`. The 1,308 withheld candidates' ledger rows state `unknown` bound to the same decision.

## Verification and continuation

- Verified integration, ledger append: 9,537 rows added (8,229 admitted markers plus 1,308 explicit unknowns; evidence-table line count grew accordingly, including sheet index rows).
- 204 manifests reconciled gate-by-gate with queue corroboration agreeing in every category. Batch ids `001`–`204`; last accepted address `01ef1790` (batch 204, the range end), last unknown `01ef0a40` (batch 204).
- `cargo run -p sheetty-cli -- check sheets` after ledger and decision rows: 0 errors, 0 warnings. Fix2 regeneration asserted the pinned `pk2.nsz` identity, the exact 153,476-function inventory and current source fingerprints, and the documented count (127,683) is taken live from the ledger.
- English/Spanish README figures synchronized. No purpose, type, implementation, runtime or matching claim.

Runtime harness: N/A, static documentation only. Rollback: the panel-6 integration commit; the ledger rows and decision row are the only product mutations, and no working copy, export or binary state changed.
