# D1 — closing the executable gaps of update `main`

Scope: update-v262144 `main` NSO only (53,106,320 executable bytes). Metadata only: no bytes, instruction text, strings or names. The update is a patch component; this report makes no whole-game or port claim. Work done on `PLA-update-work` (a copy of the fix2 project; the fix2 reference project and its exported inventory are untouched).

## Result

| Metric | Before (fix2 reference) | After (working copy) |
|---|---:|---:|
| Functions | 153,476 | 153,476 (unchanged) |
| Bytes inside function bodies | 51,275,676 (96.55287 %) | 52,255,468 (98.40 %) |
| Bytes outside every body | 1,830,644 | 850,852 |
| Gap ranges | 23,597 | 21,703 |

**No missing functions were found.** The gap was mostly truncated function bodies, not undiscovered functions.

## Cause

Six constructor-like functions (object field initialisation, each returns normally and contains a `ret`) were flagged *no-return*. That cut every caller at the call and orphaned the rest of its body: 1,905 call sites in 1,380 functions (one callee alone had 906 callers). Clearing the six flags and recomputing the callers' bodies (`.tools/ghidra_scripts/FixNoReturn.java`, existing method) recovered 979,792 bytes (53.5 % of the gap).

Independent checks that the gaps were not separate functions: only 14 of 7,098 non-padding gap starts are pointed to by any 8-byte value in the data segments, against 60,796 of 153,476 (39.6 %) real function entries; and 99.9 % of the `bl` calls that decode inside gaps land exactly on existing function entries (they are the callers' own continuation).

## Remaining 850,852 bytes (21,703 ranges), every byte classified

| Class | Ranges | Bytes | Reason / evidence |
|---|---:|---:|---|
| Padding, all zero | 14,527 | 116,500 | every byte zero |
| Padding, repeated 4-byte word | 2,112 | 8,908 | one word repeated |
| Code, strong | 531 | 42,072 | decodes fully, ends in a terminal instruction, its calls hit function entries |
| Code, plausible | 28 | 1,768 | decodes fully, ends terminal, no calls |
| Code, possible | 1,355 | 122,300 | decodes fully, calls hit entries, no terminal |
| Decodes only weakly | 357 | 11,028 | decodes fully, no corroboration (AArch64 decodes almost any data) |
| Jump-table candidates | 100 | 73,660 | directly after a computed jump |
| Data or undecided | 2,693 | 474,616 | does not decode as straight-line code |

Interpretation limits: the code classes are evidence tiers, not function boundaries; no function was created. Ranges that follow 20 remaining no-return functions (183 ranges, 32,360 bytes) were not changed: those functions contain no `ret` except one (a 24-byte function, 54 callers) that still needs a manual check.

## Consequences

- The reference inventory (fix2) and everything derived from it (153,471 C exports, gameDB index, treemap weights) still describe the old bodies. The 1,380 functions whose bodies grew need re-export before they are analysed (D4); exports are not stale for the other functions.
- None of the 38 ledger functions changed body size, so no ledger state was downgraded.
- The remaining 850,852 bytes are not a function-count problem; the denominator stays 153,476.

## Reproduction

Read-only Ghidra headless scripts in `.tools/ghidra_scripts/`: `GapClassify`, `GapProbe` (trial disassembly inside a rolled-back transaction), `GapPtrScan`, `GapPrev`, `NoReturnAudit`, `FuncSizes`; correction by `FixNoReturn`. Per-range output lives in `work/d1/` (git-ignored).
