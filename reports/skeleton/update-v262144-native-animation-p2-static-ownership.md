# P2 offline static ownership audit

Report ID: `dec108`.

**Result: partial static evidence only.** The update-packaged SDK contains the
sole static definition candidate for `FloatQuaternionEpsilon`; main's two direct
calls to the FPCR helper pass `1`, selecting FZ-enabled behavior. Canonical
inventory bounds do not assign either call site to an inventoried function, and
the inspected code does not establish a precise path from either caller to the
animation worker/evaluator. Loaded binding and worker FPCR remain unknown.

## Scope and identity

Offline inspection only; no emulator, game startup, credentials, or installation
was used. The exact archive and module identities were rechecked against the
existing dec106 package qualification. Main and SDK identities match the
update's packaged entries; the other packaged modules are recorded by package
hash, but their binaries were not available locally for independent disassembly.
No bytes, assembly, pseudocode, runtime values, or game content are included.

| Input | Identity |
|---|---|
| Build | `update-v262144` |
| Archive `pk2.nsz` SHA-256 | `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446` |
| Packaged section 22 SHA-256 (dec106 qualification) | `c5e6c573856bb085ffffe36cdbecc9ab18fef0a2e3aeecf7f724ee574217521a` |
| Main NSO SHA-256 | `89fa2d715e13a1c311a47ee02b3cc858ccf72a2086ab9704a08a2ddc1b90f7d9` |
| Main ELF SHA-256 | `b772390207e0de689ca96adaeb15df8b1499912efa74804c3766214d76d08cf2` |
| SDK ELF SHA-256 (exact packaged match) | `a1db3d3908bdc58b0a53fdad9f4c0b60bfe96b1de179a1b753d46d7c19249f54` |
| Canonical function inventory SHA-256 | `8cefd2b2b7ce412653271ceec66be1325c0fcb0bc15b1d34bf8b3cdbfd5b7f8c` |
| Prior static/package metadata | `module-qualification.json` SHA-256 `f3de68f84c92a9f6c38ceb2acf92b23c77e63b8f7a234462f85a74c9b10c2df4`; `static-context.json` SHA-256 `511e6172d0d24f7b8cf80a1d9eadc9bb582c9a17f89745fa557ce4f54c765cde` |

## Owner map and direct evidence

| Address | Canonical owner / static fact | Boundary |
|---|---|---|
| `0375e118` | Main `R_AARCH64_GLOB_DAT` relocation for undefined dynamic symbol index 54, `_ZN2nn4util6detail22FloatQuaternionEpsilonE`, into GOT `0427a118`. | Identifies an import slot, not its relocated runtime value. |
| `00ab3aa4` | SDK dynamic symbol index 18718, size 4, is the only packaged definition candidate in the qualified five-module manifest; its static f32 is approximately `1e-5`. | Not evidence the loader selected/bound this provider at runtime. |
| `031c8e50` | Canonical inventoried function, 32 bytes. Direct instructions read FPCR, preserve unrelated bits, set/clear FZ from argument bit 0, return zero. | Static helper contract only. |
| `031ba3b8` | Direct `BL` to helper. The immediately preceding instruction sets `w0 = 1`, so this call requests FZ enabled. The call lies in the inventory gap `[031ba328, 031ba3d4)`; no canonical owner row contains it. | No owner inferred from the apparent local routine start or nearest address. Following code invokes an indirect callback; target/thread role is unresolved. |
| `0323d27c` | Direct `BL` to helper. Earlier in the block `w0 = 1`, so this call also requests FZ enabled. The PC lies in inventory gap `[0323d1bc, 0323d2c0)`; no canonical owner row contains it. | No owner inferred. Local control flow continues through object/link setup and an indirect virtual call, not a direct edge to the known animation evaluation entries. |
| `027cf4b4` | Canonical 532-byte function, previously analyzed: typed bone-track SRT dispatch; rotation output at `+0x10`, mask bit `0x02`, fixed reuse/cache behavior. | No direct call edge from either FPCR caller was established. |
| `027b572c` | Canonical 580-byte alternate typed track transform dispatch, previously analyzed. | No direct call edge from either FPCR caller was established. |
| `027ce11c` | Canonical 668-byte animation/cache-owner path, previously analyzed. | Static anchor does not connect it to either FPCR caller or identify a worker FP context. |
| `027b6624`, `027b6628`, `027b6744`, `027b69a0`, `027b69c4` | Existing dec101 relocation map resolves rotation dispatch table tags 0–4 to these targets. | Dispatch ownership gaps remain as previously documented; no worker-thread link follows from this table. |

The immediate direct-call argument dataflow establishes only that both known
helper invocations select FZ. Exact function owners are absent from the canonical
inventory; therefore this audit does not attribute either call to a thread entry,
initializer, or animation worker based on proximity or naming. No static direct
or precisely resolved indirect path was found from these callers to the
animation evaluation anchors. That is a bounded finding, not proof that no
runtime/indirect path exists.

## Runtime boundary and next task

Existing static qualification covers packaged module identities and a candidate
SDK provider. The exact applicable loader binding contract/source was not
available in this pass; the package list alone cannot prove selection, import
resolution, or loaded contents. The offline audit cannot establish the actual
epsilon binding, which thread runs animation evaluation, or the FPCR/FZ state
that thread reaches. No runtime claim is made.

**Next bounded task:** qualify runtime prerequisites and, only if available,
observe the loaded import binding and FPCR on the animation evaluation thread.
Until then keep the actual loaded binding and worker FP context unknown; do not
expand the numerical API or promote behavior/matching evidence from this static
caller audit.

## Verification

- Rehashed archive, main NSO, main ELF, and SDK ELF against the package/static
  qualification records before inspection.
- Read-only AArch64 disassembly/control-flow inspection at the named addresses;
  canonical ownership checked against `re/exports/update-main/functions.tsv`.
- No QEMU/game/emulator run, no compiler/tests, no source-code change, and no
  binary/assembly/pseudocode copied into the repository.

## Key Learnings:

1. An imported-symbol relocation plus a unique packaged static definition identifies a candidate provider, not the loader's runtime binding.
2. Caller arguments can prove an FZ-enabled branch without proving the caller's canonical owner or the animation worker's FPCR state.
