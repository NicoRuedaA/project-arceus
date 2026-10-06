# P0 — AHTB rejected-version reassessment

**Date:** 2026-10-06. **Scope:** assess whether the ten rejected local AHTB table versions have a directly evidenced, bounded alternate interpretation. Binary scope is limited to those ten versions and the already-established aggregate profiles. No identifiers, names, keys, message text, byte dumps, hashes, or payload are reproduced.

## Conclusion

**No safe alternate interpretation is evidenced. Keep the parser unchanged and reject all ten.** A common failure point and repeated field value do not establish what that field means. No observed field/extent relation explains the declared value and accounts for the remaining table contents across all ten while retaining strict bounds and complete-consumption checks.

This is an evidence insufficiency conclusion, not proof that no alternate format exists. Accepting a truncated name, treating the declaration as a different unit, changing the record width, or disregarding a suffix would be speculation and would weaken current safety guarantees.

## Evidence and constraints tested

- The source-level parser contract is: `AHTB`, a little-endian `u32` entry count, then that many records of an eight-byte ID, a little-endian `u16` NUL-inclusive name length, and the declared name bytes. It checks the entry header and declared name extent against the input and rejects unconsumed trailing bytes.
- The established aggregate profile for the ten rejected versions is uniform: each declares 32 records; each parses the first 26 using the accepted record walk; and each fails at entry 26 with declared name length 276. At the point the name would begin, 173 input bytes remain, so the declaration exceeds the available extent by 103 bytes.
- The 26 earlier records follow the accepted NUL-terminated-name walk. This supports applying the existing interpretation to those records, but it does not define the entry-26 field or any alternative layout.
- The count of 32 does not repair the contradiction: six declared entries remain after entry 26, whose declared name alone cannot fit in the remaining input. No bounded relationship is established between 276, the remaining extent, the later records, or a possible suffix.
- The existing aggregate profile reports the same failure shape in all ten, but supplies no direct field semantics, alternate record boundaries, or consistent extent equation. Similarity of failures is not evidence for reinterpretation.

## Safe-format decision

Retain the current bounds checks, count-delimited walk, NUL-inclusive length handling, and exact end-of-input requirement. Do not add a lenient fallback or make any parser/test changes on this evidence. The accepted controls in the established cohort consume their declared records exactly; they do not establish an exception for these ten.

## Minimal external evidence needed

Obtain an authoritative format/specification statement or independently verified producer/consumer evidence defining the entry-26 field and record form, including:

1. the field's unit and scope (for example, whether it denotes bytes, a count, or another extent);
2. how the 32-entry declaration maps to record boundaries and any trailer/suffix; and
3. a formula or explicit boundaries that account for all remaining bytes for each of the ten versions, with checked arithmetic and no read beyond the input.

The evidence must validate the same interpretation across all ten and preserve strict bounds and full-input accounting. Until then the variants remain unsupported. No filenames or per-file selectors are needed or included in this conclusion.

No parser/source, tests, function-progress evidence, README, or canonical documentation were changed. No Ghidra, gameDB, credentials, or key access was used.
