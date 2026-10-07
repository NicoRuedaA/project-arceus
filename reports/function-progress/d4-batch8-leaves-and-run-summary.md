# D4 batch 8 — more most-called leaf functions, and summary of the batch 4–8 run

Scope: update-v262144 `main`. Metadata only: no pseudocode, strings or game names (they stay in the git-ignored `work/d4/`). Evidence: `dec130`; protocol: [`odd/PARALLEL-ANALYSIS.md`](../../odd/PARALLEL-ANALYSIS.md).

## Batch 8

| Stream | Agents | Functions | Promoted |
|---|---:|---:|---:|
| Next game-side leaf functions by caller count (24–1,000 bytes, no game callees) | 10 | 160 | 158 |
| Independent audit of batch 7 (random sample) | 2 | 60 | — |

Two functions were rejected: each ends in a tail branch to an import that the writer could not match to the live callee list. **96 of the 158 are copies of one libc++ template (string find)**, and 3 of string pop-back, instantiated in many translation units; each is a separate inventory function with a fully read body, but they are library template code that D2 could not separate, not game logic. The rest are reference-count and pointer helpers, table and hash lookups, curve and matrix maths, random generators and accessors of the encrypted, checksummed record.

Audit of batch 7: 59 correct, 1 wrong detail (an inverted flag meaning), corrected in the working copy.

## Run summary (batches 4–8, five rounds)

| | Value |
|---|---:|
| Rounds / analyst agents launched | 5 / 60 (+1 short re-run) |
| Functions analysed by agents (new) | 761 proposed, 732 promoted |
| Earlier entries re-reviewed | 64 (9 contradicted, all corrected) |
| Import stubs documented from link data | 801 |
| Ledger, analyzed | 133 → **1,666** of 153,476 (1.0855 %) |
| of which import stubs / read by analysts or re-reviewed | **801 / 865** |
| Audited summaries (random samples, 3 audits of 60) | 180: 168 correct, 12 wrong detail, 0 wrong main claim |

Infrastructure fixed in the working copy only: the import calls (816/816 PLT slots) and the 205,023 data relocations; 64 libc prototypes and 14 no-return flags. Behaviour verification and binary matching stay unknown for every function.

## Ledger

1,508 → **1,666** analyzed of 153,476 (1.0855 %), of which **801 are 16-byte import stubs identified from the link data** and **865 were read by analysts or re-reviewed**. 101 high-confidence names applied in the working copy only.

## What remains and next steps

- The caller-ranked leaf pool still holds about 11,600 game-side candidates, but its tail is dominated by duplicated library templates; a byte-level deduplication of template copies (and their reclassification as library code in D2) should come before more reading.
- About 5,500 functions call only imports; most are library-internal.
- Boundary fixes found by the analysts (a function hidden inside `01355ff0`, fragments counted as functions, thin entry stubs) need one inventory pass; they change the pinned denominator, so they were not applied.
- The exported pseudocode predates the import and relocation fixes and should be regenerated from the working copy.
- The Lua API functions linked on the game side and the remaining Lua registration tables (game bindings) are the next direct-evidence source; the callers of the documented helpers can now be read bottom-up.
