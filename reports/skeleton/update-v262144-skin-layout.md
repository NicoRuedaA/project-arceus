# Decode proven rig indices and weights, not full runtime skinning

The demonstrated lane is now implemented: ID 7/code 22 contains four unsigned
8-bit **rig/bind indices**; ID 8/code 39 contains four little-endian UNORM16
weights. `TrMbf::parse_skin_shape` selects an explicit FlatBuffer shape/buffer;
`skin_influences` preserves raw values and resolves active rig indices through
`TrSklNode::rig_index`. Existing heuristic mesh-rendering paths are unchanged.

This is structural serialization evidence, not native function decompilation,
Bevy deformation, animation playback, matrix evaluation or binary matching.

## Reproduce

Use the authorized local-private patched update RomFS; game bytes stay private.
The report records input-relative paths/hashes and aggregate observations only.

```sh
python3 .tools/trm_skin_census.py "$PRIVATE_ROMFS" \
  --output reports/skeleton/update-v262144-skin-census.json
PLA_SKIN_CORPUS="$PRIVATE_ROMFS" cargo test -p pla --test tr_skin
./.tools/ci.sh
python3 .tools/function_progress_treemap.py --build update-v262144
```

The tool also emits `update-v262144-skin-census.tsv`, used by the optional Rust
corpus test. Reconcile JSON hashes before reusing observations with another
extraction. The directory name itself is not evidence of patch identity.

## Independent multi-influence evidence

| Model/shape | Nodes / binds | Vertices | One / two / three / four influences | Raw weight sums: 65534 / 65535 / 65536 |
|---|---:|---:|---|---|
| `item_255`, shape 0 | 20 / 16 | 1159 | 303 / 500 / 233 / 123 | 46 / 951 / 162 |
| `sd9150_mysterygift`, shape 0 | 14 / 10 | 1455 | 831 / 624 / 0 / 0 | 60 / 1281 / 114 |

These are independent vertex-buffer hashes, not two names for the same bytes:

| Input | SHA-256 |
|---|---|
| `item_255.trmdl` | `c12c4e9799da1eb2872af0805f87ae1b95ee4e49b132584b0c929245de5d6890` |
| `item_255.trskl` | `fba33b772cb16871b3eb3ff22e66b3168fd05f724abab58846614ff96fbcce07` |
| `item_255.trmsh` | `81e79c2674b3ddb0678f6db08613b5dbdf5820c3aba758a5908912c8285f8cc0` |
| `item_255.trmbf` | `87e21592499057c3091658356e31650f2551d34ad8243544c69cc7acededfd1f` |
| `sd9150_mysterygift.trmdl` | `c3a27ed96e26084b148aee874bb67d1edc19bbe0e205c2f3dcd058ee8f956d6d` |
| `sd9150_mysterygift.trskl` | `327dd5a826a82356a6f77fa08859780d4d3afbb068ab8c4c3524cd502061bc54` |
| `sd9150_mysterygift.trmsh` | `75d1f11393e35f320812a79275a512277da59e8c2211429f4debe0cb6a0d4094` |
| `sd9150_mysterygift.trmbf` | `866485e9014b557536d9c22fccd16780a799368697f7a19fc0ea1ddfe3485659` |

Item 255 has 528 positive-weight lanes with joint index zero, proving that zero
is not a universal sentinel. Its active indices span 0..15; mysterygift shape 0
uses 1..9. Both sets stay below bind count, but that bound alone would not prove
the namespace because both also fit node count. The explicit reference-reader
mapping described below corroborates the **rig** interpretation. Mysterygift's
second shape has 142 single-influence vertices actively using rig zero.

## Corpus scope and limits

The independent model-linked traversal surveys **253 descriptors**: 175 link to
369 shapes, of which 177 declare the demonstrated skin lane and 192 have no skin
channels. There are **78 unresolved model links**, explicitly retained with
bounded reasons. It does not claim every archive-embedded game model is included.

The 177 skin shapes contain 128303 vertex records counted per model reference,
including repeated buffers. In particular item 255–259 share identical vertex
buffer bytes; they are NOT five independent corroborations. All 177 have stride
48, no positive-weight index outside bind count, and active index sets exactly
matching the shape's explicit mesh `RigIndex` metadata. Across these references,
raw sums are 65534 (290 vertices), 65535 (127089) and 65536 (924).

One observed UNORM16 quantum, `1/65535`, is the accepted sum error. Components
are decoded by division by 65535 without secretly rescaling their sum to one.
Zero weight disables a lane; the observed inactive index is zero. No special
meaning is assigned to 255 or another unused index. A synthetic inactive-255
test only exercises safe handling and is not an observed sentinel claim.

## Primary corroboration

Public sources are pinned, not interpreted from a moving branch:

- [pkNX Arceus Mesh schema](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/Arceus/Schemas/Poke/Model/Mesh.fbs), SHA-256 `4783ffed715770d45223a68b75bc9dd8606ac24b98320bf70a4d1b5d0687bd73`.
- [pkNX MeshBuffer schema](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/Arceus/Schemas/Poke/Model/MeshBuffer.fbs), SHA-256 `7f102504aec3fa8647c17910e2d941ca8533bfea1af9a8676727a195159cd0f0`.
- [pkNX Model schema](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/Arceus/Schemas/Poke/Model/Model.fbs), SHA-256 `26dc8f37186af2735c79f30024851f2d7015ba6a0c3002e5a142b2b2d65a1e9d`.
- [Reference importer](https://github.com/ChicoEevee/Pokemon-Switch-Model-Importer-Blender/blob/b0c98d9fcaab85a04ad35e2d111bae4cad6c1e04/PokemonSwitch.py), SHA-256 `6a1766ab1f29d0a1b055e162d025a6a25ed1bb0963fd69751a9b235785cc9ede`.

The Mesh schema corroborates semantic IDs 7/8, format codes 22/39 and per-shape
rig metadata. MeshBuffer/Model describe the vectors and asset references used by
the census. Importer lines 1493–1506 and 1744–1766 decode these four-byte/eight-byte
channels; lines 282–286, 328–329 and 2016–2022 explicitly map vertex IDs through
node RigIdx to bind records/names, while 1976–1987 retain only positive weights.
This is independent source-reader corroboration, NOT a native runtime trace.

## Supported boundary and checks

The explicit constructor selects a shape index and matching buffer index, requires
one vertex layout/stream and one UINT16 triangle index buffer per shape, validates
known format widths and exact nonoverlapping stride coverage, and uses declared
vector lengths instead of scanning to the file tail. The accessor accepts only
codes 22/39, declared vertex origins, sums 65534..65536 and uniquely mapped active
rigs below bind count. Unsupported layouts, sums, origins and rigs fail loudly.

Input files are capped at 32 MiB; shape count is capped at 64 and vertex count at
one million; shared checked FlatBuffer primitives enforce table/range bounds.
Unknown formats, extra streams, rig offsets/attachments, parent-matrix evaluation,
Euler/pivot conventions, animation tracks and actual skinning remain outside scope.

A first optional test exposed the existing mysterygift multi-buffer heuristic:
legacy paired parsing gives stride 12 instead of 48. Direct follow-up verified
that item 255's legacy parsing succeeds (48/1159) and its tail padding is zero;
the initial inference blaming item 255's index padding was WRONG. The new selected
shape path avoids both fixed origins and index-tail scans without replacing the
legacy renderer/parser.

Checks: six focused tests cover synthetic one/four influences, active joint zero,
inactive out-of-range indices, raw quantization, malformed/missing layouts and
unresolved rigs. Explicit private-corpus execution checks two independent
multi-influence models and all 177 listed shapes/128303 vertex records against
the independent census. Clean-clone optional test skips are not corpus evidence.
After source normalization, focused tests passed **6/6 (EXIT 0)**, including
explicit private-corpus execution. Final `./.tools/ci.sh`: **EXIT 0**, 79 tests
passed, 2 ignored; sheet preflight 0 errors/0 warnings. Metadata census EXIT 0;
Python compilation and `git diff --check` passed. Function-progress regeneration
is checked independently against exact build/archive and current Rust hashes.
Runtime harness: N/A, this unit
only reads serialized data. No native function address is mapped; the manual
function evidence ledger receives no state promotion.

## Rollback and next step

Rollback removes only dec096's selected skin-shape constructor/accessors/tests,
model-linked census/reports and sheet/PLAN projections; retain the dec095 skeleton
hierarchy, previous mesh/material code and Suyu assessment. Regenerate the
function-progress report after rollback because Rust fingerprints change.

Next bounded unit: establish bind/local/global matrix evaluation with independent
evidence before Bevy deformation, or resolve the documented unlinked model cases.
