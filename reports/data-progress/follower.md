# Party follower (`feat/follower`)

Brief 16 of `docs/briefs-agentes.md` (worktree `follower`, branch `feat/follower`, base
`main@472c080`). The first Pokémon of the party walks behind the player in the overworld.

## What was built

1. **`crates/pla/src/game/follower.rs` (new).** Pure logic (no Bevy in the decisions):
   - `FollowerTrail`: queue of recent player positions; steps shorter than 2 cm are ignored
     (the player standing still), the queue is capped at 512 points. `point_behind(distance)`
     walks back along the recorded path (linear interpolation inside a segment) and returns the
     point `distance` metres back; a path shorter than `distance` returns its oldest point.
   - `FollowerState` / `step`: the target is the point `FOLLOW_DISTANCE = 2.5 m` back along the
     recorded path; target speed = `distance to target * 10`, capped at `12 m/s`, approached with
     `16 m/s²` acceleration; facing turns at `6 rad/s` and the follower only advances once
     facing its target within `MOVE_ANGLE = 1.0 rad` (same rule as `wild_wander`); movement uses
     the navmesh `slide_move`; farther than `TELEPORT_DISTANCE = 15 m` from the player it
     teleports onto the target (a zone change repositions the player far away, so the same rule
     covers it); if it starts off the mesh it heads straight for the target to recover.
2. **`FollowerPlugin` systems** (all `GameState::World`; the drive system also waits for
   `not_paused`):
   - `sync_follower`: spawns / respawns / despawns the follower entity from
     `PlayerProgress.party[0]`. Resolves the exported batch model
     (`cache/pokemon_batch/pmSSSS_FF_00/pmSSSS_FF_00_idle.gltf`) from `PLA_CACHE_DIR` /
     `work/cache` / the absolute repo cache, preferring the member's form and falling back to
     form 0 (same roots as the viewer's `resolve_pokemon_gltf_rel`). Spawns 2.5 m behind the
     player's facing, grounded at navmesh height + 0.45 m (the wild-creature offset). A model
     that was not exported logs once and spawns nothing.
   - `drive_follower`: feeds the player position and navmesh walkability into `step` and writes
     the translation (x, z, navmesh height) and yaw back to the entity's `Transform`.
   - The follower hides on `GameState::Battle` enter and shows again on exit (the player avatar
     is hidden during battles too).
   - Animation is reused, not re-implemented: the entity carries `PokemonAnimSource`, so
     `game::pokemon_anim` spawns the scene, plays the idle/walk clips, matches
     `walk_playback_speed` to the real world speed and pins the root motion.
3. **Registration (minimal).** `pub mod follower;` in `game/mod.rs`; one line
   `app.add_plugins(pla::game::follower::FollowerPlugin);` in `port_viewer.rs` (next to the
   existing plugins). `assets/pokemon_model.rs` untouched (briefs 17/19 share it).

## Verification

- `cargo test -p pla --lib follower -- --nocapture --test-threads=1` → **8 passed, 0 failed**
  (`game::follower::tests::*`).
- Full `cargo test -p pla` → **597 passed, 0 failed** (exit 0).
- Clippy: **no diagnostics for `follower.rs`** (`cargo clippy -p pla --all-targets` exits 0).
  Note: with clippy 1.98 a workspace `-D warnings` run fails on 336 pre-existing diagnostics in
  other files (e.g. `chunks_exact` in parsers, `manual_range_contains` in `wild_wander` tests);
  none of them is in the files this branch touches. Not fixed here (out of scope).
- Measured figures (headless Bevy test, synthetic player on a 6 m circle at 4.5 m/s, 2400 ticks
  = 40 s at 60 Hz, synthetic flat navmesh, diagnostics printed by the test):
  **mean follower-player distance 2.844 m, max 2.870 m**, follower walked 183.1 m, 0 teleports.
  Straight walk at 4.5 m/s, steady state over 899 steps: mean 2.875 m, max 2.875 m.
  Standing player: 2.500 m.
- Walkability: unit test with a real `NavMesh::from_parts` corridor (240 m x 6 m) and the player
  zig-zagging inside it; the follower is asserted walkable after every one of the 1200 steps.
- Teleport: unit test (40 m away → exactly 1 teleport onto the target, no repeats while close).

## Verified

- Only headless: pure unit tests plus one headless Bevy test moving a synthetic player for 2400
  ticks. The numbers above come from those runs; nothing was seen on screen (the viewer is never
  run, per the operating rules).

## Assumed

- Every tunable: 2.5 m follow distance, 0.35 m arrival slack, gain 10, 12 m/s cap, 16 m/s²
  acceleration, 6 rad/s turn, 15 m teleport, 0.45 m ground offset. Chosen from the brief and the
  player's speeds (4.5 m/s walk, 8 m/s sprint); not decoded from the game.
- The models' front axis is +Z (`wild_wander::yaw_for` convention, unverified there too);
  "behind the player" and the idle facing inherit that assumption.
- The brief's "queue of positions, delay" is implemented as distance-along-path (2.5 m back
  along the recorded path), not as a fixed time delay.
- Trail cap of 512 points is enough at any frame rate this game runs at.
- Size bias (brief 17) does not exist yet: the follower spawns at scale 1.

## Unknown

- Not checked visually by anyone: sliding, slopes/steps, the 0.45 m model offset and clipping on
  real terrain are unverified; the maintainer must eyeball it.
- A party lead whose pack was not exported (354/356 export) gets no follower at all (no
  placeholder model); one warning per species is logged.
- Zone change is covered only by the 15 m teleport rule; it was not exercised headless (the
  `ZonePlugin` is not part of the follower tests).
- Behaviour when the follower is separated from the player by a wall taller than the slide logic
  (it walks along the wall and recovers only if it comes within 15 m) is not measured.

## Files

- `crates/pla/src/game/follower.rs` (new)
- `crates/pla/src/game/mod.rs` (one `pub mod` line)
- `crates/pla/examples/port_viewer.rs` (one `add_plugins` line)
- `README.md`, `README.es.md` (one sentence each in the Visual subsystem row; figures identical)

## Play-test fix (2026-10-15, branch `feat/mundo-vivo`)

The maintainer reported "the follower walks through the player". Measured with
`.tools/repro_follower.py` on the real viewer (`--automation --automation-no-gpu`, area 01:
forward, 180-degree turn straight through the follower, stop, sprint; samples every frame):

| Phase | Before | After |
|---|---|---|
| Walk through the follower | min 1.32 m, ends pressed in front | min 1.391 m (2-decimal state rounding), steps 0.96 m aside and ends 2.88 m behind |
| Player stops | stuck at exactly 1.40 m **in front** with speed 12.0 | settles at 2.50 m **behind**, speed 0 (walked around) |
| Sprint | min 1.26 m | min 2.76 m |

Raw captures: `~/work/capturas_mundo_vivo/follower-before.json`, `follower-after.json`,
screenshots `follower_side.png` / `follower_behind2.png` (GPU, offscreen, 2026-10-15).

What changed in `follower.rs`:

1. `slide_around_player`: a move that would enter the player's disc slides along the separation
   circle by the same arc length, towards the side that keeps closing on the aim direction,
   instead of crossing the player and being pushed radially. The arc rate is capped at the body
   turn rate (`TURN_RATE`), so the yaw-towards-target can follow and the "face the target"
   movement gate never throttles the step-aside into stop-and-go.
2. Stop repositioning: after `STOP_REPOSITION_DELAY = 0.5 s` standing still, an aim point left
   on the front side (the player walked backwards) is replaced by the spot behind the player's
   facing; the latch holds until the player moves again, so the arc cannot flip half-way.
3. `drive_follower` runs after the frame's locomotion (`PlayerMotionSet`), so the hard floor is
   applied after the player's final position of the frame.

SUPUESTO: the 0.5 s stop delay (not decoded); the strategy itself is port policy. The hard
floor (1.4 m) and the teleport (15 m) are unchanged. Tests: `game::follower` 14/14, including
`walks_around_the_player_instead_of_being_pushed_through` (no per-frame jump above
`MAX_SPEED * dt`, lateral step > 0.5 m, ends behind) and
`a_follower_left_in_front_walks_back_behind_the_stopped_player` (180-degree arc, min separation
kept, max step 0.2 m/frame).
