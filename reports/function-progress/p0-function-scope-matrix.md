# P0 function-scope reconciliation matrix

Date: 2026-10-06. Scope: function inventory, available exports, measured body
ranges, and the completed global gameDB index for the five executable roles in
the mandatory base-v0 + update-v262144 game. The update is a patch, not a
standalone program. Base-v0 `main` is retained for provenance and comparison,
and is explicitly **out of port work**; the update `main` replaces it. This is
an aggregate metadata reconciliation only: no function names, entry IDs,
pseudocode, native function-state promotion, Ghidra process, or gameDB command
was used or produced for this report.

## Matrix

Counts below describe available stored artifacts and measured snapshots, not a
complete set of valid functions. “Known inventory” means a recorded Ghidra or
fix2 function-entry set; where applicable, exact ID-set equality is stated as
an aggregate reconciliation without listing IDs.

| Module role | Known inventory entries / ID reconciliation | Available exports | Actual body union and executable gap | Global gameDB files / parsed rows / no-row files | Complete valid-function denominator |
|---|---|---|---|---|---|
| **Base-v0 `main`** *(provenance only; out of port work)* | 68,412 historical capped Ghidra detections in the census. No body-range union or exact inventory-to-current-export ID reconciliation is recorded in the reviewed evidence. | 3,997 C files in the global-index corpus. Assembly fallback count and exact ID-set relationship to the 68,412 detections are not established by these reports. | **Unknown.** No qualified actual-body union or `.text` complement for base `main` in the reviewed reports. | 3,997 / 3,917 / 80 | **Unknown.** 68,412 is explicitly capped, not a complete denominator. It is not pending port work. |
| **Update-v262144 `main`** | Fix2 inventory: 153,476 unique entries. The untouched fix2 FunctionManager matched all 153,476 inventory IDs one-to-one. | 153,471 C + 5 ASM fallback entries; disjoint sets whose union exactly matches all 153,476 inventory entries. | Actual union: 51,275,676 / 53,106,320 `.text` bytes; no overlap or out-of-`.text` body bytes. Complement: 1,830,644 bytes in 23,597 spans (3.44713%), outside existing bodies; not inferred functions. | 153,471 / 152,634 / 837. The five ASM entries are outside the C-only index. | **Unknown.** The exact denominator is the located fix2 inventory only; whether residual bytes contain further valid functions is unresolved. |
| **`rtld`** *(base/update byte-identical)* | Stored prior snapshot: 33 entries; read-only range snapshot detected 33 and all entry IDs matched the stored set. No claim this is complete. | Successful C export run: 31 C files. The export IDs all matched the range snapshot (31/31); 2 stored entries have no corresponding C export. | Actual union for the measured snapshot: 5,220 / 6,240 `.text` bytes; gap 1,020 bytes / 13 spans. | 31 / 31 / 0 | **Unknown.** Existing detection and range agreement is not a complete function inventory. |
| **`sdk`** *(base/update byte-identical)* | Stored prior snapshot: 13,531 entries; read-only range snapshot detected 13,531 and all IDs matched. The successful export run's separate in-memory detections numbered 13,718; it did not match the range snapshot exactly. | Successful C export run: 9,063. Against range snapshot: 8,901 IDs matched, 162 export IDs did not occur in that snapshot, and 4,630 snapshot entries had no corresponding export ID. | Actual union for the range snapshot: 1,167,456 / 5,822,640 `.text` bytes; gap 4,655,184 bytes / 11,529 spans. | 9,063 / 9,002 / 61 | **Unknown.** Detection variance and large measured gaps remain; neither export nor index rows establish completeness. |
| **`subsdk0`** *(base/update byte-identical)* | Stored prior snapshot: 5,523 entries; read-only range snapshot detected 5,523 and all IDs matched. The successful export run's separate in-memory detections numbered 5,633. | Successful C export run: 2,959. Against range snapshot: 2,870 IDs matched, 89 export IDs did not occur in that snapshot, and 2,653 snapshot entries had no corresponding export ID. | Actual union for the range snapshot: 1,746,372 / 3,445,104 `.text` bytes; gap 1,698,732 bytes / 2,268 spans. | 2,959 / 2,901 / 58 | **Unknown.** Detection variance and incomplete range census remain unresolved as function completeness. |
| **`subsdk1`** *(base/update byte-identical)* | Stored prior snapshot: 8,262 entries; read-only range snapshot detected 8,262 and all IDs matched. The successful export run's separate in-memory detections numbered 8,366. | Successful C export run: 8,274. Against range snapshot: 8,171 IDs matched, 103 export IDs did not occur in that snapshot, and 91 snapshot entries had no corresponding export ID. | Actual union for the range snapshot: 5,016,388 / 6,298,960 `.text` bytes; gap 1,282,572 bytes / 3,210 spans. | 8,274 / 8,182 / 92 | **Unknown.** The successful export run's detections and export count exceed the older snapshot count, so neither is a complete denominator. |

For all four auxiliary roles, each base-v0 NSO was verified byte-identical to
its update-v262144 counterpart. The body unions/gaps therefore describe the
same module bytes in both versions, but do not establish module loading,
reachability, semantic coverage, or a complete function set. The gap listing
classification is not reproduced as code/data truth: the range report and
later auxiliary listing triage disagree over `Data.isDefined()` treatment, and
the method discrepancy remains unresolved. Body-union measurements themselves
were not contradicted.

## Reconciliation and contradictions

1. **Inventory detections, export attempts, and gameDB rows are different
measures.** A Ghidra detection may not be exported; export filenames may not
belong to the exact entry-ID snapshot used in a separate read-only range audit;
gameDB parses C files and can yield no parsed function row. None of the counts
may substitute for another or for a complete-function denominator.
2. **Auxiliary inventory-versus-export mismatch is directly observed.** `rtld`
   has a two-entry difference between the prior snapshot and C outputs. For the
   other modules, the C output set has entries absent from the opened range
   snapshot, while many snapshot entries have no output. Those are snapshot/ID
   set discrepancies, not evidence that a file is spurious or that a function
   is missing from the binary.
3. **Auxiliary detection totals vary across runs.** The successful export
   invocations report 33 / 13,718 / 5,633 / 8,366 candidates versus prior
   metadata snapshots of 33 / 13,531 / 5,523 / 8,262. Only `rtld` agrees; the
   cause for the other differences was not reconciled. The range report used
   the prior stored entry sets and independently confirmed those sets against
   its read-only project snapshot.
4. **Auxiliary body-size totals vary by evidence source.** The exact range
   snapshots' body sums are 5,220 / 1,167,456 / 1,746,372 / 5,016,388 bytes.
   Successful C-export reports separately sum exported body sizes to 5,200 /
   1,112,700 / 1,723,376 / 5,061,464 bytes. These are not interchangeable:
   the successful export IDs only partially match the range snapshots for
   three modules, and no evidence reconciles those byte sums entry-by-entry.
5. **Update `main` has the strongest bounded reconciliation, not a complete
   denominator.** Its fix2 inventory IDs, untouched FunctionManager, and the
   disjoint C/ASM export union all reconcile exactly. The 1,830,644-byte
   executable complement is outside every currently inventoried body; it is
   not a count of missing functions. A prior clone's +1,425 detections are
   excluded from the fix2 baseline and remain an unexplained clone-only
   discrepancy.
6. **The current global gameDB view supersedes stale local/update-only counts.**
   The completed global index records 177,795 C files and 176,667 parsed rows
   across six roots, including 152,634 parsed rows for update `main`. Historical
   update-only local indexing recorded 153,470 parsed rows from 153,471 C files;
   do not combine that result with the global snapshot. The older capped update
   profile's 68,330-function / 68,326-parsed-row view is historical and not the
   fix2 denominator.
7. **Base `main` remains a separate provenance-only role.** Its 68,412 detections
   are historical and capped; the current 3,997 C files and 3,917 parsed rows
   only quantify available exports. No body union or complete valid-function
   count was established in the reviewed evidence. It is explicitly not port
   work because update `main` replaces it.

The global index succeeded for the exact available staged C corpus and passed
its recorded selftest. The 1,128 files without parsed rows are parser/index
outcomes, not proof that their bodies contain no function. Five update-main ASM
fallbacks were not staged and are absent from those global file totals.

## Evidence still needed

- **Base `main`:** a bounded, identity-qualified inventory/export-to-ID
  reconciliation and actual body-range union against its executable `.text`,
  if needed for provenance reporting. Keep it out of port work and keep its
  valid-function denominator unknown unless a complete-boundary method is
  independently justified.
- **Auxiliaries:** reconcile the successful export run against the exact stored
  range snapshot by input/project identity and entry IDs; explain per-run
  detection variance and exported-body-sum differences; resolve the listing
  API discrepancy. Then define an independent method for classifying residual
  executable spans and demonstrating inventory completeness. Until then the
  denominator remains unknown.
- **Update `main`:** assess the existing-body complement for valid function
  boundaries using direct evidence, not stored references, listing labels, or
  count arithmetic. Do not add rows or promote ledger states absent that
  evidence.
- **Global corpus:** retain the current global report as authority for available
  C-file parsing. If an ID-level gameDB-to-native inventory join is needed,
  obtain and validate that join independently; parsed-row totals alone do not
  imply one-to-one coverage. Include ASM only in a separately described scope.
- **All modules:** preserve `Unknown` for complete valid-function denominators,
  semantics, runtime use, behavior verification, and binary matching wherever
  direct evidence is absent. No function-state promotions are made here.

## Sources

- [`p0-fix2-treemap-input-audit.md`](p0-fix2-treemap-input-audit.md) and
  [`p0-fix2-treemap-profile.md`](p0-fix2-treemap-profile.md)
- [`p0-update-main-ghidra-range-reconciliation.md`](p0-update-main-ghidra-range-reconciliation.md)
- [`p0-auxiliary-range-reconciliation.md`](p0-auxiliary-range-reconciliation.md)
- [`p0-auxiliary-module-export-inventory.md`](p0-auxiliary-module-export-inventory.md)
- [`p0-global-gamedb-index.md`](p0-global-gamedb-index.md) and
  [`p0-global-gamedb-source-verification.md`](p0-global-gamedb-source-verification.md)
- [`p0-base-v0-module-provenance.md`](p0-base-v0-module-provenance.md) and
  [`p0-base-import-relocation-inventory.md`](p0-base-import-relocation-inventory.md)
- [`p0-wave8-census-audit.md`](p0-wave8-census-audit.md),
  [`p0-wave8-full-scope-audit.md`](p0-wave8-full-scope-audit.md),
  [`p0_scope_census.tsv`](../../sheets/re/p0_scope_census.tsv), and
  [`PLAN.md`](../../odd/PLAN.md)

No census, plan, README, source, or evidence ledger was changed. No function
state was advanced.
