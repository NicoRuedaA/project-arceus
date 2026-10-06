# P0 Wave 1 Evidence Reconciliation

Generated: 2026-10-06. This is a read-only reconciliation for Wave 2 Agent F.
It contains metadata, counts, evidence references, and integration instructions
only. It contains no game payloads, pseudocode, game strings, credentials, keys,
or raw symbol data. No Ghidra process, `gamedb index`, canonical-sheet edit, or
plan edit was performed.

## Status at a glance

- **Closed bounded evidence claims:** 5 (C1–C5 below).
- **Still-open P0 residual categories:** 10.
- **P0 closure:** **not achieved**. The closed claims are static and scoped; they
  do not establish loader binding, runtime reachability, semantic ownership,
  complete function analysis, or a playable port.
- The counts above are this report's reconciliation categories, not function
  coverage percentages and not a replacement for the evidence ledger.

## Input artifacts

**Superseding evidence note (2026-10-06):** current update-`main` export and
auxiliary-module detection/export measurements are recorded in
[`p0-update-main-residual-export.md`](p0-update-main-residual-export.md) and
[`p0-auxiliary-module-export-inventory.md`](p0-auxiliary-module-export-inventory.md).
They do not add a closed claim here or close O2/O3: the 3.44713% update-main
executable-byte residual is outside the fix2 Ghidra function inventory, while
auxiliary function denominators remain unknown. P0 stays open.

### Required Wave 1 reports

1. `reports/function-progress/p0-base-import-relocation-inventory.md`
2. `reports/function-progress/p0-unexplored-modules.md`
3. `reports/function-progress/p0-nca-npdm-analysis.md`
4. `reports/function-progress/p0-overlay-ownership.md`

### Canonical scope records

- `odd/PLAN.md`
- `sheets/re/p0_scope_census.tsv`

### Existing P0 context consulted

The existing `reports/function-progress/p0*.md` set was enumerated and its
identity, extraction, provenance, overlay, dependency, NPDM, ContentMeta,
classification, and sheet/plan-reconciliation records were checked. The
material cross-references include:

- `p0-base-v0-module-provenance.md`
- `p0-base-update-overlay.md`
- `p0-update-changed-content-classification.md`
- `p0-ownership-and-npdm.md`
- `p0-module-import-relocation-inventory.md`
- `p0-update-aux-ghidra-inventory.md`
- `p0-base-aux-ghidra-inventory.md`
- `p0-base-main-dynamic-metadata.md`
- `p0-update-main-dynamic-metadata.md`
- `p0-update-main-npdm-extraction.md`
- `p0-update-extraction-attempt.md`
- `p0-base-contentmeta-metadata.md`
- `p0-base-main-pin-reconciliation.md`
- `p0-base-main-candidate-pin-check.md`
- `p0-aux-module-search.md`
- `p0-sheet-plan-reconciliation.md`

The Wave 1 reports also cite the package manifests, ExeFS/RomFS inventories,
domain sheets, and the local parser/tooling reports. Those underlying payloads
are not reproduced here.

## Count and path checks

The four Wave 1 reports are internally consistent for the claims carried
forward here:

- The base import report lists exactly five named base NSO inputs: `main`,
  `rtld`, `sdk`, `subsdk0`, and `subsdk1`. Its per-module `RELA + PLT` totals
  agree with its total relocation column, and its undefined/reference counts
  retain the stated one-row structural distinction.
- The overlay report's modified groups sum to **466**, its added groups sum to
  **725**, and `17,904 + 466 + 725 = 19,095`. Its extension totals also sum to
  466 modified and 725 added. The ownership categories sum to **1,191**.
- The overlay report's subsystem-label totals are **437/29** for modified and
  **336/389** for added. Therefore the aggregate is **773 mapped / 418
  unmapped**, not 360 mapped additions.
- The NPDM report's differing ranges sum to **510 byte positions** and are
  confined, according to that report, to the opaque ACID signature/public-key
  regions. This is a static byte-diff result, not a cryptographic verdict.
- The reports use both absolute local extraction paths and repo-relative
  `work/` references. The absolute paths are valid as recorded evidence
  references but are not portable canonical paths; later edits must not turn
  them into claims that payloads are tracked in this repository.

## Closed evidence

“Closed” below means closed only for the stated static evidence dimension.

| ID | Reconciled closure | Direct evidence | What remains explicitly outside the closure |
|---|---|---|---|
| C1 | The reviewed base and update Program ExeFS directories account for the six-member name set: five NSOs plus `main.npdm`. No additional unnamed Program NSO is evidenced within those reviewed directories. | `p0-unexplored-modules.md` §§1, 4 | This is not proof of complete semantic exploration or runtime loading of every member. |
| C2 | Base-v0 module provenance is qualified through the Program → NCZ → ExeFS chain; all five base NSOs have direct identity/segment evidence, and the four auxiliary modules are byte-identical between base and update. | `p0-base-v0-module-provenance.md` §§1–6; `p0-overlay-ownership.md` §§1–2 | NCA authenticity, external-build equivalence, semantic roles, and runtime use remain open. |
| C3 | Original-NSO dynamic/import/relocation metadata is directly inventoried for all five named base NSOs. RELA/PLT, dynsym, undefined-row, and observed dynamic-tag results are structural evidence. | `p0-base-import-relocation-inventory.md` §§1–5 | Provider identity, loader binding, load order, runtime resolution, and use are not established. |
| C4 | The effective base+update RomFS population is reconciled as **17,904 unchanged / 466 changed / 725 added / 0 removed**. The 1,191 delta entries have bounded path/extension classifications. | `p0-overlay-ownership.md` §§1–7; `p0-unexplored-modules.md` §6 | Internal file changes, added-file provenance, code ownership, and runtime access remain open. |
| C5 | The update ships a replaced `main.npdm`; it was extracted and hash-checked locally, differs from base at 510 byte positions, and has a bounded field-level comparison: parsed META, ACID body/descriptors, and ACI0 fields are equal outside the opaque cryptographic regions. | `p0-nca-npdm-analysis.md` §§1, 5–6; `p0-update-main-npdm-extraction.md` §2; `p0-ownership-and-npdm.md` §2 | NPDM signature/trust validity and runtime necessity are open. Do not infer them from the byte difference. |

### Static evidence versus runtime requirements

Static closure supports identities, bounded structural metadata, package/member
relationships, overlay set arithmetic, and the limited NPDM comparison above.
It does **not** support any of the following runtime claims:

- that a candidate name match is the loader's provider or that a `DT_NEEDED`
  entry is loaded or reached;
- that a module or subsystem reads a changed/added file;
- that the update NPDM's changed opaque regions are valid, trusted, or required
  by the runtime;
- that the package is authentic merely because local member hashes and section
  relationships match; or
- that detected, exported, indexed, or compiled functions are understood or
  behaviorally covered.

## Still-open P0 items

| ID | Open category | Evidence required before closure |
|---|---|---|
| O1 | Loader binding, dependency identity, provider resolution, load order, and reached/loaded status. | Independent authorized loader or runtime evidence; retain unresolved names where the evidence contract excludes them. |
| O2 | Complete semantic function inventory, especially `rtld`; Ghidra detections and gameDB rows are not semantic coverage. | Version-qualified function inventory plus direct per-function analysis evidence; preserve separate analysis/implementation/behavior/matching states. |
| O3 | Current progress artifacts still contain the capped 68,330-era generated view while fix2 reports 153,476 located functions and 153,471 C + 5 assembly exports. | Regenerate the progress artifacts from the current source/ledger and verify the manifest fingerprint; never hand-edit generated output or promote states. |
| O4 | NCA header decode, signature verification, section-header hashes, and NCA authenticity. | Authorized header-decryption configuration and an independently configured signature verifier; keep required sensitive values out of reports and logs. |
| O5 | Update ContentMeta/CNMT semantic reconciliation. | Parse/reconcile the update ContentMeta/CNMT metadata against the package manifest and Program/patch ownership. The current report explicitly leaves update semantic rows unknown. |
| O6 | NPDM cryptographic validity/trust and runtime consequence/necessity. | Authorized signature/trust verification and a build-qualified loader comparison using the shipped update pairing; static equality/difference is insufficient. |
| O7 | Internal changes inside the 466 modified files/containers, including the one same-size change. | Format-aware metadata/member or record diffs without publishing payloads. |
| O8 | Provenance of the 725 update-only paths as genuinely new versus relocated/duplicated base content. | Authorized hash matching against base data and aggregate provenance results. |
| O9 | File-to-code ownership and runtime reachability for the 1,191 delta entries. | Direct static reference binding followed by a build-qualified runtime file-access trace. |
| O10 | Full P0 scope beyond the current seeds: the base script index has one error, the asset catalog is not tied to an effective denominator, subsystem labels are heuristic/non-exhaustive, and broader systems remain uncensused. | Resolve the script/catalog gaps, expand discovery, and document every inclusion/exclusion with evidence; do not treat the 17 labels as a complete system map. |

## Contradictions and stale statements

The following are the integration-relevant contradictions. Historical reports
should retain their original observations, but their active status must point to
the newer evidence rather than silently reusing stale conclusions.

| Source | Stale or unsupported statement | Required interpretation/correction |
|---|---|---|
| `odd/PLAN.md` active summary and P0 sections (including the lines/rows called out by `p0-sheet-plan-reconciliation.md` A3–A8, B1–B4, and C7) | Base pin/provenance and base imports/relocations are still presented as unresolved work. | Base pin/provenance and the five-NSO base structural import/relocation inventory are closed. Keep loader binding, semantic coverage, and runtime resolution open. |
| `sheets/re/p0_scope_census.tsv` `base_main_identity`, `base_main_candidate_pin_check`, `base_package_contentmeta_probe`, and `*_modules` rows | Candidate/mismatch language and “base imports/relocations unknown” language is retained after the direct base inventory. | Mark the pin and module provenance satisfied; retain structural-vs-semantic/runtime limits. The candidate row may remain as historical evidence only. |
| `sheets/re/p0_scope_census.tsv` `update_main_identity` | The 68,330 capped function count is used as the current update-main denominator and overlay/provenance clauses are stale. | Record fix2 inventory/export/index figures with a superseded-capped note; do not promote them to semantic coverage. Remove overlay/provenance from the unresolved identity clause. |
| `sheets/re/p0_scope_census.tsv` `rtld_modules`, `sdk_modules`, `subsdk0_modules`, `subsdk1_modules` | Original NSO imports/relocations are described as unknown or only synthetic-ELF zero-row evidence. | Link `p0-base-import-relocation-inventory.md`; base and update original-NSO structural metadata is available. Keep `rtld` complete-function inventory, semantics, and runtime open. |
| `sheets/re/p0_scope_census.tsv` `update_content_overlay` | Effective overlay is described as formerly unknown and its group summary omits `field` in the stale wording. | Use 17,904/466/725/0 and include all bounded groups, including `field`; keep internal diffs, added provenance, ownership, and runtime unknown. |
| `p0-base-v0-module-provenance.md` §7 | It says base original-NSO imports/relocations are unknown and treats update `main.npdm` disposition as unknown. | Add a superseding pointer: base structural metadata is closed by the Wave 1 base inventory; NPDM disposition and bounded static diff are closed by the NPDM reports. Retain NCA/runtime limits. |
| `p0-base-update-overlay.md` §§1 and 4 | It still labels update `main.npdm` and base-v0 provenance unresolved. | Preserve as historical manifest text, but point to `p0-ownership-and-npdm.md`, `p0-update-main-npdm-extraction.md`, and `p0-base-v0-module-provenance.md`; do not re-open those identity claims. |
| `p0-update-aux-ghidra-inventory.md` §§2–5 and `p0-update-main-dynamic-metadata.md` limits | Earlier wording says base provenance, update-main scope, or the effective overlay remain unknown. | Limit the remaining unknown to semantic completeness, candidate-vs-runtime dependency resolution, and the residual P0 categories. The five-module candidate edges remain candidates only. |
| `p0-base-main-dynamic-metadata.md` introduction/limits | It says base provenance and effective overlay remain unresolved. | Replace those residual references with the qualified provenance/overlay reports; retain runtime and semantic limits. |
| `p0-base-aux-ghidra-inventory.md` and `p0-aux-module-search.md` | Candidate/unavailable/no-inventory language is presented without the later package-qualified NSO evidence. | Mark these bounded historical searches as superseded for identity/extraction. Do not erase their limitations: `rtld` still lacks a complete semantic inventory. |
| `p0-update-changed-content-classification.md` §3.1 | Prose says 360 added files are name-mapped, while its table and the overlay audit reconcile to 336. | Use **336 mapped / 389 unmapped additions**; retain 360 only as explicitly stale historical wording. |
| `p0-overlay-ownership.md` §9 | It instructs the next runtime step to “re-extract” update `main.npdm`. | The file is already extracted and verified at the path recorded by `p0-update-main-npdm-extraction.md`; the next runtime step is to pair that file with update `main`, unchanged auxiliaries, and the base+update virtual RomFS. |
| `p0-base-main-candidate-pin-check.md` and `p0-base-contentmeta-metadata.md` | Their historical bodies say the pin is failed/unreconciled and the candidate must not be parsed. | Their superseding banners are correct. Integration must not cite the historical body as current status. |

### Unsupported claims to suppress

The following interpretations are not supported by the reviewed evidence:

1. “P0 is closed” because all six ExeFS names are present or because the
   effective overlay has been counted.
2. Runtime provider/binding or dependency use inferred from exact-name overlap,
   `DT_NEEDED` counts, or converted-ELF zero relocation rows.
3. Semantic file ownership inferred from directory labels, extension labels, or
   the 17 subsystem clusters.
4. NCA invalidity inferred from the bounded `NCA3` probe or NPDM signature
   validity inferred from a byte difference.
5. “Update-only content” inferred from the 19,095 effective entries; the update
   is a patch and the count is base plus update overlay.
6. Complete semantic function coverage inferred from Ghidra detections,
   pseudocode exports, gameDB rows, build success, or tests unrelated to the
   declared evidence dimension.

## Canonical-file update instructions for the integration writer

No canonical file was edited in this task. The later writer should make the
following bounded changes, preserving historical text with explicit
`SUPERSEDED`/scope notes rather than deleting provenance:

### `odd/PLAN.md`

1. In the active 2026-10-06 summary and the active P0 section, remove base
   imports/relocations, base pin, and base-v0 provenance from the open identity
   work. Replace them with: **base structural metadata for the five named NSOs
   is directly inventoried; loader binding/provider identity, semantics, and
   runtime remain open**.
2. Keep the six-member ExeFS universe and the base+update model. Do not describe
   the update as a standalone program.
3. Replace “NPDM semantic diff open” with **bounded static NPDM field comparison
   complete; opaque signature/key validity and runtime necessity remain open**.
4. Retain the effective overlay counts **17,904 / 466 / 725 / 0** and correct
   active added-label coverage to **336 mapped / 389 unmapped**. Keep 360 only as
   historical stale text if it is explicitly labelled.
5. Add the four Wave 1 reports and this reconciliation to the active P0 evidence
   chain. Keep P0 “in progress” until O1–O10 are either evidenced or explicitly
   excluded with reasons.

### `sheets/re/p0_scope_census.tsv`

Update the affected rows without changing the 17-record scope or inventing
function-progress states:

- `base_main_identity`: mark identity and five-module base structural metadata
  as known; remove base imports/relocations from `unknown` and `next_action`;
  retain semantic coverage and loader/runtime unknowns.
- `base_main_candidate_pin_check`: retain the historical mismatch as
  non-reproducible/superseded and point to the verified canonical artifact.
- `update_main_identity`: replace the capped 68,330-era current count with the
  fix2 inventory/export/index figures, label the generated capped view stale,
  and remove already-qualified base provenance/overlay from its unknown text.
- `base_package_contentmeta` and `base_main_package_metadata_probe`: record the
  qualified Program→NCZ→ExeFS chain and base pin; retain NCA authenticity,
  update CNMT semantics, and runtime limits.
- `rtld_modules`, `sdk_modules`, `subsdk0_modules`, `subsdk1_modules`: link the
  direct base structural inventory; replace “imports/relocations unknown” with
  “original-NSO structural metadata inventoried; semantic/function/runtime
  coverage remains open.” Keep `rtld`'s incomplete inventory explicit.
- `update_content_overlay`: record **17,904 unchanged / 466 changed / 725
  added / 0 removed**, include `field` among the bounded groups, and retain
  internal-diff, added-provenance, ownership, and runtime unknowns.
- `update_aux_original_dynamic_metadata`, `update_aux_dependency_probe`, and
  `update_five_role_dependency_probe`: retain their update-only/candidate-only
  scopes, but remove stale claims that base provenance or the effective overlay
  are unknown.
- Preserve the distinction between extraction/inventory and analysis,
  implementation, behavior verification, and binary matching. No ledger state
  is advanced by these reports.

### Other report pointers

Add superseding pointers to the stale report sections identified above. Do not
rewrite historical observations, publish payloads, or treat absolute local paths
as portable repository paths. Generated progress artifacts must be regenerated
by their owning workflow, not hand-edited as part of this reconciliation.

## Closure gate status

| Gate | Status | Reason |
|---|---|---|
| Build/package identity and bounded module universe | **Pass, scoped** | Base/update identities, six-member ExeFS name set, and base provenance are qualified for the reviewed package artifacts. |
| Base original-NSO structural imports/relocations | **Pass, scoped** | Five named base NSOs were parsed directly; this is not loader/runtime evidence. |
| Effective overlay set and outer classification | **Pass, scoped** | 17,904/466/725/0 and 1,191 bounded outer classifications reconcile. |
| NPDM disposition and static field diff | **Pass, scoped** | Update file is shipped/replaced/extracted; parsed fields compare equal outside opaque crypto regions. |
| NCA authenticity and NPDM cryptographic/runtime requirements | **Blocked/open** | Required authorized configuration/verifier and runtime comparison are absent. |
| Semantic scope/function coverage | **Open** | `rtld`, broader modules/systems, internal data diffs, ownership, and behavior are not closed. |
| P0 overall closure | **Fail / remain open** | O1–O10 remain; no completion claim is permitted. |

## Next action

The later integration writer should first apply the exact `PLAN.md` and census
updates above, add superseding pointers to stale reports, and leave the P0
status open. The next evidence work is independent and should be staged as
bounded tasks: authorized NCA/CNMT verification, current progress-artifact
regeneration, format-aware diffs for the 466 changed files, provenance checks
for the 725 additions, static file-to-code binding, and finally a
build-qualified loader/runtime trace. None of those runtime requirements may
be replaced by extraction, candidate name matches, export counts, or build
success.
