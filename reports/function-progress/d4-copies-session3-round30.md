# D4 depth rounds — new session continuation (round 30)

Same setup and truncation filter as rounds 27-29 (smallest remaining fully identical R##-generation clusters, 176-180 bytes; none skipped). 20 single-copy targets; boundary check and both identity gates passed for all 40 addresses. Sixteen are instances of template G (gate via a pointer, helper H, branch tail; per-target table extracted from the listings by script); 01f7fb54/0209cfe4 repeat round 26's string-equality shape with different getters (the cbnz after 0x032a28b0 sits in a hole inside the compared span); 0053561c emits the protobuf map CHECK report on mismatch. One independent auditor: 19 correct, 1 corrected (022a6d04: stage wording, its third stage has a conditional), 0 wrong.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `021f5e1c` | correct | 1 | 1 | 0 |
| `02236b1c` | correct | 1 | 1 | 0 |
| `02237e5c` | correct | 1 | 1 | 0 |
| `02238164` | correct | 1 | 1 | 0 |
| `02238a8c` | correct | 1 | 1 | 0 |
| `022394b0` | correct | 1 | 1 | 0 |
| `022a6d04` | corrected | 1 | 1 | 0 |
| `0231aab8` | correct | 1 | 1 | 0 |
| `0231b750` | correct | 1 | 1 | 0 |
| `0231bd5c` | correct | 1 | 1 | 0 |
| `0231cc34` | correct | 1 | 1 | 0 |
| `0231d804` | correct | 1 | 1 | 0 |
| `0231dde0` | correct | 1 | 1 | 0 |
| `0231e7fc` | correct | 1 | 1 | 0 |
| `02345c14` | correct | 1 | 1 | 0 |
| `0236fb40` | correct | 1 | 1 | 0 |
| `023fda00` | correct | 1 | 1 | 0 |
| `0053561c` | correct | 1 | 1 | 0 |
| `01f7fb54` | correct | 1 | 1 | 0 |
| `0209cfe4` | correct | 1 | 1 | 0 |

**Upgraded: 20. Held: 0.**

Decision: [`dec463`](../../sheets/decisions.tsv#dec463).

## Running total

| | |
|---|---|
| Remaining before this round | 4,952 |
| Round 30 (dec463) | +20 |
| **Remaining** | **4,932** |
