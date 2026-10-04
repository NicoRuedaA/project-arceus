# Evaluate a checked reference pose; preserve authored binds

`TrSkl::read_bind_records` now reads bounded FlatBuffer bind records and their
four packed vectors. The explicitly named `reference_matrix`/`reference_pose`
APIs evaluate the demonstrated **zero-pivot reference lane**. They do not claim
native evaluation, animation, deformation, coordinate conversion or binary parity.
`bind_rest_residuals` reports both identity-product errors;
`validate_bind_rest_pose` rejects disagreement without rewriting authored data.

## Evidence and convention

The reference uses radians, column vectors and **T · Rz · Ry · Rx · S** (Blender
XYZ Euler), with `global = parent_global · local`. Serialized X/Y/Z/W vectors
are expanded as affine columns; the Rust API exposes explicit row-major notation.
Node-parent indices remain in transform-node space; bind records are selected
through `rig_index`, not node index. No world-axis conversion is performed.

The independent installed Blender **5.2.1 LTS**, build `9e2066aef7ef`, dated
2026-09-01, executed `mathutils.Euler`/`Matrix.LocRotScale` directly. The pinned
[importer](https://github.com/ChicoEevee/Pokemon-Switch-Model-Importer-Blender/blob/b0c98d9fcaab85a04ad35e2d111bae4cad6c1e04/PokemonSwitch.py#L289-L327)
uses these operations and parent-left multiplication. SHA-256:
`6a1766ab1f29d0a1b055e162d025a6a25ed1bb0963fd69751a9b235785cc9ede`.
Its routine is an independent cross-game format reference, NOT a native PLA
implementation. We did not execute its Blender armature importer: edit-bone
scale/inheritance behavior is outside this plain-matrix differential.

Pinned pkNX commit `d191cd0e5c05f2af81d9a41c1f1d82e6621b351a` corroborates storage:

| Primary schema | SHA-256 |
|---|---|
| [Arceus Skeleton](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/Arceus/Schemas/Poke/Model/Skeleton.fbs) | `030e780c999c26382301c68b8c8e455febb2d7a21f8873cd48e121dfce723128` |
| [Transform](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/pkNX.Structures.FlatBuffers/Schemas/Math/Transform.fbs) | `7c4dc049b5b73d674d27b4156b230f1345a1454cf1cae6f4843c570e52ec7a5c` |
| [Matrix4x3f](https://github.com/kwsch/pkNX/blob/d191cd0e5c05f2af81d9a41c1f1d82e6621b351a/FlatBuffers/pkNX.Structures.FlatBuffers/Schemas/Math/Matrix4x3f.fbs) | `c0830e4bc8c52c63d672dad3037d54ba06193255ea4e84fb69c972bc319a1b9e` |

## Corpus agreement and the counterexample

The metadata report covers 199 exact hashed skeletons, 1264 transform nodes and
289 bind records. There are **301 multi-axis rotation nodes**, so Euler order is
not chosen from single-axis rotations alone. Twenty-four combinations compare
six Euler orders, column/transposed-basis storage and global/inverse-global
direction. XYZ/columns/inverse-global agrees on 288/289 binds at absolute 1e-4;
next-best XZY agrees on 220/289 and has maximum coefficient error 1.98054.
The agreeing files' largest identity-product residual is 1.19210e-6.

| Independent input | Nodes / binds | Multi-axis nodes | SHA-256 |
|---|---:|---:|---|
| `item_255.trskl` | 20 / 16 | 14 | `fba33b772cb16871b3eb3ff22e66b3168fd05f724abab58846614ff96fbcce07` |
| `item_267.trskl` | 39 / 35 | 33 | `a7f339701470bf33d6041b0dbc908839f3037e12e8785a4601e0460b1cfb115d` |
| `item_230.trskl` | 5 / 1 | 1 | `3596a17b907d888d82c4104adfe177d9536eb34a6d10ed4b40f945e4ee3641e6` |

**Do not infer scaled native/bind composition from that agreement.** The 98
nonidentity-scale nodes occur in nine files, but only item_230 has binds among
those files. Its rig 0 maps to node 3; both identity-product errors are **0.5**,
and inverse-global coefficient difference is **0.3333333135**. The other 97
scaled nodes have no bind records. This unit preserves item_230's authored bind
and explicitly rejects its rest-consistency check. Scale exclusion, authored
rest offsets and animation/flag effects are unresolved possibilities, not fixes.

All 1264 observed pivot pairs are exactly zero. Nonzero or non-finite pivots fail
loudly; no pivot composition is invented. Bind flags are both 1 in all 289
records; their native meanings remain unknown and other flags are rejected.
Root flag 0/1 is retained without invented evaluation behavior. Existing parser
limits and rejection of IK, attachments, rig offsets and unobserved node types
remain. Reference evaluation also checks one root, preceding parents, finite
SRT/results, rig bijection and finite positive residual tolerance.

## Reproduce and proof scope

```sh
blender --background --factory-startup --disable-autoexec \
  --python .tools/trskl_matrix_reference.py -- "$PRIVATE_ROMFS" \
  --output reports/skeleton/update-v262144-matrix-reference.json \
  --private-reference "$PRIVATE_ORACLE_OUTSIDE_REPO"
PLA_TRSKL_CORPUS="$PRIVATE_ROMFS" PLA_MATRIX_REFERENCE="$PRIVATE_ORACLE_OUTSIDE_REPO" \
  cargo test -p pla --test tr_matrices -- --nocapture
```

The tool emits JSON/TSV metadata only. Oracle matrices are game-derived and
written only to the explicit private directory outside the repository. Public
rows contain input paths/hashes, counts, convention scores and residual errors,
never source matrices/bytes. All 199 input hashes were reconciled. Archive/build
identity is inherited from the verified update-v262144 extraction evidence;
a directory name alone is not patch identity.

Five focused tests exercise author-created FlatBuffers, independent generic
axis-matrix products, noncommuting parent order, nonuniform scale, malformed
pointers/flags/matrix vectors, truncations, pivots, mapping, tolerance and
non-finite/overflow rejection. Opted-in tests compare three author-created
multi-axis/scaled transforms and all 1264 corpus nodes against Blender output.
All 289 authored binds are read; 198 files pass consistency, item_230 fails as
expected (75 passing files have no binds). Maximum corpus Rust-vs-Blender local
and global coefficient differences are **4.42667e-8 / 2.38419e-7**, below 2e-5.
Clean-clone optional skips do not constitute corpus proof.

After source normalization, explicit private-corpus focused suites passed
**15/15, EXIT 0** (matrix 5, hierarchy 4, skin 6). Three synthetic Blender
reference nodes have maximum local/global errors 8.89308e-8 / 1.26429e-7.
Initial CI stopped at Clippy's indexed-column iteration warning (**EXIT 101**);
the equivalent checked iterator was normalized and focused suites rerun.
Final `./.tools/ci.sh`: **EXIT 0**, 84 passed, 2 ignored, preflight 0 errors/0
warnings. Its unconfigured optional oracle/corpus tests skip by design; the
separate explicit run above supplies corpus evidence. Python syntax, metadata
hashes and `git diff --check` are checked separately. Final function-progress
regeneration is independently reconciled against build/archive and current Rust
fingerprint. A computational Blender
reference runtime was exercised; **no game/native runtime boundary was exercised**.
No direct native function address is mapped. The function evidence ledger remains
unchanged: no analysis, implementation, behavior or binary-match promotion.

## Rollback and next bounded task

Rollback only dec097's matrix module/re-export/tests, reference tool/reports and
sheet/PLAN projection; preserve dec094–096. Regenerate function-progress reports
when source fingerprints change. Historical dec095/096 findings remain; this
unit advances reference math but does not complete Priority 2 or the full goal.

Next: bounded native evidence for item_230's scaled bind/flags and rest-pose
semantics, or a targeted corpus extension with independently evidenced scaled
binds/nonzero pivots. Animation tracks and Bevy deformation remain separate.
