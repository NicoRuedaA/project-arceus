# D4 batch 7 — the most-called game-side leaf functions, and a second audit

Scope: update-v262144 `main`. Metadata only: no pseudocode, strings or game names (they stay in the git-ignored `work/d4/`). Evidence: `dec129`; protocol: [`odd/PARALLEL-ANALYSIS.md`](../../odd/PARALLEL-ANALYSIS.md).

## What was done

Bottom-up order: functions that call no other game function are documented first, starting with the most-called ones, because their descriptions help every caller.

| Stream | Agents | Functions | Promoted |
|---|---:|---:|---:|
| Game-side functions (no library evidence), 24–1,000 bytes, calling only imports or nothing, by caller count (47,801 down to 24 callers) | 10 | 160 | 159 |
| Independent audit of summaries accepted in batches 5 and 6 (random sample) | 2 | 60 | — |

One function was rejected because a claimed constant is not in its body. The promoted functions include Lua 5.3 C API functions linked into the game (stack, push, type and conversion functions), string and container helpers, list and tree operations, hash-map lookups, maths and filter steps, random generators, and accessors of an encrypted, checksummed record.

## Audit

| Verdict | Batch 6 audit (batches 4–5) | This audit (batches 5–6) | Total |
|---|---:|---:|---:|
| Correct | 54 | 55 | 109 |
| Minor error (wrong detail) | 6 | 5 | 11 |
| Wrong main claim | 0 | 0 | 0 |

The five new minor errors (swapped output arguments, an omitted condition or null check, a nesting order, a misdescribed helper) were corrected in the working Ghidra copy and the ledger notes record the audit. Over 120 audited summaries, about 9 % contain a wrong detail and none was wrong in its main claim.

## Ledger

1,349 → **1,508** analyzed of 153,476 (0.9826 %), of which **801 are 16-byte import stubs identified from the link data** and **707 were read by analysts or re-reviewed**. 52 high-confidence names applied in the working copy only. Behaviour verification and binary matching stay unknown.
