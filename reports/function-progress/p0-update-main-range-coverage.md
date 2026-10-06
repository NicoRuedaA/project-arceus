# P0 update-main range-coverage reconciliation

**Date:** 2026-10-06  
**Scope:** Pokémon Legends: Arceus update v262144, update `main` executable segment only. The update is a patch; this is not a standalone-program or whole-game coverage claim.

## Finding

The quoted **3.447130% is the difference between the sum of exported function body sizes and executable `.text` size**. It is not an exact unique-byte coverage result. The TSV has each function's entry address and body-address count, but no actual body address-set ranges. Its `size` field is `Function.getBody().getNumAddresses()` (per `.tools/ghidra_scripts/ExportTsv.java`), a cardinality that does not establish the contiguous interval `[entry, entry + size)`. Therefore exact union, overlap, and uncovered bytes of the real function bodies cannot be recovered from this TSV alone under the no-Ghidra constraint.

For comparison only, treating every row as a contiguous half-open interval `[id, id + size)` produces a proxy union of **50,875,760 bytes**, proxy gaps of **2,230,560 bytes (4.200178%)**, and proxy overlap excess of **399,916 bytes**. These are reproducible interval-arithmetic results, not verified real function-body coverage. The allowed evidence cannot establish whether the actual uncovered unique bytes equal these proxy gaps.

## Input and executable-range identity

- Update archive: local `pk2.nsz`, 52,657,467 bytes, SHA-256 `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446` (update v262144).
- Verified local `main.nso`: 31,882,976 bytes, SHA-256 `89fa2d715e13a1c311a47ee02b3cc858ccf72a2086ab9704a08a2ddc1b90f7d9`; module/build identifier `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e` (NSO module ID has zero padding after these 20 bytes).
- The NSO header declares text virtual start `0x0`, size `0x32a5690` (53,106,320 bytes), with the embedded text-segment SHA-256. The local generated `main.elf` is 69,791,792 bytes, SHA-256 `b772390207e0de689ca96adaeb15df8b1499912efa74804c3766214d76d08cf2`; its executable `PT_LOAD` has flags `5`, virtual range `[0x0, 0x32a5690)`, file size and memory size both 53,106,320. Hashing those ELF segment bytes matches the NSO embedded text-segment digest (`f68daca4836be616b98bbad40c84362eee7624a45f45bb39ae23d667952d190d`). This independently reconciles the executable denominator and range without decompressing or exporting code.
- Function metadata: `re/exports/update-main-fix2/functions.tsv`, 6,406,436 bytes, SHA-256 `829d810c52a7662ec4d3d958866a091a7b926d9fccd0eea1427181669abd2535`.

## Reconciliation

Counts are direct TSV parses. Function IDs are interpreted as hexadecimal entry addresses, `size` as a decimal body-address count, and all intervals in the proxy calculation are half-open and clipped to the executable range `[0x0, 0x32a5690)`.

| Measure | Result |
|---|---:|
| Data rows / parsed function entries | 153,476 / 153,476 |
| Unique entry addresses | 153,476 |
| Duplicate entry-address groups / extra rows | 0 / 0 |
| Malformed rows / non-identified statuses / blank required metadata | 0 / 0 / 0 |
| Status values | `identified`: 153,476 |
| Zero-size rows | 0 |
| Entirely outside / partially outside `.text` proxy intervals | 0 / 0 |
| Sum of reported body sizes | 51,275,676 bytes |
| `.text` denominator | 53,106,320 bytes |
| Denominator minus size sum | 1,830,644 bytes = 3.447130% |
| Proxy merged interval union | 50,875,760 bytes across 21,561 merged intervals |
| Proxy overlap: address footprint at depth ≥2 | 359,832 bytes across 1,209 overlap regions |
| Proxy overlap: excess multiplicity (`sum - union`) | 399,916 bytes |
| Proxy uncovered complement | 2,230,560 bytes across 21,560 gaps = 4.200178% |

Arithmetic: `53,106,320 - 51,275,676 = 1,830,644`; `51,275,676 - 50,875,760 = 399,916`; and `1,830,644 + 399,916 = 2,230,560 = 53,106,320 - 50,875,760`. The largest proxy gap is `[0x25e46c0, 0x2600a7c)`, 115,644 bytes.

## Method and limitations

Read the TSV as UTF-8, skipped its comment manifest, stripped the header type annotations, validated required fields and numeric address/size fields, counted rows and duplicate IDs, and summed sizes. For the explicitly labeled proxy only, formed `[entry, entry + reported_size)`, intersected each range with the exact executable virtual range, sorted endpoints, merged overlapping intervals, and subtracted the union from `.text`. NSO and ELF identity/range data were checked directly from their local headers and ELF program headers; the ELF segment contents' digest matched the NSO embedded text digest.

No Ghidra process or gameDB index was used. The local Python environment lacks the `lz4` module required by the repository NSO decompressor, so the NSO compressed segment was not independently decompressed in this task; the locally verified NSO identity/header and exact ELF executable segment were reconciled by the embedded text hash. No function-body ranges were fabricated from the size counts. No statement is made about whether any uncovered or proxy-gap bytes are code, data, reachable, or semantically relevant. Exported/identified rows and byte arithmetic are not analysis, implementation, behavior-verification, or binary-match evidence.

## Next action

To answer the exact unique-body-coverage question, obtain a metadata-only export of each function body's actual address-set ranges (or an equivalent already-produced range ledger), then merge those ranges against `[0x0, 0x32a5690)`. Keep that bounded extraction separate from this audit; do not infer code/data/reachability from resulting gaps.

## Superseding addendum — 2026-10-06: exact body ranges reconciled

The limitation above applied to the size-only TSV proxy and is superseded by the
direct source-project range reconciliation in
[`p0-update-main-ghidra-range-reconciliation.md`](p0-update-main-ghidra-range-reconciliation.md).
All 153,476 untouched fix2 FunctionManager entries match the TSV exactly. Actual
body ranges have zero overlap; the exact union covers 51,275,676 bytes (96.55287%)
of the 53,106,320-byte executable PT_LOAD. The exact complement is 23,597 gaps,
1,830,644 bytes (3.44713%) outside all existing function bodies. The listing
classifies these bytes as 40,764 instruction bytes and 1,789,880 data bytes
([`p0-update-main-gap-classification.md`](p0-update-main-gap-classification.md));
those are listing labels only, not semantic code/data, valid-function-boundary,
or reachability evidence. The old proxy arithmetic above is retained as history,
not current coverage. The complete valid-function denominator remains unknown.
