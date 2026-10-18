# Model review, generation 8 (national 810-905)

Reviewer: Sonnet subagent, 2026-10-18. Renders by CPU (lavapipe) at 384 px, same framing, base pose (not animated). Detail per model in `gen8.tsv`.

## Summary

- 96 species. 7 use the game's own PLA model (899-905, 9 model packs incl. forms), 89 converted from the user collection.
- At the start 87 species had no model. All 87 were identified among the unknown folders (section B of `modelos_faltantes.md`) and converted; **species without a model now: 0**.
- Identification: the folders pm0901-pm0990 (three-part names) hold the Galar models with their own internal numbering (NOT the national number: e.g. pm0948 is Grookey 810, pm0983 is Urshifu 892). Method: the icon inside each folder (`*_00_big.png`) plus a render of the converted model. Everything is labelled PROBABLE in `model_map.tsv` (visual identification); the render matches the species in all cases.
- 142 model rows reviewed (122 main models + 20 Gigantamax). Main models: 93 OK, 18 NO_ANIMS, 4 BAD_TEXTURE, 6 BAD_MESH, 1 BAD_COLOR.

## Corrections of existing pairings

- pm0918_00_00 was Corvisquire: it is Rookidee (821). pm0919_00_00 was Corviknight: it is Corvisquire (822). pm0920_00_00 is the real Corviknight (823); its Gigantamax pm0920_81 confirms it.
- pm0942_00/pm0942_01 (Indeedee): assigned by looks (_00 female with ram horns, _01 male). DOUBTFUL: it contradicts the PLA convention FF=01 = female. Check in game.
- `model_map.tsv` had 108 duplicated rows (same pack twice, generator bug); they were removed for the packs of this generation (14 duplicated packs remain in other generations).
- pm1007_11_00 (unknown) is Ursaluna Bloodmoon (901 form 1).

## Problems by cause (PROBABLE unless stated)

1. NO_ANIMS, 18 main models: Gossifleur, Eldegoss, Yamper, Boltund, Wooloo, Dubwool, Sizzlipede, Centiskorch, Obstagoon, Blipbug, Dottler, Orbeetle, Cursola, Dracozolt, Arctozolt, Dracovish, Arctovish (pm0905-0908, 0911, 0912, 0915, 0916, 0928_00_31, 0930-0932, 0947_00_31, 0960-0963) plus Sinistea Phony/Antique (0 clips). Cause: the Sword/Shield models carry only battle clips (`ba10_waitA01`, `ba01_landA01`...) and no `defaultwait01`; `pokemon_anim.rs` falls back to the first clip (landing). The 17 Gigantamax packs have the same issue (low severity). VERIFIED by reading the glTF clip names.
2. BAD_MESH, 6: Rolycoly 0933, Carkol 0934, Milcery 0924 (pieces floating apart in base pose; may be fine animated), Eiscue Noice 0975_12 (still shows the ice cube) and Cramorant Gulping/Gorging 0917_12/13 (identical to base: the prey is missing). Cause: the exporter keeps or drops meshes that the game toggles by visibility animation.
3. BAD_TEXTURE, 4 main + 3 Gigantamax: Centiskorch 0916 (16 of 22 effect materials without texture), Urshifu 0983_11/12 (look identical), Polteageist antique 0973_12 (same as phony); Gigantamax 0916_81, 0929_81, 0937_81 (effect materials without texture).
4. BAD_COLOR, 1: Corviknight 0920_00 pale silver instead of dark steel (possible `_lym` mask).
5. Form-level looks are all separate packs (FF=11+n): Alcremie 9 decorations, Toxtricity, Morpeko, Zacian/Zamazenta, Calyrex 3 forms, Urshifu, Zarude, Eiscue, Sinistea, Polteageist, Cramorant, Indeedee. Mega Falinks (pm0923_51, Legends Z-A) is a guess from FF=51 = mega 1 and the render (PROBABLE).

## PLA species (7)

899 Wyrdeer, 900 Kleavor, 901 Ursaluna, 902 Basculegion (male/female), 903 Sneasler, 904 Overqwil, 905 Enamorus (incarnate/therian): renders of their glTF look correct. Not checked inside the in-game Pokedex (no GPU/viewer used).

## Species WITHOUT model

None. All 96 species of generation 8 now have at least one model (Gigantamax packs exist for 19 of them).

## Conversion

133 packs converted (`work/cache/pokemon_national`, git-ignored) with `.tools/blender_export_pokemon.py` unmodified; `model_export_index.tsv` regenerated (the cache is shared, so the index also lists packs converted by other review sessions). Four packs have no animation clips (pm0972_11/12, pm0774_11, pm0824_12).
