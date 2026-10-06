# P0 update-main gap semantic triage

> **Superseded aggregate result (2026-10-06):** The initial query failed before
> execution; retain that failure as history, but use the repaired successful
> query in [semantic detail](p0-update-main-gap-semantic-detail.md) and its
> authoritative [retry](p0-update-main-gap-semantic-retry.md). The retry reports
> 1,789,328 defined data units / 1,789,880 bytes (all generic classifier
> “other”), 10,191 instructions (1,224 call / 1,397 jump with conditionality
> combined / 7,570 other flow), and 20,627 operands in the “other or
> unspecified” OperandType API-flag bucket. These are listing/API metadata only,
> not semantic knowledge, function-boundary evidence, or reachability. The
> corrected seed containment and reference totals remain authoritative; do not
> revive the earlier `getCodeUnitAt` error or fine jump subtype split. P0 is
> still open; no function-progress ledger states are promoted.

Date: 2026-10-06. Scope: the exact executable complement of existing function bodies in the update-v262144 `main` listing. The update is a patch; this report makes no standalone-program, reachability, or port-coverage claim.

## Method and execution status

- Reviewed the spreadsheet method, decompilation prompt, current continuation plan, and the current range reconciliation, gap classification, corrected seed triage, and candidate-validation evidence.
- The established exact denominator is the executable mapping minus the union of existing function bodies: **23,597 ranges / 1,830,644 bytes**. This is a byte denominator, not a function or semantic-unit denominator.
- One Ghidra headless process was launched against the existing source project with `-readOnly -noanalysis`; Ghidra 12.1.2_DEV (12.1.2) and OpenJDK 26.0.2.1 were reported by the existing evidence and the attempted run. The proposed aggregate-only script failed compilation before its query executed. Its failures were an incorrect API import and calls to unavailable convenience methods. No second process was launched, respecting the one-process limit.
- No function creation, analysis, disassembly, pseudocode export, index, or function-progress-ledger update was performed. The failed script was removed. Since its query did not execute, this report does not claim new datatype, unit-width/alignment, instruction-class, or reference-type aggregates.
- The classifications below are direct observations already established by the cited prior read-only reports, not fresh measurements in this task.

## Direct observations

### Complement-wide listing classes

| Existing listing class | Bytes | Share of exact complement |
|---|---:|---:|
| Defined instructions | 40,764 | 2.227% |
| Defined data | 1,789,880 | 97.773% |
| Undefined mapped / invalid / non-memory | 0 | 0% |
| **Total** | **1,830,644** | **100%** |

All bytes are initialized and mapped. The classifications sum exactly to the complement. The count of 23,597 gaps is a count of complement ranges, not a count of data structures or code fragments. No finer unit-size or alignment histogram was recovered.

### Seed placement and recorded incoming references

The assessed set is **13,897 unique seeds outside existing function bodies**. The corrected evidence uses containing-unit lookup, so interior addresses are not misreported as undefined.

| Containing listing class | Seed addresses | Distinct containing-unit bytes |
|---|---:|---:|
| Defined instructions | 77 | 308 |
| Defined data | 13,820 | 13,820 |
| Undefined mapped | 0 | 0 |
| Unmapped | 0 | 0 |
| **Total** | **13,897** | **14,128** |

Incoming-reference evidence for those seeds, from the correction report:

| Reference class | Unique targets | Recorded edges |
|---|---:|---:|
| Call | 8 | 11 |
| Jump (conditionality combined) | 64 | 66 |
| Other flow | 0 | 0 |
| Non-flow | 19 | 102 |
| **Any incoming reference** | **91** | **179** |

Target totals can overlap across reference classes. Sources for 63 targets / 149 edges are within existing function bodies; sources for 28 targets / 30 edges are outside function bodies but inside the executable mapping; none are outside the mapping. These are recorded references, not observed execution or proof of reachability. The earlier finer jump-subtype split is unconfirmed and is not reused.

### Candidate boundary evidence

The source-project reconciliation found 0 exact seed/function-entry matches, 652 seed addresses inside existing function bodies, and 13,897 outside. The seeds were not promoted to function starts. The separate exploratory-clone discrepancy of 1,425 manager entries remains isolated to that clone and does not alter these counts.

## What this supports—and what it does not

**Directly supported:** almost all uncovered bytes are already defined as data in the existing listing; all 13,897 outside-body seeds are contained by defined units; the instruction-classified portion at seed locations comprises 77 four-byte instruction units; incoming references fall into the aggregate classes above.

**Not established:** data-type categories or names, structure layouts, scalar widths, unit alignment, instruction mnemonics/classes, which referenced data fields are meaningful, valid new function boundaries, executable semantics, reachability, behavior, or binary matching. Ghidra's existing listing is a prior classification, not independent semantic proof. The fine-grained query could not be executed because its script did not compile, and the one-process cap precluded a corrective retry in this task. Inferring meaning from byte appearance, a seed's location, or reference records would exceed the direct evidence.

## Useful alternate characterization and next bounded task

The evidence supports a useful **structural triage**, not semantic understanding: retain the exact byte partition, stratify seed addresses by containing defined unit, and report incoming-reference classes and source contexts separately. This cleanly prioritizes the 77 instruction-contained seeds versus 13,820 data-contained seeds without relabeling the latter as code or treating references as reachability. Keep the 1,789,880 defined-data bytes and 40,764 instruction bytes as listing categories only.

Next bounded task: repair the aggregate query against the installed Ghidra API, then run at most one `-readOnly -noanalysis` process on the untouched source project to report only sanitized totals for generic data-unit categories, widths and alignment buckets, instruction flow/operand classes, instruction reference classes, and the same 13,897 seeds by containing-unit/evidence category. Do not print addresses, instruction text, game strings, custom datatype/type names, or bytes. Do not create functions, run analysis, disassemble, export pseudocode, or promote ledger states.

## Evidence and output boundary

Inputs: `p0-update-main-ghidra-range-reconciliation.md`, `p0-update-main-gap-classification.md`, `p0-update-main-gap-flow-triage-correction.md`, `p0-update-main-gap-candidate-validation.md`, and `p0-update-main-gap-flow-triage.md`. The only tracked output from this task is this report. The requested progress log is `work/progress/p0-wave3-main-gap.log`. No README, plan, census, ledger, or other report was changed. No commit or push was made.

**Correction/supersession (2026-10-06):** the failed initial query above
produced no query results. Use the repaired successful
[`detail`](p0-update-main-gap-semantic-detail.md) and authoritative
[`retry`](p0-update-main-gap-semantic-retry.md) reports for aggregate listing/API
metrics. Their “other” datatype and “other or unspecified” operand buckets are
generic API categories only, not semantic classifications. The corrected seed
containment/reference report remains authoritative; do not revive the earlier
`getCodeUnitAt` error or fine jump subtype split. P0 remains open, and ledger
states remain unchanged.
