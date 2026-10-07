# P0 Wave A1 — bounded census and discovery frontier

**Result:** The current 26 evidence records can be reconciled to the declared
base-v0 + update-v262144 package, reviewed Program/ExeFS, five-role executable,
and effective **outer** RomFS boundaries. This is a metadata/inventory baseline,
not a complete game census or an authenticity, semantic, runtime, or port-parity
claim. The update is a patch; the port target remains base plus update. This pass
read sanitized reports and the canonical census/plan only, not package-manifest
JSON, game payloads, keys, credentials, or sensitive configuration.

## Finite sources, enumeration, and stop rule

| Layer and unit | Bounded observation | Census link and direct report evidence | Limit |
|---|---|---|---|
| Package roots | The pinned base package has **6** root PFS0 entries; the pinned update has **7** reported root entries. The base root has 4 NCA, 1 NCZ, and 1 XML; its Application v0 ContentMeta XML declares five content classes. | `base_package_contentmeta`, `base_main_package_metadata_probe`, `update_contentmeta_cnmt`, `scope_discovery_exclusion_method` in `sheets/re/p0_scope_census.tsv:31,44-45,49`; `p0-base-v0-module-provenance.md:24-39`; `p0-wave8-full-scope-audit.md:20-32` | Root-entry counts are not content-semantic or NCA-authenticity denominators. Update CNMT rows are opaque/Unknown; no update parse or base-value substitution is claimed. |
| Reviewed Program/ExeFS | **Six members per version:** `main`, `main.npdm`, `rtld`, `sdk`, `subsdk0`, `subsdk1`. Five are NSOs; NPDM is process metadata. The base Program-to-ExeFS chain and 15/15 base NSO segment hashes are documented. The update replaces `main` and `main.npdm`; the four auxiliary NSOs are byte-identical to base. | Census module/package/NPDM rows `:28-35,41-46`; `p0-base-v0-module-provenance.md:24-80,142-154`; `p0-wave8-full-scope-audit.md:24-25`; `p0-update-main-npdm-extraction.md:22-40` | This is the reviewed Program member set, not all package content or runtime loading. NPDM extraction/hash check is complete; crypto trust and runtime necessity remain Unknown/Deferred. |
| Executable roles | Exactly five named roles are covered per version: `main`, `rtld`, `sdk`, `subsdk0`, `subsdk1`. Base `main` is provenance-only for the port; the update `main` replaces it. Direct original-NSO structural imports/relocations are inventoried for all five named base inputs. | Census `base_main_identity`, `update_main_identity`, four auxiliary rows, and dependency rows `:28,30,32-35,41-43`; `p0-base-import-relocation-inventory.md:1-26,72-93`; `p0-final-bounded-scope-matrix.md:56-62` | Identity and static relationships do not prove provider binding, reachability, complete valid-function counts, or semantics. |
| Effective **outer** RomFS | Base **18,370** outer paths; effective virtual base+update **19,095** outer paths = **17,904 unchanged + 466 modified + 725 added + 0 removed**. One modified path has equal size but different hash. | Census `base_content_index`, `update_content_overlay`, effective script/asset rows `:36-39,47-48`; `p0-base-update-overlay.md:34-46,83-101,133-158`; `p0-wave8-full-scope-audit.md:26` | The update's zero standalone extracted RomFS rows reflect BKTR patch representation, not zero effective content. Outer-file counts must not include archive children. |
| Nested archive members | Base-only SARC index records **257 archives / 3,416 indexed children**. | Census `base_content_index` and `effective_asset_format_inventory` `:36,48`; `p0-wave9-scope-inclusion-matrix.md:20`; `p0-final-bounded-scope-matrix.md:70` | Effective nested-child denominator and semantics are Unknown/Deferred, outside the outer-file baseline. Do not add children to 19,095. |

The discovery sources are the pinned package/provenance reports, reviewed
Program/ExeFS member sets, original-NSO structural inventories, versioned RomFS
path inventories and hash-verified overlay report, and the existing 26-record
census with its cited sanitized format/parser/index reports. Enumerate root
members as package entries, reviewed ExeFS names as members, NSOs by the five
version-qualified roles, and each effective outer RomFS path **once** under its
overlay disposition. Record named metadata/format families only where directly
evidenced; otherwise retain `Unknown` or an unclassified remainder. This is the
rule in `odd/PLAN.md:432-448,457-468` and the earlier bounded matrix
`p0-final-bounded-scope-matrix.md:107-114`.

**Stop condition:** Reconcile those finite source sets and give every census
dimension an evidence-backed status, reason, limit, and later phase where
needed. Do not silently exclude an uninspected category or expand the finite
boundary into nested members, external services, semantic ownership, runtime
reachability, or undiscovered game systems. The 26 records are status/evidence
records, **not** a completeness denominator (`odd/PLAN.md:443-468`). User-reported
emulator use is context only, not a build-qualified trace
(`p0-bounded-closure-audit.md:19-22`).

## Reconciliation and remaining wording work

The 26 rows are present at `sheets/re/p0_scope_census.tsv:24-49`: 21 records
outside the five Wave 8 scope additions, plus `update_contentmeta_cnmt`,
`update_main_npdm_proof`, `effective_script_inventory`,
`effective_asset_format_inventory`, and `scope_discovery_exclusion_method`
(`:45-49`). Their coverage is intentionally overlapping evidence, not 26
disjoint scope units. Package/ExeFS/module identities, outer overlay, base-only
script/archive evidence, effective script/asset `Unknown`s, and deferred
CNMT/NPDM questions all have named records. No positive evidence establishes
that unenumerated package-content semantics, effective nested members, all
script/asset format families, or external service inputs are absent.

**Writer attention:** Several canonical `next_action` fields still read like
superseded P0 work even though the bounded-scope decision explicitly defers
them: `base_package_contentmeta` and `base_main_package_metadata_probe` direct
NCA/CNMT verification (`:31,44`); `update_content_overlay` directs all 452
internal comparisons and historical ownership (`:39`);
`effective_script_inventory` and `effective_asset_format_inventory` direct
effective per-file joins (`:47-48`). Update the active wording to preserve these
as `Unknown`/`Deferred` with reason, limit, and later phase, not P0 exit gates.
Do not erase the evidence or invent an exclusion. The plan's bounded addendum
already supersedes those actions (`odd/PLAN.md:450-500`).

This read-only review does not certify the other P0 gates, sheet validation, CI,
or fingerprints. No gameDB index, Ghidra process, treemap regeneration, ledger
state promotion, `sudo`, commit, or push was performed.
