# D4 batch 9 — identical-function grouping, mechanical classification and representatives

Scope: update-v262144 `main` (153,476 functions). Metadata only: no pseudocode, strings or game names (they stay in the git-ignored `work/d4/`). Evidence: `dec131`; protocol: [`odd/PARALLEL-ANALYSIS.md`](../../odd/PARALLEL-ANALYSIS.md).

## Stream A — byte-level grouping (script, no model)

Every function body (Ghidra body ranges, including 2,328 non-contiguous bodies) was read from the update `main` image. PC-relative operands (branches, calls, `adrp`/`adr`, literal loads) were resolved: local targets as offsets, external targets as absolute addresses.

| Level | Rule | Groups | Functions in groups of 2+ |
|---|---|---:|---:|
| EXACT | same instruction words, same call/jump targets, same data addresses | 109,550 | 53,912 (9,986 groups) |
| SHAPE | as EXACT, but external targets ignored | 79,451 | 85,210 |

Only EXACT groups may share a description; SHAPE groups are hints and never promote a function. Largest EXACT groups are tiny (5,904 copies of a single-instruction return). 197 bodies do not end in a return or branch (possibly cut) and are never propagated. The grouping found no overlapping body ranges. A classifier agent compared the live disassembly of 2–3 members of 45 sampled groups (30 EXACT, 15 SHAPE): no merge error; 27 groups were repeated game-side templates, 16 library code, 2 unknown.

## Stream D — mechanical classification (script, no model)

Straight-line bodies of at most 8 instructions of a supported subset (moves, immediate loads, loads and stores at fixed offsets, `adrp`/`add`, `ret`, `b`, `br`, traps; no conditional branch, call, stack, frame or callee-saved register) were evaluated symbolically from the bytes. The result is the function's complete behaviour, recorded as a pattern (for example, "returns the 32-bit field at +0x18 of x0"). 29,112 functions matched; 28,308 were new to the ledger (the rest were import stubs). Independent check: 400 random matches were re-derived from Ghidra's disassembly by a separately written evaluator. The classifier and the check agreed on all 400. A first pass reported 2 disagreements, both caused by a rule in the check script; the classifier was right in both.

| Class | New rows | Class | New rows |
|---|---:|---|---:|
| this-pointer adjuster + tail call | 7,219 | getter (field at fixed offset) | 1,468 |
| no-op (returns immediately) | 6,184 | other straight-line combination | 799 |
| tail call, arguments unchanged | 5,072 | vtable-slot forwarder | 552 |
| field copy | 1,765 | returns a global address | 545 |
| returns a constant | 1,725 | stores constants | 489 |
| tail call with set arguments | 1,503 | setter / member address / global getter / chained getter / other | 993 |

22,450 of the 28,308 are on the game side or have no owner (D2), 5,858 are library code. These rows describe exactly what the instructions do. They do not say what the function is for, so they are counted separately from functions read by analysts.

## Stream B — reading representatives

| Stream | Agents | Functions | Promoted |
|---|---:|---:|---:|
| One representative of each of the 190 largest EXACT groups (48 bytes or more) | 12 | 190 | 178 |
| Audit of 71 earlier descriptions (51 representatives of groups with a documented member, 20 random from batch 8) | 2 | 71 | — |

Rejections: 8 functions reported as ending in a tail jump inlined by the decompiler (flagged as a boundary anomaly; re-queued with clarified instructions), 3 with import callees missing from the claim, 1 low confidence. 24 of the 178 were marked as library templates by the analysts. 45 names applied in the working copy (high confidence only).

Audit: 61 correct, 10 wrong detail (corrected in the working copy), 0 wrong main claim.

## Stream C — propagation

The descriptions of audited-correct representatives were copied to their identical copies: **792 rows** (610 game-side or unowned, 182 library). Each row names its representative and its EXACT group. The 178 new representatives (about 12,000 copies) are propagated only after their own audit in batch 10.

## Ledger

1,666 → **30,944** of 153,476 (20.16 %): 801 import stubs (link data), **1,043 read by analysts**, 792 propagated from audited identical functions, 28,308 mechanically classified from instructions. Behaviour verification and binary matching stay unknown for every function.
