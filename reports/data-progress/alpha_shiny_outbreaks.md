# Alphas, the rare palette and the outbreak species (Briefs 17/39 remainder)

Closes the three open items of the 2026-10-15 data session: the alpha availability flags and their
world distribution, the rare-palette probability and the species of the mass / massive mass
outbreaks. Every claim is labelled **VERIFIED** (a local decoded file or the shipped executable
proves it), **PROBABLE** (external player notes only; their figures are cited, never their text)
or **UNKNOWN**.

Method: the update v262144 data sheets already in `data/`, a fresh `gfdump` of
`bin/archive/field/resident_release.gfpak` (2,066 entries) and a constant scan over the 9.5 M
instructions of `work/pla/pk2/main.elf` in the existing Ghidra project (`PLA-update-full`),
looking for the FNV-1a 64 hashes of the config parameter names. The scan scripts are throw-away
(`ScanImm.java`, `ProbeAddr.java`, `DecompFn.java`); the findings are reproducible from the
addresses below.

## 1. Alpha availability flags and world distribution

### What is decoded (VERIFIED)

`data/field_spawn_conditions.tsv` (8,510 slot rows, 1,951 spawners, from `spawner_data.bin` +
`encount_data.bin` of `resident_release.gfpak`) carries per-slot alpha facts:

| Fact | Value |
|---|---|
| Random-alpha slot rows (`alpha` bit, `unique_alpha` = 0) | 4,126: 2,455 at weight 1, 1,657 at weight 2, 14 at weight 100 |
| Fixed alpha rows (`unique_alpha` bit) | 86, always weight 100 |
| Random-alpha rows gated by `FSYS_NS_0N_CLEARED` | 3,885 (the rest carry event flags or no condition) |
| Species with at least one alpha slot | 192 |
| Spawners whose every slot is an alpha | 100 (fixed alphas and alpha-only event spawners) |

The "availability flag" of an alpha per Pokémon is therefore **per slot, not per species**: a
species is eligible exactly where its encounter table holds an alpha row. The
`FSYS_NS_0N_CLEARED` gate on the random rows reads as "the area's Noble is calmed" (name
interpretation, LIKELY; 1,655 of the rows come from the earlier Brief 39 pass).

### Distribution (VERIFIED mechanism, port policy)

A spawner rolls one slot out of its encounter table by weight, so the alpha chance of a roll is
`alpha weight / total weight`. Over the decoded tables that is **0.97 % (weight 1 against a
100-weight normal) up to 3.7 %**, with fixed alphas at 100 %. The port now keeps the alpha rows
instead of dropping them (it used to keep only the heaviest row per species), so its alpha rate
comes from the decoded weights instead of an invented probability. The decoded gates are read
into `SlotFacts::gated_by_area_clear` and kept behind `wild::ALPHA_REQUIRE_AREA_CLEARED_FLAG`
(`false` while the port has no persisted Noble-clear state — an ASSUMED simplification).

The external notes say "0.2 %-2.0 % after calming the Noble" (PROBABLE). The decoded data narrows
that to 0.97 %-1.96 % for the light rows; the port follows the game data (game over notes).

Alpha size: the notes' "scale 255 (maximum)" matches the decoded size categories — the field
scale of `LL` is 2.5 (`pokemon_size_category_adjust_scale_config.bin`, VERIFIED value already used
by `wild::ALPHA_FIELD_SCALE`). The 3 guaranteed perfect IVs of the alpha slots were already
decoded (slot field 25 = 3).

## 2. Rare palette ("shiny") probability

### VERIFIED: the parameter set and its use in the shipped code

`bin/misc/app_config/pokemon_rare.bin` holds five parameters and nothing else (712 bytes;
`data/pokemon_variants.tsv`):

| Parameter | Raw value | Port constant |
|---|--:|---|
| `TryCount_Item` | 3 | `rare_palette::RARE_TRY_BONUS_ITEM` |
| `TryCount_Research100` | 1 | `RARE_TRY_BONUS_RESEARCH_100` |
| `TryCount_ResearchPerfect` | 2 | `RARE_TRY_BONUS_RESEARCH_PERFECT` |
| `Field_StartEffectDistance` | 35 | `RARE_EFFECT_START_DISTANCE_M` |
| `Field_SoundFrame` | 150 | `RARE_SOUND_FRAME` |

The update executable looks those parameters up by the **FNV-1a 64 hash of their names**, which
proves the "extra tries" reading lives in code and not only in the data file:

- `FUN_00f94470` (Pokémon creation path; creates the struct the field pawn copies) contains
  `0x95ccdc647c831655` = fnv1a64("TryCount_Item"), `0x79efd2242bab9e76` =
  fnv1a64("TryCount_Research100") and `0x09b7e367dc52b434` = fnv1a64("TryCount_ResearchPerfect"),
  each followed by a map lookup keyed by the same hash.
- `FUN_01b1aa88` (field sparkle trigger, runs when a created creature is rare) fetches
  `pokemon_rare` + `Field_SoundFrame` and stores the frame value in its state.
- The three call sites of `FUN_00f94470` are `FUN_01a586c8`, `FUN_01ab3778`, `FUN_01b521e4` (field
  spawn paths).

### PROBABLE: the base denominator

The notes give `N/4096` with `N` = number of PID tries: base 1, +1 research level 10, +2 perfect,
+3 shiny charm, +25 standard mass outbreak (the decoded `huge_outbreak.bin` `f7_raw` = 25),
+12 massive mass outbreak (the decoded `new_huge_outbreak_lottery.bin` `f14`/`f17` = 12). No local
file or code constant carries the 4096; it stays as `rare_palette::RARE_BASE_DENOMINATOR` with the
PROBABLE label.

### Port behaviour

`rare_palette::is_rare(seed, ctx)` rolls `tries / 4096` once per streamed spawn (deterministic in
the spawner seed). The research bonuses are live (species points/perfect from the save), the charm
is wired but stays `false` (the port has no charm item flow), the event bonuses come from the
spawner id (`we:ob` = 25, `we:mmo` = 12). A rare creature keeps its flag even when the cache has
no exported rare material (only Pikachu's `pm0025_00_00_rare_idle.gltf` exists today), in which
case it keeps the normal model.

## 3. Outbreak species

### Standard mass outbreaks (VERIFIED links, sizes cross-checked)

The earlier pass is unchanged: 227 outbreak spawners (46/44/44/48/45 per area), each with a
100-weight normal slot and a 1-weight alpha slot; the decoded per-area row carries 20 (`f1_raw`),
10-15 (`total_min`/`total_max`) and 25 (`f7_raw`). The notes explain the three numbers as 20 %
activation per map transition, 10-15 Pokémon per outbreak and +25 rare tries (PROBABLE). The port
exposes them as `OutbreakArea::activation_percent()`, `total_min..=total_max` and
`rare_bonus_tries()`.

### Massive mass outbreaks: decoded settings + labelled species pool

- VERIFIED values (`data/world_events_mmo_areas.tsv`): wave-1 size 8..10 (`f07`/`f08`), wave-2
  size 6..7 (`f09`/`f10`), `f14` = `f17` = 12. The notes read them as wave sizes and +12 rare
  tries (PROBABLE). The notes say the second wave holds 6-8; the port keeps the decoded 6..7.
- The **group to species mapping stays UNKNOWN**: the 334 encounter keys of the 166 group
  entries do not exist in any `encount_data.bin`, in the 2,066 extracted entries of
  `resident_release.gfpak`, in the loose `pokemon/data` files or as data in `main.elf`. The
  report's check is `outbreak::massive_outbreak_report` (0/334 resolved; test kept).
- Species source in the port: the notes' per-zone pools (`outbreak::MMO_SPECIES_POOLS`), labelled
  **PROBABLE**. `mmo_pool_split` crosses each pool with the decoded outbreak spawners of the zone:
  the base forms that also appear in the game data (VERIFIED subset, e.g. area 01: 44 of 60 names)
  and the evolution-stage extras (notes only). The wave-2 split (765/120/85/30 per mille) and its
  alpha chances (10 % base form / 9 % evolution — the decoded group child weights 81/10/9 carry
  the same pair) are PROBABLE constants. `--outbreak mmo` forces one for tests and captures; the
  second wave starts when the first is cleared.

## 4. Still UNKNOWN

- The base denominator of the rare roll (4096) — external note only.
- The massive-outbreak group-to-species mapping (the run-time key names are unreachable from the
  local files).
- Whether the random-alpha gate should be enforced in the port (needs the Noble-clear flag in the
  save; the port has none).
- How the game draws an outbreak id and when (unchanged from Brief 39).

## 5. Tests and verification

- `cargo test -p pla --lib`: 1,139 passed (new tests: the rare-palette try counts against
  `data/pokemon_variants.tsv`, the empirical rate, alpha slots survive the load, the decoded alpha
  share stays in 0.8 %-5 %, deterministic alpha rolls, fixed alphas always alpha, the
  `FSYS_NS_0N_CLEARED` gate read, the MMO settings/pools/wave plan).
- Boot guard (`cargo test --example port_viewer -- --test-threads=1 play_sh_area_1_starts`): PASS.
- No-window captures with the automation switches `PLA_DEBUG_FORCE_SPAWN_ALPHA` /
  `PLA_DEBUG_FORCE_SPAWN_RARE` (see the session log; files under
  `/home/nico/work/capturas_datos_alfas/`).
- The notes' per-zone lists were cross-checked against the decoded outbreak spawners; the
  generation script and figures are in the session log (`work/progress/datos-alfas.log`).
