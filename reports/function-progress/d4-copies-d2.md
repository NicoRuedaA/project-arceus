# D4 depth rounds (batch D2, exact-copy functions)

Scope: update-v262144 `main`. Metadata only (decisions `dec390` onward); summaries stay in the git-ignored `work/d4/copies-d2/`. Same method as [d4-mech-rounds-c2](d4-mech-rounds-c2.md). Pool: the 22,392 ledger records propagated as 'identical to X' (same bytes and call targets as an already-read function) and never read individually, ordered by size ascending then function id; this agent takes the odd 0-based positions (11,196 records), the parallel agent takes the even ones. Rounds are budgeted by instructions (about 4,000 per round), names r200 onward.

| Decision | Rounds | Upgraded | Held |
|---|---|---|---|
| `dec390` | 200-202 | 1800 | 0 |
