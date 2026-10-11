# Community Schema Hints — PLA v262144

**Date:** 2026-10-11
**Scope:** compare public community schema/struct definitions with decoded PLA data tables; schemas are hints and are promoted only when local record shape, widths/ranges, and domain facts support the mapping. This report records findings and blockers; it does not assert that a community schema is the native runtime reader for a given asset.

## Method and status meanings

- **verified**: candidate field order/name/type agrees with the local decoder's raw index and width, with the specified local range and fact checks.
- **unverified**: plausible public candidate, but the source-to-file binding or one or more required checks are missing.
- **contradicted**: an observed local value, index, count, or width conflicts with the candidate or expected uniqueness/integrity rule.
- No upstream implementation code is copied into this repository. Mapping TSVs retain field names/types, source URLs, commit IDs, and verification notes only.
- pkNX at commit `d191cd0e5c05f2af81d9a41c1f1d82e6621b351a` is **GPL-3.0**; PKHeX at commit `66ef5983a5d349efa2dd615f2926a1b1ee97306e` is **GPL-3.0**. Only field labels, primitive types, source links, and commit identifiers are recorded here.

## Current-worktree PML tables

### `pml_personal.tsv`

Source hint: [pkNX `PersonalInfoLAfb.fbs`](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/Arceus/Schemas/Poke/PersonalInfoLAfb.fbs), commit `d191cd0e5c05f2af81d9a41c1f1d82e6621b351a` (GPL-3.0).

**Observed:** 1,276 rows. The mapped scalar fields agree with the schema's order and primitive widths (u16 species/form and base EXP, u8 types/stats/gender/growth). Values include Pikachu (species 25): Electric/Electric, stats 35/55/40/50/50/90, sum 320; Arceus (species 493, form 0): six stats of 120, sum 720, genderless byte 255. These provide independent record-level checks alongside the 1,276-record count and field-width agreement.

**Unverified:** the schema's F02 `IsPresentInGame:bool` is not directly decoded in our TSV. `is_active` is currently derived from `base_hp > 0`, which is compatible with observed examples but does not prove it is F02. `base_stat_total`, display names, type labels, and growth labels are derived output, not raw schema fields.

Detailed map: [`sheets/data/schemas/pml_personal_fields.tsv`](../../sheets/data/schemas/pml_personal_fields.tsv).
