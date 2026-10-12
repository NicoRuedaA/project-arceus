# Field packs census: terrain, zone models, collision and navigation (PLA update v262144)

Scope: the 121 GFLXPACK files under `bin/archive/field/` (63 `terrain/`, 55 `model/pack/`, 3 at the root: `ride_pokemon`, `resident_release`, `light`). Method: `.tools/gfdump` decodes every entry (Oodle where framed) to disk with an `index.tsv`; `.tools/gfpack_census.py` classifies the decoded entries. Reproduce:

```sh
cd .tools/gfdump && cargo build --release --offline
for f in <romfs>/bin/archive/field/{terrain,model/pack}/*.gfpak <romfs>/bin/archive/field/*.gfpak; do
  .tools/gfdump/target/release/gfdump "$f" work/dump/field/<relative name without .gfpak>; done
python3 .tools/gfpack_census.py work/dump/field
```

## Observed (direct)
- All 121 packs decode; **0 entries fail** (every entry is Oodle-framed or raw and matches its declared size).
- Decoded entry classes (heuristic on the first bytes): 4,739 BNTX textures, 9,818 FlatBuffer-like blobs, 11 VFXB, **775 Havok tagfiles** (`TAG0` at offset 4, header text `SDKV20200200` = Havok SDK 2020.0.2).
- **Navigation meshes:** all 63 tagfiles in `terrain/` contain `hkaiNavMesh` (with `hkcdStaticAabbTree` and a directed graph); sizes 3,420 B to 13.6 MB. They are the single-entry packs such as `ha_area00_s03_a`, plus one large entry in each main area pack.
- **Collision shapes:** all 712 tagfiles found in `model/pack/` contain Havok Physics NP types (`hknpCompositeShape`, `hknpShapeProperties`, `hknpMaterialDescriptor`...); sizes 2,652 B to 206,452 B. `model/pack/collision.gfpak` alone has 655 entries.
- **Terrain and zone meshes use the same `tr*` formats the port already parses.** The descriptors inside the main area packs reference `.trmdl/.trmsh/.trmbf/.trmtr` names and BNTX textures:

| Area pack (terrain/) | entries | model descriptors | meshes (trmsh) | materials (trmtr) | BNTX entries |
|---|---:|---:|---:|---:|---:|
| ha_area00 | 185 | 14 | 20 | 15 | 76 |
| ha_area01 | 485 | 29 | 43 | 30 | 99 |
| ha_area02 | 870 | 25 | 39 | 26 | 142 |
| ha_area03 | 413 | 22 | 32 | 23 | 137 |
| ha_area04 | 475 | 16 | 26 | 17 | 138 |
| ha_area05 | 356 | 13 | 19 | 14 | 88 |
| ha_area06 | 164 | 9 | 17 | 10 | 81 |

(descriptor/mesh/material counts are distinct names referenced inside FlatBuffer entries, not parsed models.)
- The 80 loose `.hkx` files under `chara/` and `demo/` are character accessory (hair/cloth) motion data, not terrain collision.

## Not yet known
- Where object **placement** (position/rotation/scale of each tile, tree, rock) lives; candidates are the `terrain_table.bin` files under `bin/field/terrain/`, `tree_collision_info.bin`, and FlatBuffer entries in `model/pack/ha_areaNN`.
- How terrain textures are blended, and the lighting data (`light.gfpak`).
- The Havok tagfile contents (no parser yet): the planned path is a tagfile reader that extracts triangle/box/convex geometry and navmesh polygons.

## Pokémon pack structure (pm0025_00_00.gfpak, same container)
Observed on one pack, 180 entries, all decoded without error:
- 16 BNTX textures (`body_{a,b,c}_{alb,ao,lym,nrm}`, `eye_{alb,lym,msk,nrm}`), 54 animation clips as `.tranm` + `.tracm` pairs named `<model>_<index>_<name>` (for example `defaultwait01_loop`, `walk01_loop`, `turn_r090`).
- The model descriptor (entry 153) names three meshes (LOD0/1/2: `pm0025_00_00.trmsh`, `_lod1`, `_lod2`), a `.trmtr` and a `.trskl`; each mesh entry (155, 175, 177) names its own `.trmbf` buffer; two material entries (156, 178) name the textures, and a `rare` material set exists (entry 154 references `pm0025_00_00_rare.trmtr`).
- The `u64` array right after the header offsets (one value per entry) is not FNV-1a-64 or FNV-1-64 of the basename, the name without extension, the lowercase variants or the in-pack names: 0 matches. The hash function is unknown, so entry-to-name pairing cannot use it; pairing by content (mesh descriptor sizes against buffer sizes, BNTX internal texture names) is the working path.

## Terrain composition (verified on ha_area00, ha_area03, ha_area05)
The ground is a **heightmap**, not a mesh. Inside each main area pack (`terrain/ha_areaNN.gfpak`) there are, for the single tile `areaNN_0_0` (the terrain table names exactly one cell per area):
- `areaNN_0_0_heightmap` (BNTX): **R16 float (`0x0a05`) 1025x1025** in ha_area00, **R16 float 2049x2049** in ha_area05 and **R32 float (`0x1405`) 2049x2049** in ha_area03. Values are normalized: min 0, max 0.854 / 1.0 / 0.999, means 0.35 / 0.44 / 0.24. Rendered as grayscale they show valleys, a river and the sea at height 0 (ha_area00), and craters and islands (ha_area03).
- `areaNN_0_0_normalmap` (4.8 MB in ha_area00) and `areaNN_0_0_splat_alpha0` (BC7, 1024x1024, 2 MiB): texture blending masks.
- Three blocks of exactly 16,777,288 bytes in ha_area00 (entries 6, 7, 8): a 72-byte FlatBuffer header followed by a **4096x4096 byte mask** whose only values are 0 and 255 (0.9 %, 0.13 % and 0.01 % set in the sampled rows). The first one, drawn as an image, lines up with the land side of the heightmap (not the sea): consistent with vegetation or detail placement masks for the `plants_*` models (grass01/02, stones01, trees...) listed in the same pack. Their exact meaning is not proven.
- A 12.6 MB FlatBuffer-like entry (entry 3) and the `area00_0_0.trmtr` material, not yet decoded.
The BNTX formats `0x0a05` and `0x1405` are NOT handled by `crates/pla/src/assets/bntx.rs` (the earlier census only covered loose `.bntx` files, not the 4,739 inside packs). `.tools/bntxdump` decodes them (tegra_swizzle block-linear, tile mode 0) and `.tools/terrain_preview.py` draws the result.

Still unknown: the horizontal size of a cell and the vertical scale that turns 0..1 into meters, which splat channel maps to which ground texture, and where explicit (non-scattered) objects such as buildings are placed.

## World scale and terrain descriptor (verified 2026-10-12)
Each area pack has one descriptor entry (the FlatBuffer that names `areaNN_0_0.trmtr`, `areaNN_0_0_detailmap_N.trtbf`, the `plants_*.trmdl` models and the `.trins` names) and one large per-sample entry (ha_area00: entry 3, 12.6 MB; its `f5` vector has 1,025 x 1,025 = 1,050,625 elements). Both carry inline 3-float structs:
- **Extent `(1025.0, H, 1025.0)`** (descriptor offset 104; also at the tail of the per-sample entry) in all seven areas, with H per area = 185.0, 121.756, 200.240, 127.079, 302.0, 168.0, 36.67 (ha_area00..06).
- A second struct `(s, H, s)` with s = 1.0009765625 (= 1025/1024) in areas 00 to 05 and 0.50048828125 (= 1025/2048) in area 06.

Independent check with the Havok navigation meshes (`hkaiNavMesh`, one per area; vertices as 16-byte hkVector4): the longest run of vertices spans x, z = 0..1024.3 (ha_area00), 0..1024.2 (ha_area01) and x 0..1017.9, z 0..1024.2 (ha_area03), with maximum y 155.9, 121.5 and 125.8. Heightmap maximum x H gives 158.0 (ha_area00: 0.854 x 185) and 126.9 (ha_area03: 0.9986 x 127.079), slightly above the navmesh maximum as expected for walkable surfaces.

Conclusions, bounded:
- **Observed:** every area is 1025 units wide in x and z (also for the 2049-sample heightmaps), local origin at one corner (navmesh minimum 0, 0, 0). Heightmap values 0..1 map to 0..H.
- **Contradiction:** the second struct `s` is 1.00098 for the 2049-sample areas 02, 03 and 05, which would give 2050 units; the navmesh of ha_area03 says about 1024. So `s` is not the sample spacing there; use extent / (samples - 1). It is consistent for the 1025-sample areas and for area 06.
- **Inferred:** the unit is metres (Havok convention); not proven.
- Areas are separate maps with their own local frame; no global placement between areas appears in these packs.

## Corrections to earlier statements in this report
- The three 16,777,288-byte entries of ha_area00 are named by the descriptor as `area00_0_0_detailmap_0..2.trtbf` (4096 x 4096 byte maps); the descriptor also lists the `plants_*` models they drive (grass01/02/05, flowers, shrubs, stones). That they gate vegetation placement is still an inference.
- The `FlatBuffer-like` entries rich in floats (for example ha_area00 entry 149) are `.trmbf` mesh buffers (a 36-byte-stride vertex blob and a u16 triangle index blob), not instance lists. The `.trins` instance files named in the descriptor (for example `area00_0_0_plants_tree01_X8Z9_d0.trins`, a grid of cells X/Z) have not been located yet: their names appear only in the descriptor.
