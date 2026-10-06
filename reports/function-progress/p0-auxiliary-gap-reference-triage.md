# P0 Auxiliary Gap Reference Triage

Date: 2026-10-06. Scope: read-only references and current listing units in the
existing `rtld`, `sdk`, `subsdk0`, and `subsdk1` Ghidra snapshots. The prior
[range reconciliation](p0-auxiliary-range-reconciliation.md) is the baseline for
existing function bodies and their complementary executable gaps. This report
contains aggregates only; it does not expose function identifiers, addresses,
binary contents, or symbols.

## Identity and read-only controls

- The four opened module programs and their input ELFs matched the exact module
  identities and base/update parity recorded in the
  [export inventory](p0-auxiliary-module-export-inventory.md) and
  [base provenance](p0-base-v0-module-provenance.md). No alternate or exploratory
  project was used.
- Ghidra 12.1.2 DEV opened the existing snapshots with `-readOnly -noanalysis`.
  A metadata-only script enumerated current function bodies, gap CodeUnits, and
  existing references; it did not create functions, disassemble, analyze, or
  export pseudocode. No gameDB index was run.
- Four module scans completed sequentially (peak concurrency: **1**). Two
  preliminary script-compilation attempts failed before producing metrics. The
  corrected query was repeated while tightening the Data-vs-Undefined split;
  four final per-module scans produced the table below. Fifteen Ghidra process
  launches total, including the two failed compilation attempts.
- Recursive SHA-256 fingerprints of each existing Ghidra source-project tree
  were identical before and after the scans. The corresponding source-project
  fingerprints were:

  | Module | Before/after project-tree SHA-256 | Stable |
  |---|---|---|
  | `rtld` | `5b1f0d2e62d046d0dcbbbd32b38e8dda44b8fc093f76a7f2b1071400b4e8ed8c` | Yes |
  | `sdk` | `f3e8974c19ccec4612e416b3dcfe85fa994069c3d8ae7762ffb55e57ad5aa4b1` | Yes |
  | `subsdk0` | `1b7dbcefea83b78ab804eff6c786225a12e801a9f89b9e5796351554cfc96207` | Yes |
  | `subsdk1` | `0769bbfbc9b528b602778ed9a3bb14ae0ad6aeaa17a3872a5f3aabd4c2820935` | Yes |

## Current gap CodeUnits

The following are counts of CodeUnits intersecting the already-reconciled gaps.
The prior span counts are included to tie results to
the established baseline. For Data CodeUnits, this pass separately checked
`Data.isDefined()`; an Undefined-type Data unit is therefore reported separately
from defined Data.

| Module | Existing detected functions¹ | Baseline gap spans | Instruction CodeUnits | Defined Data CodeUnits | Undefined-type Data CodeUnits | Other CodeUnits |
|---|---:|---:|---:|---:|---:|---:|
| `rtld` | 33 | 13 | 41 | 0 | 856 | 0 |
| `sdk` | 13,531 | 11,529 | 171,212 | 5 | 3,970,300 | 0 |
| `subsdk0` | 5,523 | 2,268 | 153,632 | 9 | 1,084,159 | 0 |
| `subsdk1` | 8,262 | 3,210 | 294,421 | 10 | 104,832 | 0 |

¹ Existing snapshot detections, not a complete or valid-function denominator.

## Reference edges by source/target location

Counts below are existing reference edges. Sources are categorized as inside an
existing function body, inside an executable gap, or other initialized program
memory; targets use the same categories. `Outside` means outside the executable
region. The outside/outside cell is not included because neither endpoint is in
a body or gap. No references were categorized in an executable-region remainder
outside the body/gap partition.

| Module | Body → body | Body → gap | Body → outside | Gap → body | Gap → gap | Gap → outside | Other → body | Other → gap | Other → outside |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `rtld` | 284 | 0 | 170 | 4 | 2 | 1 | 4 | 2 | 0 |
| `sdk` | 47,659 | 2,563 | 47,219 | 15,527 | 21,713 | 7,001 | 10,752 | 8,205 | 0 |
| `subsdk0` | 86,729 | 16,691 | 56,165 | 9,433 | 27,911 | 4,304 | 4,083 | 3,382 | 0 |
| `subsdk1` | 233,976 | 2,953 | 123,469 | 29,116 | 18,827 | 6,218 | 2,562 | 12,574 | 0 |
| **Total** | **368,648** | **22,207** | **227,023** | **54,080** | **68,453** | **17,524** | **17,401** | **24,163** | **0** |

The table counts reference edges, not distinct destinations. It includes all
existing reference kinds, not only call or jump-like edges.

## Direct CALL/JUMP-like references into gaps

These are references marked direct by the stored Ghidra reference type, targeting
an address in a gap. Unique destination counts are deduplicated within each
module; edge counts are shown independently by source location and reference
kind. The per-source unique-target columns are not additive because the same
destination can be targeted from more than one source category.

| Module | Unique gap targets, all sources | Unique targets from bodies | Unique targets from gaps | Unique targets from other sources | Body CALL/JUMP edges | Gap CALL/JUMP edges | Other-source CALL/JUMP edges |
|---|---:|---:|---:|---:|---:|---:|---:|
| `rtld` | 2 | 0 | 2 | 0 | 0 / 0 | 0 / 2 | 0 / 0 |
| `sdk` | 11,868 | 0 | 11,868 | 0 | 0 / 0 | 0 / 17,928 | 0 / 0 |
| `subsdk0` | 12,546 | 1 | 12,545 | 0 | 0 / 1 | 0 / 20,302 | 0 / 0 |
| `subsdk1` | 9,163 | 12 | 9,151 | 0 | 0 / 18 | 0 / 15,349 | 0 / 0 |

Edge pairs are ordered **CALL / JUMP**. Thus the observed candidate edges into
gaps are jump-like in these snapshots; this is a structural reference
classification only. Candidate target counts and edge counts do not establish
that a target is a function, that a transfer executes, or that the target is
reachable in the game.

## Interpretation and remaining denominators

- Existing detected-body sets and their gap complements reproduce the
  [range-reconciliation baseline](p0-auxiliary-range-reconciliation.md). The
  finer `Data.isDefined()` split in this pass **does not reproduce** that
  report's claim that all gap Data was defined and no Undefined units existed:
  most gap Data CodeUnits are Undefined-type units in the opened snapshots.
  This is a listing-classification-method discrepancy, not evidence that the
  body/gap boundaries changed; the earlier report and this direct typed split
  remain separately cited rather than silently conflated. A Data unit may
  contain code-like material, and an Instruction unit may be padding or
  otherwise non-semantic.
- Existing reference edges and direct CALL/JUMP-like destinations provide
  candidate cross-links, not independently validated function boundaries,
  callability, runtime loading, reachability, behavior, or ownership.
- Function-inventory completeness and the denominator of valid functions remain
  **Unknown** for all four modules. Prior detection counts, reference counts,
  CodeUnit counts, and the number of direct candidate targets must not be
  substituted for that denominator.
- Semantic classification of gaps, validation of candidate boundaries,
  provider binding/runtime reachability, and semantic coverage remain open.
  No function-progress state was advanced.

## Evidence references

- [Auxiliary range reconciliation](p0-auxiliary-range-reconciliation.md) —
  body unions, gap spans, historical listing-class check, and tool baseline.
- [Auxiliary module inventory/export](p0-auxiliary-module-export-inventory.md) —
  exact module identities, base/update parity, and detection/export limits.
- [Base module provenance](p0-base-v0-module-provenance.md) — package chain and
  version-qualified module identity.
