# Model review - generation 7 (national 722-809)

Session revision-gen7. Renders by CPU (lavapipe), contact sheets checked by eye. Labels: VERIFICADO / PROBABLE (see gen7.tsv).

## Result
- 88 species: 3 PLA (Rowlet, Dartrix, Hisuian Decidueye), 85 converted. **SIN MODELO: none** (before: 77 without model).
- 124 model rows reviewed: 105 OK, 16 BAD_TEXTURE, 2 BAD_COLOR, 1 NO_ANIMS.
- 120 folders of the user's collection identified (PROBABLE) and written to `data/national/model_map.tsv`; all already converted under `work/cache/pokemon_national/`; export index regenerated.

## Identification method
Each unknown folder has its own icon (`*_00_big.png`); icons were compared with renders. Gen 7 models sit at pm0801-0892 (mixed with gen 8, which starts at pm0901+), not at pm0722+.
Multi-form packs: Oricorio 825_11-14, Lycanroc 829_11-13, Wishiwashi 820_11/12, Mimikyu 819_11/12, Minior 824_11/12, Silvally 862_11-28 (18 types), Necrozma 865_11-13, Magearna 882_*. FF=51 packs of Crabominable, Golisopod, Drampa, Zeraora = Mega (Z-A); 892_81 = Gigantamax Melmetal.
Least certain: pm0876 Xurkitree (by elimination among the Ultra Beasts), pm0801/0802 Mareanie/Toxapex (a previous partial note called pm0801 Nihilego; the render and icon fit Mareanie, Nihilego is pm0875), pm0883 Marshadow, pm0854 Turtonator, pm0858 Togedemaru, pm0821 Pyukumuku, and the form order of Mimikyu/Minior.

## Pairing corrections
- pm0722 was "Dhelmise" -> it is Chesnaught (652) and pm0722_51 its Mega. Dhelmise is pm0857.
- pm0729/pm0730 were "Stufful/Bewear" -> they are Skiddo (672) / Pangoro (675). Stufful/Bewear are pm0852/pm0853.
(Chesnaught, Skiddo, Pangoro belong to generation 6; the rows are fixed for the gen-6 report too.)

## Problems by cause
1. **Pack format _col/_nor/_amb (45 packs, folders named pmNNNN_00 without VV, plus 862/820)**: colours in blocks or black (BAD_TEXTURE sev 2): pm0803 Morelull, pm0821 Pyukumuku, pm0830-0833 Tapus, pm0852 Stufful, pm0853 Bewear, pm0854 Turtonator, pm0858 Togedemaru, pm0861 Type: Null, pm0876 Xurkitree, pm0880 Pheromosa, pm0820_12. Sev 1: pm0820_11, pm0857 Dhelmise. Cause: the exporter does not map this texture naming. Same packs use clip names ba10_waitA01 / fi20_walk01 instead of pmNNNN_..._wait01_loop: check the engine finds idle/walk.
2. **Gray / no colour**: pm0883 Marshadow, pm0892_81 Gigantamax Melmetal (BAD_COLOR sev 1).
3. **No animation**: pm0824_12_00 (Minior form 1) has 0 clips.
4. Rest pose / thin loose rods inflating the framing (not a model fault): pm0805, 0806, 0807, 0808, 0810, 0815.
5. PLA models (Rowlet, Dartrix, Decidueye Hisui): catalogue-verified; Pokedex screen not opened in this session.

## Species without model
None.
