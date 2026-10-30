# D4 depth rounds — new session continuation (round 23, dedicated deferred-backlog round)

Dedicated round clearing the full deferred-for-size backlog accumulated
across rounds 19-22 (4 targets, following rounds 13/18's precedent for
such clearing rounds): `00524290` (229 lines), `0177fae0` (260),
`00f0b338` (240), `0274a1bc` (244). All 4 were already boundary-checked
clean in their original rounds.

Also fixed a candidate-selection gap this round: the first boundary-check
pass against size-sorted R##-generation candidates still hit several
previously-undocumented "zombies" — targets discarded in rounds 5-9's
selection phases but never written into any committed report's
discarded-by-boundary-check prose, so they were invisible to the
grep-based exclusion-set rebuild. Discarded addresses:
`00195ee4`, `00805860`, `00c876ec`, `004bec2c`, `00c8eb94`, `02659010`,
`02c8b3a0`, `00c7b05c`, `01f98e40`, `00a645bc`, `015e3d44`, `016906f4`,
`0051f3dc`, `0026c62c`, `0044ce24`, `0060df60`, `0054a1ec`, `005e8b44`,
`002e7200` (19 total, all now fully enumerated here so future rounds'
exclusion-set rebuilds catch them).

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `00524290` | corrected | 3 | 3 | 0 |
| `0177fae0` | corrected | 3 | 0 | 3 |
| `00f0b338` | correct | 3 | 3 | 0 |
| `0274a1bc` | corrected | 2 | 2 | 0 |

**Upgraded: 8** across 3 target clusters. **Held: 3**, all in `0177fae0`
(3/3, genuine embedded-constant non-identity, unrelated to the
control-flow correction described below).

All 4 targets were LOW CONFIDENCE given their density (229-260 lines
each). The independent audit corrected all 4 — 3 of the corrections were
secondary precision details (an inexact warning-count claim, a
4-byte-vs-long counter-width mischaracterization, a 7-vs-1 misattribution
of which fields use the registry-proxy release shape that was actually
6-vs-2), but `0177fae0`'s correction was a genuine control-flow finding:
the function does not return the finalized hash node on the normal insert
path (it returns the rehash result or a lazily-cached default-value
handle), and its `[0x18]`-null fallback branch abandons the half-built
node unspliced in favor of returning an entirely different proxy object.
This did not change which members pass byte-identity verification.

This round restarts the deferred-for-size backlog at empty; round 24
resumes normal-sized R##-generation batches.

Decision: [`dec456`](../../sheets/decisions.tsv#dec456).

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
| Round 19 (dec452) | +57 |
| Round 20 (dec453) | +48 |
| Round 21 (dec454) | +8 |
| Round 22 (dec455) | +38 |
| Round 23 (dec456) | +8 |
| **Remaining** | **5,094** |

fix2 derived-report regeneration remains unregenerated (honestly marked
`state: stale`/`reason_code: generation_failed`); progress-bar numbers are
short by 5,081 until the gameDB corpus is rebuilt or located.
