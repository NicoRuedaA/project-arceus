# D4 batch 10 — audited propagation of group representatives

Scope: update-v262144 `main` (153,476 functions). Metadata only: no pseudocode, strings or game names (they stay in the git-ignored `work/d4/`). Evidence: `dec132`; grouping and method: [batch 9](d4-batch9-dedup-and-mechanical.md).

| Stream | Agents | Functions | Result |
|---|---:|---:|---:|
| Representatives of the next 155 largest identical groups not yet documented (12 bytes or more) | 9 | 155 | 146 promoted |
| Audit of every representative accepted in batch 9 | 6 | 178 | 148 correct, 30 wrong detail, 0 wrong main |
| Propagation of the 148 audited-correct descriptions to their identical copies | script | 9,875 rows | 8,445 game-side or unowned, 1,430 library |

Rejections in reading: 4 functions whose Ghidra body contains a second function (listed as boundary candidates, not changed), 4 that left out an import reached by a tail jump (re-queued), 1 low confidence.

Audit: the wrong details are mostly claims about other copies or callers that the body cannot show, wrong argument forms (a local copy passed instead of an address) and an unconditional step that is really conditional. All 30 were corrected in the working copy; their groups are propagated only after the corrected text passes a second audit (batch 11). Four names without support in the body were reverted to the default name.

Validator changes: when Ghidra's body is split into address ranges, import calls are now taken only from instructions inside those ranges (the disassembly listing also returned the neighbours between the ranges), and a reported split body is no longer rejected unless the analyst saw a second function inside it.

## Ledger

30,944 → **40,965** of 153,476 (26.69 %): 801 import stubs (link data), **1,189 read by analysts**, 10,667 copied from audited identical functions, 28,308 mechanically classified from instructions. Behaviour verification and binary matching stay unknown for every function.
