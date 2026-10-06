# P0 O9 — static update-main path-token candidates

Date: 2026-10-06. Scope: static string-reference candidates from the existing fix2 update-main program against the effective update RomFS path inventory. This is not runtime ownership proof. The update is a patch; the game target is base plus update, not the update alone.

## Method and query outcome

- Read the exact local effective update path inventory (19,095 entries) into memory and compared its full path tokens against existing string data in the fix2 update-main program. Matching used exact path equality and a separate normalized-only comparison (case-folding and slash normalization). Path values were not written to the report or another output file.
- The query used the existing project and existing functions only. It did not read or treat the legacy `strrefs.tsv` as fix2 evidence.
- The query found 118 string data units whose represented values matched effective inventory path tokens. Of those, 93 distinct path tokens had direct references from existing functions: 0 exact-match reference records and 97 normalized-only reference records, spanning 52 distinct functions.
- Matches are against the complete effective inventory, not a delta-only path set. They may therefore refer to unchanged entries as well as changed/added entries; this result does not identify which individual candidate belongs to the 1,191-entry delta.

## Normalized-only direct xrefs by coarse group and extension

`Path tokens` counts distinct matched effective paths in the row. `Xrefs` counts distinct data-reference source locations within existing functions; `functions` counts distinct referenced functions in the row. Rows are aggregate only. A function may occur in more than one row, so row function counts are not additive to the global distinct-function count.

| Coarse group | Extension | Path tokens | Direct xrefs | Functions |
|---|---|---:|---:|---:|
| appli | .arc | 1 | 1 | 1 |
| appli | .bin | 38 | 41 | 27 |
| appli | .trlgt | 1 | 1 | 1 |
| archive | .gfpak | 1 | 1 | 1 |
| chara | .bin | 1 | 1 | 1 |
| chara | .trmdl | 15 | 15 | 2 |
| effect | .ptcl | 19 | 20 | 4 |
| effect | .wbin | 6 | 6 | 6 |
| event | .bin | 5 | 5 | 5 |
| font | .bffnt | 1 | 1 | 1 |
| pokemon | .bin | 2 | 2 | 2 |
| sound | .bin | 3 | 3 | 1 |
| **Total (row counts)** | — | **93** | **97** | **53** |

The global distinct-function count is **52**. Exact-match direct xrefs were **0**; there are no exact-match group rows.

## Evidence boundary

- A direct reference from a string data unit to an existing function is only a static string-xref candidate. This query did not establish an actual loader/file-open call, call-graph reachability, runtime access, loaded state, or semantic ownership.
- No function was created; no analysis, disassembly, pseudocode export, gameDB operation, ledger update, or canonical documentation change was performed.
- Exactly **one** Ghidra headless process was launched, with `-readOnly -noanalysis`, against the specified untouched fix2 project. The query was compiled with `javac -proc:none` against the local Ghidra JARs before launch (202 JARs found).
- Independent project-tree fingerprint before and after: **8 files / 709,739,522 bytes**, SHA-256 `9c6954749402700c183585a6663ffe1643ff01fee2e678102b083cd97805bf31` — unchanged.
- The prior gap/reference report records its separate initialized-memory source-iterator limits; those figures are not part of this path-token query.

## Evidence references

- `reports/function-progress/p0-wave8-static-file-code-candidates.md` — prior inconclusive attempt; this successful query supersedes its unavailable-count result for the present inventory-wide static candidate scope.
- `reports/function-progress/p0-update-main-gap-reference-strata.md` — independent static reference-triage scope and limitations.
- `reports/function-progress/p0-overlay-ownership.md` — effective overlay grouping and unresolved ownership boundary.
- `reports/function-progress/update-v262144-fix2/manifest.json` — fix2 inventory/profile limitations, including unavailable complete fix2 string-reference export.
- Timestamped progress: `work/progress/p0-wave9-static-file-code.log`.
