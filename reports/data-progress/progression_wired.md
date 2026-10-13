# Progression and styles wired into the game battle

Scope: the Bevy game battle (`crates/pla/src/game/`) now uses the engine work of briefs 26
(progression) and 29 (styles and action order). The rules are the engine's
(`battle_engine::progression`, `battle_engine::styles`); this report covers only how the game
calls them, what it shows and what is assumed. Nothing here was checked by eye: every claim
below comes from unit tests, headless Bevy tests or the console `battle_demo`.

Labels: **VERIFIED** = read from the game's tables or code (see `progression.md`,
`battle_styles.md`) and exercised by a test here. **LIKELY** = the engine's own label.
**ASSUMED** = a port choice with no game evidence.

## What the player sees

**Turns (default: PLA action order).** At battle start the action queue is enabled
(`Battle::enable_action_queue`). The combatant with the lowest wait acts alone; when it is the
opponent, it acts at once (`ActiveBattleSession::poll_ai`, called every frame and before each
input). A fast Pokémon or an agile move can give two turns in a row. A panel at the top right
lists the next 6 turns (`turn_order_preview(6)`; `>` marks the player's turns, with the wait in
ticks).

**Styles.** Keys: arrows / A / D highlight a move; **Q** uses it in Agile Style, **E** in Strong
Style, **Enter** or the move's number in the normal style. The move grid shows the power of each
style (`Pow N A S`) and the strong accuracy when it differs. The log says
"X used an Agile Style move!" / "X used a Strong Style move!" before the move line, and how the
action points moved the next turns ("next turn was pushed back (+5)"). Gamepad: not mapped
(no free button). Q / E also turn the camera a little while held (they were the camera keys).

**After a win** (battle screen stays up until everything is done):
1. "X gained N Exp. Points!" after "Battle over: you won!";
2. one "X grew to level L!" line per level; stats recomputed (`level_up_combatant`, current HP
   grows with max HP);
3. moves of the crossed levels: learned at once into a free slot ("X learned M!"); when 4 are
   known, a prompt: **1-4** forget that move, **Backspace** give up;
4. a level-up evolution (`check_evolution`): "What? X is evolving!", 2.5 s placeholder animation
   (the arena Pokémon pulses in size, a white flash builds up; existing components only),
   **Backspace** stops it; otherwise the species changes ("Congratulations! Your X evolved into
   Y!"), the arena model of slot 0 is respawned with the new species, and the save gets it.

**Bag.** Rare candy (#50, VERIFIED level-up column) and effort items (`ev_amount_*` columns) are
listed as "Level +1 #50" / "Effort Atk+ #46" and use `use_progress_item` on the Pokémon that is
out; they cost a turn. A move that does not fit or a level-up evolution caused by a candy is
offered after the battle. New games get rare candy x3 and three effort items; every item of an
existing save bag now shows in the battle bag.

## Flags

- Game: PLA turns are the default. `--classic-turns` (or `PLA_TURNS=classic`) restores the old
  once-per-round flow (no styles; Q/E then say so). `--pla-turns` / `PLA_TURNS=pla` select it
  explicitly.
- The scripted `--auto-demo` battle beats were written for the round flow: they run classic
  unless `--pla-turns` is given (`battle_demo::demo_turn_mode`). Under PLA turns the scripted
  Thunder Wave missed with the fixed seed, so the old frames would change.
- Console `cargo run -p pla --example battle_demo`: unchanged by default; `--pla-turns` runs the
  queue with `move <slot> [agile|strong]` and prints the turn order.

Why PLA turns could be the default: all existing battle-controller, save-wire and headless tests
pass with it (290 `game::` tests), and the new headless session tests pass in both flows.

## Save format v4 (VERIFIED by tests, port format)

`PLASAVE 4` adds a ninth member field, the cumulative experience (`-` = unknown). v3 files load
through `migrate_v3_to_v4` (every experience unknown, which the rules read as "exactly the
threshold of the saved level"). Form and experience travel through the battle in
`ActiveBattleSession::member_meta` (the engine combatant has neither) and back into the save by
`save_wire::snapshot` / `apply_sync`; the nickname is kept. `install_party` now also keeps the
battle rules (status rules were lost before) and the move-style table, and restarts the queue.

## Assumed (port choices)

- Experience yield `base_exp * level / 7` (x1.5 trainers, never used yet: wild battles only),
  only the Pokémon out at the end participates, a fainted one gets nothing, a catch or a run
  gives no experience.
- Evolution context: no friendship, gender, time, place or weather, so those evolutions never
  fire in game; no "learn on evolution" moves.
- Each queue step is one engine turn, so end-of-turn effects (status damage, durations) tick
  after every single action instead of once per round. How PLA times them is UNKNOWN.
- Opponent style: deterministic rule in `game/battle_style_ai.rs` (normal if the normal move
  knocks out; strong if only strong knocks out or the user still acts first after a strong use;
  agile if the user then acts again first; else normal). The game's `modeChangeAi_*` scoring was
  not traced.
- Mastery (brief D3) is not tracked: every move whose data allows styles counts as mastered (the
  start-of-battle hint says so). Non-move actions (item, ball, switch) cost 0 action points
  (engine ASSUMED).

## Verification

```sh
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- env CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=$PWD/target cargo test --offline -p pla --lib game::
printf 'move 1 strong\nmove 2 agile\nquit\n' | cargo run -q --offline -p pla --example battle_demo -- --pla-turns
```

Tests added: save v3→v4 migration and round trip; experience/level/learn/evolution decisions
(`battle_progress`); style log lines; turn-order formatting and flag parsing (`battle_turns`);
AI style rule; style input state machine, PLA vs classic flow, faster opponent acting first,
queue advance on items, rare candy and effort items (`battle_controller`); evolution animation
system (`evolution_fx`); and two headless Bevy sessions with the real save wiring and input
system (`battle_flow_headless`): win → experience → level 15 → thunder-fang prompt → Shinx
evolves into Luxio → autosave → the v4 file reloads the same member; and the classic flag.

## What to check by eye (maintainer)

Start the viewer, walk into a wild Pokémon: the turn list appears at the top right; press Q or E
on a move and read the log; win a battle and watch the experience lines. For an evolution, use
the default party's bench: switch (S) to Starly (Lv 14, evolves at 14) or Bidoof (Lv 17,
evolves from 15) and win with it out, or give it "Level +1 #50" from the bag (I) first (Pikachu
itself evolves only with a Thunder Stone, which the bag does not use). Accept or skip a move
prompt with 1-4 / Backspace and watch (or stop with Backspace) the evolution pulse; the arena
model should change to the evolved species, and F5 + restart should keep it.
