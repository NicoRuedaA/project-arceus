# P0 Global gameDB Paired Fingerprint Gate

Status: **paired gate PASS; no gameDB command or index was run**.

The source-verification report's exact manifest-record procedure was applied in
one script to both the six specified source roots and their corresponding
staged roots. It independently generated the source and staging manifests,
checked exact relative-path sets, and byte-compared every matching file pair.

## Paired results

| Module | Source `.c` files | Staged `.c` files | Source bytes | Staged bytes |
|---|---:|---:|---:|---:|
| `base-v0/main` | 3,997 | 3,997 | 52,147,883 | 52,147,883 |
| `update-v262144/main` | 153,471 | 153,471 | 445,177,385 | 445,177,385 |
| `aux/rtld` | 31 | 31 | 32,481 | 32,481 |
| `aux/sdk` | 9,063 | 9,063 | 6,926,005 | 6,926,005 |
| `aux/subsdk0` | 2,959 | 2,959 | 9,445,415 | 9,445,415 |
| `aux/subsdk1` | 8,274 | 8,274 | 28,226,651 | 28,226,651 |
| **Total** | **177,795** | **177,795** | **541,955,820** | **541,955,820** |

- Exact relative-path sets: **matched in all six modules**; **0 missing, 0 extra**.
- Direct byte comparisons: **177,795 pairs; 0 mismatches**.
- Source aggregate manifest fingerprint: `fadb16a7b38a2def0bb352d855272c0076c578720b3900a2401f5564a84d2b59`.
- Staging aggregate manifest fingerprint: `fadb16a7b38a2def0bb352d855272c0076c578720b3900a2401f5564a84d2b59`.
- Staged `.gamedb/`: **absent**.
- **Paired gate: PASS.**

## Procedure and interpretation

For each `.c` file, the same script formed the record
`UTF8(module) + NUL + UTF8(relative POSIX path within that module) + NUL + ASCII(decimal byte length) + NUL + ASCII(lowercase SHA-256(file bytes)) + LF`.
Records were sorted lexicographically by `(module, relative POSIX path)`;
SHA-256 of their concatenation produced each aggregate fingerprint. Source and
staging manifests were calculated independently with this procedure.

The earlier preparation and pre-index/index reports contain fingerprints from
inconsistent or unrecorded procedures. Those older fingerprints are superseded
for source-vs-staging identity by this paired gate; the differing historical
values do not contradict the independently equal manifests and complete byte
comparison documented here. This PASS establishes corpus identity only. It
does not run, authorize, or claim results from indexing.
