# P0 Wave 7 — Scope and ownership audit plan

**Audit date:** 2026-10-06. **Mode:** read-only planning; no canonical document,
sheet, function ledger, or game payload was modified. No Ghidra or gameDB
commands were run. The target remains the mandatory base v0 plus update v262144
overlay; the update is a patch, not a standalone program.

## Current evidence boundary

The live census contains **21 data records**, not 20: it has 21 IDs after the
header. `odd/PLAN.md` §P0's 2026-10-06 integration addendum correctly says 21,
but the immediately preceding summary at lines 155–156 says 20 and is stale.
This audit does not propose a new denominator or declare the census complete.

Directly supported, scoped facts include the effective overlay arithmetic
(19,095 entries: 17,904 unchanged / 466 modified / 725 added / 0 removed),
14/466 successful internal structural comparisons, and a successful local
comparison against base payload hashes for all 725 additions (33 exact matches,
692 non-matches). None of those is semantic ownership. Parser acceptance,
compile acceptance, labels based on paths/extensions/strings, exact hash
matches or non-matches, and gameDB name edges remain their own weaker evidence
classes. Ownership is still **Unknown for 1,191/1,191** delta entries; runtime
loaded/reached is Unknown, not a negative result.

The fix2 profile is currently marked `current`; its manifest identifies the
pinned update archive and inventory and records the Rust-source fingerprint.
It covers only 153,476 located update-main functions (22 documented/analyzed,
8 partial implementation, 0 behavior-verified, 0 binary-matched) and is not a
complete valid-function or whole-game denominator. The successful global
gameDB index covers 177,795 available C export files in six roots; it does not
measure semantics or complete module inventories. Its old attempt narratives
in the same report are historical; use the final successful addendum.

## Open-category task matrix

The O labels below follow `p0-wave1-reconciliation.md`; where a later report
narrows or supersedes a task, that current state is called out. “Parallelizable”
means independent local evidence work can proceed concurrently with isolated
outputs, not that the category can necessarily be closed. All paths in the
last column are proposed, unique report outputs for bounded follow-up agents;
this audit itself does not create them.

| Category | Direct evidence already done (and class) | Missing proof / remaining scope | Parallelizable? | Tool / prerequisite | Bounded agent task | Exact output report path |
|---|---|---|---|---|---|---|
| **O1 — loader binding, provider identity, load order, loaded/reached** | Original-NSO imports/relocations structurally inventoried for five base modules; update five-module dynamic metadata and exact-name consumer/provider overlaps measured. `main` has 3 declared `DT_NEEDED`; nine name-overlap edges are candidate edges only. | Actual loader provider/binding and load order; loaded modules and reached dependencies from a qualified execution. Static import/name matches do not show either. | **Partly.** Static edge reconciliation is local; runtime closure is blocked. | Local metadata/source-reference tracing now. Runtime requires all three together: identifiable launchable runtime, verified loader-ready base+update pair, and module/bind/instruction/file-access instrumentation. Current feasibility report found none in checked locations. | Produce a sanitized static edge ledger distinguishing declared/imported/candidate-provider edges and unresolved cases; separately record runtime prerequisites as unavailable without inferring failure. Do not launch or search outside authorized locations. | `reports/function-progress/p0-wave7-static-dependency-edges.md` |
| **O2 — complete semantic function inventory / function understanding** | Fix2 update-main inventory and exact executable-body union; listing/reference triage and semantic-query retry provide aggregate classifications only. Four auxiliary body unions/gaps are measured for existing Ghidra detections; global index and C exports count available files/rows only. | Function validity/boundaries in the 1,830,644-byte update-main body complement; semantic analysis; complete function denominators/coverage for base/auxiliary modules (notably `rtld`); no inference from detected/exported/indexed rows. | **Partly.** Existing reports/exports can be reconciled offline, but new native-analysis evidence is outside this task because Ghidra is prohibited. | Existing pinned reports, inventories, and sanitized exports only. Any later direct function-boundary/semantic claims require an authorized analysis environment and per-function evidence; not Ghidra in this Wave 7 task. | Cross-reference existing gap, inventory, export and index evidence; identify which claims are already bounded vs still unknown, and list an evidence-backed queue without inventing functions or changing the ledger. | `reports/function-progress/p0-wave7-function-scope-reconciliation.md` |
| **O3 — stale capped progress artifacts vs current fix2 view** | Current fix2 generated profile has generation status `current`; manifest archive SHA, inventory SHA/count (153,476), ledger SHA and Rust-source fingerprint are present and align with the latest profile report. Legacy capped profile is explicitly historical. | No current artifact-generation defect is evidenced. Recheck only if Rust sources or the evidence ledger change; do not claim the profile fills semantic/scope gaps. | **No open task now** (bounded artifact concern resolved); reopens only on input change. | Read-only manifest/status/fingerprint validation; regeneration only under project rule if sources/ledger change. | No agent task now. Retain this as a closed, scoped category and avoid legacy-profile refresh. | Existing evidence: `reports/function-progress/p0-fix2-treemap-profile.md`; no new report required. |
| **O4 — NCA header/signature/authenticity** | Package/member and Program→NCZ→ExeFS identity evidence is qualified; base ContentMeta XML parsed. Prerequisite audit checked local capability without opening credential material. | NCA header decode/field checks, independent trusted `sign1` verification, and authenticity result; update ContentMeta NCA remains opaque. Neither valid nor invalid has been established. | **No**, not to closure under current inputs. A non-secret prerequisite inventory can be updated locally, but verification is blocked. | Requires separately authorized in-memory header-decryption configuration and an independent trusted NCA signature verifier. No key discovery, key disclosure, or credential inspection. | Keep the blocker explicit; if authorized prerequisites become available, independently verify only pinned inputs in memory/scratch and report sanitized outcomes. | `reports/function-progress/p0-wave7-nca-prerequisite-status.md` |
| **O5 — update ContentMeta/CNMT semantic reconciliation** | Base ContentMeta parsed; update package member inventory identifies an opaque update ContentMeta NCA and absence of expected update XML. | Update CNMT semantic rows/counts and reconciliation to package manifest/Program/patch ownership. Unknown is not zero and cannot be copied from base. | **No**, until an authorized plaintext descriptor is available. | Local authorized plaintext XML/CNMT descriptor; no NCA decryption in this plan. If none is present, report blocked. | Recheck only the known descriptor locations and existing manifests; parse/reconcile if a permitted plaintext descriptor exists, otherwise record the precise blocker without accessing secrets. | `reports/function-progress/p0-wave7-update-cnmt-readiness.md` |
| **O6 — update `main.npdm` cryptographic validity/trust and runtime necessity** | Update `main.npdm` replacement disposition, extraction and hash verification are complete; bounded field comparison is equal outside opaque cryptographic regions. | Trust/signature validity and whether the update NPDM is required by the effective runtime/loader. A static byte diff/field parse proves neither. | **Partly.** Existing-file static consistency can be reviewed locally; cryptographic and runtime conclusions are blocked by missing qualified verifier/runtime. | Authorized independent trust/signature verifier for crypto; qualified loader/runtime pair plus instrumentation for necessity. The previously verified NPDM must not be re-extracted as a planned action. | Preserve the qualified static result and identify exact missing verifier/runtime evidence. Do not interpret opaque regions or re-extract. | `reports/function-progress/p0-wave7-npdm-proof-gaps.md` |
| **O7 — internal changes in 466 modified files/containers** | 14/466 successful structural comparisons: SARC 10/10 and GFLXPACK 4/10. Message, AHTB, BLUA and event audits give parser/compile outcomes and some positional metadata. | Reconcile each parser result to the exact modified-file cohort; 452/466 still have no successful paired internal comparison. Remaining formats require format-specific validated structure. Structural diffs do not establish meaning/owner. | **Yes.** Independent format/group lanes can run in parallel after/alongside an aggregate cohort reconciliation. | Local authorized extracted files and existing parsers, metadata-only bounded probes; no Ghidra/gameDB. Do not loosen AHTB or GFLXPACK rules without independent format evidence. | First produce intersections of file × format × parser attempt/side/acceptance × paired-comparison status; then bounded separate message/event and opaque-bin/data investigations. Keep unsupported counts explicit. | `reports/function-progress/p0-wave7-modified-overlay-coverage.md` |
| **O8 — provenance of 725 update-only paths** | Direct local hash comparison against all 18,370 base payloads completed: 33 exact matches, 692 non-matches, zero unreadable/inconsistent inputs. | Provenance history for both matching and non-matching additions; no match is not proof of genuinely new content, and a match does not identify ownership. Existing reports do not cross-tab exact-match status by group. | **Yes.** A local aggregate cross-tab / independent manifest-reconciliation lane can run without O7's parser changes. | Existing effective overlay inventories and bounded local digest/metadata outputs; no payload disclosure. | Cross-tab match status against the 11 bounded groups/extensions and reconcile to the 725 total; identify what provenance the evidence can and cannot support. Do not label 692 “new”. | `reports/function-progress/p0-wave7-added-provenance-cross-tab.md` |
| **O9 — direct file-to-code ownership and runtime use of 1,191 delta entries** | Overlay group/extension classification and candidate subsystem labels are available; static module dependency names/relocations are known at aggregate level. Existing ownership audit explicitly has no verified direct code reference or runtime file-access trace. | Direct resource-key/path/hash reference joined through the loader/consumer in the exact build for static candidate ownership; runtime file access tied to caller/module and scenario to prove use. Current hash/path labels, parser acceptance, SCC/string labels and gameDB name edges are heuristics. | **Yes for static candidate edges; runtime is hard-blocked.** | Existing authorized static exports/source/string/reference metadata for exact build. Runtime additionally needs O1's three prerequisites. Do not treat gameDB name-only edges as proof. | Trace identifiable static resource references to loader and consumer, classifying direct reference vs heuristic and recording module-relative source evidence; no claim of execution. Keep a separate runtime unknown column. | `reports/function-progress/p0-wave7-file-code-candidate-edges.md` |
| **O10 — full P0 scope / inclusion and exclusion census** | Census records package identity, five NSO modules, base RomFS/script/catalog, overlay delta, subsystem heuristics, global index, fix2 profile, gap-flow correction, and integration evidence. PLAN itself says census is not complete. | Discover and justify remaining scope/exclusions; effective-update script/catalog coverage, all systems/assets and modules must not be assumed from base-only indexes or 17 heuristic clusters. Dedicated explicit rows are absent for update ContentMeta/CNMT semantics and NPDM cryptographic trust/runtime necessity (they appear only embedded in broader integration prose/other records), and there is no dedicated census category for full scope-discovery methodology/exclusions. | **Yes.** Local report/plan/census reference reconciliation can proceed independently of hard-blocked crypto/runtime work. | Current census, PLAN, overlay manifest/classification and existing base-only script/catalog reports. Read-only only in this wave; any canonical census changes require a later integration task, not these agents. | Compare every census record to O1–O9 and the effective overlay; list uncovered P0 scope dimensions and evidence-backed inclusions/exclusions. Do not invent a denominator or edit canonical files. | `reports/function-progress/p0-wave7-census-coverage-audit.md` |

## Census and active-wording discrepancies

The user-supplied “20-record census” description is stale for the checked-in
file: the census now has **21** record IDs. The plan's line 155 assertion of 20
records is also stale; its earlier line 18 and the census itself say 21. This is
a record count only, not a completeness denominator.

Explicit sub-scope still lacks its own census record: update ContentMeta/CNMT
semantics; NPDM signature/trust and runtime necessity; and the method for
discovering/documenting complete inclusions and exclusions. These topics are
mentioned in the broader integration/evidence rows and plan prose, but an
embedded mention is not an independently trackable census row. Also, the base
script index (800 records with one error) is base-only, and the asset-format
catalog is not tied to a versioned effective overlay. These are not proof that
the corresponding update/effective scope is absent; they expose the remaining
census work.

Active stale wording to avoid relying on:

- `odd/PLAN.md` lines 155–156 says **20** census records; the current file has 21.
- Historical text in `p0-wave1-reconciliation.md` O3 says current generated
  artifacts still contain only the capped 68,330-function view. The current
  fix2 profile and manifest supersede that artifact-status claim; semantic
  completeness remains open under O2.
- Historical early sections of `p0-global-gamedb-index.md` describe blocked or
  empty attempts. The final dated addendum records the successful retry; use
  that result, without upgrading it to semantic coverage.
- Historical `p0-overlay-ownership.md` §6 says 0/466 internal diffs and §9
  says to re-extract update `main.npdm`. Later addenda supersede the former
  (14/466) and current NPDM reports supersede the latter (already extracted and
  verified). The unchanged active conclusions are ownership Unknown and runtime
  Unknown.
- The 336 mapped / 389 unmapped added-label split is heuristic only; 360 is
  stale prose in the older changed-content classification report, not a current
  ownership metric.

## Priority and parallel execution

**Priority order:**

1. **Local and immediately actionable:** O10 census/scope reconciliation; O7
   modified-file cohort × parser-result reconciliation; O8 addition group ×
   hash-match cross-tab; O9 static direct-reference/loader candidate tracing;
   O2 reconciliation of already available inventories, gaps and export facts.
   These lanes must preserve the current `Unknown` states and can run in
   parallel with isolated report outputs.
2. **Format-evidence gated, still local:** continue O7 format-specific work
   only where a validated parser/rule exists. The six GFLXPACK rejects and ten
   AHTB failures remain unsupported pending the exact independent evidence
   already stated in their reports; parser acceptance elsewhere is not a reason
   to relax bounds.
3. **Hard prerequisite tracks, not on the local critical path:** O4 NCA
   header/signature verification and O5 update CNMT semantics await authorized
   verifier/decryption configuration or an authorized plaintext descriptor.
   O1 runtime resolution and O6 NPDM runtime necessity await the runtime +
   loader-ready base/update pair + instrumentation together. Keep loaded/reached
   unknown; no launch or credential operation is proposed.
4. **O3 is not queued:** fix2 artifact generation is current and its manifest is
   scoped correctly. Revalidate only after an input change.

**Ready-to-run parallel lanes: 5** — O10 census coverage; O7 modified overlay
reconciliation; O8 added-path provenance cross-tab; O9 static ownership
candidate tracing; O2 existing-evidence function-scope reconciliation. Their
outputs must be separate, metadata-only reports. **Hard-blocked proof gates: 3**
— NCA header/signature authenticity (O4), update CNMT semantics (O5), and
qualified runtime binding/use plus NPDM runtime necessity (O1/O6). These form
two coordinated blocker tracks (NCA/CNMT and runtime), not a claim that only
three O categories are open. **Open categories: 9 of O1–O10**; O3 is closed for
the bounded generated-artifact concern, while O2's semantic scope remains open.

No agent has been dispatched and no parallel work executed in this audit.
