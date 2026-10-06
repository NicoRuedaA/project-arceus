# P0 update-main complement reference-strata triage (PT_LOAD correction)

Date: 2026-10-06. Scope: the exact executable PT_LOAD interval in update-v262144 `main.elf`, minus existing function bodies in the untouched `PLA-update-fix2` Ghidra project. The update is a patch component; this report makes no standalone-program, reachability, or port-coverage claim.

## Outcome and method

The earlier attempt was invalid because it searched for a memory block named exactly `.text` and therefore formed an empty range. This corrected query did **not** use block names. It constructed the executable address set explicitly as `[0x0, 0x32a568f]`, corresponding to PT_LOAD `[0x0, 0x32a5690)`, then subtracted the union of actual `Function.getBody()` address sets.

The resulting complement is **23,597 ranges / 1,830,644 bytes**, independently matching the existing range-reconciliation and gap-classification reports. The query visited reference source locations from initialized memory once using the validated `ReferenceManager.getReferenceSourceIterator(AddressSetView, boolean)` API; for each source it visited the existing `getReferencesFrom` records once. The run traversed 2,799,393 source locations and 2,830,748 reference records. It aggregated only edges with at least one endpoint in the complement.

Location classes are exclusive, with this precedence: existing function body; complement instruction; complement data (other defined code units); other address within the explicit executable PT_LOAD; outside that executable interval. The additional outside-executable class keeps references crossing the PT_LOAD boundary visible rather than dropping them. Inbound unique counts are distinct counterpart **source** locations; outbound unique counts are distinct counterpart **target** locations. Unique counts are per direction and class, not edge counts.

Flow buckets are exclusive and ordered: CALL (`isCall`), JUMP (`isJump`, conditionality combined), OTHER_FLOW (`isFlow` after CALL/JUMP), then NON_FLOW. This is a classification of stored reference records, not a control-flow graph or reachability analysis.

## Reference strata

| Counterpart location class | Inbound unique sources | Inbound edges | Outbound unique targets | Outbound edges |
|---|---:|---:|---:|---:|
| Existing function body | 41,504 | 43,596 | 330 | 1,280 |
| Complement instruction | 1,231 | 1,231 | 929 | 1,221 |
| Complement data | 0 | 0 | 3 | 10 |
| Other executable | 0 | 0 | 0 | 0 |
| Outside executable PT_LOAD | 5 | 5 | 40 | 156 |
| **Total** | **42,740** | **44,832** | **1,302** | **2,667** |

## Non-overlapping flow-type breakdown

Each row reports unique counterpart locations and edge counts using the same direction definitions above.

| Flow class | Counterpart location class | Inbound unique sources | Inbound edges | Outbound unique targets | Outbound edges |
|---|---|---:|---:|---:|---:|
| CALL | Existing function body | 11 | 11 | 226 | 1,111 |
| CALL | Complement instruction | 0 | 0 | 0 | 0 |
| CALL | Complement data | 0 | 0 | 0 | 0 |
| CALL | Other executable | 0 | 0 | 0 | 0 |
| CALL | Outside executable PT_LOAD | 0 | 0 | 0 | 0 |
| **CALL total** |  | **11** | **11** | **226** | **1,111** |
| JUMP | Existing function body | 172 | 173 | 96 | 130 |
| JUMP | Complement instruction | 1,221 | 1,221 | 929 | 1,221 |
| JUMP | Complement data | 0 | 0 | 0 | 0 |
| JUMP | Other executable | 0 | 0 | 0 | 0 |
| JUMP | Outside executable PT_LOAD | 0 | 0 | 0 | 0 |
| **JUMP total** |  | **1,393** | **1,394** | **1,025** | **1,351** |
| OTHER_FLOW | Existing function body | 0 | 0 | 0 | 0 |
| OTHER_FLOW | Complement instruction | 0 | 0 | 0 | 0 |
| OTHER_FLOW | Complement data | 0 | 0 | 0 | 0 |
| OTHER_FLOW | Other executable | 0 | 0 | 0 | 0 |
| OTHER_FLOW | Outside executable PT_LOAD | 0 | 0 | 0 | 0 |
| **OTHER_FLOW total** |  | **0** | **0** | **0** | **0** |
| NON_FLOW | Existing function body | 41,321 | 43,412 | 12 | 39 |
| NON_FLOW | Complement instruction | 10 | 10 | 0 | 0 |
| NON_FLOW | Complement data | 0 | 0 | 3 | 10 |
| NON_FLOW | Other executable | 0 | 0 | 0 | 0 |
| NON_FLOW | Outside executable PT_LOAD | 5 | 5 | 40 | 156 |
| **NON_FLOW total** |  | **41,336** | **43,427** | **55** | **205** |

The flow totals reconcile to 44,832 inbound and 2,667 outbound edges. The 43,596 inbound edges from existing function bodies reproduce the earlier executable-source complement count; the corrected full-source-domain result additionally includes references from complement instructions and outside-executable sources.

## Verification, safety, and limits

- Inspected existing Ghidra Java query scripts and validated installed API signatures with `javap`, including `ReferenceManager.getReferenceSourceIterator(AddressSetView, boolean)`, `ReferenceManager.getReferencesFrom(Address)`, `ReferenceIterator`, function bodies, and listing containment. The first standalone compile omitted Ghidra's Features classpath and failed; after correcting the classpath, the query compiled successfully with `javac -proc:none` before launch.
- Launched exactly **one** Ghidra headless process against `/home/nico/work/decompilacion/work/pla/ghidra-projects/PLA-update-fix2.gpr` with `-readOnly -noanalysis`; it completed and returned aggregate counts. No function creation, auto-analysis, disassembly, pseudocode export, gameDB indexing, ledger/canonical-document edits, or project mutation was requested.
- Source-project database tree fingerprint before and after: 8 files / 709,739,522 bytes / `e36c645d66740527c7a00697f84d817eb036aaffbe279d037a5f9a9f43f05e98` — unchanged.
- Reference iteration covers initialized-memory source locations. It does not establish the absence of references whose source is outside that iterator domain, nor does any reference count prove reachability, function boundaries, valid code, behavior, or binary matching.
- Output reports aggregates only: no addresses, IDs, function names, text, symbols, or bytes. The previously existing untracked failed report at this path was collision-checked and replaced with this corrected result. Timestamped progress: `work/progress/p0-wave8-main-gap-refs.log`.
