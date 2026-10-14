# In-game interface (UI): textures and layouts

Research and extraction only (phases 1-2 of the UI track): the game's own 2D interface
assets of update v262144 were exported from the user's dump and their layout files decoded,
so a later task can rebuild the screens in the port. No runtime code changed.

All output is game content and lives only under the git-ignored `work/ui/`. This report and
the committed tools hold format descriptions, counts and file/screen names only.

| What | Where (git-ignored) | Tool |
|---|---|---|
| PNG per texture | `work/ui/png/<screen>/...` | `cargo run -p pla --example ui_export -- <bin-root> work/ui` |
| Texture index | `work/ui/index.tsv` | same |
| Unpacked screen packs | `work/ui/unpacked/<pack>/...` + `index.tsv` | same |
| Contact sheets | `work/ui/sheets/<screen>.png` (+ `_p2`...) and `<screen>.tsv` | `.tools/ui_contact_sheets.py` |
| Decoded layouts | `work/ui/layouts/<screen>.json` | `.tools/ui_layout_decode.py --all` |
| Reconstructions | `work/ui/reconstruction/*.png`, `boxes/`, `index.tsv` | `.tools/ui_layout_render.py --proof` |

Status words: **VERIFIED** = checked on all real files or by an exact size/structure check;
**PROBABLE** = consistent on the data and visually right in reconstructions, not proven
field by field; **UNKNOWN** = not decoded or meaning not known.

## 1. Where the interface lives

- `bin/appli/<screen>/` (58 folders): 1,864 standalone `.bntx`, 257 `.arc`, 137 `.bin`,
  3 `.trlgt`, 1 `.prmb`.
- **The most important screens are not in `bin/appli`:** they ship inside 25 packs
  `bin/archive/appli/*.gfpak` (Game Freak GFLXPACK, Oodle-compressed entries, FNV-1a 64
  path hashes): `battle_menu` (command menu, moves, battle party list, battle status),
  `battle_resident` (battle HUD plates, action order), `resident_hud_arc00/01` (field HUD),
  `pause_menu` (options, save, online), `poke_dex` (Pokédex detail), `map`,
  `map_mask_texture`, `system_resident` (message box, button help, number picker).
  Each has language variants (`_eng`, `_fre`, ...); only the base and `_eng` packs are
  unpacked (11 packs, 362 entries, 0 decode failures).
- Pack entry names: 115 of 362 resolved by hashing the `bin/...` paths found inside the pack
  (VERIFIED on `battle_menu`, `battle_resident`, `pause_menu`, `system_resident`, `map`).
  The `resident_hud_*` and most `poke_dex` hashes do **not** match the paths their manifests
  name (hash input UNKNOWN for those packs); their archives are named from the manifest
  instead (the `_lyt.bin` lists the archive path and its `.bflyt`, which identifies the
  archive by content). 24 unnamed `poke_dex` entries are not layouts (no SARC/BNTX magic;
  content UNKNOWN, not needed here).

## 2. Phase 1: textures

`ui_export` reuses `pla::assets::bntx` (BC1/3/4/5/7, RGBA8, RGBA32F, Tegra deswizzle).
Mip 0 / layer 0 only. The BNTX component selector (`channels`, the GPU swizzle) is applied
so each PNG shows what the GPU samples (VERIFIED encoding: 0 zero, 1 one, 2-5 = R,G,B,A;
the UI uses `1,1,1,R` for BC4 alpha masks and `R,R,R,G` for BC5 grey+alpha).

| Source | Textures OK | Failed |
|---|---:|---:|
| `bin/appli` standalone `.bntx` | 1,864 | 0 |
| `bin/appli` layout archives (atlas inside each `.arc`) | 3,066 | 1 |
| Screen packs (base + `_eng`): standalone + archive atlases | 1,565 | 0 |
| **Total** | **6,495** | **1** |

By format (OK): BC7 2,317 · BC5 1,558 · BC4 1,478 · BC3 865 · BC1 166 · RGBA8 105 ·
RGBA32F 6. The one failure is `bin/appli/report_backup/bin/report_top_00.arc`: in
`romfs_patched` its node table points at the wrong data for 32 of 37 entries (the base copy
in `pk1/romfs` is consistent, so this looks like an overlay/rebuild defect of that one
file, cause UNKNOWN); the `pause_menu` pack carries a good `report_top_00.arc`.

62 screen ids have textures (51 `bin/appli` folders + 11 packs); 75 contact-sheet pages
(200 tiles per page, each tile labelled `<number> <texture name>`; the sidecar TSV gives the
full path, size and format per tile).

## 3. Phase 2: formats found

### 3.1 `.arc` = SARC (VERIFIED)

Standard Nintendo SARC (`SARC`, BOM `FEFF`, `SFAT` 16-byte nodes, `SFNT` names). **The name
offset in the node attributes counts 4-byte units**; both `pla::assets::sarc` and
`.tools/sarc_extract.py` read it as bytes, which only worked for one-file archives. Fixed
in this task (unit test added). Entries: `blyt/*.bflyt` 475, `anim/*.bflan` 2,348,
`timg/__Combined.bntx` 253, `fcpx/*.bfcpx` 191 (font pairs), `bgsh/*.bnsh` 58 and
`*.bushvt` 59 (shaders, not decoded).

### 3.2 `.bflyt` = Nintendo Ui2D layout "FLYT", version 9.0.0.0 (all 770 files)

Header 0x14: `FLYT`, BOM, header size, version u32, file size u32 (checked), section count.
Sections are `magic u32 + size u32 + payload`, read in order:

| Section | Content | Status |
|---|---|---|
| `lyt1` | draw-from-centre u8, width f32, height f32 (744 at 1920×1080, 26 at 1280×720), max parts size, name | VERIFIED |
| `txl1` / `fnl1` | u16 count, offsets relative to the table, names (texture names = BNTX names in the archive atlas) | VERIFIED (4,451/4,451 resolve) |
| `mat1` | u16 count, offsets from section start, materials (below) | VERIFIED sizes |
| `pan1 pic1 txt1 wnd1 bnd1 prt1 ali1 scr1` | panes: null, picture, text, window, bounding, part, alignment, scissor | layout VERIFIED, meaning below |
| `pas1` / `pae1` | children begin / end | VERIFIED (balanced in all files) |
| `grp1` `grs1` `gre1` | groups: name 34 bytes, u16 count, 24-byte pane names | VERIFIED |
| `usd1` | user data: u16 count, 12-byte entries (name offset, data offset, u16 count, u8 type) | PROBABLE |
| `cnt1` (449) `ctl1` (18) `cpt1` (2) | control / capture sections | UNKNOWN (counted, not decoded) |

Pane common block (0x54 bytes incl. header), VERIFIED by tree consistency:

| Offset | Field |
|---|---|
| 0x08 | flags u8: bit0 visible, bit1 children inherit alpha, bit2 location adjust |
| 0x09 | origin u8: bits 0-1 x (0 centre, 1 left, 2 right), 2-3 y (0 centre, 1 top, 2 bottom), 4-5 / 6-7 the same for the parent anchor |
| 0x0A | alpha u8 · 0x0B magnify flags u8 |
| 0x0C | name char[24] · 0x24 user data name char[8] |
| 0x2C | translate f32×3, rotate f32×3 (degrees), scale f32×2, size f32×2 |

Picture: 4 vertex colours RGBA8 (TL, TR, BL, BR), material u16, UV-set count u8, flags u8,
then 32 bytes per UV set (TL, TR, BL, BR). Window: inflation i16×4, frame size u16×4, frame
count u8, flags u8, content offset, frame table offset; content = vertex colours + material
+ UVs; each frame = material u16 + flip u8. Text: buffer/string sizes, material, font,
position, alignment, colours, font size, spacing (geometry only; **the text itself is never
decoded**, out of scope; no message labels were found in `txt1`). Part: property count,
magnify f32×2, 40-byte properties (pane name[24], 4 usage-flag bytes, offsets of an override
pane, override user data and a 52-byte basic-info block), then the part layout name.

Material record (v9), VERIFIED by exact size: 11,192 of 11,219 records parse to exactly the
next record; the other 27 carry a tail block when flag bit 19 or 21 is set (detailed
combiner / user shader, UNKNOWN content, kept raw).

| Bytes | Field |
|---|---|
| 28 | name |
| 4 | flags: bits 0-1 texture maps, 2-3 texture SRTs, 4-5 coord gens, 6-8 TEV stages, 9 alpha compare, 10 blend, 11 texture only, 12 separate alpha blend, 14 indirect, 15-16 projection maps, 17 font shadow, 18 alpha interpolation |
| 4 | UNKNOWN (`00 02 04 08` in samples) |
| 4 + 4 | black colour, white colour RGBA8 (output = black + (white − black) × texel) |
| 4 each | texture map: texture index u16, wrap/filter byte S, T (wrap = low 2 bits: clamp, repeat, mirror) |
| 20 each | texture SRT: translate f32×2, rotate f32, scale f32×2 |
| 16 each | coord gen (matrix, source, 14 bytes UNKNOWN) |
| 4 / 8 / 4 / 4 / 12 / 20 / 8 | TEV stage, alpha compare, blend, alpha blend, indirect, projection, font shadow |

Part property usage flags, PROBABLE (every combination in the data agrees with the values
stored, and the battle command menu renders exactly as in the game only with them):
byte 1 = basic info: 1 visibility overridden, 2 visibility value, 8 translate, 16 size,
32 scale, 64 rotate, 128 alpha, 4 UNKNOWN; byte 2 = material from the override pane
(1 colours, 2 textures; **its material index points into the outer layout**); byte 3 bit 0
= vertex colours of the override pane.

Pane position, PROBABLE: global = parent × translate(anchor + translate) × rotate Z ×
scale. The anchor is the parent's origin point shifted by half the parent size toward the
`parent origin` side (centre = the parent's pivot). The other reading (relative to the
parent's actual rectangle) misplaces 187 panes; the status pages only line up with this one.

Totals: 770 layouts, 18,787 panes (7,114 picture, 6,555 null, 2,142 text, 2,040 part,
729 window, 135 alignment, 43 bounding, 29 scissor), 11,219 materials.

### 3.3 `.bflan` = Ui2D animation "FLAN", version 9.0.0.0 (3,683 files)

`pat1` (tag: order, name, group names; the frame-range bytes are UNKNOWN and kept raw) and
`pai1`: frame count u16, loop u8, texture list, entries (name[28], tag count, kind 0 pane /
1 material), each tag (`FLPA` pane SRT and size, `FLVI` visibility, `FLVC` vertex colours
and pane alpha, `FLMC` material colours, `FLTS` texture SRT, `FLTP` texture pattern,
`FLAC`, `FLIM`, `FLCT`, `FLWN`) with targets (index, target, curve 1 step / 2 hermite,
keys: frame, value, slope). Decoded: 11,660 entries, 14,175 tags, 22,272 curves (20,319
hermite, 1,953 step). Target meanings for `FLPA/FLVI/FLVC/FLMC/FLTS/FLTP` are PROBABLE
(standard Ui2D order; the rest kept as numbers). The renderer evaluates them at one frame.

### 3.4 `*_lyt.bin` = FlatBuffers binding manifest (128 files: 72 in `bin/appli`, 56 in packs; VERIFIED structure)

Root table: field 0 resources {arc path, resource name}, 1 layouts {resource, `.bflyt`,
work name}, 2 animations {work, part path, `.bflan`, alias}, 3 panes {work, pane name,
part path, alias}, 4 one byte (present in 33 files, value 1, meaning UNKNOWN). It is the
game's own wiring of layout panes/animations to program names (e.g. `anime_keep`,
`pane_all`); field names are inferred from content.

### 3.5 Other files

- `uikit_*.bin` (71): FlatBuffers widget descriptions (lists, scroll areas, cursor groups);
  schema UNKNOWN, dumped generically into the JSON.
- 29 other `bin/appli` `.bin` (shop, Pokédex research tables, map tables, dress-up, tips, crafting list):
  game data tables, not layouts; several are already decoded elsewhere (`shop`, crafting).
- `.trlgt` (3, light rigs for the Pokémon model viewer) and `parameter/constant_data.prmb`:
  not UI layouts, not decoded here.

## 4. Reconstructions (proof)

`ui_layout_render.py` draws a layout offscreen with its textures: pictures (material
texture 0, UVs, texture SRT, wrap, black/white colours, vertex colours, alpha), windows
(nine-slice style frames, PROBABLE), parts with their overrides, and the layout's opening
and resting animations at their last frame. Not drawn: text (out of scope), 2nd/3rd texture
of multi-texture materials (shows as a plain white area where it is a mask), combiner
details, scissor/mask, runtime content (white squares are the game's placeholder textures
that code replaces with Pokémon/item icons).

Checked by eye against how the game looks (no reference screenshots are stored):

| Screen (pack/folder → layout) | Result |
|---|---|
| Battle command menu (`battle_menu` → `battle_command_03`) | Matches: Fight (A) diamond, Items / D-pad / Pokémon, Run (B), correct sizes and textures |
| Battle HUD (`battle_resident` → `battle_top_02`) | Matches: player plate on the brush circle with HP bar, party dots, stat arrows; button guides top-right |
| Enemy plate (`cmn_pokeplate_enemy_02`) | Drawn alone (the game places these over the 3D Pokémon at runtime) |
| Battle party list / status (`pokelist_battle_00`, `battle_state_00`) | Coherent rows, HP bars, move rows; icons are placeholders |
| Summary screen (`status` → `status_top_00`, `pt_status_scr_basic_00`) | Matches the game's layout: name plate, L/R tabs, info rows, EXP bar, markings |
| Pokédex detail (`poke_dex` → `pokedex_detail_00`) | Notebook page, tabs, research counters, list on the right |
| Field HUD (`resident_hud_arc01` → `fieldinfo_00`) | Time bar, stealth eye, mission list, ride and item shortcuts |
| World map (`map` → `map_top_00`) | Hisui map with fog clouds; area highlights show as white (mask texture not applied) |
| Bag, box (`bag_top_00`, `box_top_00`) | Frames right, but item/Pokémon grids are runtime lists (`uikit`), so the static file overlaps several grid copies |
| Options, message box, title menu | Static frames only; their rows/text are runtime |

18 layouts in the proof set, 0 render failures, 0 missing textures.

## 5. Usefulness for the port (ranked)

1. **Battle**: command menu, move list, battle HUD plates and HP gauges, action order
   (`battle_menu`, `battle_resident`).
2. **Field HUD** (`resident_hud_arc00/01`): time, stealth, missions, shortcuts, pick-up and
   area-name pop-ups.
3. **Party / summary** (`status`, battle `pokelist_battle_00`, `bag_top_over_00`).
4. **Bag / inventory** (`bag`, `system_resident` number picker and item message).
5. **Box** (`box`).
6. **Pokédex** (`poke_dex`, `pokedex_result`, `pokedex_del`).
7. **Map** (`map`, `map_mask_texture`).
8. **Boot / title / options / save** (`boot`, `titlemenu`, `title_demo`, `pause_menu`).
9. Everything else (crafting, shop, dress-up, missions, credits, minigames, mystery gift).

## 6. Not done / unknown

- Text: never decoded (out of scope); text panes are geometry only.
- `cnt1`/`ctl1`/`cpt1` sections, material tail blocks (27), `uikit_*.bin` schema,
  `_lyt.bin` field 4, the 4 UNKNOWN material bytes, `pat1` frame range.
- Multi-texture blending, scissor/mask panes, alignment-pane auto layout (children are drawn
  at their stored positions).
- `resident_hud_*` / `poke_dex` pack path hashes; 24 unnamed `poke_dex` entries.
- No game function was analysed for this task; the evidence ledger is unchanged.

## 7. What the next phase (using them in the game) needs

- A Bevy loader for the decoded JSON (or a Rust port of the decoder) that builds `bevy_ui`
  nodes or sprites from the pane tree: the transform rule above, origin/anchor, nine-slice
  windows, parts with overrides.
- Material colour interpolation and vertex colours as a small UI shader (or pre-tinted
  images), and a mask path for multi-texture materials.
- An animation player for `.bflan` curves (hermite/step) bound by the `_lyt.bin` aliases
  (`anime_wake`, `anime_keep`, ...), so screens open and highlight like the game.
- Runtime content: lists from `uikit_*.bin` (decode its schema), Pokémon/item icons from
  `icon_pokemon`/`res_item`/`res_pokemon`, and port-side text (development labels; game
  text stays out of scope).
- Collaborators regenerate everything from their own dump with the commands in the table
  at the top; nothing from `work/ui/` is ever committed.
