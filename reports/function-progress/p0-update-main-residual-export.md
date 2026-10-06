# P0 update-main residual export audit

Date: 2026-10-06. Scope: update-v262144 `main` NSO executable segment only.
Pinned input: `pk2.nsz`, SHA-256
`f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`.
Extracted `main.nso`: 31,882,976 bytes, SHA-256
`89fa2d715e13a1c311a47ee02b3cc858ccf72a2086ab9704a08a2ddc1b90f7d9`,
build ID `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e` (40-byte ID with zero
padding in the NSO header). The update is a patch; these measurements are not a
standalone-program or port-coverage claim.

## Finding: what the 3.45% represents

The residual is executable bytes not covered by any function in the current
fix2 Ghidra inventory. It is not a set of known functions whose C exports were
omitted, and it is not caused by the executable-byte denominator being short.

| Coverage dimension | Count | Executable bytes | Share of 53,106,320 bytes |
|---|---:|---:|---:|
| Functions in `re/exports/update-main-fix2/functions.tsv` | 153,476 | 51,275,676 | 96.55287% |
| C pseudocode files | 153,471 | 51,235,532 | 96.47728% |
| Assembly fallback functions | 5 | 40,144 | 0.07559% |
| Executable bytes outside every inventoried function | — | 1,830,644 | 3.44713% |
| Total | — | 53,106,320 | 100.00000% |

The five assembly functions are in the existing export report and are already
present as `.s` fallbacks, so they account for 0.07559%, not the 3.44713%
residual. Every one of the 153,476 inventoried entry points is represented by a
C file or assembly fallback. Thus, “100% of the inventory exported” and
“96.55% of executable bytes covered by inventoried functions” are different
claims.

The denominator is consistent with the pinned executable: the existing
independent Suyu comparison reports the `main.elf` executable LOAD segment as
0x32a5690 = 53,106,320 bytes = 13,276,580 32-bit words, equal to its translated
text-word count. This validates the denominator arithmetic; it does not prove
that every translated word is reachable code. Existing comparison also found
68,330/68,330 Ghidra inventory entries among Suyu block starts, while 2,160,892
Suyu blocks start outside all Ghidra function bodies. Treat the latter as a
candidate-discovery signal, not proof that each block is a missing function.

## Attempt to recover further exports

Tool versions observed in the run log: Ghidra 12.1.2 and OpenJDK
26.0.2.1. The sole process used the existing `PLA-update-fix2` analysis in an
isolated copy under ignored `work/p0-main-residual-ghidra-20261006/`; the source
project and existing exports were not opened for writing. Command shape:

```text
/opt/ghidra/support/analyzeHeadless work/p0-main-residual-ghidra-20261006/project PLA-update-fix2 \
  -process main.elf -noanalysis \
  -scriptPath /home/nico/work/decompilacion/.tools/ghidra_scripts \
  -postScript GapSeeds.java create work/p0-main-residual-ghidra-20261006/gap-seeds.tsv \
  -postScript ExportTop.java work/p0-main-residual-ghidra-20261006/export \
  work/p0-main-residual-ghidra-20261006/gap-seeds.tsv
```

`GapSeeds.java` reported 14,549 created candidates and wrote 14,549 tab-
delimited candidate rows. `ExportTop.java` expects one bare hexadecimal address
per line; it stopped at the first candidate with an invalid-address error.
Result: **0** additional C files. Its final function-manager count (169,450)
does not reconcile with the prior 153,476 inventory count plus its reported
14,549 creations (168,025); candidate additions therefore remain unvalidated
and are not added to inventory or coverage. The cloned project contains these
experimental changes only; they do not affect the original inventory.

No second Ghidra process was launched. A separate SDK-export Ghidra process was
observed afterward in the environment; it was not launched by this workstream.
No further export was completed or is justified from the existing inventory in
this task: its known functions are already exported, while the residual has no
validated function entry points. The exploratory seed set is potentially
exportable after the address-format and candidate-count discrepancies are
resolved, but that needs a separately bounded run; it cannot be treated as
additional coverage here.

## Reconciliation and parity checks

- Recomputed the source-inventory row count, unique-address count and function
  size sum directly from `re/exports/update-main-fix2/functions.tsv`:
  153,476 rows, 153,476 unique IDs, 51,275,676 bytes.
- Recounted the local export files: 153,471 `.c` files and 5 `.s` fallbacks.
- Matched export filename entry addresses against inventory IDs: 153,476 unique
  inventory IDs equal the 153,471 C IDs plus 5 assembly IDs, with zero overlap,
  zero missing IDs and zero extra IDs.
- Recomputed the NSO size/hash above and matched the pinned update-main identity
  recorded in the prior export and feasibility reports.
- Read the existing gameDB SQLite index in read-only mode (no `gamedb index`):
  153,471 files, 153,470 function rows/symbols, 61,082 strings and 49,098,547
  edges. One C file has no parsed function row; the five `.s` files are not
  indexed as source. The index count is not export-byte coverage.
- The existing eight-batch read-only parity record reports, per address range,
  zero missing/extra/duplicate paths and 25/25 non-empty sampled bodies per
  batch (200 samples). Its one irregularity is the unparsed C file. This audit
  did not rerun the index or those batches.
- The prior report records Ghidra 12.1.2, the fix2 inventory/export process,
  and its exact identities and totals: `reports/function-progress/update-main-full-pseudocode-export.md`.
- The denominator and Suyu block comparison are recorded at
  `reports/suyu/update-v262144-feasibility.md`, section “Static AOT Source
  export and coverage comparison”.

Index availability and analysis coverage remain separate from these export
counts. Export/index evidence does not establish function understanding,
implementation, behavior verification or binary matching. No function evidence
ledger was changed, and no `gamedb index` was run.

## README metric note

The current README and Spanish README both give the rounded 96.55% / 3.45%
executable-byte split; that arithmetic remains correct. Their completion-plan
row also distinguishes 100% of the inventoried functions from 96.55% of
executable bytes. However, the summary sentence describing the residual as
“needs emulator tracing” is not supported by this evidence: these bytes are
outside the Ghidra function inventory, and the export attempt did not establish
their code/function status. Correct the characterization in a later authorized
README-only task; README files were not edited here.

## Next bounded task

Normalize the candidate TSV to bare addresses without exposing its contents,
reconcile candidate rows and function-manager totals against the original
inventory and executable ranges, then run one isolated Ghidra export only for
the validated, genuinely new candidates. Keep any resulting C/disassembly
ignored under `work/`; update function inventory and byte coverage only after
direct duplicate-free reconciliation. Do not use this as analysis or behavior
evidence.

## Superseding addendum — 2026-10-06

The untouched fix2 Ghidra project has now been reconciled directly against the
TSV: all 153,476 function entries match exactly, the actual body ranges have
zero overlap, and their unique union is 51,275,676 bytes (96.55287%) of the
53,106,320-byte executable PT_LOAD. The residual is exactly 1,830,644 bytes
(3.44713%) outside all existing function bodies, not a proxy requiring further
body-range reconciliation. The direct reports
[`p0-update-main-ghidra-range-reconciliation.md`](p0-update-main-ghidra-range-reconciliation.md)
and [`p0-update-main-gap-classification.md`](p0-update-main-gap-classification.md)
supersede the earlier range limitation. Ghidra's listing state labels 40,764
gap bytes as instructions and 1,789,880 as data; semantic code/data identity,
valid new function boundaries, and reachability remain unknown.

The validated candidate addresses include 13,897 seeds outside existing bodies,
652 inside existing bodies, and no existing entry matches. No additional export
was performed; these remain candidates, not established functions. The next
bounded task is read-only call/jump-flow-reference triage of the 13,897 outside-
body seeds, without assuming or creating function boundaries. The separate
exploratory-clone manager discrepancy of +1,425 remains unexplained at identity
level; it is not a discrepancy in the untouched fix2 source project, whose
153,476 functions match the TSV exactly. No function-state or export claim is
promoted by the listing classification.
