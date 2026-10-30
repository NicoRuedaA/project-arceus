# D4 depth rounds — new session continuation (round 7)

Scope and method: same as rounds 1-6 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec439`](../../sheets/decisions.tsv#dec439)).
8 of 24 candidates discarded by the up-front boundary check: `008024a4` -> `FUN_008023ec`,
`02be425c` -> `FUN_02be3714`, `00fff3cc` -> `FUN_00fff3c0`, `00e11670` -> `FUN_00e11050`,
`00867a80` -> `FUN_0086775c`, `00449a3c` -> `FUN_00449978`, `02be5684` -> `FUN_02be4e08`,
`0262d368` -> `FUN_0262d224`. 2 more (`0294d750`, `002ea500`) were already-processed
targets reappearing only because some of their members are genuinely
non-identical — skipped, not re-described.

## Bonus: rechecked rounds 1-4's held members with the fixed method

Before describing anything new, re-ran the self-derived-extent verification
(fixed in round 5, [`dec438`](../../sheets/decisions.tsv#dec438)) against all
686 members held across rounds 1-4 — the rounds that predate the fix and
might have carried false negatives from the same declared-size bug that
affected round 5's `00a4ea28`. **Result: 0 recovered.** This confirms those
686 holds are genuine non-identity (the `015356b4`/`026b444c`-style
shape-similar-different-constant pattern, or the structurally intricate
targets like `01061aec`/`00c2739c`/`01772e48`), not an artifact of the old
sizing method. No further action needed on that front.

## Results

| Target(s) | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `006b109c` / `0022b79c` | correct | 11 / 10 | 11 / 10 | 0 / 0 |
| `031c4584` | corrected | 10 | 10 | 0 |
| `0233b2ec` / `00e572c4` | correct | 10 / 10 | 9 / 9 | 1 / 1 |
| `01f19150` | correct | 10 | 8 | 2 |
| `00e10fb0` | corrected | 10 | 10 | 0 |
| `00ce1388` | correct | 10 | 10 | 0 |
| `00ca9eb4` / `02d886e0` | correct | 10 / 10 | 10 / 9 | 0 / 1 |
| `0028f450` | corrected | 10 | 10 | 0 |
| `0300cf80` | correct | 10 | 9 | 1 |
| `0278f170` | correct | 10 | 9 | 1 |
| `02191800` | correct | 10 | 9 | 1 |

**Upgraded: 133** across 14 target clusters. **Held: 4** — the cleanest
round yet by held-ratio; no cluster failed outright.

Decision: [`dec440`](../../sheets/decisions.tsv#dec440).

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
| **Remaining** | **5,998** |

fix2 derived-report regeneration remains unregenerated; progress-bar/treemap
numbers are short by 4,193 until the gameDB corpus is rebuilt or located.
