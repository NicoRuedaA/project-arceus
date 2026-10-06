# P0 Wave 1 — Unexplored modules and scope census audit

Generated: 2026-10-06. Read-only audit. This report does not edit the canonical
plan or census, and contains metadata and evidence references only. It contains
no game payloads, pseudocode, game strings, credentials, keys, or raw symbol
data.

## 1. Finding

The canonical base+update executable set is accounted for by the package-level
ExeFS directory: `main`, `main.npdm`, `rtld`, `sdk`, `subsdk0`, and `subsdk1`
in each version. There is no direct evidence of an additional unnamed
executable NSO in the reviewed Program ExeFS directories. The remaining module
gap is therefore **depth and evidence type**, not an unbounded list of missing
module names:

- base original-NSO dynamic metadata/import counts and relocation forms are now
  directly inventoried, but loader binding, dependency identity, and runtime use
  remain unknown;
- `rtld` has no complete reconciled function inventory, especially on the base
  side;
- function detections, pseudocode exports, and gameDB rows are not semantic
  analysis, behavior, or runtime-use evidence for any module;
- loader binding, provider resolution, and reached/loaded status remain unknown.

The RomFS overlay is measured, but its changed and added groups remain
under-explored at file-internal, provenance, ownership, and runtime levels.
The update package's Program ExeFS directory is directly inspected, while its
package-level ContentMeta/CNMT reconciliation is not represented as a completed
P0 census unit.

Unknown below means missing direct evidence, not zero coverage. A missing
denominator is reported as `—`; no percentage is inferred from an inventory
count alone.

## 2. Scope sources and build identity

### Sources inspected

| Source | Direct scope evidence |
|---|---|
| `odd/PLAN.md` | Current P0 route, base+update overlay rule, unresolved base imports/relocations, NCA signature, NPDM field diff, file ownership, runtime resolution, and unexplored modules/areas. |
| `sheets/re/p0_scope_census.tsv` | 17 manually curated P0 records; its doctrine explicitly says unknown records gaps rather than zero coverage. |
| `sheets/re/base_update_diff.tsv` | Base/update module sizes, legacy capped `main` counts, and base RomFS/SARC totals. It does not provide per-auxiliary-module inventories. |
| `work/pla/pk1/manifest.json` and `work/pla/pk2/manifest.json` | Local package identities and NCZ/section metadata. The base Program chain and update ExeFS section are further qualified by the P0 provenance/extraction reports. |
| `reports/function-progress/p0-base-v0-module-provenance.md` | Base Program → NCZ → ExeFS chain; six base ExeFS members; 15/15 base NSO segment hashes verified. |
| `reports/function-progress/p0-ownership-and-npdm.md` and `p0-update-main-npdm-extraction.md` | Update ExeFS has the same six-member name set; update `main` and `main.npdm` replace base members; update NPDM is extracted and hash-verified. |
| `reports/function-progress/p0-base-update-overlay.md` and `p0-update-changed-content-classification.md` | Effective RomFS set comparison and changed/added group classification. |
| `reports/function-progress/p0-module-import-relocation-inventory.md`, `p0-base-import-relocation-inventory.md`, and `p0-update-aux-ghidra-inventory.md` | Original-NSO dynamic counts for the five update NSOs and five base NSOs, plus name-overlap candidate edges; no runtime binding proof. |
| `reports/function-progress/update-main-full-pseudocode-export.md` and `reports/function-progress/update-v262144/manifest.json` | Current fix2 export/index report versus a still capped generated progress manifest. |

### Pinned build

| Build object | Identity evidence |
|---|---|
| Base package | `pk1.nsz`, 2,334,586,382 bytes, SHA-256 `00167d5e00bf7f5fca5311f984069497f79812cbbd03c32eb7f2fc1649af2acc`. |
| Update package | `pk2.nsz`, v262144, 52,657,467 bytes, SHA-256 `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`. |
| Base `main` | 31,755,066 bytes, module ID `7fcad279…`, SHA-256 `6f0e5f4a…ee994a0`; the canonical artifact matches the only documented pin. |
| Update `main` | 31,882,976 bytes, module ID `aee8f150…`, SHA-256 `89fa2d71…b90f7d9`; all three embedded NSO segment hashes pass. |
| Effective RomFS | 18,370 base entries; 19,095 update-virtual entries: 17,904 unchanged, 466 changed, 725 added, 0 removed. |

The update is a patch over the base, not an independently executable program.
The effective port scope is base plus update replacements/additions, with the
update `main` and `main.npdm` replacing their base counterparts and the four
auxiliary NSOs byte-identical between versions.

## 3. Evidence vocabulary used in this audit

| Label | What the reviewed evidence proves | What it does not prove |
|---|---|---|
| Extracted | A bounded file/member or virtual inventory exists locally and passed the stated identity/hash gate. | Complete package semantics, ownership, or runtime use. Zero extracted update-only RomFS rows means BKTR patch representation, not zero effective update content. |
| Inventoried | Metadata rows, package entries, format counts, or original-NSO structural counts were produced for the stated scope. | Complete function recovery, semantic understanding, or all-module coverage when the denominator is absent. |
| Exported | Pseudocode or assembly artifacts were produced for the stated function inventory. | Analysis, implementation, behavior, or binary matching. |
| Indexed | gameDB rows were generated and independently checked for the stated tree. | Semantic analysis or runtime reachability. |
| Classified | A logical or heuristic group label was assigned. | Direct file-to-code ownership; path/SCC name matching remains a hypothesis. |
| Runtime-observed | A trace or loader/emulator observation directly records loading, binding, access, or execution. | No reviewed P0 source supplies this state; runtime remains unknown. |

## 4. Executable-module census

The package-level evidence enumerates six ExeFS members per version. `main.npdm`
is process metadata, not an NSO. The four auxiliary NSOs are byte-identical
between base and update. A separate direct base-v0 inventory now measures
original-NSO dynamic metadata for all five base NSOs; it does not prove loader
binding or runtime use.

| Unit | Extracted / identity | Inventoried | Exported / indexed | Classified | Runtime-observed | Current gap |
|---|---|---|---|---|---|---|
| Base `main` | Yes; package-qualified, 15/15 segment hashes across base NSOs | Structural original-NSO metadata is directly inventoried; 68,412 capped Ghidra detections also exist | Partial historical base export/index evidence; no complete semantic inventory denominator | Own-code attribution is high-confidence heuristic | Unknown | Loader binding, semantic coverage, and unrepresented functions; base structural import/relocation inventory is no longer open. |
| Update `main` | Yes; exact NSO pin and 3/3 segment hashes | Yes structurally: original-NSO dynamic metadata | Fix2 report: 153,476 located; 153,471 C plus 5 assembly fallbacks; 153,471 gameDB files and 153,470 function rows | Function/subsystem labels remain identification or heuristic only | Unknown | Loader/runtime resolution, semantic analysis, behavior, and the stale generated progress view. |
| Base/update `rtld` | Yes; exact base/update identity; byte-identical | Direct original-NSO metadata exists for both versions; update Ghidra metadata detection exists (33 rows) | No complete reconciled function inventory/index, especially base-side | SDK-runtime-loader role is heuristic | Unknown | Complete function inventory/semantic coverage and runtime binding; converted-ELF zero rows are not evidence of no relocations. |
| Base/update `sdk` | Yes; exact identity; byte-identical | Direct original-NSO dynamic metadata exists for both versions | Metadata-only Ghidra detections (13,531) for the exact module; not semantic pseudocode coverage | Shared-library/provider role is heuristic | Unknown | Semantic function coverage, provider binding, and runtime use. |
| Base/update `subsdk0` | Yes; exact identity; byte-identical | Direct original-NSO dynamic metadata exists for both versions | Metadata-only Ghidra detections (5,523); not semantic pseudocode coverage | Subsystem role is heuristic | Unknown | Semantic function coverage, provider binding, and runtime use. |
| Base/update `subsdk1` | Yes; exact identity; byte-identical | Direct original-NSO dynamic metadata exists for both versions | Metadata-only Ghidra detections (8,262); not semantic pseudocode coverage | Subsystem role is heuristic | Unknown | Semantic function coverage, provider binding, and runtime use. |
| Base `main.npdm` | Yes; base ExeFS member, format/hash known | Format/layout identified; fields not semantically compared | Not applicable: process metadata, not code | Process metadata | Unknown | Field-level interpretation is not recorded in the current audit. |
| Update `main.npdm` | Yes; extracted to `work/pla/pk2/main.npdm`, `META`, hash verified, differs from base | Format/layout identified; semantic diff not parsed | Not applicable: process metadata, not code | Replacement process metadata | Unknown | Field-by-field base/update diff and runtime requirement are open. |

### Module conclusion

The direct package evidence does **not** support claiming an undiscovered seventh
Program NSO. It supports claiming that all five base NSOs and all five update
NSOs have bounded original-NSO structural metadata, and that the five update
NSOs have candidate dependency comparisons. The phrase “modules not explored”
in the plan/census should therefore be read as unresolved module dimensions
(complete function inventory, semantics, loader binding, and runtime), not as
proof that an additional module name exists.

## 5. Census-record reconciliation (all 17 records)

This table preserves the current census scope while separating completed identity
or inventory evidence from residual unknowns.

| Census record | Current evidence state | Exact unexplored or under-explored area |
|---|---|---|
| `base_main_identity` | Identity and direct base original-NSO structural metadata qualified; capped function/call-edge counts exist | Loader binding and semantic coverage; count denominator is capped. |
| `base_main_candidate_pin_check` | Historical mismatch superseded; canonical artifact satisfies the recorded pin | No identity gap remains for the canonical artifact; retain only residual base-module gaps. |
| `update_main_identity` | Identity and original dynamic metadata qualified | Runtime provider resolution, semantic ownership, and non-`main` scope; census count is older than fix2. |
| `base_package_contentmeta` | Base package, Program chain, six ExeFS members, and ContentMeta row hashes qualified | NCA-header decode/signature verification and semantic treatment of non-Program content. |
| `rtld_modules` | Identity, extraction, byte equality, and direct base/update dynamic metadata qualified; update metadata detection exists | No complete reconciled function inventory, especially base-side; no runtime evidence. |
| `sdk_modules` | Identity, byte equality, and direct base/update dynamic metadata qualified | Semantic coverage and runtime provider binding. |
| `subsdk0_modules` | Same evidence class as `sdk` | Semantic coverage and runtime provider binding. |
| `subsdk1_modules` | Same evidence class as `sdk` | Semantic coverage and runtime provider binding. |
| `base_content_index` | Base RomFS and base SARC index measured | Effective per-file ownership/subsystem map and semantic coverage; no complete denominator for code/data binding. |
| `base_script_index` | Base script index has 800 rows: 799 `ok`, one `error` | Error row is unresolved; update script overlay, script-to-code binding, and runtime reachability are not reconciled. |
| `asset_format_catalog` | Format/magic census has 25 rows; most are `identified`, `trskl`/`tranm` are `partial` | Catalog is not tied to an effective-overlay denominator or per-file runtime use. |
| `update_content_overlay` | Effective counts and changed/added directory groups are hash-qualified | Internal changes in 466 files, provenance of 725 additions, and runtime use remain unknown. |
| `subsystem_clusters` | 17 string/SCC-derived labels and representative IDs exist | Labels are heuristic and non-exhaustive; no direct file-to-code or runtime ownership. |
| `update_aux_original_dynamic_metadata` | Four exact update auxiliary NSOs have bounded original-NSO counts | Base counterparts are now directly inventoried in `p0-base-import-relocation-inventory.md`; semantic coverage and runtime binding remain open. |
| `update_aux_dependency_probe` | Four-auxiliary candidate-name comparison exists as historical bounded evidence | Candidate edges are not loader/runtime proof and are superseded in scope by the five-role comparison. |
| `update_five_role_dependency_probe` | All five update NSOs have candidate-name comparison and aggregate counts | 9 of 20 directed pairs had no exact name match, but this is not global absence; runtime remains unknown. |
| `base_main_package_metadata_probe` | Historical probe is superseded for identity by the qualified Program→ExeFS chain | NCA signature verification and NPDM semantic diff remain open; do not repeat the identity pin check. |

## 6. Effective RomFS and update-data groups

### Measured overlay classes

| Effective class | Count | Evidence state | Gap |
|---|---:|---|---|
| Unchanged common files | 17,904 | Full common-set hash comparison | Runtime use and file-to-code ownership are not observed. |
| Changed common files | 466 (465 size-different + 1 same-size) | Full common-set hash comparison | No per-entry/record diff; the same-size change is detected but not semantically inspected. |
| Added update files | 725 | Update virtual RomFS inventory and set comparison | No base counterpart; new-versus-relocated provenance is unknown. |
| Removed files | 0 | Path-set comparison | This is a measured overlay result, not proof that no runtime behavior changed. |

### Changed groups

| Group | Count |
|---|---:|
| `message` | 369 |
| `event` | 40 |
| `haxe` | 17 |
| `appli` | 13 |
| `archive` | 11 |
| `flagwork` | 7 |
| `misc` | 5 |
| `pokemon` | 3 |
| `chara` | 1 |
| **Total** | **466** |

### Added groups

| Group | Count |
|---|---:|
| `message` | 320 |
| `trainer` | 309 |
| `haxe` | 74 |
| `event` | 12 |
| `appli` | 4 |
| `field` | 4 |
| `misc` | 2 |
| **Total** | **725** |

These labels are path/extension heuristics. The current 17-label subsystem set
name-maps 437 changed and 336 added files, while 29 changed and 389 added files
have no matching label. Even the name-mapped subset has no established code/data
edge. The 336/389 split is the reconciled total for 725 additions; the older
360-added figure is inconsistent with the group table and is treated as stale.
The missing-label set is a discovery gap, not evidence that those files are
unused or out of scope.

## 7. Gaps and evidence required to close them

The following are bounded gap categories, not invented coverage percentages.

| ID | Category | Direct evidence of the gap | Evidence needed to close it |
|---|---|---|---|
| G1 | Loader binding and dependency identity | The base and update original-NSO inventories count `DT_NEEDED`/relocation/import rows but intentionally retain no dependency names or runtime binding; all reports leave loaded/reached status unknown. | Obtain independent authorized loader evidence for provider identity, load order, binding, and reached status; preserve unresolved names where the evidence contract excludes them. |
| G2 | Complete `rtld` function inventory | Census says no complete Ghidra inventory exists for `rtld`; update metadata detection is only 33 rows, while the new base report supplies structural NSO metadata rather than semantic function coverage. | Produce a version-qualified function/metadata inventory for exact `rtld` inputs, with explicit recovered/unrecovered limits; do not infer from synthetic-ELF zero rows. |
| G3 | Semantic function coverage | Ghidra detection counts, C/assembly exports, and gameDB rows are explicitly non-semantic in the reports and plan. | Direct per-function analysis evidence and ledger entries for the declared module/build denominator; keep analysis, implementation, behavior, and binary matching independent. |
| G4 | Update/base NPDM semantic diff | Update NPDM extraction proves size/magic/hash/difference, but §3 of the NPDM report says ACID/ACI0/service/capability fields are uninterpreted. | Field-by-field metadata parse of both NPDMs; a separate qualified loader experiment is needed to claim runtime necessity. |
| G5 | NCA header and signature | The base package chain is qualified through NCZ descriptors/PFS0/member hashes, while NCA-header decode/signature verification remains explicitly unknown. | Authorized key/configuration path plus independent header decode and signature verification; keep keys and sensitive material out of reports. |
| G6 | Internal diff of changed files | Overlay report proves 466 changed container/file hashes only; classification report says no internal SARC/GFPAK/table/script diff exists. | Format-aware metadata diffs for all 466 changed files: internal entries for containers and record/field deltas for tables/scripts, without publishing payloads. |
| G7 | Added-file provenance | Overlay report states the 725 added paths have no base counterpart and new-versus-relocated provenance is unknown. | Hash-match added content against authorized base data, then record only aggregate provenance outcomes and evidence references. |
| G8 | File→code ownership and runtime reachability | Classification report explicitly says directory matching does not establish a function edge and no runtime/file-access trace exists. | Static path/reference binding for candidate edges, followed by a build-qualified loader/emulator file-access trace for runtime claims. |
| G9 | Non-exhaustive subsystem census | 17 clusters are derived from update-main string/SCC references; 29 changed and 389 added files are unmapped by name, and mapped labels are still hypotheses. | Expand discovery from the effective overlay and executable modules; establish ownership with direct references and document exclusions. |
| G10 | Scripts and asset catalog reconciliation | Base script index has one error; asset-format counts are `scope-unresolved`; neither is tied to the effective overlay/runtime denominator. | Resolve the script-index error, reconcile base/update effective files, validate format descriptors, and record per-file status with `—` where no denominator exists. |
| G11 | Update package metadata scope | `pk2/manifest.json` includes a package CNMT NCA, while reviewed P0 records do not provide a completed update ContentMeta/CNMT reconciliation; only the update Program ExeFS directory is directly used for the six-member result. | Parse and reconcile update ContentMeta/CNMT metadata against the update package manifest and Program/patch ownership, without exposing payloads. |

## 8. Contradictions between plan, census, reports, and manifests

The prior reconciliation register records approximately 70 stale entries across
21 files. The material contradictions relevant to this audit are:

| Cluster | Contradiction | Resolution for the integration writer |
|---|---|---|
| C1 — stale base identity language | Older `p0-base-main-candidate-pin-check.md`, `p0-base-contentmeta-metadata.md`, and historical plan/census wording say the base pin is unresolved or failed. | The reconciliation report and current census state that the canonical base `main` matches the only documented pin; treat the older text as superseded history. |
| C2 — stale base provenance language | `p0-base-aux-ghidra-inventory.md` still calls `sdk`/`subsdk0`/`subsdk1` candidates and says package mapping is unknown. | `p0-base-v0-module-provenance.md` qualifies all six base ExeFS members and 15/15 NSO segment hashes; residual is metadata/semantics, not identity. |
| C3 — stale overlay/NPDM language | `p0-base-update-overlay.md` §4 still says update NPDM and base provenance are unresolved. | `p0-ownership-and-npdm.md` and `p0-update-main-npdm-extraction.md` prove the update NPDM is shipped, replaced, extracted, and different; overlay counts are also qualified. |
| C4 — stale auxiliary/main dependency language | `p0-update-aux-ghidra-inventory.md` and `p0-update-main-dynamic-metadata.md` retain text saying base provenance/overlay or update-main scope is unresolved. | Read the newer five-role comparison and base-v0/overlay reports for current scope; retain only runtime binding and semantic unknowns. |
| C5 — capped versus fix2 counts | `sheets/re/base_update_diff.tsv`, `reports/function-progress/update-v262144/manifest.json`, and the generated progress TSV still use the 68,330 capped update-main denominator, while the fix2 report records 153,476 located functions and 153,471 exports. | Do not use the generated manifest as current fix2 coverage. Regeneration is a separate writer task; this audit does not modify it. |
| C6 — extracted versus effective update data | `base_update_diff.tsv` records zero extracted update RomFS/SARC rows, while the overlay reports measure 19,095 virtual effective entries and 466/725 changes/additions. | Interpret zero as “no standalone extracted update RomFS rows because the update is BKTR”; use the effective overlay manifest for content scope. |
| C7 — base import/relocation report not propagated | `p0-base-import-relocation-inventory.md` is complete for the five named canonical base NSOs, but `odd/PLAN.md`, `p0-base-v0-module-provenance.md` §7, and census unknown/next-action text still describe base imports/relocations as open. | Treat direct base structural metadata as closed; retain only dependency identity/binding, semantic coverage, and runtime unknowns. |
| C8 — subsystem-match count drift | `p0-update-changed-content-classification.md` says 360 additions are name-mapped, while the newer `p0-overlay-ownership.md` and its group totals give 336 mapped and 389 unmapped additions. | Use 336 mapped + 389 unmapped = 725; do not use the stale 360 figure. |

`generation-status.json` says `current`, but that status is internally consistent
with the capped generator inputs shown in its companion manifest, not with the
fix2 inventory. This is a freshness/scope contradiction, not evidence that fix2
was absent.

## 9. Bounded recommendation for the integration writer

Do not expand the canonical census by inventing module names. Preserve the six
Program ExeFS members as the executable-module universe and add evidence links
for the already-qualified identities. The next integration write should be
bounded to:

1. record `p0-base-import-relocation-inventory.md` as the direct closure of
   base structural imports/relocations; keep dependency identity, loader
   binding, semantic coverage, and the incomplete `rtld` function inventory
   distinct rather than treating synthetic-ELF zero rows as zero;
2. add the update package ContentMeta/CNMT reconciliation gap if it is intended
   to be part of P0 package scope;
3. retain the effective overlay counts and the 466/725 group totals, while
   keeping internal diffs, added provenance, ownership, and runtime as unknown;
4. mark the capped function-progress manifest and legacy reports as superseded
   by fix2 without promoting export/index states to analysis, behavior, or
   binary matching; and
5. keep the 17 subsystem labels as discovery seeds only, explicitly listing the
   unmapped groups and the plan's broader uncensused areas (combat, AI, audio,
   UI, progression, and any other systems) as outside current evidence.

No canonical file was changed by this audit. No Ghidra process or `gamedb index`
was run.
