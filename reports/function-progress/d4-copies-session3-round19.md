# D4 depth rounds — new session continuation (round 19)

Scope and method: same as rounds 1-18 (decisions [`dec434`](../../sheets/decisions.tsv#dec434)-[`dec451`](../../sheets/decisions.tsv#dec451)).
4 of 24 candidates discarded by the up-front boundary check: `01b8f7b4` -> `FUN_01b8f430`,
`018fa6c0` -> `FUN_018fa51c`, `018d8e7c` -> `FUN_018d8d00`, `00ea5534` -> `FUN_00ea547c`;
topped up to 24 clean candidates. 2 clean candidates set aside for sheer
size, restarting the deferred-for-size backlog after round 18 cleared it:
`0177fae0` (260 lines), `00f0b338` (244 lines). One target (`00a5fc1c`)
was initially omitted from the write-up despite being read and flagged in
the proposal's intro paragraph — the second such omission this session,
after round 15's `005482fc` — caught by the auditor and supplemented with
its own dedicated audit pass before this round's decision was finalized.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---|---|---|
| `01b31b0c` | correct | 3 | 3 | 0 |
| `018ddb88` | correct | 3 | 3 | 0 |
| `018d8fe4` | correct | 3 | 3 | 0 |
| `01ade044` | correct | 3 | 3 | 0 |
| `01a247f4` | correct | 3 | 3 | 0 |
| `017e2e3c` | correct | 3 | 3 | 0 |
| `018ecba8` | correct | 3 | 2 | 1 |
| `017ed0a4` | correct | 3 | 3 | 0 |
| `0163c16c` | correct | 3 | 3 | 0 |
| `017ed048` | correct | 3 | 2 | 1 |
| `017e8c58` | correct | 3 | 3 | 0 |
| `0179ff24` | corrected | 3 | 3 | 0 |
| `015b4e7c` | correct | 3 | 3 | 0 |
| `00b1265c` | correct | 3 | 3 | 0 |
| `015a3284` | correct | 3 | 0 | 3 |
| `0125d798` | correct | 3 | 3 | 0 |
| `00c4d788` | corrected | 3 | 0 | 3 |
| `00bdc47c` | correct | 3 | 3 | 0 |
| `00bdb534` | corrected | 3 | 3 | 0 |
| `00b295f4` | correct | 3 | 3 | 0 |
| `00984328` | correct | 3 | 3 | 0 |
| `00a5fc1c` | corrected | 3 | 2 | 1 |

**Upgraded: 57** across 22 target clusters. **Held: 9**, concentrated in
`00c4d788` and `015a3284` (3/3 each).

Decision: [`dec452`](../../sheets/decisions.tsv#dec452).

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
| **Remaining** | **5,196** |

fix2 derived-report regeneration remains unregenerated (honestly marked
`state: stale`/`reason_code: generation_failed`); progress-bar numbers are
short by 4,995 until the gameDB corpus is rebuilt or located.
