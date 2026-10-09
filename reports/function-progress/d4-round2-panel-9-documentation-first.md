# D4 round-2 panel 9 — documentation-first batch integration

**85,006/153,476 located update-main functions now carry static documentation markers (55.39%).** Panel 9 integrates 609 batch manifests covering its full assigned range (`02826cf4`–`02e3b1f0`, 9,740 functions): 9,671 complete-body records admitted as `analyzed_documented`, 4 withheld decompile-timeout subjects explicitly recorded as `unknown`, and 65 addresses skipped because the evidence ledger already carried a row for them. This is static operation documentation only — not game-purpose interpretation, types (N2), implementation, behavior verification or binary matching.

## Integration evidence gates

- Admission required manifest `accepted=true`, `audit_status=accepted` and `validation_status=passed`; queue corroboration was resolved explicitly, not silently. `work/d4/tasks/panel-9-queue.json` is a stale pre-work snapshot (every entry still `assigned`), so the manifest audit/validation gates decided admissibility, exactly as the panel status report records it: 9,736 accepted + 4 decompile-timeout withheld (`0289e034`, `029acdcc`, `029f0198`, `02caceec`, `validation_status=not_run`). The stale-queue reconciliation category covers all 9,675 processed non-skipped subjects and is listed in the census; nothing was patched into the queue file. No `Propagated` markers were added: identical-copy propagation did not happen in this round.
- Per-address hashes were joined to stored scratch bytes in `work/d4/scratch/panel-9/R{addr}-auto-01/attempt-01`: `raw_sha256` against the raw response bytes, `proposal_sha256` against the reserialized proposal record and audit, `audit_sha256` against the reserialized audit record (embedded `fresh_raw_sha256`/`proposal_sha256` must reproduce both manifest hashes). 9,653 records verified on all checks; 22 verified on the core join (raw bytes + audit embedded fields) without one ancillary serialization check; 0 not-joinable; 0 mismatch. No verified count was faked.
- Analysis status/evidence columns changed only; implementation, verification and binary-match columns remain `unknown`. No N2 register changes.

## Measured result

| Category | Current markers |
|---|---:|
| Complete-body analyst records | 23,834 + 9,671 (round 2 integrated across panels so far) |
| Audited EXACT copies | 22,392 (unchanged; no propagation in this round) |
| Deterministic mechanical | 28,308 (unchanged) |
| Import stubs | 801 (unchanged) |
| Total / still without marker | **85,006 / 68,470** |

Decision: `sheets/decisions.tsv#dec292`. Census and join details (metadata only): `work/d4/orchestrator/round2-panel-9-census.json`; integrator: `work/d4/orchestrator/integrate_round2.py`. The four withheld subjects' ledger rows state `unknown` bound to the same decision and carry the withheld/not_run gate details.

## Verification and continuation

- Final part: verified integration, ledger append: 9,675 rows added (evidence-table lines 76,585 → 86,260, including its sheet index rows; 9,671 admitted markers plus 4 explicit unknowns).
- Per-panel manual checks: 609 manifests reconciled against the panel status record (9,736 accepted / 4 withheld); last processed batch `609`, last accepted address `02e3b1f0`; last unknown address `02caceec` (batch 358).
- `cargo run -p sheetty-cli -- check sheets` after ledger and decision rows: recorded in the round log. Fix2 regeneration asserted the pinned `pk2.nsz` identity, the exact 153,476-function inventory and current Rust-source fingerprints before committing.
- English/Spanish README figures and the documentation-queue round note synchronized. No purpose, type, implementation, runtime or matching claim.

Runtime harness: N/A, static documentation only. Rollback: the panel-9 integration commit; the ledger rows and decision row are the only product mutations, and no working copy, export or binary state changed.
