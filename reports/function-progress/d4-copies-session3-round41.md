# D4 depth rounds — new session continuation (round 41)

Same setup and truncation filter as rounds 27-40, size range widened to 800 bytes (380-404 bytes taken; 2 candidates skipped as truncated, 4 irregular ones returned to the pool). 16 single-copy targets, all instances of one template S (script-binding argument-count check that initializes the constructed object only when the count matches); per-target parameters and INIT stores extracted by script. Boundary check and both identity gates passed for all 32 addresses. One independent auditor checked the template against every body and every row: 16 correct, 0 corrected, 0 wrong.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `021a042c` | correct | 1 | 1 | 0 |
| `00dcecec` | correct | 1 | 1 | 0 |
| `00f79340` | correct | 1 | 1 | 0 |
| `01668da4` | correct | 1 | 1 | 0 |
| `020dc598` | correct | 1 | 1 | 0 |
| `00dc1564` | correct | 1 | 1 | 0 |
| `00e0bed4` | correct | 1 | 1 | 0 |
| `01eeb054` | correct | 1 | 1 | 0 |
| `01070f40` | correct | 1 | 1 | 0 |
| `022e96d8` | correct | 1 | 1 | 0 |
| `02048e60` | correct | 1 | 1 | 0 |
| `022e1418` | correct | 1 | 1 | 0 |
| `022f5848` | correct | 1 | 1 | 0 |
| `023a90d0` | correct | 1 | 1 | 0 |
| `023c3514` | correct | 1 | 1 | 0 |
| `00802e18` | correct | 1 | 1 | 0 |

**Upgraded: 16. Held: 0.**

Decision: [`dec474`](../../sheets/decisions.tsv#dec474).

## Running total

| | |
|---|---|
| Remaining before this round | 4,792 |
| Round 41 (dec474) | +16 |
| **Remaining** | **4,776** |
