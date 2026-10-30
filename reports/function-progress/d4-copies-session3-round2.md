# D4 depth rounds — new session continuation (round 2)

Scope and method: same as [round 1](d4-copies-session3.md) (decision
[`dec434`](../../sheets/decisions.tsv#dec434)). This round selected the next 20
largest remaining "identical-to" clusters by member count (ranks 7-26 of the
7,635-record remaining pool after round 1), plus `007c4b3c` carried over from
round 1 (read but never written up there).

## New finding: member-side function-boundary merges

Round 1 found that a *target* address can resolve into a larger containing
function in this freshly-reseeded project. This round found the same problem
can also affect *members*: within the otherwise-clean target `001bbe18`, every
one of its 43 claimed members individually resolved into unrelated larger
functions (e.g. claimed member `001c2a80` actually lies inside `FUN_001c24f0`,
a 395-instruction function, not a standalone 224-byte one) — so the live
disassembly fetched for "the member" was never the member at all. The whole
cluster is held this round regardless of audit verdict, since nothing about it
could be verified.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `00802998` | correct | 55 | 55 | 0 |
| `001bc7b0` | corrected | 54 | 54 | 0 |
| `00c870dc` | correct, but own address merges into `FUN_00c870d4` | 51 | — | 51 (deferred) |
| `0121fce0` | corrected | 50 | 46 | 4 |
| `00857f7c` | correct, but own address merges into `FUN_00857f18` | 48 | — | 48 (deferred) |
| `03034c34` | correct | 47 | 47 | 0 |
| `010822b4` | corrected | 43 | 43 | 0 |
| `001c15cc` | corrected | 43 | 43 | 0 |
| `001c14b8` | corrected | 43 | 43 | 0 |
| `001c1200` | corrected | 43 | 43 | 0 |
| `001c0f7c` | corrected | 43 | 43 | 0 |
| `001bbe18` | corrected, but every member individually merges into an unrelated function | 43 | 0 | 43 (deferred) |
| `001b93f0` | correct | 43 | 43 | 0 |
| `02667350` | corrected, but own address merges into `FUN_0265f458` | 42 | — | 42 (deferred) |
| `015220b0` | corrected, but 0/41 members verified | 41 | 0 | 41 |
| `00fdc6c0` | corrected | 40 | 32 | 8 |
| `02bd6d54` | corrected, but own address merges into `FUN_02bd6c0c` | 38 | — | 38 (deferred) |
| `00c980bc` | correct | 38 | 38 | 0 |
| `016f182c` | corrected | 36 | 36 | 0 |
| `007c4b3c` | corrected | 138 | 98 | 40 |
| `0085e898` | **UNSUPPORTED** | — (not in this round's target list after audit) | — | — |

**Upgraded: 664** across 14 clean target clusters. **Held: 315** (266 to the
4 boundary-merged targets + `001bbe18`, the rest to genuine per-member
non-identity or isolated audit-visible anomalies).

The independent audit this round caught substantially more than round 1,
including: wrong call-argument sources (`010822b4` reads release args from the
container's own fields, not the extracted object's), a miscounted descriptor
field count (`001c15cc`'s tag-6 entry has 4 fields, not 2), a wholly-omitted
4th conditional entry (`001c0f7c`'s tag 6), a dead-code branch identified from
the function's own invariants (`001c1200`'s `*(param_1+0x180)==0` path is
unreachable given `+0x180` is always set to `param_1` itself at entry), and a
false "two different thresholds" claim in `007c4b3c` that the audit showed was
actually one identical test reached from two branches.

Decision: [`dec435`](../../sheets/decisions.tsv#dec435).

## Running total, this session

| | |
|---|---|
| Pool (ledger rows marked `identical to X`, never read individually) | 22,392 |
| Upgraded before this session (dec380-398, dec410, dec433) | 12,201 |
| Upgraded round 1 (dec434) | 2,556 |
| Upgraded round 2 (dec435) | 664 |
| **Remaining** | **6,971** |

fix2 derived-report regeneration remains unregenerated (see round 1's note);
the progress-bar/treemap numbers in `README.md`/`docs/PROGRESS.md` are short
by 3,220 (2,556 + 664) until the gameDB corpus is rebuilt or located.
