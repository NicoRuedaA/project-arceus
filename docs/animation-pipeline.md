# TR skeleton and animation pipeline status

## Implemented data path

`crates/pla/src/assets/tr/bevy.rs` adapts already-decoded TR values into Bevy:

1. `BevyTrSkeleton::from_tr` converts local SRTs, parent indices, rig ordering,
   and bind matrices. It rejects nonzero pivots and rejects bind matrices unless
   both matrix multiplication orders validate against the existing Blender
   XYZ reference rest pose. The serialized bind-record order is the Bevy joint
   order; influences are not confused with transform-node indices.
2. `insert_skin_attributes` uses the validated ID 7 (four rig-index lanes) and
   ID 8 (four UNORM16 weight lanes) parser and writes Bevy's Uint16x4 joint
   indices and Float32x4 weights.
3. `bevy_animation_clip` makes Bevy translation, rotation, and scale keyframe
   curves using the parsed frame rate. Duplicate frame entries retain the first
   value, matching `TrAnmChannel::reference_records`; rotations use the pinned
   importer's packed-quaternion decoder. Bevy's interpolation between authored
   records is an explicit port policy, not evidence of the game's interpolation.
4. `anim_spike` is a strict windowed prototype. It renders one supplied
   `.trskl`/`.trmbf`/`.trmsh`/`.tranm` set and emits one PNG per clip frame under
   `work/anim-sequence/` by default. It refuses heuristic mesh positions or
   unvalidated rest poses.

The adapter and example compile across `pla` targets. The example has not yet
been run against readable game assets, so no rendered screenshot sequence is
claimed. `crates/sheetty/src/emit.rs` was also fixed because integer-shaped
sheet values declared as `f64` had been emitted as integer Rust literals,
preventing the workspace from compiling.

## Reference and limits

The source references are `.tools/tranm_importer_reference.py` and
`.tools/trskl_matrix_reference.py`. The animation reference tool reconstructs
the pinned importer constructors and `get_track_transforms` values; it does not
model scene application or game playback. The matrix reference is Blender's
`T * Rz * Ry * Rx * S` reference pose with zero pivots. Existing parser tests
cover record preservation and bind residuals; this task adds Bevy-clip sampling
checks at authored translation/rotation key frames. The opt-in `tr_animation_reference` test compares Bevy curve evaluation at each
frame against the pinned importer oracle when `PLA_ANIMATION_CORPUS` and
`PLA_ANIMATION_REFERENCE` are configured. Those private oracle inputs are not
configured in this environment, so the real-data Bevy/importer comparison was
skipped; the available tests cover synthetic curve sampling and the source
provides the gated real-asset comparison. Bevy interpolation between authored
keys is not equated with game playback behavior.

The readable sample at
`bin/demo/graphic/sd/sd9150/sd9150_mysterygift/` has a `.trskl`, `.trmbf`,
`.trmdl`, `.trmsh`, `.trmtr`, four BNTX textures, a 10,968-byte `.tranm`, and
`.trac*` siblings `.tracl`, `.tracn`, `.tracp`, `.tracr`, and `.tracs`. Its
animation is reported by the census as 481 keyframes. The sidecars are
co-located, but their exact state-machine or clip-selection semantics are not
established. The ROMFS scan found 35 `.tracm` files; no demonstrated mapping
from those files to idle/walk states has been established. The requested
speed-driven idle/walk selector is therefore intentionally not inferred or
implemented.

These are observed loose filenames, not proof that identically named packed
records can be recovered from a `.gfpak`.

## Pack observations

The repository's `GfPak` parser reads `GFLXPACK`, entry count, numeric kind, a
sequence of table offsets, and 24-byte entry records (`type_id`, entry kind,
checksum, size, flags, data offset, reserved). It currently requires the first
payload offset to equal `table_offset + count * 24`; that means it rejects the
Pokémon sample below, whose first payload begins eight bytes after that
boundary. The offsets and some record fields were separately inspected for the
sample, but the rejected pack is not evidence that the parser supports its
layout. The records expose no original loose filenames or extension mapping.

| Sample | Observed pack metadata | Payload prefixes observed | What remains unknown |
|---|---|---|---|
| `bin/archive/pokemon/pm0025_00_00.gfpak` | Header kind 4; table offset `0x1168`; 180 records; first payload offset `0x2250` (table ends at `0x2248`, +8); parser rejects this offset relation | Separately inspected payload prefixes: 177 start `8c 06`; 3 start `cc 06`; some wrapped payloads expose `BNTX` at offset 8 | The parser source documents meanings for kinds 1 and 3, not kind 4; records cannot be assigned to `.trskl`, `.trmbf`, or a clip/state file from metadata alone. |
| `bin/archive/chara/data/tr/tr0002_00_doctor_b.gfpak` | 163 records; kind 1; each record type ID 9 and entry kind 3 | 162 payloads start `8c 06`; 1 starts `cc 06` | The record-to-asset type mapping and the significance of the two prefixes are not established here. |

The project inventory describes Pokémon-style packs as multi-table and the
`.trmbf`/`.trskl` payloads in them as Oodle-compressed; that does not establish
the exact record-to-type mapping for this sample. The prefix observations above
are bytes, not a verified codec identification. Therefore packed file types
remain unknown except for directly visible inner markers such as `BNTX`; no
`.tracm` or `.tranm` state selection can be attributed to these records.

## Reproduction

From a windowed desktop session with readable model files:

```sh
cargo run -p pla --example anim_spike -- \
  /path/to/model.trskl /path/to/model.trmbf /path/to/model.trmsh \
  /path/to/clip.tranm work/anim-sequence
```

The windowed command completed on the readable mysterygift assets and emitted
481 PNG files (`frame-0000.png` through `frame-0480.png`) under `work/`. Their
PNG structure and image payloads are valid. Visual fidelity is not established:
the inspected early frames are black, and a later frame renders a planar,
unrecognizable shape rather than a verified character model. Game archives,
source assets, screenshots, and private reference traces remain outside
version control. The required function-progress report regeneration
failed with an opaque wrapped exception and correctly replaced its outputs with
stale placeholders. Its exact cause remains unknown; the stale artifacts are not
valid coverage evidence.
