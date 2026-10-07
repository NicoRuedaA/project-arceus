# P0 bounded-closure audit

**Date:** 2026-10-06. **Mode:** read-only evidence audit and proposed closure gate.
Target scope is the mandatory base v0 plus update v262144 overlay; the update is
a patch, not a standalone program. This audit proposes a bounded P0 definition;
it does not declare P0 closed. No game payload, key material, Ghidra, gameDB
command, or runtime experiment was used.

## Scope decision

P0 can close as a **bounded preparation-and-census phase**, not as proof of
whole-game semantic or runtime completeness. Closure should require a finite,
versioned category register with explicit inclusion/exclusion/unknown status,
evidence, reason, denominator scope, and next phase for every discovered
category. Unknown is acceptable only when the evidence and reason are recorded,
and the unresolved question is explicitly deferred. New categories uncovered
while reconciling the register must be added before the gate passes.

**User-reported operational context:** the user reports having used the ISO in
an emulator. This is not a build-qualified observation in the reviewed evidence:
it proves neither the exact base/update build nor which module or file loaded or
was reached. Record it only as reported context, not as a runtime trace.

## Minimal bounded closure matrix

| P0 dimension | Evidence already present | Minimum remaining bounded local work | Acceptable Unknown / deferred state | Exit condition |
|---|---|---|---|---|
| **Input identity and scope boundary** | Pinned base/update package identities; qualified base Program-to-ExeFS chain; five base NSO identities and segment hashes; update `main` identity; package/ExeFS member sets; current 26-row census and Wave 9 14-dimension candidate checklist. | Reconcile the census against package roots, Program ExeFS, effective outer RomFS, nested members, scripts/assets, executable roles, and non-RomFS metadata. Define discovery sources, enumeration rule, boundary, and stopping rule; mark every candidate included, excluded with affirmative evidence, unknown, or blocked. Keep the candidate checklist open to newly found dimensions. | Unbounded undiscovered universe remains Unknown until the documented bounded discovery method has completed; out-of-scope claims require positive boundary evidence. | Every category exposed by the declared method is represented once, has status/reason/evidence/denominator scope, and no unenumerated category remains in the declared boundary. Do not claim all possible game scope. |
| **Package and member provenance / overlay arithmetic** | Base package chain and NSO provenance qualified; effective overlay is 19,095 outer entries = 17,904 unchanged + 466 modified + 725 added + 0 removed; 33 additions byte-match a base payload and 692 do not. Cross-tab exists by coarse group and extension. | Record those counts and their distinct units in the census; reconcile the per-file effective manifest to the overlay classification and group/extension partitions. Keep non-matches labelled “no exact base hash match,” not “new.” Mark every modified/added cohort and parser result without conflating attempts and entries. | Added historical provenance for the 692 nonmatches, semantic ownership for all 1,191 delta entries, and the 452 modified entries without a successful internal comparison remain Unknown/deferred. Exhaustive per-container semantic diff is not needed for P0 closure. | Arithmetic reconciles exactly; each outer-file row has effective disposition/provenance class and evidence or explicit Unknown reason/next phase; no parser outcome is presented as semantic ownership or total delta coverage. |
| **Executable module identities and static imports** | Five module roles identified. Base-v0 original-NSO imports/relocations directly inventoried for all five modules. Update auxiliary structural metadata and five-module name-overlap candidate probe exist; `main` declares 3 `DT_NEEDED`; four update auxiliaries are byte-identical to base. Candidate edges are explicitly static-only. | Normalize census records so each module role/build is covered by pinned identity, extraction/conversion limits, direct original-image import/relocation inventory status, and static candidate status. Cross-reference the five-module reports; distinguish declared, referenced, candidate-provider, loaded, and reached. Do not treat synthetic ELF zero imports as original-NSO absence. | Provider identity/binding, load order, loaded/reached dependencies, and runtime use are P2-deferred Unknown. User-reported emulator use does not change these states. | All five base and update module roles have identity and static inventory status (complete, partial, or Unknown with reason); imports/candidates are recorded without claiming loader binding or runtime use. |
| **Function inventory, export, and index** | Fix2 update-main profile is current for exact pinned archive/inventory/source fingerprint: 153,476 located functions, 153,471 C + 5 ASM; exact body union 51,275,676/53,106,320 bytes. Global gameDB retry succeeded over 177,795 available C exports across six roots; 176,667 parsed rows; selftest 87/87. Base `main`/auxiliary completeness and valid-function denominators remain unknown. | Reconcile census counts to the final successful global-index addendum and current fix2 manifest, labeling each metric's unit/scope and separating inventory, export, index file, parsed function row, and body-byte coverage. Record base `main` as provenance-only, not pending port work. Preserve auxiliary snapshot/export mismatches and update-main residual as explicit gaps, not inferred functions. | Complete valid-function denominators for base `main`, auxiliaries, and update-main residual; function semantics, behavior, and matching remain Unknown/deferred. 1,128 indexed C files without parsed rows are parser/index outcomes, not empty functions. | Every module role has a bounded inventory/export/index status and evidence; exact denominator limitations and residuals are explicit; no claim of whole-game or complete valid-function coverage. No function-progress state promotion is implied. |
| **Effective per-file data groups** | Exact outer-file overlay; 11 coarse update-delta groups; existing base-only script index and format catalog; Wave 8 parser cohort matrix; Wave 9 added-hash cross-tab; O9 found 118 matching string data units, 0 exact-path xrefs and 97 normalized-only xrefs across 52 functions. These references are candidates, not ownership. | Build or reconcile a bounded per-file metadata join for effective outer entries: disposition, group/extension/format when established, applicable parser result, provenance class, and source citation. Explicitly separate outer files from nested archive members. Reconcile effective scripts and asset-format rows to the 19,095-entry manifest; count unclassified/unmatched remainder. Do not require interpreting every payload. | Semantic owner, file-to-code ownership, loader/file-open path, runtime access, and nested-member completeness remain Unknown/deferred. Base-only 800-script and unversioned catalog totals are not effective denominators. | Every outer entry belongs to one reconciled disposition and a known group or explicit unclassified/Unknown bucket; overlapping format/parser views are not summed. Every missing join has reason/evidence/next phase. |
| **NPDM disposition** | Update `main.npdm` has been extracted, hash-verified, and structurally compared in bounded fashion; update replacement disposition is established; comparable non-cryptographic fields match. | Change census/plan phrasing that still lists NPDM extraction or disposition as pending. Keep disposition evidence linked and separate it from trust/necessity. | Opaque signature/key validity and runtime necessity remain Unknown/deferred; no verification material is sought. | The record says disposition/extraction/comparison complete and only names residual trust/runtime questions as deferred. |
| **Known unknowns / deferrals register** | Existing reports retain explicit unknowns for NCA/CNMT, runtime, ownership, function denominators, function semantics, and data scope. | Consolidate each residual into a status, reason/evidence, and phase owner; correct stale next-action wording so deferred topics are not represented as P0 closure work. | Allowed when explicitly scoped and not represented as zero, false, excluded, or failed. | No unreasoned Unknown remains; every deferral is deliberately out of P0 and traceable to a later phase or prerequisite. |

## Explicitly out of the P0 closure gate

- **NCA signature/authenticity:** no verification is required for this bounded
  P0 close. Its technical status remains unverified/Unknown. Do not read keys or
  seek verifier material.
- **Update ContentMeta/CNMT internal decryption or semantic parsing:** not a
  P0 gate; status remains Unknown, with no inference from base metadata.
- **Runtime behavior, loaded/reached dependencies, and file access:** P2-deferred.
  Static imports and path references are candidate evidence only.
- **Semantic understanding of functions; complete whole-game or valid-function
  denominators; function-to-file ownership:** later analysis/coverage phases,
  not P0 closure requirements.
- **NPDM cryptographic signature/key validity and runtime necessity:** defer;
  NPDM file disposition is already closed.

## Census and wording changes needed before closure

The current census has **26 records**, but Wave 9 explicitly says its 14
candidate dimensions are a checklist, not a complete universe. The minimum
integration is therefore not “add every proposed row blindly”; it is reconcile
the present rows and candidates under a declared boundary, then add only
independent dimensions not already represented. At minimum:

1. Strengthen `scope_discovery_exclusion_method` with discovery inputs,
   enumeration/stopping rule, boundary, status, and evidence-backed exclusions;
   do not label uninspected categories excluded.
2. Keep `update_contentmeta_cnmt` as explicit Unknown/deferred, not a closure
   blocker. Keep `update_main_npdm_proof` but split/update its fields so closed
   replacement disposition is distinct from deferred crypto validity and
   runtime necessity.
3. Ensure dedicated, reconciled census coverage for effective scripts, effective
   assets/formats, and nested archive members; the current script and format
   rows are base-only or unresolved. Add or link an effective outer-file
   reconciliation and unclassified remainder without duplicating per-function
   rows or byte weights.
4. Ensure each of five executable roles has an unambiguous static identity/import
   inventory link and bounded function/export/index status. Preserve base `main`
   as provenance only and retain unknown function denominators.
5. Update `odd/PLAN.md` to replace its active P0 task list requiring runtime
   binding, NCA/CNMT, all 452 inner comparisons, 725 historical provenance,
   semantic ownership, and semantic gap/candidate work. State that P0 closes on
   the bounded census/evidence gate above and that those topics are deferred.
   Update both README status summaries in sync: do not call deferred crypto,
   runtime, semantic ownership, or complete-function denominators “P0 open
   blockers.” Preserve accurate technical Unknowns. Keep the required base +
   update language.

## Recommended gate and minimum remaining work

**Recommended gate:** close P0 only when the finite bounded-scope register is
reconciled and has no unenumerated categories within its stated boundary; all
included evidence and exclusions are cited; every Unknown has a reason and
next-phase disposition; overlay, module/import, bounded function/export/index,
and per-file data-group counts reconcile; and `PLAN.md` plus both README files
state the same P0 boundary. This gate does not require resolving every Unknown.

**Minimum remaining work:** (1) reconcile the 26 census records and Wave 9
candidate dimensions against a written discovery boundary/stopping rule; (2)
finish the effective per-file join at metadata level, including scripts/assets,
groups, parser status, unknown remainder and nested-member boundary; (3) normalize
the five-role static module/import and bounded inventory/export/index entries to
current authoritative reports; (4) relabel crypto/CNMT/runtime/semantic/ownership
unknowns as explicit deferrals and correct NPDM status; (5) synchronize PLAN and
README English/Spanish wording. Until those steps pass, P0 remains open under
this proposed bounded definition.

## Evidence reviewed

`odd/PLAN.md`; `sheets/re/p0_scope_census.tsv`; `README.md`; `README.es.md`;
`p0-wave8-census-audit.md`; `p0-wave8-full-scope-audit.md`;
`p0-wave9-scope-inclusion-matrix.md`; `p0-function-scope-matrix.md`;
`p0-base-v0-module-provenance.md`; `p0-base-update-overlay.md`;
`p0-base-import-relocation-inventory.md`; `p0-global-gamedb-index.md`;
`p0-fix2-treemap-profile.md`; `p0-wave8-overlay-coverage-matrix.md`;
`p0-wave8-added-provenance-cross-tab.md`; `p0-wave9-static-file-code-triage.md`;
`p0-nca-npdm-analysis.md`; `p0-update-main-npdm-extraction.md`;
`p0-runtime-dependency-resolution.md`.

*Metadata/evidence references only. No canonical source, census, plan, README,
function ledger, or game data was changed.*

## Final boundary decision — 2026-10-06

This dated addendum records the user's final bounded-scope decision and
supersedes this audit's broader proposed discovery/reconciliation work where it
would expand P0 into crypto, runtime, semantic, or nested-member investigation.
It does not declare P0 closed. Keep P0 **In progress** until a reviewer verifies
that all five local closure gates in `odd/PLAN.md` pass.

**P0 in-scope baseline:** pinned base v0 and update v262144 package identities
and recorded member-set metadata; the six Program ExeFS members per version
(five NSOs plus NPDM); five named executable roles per version, version-qualified
NSO identities, and original-NSO structural imports/relocations; the effective
outer RomFS set (base 18,370; effective 19,095 = 17,904 unchanged / 466 modified
/ 725 added / 0 removed); available module function detections, exports, index
results, and body/gap metrics, with denominators `Unknown` unless complete; and
outer-file format classes/parser outcomes plus each census row's status,
evidence, limits, and next phase. The finite sources are the pinned/package and
ExeFS provenance reports, original-NSO inventory reports, versioned RomFS
manifest/overlay reports, parser reports, available-index/fix2 reports, and the
existing **26** census records. Census count is not a universe denominator; the
boundary is not a claim of whole-game semantics. Outer files and nested archive
members remain separate units. The update is a patch; the port target remains
base + update.

**Explicitly deferred outside P0; retain `Unknown`/`Deferred`, not verified:**
(1) NCA header signature authenticity; (2) full update CNMT semantic parsing;
(3) NPDM cryptographic trust and runtime necessity; (4) loaded/reached runtime
observations; (5) semantic function understanding, behavior, and matching;
(6) a complete valid-function denominator; (7) semantic file-to-code ownership;
and (8) nested archive/member semantics. Reasons and limits remain in the linked
sanitized reports and census. No update ContentMeta parse is claimed, no base
CNMT values are copied, and the user's reported emulator use is not an
instrumented trace. None of these deferred dimensions is a P0 closure gate.

*Metadata/evidence references only; no raw package metadata, payload, keys, or
credentials were read or added by this boundary decision.*
