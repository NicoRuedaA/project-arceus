# D4 depth rounds — new session continuation (round 3)

Scope and method: same as [round 1](d4-copies-session3.md)/[round 2](d4-copies-session3-round2.md)
(decisions [`dec434`](../../sheets/decisions.tsv#dec434), [`dec435`](../../sheets/decisions.tsv#dec435)).
This round added an up-front cleanliness check (decompile a target, compare the
returned `FUN_<addr>` name against the address requested) before writing any
description, to avoid spending effort on boundary-merged targets. 6 of 20
candidates were discarded this way before any reading: `009e28c4` -> `FUN_009dc444`,
`00a359b8` -> `FUN_00a353a0`, `027c57c0` -> `FUN_027c56bc`, `00ed59f4` -> `FUN_00ed593c`,
`0156af20` -> `FUN_0156a974`, `00857978` -> `FUN_008575b8`.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `007fa280` | correct | 35 | 35 | 0 |
| `0294d750` | correct | 33 | 23 | 10 |
| `001c0b80` | correct | 33 | 33 | 0 |
| `00193684` | corrected | 33 | 29 | 4 |
| `008a94a8` | corrected | 32 | 32 | 0 |
| `02bb71f0` | corrected (family) | 31 | 31 | 0 |
| `02bb70ec` | corrected (family) | 31 | 31 | 0 |
| `01824324` | correct | 31 | 31 | 0 |
| `02bb6fe8` | corrected (family) | 29 | 28 | 1 |
| `026b444c` | corrected, but 0/29 members verified | 29 | 0 | 29 |
| `00400624` | corrected | 28 | 19 | 9 |
| `026acc44` | corrected (family) | 26 | 26 | 0 |
| `002ea500` | correct | 26 | 16 | 10 |
| `0262cba8` | corrected (family) | 25 | 12 | 13 |

**Upgraded: 346** across 13 target clusters. **Held: 76 + 29 (`026b444c`) = 105.**

This round was the first with no audit `UNSUPPORTED` verdict, but the deepest
target (`026b444c`, a 3-level nested-recursion variant/reference-assignment
routine, 609 decompiled lines) reproduced the now-familiar "shape-similar
instantiation, not byte-identical copy" finding from `dec434`/`dec435`
(`015356b4`, `015220b0`) on every single one of its 29 claimed members — its
own description is accepted (with substantial corrections) but its propagated
copy-group looks mistaken in the same way. Left untouched.

Decision: [`dec436`](../../sheets/decisions.tsv#dec436).

## Running total, this session

| | |
|---|---|
| Pool | 22,392 |
| Before this session | 12,201 |
| Round 1 (dec434) | +2,556 |
| Round 2 (dec435) | +664 |
| Round 3 (dec436) | +346 |
| **Remaining** | **6,625** |

fix2 derived-report regeneration remains unregenerated (see round 1's note);
the progress-bar/treemap numbers are short by 3,566 (2,556+664+346) until the
gameDB corpus is rebuilt or located.
