# P0 — Message-table overlay structural comparison

**Date:** 2026-10-06. **Scope:** locally available base and effective update virtual RomFS trees; metadata-only parser comparison.

## Method and parser results

The local overlay audit and aggregate extraction metadata were consulted. The comparison enumerated local `.dat` and `.tbl` files and selected normalized `.dat`/`.tbl` stems with at least one changed or added member. No message keys or text, file names, raw payload bytes, per-file paths, or hashes were emitted or retained.

The existing `pla::assets::ahtb::Ahtb::parse`, `pla::assets::message::MessageFile::parse`, and `pla::assets::message::MessageStore::parse` APIs were called directly from an ignored scratch Rust helper referencing the `pla` crate. `MessageStore::parse(tbl, dat)` was run for each version of every pair with both members present in both versions. No tracked Rust source was changed.

| Aggregate | Result |
|---|---:|
| Changed `.dat`/`.tbl` files | 376 |
| Added `.dat`/`.tbl` files | 320 |
| Unchanged `.dat`/`.tbl` files in the local trees | 5,870 |
| Removed `.dat`/`.tbl` files | 0 |
| Affected normalized pair candidates | 356 |
| Pairs complete on both versions | 189 |
| Pairs complete only on base / only on update / complete on neither | 0 / 160 / 7 |
| Complete pair versions parsed (2 per pair) | 378 |
| `.dat` parser accepted / rejected | 378 / 0 |
| `.tbl` parser accepted / rejected | 368 / 10 |
| `MessageStore` accepted / rejected pair versions | 368 / 10 |
| Pair candidates with both versions accepted / one accepted / neither accepted | 179 / 10 / 0 |

The 10 rejected pair versions were rejected by the `.tbl` parser; all corresponding `.dat` files parsed. Pair completeness means both files are present on both sides, not that the parsers accepted them.

## Structural differences for parser-accepted pairs

Entry totals across the 179 fully accepted pairs were **35,715 base** and **39,505 update**, a delta of **+3,790**. All 35,715 shared entry indices were compared: **31,276 offsets**, **6,238 lengths**, and **838 flags** differed. These are positional structural comparisons only; the values do not identify message meaning.

File-size delta bins for all 189 complete pairs use byte ranges `negative`, `[0, 1 KiB)`, `[1, 4 KiB)`, `[4, 16 KiB)`, `[16, 64 KiB)`, `[64, 256 KiB)`, `[256 KiB, 1 MiB)`, and `>=1 MiB`:

| Member type | Negative | 0–<1 KiB | 1–<4 KiB | 4–<16 KiB | 16–<64 KiB | 64–<256 KiB | 256 KiB–<1 MiB | ≥1 MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `.dat` size delta | 8 | 121 | 29 | 22 | 9 | 0 | 0 | 0 |
| `.tbl` size delta | 0 | 149 | 30 | 10 | 0 | 0 | 0 | 0 |

Entry-length frequency distributions cover the 35,715 shared indices and use UTF-16 code-unit ranges `[1,2)`, `[2,4)`, `[4,8)`, `[8,16)`, `[16,32)`, `[32,64)`, `[64,128)`, `[128,256)`, and `[256,512)`; all other ranges had zero entries:

| Length range | Base | Update |
|---|---:|---:|
| `[1,2)` | 97 | 97 |
| `[2,4)` | 1,159 | 1,170 |
| `[4,8)` | 10,948 | 10,914 |
| `[8,16)` | 7,059 | 7,075 |
| `[16,32)` | 6,773 | 6,806 |
| `[32,64)` | 4,475 | 4,413 |
| `[64,128)` | 3,400 | 3,370 |
| `[128,256)` | 1,685 | 1,751 |
| `[256,512)` | 119 | 119 |

Offset deltas are relative to the parser's file base. Of the 31,276 changed offsets, the signed-delta distribution was: `[-4096,-256)`: 1,472; `[-256,-16)`: 629; `[-16,0)`: 1,099; `[0,16)`: 6,450; `[16,256)`: 15,728; `[256,4096)`: 5,884; `[4096,65536)`: 14. Other buckets, including deltas below `-4096` or at least `65536`, had zero entries.

Flag-value frequency differences across shared entry indices (base → update; omitted values occur zero times on both sides):

| Flag | Count | Flag | Count | Flag | Count |
|---:|---:|---:|---:|---:|---:|
| 0 | 31,689 → 31,689 | 1 | 471 → 461 | 2 | 9 → 9 |
| 4 | 1,702 → 1,713 | 5 | 84 → 84 | 8 | 108 → 108 |
| 9 | 42 → 41 | 32 | 3 → 3 | 64 | 43 → 43 |
| 65 | 35 → 35 | 68 | 6 → 6 | 72 | 6 → 6 |
| 73 | 1 → 1 | 128 | 11 → 11 | 129 | 12 → 12 |
| 132 | 2 → 2 | 256 | 23 → 23 | 258 | 4 → 4 |
| 512 | 1,099 → 1,099 | 513 | 295 → 295 | 516 | 38 → 38 |
| 517 | 9 → 9 | 520 | 6 → 6 | 525 | 1 → 1 |
| 576 | 3 → 3 | 580 | 2 → 2 | 641 | 2 → 2 |
| 768 | 9 → 9 |  |  |  |  |

## Verification and limits

Focused existing parser tests: `cargo test -p pla --test assets` — **21 passed, 0 failed**.

This establishes parser acceptance and metadata-level differences only for the listed locally available pairs. Parser success does not establish semantic ownership, runtime use, meaning of flags, or complete game-file coverage; rejected `.tbl` files remain structurally unparsed by these APIs. No binary, pseudocode, or source ownership claim is made.
