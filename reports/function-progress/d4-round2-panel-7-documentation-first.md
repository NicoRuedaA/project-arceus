# D4 round-2 panel 7 — documentation-first batch integration

**65,679/153,476 located update-main functions now carry static documentation markers (42.79%).** Panel 7 integrates 609 batch manifests covering its full assigned range (`01ef1960`–`022e13e4`, 9,740 functions): 9,607 complete-body records admitted as `analyzed_documented`, 21 withheld candidates explicitly recorded as `unknown`, and 112 addresses skipped because the evidence ledger already carried a row for them. This is static operation documentation only — not game-purpose interpretation, types (N2), implementation, behavior verification or binary matching.

## Integration evidence gates

- Admission required all four gates together: manifest `accepted=true`, `audit_status=accepted`, `validation_status=passed`, and a `documented` queue entry in `work/d4/tasks/panel-7-queue.json`. All 9,607 admissions were corroborated by the queue; the 21 candidates the queue marks `withheld` match the manifests exactly. No queue/manifest reconciliation mismatch occurred and no `Propagated` markers were added: identical-copy propagation did not happen in this round.
- Per-address hashes were joined to stored scratch bytes in `work/d4/scratch/panel-7/R{addr}-auto-01/attempt-01`: `raw_sha256` against the raw response bytes, `proposal_sha256` against the reserialized proposal record and audit, and `audit_sha256` against the reserialized audit record (whose embedded `fresh_raw_sha256`/`proposal_sha256` fields must reproduce both manifest hashes). 9,627 records verified on all checks; 1 verified on the core join (raw bytes + audit embedded fields) without one ancillary serialization check; 0 not-joinable; 0 mismatch. No verified count was faked.
- Analysis status/evidence columns changed only; implementation, verification and binary-match columns remain `unknown`. No N2 register changes.

## Measured result

| Category | Current markers |
|---|---:|
| Complete-body analyst records | 4,571 + 9,607 (round 2 integrated here) |
| Audited EXACT copies | 22,392 (unchanged; no propagation in this round) |
| Deterministic mechanical | 28,308 (unchanged) |
| Import stubs | 801 (unchanged) |
| Total / still without marker | **65,679 / 87,797** |

Decision: `sheets/decisions.tsv#dec290`. Census and join details (metadata only): `work/d4/orchestrator/round2-panel-7-census.json`; integrator: `work/d4/orchestrator/integrate_round2.py`. Withheld subjects remain listed privately in the queue file; their 21 ledger rows state `unknown` with `sheets/decisions.tsv#dec290`.

## Verification and continuation

- Final part: verified integration, ledger append: 9,628 rows added (evidence-table lines 57,301 → 66,929, including its sheet index rows; counted as 9,607 documented markers plus 21 explicit unknowns).
- Per-panel manual checks: `work/d4/tasks/panel-7-queue.json` (documented 9,719 minus 112 pre-existing → 9,607 here; withheld 21) reconciled exactly against the 609 manifests; last processed batch `609`, last accepted address `022e13e4`, last unknown `022b1c10` (batch 585).
- `cargo run -p sheetty-cli -- check sheets` after ledger and decision rows: recorded in the round log. Fix2 regeneration asserted the pinned `pk2.nsz` identity, the exact 153,476-function inventory and current Rust-source fingerprints before committing.
- English/Spanish README figures and the documentation-queue round note synchronized. No purpose, type, implementation, runtime or matching claim.

Runtime harness: N/A, static documentation only. Rollback: the panel-7 integration commit; the ledger rows and decision row are the only product mutations, and no working copy, export or binary state changed.
