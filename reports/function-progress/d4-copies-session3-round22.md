# D4 depth rounds — new session continuation (round 22)

Reverted to the "D4 R##" generation selection method used in rounds 1-20,
after round 21's detour into the much-less-reliable "D4 batch 10"
generation.

**Methodology fix**: discovered that targets discarded by an earlier
round's up-front boundary check were never removed from the pending pool
(their member rows stay `Propagated` forever since no row ever references
them as upgraded), so they kept resurfacing at the top of any size-sorted
candidate list as "zombies". The first boundary-check pass on size-sorted
candidates still hit 18/24 mismatches — all zombies previously discarded
in rounds 5-9 — before building a fuller exclusion set from every prior
round's results table AND its "discarded by the boundary check" prose.
After that, a top-up batch of smaller (mostly 2-member) clusters survived
23/24.

Final batch: 22 described target clusters (grouping 6 same-shape siblings
under 2 shared headings) plus 4 deferred for size (`00524290`, 229 lines,
and `0177fae0`/`00f0b338`, 260/240, all three carried over from the
restarted backlog; `0274a1bc`, 244 lines, new this round).

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `01f19020` | corrected | 6 | 5 | 1 |
| `00c39e88` | correct | 5 | 0 | 5 |
| `001a3d90` | corrected | 3 | 0 | 3 |
| `001abd50` | correct | 3 | 3 | 0 |
| `02ef96a4` | correct | 2 | 2 | 0 |
| `00803224` | correct | 2 | 2 | 0 |
| `00c7bde4` | correct | 2 | 2 | 0 |
| `011865a4` | correct | 2 | 2 | 0 |
| `00d22e10` | corrected | 2 | 2 | 0 |
| `004eb030` | correct | 2 | 0 | 2 |
| `00a887b8` | correct | 2 | 0 | 2 |
| `0082d6fc` | correct | 2 | 2 | 0 |
| `00be090c` | correct | 2 | 2 | 0 |
| `0159ed50` | correct | 2 | 0 | 2 |
| `00d607d0` | correct | 2 | 2 | 0 |
| `006e670c` | corrected | 2 | 2 | 0 |
| `014b9850` | corrected | 2 | 2 | 0 |
| `02353bf0` | corrected | 2 | 2 | 0 |
| `02394990` | corrected | 2 | 2 | 0 |
| `0240f230` | corrected | 2 | 2 | 0 |
| `02d03b50` | correct | 2 | 2 | 0 |
| `030e79a8` | correct | 2 | 2 | 0 |

**Upgraded: 38** across 17 target clusters. **Held: 15**, concentrated in
`00c39e88` (5/5), `001a3d90` (3/3), `004eb030` (2/2), `00a887b8` (2/2), and
`0159ed50` (2/2).

8 of the 22 targets needed a wording correction (0 rejected outright).
Notably, the exact same flag-polarity error ("NOT yet set" vs. "IS set")
was repeated identically across all four members of the `014b9850`
family — a single root misreading propagated faithfully across the
grouped write-up rather than four independent mistakes.

The self-derived-extent disassembly check found 38 of 53 pending members
(72%) byte-identical — back in this session's normal R##-generation range
after round 21's 2% outlier, confirming the revert to R##-generation
selection restored normal yield.

Decision: [`dec455`](../../sheets/decisions.tsv#dec455).

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
| **Remaining** | **5,102** |

fix2 derived-report regeneration remains unregenerated (honestly marked
`state: stale`/`reason_code: generation_failed`); progress-bar numbers are
short by 5,089 until the gameDB corpus is rebuilt or located.
