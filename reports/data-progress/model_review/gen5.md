# Model review - generation 5 (national 494-649)

Session revision-gen5. Renders by CPU (lavapipe), 10 contact sheets checked by eye. Labels: VERIFICADO / PROBABLE (see gen5.tsv).

## Result
- 156 species, all with a model: 13 species use the game's own PLA model (Oshawott line, Petilil, Lilligant, Basculin, Zorua, Zoroark, Rufflet, Braviary, the three Forces of Nature); the rest are converted packs. **SIN MODELO: none.**
- 198 packs reviewed (18 of them PLA-catalogue packs, rendered from the converted copy; the in-game Pokedex screen was not opened in this session): 180 OK, 9 BAD_TEXTURE, 9 BAD_COLOR. No WRONG_SPECIES. No missing idle clip; walk clip missing only in pm0555_12_31 and pm0569_81_00 (battle-only forms, expected).
- 4 packs that were missing were converted (Galar Darumaka, Galar Darmanitan x2, Gigantamax Garbodor) and the export index regenerated.

## Pairing corrections (data/national/model_map.tsv)
- pm0550_00_41 is White-Striped Basculin (Hisui), not Red-Striped.
- pm0555_11_31 / pm0555_12_31 are Galarian Darmanitan (standard / Zen), not the Unova forms.
- pm0646_12_00 is White Kyurem and pm0646_13_00 is Black Kyurem (they were swapped).
- Two duplicate rows removed (pm0562_00_31, pm0618_00_31).

## Problems by cause
1. **Texture mapping of _col/_nor packs**: Tympole pm0535, Palpitoad pm0536, Seismitoad pm0537 (colour blocks/black patches, sev 2); Shelmet pm0616, Bouffalant pm0626 (sev 1).
2. **Face/eye material not applied**: Solosis pm0577, Duosion pm0578, Reuniclus pm0579 are plain green blobs with no eyes (sev 2).
3. **Effect mesh rendered opaque**: Victini pm0494, AuraEf_col shows as a black mass (sev 2).
4. **Washed-out / grey colours**: Cofagrigus pm0563 (sev 2); Minccino, Cinccino, Durant, Zekrom, Foongus, Woobat, Tirtouga, Carracosta (sev 1, PROBABLE).
5. Not defects: Snivy/Servine/Serperior thin vines and long rods are real meshes that inflate the framing.

## Species without model
None.
