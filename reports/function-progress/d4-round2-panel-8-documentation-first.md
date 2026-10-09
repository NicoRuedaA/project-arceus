# D4 round-2 panel 8 — documentation-first batch integration

**75,335/153,476 located update-main functions now carry static documentation markers (49.09%).** Panel 8 integrates 609 batch manifests covering its full assigned range (`022e15b0`–`02826b7c`, 9,740 functions): 9,656 complete-body records admitted as `analyzed_documented`, 84 addresses skipped because the evidence ledger already carried a row for them, and no unknown rows (the panel withheld nothing). This is static operation documentation only — not game-purpose interpretation, types (N2), implementation, behavior verification or binary matching.

## Integration evidence gates

- Admission required all four gates together: manifest `accepted=true`, `audit_status=accepted`, `validation_status=passed`, and a `documented` queue entry in `work/d4/tasks/panel-8-queue.json`. The panel queue states `documented` for all 9,656 admitted addresses and the manifests agree; no queue/manifest reconciliation mismatch occurred and no `Propagated` markers were added: identical-copy propagation did not happen in this round.
- Cross-panel overlap handling: `work/d4/orchestrator/panel-8-overlap-note.json` lists 16 addresses that also occurred in panel-4 round-1 scratch. The note is confirmed empirically: all 16 addresses sit inside panel-8 batches 305/306 and inside panel-8's range; the round-2 panel-4 range (`00d52adc`–`011ca6b0`) contains none of them, so no round-2 row was dropped and panel-8 evidence wins the dedupe as recorded. Ledger writes happen only this panel's commit.
- Per-address hashes were joined to stored scratch bytes in `work/d4/scratch/panel-8/R{addr}-auto-01/attempt-01`: `raw_sha256` against the raw response bytes, `proposal_sha256` against the reserialized proposal record and audit, `audit_sha256` against the reserialized audit record (embedded `fresh_raw_sha256`/`proposal_sha256` must reproduce both manifest hashes). 9,640 records verified on all checks; 16 verified on the core join (raw bytes + audit embedded fields) without one ancillary serialization check; 0 not-joinable; 0 mismatch. No verified count was faked.
- Analysis status/evidence columns changed only; implementation, verification and binary-match columns remain `unknown`. No N2 register changes.

## Measured result

| Category | Current markers |
|---|---:|
| Complete-body analyst records | 14,178 + 9,656 (round 2 integrated across panels so far) |
| Audited EXACT copies | 22,392 (unchanged; no propagation in this round) |
| Deterministic mechanical | 28,308 (unchanged) |
| Import stubs | 801 (unchanged) |
| Total / still without marker | **75,335 / 78,141** |

Decision: `sheets/decisions.tsv#dec291`. Census and join details (metadata only): `work/d4/orchestrator/round2-panel-8-census.json`; integrator: `work/d4/orchestrator/integrate_round2.py`.

## Verification and continuation

- Final part: verified integration, ledger append: 9,656 rows added (evidence-table lines 66,929 → 76,585, including its sheet index rows; all admitted).
- Per-panel manual checks: `work/d4/tasks/panel-8-queue.json` (documented 9,740 minus 84 pre-existing → 9,656 here) reconciled exactly against the 609 manifests; last processed batch `609`, last accepted address `02826b7c`; no unknown addresses.
- `cargo run -p sheetty-cli -- check sheets` after ledger and decision rows: recorded in the round log. Fix2 regeneration asserted the pinned `pk2.nsz` identity, the exact 153,476-function inventory and current Rust-source fingerprints before committing.
- English/Spanish README figures and the documentation-queue round note synchronized. No purpose, type, implementation, runtime or matching claim.

Runtime harness: N/A, static documentation only. Rollback: the panel-8 integration commit; the ledger rows and decision row are the only product mutations, and no working copy, export or binary state changed.
