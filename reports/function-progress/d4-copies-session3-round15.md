# D4 depth rounds — new session continuation (round 15)

Scope and method: same as rounds 1-14 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec447`](../../sheets/decisions.tsv#dec447)).
2 of 24 candidates discarded by the up-front boundary check: `007c4500` -> `FUN_007c40f4`,
`027cef9c` -> `FUN_027cec70`; topped up to 24 clean candidates. 5 more clean
candidates set aside for sheer size: `01891db0` (288 lines), `0144de40`
(249), `00e675ec` (244), `02a74050` (309), `027bfe5c` (959 — a true
giant), restarting the deferred-for-size backlog after round 13 cleared it.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `00c0d7dc` | corrected | 4 | 0 | 4 |
| `008b3124` | corrected | 4 | 4 | 0 |
| `006e6544` | corrected | 4 | 4 | 0 |
| `00549ed0` | correct | 4 | 4 | 0 |
| `005481ec` | corrected | 4 | 4 | 0 |
| `0314f8b0` | corrected | 3 | 3 | 0 |
| `03033944` | correct | 3 | 3 | 0 |
| `02fc0afc` | correct | 3 | 3 | 0 |
| `02f05754` | correct | 3 | 3 | 0 |
| `02f027d8` | corrected | 3 | 3 | 0 |
| `02ed8a38` | corrected | 3 | 3 | 0 |
| `02e2a100` | correct | 3 | 3 | 0 |
| `02c43a94` | correct | 3 | 0 | 3 |
| `02b3b4e4` | correct | 3 | 2 | 1 |
| `02afa344` | correct | 3 | 2 | 1 |
| `02a1b540` | correct | 3 | 1 | 2 |
| `029d2710` | corrected | 3 | 3 | 0 |
| `0279c988` | correct | 3 | 3 | 0 |

**Upgraded: 48** across 18 target clusters. **Held: 11**, concentrated in
`00c0d7dc` (0/4) and `02c43a94` (0/3).

Decision: [`dec448`](../../sheets/decisions.tsv#dec448).

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
| **Remaining** | **5,367** |

fix2 derived-report regeneration remains unregenerated (honestly marked
`state: stale`/`reason_code: generation_failed`); progress-bar numbers are
short by 4,824 until the gameDB corpus is rebuilt or located.
