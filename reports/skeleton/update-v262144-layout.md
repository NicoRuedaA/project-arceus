# Read skeleton transforms and parents without inventing matrix semantics

`TrSkl` now resolves bounded FlatBuffer transform-node tables, local scale/rotate/
translate (SRT), pivots, parent indices and rig indices. The old printable-string
scan and absolute-offset interpretation are replaced. This is structural asset
parsing, **not native-function decompilation, animation parity or skinning**.

## Reproduce

Use the authorized local-private patched update RomFS; never copy its contents into
reports or tracked fixtures. Commands below only publish metadata or test results:

```sh
python3 .tools/trskl_census.py "$PRIVATE_ROMFS" \
  --output reports/skeleton/update-v262144-census.json
PLA_TRSKL_CORPUS="$PRIVATE_ROMFS" cargo test -p pla --test trskl
./.tools/ci.sh
python3 .tools/function_progress_treemap.py --build update-v262144
```

The census records all relative input paths, sizes and SHA-256 hashes. Reconcile
those hashes before reusing its result with another extraction; the path alone
does not prove patch identity. Scope is the existing authorized patched RomFS
(`pk2`, update v262144), not an independent proof of the extractor's correctness.

## Evidence and corrections

| Reference | Bytes | SHA-256 | Nodes | Bind records |
|---|---:|---|---:|---:|
| `item_228.trskl` | 2780 | `daa83b96cef7b0f1dee3b2c1e94a254dee851dff5754ad370a48f87b6868236e` | 16 | 12 |
| `item_001.trskl` | 420 | `9154ec7ed197079e893aaf5e1e9c6b4f58ffae20263bab3afea5b12bf3984072` | 3 | 0 |

An independent Python traversal and Rust parser both accept **199 files, 1264
transform nodes and 289 bind records**, with 75 zero-bind skeletons. Every file has
one root, parents precede children, all local SRT/pivot components are finite, and
nonnegative rig indices cover its bind vector once each. These checks corroborate
the serialization, not the game's evaluation of transforms.

Corrects the `.trskl` portion of dec043/dec047 and the old PLAN assumption:

- First `u32` is a root-table offset, not a header-size declaration.
- Vectors declare counts; each entry points relative to its own field address.
  The `item_228` vector near `0x28` describes binds, not local node transforms.
- Signed shared-vtable offsets can point forward; no fixed 68-byte record stride
  or constant bone-table address is a valid parser contract.
- Root field 0 is 1 in 13 references, contradicting the upstream schema's
  always-default comment. It is exposed as `root_flag`, with unknown semantics.
- Parent/rig field omission means no index in this corpus. The public schema's
  comments mention -1 but do not declare that default; generic generated
  default-zero behavior would create invalid roots and spurious rig references.

## Primary references

Public pkNX schemas are pinned to commit
`d191cd0e5c05f2af81d9a41c1f1d82e6621b351a` (resolved with `git ls-remote`, then read
from a shallow no-checkout reference clone outside the repository):

| Schema | SHA-256 |
|---|---|
| [Arceus Skeleton](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/Arceus/Schemas/Poke/Model/Skeleton.fbs) | `030e780c999c26382301c68b8c8e455febb2d7a21f8873cd48e121dfce723128` |
| [Shared Transform](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/pkNX.Structures.FlatBuffers/Schemas/Math/Transform.fbs) | `7c4dc049b5b73d674d27b4156b230f1345a1454cf1cae6f4843c570e52ec7a5c` |
| [Shared PackedVec3f](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/pkNX.Structures.FlatBuffers/Schemas/Math/PackedVec3f.fbs) | `56401783a5c40843ed8180929c9297ac5e2d2adb163ab6951ab188835184ff42` |
| [Shared Matrix4x3f](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/pkNX.Structures.FlatBuffers/Schemas/Math/Matrix4x3f.fbs) | `c0830e4bc8c52c63d672dad3037d54ba06193255ea4e84fb69c972bc319a1b9e` |

The schemas corroborate field roles and distinguish transform nodes from bind
records. They do not prove engine matrix evaluation. The
[FlatBuffers encoding contract](https://flatbuffers.dev/internals/) corroborates
relative references, signed vtables, inline structures and length-prefixed names.

## Supported boundary and remaining work

The parser caps files at 32 MiB, vectors at 8192 entries and names at 1024 bytes,
checks ranges and
field extents/alignment, rejects invalid UTF-8/termination and non-finite values,
and rejects forward/self parents, duplicate/out-of-range rig indices and multiple
roots. Zero binds are valid. Nonzero rig offsets, IK, locator attachments,
nonzero node types and root flags outside the observed 0/1 set remain unsupported.
Bind addresses are structurally checked, but matrix contents are not decoded.

Local matrix construction, Euler order, pivot composition, coordinate conversion,
bind-matrix direction/storage, multi-influence skinning and actual `.tranm` tracks
still need direct reference evidence. No runtime harness was run: N/A for this
read-only serialization unit. `TrAnm` remains a heuristic float-triple scanner.
No native function address is mapped to this parser, so
`sheets/re/function_progress_evidence.tsv` receives **no state promotion**.

## Checks and rollback boundary

The focused suite covers synthetic relative offsets/forward shared vtables,
nonidentity SRT, zero-bind skeletons, omitted root/rig fields, invalid ranges,
names, parents/rigs/transforms, every truncation, and a deterministic byte-mutation
sweep. Its optional corpus test requires the exact 199-file input and is explicitly
skipped without `PLA_TRSKL_CORPUS`; a clean-clone pass is not a corpus pass.

Results: final focused suite passed 4/4 including the explicitly supplied corpus
(exit 0); metadata oracle passed (exit 0). After correcting initial clippy style
errors (exit 101), final `./.tools/ci.sh` passed (exit 0): 73 tests passed, 2
ignored; sheet preflight 0 errors/0 warnings. This does not count skipped optional
fixture branches as executed corpus evidence. Function-progress regeneration is
verified separately against the current Rust fingerprint. Rollback removes only
the dec095 skeleton parser/test/census/report and its sheet/PLAN projections;
prior material/mesh parsing and Suyu readiness work are independent. Regenerate
function-progress reports after any rollback because the Rust fingerprint changes.

Next unit: establish Euler/pivot and bind-matrix evaluation using direct native or
independent reference evaluation, or select multi-influence models from the census
without pretending serialized SRT is already a matrix.
