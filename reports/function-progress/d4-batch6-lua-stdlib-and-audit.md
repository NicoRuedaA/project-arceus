# D4 batch 6 — Lua standard library from its registration tables, high-caller helpers, and an audit

Scope: update-v262144 `main`. Metadata only: no pseudocode, strings or game names (they stay in the git-ignored `work/d4/`). Evidence: `dec128`; protocol: [`odd/PARALLEL-ANALYSIS.md`](../../odd/PARALLEL-ANALYSIS.md).

## New evidence source: registration tables in data

Since the data relocations were applied ([batch 4](d4-batch4-imports-and-relocations.md)), tables of (name pointer, function pointer) pairs in data can be read. Ten such tables with a 16-byte stride are the Lua 5.3 standard-library registration tables (`luaL_Reg`): base, bit32 (compatibility build), coroutine, debug, io, io file methods, math, string, table and utf8 — 135 registrations, 132 distinct functions in the inventory. The registration proves which library entry a function is; the analysts then checked each body against the Lua 5.3 function.

## Results

| Stream | Agents | Functions | Promoted |
|---|---:|---:|---:|
| Lua 5.3 standard-library C functions (registration re-checked by the writer) | 9 | 132 | 126 |
| Game-side helpers whose only callees are imports, most-called first (up to 6,921 callers) | 1 | 16 | 14 |
| **Total** | **10** | **148** | **140** |

Rejected: 5 thin entry stubs that tail-jump into a body shared with another library function (the live decompiler inlines the shared body), 2 import-list mismatches (one of them also a stub) and 1 decompiler/disassembly mismatch; 1 claimed constant not in the body.

All promoted Lua functions match the stock Lua 5.3 functions; none is disabled or replaced by the game. The build includes the 5.3 compatibility options (bit32 library, old math functions, `__ipairs`). Their Lua API callees are identified only by use and remain unnamed.

## Audit of earlier accepted summaries

Two independent auditors re-read 60 randomly sampled functions accepted in batches 4 and 5, without trusting the accepted text:

| Verdict | Count |
|---|---:|
| Correct | 54 |
| Minor error (wrong detail, main purpose right) | 6 |
| Wrong main claim | 0 |

The minor errors were offsets, the number of vtable stores, an off-by-one buffer bound, an omitted global write and one name the body does not prove. They were corrected in the working Ghidra copy and the six ledger notes now record the audit. Estimated rate of wrong details in accepted summaries: about 1 in 10; of wrong main claims: none found in 60.

## Ledger

1,209 → **1,349** analyzed of 153,476 (0.8790 %), of which **801 are 16-byte import stubs identified from the link data** and **548 were read by analysts or re-reviewed**. 84 high-confidence names applied in the working copy only. Behaviour verification and binary matching stay unknown.
