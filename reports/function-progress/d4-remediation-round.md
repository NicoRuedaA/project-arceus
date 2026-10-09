# D4 remediation round + round-2 amendment supplements

**129,524/153,476 located update-main functions now carry static documentation markers (84.40 %).** The remediation round reworked the round-2 gate-withhold surface with a complete-body refresher read and independent audit against the exact body bytes: 1,155 manifest-accepted records upgraded (1,154 in place + 1 fresh row), 686 round-2 accepted records that had been skipped by the id-exists rule supplemented in place, and 20 gap addresses recorded as explicit unknowns. This is static operation documentation only — not game-purpose interpretation, types (N2), implementation, behavior verification or binary matching.

## What was integrated

- **Remediation-1** (decision `dec298`): 176 batch manifests, 2,814 entries, no cross-batch duplicates. 473 manifest-accepted records (`accepted=true`, `audit_status=accepted`, `validation_status=passed`): 472 upgraded in place over their existing unknown rows and 1 appended fresh (`007dd09c`, which had no ledger row and appears on `remediation-gaps.json` as the round-2 panel-2 manifest gap). 2,341 withheld records were left untouched (2,247 on round-2 rows `dec294`/`dec295`, 94 on earlier unknown rows — no note churn). Last accepted address `010d63e8` (batch 176).
- **Remediation-2** (decision `dec299`): 44 batch manifests, 699 entries, no cross-batch duplicates. 682 manifest-accepted records, all upgraded in place. 17 withheld records: 13 untouched on round-2 rows, 4 with no ledger row appended as explicit unknown gap supplements (`017d49c8`, `0180c338`, `018ee1e0`, `01a2b778`). Last accepted address `02269b50` (batch 44).
- **Round-2 amendment set** (decision `dec300`): accepted round-2 records whose ledger row was a pre-round-2 unknown row and which the round-2 integration skipped because the id already existed — 686 upgraded in place (panel-7 112, panel-8 84, panel-9 65, panel-5 79, panel-4 160, panel-2 40, panel-10 51, panel-6 95, bound to each panel's own round-2 decision `dec290`–`dec297`).
- **Gap supplements**: of the 21 `remediation-gaps.json` addresses, 1 (`007dd09c`) now carries a real remediation record and is documented; the other 20 are appended as `unknown` rows (16 with no manifest record anywhere, 4 remediation-2-withheld), each with the gap reason. Nothing was invented.

## The amendment-set count: expectation vs verification

The working expectation was ~334 (79/160/95 for panels 5/4/6), because only those three round-2 decision texts declared their accepted-set skip counts. The direct recomputation over the committed round-2 manifests and the ledger shows the other panels' skipped sets were also manifest-accepted subjects over pre-round-2 unknown rows, extending the set to all eight panels: **686, not ~334**. Each candidate was re-verified against stored round-2 scratch bytes at upgrade time (`work/d4/scratch/panel-N/R{addr}-auto-01/attempt-01`): 261 verified on the full core join (raw bytes, audit embedded fields, proposal and audit reserialization — panels 7, 8, 9) and 425 verified on the core join (raw bytes, audit embedded fields, proposal reserialization; the audit reserialization did not bind, the same ancillary convention as each panel's round-2 run — panels 5, 4, 2, 10, 6). 0 not joinable, 0 mismatch, 0 refused, and the remediation-1/remediation-2 accepted sets are disjoint from the amendment set, so the three decision sets do not double count.

## Evidence gates

- Upgrades required all three manifest gates plus a body-file sha256 recomputed on disk against `decompiled/update-main/FUN_{addr}_{addr}.c`: **473/473 (remediation-1) and 682/682 (remediation-2) verified, 0 mismatch, 0 missing, 0 refused upgrades**. On any mismatch the upgrade would have been withheld and logged; none was.
- Ancillary batch-level check (remediation manifests record `audit_sha256` of the panel's own audit file): remediation-2 bound 682/682; remediation-1 bound 416/473 with 57 unbound — the audit files were rebuilt after manifest capture (several `-rebuild` variants on disk bind correctly) and batch 043 has no audit file on disk (it contains no accepted record). Per the round-2 ancillary convention this non-binding is recorded, never a rejection; the per-address binding is the body-file join above.
- In-place upgrades kept the ledger id, build, function id and every other column byte-identical; only `analysis_status` → `analyzed_documented`, `analysis_evidence` → the decision-row reference and `analysis_notes` were rewritten. 1,840 rows upgraded in place; 21 rows appended (1 documented + 20 unknown).
- Withheld remediation entries that already carry a round-2 unknown row are unchanged on purpose: no accepted summary exists for them and no notes were churned.
- Analysis status/evidence/notes changed only; implementation, verification and binary-match columns remain `unknown`. No N2 register changes.

## Measured result

| Category | Current markers |
|---|---:|
| Complete-body analyst records | 76,182 + 1,841 (1,154 remediation upgrades + 686 round-2 amendments + 1 fresh row) = **78,023** |
| Audited EXACT copies | 22,392 (unchanged; nothing propagated) |
| Deterministic mechanical | 28,308 (unchanged) |
| Import stubs | 801 (unchanged) |
| Total / still without marker | **129,524 / 23,952** |

Decisions: `sheets/decisions.tsv#dec298`, `sheets/decisions.tsv#dec299`, `sheets/decisions.tsv#dec300`. Census and join details (metadata only, no imported summaries): `work/d4/orchestrator/remediation-1-census.json`, `work/d4/orchestrator/remediation-2-census.json`, `work/d4/orchestrator/round2-amendments-census.json`; integrator: `work/d4/orchestrator/integrate_remediation.py` (extends `integrate_round2.py`, reusing its core-join verifier read-only).

## Verification and continuation

- `cargo run -p sheetty-cli -- check sheets` after ledger, decision rows and census updates: 0 errors, 0 warnings.
- Fix2 regeneration asserted the pinned `pk2.nsz` identity (sha256 `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`), the exact 153,476-function fix2 inventory and the current 49-file Rust source fingerprint; `generation-status.json` reports `current`. The documented count (129,524) is taken live from the ledger.
- English/Spanish README figures synchronized, including the completion-plan row and `odd/DECOMPILACION-PLAN.md`.
- Honest limitations: the round-2 queue corroboration for the amendment set was recorded at round-2 time in `_dec290–dec297` and is not re-executed here; what is recomputed today is the per-address core join to the stored round-2 scratch bytes. Remediation records rest on their manifests' recorded gates plus today's recomputed body-byte and (where bound) audit-file checks; 57 remediation-1 batch-level audit-file bindings could not be re-derived and are disclosed as such.
- Nothing is pushed.

Runtime harness: N/A, static documentation only. Rollback: this commit; the ledger rows, decision rows, READMEs and fix2 report artifacts are the only mutations, and no working copy, export or binary state changed.
