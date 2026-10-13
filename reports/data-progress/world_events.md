# World events — distortions, outbreaks, spawn time/weather (PLA v262144)

**In plain words:** the game data for space-time distortions is now readable: where the 25 distortion domes are, which
Pokémon appear in each (with levels, alphas and odds), and which items can drop. The day/night and weather rules of
every wild spawn are decoded too (for example ghosts only at night, fire types never in rain). Regular mass outbreaks
are decoded (which spawner turns on for each outbreak). Massive mass outbreaks are only half decoded: their settings
are read, but the list of species they use was not found in any file.

Generator: `.tools/events_decode.py` (needs `target/release/gfdump`, see `.tools/gfdump`). Checks:
`python3 .tools/tests/test_events_decode.py` (4/4 pass) and `cargo run -q --offline -p sheetty-cli -- check data`
(0 errors). Labels: **VERIFIED** = exact data link or arithmetic checked by the generator/test; **LIKELY** = strong
cross-check with known game facts; **ASSUMED** = guess; **UNKNOWN** = not decoded. Nothing here was checked against the
running game. The runtime port of distortions and outbreaks (Brief 39) is described in [Runtime (Brief 39)](#runtime-brief-39).

## Sheets

| Sheet | Rows | Source |
|---|--:|---|
| `data/world_events_distortion_sites.tsv` | 116 | `bin/field/param/placement/<area>/wormhole/wormhole.bin` (in `resident_release.gfpak`) |
| `data/world_events_distortion_points.tsv` | 3,091 | same file, fields 6 and 8 |
| `data/world_events_distortion_spawners.tsv` | 464 | `.../wh_spawner/spawner_data.bin` |
| `data/world_events_distortion_encounters.tsv` | 166 | `bin/encount/<area>/encount_data.bin` (keys used by `wh_spawner`) |
| `data/world_events_distortion_items.tsv` | 552 | `.../wh_spawner/wh_item_table.bin` |
| `data/world_events_outbreak_areas.tsv` | 5 | romfs `field/encount/huge_outbreak.bin` |
| `data/world_events_outbreak_spawners.tsv` | 454 | `spawner_data.bin` gated by `WSYS_MASS_GANERATION_AREAnn` |
| `data/world_events_mmo_areas.tsv` | 5 | `field/encount/new_huge_outbreak_lottery.bin` |
| `data/world_events_mmo_time_limits.tsv` | 3 | `field/encount/new_huge_outbreak_time_limit.bin` |
| `data/world_events_mmo_groups.tsv` | 480 | `field/encount/new_huge_outbreak_group.bin` |
| `data/world_events_mmo_group_lottery.tsv` | 501 | `field/encount/new_huge_outbreak_group_lottery.bin` |
| `data/world_events_area_weather.tsv` | 9 | `bin/field/weather/weather_table.bin` (in `resident_release.gfpak`) |
| `data/world_events_alpha_settings.tsv` | 12 | `bin/encount/common/oybn_setting.bin` |
| `data/world_events_npc_pokemon.tsv` | 131 | `.../npc_pokemon/npc_pokemon.bin` + `placement/common/pokemon_model_set.bin` |
| `data/world_events_species_time_weather.tsv` | 322 | romfs `pokemon/data/poke_encount.bin` |
| `data/field_spawn_conditions.tsv` | 8,510 | every regular spawner × encounter slot (1,951 spawners) |

`data/field_spawn_table.tsv` is unchanged. **Correction to it:** its `companion_count` column is not a companion
count. The slots it read from (those with slot field 25 = 3) are the **alpha** variants: 3 guaranteed perfect IVs, a
separate weight of 1 or 2 next to the normal weight 100, and the alpha flag in the slot's flag table. Use
`field_spawn_conditions.tsv` (`alpha` column) instead.

## 1. Space-time distortions

### What the data says

- **Sites (VERIFIED).** Each area has 4–6 dome sites (`WormholeAA_SS`: AA = site, SS = story variant), 25 sites and
  116 records in total: area 01 has 4 sites, 02 5, 03 5, 04 5, 05 6. All variants of a site share the dome centre
  (x, y, z in area-local metres, same frame as the spawn table).
- **Story gating (VERIFIED names, LIKELY meaning).** Each variant carries a `phase_condition` on work `WPHA_STORY`
  with comparator `less_than_equal` (first variant, e.g. ≤ 39999), `between` (e.g. 40000–49999) or
  `greater_than_equal` (≥ 100000). The comparator names were recovered by hashing candidate words with FNV-1a 64.
  Distortions start in area 01 at story ≤ 39999 and in area 05 at ≤ 69999.
- **Spawners (VERIFIED links).** Every variant lists one `main_whSpawnerAA01_SS` and three `sub_whSpawnerAAnn_SS`;
  FNV-1a 64 of each name equals a `wh_spawner` id (464/464). Main: shape `shape` (110) or `sphere` (6), radius 50,
  spawner counts 2..3. Sub: `s_sphere`, radius 6, count 1..1. Counts = spawner fields 7/8, LIKELY min/max alive.
- **Encounter tables (VERIFIED links, LIKELY semantics).** Only three keys are used, but the **same key has
  different contents in every area**, so a key must be resolved in the area's own `encount_data.bin`:

  | Area | Main dome (levels) | Sub spheres | Post-game sub table (story ≥ 100000) |
  |---|---|---|---|
  | 01 | Onix, Steelix, Haunter, Gengar, Lickitung, Lickilicky, Ursaring, Toxicroak, Eevee, Leafeon, Sylveon (group 20–23) | Sneasel, Weavile | — |
  | 02 | Floatzel, Snorlax, Drifblim, Lopunny, Luxio, Luxray, Heracross, Eevee, Umbreon, Flareon (20–23) | Porygon, Porygon2, Porygon-Z | + Cyndaquil, Quilava, Hisuian Typhlosion |
  | 03 | Kadabra, Alakazam, Rhydon, Rhyperior, Skuntank, Carnivine, Mr. Mime, Eevee, Vaporeon, Flareon (30–33) | Magnemite, Magneton, Magnezone | — |
  | 04 | Magmar, Magmortar, Dusclops, Dusknoir, Octillery, Drapion, Ambipom, Eevee, Jolteon, Sylveon (35–38) | Cranidos, Rampardos, Shieldon, Bastiodon | + Rowlet, Dartrix, Hisuian Decidueye |
  | 05 | Electabuzz, Electivire, Pikachu, Raichu, Sealeo, Walrein, Rapidash, Tangrowth, Scyther, Eevee, Glaceon, Espeon (45–48) | Scizor | + Oshawott, Dewott, Hisuian Samurott |

  Every species also has an alpha slot with weight 1. Weights and own level ranges are in the sheet. The tables do
  not change with the story variant; only the post-game sub table is added.
- **Item lottery (VERIFIED structure, LIKELY meaning).** Each variant points (field 3) to one table of 23 (item,
  weight) pairs. Item ids cross-checked locally against the game's item names (names are not stored): evolution items
  (Up-Grade, Dubious Disc, Electirizer, Magmarizer, Protector and id 1611, whose name did not decode: 5 each), ten evolution stones (2 each),
  Stardust 70, Star Piece 15, Comet Shard 5, Nugget 10, and red/blue/green shards with the rest. Later story variants
  lower only the shard weights (total 1000 → 750), so rare items become relatively more common; area 05 favours green
  shards, areas 01–04 red. How many items drop per distortion is UNKNOWN.
- **Points (LIKELY).** Each site stores 87–128 points (field 6 positions, field 8 names such as `0101`). They look like
  item/appearance positions inside the dome; not checked in code. Four variants store a slightly different point list
  than their site's first variant; the sheet keeps the first variant's list.
- **Unexplained:** wormhole fields 4 (always 1) and 5 (always 30), spawner fields 5/6/13/14/17–19/21/22.

### Timers (from `field_wormhole_config.bin` + targeted code reading, LIKELY)

Read in `FUN_024a2030`, `FUN_024a5d60`, `FUN_024a14d8`, `FUN_024a023c` (pseudocode, not executed). The config keys are
looked up by FNV-1a 64 hash; the arithmetic is as follows.

| Timer | Rule | Values |
|---|---|---|
| A: time until the next distortion | roll 0..99 (Xoroshiro128+), pick one of 5 minute values by weight; two tables chosen by the flag named in `kWhEventFlag` (`FSYS_FORCE_WEATHER_ENABLE`) | before: 5/10/15/25/40 min with weights 10/20/20/25/25; after: 5/10/15/25/40 min with weights 20/10/20/25/25. Result × 1800 frames (30 fps) |
| B: warning ("notice") phase | `(kWhTimerB_AddValue + distance_to_player / kWhTimerB_DivValue) × 30` frames; 30 frames if the divisor is < 1 | 60 s + half the distance in metres, in seconds (which config value is the addend and which the divisor is inferred from the names) |
| C: active phase | `kWhTimerC × 1800` frames | 4 min |
| Appear / vanish countdowns | value × 30 frames | appear 7 s, vanish 15 s (middle) and 7 s (final) |

Not traced to code: `kWhSpawnTimeMin/Max` (15/23), `kWhSpawnDistance` (10, ride 15), `kSubSpawnerPokeSpawnDistance`
(10), `kRareCollorEffectTimer` (15). Despawn beyond 50 m and the BGM/effect names come from the N2 report
(`reports/function-progress/n2-field-placement-and-distortions.md`). Site selection weighting: no per-site weight
exists in the data; the N2 report describes a weighted zone pick, source of the weights UNKNOWN. Whether the time
of day restricts distortions: no time field in the distortion data; UNKNOWN.

## 2. Mass outbreaks

### Regular mass outbreaks (VERIFIED links, ASSUMED counts)

`huge_outbreak.bin` has one record per area (key = FNV-1a 64 of `ha_areaNN`, VERIFIED): unlock gate `WPHA_STORY` ≥
14800 / 31200 / 40800 / 51200 / 60200 (areas 01–05; comparator hash not resolved), the work `WSYS_MASS_GANERATION_AREAnn`
that holds the active outbreak id, and `outbreak_count` = 46 / 44 / 44 / 48 / 45. That count equals the highest id
used by any spawner gate (VERIFIED). For each id exactly one spawner is gated `equal` (and has spawner field 10);
it holds a 100-weight normal slot and a 1-weight alpha slot, with counts 4..4. Up to 4 regular spawners per id are
gated `not_equal` and therefore switch off during that outbreak (VERIFIED from the comparator names). The other values
(20, 10, 15, 25) are ASSUMED: 10–15 = Pokémon per outbreak, 25 = extra shiny rolls, 20 = unknown (perhaps a daily
chance). How and when an id is drawn is UNKNOWN (the native consumer was not found).

### Massive mass outbreaks (partly decoded, species BLOCKED)

- Per-area settings (`world_events_mmo_areas`): flag `FSYS_NEW_OUTBREAK_OPEN_AREAnn` plus 16 u32 values
  (25, 11, 15, 1, 1, 8, 10, 6, 7, 15, 20, 10, 12, 228/236/220, 120/124/116, 12). Earlier sheets
  (`huge_outbreak_lottery.tsv`) already had these numbers; meanings UNKNOWN. Hypotheses only: 8–10 and 6–7 could be wave
  sizes, 12 extra shiny rolls.
- Time limits: three records with weights 75/15/10 (sum 100, VERIFIED) and values 420/540/660 (ASSUMED seconds) plus
  two parallel vectors (thresholds 300/150/60/0 and values 3/5/7/9); meaning UNKNOWN.
- Groups: 166 groups; each points to 3 encounter-table keys with weights 81/10/9 (157 groups) or 1 key with 100
  (9 groups). Seven group lotteries, each with three weighted lists of 33/27/6/2 groups.
- **Blocker:** none of the 480 encounter keys used by the groups exists in any `encount_data.bin`, anywhere in
  `resident_release.gfpak`, in the loose romfs files or in `main.elf` (byte search). The keys are FNV-1a 64 hashes of
  names that differ only in the last character. Which species a massive outbreak spawns is therefore UNKNOWN.

## 3. Wild spawn time, weather, alpha and form (LIKELY, strong cross-checks)

Encounter slot fields (field numbers in `encount_data.bin` slot tables):

| Field | Meaning | Confidence |
|---|---|---|
| 0 | species | VERIFIED (earlier report) |
| 3 | form (Burmy 0–2, Unown 1–27, Hisuian forms 1, white-striped Basculin 2) | LIKELY |
| 25 | guaranteed perfect IVs, 3 on every alpha slot | LIKELY |
| 29 / 30 / 31 | weight / own min level / own max level | LIKELY (earlier report) |
| 33–41 | custom moveset: flag, 4 move ids, 4 per-move flags (16 slots) | LIKELY |
| 45 | condition table: f0 condition type hash, f1 comparator hash, f2–f6 string arguments, **f7–f18 = 12 multipliers** | see below |
| 46 | flag table: bit 0 alpha (1,971 slots), bit 1 unique alpha (88 weight-100 alphas), bits 2/3 unknown | LIKELY / UNKNOWN |
| 2, 6–8, 9–12, 13–24, 28, 32, 42, 44 | not decoded (13–24 are twelve −1 values, probably IVs/EVs) | UNKNOWN |

The 12 multipliers are 4 time slots then 8 weathers, in the same order as the weather ids 2..9: sunny, cloudy, rain,
snow, drought (intense sun), fog, rainstorm, snowstorm. `poke_encount.bin` holds the same 12 values per species (and
form) as defaults. A value of −1 means "no override": in a slot it falls back to the species default, and a −1 default
is neutral (1.0). An absent value is the FlatBuffers default 0 and means "never". `field_spawn_conditions.tsv` stores
the resulting effective multipliers.

Validation (all from the data, compared with known game behaviour):

- **Night slot (slot 4):** the species defaults make 23 species night-only. They include 12 of the 21 Ghost
  species, plus Zubat, Golbat, Crobat, Murkrow, Honchkrow and the Clefairy line. **Silcoon and Beautifly never appear at
  night, while Cascoon and Dustox appear only at night**, which is the known day/night split of Wurmple's evolutions.
  Espeon and Combee are halved at night. Starly slots exclude night. Drifloon is night-only by default (not "evening").
- **Slots 1–3** are never different from each other anywhere in the data, so their order (morning, day, evening) is
  ASSUMED.
- **Weather order:** mean effective multiplier by type: Fire 1.17 sunny / 1.42 drought / 0.42 rain and snow; Ice
  1.17 snow / 1.43 snowstorm / 0.57 drought; Water 1.05 rain / 1.11 rainstorm / 0.88 drought; Electric 1.15
  rainstorm. Individual slots: Ponyta ×2 in drought and never in rain, snow or storms (Bidoof ×5 replaces it in those
  spawners); Thundurus only in rainstorm (area 03); Tornadus and Lucario only in snowstorm (area 05).
- **Per-area weather table** (`world_events_area_weather`, rows sum to 100, VERIFIED arithmetic, LIKELY order): area 01
  sunny 60 / cloudy 15 / rain 15 / drought 4 / fog 3 / rainstorm 3; 02 rain 25; 03 drought 20, rainstorm 10; 04 snow 10,
  snowstorm 5; 05 snow 25, snowstorm 20; 00 and 06 always sunny. Areas match the weather-gated species above.
  Fields 1/2 (4/6 or 2/4) are UNKNOWN, possibly a duration range in game hours.
- **Conditions:** resolved names `work_condition`, `phase_condition`, `quest_progress_condition`, `equal`,
  `not_equal`, `between`, `greater_than_equal`, `less_than_equal`. Type `3ddd5feb4a68a149` takes flag names, labelled
  `flag_condition?`. 1,655 random alpha slots of area N require flag `FSYS_NS_0N_CLEARED` (area 05 also has 3 slots
  on `NS_04`). That reads as "after the area's Noble is calmed", but the flag meaning is inferred from its name.
- `bin/encount/common/oybn_setting.bin` (12 areas/subareas, values 15/15/15/20/20) and `field_spawner_config`
  (`unique_oybn_leave_count` 3) are exported raw; their meaning is UNKNOWN.

## 4. Fixed overworld Pokémon (`npc_pokemon`, quick pass)

131 placed Pokémon in 30 area files (71 in `ha_area00` and its subareas, i.e. Jubilife Village). Name, position, Y rotation (LIKELY degrees) and
story gate are decoded. 103/131 `model_key`s resolve in `pokemon_model_set.bin` (species, form, gender string, size
byte: a `_big` prop has 255 and a `_small` one 1, LIKELY a size scalar). Most rows are story/quest props, which are out
of the port's scope.

## Unknown (summary)

Time-of-day slot boundaries (hours) are not in these files: `bin/field/env/<area>/light_time_table.bin` holds lighting
parameters per slot, not boundaries. Also unknown: the order of time slots 1–3, how many items a distortion drops,
how distortion sites are weighted, how outbreak ids are drawn and when, the massive-outbreak species and most of its
numeric settings, the regular-outbreak values 20/10/15/25, weather durations, and slot fields 2, 6–13, 28, 32, 42, 44
and flag-table bits 2/3.

## Implementation plan

### Brief 9 (clock and weather)

1. Clock: 4 slots with index 3 = night (VERIFIED by spawn data). Keep the ASSUMED 6-hour boundaries until a source is
   found. Expose `time_slot()`; spawn code reads `time_<slot>` from `field_spawn_conditions`.
2. Weather: replace the global default weights of `data/weather/weather_lottery_matrix.tsv` with the per-area weights of
   `world_events_area_weather.tsv` (ids 2..9 in the column order). Change weather every `f1..f2` game hours (ASSUMED);
   use one seeded RNG per area (the game uses Xoroshiro128+, not ported).
3. Spawning: the effective slot weight is `weight × time_<slot> × weather_<w>`; a 0 removes the slot. Alphas are
   rows with `alpha=1`; the `FSYS_NS_0N_CLEARED` condition can be modelled as a save flag.

### Brief 39 (distortions and outbreaks)

1. Distortion state machine (pure module): Idle → Notice → Active → End. Timer A from the lottery above (the "before"
   table until the event flag is known), Notice = 60 s + distance/2 s, Active = 4 min, vanish warnings 15 s and 7 s.
2. Pick a site: among the area's sites, the variant whose story gate matches the save's story value; uniform choice
   between sites (ASSUMED). Centre = site x,y,z.
3. Contents: the main spawner (radius 50, 2–3 alive) and three sub spheres (radius 6, 1 alive each) draw from
   `world_events_distortion_encounters` (time/weather multipliers are all 1 in these tables, except where listed).
   Items: weighted draw from `world_events_distortion_items` at points from `world_events_distortion_points`
   (count per event ASSUMED).
4. Mass outbreaks: per unlocked area (story ≥ gate), draw an id 1..`outbreak_count` (rule ASSUMED), enable the `equal`
   spawner and disable the `not_equal` ones listed in `world_events_outbreak_spawners`, spawn 4 alive until 10–15
   (ASSUMED) are done.
5. Massive outbreaks: blocked on species data; only placeholder species are possible, and they must be labelled.

## Runtime (Brief 39)

**In plain words:** the port now runs space-time distortions and regular mass outbreaks while you walk around. A
distortion forms at one of the area's dome sites, becomes active for 4 minutes with rare Pokémon and a few items,
then fades. A mass outbreak puts a group of one species near its decoded spot and switches off the few normal spawners
the game switches off. Massive mass outbreaks stay out: their species are not in any decoded file, so nothing is
invented. Nothing is saved; events restart each session.

Code: `crates/pla/src/game/distortion.rs` (pure data + machine), `crates/pla/src/game/outbreak.rs` (pure outbreak
data, draw rule, massive-outbreak resolution), `crates/pla/src/game/world_events.rs` (sheet helpers, slot weights,
Bevy plugin). Viewer hook: one plugin line in `examples/port_viewer.rs`. Numbers without a window:
`cargo run -p pla --example world_events_report -- [--hours 3] [--event-seed N] [--story N]`.

How it reaches the world: each alive event Pokémon is a synthetic spawner (`we:<slot>#<generation>`) added to
`ActiveAreaSpawners`, so the existing wild streaming spawns, fights and catches it. A defeated or caught one is seen
through `ConsumedSpawners`; its slot gets a new id after the refill delay (distortions) or counts towards the outbreak
total (outbreaks).

| Item | Rule in the port | Label |
|---|---|---|
| Config | 37/37 values of `wormhole_config_values.tsv` read by key | VERIFIED (test) |
| Idle (timer A) | 5/10/15/25/40 min, weights 10/20/20/25/25 ("before" table; the event flag is not modelled) | LIKELY (code reading) |
| Notice | 60 s + distance to the player / 2; appear cue in the last 7 s | LIKELY / cue ASSUMED |
| Active | 4 min; warnings at 15 s and 7 s before the end | LIKELY / cue ASSUMED |
| End | 3 s clean-up, then a new timer A | ASSUMED |
| Site choice | uniform among the area's sites whose story gate matches (one variant per site) | ASSUMED (game weights UNKNOWN) |
| Distortion contents | main spawner `min..max` (2..3) alive at dome points, 3 sub spheres with 1 alive each, species from the area's own encounter table | links VERIFIED, counts LIKELY, use of points ASSUMED |
| Refill | a cleared slot spawns again after 15-23 s (`kWhSpawnTimeMin/Max`) | ASSUMED meaning |
| Items | 6 drops per distortion on distinct dome points, item by weight from the site's 23-entry table; Confirm within 3 m adds 1 to the bag | table VERIFIED, count ASSUMED |
| Outbreak spawners | the 227 `equal`-gated spawners are off unless their outbreak runs | VERIFIED gates |
| Switched-off spawners | rebuilt from `field_spawn_table.tsv` (`WSYS_MASS_GANERATION_AREAnn=id` rows other than the outbreak spawner): 227/227 sets match `disabled_spawners`; removed during the outbreak, restored after | VERIFIED (test) |
| Outbreak draw | one per unlocked area per game day, id uniform in `1..outbreak_count`; ends after 10..15 cleared or at the day change; 4 alive at the decoded centre within its radius | ASSUMED |
| Massive outbreaks | 166 groups / 334 keys, 0 keys resolve in any decoded table -> 0 usable groups, all skipped | VERIFIED count, species UNKNOWN |
| Time / weather | slot weight x time multiplier x weather multiplier (0 = never), from the sheets; alpha slots skipped | multipliers LIKELY |
| Story | `--story <n>`, default 100000 (the port has no story progress) | ASSUMED |
| Visuals / audio | translucent dome sphere (faint in Notice, stronger in Active), red pillar over an outbreak, glowing item markers; cues as `WorldEventCue` messages, log lines and the HUD toast; no game effect, BGM or sound is used | ASSUMED look |

Viewer flags: `--story <n>`, `--event-seed <u64>`, `--event-time-scale <f>` (only the distortion timers),
`--distortion-now` (skip the first Idle timer), `--outbreak <id|off>` (force or disable the start area's outbreak),
`--no-world-events`.

Tests (`cargo test -p pla --lib -- world_events distortion outbreak`, 22 pass): machine sequence and durations,
threshold cues in one long step, deterministic site sequence per seed, timer A frequencies, real sheet links
(116 variants, 464 spawners, 4/5/5/5/6 sites per area), area 04 sub table species, outbreak disabled sets 227/227,
draw determinism and range, outbreak replacing the area spawn table and restoring it, outbreak total and refill ids,
massive outbreaks 0/334, flags, loot range, and a headless Bevy run on area 01 (notice, materialise with 5-6 event
spawners, warnings, end, forced outbreak with 4 Pichu, markers).

Native functions marked `partial` in the evidence ledger (relation `distortion-machine`): `024a2030` (timer A
lottery), `024a5d60` (notice duration), `024a1490` (to Notice), `024a14d8` (to Active, active duration), `024a18b8`
(pre-end warning), `024a1b64` (end clean-up). `024a1de8` (zone lottery) is not ported (uniform choice instead). No
behaviour verification or binary matching is claimed.

Still unknown or skipped: the event flag that switches the timer table, the game's site weights, how many items drop,
the 50 m content despawn rule (the port uses the normal 120/145 m streaming), the distortion weather override, alpha
event Pokémon (Brief 17), the regular-outbreak values 20 and 25, and every massive-outbreak species.
