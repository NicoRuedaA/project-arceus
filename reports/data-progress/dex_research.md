# Pokédex research: rank ladder, research tasks and their wiring (Brief 43)

**In plain words.** The Pokédex research system has three layers, and all three are now decoded
from the player's own dump and wired into the port: the **rank ladder** (the research points each
player rank needs), the **tasks** (1611 per-species tasks like "catch N", "use move X N times"),
and the **counters** that advance them (catches, moves used in battle). The port stores the
counters in the save (format v9) and shows the player rank, each species' research level and the
selected species' task list on the Pokédex screen.

Code: `.tools/dex_research_probe.py` (rank ladder → `data/dex_ranks.tsv`),
`.tools/decode_dex_tasks.py` (task catalog → `data/dex_tasks.tsv`),
`crates/pla/src/game/dex.rs` (model: tasks, levels, points, rank),
`crates/pla/src/game/dex_screen.rs` (screen),
`crates/pla/src/game/save_wire.rs` + `save_game.rs` (counters, save v9).
Nothing here reads game text.

## Sources (never redistributed; the repository holds no game data)

| File | Size | SHA-256 (measured) |
|---|---|---|
| `pokedex_rank_table.bin` | 204 B | `3f8fdc8307cefa06a23a8eec29954888a6925f55355834cee33b2596b7c2508e` |
| `pokedex_research_task_table.bin` | 108768 B | `2bfdfaf47dc424c80970c8f61ef00b75496494dfe30ab3e6bc80d252582c8e75` |
| `pokedex_form_storage_index_table.bin` | 1910 B | not decoded (318-entry header; unrelated to research) |

## Verified / PROBABLE

**Rank ladder (PROBABLE, `data/dex_ranks.tsv`).** The file is a FlatBuffer: its root table's vector
holds **eleven records, index = rank id**. Records 1..10 share one vtable (two fields: rank id,
points) and form the ladder 1 → 500, 2 → 1800, 3 → 3500, 4 → 6000, 5 → 8500, 6 → 11000, 7 → 15000,
8 → 20000, 9 → 30000, 10 → 60000; record 0 is the empty table (rank 0, no threshold). Version 1 of
the probe missed **rank 1 = 500** because it looked for a `(2..=10, points)` shape; `.tools/dex_research_probe.py`
now walks each record's vtable and reads all eleven. This closes the "candidate 500, layout UNKNOWN"
gap and matches the external hint's rank-1 row.

**Task catalog (PROBABLE, `data/dex_tasks.tsv`).** The task file is a schema-free FlatBuffer whose
root holds one vector with **1611 task tables over 242 species** (national dex ids in ascending
runs, exactly the active species of `pml_personal.tsv`; 1..9 tasks per species). The decoded
fields per task — with the field numbering the game's loader (`FUN_012f7198`) materializes, which
is the file's vtable slot + 2:

| Field | Meaning | Confidence |
|---|---|---|
| species | national dex id | VERIFIED by structure (ascending runs, 242 distinct) |
| kind | task kind 0..=27; `FUN_01030984` switches on it and reads a different per-species counter per kind | PROBABLE |
| kind 1 target | a move the species can learn (Pikachu: 84 thunder-shock, 85 thunderbolt; Vulpix: 52 ember) | PROBABLE |
| type/param bytes | raw second/third parameters (127 records carry one ≤ 17; 26 carry one in 2..4) | RAW, meaning UNKNOWN |
| thresholds | up to **five strictly ascending count thresholds**; the count of leading nonzero entries is the task's step count (the loader counts exactly that run) | PROBABLE |
| `field14` | **points per step, base**: 10 in every record except Arceus' (90); `FUN_0102f528` accumulates `(field14 + field15) * levels_gained` into the species' points | PROBABLE (code) |
| `field15` | **points per step, bonus**: 10 for the 709 driven tasks (red double-arrow) and 0 for the other 902; Arceus' record is 10, so its single step is 100 = 90 + 10 | PROBABLE (code) |
| 3× u64 | almost always the FNV-1a offset basis ("none"); ~19 records carry unique values (special tasks, e.g. kind 27) | RAW, meaning UNKNOWN |

Task shape measured: 1304 tasks with 5 steps, 133 with 3, 174 with 1; kinds 0..27 with 23 values
present (0: 240 tasks — one per species almost everywhere —, 1: 393, 2: 127, 3: 164, 4: 112, ...).

**The per-species state (code, `FUN_01030984` / `FUN_0102ecd0` / `FUN_010308d0`).** A species
slot (stride 0x58) holds a u16 research-point field, a 3-bit level per task slot (10 slots) and
per-kind u16 counters; the game computes a task's level by cascading its thresholds, and the rank
sum adds each registered species capped at 99 points (200 when the game's per-species flag is set,
which the port reads as "perfect"). **Points per step (code):** `FUN_0102f528` adds
`(field14 + field15) * levels_gained` per task to the species' points — a standard step is 10, a
driven step 20, and Arceus' only step is 100 (`field14 = 90` + `field15 = 10`). **Rank bonus at 100
points (code):** the same routine counts every species crossing from below 100 to 100+ points
(`(old < 100) && (new > 99)`) and adds **100 points per crossed species** to the stored rank total
(`iStack_4d0 * 100`); the getter clamps the species' displayed points to 100 with the flag and 99
without it, and `FUN_0102ecd0`/`FUN_010308d0` read that clamp. The stored rank total itself caps at
99999. The port models the crossing as the state-free `points >= 100` test (same one-time quantity
for a player whose points only grow).

**The per-species flag (partially UNKNOWN).** `FUN_010357f8` materializes the species' tasks and
walks them with a per-record marker byte: a marked task with stored level 1 (no threshold passed)
clears the result; marked tasks with level > 1 and unmarked tasks carry it, and a species with no
task records returns 1. The marker byte's exact offset in the materialized record could not be
pinned from the decompiled stack layout, so the flag's precise condition ("all tasks started" vs a
subset) stays UNKNOWN; the port keeps its existing "all tasks complete" reading for the 99/200
split.

## What the port does (implementation)

- **Model (`dex.rs`)**: `DexTask::level` (1 + thresholds passed, ≤ 6), `step_points`
  (`field14 + field15`: 10 standard, 20 driven, 100 Arceus) and `points` ((level−1) × step_points),
  `species_points`, `species_level` (1 + points/10, ≤ 10, PROBABLE from the 10-points-per-level
  external hint), `species_perfect`, `species_rank_contribution` (200 perfect / min(points, 99)),
  `species_rank_bonus` (+100 from 100 points, the code's crossing) and `rank_points` (both, over
  the registered species); `rank_for_points` keeps the decoded ladder and floors at rank 0.
- **Events wired (only the kinds with decoded meaning)**: a **catch** advances the species'
  kind-0 counter (`apply_outcome`), and every move the party uses against the wild (hits and
  misses, in order) advances that move's kind-1 counter. Kinds 2..27 exist in the model and the
  save but stay at 0: their semantics is UNKNOWN and no event feeds them (declared, not invented).
- **Save (format v9)**: `research=species:kind:target:count,...` on the save body; v8 and older
  load with an empty map (documented no-op migration).
- **Screen (`dex_screen.rs`)**: the status line shows the player rank and points; each registered
  row shows its research level and points; the selected species' task panel lists up to 3 tasks
  with counter/last-threshold, level and points.

## Unknown (do not invent)

- The label of every task kind except kinds 0 (catch count: default kind, the registration
  counter the game's readers test) and 1 (move uses: every record names a move of the species).
- The meaning of the raw type/param bytes, of slot 16 (absent from every record) and of the u64
  hash slots.
- The exact species-level boundaries (points/10 is PROBABLE from the external hint) and the
  per-species flag's definition (the marker byte of `FUN_010357f8` is not pinned; read as all
  tasks complete).
- The game text of every task (out of scope by the project rules).

## Reproduce

```sh
python3 .tools/dex_research_probe.py && python3 .tools/decode_dex_tasks.py
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=/home/nico/work/cargo-target-fixes cargo test -p pla --lib -- dex
```
