# Throwing module, world capture and baits (`feat/throwing-module`)

Brief 18 of `docs/briefs-agentes.md` (worktree `throwing-module`, branch
`feat/throwing-module`). Ball kinds, bait/berry throwing, the overworld capture
animation and bag counters. Integrated against the bag model merged by Brief 14
(`PlayerProgress.bag`, `pocket_code`): the module consumes and counts the same
map the bag screen shows; no second counter exists.

## What was built

1. **`crates/pla/src/game/throwing.rs` (new).** Ball identities are data-driven:
   - `BallKind` (9 kinds, `ALL_BALL_KINDS`) carries the decoded Hisui item id
     (1710-1713, 1746-1750) and the `capture_ball_multipliers.tsv` category
     (`Regular`, `Great`, `Ultra`, `Feather`, `Wing`, `Jet`, `Heavy`, `Leaden`,
     `Gigaton`). An integration test pins every id and category to
     `data/item_effects.tsv`.
   - `ThrowableTable` parses `data/capture/throwable_parameters.tsv`
     (`f00` item id, `f06_f32` speed, `f07_string` class). Decoded ball speeds
     are 40 for the winged line and 30 for the rest; bait rows are 15 or empty.
   - `ThrowParams::for_ball` / `for_throwable`, `launch_velocity`,
     `pick_aim_point`: the viewer's ballistic math extracted to pure functions
     (a test flies the arc and checks it lands near the target).
   - Bag counters over `PlayerProgress.bag` + `ItemTable.ball_category`:
     `bag_ball_ids`, `ball_count`, `consume_item`, `consume_ball` (lowest bag id
     first, so legacy ids 2-4 and Hisui ids 1710-1750 both count and are
     consumed in id order).
   - `PokeBall`, `BallInventory` (selected ball, thrown count, status line,
     bait cycling), `BallVisuals` / `spawn_ball` (three primitives; colours per
     ball line are ASSUMED).
2. **`crates/pla/src/game/capture_world.rs` (new).** Overworld capture roll and
   baits:
   - `roll_capture` evaluates `capture_index`/`capture_probability` with the
     `BattleData` sheets (ball category + suitable range, status `None`,
     backstrike factor, rank adjustment; player/pokemon rank 1 and full HP are
     ASSUMED: the port has no rank system and the overworld has no damage).
     The two draws are re-implemented in `draw_capture_checks` in the same order
     and bound as `battle_engine::two_shake_capture`; a test proves the outcome
     and the final RNG state match for 6 probabilities x 5 fixed seeds. The seed
     comes from `PlayerProgress.rng_seed` and the advanced state is written back,
     so worlds and battles share one stream.
   - `shake_count` maps checks passed to 1-3 shakes (`SHAKES_ON_CAPTURE = 3`);
     `suitable_range` (winged line beyond 12 m, heavy line on a backstrike) and
     `is_backstrike` (dot > 0.3) are ASSUMED conditions, named in code.
   - `BaitTable` parses `data/capture/bait_parameters.tsv` (29 rows; id, type,
     taste, category, two hex colours, special effect); `bait_multiplier` reads
     `capture_index_baiteffect_available_factor` / `..._not_available_factor`
     from `capture_context_modifiers.tsv` (1.3 / 1.0 decoded).
   - `store_caught` adds the caught member to the party, else deposits into the
     box, else records the species only; `build_caught_member` builds it from
     the battle data (tackle fallback, same as `battle_controller`).
   - `BaitObject` / `BaitProjectile` / `AttractedTo`, `spawn_bait`,
     `step_bait_projectiles`, `bait_tick` (20 s lifetime),
     `bait_attract_wild`: armed baits re-home nearby wild creatures by
     overwriting their `WanderHome` (reusing `wild_wander`) and restore the
     original home when the bait expires or the creature leaves. Registered by
     `CaptureWorldPlugin` (one line in `port_viewer.rs`).
3. **`ball_lifecycle.rs` (extended).** `Hit -> Absorbing -> Dropping ->
   Shaking -> Captured/Breakout`: `hit_pokemon(at, ground_y, max_shakes,
   will_capture)`, events `Absorbed`, `Dropped`, `Shake(n)`, `Captured`,
   `BrokeFree`, and the despawn reasons. Timings are ASSUMED presentation values
   (absorb 0.35 s, shake 0.65 s, drop cap 1.5 s, capture aura 1.8 s). Tests
   assert the exact event sequences for a catch and a breakout and that no
   `BallBody` entity survives 400 ticks across 60 mixed throws.
4. **`crates/pla/examples/port_viewer.rs` (minimal diff).** Imports swapped to
   the two modules; the ball throw consumes from the bag and refuses with a
   message when empty; a hit runs the overworld roll and starts the lifecycle;
   `Captured` stores the creature (party/box/recorded), empties its spawner and
   despawns it; `BrokeFree` shows the creature again; `X` throws the selected
   bait (bag item, decoded colour); the shake wiggle and star child are shown.
   Scripted runs (`--auto-demo`, controls demo, `--throw`) stock the demo bag so
   the real counters are exercised. `bait_throw_input` is registered in its own
   `add_systems` because the world tuple is at Bevy's 20-system limit.

## Verification

- `cargo test -p pla --lib game::throwing::tests` → **9 passed**, 0 failed.
- `cargo test -p pla --lib game::capture_world::tests` → **10 passed**, 0 failed.
- `cargo test -p pla --lib game::ball_lifecycle::tests` → **11 passed**, 0 failed.
- Full `cargo test -p pla` → **714 lib tests passed**, 0 failed; all integration
  test binaries pass (3 ignored: absent fixtures, the repository convention).
  The run was made on the branch with `main@f45e78a` merged in.
- `cargo clippy -p pla --lib` → exit 0; no diagnostic in the three touched files
  (the crate keeps 323 pre-existing warnings elsewhere, out of scope).
- `cargo fmt --check`: `throwing.rs` and `capture_world.rs` are clean;
  `ball_lifecycle.rs` keeps the pre-existing drift of `main` (the gate is
  already red in 114 files of the package, e.g. `port_viewer.rs` 94 hunks,
  `battle_controller.rs` 73; not reformatted here to avoid diff noise).
- Measured numbers: 9 ball kinds (6 speeds 30, 3 speeds 40); fallback
  multipliers equal the decoded sheet in 9 categories x 2 range states = 18
  exact matches; 29 bait rows; draws equal `two_shake_capture` in 30
  seed/probability combinations; capture plays absorb, drop, 3 shakes, click;
  a failed roll plays absorb, drop, shake, breakout.
- No native function implements this report's Rust items beyond the engine
  functions already merged, so the evidence ledger and the fix2 profile were not
  touched; the README completion-plan rows do not change either (its counters
  come from the ledger).

## Verified / Assumed / Unknown

**VERIFIED (test-backed):** ball item ids, categories and decoded speeds against
`data/item_effects.tsv` + `data/capture/throwable_parameters.tsv`; fallback
multipliers against `capture_ball_multipliers.tsv`; RNG parity and state advance
with the battle engine; bag counting/consumption over the merged bag model;
party -> box -> recorded storage; life-cycle event sequences and despawn reasons;
bait attraction re-homes and restores a creature (headless Bevy test); bait
factor 1.3/1.0 from `capture_context_modifiers.tsv`.

**ASSUMED (named in code):** per-line gravity and colours; suitable-range
12 m / backstrike dot 0.3; bait radii 20 m attract / 2 m approach / 3 m feed;
bait lifetime 20 s; animation timings (absorb 0.35 s, shake 0.65 s, drop cap
1.5 s, aura 1.8 s); rank 1 and full HP as overworld roll inputs; favourite-bait
detection maps to the "available" factor; bait fallback speed 15 m/s.

**UNKNOWN:** the game's ball open/close/wobble assets were not found (no ball
entry under `bin/effect/` or `bin/field/`, and no ball in
`data/field_model_export_index.tsv` — only `gimmick_balloon01/02`), so the
animation is procedural with ASSUMED timing; the exact game timings; whether a
successful catch during the absorb phase should differ.

## What the maintainer should eyeball

- Throw a ball: impact absorb, drop, wobble count matching the roll, click, star
  aura, catch message and despawn; a failed roll returns the creature and the
  ball despawns.
- Bait throw (`X` after `V` selects): sphere lands, nearby creatures walk to it,
  they resume their wander when it expires; capture with bait nearby shows the
  bait factor in the roll line.
- The bag screen and the throw log agree: throwing decrements the exact ball
  line (legacy or Hisui id) and refuses when empty.
