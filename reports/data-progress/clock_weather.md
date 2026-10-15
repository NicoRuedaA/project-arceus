# Day/night clock, blended zone lighting and weather (Brief 9)

Implemented in `crates/pla/src/game/{clock,weather,lighting,render_look,save_game,save_wire}.rs` on branch
`feat/game-clock` (worktree `game-clock`; one worker did both parts the brief suggested as `game-clock` and
`weather`). The viewer was **not** run (project rule 7); everything below was verified with unit tests and
one windowless `look_headless --clock-report` run (six renders in a single process).

## What the data says (and what it does not)

- **Decoded**: the per-area weather weights and `f1_raw`/`f2_raw` in `data/world_events_area_weather.tsv`
  (`ha_area00`/`06` always sunny; area 05 snow 25 / snowstorm 20; rows sum to 100, LIKELY), the global
  default weights in `data/weather/weather_lottery_matrix.tsv` (40/20/15/10/5/5/3/2) and the 12 spawn
  multipliers (4 time slots then 8 weathers) in `data/field_spawn_conditions.tsv`.
- **VERIFIED**: slot index 3 is night (spawn data: 23 night-only species land in the 4th multiplier).
- **PROBABLE (external hint, no game file carries the hours)**: the band boundaries are 05:00, 12:00,
  18:00 and 21:00 (morning 05-12, day 12-18, evening 18-21, night 21-05) and the default speed is one
  game minute per real second (one game hour per real minute, a day in 24 real minutes) — the hint's
  rate, which the old default already used. `reports/data-progress/world_events.md` states that the
  hour boundaries are not in any unpacked file. The hint was adopted as PROBABLE (not verified): its
  four bands are consistent with the verified slot order and with slot 3 = night, and nothing in the
  data contradicts it.
- **ASSUMED**: the order of slots 0..2 (morning/day/evening; identical everywhere in the spawn data),
  and the `f1/f2` interval units (read as whole game hours: 2..4 or 4..6).
- **UNKNOWN**: the game's clock rate, the weather transition visuals, the `light.gfpak` weather "modifier"
  (offsets/factors) and the 63 `local_weather` placements. No decoded per-weather fog/light numbers exist,
  so every visual response below is this port's own.

## Clock (`game/clock.rs`)

- `GameClock { minutes_per_real_second, t_hours, pinned }`, pure and unit-tested; advances only while the
  world/battle runs and not paused (same run condition as play time). Default speed **1.0 game minute per
  real second** (a game day = 24 real minutes) — PROBABLE (the hint's rate), `--time-scale X` overrides it.
- `slot_blend(t)` returns the two surrounding slots and a smoothstep factor. Slot centres sit at
  08:30/15:00/19:30/01:00, so the band boundaries are 05:00/12:00/18:00/21:00 with a 50/50 mix —
  PROBABLE hours (the hint). At a centre the factor is exactly 0, so `--time-slot N` (which pins the
  clock to the slot centre) still shows exactly the decoded slot.
- `blend_env(a, b, f)` interpolates two `ZoneEnv`s component-wise (linear RGB; sun direction mixed and
  re-normalised, degenerate pairs fall back to the first slot). ASSUMED shape.
- Rest: the pause-menu Rest row (Brief 21) heals and now **advances the clock to the start of the next
  band** (05:00/12:00/18:00/21:00; the original's tent menu offers a chosen phase,
  `n2-base-camps-and-travel.md` §2, matching those four starts, `FUN_02499418`). The toast shows the new time.

## Lighting (`game/lighting.rs`)

The plugin loads all four decoded slots of the area (it used to load one) and keeps the environment in a
`LoadedZoneEnv` resource. `sync_environment` follows the clock and re-applies sun colour/intensity/
direction, ambient, clear colour, camera fog, cloud colour and the sky-gradient mesh at most once per
1/64 of a slot segment (~5.6 s at the default speed; the sky mesh is small and its old asset is removed on
swap). No `GameClock` resource (examples/tests): the plugin falls back to the fixed `slot` exactly as
before. Scene measurements from the single windowless run (area 00, 1280x720, vivid look, mean sRGB/linear
luminance over the frame, one process, six views — see `work/clock_report/frame_*.png`, git-ignored):

| view (clock) | weather | mean sRGB | mean linear |
|---|---|---:|---:|
| 03:00 night | sunny | 0.0377 | 0.0113 |
| 09:00 morning | sunny | 0.5101 | 0.3344 |
| 15:00 day | sunny | 0.5352 | 0.3555 |
| 21:00 evening | sunny | 0.4692 | 0.2936 |
| 15:00 day | rain (4) | 0.5112 | 0.3254 |
| 03:00 night | snow (5) | 0.0347 | 0.0111 |

Night is 7 % of the day view; rain darkens the day view by ~4.5 % of mean luminance (the particles add
light back), snow darkens the night view by ~8 %. The frame is the synthetic `look_headless` scene, not
game content; whether the transition "feels" right must be judged by the maintainer.

## Weather (`game/weather.rs`)

- Lottery: one weighted pick per change from the area's decoded weights (global defaults as fallback),
  `disable_system_flag` read but **ignored** (the flag system does not exist yet). One `XorShift32` stream
  per run (`--weather-seed`, default 0x9e3779b9); the game uses Xoroshiro128+, which is not ported.
- Duration: uniform whole hours in `[f1, f2]` (2..4 or 4..6; ASSUMED units). The machine advances with
  game-clock minutes, so a pinned clock (`--time-slot`) freezes the weather too, and `--weather <id|name>`
  forces one weather (renders/tests).
- Unit test: 10,000 draws with seed 0x12345678 match the decoded weights within **0.56 points** (max
  deviation; e.g. sunny 40.13 % vs 40.00 %).
- Visual response (`WeatherVisual::for_id`, all **ASSUMED** because no decoded weather parameters exist):
  fog multiplier 0.8 (drought) .. 3.0 (fog), light multiplier 0.65 (rainstorm) .. 1.08 (drought), and
  `render_look::apply_weather` applies them to `LookParams` (pure and tested; the CLEAR weather is exactly
  neutral). Rain = 800 streak quads falling at 17 m/s, snow = 450 flakes at 2.1 m/s with sway, spawned in a
  30 m box around the camera and recycled above it; one shared `Rectangle` mesh + unlit alpha-blend
  `StandardMaterial` per kind, movement on the CPU (no custom WGSL, no new crates). The particle budget and
  speeds are ASSUMED.

## Save format v6 (`game/save_game.rs`)

`CURRENT_VERSION = 6` (v5 is the party experience from connected progression). `PlayerSave::time_slot` (0..3) is replaced by `time_minutes` (0..1439, the clock's
minute of day); `PlayerProgress.time_minutes` mirrors the live clock every frame. Migration v5→v6 rewrites
`time_slot=N` to the centre minute of that band (N=0→510, 1→900, 2→1170, 3→60; the band hours are PROBABLE)
and a v5 file without the line gets the default start (15:00). v1-v4 files pass through their earlier steps
(v4→v5 adds the unknown experience) and are converted the same way. Out-of-range minutes are refused on write and on read.

## Verified by

- `cargo test -p pla`: **653 lib tests passed after merging main, 0 failed** (including 9 new in `clock.rs`,
  7 in `weather.rs`, 4 new/updated in `save_game.rs`, 1 updated in `save_wire.rs`). The real-sheet weather
  and lighting tests run against the committed TSVs.
- The single windowless run above (six views, one process, no window).
- Not verified: anything in a played/viewer session (rule 7), the original game's own look and timing.
- **Band update (2026-10-15):** the 05/12/18/21 bands, the 08:30/15:00/19:30/01:00 centres, the
  next-band rest jump and the v5→v6 migration centres are covered by `clock.rs`, `save_game.rs` and
  `dex_screen.rs` unit tests; full `cargo test -p pla --lib` green at merge time (1138 passed before
  the final merge) and the windowless smoke run green; not seen on screen by any agent.

## Unknown / not done

- The real slot hours are not in any unpacked file (the 05/12/18/21 bands are PROBABLE from the
  external hint); the game's own clock rate stays undecoded (the 1 min/s default matches the hint);
  real weather durations and visuals, `light.gfpak` weather modifier, `local_weather` placements and
  the `disable_system_flag` semantics are UNKNOWN.
- The clock exists (`GameClock::slot()`) but wild spawning still ignores the decoded `time_*`/`weather_*`
  multipliers (`data/field_spawn_conditions.tsv`) — that wiring is a follow-up; the report
  `world_events.md` describes the intended formula `weight x time_<slot> x weather_<w>`.
- Weather is not persisted in the save (it re-rolls on start), rain/snow have no sound, no wet/dry surface
  transitions (`weather_dry_transitions.tsv` is decoded but unused) and the `Water` material is untouched.
