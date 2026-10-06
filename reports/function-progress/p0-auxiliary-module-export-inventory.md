# P0 Auxiliary Module Function Detection and Pseudocode Export

Generated: 2026-10-06. Scope: the four auxiliary NSOs in base v0 and update
v262144 (`rtld`, `sdk`, `subsdk0`, `subsdk1`). This report records executable
identity, static Ghidra detection/export metadata, and limitations only. It
contains no game bytes, pseudocode, strings, or symbol names and advances no
function-progress evidence state.

## Exact build identities and parity

The base inputs are the canonical files under `work/pla/pk1/exefs/`; the update
inputs are the selectively extracted members under
`work/pla/p0-update-aux-20261005-verified/`. The update archive is `pk2.nsz`,
SHA-256 `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`.
Its pinned section 22 SHA-256 is
`c5e6c573856bb085ffffe36cdbecc9ab18fef0a2e3aeecf7f724ee574217521a`. Base
package provenance is recorded as `pk1.nsz`, SHA-256
`00167d5e00bf7f5fca5311f984069497f79812cbbd03c32eb7f2fc1649af2acc`.

Each pair was re-read in this run, size/hash checked, compared byte-for-byte,
and parsed with `read_nso_image`; all three embedded segment hashes validated.
The module IDs below are the NSO header build IDs. The executable-byte
denominator is the decompressed `.text` segment size (not the compressed NSO
file size, total mapped memory, or `.text` plus data).

| Module | Base NSO size | Update NSO size | SHA-256 (both) | Module ID (both) | Executable `.text` bytes | Base/update parity |
|---|---:|---:|---|---|---:|---|
| `rtld` | 7,187 | 7,187 | `bc175ad9865adb48f9cbe9d7993ddb3367d3771bc06642c87d16ccf4f4dc9ff4` | `6f8b1226cc8cd958021d6bfd226ebf23f892ab9b000000000000000000000000` | 6,240 | byte-identical |
| `sdk` | 5,754,872 | 5,754,872 | `85aaf84172367499c02cbb5c317c419b9c768658f5a44de743da40d4c6441a4b` | `b39add63421be525d352e576dab438d3812efdf0000000000000000000000000` | 5,822,640 | byte-identical |
| `subsdk0` | 3,409,290 | 3,409,290 | `773153d0734c8feaacd5a1e6b21bd1ede8c060a142f9d91452897776f31d9827` | `535249a546887cac875a099e9c8ef367077cf58b000000000000000000000000` | 3,445,104 | byte-identical |
| `subsdk1` | 4,920,073 | 4,920,073 | `c0a7a238688caf191c7c40badcbeac4d037a95ccaed644ce7fe2e63b5f898a13` | `4dd097b435891c5e0e15a65dbe47124f4d7fd1c1000000000000000000000000` | 6,298,960 | byte-identical |
| **Total** | **14,091,422** | **14,091,422** | — | — | **15,572,944** | **4/4 pairs** |

Because each base/update pair has the same verified bytes, one deterministic
static analysis/export per module role is reusable for both versions. This
establishes byte-level identity and reuse of static results for these four
modules only. It does **not** establish that a module is loaded, bound to a
provider, reached, or executed in either version; nor does it establish
semantic ownership, behavior, or coverage of the update `main`/effective game.

## Ghidra detections and exported pseudocode

Ghidra 12.1.2 (`12.1.2_DEV`) with OpenJDK 26.0.2.1 was invoked headlessly,
one process at a time, against the existing module projects opened with
`-readOnly`. The custom `ExportAuxC.java` script exported address-keyed C files
to fresh ignored locations under `work/pla/p0-aux-export-20261006/`; project
changes were discarded. The `subsdk1` first attempt hit the 120-second tool
timeout after writing a partial export; those files were left untouched and a
successful rerun wrote to a separate fresh directory. The counts below are
from the successful runs.

| Module | Ghidra function candidates | Non-thunk/non-external attempted | C exports | Decompile failures | Exported function-body bytes | Export bytes / `.text` bytes |
|---|---:|---:|---:|---:|---:|---:|
| `rtld` | 33 | 31 | 31 | 0 | 5,200 | 83.33% |
| `sdk` | 13,718 | 9,063 | 9,063 | 0 | 1,112,700 | 19.11% |
| `subsdk0` | 5,633 | 2,959 | 2,959 | 0 | 1,723,376 | 50.02% |
| `subsdk1` | 8,366 | 8,274 | 8,274 | 0 | 5,061,464 | 80.35% |
| **Total** | **27,750** | **20,327** | **20,327** | **0** | **7,902,740** | **50.75%** |

Exported-body bytes are the sum of successful functions' Ghidra body sizes;
coverage is that sum divided by decompressed executable `.text` bytes. This is
a measured export ratio, not a claim that the remaining bytes are all
uncovered code: function bodies may include overlaps or non-code bytes, and
code may be unrecognized. Thunks and external functions are excluded from C
export attempts (7,423 combined). All attempted functions completed decompilation
in the successful runs.

These rows distinguish **Ghidra detections** from a reconciled complete
function inventory. Completeness remains **unknown**: this work did not
reconcile every executable byte range against independently established
function boundaries, nor classify every code/data/unresolved span. The
candidate and export counts are therefore not a total-function denominator,
semantic coverage, or recovered/unrecovered inventory.

### Detection-count variance

The prior update metadata report records detected counts of 33 (`rtld`), 13,531
(`sdk`), 5,523 (`subsdk0`), and 8,262 (`subsdk1`). This run's read-only project
opens performed in-memory auto-analysis before export and reported 33, 13,718,
5,633, and 8,366, respectively. The three non-`rtld` counts differ; no cause
was reconciled in this task. Treat the counts in the export table as the
successful run's direct results, preserve the earlier values as historical
results, and do not claim they are interchangeable or a complete inventory.

The SDK run also logged an invalid-PNG analyzer diagnostic in data; its
headless analysis and pseudocode export completed successfully. The diagnostic
did not produce a Ghidra decompilation failure.

## Tool and command record

- Input validation: `uv run --with lz4 --no-project python3` loaded
  `.tools/nso_to_elf.py:read_nso_image`; it verified NSO magic, decompressed
  segments, embedded hashes, module IDs, and byte equality of each base/update
  pair. No converted ELF was created by this validation step.
- Existing Ghidra program ELFs were rechecked by SHA-256 against the recorded
  deterministic conversions of the verified NSOs. Ghidra 12.1.2_DEV on Java
  26.0.2.1 ran `analyzeHeadless <existing-project-dir> <project-name>
  -process module.elf -readOnly -scriptPath <fresh-work-script-dir>
  -postScript ExportAuxC.java <fresh-module-export-dir>` once for each module,
  plus one separate successful retry for `subsdk1` after the tool timeout.
- Ghidra invocations: **5 sequential launches, peak concurrency 1**. No
  existing Ghidra project was intentionally modified or overwritten.
- The previous metadata inventory records ELF language
  `AARCH64:LE:64:v8A` with default compiler spec. Synthetic ELF relocation
  counts are not used here as original-NSO import evidence.
- `gamedb index`: **not run**, as required.
- Index status: no index was created or updated by this task; export artifacts
  are unindexed.

## Blockers, unresolved scope, and next action

- Complete/reconciled function counts and code/data/unresolved executable-byte
  classification remain unknown. Ghidra's candidate counts are detections,
  not proof of complete enumeration.
- Static export does not resolve provider identity, loader binding, runtime
  loading/reachability/use, ownership, or semantics.
- Historical and current Ghidra detection counts differ for `sdk`, `subsdk0`,
  and `subsdk1`; the run/configuration delta was not investigated further.
- Next bounded action: reconcile per-module executable-byte ranges and
  candidate function bodies against a documented complete-inventory method;
  retain unknown for unreconciled spans. Keep runtime/provider resolution and
  semantic analysis separate.

## Evidence paths

- Exact identities and prior verified base/update parity:
  [`p0-base-v0-module-provenance.md`](p0-base-v0-module-provenance.md#version-qualified-base-module-identity)
  and [`p0-update-aux-ghidra-inventory.md`](p0-update-aux-ghidra-inventory.md#update-candidate-inventory).
- Prior detected counts and original update NSO metadata:
  [`p0-update-aux-ghidra-inventory.md`](p0-update-aux-ghidra-inventory.md#update-candidate-inventory).
- This run's private metadata-only progress log:
  `work/progress/p0-auxiliary-export.log`.
- This run's ignored pseudocode, Ghidra logs, and helper:
  `work/pla/p0-aux-export-20261006/`.
- NSO validation implementation: [`.tools/nso_to_elf.py`](../../.tools/nso_to_elf.py).
