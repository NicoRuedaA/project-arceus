# D4 round-2 panel 1 — documentation-first batch integration

**139,186/153,476 located update-main functions now carry static documentation markers (90.69%).** Panel 1 integrates 608 batch manifests covering its full assigned range (`00000190`–`004687d4`, 9,740 queue entries): 9,662 complete-body records bound to the panel decision as `analyzed_documented` (9,599 appended fresh + 63 upgraded in place over pre-existing unknown rows) and 69 withheld candidates explicitly recorded as `unknown`. Unlike every other round-2 panel, zero manifest addresses collided with an already-documented ledger row: 3,076 earlier documented rows (round-1 region work) sit inside the range but at different function-split boundaries. This is static operation documentation only — not game-purpose interpretation, types (N2), implementation, behavior verification or binary matching.

## Integration evidence gates

- Admission required all three manifest gates (`accepted=true`, `audit_status=accepted`, `validation_status=passed`) with queue corroboration in `work/d4/tasks/panel-1-queue.json`. Corroboration: the queue is a stale pre-work snapshot (all 9,740 entries `assigned`), so the manifest gates decided admissibility, recorded explicitly as a queue staleness reconciliation and not as a silent substitution. The panel status report (9,731 manifest entries, 9,662 accepted, 0 held, 608 batches) matches the manifests exactly.
- Upgrade-in-place semantics (added in the remediation round): 63 manifest-accepted addresses whose ledger rows carried `analysis_status=unknown` (prior evidence dec133–dec252) were upgraded in place — analysis status/evidence/notes rewritten, ledger id and every other column byte-identical; no duplicate ids appended. 0 upgrades refused (0 join mismatches among them). Withheld candidates whose addresses already carry unknown rows were left untouched (0 such cases in panel 1 — every withheld address was fresh).
- Manifest gap recorded: 9 queue addresses (`000b1d70`, `000b1ed4`, `000b1f44`, `000b1fbc`, `000b206c`, `000b2c3c`, `000b2c70`, `000b2ea0`, `000b2ee0`) have no round-2 manifest record; all 9 also have no ledger row. Nothing was invented for them; the gap is recorded and their documentation remains unknown.
- Batches 001–002 (48 entries) are pre-existing round remnants (they predate `panel1_pipeline.py`, per the panel status note). They were integrated with the remediation-round pre-existing-row semantics: 0 already-documented skips, 1 unknown-row upgrade (`00009f10`, batch 002), and fresh appends for the rest (37 documented + 10 unknown). The 7 batch-002 redemptions left unknown are records whose writer-validation passed but whose audit was withheld by the pre-existing round's gates; the gates were not re-judged retroactively.
- Per-address hashes were joined to stored scratch bytes in `work/d4/scratch/panel-1/R{addr}-auto-01/attempt-01`: `raw_sha256` against the raw response bytes, embedded audit `fresh_raw_sha256`/`proposal_sha256` fields against both manifest hashes, and proposal reserialization hash against `proposal_sha256`. All 9,731 verified-set records verified on that core join. Zero records passed the ancillary `audit_sha256` reserialization check: panel-1's manifest audit hashes do not bind to the stored `audit.json` reserialization (recorded as an ancillary non-joinable check, never a rejection; the record binding is the embedded-field join). 0 not joinable; 0 raw or embedded-field mismatch. No verified count was faked.
- Analysis status/evidence columns changed only; implementation, verification and binary-match columns remain `unknown`. No N2 register changes.

## Measured result

| Category | Current markers |
|---|---:|
| Complete-body analyst records | 69,183 + 1,858? → see breakdown below |
| Audited EXACT copies | 22,392 (unchanged; no propagation in any round-2 panel) |
| Deterministic mechanical | 28,308 (unchanged) |
| Import stubs | 801 (unchanged) |
| Total / still without marker | **139,186 / 14,290** |

Complete-body analyst records breakdown: 76,182 (post-remediation baseline per `d4-remediation-round.md`) includes 1,841 from the remediation day; the pre-panel-1 ledger held 129,524 documented markers across all categories of documentation, of which 69,324 were complete-body analyst records and the rest audited-exact, mechanical and stub markers. Panel 1 adds 9,662 complete-body analyst records (9,599 fresh + 63 upgrades), a 13.9% growth of the analyst-record pool on its own panel range.

## Decision

`sheets/decisions.tsv#dec301`. Census and join details (metadata only): `work/d4/orchestrator/round2-panel-1-census.json`; integrator: `work/d4/orchestrator/integrate_round2.py` (extended with the remediation-round `--upgrade` in-place mode). The 69 withheld candidates' ledger rows state `unknown` bound to the same decision. Last processed batch `608` (byte range end `004687d4`); last unknown `00433ff8` (batch 578).

## Verification and continuation

- Verified integration: 9,668 ledger rows appended and 63 rows upgraded in place; evidence-table line count grew accordingly, including sheet index rows.
- `cargo run -p sheetty-cli -- check sheets` after ledger and decision row: 0 errors, 0 warnings. Fix2 regeneration asserted the pinned `pk2.nsz` identity, the exact 153,476-function inventory and current source fingerprints, and the documented count (139,186) is taken live from the ledger.
- English/Spanish README figures synchronized, including the completion-plan row and `odd/DECOMPILACION-PLAN.md`. No purpose, type, implementation, runtime or matching claim.

Runtime harness: N/A, static documentation only. Rollback: the panel-1 integration commit; the ledger rows and decision row are the only product mutations, and no working copy, export or binary state changed.
