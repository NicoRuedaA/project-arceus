# D4 depth rounds — new session continuation (round 10)

## Scope and method

Same method as rounds 1–9. 24 candidates ranked, 12 merged-boundary candidates discarded, two oversized/dense candidates deferred, leaving 18 independently audited target functions. The complete proposed descriptions and complete per-target independent audit text are preserved in the git-ignored `work/d4/copies-session3/round10-descriptions.md`. The auditor found 7 correct, 11 corrected, 0 rejected.

## Verification

The 112 proposed member functions were re-disassembled live from GhidraMCP 6.0.0 (`main.elf`). Their own instruction extents were derived from last instruction + 4, and in-function branch targets normalized relative to each function start before byte-text comparison. 99 matched the audited target; 13 mismatched and are held; 0 fetch errors. No held row is advanced.

## Results

| Target | Audit verdict | Cluster size | Verified identical | Held |
|---|---|---:|---:|---:|
| `00ce1bf4` | correct | 7 | 7 | 0 |
| `00b9492c` | corrected | 7 | 7 | 0 |
| `008c29a4` | correct | 7 | 7 | 0 |
| `00534138` | corrected | 7 | 6 | 1 |
| `0311e188` | correct | 6 | 6 | 0 |
| `0303282c` | corrected | 6 | 6 | 0 |
| `02fa9710` | correct | 6 | 6 | 0 |
| `02bdd7b8` | correct | 6 | 6 | 0 |
| `02a88894` | corrected | 6 | 6 | 0 |
| `0294f5a0` | corrected | 6 | 6 | 0 |
| `023a626c` | corrected | 6 | 0 | 6 |
| `020a3cf0` / `01f19020` | corrected | 12 | 9 | 3 |
| `01c10fe4` | corrected | 6 | 3 | 3 |
| `01840608` | correct | 6 | 6 | 0 |
| `00d48d04` | corrected | 6 | 6 | 0 |
| `00c7dbc0` | correct | 6 | 6 | 0 |
| `00a6dd70` | corrected | 6 | 6 | 0 |

**Upgraded: 99** across 18 target clusters. **Held: 13**, all disassembly mismatches; no fetch errors.

Decision: `dec443`.
