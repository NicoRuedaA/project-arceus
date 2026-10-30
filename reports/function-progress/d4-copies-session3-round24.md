# D4 depth rounds — new session continuation (round 24)

Back to normal-sized R##-generation batches after round 23's backlog
clear. 24 clean candidates, no boundary-check discards this round (all
pre-verified during rounds 22/23 selection). 2 clean candidates set aside
for sheer size, restarting the deferred-for-size backlog after round 23
cleared it: `00fb03f4` (243 lines), `00aff84c` (219 lines).

The audit was split into two parallel 11-target subagent runs after the
first attempt at a single combined audit failed outright on an API error
(response exceeded the 64,000-output-token limit, with zero progress
saved) — a new failure mode for this session's audit pipeline. Both split
audits succeeded.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `00549fc4` | corrected | 2 | 2 | 0 |
| `03023e40` | correct | 2 | 2 | 0 |
| `02511954` | **wrong — rejected** | 2 | 0 | 2 |
| `00d3c940` | correct | 2 | 2 | 0 |
| `00e89560` | correct | 2 | 2 | 0 |
| `0054af04` | corrected | 2 | 2 | 0 |
| `00027b70` | corrected | 2 | 2 | 0 |
| `00226ad0` | correct | 2 | 2 | 0 |
| `002285d0` | correct | 2 | 2 | 0 |
| `005342a8` | corrected | 2 | 2 | 0 |
| `00536860` | correct | 2 | 2 | 0 |
| `00c9361c` | correct | 2 | 0 | 2 |
| `017e9368` | correct | 2 | 2 | 0 |
| `01d15df4` | correct | 2 | 2 | 0 |
| `00226990` | correct | 2 | 2 | 0 |
| `00228490` | correct | 2 | 2 | 0 |
| `0280f77c` | correct | 2 | 1 | 1 |
| `0181c614` | correct | 2 | 0 | 2 |
| `011f6128` | correct | 2 | 1 | 1 |
| `01164a0c` | corrected | 2 | 0 | 2 |
| `01b7fa78` | correct | 2 | 0 | 2 |
| `00f0cb28` | corrected | 2 | 2 | 0 |

**Upgraded: 32** across 20 target clusters (`02511954` excluded entirely
— rejected outright by the audit, so its 2 members are held regardless of
both passing the byte-identity disassembly check; only audited-correct
descriptions are eligible for upgrade). **Held: 12** — 10 for genuine
non-identity (concentrated in `00c9361c`, `0181c614`, `01164a0c`, and
`01b7fa78`) plus `02511954`'s 2 rejected-description holds.

Notable catches: `02511954` was mistraced as operating on 2-word element
pairs doing a multi-element range erase, when symbolic pointer-arithmetic
tracing showed it is actually a classic single-iterator `vector::erase` on
1-word (pointer) elements; `0054af04`'s guard-byte semantics were inverted
(its vtable+0x10 call is an already-initialized assertion firing when the
guard IS set, matching `00549fc4`'s assertion-tail shape, not a setup call
gated before the guard is set) — and `00549fc4` itself had its own
flag-byte condition stated backwards; `00027b70`'s fourth sub-decoder
(type codes 2 and 4) takes a boolean discriminator instead of the
destination range the other three sub-decoders take, and only 4 of its
many failure paths actually call the error-report helper pair (most just
return `-1` silently); `01164a0c`'s hot-path vtable+0x18 calls were
misdescribed as releasing an old value when they actually retain/share
the just-copied new value (`param_3` is left unmodified, unlike a
destructive move), and its cold-path field exchange is 3 cross-structure
swaps plus one same-structure copy, not 4 swapped pairs.

Decision: [`dec457`](../../sheets/decisions.tsv#dec457).

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
| Round 24 (dec457) | +32 |
| **Remaining** | **5,062** |

fix2 derived-report regeneration remains unregenerated (honestly marked
`state: stale`/`reason_code: generation_failed`); progress-bar numbers are
short by 5,113 until the gameDB corpus is rebuilt or located.
