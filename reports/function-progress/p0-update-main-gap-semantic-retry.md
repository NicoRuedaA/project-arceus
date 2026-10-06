# P0 update-main gap semantic query retry

Date: 2026-10-06. Scope: aggregate listing metadata in the exact executable complement of existing function bodies for update-v262144 `main`. The update is a patch component; this report does not claim standalone execution, reachability, semantics, or port coverage.

## Validation and execution

- Reviewed the failed `p0-update-main-gap-semantic-triage.md` report and its progress log, the exact-range reconciliation, corrected seed-containment evidence, candidate validation, and corrected reference triage. The expected earlier per-run `ghidra.log` under `work/p0-main-gap-semantic-triage/` was absent; the failed report and `work/progress/p0-wave3-main-gap.log` record the compile failure (wrong reference-type package and unavailable `Data` convenience methods).
- Inspected existing scripts in `.tools/ghidra_scripts`, including `GapSeeds.java`, `P0MetadataOnly.java`, and the corrected `CorrectGapFlowListing.java`. Confirmed the installed Ghidra 12.1.2 API from JAR signatures with `javap` before launch. The repaired query uses generic `CodeUnit`/`Data`/`DataType` interfaces, `Data.getBaseDataType()`, `DataType.getLength()` and `getAlignment()`, `Array`/`Structure`/integer and float base interfaces, `Instruction.getFlowType()` and operand APIs, `OperandType` predicates, and `Listing.getCodeUnitContaining()`.
- Compiled the query with `javac -proc:none` against the installed Ghidra JAR classpath before launching Ghidra; compilation succeeded. The script did not import the failed reference type or use the unsupported `Data` convenience methods.
- Launched exactly **one** Ghidra headless process against `/home/nico/work/decompilacion/work/pla/ghidra-projects/PLA-update-fix2.gpr`, processing `/main.elf` with `-readOnly -noanalysis`. It exited 0 and emitted six aggregate result records. No function creation, analysis, disassembly, pseudocode export, indexing, ledger update, or project write was requested.
- The source project tree was verified immediately before and after: **8 files / 709,739,522 bytes**, SHA-256 `e36c645d66740527c7a00697f84d817eb036aaffbe279d037a5f9a9f43f05e98` both times.
- Query output and this report contain aggregates only: no addresses, type names, symbols, instruction text, strings, or bytes. The temporary Java query was removed after the run.

## Results

The executable complement is the established **23,597 ranges / 1,830,644 bytes**. The query independently counted its existing listing units.

### Data units

Categories are exclusive: arrays and structures by their Ghidra interfaces, primitive scalar bases by integer/float interfaces after `Data.getBaseDataType()`, and all remaining units as other. Byte totals count the portions within the complement; length buckets use the full `CodeUnit.getLength()` for each unique unit.

| Generic data category | Units | Bytes in complement |
|---|---:|---:|
| Primitive | 0 | 0 |
| Array | 0 | 0 |
| Structure | 0 | 0 |
| Other | 1,789,328 | 1,789,880 |
| **Total defined data** | **1,789,328** | **1,789,880** |

| CodeUnit length (bytes) | Units |
|---|---:|
| Unknown / zero | 0 |
| 1 | 1,789,238 |
| 2 | 1 |
| 3–4 | 17 |
| 5–8 | 72 |
| 9–16 | 0 |
| 17–32 | 0 |
| 33–64 | 0 |
| 65+ | 0 |
| **Total** | **1,789,328** |

| Ghidra datatype alignment metadata | Units |
|---|---:|
| Unknown / zero | 0 |
| 1 | 1,789,240 |
| 2 | 1 |
| 4 | 17 |
| 8 | 70 |
| 9–16 | 0 |
| 17+ | 0 |
| **Total** | **1,789,328** |

The length buckets here are **CodeUnit lengths**; they are not interchangeable with a histogram of datatype-reported lengths from other reports. Alignment is datatype metadata, not the address alignment of a listing unit.

### Instruction units

| Flow class | Instructions |
|---|---:|
| Call | 1,224 |
| Jump (conditionality combined) | 1,397 |
| Other flow | 7,570 |
| No flow | 0 |
| **Total** | **10,191** |

Instruction units cover **40,764 bytes**. Operand-width flags from the installed `OperandType` API were bucketed exclusively by byte, word, quadword, then other/unspecified; this is API-flag metadata, not decoded operand semantics.

| Operand-width flag bucket | Operands |
|---|---:|
| Byte | 0 |
| Word | 0 |
| Quadword | 0 |
| Other or unspecified | 20,627 |
| **Total** | **20,627** |

### Outside-body seed containment

The input contains **14,549 unique seeds**. After excluding the **652** already inside existing function bodies, all **13,897** remaining seeds were classified with `Listing.getCodeUnitContaining()`:

| Containing listing unit | Seed addresses | Distinct unit bytes |
|---|---:|---:|
| Instruction | 77 | 308 |
| Data | 13,820 | 13,820 |
| Undefined mapped | 0 | — |
| Unmapped | 0 | — |
| **Outside-body total** | **13,897** | **14,128** |

These counts describe the existing listing only; they do not validate function boundaries, references, reachability, or executable meaning.

## Boundaries and files

The result independently reproduces the previously established complement byte totals while adding generic datatype categories, CodeUnit length/alignment distributions, instruction flow/operand-flag buckets, and corrected seed containment. It does not establish data semantics/layouts, instruction semantics, function validity, runtime behavior, or binary matching. No canonical source, evidence ledger, README, inventory, or Ghidra project was modified; no commit or push was made.

Only repository report created for this task: this file. Timestamped progress: `work/progress/p0-wave5-gap-query.log`. Temporary Ghidra output was kept under `/tmp/opencode/`.
