# D4 depth rounds — new session continuation (round 31)

Same setup and truncation filter as rounds 27-30 (smallest remaining fully identical R##-generation clusters, 184-212 bytes; none skipped). 20 single-copy targets; boundary check and both identity gates passed for all 40 addresses. Most targets share a preamble (PRE: gated cast-and-convert of the first argument) followed by different tails (member-function-pointer call, typed store, tail calls); descriptions use shared macro definitions (PRE, MCALL, ACQ, REL) with per-target parameters. One independent auditor: 19 correct, 1 corrected (02668584: its loop is do-while with an unsigned bound test), 0 wrong.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `0233136c` | correct | 1 | 1 | 0 |
| `01230860` | correct | 1 | 1 | 0 |
| `010a0c90` | correct | 1 | 1 | 0 |
| `021beb4c` | correct | 1 | 1 | 0 |
| `026983c8` | correct | 1 | 1 | 0 |
| `02773b38` | correct | 1 | 1 | 0 |
| `0303b32c` | correct | 1 | 1 | 0 |
| `016246a4` | correct | 1 | 1 | 0 |
| `02071720` | correct | 1 | 1 | 0 |
| `02237904` | correct | 1 | 1 | 0 |
| `01f0324c` | correct | 1 | 1 | 0 |
| `01fa179c` | correct | 1 | 1 | 0 |
| `021aa794` | correct | 1 | 1 | 0 |
| `021be054` | correct | 1 | 1 | 0 |
| `022112d4` | correct | 1 | 1 | 0 |
| `02267258` | correct | 1 | 1 | 0 |
| `0231bffc` | correct | 1 | 1 | 0 |
| `02658f40` | correct | 1 | 1 | 0 |
| `02668584` | corrected | 1 | 1 | 0 |
| `00cdde04` | correct | 1 | 1 | 0 |

**Upgraded: 20. Held: 0.**

Decision: [`dec464`](../../sheets/decisions.tsv#dec464).

## Running total

| | |
|---|---|
| Remaining before this round | 4,932 |
| Round 31 (dec464) | +20 |
| **Remaining** | **4,912** |
