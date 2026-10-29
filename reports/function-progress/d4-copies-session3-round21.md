# D4 depth rounds — new session continuation (round 21)

First round drawing from a different, much older propagation generation
("D4 batch 10") rather than the "D4 R##" generations used in rounds 1-20,
found by sorting ALL remaining pending clusters by size instead of only
R##-labeled ones.

**Discovery**: of the 50 largest still-pending `D4 batch 10` target
clusters, 24 failed the up-front boundary check — including the three
largest (`012b8954`, 640 members; `008a9628`, 229; `010d216c`, 117, all
merged into other functions) — a 48% discard rate, far above this
session's typical 0-4-per-24. Of the 26 survivors, 1 was set aside for size
(`01772e48`, 256 lines), leaving 23 target clusters totaling 384 pending
members, an order of magnitude more than a typical round.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `015356b4` | corrected | 110 | 0 | 110 |
| `015220b0` | corrected | 41 | 0 | 41 |
| `01a865b4` | **wrong — rejected** | 56 | 0 | 56 |
| `007c4b3c` | corrected | 40 | 0 | 40 |
| `009c3fa4` | corrected | 5 | 0 | 5 |
| `026b444c` | **wrong — rejected** | 29 | 0 | 29 |
| `026b339c` | **wrong — rejected** | 16 | 0 | 16 |
| `00c2739c` | corrected | 24 | 0 | 24 |
| `0262cba8` | corrected | 13 | 0 | 13 |
| `0294d750` | corrected | 10 | 0 | 10 |
| `026938ac` | corrected | 1 | 0 | 1 |
| `00400624` | corrected | 9 | 0 | 9 |
| `02779bd0` | corrected | 8 | 0 | 8 |
| `00fdc6c0` | corrected | 8 | 0 | 8 |
| `00193684` | correct | 4 | 0 | 4 |
| `00f51ea0` | correct | 2 | 2 | 0 |
| `007eafe0` | correct | 2 | 2 | 0 |
| `00802d68` | correct | 1 | 1 | 0 |
| `012b9c10` | correct | 2 | 2 | 0 |
| `02bb6fe8` | correct | 1 | 1 | 0 |
| `008057c4` | corrected | 1 | 0 | 1 |
| `007e3dc0` | correct | 1 | 0 | 1 |
| `00c8f8d8` | correct | 1 | 0 | 1 |

**Upgraded: 8** across 5 target clusters. **Held: 377** — concentrated
almost entirely in `015356b4` (0/110), `01a865b4` (0/56), `015220b0`
(0/41), `007c4b3c` (0/40), `026b444c` (0/29), and `00c2739c` (0/24). The
self-derived-extent disassembly check found that only ~2% of this batch's
384 pending members pass byte-identity, concentrated entirely in 5 small
bytecode-helper-style targets with no embedded per-type constants
(`00f51ea0`, `008057c4`, `007eafe0`, `012b9c10`, `00802d68`); every larger
target in the batch — pool allocators, the lazy shared-registry/proxy-cache
family, the hash-table insert, and the composite destructors — held in its
entirety.

Three targets (`01a865b4`, `026b444c`, `026b339c`) were rejected outright
by the independent audit, each for the same class of error: describing a
genuine three-outcome branch as having only two outcomes. All three held
their entire cluster regardless (0 members had passed byte-identity
anyway), so the rejections cost no upgrades.

`D4 batch 10`'s very low yield-per-target-described ratio compared to the
`R##` generations (which have run roughly 75-85% pass rates every round
this session) suggests continuing to mine batch 10 by cluster size alone is
a poor strategy going forward. Round 22 reverts to the `R##`-generation
selection method used in rounds 1-20.

Decision: [`dec454`](../../sheets/decisions.tsv#dec454).

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
| Round 6 (dec439) | +156 |
| Round 7 (dec440) | +133 |
| Round 8 (dec441) | +113 |
| Round 9 (dec442) | +124 |
| Round 10 (dec443) | +99 |
| Round 11 (dec444) | +92 |
| Round 12 (dec445) | +66 |
| Round 13 (dec446) | +35 |
| Round 14 (dec447) | +54 |
| Round 15 (dec448) | +48 |
| Round 16 (dec449) | +46 |
| Round 17 (dec450) | +50 |
| Round 18 (dec451) | +18 |
| Round 19 (dec452) | +57 |
| Round 20 (dec453) | +48 |
| Round 21 (dec454) | +8 |
| **Remaining** | **5,140** |

fix2 derived-report regeneration remains unregenerated (honestly marked
`state: stale`/`reason_code: generation_failed`); progress-bar numbers are
short by 5,051 until the gameDB corpus is rebuilt or located.
