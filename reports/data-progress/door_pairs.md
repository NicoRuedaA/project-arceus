# Door pairs and door prompt (Brief 13, door / warp part)

**In plain words.** Doors already worked since Brief 8: walk into a door box, press E, and the paired exit loads.
This step checks the whole door table as pairs (does every door have a way back?) and shows a small hint
("[E] Enter ...") while the player stands at a door, so a door is not an invisible trigger any more. Interiors
(houses, caves, tents) still cannot be entered: the hint says so, and the zone change is refused with a notice
(Brief 37). Jump, fall, climb and interact states are NOT part of this work (they live in the movement branch).

Code: `crates/pla/src/game/door_pairs.rs` (`DoorPairIndex`, `DoorPrompt`, `DoorPairsPlugin`), registered in the
viewer with one line. Windowless check: `cargo run -p pla --example door_pairs_report`.

## Verified (unit tests on the real `data/warp_links.tsv`, run headless)

- 110 `DOOR_*` rows: 110/110 name an exit row that exists, 110/110 exits sit in the destination area, 110/110 pairs are
  two-way (the exit's own exit is the door). (The earlier report counted 50/55 two-way *sub-area* pairs by a different
  rule, arrival on the navmesh in both directions; this check is only about names.)
- 55 doors lead to a main area (loadable today) and 55 into a sub-area (refused until Brief 37).
- 0/110 arrivals fall inside the exit door's own trigger box (+0.35 m pad), so arriving never lets the very next E
  press reuse the exit door.
- The prompt follows the player: shown on a door centre, hidden away from it (headless Bevy test).

## Assumed

- Door box axes (width along local x) and the 0.35 m pad: inherited from `zone.rs`, ASSUMED.
- Prompt wording and placement: invented UI (no game text is used).
- Every door is open; `gate_flag` (13 rows) is ignored (story is out of scope).

## Unknown

- **Ropes and ladders.** None of the 55 exported `gimmick_*` names is a rope or ladder (trees, rocks, crystals,
  grass, boxes, balloons, snow, `yachi01`, two temple gates). `bg_event` types are not decoded. Nothing was hooked up;
  `climbable_gimmick_candidates` returns an empty list on the real names.
- The door open/close animation and sound playback (`open_sound`/`close_sound` names are in the sheet; no audio here).

## 2026-10-15: interaction reach and interior destinations (Brief 37)

Both kinds of destination load now (main areas and interior sub-areas, see
`reports/data-progress/interiors.md`), so the hint is the same for both
(`[E] Enter <dest>`), and the prompt uses the *per-door reach* below, exactly like the E press.

The decoded navmesh excludes the door openings (the original data keeps indoor and outdoor
surfaces as separate islands), so the sheet trigger box can sit well away from any walkable point:
the fixed 0.35 m pad of Brief 13 left the Jubilife houses unreachable on foot (the doors tested on
2026-10-15 were 0.001 m to 0.65 m short; the S09 hall door needs ~1.9 m). `game::zone::DoorReachIndex`
measures, once per area change, the distance from each door centre to the closest walkable face
(one nearest-face query per door) and sets the door's reach to `max(0.75, distance + 0.1)`. Doors
whose centre is on the navmesh keep the tight 0.75 m base margin (this is the sheet-level figure
used by the `DoorPairStats::arrival_in_exit_box` check, still 0/110: arriving never re-uses its
exit door with one E press at the base margin). `crates/pla/examples/door_reach_report.rs` prints
the measured reach table (worst door: `DOOR_AREA00_S09_TO_AREA00`, ~2.4 m to its walkable island).
