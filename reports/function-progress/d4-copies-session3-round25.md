# D4 depth rounds — new session continuation (round 25)

First round run on a second machine, without the private `work/d4/` scratch of
rounds 1–24. The capped Ghidra project was rebuilt from the user's own dump per
`REPRODUCE.md` §1–3: update ExeFS section `c5e6c573…` (PFS0 at +0x8000), `main` NSO
segment hashes verified by `nso_to_elf.py`, 25-minute capped auto-analysis. Its
`ExportTsv` output reproduced the committed `sheets/re/functions.tsv` exactly (68,330
functions, identical addresses and sizes), so extents from this project are the
committed ones.

## Selection

The exclusion set was rebuilt from every committed `d4-copies-*` report (every 8-hex
address mentioned: 187 R##-generation targets). New this round, the byte-identity gate
ran **before** any description was written, over all 1,586 remaining R##-generation
targets:

- 1,090 targets (or their members) have no committed extent: they are outside the
  68,330-function capped inventory, so they need a project seeded from the ledger
  (`SeedFunctions.java`), as in round 1. Not attempted this round.
- Among the sized ones, 320 clusters were fully identical; the 24 largest
  multi-member clean clusters were taken (48 members, 168–732 bytes, at most 175
  decompiled lines, so nothing deferred for size).

The up-front boundary check (live function extent = inventory size) passed for all 72
addresses (24 targets + 48 members); no discards.

## Methodology finding: truncated and holed bodies

Ghidra treats `0x032a4530`, `0x032a2e00` and `0x032a24f0` as non-returning. All three
return, so the recorded body (and the inventory size) of a function that calls them
is holed or truncated:

- after `0x032a4530` a single `b` rejoins the body, sitting in a 4-byte hole between
  body ranges or just past the inventory extent;
- after `0x032a2e00` a 9-instruction block (call `0x00000250`, call `0x032a2e10`, `b`
  back) lies past the extent;
- after `0x032a24f0` the `cbz w0` that returns the found node lies in a hole, so the
  decompiler's `return 0` for `00269100` is wrong.

Descriptions were therefore written from the instructions, not the decompiler, and
the identity gate was run three ways: Ghidra's live listing per body (body size and
range count must match too), `llvm-objdump` over the full `[min, max]` span including
holes, and the true extent including the trailing continuation. All 48 members are
identical under all three. The unwind-only landing pads just past `005353b8` were
also compared and are identical in both copies.

Earlier rounds compared Ghidra's listing of the recorded body; any function whose
continuation lies past its extent would not have had that continuation compared. A
re-check of earlier upgrades that call these three functions may be worth a later
round.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `014aff90` | correct (family F1) | 2 | 2 | 0 |
| `014de8f0` | correct (family F1) | 2 | 2 | 0 |
| `015c7404` | correct (family F1) | 2 | 2 | 0 |
| `014d63f0` | correct (family F1) | 2 | 2 | 0 |
| `020aee80` | correct (family F1) | 2 | 2 | 0 |
| `0271221c` | correct | 2 | 2 | 0 |
| `005353b8` | corrected | 2 | 2 | 0 |
| `024a7294` | correct (pair P1) | 2 | 2 | 0 |
| `00c9ba7c` | correct (pair P1) | 2 | 2 | 0 |
| `011989c0` | correct | 2 | 2 | 0 |
| `01599b58` | correct | 2 | 2 | 0 |
| `00c98a6c` | correct | 2 | 2 | 0 |
| `019dafa8` | corrected (pair P2) | 2 | 2 | 0 |
| `01e9a0a4` | corrected (pair P2) | 2 | 2 | 0 |
| `00ab690c` | correct | 2 | 2 | 0 |
| `00203cd0` | correct | 2 | 2 | 0 |
| `00caeb50` | correct | 2 | 2 | 0 |
| `0295eb20` | correct | 2 | 2 | 0 |
| `00ce3110` | correct | 2 | 2 | 0 |
| `0137dd4c` | correct | 2 | 2 | 0 |
| `00269100` | correct | 2 | 2 | 0 |
| `003a7740` | correct | 2 | 2 | 0 |
| `00591950` | correct | 2 | 2 | 0 |
| `003a8090` | correct | 2 | 2 | 0 |

**Upgraded: 48** across 24 target clusters. **Held: 0.**

Family F1 (5 targets) is one body differing only in the field offset read on its
fallback path (`0xd8`/`0xb8`/`0xb0`/`0x160`/`0xd0`). Pairs P1 and P2 are the same
operations with different register assignment or block order. The audit was split up
front into two independent 12-target runs (after round 24's single-run output-limit
failure). Both completed: 0 wrong, 3 targets corrected under 2 headings, 21 correct.
The corrections: for pair P2, the description wrongly said the power-of-two scan loop
moves between the two copies; in fact only the general loop's `*(x+0x10) == k` check
block moves. For `005353b8`, the landing-pad stubs all jump to `0x535608` rather than
each being "+0xc".

Decision: [`dec458`](../../sheets/decisions.tsv#dec458).

## Running total, this session

| | |
|---|---|
| Remaining after round 24 (dec457) | 5,062 |
| Round 25 (dec458) | +48 |
| **Remaining** | **5,014** |

The fix2 derived reports are still not regenerated (same gameDB-corpus gap as rounds
1–24), so the progress-bar numbers stay stale.
