# Terrain walk rule: real-key verification (2026-10-18)

Headless windowless viewer (**never a visible window**), real `KeyboardInput` events through
`.tools/gamectl.py` (`--automation`, dev build of this branch, own `--automation-dir`); no
teleports and no forced states for the movement checks. Capture files in
`/home/nico/work/capturas_terreno_colision/` (with `LEEME.txt`).

## What was driven

Area 00 (Jubilife Village), scratch save, follower Pikachu in the party:

| Check | Keys | Result |
|---|---|---|
| F1 cycle | `F1` once / twice / third press | off → ENCIMA (cyan walls + grey floors + **green heightmap grid glued to the ground, ~64 m**) → SOLO (same grid over a near-black background, **red = slope over 45 deg**) → off. Captures `f1_final_*.png` and `f1_muros_*.png` (walls cyan next to the houses) |
| Walk on plain | hold `W`/`S` | ~4.5 m/s, Grounded; the follower follows 2.5 m behind, `teleports` 0, `walked` matches the player's path |
| Gentle slope | `W` over the hills north of the spawn | Y follows the heightmap (75.8 → 95 → 83 → 102 m), never leaves the ground |
| Wall that blocks | `S`/`D` into the village fence and house walls | position stops at the collider (no penetration, no vibration); at the rocky slope the TREPAR prompt appears (the climb feature is unchanged and offers the intended grab) |
| Cliff edge | `W` off a 49 m drop | the drop is allowed by design: the player falls by gravity (Airborne) and lands Grounded at the bottom; climbing *up* the same face is blocked by the walk rule (only TREPAR) |
| House enter/exit | walk to `DOOR_AREA00_S03_E` (closed-loop steering on real keys), `E` | prompt `[E] Enter Interior ha_area00_s03_e` → E → zone `ha_area00_s03_e`, standing on the house floor (collider floors), Grounded; walk to the exit trigger, `E` → back to `ha_area00` at the door. Captures `puerta_casa3.png`, `interior_s03e.png` |

Area 01 (Obsidian Fieldlands), new session:

| Check | Result |
|---|---|
| 22 wild creatures streamed | the nearest (Drifloon) wandered 3.4 m x / 2.5 m z over 12 s (step frames), Y follows the terrain (60.4 → 61.0), stays inside the map, no teleport |
| Follower after a big fall | the follower stays on the edge right after the player falls (2D arrival), but walks down and keeps trailing as soon as the player moves away; never permanently stuck (observed 30 m vertical temporary gap) |

Smoke scenario (62 checks, GPU, same dev build):
`.tools/smoke_viewer.py --no-build --dir work/automation-terreno-colision` → **124 PASS, 0 FAIL,
exit 0** (area 01, time slots, every pause screen, save/load, wild battle to the end, trainer
battle `giratina01` via the real `V` key). Log: `smoke.log` in the same captures folder
(`work/capturas-terreno-colision/smoke.log`).

## Observations (not fixed here)

* At the base of a climbable surface, holding a direction *into* it re-grabs immediately after
  releasing with `K`; escaping needs walking away from the face. Pre-existing climb-feature
  behaviour (unchanged by this branch), noted for the climb task.
* The follower is 2D (trail following): after the player drops off a tall cliff, the follower
  can remain on the lip until the player moves away. No teleport loop, no stuck state.
