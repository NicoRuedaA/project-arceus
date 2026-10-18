# Model review - generation 4 (national 387-493)

Session revision-gen4. 190 renders by CPU (lavapipe) checked on 11 contact sheets, plus an in-game pass of the F7 species panel for the 154 PLA packs (every one placed its own pack). Labels: VERIFICADO / PROBABLE (see gen4.tsv).

## Result
- 107 species, all with the game's own PLA model (none converted from the external collection). **SIN MODELO: none.**
- 180 packs reviewed (169 PLA rows incl. females/forms, 11 extra "Mega" packs that the game does not have): 179 OK, 1 NO_ANIMS. No WRONG_SPECIES, BAD_MESH, BAD_TEXTURE or BAD_COLOR found.
- Walk/idle: all packs export clips except the one below. Variants (Burmy/Wormadam/Shellos/Gastrodon/Cherrim/Rotom/Hippopotas/Hippowdon) pair to their canonical form.
- No pairing corrections were needed in data/national/model_map.tsv.

## Problems by cause
1. **Game export gap (.gfpak -> glTF)**: Roserade female pm0407_01_00 exports only a static pose (no idle/walk, sev 2). The F7 panel uses base pm0407_00_00, so play is unaffected.
2. **Extra non-canon forms (not defects)**: 11 packs pm0NNN_5[12]_00 (Staraptor, Lopunny, Garchomp x2, Lucario x2, Abomasnow, Gallade, Froslass, Heatran, Darkrai) show the right species but the game has no such forms; form names are PROBABLE and they are unreachable in play.
3. Not defects: Magnezone looks dark (metal), Tangrowth shows an arms-out first frame.
4. Shiny packs (VV=01) look plausible, judged by colour only (PROBABLE).

## Species without model
None.
