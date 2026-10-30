# D4 depth rounds — second-machine session summary (rounds 25–42)

Rounds 25–42 (`dec458`–`dec475`) ran on a second machine, with the capped Ghidra project
rebuilt from the user's own dump. That project's export reproduces the committed
`sheets/re/functions.tsv` exactly: 68,330 functions with the same addresses and sizes.

## Totals

| | |
|---|---|
| Remaining after round 24 | 5,062 |
| Upgraded in rounds 25–42 | 294 |
| Held | 0 |
| **Remaining** | **4,768** |

Every upgraded record went through the same steps:
- a boundary check;
- byte identity checked against the live Ghidra listing, and again over the full byte span (holes included);
- a description written from the instructions, not the decompiler;
- an audit by an independent reader, which found 0 wrong descriptions. A few corrected ones are listed in each round's report.

## Method findings

- **Truncated and holed bodies.** Ghidra treats several callees as non-returning even though they return: `0x032a4530`, `0x032a2e00`, `0x032a24f0`, `0x032a28b0` and `0x032a45e0`. Recorded bodies, and so inventory sizes, end early or contain holes after these calls.
  - Round 25 compared each function's true extent, continuation included.
  - From round 27 on, targets whose body ends in such a call followed by more code are skipped and logged in each round's `skipped.tsv`.
  - Upgrades from earlier rounds that call these functions may not have had their continuation compared, so a recheck is worthwhile.
- **Selection order.** The identity gate runs before any description is written, so no reading effort goes to clusters that would be held.
- **Templates.** Many remaining small functions are instances of a few templates. A template is described once and audited against every body, with per-target parameters extracted by script. These include script-binding wrappers (cast-and-convert preambles, member-function-pointer calls, argument-count checks).

## Where the pool stands

- **R##-generation targets inside the 68,330-function inventory:** every multi-copy cluster and every templated single-copy cluster up to 800 bytes is done. About 55 irregular single-copy clusters remain, each needing its own read.
- **The 1,231 targets outside that inventory (1,803 member rows):** the project was then seeded with all 153,476 ledger addresses, giving 153,847 functions. Under seeded extents, 1,680 of the 1,803 members (93%) are byte-identical to their target, across 1,134 fully identical clusters.
  - Caveat: seeded extents have not been through the truncation filter. Some seeds landed inside existing functions (69,536 "inside existing" against 68,330 capped functions), and some seeded bodies may end early. These 1,680 are candidates, not verified copies.
  - Next step for them: run the same boundary and truncation filters, then describe and audit them in the same way as rounds 27–42.
- **The "D4 batch 10" generation (~1,933 rows):** not touched. Round 21 measured very low identity in this group.
