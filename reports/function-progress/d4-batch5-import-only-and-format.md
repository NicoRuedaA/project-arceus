# D4 batch 5 — 175 more functions: import-only helpers and text-formatting game functions

Scope: update-v262144 `main`. Metadata only: no pseudocode, strings or game names (they stay in the git-ignored `work/d4/`). Evidence: `dec127`; protocol: [`odd/PARALLEL-ANALYSIS.md`](../../odd/PARALLEL-ANALYSIS.md). Builds on [batch 4](d4-batch4-imports-and-relocations.md) (import calls and data pointers resolved).

## What was done

| Stream | Agents | Functions | Promoted |
|---|---:|---:|---:|
| Functions whose only direct callees are imports (one or two per distinct import set, most-called first; 24–1,500 bytes) | 10 | 160 | 150 |
| Game-side functions that call the formatted print/scan imports (64–1,500 bytes) | 2 | 28 | 25 |
| **Total** | **12** | **188** | **175** |

Rejected (13): 10 boundary anomalies reported by the analysts (blocks that are fragments of a neighbour, entry stubs falling into a shared body, one Ghidra function that holds two functions), 3 low confidence and 2 claims that did not match the live body (an import missing from the list, an id literal not in the body); two functions fall in two classes.

The import-only functions are mostly general utilities: string and container helpers, locking wrappers, easing and quaternion maths, hash helpers and library-internal routines (networking stack, Havok, Wwise). The text-formatting functions build resource paths and UI element names before a lookup.

## Quality control

Every function passed the mechanical validator against the live body. A writer spot check of two accepted summaries found one wrong detail in a high-confidence summary (the element size of a vector growth helper was 128 bytes, not 32); it was corrected from the live body before applying. Summaries remain analyst hypotheses beyond the mechanically checked claims; the next batch adds an independent audit of a sample.

## Ledger

1,034 → **1,209** analyzed of 153,476 (0.7877 %), of which **801 are 16-byte import stubs identified from the link data** and **408 were read by analysts or re-reviewed**. 30 high-confidence names applied in the working Ghidra copy only. Behaviour verification and binary matching stay unknown.

## New boundary findings (not applied)

`02d6bbb8`, `02c2cfec`, `00b1e8d8`, `00b381d8`, `0035d790`, `0051fe10`, `01483bd8`, `01951a40`, `01131270`, `0127b230`: bodies that include or are part of a neighbouring function according to the analysts. They need a boundary pass before they can be documented; fixing them changes the pinned inventory.
