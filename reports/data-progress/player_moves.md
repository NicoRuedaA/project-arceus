# Player movement: jump, fall, climb, dodge, crouch and hide (Briefs 13 and 22)

**In plain words.** The protagonist can now jump, fall off ledges, land, and grab a climbable
surface. The numbers (jump power, gravity, step height, air control, fall-damage heights) come
from the game's own player parameter files, which this task decoded into a sheet. The jump itself
is a port decision: the original game has no plain on-foot jump (jumping is something you do
while riding), so the on-foot jump reuses the decoded first-jump power. No ladder, rope or vine
placement has been decoded, so climbing is exercised by tests and by a debug ladder you can ask
for, not by real map data.

Sheet: [`data/player_controller.tsv`](../../data/player_controller.tsv) (310 parameters).
Decoder: `.tools/player_config_decode.py`. Code: `crates/pla/src/game/player_motion.rs`
(vertical state machine), `crates/pla/src/game/interact.rs` (climbables), clip selection in
`crates/pla/src/game/player_anim.rs`, one jump binding in `crates/pla/src/game/input_map.rs`.

## Sources

| File (`bin/misc/app_config/`) | Rows | Content |
|---|--:|---|
| `player_controller_config.bin` | 148 | locomotion speeds, jump powers, air control, step height, swim depths, stamina, roll angles, navigator, idle-motion intervals |
| `player_config.bin` | 138 | pinch (low HP) thresholds, fall/water penalty tiers, invincibility frames, input-buffer frames, camera, respawn, knock-back |
| `player_collision_config.bin` | 24 | body-part hit volumes, `talk_radius` 1.7, `Item_pick_radius` 1 |

Layout (recovered by walking the FlatBuffers, no schema): root field 0 is a vector of parameter
tables `{name, kind_hash u64, type_code, min, max, value vector<string>, extra}`. 310/310
entries decode with a non-empty name; 258/261 numeric values lie inside the file's own
`[min, max]` (3 exceed it: `control_maxCrouchingSpeed`, `control_airHorizontalVelocityMax`,
`navigator_maximumSpeed`). Values are copied verbatim as strings.

## Decoded values used by the code

| Constant | Value | Source parameter | Status |
|---|--:|---|---|
| `JUMP_SPEED` | 3.25 m/s | `control_firstJumpPower` (second/third/fourth = 0, so no double jump) | value VERIFIED, reading it as a take-off speed ASSUMED |
| `GRAVITY` | 9.8 m/s^2 | float at VA `0x0413a0f8` of `main.elf`, used by the roll tick `FUN_01a14f80` as a scale | value VERIFIED (read from the ELF segment), role PROBABLE |
| `MAX_FALL_SPEED` | 50 m/s | `control_maxFallSpeed` | value VERIFIED, units PROBABLE |
| `FIND_GROUND_RANGE` | 0.15 m | `find_ground_range` | value VERIFIED, role PROBABLE |
| `GROUND_MARGIN` | 1.5 m | `jump_groundMargin` | value VERIFIED, role ASSUMED (used as the vertical grab slack of climbables) |
| `STEP_HEIGHT_MAX` | 0.4 m | `control_stepHeightMax` | value VERIFIED; used as the ledge threshold (ASSUMED use) |
| `AIR_MOVE_RATE` / `AIR_HORIZONTAL_SPEED_MAX` | 0.25 / 11 | `control_airMoveRate`, `control_airHorizontalVelocityMax` | value VERIFIED, units PROBABLE |
| `FALL_PENALTY_HEIGHTS` / `FALL_PENALTY_DAMAGE` | 7, 10, 14, 19 m / 0, 45, 55, 70 | `penalty_landHeight0..3`, `penalty_landDamage0..3` | values VERIFIED; damage unit UNKNOWN, not applied (no player HP in the port) |
| `LAND_TIME` | 0.35 s | none | ASSUMED (clip length not measured) |
| `CLIMB_SPEED` | 1.5 m/s | none | ASSUMED (on foot; no decoded on-foot climb exists) |
| `SNEASLER_CLIMB_SPEED` | 4.8 m/s | `climb_move_speed_max` of `ride_nyuura_controller_config` (`data/ride_params.tsv`) | value VERIFIED, unit PROBABLE as m/s (2026-10-15) |
| `APPROACH_GRAB_REACH` | 4 m | none | ASSUMED (the base is a 1 m scan cell and the walkable navmesh can stop ~3 m short of it, measured on foot) |

## Behaviour

- Vertical machine `Grounded -> Airborne -> Grounded` with a landing timer that drives the
  exported `land` clip; clips `jump_start` (rising) and `jump_fall` (descending).
- A grounded avatar whose ground drops more than a step (0.4 m) falls instead of snapping down
  (`is_ledge_drop`; the viewer's ground snapping skips such drops). An avatar below the ground
  (jump into a slope) lands at once, so it never tunnels through the terrain.
- Horizontal speed in the air is the ground speed scaled by `control_airMoveRate`.
- Climbing: `Confirm` near a `Climbable` attaches (full 2..6 m radius); **walking into one**
  attaches too (approach grab, 4 m reach, movement headed at the base — a port reading of the
  original Sneasler wall grab); up/down input moves along it; jump lets go; the top steps off
  (falls back if there is no platform, as with the debug ladder). While mounted on Sneasler the
  climb uses the decoded 4.8 m/s instead of the ASSUMED 1.5 (2026-10-15, user-report fix).
- Discoverability (2026-10-15): the viewer shows a bottom-centre prompt while a climbable is in
  reach or while climbing (`game::interact::climb_hint_line`, exact key names) and a controls
  line when a mount is taken (`game::ride::mount_controls_line`). Both strings are Spanish
  because they answer a direct user request; earlier HUD strings stay English.
- Swimming (`game::water`) wins over the whole machine; teleports (arena, save load, zone change)
  reset it. Systems run only in `GameState::World` and when not paused.
- Bindings: Jump = keyboard `K` (`X` is taken by the bait throw in aim mode) / gamepad west button. The gamepad north button stays the dodge.

## Not derived from data

- ~~No ladder, rope, vine or climb-surface placement is decoded~~ **Resolved 2026-10-15**: the
  original has no ladder or rope objects at all (`ropes_ladders_unknown.md`); the real climbable
  surfaces are the terrain cliffs, derived from the heightmaps with the decoded Sneasler angle
  rule and loaded per area from `data/field_climb_surfaces.tsv` (`climb_surfaces.md`). The debug
  ladder (`PLA_DEBUG_CLIMB=1`) stays for eyeballing without data.
- No climb clip exists in the 423 exported-capable player clips by name (see Brief 22 below); the
  climb uses `walk` as a placeholder.
- Doors, warps and ropes tied to zone changes are Brief 8b (another worktree), not here.
- Fall damage, stamina (`stamina_max` 1300, 8 per frame recovery and sprint cost, 60 frame penalty
  are decoded) and the roll i-frames are not applied to any state in the port.

## Verification

Unit tests of the pure physics (apex height vs `v^2 / 2g`, airtime, terminal speed, landing
rules, ledge test, fall tiers) and headless Bevy tests with synthetic key input (jump, climb up
and step off, jump off a ladder, ignore a far ladder, approach grab towards/away/past, mounted
Sneasler climb speed). Nobody has seen it in a window: how the jump and the climb feel is for
the maintainer to judge.

**Real-key reproduction and fix (2026-10-15, task trepar2).** Played the viewer with the same
input path as the maintainer (`--automation` + `gamectl.py` keyboard events, no teleport, no
forced state) at area-01 patch 1180 (base (716, 607), 4 m, 50-61 deg). Before the fix: walking
north with `W` up the slope never grabbed (the mechanic only listened to `Confirm`, which the
guide did not document and no on-screen prompt named), and on 50-61 deg slopes the walk simply
climbs the terrain on foot (navmesh and terrain sampling let it rise ~7 m), so nothing reads as
a "wall". After the fix, all with real keys: spawn (714, 617) -> `W` north -> auto-grab at the
base ((716, 45.2, 607), motion `Climbing`) -> `W` to the crest -> step-off at (720, 49.8, 600.8);
`Enter` grab -> `S` down -> bottom exit at (716.1, 44.65, 606.3); mounted on Sneasler (V) the
same grab climbs 2.08 m per 25 frames ~ 5 m/s (4.8 decoded) vs 0.65 m = 1.56 m/s on foot.

**Final key pass (2026-10-16, same code as merged).** Replayed on a fresh run (area 01, spawn
(496, 354), walking to real cliffs 30-42 m out, no teleport): the in-reach prompt shows
(`61`), walking `D` into the wall auto-grabs (`62`), `W` climbs and steps off on the crest,
`Enter` grabs and `S` releases at the bottom (`63`), mounting Sneasler (`G` x3 + `V`) shows the
controls line (`64`), and on the 7.8 m patch 914 a mid-wall climb measures **0.96 m in 12 frames
= 4.8 m/s mounted vs 0.30 m = 1.5 m/s on foot** (`65`/`67`), crest at y=40.75 (`68`). Note: the
on-foot approach grab needs the walk to actually move into the face; against a face whose
navmesh foot blocks the step at 0 m (measured at patch 914) it does not fire until the avatar
walks in from a metre or two back, which is the normal way a player runs at a wall.
Captures (offscreen PNGs, UI included) in `/home/nico/work/capturas_trepar2/`: `00`/`01`/`02`
(the old build: in reach of the wall and NO prompt on screen — the reported failure), `10_aproximacion_con_aviso`
(the fix: prompt while in reach), `20_subiendo_auto` (climbing after walking in, prompt visible),
`25_agarre_con_enter`, `26_fondo_bajado`, `30_cima` (no prompt at the crest),
`40_montado_sneasler_toast` (mount controls line), `41_sneasler_trepando` (mounted mid-wall).
