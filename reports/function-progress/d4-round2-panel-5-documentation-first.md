# D4 round-2 panel 5 — documentation-first batch integration

**93,548/153,476 located update-main functions now carry static documentation markers (60.95%).** Panel 5 integrates 610 batch manifests covering its full assigned range (`011ca750`–`016a1ee4`, 9,740 functions): 8,542 complete-body records admitted as `analyzed_documented`, 1,070 withheld/error candidates explicitly recorded as `unknown`, and 128 addresses skipped because the evidence ledger already carried a row for them (79 of those 128 sit inside the manifest-accepted set and therefore carry no new marker). This is static operation documentation only — not game-purpose interpretation, types (N2), implementation, behavior verification or binary matching.

## Integration evidence gates

- Admission required manifest `accepted=true`, `audit_status=accepted` and `validation_status=passed`. Queue corroboration was resolved explicitly, not silently: `work/d4/tasks/panel-5-queue.json` is a stale pre-work snapshot (every entry still `assigned`), so the manifest audit/validation gates decided admissibility; this is recorded as a queue staleness reconciliation in the census, and nothing was patched into the queue file. Gate outcome over the 9,740 unique addresses: 9,612 not already recorded, of which 8,542 passed all three gates; 1,088 audit-withheld-with-passed-validation, 26 audit-accepted-but-writer-validation-failed, 4 pipeline-error (`error/error`) and 1 fully withheld candidates stay unknown. No `Propagated` markers were added: identical-copy propagation did not happen in this round.
- Per-address hashes were joined to stored scratch bytes in `work/d4/scratch/panel-5/R{addr}-auto-01/attempt-01`: `raw_sha256` against the raw response bytes, embedded audit `fresh_raw_sha256`/`proposal_sha256` fields against both manifest hashes, and proposal reserialization hash against `proposal_sha256`. 9,608 joined records verified on that core join. Zero records passed the ancillary `audit_sha256` reserialization check: panel-5's manifest audit hashes do not bind to the stored `audit.json` reserialization (the audit files still carry the decision contents; the record binding is the embedded-field join). Per the round rule this ancillary non-binding is recorded, not treated as a rejection; 4 pipeline-error subjects have no attempt directory at all (not joinable). 0 raw or embedded-field mismatch. No verified count was faked.
- Analysis status/evidence columns changed only; implementation, verification and binary-match columns remain `unknown`. No N2 register changes.

## Measured result

| Category | Current markers |
|---|---:|
| Complete-body analyst records | 33,505 + 8,542 (round 2 integrated across panels so far) |
| Audited EXACT copies | 22,392 (unchanged; no propagation in this round) |
| Deterministic mechanical | 28,308 (unchanged) |
| Import stubs | 801 (unchanged) |
| Total / still without marker | **93,548 / 59,928** |

Decision: `sheets/decisions.tsv#dec293`. Census and join details (metadata only): `work/d4/orchestrator/round2-panel-5-census.json`; integrator: `work/d4/orchestrator/integrate_round2.py`. The 1,070 candidates' ledger rows state `unknown` bound to the same decision with their gate details (`withheld/…`, `error/…`).

## Verification and continuation

- Final part: verified integration, ledger append: 9,612 rows added (9,514 markers cumulative this round after panel 5; evidence-table lines grew accordingly, including sheet index rows).
- Per-panel manual checks: 610 manifests reconciled gate-by-gate (batch ids `001`–`610`; batch 610 contains 9 admitted and 1 withheld record). Highest batch processed `610`; last accepted address `016a1ee4` (batch 609, the range end), last unknown `0169d630` (batch 606).
- `cargo run -p sheetty-cli -- check sheets` after ledger and decision rows: recorded in the round log. Fix2 regeneration asserted the pinned `pk2.nsz` identity, the exact 153,476-function inventory and current Rust-source fingerprints before committing.
- English/Spanish README figures and the documentation-queue round note synchronized. No purpose, type, implementation, runtime or matching claim.

Runtime harness: N/A, static documentation only. Rollback: the panel-5 integration commit; the ledger rows and decision row are the only product mutations, and no working copy, export or binary state changed.
