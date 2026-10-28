# D4 depth rounds — new session continuation (round 13, dedicated deferred-backlog round)

Scope and method: same as rounds 1-12 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec445`](../../sheets/decisions.tsv#dec445)),
but this round deliberately clears the full "deferred for size" backlog
accumulated since round 6 instead of selecting fresh size-descending
candidates. 13 targets, all previously set aside as too large/dense for
their original rounds: `0262d890` (457 lines), `0262d224` (321), `016f1a08`
(205), `01b98110` (279), `00d797e8` (294), `00d378f0` (198), `00a5156c`
(414), `027a72c4` (308), `02a6e14c` (381), `0264b630` (1375 — the largest
target processed this session), `0251d1f8` (699), `0135923c` (377),
`00f4e708` (244). All 13 were already boundary-checked clean in their
original rounds (no merges).

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `0262d890` | corrected | 11 | 0 | 11 |
| `0262d224` | corrected | 11 | 9 | 2 |
| `016f1a08` | correct | 11 | 0 | 11 |
| `01b98110` | corrected | 9 | 0 | 9 |
| `00d797e8` | correct | 8 | 3 | 5 |
| `00d378f0` | corrected | 8 | 8 | 0 |
| `00a5156c` | corrected | 7 | 0 | 7 |
| `027a72c4` | corrected | 6 | 5 | 1 |
| `02a6e14c` | corrected | 5 | 0 | 5 |
| `0264b630` | correct | 5 | 0 | 5 |
| `0251d1f8` | correct | 5 | 5 | 0 |
| `0135923c` | corrected | 5 | 0 | 5 |
| `00f4e708` | correct | 5 | 5 | 0 |

**Upgraded: 35** across 13 target clusters. **Held: 61**, an unusually high
hold rate (seven targets held their entire cluster) explained by the
batch's density: these are the most structurally complex functions
processed this session, and per-instance variation in embedded constants
(allocator-helper addresses, struct sizes, stamped function-pointer
constants) is correspondingly more common. Spot-checked `0262d890` against
member `02630154` via direct normalized-disassembly diff to confirm the
holds are genuine (a different pool-allocator callee, a different
struct-size immediate, and different stamped function pointers), not a
verification-method artifact.

Decision: [`dec446`](../../sheets/decisions.tsv#dec446).

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
| **Remaining** | **5,469** |

fix2 derived-report regeneration remains unregenerated (honestly marked
`state: stale`/`reason_code: generation_failed`); progress-bar numbers are
short by 4,722 until the gameDB corpus is rebuilt or located. The
deferred-for-size backlog is now fully cleared — the next round returns to
normal size-descending fresh-candidate selection.
