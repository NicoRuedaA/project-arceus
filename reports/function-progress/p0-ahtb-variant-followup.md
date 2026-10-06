# P0 — AHTB rejected message-table follow-up

**Date:** 2026-10-06. **Scope:** determine whether the existing AHTB parser can safely accept the 10 locally available rejected message-table versions.

## Evidence reviewed

The prior overlay audit records 378 complete pair versions, with 368 `.tbl` versions accepted and 10 rejected by `Ahtb::parse`; all 378 corresponding `.dat` files parsed. It records only the aggregate accept/reject totals. It does not preserve per-file parser errors, rejected-file identifiers, or header/entry-layout profiles for the rejected set.

The current parser accepts only the exact `AHTB | u32 count | count × (u64 id, u16 NUL-inclusive name length, name bytes)` layout and requires the count-delimited records to consume the entire input. Its failure classes are a bad/short header, truncation before an entry header, zero or out-of-bounds name length, and trailing bytes. The existing aggregate evidence does not say how the 10 failures distribute among those classes, and does not identify which ten local files comprise that set.

## Decision

**No parser or test change.** With no retained selector for the ten files and no aggregate rejection-class/structural metadata, inspecting more files to rediscover the set would exceed the explicit limit of inspecting only those ten. Nor does the current evidence support a common alternate header, record width, terminator rule, or bounded trailer. Relaxing any of these checks would therefore be speculative and could admit malformed unrelated tables.

## Exact unresolved structural rule / prerequisite

Provide a sanitized source-local rejection manifest for exactly the ten known rejected table versions. It must identify them without exposing names or paths and report, in aggregate, each exact parser rejection class plus bounded structural facts sufficient to distinguish header/count/record/name-terminator/trailing-data variants. With that selector, the ten files can be inspected in memory only; a parser variant should be added only if their structures converge on one directly evidenced rule and the existing bounds and full-consumption validation can remain strict.

No real table bytes, names, keys, strings, paths, or hashes are included here. No code, tests, evidence-ledger states, or canonical documentation were changed.
