# P0 final bounded-scope matrix

**Date:** 2026-10-06. **Mode:** read-only, metadata/evidence-reference audit.
**Scope:** declared base v0 + update v262144 package roots, reviewed Program
ExeFS sets, five executable module roles, effective outer RomFS, recorded static
inventories, and detected data/parser families. The update is a patch; the target
is the base with that patch applied. This matrix does not claim authenticity,
semantic completeness, or a build-qualified runtime trace.

## Verdict

**Qualified recommendation: the bounded metadata baseline is eligible for P0
closure under the user's stated scope decision, provided the local gates in the
final section pass.** All finite declared package-root, reviewed Program ExeFS,
module-role, effective outer-file, current fix2, and available-C-index scopes
have bounded evidence or an explicit Unknown/Deferred explanation. No further
P0 category wholly outside these declared roots is supported by the reviewed
evidence. This is not a finding that the game-wide discovery universe is
exhaustive: whole-game semantic completeness and undiscovered systems remain
outside this baseline and explicitly Unknown/Deferred.

This audit alone does **not** authorize claiming that the repository's current
P0 status is already closed. The current PLAN and both README status tables
still say P0 is in progress pending local gates. Sheetty and CI were not run.
`git diff --check` returned clean for tracked changes, but by default did not
check these two untracked output files. See **Last local gate**.

## Status convention and evidence boundary

`Known` means the bounded identity, count, or inventory state is directly
recorded for the stated unit—not that its contents are semantically understood.
`Unknown` means evidence does not settle that question; it is not zero or
absence. `Deferred` is an explicit out-of-P0 disposition. `Blocked` is used
only when an evidence path is unavailable or gated; a blocked sub-question does
not block P0 if the scope decision defers it.

The matrix reconciles the 26 census records in
`sheets/re/p0_scope_census.tsv` against the bounded package/ExeFS/overlay
reports, Wave 8 audits, Wave 9 inclusion matrix, PLAN addendum, and both README
completion tables. A read of the requested package-manifest JSON exposed
embedded key/counter/preview metadata in the tool output; those fields were not
used, copied into this report, or repeated in the response. This was contrary to
the explicit no-key-read constraint; no further manifest inspection was done.
The report relies on sanitized evidence reports for package counts. No game
payload, pseudocode, runtime trace, Ghidra/gameDB operation, or source artifact
was used to upgrade a claim.

## Bounded P0 matrix

| Declared unit / count | Evidence and census reconciliation | Status | Rationale / unresolved boundary | Next phase or closure treatment |
|---|---|---|---|---|
| **Base package root — 6 entries** | `work/pla/pk1/manifest.json`; `p0-base-v0-module-provenance.md`; `p0-base-contentmeta-metadata.md`; census `base_package_contentmeta` | **Known** (root count and bounded identity); **Unknown** (authenticity) | Six manifest entries and the Program-to-ExeFS chain are recorded; base ContentMeta has partial reconciliation. A package digest/manifest is not NCA signature authenticity. | P0: retain the six-entry root boundary and citations. Authenticity is Deferred, not a closure gate. |
| **Update package root — 7 entries** | `work/pla/pk2/manifest.json`; `p0-wave8-full-scope-audit.md` §Manifest reconciliation; census `update_contentmeta_cnmt`, `update_content_overlay` | **Known** (root count and listed members); **Unknown / Deferred** (update ContentMeta semantics) | Seven listed members bound the update root inventory. The update ContentMeta/CNMT is opaque in reviewed evidence; no update parse or copied base CNMT values are claimed. | P0: preserve as opaque/Unknown. CNMT semantic parsing is Deferred outside P0. |
| **Base Program ExeFS — 6 members** | `p0-base-v0-module-provenance.md`; `p0-wave8-full-scope-audit.md`; census `base_main_identity`, `rtld_modules`, `sdk_modules`, `subsdk0_modules`, `subsdk1_modules`, and package record | **Known** (reviewed member set) | Five NSOs plus process metadata are enumerated. Five base NSO identities and 15/15 segment hashes are qualified. This is the reviewed Program ExeFS set, not package-wide semantic completeness. | P0: keep the six-member finite boundary; no extra unnamed Program NSO is indicated within the reviewed set. |
| **Update Program ExeFS — 6 members** | `p0-ownership-and-npdm.md`; `p0-update-main-npdm-extraction.md`; `p0-wave8-full-scope-audit.md`; census `update_main_identity`, four auxiliary rows, `update_main_npdm_proof` | **Known** (reviewed member set and disposition); **Unknown / Deferred** (NPDM trust and runtime necessity) | Five NSOs and process metadata are enumerated; update replaces `main` and `main.npdm`; four auxiliary NSOs are byte-identical to base. Bounded NPDM extraction/field comparison is complete, not cryptographic trust or runtime proof. | P0: record identity/disposition. Trust and runtime necessity are Deferred to P2; do not re-extract NPDM. |
| **Executable role: `main`, base v0** | `p0-base-v0-module-provenance.md`; `p0-base-import-relocation-inventory.md`; `p0-base-main-pin-reconciliation.md`; census `base_main_identity` | **Known** (identity/provenance and original-NSO structural inventory); **Unknown** (complete valid-function denominator) | The exact base module is provenance for the port target, not pending port work. Existing detections/exports do not establish semantic completeness. | P0: close bounded identity/import-inventory reporting; preserve base `main` as provenance-only. Broader function coverage is P8. |
| **Executable role: `main`, update v262144** | `p0-update-main-dynamic-metadata.md`; `p0-base-import-relocation-inventory.md`; `p0-fix2-treemap-profile.md`; fix2 `manifest.json` + `generation-status.json`; census `update_main_identity`, `update_main_fix2_treemap_profile` | **Known** (pinned module, static metadata, current located inventory); **Unknown** (complete valid functions, residual semantics) | Current fix2 scope: 153,476 located functions; 153,471 C exports + 5 ASM fallbacks; 51,275,676 body bytes of 53,106,320 executable bytes. Residual is 1,830,644 bytes, not inferred functions. Manifest status is `current` and records archive/inventory/ledger/Rust fingerprints. | P0: accept only as the bounded fix2 profile. Function validity, semantics, behavior, matching, and residual interpretation remain Unknown/Deferred to later coverage work. |
| **Executable role: `rtld`** | `p0-base-v0-module-provenance.md`; `p0-base-import-relocation-inventory.md`; `p0-update-aux-ghidra-inventory.md`; census `rtld_modules`, auxiliary dynamic/dependency records | **Known** (identity and bounded original-image structural metadata); **Unknown** (complete functions/provider/runtime) | Exact base/update identity and byte equality are recorded; detected function/range inventories are not complete denominators. Static candidate relationships do not prove loader binding. | P0: record inventory/export/index limits. Semantic ownership and load/reachability are Deferred/Unknown. |
| **Executable role: `sdk`** | Same module evidence as above; census `sdk_modules`, auxiliary dynamic/dependency records | **Known** (identity and bounded structural metadata); **Unknown** (complete functions/provider/runtime) | Identity is qualified and the reviewed base/update module is byte-identical. Complete function denominator and provider binding are not evidenced. | P0: record bounded status; complete function reconciliation is later-phase work. |
| **Executable role: `subsdk0`** | Same module evidence as above; census `subsdk0_modules`, auxiliary dynamic/dependency records | **Known** (identity and bounded structural metadata); **Unknown** (complete functions/provider/runtime) | Identity is qualified and the reviewed base/update module is byte-identical. Auxiliary listing `Data.isDefined()` results vary by method; this unresolved method variance does not change body unions/import evidence. | P0: retain both listing results and the stated limit. Do not infer additional functions. |
| **Executable role: `subsdk1`** | Same module evidence as above; census `subsdk1_modules`, auxiliary dynamic/dependency records | **Known** (identity and bounded structural metadata); **Unknown** (complete functions/provider/runtime) | Identity is qualified and the reviewed base/update module is byte-identical. Complete function denominator and runtime use are Unknown. | P0: record bounded status; semantic/runtime proof remains outside the baseline. |
| **All five roles: imports/relocations and static dependency candidates** | `p0-base-import-relocation-inventory.md`; `p0-update-aux-ghidra-inventory.md`; `p0-runtime-dependency-resolution.md`; census `update_aux_original_dynamic_metadata`, `update_aux_dependency_probe`, `update_five_role_dependency_probe` | **Known** for measured structural records/candidate counts; **Unknown / Deferred** for provider resolution and runtime | Original-image structural metadata is distinguished from synthetic ELF limitations. The update main's declared dependencies and exact-name candidate edges are static only; declared/imported/candidate/loaded/reached are different states. | P0: reconcile the bounded static inventory and keep runtime binding/load state Unknown. Runtime observation, if needed, is P2. |
| **Effective outer RomFS: 19,095 entries** | `p0-base-update-overlay.md`; `p0-overlay-ownership.md`; `p0-wave8-overlay-coverage-matrix.md`; census `update_content_overlay` | **Known** (effective outer-file arithmetic); **Unknown** (semantic owner/runtime) | Exact partition: 17,904 unchanged + 466 modified + 725 added = 19,095; 0 removed. This denominator counts outer files once. It is not a nested-member or semantic coverage denominator. | P0: close outer-set arithmetic. Do not expand claims to inner members or runtime usage. |
| **Modified outer-file cohort — 466** | `p0-overlay-ownership.md`; `p0-wave8-overlay-coverage-matrix.md`; census `update_content_overlay` | **Known** (cohort and parser/compare outcomes); **Unknown** (452 without successful internal comparison) | 465 differ in size and one same-size entry differs by hash. Successful inner comparisons are 14/466 (SARC 10/10; GFLXPACK 4/10); 452/466 have no successful internal comparison. Parser slices overlap and are not additive. | P0: state 14/466 and 452/466 accurately; remaining comparisons/semantics are not required for bounded closure. |
| **Added outer-file cohort — 725** | `p0-wave8-added-provenance-cross-tab.md`; `p0-overlay-ownership.md`; census `update_content_overlay` | **Known** (hash-comparison partition); **Unknown** (historical provenance/ownership) | 33 exact base-payload hash matches + 692 non-matches + 0 unreadable/inconsistent inputs. A non-match does not prove newness; a match does not prove ownership. | P0: retain aggregate counts and limits; historical provenance/ownership are Deferred. |
| **Outer delta groups and extension families** | `p0-overlay-ownership.md` §§3–4; `p0-wave8-overlay-coverage-matrix.md`; census `update_content_overlay` | **Known** (bounded group/extension partitions); **Unknown** (semantic labels/ownership) | Eleven named coarse delta groups sum to 1,191; seven extension classes also sum to 1,191. `.bin` is not asserted as one format. Group/extension are overlapping views, not disjoint extra populations. | P0: retain as bounded metadata classifications only; no semantic owner inference. |
| **Parser and format cohorts** | `p0-wave8-overlay-coverage-matrix.md`; `p0-overlay-ownership.md` superseding addendum; census `update_content_overlay` | **Known** for reported bounded outcomes; **Unknown** for untested/unaccepted cases | Examples: SARC 10/10 compared; GFLXPACK 4/10 compared; message parser results are complete-pair-cohort only (189 pairs, 179 fully accepted/10 partial); Lua `.blua` 96/108 compile accepts; event `.bin` 68/90 accepts/22 unsupported; AHTB 10/10 strict rejects. These attempts do not combine into a universal file-coverage numerator. | P0: preserve denominator/unit distinctions and unsupported/unknown cohorts. No parser broadening or semantic claims. |
| **Effective scripts** | `p0-wave8-census-audit.md` denominator guardrails; `p0-wave8-overlay-coverage-matrix.md`; census `base_script_index`, `effective_script_inventory` | **Known** (base-only 800 index; 91 changed/added `.blua` delta); **Unknown** (effective script denominator/status join) | Base index is 799 successful + 1 error and is not an effective base+update denominator. Delta-only `.blua` count cannot determine effective scripts. | P0: explicitly mark the effective script denominator Unknown in this bounded baseline; reconcile by effective path if later needed. No runtime ownership inferred. |
| **Assets / format families** | `sheets/domain/asset_formats.tsv` references in Wave 8 audits; `p0-wave8-overlay-coverage-matrix.md`; census `asset_format_catalog`, `effective_asset_format_inventory` | **Known** (17 candidate family labels/catalog rows and bounded delta counts); **Unknown** (effective per-file catalog/unclassified remainder) | Catalog counts mix scope/version and may include nested members; they are not a disjoint 19,095-file denominator. Four BNTX additions have no parser result in the cited audit set; other families likewise have cohort limits. | P0: classify the family universe as named candidate groups plus explicit Unknown/unclassified remainder; per-file semantics/ownership belong to later reconciliation. |
| **Nested archive members** | `p0-wave9-scope-inclusion-matrix.md`; `p0-wave8-full-scope-audit.md`; census `base_content_index`, `effective_asset_format_inventory` | **Known** (base-only SARC index: 257 archives / 3,416 indexed children); **Unknown** (effective nested-child denominator) | Nested members are a separate layer, never added to the 19,095 outer entries. No complete effective base+update nested-member inventory is evidenced; the 452 modified entries without successful internal comparison are one explicit residual, not the full definition of nested scope. | **Deferred** beyond the bounded outer-file baseline; retain Unknown and do not claim nested completeness. |
| **Auxiliary detected module ranges** | `p0-auxiliary-range-reconciliation.md`; `p0-auxiliary-gap-reference-triage.md`; census auxiliary module rows | **Known** (body unions/gaps for existing detections); **Unknown** (inventory completeness and interpretation) | Reports preserve exact detected-body byte/range aggregates, while `Data.isDefined()` results conflict by method. Neither result proves valid-function denominators, semantics, or reachability. | P0: report the aggregates with method variance explicit; do not turn gaps into functions. Full reconciliation is later phase. |
| **Available six-root C index** | `p0-global-gamedb-index.md` final reauthorized retry; census `global_gamedb_index` | **Known** (available corpus/index result); **Unknown** (whole-module/function completeness) | 177,795 available C files indexed; 176,667 parsed function rows; 83,373 strings; 176,667 symbols; 48,991,899 edges; selftest 87/87. 1,128 files have no parsed function row; five update-main ASM fallbacks are outside the C-only corpus. | P0: accept as available-C-index baseline only. Do not infer empty files, complete modules, semantic analysis, or port coverage. |
| **Fix2 update-main progress profile** | `p0-fix2-treemap-profile.md`; generated manifest and generation status; census `update_main_fix2_treemap_profile` | **Known** (current scoped artifact and independent state partitions) | Manifest status `current`; 153,476 located; 22 analyzed/documented; 8 partial implementations; 0 behavior-verified; 0 binary-matched. This is update-main only, not a complete valid-function denominator. | P0: accept freshness/profile identity as a baseline check. Do not promote function states from export/index/build or claim whole-game coverage. |
| **Non-RomFS package content / external services and configuration beyond reviewed members** | Root manifests, `p0-wave8-full-scope-audit.md`, `p0-wave9-scope-inclusion-matrix.md`; census `update_contentmeta_cnmt`, `update_main_npdm_proof`, `scope_discovery_exclusion_method` | **Known** (presence of enumerated root members and reviewed process metadata); **Unknown / Deferred** (unenumerated service/config universe) | The finite listed package roots are bounded, but the evidence does not define all possible system services, external inputs, or operational dependencies. This is not silently excluded or treated as absent. | Baseline: retain explicit Unknown beyond finite package roots. Runtime/service inventory is outside P0; no absence claim. |

## Explicitly Deferred or excluded from P0 claims

| Dimension | Status | Evidence/rationale | Treatment |
|---|---|---|---|
| NCA signature authenticity | **Deferred / Unknown** | No trusted signature verification result is evidenced. Package hashes and identity chains do not prove authenticity. | Not a P0 gate; do not claim valid or invalid. |
| Update ContentMeta/CNMT semantic parsing | **Deferred / Unknown** | Update ContentMeta remains opaque in reviewed evidence; no parse is claimed and no base CNMT values are substituted. | Not a P0 gate; later only under separately authorized scope. |
| NPDM cryptographic trust or runtime necessity | **Deferred / Unknown** | Bounded disposition/extraction/field comparison is distinct from trust and runtime necessity. | Deferred to P2; do not re-extract for this audit. |
| Runtime loaded/reached, dependency binding, file access, and actual use | **Deferred / Unknown** | No build-qualified runtime trace was reviewed. User-reported emulator/ISO use is context only, not independent runtime evidence. | Runtime proof is not required for this P0 baseline; no trace is claimed. |
| Semantic function ownership, function meaning, and file-to-code ownership | **Deferred / Unknown** | Ghidra detections, C-index rows, static references, paths and parser outcomes do not establish semantics or ownership. | Later analysis/coverage phases; no function-state promotion. |
| Complete valid-function denominators, all-module semantic completeness, whole-game completeness, port parity | **Deferred / Unknown** | Fix2 covers located update-main inventory only; base/auxiliary inventories and residual valid-function scope remain incomplete/unknown. | P8/P9 or later; no whole-game claim. |
| Nested-member complete inventory and per-file effective script/asset semantic mapping | **Unknown / Deferred** | Base-only nested index and base/delta script or mixed-scope catalog counts do not yield effective denominators. | Explicitly outside this bounded baseline; never imply zero/absence. |

No affirmative exclusion is made for any uninspected content category. “Excluded”
is not used as a synonym for opaque, unknown, unavailable, or deferred. The finite
declared root/member sets and outer-file set are included; their unresolved
semantic subquestions stay explicit.

## Census reconciliation (26 records)

The census still has **26 records**, not 26 scope units. Records 24–26 provide
the global C index, fix2 profile, and integrated gap/status evidence; records
28–30 bound base/update `main` and pin reconciliation; records 31–35 cover the
package and four auxiliary roles; records 36–40 cover base content, scripts,
format candidates, effective overlay, and heuristic subsystem labels; records
41–44 capture structural dependency and package/NPDM evidence; records 45–49
make update CNMT, NPDM residuals, effective script/asset gaps, and discovery
boundary explicit. The preceding records (including the integrated Wave 1/2
evidence and corrected flow triage) carry the gap and evidence-history details.
Each is linked above by its census ID or evidence family; no new denominator or
duplicate native-function row is introduced by this audit.

The 26-record ledger is sufficient to cross-reference the finite P0 roots and
their current Unknowns under this bounded scope, but it is not itself a
complete-game census. The boundary condition is: **enumerate the listed package
root entries, the six members of each reviewed Program ExeFS, the five named
module roles, and every effective outer RomFS entry once; record a named
metadata family when evidenced and otherwise retain an explicit Unknown or
unclassified remainder. Do not extend that finite boundary to nested members,
external services, semantics, runtime behavior, or undiscovered game systems.**

## Readiness and last local gate

No **entirely unnamed/unbounded P0 category** remains inside the finite declared
package-root / reviewed-ExeFS / five-module / effective-outer-RomFS universe:
each such root has a count or role set and a cited evidence source, while
subquestions without direct evidence are Unknown/Deferred above. The discovery
universe beyond those roots is not exhaustively bounded and is explicitly
outside this baseline—not affirmatively absent or excluded.

**Recommendation:** mark only the **bounded metadata baseline** P0 closure after
the remaining repository-local completion checks confirm the active PLAN
criteria. Before any overall status change, reconcile this matrix with the
current 26 records and verify: (1) `cargo run -p sheetty-cli -- check sheets`,
(2) configured CI/local checks, (3) `git diff --check`, and (4) current fix2 and
global-index fingerprints against their own recorded inventories. The diff
check was run against tracked changes only; sheetty, CI, and fresh fingerprint
recomputation were not performed. PLAN and both README tables still say
P0 is in progress; this report is evidence for a qualified closure decision,
not a claim those gates passed. No authenticity, semantic, runtime, whole-game,
or port-parity claim follows from that decision.

## Evidence reviewed

- `.opencode/skills/decompilacion/SKILL.md` and `THE-SPREADSHEET-METHOD.json`
- `sheets/re/p0_scope_census.tsv`; `odd/PLAN.md`; `README.md`; `README.es.md`
- `work/pla/pk1/manifest.json`; `work/pla/pk2/manifest.json` (inventory scope
  only); `reports/function-progress/p0-base-v0-module-provenance.md`
- `p0-base-import-relocation-inventory.md`; `p0-ownership-and-npdm.md`;
  `p0-update-main-npdm-extraction.md`; `p0-nca-npdm-analysis.md`
- `p0-base-update-overlay.md`; `p0-overlay-ownership.md`;
  `p0-wave8-overlay-coverage-matrix.md`; `p0-wave8-added-provenance-cross-tab.md`
- `p0-wave8-census-audit.md`; `p0-wave8-full-scope-audit.md`;
  `p0-wave9-scope-inclusion-matrix.md`; `p0-bounded-closure-audit.md`
- `p0-global-gamedb-index.md`; `p0-fix2-treemap-profile.md`;
  `update-v262144-fix2/manifest.json`; `generation-status.json`
- `p0-wave9-static-file-code-triage.md`; `p0-auxiliary-range-reconciliation.md`;
  `p0-auxiliary-gap-reference-triage.md`

*Metadata/evidence references only. No payload, pseudocode, or runtime result is
included.*
