# D4 round-2 panel 2 — documentation-first batch integration

**109,763/153,476 located update-main functions now carry static documentation markers (71.52%).** Panel 2 integrates 610 batch manifests covering its assigned range (`0046888c`–`007e7e5c`, 9,740 queued functions; 9,739 covered by manifests): 7,648 complete-body records admitted as `analyzed_documented`, 1,997 withheld candidates explicitly recorded as `unknown`, and 94 addresses skipped because the evidence ledger already carried a row for them. This is static operation documentation only — not game-purpose interpretation, types (N2), implementation, behavior verification or binary matching.

## Integration evidence gates

- Admission required all four gates together: manifest `accepted=true`, `audit_status=accepted`, `validation_status=passed`, and a `documented` queue entry in `work/d4/tasks/panel-2-queue.json`. Reconciliation was exact: the queue's 7,688 `documented` addresses match the manifests (7,648 here plus 94 that already carried prior ledger rows), its 2,051 `withheld` subjects match the manifests' failed gates (1,997 here plus 54 already recorded), and its single `decompile_failed` subject (`007dd09c`) has **no manifest record and no ledger row** — nothing was invented for it and the gap is recorded as a manifest/queue reconciliation item instead. No `Propagated` markers were added: identical-copy propagation did not happen in this round.
- Legacy-file exclusion: `work/d4/orchestrator/panel-2-batch-00[1-4]-legacy4panel.json` carry superseded round-1 batch manifests whose batch ids collide with the round-2 batch series and whose 112 addresses fall outside this panel's queue (96 of them were already in the ledger through other evidence; none belongs to the round-2 assignment). The integrator excludes suffixed legacy files explicitly; the exclusion and the address-set checks are recorded in the census. Private reference only — nothing from these files entered the ledger.
- Per-address hashes were joined to stored scratch bytes in `work/d4/scratch/panel-2/R{addr}-auto-01/attempt-01`: `raw_sha256` against the raw response bytes, embedded audit `fresh_raw_sha256`/`proposal_sha256` fields against both manifest hashes, and proposal reserialization hash against `proposal_sha256`. 9,645 joined records verified on that core join. Zero records passed the ancillary `audit_sha256` reserialization check: panel-2's manifest audit hashes do not bind to the stored `audit.json` reserialization (the audit files still carry the audit decisions; the record binding is the embedded-field join). Per the round rule this ancillary non-binding is recorded, not treated as a rejection; 0 not joinable; 0 raw or embedded-field mismatch. No verified count was faked.
- Analysis status/evidence columns changed only; implementation, verification and binary-match columns remain `unknown`. No N2 register changes.

## Measured result

| Category | Current markers |
|---|---:|
| Complete-body analyst records | 50,614 + 7,648 (round 2 integrated across panels so far) |
| Audited EXACT copies | 22,392 (unchanged; no propagation in this round) |
| Deterministic mechanical | 28,308 (unchanged) |
| Import stubs | 801 (unchanged) |
| Total / still without marker | **109,763 / 43,713** |

Decision: `sheets/decisions.tsv#dec295`. Census and join details (metadata only): `work/d4/orchestrator/round2-panel-2-census.json`; integrator: `work/d4/orchestrator/integrate_round2.py`. The 1,997 candidates' ledger rows state `unknown` bound to the same decision.

## Verification and continuation

- Final part: verified integration, ledger append: 9,645 rows added (7,648 admitted markers plus 1,997 explicit unknowns; evidence-table line count grew accordingly, including sheet index rows).
- Per-panel manual checks: batch ids `001`–`610`, 610 manifests reconciled gate-by-gate with queue corroboration agreeing in every category. Highest batch processed `610`; last accepted address `007e7e30` (batch 609), last unknown `007e7e5c` (batch 609, the range end). `007dd09c` remains intentionally absent (no evidence record exists for it).
- `cargo run -p sheetty-cli -- check sheets` after ledger and decision rows: recorded in the round log. Fix2 regeneration asserted the pinned `pk2.nsz` identity, the exact 153,476-function inventory and current Rust-source fingerprints before committing.
- English/Spanish README figures and the documentation-queue round note synchronized. No purpose, type, implementation, runtime or matching claim.

Runtime harness: N/A, static documentation only. Rollback: the panel-2 integration commit; the ledger rows and decision row are the only product mutations, and no working copy, export or binary state changed.
