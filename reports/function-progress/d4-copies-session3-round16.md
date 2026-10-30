# D4 depth rounds — new session continuation (round 16)

Scope and method: same as rounds 1-15 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec448`](../../sheets/decisions.tsv#dec448)).
All 24 fresh candidates boundary-checked clean (no merges this round). 6
clean candidates set aside for sheer size, continuing the deferred-for-size
backlog: `01891db0` (288 lines), `0144de40` (249), `00e675ec` (244),
`02a74050` (309), `027bfe5c` (959), `0251cc34` (697 — another true giant,
same multi-stage-destructor shape as round 11's `0251d1f8`).

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `005482fc` | corrected | 4 | 4 | 0 |
| `0279c41c` | correct | 3 | 3 | 0 |
| `026ad080` | corrected | 3 | 0 | 3 |
| `0264294c` | corrected | 3 | 0 | 3 |
| `02622c38` | corrected | 3 | 3 | 0 |
| `02622900` | corrected | 3 | 3 | 0 |
| `0261e594` | correct | 3 | 3 | 0 |
| `0261e25c` | correct | 3 | 3 | 0 |
| `026198e4` | correct | 3 | 3 | 0 |
| `0261950c` | correct | 3 | 3 | 0 |
| `02614528` | correct | 3 | 3 | 0 |
| `026141f0` | correct | 3 | 3 | 0 |
| `026107f8` | correct | 3 | 3 | 0 |
| `0260fa30` | correct | 3 | 3 | 0 |
| `02609058` | correct | 3 | 3 | 0 |
| `0242a8cc` | correct | 3 | 1 | 2 |
| `021c2620` | corrected | 3 | 2 | 1 |
| `0214c5ec` | correct | 3 | 3 | 0 |

**Upgraded: 46** across 18 target clusters. **Held: 9**, with `026ad080`
and `0264294c` (the deep 7-field-clone family, flagged for density before
auditing) holding their entire clusters — consistent with the established
shape-correct-but-differing-embedded-constant pattern.

Decision: [`dec449`](../../sheets/decisions.tsv#dec449).

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
| **Remaining** | **5,321** |

fix2 derived-report regeneration remains unregenerated (honestly marked
`state: stale`/`reason_code: generation_failed`); progress-bar numbers are
short by 4,870 until the gameDB corpus is rebuilt or located.
