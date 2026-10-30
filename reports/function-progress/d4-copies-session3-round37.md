# D4 depth rounds — new session continuation (round 37)

Same setup and truncation filter as rounds 27-36 (smallest remaining fully identical R##-generation clusters, 324-360 bytes; none skipped). 10 single-copy targets; boundary check and both identity gates passed for all 20 addresses. Five targets share two templates (R, R2: script-binding argument-count checks); 02ea94e8 builds a scaled transform and a bounds midpoint/half-extent with NEON. One independent auditor: 10 correct, 0 corrected, 0 wrong.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `01230140` | correct | 1 | 1 | 0 |
| `00d04838` | correct | 1 | 1 | 0 |
| `0214ceb8` | correct | 1 | 1 | 0 |
| `00f0e234` | correct | 1 | 1 | 0 |
| `02ea94e8` | correct | 1 | 1 | 0 |
| `00f720ec` | correct | 1 | 1 | 0 |
| `0163cbac` | correct | 1 | 1 | 0 |
| `021779b4` | correct | 1 | 1 | 0 |
| `00f0eb88` | correct | 1 | 1 | 0 |
| `00f2a34c` | correct | 1 | 1 | 0 |

**Upgraded: 10. Held: 0.**

Decision: [`dec470`](../../sheets/decisions.tsv#dec470).

## Running total

| | |
|---|---|
| Remaining before this round | 4,842 |
| Round 37 (dec470) | +10 |
| **Remaining** | **4,832** |
