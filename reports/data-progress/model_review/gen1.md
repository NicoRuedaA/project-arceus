# Model review - generation 1 (national 1-151)

Session revision-gen1 (Claude Sonnet), 2026-10-18. Renders by CPU (lavapipe) at 384 px, same framing, base pose (not animated), 14 contact sheets checked by eye. Labels: VERIFICADO / PROBABLE (see gen1.tsv).

## Result
- 151 species, all with a model: **SIN MODELO: none.** 52 species have a pack from the game's own PLA catalogue (66 packs incl. forms); the other packs come from the user's collection (99 species converted).
- 249 packs reviewed (base, female/alt, colour variants, Alola/Galar/Hisui/Paldea forms, Megas, Gigantamax): 206 OK, 26 BAD_TEXTURE, 15 BAD_COLOR, 2 BAD_MESH. No WRONG_SPECIES: every model is the right Pokemon.
- 14 packs were missing and were converted with the Blender exporter: 12 Gigantamax packs (Venusaur, Charizard, Blastoise, Butterfree, Pikachu, Meowth, Machamp, Gengar, Kingler, Lapras, Eevee, Snorlax) and Galarian Ponyta / Galarian Rapidash (they sit in the nested folders pm0077_00 / pm0078_00, which the batch script does not find). Export index regenerated (1576 models, 1572 ok).
- Animations: all 249 packs have the idle clip (defaultwait) and all but the 12 Gigantamax packs have walk (Gigantamax is battle-only, expected). Legendary birds 144-146 have only wait/walk/notice (no battle idle). The in-game Pokedex was not opened in this session, so the PLA rows are judged from the converted copy only.

## Pairing corrections (data/national/model_map.tsv)
- pm0026_51_00 was labelled "Alolan Form"; it is Mega Raichu X. pm0026_52_00 is Mega Raichu Y (was X).
- Paldean Tauros: pm0128_11_51 is the Combat Breed (was Tauros form 0), pm0128_12_51 is the Blaze Breed (red flame marks, was Combat), pm0128_13_51 is the Aqua Breed (was Blaze). Rows 11/12/13 now carry form 1/2/3. PROBABLE (visual).
- 9 exact duplicate rows removed (Galar Meowth, Slowpoke, Slowbro, Farfetch'd, Weezing, Mr. Mime, Articuno, Zapdos, Moltres).

## Problems by cause (all in gen1.tsv)
1. **Face/body texture parts mismapped (black patches, blocks, broken face)**: Rattata x4 and Raticate x4 (sev 2), Ekans (sev 2, also fully stretched pose), Nidoran-male, Nidorino (sev 2), Nidoqueen, Nidoking (sev 1), Gastly (round sphere with no eyes/mouth, sev 2, PLA), Gengar (white/pink blotches, sev 2, PLA), Dewgong, Kabuto (sev 1). VERIFICADO for the sev 2 ones.
2. **Wing / translucent material wrong**: Butterfree x2 (grey-black blobs instead of white wings, sev 2); Caterpie orange blotches, Victreebel black patch (sev 1).
3. **Collapsed mesh with metal-like shading**: Parasect x2 (sev 3, PLA pack): shape unrecognisable. Floating sphere meshes on Paras x2 (sev 1).
4. **Washed-out / wrong colours**: Nidoran-female and Nidorina (grey/white with black patches, sev 2), Clefable (beige, sev 2, PLA), Krabby (sev 2), Kingler, Spearow x2, Golbat x2, Venomoth, Geodude, Graveler, Ponyta, Rapidash, Aqua Breed Tauros (sev 1, PROBABLE).
5. **Helper meshes inflate the framing (not real defects)**: Bulbasaur and Venusaur long antenna rods, Galarian Weezing chimney, Gigantamax Gengar white box, Galarian Rapidash black blob (sev 1).
6. **Cause on exporter side**: items 1-4 look like texture-layer / material-mask problems of the old `_col/_nor` style packs and of the PLA baked colours; the exporter was not touched (out of scope), the affected packs are listed here for the exporter panel.

## Species without model
None (all 151 have at least one model).
