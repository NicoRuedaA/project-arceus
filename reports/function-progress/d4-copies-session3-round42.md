# D4 depth rounds — new session continuation (round 42)

Same setup and truncation filter as rounds 27-41 (remaining fully identical R##-generation clusters up to 800 bytes; 12 irregular candidates returned to the pool). 8 single-copy targets, all instances of round 41's template S (zero-fill INIT variants); per-target parameters extracted by script. Boundary check and both identity gates passed for all 16 addresses. One independent auditor checked the template against every body and every row: 8 correct, 0 corrected, 0 wrong.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `0080dac8` | correct | 1 | 1 | 0 |
| `00816fb4` | correct | 1 | 1 | 0 |
| `01f22c9c` | correct | 1 | 1 | 0 |
| `02286984` | correct | 1 | 1 | 0 |
| `01f44830` | correct | 1 | 1 | 0 |
| `023b1c88` | correct | 1 | 1 | 0 |
| `01f33de0` | correct | 1 | 1 | 0 |
| `0240ffac` | correct | 1 | 1 | 0 |

**Upgraded: 8. Held: 0.**

Decision: [`dec475`](../../sheets/decisions.tsv#dec475).

## Running total

| | |
|---|---|
| Remaining before this round | 4,776 |
| Round 42 (dec475) | +8 |
| **Remaining** | **4,768** |
