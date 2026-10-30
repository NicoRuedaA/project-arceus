# D4 depth rounds — new session continuation (round 27)

Same setup as rounds 25-26 (capped Ghidra project, live-listing and full-span identity gates, boundary check, descriptions from the instructions). Selection changed to the smallest remaining fully identical R##-generation clusters (80-164 bytes), with a new filter: a target is skipped when its recorded body is truncated, i.e. it ends at a call followed by more code (not a new function), or has a hole holding more than one branch. Of the 32 smallest candidates examined, 17 were truncated this way (Ghidra treats their last callee as non-returning, so the inventory extent is only a fragment); they are logged in the round's skipped.tsv and left for a pass that recovers true extents. 15 kept targets, all single-copy; boundary check and both identity gates passed for all 30 addresses. Nine targets share one template (same preamble, different gate global, helper and tail target) and were described once with a per-target parameter table. One independent auditor: 15 correct, 0 corrected, 0 wrong.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `0300fc98` | correct | 1 | 1 | 0 |
| `0054a084` | correct | 1 | 1 | 0 |
| `0315070c` | correct | 1 | 1 | 0 |
| `00a22cd8` | correct | 1 | 1 | 0 |
| `002eea08` | correct | 1 | 1 | 0 |
| `016324f0` | correct | 1 | 1 | 0 |
| `01fa1ac0` | correct | 1 | 1 | 0 |
| `0075b694` | correct | 1 | 1 | 0 |
| `00d56200` | correct | 1 | 1 | 0 |
| `00d56930` | correct | 1 | 1 | 0 |
| `00d56e00` | correct | 1 | 1 | 0 |
| `01fbe224` | correct | 1 | 1 | 0 |
| `0228f360` | correct | 1 | 1 | 0 |
| `0228fa98` | correct | 1 | 1 | 0 |
| `02380b78` | correct | 1 | 1 | 0 |

**Upgraded: 15. Held: 0.**

Decision: [`dec460`](../../sheets/decisions.tsv#dec460).

## Running total

| | |
|---|---|
| Remaining before this round | 4,997 |
| Round 27 (dec460) | +15 |
| **Remaining** | **4,982** |
