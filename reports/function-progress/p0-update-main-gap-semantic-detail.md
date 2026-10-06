# P0 update-main gap semantic detail

Date: 2026-10-06. Scope: existing listing units in the executable complement of function bodies. These are structural listing aggregates only; they do not establish semantics, reachability, or port coverage.

## Method and execution

- Read the prior failed triage report and its progress log, the Spreadsheet Method reference, the corrected seed-containment report, existing scripts, and the installed Ghidra 12.1 API/JAR signatures before preparing the query.
- Ran exactly **one Ghidra process** against the untouched source project using `-readOnly -noanalysis`. The aggregate-only query read the existing executable complement, listing units, and seed file. It did not create functions, analyze, disassemble, export pseudocode, or write evidence ledgers. The process exited 0.
- The first wrapper invocation used a command not on `PATH` and exited before launching Ghidra; the installed launcher was then located and used for the sole Ghidra process.
- Output was limited to generic counts; no candidate values, addresses, game-specific symbol/type names, strings, or bytes were emitted by the query or reproduced here.

## Aggregate results

### Defined data units

Datatype classification used the installed API's `Array` and `Structure` interfaces; primitive means a base datatype recognized as an integer or floating-point scalar after typedef unwrapping. All remaining datatypes are grouped as other. Counts are unique listing units; bytes are their full CodeUnit lengths.

| Generic data category | Units | Bytes |
|---|---:|---:|
| Primitive | 0 | 0 |
| Array | 0 | 0 |
| Structure | 0 | 0 |
| Other | 1,789,328 | 1,789,880 |
| **Total** | **1,789,328** | **1,789,880** |

| Datatype size bucket (bytes) | Units |
|---|---:|
| Zero or unknown | 2 |
| 1 | 1,789,238 |
| 2 | 1 |
| 3–4 | 17 |
| 5–8 | 70 |
| 9–16 | 0 |
| 17–32 | 0 |
| 33–64 | 0 |
| 65+ | 0 |

| Datatype alignment reported by Ghidra | Units |
|---|---:|
| Zero or unknown | 0 |
| 1 | 1,789,240 |
| 2 | 1 |
| 4 | 17 |
| 8 | 70 |
| 9–16 | 0 |
| 17+ | 0 |

These alignment buckets describe datatype alignment metadata, not the memory address alignment of each unit. The size/alignment totals each sum to the 1,789,328 data units.

### Existing instruction units

| Flow category | Instructions |
|---|---:|
| Call | 1,224 |
| Jump (conditionality combined) | 1,397 |
| Other flow | 7,570 |
| No flow | 0 |
| **Total** | **10,191** |

The instruction units cover **40,764 bytes**, consistent with 10,191 four-byte instruction units. Operand-width flags were available through the installed `OperandType` API; the query counted **20,627 operands** in the fallback `other or unspecified` category, with **0** flagged byte, **0** word, and **0** quadword. This is an API-flag classification, not a decoded semantic width analysis.

### Seed containment

For all assessed seeds, the query used `Listing.getCodeUnitContaining(address)` and excluded seeds inside existing function bodies. Results agree with the corrected baseline:

| Containing unit | Seed addresses | Distinct containing-unit bytes |
|---|---:|---:|
| Instruction | 77 | 308 |
| Data | 13,820 | 13,820 |
| Undefined mapped | 0 | 0 |
| Unmapped | 0 | 0 |
| **Total** | **13,897** | **14,128** |

The query itself counted 13,897 unique containing units for these seeds. Containment remains listing evidence only; it does not establish that a seed is a valid function start or that referenced data is meaningful.

## Limits and outcome

This completed the finer aggregate query that the previous report could not run. It establishes generic data-unit size/alignment distributions, generic instruction flow/operand-flag distributions, and corrected seed containment. It does **not** establish datatype semantics or layouts, instruction mnemonics, reference meaning, function boundaries, execution, behavior, or binary matching. In particular, “other” is the remainder after the explicitly stated generic datatype tests, not a claim that all such units share one semantic type.

No canonical documentation, function-progress ledger, other report, or Ghidra project was modified. No gameDB indexing, commit, or push was performed.
