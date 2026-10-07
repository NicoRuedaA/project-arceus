# P0 O2 — Final bounded static inventory/provenance gate

**Date:** 2026-10-06. **Verdict: FAIL — documentation correction required.**
The underlying static inventory and provenance evidence is sufficiently bounded
for the user's reduced P0 scope. The local record still presents an obsolete
empty-index status as the opening status of the global gameDB report; until that
current-status contradiction is corrected, the documented static inventory gate
does not pass. No canonical files were edited. This review ran no Ghidra,
gameDB, or treemap command and does not infer that detected bodies are the
complete valid-function universe.

## Scope and evidence reviewed

Reviewed the current bounded-closure decision in `odd/PLAN.md`, the 26-row
`sheets/re/p0_scope_census.tsv`, both README status/completion-plan tables, the
base module provenance and five-NSO import/relocation reports, update-main
residual/range reports, auxiliary export/range reports, fix2 profile report and
manifest/status, and the global gameDB index report. All reported percentages
and denominators below retain their documented scope; none is promoted to
whole-game or complete-function coverage.

## Reconciled measurements

### Base identity and original-NSO imports

The base provenance report and direct five-module import/relocation inventory
agree on all five NSO sizes, SHA-256 digests, and module IDs. The provenance
chain identifies the package `Program` NCZ and ExeFS members; 15/15 embedded
NSO segment hashes passed in the recorded verification. The import report uses
the original NSO images, not synthetic ELF exports, and covers all five named
base roles. Its aggregate distinction is explicit: `main` has 3 `DT_NEEDED`,
209,391 RELA, 815 PLT, 937 dynsym (900 undefined; 899 referenced-undefined)
rows; `rtld`, `sdk`, `subsdk0`, and `subsdk1` have 0, 0, 0, and 0 `DT_NEEDED`,
respectively, with the per-module relocation/symbol counts in that report.
These are bounded structural counts, not provider binding or runtime use.

The former statement in the provenance report that base original-NSO imports
remain unknown is expressly superseded by its status pointer and the dedicated
direct import report. Do not treat that dated “What remains unknown” paragraph
as the current import status. No conflicting current base identity or import
count was found.

### Update `main`, fix2 profile, and index

- Update identity is scoped to the pinned `pk2.nsz` SHA-256
  `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`, module
  ID `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e`, and 53,106,320 executable
  `.text` bytes. The archive file itself was not present at the checked local
  paths, so this audit verified the recorded pin and its manifest/report
  linkage, not a fresh hash of the archive bytes.
- The fix2 inventory is 153,476 unique entries / 51,275,676 body bytes. Its
  exact body union has no overlap; 1,830,644 bytes (3.44713%) in 23,597 ranges
  lie outside existing bodies. That is a bounded byte residual, not 1,830,644
  bytes of known missing functions. Ghidra listing labels 40,764 residual bytes
  as instructions and 1,789,880 as data; those labels do not establish
  semantics, valid new function boundaries, or reachability. The complete
  valid-function denominator remains Unknown with this exact byte/range bound
  and later semantic/reconciliation work explicitly deferred.
- The 153,476 located entries have outputs: 153,471 C plus 5 ASM fallbacks.
  These are separate from body-byte coverage and from the available-C index.
- The global index report's successful retry records 177,795 C files across six
  roots, 176,667 parsed function rows/symbols, 83,373 strings, 48,991,899 edges,
  and selftest 87/87. Its per-root file/row/no-row triplets sum to
  base-main 3,997/3,917/80; update-main 153,471/152,634/837; `rtld` 31/31/0;
  `sdk` 9,063/9,002/61; `subsdk0` 2,959/2,901/58; `subsdk1` 8,274/8,182/92.
  Thus 1,128 C files have no parsed function row. They are parser/index results,
  not evidence of empty source or missing/invalid functions. The corpus is
  available C exports only; base-main/auxiliary inventories remain partial and
  five update-main ASM fallbacks are excluded. All these Unknown denominators
  have explicit bounded units and later-phase limits in the report/README.

### Base and auxiliary function detections, body ranges, exports

- Base `main` has 68,412 Ghidra inventory entries, is recorded as provenance
  only, and its 3,997 C export/index files are correctly described as a partial
  inventory-bounded view (not a complete export denominator).
- The successful auxiliary export run reports 27,750 Ghidra candidates,
  20,327 C exports, and 7,902,740 summed body bytes across 15,572,944 `.text`
  bytes. The resulting 50.75% is a summed-body-size ratio, explicitly not
  coverage. Complete auxiliary function denominators remain Unknown.
- The no-analysis auxiliary range snapshot reports 27,349 detected entries
  (rtld 33, sdk 13,531, subsdk0 5,523, subsdk1 8,262), exact stored-inventory
  ID intersections, and body unions/gaps for those snapshot detections:
  `rtld` 5,220/6,240 (1,020 gap bytes, 13 spans); `sdk` 1,167,456/5,822,640
  (4,655,184 bytes, 11,529 spans); `subsdk0` 1,746,372/3,445,104
  (1,698,732 bytes, 2,268 spans); `subsdk1` 5,016,388/6,298,960
  (1,282,572 bytes, 3,210 spans). These sums reconcile exactly to the
  respective `.text` sizes for the existing snapshot detections; they are not
  evidence that the snapshot enumerates every valid function.
- The 401-candidate difference between the successful export run (27,750) and
  the no-analysis range snapshot (27,349) is bounded and disclosed: the export
  report attributes its run to in-memory auto-analysis and retains the earlier
  counts as historical; the range report uses `-noanalysis`. Range-snapshot
  export-ID intersections total 19,973/20,327, with 354 export IDs absent from
  those snapshots (162 sdk, 89 subsdk0, 103 subsdk1). The reports disclose the
  snapshot/configuration variance and do not conflate the measurements. Its
  cause remains Unknown; resolving completeness is not required by this
  reduced P0 gate and must not be silently represented as equality.
- The auxiliary `Data.isDefined()` disagreement is expressly preserved as a
  method discrepancy. It does not change the body unions; neither listing
  classification proves code/data semantics. This Unknown is quantified and
  cited in the plan/census with no unjustified exclusion.

## Fingerprint checks

Read-only local checks produced:

| Check | Result |
|---|---|
| Fix2 generation status | `current`; status manifest SHA-256 equals actual `manifest.json` SHA-256 `e9d4e595da841f2cba3a7ee532e0571e5b6ece5bdd59dfb77f0fd352457edb57` |
| Inventory fingerprint | Actual `functions.tsv` SHA-256 matches manifest `829d810c52a7662ec4d3d958866a091a7b926d9fccd0eea1427181669abd2535`; 153,476 rows and 51,275,676 summed body bytes re-counted |
| Evidence ledger | Actual SHA-256 matches manifest `5e83e85ac050dac7b23dd38a17ea42d8cab73abc5b3ccb06a88a8bec4f6f8cb8` |
| Rust sources | All 49 manifest-listed files exist and each SHA-256 matches; manifest aggregate fingerprint is `e388b355e086b370295b2b41f3f59b10b43806f21404469a72bb156137f34bfb` |
| gameDB snapshot | Actual staged `index.sqlite` SHA-256 matches the fix2 manifest's `3205e6bf6ded50dd762ed73e3f63d4a159e8f1ecd242440ae61128b644eed8f9`; global report records the successful one-shot index outcome |
| Archive bytes | Not locally available at `/home/nico/work/pla/pk2.nsz` or repository `work/pla/pk2.nsz`; recorded archive pin is present in manifest and cited reports |

No `gamedb index`, Ghidra, or treemap operation was run. Fingerprint checks do
not imply semantic coverage or justify function-state promotion.

## Findings and exact correction required

**Blocking stale status:** `p0-global-gamedb-index.md` lines 3–8 opens with
“attempted but incomplete; empty DB” and says post-cancellation checks found zero
rows. Its later explicitly reauthorized retry (lines 131–175) supersedes that
state and records exit code 0 plus the populated six-root index. The historical
events are valid, but the opening `Status` is not labeled historical and reads
as current. Correct only that opening status/preamble to state that the latest
retry succeeded and the index is populated; label the zero-row/canceled attempt
as an earlier historical snapshot. Preserve both attempt records and their
counts. Until this correction is made, verdict remains **FAIL**.

**Non-blocking bounded historical value:** census `update_main_identity` still
quotes 153,471 files / 153,470 parsed rows, while the current global index is
153,471 / 152,634. Census `global_gamedb_index` explicitly identifies the former
as an older snapshot and supersedes it for current reporting; README and PLAN
use the newer 152,634 figure. This is not an unexplained current-count conflict,
but the `update_main_identity` field is easier to misread if encountered alone.
If it is edited in a later authorized canonical-doc pass, replace those two
index numbers with current counts or label that clause “historical index
snapshot”; keep the rest of its update identity/function data intact.

**Spanish README synchronization defect:** `README.es.md` P0 gate list jumps
from item (4) to item (6), and the item (4) sentence is interrupted by “índice
global C frente a sus inventarios/fuentes”. The English README has distinct
items (4) evidence labels, (5) fingerprint checks, and (6) sheetty/CI. This is
not a measurement discrepancy, but the README pair is not fully synchronized;
repair the Spanish list in a later authorized README edit.

No other current P0 static identity, import count, fix2 count/body-byte value,
export count, range union, or global-index total reviewed here was found
internally inconsistent. The base/auxiliary/update-main Unknown denominators
are explicitly scoped, quantified where applicable, and distinguished from
whole-universe claims. NCA authenticity, update CNMT semantics, NPDM crypto or
runtime necessity, loaded/reached, ownership, and complete semantic/function
universe claims remain explicitly outside the reduced P0 static gate.

**Gate result:** **FAIL pending the one exact blocking correction to the global
index report's opening status.** Once the stale opening is re-labeled as
historical/current status is corrected, the bounded static inventory and
provenance evidence reviewed here is sufficient for the reduced P0 gate; this
does not close P0's other local gates or claim complete valid-function
inventory.
