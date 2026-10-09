# D4 round-2 panel 10 — documentation-first batch integration

**119,454/153,476 located update-main functions now carry static documentation markers (77.83%).** Panel 10 integrates 153 batch manifests covering its full assigned range (`02e3b364`–`032a2320`, 9,744 functions, ~32 per batch): 9,691 complete-body records admitted as `analyzed_documented`, 2 withheld candidates explicitly recorded as `unknown`, and 51 addresses skipped because the evidence ledger already carried a row for them. This is static operation documentation only — not game-purpose interpretation, types (N2), implementation, behavior verification or binary matching. This closes the round-2 ledger integration of panels 2, 4, 5, 7, 8, 9 and 10.

## Integration evidence gates

- Admission required all four gates together: manifest `accepted=true`, `audit_status=accepted`, `validation_status=passed`, and a `documented` queue entry in `work/d4/tasks/panel-10-queue.json`. Reconciliation was exact: queue `documented` agrees with the manifests for 9,691, queue `withheld` agrees for both candidates (`02ee6a54` and `02eea158`, multi-call routines whose writer validation failed and whose independent audit withheld them at low confidence; both belong to the main-image fixed ASM-export boundary set, so their bodies were never valid C exports for these gates), and no queue address lacks a manifest. No `Propagated` markers were added: identical-copy propagation did not happen and none is claimed.
- Per-address hashes were joined to stored scratch bytes in `work/d4/scratch/panel-10/R{addr}-auto-01/attempt-01`: `raw_sha256` against the raw response bytes, embedded audit `fresh_raw_sha256`/`proposal_sha256` fields against both manifest hashes, and proposal reserialization hash against `proposal_sha256`. 9,693 joined records verified on that core join. Zero records passed the ancillary `audit_sha256` reserialization check: panel-10's manifest audit hashes do not bind to the stored `audit.json` reserialization (the audit files still carry the audit decisions; the record binding is the embedded-field join). Per the round rule this ancillary non-binding is recorded, not treated as a rejection; 0 not joinable; 0 raw or embedded-field mismatch. No verified count was faked.
- Analysis status/evidence columns changed only; implementation, verification and binary-match columns remain `unknown`. No N2 register changes.

## Measured result

| Category | Current markers |
|---|---:|
| Complete-body analyst records | 58,262 + 9,691 (round-2 total across panels 2, 4, 5, 7, 8, 9, 10) |
| Audited EXACT copies | 22,392 (unchanged; no propagation in any round-2 panel) |
| Deterministic mechanical | 28,308 (unchanged) |
| Import stubs | 801 (unchanged) |
| Total / still without marker | **119,454 / 34,022** |

Decision: `sheets/decisions.tsv#dec296`. Census and join details (metadata only): `work/d4/orchestrator/round2-panel-10-census.json`; integrator: `work/d4/orchestrator/integrate_round2.py`. The 2 candidates' ledger rows state `unknown` bound to the same decision.

## Verification and continuation

- Final part: verified integration, ledger append: 9,693 rows added (9,691 admitted markers plus 2 explicit unknowns; evidence-table line count grew accordingly, including sheet index rows).
- Per-panel manual checks: 153 manifests reconciled gate-by-gate with queue corroboration agreeing in every category. Batch ids `001`–`153`; last accepted address `032a2320` (batch 153, the range end), last unknown `02eea158` (batch 16).
- `cargo run -p sheetty-cli -- check sheets` after ledger and decision rows: recorded in the round log. Fix2 regeneration asserted the pinned `pk2.nsz` identity, the exact 153,476-function inventory and current Rust-source fingerprints before committing.
- English/Spanish README figures and the documentation-queue round note synchronized. No purpose, type, implementation, runtime or matching claim.

Runtime harness: N/A, static documentation only. Rollback: the panel-10 integration commit; the ledger rows and decision row are the only product mutations, and no working copy, export or binary state changed.
