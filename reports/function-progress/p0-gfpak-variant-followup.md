# P0 — GFLXPACK parser rejection follow-up

**Date:** 2026-10-06. **Scope:** six modified, locally readable GFLXPACK files rejected by the bounded parser; base/update comparison was limited to those six pairs. No names, payload data, strings, or raw hashes are recorded.

## Findings

The six update files all reach the parser's final table-selection failure: no scanned table candidate has a first entry whose `data_offset` equals `table_offset + entry_count * 24`. There were no signature/minimum-length failures, and all six had scanned offsets. The corresponding base files also fail the same invariant.

Further metadata-only inspection found exactly one candidate per file whose first entry instead points to `table_offset + entry_count * 24 + 8`. The candidate table and entry-count interpretation is internally plausible for all six pairs: entry extents are within the file for all six base files and four of the six update files; entry offsets are nondecreasing in all twelve files. The two remaining update files have at least one entry extent outside the file under the parser's current 24-byte entry interpretation. Payloads were not read or emitted.

The six candidates are not otherwise one uniform header shape. Their header `kind` values group as one value occurring once, another occurring four times, and a third occurring once; the number of scanned offsets groups as 4 once, 6 four times, and 1,860 once. The last case makes the current heuristic especially ambiguous: the parser treats every leading in-range u64 as a table offset, but the meaning and boundary of that large offset sequence are not established. The two out-of-bounds update cases independently prevent accepting the `+8` rule as a safe parser variant for the full rejected set.

## Decision

No Rust parser change or synthetic parser test was added. The common `+8` first-entry relationship is useful evidence, but it is not enough to establish that these are six instances of one bounded format variant: the header shapes differ, the offset-list boundary is uncertain in one case, and two update entry tables fail payload-extent bounds. Adding a relaxed equality check alone would accept files without proving their entry records or payload extents are valid.

## Minimum evidence needed to resume

1. Direct structural evidence defining the supported header `kind` values and the exact count/boundary rule for the leading offset array, especially for the file with 1,860 scanned offsets.
2. A validated explanation of the two update entries whose `data_offset + size` exceeds file length under the current record interpretation. This must establish whether the fields mean something different, whether another table interpretation is correct, or whether those entries are invalid; do not infer a fix from the base counterpart alone.
3. Confirmation from a local, independently validated sample or authoritative format reference that the eight-byte region between the entry-table end and first payload is a structural part of this variant, rather than a coincidental field match.

Until those points are established, the six files remain rejected and the existing bounded parser acceptance rules remain unchanged. The function-progress evidence ledger was not edited: parser metadata is not native-function evidence.
