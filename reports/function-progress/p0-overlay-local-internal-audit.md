# P0 — Local overlay internal-structure audit

**Date:** 2026-10-06. **Scope:** local, authorized base-v0 + update-v262144 artifacts. Metadata only; no external colleague input.

## Availability and method

Both package files, the base and update virtual RomFS trees, the base ExeFS set, the update `main`, four update auxiliary modules, both `main.npdm` files, and the already-decoded update ExeFS section were present and passed bounded read checks. The inventories reconciled to **18,370/18,370** readable base files and **19,095/19,095** readable update virtual files; every file also matched its inventory size. No missing or unreadable virtual files were found.

The audit used the local manifests and extracted trees. It re-derived the **466** modified common files (465 size changes plus one same-size hash difference), then probed headers without emitting them. For supported containers, a local metadata-only parser compared member/record counts, bounded extents, and in-memory digests. Names and payload bytes were not emitted or retained. Additions were SHA-256 matched against all readable base payloads; only aggregate match counts are recorded.

## Modified entries

| Outer format | Modified | Header-readable | Internal comparison completed | Result / limitation |
|---|---:|---:|---:|---|
| SARC (`.arc`) | 10 | 10 | 10 | All ten parsed on both sides. No member additions/removals; 51 existing member payload digests differed in aggregate. |
| GFLXPACK (`.gfpak`) | 10 | 10 | 4 | Four parsed on both sides and had no record-count, record-metadata, or member-payload digest changes. Six were not accepted by the bounded parser. |
| Lua 5.3 bytecode (`.blua`) | 17 | 17 | 0 | Header probe recognized all 17; no internal record diff attempted. |
| `.bin` (format unresolved) | 53 | 53 | 0 | Header probe did not identify a supported structure. |
| `.dat` (catalogued outer label) | 189 | 189 | 0 | Header probe did not identify a supported structure. |
| `.tbl` (catalogued outer label) | 187 | 187 | 0 | Header probe did not identify a supported structure. |
| **Total** | **466** | **466** | **14** | **452** lack a successful internal comparison; 0 are unreadable. |

Thus structural work advanced beyond the previous outer-only baseline for **14/466** modified entries. For SARC, the member-set comparison found no additions or removals; changed payload digests are evidence of changed members, not decoded semantics. The six GFLXPACK failures are parser/format limitations, not evidence of unreadable files or encryption. The other **446** entries were readable but have no validated local inner-format parser in this pass. Header probing is not itself a record-level analysis.

## Added-entry provenance by local payload matching

| Check | Count |
|---|---:|
| Added payloads with an exact SHA-256 match somewhere in the base RomFS | **33** |
| Added payloads with no exact match in the base RomFS | **692** |
| Unknown because an input was missing, unreadable, or size-inconsistent | **0** |
| **Total additions** | **725** |

The comparison covered all **18,370** base payloads and all **725** additions. A match establishes byte identity with some base payload only; it does not establish semantic ownership or provenance history. A non-match does not prove first-ever creation.

## Blockers and next action

No decryption blocker prevented this pass: the local virtual trees were already available and all inventoried files could be read. No key discovery, brute force, or key extraction was attempted. The exact remaining prerequisites for broader internal analysis are a validated, bounded parser for the six rejected GFLXPACK variants and format-specific record parsers for the **446** readable `.bin`, `.dat`, `.tbl`, and `.blua` entries. To establish code ownership or runtime use, separate direct static-reference or runtime file-access evidence is still required; neither path/name classification nor a base hash match supplies it.

All findings are aggregate counts and format labels. No proprietary paths, payload strings, raw payload hashes, keys, bytes, or member names are included.
