# P0 Wave 8 — Census scope-completeness audit

**Audit date:** 2026-10-06. **Mode:** independent, read-only scope audit. The
canonical census, plan, and README files were not changed. No payloads or keys
were read; no Ghidra or gameDB commands were run. This evaluates O10 scope
completeness and explicit inclusion/exclusion accounting, not format-parser
correctness or semantic ownership.

## Finding

The canonical `sheets/re/p0_scope_census.tsv` has **21 data records** (21 IDs
after the header). It is a useful, evidence-linked partial ledger with separate
`known`, `unknown`, `evidence`, and `next_action` fields. It is **not a complete
scope census**: it has no declared discovery boundary/method, no dedicated
inclusion/exclusion register or reason field, and several effective-build
dimensions are only mentioned inside broader integration records. This agrees
with `odd/PLAN.md`, which explicitly says the census is not proof of complete
scope and requires a reason and evidence for every included or excluded unit.

No direct instance was found where a base-only metric is explicitly asserted as
the effective base+update denominator. The more important issue is that several
base-only/catalog metrics sit beside overlay metrics without a reconciled
effective per-file catalog; they must not be used as effective denominators.

## Dimensions captured adequately for this bounded census

“Adequately” here means the record has a bounded subject, scoped evidence, and a
visible limitation; it does not mean that the underlying game scope is complete.

| Dimension | Evidence in current census | Scope limit retained |
|---|---|---|
| Package/build identity and base Program provenance | Base package/ContentMeta record, package pins and qualified Program-to-ExeFS chain | NCA header/signature authenticity remains unknown; update ContentMeta/CNMT semantics are not covered by the base record. |
| Executable-module identity/inventory | Base and update `main`, four auxiliary module records, plus base structural import/relocation work referenced by the census | Function detections and body unions are not complete semantic inventories. Update `main.npdm` is not a separate census record. |
| Effective RomFS overlay arithmetic | `update_content_overlay`: 19,095 effective entries = 17,904 unchanged + 466 modified + 725 added; zero removed | Counts establish the outer-file set only; added provenance, internal changes for remaining files, ownership, and runtime use stay open. |
| Function progress / residual bytes | Current fix2 profile row and update-main gap correction | Fix2 covers located update-main functions only; outside-body bytes are not function rows or an inferred denominator. |
| Available C export indexing | Global-index row records all six roots, 177,795 available C files, 176,667 parsed rows, and limitations | Available exports are not a complete module inventory or semantic/behavior/port coverage. |
| Heuristic subsystem groupings | `subsystem_clusters` row explicitly labels the evidence heuristic and non-exhaustive | The 17 labels are a search seed, not a closed subsystem inventory or file-to-code map. |

The effective overlay arithmetic is consistent with the package/RomFS manifest
reports and the current integration addendum. The base `main` is provenance,
not port work; the target is the mandatory base plus the update patch, whose
`main` replaces base `main`.

## Missing record categories / dimensions

These are census omissions, not claims that the underlying assets or data do
not exist.

1. **Discovery method and exclusion ledger.** No row or companion section
   declares the scope-discovery sources, enumeration rules, stopping condition,
   or excluded categories with a reason and evidence. The current ledger names
   what was inspected, but does not establish why every other candidate area is
   in-scope, out-of-scope, blocked, or not yet inspected. This is the central
   O10 gap.
2. **Update ContentMeta/CNMT semantics.** The base ContentMeta record is
   qualified, and integration text says the update ContentMeta member is opaque
   and its rows/count unknown. There is no dedicated census record for the
   update descriptor, its unavailable semantic rows, or why that dimension
   cannot currently be resolved. Do not copy base row counts to update.
3. **NPDM proof dimensions.** The update `main.npdm` replacement and bounded
   field comparison are now established in separate reports, but the census
   lacks its own record. Cryptographic trust/signature validity and runtime
   necessity are different unknowns and need separate reasons; they are not
   equivalent to the now-resolved file disposition.
4. **Effective script inventory.** `base_script_index` is expressly base-only
   (800 entries, including one failed index result). The overlay record reports
   91 changed/added `.blua` files in the delta, but there is no effective
   base+update script manifest/count, per-file status reconciliation, or
   explanation of the failed base entry. The base index is not the effective
   script denominator.
5. **Effective asset/format manifest.** `asset_format_catalog` is explicitly
   `scope-unresolved`; its aggregate format counts come from an unversioned
   RomFS-plus-archive catalog, not a reconciled effective overlay inventory.
   The catalog has no complete unclassified/unknown-format remainder and no
   per-file provenance/version mapping. Neither catalog counts nor the base
   SARC index establish all effective assets or their consumers.
6. **Package-level content categories/exclusions.** The base package record
   notes all top-level members and base ContentMeta rows, but package member
   classes (including non-Program content and opaque ContentMeta) do not each
   have independent inclusion/exclusion status. The update package's opaque
   ContentMeta and package-to-effective-content reconciliation are embedded in
   broader text rather than independently trackable records.
7. **Non-ROMFS runtime/configuration scope.** There is no explicit category
   matrix for process metadata, loader/runtime prerequisites, external/system
   services, save/configuration interfaces, and other non-RomFS inputs stating
   included/excluded/unknown plus rationale. Related facts appear across
   package, dependency, and plan reports but do not amount to a scope inventory.
8. **Other game systems and content classes.** The census acknowledges that
   heuristic subsystem discovery is incomplete. It does not enumerate remaining
   systems, resource classes, or module/data relationships, nor record explicit
   exclusions. The `Unknown` boundary is therefore qualitative rather than a
   complete list of undiscovered categories.

## Rows whose unknowns/reasons are incomplete or need qualification

- **`base_script_index`:** clearly labels the base-only scope and says the one
  failed entry is unresolved, but does not identify a sanitized failure class,
  whether the failure is an extraction/index/parse issue, or what evidence would
  distinguish them. It also does not specify the effective-overlay reconciliation
  needed before any script denominator can be used.
- **`asset_format_catalog`:** appropriately marks version/scope unresolved, but
  does not state a denominator or count for uncatalogued/unclassified files,
  distinguish nested archive entries from outer RomFS files, or list exclusions.
  Because catalog rows include both RomFS and nested archive scope, summing
  catalog totals as if they were disjoint RomFS files would be invalid.
- **`base_content_index`:** records the base-only 18,370-file/SARC subset and
  says asset-to-module mapping is incomplete, but does not explain which content
  classes are intentionally outside that index or how they are represented
  elsewhere. It must not stand in for the 19,095-entry effective RomFS manifest.
- **`update_content_overlay`:** correctly separates patch inputs from the
  materialized effective overlay, but its `known` field still starts with the
  historical zero-extracted-update-RomFS/SARC statement. Keep that fact clearly
  labeled as patch representation, not as missing effective content. Its
  `unknown` field should separately enumerate update ContentMeta/CNMT,
  update NPDM trust/runtime necessity, addition provenance, remaining internal
  comparisons, semantic ownership, and runtime use rather than relying on
  dispersed integration prose.
- **`subsystem_clusters`:** accurately says non-exhaustive and heuristic, but no
  explicit exclusions or discovery stopping rule; it cannot be interpreted as
  the complete system list.
- **`global_gamedb_index`:** its current row gives strong corpus exclusions and
  correctly preserves unknown function denominators. Keep the global-index
  report's final successful retry as authority; older blocked/empty narratives
  in that report are history, not current status.
- **`update_main_fix2_treemap_profile`:** its non-whole-game and residual-byte
  limitations are clear. It is a profile/integration record, not a separate
  game-scope category; do not count its bytes or functions as coverage of base,
  auxiliary modules, or RomFS.
- **`p0_wave2_evidence_integration`:** useful broad integration but combines
  parser outcomes, function gap aggregates, ContentMeta, and ownership unknowns.
  The embedded mentions do not replace dedicated rows for update ContentMeta,
  NPDM cryptographic/runtime proof, or scope-discovery exclusions.

## Denominator guardrails

- **Base script index (800)** and unversioned `asset_formats.tsv` counts are not
  effective-overlay denominators. The overlay has 91 changed/added `.blua`
  entries, but the full effective script count cannot be derived from that delta
  alone; it requires path-level reconciliation across unchanged, modified, and
  added entries.
- **Base RomFS (18,370)** is not effective RomFS (19,095). The latter is the
  applicable outer-file denominator for the overlay manifest, but not a
  semantic-asset or runtime-use denominator.
- **Update-main fix2 (153,476 functions / 51,275,676 body bytes)** is an
  update-main-only profile. Its 1,830,644 executable bytes outside existing
  bodies are a separate residual, not extra functions. Do not apply this profile
  to the five-module set or whole game.
- **Global gameDB (177,795 C files)** means available staged exports across six
  roots, not all functions/modules. Base-main and auxiliary inventories are
  partial; five update-main assembly fallbacks are excluded; 1,128 files have
  no parsed function rows. None of those counts is a semantic denominator.
- **Format catalog counts overlap scope** where nested archive entries are
  counted alongside RomFS files; they are not safely addable into an outer-file
  total. For any effective asset denominator, derive a version-pinned,
  per-file inventory and declare whether archive children are included.

No evidence reviewed here shows that these base-only figures were already used
as effective-overlay percentages. The audit identifies a concrete misuse risk
and the missing reconciliations, rather than alleging an existing calculation
error.

## Keep evidence classes separate

O10 asks whether the census identifies the full set of relevant dimensions and
records why they are included, excluded, or unknown. It does not re-test whether
SARC/GFLXPACK/AHTB/message/Lua/event parsers accept data. Parser acceptance or
rejection answers a format-specific structural question for a bounded sample;
it cannot establish a complete scope boundary. Likewise, package/path/extension
labels and subsystem-name matches do not establish semantic ownership, and
static candidate edges or hashes do not establish runtime use. Preserve
`Unknown` for scope, ownership, and runtime where direct evidence is absent.

## Integration instructions for a later canonical update

This report is non-canonical. A later authorized integration task should:

1. Preserve the existing **21** IDs and add distinct, independently actionable
   records for update ContentMeta/CNMT semantics; NPDM disposition versus
   cryptographic trust/runtime necessity; effective script inventory; effective
   asset/format inventory; and scope-discovery/exclusion methodology. Add any
   further categories uncovered by that method rather than treating this list as
   closed.
2. Add explicit fields or a linked table for `inclusion_status` (included,
   excluded, unknown, blocked), `reason`, `evidence`, and `denominator_scope`.
   An exclusion requires affirmative evidence and rationale; absent evidence is
   `unknown`, not excluded or N/A.
3. Reconcile script and asset/catalog records to the version-pinned effective
   base+update manifest by path. Separate outer-file entries from nested archive
   children and report unknown/unclassified remainders; do not infer totals from
   base-only catalogs or delta-only counts.
4. Keep package-member, ExeFS/module, effective RomFS, scripts/assets,
   function-inventory, runtime/dependency, and semantic-ownership dimensions
   distinct. Cite the current qualified reports and label superseded historical
   statements as such.
5. Update both `odd/PLAN.md` and the English/Spanish README status tables only
   during that separate integration task, keep their figures synchronized, and
   leave function-progress states unchanged unless their own evidence gates are
   met. This audit itself makes no canonical edits.

## Evidence reviewed

- `.opencode/skills/decompilacion/SKILL.md` and `THE-SPREADSHEET-METHOD.json`
- `odd/PLAN.md`; `sheets/re/p0_scope_census.tsv`
- Base/update package, ExeFS provenance/extraction, ContentMeta/NPDM, and
  effective RomFS overlay reports
- `sheets/domain/asset_formats.tsv`; base script index metadata
- Overlay changed-content and ownership classifications
- Global gameDB index report; fix2 profile report and generated manifest

*Metadata and evidence references only. No game payload, proprietary paths,
strings, keys, pseudocode, or binaries are included.*
