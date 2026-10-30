# D4 depth rounds — new session continuation (round 11)

Scope and method: same as rounds 1-10 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec443`](../../sheets/decisions.tsv#dec443)).
2 of 24 candidates discarded by the up-front boundary check: `004bef00` -> `FUN_004be470`,
`01b8097c` -> `FUN_01b8016c`. 5 clean candidates set aside for sheer size:
`00a5156c` (414 lines), `027a72c4` (308 lines), `02a6e14c` (381 lines),
`0264b630` (1375 lines), `0251d1f8` (699 lines) — a dedicated future round.
The first two of those were themselves deferred from round 10 and resurfaced
as the largest fresh candidates this round.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `00a23a98` | corrected | 6 | 6 | 0 |
| `008c3384` | correct | 6 | 6 | 0 |
| `004f7ce0` | corrected | 6 | 1 | 5 |
| `0028f2a4` | corrected | 6 | 6 | 0 |
| `00225eb8` | corrected | 6 | 6 | 0 |
| `001c16ec` | corrected | 6 | 6 | 0 |
| `001a91a4` | corrected | 6 | 6 | 0 |
| `001a8d24` | corrected | 6 | 6 | 0 |
| `030ef8e8` | correct | 5 | 5 | 0 |
| `0303b688` | corrected | 5 | 5 | 0 |
| `02e69a8c` | correct | 5 | 5 | 0 |
| `02d03c20` | corrected | 5 | 5 | 0 |
| `02cbe6b0` | corrected | 5 | 4 | 1 |
| `02664bc8` | corrected | 5 | 0 | 5 |
| `02511860` | corrected | 5 | 5 | 0 |
| `02273da0` | correct | 5 | 5 | 0 |
| `01f58180` | correct | 5 | 5 | 0 |
| `02273918` | corrected | 5 | 5 | 0 |
| `0184a148` | corrected | 5 | 5 | 0 |

**Upgraded: 92** across 19 target clusters. **Held: 11**, concentrated in
`02664bc8` (0/5, one of two targets this round flagged MEDIUM CONFIDENCE for
density, confirmed genuinely non-identical across its whole cluster) and
`004f7ce0` (1/6).

Decision: [`dec444`](../../sheets/decisions.tsv#dec444).

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
| **Remaining** | **5,570** |

fix2 derived-report regeneration remains unregenerated (now honestly marked
`state: stale` / `reason_code: generation_failed` rather than left stale
with old data, per a tooling fix made earlier this session); progress-bar
numbers are short by 4,621 until the gameDB corpus is rebuilt or located.
