# D4 depth rounds — new session continuation (round 29)

Same setup and truncation filter as rounds 27-28 (smallest remaining fully identical R##-generation clusters, 176 bytes; 2 candidates skipped as truncated). 15 single-copy targets, all instances of the round-27 template G (gate via a pointer, helper H, branch tail), described once with a per-target table extracted from the listings by script; boundary check and both identity gates passed for all 30 addresses. One independent auditor checked the template and every row against each body: 15 correct, 0 corrected, 0 wrong.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `02088ab0` | correct | 1 | 1 | 0 |
| `020c5f5c` | correct | 1 | 1 | 0 |
| `020c9310` | correct | 1 | 1 | 0 |
| `020c98c8` | correct | 1 | 1 | 0 |
| `020cd098` | correct | 1 | 1 | 0 |
| `020ce424` | correct | 1 | 1 | 0 |
| `020e573c` | correct | 1 | 1 | 0 |
| `020e7490` | correct | 1 | 1 | 0 |
| `02100188` | correct | 1 | 1 | 0 |
| `021003dc` | correct | 1 | 1 | 0 |
| `0210069c` | correct | 1 | 1 | 0 |
| `021013e8` | correct | 1 | 1 | 0 |
| `02104e58` | correct | 1 | 1 | 0 |
| `02105150` | correct | 1 | 1 | 0 |
| `0210c43c` | correct | 1 | 1 | 0 |

**Upgraded: 15. Held: 0.**

Decision: [`dec462`](../../sheets/decisions.tsv#dec462).

## Running total

| | |
|---|---|
| Remaining before this round | 4,967 |
| Round 29 (dec462) | +15 |
| **Remaining** | **4,952** |
