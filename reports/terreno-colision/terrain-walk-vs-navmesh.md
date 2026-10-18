# Terrain walk rule vs the decoded navmesh (areas 00 and 01)

Evidence for the user decision of 2026-10-17 (movement uses the port's own terrain rule,
`game::terrain_collision`; the navmesh is kept for the interior minimap and report helpers only).
The question this measures: **where does the port now allow walking that the original game's
navigation forbade, and where does it block something the navmesh allowed** — in square metres,
for the two first areas.

## Method

`cargo run -p pla --example terrain_walk_report -- --area 0,1 --step 4` (headless, no window).
Raw output: [`terrain-walk-report-area00-01.txt`](terrain-walk-report-area00-01.txt).

Every 4 m grid point of the terrain extent (1025 x 1025 m per area) is classified by two rules:

* **navmesh**: `NavMesh::is_walkable` (what the movement used before the change).
* **terrain rule** (static reading of `game::terrain_collision`): ground exists in the decoded
  heightmap, local slope <= `MAX_WALK_SLOPE_DEG` (45 = SUPUESTO), the decoded water sheet is not
  deeper than `SWIM_ENTER_DEPTH` (1.2 m) on foot, and the player capsule
  (`PLAYER_RADIUS` 0.35 x `PLAYER_HEIGHT` 1.8) is not pushed out by placed-object collision
  (`WorldCollision`, streamed in 100 m tiles so the whole area is covered).

Water comes from the water placement models decoded like `water_report.rs`; collision from the
same placement sheet + `_col.trcol` packs the viewer streams. Caveat: a static map cannot see
the walker's feet, so a steep point counts as blocked (a walker cannot stand there) and the
in-play "walking off a drop is always allowed" rule is not applied; both readings only widen
the reported differences, they do not hide any.

## Results

| Area | navmesh walkable | rule walkable | both | only navmesh (lost) | only rule (gained) |
|---|---|---|---|---|---|
| ha_area00 (Jubilife Village) | 64.20 ha | 71.99 ha | 63.80 ha | 3,952 m2 | 8.19 ha |
| ha_area01 (Obsidian Fieldlands) | 62.86 ha | 87.73 ha | 62.17 ha | 6,880 m2 | 25.56 ha |
| **total (both areas)** | **127.05 ha** | **159.72 ha** | **125.97 ha** | **10,832 m2 (1.08 ha)** | **337,488 m2 (33.75 ha)** |

Area 00 keeps **99.4 %** of its navmesh-walkable surface, area 01 **98.9 %**. The 1.08 ha lost in
total are, by cause (cells of 16 m2):

| Area | slope | deep water | placed colliders | several causes |
|---|---|---|---|---|
| ha_area00 | 9 cells (144 m2) | 224 cells (3,584 m2) | 14 cells (224 m2) | 0 |
| ha_area01 | 186 cells (2,976 m2) | 151 cells (2,416 m2) | 52 cells (832 m2) | 41 cells (656 m2) |

The deep-water losses concentrate on the moat/water margins of area 00 and the river banks of
area 01 (the navmesh drew a walkable margin the port's rule now blocks on foot; a mount crosses
them, `WalkContext::mounted`). The slope losses are cliff faces the navmesh crossed with one big
polygon. The collider losses are points inside house/fence footprints the navmesh had cut out.

The gained 33.75 ha are the **background terrain outside the designed navigation**: the original
game fenced those fields/elevated grounds with invisible barriers (no navmesh), and the port's
rule now accepts them (terrain exists, slope <= 45, no collider). The per-area map in the raw
output shows them as `T` bands at the map borders and around the playable bowl — this is the
fidelity loss the user was warned about before the change, quantified. Inside the designed
areas the map is almost all `#` (both rules agree).

## Reproduction

```sh
cargo run -p pla --example terrain_walk_report -- --area 0,1 --step 4
# other areas: --area all; finer grid: --step 2 (4x the samples)
```

The example prints the per-area summary, the cause breakdown and an 8 m/char map
(`#` both, `N` only navmesh, `T` only rule, `.` neither).
