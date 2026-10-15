# Pokédex research: rank ladder and the task table (Brief 43, data phase)

**In plain words.** The Pokédex's research system has two halves: the player's rank (a ladder of
research points) and the per-species research tasks (catch 2 Bidoof, defeat 3 with a move, ...).
The game files for both exist and were located in the player's own dump:
`bin/appli/pokedex/res_table/`. The **rank ladder** is decoded (PROBABLE): nine records give the
points each rank needs. The **per-species task list** is not decoded yet (UNKNOWN): the table is
found, its record count is measured, but its record fields and its species grouping are not
established, so the port does not show or award research points.

Code: `.tools/dex_research_probe.py` (read-only prober) → `data/dex_ranks.tsv` (tracked sheet),
`crates/pla/src/game/dex.rs` (rank ladder model + tests). Nothing here reads game text.

## Sources (never redistributed; the repository holds no game data)

| File | Size | SHA-256 (measured) |
|---|---|---|
| `pokedex_rank_table.bin` | 204 B | `3f8fdc8307cefa06a23a8eec29954888a6925f55355834cee33b2596b7c2508e` |
| `pokedex_research_task_table.bin` | 108768 B | `2bfdfaf47dc424c80970c8f61ef00b75496494dfe30ab3e6bc80d252582c8e75` |
| `pokedex_form_storage_index_table.bin` | 1910 B | not decoded (318-entry header; unrelated to research) |

## Verified

- Both research files exist in the v262144 update with the sizes and hashes above; the prober
  refuses to run against any other build.
- The rank table holds **11 records** in its own offset list (u32 count at `0x14`) and the task
  table announces **1611 task records** (u32 count at `0x18`).
- The task records really are individual blocks: **1592** of them end in the same three 64-bit
  hash slots (measured by scanning the record region that starts at `0x1934`), consistent with
  1611 records where a few carry non-default hashes.
- The nine rank records each point by a relative offset at the same tail block (they are rows of
  one table), and their `(rank, points)` pairs are monotone: 2 → 1800, 3 → 3500, 4 → 6000,
  5 → 8500, 6 → 11000, 7 → 15000, 8 → 20000, 9 → 30000, 10 → 60000.

## Probable

- The rank-ladder reading itself: the records repeat a fixed 12-byte shape `(relative pointer,
  rank id, points)` and only the nine monotone pairs are accepted as a ladder by the prober;
  `data/dex_ranks.tsv` carries the file and byte offset of each record.
- The three default 64-bit values per task record are placeholder hashes ("no requirement"): an
  FNV-1a-64-shaped constant repeated three times in 1592 records.
- The first fields of a task record look like `(relative pointer, species id, task kind, ...)`:
  the first record of the region mentions species 905 (the last species), and many records repeat
  one species id, which fits several tasks per species — but the field order past the species id
  is NOT established.

## Unknown (do not invent)

- **The per-species research tasks**: task kind values, target counts, the points each task step
  awards and how the three hash slots describe requirements. No `data/dex_tasks.tsv` is written.
- The meaning of the rank table's own offset list (it points at the file tail, not at the records)
  and of the tail block (a candidate `500` value may be rank 1's threshold — UNKNOWN).
- The game text of every task (out of scope by the project rules: no dialogue/text).
- Whether the port's save tracks enough per-species counters: it stores `seen`, `caught`,
  `defeated` and `caught_count` totals only, so per-species catch/defeat counts would come from
  a future save-format addition, not from data invented here.

## Next step (when picked up)

Decode one task record against a known species/task (e.g. species 905's first record) with the
game's own UI (the research page of that species shows its tasks) to pin the field order, then
extend the prober to emit `data/dex_tasks.tsv` and wire points into `game/dex.rs`.
