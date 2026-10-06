# P0 Auxiliary Module Ghidra Range Reconciliation

Reconciliation date: 2026-10-06. Scope: the four auxiliary NSOs (`rtld`,
`sdk`, `subsdk0`, `subsdk1`) byte-identical in base v0 and update v262144.
This is static Ghidra metadata only. It does not establish function-inventory
completeness, semantics, runtime loading/reachability, behavior, or binary-match
evidence. No function-progress state was changed.

## Input identity and tools

The update package input is `pk2.nsz`, SHA-256
`f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`. The four
base/update NSO pairs were previously re-read, hash-checked, segment-hash
validated, and found byte-identical; these module hashes therefore identify the
same bytes in both versions.

| Module | NSO SHA-256 (base and update) | `.text` bytes |
|---|---|---:|
| `rtld` | `bc175ad9865adb48f9cbe9d7993ddb3367d3771bc06642c87d16ccf4f4dc9ff4` | 6,240 |
| `sdk` | `85aaf84172367499c02cbb5c317c419b9c768658f5a44de743da40d4c6441a4b` | 5,822,640 |
| `subsdk0` | `773153d0734c8feaacd5a1e6b21bd1ede8c060a142f9d91452897776f31d9827` | 3,445,104 |
| `subsdk1` | `c0a7a238688caf191c7c40badcbeac4d037a95ccaed644ce7fe2e63b5f898a13` | 6,298,960 |
| **Total** | — | **15,572,944** |

Ghidra 12.1.2 DEV (build date 2026-06-29); OpenJDK 26.0.2.1. Four sequential
headless invocations (peak concurrency **1**) opened the existing update-module
projects with `-noanalysis -readOnly`. No functions were created, no analysis
was run, and no project was written. `gamedb index` was not run.

## Method

A read-only Ghidra script enumerated every detected function's entry ID and
actual body address ranges, executable memory blocks, and CodeUnits across each
executable block. A local aggregation compared the extracted entry IDs with
the corresponding existing `functions.tsv` inventory and compared C-export
entry IDs with the same extracted function set. It merged inclusive body ranges
to calculate unique unions, weighted body overlap, in-`.text` coverage, and
out-of-`.text` ranges. It subtracted the body union from `.text` and intersected
the remaining intervals with the Ghidra listing's Instruction, defined Data,
and Undefined CodeUnit classes. Exact range and ID intermediates remained
temporary and are not included in this report.

In every program, the single executable block's byte length equaled that
module's `.text` denominator. Thus the measured block and reported `.text`
denominator coincide. Ghidra CodeUnit classes describe listing representation;
they do not prove that a span is semantically code or data.

## Results

| Module | Existing inventory entries | Read-only Ghidra detections | Entry-ID intersection | C exports in prior successful export run | Export IDs matching this project snapshot | All detected body bytes (sum) | Exact unique body union | Inter-function overlap | Union outside `.text` | `.text` gaps (bytes / spans) | Gap listing classes: Instruction / defined Data / Undefined (bytes) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `rtld` | 33 | 33 | 33 | 31 | 31 / 31 | 5,220 | 5,220 | 0 | 0 | 1,020 / 13 | 164 / 856 / 0 |
| `sdk` | 13,531 | 13,531 | 13,531 | 9,063 | 8,901 / 9,063 | 1,167,456 | 1,167,456 | 0 | 0 | 4,655,184 / 11,529 | 684,848 / 3,970,336 / 0 |
| `subsdk0` | 5,523 | 5,523 | 5,523 | 2,959 | 2,870 / 2,959 | 1,746,372 | 1,746,372 | 0 | 0 | 1,698,732 / 2,268 | 614,528 / 1,084,204 / 0 |
| `subsdk1` | 8,262 | 8,262 | 8,262 | 8,274 | 8,171 / 8,274 | 5,016,388 | 5,016,388 | 0 | 0 | 1,282,572 / 3,210 | 1,177,684 / 104,888 / 0 |
| **Total** | **27,349** | **27,349** | **27,349** | **20,327** | **19,973 / 20,327** | **7,935,436** | **7,935,436** | **0** | **0** | **7,637,508 / 17,020** | **2,477,224 / 5,160,284 / 0** |

The sums were checked against the per-module executable denominators: for each
module, unique function-body bytes plus gap bytes equal `.text` bytes, and the
two listing-class byte totals equal the gap byte count. No detected body range
extended outside `.text`; the sum of body bytes equaled the unique body union
in all four snapshots, so no inter-function body overlap was observed.

### Evidence categories and limitations

- **Static detections / inventory:** each extracted set of entry IDs exactly
  matched its existing metadata inventory (33/33, 13,531/13,531, 5,523/5,523,
  and 8,262/8,262). This is agreement with that stored snapshot, not proof of
  complete function enumeration.
- **C exports:** the previously recorded successful run exported 31, 9,063,
  2,959, and 8,274 C files. Matching their embedded entry IDs against these
  read-only program snapshots yielded 31, 8,901, 2,870, and 8,171 matches.
  The remaining 0, 162, 89, and 103 export IDs respectively were not present in
  the opened snapshots. Conversely, 0, 4,630, 2,653, and 91 snapshot entries
  had no corresponding ID among those export files. This is a snapshot/entry-ID
  reconciliation only; the prior inventory independently records function
  detection-count variance between read-only project runs.
- **Body bytes and unique coverage:** body-byte sums and exact unions are
  address-range measurements, not function counts or semantic coverage. The
  detected-body unions cover 5,220 / 6,240 `rtld` bytes, 1,167,456 / 5,822,640
  `sdk` bytes, 1,746,372 / 3,445,104 `subsdk0` bytes, and 5,016,388 / 6,298,960
  `subsdk1` bytes. The exact complementary gaps are shown above.
- **Listing classes:** all gap bytes were represented as Ghidra Instruction or
  defined Data CodeUnits; none were Undefined in these snapshots. These labels
  do not resolve whether any bytes are executable logic, embedded data, padding,
  or misclassified code.
- **Not conflated:** static detections, complete-function inventory, C-export
  counts, summed body bytes, unique body coverage, and `gamedb` indexing are
  separate measures. Complete-function inventory remains unknown; no gameDB
  index was built or consulted in this task.

## Next action

Retain the current findings as a precise range and listing census, not as proof
that every function is known. If completeness is needed, establish an
independent, documented function-boundary method for the unclaimed executable
spans and reconcile its evidence without modifying these projects or inferring
semantics/runtime behavior.

## Evidence references

- Input parity, module identity, and prior static detection/C-export counts:
  [`p0-auxiliary-module-export-inventory.md`](p0-auxiliary-module-export-inventory.md).
- Base package and auxiliary module provenance:
  [`p0-base-v0-module-provenance.md`](p0-base-v0-module-provenance.md).
- Prior update structural counts and original NSO metadata:
  [`p0-update-aux-ghidra-inventory.md`](p0-update-aux-ghidra-inventory.md).
