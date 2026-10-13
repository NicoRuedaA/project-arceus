# NPCs: placement, appearance, behaviour data (PLA update v262144)

**In plain words.** Every person standing in Jubilife Village or in a camp is a row of game data: where they
stand, which way they face, which "costume recipe" they wear, and under which story or time-of-day condition that
row is active. All of that is now decoded into sheets. A generic villager is built like a doll from three pieces
(a shared body skeleton, a dress and a head) and the game's recipe table says which pieces; the named characters
(guards, wardens, the Pearl/Diamond clan leaders...) are one ready-made model each. Three NPCs were assembled with the
existing player pipeline and rendered offscreen: shapes, faces and walk poses are right; generic villagers come out
with too-light clothes, white hair and pale skin because their colours are chosen at runtime and that rule is not in
the files found. NPCs do not walk along stored routes: the only path data belongs to wild Pokémon.

Out of scope and not decoded: dialogue, text, story/quest scripts. Story gates are kept as raw flag names and values.

| Sheet | Rows | Generator |
|---|--:|---|
| [`data/npc_placements.tsv`](../../data/npc_placements.tsv) | 1,662 (1,325 npc + 337 npc_pokemon variants) | `.tools/npc_decode.py` |
| [`data/npc_appearance.tsv`](../../data/npc_appearance.tsv) | 239 character recipes (203 cc, 36 tr) | same |
| [`data/npc_animations.tsv`](../../data/npc_animations.tsv) | 3,932 (clip, motion directory) pairs | same |
| [`data/npc_paths.tsv`](../../data/npc_paths.tsv) | 87 point paths (wild-spawner routes) | same |
| [`data/npc_config.tsv`](../../data/npc_config.tsv) | 152 tuning parameters (reaction ranges, look-at, controller) | same |

Tools: `.tools/npc_decode.py` (needs `.tools/gfdump`, numpy for the id search), `crates/pla/examples/npc_report.rs`
(headless counts + ground check), `crates/pla/examples/npc_export.rs` (glTF assembly), `.tools/npc_render_sheet.py`
(one-process Blender contact sheet). Nothing here was run in the viewer; no game code was traced.

## 1. Placement (`npc`, `npc_pokemon` placement kinds)

Files `bin/field/param/placement/<area>/npc/npc.bin` and `.../npc_pokemon/npc_pokemon.bin` inside
`resident_release.gfpak` (located by path hash, like `bg_parts`). 52 areas have NPCs, 30 have NPC Pokémon.

### Counts per area (records = distinct NPCs; variants = condition-dependent copies of a record)

| Area (main + sub-areas with NPCs) | NPC records | NPC variants | NPC-Pokémon records / variants |
|---|--:|--:|--:|
| ha_area00 Jubilife (main) | 99 | 470 | 48 / 167 |
| ha_area00 sub-areas (28 with NPCs: Galaxy building, houses...) | 112 | 364 | 23 / 58 |
| ha_area01 + 1 sub-area | 31 + 2 | 83 + 6 | 5 + 2 / 6 + 2 |
| ha_area02 + 6 sub-areas | 39 + 14 | 89 + 19 | 10 + 7 / 16 + 7 |
| ha_area03 (+1 sub-area, Pokémon only) | 26 | 66 | 7 + 1 / 19 + 2 |
| ha_area04 + 2 sub-areas | 35 + 5 | 94 + 6 | 7 + 1 / 16 + 1 |
| ha_area05 + 7 sub-areas | 36 + 15 | 87 + 22 | 13 + 6 / 36 + 6 |
| ha_area06 + 1 sub-area | 4 + 2 | 13 + 6 | 1 / 1 |
| **Total** | **420** | **1,325** | **131 / 337** |

564 variants (NPC and Pokémon) sit at exactly (0,0,0); per-area counts (and time/story-gated counts) are printed
by `npc_report`.

Area names (Fieldlands etc.) follow `field_spawn_table.tsv`; per-area detail (time-gated, story-gated, at origin) is
printed by `npc_report`.

### Record layout and confidence (FlatBuffers, field numbers only)

| Field | Meaning | Confidence |
|---|---|---|
| rec f0 / f1 | key string (`area00_npc_g005`, `area04_npc_captain7`) / FNV-1a 64 of it | VERIFIED (416/417 hashes = key) |
| rec f2 | FNV-1a 64 of the **character id** = key of the recipe table (§2) | VERIFIED: 1,294/1,325 variants hit the table; the other 31 are `senpai` (hash VERIFIED, not in the table: ASSUMED resolved at runtime to `tr0036_00_senpai_m` or `tr0037_00_senpai_f`, the opposite of the player) |
| rec f3 | u64, 156 distinct, 112 records empty | unresolved; LIKELY the talk/interaction id (text out of scope) |
| rec f4[] | placement variants; f7 position, f8 rotation, f9 scale (same instance layout as `bg_parts`) | position VERIFIED by ground check (below); rotation: only y is ever non-zero (1 exception), degrees LIKELY; scale 1 everywhere except one 0.8 (absent components read as 1, ASSUMED default) |
| variant f0 | condition kind hash. Resolved: `time_zone`, `time_zone_afternoon`, `time_zone_night`. Unresolved hashes named by the flags they test: quest progress (`sub_NNN`, `chapN`), story phase work (`WPHA_STORY`), flag (`FSYS_*`, `FEVE_*`, `FMAP_*`), work (`WSYS_*`, `WEVE_*`), default (no flag, usually the last variant), event label (`Ev_main...`) | names VERIFIED where resolved; roles LIKELY |
| variant f1 | comparison: `equal`, `not_equal`, `between`, `less_than_equal`, `greater_than_equal` | VERIFIED (all 5 hashes resolved) |
| variant f2 / f3 / f4 | flag or work name / value / second value (range end for `between`) | strings VERIFIED; meaning of story values unknown (kept raw) |
| variant at (0,0,0) | 564 variants (npc + Pokémon) have no position | LIKELY "not present in that state" |
| behaviour f5[i] (one per variant) | f17 / f18 facial expression (`normal`, `laugh`, `sad`, `serious`, `rejoice`, `trouble`, `seel`, `surprise`, `anger`) | names VERIFIED (hash), which slot is talk vs idle unknown |
| behaviour f20 | FNV of a linked placement key (17 rows, e.g. an NPC tied to its Pokémon `area00_poke_save_15`) | VERIFIED hash, role ASSUMED |
| behaviour f22 | string tag (208 rows: `ha_area00_bandit_1_01`, `area00_npc_photo_01_01`...) | string VERIFIED, meaning unknown |
| behaviour f0, f4..f11, f14, f16, f21, f24 | small integers/bools, kept raw in `beh_raw` | unknown. f5 (1..14) is a candidate for the controller's `wait_int` (idle-pose index, §3): ASSUMED, not tested |
| npc_pokemon rec f2 | key of the **NPC-Pokémon table** (resident entry 86, 400 rows: national dex species, form, sex string `Male`/`Famale`/`Unknown`, two bools, u32 127) | species VERIFIED as table data (290/337 variants resolve; 47 have an empty id) |

Time of day: 202 NPC variants are time-gated (117 `time_zone` with values 0..3, 44 afternoon, 41 night), all in
Jubilife and its sub-areas; i.e. village schedules exist as data. 1,045 are gated by story/quest flags.

### Ground check (`npc_report`, |dy| <= 3 m, positioned NPC variants only)

| Data | Result |
|---|---|
| Main areas vs terrain heightmap | 602/632 = **95.3 %**, mean 0.81 m |
| Main areas vs own navmesh (any mesh of the pack) | 594/642 = 92.5 % (26 points have no navmesh polygon within 1 m: roofs, platforms, water edges) |
| Sub-areas vs own navmesh (own frame) | 159/242 = 65.7 %; **without `ha_area00_s01` 134/140 = 95.7 %**. `ha_area00_s01` (two-storey building) only 25/102: its navmesh pack does not cover the upper floor or the frame differs (unknown) |
| NPC Pokémon vs navmesh | 178/214 = 83.2 % (perched/flying ones are off-mesh by design, ASSUMED) |

## 2. Appearance (character recipe table)

Found inside `resident_release.gfpak` entry 85 (no path resolves to it; found by content). One record per character
id; the placement's `chara_hash` is its key. 225/239 ids are named (exact hash matches: 66 from plain strings in Lua
scripts / chara tables / pack names, 159 by exhaustive candidate hashing); 14 stay as hashes but are fully usable.

| Field | Meaning | Confidence |
|---|---|---|
| f0 / f1 | FNV(id) / 3-digit group: rows of one group share a look and differ in motion set or held item | VERIFIED |
| f4.f1 | model list: cc = `cc_base0001_00_young_m.trmdl` + `<b>_dress<n>_<v>_<name>.trmdl` + `<b>_face<n>_00_<name>.trmdl`; tr = one `trNNNN_00_<name>.trmdl` | VERIFIED (paths; all resolve in the packs) |
| f4.f2 | hidden-mesh names (`hide_a_mesh`, 16 recipes) | string VERIFIED; rule ASSUMED (hide that mesh) |
| f3.f1 | animation controllers: body `.tracn` (motion set), plus per-part face/dress `.tracn` | VERIFIED |
| f5 | locator skeleton (`cc_base0001_00_loc.trskl`: head/eye/mouth effect points) | VERIFIED |
| f8 | held item model (`item_404.trmdl`..., 24 recipes) | VERIFIED |
| f10 | body type `a`, `b`, `cf`, `cm`, `df`, `dm`, `n*` = `chara/cc_value_list/chara_create_value_list_<code>.trbsm` (per-bone mesh scalers) | name match VERIFIED; how scalers apply ASSUMED, not implemented |

**How a cc NPC is composed (VERIFIED from files):** all parts live in ONE pack `chara/data/cc/cc.gfpak` at
`bin/chara/data/cc/model/<base|dress|face>/<stem>/<stem>.*`. The base `cc_base0001_00_young_m.trskl` is a full biped
(74 nodes, 72 rigs) shared by every body type, male and female; face (`rig offset 72`, 45 nodes incl. hair/jaw) and
dress (`rig offset 137+`) carry extension skeletons exactly like the player's parts. Body prefixes: `au` infant,
`bu` child, `cf`/`cm` old, `df`/`dm` adult. **Distinct looks:** 54 dress x face combinations among the cc recipes
(52 placed), 37 dress models, 36 face models; plus 33 tr packs (36 tr ids). 164 distinct characters are placed.
Colours: the cc materials, like the player's, have white `BaseColorLayer*` and grey hair tiles; no per-NPC colour
field was found in the recipe (unknown: runtime palette or the unexplained recipe field f9 / f2).

### Samples assembled (`npc_export`, offscreen Blender sheet looked at)

| Id | Parts | Primitives | Clips | Height | Look (honest) |
|---|---|--:|---|--:|---|
| `cc0058_00_vi_young_m_c` (adult villager, 36 placements) | base + `dm_dress0002_00` + `dm_face0002_00` | 15 | idle, walk, run | 1.72 m | shape, sandals, belt, face and walk pose right; tunic/trousers almost white and hair white-grey: colours not applied (runtime) |
| `cc0016_00_pr_young_f` (Pearl-clan woman, 36) | base + `df_dress0002_03` + `df_face0002_00` | 15 | idle, walk (the controller borrows `cb0005_01_kmn_f` walk) | 1.71 m | green kimono and apron coloured correctly, hair white-grey, skin pale |
| `tr0023_00_guard_lower_m` (guard, 59) | own pack | 22 | idle, walk (no run in its controller) | 1.81 m | red uniform, hat, mask and boots coloured; looks right |

Not applied: body-type scalers (female uses the male-proportioned base skeleton; heights look plausible anyway),
hair/skin/eye colours, held item, hide-mesh rule, face animation, IK, spring bones. Output (git-ignored):
`work/cache/npc/<id>/npc.gltf`, sheet `work/cache/npc/_sheet/npc_sheet.png`.

## 3. Behaviour and animation

* **Walking paths: none for NPCs.** The `path` kind (87 paths, 71 in ha_area03 incl. `skypath_*`) is referenced by
  name only from `spawner` files, never from npc/npc_pokemon files or Lua scripts: wild-Pokémon routes. NPCs in the
  data stand at one spot per variant; moving NPCs, if any, are script-driven (out of scope).
* **Schedules:** time-of-day variants (§1) move or hide village NPCs by time zone 0..3, afternoon and night.
* **Idle behaviour = the body animation controller.** The recipe's body `.tracn` points to `.tracr` (clip list,
  in `npc_appearance.controller_clips`), `.tracs` (state machine, not decoded), `.tracp` (parameters: `wait_int`,
  `move_speed`, `turn`, `add_idling_int`, `runstop`...), `.tralk` (look-at) and `.trbik` (biped IK). Motion sets:
  `cb0001` adult, `cb0000` child, `cb0002` cfl, `cb0003` wld, `cb0004` old, `cb0005` kimono, `cb1000`/`cb1001` "lsp"
  life-scene poses (pt01a..pt03c: e.g. pt01a lists 20 different wait poses 05121..05253), `cb9999` additive gestures
  (yes, no, speak, idling01..08). 81 recipes use an lsp pose controller.
* **Clips:** 3,932 (clip, directory) pairs, every one VERIFIED by the pack hash of `<dir>/<clip>.tranm`. Name classes
  (pattern guess): face 2,138, idle 563, turn 444, talk/gesture 305, walk 199, battle 143, run 92.
* **Tuning (`npc_config.tsv`, values VERIFIED, use ASSUMED from names):** NPC reaction ranges forward 3.5 m outdoor /
  2 m indoor within 75°, medium 1.5 m, close 0.95 m, reset 4 m; look-at: head 60°/s (start 140°/s), eyes 60°/s,
  gives up behind 135°; controller: walk 1.5 m/s, run 5, dash 7, capsule radius 0.35 m, height 0.75 m.

## 4. Brief 31 runtime status (first DS2-2 task)

Hook points read in `crates/pla/src/game/` (read-only): `world_objects.rs` (distance streaming
`should_be_alive`, 200/230 m), `zone.rs` (zone change despawns area content), `navigation.rs` (`height_at`,
`nearest`), `player_anim.rs` (`select_clip`, root-motion pinning, `step_facing`), `wild_wander.rs` (`yaw_for`,
`turn_towards`), `input_map.rs` (`Action::Confirm`), `PlayerAvatar` in `game/mod.rs`.

1. **Data load** `game/npc.rs`: parse `npc_placements.tsv` + `npc_appearance.tsv` (pure functions, unit-tested). The runtime also loads the animation census and tuning values from `npc_animations.tsv` and `npc_config.tsv`.
   Variant choice per record: drop (0,0,0) variants; prefer `time_zone*` variants matching the clock (0..3 ASSUMED
   morning/day/evening/night), else `default`, else the first story variant with a configured story level
   (`WPHA_STORY` value, e.g. post-game); keep it switchable because story semantics are not decoded.
2. **Models**: `game/npc.rs` requests existing `work/cache/npc/<id>/npc.gltf` exports through the same
   Bevy glTF scene and animation asset path used by the player. Missing exports use a neutral capsule only;
   clothing, hair and skin colours remain **SUPPOSED** and are not fabricated.
3. **Spawn** is distance-instanced like `world_objects` (200 m load / 230 m unload hysteresis), selected for
   the active area and time slot, and ground-snapped with `navigation.height_at` when |dy| > 0.5 m. Trainer
   red capsule markers remain owned by `trainer_encounter.rs` and are not replaced.
4. **Idle** selects a stable `wait*_loop` candidate from the decoded controller list and plays the exported
   `idle` clip. NPC routes remain **UNKNOWN**: `npc_paths.tsv` is wild-Pokémon path data and is not used.
5. **Look-at player** uses only bounded root yaw (forward reaction range/angle, close range, 60°/s and 135°
   behind from `npc_config`). Head/torso bones are not guessed because the existing player/model API exposes
   no stable NPC rig contract. Pure tests cover clamping and determinism.
6. **Interaction prompt/dialogue** remains out of scope; talk ids, text and story gates are still UNKNOWN.

Headless coverage for this runtime is in `game::npc` unit tests; no viewer/window/audio run is required for
the implementation evidence.

## Verified / Assumed / Unknown

* **Verified:** placement file locations and record structure; positions on the ground (95 % main areas, 96 % of
  interiors except the Galaxy building); character id hashes -> recipe table (all except `senpai`); recipe part
  paths, skeleton layout, clip directories; condition operator and time-zone names; NPC-Pokémon species table;
  three NPCs assemble on the shared skeleton and look right in shape (offscreen render, not the game viewer).
* **Assumed:** rotation in degrees about Y; absent scale = 1; (0,0,0) variant = absent; condition-kind roles;
  `senpai` resolution; behaviour f5 = idle-pose index; body-type scalers; config semantics.
* **Unknown:** talk id (rec f3) meaning, behaviour scalars, which facial slot is talk vs idle, recipe f2/f9, NPC
  colours (hair/skin/clothes palette), `.tracs` state machine, the Galaxy-building upper floor ground, story value
  semantics (deliberately not decoded).
