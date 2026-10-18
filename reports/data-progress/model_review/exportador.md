# Model exporter fixes (follow-up of the generation 1-9 reviews)

Session exportador-modelos (Claude Sonnet), 2026-10-18. Tools touched: `.tools/blender_export_pokemon.py`,
`.tools/convert_national_batch.py` (no Rust changed). 550 packs were reconverted (only the affected ones) and
`data/national/model_export_index.tsv` regenerated (1576 models, all `ok`). Renders: CPU (lavapipe), 384 px, base pose,
57 packs compared before/after on contact sheets (30 named in the reviews + 27 random affected ones). Labels: VERIFICADO / PROBABLE.

## What was fixed (by cause)

1. **Wrong normal maps in the `_col/_nor/_amb` packs (root cause of the black patches / crossed colours)** - VERIFICADO.
   The packs' own material graph rebuilds the tangent normal as `R <- PNG alpha, G <- PNG green` (blue unused), so the
   `_nor_ts.png` files are NOT normal maps as stored (red is constant 1.0, alpha holds X). The exporter copied them as glTF
   normal maps, so lighting was garbage (black blocks, metal-like patches). Besides, some `_nor.png` files are not wired to
   any normal input in the pack at all (Tympole, Natu line) and were picked up by file name. Now: the Combine-Color layout
   of the graph is read and the normal rebuilt (`B` reconstructed from X/Y); unwired `_nor` files are dropped.
   314 packs carried raw `_nor` textures; all are now fixed (0 left). Of the 122 BAD_TEXTURE rows of the reviews, 102 belong to
   these packs (Ledyba line, Natu/Xatu, Whismur line, Skitty, Tympole line, Tapus, Bewear, Stufful, Pheromosa, Shelmet,
   Bouffalant, Morelull, ...); 15 of the 47 BAD_COLOR rows were also this (dark patches).
2. **Translucent materials exported opaque** - VERIFICADO. The simplified material dropped the alpha channel and blend mode:
   Victini's aura was a black mass, Gastly's gas shell (opaque, hid the face), Solosis/Duosion/Reuniclus' glass dome
   hid the eyes, eyelash/eye cut-outs. Now alpha is read from the texture (blend or cut-out by pixel statistics) when the
   pack's material is not opaque; materials named `smoke*` (opaque texture, drawn translucent in game) get alpha 0.5.
   91 packs now carry an alpha mode. PROBABLE for the `smoke` look, VERIFICADO for the texture alpha.
3. **Flame effect materials without data** (Centiskorch, Galarian Ponyta/Rapidash, Gigantamax Centiskorch, ...): the packs ship
   bare default grey materials (`FireCore*`, `FireMask*`, `SmokeCore/Mask*`), no texture and no colour anywhere. They get a flat
   translucent emissive flame/smoke tint (PROBABLE: invented look, not game data).
4. **Clips: engine could not find idle/walk/battle in Sword/Shield packs (gen 7/8)** - VERIFICADO. These packs only have
   `ba10_waitA01`, `fi20_walk01`, `ba20_buturi01`, ... and no `defaultwait01`, so `pokemon_anim.rs` fell back to the FIRST clip of the
   file (a landing animation, `ba01_land`). The exporter now adds alias copies under the names the engine looks up
   (`defaultwait01_loop`, `battlewait01_loop`, `walk01_loop`, `attack01`, `rangeattack01_start`, `damage01_start`,
   `down01_start`, `notice01`), also for FBX-style names (`Origin|pm0019_00_00_ba10_waitA01|Base Layer`). No Rust change was needed.
   Packs without `defaultwait`: 476 -> 8 (4 Unown letters and 4 Ogerpon VV=99 effect forms, which only carry a damage clip).
5. **Packs with 0 clips** (Hoopa Confined pm0774_11, Minior pm0824_12, Sinistea/Polteageist antique pm0972_11/12) - VERIFICADO.
   Cause: the "drop the duplicate 20000+ clip set" rule triggered because an unrelated low block (`05148_campose48_*`,
   `00939_loopaction04_*`) existed, and everything real was dropped. The rule now drops a 20000+ clip only if the same clip
   exists in a lower block. All four now have the full engine set.
6. **Batch script could not find nested folders** (`pm0916_00/pm0916_00_00`, Galarian Ponyta/Rapidash, Corsola, Centiskorch):
   `convert_national_batch.py` now also tries `pmNNNN_FF` as top folder, and has `--packs-file`, `--force`, `--out-root`.

Counts of the review rows (1649 rows, 245 not OK): BAD_TEXTURE 122 (102 fixed by cause 1; of the other 20, Victini, Gastly, Solosis line and the flame packs improved by causes 2/3), BAD_COLOR 47
(15 fixed), NO_ANIMS 53 (49 fixed; the other 4 are the Ogerpon effect forms), BAD_MESH 23 (see below, mostly not a defect).

## What is NOT an exporter defect / not fixable with the data we have

- **"Loose pieces" (rods, rings, whips, extra arms: Bulbasaur, Venusaur, Floragato, Meowscarada, Skeledirge, Quaquaval, Gogoat,
  Scatterbug, Mega Chesnaught, Mega Steelix, Mega Barbaracle, Hoopa Unbound, ...)** - VERIFICADO as a rest-pose artefact. The
  reviews rendered the bind pose without animation. Evaluating the idle clip in Blender (frame 1) collapses these meshes:
  Bulbasaur vine whips 1.15 m -> 0.17 m, Floragato yoyo 2.08 -> 0.11, Scatterbug particles/feelers, Quaquaval gel 2.47 -> 0.08,
  Mega Chesnaught vine 7.32 -> 0.61, Meowscarada claw 1.49 -> 0.84. The game hides/moves them through the animation. No mesh was
  dropped. (Wide framing in a static render is therefore not a bug; Gogoat's vine stays long on purpose.)
- **Washed-out / wrong colours of Trinity-shader packs (Aron cow pattern, Pineco teal, Steelix, Cofagrigus, Corviknight, Golbat,
  Primal Kyogre, Gholdengo flat gold)** - the baked base colour is the pack's own shader result. Checked on Aron (pm0304): the
  pack's own `ColorProcess`/`LayeredMaskScaling` node groups were re-run in Blender (Cycles emission bake) and give the same
  black/white mask pattern as our bake. Pineco's pack has no layer colours wired at all (albedo itself is teal). Metallic is 0 in
  these materials (metal comes from elsewhere in the game's shader), so gold/steel stay flat. Needs data we do not have
  (retail `.trmtr` parameters), not a conversion bug. Left as is.
- **Gengar pm0094 white/pink blotches and Parasect pm0047 collapsed mesh** (PLA catalogue packs): not touched; same Trinity
  colour limit / bad source mesh.
- **Roserade female pm0407_01_00** (game export): the converted copy from the user collection has idle/walk now; the in-game
  PLA export (`.gfpak -> glTF`, Rust side) was not changed.
- **Ogerpon VV=99 forms**: only one clip (damage01) exists in the pack.

## Re-run

`python3 .tools/convert_national_batch.py --only 1,2,3,4,5,6,7,8,9 --packs-file <list> --force --out-root work/cache/export_new`
(two Blender processes, ~15 packs/min), then move the packs into `work/cache/pokemon_national/` and run
`python3 .tools/build_national_export_index.py`.
