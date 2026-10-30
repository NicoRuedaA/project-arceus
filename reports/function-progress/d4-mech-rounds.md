# D4 depth rounds: remaining mechanically classified functions

Scope: update-v262144 `main`. Metadata only (decisions `dec327` onward); summaries stay in the git-ignored `work/d4/mech-rounds/`.

Pool: ledger rows still carrying only the deterministic mechanical rationale, minus library-owned families (NET, HAVOK, WWISE, NN_SDK, LUA_CORE, OODLE) and the pilot's held functions; rounds of 500, most callers first, ties by address. Method as in [d4-pilot500](d4-pilot500.md): live bodies from the working project, hand-written description by one reader, independent audit by a second, third reader for corrections; only audited-correct records are upgraded; no Ghidra mutation.

| Decision | Rounds | Upgraded | Held | Read / mechanical after |
|---|---|---|---|---|
| `dec327` | 1-5 | 2476 | 24 | 104,932 / 25,348 |
| `dec330` | 6-10 | 2457 | 43 | 107,389 / 22,891 |
| `dec331` | 11-15 | 2418 | 82 | 109,807 / 20,473 |
| `dec332` | 16-20 | 2477 | 23 | 117,225 / 13,055 (merged with parallel rounds) |
| `dec333` | 21-22 | 995 | 5 | 120,575 / 9,705 (merged with parallel rounds) |
