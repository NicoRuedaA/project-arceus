# D4 depth rounds — new session continuation (round 9)

Scope and method: same as rounds 1-8 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec441`](../../sheets/decisions.tsv#dec441)).
3 of 24 candidates discarded by the up-front boundary check: `02bef794` -> `FUN_02bef61c`,
`015163b0` -> `FUN_015162d0`, `012580ac` -> `FUN_01257cf4`.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `0307fe2c` | correct | 7 | 7 | 0 |
| `03034edc` | corrected | 7 | 7 | 0 |
| `02ee9b40` | corrected | 7 | 7 | 0 |
| `02e69b8c` | correct | 7 | 7 | 0 |
| `02d87c10` | correct | 7 | 7 | 0 |
| `02bed4a4` | corrected | 7 | 7 | 0 |
| `0283d178` | correct | 7 | 7 | 0 |
| `0268d150` | corrected | 7 | 7 | 0 |
| `0266fce4` | corrected | 7 | 0 | 7 |
| `01ee9ccc` | correct | 7 | 1 | 6 |
| `01dc276c` | corrected | 7 | 4 | 3 |
| `01d5f848` | corrected | 7 | 7 | 0 |
| `01ba031c` | correct | 7 | 2 | 5 |
| `01b6f050` | corrected | 7 | 7 | 0 |
| `0186fbf4` | corrected | 7 | 7 | 0 |
| `0186f72c` | corrected | 7 | 7 | 0 |
| `0186f0d4` | corrected | 7 | 7 | 0 |
| `015db160` | corrected | 7 | 7 | 0 |
| `015017dc` | correct | 7 | 6 | 1 |
| `0142fd90` | corrected | 7 | 7 | 0 |
| `01140fa4` | correct | 7 | 6 | 1 |

**Upgraded: 124** across 21 target clusters. **Held: 23**, concentrated in
`0266fce4` (0/7, the densest-flagged target this round, confirmed genuinely
non-identical across its whole cluster) and `01ee9ccc` (1/7 — a member of the
"backward 0x6b scan" family whose round-8 sibling `00f0ca04` held 0/8; both
dense/family-risk flags raised before auditing paid off).

Decision: [`dec442`](../../sheets/decisions.tsv#dec442).

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
| **Remaining** | **5,761** |

fix2 derived-report regeneration remains unregenerated; progress-bar/treemap
numbers are short by 4,430 until the gameDB corpus is rebuilt or located.
