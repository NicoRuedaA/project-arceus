# D4 depth rounds — new session continuation (round 17)

Scope and method: same as rounds 1-16 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec449`](../../sheets/decisions.tsv#dec449)).
1 of 24 candidates discarded by the up-front boundary check: `01d5fb50` -> `FUN_01d5f9a0`;
topped up to 24 clean candidates. 6 clean candidates set aside for sheer
size, unchanged from round 16: `01891db0` (288 lines), `0144de40` (249),
`00e675ec` (244), `02a74050` (309), `027bfe5c` (959), `0251cc34` (697) —
the deferred-for-size backlog has not shrunk across two rounds and now
warrants a dedicated clearing round next, following round 13's precedent.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `02138f98` | correct | 3 | 3 | 0 |
| `01f8a6c8` | correct | 3 | 3 | 0 |
| `01ff4ed0` | correct | 3 | 2 | 1 |
| `01de59d0` | correct | 3 | 3 | 0 |
| `01dda824` | corrected | 3 | 3 | 0 |
| `01dda57c` | corrected | 3 | 3 | 0 |
| `01d702ac` | corrected | 3 | 0 | 3 |
| `01d16650` | correct | 3 | 3 | 0 |
| `01cea604` | correct | 3 | 3 | 0 |
| `01cea524` | correct | 3 | 3 | 0 |
| `01c2f900` | correct | 3 | 3 | 0 |
| `01c2f820` | correct | 3 | 3 | 0 |
| `01c2c1ac` | correct | 3 | 3 | 0 |
| `01c2c0cc` | correct | 3 | 3 | 0 |
| `01c172f0` | correct | 3 | 3 | 0 |
| `01c17210` | correct | 3 | 3 | 0 |
| `01b98030` | correct | 3 | 3 | 0 |
| `01b97f50` | correct | 3 | 3 | 0 |

**Upgraded: 50** across 18 target clusters. **Held: 4**, concentrated in
`01d702ac` (0/3).

Decision: [`dec450`](../../sheets/decisions.tsv#dec450).

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
| **Remaining** | **5,271** |

fix2 derived-report regeneration remains unregenerated (honestly marked
`state: stale`/`reason_code: generation_failed`); progress-bar numbers are
short by 4,920 until the gameDB corpus is rebuilt or located.
