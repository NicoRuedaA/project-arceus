# D4 depth rounds — new session continuation (round 8)

Scope and method: same as rounds 1-7 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec440`](../../sheets/decisions.tsv#dec440)).
5 of 24 candidates discarded by the up-front boundary check: `01b72da8` -> `FUN_01b70dfc`,
`01515550` -> `FUN_0151358c`, `00432768` -> `FUN_004323e8`, `026b34b0` -> `FUN_026b339c`,
`00a49200` -> `FUN_00a48be8`. 3 more clean candidates set aside for sheer size:
`01b98110` (278 lines), `00d797e8` (293 lines), `00d378f0` (197 lines) —
a dedicated future round.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `01a5fd00` | corrected | 9 | 5 | 4 |
| `01843d90` | correct | 9 | 7 | 2 |
| `013bd284` | corrected | 9 | 6 | 3 |
| `012ba180` | corrected | 9 | 9 | 0 |
| `0086be50` | correct | 9 | 9 | 0 |
| `001ddac4` | corrected | 9 | 9 | 0 |
| `02f5b29c` | corrected | 8 | 8 | 0 |
| `02e7ef40` | correct | 8 | 8 | 0 |
| `02e798f0` | correct | 8 | 8 | 0 |
| `0297eb20` | correct | 8 | 6 | 2 |
| `00f166b8` | corrected | 8 | 8 | 0 |
| `00f0ca04` | correct | 8 | 0 | 8 |
| `00c7a610` | correct | 8 | 7 | 1 |
| `00c0ca24` | correct | 8 | 8 | 0 |
| `00a6e968` | correct | 8 | 7 | 1 |
| `009c3ba4` | correct | 8 | 8 | 0 |

**Upgraded: 113** across 16 target clusters. **Held: 21**, with `00f0ca04`
(the "backward scan" family) failing 8/8 — unusual for this otherwise-reliable
family, worth a closer look in a future round rather than assumed identical
to its siblings.

Decision: [`dec441`](../../sheets/decisions.tsv#dec441).

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
| **Remaining** | **5,885** |

fix2 derived-report regeneration remains unregenerated; progress-bar/treemap
numbers are short by 4,306 until the gameDB corpus is rebuilt or located.
