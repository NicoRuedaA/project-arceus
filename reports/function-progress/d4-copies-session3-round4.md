# D4 depth rounds — new session continuation (round 4)

Scope and method: same as rounds 1-3 (decisions [`dec434`](../../sheets/decisions.tsv#dec434),
[`dec435`](../../sheets/decisions.tsv#dec435), [`dec436`](../../sheets/decisions.tsv#dec436)).
6 of 20 candidates discarded by the up-front boundary check: `00863eb8` -> `FUN_00863cf0`,
`015cacd4` -> `FUN_015cac1c`, `015ae81c` -> `FUN_015ae810`, `01168414` -> `FUN_01168078`,
`01060f90` -> `FUN_01060e7c`, `00f50014` -> `FUN_00f4ff5c`.

An audit subagent failed mid-round on an unrelated session-usage-limit error
(not a repository or methodology problem); relaunched after the limit reset,
no partial/corrupt state resulted since nothing is written to the ledger
before verification completes.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `01061aec` | corrected | 25 | 0 | 25 |
| `00c2739c` | correct | 24 | 0 | 24 |
| `01772e48` | correct | 23 | 0 | 23 |
| `031a8b58` | correct | 23 | 23 | 0 |
| `01dd0768` | corrected | 23 | 23 | 0 |
| `00f52368` | correct | 23 | 23 | 0 |
| `02bdff8c` | corrected | 22 | 22 | 0 |
| `001c1174` | correct | 21 | 21 | 0 |
| `026b3fd8` | correct | 21 | 21 | 0 |
| `00192174` | correct | 19 | 19 | 0 |
| `00a50250` | corrected (scope-of-function finding, see above) | 18 | 18 | 0 |
| `02be5934` | corrected | 18 | 18 | 0 |
| `0141d37c` | correct | 17 | 17 | 0 |
| `026b51fc` | corrected | 17 | 17 | 0 |

**Upgraded: 222** across 11 target clusters. **Held: 72**, all concentrated in
the 3 most structurally intricate targets of the batch (`01061aec`'s odd
early-return capacity-hint branch, `00c2739c`'s double independent
release-triple, `01772e48`'s hash-insert with two different allocation
sizes on different paths) — each already flagged with explicit uncertainty
in the proposal before the audit ran. Unlike the `015356b4`/`015220b0`/`026b444c`
cases in prior rounds (correct description, non-identical members), these
three read as genuinely more complex functions whose compiled control flow
does not match member-to-member; no single "different embedded constant"
explanation was identified. Left untouched, available for a dedicated look.

Decision: [`dec437`](../../sheets/decisions.tsv#dec437).

## Running total, this session

| | |
|---|---|
| Pool | 22,392 |
| Before this session | 12,201 |
| Round 1 (dec434) | +2,556 |
| Round 2 (dec435) | +664 |
| Round 3 (dec436) | +346 |
| Round 4 (dec437) | +222 |
| **Remaining** | **6,403** |

fix2 derived-report regeneration remains unregenerated (see round 1's note);
progress-bar/treemap numbers are short by 3,788 (2,556+664+346+222) until the
gameDB corpus is rebuilt or located.
