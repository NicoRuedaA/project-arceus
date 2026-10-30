# D4 depth rounds — new session continuation (round 18, second dedicated deferred-backlog round)

Scope and method: same as rounds 1-17 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec450`](../../sheets/decisions.tsv#dec450)),
but this round deliberately clears the full "deferred for size" backlog
accumulated across rounds 14-17 instead of selecting fresh size-descending
candidates — the second such dedicated round, following round 13's
precedent. 6 targets, all previously set aside as too large/dense for
their original rounds: `01891db0` (288 lines), `0144de40` (249), `00e675ec`
(244), `02a74050` (309), `027bfe5c` (959 — a true giant), `0251cc34` (697).
All 6 were already boundary-checked clean in their original rounds (no
merges). Three were cross-referenced against prior-round sibling
families (`00e675ec`↔round 12's `00f4e708`; `02a74050`↔round 11's
`027a72c4`; `0251cc34`↔round 11's `0251d1f8`); the audit independently
fetched and directly compared each referenced function rather than
trusting the claimed match, and re-sampled the two giants at verified
real line numbers.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `01891db0` | corrected | 4 | 4 | 0 |
| `0144de40` | corrected | 4 | 4 | 0 |
| `00e675ec` | correct | 4 | 4 | 0 |
| `02a74050` | corrected | 3 | 3 | 0 |
| `027bfe5c` | corrected | 3 | 0 | 3 |
| `0251cc34` | corrected | 3 | 3 | 0 |

**Upgraded: 18** across 6 target clusters. **Held: 3**, all in `027bfe5c`
(0/3) — the one target with no established family match, consistent with
the novel-shape flag raised before auditing.

Decision: [`dec451`](../../sheets/decisions.tsv#dec451).

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
| **Remaining** | **5,253** |

fix2 derived-report regeneration remains unregenerated (honestly marked
`state: stale`/`reason_code: generation_failed`); progress-bar numbers are
short by 4,938 until the gameDB corpus is rebuilt or located. The
deferred-for-size backlog is now fully cleared again — round 19 returns
to normal size-descending fresh-candidate selection.
