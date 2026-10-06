# P0 update-main Ghidra body-range reconciliation

Date: 2026-10-06. Scope: update-v262144 `main.elf` in the existing `PLA-update-fix2` Ghidra project. The update is a patch component; this report makes no standalone-program or port-coverage claim.

## Source project and method

- Read-only source project: `/home/nico/work/decompilacion/work/pla/ghidra-projects/PLA-update-fix2.gpr`, project name `PLA-update-fix2`, program `/main.elf`.
- The project database (`PLA-update-fix2.rep`) contains 8 files / 709,739,522 bytes; deterministic relative-path, file-size, and per-file-SHA256 tree digest: `025aea555dba0fcdb24893d54717a6ed3f99951d30093ffb54e1c2aac476fddb`. The tree digest was unchanged by the read-only run.
- The candidate-mutated clone is separate: `work/p0-main-residual-ghidra-20261006/project/PLA-update-fix2.gpr`; its database tree has 8 files / 728,351,746 bytes and digest `75e8c7c80f76db714c8069cd34939f153379b7cfc30738378484383ba2327e14`. The earlier residual-export log confirms that this clone—not the untouched source—was used for the 14,549-creation attempt.
- Tool versions: Ghidra `12.1.2_DEV` (12.1.2) and OpenJDK `26.0.2.1`.
- Exactly **one** Ghidra headless process was launched, with `-readOnly -noanalysis`; the script iterated existing function bodies and wrote function entry IDs and every inclusive body address-set range only under `work/p0-main-ghidra-ranges-20261006/`. It did not create functions, run analysis, or export pseudocode. No gameDB index was run.
- Exact executable PT_LOAD used for range accounting: virtual addresses `[0x0, 0x32a5690)`, i.e. inclusive end `0x32a568f`, 53,106,320 bytes, independently confirmed by `readelf -W -l work/pla/pk2/main.elf`.

## Function-manager and inventory reconciliation

| Check | Result |
|---|---:|
| Untouched project FunctionManager functions | 153,476 |
| Function entry rows / unique entries in fix2 TSV | 153,476 / 153,476 |
| Ghidra entry IDs present in TSV | 153,476 |
| Ghidra IDs missing from TSV / TSV IDs missing from Ghidra | 0 / 0 |
| Actual non-empty body address ranges | 157,789 |
| Empty function bodies | 0 |

Thus the **untouched** fix2 project manager is an exact one-to-one match for the 153,476-row fix2 inventory. Its actual function-body address cardinality sum is 51,275,676 bytes.

The earlier clone run reported 14,549 creations and 169,450 final manager functions. Its implied pre-seed count is `169,450 - 14,549 = 154,901`, which is **1,425 above** the untouched project's verified 153,476. Adding 14,549 to the untouched manager instead yields 168,025, also 1,425 below the clone's final count. This reconciles the arithmetic: that run's manager was not the untouched fix2 baseline represented by the TSV. The clone's extra 1,425 pre-existing functions are not identified by the available snapshot/log evidence; no claim is made about their identities or bodies.

## Comparison of the 14,549 seed addresses with the untouched manager

The ignored candidate seed file contains 14,549 rows and 14,549 unique addresses. Compared directly with the untouched Ghidra entries and their exported actual body ranges:

| Seed comparison | Count |
|---|---:|
| Exact matches to existing function entry IDs | 0 |
| Seed addresses inside any existing function body | 652 |
| Seed addresses outside all existing function bodies | 13,897 |

These are address-membership facts only. They do **not** validate the 13,897 outside-body addresses as functions or authorize creating/exporting them. The 652 body-contained candidates further show why the clone's candidate-creation count cannot be interpreted as 14,549 additions to the untouched inventory.

## Actual body union, overlap, and gaps

The script exported all ranges from `Function.getBody()`; ranges are inclusive and retained only in the ignored output. Aggregation against the executable PT_LOAD gives:

| Measure | Result |
|---|---:|
| Sum of all function-body address cardinalities | 51,275,676 bytes |
| Body bytes within executable PT_LOAD | 51,275,676 bytes |
| Body bytes outside executable PT_LOAD | 0 bytes |
| Merged union ranges inside PT_LOAD | 23,598 |
| Unique body-byte union inside PT_LOAD | 51,275,676 bytes (96.55287%) |
| Overlap between function bodies inside PT_LOAD | 0 bytes |
| Uncovered PT_LOAD gaps | 23,597 ranges / 1,830,644 bytes (3.44713%) |

The exact body-range union therefore confirms that the 1,830,644-byte residual is genuinely outside every function body in this project, rather than an artifact of summing overlapping body sizes. This does not establish that uncovered bytes are code, reachable, or missing functions.

## Artifacts and constraints

Raw entries, all address-set ranges, script, and Ghidra log are ignored under `work/p0-main-ghidra-ranges-20261006/`. No pseudocode or function creations were produced. No ledger/status, README/PLAN, census, or other tracked file was changed; no commit or push was made. The only tracked artifact from this task is this report. Progress: `work/progress/p0-main-ghidra-ranges.log`.
