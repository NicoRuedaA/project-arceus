# Model review - generation 3 (national 252-386)

Session revision-gen3. Renders by CPU (lavapipe), 12 contact sheets (204 packs) checked by eye. Labels: VERIFICADO / PROBABLE (see gen3.tsv).

## Result
- 135 species, all with a model: 20 species use the game's own PLA model (37 packs: Wurmple line, Lotad line, Seedot line, Taillow/Swellow, Wingull/Pelipper, Ralts line, ...); the rest are converted packs. **SIN MODELO: none.**
- 204 packs reviewed (the 37 PLA-catalogue packs were rendered from the converted copy; the in-game Pokedex screen was not opened in this session): 149 OK, 39 BAD_TEXTURE (17 severity 2, 22 severity 1), 16 BAD_COLOR (4 severity 2, 12 severity 1). No WRONG_SPECIES, no BAD_MESH, no NO_ANIMS: every pack has idle (wait) and walk clips.
- No pairing corrections were needed in data/national/model_map.tsv; no pack had to be converted, so the export index is unchanged.

## Problems by cause
1. **Texture mapping of _col/_nor(_ts) packs (3DS-style, 64 of the 204 packs)**: 36 packs show black/metallic patches or crossed colours (Whismur, Loudred, Exploud, Skitty, Wailmer, Claydol, Cradily, Anorith, Huntail, Castform normal/Snowy, Ninjask sev 2; Delcatty, Shedinja, Nincada, Lileep, Spheal, Sealeo, Walrein, Clamperl, Gorebyss, Relicanth, Castform Sunny, Beautifly sev 1). Same cause as generation 5; exporter fix needed, packs left as they are.
2. **Wrong/washed colours in _baked packs (PROBABLE)**: Aron (cow-like black/white), Lairon, Aggron, Mega Aggron, Registeel, Dusclops (near white), Marshtomp (teal), Nuzleaf, Shuppet, Seviper, Numel (green patch) sev 1; Primal Kyogre (lilac instead of dark blue) sev 2.
3. **Face/cap mask not textured**: Spinda has blue head and ears, sev 2; Mega Mawile has pink/yellow patches over a grey body, sev 2 (PROBABLE).
4. **Grey flat mouth zone**: Swalot x2, sev 1 (PROBABLE).
5. Not defects: Swampert, Linoone, Galarian Zigzagoon look lying down in the standard framing but are upright (checked at two yaw angles); Torkoal, Whiscash, Barboach, Swablu/Altaria and Mega Chimecho look small because of long clouds/whiskers/ribbons that inflate the framing.

## Species without model
None.
