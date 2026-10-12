# Havok navigation meshes and collision shapes (PLA update v262144)

Reader: `crates/pla/src/assets/havok_tag.rs` (tagfile container), `navmesh.rs` (`hkaiNavMesh`), `collision.rs` (`hknpMeshShape`). Format derived from the files' own bytes (embedded type-name strings, section tags); no Havok SDK code was used or consulted. Reproduce:

```sh
cargo run --release -p pla --example navmesh_report     # writes work/cache/navmesh/ (OBJ, PNG previews, summary tsv)
cargo run --release -p pla --example collision_report   # totals and material names
```
Per-pack summary (committed): [`data/field/navmesh_summary.tsv`](../../data/field/navmesh_summary.tsv).

## Tagfile format (observed)
- Sections: big-endian `u32 = size | flags << 30` (size includes the 8-byte header; flag bit 0 = leaf), 4-byte tag. `TAG0` > `SDKV` (`20200200`), `DATA`, `TYPE` > `TST1 TNA1 FST1 TBDY TPAD`, `INDX` > `ITEM`.
- Type table: packed ints (`0xxxxxxx`; `10xxxxxx` +1 byte; `110xxxxx` +2; `1110xxxx` +3). `TNA1` = type names + template args; `TBDY` = per type `index, parent, flags` then optional format / subtype / version / size+align / abstract / members `(name, flags, offset, type)`.
- `ITEM` = 12-byte little-endian records `(type | kind<<28, offset, count)`, kind 1 = object, 2 = array. Inside an object every pointer or array member is a 4-byte index into this item table (0 = null); array length is the item's `count`. Member offsets in `TBDY` are offsets in this compact serialization (pointers are 4 bytes), not in-memory offsets.
- Result: 63/63 terrain navmesh files and 655/655 + 57/57 collision files parse; every item reference resolves.
- Unexplained: an extra 2-byte value (`c1 00` / `c2 00`) sits before the member count of some types (for example `hkReferencedObject`); it is skipped. Template-argument value semantics, interface lists and `TPAD` are not interpreted.

## hkaiNavMesh (proven)
- Layout: `faces` (16 B: startEdgeIndex i32, startUserEdgeIndex i32, numEdges u16, numUserEdges u16, clusterIndex i16, pad), `edges` (20 B: a i32, b i32, oppositeEdge u32, oppositeFace u32, flags u8, pad, cost half), `vertices` (hkVector4, x y z w, 16 B), per-face and per-edge `i32` data arrays, `aabb`, `erosionRadius`. A face polygon is the `a` vertices of edges `startEdge .. startEdge+numEdges`.
- Counts: 63 packs, **70 meshes, 359,579 faces**. The 7 main area packs hold two meshes each (a `NavMesh`+`NavGraph` pair twice) plus `ha_area11`.

| Area | primary faces | vertices | x,z range | max y | vertices within 2 m of heightmap (H) | median abs dy |
|---|---:|---:|---|---:|---:|---:|
| 00 | 24,177 | 30,968 | 7.8–1024.3 | 155.9 | 99.78 % (185.0) | 0.171 |
| 01 | 47,076 | 62,112 | 0.6–1024.2 | 121.5 | 97.72 % (121.756) | 0.152 |
| 02 | 57,978 | 83,777 | 0.6–1024.2 | 200.3 | 99.94 % (200.24) | 0.111 |
| 03 | 32,037 | 43,150 | 0–1024.2 | 125.8 | 99.82 % (127.079) | 0.142 |
| 04 | 94,759 | 132,212 | 0.6–1024.2 | 304.9 | 99.52 % (302.0) | 0.160 |
| 05 | 48,494 | 65,632 | 0.6–1024.2 | 168.7 | 98.28 % (168.0) | 0.188 |
| 06 | 163 | 302 | 454–598 (partial) | 35.7 | 100 % (36.67) | 0.098 |

- Heightmap agreement uses the verified frame (extent 1025, normalized height x H, x along columns, z along rows). Vertex y vs heightmap: median 0.1 to 0.2 units, so the navmesh frame equals the terrain frame (no offset).
- Query API check (`height_at` on a 7-unit grid inside the faces): 87 % to 97.5 % of walkable points within 2 m of the heightmap (areas 00-05); the rest are on steep slopes where one large polygon cuts across the heightmap.
- Preview `work/cache/navmesh/ha_area00_preview.png` (grey heightmap, green primary navmesh, orange second mesh), inspected: land is walkable, sea (height 0) is not, cliff bands and the village buildings are holes. Second mesh: a road strip and a rectangular coastal patch.

## hkaiNavMesh (not proven)
- Edge flags are the single value 4 on all 1,255,402 edges; face data is 0 on the primary meshes and 3 on every face of the second mesh; edge data is always 0. Their meaning is unknown.
- The second mesh in each main pack follows the ground only loosely (45 % to 95 % of vertices within 2 m); its role (for example another agent type or surface class) is not established.
- Sub-area packs (`ha_areaNN_sXX*`): 55 packs; most hold tiny meshes (4 to 17 faces, 20 of them identical) in what looks like an object-local frame; larger ones (`ha_area00_s02` 14,419 faces, `ha_area00_s01`) do not match the main area's heightmap in the same frame (0 of 707 and 527 of 18,854 vertices within 2 m). Their placement is unknown. `ha_area11` has extent 2047 (2048-wide map) and no heightmap was compared.
- Edge winding, clusters and the graph (`hkaiDirectedGraphExplicitCost`) and the `hkcdStaticAabbTree` are not used.
- Metres as unit: inferred.

## hknpMeshShape collision (proven geometry, unproven semantics)
- 712 of 712 shapes (655 in `collision.gfpak`, 57 in `resident_bg_parts_model.gfpak`) are `hknpMeshShape`; no other shape class occurs. 49 shapes have no geometry sections.
- Geometry: sections with 16-bit quantized vertices and primitives of four 8-bit ids; `c == d` means a triangle, else a quad ring `a-b-c-d`. Position = `(sectionOffset + q) * bitScale16Inv` (1/512 in every file read). Totals: 4,782 sections, 177,686 vertices, 232,681 triangles (102,165 quads split in two).
- Verified on entry 0: 8+4 vertices form a 4.79 x 2.72 x 2.60 box with 5 faces whose quad rings follow the box edges exactly; all primitive ids are in range in every file.
- Material names from the palette: 229 distinct, for example `attribute_rock` (151), `attribute_stone`, `attribute_wood`, `attribute_water`, `attribute_snow`, `attribute_ice`, `attribute_gravel`, `attribute_soil`, `attribute_sand`, `attribute_grass`, plus object-specific names (`gimmick_tree09`, `plants_tree01_col_01`). The primitive-to-material link (`shapeTagTable`, `shapeTag`) is not decoded.
- Not known: entry names (the pack hashes are not resolved to model names), how a shape is placed in the world, quad diagonal convention (both diagonals are possible; a-c is used), the BVH (`hkcdSimdTree`, `hknpAabb8TreeNode`) which is ignored.

## API
- `pla::assets::navmesh::{NavMesh, AreaNavigation, load_pack_navmeshes}`: `AreaNavigation::load(bin_root, area)`, `is_walkable(x, z)`, `height_at(x, z)`, `nearest(x, z, max_distance)`.
- `pla::assets::collision::{CollisionMesh, load_collision_pack}`: vertices, triangles, material names, `height_at`.
- Tests read the real game files and skip when they are absent.

## Walkable components and safe start (2026-10-12)
Adjacency comes from `NavEdge::opposite_face` (the format stores it), so components are faces joined across shared edges (`game::spawn_point::components`). Numbers from `cargo run -p pla --example spawn_report` (headless):

| Area | Faces | Components | Largest (faces / m2) | OLD start: component, slope | NEW start (x,z), slope |
|---|---|---|---|---|---|
| 00 | 24,177 | 147 | 23,291 / 638,372 | #0, 17.0 deg (above 15) | (445.2, 493.4), 3.4 deg |
| 01 | 47,076 | 304 | 27,990 / 403,454 | #0, 12.1 deg | (496.5, 353.7), 1.8 deg |
| 02 | 57,978 | 449 | 30,161 / 341,806 | #0, 13.4 deg | (492.2, 516.8), 11.0 deg |
| 03 | 32,037 | 248 | 10,720 / 167,121 | #16 (138 faces, 872 m2), 36.3 deg; (512.5,512.5) was off the mesh and was snapped there | (257.6, 617.6), 4.5 deg |
| 04 | 94,759 | 925 | 46,796 / 282,849 | #3 (2,641 faces, 19,872 m2) plateau at y=129.5 | (668.6, 732.0), 7.3 deg |
| 05 | 48,494 | 661 | 14,853 / 262,306 | #0, 5.5 deg | (533.7, 663.5), 15.0 deg |

- Findings: hypothesis (a) confirmed for areas 03 and 04 (start on a small disconnected island / plateau). Areas 01, 02, 05 started in the largest component, so the island trap does not explain them; the remaining suspect is the point-wise collision (axis-only slide cannot follow a diagonal trail), addressed by the angled-slide fallback (`slide_move`: +-30/60/90 deg). Hypothesis (b) does not occur at the old defaults except area 03 (off-mesh fixed point, then snapped); hypothesis (c) is handled by requiring the chosen face to be the highest one at its centre.
- New start rules: largest component by m2, face slope <= 15 deg, five 24-point rings out to 10 m all walkable, in the largest component and within 5 m of the start height, closest to the anchor (area 00: placed `house` objects; others: first spawners of `ha_areaNN`; both snapped to the mesh within 15 m; fallback component centroid).
- Audit for all six areas: start walkable, in component #0, 10 m radial walkable, and BFS over face adjacency reaches 24 of 24 anchor landmarks.
- Not proven: edge flags (a non-traversable edge type would invalidate adjacency), and no viewer run was made (rule 7); verification is numeric plus unit tests.
