# P0 Wave 8 — Full-scope census reconciliation

**Date:** 2026-10-06. **Mode:** read-only metadata audit. Scope is the mandatory
base v0 plus update v262144 overlay; the update is a patch, not a standalone
program. This report is not a claim that the current census is complete. No
canonical source, census, plan, README, or function ledger was changed. No game
payload was read. No Ghidra or gameDB command was run.

## Result

The current census has **21 records** (IDs in `sheets/re/p0_scope_census.tsv`,
rows 14–34). They represent executable identity/inventory, selected package and
RomFS dimensions, export/index summaries, overlay evidence and selected gap
triage. Comparing those records with the exact local package manifests and the
qualified ExeFS/RomFS/overlay evidence exposes untracked scope dimensions and
missing denominators. The count 21 is a census-record count, not a scope
denominator. Neither package manifests nor the overlay manifest prove complete
semantic, functional, or runtime coverage.

## Manifest reconciliation

| Layer | Exact metadata scope evidenced | Census coverage and limit |
|---|---|---|
| Package roots | Base manifest: 6 package entries; update manifest: 7. Base package metadata records five content-row classes; the update package manifest records its program content and package metadata members, but update CNMT semantics remain unavailable. | `base_package_contentmeta` covers base identity/partial row reconciliation. There is no dedicated census record for update CNMT semantics or non-program package-content reconciliation. Package root entries are not equivalent to fully inspected content. |
| Program ExeFS | Exactly 6 members in each version. Five are NSOs; the sixth is process metadata. Base v0 provenance verifies all five NSO identities and 15/15 segment hashes. Update evidence verifies the five NSOs and establishes replacement of `main` and process metadata; four auxiliary NSOs are byte-identical across versions. | Census records cover the five code roles via `base_main_identity`, `update_main_identity`, and four auxiliary module rows. Process metadata appears only inside broader evidence; no dedicated census record tracks its trust/crypto and runtime-necessity gaps. No evidence supports an additional unnamed Program NSO in the reviewed ExeFS sets. |
| RomFS / effective overlay | Base inventory: 18,370 entries. Update virtual inventory/effective overlay: 19,095 entries = 17,904 unchanged + 466 modified + 725 added; 0 removed. The entry-set and common-file hash comparison supports this arithmetic. | `base_content_index` and `update_content_overlay` cover aggregate counts, not a complete per-file effective manifest joined to semantics/owners. All 1,191 delta entries lack established semantic ownership. Of 466 modified files, 14 have successful internal structural comparisons and 452 do not. |
| Fix2 update-main | Exact profile denominator: 153,476 located function rows / 51,275,676 body bytes; executable text is 53,106,320 bytes, with a separate 1,830,644-byte residual outside inventoried bodies. Exports: 153,471 C plus 5 assembly fallbacks. | The fix2 report/manifest are current for the exact pinned update archive, inventory, ledger and Rust fingerprint. This is only located update-main scope, not a complete valid-function denominator or whole-game scope. |

The two package root entry counts above are manifest-level counts only. They do
not convert opaque or unparsed package members into absent content, and they do
not close update ContentMeta/CNMT meaning, NCA header/signature validation, or
non-Program payload scope.

## What the evidence stages establish

| Stage | Supported claim | Explicit non-claim |
|---|---|---|
| Extracted / identity-qualified | Pinned package/member identities, bounded NSO segment verification, and the specified local extracted or virtual inventories are documented. | Extraction or a local identity match does not prove all package semantics, ownership, or runtime use. A zero standalone update-RomFS extraction count reflects patch representation, not zero effective update data. |
| Inventoried | Package entries, ExeFS members, RomFS counts/delta, detected function rows and measured body ranges are recorded for their stated scopes. | Inventory is not automatically exhaustive at another layer. Ghidra detections and current-body range unions do not establish all valid functions. |
| Exported | Update-main has the stated C and assembly outputs; auxiliary exports cover attempted detected functions in the cited successful runs. | C/assembly export is not semantic analysis. Auxiliary candidate counts and successful export attempts have no complete-function denominator. |
| Indexed / parsed | The global gameDB run indexed all 177,795 currently available staged C files across six roots: 176,667 parsed function rows; 1,128 indexed files have no parsed row. Update-main-only profile records 153,471 indexed files, 152,634 parsed function rows and 837 with no parsed row; five assembly fallbacks are outside that index. | These are corpus/file/parser results, not whole-module inventory, analysis, behavior, or evidence of semantically empty files. |
| Internally compared | Overlay set/hash comparison accounts for 19,095 effective entries. Successful internal structural comparisons cover 14/466 modified entries (SARC 10/10, GFLXPACK 4/10). Exact-base hash comparison of additions reports 33 matches and 692 non-matches. | Structural acceptance is not semantic understanding. Hash non-match does not prove new provenance; 14 comparisons are not coverage of the remaining 452. |
| Semantically understood / implementation | Fix2 evidence ledger reports 22 update-main functions documented/analyzed and 8 with partial implementation. These are independently recorded states. | Remaining functions stay unknown; exported, indexed, parsed, identified or tested rows do not upgrade analysis or implementation. |
| Behavior-verified / binary-matched | Fix2 manifest records 0 behavior-verified and 0 binary-matched update-main functions; all 153,476 in each dimension remain unknown. | No runtime launch/trace is evidenced. Loaded, reached and executed remain unknown, not false or zero. |

The fix2 manifest is `current` and matches the recorded pinned archive,
inventory, evidence-ledger digest and Rust-source fingerprint. This validates
artifact freshness and profile scope only; it does not repair the missing
whole-game/function denominators.

## Census records and scope not independently represented

The existing rows collectively cover the following record families: global C
index; update-main fix2 progress and byte-gap summaries; base package identity;
five executable module roles across base/update; base content/script/format
indexes; effective update overlay and heuristic subsystem labels; auxiliary
dynamic/dependency metadata; and a base-main pin reconciliation. The 21 records
are not 21 exhaustive scope units, and shared mentions do not create a
trackable denominator for the mentioned dimensions.

The following scope groups are absent as distinct, reconciled census units or
remain materially partial against the manifests:

1. **Update package metadata and non-program package contents.** Base ContentMeta
   has partial reconciliation; update CNMT is opaque/unknown. Package-root
   sidecar and non-program content rows are not individually reconciled to a
   semantic inventory. Package presence is known; completeness of their meaning
   is not.
2. **Effective per-file data universe.** The overlay totals reconcile, but the
   census does not provide a single per-file manifest joined to source/version,
   format/parser result, subsystem, provenance, owner, and runtime state. The
   base script index has 800 records (799 successful, 1 error) and is not an
   effective base+update script denominator. The format catalog is not tied to
   a versioned effective overlay.
3. **Modified-file internal coverage.** The exact cohort is 466; only 14
   successful paired internal comparisons are supported; 452 lack successful
   comparisons. Parser outcomes for selected formats do not substitute for
   pairwise coverage.
4. **Added-content provenance and ownership.** The 725-entry denominator is
   exact; hash comparison yields 33 exact matches and 692 non-matches, but neither
   status resolves new-versus-relocated provenance. Ownership remains unknown
   for 1,191/1,191 modified-plus-added entries; directory/extension labels are
   heuristics.
5. **Complete executable function scope beyond update-main.** Base-main has
   68,412 historical capped detections, not a complete function denominator.
   Auxiliary exports/detections (including run-to-run count variance) have no
   complete reconciled function denominator. `rtld` lacks a complete reconciled
   function inventory. The global C index denominator is available staged files,
   not complete module functions; 1,128 indexed C files have no parsed function
   row and five update-main assembly fallback functions are not indexed.
6. **Update-main residual and candidate validity.** The exact residual is
   1,830,644 executable bytes outside existing bodies, and listing/reference
   metadata classifies ranges/seeds only. Whether residual bytes contain
   additional valid functions, and their semantics/reachability, remains
   unknown; do not mint functions from references or code/data labels.
7. **Other game systems and runtime data flow.** Heuristic subsystem clusters
   and changed/added path groups are not an exhaustive system census. Combat,
   AI, audio, UI, progression, and any other systems have no complete inclusion
   or exclusion evidence in these manifests. Runtime file access, module load,
   binding, and reachability have no observed denominator or positive/negative
   result.

## Denominator ledger

| Dimension | Known denominator | Status |
|---|---:|---|
| Census records | 21 records | Count known; completeness denominator **Unknown**. |
| Package root entries | Base 6; update 7 | Manifest entry counts only; content semantics/inspection denominator **Unknown**. |
| Program ExeFS | 6 members per version (5 NSOs + process metadata) | Member-set denominator known for reviewed Program ExeFS; semantic/module-function denominators **Unknown**. |
| Effective RomFS | Base 18,370; update virtual 19,095; delta 1,191 | Entry totals known; per-file semantic/owner/runtime coverage **Unknown**. |
| Modified internal comparisons | 466 | 14 successful, 452 without successful comparison. |
| Added provenance hash comparison | 725 | 33 exact matches, 692 non-matches; provenance conclusion **Unknown** for non-matches and ownership **Unknown** for all. |
| Update-main fix2 inventory | 153,476 located functions / 51,275,676 body bytes | Exact profile denominator, but complete valid-function and whole-game denominator **Unknown**. |
| Update-main executable text | 53,106,320 bytes | Known byte scope; 1,830,644 outside existing bodies, not function rows. |
| Base-main / auxiliary valid functions | **Unknown** | Historical/detected/exported counts are not complete denominators. |
| Available staged C index | 177,795 files, six roots | Exact available-file corpus; complete source/export/function corpus **Unknown**. |
| Semantic understanding, behavior, binary matching, runtime loaded/reached | **Unknown** | Fix2 observed evidence: 22 analyzed, 8 partial implementation, 0 behavior-verified, 0 binary-matched; runtime not observed. |

## Evidence links and bounded next actions

Evidence supports exact package identities and bounded ExeFS/RomFS membership,
selected structural inventories, the hash-qualified overlay arithmetic, and
the current fix2 artifact's scoped denominator. It does **not** support a claim
that the 21-record census lists every scope category, that all functions/data
were semantically covered, or that any unobserved module/file/function is
absent.

References:

- `sheets/re/p0_scope_census.tsv` — current 21 records and per-record unknowns.
- `odd/PLAN.md` — current P0 scope and explicit statement that the census is
  partial; use the latest dated integration addendum where older prose conflicts.
- `reports/function-progress/p0-base-v0-module-provenance.md` — base package to
  Program/ExeFS identity and five base NSO identities.
- `reports/function-progress/p0-ownership-and-npdm.md` — exact six-member
  ExeFS set in both versions and process-metadata replacement.
- `reports/function-progress/p0-base-update-overlay.md` and
  `reports/function-progress/p0-update-changed-content-classification.md` —
  effective RomFS totals/groups; classification remains heuristic.
- `reports/function-progress/p0-base-import-relocation-inventory.md` and
  `reports/function-progress/p0-update-aux-ghidra-inventory.md` — five-module
  structural evidence, not provider/runtime proof.
- `reports/function-progress/p0-auxiliary-module-export-inventory.md` — current
  auxiliary export results and unknown complete-function denominator.
- `reports/function-progress/p0-fix2-treemap-profile.md` and
  `reports/function-progress/update-v262144-fix2/manifest.json` plus
  `generation-status.json` — exact profile scope, state counts and freshness.
- `reports/function-progress/p0-wave7-scope-and-ownership-plan.md` — prior
  bounded audit plan; its O2/O10 work remains evidence reconciliation, not a
  complete-scope assertion.

Bounded follow-up, without changing canonical files in this task:

1. Create a separate, version-qualified package-content matrix for both package
   roots and the Program ExeFS, marking each content class as identity-qualified,
   opaque, or semantically reconciled; leave unavailable update CNMT semantics
   **Unknown**.
2. Reconcile one effective RomFS per-file manifest to overlays and existing
   parser/hash evidence. Start with the 452 modified entries lacking successful
   internal comparison, then cross-tab the 725 additions by group without
   calling non-matches “new”. Keep runtime and ownership separate.
3. Define explicit function denominators only after a complete, documented
   inventory method exists for base-main and each auxiliary role; reconcile the
   update-main residual independently. Do not use export/index totals as a
   substitute and do not add function rows without qualifying direct evidence.
4. Add a future census inclusion/exclusion reconciliation for unrepresented
   systems/data families; no absence claim until each candidate universe has a
   bounded manifest and documented exclusion rationale.
5. Keep loader/provider binding and file/module runtime observation as separate
   prerequisite-gated tracks. Until qualified runtime evidence exists, retain
   loaded/reached as **Unknown**.
