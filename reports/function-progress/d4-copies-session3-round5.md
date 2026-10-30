# D4 depth rounds — new session continuation (round 5)

Scope and method: same as rounds 1-4 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec437`](../../sheets/decisions.tsv#dec437)).
7 of 20 candidates discarded by the up-front boundary check: `00c8f6d0` -> `FUN_00c8ee14`,
`014f37bc` -> `FUN_014f31a4`, `00cfc600` -> `FUN_00cfbfe0`, `00a63974` -> `FUN_00a5f5d4`,
`026b4524` -> `FUN_026b444c`, `01604934` -> `FUN_0160487c`, `02c56454` -> `FUN_02c5631c`.

## Verification bug found and fixed this round

Normalized-disassembly verification uses each function's `original_bytes`
size (from the fix2 inventory) to decide which hex operands are "internal"
(rewritten relative to the function's own start) vs "external" (left
absolute). For `00a4ea28`, the recorded size (172 bytes) significantly
understates the function's real extent in this reseeded project — its own
internal branches reach roughly 0x320 bytes past the start — so every branch
target past 172 bytes was left as an absolute address and legitimately
differed between every claimed member, producing 16/16 false mismatches
despite the function being correctly described (audit verdict: CORRECT) and
genuinely byte-identical. Fix: derive each function's extent directly from
its own live disassembly (max instruction address + 4) instead of trusting
the external declared size. Re-running verification with this method
recovered all 16 `00a4ea28` members. **This is now the standard method for
all future rounds** — the externally recorded size can understate a
function's true body in a freshly reseeded project; self-derived extent
cannot.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `00c8f8d8` | correct | 17 | 16 | 1 |
| `026b339c` | corrected | 16 | 0 | 16 |
| `00a4ea28` | correct | 16 | 16 (16 recovered after the fix above) | 0 |
| `009c3fa4` | corrected | 16 | 11 | 5 |
| `01e247f8` | correct | 15 | 9 | 6 |
| `00cc5a00` | **UNSUPPORTED** | — | — | — |
| `00449014` | correct | 15 | 0 | 15 |
| `024e2580` | corrected | 14 | 13 | 1 |
| `01e274c8` | **UNSUPPORTED** | — | — | — |
| `00f0ca60` | corrected | 14 | 14 | 0 |
| `027a79d8` | corrected (family) | 13 | 13 | 0 |
| `027a7520` | corrected (family) | 13 | 13 | 0 |
| `0278f298` | correct | 13 | 11 | 2 |

**Upgraded: 116** across 9 target clusters. **Held: 46** (plus the 2
`UNSUPPORTED` targets' members, not individually counted since they were
dropped before the verification step). `026b339c` (a 3-level nested-recursion
variant, structurally close to round 3's excluded `026b444c`) and `00449014`
(the densest, most register-artifact-heavy target of the batch) both held
100% of their members despite a correct/accepted description — consistent
with the established "complex target, genuinely divergent members" pattern
from round 4, not the boundary-size bug fixed above (both were re-checked
with the self-derived-extent method and the mismatches persisted).

Decision: [`dec438`](../../sheets/decisions.tsv#dec438).

## Running total, this session

| | |
|---|---|
| Pool | 22,392 |
| Before this session | 12,201 |
| Round 1 (dec434) | +2,556 |
| Round 2 (dec435) | +664 |
| Round 3 (dec436) | +346 |
| Round 4 (dec437) | +222 |
| Round 5 (dec438) | +116 |
| **Remaining** | **6,287** |

fix2 derived-report regeneration remains unregenerated; progress-bar/treemap
numbers are short by 3,904 until the gameDB corpus is rebuilt or located.
