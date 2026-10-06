# Update-main full pseudocode export and gameDB indexing

Date: 2026-10-05. Build: update-v262144, `main` NSO only (`pk2.nsz`, SHA-256
`f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`).
Decision: [`dec114`](../../sheets/decisions.tsv). This report holds metadata only;
pseudocode, disassembly and the gameDB index stay local and ignored by git.

## Result

| Dimension | Before | After | Byte-weighted after |
|---|---|---|---|
| Pseudocode exported (`decompiled/update-main/*.c`) | 12,303 | **68,327 / 68,330** | 99.8714 % |
| gameDB files, module `ns.update.main` | 10,019 | **68,327** | — |
| gameDB function rows, `ns.update.main` | 9,767 | **68,326** | 99.8706 % |
| Indexed without a function row, `ns.update.main` | 252 | **1** | 0.0009 % |
| gameDB function rows, `ns.base.main` (3,997 files) | 3,917 | **3,997** | — |

Source: `reports/function-progress/update-v262144/function-progress.tsv`
(regenerated, `generation-status.json` state `current`). Exported or parsed
pseudocode is not analysis, implementation, behavior or binary-match evidence
(`AGENTS.md`); those states are unchanged.

## Export

Tool: Ghidra 12.1.2 headless, project `PLA-update-capped`, `-noanalysis -readOnly`,
script `.tools/ghidra_scripts/ExportTop.java`. Address lists are the inventory
(`re/exports/update-main/functions.tsv`) minus existing exports.

| Run | Wanted | ok | fail | missing | Seconds |
|---|---|---|---|---|---|
| Pilot (every 112th pending address) | 500 | 500 | 0 | 0 | 4 |
| Remaining | 55,527 | 55,524 | 3 | 0 | 232 |
| Retry of the 3 failures, 1,800 s timeout | 3 | 0 | 3 | 0 | 4 |

The remaining run logged 9 decompiler `pcode error` warnings; the affected
functions still produced C output. `ExportTop.java` now takes an optional
timeout argument and logs `EXPORT-TOP-FAIL <address> <error>`.

## Functions the decompiler rejects

| Address | Bytes | Instructions | Decompiler error |
|---|---|---|---|
| `000a713c` | 9,160 | 2,290 | `Recoverable Error: Unable to find unique hash for varnode` |
| `000a9b84` | 9,116 | 2,279 | same |
| `02ee6a54` | 6,192 | 1,548 | same |

Total 24,468 bytes (0.1286 % of update-main). The error is not a timeout:
it repeats at 1,800 s. Attempts that did not produce C, all in a read-only
project with in-memory changes rolled back:

- each of the 22 boolean `DecompileOptions` setters, flipped to `true` and to `false` (44 runs per function);
- `setEliminateUnreachable(false)`, `setRespectReadOnly(false)`;
- simplification styles `normalize`, `register` and `firstpass` (these styles emit no C by design);
- removing stored local variables (there are none: 0 locals, 0 hash-storage variables, default signature) and resetting the signature;
- project `PLA-update`: it has no `main.elf` program.

Fallback: `.tools/ghidra_scripts/ExportAsm.java` exported the full disassembly
(address, bytes, instruction) of each function to
`re/exports/update-main/asm_decompile_failed/` (outside the gameDB root).
Instruction count × 4 bytes equals each inventory size, so the bodies are
complete. The generator keeps these 3 as `no_matching_export_in_snapshot`.
Untried options: another decompiler, or debugging the Ghidra decompiler.

## gameDB

Two causes of the old gap, both verified:

1. The 2,284 unindexed exports were written after the last index run
   (`index.sqlite` mtime 2026-10-03 19:15). Reindexing fixed it.
2. The 252 files without a function row: the parser rejected Ghidra header
   shapes. Reproduced with synthetic files; each of the 3 fixes below has a
   test in `gamedb/tests/audit.rs` that fails on the original parser and passes
   with the fix.

| Header shape | Fix in `gamedb/src/parse.rs` |
|---|---|
| Return type on its own line plus wrapped parameters (`undefined4` / `FUN_x(a,` / `b)`) | `can_join` also accepts a column-0 name whose previous non-blank line is a column-0 bare type (`prev_line_is_bare_type`) |
| Array return type on its own line (`undefined1  [16]`) | `[` and `]` are allowed in that bare type |
| Parameter list over 9–14 lines | `MAX_HEADER_LINES` 8 → 32 |

Remaining unparsed file: `FUN_02e197b4` (one-line header with a
pointer-to-array return type `undefined1 (*) [12] FUN_x(...)`); not fixed.

Verification: `cargo test --release` in `gamedb` 29/29, `gamedb selftest`
87/87. Full reindex: `gamedb index -r decompiled --force`, 72,324 files,
0 failed, 1,047.81 s. The submodule now differs from upstream
`smileybaal/gamedb` commit `f75321b`; changes are uncommitted.

## Generator

`.tools/function_progress_treemap.py` expected counts updated: C exports
68,327 / 19,005,348 bytes; gameDB files 68,327, function rows 68,326,
unparsed 1. `python3 -m unittest test_function_progress_treemap` (in `.tools/`):
26 tests OK.

## Scope limit (added 2026-10-05, dec117)

"Every inventory function" means the 68,330 functions of the capped Ghidra
analysis. Their bodies cover 19,017,816 of the 53,106,320 executable bytes of
`main` (35.81 %); Suyu's static translation found 2,160,892 basic blocks
outside them. See
[`reports/suyu/update-v262144-feasibility.md`](../suyu/update-v262144-feasibility.md#static-aot-source-export-and-coverage-comparison-2026-10-05-dec117).
The percentages above are of the inventory, not of all code in `main`.
