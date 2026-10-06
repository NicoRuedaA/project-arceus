# P0 fix2 function-progress treemap profile

Date: 2026-10-06. Profile key: `update-v262144-fix2`. Scope: Pokémon Legends: Arceus update v262144 `main` NSO only. The update is a patch and requires the base game; this report does not describe a standalone program or whole-game coverage.

## Pinned inputs and independent scope metrics

| Evidence | Value |
|---|---|
| Update archive | `pk2.nsz`, 52,657,467 bytes, SHA-256 `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446` |
| Canonical inventory | `re/exports/update-main-fix2/functions.tsv`, SHA-256 `829d810c52a7662ec4d3d958866a091a7b926d9fccd0eea1427181669abd2535` |
| Evidence ledger | `sheets/re/function_progress_evidence.tsv`, SHA-256 `5e83e85ac050dac7b23dd38a17ea42d8cab73abc5b3ccb06a88a8bec4f6f8cb8` |
| Inventory | 153,476 unique function rows; 51,275,676 original body bytes |
| Executable `.text` | 53,106,320 bytes; 1,830,644 bytes outside existing function bodies |
| C exports | 153,471 functions, 51,235,532 body bytes |
| Assembly fallback | 5 functions, 40,144 body bytes; represented as `assembly_fallback_exported`, not C pseudocode and not missing exports |
| gameDB module `ns.update.v262144.main` | 153,471 files; 152,634 parsed function rows; 837 indexed files without a parsed row |

The `.text` residual is a separate scope metric, not an invented function inventory row or treemap weight. Function rows partition only the inventory body bytes, once per unique normalized address.

## Evidence and mapping behavior

The manual `sheets/re/function_progress_evidence.tsv` remains the state source and retains its own recorded `update-v262144` build key. The profile joins rows to fix2 by normalized eight-hex `function_id`, rejects every unmatched/duplicate ledger row, and leaves inventory functions absent from the ledger unknown. Analysis, implementation, behavior verification, and binary matching remain independent.

No complete fix2 `strrefs.tsv` is available. Therefore the profile marks subsystem groups unknown and records the mapping as unavailable; it does not reuse the legacy reference export. gameDB parsing status is likewise separate from semantic analysis or behavior.

## Generated artifacts

The profile writes only to `reports/function-progress/update-v262144-fix2/` (`function-progress.tsv`, `analysis.png`, `port.png`, `index.html`, `manifest.json`, and `generation-status.json`). Its generation status is `current`; the manifest records 153,476 functions / 51,275,676 body bytes, archive and inventory hashes above, ledger SHA-256 above, and Rust-source fingerprint SHA-256 `e388b355e086b370295b2b41f3f59b10b43806f21404469a72bb156137f34bfb`. The gameDB index snapshot fingerprint is stored in the generated manifest. The manifest denominator is explicitly `main` NSO function-body bytes, not whole-game coverage. The existing capped legacy key `update-v262144` and its output directory remain separate and were not regenerated.

The global gameDB index has no useful `functions.file_id` index for the correlated anti-join. A repeated `NOT EXISTS` count consequently scanned the large functions table once per file and did not complete. Counts are now derived from the single grouped `LEFT JOIN` already needed to map each file; a read-only query returned all 153,471 module file rows in approximately 0.13 seconds and the generated profile reconciles the expected 837 no-parsed files.

## Addendum — audited assembly source path

The five assembly fallback IDs are enumerated from the audited source directory
`work/pla/decompiled-fix2/asm/`. A regression test creates a separate stale
lookalike directory and confirms the fix2 profile ignores it; the discovered
assembly ID set must match the five pinned inventory entries exactly. After this
path correction, the focused suite passed **35 tests**, and
`python3 .tools/function_progress_treemap.py --build update-v262144-fix2`
regenerated the fix2-only output with status `current`. The legacy profile was
not regenerated.

Progress: `work/progress/p0-fix2-treemap.log`.
