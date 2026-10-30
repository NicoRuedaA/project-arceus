# D4 depth rounds — new session continuation (round 32)

Same setup and truncation filter as rounds 27-31 (smallest remaining fully identical R##-generation clusters, 212-224 bytes; 1 candidate skipped as truncated). 20 single-copy targets; boundary check and both identity gates passed for all 40 addresses. Seventeen are instances of a two-stage template (fetch an object, offset it by K, then the gated preamble and a branch tail; per-target table of K, gate, helper and tail extracted by script); 002c1e4c emits the protobuf repeated_field CHECK report for a negative count. One independent auditor: 20 correct, 0 corrected, 0 wrong.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `00ce1278` | correct | 1 | 1 | 0 |
| `00f1f480` | correct | 1 | 1 | 0 |
| `014dae84` | correct | 1 | 1 | 0 |
| `014db688` | correct | 1 | 1 | 0 |
| `01503340` | correct | 1 | 1 | 0 |
| `015db030` | correct | 1 | 1 | 0 |
| `015db9f0` | correct | 1 | 1 | 0 |
| `015dbd34` | correct | 1 | 1 | 0 |
| `015dc050` | correct | 1 | 1 | 0 |
| `016702ec` | correct | 1 | 1 | 0 |
| `020b3914` | correct | 1 | 1 | 0 |
| `02128a60` | correct | 1 | 1 | 0 |
| `02128d20` | correct | 1 | 1 | 0 |
| `0241fcf4` | correct | 1 | 1 | 0 |
| `024205e4` | correct | 1 | 1 | 0 |
| `02085db4` | correct | 1 | 1 | 0 |
| `002c1e4c` | correct | 1 | 1 | 0 |
| `007eaeac` | correct | 1 | 1 | 0 |
| `007eee74` | correct | 1 | 1 | 0 |
| `007f012c` | correct | 1 | 1 | 0 |

**Upgraded: 20. Held: 0.**

Decision: [`dec465`](../../sheets/decisions.tsv#dec465).

## Running total

| | |
|---|---|
| Remaining before this round | 4,912 |
| Round 32 (dec465) | +20 |
| **Remaining** | **4,892** |
