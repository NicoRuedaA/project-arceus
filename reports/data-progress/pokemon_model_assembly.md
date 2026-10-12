# Pokémon model-pack assembly investigation

## Scope and result

This report records a bounded inspection of
`bin/archive/pokemon/pm0025_00_00.gfpak` (Pikachu, form 00, variation 00) from
the local Pokémon Legends: Arceus v262144 RomFS. The pack is 180 entries and
every Oodle-framed entry decoded to its declared size. The metadata probe kept
at most 16 mesh candidates / 512 MiB and only ran against this one pack.

The pack's base and two LOD mesh-buffer/descriptor associations are still
ambiguous. The descriptor payloads name expected `.trmbf` files, but those
filenames were not found in the candidate mesh-buffer payloads; all nine
cross-pairs pass the current parser. Sixteen BNTX entries contain one
internally named texture each. A complete Bevy render is **not yet proven**: the candidate
skeleton entry has 14 roots but no bind records or rig mappings, and material
parameter-to-texture semantics are not parsed. `pokemon_spike` is currently a
bounded metadata probe, not a model renderer.

## Observed

| Evidence | Observation |
| --- | --- |
| Pack table | 180 entries; all 180 decoded; no bounds failures or decode failures. |
| Entry 153 strings | Contains `pm0025_00_00.trmtr`, `pm0025_00_00.trskl`, and base/LOD1/LOD2 `.trmsh` names. This is a model-level reference candidate; the exact container schema is not yet parsed. |
| Entry 155 strings | Names `pm0025_00_00.trmbf`, five shape names, and `body_a`, `body_b`, and `body_c` material labels. |
| Entries 152, 174, and 176 | All three pass `TrMbf::parse`; each has five parser-compatible shapes with each of the three candidate descriptors. The mesh-buffer payloads have no observed embedded filenames to identify their LOD. |
| Entries 155, 175, and 177 | These descriptor candidates contain strings naming the base, LOD1, and LOD2 `.trmbf` assets respectively. Their archive indices are known, but the association from those names to the three anonymous mesh-buffer entries is not decoded. |
| Candidate cross-product | Trying every `TrMbf` candidate against every descriptor yields nine parser-compatible pairs. This is a parser compatibility result, not proof of the corresponding mesh-buffer/descriptor identity. |
| Entries 158–173 | Sixteen BNTX containers, each with one texture. Names group as `body_a`, `body_b`, `body_c`, and `eye`, with suffixes including `_alb`, `_ao`, `_lym`, `_nrm`, and `_msk`. |
| Entry 156 strings | Contains material-like parameter names such as `BaseColorLayer*`, `EmissionColorLayer*`, `UVScaleOffset*`, `Opaque`, and `CastShadow`. No parser currently establishes how these fields select BNTX entries or material layers. |
| Entry 151 strings | Contains `origin`, effect/attachment names, and eye-joint names. After locator strings were made representable, skeleton parsing still rejects it with `roots=14`, `bind_count=0`, and `unmapped_rigs=0`. |

## Pairing conclusions

- **Observed:** descriptor candidates contain distinct expected `.trmbf`
  filenames, while the candidate mesh-buffer payloads do not expose those
  filenames. Nine descriptor/buffer pairs pass the parser, so exact pairings
  remain unknown.
- **Inferred:** the pack's entry ordering may correspond to base/LOD1/LOD2
  groups, but adjacency has not been validated against container metadata or
  reference output and must not be treated as an established pairing.
- **Inferred:** entry 153 is the model-level descriptor because it references
  the `.trmtr`, `.trskl`, and three `.trmsh` assets by name. The string evidence
  supports this role, but the FlatBuffer table structure and archive-entry
  index linkage have not yet been decoded into typed records.
- **Observed:** BNTX texture names carry body/eye grouping and suffix labels.
- **Unknown:** the exact meanings and shader bindings for `lym`, `msk`, and the
  `trmtr` parameter fields. Do not equate suffixes to renderer material inputs
  without parsing the material descriptor or matching a reference render.
- **Unresolved:** no valid bone-to-rig or inverse-bind mapping was found for
  this pack. The entry-151 error is not evidence that the file has no skeleton;
  it means the currently supported hierarchy/bind contract does not cover it.

## Out-of-bounds archive entries

The separate archive census identified one out-of-bounds final row in each of
`pm1006_11_41.gfpak` and `pm1006_12_41.gfpak`. Both are row 174 of 175, type 9,
kind 3, flags 204, with a 78-byte payload declaration and the same observed
`CC 06 20 00 00 00 1C 00 2C 00 07 00 08 00 0C 00` prefix. The first points at
offset 1,572,848 with 64 bytes remaining (14-byte overrun); the second points
at 1,635,984 with 32 bytes remaining (46-byte overrun). Both declare decoded
size 76. The other rows in each pack are in bounds. This confirms the payload
range extends past EOF, but does not distinguish a truncated source archive
from an alternate entry-size convention; the final frame could not be decoded
through the current bounds-checked parser. Do not treat these as validly
decoded entries.

## Reproduction

The following probes were run only on the named pack, inside the required
memory-limited scope, with build output directed to the worktree target dir:

```sh
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- \
  env CARGO_TARGET_DIR="$PWD/target" CARGO_BUILD_JOBS=3 \
  cargo run --offline -q -p pla --example pokemon_spike -- \
  /home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin/archive/pokemon/pm0025_00_00.gfpak
```

The probe writes no decoded assets. Its bounded log is
`work/progress/model-pm0025-spike.log` (ignored by Git).

## Related animation spike

The Mysterygift clip is
`sd9150_misterygift_anim.tranm` (481 frames at 60 fps, ten name-mapped tracks).
Its first translation key on `sd9150_mysterygift_joint1` is
`[-1.2342589, 0.14076617, 3.3655112]`; the final key is `[0, 0, 0]`. The
`anim_spike` example now computes per-frame skinned bounds with a 100,000-vertex
and 50,000,000 frame-vertex-evaluation cap and reframes its camera. Under the
6 GiB / zero-swap scope, frames 50, 100, and 200 were opened and show the
animated, untextured box. Frames 0–40 remain clear-color; the exact cause is
not yet established. The colorless appearance is explained by the example's
constant `StandardMaterial`; its loose-file BNTX assets are not loaded there.
This run used the host display (`DISPLAY=:0`), so headless rendering has not
been verified.
