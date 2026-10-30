# D4 depth rounds — new session continuation (round 38)

Same setup and truncation filter as rounds 27-37 (smallest remaining fully identical R##-generation clusters, 364-368 bytes; none skipped). 10 single-copy targets; boundary check and both identity gates passed for all 20 addresses. Seven are instances of round 37's template R (per-target parameters extracted by script). One independent auditor: 8 correct, 2 corrected (00612270: pb re-read after a's call; 0143bbc0: undefined bucket name), 0 wrong.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `00612270` | corrected | 1 | 1 | 0 |
| `012b1998` | correct | 1 | 1 | 0 |
| `0143bbc0` | corrected | 1 | 1 | 0 |
| `007f1d98` | correct | 1 | 1 | 0 |
| `00a46c88` | correct | 1 | 1 | 0 |
| `00cca930` | correct | 1 | 1 | 0 |
| `00e97074` | correct | 1 | 1 | 0 |
| `00ee2030` | correct | 1 | 1 | 0 |
| `0161c318` | correct | 1 | 1 | 0 |
| `016330ac` | correct | 1 | 1 | 0 |

**Upgraded: 10. Held: 0.**

Decision: [`dec471`](../../sheets/decisions.tsv#dec471).

## Running total

| | |
|---|---|
| Remaining before this round | 4,832 |
| Round 38 (dec471) | +10 |
| **Remaining** | **4,822** |
