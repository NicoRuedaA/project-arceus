# Suyu export is not ready for the local PLA update

Assessment date: 2026-10-04. Status: **prerequisites verified; export not attempted**.
There is no evidence yet of PLA compatibility, translation coverage, speed-up, or
behavioral parity. Continue Ghidra and the evidence-backed Rust port while the
exporter prerequisites are unresolved. This is a readiness decision, not a final
performance go/no-go.

## Pinned upstream and observed contract

Repository: `https://github.com/suyu-emu/suyu-main.git`.
Branch: `mk8-recomp`; immutable commit:
`fbf385a6137ca98672a1ddc5dd478ef00d790c12`.
Resolved with `git ls-remote <repository> refs/heads/mk8-recomp`.
The pinned README says v0.0.11, but GameExport already contains v0.0.12 regeneration
guidance and a v0.0.12 release document exists. Use the commit, not the version label.

| Check | Direct evidence at the pinned commit | Consequence |
|---|---|---|
| Host build | README lines 212–267; `CMakeModules/CPMUtil.cmake:4` requires CMake 3.31; C++20 in `CMakeLists.txt:383` | A complete Linux host build and dependencies are prerequisites; no installation was performed. |
| Input | `src/core/loader/loader.h:38–49`, `loader.cpp:90–154`; `src/suyu/game_export.cpp:1216–1219` | Supported container inputs are NSP/XCI; NSZ is not a supported loader type. The local NSZ cannot simply be selected as a ready game/update. |
| Update install | `docs/user/GameExport.md:119–140`; `game_export.cpp:1891–1893` | Update installation accepts NSP. Export the base game with the intended update selected, not an update alone. |
| Update selection | `game_export.cpp:1323–1378`, `src/core/file_sys/patch_manager.cpp:175` onward | The displayed update uses the patcher's selection. An unreadable update can leave base code; require verified v262144 plus the expected module build ID before relying on output. |
| Extracted input | `GameExport.md:201–210`; `src/core/loader/deconstructed_rom_directory.cpp:157–179` | A clean ExeFS with matching `main.npdm` is required by the game loader. A lone renamed NSO is not proof of a valid update game. Version overrides are labels, not identity evidence. |
| Export scope | `game_export.cpp:3561–3677,3748–3816` | The export analyzes and translates available NSO modules, not a documented main-only function slice. Bound the experiment externally and assess `main` separately. |
| Linux output | `GameExport.md:38–50,103–107` | Source-only output; generated libraries still depend on Suyu/game data. Strict AOT has no JIT fallback; Hybrid can fall back. Neither is a Rust port. |
| Measurable output | `game_export.cpp:3819–3832` | Module block/emitted/unhandled counts are available in export logs, but none were obtained locally. |
| Provenance | `PROVENANCE.md:11–19`, README license section | Upstream is GPL-3.0-or-later with inherited GPL-2.0-or-later files. This does not license game-derived C or establish permission to redistribute it. |

Primary sources: [README](https://github.com/suyu-emu/suyu-main/blob/fbf385a6137ca98672a1ddc5dd478ef00d790c12/README.md),
[GameExport](https://github.com/suyu-emu/suyu-main/blob/fbf385a6137ca98672a1ddc5dd478ef00d790c12/docs/user/GameExport.md),
[export implementation](https://github.com/suyu-emu/suyu-main/blob/fbf385a6137ca98672a1ddc5dd478ef00d790c12/src/suyu/game_export.cpp),
[loader](https://github.com/suyu-emu/suyu-main/blob/fbf385a6137ca98672a1ddc5dd478ef00d790c12/src/core/loader/loader.cpp),
[patch manager](https://github.com/suyu-emu/suyu-main/blob/fbf385a6137ca98672a1ddc5dd478ef00d790c12/src/core/file_sys/patch_manager.cpp),
[provenance](https://github.com/suyu-emu/suyu-main/blob/fbf385a6137ca98672a1ddc5dd478ef00d790c12/PROVENANCE.md).
Web browsing read the branch guide; cache misses for immutable URLs were resolved
by direct HTTPS reads of these public source files. GitHub tree API returned a rate
limit; no repository-wide checkout or recursive dependency download was needed.

## Local checks, with bounded scope

| Item | Result |
|---|---|
| Authorized archive | `pk2.nsz`: 52,657,467 bytes; SHA-256 `f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446`, verified with `sha256sum pk2.nsz`. |
| Update main | `/home/nico/work/pla/pk2/main.nso`: SHA-256 `89fa2d715e13a1c311a47ee02b3cc858ccf72a2086ab9704a08a2ddc1b90f7d9`; NSO header build ID `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e000000000000000000000000`. |
| Base main, exclusion control | `/home/nico/work/pla/pk1/exefs/main`: SHA-256 `6f0e5f4a76a0f8b147540296522e02f0d03d591bfa53929d02cacc910ee994a0`; build ID `7fcad279539de183b25c11834fd4a030591cfe25000000000000000000000000`. It is NOT the update target. |
| Input directory | Update workspace contains `main.nso`, `main.elf`, extraction metadata/sections and patched RomFS, not a clean complete ExeFS. The only checked complete ExeFS is under `pk1`, so it cannot stand in for update v262144. No files were combined or renamed. |
| Exporter | `command -v suyu suyu-cmd` finds neither. Depth-two checks of `/home/nico/work`, `/home/nico/Downloads`, `/home/nico/.local/bin`, `/mnt/dev` find no Suyu executable, AppImage, archive or named checkout. Standard Suyu/Yuzu user-data roots checked are absent. This is not an exhaustive machine search. |
| Build tools | `cmake` absent on PATH; Ninja 1.13.2, GCC 16.2.1 and Clang 22.1.8 available. pkg-config finds Qt6Core/Widgets 6.11.2, SDL3 3.4.16 and FFmpeg components. Presence is not a successful dependency configure/build. |
| Storage | About 370 GiB free under `/home/nico/work`, 3.0 GiB under `/mnt/dev`. Any future exporter build/cache belongs outside the repository on the home volume. |
| Secrets and runtime | No local key configuration was validated, no game/emulator was launched, and no update was installed. Do not inspect extraction manifest payloads: metadata can embed cryptographic material. Only explicit non-secret identity fields belong in evidence. |

Public-file SHA-256 anchors for reproducible source verification:

```text
README.md                          097f2c488e9a5c69a955648586beb7e89e6ed63431720a64695884cd75bba88e
docs/user/GameExport.md             91894cb74e2819856a351b3ccea96fad3cd5e8d90122ab804874788382e9c416
PROVENANCE.md                      893b62700ea3f4608a3fa6e6bca377022eaf55a570b8ab5d523032109f0a9cd7
src/suyu/game_export.cpp            6798c9708cf091eb8de2c36693e195ccd65a4a400fb9e267a263c9153efba3eb
src/core/loader/loader.cpp          3558d0057747e14febf6f76a4790f9968a752aab894fc9b425a2c86b771b520e
src/core/file_sys/patch_manager.cpp  391f59ef9629d6131bfeb8d3cec6f1ec2c23c4b0847ed3c54a109d072e421e0d
```

## Next bounded action and cost gate

First provide an exporter built from the pinned commit and a validated, complete
update input. Do not launch an export with the base ExeFS or treat version overrides
as an update conversion. Do not download games, keys or firmware.

A source build would fetch/build third-party dependencies and write host binaries
and caches; an export scans all available modules and emits game-derived C. Neither
runtime nor output size has been measured for PLA. Suggested **future limits**, not
estimates: one 60-minute host-build attempt with a 16-GiB private workspace cap;
after readiness, one source-only export with a 20-minute/4-GiB cap. Confirm the
budget before launching that substantial work. No such attempt occurred here.

Keep all generated C, intermediate ExeFS, logs with game material and packages in
private storage outside the repository. Record only allowlisted module build IDs,
relative PCs, counts, tool revision, elapsed time and explicit fallback results.
Compare offsets against update `main` Ghidra evidence after confirming identity;
record export/compile/runtime independently. This assessment changes no native
function analysis, implementation, behavioral verification or binary-match state.
