# P0 — Update v262144 changed/added content classification (metadata only)

Generated: 2026-10-06T10:46:06Z (UTC). Scope: Pokémon Legends: Arceus update
v262144 (`pk2.nsz`, SHA-256 `f68eecf0…9e6446`), effective base+update RomFS overlay.
Status: **classification heuristic; ownership and runtime use remain unknown.**

**Superseding note (2026-10-06):** §3.1's old “360 added” prose is stale. The
reconciled result is **336 mapped / 389 unmapped additions**, based only on
heuristic string-path/SCC label matching. This does not establish semantic
ownership or code-to-file references; see `p0-overlay-ownership.md` §4.3 and
`p0-wave1-reconciliation.md` §“Count and path checks”.

This is a **metadata-only** artifact. It records paths, counts, sizes, extensions,
directory patterns and heuristic group labels. It contains **no** game bytes,
pseudocode, strings, keys, or asset payloads. It was produced read-only from the
committed inputs below; the effective-overlay counts are re-derived, not trusted
blindly, and they reconcile exactly with `p0-base-update-overlay.md`.

## Inputs and method

| Input | Role |
|---|---|
| `work/pla/pk1/romfs_files.tsv` | Base RomFS inventory (18,370 path/offset/size rows) |
| `work/pla/pk2/romfs_patched.tsv` | Update virtual RomFS inventory (19,095 rows) |
| `reports/function-progress/p0-base-update-overlay.md` | Hash-verified overlay manifest (466 changed / 725 added / 0 removed) |
| `sheets/domain/subsystems.tsv` | String-path/SCC subsystem cluster labels (17 rows; `subsystem_cluster.py`) |
| `sheets/re/base_update_diff.tsv`, `sheets/re/p0_scope_census.tsv` | Committed metadata census and `subsystem_clusters` note |

Method: parse both TSVs (path, size); `added` = update-only paths; `changed` =
common paths with differing size plus the single hash-verified same-size change
recorded in `p0-base-update-overlay.md` §5. Group by second path component
(`/bin/<dir>`), file extension, and — where the domain cluster label set contains
a matching top-level directory name — by the string-path/SCC heuristic. No file
content was read, printed, or copied; only the TSV metadata columns were used.

Re-derived counts: **added 725, changed 466 (465 size + 1 same-size), removed 0**
— identical to `p0-base-update-overlay.md` §2.

## 1. Changed content (466 files)

### 1.1 By top-level directory

| Directory | Changed | Share |
|---|---:|---:|
| `/bin/message` | 369 | 79.2% |
| `/bin/event` | 40 | 8.6% |
| `/bin/haxe` | 17 | 3.6% |
| `/bin/appli` | 13 | 2.8% |
| `/bin/archive` | 11 | 2.4% |
| `/bin/flagwork` | 7 | 1.5% |
| `/bin/misc` | 5 | 1.1% |
| `/bin/pokemon` | 3 | 0.6% |
| `/bin/chara` | 1 | 0.2% |
| **Total** | **466** | 100% |

### 1.2 By extension × directory

| Ext | Dir | Count | Notes |
|---|---|---:|---|
| `.dat` | message | 189 | message payload files |
| `.tbl` | message | 180 | companion string/offset tables |
| `.bin` | event | 40 | event/quest/trigger tables |
| `.blua` | haxe | 17 | compiled Haxe event scripts |
| `.arc` | appli | 10 | UI packages |
| `.gfpak` | archive | 10 | packaged asset containers |
| `.tbl` | flagwork | 7 | flag/work tables |
| `.bin` | misc | 5 | app config / play report |
| `.bin` | appli | 3 | icon/position/tips data |
| `.bin` | pokemon | 3 | AI/drop/encounter data |
| `.bin` | archive | 1 | archive contents index |
| `.bin` | chara | 1 | facial template table |

Extension totals: `.dat` 189, `.tbl` 187, `.bin` 53, `.blua` 17, `.arc` 10,
`.gfpak` 10.

### 1.3 Representative path patterns

- `/bin/message/<Lang>/{common,script}/*.{dat,tbl}` — 10 languages
  (`English, French, German, Italian, JPN, JPN_KANJI, Korean, Simp_Chinese,
  Spanish, Trad_Chinese`); `common/` tables (`btl_app`, `btl_loss`, `field_info`,
  `map`, `message_error`, `namelist`, `questlist_sub`, `tips`, `trname`, `trtype`)
  and `script/` files (`battle_portal`, `bed`, `box_release`, `farmharvest`,
  `icho_rare_shop`, `picture`, `z_area00`, `z_fukidashi`, `chap_10`).
- `/bin/event/event_progress/…` — `trigger/*`, `npc_talk_table/*`,
  `npc_pokemon_talk_table/*`, `ginkgo/*`, `phase/phase_progress_table_story.bin`,
  `quest_list_main.bin`, `quest_list_sub.bin`, `event_list.bin`,
  `script_id_record_release.bin`.
- `/bin/haxe/release/event/*.blua` — 17 event scripts (`battleportal`,
  `bedtimechange`, `farmharvestevent`, `picturecameraevent`, `zukancheck`, …).
- `/bin/appli/title_demo/bin/title_demo_01*_<lang>.arc` (9 locale variants),
  `report_backup/bin/report_top_00.arc` + `_lyt.bin`, `map/data/map_icon_pos_table.bin`,
  `tips/bin/tips_data.bin`.
- `/bin/archive/*.gfpak` — `ai`, `appli/map`, `appli/pause_menu`, `appli/poke_dex`,
  `field/resident_release`, `field/model/pack/ha_area00`, four
  `pokemon/pm*_41.gfpak`; plus `archive_contents.bin`.
- `/bin/flagwork/*.tbl` — `event_flags`, `event_works`, `map_flags`, `map_works`,
  `phase_works`, `system_flags`, `system_works` (all seven).
- `/bin/misc/app_config/*.bin` (`app_config_list`, `event_balloonrun_config`,
  `event_farm_config`, `field_huge_outbreak`) and `play_report/play_report.bin`.
- `/bin/pokemon/data/{poke_ai,poke_drop_item,poke_event_encount}.bin`;
  `/bin/chara/table/facial_template_table.bin`.

### 1.4 Anomaly

`/bin/event/event_progress/trigger/trigger_map_area00_s02.bin` (5,176 B) changed
content at identical size; it is included here as the 466th changed file per the
hash-verified manifest (`p0-base-update-overlay.md` §5).

## 2. Added content (725 files)

### 2.1 By top-level directory

| Directory | Added | Share |
|---|---:|---:|
| `/bin/message` | 320 | 44.1% |
| `/bin/trainer` | 309 | 42.6% |
| `/bin/haxe` | 74 | 10.2% |
| `/bin/event` | 12 | 1.7% |
| `/bin/appli` | 4 | 0.6% |
| `/bin/field` | 4 | 0.6% |
| `/bin/misc` | 2 | 0.3% |
| **Total** | **725** | 100% |

### 2.2 By extension × directory

| Ext | Dir | Count | Notes |
|---|---|---:|---|
| `.bin` | trainer | 309 | trainer definitions/parties |
| `.dat` | message | 160 | message payload files |
| `.tbl` | message | 160 | companion tables |
| `.blua` | haxe | 74 | new event scripts |
| `.bin` | event | 12 | new quest triggers |
| `.bntx` | appli | 4 | tip images |
| `.bin` | field | 4 | outbreak/spawn data |
| `.bin` | misc | 2 | app config tables |

Extension totals: `.bin` 327, `.dat` 160, `.tbl` 160, `.blua` 74, `.bntx` 4.

### 2.3 Representative path patterns

- `/bin/trainer/trdata_single_001.bin … _242.bin` (242) plus 67 numbered
  `trdata_trainerXX_YY[A-C].bin` files — 309 trainer data files.
- `/bin/message/<Lang>/…` — 32 per language × 10 languages: `common/title.{dat,tbl}`,
  `script/{sub_201,sub_250,sub_251,sub_252,sub_256,sub_257,sub_258,sub_259,
  sub_261,sub_350,sub_351,sub_352,arceus_challenge,gonbe_discover,staffrollreplay}.{dat,tbl}`.
- `/bin/haxe/release/event/*.blua` — 74 new scripts (`ev_sub*`,
  `arceuschallenge`, `battleportal*`, `picturevisitor*`, `staffrollreplay`, …).
- `/bin/event/event_progress/trigger/trigger_quest_sub{201,250…352}.bin` (12).
- `/bin/appli/tips/bin/tips_img_11xx_0x.bntx` (4).
- `/bin/field/encount/new_huge_outbreak_*.bin` (4).
- `/bin/misc/app_config/{event_restriction_battle,event_work}.bin` (2).

## 3. Heuristic ownership classification (labels are HYPOTHESES)

Ownership is **not** established. The following classifies each group by
path/directory/extension convention only, and is explicitly a heuristic.

| Class (heuristic) | Groups | Basis |
|---|---|---|
| Game text / message tables | message `common/*.{dat,tbl}`, `script/*.{dat,tbl}` | `/bin/message/<Lang>` path convention |
| Event / quest / trigger data tables | event `event_progress/**/*.bin` | `event_progress`, `trigger`, `npc_talk`, `quest_list` path tokens |
| Compiled event scripts | haxe `release/event/*.blua` | `.blua` extension + `release/event` path |
| Flag / work save tables | flagwork `*.tbl` | directory + known sheet names (`sheets/domain/*_{flags,works}.tsv`) |
| Trainer definitions | trainer `trdata_*.bin` | directory + `trdata` token |
| App configuration tables | misc `app_config/*.bin` | `app_config` path token |
| Telemetry / play statistics | misc `play_report/play_report.bin` | `play_report` path token |
| Pokémon numeric data | pokemon `data/*.bin` | directory + `poke_*` tokens |
| Character asset configuration | chara `table/facial_template_table.bin` | `table` + `facial_template` token |
| Packaged asset containers | archive `*.gfpak`; appli `*.arc` | GFPAK/ARC container extensions |
| Image assets | appli `*.bntx` | BNTX texture extension |
| Field encounter / spawn data | field `encount/new_huge_outbreak_*.bin` | `encount` + `new_huge_outbreak` tokens |

### 3.1 Subsystem heuristic (string-path/SCC) — availability only

The domain cluster set (`sheets/domain/subsystems.tsv`, 17 labels) is derived by
`subsystem_cluster.py` from update-main string/SCC references over **functions**,
not over RomFS files. Matching a file's top-level directory to a cluster name is a
**name coincidence**, not a code→asset edge. Middleware labels
(`grpc`, `openssl`, `havok`, `grpc_core`, `absl`, `re2`, `protobuf`, `webrtc`) match
no RomFS top-level directory.

| Dir | Changed | Added | Cluster label present? |
|---|---:|---:|---|
| message | 369 | 320 | yes (`message`, game_data) |
| event | 40 | 12 | yes (`event`, game_data) |
| appli | 13 | 4 | yes (`appli`, game_data) |
| archive | 11 | 0 | yes (`archive`, game_data) |
| pokemon | 3 | 0 | yes (`pokemon`, game_data) |
| chara | 1 | 0 | yes (`chara`, game_data) |
| haxe | 17 | 74 | **no** |
| trainer | 0 | 309 | **no** |
| flagwork | 7 | 0 | **no** |
| misc | 5 | 2 | **no** |
| field | 0 | 4 | **no** |

Mapped-by-name: 437 changed / 336 added. Unmapped: 29 changed / 389 added.
`sheets/re/p0_scope_census.tsv#subsystem_clusters` itself records grouping as
"string-path/SCC heuristic evidence, not semantic ownership."

## 4. What cannot be concluded

- **File-level ownership / semantic subsystem.** Directory-name matching does not
  show that any update-main (or other module) function reads these files. There is
  no direct data/call reference established from these paths to code.
- **Runtime use and reachability.** No execution, loader trace, or reference-count
  evidence; added files may be unreferenced, region-gated, or unused.
- **Provenance of added files** (new content vs. relocated/duplicated base content):
  added paths have no base counterpart to compare (`p0-base-update-overlay.md` §4).
- **Internal changes inside containers.** For `.arc`/`.gfpak`/`.bin`/`.blua`, only
  the container-level hash/size is known; no per-entry SARC/internal diff exists
  (`p0-base-update-overlay.md` §4). The one same-size changed file was not inspected.
- **Format semantics.** Extension conventions (`.dat`/`.tbl` pairing, `.blua`
  compiled-script status) are assumed, not verified by content this pass.
- **Completeness of the canonical subsystem list.** The 17 clusters are a
  non-exhaustive heuristic seed, not an ownership map.

## 5. What would resolve it

1. **Per-entry container diff (metadata only):** enumerate SARC/GFPAK internal
   entries with hashes for the 466 changed containers to name the changed members.
2. **Added-file provenance:** hash-match added payloads against the base tree to
   distinguish new content from renamed/relocated base content.
3. **Static reference binding:** derive direct update-main (and auxiliary-module)
   path/string references and call edges to these files to establish *candidate*
   consumption edges — still not runtime proof.
4. **Runtime evidence:** file-access trace under emulator/loader instrumentation to
   confirm actual use and ordering.
5. **Format descriptors:** parse container headers/magics to confirm `.dat`/`.tbl`,
   `.blua`, and GFPAK semantics without copying payloads.

## Citations

- `reports/function-progress/p0-base-update-overlay.md` §2 (counts), §3 (overlay
  rule), §4 (unknowns), §5 (anomaly), Appendix A (added), Appendix B (changed).
- `work/pla/pk1/romfs_files.tsv`, `work/pla/pk2/romfs_patched.tsv` (metadata inputs).
- `sheets/domain/subsystems.tsv`; `sheets/re/base_update_diff.tsv` (`subsystems_total`
  = 17); `sheets/re/p0_scope_census.tsv` (`subsystem_clusters`, `update_content_overlay`,
  `asset_format_catalog`).
- `reports/function-progress/p0-base-update-overlay.md` §1 for the update `main`
  replacement context (ExeFS members unchanged except `main`).

*End of classification. Metadata only; no game bytes, pseudocode, strings, or keys.*
