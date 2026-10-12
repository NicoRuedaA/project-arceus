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
