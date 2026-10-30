# D4 depth rounds — new session continuation (round 12)

Scope and method: same as rounds 1-11 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec444`](../../sheets/decisions.tsv#dec444)).
9 of 24 candidates discarded by the up-front boundary check: `0156f8e0` -> `FUN_0156f17c`,
`012b4eec` -> `FUN_012b4c88`, `011efee0` -> `FUN_011ef984`, `00a87aa8` -> `FUN_00a879c0`,
`004f83f4` -> `FUN_004f8318`, `0019e86c` -> `FUN_0019e850`, `02f40ae0` -> `FUN_02f40ab8`,
`02aa466c` -> `FUN_02aa457c`, `0296babc` -> `FUN_0296b8ac`; topped up to 22
clean candidates. 2 more clean candidates set aside for sheer size:
`0135923c` (377 lines), `00f4e708` (244 lines) — joining the growing
deferred-for-size set from rounds 6, 8, and 11.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `0170a324` | corrected | 5 | 5 | 0 |
| `010d0de4` | correct | 5 | 4 | 1 |
| `00fe91d8` | corrected | 5 | 2 | 3 |
| `00f666d4` | corrected | 5 | 0 | 5 |
| `00f4f214` | correct | 5 | 0 | 5 |
| `00d0b5e4` | corrected | 5 | 5 | 0 |
| `00cf4e28` | correct | 5 | 5 | 0 |
| `00ccf0a0` | correct | 5 | 5 | 0 |
| `00add27c` | correct | 5 | 5 | 0 |
| `00781be4` | correct | 5 | 5 | 0 |
| `00598fa0` | correct | 5 | 5 | 0 |
| `00446ca4` | correct | 5 | 0 | 5 |
| `00193940` | corrected | 5 | 0 | 5 |
| `03081e90` | corrected | 4 | 4 | 0 |
| `02f41224` | correct | 4 | 4 | 0 |
| `02ed6760` | correct | 4 | 4 | 0 |
| `02e6a1c4` | correct | 4 | 4 | 0 |
| `02afad10` | corrected | 4 | 2 | 2 |
| `02970d04` | correct | 4 | 3 | 1 |
| `027968a8` | corrected | 4 | 4 | 0 |

**Upgraded: 66** across 20 target clusters. **Held: 27**, with four targets
holding their entire cluster (`00193940`, `00446ca4`, `00f4f214`, `00f666d4`)
— consistent with the established shape-correct-but-differing-embedded-
constant pattern (dec438/round5 and others), not a methodology concern.

Decision: [`dec445`](../../sheets/decisions.tsv#dec445).

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
| **Remaining** | **5,504** |

fix2 derived-report regeneration remains unregenerated (honestly marked
`state: stale`/`reason_code: generation_failed`); progress-bar numbers are
short by 4,687 until the gameDB corpus is rebuilt or located.
