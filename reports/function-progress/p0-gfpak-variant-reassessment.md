# P0 — GFLXPACK modified-entry variant reassessment

**Date:** 2026-10-06. **Scope:** metadata-only reassessment of the six modified GFLXPACK pairs previously rejected by the bounded comparator. No payload, names, strings, paths, bytes, or raw hashes are recorded.

## Outcome

**No alternate variant passed direct layout validation.** This pass could not perform a fresh binary-level comparison: the two inventory manifests were discoverable, but none of their listed absolute virtual-file paths was locally readable (0 of 18,370 base entries and 0 of 19,095 update entries). The six pair identities and the accepted control samples therefore could not be safely resolved for this pass. No unrelated local files were substituted.

Accordingly, no candidate layout was accepted or rejected on new binary evidence, and no parser or test changes were made. The following are carried forward from the prior metadata-only follow-up, not newly re-measured here:

| Prior observation | Count / result |
|---|---:|
| Modified GFLXPACK pairs under investigation | 6 |
| Pairs with a unique first-entry candidate at table end + 8 | 6/6 |
| Candidate header-kind distribution | 1 / 4 / 1 |
| Scanned-offset-count distribution | 4 once; 6 four times; 1,860 once |
| Candidate entry extents bounded in base / update | 6/6 base; 4/6 update |
| Update candidates with at least one out-of-bounds extent under the 24-byte interpretation | 2/6 |
| Pairs accepted by the current comparator | 0/6 |

## Alternate-layout checks and validation boundary

The plausible metadata-only alternatives remain a 24-byte record table with an eight-byte structural gap before payload data, or a wider record interpretation that accounts for the +8 relationship. Neither can be established from the available evidence. The previous observations supply only a first-record offset coincidence; they do not independently define record width, all field meanings, a bounded table end, or a payload-start rule.

In particular, a safe validation still requires all of the following on the actual local inputs and accepted controls:

1. A single, independently evidenced rule for the header kind and offset-array length/boundary, resolving the 1,860-offset scan without heuristic selection.
2. A record interpretation in which independently meaningful fields agree across every entry and the checked record-count sum ends within the file.
3. Bounded payload extents for every interpreted entry; the two previously out-of-bounds update cases must be explained by direct fields in the same layout, not ignored or inferred from their base counterparts.
4. A corresponding discriminating check on accepted controls, showing the proposed rule distinguishes the variant rather than merely fitting the six first-entry values.
5. Direct evidence that the eight-byte region or any proposed wider record is structural, rather than padding/coincidence.

Because the local virtual files and controls were unavailable, these checks could not be run or reproduced in this reassessment. The +8 relationship is not sufficient evidence to relax selection or accept any out-of-bounds entry. The current parser boundary must remain unchanged; the six remain unaccepted for internal comparison.

## Safe next action

Restore or remount the exact authorized base/update virtual RomFS inputs and accepted control samples referenced by the inventories, then repeat the bounded metadata-only checks above. Until those inputs are locally readable, do not select an alternate layout, modify the parser, or claim that the six pairs passed direct validation.
