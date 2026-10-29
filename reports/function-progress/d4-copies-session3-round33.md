# D4 depth rounds — new session continuation (round 33)

Same setup and truncation filter as rounds 27-32 (smallest remaining fully identical R##-generation clusters, 224-228 bytes; 5 candidates skipped as truncated). 20 single-copy targets; boundary check and both identity gates passed for all 40 addresses. Thirteen are instances of the round-32 two-stage template (per-target K, gate, helper and tail extracted by script); the rest are variants of the same gated preamble with member-function-pointer or copy tails. One independent auditor: 20 correct, 0 corrected, 0 wrong.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `00db16c4` | correct | 1 | 1 | 0 |
| `00e819b4` | correct | 1 | 1 | 0 |
| `00f16554` | correct | 1 | 1 | 0 |
| `0162483c` | correct | 1 | 1 | 0 |
| `01eea004` | correct | 1 | 1 | 0 |
| `01f908cc` | correct | 1 | 1 | 0 |
| `01fd7088` | correct | 1 | 1 | 0 |
| `0206f39c` | correct | 1 | 1 | 0 |
| `02070048` | correct | 1 | 1 | 0 |
| `02089044` | correct | 1 | 1 | 0 |
| `020e6dfc` | correct | 1 | 1 | 0 |
| `020e81e4` | correct | 1 | 1 | 0 |
| `02182f84` | correct | 1 | 1 | 0 |
| `021d4fb8` | correct | 1 | 1 | 0 |
| `021f652c` | correct | 1 | 1 | 0 |
| `02246544` | correct | 1 | 1 | 0 |
| `02420984` | correct | 1 | 1 | 0 |
| `0244fdd0` | correct | 1 | 1 | 0 |
| `00d56ff8` | correct | 1 | 1 | 0 |
| `014f06b8` | correct | 1 | 1 | 0 |

**Upgraded: 20. Held: 0.**

Decision: [`dec466`](../../sheets/decisions.tsv#dec466).

## Running total

| | |
|---|---|
| Remaining before this round | 4,892 |
| Round 33 (dec466) | +20 |
| **Remaining** | **4,872** |
