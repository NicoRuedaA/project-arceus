# D4 round-2 panel 4 — documentation-first batch integration

**102,115/153,476 located update-main functions now carry static documentation markers (66.53%).** Panel 4 integrates 306 batch manifests (batch ids `001`–`306`; batch 306 is a single supplemental record) covering its full assigned range (`00d52adc`–`011ca6b0`, 9,740 functions): 8,567 complete-body records admitted as `analyzed_documented`, 959 withheld candidates explicitly recorded as `unknown`, and 214 addresses skipped because the evidence ledger already carried a row for them (160 of those 214 sit inside the manifest-accepted set and therefore carry no new marker). This is static operation documentation only — not game-purpose interpretation, types (N2), implementation, behavior verification or binary matching.

## Integration evidence gates

- Admission required manifest `accepted=true`, `audit_status=accepted` and `validation_status=passed`. Queue corroboration was resolved explicitly, not silently: `work/d4/tasks/panel-4-queue.json` is a stale pre-work snapshot (every entry still `assigned`), so the manifest audit/validation gates decided admissibility and the panel status record (8,727 accepted / 1,007 withheld-failing / 6 decompile-failed subjects) agrees with the manifest gates. The stale-queue reconciliation category covers all 9,526 processed non-skipped subjects and is listed in the census; nothing was patched into the queue file. Gate outcome over the 9,740 unique addresses: 989 audit-withheld-with-passed-validation, 17 audit-accepted-but-writer-validation-failed and 7 fully withheld candidates stay unknown. No `Propagated` markers were added: identical-copy propagation did not happen in this round.
- Cross-panel overlap handling: the `panel-8-overlap-note.json` 16 addresses belong to panel-4 **round-1** scratch (`0258xxxx`, range not integrated in this round); the round-2 panel-4 range (`00d52adc`–`011ca6b0`) contains none of them, so nothing was dropped and the empirical check is recorded in the census. Panel-8 keeps winning that overlap.
- Per-address hashes were joined to stored scratch bytes in `work/d4/scratch/panel-4/R{addr}-auto-01/attempt-01`: `raw_sha256` against the raw response bytes, embedded audit `fresh_raw_sha256`/`proposal_sha256` fields against both manifest hashes, and proposal reserialization hash against `proposal_sha256`. 9,526 joined records verified on that core join. Zero records passed the ancillary `audit_sha256` reserialization check: panel-4's manifest audit hashes do not bind to the stored `audit.json` reserialization (the audit files still carry the audit decisions; the record binding is the embedded-field join). Per the round rule this ancillary non-binding is recorded, not treated as a rejection; 0 not joinable; 0 raw or embedded-field mismatch. No verified count was faked.
- Analysis status/evidence columns changed only; implementation, verification and binary-match columns remain `unknown`. No N2 register changes.

## Measured result

| Category | Current markers |
|---|---:|
| Complete-body analyst records | 42,047 + 8,567 (round 2 integrated across panels so far) |
| Audited EXACT copies | 22,392 (unchanged; no propagation in this round) |
| Deterministic mechanical | 28,308 (unchanged) |
| Import stubs | 801 (unchanged) |
| Total / still without marker | **102,115 / 51,361** |

Decision: `sheets/decisions.tsv#dec294`. Census and join details (metadata only): `work/d4/orchestrator/round2-panel-4-census.json`; integrator: `work/d4/orchestrator/integrate_round2.py`. The 959 candidates' ledger rows state `unknown` bound to the same decision with their gate details (`withheld/…`, `accepted/failed`).

## Verification and continuation

- Final part: verified integration, ledger append: 9,526 rows added (8,567 admitted markers plus 959 explicit unknowns; evidence-table line count grew accordingly, including sheet index rows).
- Per-panel manual checks: 306 manifests reconciled gate-by-gate. Highest batch processed `306`; last accepted address `011ca6b0` (batch 305, the range end), last unknown `011c8728` (batch 305).
- `cargo run -p sheetty-cli -- check sheets` after ledger and decision rows: recorded in the round log. Fix2 regeneration asserted the pinned `pk2.nsz` identity, the exact 153,476-function inventory and current Rust-source fingerprints before committing.
- English/Spanish README figures and the documentation-queue round note synchronized. No purpose, type, implementation, runtime or matching claim.

Runtime harness: N/A, static documentation only. Rollback: the panel-4 integration commit; the ledger rows and decision row are the only product mutations, and no working copy, export or binary state changed.
