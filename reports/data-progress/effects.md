# Visual effects: census, move mapping, particle format and playback plan

Written 2026-10-13 (worktree `efectos-movimientos`, brief 42 phase "census + format").

**In plain words.** The game keeps every move's look in two kinds of files: a *timeline* per
move (which particle bursts to start, on whom, at which frame, plus sounds) and the *particle
files* themselves (how each spark, flame or glow is born, moves, changes colour and fades).
Both formats can now be read by our own Rust code. Which timeline a move uses is settled by
the game's own file-name rule. A small picture renderer (no window, no GPU) draws three frames
of a real effect from the decoded data, so the decoding produces believable pictures, but
several timing and motion details are still assumptions. Nothing is wired into the game yet;
this report ends with the plan for that.

## 1. Census (VERIFIED: counted by `examples/effect_census.rs`)

`bin/effect`: **2,108 files, 2,530.3 MiB**.

| Folder | Files | Size | Format |
|---|---:|---:|---|
| `battle/particle` | 1,198 `.ptcl` | 1,809.3 MiB | VFXB particles (231 folders: `ewNNN` moves, `es*` status, `ee*` events, `cmn_*` common, `en*` noble/field-attack) |
| `battle/sequence` | 304 `.wbin` | 1.0 MiB | FlatBuffers timelines (196 `ew*` move files) |
| `battle/camera` | 32 `.trcma` | 0.3 MiB | camera animations (not decoded) |
| `battle/model` | 14 `.tranm` + 10 `trac*` | <0.1 MiB | ball model animation (not decoded) |
| `field/particle` | 274 `.ptcl` | 343.3 MiB | overworld effects |
| `demo/*` | 222 `.ptcl` | ~385 MiB | cut-scene effects (story: out of scope) |
| `poke/particle` | 54 `.ptcl` | 31.7 MiB | per-species effects (8 species folders, e.g. `pm0077` Ponyta) |

* Every `.ptcl` starts with `VFXB` and is VFX version `0x16`; **1,748/1,748 parse** with
  `pla::assets::ptcl`, 1,742 carry an embedded `BNTX` texture bank that parses (6 have none).
* Totals over all particle files: **9,647 emitters** (850 are child emitters), 11,202 texture
  names. Shapes: 5,341 billboards, 2,113 streaks (velocity-look), 1,885 flat polygons, 257 mesh
  emitters, 51 ribbons. Blend byte: 7,446 normal, 1,734 add, 212 subtract, 255 other.
* All **304/304 sequences parse** (133 distinct command types, 990 particle plays, 674 distinct
  particle paths, all existing). 674 of the 1,198 battle particle files are played by a
  sequence; the other 524 (ball sets `eb_set`, form-change auras, ...) are started by code.
* Packs: a scan of all **678 `.gfpak` (102,415 entries)** finds 11 embedded VFXB, all in
  `archive/field/resident_release.gfpak` (weather, flare, warp-area, HP sign). **11/11 names
  resolved** by FNV-1a 64 of the full path (candidate folders from `bin/effect/...` strings
  in the executable), e.g. `bin/effect/field/particle/weather/ef_weather_04_snow/ef_weather_04_snow_cam01.ptcl`.
* Small index: [`data/effect_index.tsv`](../../data/effect_index.tsv) (1,748 rows: emitters,
  textures, add emitters, longest life, mesh/ribbon emitters, shapes, sequences that play it).

## 2. How a move selects its effect

**Rule (VERIFIED).** The function `FUN_0089f36c` (update main NSO; read-only study) formats
the sequence name with `"ew%03d.wbin"` from the move id. The data agrees exactly: the 177
`ewNNN.wbin` files are the 177 moves whose waza record has `style_flag = 1` in
`data/pml_move_effects.tsv` (the moves usable in this game), with no extra and no missing.
The same function swaps in **15 species/form-specific** timelines; their (move, species, form,
file) rows were read from the table it indexes in `main.elf` (pointer relocations resolve to
`ew449_pokeonly` ... `ew827_pokeonly`, `ew467_1`), e.g. Judgment by Arceus, Roar of Time by
Dialga (both forms), Shadow Force by Origin Giratina, the noble/hisuian signature moves.
`ew830/836/844_pokeonly` exist but this function never selects them (UNKNOWN user).

**Output.** [`data/move_effect_map.tsv`](../../data/move_effect_map.tsv): 192 rows (177 default
+ 15 overrides), with sequence length, first frame a particle lands on the target, particle
plays (`path@start-end:side`), sound events (Wwise names) and confidence columns. Runtime
helper: `pla::assets::effect_sequence::sequence_name(move, species, form)`.

**Spot checks (LIKELY by name and content):**

| Move | Sequence | Particles (side) | First target frame |
|---|---|---|---:|
| Thunderbolt (85) | `ew085` | `at_start` (user) x3, `beam01` (user), `df_hit01` + `spark01` (target); textures `cpt_2_thunder*` | 38 |
| Ember (52) | `ew052` | `fire_muzzle`, `bullet` (user), `hit` (target); flame colours yellow to red | 24 |
| Quick Attack (98) | `ew098` | `at_bgkem` x2, `hideline` (user), `df_hit` (target) | 37 |
| Tackle (33) | `ew033` | `at_bgkem` (user), `df_hit` (user and target), `cmn_collision_dummy` | 30 |
| Hydro Pump (56) | `ew056` | `charge`, `muzzle`, `beam01` (user), `hit01`, `bom01` (target) | 40 |

Surf (57) and Water Gun (55) are **not in this game's move pool** (`style_flag = 0`, no
sequence), so they have no effect to map; Hydro Pump is the water example above.

**Coverage.** 177/177 usable moves have a VERIFIED file mapping. Timings and sides are LIKELY.

## 3. Sequence format (`.wbin`)

FlatBuffers without schema: root field 0 = vector of tracks; a track has an id table (field 0),
a target table (field 1, RAW) and a vector of commands (field 2). A command is a union: u8 type
(field 0) and a table (field 1) whose field 0/1 are start/end frames for almost every type.

| Type | Meaning | Confidence |
|---|---|---|
| `0x02` | play particle; field 2 = path under `bin/effect/battle/particle/` | VERIFIED (paths exist) |
| `0x09` | placement of the preceding `0x02`: field 2 side (0 user, 1 target), field 3 locator id | side LIKELY (93 % of `_at_` names are 0, 97 % of `_df_`/`hit` are 1); locator ASSUMED |
| `0x0d` | particle scale over a frame range (vec3) | LIKELY |
| `0x04` | stop particle at a frame | LIKELY |
| `0x94`/`0x95`/`0x96` | sound event (field 3 = `PLAY_EW0085_01` ...) | VERIFIED names; timing LIKELY |
| `0x59`/`0x61`/`0x68`/`0x69` | camera animation (`.trcma` paths) | LIKELY by strings |
| `0x1c`-`0x1e`, `0x3e`-`0x40`, `0x70`-`0x72` | Pokémon animation-graph triggers (`one_attack_int`, `stun01_bool` ...) | LIKELY by strings |
| others (~120 types) | camera shake, colour, motion, UI text... | UNKNOWN |

Frame rate of sequence frames: ASSUMED 30 per second (the game runs at 30 fps); not proven.

## 4. Particle format (`.ptcl`, VFXB 0x16) and decoded fields

Structure (VERIFIED on 1,748 files): 0x20-byte header; chain of sections each with a 0x20-byte
header (magic, size, child, next, attribute and data offsets, child count). `ESTA` → `ESET`
(name at data+0x10) → `EMTR` emitters (child emitters nest), `GRTF` (`GTNT` id→name table +
`BNTX`), `PRMA` (`PRIM` meshes), `G3PR`, `GRSN` (shaders). Emitter data is always **2,752 bytes**.

| Field | Offset | Confidence and evidence |
|---|---|---|
| name | 0x10 | VERIFIED (readable names: `glow`, `fire`, `spark02_up_side_add`) |
| 6 key-frame tables (8 keys of x,y,z,time) and their counts | 0x3C0.., counts 0x60.. | VERIFIED structure: 9,647/9,647 emitters have zero keys past the count and non-decreasing times |
| colour 0 / alpha 0 / colour 1 / alpha 1 / scale over life | tables 1-5 | LIKELY: alpha tables always have x=y=z; Ember flames go yellow → orange → red; glows fade 1 → 0 |
| table 6 | 0x680 | UNKNOWN meaning |
| texture of samplers 0/1/2 | 0x9F0/0xA10/0xA30 (u64 id) | VERIFIED: 15,335/15,335 ids found in the file's `GTNT` |
| atlas cells (u, v) of sampler 0 | 0x308 | LIKELY (1024x128 flame strip = 8 x 1) |
| flip-book cell list | 0x110 (count), 0x120 (cells) | RAW / ASSUMED timing |
| display type (billboard, Y-billboard, polygon XY/XZ, velocity-look...) | 0x8AA | LIKELY (ring/wave names → polygon, line/spark → velocity-look) |
| blend (0 normal, 1 add, 2 subtract) | 0x89E | LIKELY (value 2 in 69 % of `_sub` names, 0 % of `_add`/`_nor`; value 1 in 46 % of `_add`). Half of `_add`-named emitters read 0, so some additive looks come from the shader: open question |
| shader index | 0x914 | VERIFIED dense `0..n` per file (1,748/1,748) |
| mesh id | 0x8D0 | VERIFIED resolves to a `PRIM` id in the sampled file |
| ribbon (stripe) | `EP03` attribute | LIKELY (`stripe`, `trail`, `tail` names) |
| particle life (frames) | 0x8B8 | LIKELY (hit marks 12, glows 24, smoke 28-64) |
| particles per emission | 0x800 (f32) | LIKELY (dust `kona_up` 50, glow 1) |
| one-time flag | 0x7F0 | LIKELY (0 in 82 % of `loop` emitters, 1 in 76 % of others) |
| constant colours 0/1 (RGBA) | 0x9A0 / 0x9B0 | LIKELY |
| emission start/duration/interval, life random, gravity, base size, all-direction speed | 0x7F4/0x7F8/0x7FC, 0x8BC, 0xB0, 0x9C0, 0x858 | ASSUMED |

Example, decoded from `ew052/ew052_hit.ptcl` (Ember hit; values straight from the file):

| Emitter | Shape | Blend | Life | Rate | Colour over life | Alpha over life | Size over life | Texture |
|---|---|---|---:|---:|---|---|---|---|
| `fire2` | billboard | normal | 28 | 6 | (1,.56,.14)@.15 → (1,.19,0)@.41 → (1,.06,0)@.80 | 1 | 0 → 1@.19 → 1.3@1 | `cpt_2_fire0004` (8-cell flip-book) |
| `fire` | streak | normal | 28 | 6 | (1,1,.14)@.12 → (1,.44,.04)@.43 → (.85,.1,.03)@.8 | 1 | 0 → 1@.19 → 1.3@1 | `cpt_2_fire0005` |
| `hit_mark` | billboard | normal | 12 | 1 | (1,1,.83) → (1,1,.18)@.19 → (1,.1,0)@.89 | 1@.2 → 1@.43 → 0@1 | 1.85 → 1.61@.13 → .14@.26 | `cpt_0_shock0009` |
| `glow` | billboard | add | 24 | 1 | constant (1,.55,.17) | 1 → .71@.09 → 0@1 | .5 → 1@.23 → 1.76@1 | `cpt_1_circle0004` |
| `kona_up` | streak | add | 64 | 50 | (1,.88,.03)@.02 → (1,.2,0)@.78 | 1 → 1@.52 → 0@1 | per-axis | `cpt_0_mark0002` |

Rust: `pla::assets::ptcl::{PtclFile, Emitter, KeyTable}` (`Emitter::color_at/alpha_at/scale_at`,
`emission_schedule`, `shape`), `pla::assets::effect_sequence::{EffectSequence, sequence_name}`.
Unit tests: synthetic file layout, key interpolation, synthetic emitter particle counts
(one-time burst = rate; loop of 1 every 2 frames for 10 frames = 5), 5 real particle files
(emitter/texture counts, names, display, blend, life, every sampler id resolves, every texture
name present in the embedded BNTX), Thunderbolt and Ember timelines (paths, sides, sounds).

## 5. CPU preview (no window)

`cargo run -p pla --example effect_preview -- ew052/ew052_hit.ptcl` writes
`work/effects/preview_ew052_hit.png` (3 frames: 3, 9, 18; front view auto-fitted). I looked at
it: flame flip-book cells spray outward and shift from yellow to orange to red, with small add
sparks around a dim glow. `ew085/ew085_spark01.ptcl` (Thunderbolt) shows a yellow star flash
with jagged electric bolts that fades out by frame 18. These pictures prove the curves, colours,
textures and atlas cells decode into a believable shape; **positions and speeds are guesses**
(all-direction launch at the ASSUMED speed field), and the camera is not the game's.

## 6. Playback plan for Bevy (design only; no game code changed)

**Data flow.** `BattleMove.id` (crates/pla/src/battle.rs, `BattleMove { id, name, ... }`) →
`sequence_name(id, species, form)` → `EffectSequence` → per-track particle plays (start/end
frame, side, locator) → `PtclFile` per path (cached) → emitters.

**Runtime pieces (a new `game/vfx.rs`, written by the later agent):**
1. `EffectLibrary` resource: lazy cache `path → Arc<PtclFile>` plus GPU textures. Texture
   prep offline or on load: decode each BNTX texture once to RGBA (BC4/BC5 masks become alpha),
   pack per-file textures into an atlas, keep the decoded `atlas_div` cells as UV rects.
2. `EffectPlayer` component (one per playing particle file): owner slot (user/target), start
   time, local frame counter, emitter states (next emission frame, alive particles).
3. Particles as a CPU struct-of-arrays per player (position, velocity, age, life, seed), stepped
   at the sequence frame rate; colour/alpha/scale from `KeyTable::sample(age/life)`.
4. Rendering: one instanced quad mesh per (texture page, blend) with per-instance data
   (centre, size, rotation, uv rect, rgba). Two materials: alpha blend (`BlendType::Normal`)
   and additive (`Add`, emissive-style, depth write off); `Sub` later. Billboards face the
   camera; streaks stretch along velocity; polygon XY/XZ are flat quads; mesh emitters and
   ribbons are phase 2.
5. `EffectSequencePlayer` component: plays one `.wbin`, spawning an `EffectPlayer` at each
   `0x02` start frame on the slot given by the side byte, stopping at `0x04`/end frame,
   applying `0x0d` scale; exposes `first_target_frame` so the hit reaction and HP bar can wait for it.

**Battle events → effects** (positions: slot transforms of `PokemonBattleSlot(0|1)`,
locator ids mapped later to bones; until then the Pokémon's centre at ~half its height):

| Event | Effect |
|---|---|
| `Event::UsedMove { actor, target, name, damage }` | sequence of the move id (user = actor slot, target = target slot); Hit reaction delayed to `first_target_frame / 30` s |
| `Event::Missed` | same sequence, target particles skipped (ASSUMED) |
| critical hit / type effectiveness | `cmn_critical`, `cmn_affinity_advantage/disadvantage/none` |
| status from `take_effect_events()` | `es002_poison`, `es003_fire` (burn), `es004_ice` (frostbite), `es005_paralysis`, `es001_nemuke` (drowsy), `es011_status_up` / `es012_status_down`, `es008_heal`, `es015_sick_heal` (names LIKELY) |
| `Event::KnockedOut` | no particle file found by name yet (UNKNOWN); keep the `down01` clip |
| battle start / item / capture | `cmn_battle_start_*`, `cmn_item_use*`, `ee101` ball sequences |

**Exact hook points for the later agent:**
* `crates/pla/src/game/battle_anim.rs`: `requests_for_events` (lines ~47-84) builds the per-turn
  timing (`MOVE_STEP_S` 1.4 s, `HIT_DELAY_S` 0.45 s). Add a sibling pure function
  `effect_spawns_for_events(events, move_id_of) -> Vec<TimedEffect>` with the same cursor logic,
  and later replace the fixed delays by the sequence length and `first_target_frame`
  (Thunderbolt is 91 frames, about 3 s, so 1.4 s per move is too short). `drive_battle_anims`
  (~96-130) shows the pattern to copy: a `Local<usize>` cursor over `ActiveBattleSession::event_log`.
* `crates/pla/src/game/battle_controller.rs`: `ActiveBattleSession` (~181) owns `event_log`;
  in the turn code (~391-405) `play_turn` returns the events and `take_effect_events()` the
  status events. Status effects are currently only turned into log lines; to show `es*` effects,
  keep them (e.g. a parallel `effect_log`) instead of dropping them after formatting. The move id
  for an event comes from `battle.combatants[actor].moves[..].data.id`.
* Register an `EffectsPlugin` with one `add_plugins` line next to `BattleAnimPlugin` in the viewer.

**Effort (estimates):** textures + library + billboard/streak renderer with both blends ~3-5
days; sequence player + battle hooks ~2-3 days; ribbons, mesh emitters, child emitters and the
field attributes (`FRND` random, `FSPN` spin, gravity) ~1-2 weeks; proving the ASSUMED emission
and speed fields (frame-by-frame comparison with the real game by eye) open-ended.

**Top 12 to cover first.** Each move has its own particle folder (the only sharing found: Mach
Punch (183) reuses two Quick Attack files, and the invisible `cmn_collision_dummy` is played by
17 sequences), so "most common" is measured two ways:
* Moves most likely to be seen (rows in `data/pml_learnsets.tsv`, LIKELY proxy): Tackle (33),
  Bite (44), Double-Edge (38), Crunch (242), Quick Attack (98), Confusion (93), Ancient Power
  (246), Slash (163), Astonish (310), Iron Defense (334), Psychic (94), Hydro Pump (56).
* Building-block textures shared by most move effects (primary texture of `ewNNN` emitters,
  number of moves): `cpt_1_circle0004` 100, `cpt_1_circle0001` 94, `cpt_1_circle0005` 91,
  `cpt_2_shock0014` 88, `cpt_1_water0001` 54, `cpt_1_circle0003` 38, `cpt_2_dust0001` 37,
  `cpt_3_flow0016` 37, `cpt_cmn_white0001` 33, `cpt_2_shock0026` 29, `cpt_0_mark0006` 29,
  `cpt_2_shock0003` 26. A billboard renderer that handles these covers most hit flashes.
* Plus the shared battle-wide ones: `cmn_critical`, `cmn_affinity_*`, `es002`-`es005` status.

## Verified / Likely / Assumed / Unknown

* **Verified:** file census and sizes; VFXB section layout and 2,752-byte emitter block on all
  1,748 files; key-table structure; texture-id resolution; shader index density; 11/11 pack
  VFXB names; move → sequence rule (format string + 177/177 set equality) and the 15 overrides;
  particle paths in sequences exist; sound event names.
* **Likely:** colour/alpha/scale table roles, display type, blend 0/1/2, life, emission rate,
  one-time flag, constant colours, atlas cells, sequence side byte and start/end frames,
  ribbon = `EP03`, status sequence meanings by name.
* **Assumed:** emission start/duration/interval, life randomness, gravity, base size,
  all-direction speed, flip-book timing, 30 frames per second, locator ids, the preview camera.
* **Unknown:** table 6; blend values 3-5 and the additive look of `_add` emitters that read 0;
  most of the 133 sequence command types; `.trcma` camera and `.tranm`/`trac*` ball animation
  formats; emitter shapes (sphere/circle volumes) and the field attributes' parameters; who plays
  `ew830/836/844_pokeonly`; whether the preview matches the game (never compared by eye).

## Reproduce

```bash
python3 .tools/effect_sequence_decode.py                       # data/move_effect_map.tsv
cargo run -p pla --example effect_census -- --packs --write    # census + data/effect_index.tsv
cargo run -p pla --example effect_preview -- ew052/ew052_hit.ptcl
cargo test -p pla ptcl effect_sequence
```
(`PLA_BIN_ROOT` overrides the game data root; `PLA_MAIN_ELF` the executable used only for
path-name candidates.)
