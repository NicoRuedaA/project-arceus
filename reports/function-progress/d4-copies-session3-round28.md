# D4 depth rounds — new session continuation (round 28)

Same setup and truncation filter as round 27 (smallest remaining fully identical R##-generation clusters, 168-176 bytes this round; no candidate skipped as truncated). 15 single-copy targets; boundary check and both identity gates passed for all 30 addresses. Eleven targets fit two shared templates (template G as in round 27, 7 targets; a template S that discards the gated call's result and tail-calls 0x0004ff10, 4 targets) and were described once each with per-target parameter tables; 00ec9ba0 matches round 26's 01101c90 with constant 0x20 instead of 0x80. One independent auditor: 15 correct, 0 corrected, 0 wrong.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `00e786b0` | correct | 1 | 1 | 0 |
| `00ead140` | correct | 1 | 1 | 0 |
| `00ec9ba0` | correct | 1 | 1 | 0 |
| `00f39b2c` | correct | 1 | 1 | 0 |
| `011468c4` | correct | 1 | 1 | 0 |
| `02094fdc` | correct | 1 | 1 | 0 |
| `0241fb34` | correct | 1 | 1 | 0 |
| `00e965c0` | correct | 1 | 1 | 0 |
| `0126afd4` | correct | 1 | 1 | 0 |
| `01f8b560` | correct | 1 | 1 | 0 |
| `022c8760` | correct | 1 | 1 | 0 |
| `02332ab0` | correct | 1 | 1 | 0 |
| `006ab340` | correct | 1 | 1 | 0 |
| `01f0418c` | correct | 1 | 1 | 0 |
| `01f9125c` | correct | 1 | 1 | 0 |

**Upgraded: 15. Held: 0.**

Decision: [`dec461`](../../sheets/decisions.tsv#dec461).

## Running total

| | |
|---|---|
| Remaining before this round | 4,982 |
| Round 28 (dec461) | +15 |
| **Remaining** | **4,967** |
