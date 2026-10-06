# P0 Wave 8 — Added-content hash-match cross-tab

**Date:** 2026-10-06. **Status:** independently reproduced from the currently
available local virtual RomFS trees; aggregate-only results.

## Method and coverage

Used the documented base and update virtual-RomFS inventories and extracted
trees, without writing per-file match data. The inventories contained exactly
18,370 base entries and 19,095 update entries. Every inventoried file was
readable and matched its recorded size. SHA-256 digests were computed in memory
for all base payloads and compared against each of the 725 update-only entries;
the additions were aggregated by the exact existing `/bin/<directory>` group
and extension labels in `p0-update-changed-content-classification.md` and
`p0-overlay-ownership.md`.

## Reproduced whole-population totals

| Exact-base-hash status | Added entries |
|---|---:|
| Exact SHA-256 match to some base payload | **33** |
| No exact SHA-256 match in the base payload set | **692** |
| Unknown due to unreadable or inconsistent input | **0** |
| **Total additions** | **725** |

## By existing heuristic directory group

| Existing group | Match | Non-match | Unknown | Total |
|---|---:|---:|---:|---:|
| `message` | 30 | 290 | 0 | 320 |
| `trainer` | 0 | 309 | 0 | 309 |
| `haxe` | 0 | 74 | 0 | 74 |
| `event` | 2 | 10 | 0 | 12 |
| `appli` | 1 | 3 | 0 | 4 |
| `field` | 0 | 4 | 0 | 4 |
| `misc` | 0 | 2 | 0 | 2 |
| `archive` | 0 | 0 | 0 | 0 |
| `flagwork` | 0 | 0 | 0 | 0 |
| `pokemon` | 0 | 0 | 0 | 0 |
| `chara` | 0 | 0 | 0 | 0 |
| **Total** | **33** | **692** | **0** | **725** |

## By existing extension label

| Extension | Match | Non-match | Unknown | Total |
|---|---:|---:|---:|---:|
| `.bin` | 2 | 325 | 0 | 327 |
| `.dat` | 30 | 130 | 0 | 160 |
| `.tbl` | 0 | 160 | 0 | 160 |
| `.blua` | 0 | 74 | 0 | 74 |
| `.bntx` | 1 | 3 | 0 | 4 |
| **Total** | **33** | **692** | **0** | **725** |

## Interpretation and limitations

An exact match means byte identity to at least one payload in the base set; it
does not establish semantic ownership, code-to-file linkage, or historical
provenance. A non-match does not prove that content was created for this update.
Directory-group and extension labels are the existing heuristic classifications,
not semantic ownership categories. The base digest lookup and per-addition
comparisons existed only in memory; no paths, individual hashes, payload bytes,
or per-file match records are reported or retained here. This does not change
the established conclusion that semantic ownership is unknown for all 1,191
modified/added overlay entries.

No Ghidra/gameDB work, canonical edits, evidence-ledger changes, or repository
history operations were performed.
