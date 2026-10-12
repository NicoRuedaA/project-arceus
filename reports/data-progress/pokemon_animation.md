# Pokémon skeletal animation (idle / walk)

Code: `crates/pla/src/game/pokemon_anim.rs` (`PokemonAnimPlugin`), tests: `crates/pla/tests/pokemon_anim.rs` (headless, no window).

## Root cause of the "rest-pose T"
The exported glTFs are valid: one skin per model with `inverseBindMatrices`, `JOINTS_0` as UNSIGNED_SHORT, float `WEIGHTS_0`, and linear translation/rotation/scale channels. The failure was in the viewer: it copied mesh primitives out of the glTF into plain `Mesh3d` entities. That drops the skeleton nodes, the `SkinnedMesh` component and the `AnimationPlayer`; a mesh with joint attributes but no `SkinnedMesh` breaks the render pipeline layout, so earlier code stripped the joint attributes, leaving a static mesh.

## Fix
The viewer now spawns the glTF scene (`WorldAssetRoot`), so Bevy builds skin, joints and player. The plugin finds the player, builds an `AnimationGraph` with the idle clip (`defaultwait01*`) from `*_idle.gltf` and the walk clip (`walk01*`) from the sibling `*_walk.gltf` (same skeleton, same target ids), loops idle, and cross-fades to walk while the root moves faster than 0.25 m/s. No clip: model stays static. The exporter was not changed.

## Evidence (headless `cargo test -p pla --test pokemon_anim`)
- Pikachu 25, Geodude 74, Bidoof 399, Zubat 41: scene has `SkinnedMesh`, `AnimationPlayer` wired, idle and walk clips with non-zero duration and targets; 30/86, 41/59, 36/69, 16/40 joints differ between t=0 and mid-clip.
- Root movement switches Pikachu to walk and back to idle.
- Staravia 397 exports static only (rest pose disagrees with bind, see pokemon_batch.md): loads, no animation.
- Headless Bevy needs `App::finish()`/`cleanup()` and registered reflect types in tests.

## Not proven
One offscreen auto-demo run was made, but in that worktree the Pokémon cache was not under `work/`, so the frames still showed placeholder spheres: no visual proof of animated Pokémon in the viewer yet. Wild creatures only walk if world code moves them.

## Viewer panic fix (2026-10-12)

- Failure: `port_viewer` panicked when spawning a Pokémon glTF scene: `world contains the unregistered type bevy_transform::...::Transform`.
- Root cause: `pla` does not enable Bevy's `reflect_auto_register`, so the reflection registry was empty. The headless test hid this by calling `register_derived_types()` by hand; the viewer never did.
- Fix: `PokemonAnimPlugin::build` registers all derived types itself (idempotent). The test no longer does it manually and a new test checks the registry.
- Visually checked (one hidden-window `--script` run, no panic): the wild Staravia and the player Pikachu render with baked colour textures (Pikachu looks dark/under-lit). Animation was NOT verified visually: only one frame per scene was inspected.

## Root motion pinned (2026-10-12, fix/pokeanim; tests and data only, not yet seen by the maintainer)

- Symptom reported by the maintainer: Pokémon "do the animation and return to the starting point".
- Root cause: the walk clips carry ROOT MOTION. Census of all 356 packs (`examples/pokemon_clip_census.rs`): 381 of 382 `walk01_loop` clips move the `origin` bone only along +Z (dx = 0). Per cycle: Pikachu 0.600 m in 1.02 s, Geodude 0.800 m / 1.02 s, Bidoof 0.240 m / 0.62 s (form 10: 0.400 m / 1.02 s), Starly 0.242 m / 0.52 s, Zubat 3.000 m / 1.35 s. Other root-ish bones (`waist`, feet, shoulders) sway by under 0.1 m (Zubat `waist` 0.29 m in `defaultwait`); they are real body motion and are kept. No clip other than walk moves `origin` horizontally in the idle/walk set. Bevy applied that translation to the model, so it slid forward during the clip and snapped back at the loop while the wander system moved the entity separately.
- Fix (`pokemon_anim.rs`): `pin_root_motion` runs in `PostUpdate` after the animation systems and before transform propagation (same scheme as `player_anim.rs`): the `origin` bone keeps its rest x/z, the vertical component is kept.
- Walk speed matching: the clip's own root speed is measured online from the raw (unpinned) `origin` z (`RootMotionMeter`, after 0.3 s of walk and 0.5 s of clip time) and the walk playback speed is `entity speed / clip root speed` clamped to 0.5..2 (`walk_playback_speed`). Measured in the headless test: Pikachu 0.595, Geodude 0.796, Bidoof 0.396, Zubat 2.238 m/s, matching the census figures above.
- Tests: pure unit tests (speed scaling, clamp, meter on a looping ramp, vertical bob kept) and headless `walk_root_motion_is_pinned_and_speed_is_matched`: for the four species the `origin` x/z stays exactly at rest for 190 frames of walking while other joints keep rotating.
- Not proven: visual result in the viewer. The speed estimate is online, so the first ~0.5 s of the first walk plays at normal speed.
