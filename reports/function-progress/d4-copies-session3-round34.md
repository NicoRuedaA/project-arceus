# D4 depth rounds — new session continuation (round 34)

Same setup and truncation filter as rounds 27-33 (smallest remaining fully identical R##-generation clusters, 232-244 bytes; 5 candidates skipped as truncated). Shapes are now mostly unique, so the batch was cut to 10 single-copy targets (the other 10 return to the pool); boundary check and both identity gates passed for all 20 addresses. 02087cc0 has the hole after 0x032a45e0 inside the compared span. One independent auditor: 10 correct with two wording nits, both applied (0307c494 counted as corrected: the empty-slot path reads *(T+8) before and after its stores), 0 wrong.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `0122f4a4` | correct | 1 | 1 | 0 |
| `0307c494` | corrected | 1 | 1 | 0 |
| `00f3a11c` | correct | 1 | 1 | 0 |
| `01f60e40` | correct | 1 | 1 | 0 |
| `0211a1c8` | correct | 1 | 1 | 0 |
| `0214e680` | correct | 1 | 1 | 0 |
| `023314dc` | correct | 1 | 1 | 0 |
| `00468944` | correct | 1 | 1 | 0 |
| `02087cc0` | correct | 1 | 1 | 0 |
| `00d571c0` | correct | 1 | 1 | 0 |

**Upgraded: 10. Held: 0.**

Decision: [`dec467`](../../sheets/decisions.tsv#dec467).

## Running total

| | |
|---|---|
| Remaining before this round | 4,872 |
| Round 34 (dec467) | +10 |
| **Remaining** | **4,862** |
