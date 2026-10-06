# P0 update-main outside-body seed flow-reference triage

Date: 2026-10-06. Scope: update-v262144 `main` NSO only, using the untouched `PLA-update-fix2` Ghidra project. The update is a patch component; this report makes no standalone-program or whole-game/port-coverage claim.

## Method and safety

- Read the range reconciliation, gap classification, and candidate validation reports. The range reconciliation records **0** exact entry matches among the 14,549 seeds, with 652 inside existing function bodies and 13,897 outside; the seed set's exact entry intersection is rechecked below.
- Queried `/main.elf` in `/home/nico/work/decompilacion/work/pla/ghidra-projects/PLA-update-fix2.gpr` only. Both headless invocations used `-readOnly -noanalysis`; the successful Java script read the existing FunctionManager, Listing, Memory, and ReferenceManager. It did not analyze, disassemble, create functions, or export pseudocode.
- There were **2 sequential headless invocations**, never more than **1 concurrent Ghidra headless process**. The first stopped before script execution because Python was unavailable without PyGhidra; the second completed with a Java Ghidra script. No candidate values were printed. The temporary query script was removed after collection.
- The script filtered the candidate set against existing function bodies, then queried listing type and incoming references for the 13,897 outside-body addresses. The executable interval used for source context is the existing report's PT_LOAD range; source addresses in that interval but outside every function body are classed as outside-body executable.

## Aggregate findings

### Candidate placement and listing type

| Measure | Unique candidate addresses |
|---|---:|
| Candidate set total | 14,549 |
| Exact entry-body seed intersection | 0 |
| Inside existing function bodies | 652 |
| Outside existing function bodies (triage set) | 13,897 |
| Outside-body candidates at instructions | 77 |
| Outside-body candidates at defined data | 0 |
| Outside-body candidates at undefined mapped addresses | 13,820 |
| Outside-body candidates at unmapped addresses | 0 |

The listing-type buckets reconcile to all 13,897 candidates. The 77 instruction-address candidates resolve to 77 distinct listing instructions, totaling 308 instruction bytes; no defined-data listing units were reached, so their distinct-unit byte total is 0. Undefined/unmapped candidate-byte totals are not meaningful as listing-unit sizes and are not assigned. Listing classification is descriptive of the existing project state, not proof that any address is a function start.

### Incoming references to outside-body candidates

Counts below separate distinct candidate targets from reference edges. A target may have edges of more than one class, so unique-target counts across classes are not additive.

| Reference classification | Unique candidate targets | Reference edges |
|---|---:|---:|
| CALL | 8 | 11 |
| Unconditional JUMP | 2 | 2 |
| Conditional or other flow | 63 | 64 |
| Non-flow | 19 | 102 |
| **Any incoming reference** | **91** | **179** |

### Source context and fallthrough

| Source context | Unique candidate targets | Reference edges |
|---|---:|---:|
| Within an existing function body | 63 | 149 |
| Outside all function bodies but within executable mapping | 28 | 30 |
| Outside executable mapping | 0 | 0 |

Ghidra's reference-type API supports identifying fallthrough references. No incoming reference to this candidate set was classified as fallthrough: **0 targets / 0 edges**. The remaining **91 targets / 179 edges** were non-fallthrough (explicit) references; this includes both flow and non-flow references, not only control-flow edges.

## Interpretation and limits

- Direct CALL/JUMP/other-flow references are evidence of recorded references only. They do not establish reachability, function boundaries, or semantic validity. Conversely, the 13,806 candidates without an incoming reference are not thereby invalid.
- The reference counts reflect the current ReferenceManager records and the candidate target addresses only. This does not add outgoing-reference, callgraph reachability, recursive traversal, disassembly, or behavioral evidence.
- The listing byte total counts each distinct instruction listing unit reached by at least one candidate once; candidate-address counts remain unique-address counts. There are no data-unit bytes in this set.
- No function-progress evidence, ledger, README/PLAN, census, source, or inventory was changed. No gameDB index, commit, or push was performed. The project is still the untouched source project; the candidate-mutated clone was not opened.

Progress log: `work/progress/p0-main-gap-flow-triage.log`.

## Correction addendum — 2026-10-06

The original candidate-placement and reference tables above are historical and are superseded where corrected below. The query used `Listing.getCodeUnitAt(address)`, an exact-start lookup; it incorrectly treated candidate addresses inside defined listing units as undefined. The correction used `Listing.getCodeUnitContaining(address)`: of 13,897 outside-body seeds, 77 are in defined instructions (308 distinct instruction bytes), 13,820 are inside defined data units (13,820 bytes), 0 are undefined, and 0 are unmapped. Distinct containing units total 14,128 bytes. The whole complement remains 40,764 instruction bytes + 1,789,880 defined-data bytes + 0 undefined = 1,830,644 bytes.

Corrected incoming-reference aggregate: CALL 8 targets/11 edges; JUMP (`ReferenceType.isJump`, conditionality combined) 64/66; other flow 0/0; non-flow 19/102; total 91 targets/179 edges. Source contexts remain 63 targets/149 edges within functions, 28/30 outside functions but within executable mapping, and 0/0 outside executable; fallthrough remains 0/0. The former 2 unconditional + 63 conditional/other-flow subtype split is unconfirmed, not disproven, and is superseded for current reporting by the combined JUMP class. Listing/reference metadata does not establish function validity, reachability, or semantics. The next work is semantic triage of defined data/instruction gaps and candidate validity; no function ledger state is promoted. Detailed authoritative evidence: [correction report](p0-update-main-gap-flow-triage-correction.md).
