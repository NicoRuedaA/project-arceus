# Battle move effects in the port: what plays, how, and what is assumed

Written 2026-10-13 (worktree `efectos-juego`, brief 42 phase "playback"). Builds on the
format work in [`effects.md`](effects.md).

**In plain words.** Battles now show the game's own move effects: when a Pokémon uses a move,
the port reads that move's timeline from the game files and plays its particle bursts (sparks,
flames, glows, rings, dust) on the attacker and on the target at the frames the timeline gives.
Status effects (poison, burn, frostbite, paralysis, drowsiness) play their own effect on the
Pokémon that gets them. The look comes from the decoded files; how particles move, how big one
game unit is, and a few drawing details are guesses, so the result is "believable", not "proven
identical". Nobody has compared it with the real game by eye yet.

## Coverage (measured by `examples/effect_coverage.rs`)

| What | Count |
|---|---|
| Moves usable in this game (default timelines `ewNNN`) | 177 |
| ... with at least one particle file drawn | **176 of 177** (Splash's timeline plays no particle file) |
| ... including the 15 species/form overrides | 191 of 192 |
| Particle files used by those timelines + status timelines | 629 |
| Emitters in them (counted once per file) | 3,903 |
| ... drawn | **3,356** (billboard 1,807, flat polygon XY 520 / XZ 150, streak 864, upright billboard 15) |
| ... skipped | **547**: child emitters 366, mesh particles 94, untextured 66 (e.g. the invisible `cmn_collision_dummy`), unknown display value 7: 11, ribbons 10 |
| Moves with every emitter drawn | 42 of 177 (134 have at least one skipped emitter) |
| Drawn blends | alpha 2,428, additive 841, subtractive 87 |
| Emitters drawing a plain white placeholder texture | 75: 33 draw their second texture instead, 42 a soft round dot |
| Effect textures decoded into the cache | 357 distinct names (2,654 file-texture pairs), 0 failures |

Top-12 list of `effects.md`: all 12 play (Tackle, Bite, Double-Edge, Crunch, Quick Attack,
Confusion, Ancient Power, Slash, Astonish, Iron Defense, Psychic, Hydro Pump). Slash loses 9 of
its 18 emitter plays (its slash arcs are not billboards), so it will look thinner than the game.

Shared marks: `cmn_critical`, `cmn_affinity_advantage` and `cmn_affinity_disadvantage` exist and
are started on a critical / super-effective / not-very-effective hit, but **their timelines play
no particle file** (their commands are of the undecoded kinds: camera, UI, colour...), so nothing
visible comes from them yet. There is no other generic hit effect in the move map, so none is
added. Status: `es001` drowsy, `es002` poison, `es003` burn, `es004` frostbite, `es005`
paralysis (names LIKELY) each play one particle file on the Pokémon that received the status.

### Moves with no effect file (nothing plays, same as the game)

The viewer log reported four missing timelines: `ew045`, `ew036`, `ew017`, `ew028`. They are
**Growl (45), Take Down (36), Wing Attack (17) and Sand Attack (28)**, moves whose waza record
has `style_flag = 0` in `data/pml_move_effects.tsv`: they survive in the learnsets as move-tutor
leftovers, but they are **not usable in this game** (the 177 `ewNNN.wbin` files match the 177
`style_flag = 1` moves one to one, and `bin/effect/battle/sequence` has no file for them,
VERIFIED). The original has no timeline to load for them and therefore plays nothing; the port
skipped them the same way. What was off was where the names came from: the viewer's demo
movesets (`game::battle_controller::moveset` / `wild_moveset`) picked those tutor moves.
They now only pick usable moves from each species' real learnset (rollout, crunch, gust,
aerial-ace, thunder-shock, ...). The effect driver splits any absent timeline name into a
`warn!` for names the game *does* use (a real gap in the extracted data; previously the message
for every name) and a `debug!` for not-usable moves (normal: no effect, like the original).

## How it plays (code: `crates/pla/src/game/effects.rs` and `game/effects/`)

1. **Battle hook.** `EffectsPlugin` reads the new events of `ActiveBattleSession` (the same
   cursor pattern as the animations). `effects_for_events` turns `UsedMove` into the move's
   timeline (`sequence_name(move, species, 0)`), `Missed` into the same timeline without its
   target-side particles (ASSUMED), `StatusApplied` into the status timeline. Other code can
   play any timeline or particle file with a `PlayEffect` message (slot or world-point anchors).
2. **Timeline.** Particle plays (command `0x02`, VERIFIED paths) start at their frame (LIKELY) on
   the user or the target slot (`0x09` side byte, LIKELY), at 0.5 m above the creature's feet
   (ASSUMED: no bone locators yet), and emit until the play's end frame. All other command kinds
   (camera shake/animation, flashes, colour, model motion, scale `0x0d`) are skipped: none is
   decoded well enough.
3. **Timing (sync with animation and sound).** With effects on, a move occupies its timeline
   length (clamped to 0.9-4 s) instead of the fixed 1.4 s, and the target flinches when the first
   particle lands on it (`first_target_frame / 30`) instead of after 0.45 s. The animations
   (`battle_anim::requests_for_events_timed`) and the hit sounds
   (`audio::events::cues_for_battle_events_timed`) read the same `MoveTimingTable`, so hit
   flinch, hit sound and the target particles start at the same moment (unit-tested). With
   `--effects off` the table is not loaded and the old fixed pacing stays.
4. **Particles.** `sim::EffectSim` per particle file: emissions from the decoded schedule,
   particles born at their scheduled frame (results do not depend on the frame rate; tested),
   colour / alpha / size over life from the key tables, flip-book cell over life, then
   retired at the end of their life. Instances despawn themselves when nothing is left; a battle
   that ends clears its effects.
5. **Drawing.** One mesh of quads per emitter, rebuilt on the CPU each frame to face the camera
   (billboards; upright billboards; streaks stretched along their on-screen velocity; flat XY /
   XZ polygons in the attacker-to-target frame). Custom material (`effects/effects.wgsl`):
   texture x vertex colour, premultiplied output, and three blend states (alpha, additive,
   subtractive = destination minus particle). No depth writes; drawn over the scene (see below).
   Textures are shared by name; materials by (texture, blend, wrap).
6. **Budget.** `--effects low` (default): half of each emission (at least 1), at most 300 live
   particles per particle file and 1,000 in total. `--effects high`: full emission, 600 / 2,000.
   `--effects off`: nothing. Also `PLA_EFFECTS=...` or `effects=...` in `settings.txt` (note:
   saving settings from the pause menu rewrites the file without that key). Measured at high quality
   on sampled frames: Ember about 100 live particles, Thunder Shock about 20, Tackle about 55;
   40 simultaneous Thunderbolts stay under the low-quality 1,000 cap (test).
7. **Textures.** Decoded once from each file's `BNTX` with the NX component mapping applied
   (BC4 masks `[R,R,R,R]` become grey-white sprites whose alpha is the mask; BC5 `[R,R,R,G]` use
   G as alpha) and cached as PNG under `$PLA_CACHE_ROOT/effects/textures/` with
   `effects/texture_index.tsv` (git-ignored). Loading runs on one background thread; a play
   whose file is still loading waits up to 1.5 s. All move timelines of both combatants are
   preloaded when a battle starts.

## Verified / Likely / Assumed / Unknown

* **Verified** (tests, counts): the timing table parses (192 rows) and the per-move pacing is
  shared by animation, sound and effects; spawn counts follow rate and are independent of the
  step size; one-time bursts die after their life; per-effect and total caps hold; Ember's real
  timeline spawns its 3 particle files (muzzle, bullet on the user, hit on the target) and
  everything despawns on its own (headless Bevy test); 0 texture decode failures over 357
  textures; the PNG cache round-trips exactly; the WGSL material compiles and draws (windowless
  GPU render, `examples/effects_headless.rs`, RTX 4060 / Vulkan).
* **Likely:** what `effects.md` lists (curves, life, rate, display, blend 0/1/2, side, frames);
  sampler wrap bytes `+8/+9` = 0 mirror / 1 repeat / 2 clamp (atlas strips and flip-books read
  2 in 492 emitters, scrolling `flow` textures 1, the quarter-circle `cpt_1_circle*` masks 0 in
  1,217; no other byte separates the groups); NX component mapping of the textures.
* **Assumed (port choices, never compared with the game):**
  * 1 effect unit = 1 m (`WORLD_SCALE`): with it the decoded sizes give 0.5-1 m flashes;
  * `omni_speed` is in units per **second** (per frame, a hit would spray over ~8 m);
    launch direction uniformly random; gravity per frame squared (it is almost always 0);
  * a single-cell texture with mirror wrap spans UV 0..2, so the quarter masks form a full
    circle (no repetition-count field was found);
  * emitters whose blend byte is 0 but whose name contains `add` are drawn additively
    (107 emitters; half of the `_add`-named emitters read 0, see `effects.md`);
  * a plain white sampler-0 texture (`cpt_cmn_white*`) is replaced by the sampler-1 texture
    (33 emitters; exact only if the two textures are multiplied) or by a soft round dot
    (42 emitters, named `light`/`shine`; drawn as solid squares otherwise, seen in the burn
    effect);
  * blend values 3-5 drawn additively; effects centred 0.5 m above the feet; effect local +Z
    points from attacker to target; missed moves skip the target particles; 30 frames per
    second; the turn-length clamp 0.9-4 s;
  * drawn **over** the scene (no depth test), because effects centred inside the Pokémon were
    otherwise hidden by the model (checked in the GPU render).
* **Unknown / not done:** child emitters (366), mesh particles (94), ribbons (10), display
  value 7 (11): skipped and counted. Emitter shapes/volumes, emitter offsets and animation,
  the sixth key table, colour table 1 and sampler 1/2 (multi-texture combiners), UV scrolling,
  particle rotation, soft (depth-faded) particles, the real air resistance (a field at `0x72C`
  holds 0.8-1.0 values typical of air resistance, but all sampled hit emitters read 1.0, so it
  is not applied). Camera shake and screen flash commands are not decoded, so none plays.
  Projectiles do not travel: an Ember "bullet" plays at the attacker (motion commands
  undecoded). Tackle's hit also plays on the attacker (the game moves the attacker to the
  target; the port does not). Per-move sounds (`PLAY_EW*`) are not wired.

## What to look at in a battle

Start a wild battle (`./play.sh`, or the viewer with `--effects high` for full particles) and
use Thunder Shock / Thunderbolt (Pikachu), then watch the wild Pokémon's moves:
* a yellow charge glow on the attacker, then an electric star flash with sparks on the target
  when the target flinches and the hit sound plays (all three together);
* Ember: a flame puff at the attacker, flames bursting on the target;
* Tackle: a short bright glow on both Pokémon at the impact;
* statuses (CPU render looked at): dark purple bubbles rising (poison), a flame burst with
  an orange glow (burn), white snowflake bits (frostbite), yellow electric lines
  (paralysis), dark grey bubbles (drowsy; their colour may be wrong: colour table 1 and the
  second sampler are not used);
* things that may look wrong and why: effects too big/small (unit assumed), particles drifting
  too slowly or fast (speed assumed), dark rings (subtractive emitters), missing slash arcs or
  trails (mesh/ribbon skipped), effects drawn in front of everything (on purpose).

## Reproduce

```bash
cargo run -p pla --example effect_coverage                    # coverage + texture cache
cargo run -p pla --example effect_preview -- --moves 84,52,33  # CPU render, plugin math
cargo run -p pla --example effects_headless -- --moves 84,52,33 --frames 22,33,42  # GPU, no window
cargo test -p pla effects ptcl_textures battle_anim audio::events
```
The CPU render (default `work/effects/battle_preview.png`) and the GPU frames were looked at: Thunder
Shock shows a charge glow then a lightning star on the target, Ember a flame puff then a flame
burst on the target, Tackle a brief glow flash on both. Not compared with the real game.
