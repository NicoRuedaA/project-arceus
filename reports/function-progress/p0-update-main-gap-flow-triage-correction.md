# P0 update-main gap flow-triage evidence correction

Date: 2026-10-06. Scope: update-v262144 `main.elf` in the untouched `PLA-update-fix2` Ghidra project. The update is a patch component; this report makes no standalone-program or whole-game/port-coverage claim.

## Verification and safety

- Source project: `/home/nico/work/decompilacion/work/pla/ghidra-projects/PLA-update-fix2.gpr`, project `PLA-update-fix2`, program `/main.elf`. Candidate input: the existing 14,549-row seed file under `work/p0-main-gap-candidates-20261006/addresses.txt`; no individual candidate values are reproduced.
- Launched exactly **one** Ghidra headless process against the source project with `-readOnly -noanalysis`. No function creation, analysis, disassembly, indexing, pseudocode export, or project writes were requested. The Java query read FunctionManager, Listing, Memory, and ReferenceManager only.
- The source project database tree was hashed immediately before and after using the same deterministic convention: sorted relative path, NUL, file size in decimal, NUL, per-file SHA-256 hex, newline; SHA-256 over the concatenation. Both measurements are 8 files / 709,739,522 bytes / `e36c645d66740527c7a00697f84d817eb036aaffbe279d037a5f9a9f43f05e98`: **stable**.
- The range reconciliation and gap-classification reports agree that the exact executable complement is 23,597 ranges / 1,830,644 bytes. This query independently rebuilt that complement from executable, initialized memory minus all existing function bodies.

## Corrected candidate listing result

`Listing.getCodeUnitContaining(address)` returns the existing CodeUnit whose address range contains the requested address, including when the requested address is in the middle of that unit. `Listing.getCodeUnitAt(address)` is an exact-start lookup; it returns a unit only when the requested address is that CodeUnit's minimum/start address. Therefore a null result from `getCodeUnitAt` does **not** establish that the address is undefined: the address may lie inside a defined instruction or data unit. The prior flow-triage result classified every seed with its unit-start-only lookup as undefined, which caused the contradiction.

For every one of the 13,897 candidates outside existing function bodies, the correction query used `getCodeUnitContaining`. Listing-type counts are unique candidate addresses; byte totals count each distinct containing unit once.

| Candidate classification | Addresses | Distinct CodeUnit bytes |
|---|---:|---:|
| Defined instruction | 77 | 308 |
| Defined data | 13,820 | 13,820 |
| Undefined mapped | 0 | 0 |
| Unmapped | 0 | 0 |
| **Total** | **13,897** | **14,128** |

The query found 13,897 distinct units, totalling 14,128 bytes: 308 instruction bytes plus 13,820 data bytes. Thus the original report's claim that 13,820 seed addresses were undefined is **wrong**. Those are addresses inside defined data units, not undefined addresses. This is a listing classification correction only; it does not establish function starts, executable semantics, reachability, or validity of the underlying data.

The same run recomputed byte-unit accounting across the **entire** exact complement:

| Complement listing class | Bytes |
|---|---:|
| Defined instructions | 40,764 |
| Defined data | 1,789,880 |
| Undefined mapped | 0 |
| **Total** | **1,830,644** |

This independently reproduces the prior gap-classification totals and confirms that the candidate data findings are consistent with the complement-wide defined-data classification.

## Incoming-reference recount

The run revisited incoming ReferenceManager records for the same outside-body candidate set. Counts below classify each edge using `ReferenceType.isCall()`, then `isJump()`, then `isFlow()`, otherwise non-flow. Ghidra's `isJump()` class groups jump references; it does not separate conditional from unconditional jumps. Unique targets are per class and are not additive across classes.

| Reference type class | Unique candidate targets | Edges |
|---|---:|---:|
| CALL | 8 | 11 |
| JUMP (`isJump`, conditionality combined) | 64 | 66 |
| Other flow (`isFlow` after CALL/JUMP) | 0 | 0 |
| Non-flow | 19 | 102 |
| **Any incoming reference** | **91** | **179** |

| Source context | Unique candidate targets | Edges |
|---|---:|---:|
| Within an existing function body | 63 | 149 |
| Outside function bodies, within executable mapping | 28 | 30 |
| Outside executable mapping | 0 | 0 |

There were 0 fallthrough targets / 0 fallthrough edges and 179 explicit edges. Overall incoming-reference totals, source-context totals, and fallthrough totals match the prior flow report. Its finer split of JUMP into 2 unconditional-jump targets/edges and 63 conditional-or-other-flow targets/64 edges cannot be validated from this query's aggregate `isJump()` output: the API predicate used here merges jump subtypes. Therefore treat that earlier *subtype split* as unconfirmed, not as disproven; the corrected aggregate is 64 jump-class targets / 66 edges. Reference records remain evidence of references only, not proof of reachability or function boundaries.

## Reproduction artifacts

- Temporary aggregate-only query: `work/p0-main-gap-flow-triage-correction/CorrectGapFlowListing.java`; Ghidra output: `work/p0-main-gap-flow-triage-correction/ghidra.log`. No addresses, instructions, strings, or bytes were emitted in the report.
- Progress log: `work/progress/p0-main-flow-triage-correction.log`.
- This is the only report created for the correction. Existing reports and canonical documentation were not edited; no commit or push was made.
