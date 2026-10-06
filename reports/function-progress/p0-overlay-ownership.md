# P0 — Effective base+update overlay ownership audit

Generated: 2026-10-06T11:27:00Z (UTC). Status: **complete for metadata
classification; semantic ownership and runtime reachability remain unresolved.**

This is an evidence-only report. It contains no game bytes, pseudocode, raw
strings, keys, or asset payloads. The update is a patch: the playable target is
the base package with the update overlay applied, not the update package alone.

## 1. Build and overlay identity

| Artifact | Size (bytes) | SHA-256 / build identity |
|---|---:|---|
| Base package `pk1.nsz` | 2,334,586,382 | `00167d5e00bf7f5fca5311f984069497f79812cbbd03c32eb7f2fc1649af2acc` |
| Update package `pk2.nsz` | 52,657,467 | `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446` |
| Base ExeFS `main` | 31,755,066 | SHA `6f0e5f4a76a0f8b147540296522e02f0d03d591bfa53929d02cacc910ee994a0`; NSO ID `7fcad279539de183b25c11834fd4a030591cfe25000000000000000000000000` |
| Update ExeFS `main` | 31,882,976 | SHA `89fa2d715e13a1c311a47ee02b3cc858ccf72a2086ab9704a08a2ddc1b90f7d9`; NSO ID `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e000000000000000000000000` |

The effective RomFS inventory is 19,095 entries: 18,370 base entries plus
the update's replacements and additions. The package and `main` identity
anchors are recorded in `reports/function-progress/p0-base-update-overlay.md`
§1 and §2.

## 2. Overlay counts

| Effective class | Count | Denominator / basis |
|---|---:|---|
| Unchanged | **17,904** | Common base/update paths, SHA-256 equal; 17,904 / 19,095 = 93.8% of effective update entries |
| Modified | **466** | Common paths, SHA-256 different; 466 / 19,095 = 2.4% |
| — modified, size differs | 465 | TSV size delta |
| — modified, same size | 1 | Hash-verified anomaly recorded in the overlay manifest |
| Added | **725** | Update-only paths; 725 / 19,095 = 3.8% |
| Deleted | **0** | Base-only paths; 0 / 18,370 |
| **Effective total** | **19,095** | 17,904 + 466 + 725 |

The 1,191-entry audit population is **466 modified + 725 added**. The counts
were independently re-derived from `work/pla/pk1/romfs_files.tsv` and
`work/pla/pk2/romfs_patched.tsv`, adding the one same-size change identified
by `p0-base-update-overlay.md` §5. No game content was copied or printed.

## 3. Classification method and evidence boundary

1. Set membership came from the two local RomFS inventories. Modified means a
   common path with a size difference plus the manifest's verified same-size
   exception; added means update-only.
2. Each delta entry is covered by a bounded group keyed by `/bin/<domain>`,
   extension, and known path convention. The aggregate tables below sum to
   466 modified, 725 added, and 1,191 total delta entries.
3. Outer format labels use the committed `sheets/domain/asset_formats.tsv`
   catalog where applicable: `.arc`/SARC, `.gfpak`/GFLXPACK, `.bntx`/BNTX,
   `.blua`/Lua 5.3, `.tbl`/AHTB, and `.dat`/u32-prefixed. `.bin` entries are
   classified by path/domain convention only; this pass does not claim one
   common binary format for them.
4. Candidate subsystem labels are limited to a top-level directory matching a
   label in `sheets/domain/subsystems.tsv`. This is name/path evidence, not a
   code-to-file edge. No ownership is inferred from a filename alone.

## 4. Complete aggregate classification

### 4.1 Domain group × outer data/container type

| Bounded group | Modified | Added | Total | Outer type / available evidence | Candidate subsystem label |
|---|---:|---:|---:|---|---|
| `message` | 369 | 320 | **689** | `.dat`/`.tbl` message payload/table pairs; extension-level catalog evidence plus path convention | `message` name match only |
| `trainer` | 0 | 309 | **309** | `.bin` trainer data/party records; path convention only | Unknown |
| `haxe` | 17 | 74 | **91** | `.blua` compiled Lua 5.3 event scripts; catalog magic/loader evidence | Unknown (`haxe` has no cluster label) |
| `event` | 40 | 12 | **52** | `.bin` event/quest/trigger records; path convention only | `event` name match only |
| `appli` | 13 | 4 | **17** | 10 SARC `.arc` UI packages, 3 `.bin` UI/application records, 4 BNTX `.bntx` images | `appli` name match only |
| `archive` | 11 | 0 | **11** | 10 GFLXPACK `.gfpak` asset packs, 1 `.bin` archive index | `archive` name match only |
| `flagwork` | 7 | 0 | **7** | `.tbl` flag/work tables; path convention plus AHTB extension catalog | Unknown (`flagwork` has no cluster label) |
| `misc` | 5 | 2 | **7** | 6 application/config records, 1 play-report record; `.bin` path convention | Unknown |
| `field` | 0 | 4 | **4** | `.bin` encounter/spawn records; path convention only | Unknown |
| `pokemon` | 3 | 0 | **3** | `.bin` Pokémon data records; path convention only | `pokemon` name match only |
| `chara` | 1 | 0 | **1** | `.bin` character configuration record; path convention only | `chara` name match only |
| **Total** | **466** | **725** | **1,191** | Complete delta population | — |

The group table is exhaustive, not a sample. It is intentionally a bounded
classification rather than a list of proprietary file contents.

### 4.2 Extension aggregate

| Extension | Modified | Added | Total | Catalog status |
|---|---:|---:|---:|---|
| `.dat` | 189 | 160 | **349** | Extension-level u32-prefixed data label |
| `.tbl` | 187 | 160 | **347** | Extension-level AHTB table label |
| `.bin` | 53 | 327 | **380** | No single format asserted; path/domain classification only |
| `.blua` | 17 | 74 | **91** | Lua 5.3 bytecode / `mlua` loader identified |
| `.arc` | 10 | 0 | **10** | SARC container identified |
| `.gfpak` | 10 | 0 | **10** | GFLXPACK container identified |
| `.bntx` | 0 | 4 | **4** | BNTX image asset identified |
| **Total** | **466** | **725** | **1,191** | — |

The catalog supports an extension-level format label for 811 / 1,191
(68.1%) delta entries (`.dat`, `.tbl`, `.blua`, `.arc`, `.gfpak`, `.bntx`).
The remaining 380 / 1,191 (31.9%) are `.bin` entries whose more specific
classification is path-based.

### 4.3 Candidate subsystem-label coverage

Using only exact top-level directory/name matches against the 17-label
`subsystems.tsv` set:

| Population | Label match available | Unmapped | Denominator |
|---|---:|---:|---:|
| Modified | 437 (93.8%) | 29 (6.2%) | 466 |
| Added | 336 (46.3%) | 389 (53.7%) | 725 |
| **Delta** | **773 (64.9%)** | **418 (35.1%)** | **1,191** |

The mapped labels are `message`, `event`, `appli`, `archive`, `pokemon`, and
`chara`. A label match is only a domain discovery hint: the source sheet is
generated from update-main string/SCC clustering over functions, and does not
establish that any function reads a particular RomFS entry.

The earlier `p0-update-changed-content-classification.md` §3.1 prose reports
360 mapped added entries, but its listed label set and per-directory counts
sum to 336. This audit re-derived the path sets directly and uses the
reproducible 336 value; the discrepancy is retained as a source-manifest
limitation rather than silently treated as ownership evidence.

## 5. Ownership evidence categories

The following is a mutually exclusive outer-evidence classification of the
1,191 delta entries. It must not be read as semantic ownership.

| Evidence category | Count | Share | What is actually supported |
|---|---:|---:|---|
| Code module | **0** | 0.0% | No delta entry is an ExeFS module. Update `main` replacement is established at module level only. |
| Script | **91** | 7.6% | `.blua` format and loader are identified; exact reader/module owner is unknown. |
| Table/data | **1,064** | 89.3% | Message, event, trainer, field, flag/work, and Pokémon table/data groups; mostly path convention, with catalog support for `.dat`/`.tbl`. |
| Asset/container | **24** | 2.0% | 10 SARC, 10 GFLXPACK, and 4 BNTX outer formats; internal members and consumers are unknown. |
| Configuration/telemetry | **12** | 1.0% | Application/UI records, archive index, app configuration, character configuration, and play-report path groups. |
| Unknown outer type | **0** | 0.0% | Every delta entry has a bounded path/extension group; `.bin` groups remain format-ambiguous. |
| **Total** | **1,191** | **100%** | — |

Semantic ownership remains unknown for **1,191 / 1,191 (100%)**: there is no
verified direct update-main/auxiliary-module reference to a changed or added
RomFS path and no runtime file-access trace in this pass. The category table
describes the strongest available evidence, not a claim that a module owns the
file.

## 6. Modified-container internal-analysis coverage

| Coverage item | Count | Denominator | Result |
|---|---:|---:|---|
| Hash-verified modified outer entries | 466 | 466 | Complete at outer-file level |
| Modified entries with size delta | 465 | 466 | Outer size evidence available |
| Modified entries with same-size content delta | 1 | 466 | Hash evidence available; no internal diff |
| Modified entries with verified internal member/record diff | **0** | **466** | Not performed / no evidence record |
| Modified entries classified outer-only | **466** | **466** | 100% |

The existing base SARC index is not an update internal diff, and the virtual
update inventory does not by itself enumerate changed members inside SARC or
GFLXPACK containers. No internal comparison is claimed for `.arc`, `.gfpak`,
`.bin`, `.dat`, `.tbl`, or `.blua`. In particular, the one same-size modified
entry is known to differ only at outer-file hash level; its internal records
remain unknown. Evidence: `p0-base-update-overlay.md` §4–§5 and
`p0-update-changed-content-classification.md` §4.

## 7. Unresolved ownership groups

| Group | Entries | Current evidence | Ownership unresolved because |
|---|---:|---|---|
| `message` | 689 | Path plus `.dat`/`.tbl` format convention; name-cluster match | No direct reader binding or runtime access trace |
| `trainer` | 309 | Trainer path convention | No matching subsystem label or direct consumer evidence |
| `haxe` | 91 | `.blua` Lua format/loader; event-script path | Format is identified, but script owner/call reachability is not |
| `event` | 52 | Event path convention; name-cluster match | No direct data binding or runtime trace |
| `appli` | 17 | SARC/BNTX/path evidence; name-cluster match | Container/image members and consuming code are unknown |
| `archive` | 11 | GFLXPACK/path evidence; name-cluster match | Pack members and consumer module are unknown |
| `flagwork` | 7 | AHTB/path evidence | No matching subsystem label or direct consumer evidence |
| `misc` | 7 | Configuration/telemetry path evidence | No direct consumer or runtime evidence |
| `field` | 4 | Encounter/spawn path convention | No matching subsystem label or direct consumer evidence |
| `pokemon` | 3 | Pokémon path convention; name-cluster match | Path label is not a code/data edge |
| `chara` | 1 | Character path convention; name-cluster match | Path label is not a code/data edge |
| **Total** | **1,191** | — | **All semantic ownership unresolved** |

## 8. Confidence and limitations

- **High confidence:** package hashes, NSO identities, overlay set counts, the
  465 size changes, the one same-size hash change, 725 additions, and zero
  deletions. These are independently reconciled by the overlay manifest.
- **Moderate confidence:** extension-level format labels supported by
  `sheets/domain/asset_formats.tsv`; the report applies them as outer-format
  evidence and does not claim internal parsing of the delta.
- **Lower confidence:** `.bin` semantic labels and all directory/domain
  classifications; they are conventions and bounded metadata groups.
- **Not established:** code-module ownership, direct data-to-code binding,
  runtime reachability, internal member changes, provenance of additions as
  genuinely new versus relocated/duplicated base content, and external-build
  equivalence.
- The update package is not standalone. Runtime integration also requires the
  update `main` paired with the shipped update `main.npdm`, the unchanged
  auxiliary modules, and the base RomFS with the BKTR overlay. The update NPDM
  disposition is documented in `p0-ownership-and-npdm.md` §2.

## 9. Next action for runtime/integration

First create the effective runtime input without changing the evidence ledger:
re-extract the pinned update `main.npdm`, pair it with update `main` and the
byte-identical auxiliary modules, and mount the base+update virtual RomFS.
Then run a bounded emulator/loader file-access trace over the unresolved domain
groups. In parallel, produce metadata-only internal diffs for the 466 modified
structured files (starting with SARC and GFLXPACK) and bind candidate readers
from update-main/auxiliary static references. Do not promote any group from
unknown ownership until a direct static edge or runtime access record exists.

## Evidence paths

- `reports/function-progress/p0-base-update-overlay.md` §§1–5 and Appendices A–B
- `reports/function-progress/p0-update-changed-content-classification.md` §§1–5
- `reports/function-progress/p0-ownership-and-npdm.md` §§1–3
- `work/pla/pk1/romfs_files.tsv`
- `work/pla/pk2/romfs_patched.tsv`
- `sheets/domain/asset_formats.tsv`
- `sheets/domain/subsystems.tsv`
- `sheets/re/base_update_diff.tsv`
- `sheets/re/p0_scope_census.tsv#update_content_overlay`
- `REPRODUCE.md` §§0, 4–5

*End of report. Metadata and evidence references only.*
