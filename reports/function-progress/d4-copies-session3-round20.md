# D4 depth rounds — new session continuation (round 20)

Scope and method: same as rounds 1-19 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec452`](../../sheets/decisions.tsv#dec452)).
24 clean candidates, no boundary-check discards this round. 3 clean
candidates set aside for sheer size, restarting the deferred-for-size
backlog after round 18 cleared it: `0177fae0` (260 lines, carried from
round 19), `00f0b338` (244 lines, carried from round 19), `00524290` (233
lines, new this round).

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `00984080` | correct | 3 | 3 | 0 |
| `009246f0` | corrected | 3 | 0 | 3 |
| `008f2e50` | correct | 3 | 3 | 0 |
| `008f2ba0` | correct | 3 | 3 | 0 |
| `008f2494` | correct | 3 | 3 | 0 |
| `008f220c` | correct | 3 | 3 | 0 |
| `008f18e4` | correct | 3 | 3 | 0 |
| `008f160c` | correct | 3 | 3 | 0 |
| `008ccde4` | correct | 3 | 3 | 0 |
| `007ff75c` | corrected | 3 | 1 | 2 |
| `007b41d4` | corrected | 3 | 0 | 3 |
| `00780560` | corrected | 3 | 2 | 1 |
| `0071d3fc` | corrected | 3 | 3 | 0 |
| `0054b07c` | **wrong — rejected** | 3 | 0 | 3 |
| `00534abc` | correct | 3 | 3 | 0 |
| `0051f57c` | correct | 3 | 2 | 1 |
| `00504ff0` | correct | 3 | 2 | 1 |
| `002d2904` | correct | 3 | 2 | 1 |
| `002a6bdc` | correct | 3 | 3 | 0 |
| `001b95e8` | correct | 3 | 3 | 0 |
| `001ac650` | correct | 3 | 3 | 0 |

**Upgraded: 48** across 20 target clusters (`0054b07c` excluded entirely —
its description was rejected outright by the auditor, so its 3 members are
held regardless of the fact that all 3 independently passed the
byte-identity disassembly check; only audited-correct descriptions are
eligible for upgrade). **Held: 15** — 12 for genuine non-identity
(concentrated in `009246f0` and `007b41d4`, 3/3 each, both pre-flagged
LOW/MEDIUM CONFIDENCE for density) plus `0054b07c`'s 3 rejected-description
holds.

`0054b07c` is the first outright "wrong" verdict (full rewrite required,
not just a wording correction) since round 17; it compounded three errors —
misattributed field stores, a miscalled "second" global that was actually
the same one reused, and a mischaracterized call argument — caught in full
by the independent audit. The density/decompiler-warning confidence flags
on `009246f0`, `007ff75c`, and `007b41d4` correlated directly with their
hold rates (3/3, 2/3, 3/3), confirming those flags were substantive rather
than reflexive hedging.

Decision: [`dec453`](../../sheets/decisions.tsv#dec453).

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
| **Remaining** | **5,148** |

fix2 derived-report regeneration remains unregenerated (honestly marked
`state: stale`/`reason_code: generation_failed`); progress-bar numbers are
short by 5,043 until the gameDB corpus is rebuilt or located.
