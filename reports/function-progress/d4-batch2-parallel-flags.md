# D4 batch 2 — parallel analysis of the functions that contain known flag/work ids

Scope: update-v262144 `main`. Metadata only: no pseudocode, no flag or game names (those stay in the git-ignored `work/d4/`). Evidence: `dec124`; protocol: [`odd/PARALLEL-ANALYSIS.md`](../../odd/PARALLEL-ANALYSIS.md).

## What was done

86 functions whose exported pseudocode contains a literal id from the seven flag/work tables (1,407 ids; 66 distinct ids found) were split into six address-ordered batches and analysed by six read-only agents in parallel. A single writer validated and applied the results.

| Step | Result |
|---|---:|
| Functions in the task | 86 (85 valid entries; 1 malformed id skipped) |
| Agent wall time (parallel) | about 2–7 minutes each |
| Claimed id sets found in the function's live body | 81 of 85 (the 4 misses are 20–40 KB functions) |
| Promoted to `analyzed_documented` | **42** |
| Excluded: body not fully read / too large (> 1,500 B) / low confidence / boundary or thunk / id not found | 11 / 16 / 9 / 6 / 1 |
| Names applied in the working Ghidra project (high confidence) | 16 |

Ledger: 43 → 85 analyzed of 153,476 (0.0554 %). Behaviour verification and binary matching stay unknown for all of them.

## Findings

- **Exported pseudocode can run into the neighbouring function.** Agents found three files where the export contains the next function's body (so ids appear in the wrong function), plus overlapping function ranges around three entries (`01355ff0`, `01356030`, `01359510`) and 8-byte adjuster thunks whose export shows the target inlined. All of these were excluded; the overlapping ranges need a boundary review in Ghidra.
- **Ids are not always flag reads or writes.** Some functions only store the ids in a record (pass-through); others compare, reset or add to a work value. Flag/work access goes through a small set of shared accessors (flag read, flag set, flag clear, work read, work write, work add), which the agents confirmed from their bodies.
- **Name evidence beyond the tables.** Hashing log-field names, comparison-operator tokens and table names with the engine hash identified several string-keyed lookups by name.
- **Boundary risk is the main quality risk of the export**, not the agents' reading: 81 of 85 id claims verified against live bodies.

## Limits

Agent summaries are hypotheses until verified by a writer; only the id-use claims are mechanically checked. Callee semantics (the accessors' own bodies aside) are not analysed; functions over 1,500 bytes were not promoted even when the agent reported a full read.
