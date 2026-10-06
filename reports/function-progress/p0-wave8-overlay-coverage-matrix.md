# P0 — Wave 8 overlay structure/parser coverage matrix

**Date:** 2026-10-06. **Scope:** metadata-only reconciliation of format-specific internal-structure and parser reports against the canonical effective base+update overlay. No payloads were inspected for this reconciliation; no proprietary paths, names, strings, raw hashes, or bytes are included.

## Canonical population and accounting rules

The canonical, hash-verified overlay contains **466 modified** common entries and **725 added** update entries: **1,191 delta entries** total. The game target is base plus update; the update is a patch, not a standalone program. Semantic ownership remains **unknown for 1,191/1,191**.

The tables below keep distinct units separate:

- **Entries** count unique overlay members, not parser invocations.
- **Paired candidates** count normalized `.dat`/`.tbl` pairings; a candidate is not two independent overlay entries in every status class.
- **Parser attempts** count side/member invocations, including both base and update versions of modified entries. An accepted parse, compile probe, or unsupported result is not an additional successful inner structural comparison.
- The headline **14/466 successful internal comparisons** is only the local SARC/GFLXPACK member/record comparison result: SARC 10/10 plus GFLXPACK 4/10. Do not add message-table pair parsing, Lua compilation, or event-table parser acceptance to 14.
- Evidence categories overlap across views. For example, `.tbl` parser outcomes are also part of the changed `.dat`/`.tbl` pair cohort, and `.blua` compile probes are a parser outcome, not an inner structural diff. Do not sum the separate tables as disjoint coverage.

## Format denominators: unique overlay entries

| Format / bounded slice | Modified entries | Added entries | Total delta entries | Coverage boundary |
|---|---:|---:|---:|---|
| SARC (`.arc`) | 10 | 0 | 10 | All 10 modified entries compared internally on both sides. |
| GFLXPACK (`.gfpak`) | 10 | 0 | 10 | 4 modified entries compared internally; 6 remain unaccepted by the bounded parser. |
| Lua 5.3 bytecode (`.blua`) | 17 | 74 | 91 | All entries compile-probed on the update side; modified entries also probed on the base side. Compilation only. |
| Message `.dat` | 189 | 160 | 349 | Parser results are reported for the 189 complete-pair candidate cohort, not as a one-to-one parse of all 349 entries. Added-only pair candidates were not parsed. |
| Message `.tbl` | 180 | 160 | 340 | Same complete-pair limitation; the ten AHTB rejections are within the 189-pair parser cohort. |
| Flag/work `.tbl` | 7 | 0 | 7 | Included in the canonical `.tbl` denominator but not established as part of the message-table parser cohort. |
| Event-progress `.bin` | 39 | 12 | 51 | The bounded event parser slice; 1 additional modified event `.bin` lies outside this slice. |
| Other `.bin` groups (all domains) | 14 | 315 | 329 | No corresponding parser result in the cited audit set. Modified breakdown: 1 event outside event-progress, 3 appli, 5 misc, 3 Pokémon, 1 archive, 1 character. Added breakdown: 309 trainer, 4 field, 2 misc. |
| BNTX (`.bntx`) | 0 | 4 | 4 | No parser result in the cited audit set. |
| **Total** | **466** | **725** | **1,191** | Exact canonical totals. |

The `.dat` and `.tbl` extension totals are 189+187=376 modified and 160+160=320 added. This reconciles the message-table audit's 376 changed and 320 added `.dat`/`.tbl` members; the seven modified flag/work `.tbl` entries are included in the extension totals, not in the message ownership group. The message table has 369 modified entries overall (189 `.dat` plus 180 message `.tbl`) and 320 additions.

## Pair and parser-result denominators

### Message `.dat`/`.tbl` candidate cohort

| Measure | Count | Unit / interpretation |
|---|---:|---|
| Affected normalized pair candidates | 356 | Candidate pairs with a changed, added, removed, or otherwise affected member. |
| Complete on both base and update sides | 189 | Pair candidates; only this subset was run through the paired parser comparison. |
| Complete only on base / only on update / neither | 0 / 160 / 7 | Pair candidates; the 160 update-only-complete candidates correspond to the 320 added pair members. |
| Complete pair versions parsed | 378 | 189 candidates × 2 overlay sides. Per-member-type parser attempts below use this same paired cohort. |
| `.dat` parser accepted / rejected | 378 / 0 | Attempts, not unique modified/addition entries. |
| `.tbl` / AHTB parser accepted / rejected | 368 / 10 | Attempts, not unique entries. All 10 rejections are the established out-of-bounds name-length failure; no safe alternate interpretation was found. |
| `MessageStore` pair versions accepted / rejected | 368 / 10 | Pair-version attempts; this result overlaps the `.dat`/`.tbl` parser outcome above. |
| Pair candidates with both sides accepted / one side accepted / neither | 179 / 10 / 0 | Candidate-level status; pair completeness does not imply parser acceptance. |

For the 179 fully accepted pair candidates, positional structure was compared: 35,715 base and 39,505 update entries, with 31,276 offsets, 6,238 lengths, and 838 flags differing across shared indices. These are metadata differences, not message meaning or semantic ownership. The 10 rejected `.tbl` inputs were also separately re-inspected and reassessed; they remain rejected.

The reports do **not** support distributing the 378 attempts by changed versus unchanged member roles within each complete pair. Therefore the result is kept at its reported complete-pair cohort denominator; it is not presented as 378 modified `.dat` plus 378 modified `.tbl` overlay entries. The 160 update-only-complete pair candidates / 320 added members have no message-parser acceptance result in this audit.

### Lua compile probes

| Population | Entries / versions present | Accepted | Rejected | Attempts |
|---|---:|---:|---:|---:|
| Base versions of modified `.blua` | 17 | 17 | 0 | 17 |
| Update versions of modified `.blua` | 17 | 14 | 3 | 17 |
| Added update `.blua` | 74 | 65 | 9 | 74 |
| **Update overlay outcome** (modified update versions + additions) | **91** | **79** | **12** | **91** |
| **All versions probed** | **108** | **96** | **12** | **108** |

All 108 headers were classified as Lua 5.3. “Accepted” means only that `load_chunk` compiled the chunk into an uncalled function; it is not an inner structure comparison, execution, behavior, or ownership result.

### Event-progress parser slice

| Population | Entries | Accepted | Rejected | Unsupported | Attempts |
|---|---:|---:|---:|---:|---:|
| Base versions of 39 modified event-progress `.bin` entries | 39 | 28 | 0 | 11 | 39 |
| Update versions of those modified entries | 39 | 28 | 0 | 11 | 39 |
| Added event-progress `.bin` entries | 12 | 12 | 0 | 0 | 12 |
| **All event-progress parser attempts** | — | **68** | **0** | **22** | **90** |
| **Effective update-side entries in this slice** | **51** | **40** | **0** | **11** | **51** |

The 22 unsupported attempts are the two versions of 11 modified entries; they are not 22 unique overlay entries. The parser does not check a magic signature, and accepted means only that its existing version/extent/identifier checks returned success. The event slice covers 39 of 40 modified event-directory `.bin` entries and all 12 additions; it does not cover the remaining modified event `.bin` or any other `.bin` domain.

## Modified comparison headline and residuals

| Modified format group | Denominator | Successful internal comparisons | Parser/compile or other bounded result, not counted in that numerator |
|---|---:|---:|---|
| SARC `.arc` | 10 | 10 | No member additions/removals; 51 existing member payload digests differed in aggregate. |
| GFLXPACK `.gfpak` | 10 | 4 | 6 rejected by the current parser; follow-up found no directly validated alternate layout. |
| All other modified groups | 446 | 0 | Includes message parser cohort outcomes, `.blua` compile outcomes, event-progress parser outcomes, and entries with no applicable result. |
| **Total** | **466** | **14** | **452/466** have no successful inner structural comparison. |

The 452 residual comparison denominator reconciles exactly as 6 GFLXPACK + 17 `.blua` + 53 `.bin` + 376 `.dat`/`.tbl` = 452. This does **not** mean all 452 are untouched by any parser: the parser outcomes above overlap these entries and answer narrower questions. The other-format `.bin` remainder is 14 entries without the event-progress parser result. Message parser acceptance/rejection does not promote its entries into the separate 14/466 inner-comparison result.

## Added-entry provenance and ownership

The local matching audit reports 33/725 added payloads with an exact base-payload match, 692/725 without an exact match, and 0 unknown due to missing/unreadable/inconsistent inputs. These are an exhaustive **provenance-match** partition, not a format/parser partition: matches can overlap any format row above, and non-match does not prove first-ever creation. The ownership audit remains authoritative for the semantic conclusion: **1,191/1,191 delta entries have unknown semantic ownership**. Parser acceptance, compilation, extension labels, path grouping, and base-byte identity do not establish a code reader, module owner, or runtime use.

## Reconciliation notes and next per-format work

1. **SARC:** retain the 10/10 comparison result; next compare internal structure for the remaining applicable containers only if present in the canonical modified set (none are outstanding in this SARC slice).
2. **GFLXPACK:** six modified pairs remain rejected. Resume only after exact authorized inputs and controls are readable and direct evidence resolves offset-array boundaries, record interpretation, and all payload extents; do not relax the parser from the carried-forward `+8` observation.
3. **Message `.dat`/`.tbl` and flag/work `.tbl`:** preserve the 356-candidate / 189-complete-pair denominator and accepted/rejected counts. Next establish parser results for the 320 added message members and any modified/added table members outside the complete-pair cohort; keep the ten AHTB variants unsupported absent a bounded rule.
4. **`.blua`:** 12 effective update-side compile rejections remain (3 modified, 9 added). Any follow-up must remain metadata-only unless separately authorized; compilation is not behavior. Do not infer format failures from the header signature alone.
5. **`.bin`:** first extend the parser cohort beyond 39 modified + 12 added event-progress entries, while separating the single out-of-slice event modification from other domain-specific binary formats. Do not apply the event parser to unrelated `.bin` groups without format evidence.
6. **BNTX and remaining additions:** 4 BNTX additions, 315 non-event `.bin` additions, and 320 added message members have no result in this cited audit set. Select format-specific, bounded metadata checks next; keep provenance/history and ownership unknown.

**Scope mismatch explicitly retained:** prior ownership-report text in §6 says 0/466 internal diffs, but its later superseding structural-parser addendum records 14/466. This matrix follows the superseding addendum and the local internal audit. The individual parser reports are cohort-limited as described above and cannot be summed into a single disjoint “covered entries” total.

## Evidence references

- `reports/function-progress/p0-base-update-overlay.md` — canonical 466/725 overlay manifest.
- `reports/function-progress/p0-update-changed-content-classification.md` and `reports/function-progress/p0-overlay-ownership.md` — extension/domain denominators and 1,191-entry ownership-unknown boundary.
- `reports/function-progress/p0-overlay-local-internal-audit.md` — SARC/GFLXPACK inner comparisons and added-payload matching.
- `reports/function-progress/p0-message-table-overlay-audit.md` — pair cohort and message parser results.
- `reports/function-progress/p0-ahtb-overlay-reinspection.md` and `reports/function-progress/p0-ahtb-variant-reassessment.md` — `.tbl` rejection profile and no-safe-alternate decision.
- `reports/function-progress/p0-gfpak-variant-followup.md` and `reports/function-progress/p0-gfpak-variant-reassessment.md` — carried-forward GFLXPACK variant limits.
- `reports/function-progress/p0-blua-overlay-compile-audit.md` — Lua compile-probe outcomes.
- `reports/function-progress/p0-event-table-overlay-audit.md` — event-progress `.bin` parser slice.
