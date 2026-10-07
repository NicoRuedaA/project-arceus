# D4 batch 3 — twelve parallel analysts: large functions and a sample of hash users

Scope: update-v262144 `main`. Metadata only: no pseudocode, strings or game names (they stay in the git-ignored `work/d4/`). Evidence: `dec125`; protocol: [`odd/PARALLEL-ANALYSIS.md`](../../odd/PARALLEL-ANALYSIS.md).

## What was done

| Batch | Agents | Functions | Task |
|---|---:|---:|---|
| A | 8 | 33 | Functions of batch 2 that were too large or only partly read; analysts had to produce a contiguous section coverage map over the live decompiled lines |
| B | 4 | 64 | Small functions that load the engine hash basis, 8 per cluster for the 8 largest D3 clusters; analysts had to report hashed lookups |

Results after mechanical validation (all against the live function bodies):

| | Promoted | Main rejection reasons |
|---|---:|---|
| A | 19 of 33 | 7 over 6,000 bytes; 5 not fully read; 1 coverage map not contiguous; 1 claimed id not in the live body |
| B | 29 of 64 | 17 cite a hash literal that is not in the live body; 13 string/hash pair that does not recompute; 5 low confidence |

Ledger: 85 → 133 analyzed of 153,476 (0.0867 %). Names applied only for high confidence (7), in the working Ghidra project only. Behaviour verification and binary matching stay unknown.

## Findings

- **Agent self-confirmation is not evidence.** Agents marked 14 string/hash pairs as confirmed with the engine hash; recomputing them, only 1 matched. Every claimed pair is now recomputed by the validator and any function with a mismatch is not promoted.
- **The hash-user list is noisy.** Of the 64 sampled functions, most only store the hash basis as an "empty name" default, are constructors or wrappers, or are tiny non-hashing stubs; about 3 in 4 do not hash a string at all. Loading the basis identifies hash *state*, not hash *lookups*; use it to find candidates, not as proof of a lookup.
- **Large functions need the coverage map and still stall.** The live decompiler truncates very large bodies (one at 120,000 characters) or times out (functions of 20–40 KB); those stay unpromoted. Section coverage maps over the live lines were a workable control for functions up to 6,000 bytes.
- **Process lesson.** The first launch failed because the Ghidra server was not running (a start check that matched its own command line). Four agents refused to guess, which was the right behaviour; five used the exported pseudocode and were re-run against the live server except two hash-batch agents whose claims are validated mechanically instead.

## Limits

Analyst summaries are hypotheses; only ids, hash literals and string/hash pairs are mechanically checked. Callee semantics are not analysed, and functions over 6,000 bytes were not promoted.
