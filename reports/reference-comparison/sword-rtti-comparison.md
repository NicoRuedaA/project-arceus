# Reference comparison: Pokémon Sword decompilation vs. PLA update `main`

Date: 2026-10-10. Status: **hint only, not evidence.**

## Scope

Compare the type-name (RTTI) inventory of the PLA update `main` NSO with the
public Sword decompilation repositories, to assess whether shared engine or
library data can accelerate PLA work.

Repositories reviewed (GitHub API, 2026-10-10; neither declares a licence):

- `charlieduzstuf/pokesword` (fork of `notyourav/pokesword`), Pokémon Sword
  build 562, Switch. Files read: `README.md`, `PROGRESS.md`,
  `data/rtti_classes.csv`, `data/functions_main.csv`,
  `data/name_propagation.csv`, `lib/NintendoSDK/README.md`, `vendor/pkNX/*.fbs`.
- `benpushkarev-commits/FireRed-remake-and-remaster`: Game Boy Advance
  FireRed (ARM/Thumb), based on `pret/pokefirered`. No applicable function data.

## Method

1. `strings` over `work/pla/pk2/main.elf`, filtered for Itanium-ABI type-name
   strings; demangled with `c++filt` (`_ZTS` prefix). Result: 1,974 names.
2. Compared the `scope` column of Sword `data/rtti_classes.csv`
   (1,843 distinct names) by exact string equality.

## Results

| Metric | PLA | Sword |
|---|---|---|
| Distinct type names | 1,974 | 1,843 |
| Common | 144 | 144 |
| `SiSDK3`, `PPFX`, `gflib` names | 0 | present (223 / 43 / n.a.) |
| Havok (`hk*`) names | present | 0 |

Common types by namespace: `nn::pia` 68, `nn::ui2d` 37, `nn::vfx` 10,
`nn::font` 9, `nn::gfx` 2, `pead::*` 8, `nn::MemoryResource` 1. Full list:
[`sword-shared-sdk-types.txt`](sword-shared-sdk-types.txt).

## Findings

- The overlap consists only of Nintendo SDK libraries. No game-engine class is
  shared.
- Middleware differs (Sword: `SiSDK3`, `PPFX`; PLA: Havok, gRPC, protobuf,
  `nn::npln`). The data do not demonstrate a shared engine. They do not prove
  the opposite either: only RTTI names were compared.
- Sword function names are derived from error strings or are generated labels
  (`name_propagation.csv`, `functions_main.csv`). They are not recovered symbols.
- Sword matched bodies are byte matches against Sword's own compiler output and
  do not transfer to PLA.

## Permitted use

- The 144 shared SDK type names: hints for naming SDK classes in PLA, to be
  confirmed against PLA's own structure. SDK version equality is not verified.
- `vendor/pkNX/*.fbs` (Sword/Shield data schemas): starting point for reading
  PLA data files; PLA layouts must be verified independently.
- `letsgo-pipeline/` scripts (FireRed repo): candidate tooling for model and
  texture formats; not assessed against PLA.

## Not permitted

Per `AGENTS.md`, none of this material may advance analysis, implementation,
behaviour or binary-match states in
`sheets/re/function_progress_evidence.tsv`. The reference repositories are
unlicensed and contain data derived from copyrighted binaries; no content from
them is copied here other than the type-name list above.

## Input fingerprint

`work/pla/pk2/main.elf` SHA-256 prefix `b772390207e0de68`.

## Addendum: `pead` vs. `sead` (3DWDecomp)

Reference: `shibbo/3DWDecomp` (Super Mario 3D World + Bowser's Fury; unlicensed;
read 2026-10-10). It contains a `sead` library reconstruction (`src/sead`,
`include/sead`; 2,899 functions, 77.9 % matched per its README).

Also reviewed, with no use for PLA: the Switchbrew `ExeFS` and `NSO0` pages
(file layout and NSO header only; no module contents or names).

### Findings

- PLA has 24 base classes in namespace `pead`. 23 exist as classes in
  `include/sead`; `RuntimeTypeInfo` is a namespace in `sead`. All 24 have a
  counterpart. Name match alone does not prove identical code.
- `pead::ExpHeap` (`FUN_0071ca64`, `ExpHeap::dumpYAML`): same structure as
  `seadExpHeap.cpp` (scoped lock, base `Heap::dumpYAML` call, 128-character
  scratch string, same print order `heap_type`, `alloc_mode`, `use_list_size`,
  `free_list_size`). Difference: PLA has no `find_free_block_mode` line, so the
  `sead` version differs from 3DWDecomp's.
- String literals from the `sead` sources found in PLA `main`: `heap` 18/58
  (`seadHeap.cpp` 11/21, `seadExpHeap.cpp` 6/14); `thread` 0/18; `prim` 2/23.
  Matches in `codec`, `framework` and `system` are generic text and not
  discriminating.

### Conclusion

`pead::Heap` and `pead::ExpHeap` correspond to `sead::Heap` and `sead::ExpHeap`
(different `sead` version). `Thread`, `Mutex`, `SafeString` and `Delegate`
could not be confirmed or ruled out by string comparison; they need a
function-level structural comparison. 3DWDecomp builds with Clang for NX
1.8.14; PLA's compiler is not established, so byte-level matching is not
assumed.

Hint only: nothing here advances states in
`sheets/re/function_progress_evidence.tsv`.
