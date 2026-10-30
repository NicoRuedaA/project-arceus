# D4 depth rounds — new session continuation (round 14)

Scope and method: same as rounds 1-13 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec446`](../../sheets/decisions.tsv#dec446)),
back to normal size-descending fresh-candidate selection after round 13
cleared the deferred-for-size backlog. 4 of 24 candidates discarded by the
up-front boundary check: `0186e924` -> `FUN_0186e738`, `00f88ec4` -> `FUN_00f88c58`,
`00d76af4` -> `FUN_00d76a3c`, `00a836e4` -> `FUN_00a834f4`; topped up to 24
clean candidates. 3 more clean candidates set aside for sheer size:
`01891db0` (288 lines), `0144de40` (249 lines), `00e675ec` (244 lines) —
restarting the deferred-for-size backlog.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `02682a30` | correct | 4 | 4 | 0 |
| `01d1f128` | correct | 4 | 4 | 0 |
| `01d1edfc` | corrected | 4 | 4 | 0 |
| `01bc48b4` | corrected | 4 | 1 | 3 |
| `01b91c38` | correct | 4 | 3 | 1 |
| `01841c24` | correct | 4 | 4 | 0 |
| `01775a64` | correct | 4 | 4 | 0 |
| `01685554` | correct | 4 | 0 | 4 |
| `013bff5c` | correct | 4 | 0 | 4* |
| `012bbe2c` | corrected | 4 | 0 | 4 |
| `011488dc` | corrected | 4 | 4 | 0 |
| `00ea4d58` | correct | 4 | 0 | 4 |
| `00e680f8` | corrected | 4 | 0 | 4 |
| `00ca64a4` | correct | 4 | 4 | 0 |
| `00ca4374` | correct | 4 | 4 | 0 |
| `00c98be0` | correct | 4 | 4 | 0 |
| `00c7c098` | correct | 4 | 2 | 2 |
| `00c693dc` | corrected | 4 | 3 | 1 |
| `0086bd78` | correct | 4 | 3 | 1 |
| `009e23ec` | correct | 4 | 2 | 2 |
| `00a852b0` | corrected | 4 | 4 | 0 |

\* `013bff5c`'s cluster includes one member (`01404e0c`) that could not be
verified — a GhidraMCP disassembly request hung past a reasonable timeout
and was left held rather than retried indefinitely, since an inconclusive
check is not grounds to advance a record.

**Upgraded: 54** across 21 target clusters. **Held: 29** (plus the 1
unverifiable member above), with `00e680f8`/`00ea4d58`/`012bbe2c`/
`013bff5c`/`01685554` holding their entire cluster.

Decision: [`dec447`](../../sheets/decisions.tsv#dec447).

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
| **Remaining** | **5,415** |

fix2 derived-report regeneration remains unregenerated (honestly marked
`state: stale`/`reason_code: generation_failed`); progress-bar numbers are
short by 4,776 until the gameDB corpus is rebuilt or located.
