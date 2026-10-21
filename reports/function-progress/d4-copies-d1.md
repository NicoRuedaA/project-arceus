# D4 depth rounds (exact copies, batch D1, rounds 100+): propagated byte-identical copies re-read

Scope: update-v262144 `main`. Metadata only (decisions `dec380` onward); summaries stay in the git-ignored `work/d4/copies-d1/`. Same method as [d4-mech-rounds-c2](d4-mech-rounds-c2.md). Pool: the 22,392 ledger records propagated as identical to an already-read function, ordered by size ascending then address; batch D1 takes the even positions (11,196 functions), batch D2 the odd ones. About 4,000 instructions per round.

| Decision | Rounds | Upgraded | Held |
|---|---|---|---|
| `dec380` | 100-104 | 2382 | 1 |
| `dec381` | 105-106 | 503 | 0 |
| `dec382` | 107-110 | 820 | 0 |
| `dec383` | 111-114 | 642 | 4 |
| `dec384` | 115-116 | 279 | 3 |
| `dec385` | 117-120 | 469 | 14 |
| `dec386` | 121-123 | 297 | 34 |
| `dec387` | 124-126 | 304 | 7 |
