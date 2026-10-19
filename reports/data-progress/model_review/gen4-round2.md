# Model review - generation 4, round 2 (national 387-493)

Session gen4-fixes (2026-10-19). Round 1 (`gen4.md`) judged converted copies; this round judges what the engine shows:
CPU viewer (no window, no GPU), `--spawn-species` with an orbit camera at yaw 0 and 180, idle clip, 136 species/forms.
Metadata only; evidence in `work/scenes_out/gen4-fixes/` (git-ignored). Labels: VERIFIED / PROBABLE / UNKNOWN.

## What the game shows (VERIFIED)
- 107 species. 143 of 154 catalogue entries come from the game's own PLA packs; 11 come from the converted collection
  (forms 398:1, 428:1, 445:1-2, 448:1-2, 460:1, 475:1, 478:1, 485:1, 491:1). Those 11 packs were reconverted by the agy batch
  and are in metres (`gltf_unit_scale.py --dry-run`: nothing to scale).
- Code defect: `resolve_model` fell back to the catalogue's form 0 before looking at the collection, so F7 / `--spawn-species`
  showed the base model for the forms above. Fixed (exact form first).

## Defects by family
| Species | Defect | Cause | State |
|---|---|---|---|
| 392 401 402 418 425 426 428* 432 446 452 456 460 463 477 483:0 484:0 (+ female packs 401 402 418 456 460) | Textures on the wrong part of the body (Drifloon/Drifblim cross, Kricketune mouth, Abomasnow/Lickilicky/Munchlax/Drapion patches) | PLA block `UVScaleOffset (2,1)` not applied; Drifloon pack file names unpadded (`pm425_...`) so the fixer skipped it | FIXED (`fix_pla_uv_mirror.py`, regex accepts 3-digit names) |
| 442 Spiritomb | Opaque pink disc hides the Pokemon | `Transparent` shader blocks `body_b_00/03` exported opaque | FIXED, PROBABLE look (alpha 0.35) |
| 483:1 Origin Dialga | Dark blue blob over the chest/back | `gem_mesh` is a `Transparent` block; alpha rule applied (0.4) but the blob did not change on renders | NOT FIXED, UNKNOWN (retail blend not decoded) |
| 398:1 445:1-2 448:1-2 460:1 478:1 485:1 491:1 | Base model shown instead of the Mega pack | `resolve_model` fallback order | FIXED in code (Rust, needs F7 check by the user) |
| 484:1 Origin Palkia | Pale cover shell over the body | `Transparent` block `cover_mesh` | NOT FIXED, UNKNOWN |
| 407 Roserade female | Static pose only (no idle/walk) | export gap, round 1 | NOT FIXED (F7 uses the base pack) |
| 390 391 392 467 479:* | Fire / Rotom parts look flat | `Unlit` shader blocks | not a defect (judged against icons) |
| Others (95 species/forms) | none seen at 2 angles | - | OK |

*428 Lopunny base needed the fix; its Mega pack is a collection pack and was already right.

Species with defects: 20 of 107 (counting 483 and 484 once each). Fixed 18 by data or code; 2 open (Origin Dialga, Origin Palkia).

## Limits (honest)
- Walk clip: a spot check of 8 species showed no visible difference from idle in the CPU shots; no walk-specific defect found, not proven.
- Shiny packs (VV=01) are not reachable in the viewer; colour not re-checked.
- No Blender reconversion was needed: no collection pack of generation 4 showed a defect.
