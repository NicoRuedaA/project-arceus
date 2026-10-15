# Sub-area shell coverage: the real interiors behind the SUPUESTO fallback (Brief 37 follow-up)

**In plain words.** Six interiors used to be filled with a grey provisional floor because the
port believed the game shipped no room for them. That belief is wrong: five of the six do have
their real room in the game's files, already exported to the model cache, and the port now
shows them instead of the placeholder. Only one (`ha_area05_s02`) really has no placed
geometry: the game fills it with its own terrain and plants, so its labelled fallback stays.

Status: **VERIFIED against the sheets and the exported glTF cache** (numbers below); the runtime
change is in `crates/pla/src/game/interior.rs` (plan) and `crates/pla/src/game/zone.rs`
(unchanged flow; it stops drawing the placeholder when the plan reports a real shell).

## What was wrong

`interior.rs` decided "real room" with one rule: at least one streamable placement model whose
name starts with `room_`. That is right for the Jubilife houses but wrong for every other kind
of interior — caves use `ground_*cave*` shells, the temple uses `ground_area05_ktemple_*`
floors/statues/pillars, and the area-04 building and staircase use `objects_*` models. So the
plan flagged five scenes that DO have real geometry, and the runtime painted a SUPUESTO slab
over real floors plus a `[SUPUESTO] ... not game data` notice.

The stale part: `reports/data-progress/subzones.md` (written 2026-10-13) recorded 14 shell
models as `blocked` in the exporter. The export index and the cache were refreshed later
(2026-10-13 14:47-14:49) and **all 11 shells named in that list are now `ok` with a glTF on
disk** (`room_area00_s01`, `room_area00_s03`, `room_area02_s01_a`, `room_area05_s04_a`,
`room_area01_s01`, `room_area03_s03`, `ground_area04_cave03`, `ground_area04_cave07`,
`ground_area05_ktemple_pillar02`, `ground_area05_ktemple_floor001`, `002`). The placements
sheet's own `model_status` column is a snapshot and still says `blocked` for some of them; the
runtime already treats the export index as authoritative (`parse_placements_for_spawn`), so the
models were streaming all along — only the `room_*` name test kept the placeholder on top.

## The measurement (`data/subzone_shells.tsv`)

`.tools/subzone_shells.py` repeats the method of `subareas.md` (place each model's glTF
`POSITION` accessor bounds with its placement row: `x, y, z`, `rot_y` degrees, `scale_*`) over
every sub-area and compares the union box with the sub-area navmesh box of
`data/subzone_index.tsv`:

- `coverage_x` / `coverage_z` = fraction of the navmesh box inside the placed box per axis.
- `shell = covered` when both are >= 0.95 and the placed box reaches the navmesh y band within
  0.5 m; `partial` when some geometry exists; `none` when nothing is placed.

Result over the 55 sub-areas: **50 covered, 4 partial, 1 none**. The five formerly-fallback
interiors:

| Sub-area | Shell model(s) | coverage x / z | Kind |
|---|---|---|---|
| `ha_area04_s01` | `objects_area04_stairs01` | 0.974 / 0.978 | staircase hall (1 model) |
| `ha_area04_s02` | `ground_area04_cave03` (+ water, bonfires) | 1.0 / 1.0 | cave |
| `ha_area04_s03` | `ground_area04_cave07` (+ plate) | 1.0 / 1.0 | cave |
| `ha_area04_s04` | `objects_area04_building01_b` | 1.0 / 1.0 | building interior (own terrain) |
| `ha_area05_s01` | 15 `ground_/objects_/gimmick_area05_ktemple_*` models | 1.0 / 1.0 | temple |
| `ha_area05_s02` | *(none placed: `bg_parts_count = 0`)* | — | `none`: own terrain only |

Confidence: **PROBABLE** (box test; `rot_y` is sheet data marked LIKELY). The four remaining
`partial` rows are known and harmless: `ha_area00_s02` and `ha_area00_s07` still have a small
`room_*` model inside a bigger navmesh (the `room_*` rule keeps them covered), `ha_area03_s01`
is a tent with the same situation, and `ha_area05_s02` is the `none` above.

## `ha_area05_s02`: what the game does place there

The sub-area record lists `bg_parts_count = 0` and the empty `models` column, and it is the
only sub-area with no placement row at all. Its scene is its **own terrain pack**
(`bin/archive/field/terrain/ha_area05_s02.gfpak`): dumping it (`.tools/gfdump`) shows a real
heightmap group whose internal full paths were verified by hashing them (FNV-1a 64 matches the
pack's entry hashes):

- `bin/field/terrain/ha_area05_s02/terrain/terrain_1_1_heightmap.bntx` (entry 81)
- `bin/field/terrain/ha_area05_s02/terrain/terrain_1_1_normalmap.bntx` (entry 82)
- `bin/field/terrain/ha_area05_s02/terrain/terrain_1_1_splat_alpha.bntx` (entry 97)
- `bin/field/terrain/ha_area05_s02/navmesh/navmesh.trnm` (entry 0)

Tile naming is `terrain_1_1` here and in `ha_area04_s04`; `ha_area00_s02` uses
`terrain_area01_demo` — the interior packs reuse the main-area convention
`bin/field/terrain/<zone>/terrain/<tile>_{heightmap,normalmap,splat_alpha}.bntx` (the record's
`f20` names the terrain table at `bin/field/terrain/<zone>/<zone>_terrain_table.bin`, which is
not an entry of the pack). So the "own terrain naming UNKNOWN" of the interiors report is now
**decoded**; loading it into the engine is a runtime task left open (the port keeps the
non-samplable placeholder there and the SUPUESTO slab only for `ha_area05_s02`, whose navmesh
is a flat 1025 m field).

## Runtime change

- `InteriorPlan.has_room` = shell sheet says `covered` **or** a `room_*` model streams.
- The SUPUESTO fallback and its notice are now drawn only when no real shell exists
  (`ha_area05_s02`). The five interiors show only their placed game models.
- `data/subzone_shells.tsv` is re-generated with `python3 .tools/subzone_shells.py`; the
  library tests read it directly (`real_shells_sheet_covers_the_six_fallback_interiors`).

## Evidence / reproduction

```sh
python3 .tools/subzone_shells.py                 # 55 rows: 50 covered / 4 partial / 1 none
cargo test -p pla --lib --release interior -- --test-threads=1
cargo test -p pla --lib --release minimap -- --test-threads=1
```

Viewer captures of the five interiors (headless GPU, isolated automation dir) are listed in
`reports/data-progress/interiors.md`.

## UNKNOWN / limits

- The box test is coarser than the navmesh; a wrong `rot_y` sign in a sheet row would move a
  model without changing its box (the same limit `subareas.md` declares).
- `ha_area05_s02`'s terrain is decoded but not loaded; what the player sees there is the
  labelled fallback.
- Whether any of the five interiors also place story-gated extra geometry is out of scope
  (gate flags are ignored as before).
