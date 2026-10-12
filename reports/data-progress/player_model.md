# Player character model (p1 / p2)

Tool: `crates/pla/examples/player_export.rs` (+ `crates/pla/src/assets/player_model.rs`), index [`data/player_model_export_index.tsv`](../../data/player_model_export_index.tsv), contact sheet script `.tools/render_player_sheet.py`, table decoder `.tools/dressup_table_decode.py`. Output (git-ignored): `work/cache/player/<p1|p2>_default/player.gltf`.

Run: `player_export <romfs>/archive/chara/data work/cache/player --index data/player_model_export_index.tsv` (flags: `--variant`, `--parts a,b,..`, `--name`, `--materials` (dump), `--skin N`, `--no-skin-tone`, `--skip-hide`).

## Result

Both variants export as ONE skinned glTF each (p1: 6 parts, 26 primitives; p2: 6 parts, 32 primitives) on one merged skeleton with 9 clips. Both were rendered offscreen (Blender headless, one process, unlit) and looked at: the Survey-Corps-style outfit, face, hair shape, scarf, belt, boots and bag are all recognisable and attached correctly; walk pose frames look like a walk. Not proven: hair/eyebrow colour (white, see below), skin tone (hypothesis), lighting/shader, expression/face rig, cloth/hair physics.

## What the packs hold (resolved by FNV-1a 64 of the full path)

| Pack | Content |
| --- | --- |
| `pc/<v>/model/<cat>/<stem>.gfpak` | one pack per part, files flat at `bin/chara/data/pc/<v>/model/<cat>/<stem>/<stem>.{trmdl,trmsh,trmbf,trmtr,trskl}` + `<stem>_<mat>_{alb,ao,msk,mtl,nrm,rgn,wlm}.bntx`. p1: 66 packs (bag 1, bottoms 5, dress 10, eyebrow 1, eyewear 4, face 1, hair 14, hat 11, shoes 14, tops 5), p2: 71 (adds `base`, hair 16, face 1). |
| `pc/p2/model/base/p2_base0001_00_default` | only a `.trmdl` and the ONLY full `.trskl`: 76 nodes, 74 rigs, 10 IK records, no mesh |
| `pc.gfpak` (714 entries) | the `p2_base` trmdl/trskl copy, `pc0002_00.{tralk,trbik,trspb,tracr,tracp,tracs,tracl}` (look-at, biped IK, spring bones, animation controller), 423 `pc0002_00_<nnnnn>_<name>.tranm` + 280 `.traef` |
| `pc/dressup_motion.gfpak` (9,398 entries) | same 423 clip names resolve here too (22 manifests of ~420); not used |

Animation directory: the text of the folder holding the `.tranm` files is unknown (candidates under `bin/chara/data/pc/...` all failed), but all 423 hashes invert to ONE FNV state (7618479347468000054) before the `/name` suffix, so names resolve exactly through that state (constant `ANIM_DIR_STATE` in the example). Clip groups by number: 01xxx battle, 02xxx field (wait01 02001, walk01 02101, run01 02121, dash01 02140, jump/land/swim, 02301-02325 idling01-09, turns, sit...), 03xxx ride/other-state sets, 05xxx/06xxx actions (throw, pickup...), 07xxx gestures, 09xxx photo poses. Exported: `idle`(02001), `idle2`(02012), `walk`(02101), `run`(02121), `dash`(02140), `jump_start`, `jump_fall`, `land`, `swim`; all 9 parse and map onto the merged skeleton (62-64 tracks).

## Skeleton (the key finding)

Parts do not carry copies of the biped. Face, hair and clothes packs each carry a small extension `.trskl` whose root field 4 is a rig offset: face 74 (45 nodes p1, 51 p2), hair 125 (<=22), clothes 147+ (p2 default dress 31 skirt/sleeve/parts bones). Root nodes of an extension name their parent bone in the base skeleton in the locator string (`head`, `hips`, `neck`, `left_arm_02`). Vertex joint bytes are indices in that one global rig space (with unused gaps). `build_player_skeleton` appends the extension nodes/binds to the base and keeps a global-to-compact rig table; vertex joints are remapped. Existing `TrSkl::parse` rejects IK records and rig offsets, so IK-tolerant variants were added (`parse_ignoring_ik`, IK records ignored, not applied). Bind/rest agreement: base 1e-4 OK; the p2 hair extension is off by up to 4.5e-4 m, so the player uses 1e-3 (`BIND_EPSILON`).

Clip tracks name 63 bones; the only name absent from the skeleton is `chara_root_exp` (1 zero translation key in every clip), dropped. Extension bones (skirt, hair, sleeves) have no animation tracks: they stay at rest (spring bones `trspb` not simulated). Walk/run clips include forward root motion (the posed bbox moved about 0.5-0.8 m along facing by mid-clip); the viewer must either drive movement from the clip or strip it.

## Tables

* `dressup_table_p1.bin` (948 rows) / `p2` (977): a pure item catalogue. Each row = one item colour variant (`p1_hrs0001_00_default_darkbrown`, ...), many empty-string fields, a few string fields (pack stem, `bin/chara/data/pc/p1`, `motion`, category `Shoes`...), 64-bit hashes (FNV of unresolved strings) and small ints. It has NO default-outfit flag. 66/68 distinct item stems match the 66/71 packs. Colour variants (15 hair colours `darkbrown`... in `appli/table/hair/hair_color_table.bin`, names only) are not materials in the packs.
* Default look used here (parts named `default`, which also render as the Survey-Corps-style uniform): face0001, eyb0001, hair (`p1_hrs0001_00_default`, `p2_hrs0002_00_default`), `drs0001_00_default` (full-body dress), shoes `ftw0002_00_default` (straw sandals), `bag0001_00_default`. Whether this is the game's starting outfit is NOT proven (the Galaxy-team parts `*_team` are an alternative).
* `skin_color_table.bin`: 8 rows `{u64 key, u8 r,g,b}`: two groups of the same 4 tones (253,232,215), (252,222,220), (204,162,146), (139,108,103); keys are unresolved hashes, so which group is p1/p2 and which tone is default is unknown. The exporter multiplies tone 0 (lightest) into skin-bound materials (`face`, `face_skin`, `body_skin`) in linear light: a hypothesis, switchable (`--skin N`, `--no-skin-tone`).

## Materials

All part TRMTR `BaseColorLayer1..4` are white (255,255,255) placeholders; colour of clothes comes from the albedo textures (the baked mask-lerp from [pokemon_color.md](pokemon_color.md) is applied, unchanged). Hair and eyebrow colour come from a runtime colour choice that was not found, so hair/eyebrows export with their albedo only (grey-white). Eye/hair/clothes shader extras (emission, roughness, AO, metallic, weather layer) are not applied.

## Size, origin, facing (rest pose, glTF metres, from vertex positions)

* p1: x -0.643..0.643 (arms out in the rest pose), y 0.000..1.492, z -0.216..0.183. p2: y 0.000..1.459, z -0.195..0.183.
* Height is ~1.46-1.49 m including hair spikes/hair (face top ~1.40 m), not 1.8 m: scale the 1.8 m capsule down, or scale the model if 1.8 m is wanted.
* Origin: at the feet (min y = 0), x centred on the midline, z about at the body centre (feet extend to about +0.18).
* Facing: +Z in glTF (confirmed in the renders: the face and toes are on the +Z side, the bag on -Z). Y up.
* "hide_*" meshes (hide_a..d) are real geometry (back hair, gaiters), not hidden-only parts: dropping them left the back of the head bald and the p1 ankles bare, so they are kept (`--skip-hide` drops them).

## Not done / limits

Bonus trainers (`tr/`) and `cc` characters not exported (the part layout differs; not investigated). Trainer/NPC packs are not assumed to share this layout. Face expressions, IK, spring bones, hair/skin colour variants and dress-up hide logic are not implemented. Only frames 0 and mid-clip were looked at, offscreen.
