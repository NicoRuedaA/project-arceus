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
