# Alpha Pokémon aura (Brief 41)

Port-side visual effect for alpha Pokémon: `crates/pla/src/game/alpha_aura.rs` and `alpha_aura.wgsl`.
Own implementation for Bevy; one reference image (a creature wrapped in a dark red-and-black aura) was
used as a visual guide only.

## Evidence

| Item | Status |
|---|---|
| Which spawn slots are alpha | VERIFIED data (`data/field_spawn_conditions.tsv`; `alpha` and `unique_alpha` flags), read in `game::wild` |
| Alpha field scale 2.5x (`LL_field`) | VERIFIED data (`data/pokemon_variants.tsv`), now the constant `wild::ALPHA_FIELD_SCALE` |
| Alpha spawn probability | UNKNOWN (only the slot weight is decoded; the port follows the slot flags) |
| The aura itself (look, colours, particles, light) | NOT in the extracted data. ASSUMED: every number was tuned by eye against the reference image |
| Alpha battle scale | UNKNOWN; the arena opponent keeps scale 1.0 and only gets the aura |

## What an alpha gets

* Rim shell: the creature's meshes drawn again (same mesh and skin handles) with a fresnel red emission
  only (additive, no body tint: the model renders opaque with its own colours), a slow pulse (about 3 s).
* Outline: ONE inverted-hull shell extruded 2.2 % of the height along the skinned normals (tight, with a
  mild noise). An earlier version stacked 4 noisy layers and tinted the body; it looked like two
  overlapped creatures and was removed after review.
* Billboards: a few small dark-red smoke wisps (faint by day, full at night) and sparks (max 24 per
  creature, 5+3 at high, 3+2 at low, only within 80 m of the camera, none with `--effects off`).
* A red point light (no shadows, only within 45 m of the camera, none with `--effects off`).
* All of it scales with the measured model size (world-space mesh boxes).
* Meshes with morph targets or without normals get no shell; blended meshes (glass) get none; eyes get the
  rim only.

## Where the flag is set

* Field: `stream_wild_spawners` in `examples/port_viewer.rs` inserts `AlphaAura` when the rolled variant is
  alpha.
* Battle arena: `setup_battle_arena` copies it when the overworld creature that started the battle has it.
* Debug: `--force-alpha [N|all]` and the automation commands `force_alpha` and `start_battle {"alpha":true}`.
* HUD: the NEARBY line shows `ALPHA`.

## Verification

* Unit tests: aura parameters, quality budgets, shell layers, wisp motion, light, texture, `--force-alpha`
  parsing, force rules (`game::alpha_aura`), the automation commands (`game::automation`).
* Headless Bevy tests: an alpha slot gets `AlphaAura` and the 2.5x scale and a normal one does not
  (`alpha_spawn` in `port_viewer`), and a start-up schedule guard with `--force-alpha all --style original`.
* Windowless automation screenshots (offscreen GPU, no window): field creatures close and mid-distance by
  day (`--time-slot 1`) and night (`--time-slot 3`), `--style plain` and `--style original`, `--effects off`
  / `low` / `high`, and battles against an alpha Haunter and Ursaring. Not compared with the real game.
* Known weakness: by day the dark smoke blends with the bright grass into a dirty olive tone; the red rim
  carries the look there. The effect reads best at night and in battle.
* `PLA_AURA_TUNE` (`key=value,...`, debug only) overrides the look numbers without a rebuild.
