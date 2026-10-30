# D4 depth rounds — new session continuation (round 26)

A small round on the round-25 setup: the same capped Ghidra project, the same
three-way identity gate, and descriptions written from the instructions.

Selection, from the round-25 prefilter of the remaining R##-generation targets: the 5
remaining multi-member fully identical clusters, plus the 7 largest single-copy ones
between 120 and 260 bytes. That gives 12 targets with 17 members. The boundary check
passed for all 29 addresses. Nothing lies past any body end: the next instruction is
always a new function or padding.

Two more callees that Ghidra treats as non-returning turn out to return, and each
leaves a 4-byte hole inside a body:
- `0x032a28b0`, in `01fa2f24`: the hole holds a `cbnz` on its result;
- `0x032a45e0`, in `02210954`: the hole holds a `b` back into the body.

Both holes are inside the compared span. With round 25's three (`0x032a4530`,
`0x032a2e00`, `0x032a24f0`), five such callees are now known.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `00f6bab8` | correct | 2 | 2 | 0 |
| `023bb208` | correct | 2 | 2 | 0 |
| `00c1d1dc` | correct | 2 | 2 | 0 |
| `01fa2f24` | correct | 2 | 2 | 0 |
| `01101c90` | correct | 2 | 2 | 0 |
| `00ff4ab0` | correct | 1 | 1 | 0 |
| `022cbbe0` | correct | 1 | 1 | 0 |
| `01fd7358` | correct | 1 | 1 | 0 |
| `02210954` | correct | 1 | 1 | 0 |
| `018f1a44` | corrected | 1 | 1 | 0 |
| `02088154` | correct | 1 | 1 | 0 |
| `02898ff8` | correct | 1 | 1 | 0 |

**Upgraded: 17. Held: 0.** A single independent auditor checked all 12 targets: 11
correct, 1 corrected, 0 wrong. The correction: in `018f1a44` the bucket index uses a
32-bit mask, but the chain test masks `*(x+8)` with a 64-bit `n−1`.

With round 25, all multi-member fully identical clusters among the sized
R##-generation targets are now done. The remaining sized ones are single-copy
clusters (284 clusters, one member each).

Decision: [`dec459`](../../sheets/decisions.tsv#dec459).

## Running total

| | |
|---|---|
| Remaining after round 25 (dec458) | 5,014 |
| Round 26 (dec459) | +17 |
| **Remaining** | **4,997** |
