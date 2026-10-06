# P0 — AHTB overlay reinspection

**Date:** 2026-10-06. **Scope:** reconstruct the changed message-table cohort from the current base/update RomFS manifests and profile rejected `.tbl` inputs using the existing AHTB parser. Names, text, file paths, raw bytes, and hashes were not emitted or retained in report artifacts.

## Selection and parser results

The selector was rebuilt from `work/pla/pk1/romfs_files.tsv` and `work/pla/pk2/romfs_patched.tsv`, resolving members against their corresponding extracted RomFS trees. Normalized `.dat`/`.tbl` pair candidates were selected when either member was added, removed, or byte-different between the two trees. The complete-both-sides subset was parsed on both sides with the existing `pla::assets::ahtb::Ahtb::parse` through an ignored scratch Rust helper. Selection was not based on the previous sanitized report or a presumed list of ten selectors.

| Aggregate | Result |
|---|---:|
| Affected normalized `.dat`/`.tbl` pair candidates | 356 |
| Candidates complete on both sides | 189 |
| `.tbl` versions passed to `Ahtb::parse` | 378 |
| Accepted / rejected | 368 / 10 |
| Rejection class: zero or out-of-bounds name length | 10 |
| Rejections in other parser classes | 0 |

All ten rejected versions had the exact parser diagnostic **`bad name length 276 at entry 26`**. They were not header failures, entry-header truncations, or trailing-byte failures.

## Structural profiles

The following compares the rejected versions with the accepted versions in the same changed-pair cohort. Histograms are aggregated; no selector, name, string, path, payload, or hash is disclosed.

| Structural measure | Rejected (10) | Accepted controls (368) |
|---|---:|---:|
| Header magic | `AHTB`: 10 | `AHTB`: 368 |
| Declared count | 32: 10 | Ranges `[4,8)`: 10; `[8,16)`: 30; `[16,32)`: 20; `[32,64)`: 20; `[64,128)`: 90; `[128,256)`: 92; `[256,512)`: 82; `[512,1024)`: 20; `>=1024`: 4 |
| Input size | 926 bytes: 10 | `[64,256)`: 20; `[256,1024)`: 50; `[1024,4096)`: 90; `[4096,16384)`: 184; `[16384,65536)`: 24 |
| Fully walked records before outcome | 26: 10 | Declared count matched parsed count for all 368 |
| Entry-26 name-length range | 276 bytes: 10 | `[8,16)`: 56; `[16,32)`: 232; `[32,64)`: 40 |
| Entry-26 record-start offset modulo 4 | `3`: 10 | `0`: 142; `1`: 70; `2`: 14; `3`: 102 |
| Successfully walked name lengths before entry 26 | `[8,16)`: 12 each; `[16,32)`: 14 each | Accepted record lengths remain within each file's declared extent |
| Terminal byte of successfully walked names | NUL at record end: 26/26 each | NUL at record end: 75,878/75,878 |
| Extent at parser failure | Payload begins at offset 753; 173 bytes remain, but declared length is 276 (103-byte overrun) | Exact full-file consumption: 368/368 |

## Decision and next evidence

**No parser or test changes.** The rejected cohort converges on one parser failure, but the evidence does not establish a safe alternate record/name-length rule: the declared length at entry 26 exceeds the entire remaining file extent, while the 26 preceding records follow the accepted 10-byte entry-header / NUL-terminated-name walk. Changing the length interpretation or accepting a partial record would weaken the parser's bounds and exact-consumption guarantees without a directly evidenced format rule.

The next useful evidence is an independently documented or otherwise directly verified definition of the entry-26 field/record variant, including how the declared table count and remaining extent are meant to be interpreted. Until then the precise blocker is the repeated out-of-bounds name length at entry 26; no fixture should be derived from these proprietary inputs.

No tracked Rust source, tests, native function-progress evidence, or canonical documentation were changed. The focused assets test suite was not rerun because implementation and fixtures were intentionally left unchanged.
