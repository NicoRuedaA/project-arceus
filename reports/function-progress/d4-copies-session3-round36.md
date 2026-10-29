# D4 depth rounds — new session continuation (round 36)

Same setup and truncation filter as rounds 27-35 (smallest remaining fully identical R##-generation clusters, 272-316 bytes; none skipped). 10 single-copy targets; boundary check and both identity gates passed for all 20 addresses (00264a2c's two holes hold the cbz after 0x032a24f0, inside the compared span). 031deb54 emits a Havok hkArrayUtil 'Array size overflow' report for a negative count. One independent auditor: 9 correct, 1 corrected (01d65c40: x11 is caller-saved across the fallback call), 0 wrong.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `021a01c0` | correct | 1 | 1 | 0 |
| `02421748` | correct | 1 | 1 | 0 |
| `031deb54` | correct | 1 | 1 | 0 |
| `00a22bc0` | correct | 1 | 1 | 0 |
| `00ff5c80` | correct | 1 | 1 | 0 |
| `01d65c40` | corrected | 1 | 1 | 0 |
| `0201f2b4` | correct | 1 | 1 | 0 |
| `0234631c` | correct | 1 | 1 | 0 |
| `00264a2c` | correct | 1 | 1 | 0 |
| `0028245c` | correct | 1 | 1 | 0 |

**Upgraded: 10. Held: 0.**

Decision: [`dec469`](../../sheets/decisions.tsv#dec469).

## Running total

| | |
|---|---|
| Remaining before this round | 4,852 |
| Round 36 (dec469) | +10 |
| **Remaining** | **4,842** |
