# D4 depth rounds — new session continuation (round 6)

Scope and method: same as rounds 1-5 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec438`](../../sheets/decisions.tsv#dec438)).
6 of 24 candidates discarded by the up-front boundary check: `00f81748` -> `FUN_00f816b0`,
`00d7f484` -> `FUN_00d7f3cc`, `0083b3f4` -> `FUN_0083b2d4`, `018cf1d0` -> `FUN_018cecd8`,
`00ea6eb0` -> `FUN_00ea6898`, `0262d99c` -> `FUN_0262d890`. 3 more clean candidates
set aside this round for sheer size rather than described: `0262d890` (456
decompiled lines, dozens of PIC-construction/unreachable-block warnings),
`0262d224` (320 lines), `016f1a08` (204 lines) — a dedicated future round.

## Results

| Target(s) | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `00781b1c` | correct | 12 | 12 | 0 |
| `008b2570` | corrected (dead-code release claim) | 13 | 13 | 0 |
| `008b3558` | corrected | 12 | 12 | 0 |
| `009d6140` | corrected (same as 008b2570) | 12 | 12 | 0 |
| `00a501a0` | corrected | 13 | 13 | 0 |
| `00c00fa8` | corrected (omitted 3rd gate) | 12 | 5 | 7 |
| `00c15ee8` | correct | 12 | 8 | 4 |
| `00cfbf40` | corrected (same as 00a501a0) | 12 | 12 | 0 |
| `00e2c400` | correct | 11 | 9 | 2 |
| `0264af50` | corrected | 13 | 12 | 1 |
| `027ca9dc` | corrected (inverted grow/shrink framing) | 12 | 6 | 6 |
| `029e62dc` | corrected (dead-code release claim) | 11 | 10 | 1 |
| `02f7a15c` | correct | 11 | 11 | 0 |
| `030ff760` | corrected (inverted branch condition) | 12 | 12 | 0 |
| `015e45a0` | correct | 11 | 9 | 2 |

**Upgraded: 156** across 13 target clusters. **Held: 23**, spread across 7
clusters rather than concentrated in one — none of this round's held members
trace to a single "shape-similar, different constant" or "all-members-merged"
pattern; each looks like ordinary per-instance divergence.

Audit continues to earn its keep: two inverted branch conditions
(`030ff760`'s vtable+8/vtable+0x20 dispatch was backwards; `027ca9dc`'s
"growing" paragraph was describing the shrink path) and two cases of a
described "release" that is actually dead code as decompiled (`008b2570`/`009d6140`'s
displaced-value release always reads an already-zeroed slot; `029e62dc`'s
trailing-pair release checks a variable that's unconditionally zeroed first) —
exactly the kind of error that would have quietly corrupted the ledger's
behavioral record if shipped uncorrected.

Decision: [`dec439`](../../sheets/decisions.tsv#dec439).

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
| **Remaining** | **6,131** |

fix2 derived-report regeneration remains unregenerated; progress-bar/treemap
numbers are short by 4,060 until the gameDB corpus is rebuilt or located.
