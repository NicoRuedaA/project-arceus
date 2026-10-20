# D4 quality spot-check of depth rounds 45-51 (C1) and 60-66 (C2)

Date: 2026-10-20. Scope: update-v262144 `main`, 6,542 accepted upgrades (C1 r45-r51: 3,414; C2 r60-r66: 3,128) including the library-owned families (NET, HAVOK, WWISE, NN_SDK, LUA_CORE) and PLT stubs. Metadata only; the ledger was NOT changed. Raw data stays in the git-ignored `work/d4/spotcheck2/`; scripts are `scripts/d4_spotcheck2_{sample,build,validate,analyze}.py`. Same method as [d4-quality-spotcheck.md](d4-quality-spotcheck.md).

## Method
- Fixed seed `20261021`. 300 real accepted functions, stratified by round (21-22 per round, 14 rounds), oversampling: >=5 instructions (84 sampled), library-owned (276; the pool is mostly library code), PLT stubs (24), and medium-confidence author records (15; no low-confidence record was accepted).
- 30 decoys (stored description swapped from another function; stricter than last time: the donor body must differ in code) and 30 held functions (held by the round gate, see below; author summary shown). 360 items in 12 blind batches of 30.
- Live bodies fetched read-only from `PLA-update-work` / `main.elf` (one Ghidra MCP server, shut down afterwards). The leading "mechanical class" comment of the decompiler output was stripped so readers could not see the earlier description.
- 12 fresh blind reader sub-agents, no knowledge of decoys, held items or original authors; each hand-wrote an own summary, verdict (correct / wrong_detail / unsupported / wrong), quoted instructions and a rationale. The validator found no duplicated rationales or invented quotes; it flagged 17 rationales as very short (terse, not scripted).

## Results (real items)
Raw: 291 correct, 2 wrong_detail, 7 unsupported, 0 wrong. First-pass correct rate 291/300 = 97.0 % (95 % CI of non-correct 3.0 %: 1.4-5.6 %).

| Stratum | n | non-correct |
|---|---|---|
| C1 r45-r51 | 149 | 6 (r46: 3, r47: 1, r49: 2) |
| C2 r60-r66 | 151 | 3 (r61: 2, r66: 1) |
| library-owned | 276 | 8 (NET 6, NN_SDK 1, HAVOK 1) |
| PLT stubs | 24 | 1 |
| >=5 instructions | 84 | 1 |
| <5 instructions | 216 | 8 |
| medium confidence | 15 | 1 |
| rounds with 0 hits | 45, 48, 50, 51, 60, 62-65 | 0 |

## Adjudication of the 9 raw hits
1. 7 "unsupported": the stored text names the callee of a 2-instruction tail branch (3 distinct nn::os event APIs, one nn::socket API on a NET thunk) or says a branch target has no import name. The name comes from the PLT name map, which the blind readers did not have. Checked against `work/d4/plt/plt-map.json`: the names are correct for the targets tested, and the targets said to have no name are indeed absent. Not errors (same blindness artifact as in the first spot-check).
2. 1 wrong_detail, PLT stub: text says the GOT slot address is computed in x16 "then" the pointer is loaded; the add that completes the address comes after the load. Wording-order inaccuracy only (values correct).
3. 1 wrong_detail, a 3-instruction getter: "registers including x0 are unchanged" while x1 is written (ambiguous wording, x0 is indeed unchanged).

After adjudication: 0 semantic errors, 2 literal wording imprecisions out of 300.

## Decoys
30/30 flagged (20 wrong, 10 wrong_detail). Detection rate 100 % (no identical-body decoys this time), so the readers really read the code.

## Estimates over the 6,542 upgrades (Clopper-Pearson 95 %)
| Definition | k/n | Rate | 95 % CI | Functions |
|---|---|---|---|---|
| any raw non-correct verdict | 9/300 | 3.00 % | 1.38-5.62 % | ~196 (90-368) |
| literal wording error after adjudication | 2/300 | 0.67 % | 0.08-2.39 % | ~44 (5-156) |
| real semantic error after adjudication | 0/300 | 0 % | 0-1.22 % | 0 (0-80) |
| library-owned, raw non-correct | 8/276 | 2.90 % | 1.26-5.63 % | n/a |

Caveat: oversampling is deliberate (large, library, PLT, low confidence), so the pooled rates are conservative-to-neutral for the small stubs that dominate the pool, not exactly population-weighted. A per-round estimate is not possible (21-22 draws per round); r46 (3/21) and r49 (2/21) are the highest but all five are PLT-name metadata claims that check out.

## Held functions (495 held in total: 493 "branch to the address right after the body", 1 audit verdict unsupported, 1 correction not confirmed)
30 sampled (28 boundary, both non-boundary holds). Reader verdicts: 24 correct, 1 wrong_detail, 5 unsupported.
- The 5 unsupported are all texts that state the boundary anomaly (the body ends with a branch to the next address); the blind reader cannot see where the body ends. Deterministic check of all 28 sampled boundary holds: 28/28 really end in a `b` to the address right after the body. The holding condition was therefore real in every case, and the texts were otherwise correct.
- The 1 wrong_detail is the "correction not confirmed" hold: it writes the address 0x378a830 into a register that only holds the page; the hold was justified.
- The "audit verdict unsupported" hold was read as correct by the blind reader (a 6-instruction function): that hold looks over-cautious.
Conclusion: holding was justified by structure (28/28), and for the non-boundary holds 1 of 2 was justified. Releasing the boundary holds later would be low-risk if the boundary wording is kept explicit.

## Inconsistent criteria / patterns
1. Metadata-derived claims (callee names from the PLT map, "no import name") are not visible in the instruction list; about 670 texts in these rounds name a callee and about 330 make a "no import name" statement. Strict instruction-only readers flag them as unsupported (7 of 9 hits). Content is true.
2. The `ret`/x30 wording has been fixed in these rounds (about 2,265 texts mention x30); about 39 texts still combine `ret` with "no register" wording (rough text-pattern count).
3. "Computed in x16 then loaded" ordering in PLT stub texts, and "registers including x0 unchanged" when another register is written: wording ambiguity, not behavioural.
4. Held boundary texts rely on knowledge of the body end, which is not in the instruction list.

## Recommendation
No round has to be redone: 0 semantic errors in 300, 14 rounds with no systematic cluster. Optional, not applied (ledger NOT touched): reword the 2 literal items (function ids 032a4e00 in r46, 03242bac in r66), and review the PLT-stub texts for the slot-address ordering wording by script (the instruction body is fixed). Optionally label PLT-name claims as "per PLT map".
