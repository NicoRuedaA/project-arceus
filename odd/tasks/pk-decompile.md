# Feature: pk-decompile — Pokémon Legends: Arceus (pk1 + pk2)

The Spreadsheet Method applied to a legally obtained, user-provided copy of
**Pokémon Legends: Arceus** for Nintendo Switch, with the goal of producing
portable, provenance-tracked knowledge (sheets) and a Rust/Bevy port reference.

## Status

- State: in progress — extraction + code coverage complete; the port runs real
  events, decodes BNTX textures and binds the message/event-data chains. Open:
  `event_list.bin` record layout, message `.dat` text encoding, ASTC + 8
  unknown-format BNTX files, the field system, `tr*` models/animations.
- Last update: 2026-10-03
- Owner: nico (user-invoked decompilation)
- Memory mirror: `odd/pk-decompile/tasks` — **BLOCKED**: local Engram binary is
  older than v2.0.0-rc.11 (`instance-id` unsupported). This file is the source
  of truth until Engram is upgraded.

## Current ODD work unit — Priority 1 vertex layout (complete; validated in working tree)

The raw boundaries and three-model layout are cross-checked, and the layout-aware implementation is complete in the parser and mesh spike. The work unit is validated in the working tree and remains uncommitted; no explicit commit request was given. Preserve older claims with explicit correction tags. Engram remains unavailable as recorded above.

**Vertex data starts at `0x44` for these references.** `u32@0x40` is the vertex payload size; the index buffer begins at `0x44 + u32@0x28`, leaving a 28-byte gap *after* the vertex payload. The previous `TrMbf::parse` offset `0x60` was therefore 28 bytes too late for these files; the declared-layout parser now returns `0x44`. Position values at byte offsets `0/4/8` match the `.trmsh` bounding-box extrema exactly for item and rock, and to a consistent 0.004 margin for cliff.

| Model | Payload bytes | Index start | Index count / max | Vertices / stride / triangles | Position floats |
|---|---:|---:|---:|---:|---|
| `item_228` | 98,208 | `0x18000` | 3,564 / 2,045 | 2,046 / 48 / 1,188 | 0, 1, 2 |
| `d110_gimmick_rock02_lod1` | 7,200 | `0x1c80` | 696 / 199 | 200 / 36 / 232 | 0, 1, 2 |
| `ground_area02_cliff01` | 46,200 | `0xb4d8` | 6,303 / 1,154 | 1,155 / 40 / 2,101 | 0, 1, 2 |

The `.trmsh` attribute section has one unique `[6, stride, attr_count, descending field-offsets]` candidate per reference. For each field offset `f` at ordinal `i`, the associated record ID is at `table_start + f + 20 + 4*i`; the code follows it. The ID 1 byte offset is the preceding zero; other record byte offsets follow the code. The three parsed layouts are:

| Model | ID / format code / byte offset / size |
|---|---|
| item | `1/51/0/12`, `2/43/12/8`, `3/43/20/8`, `6/48/28/8`, `7/22/36/4`, `8/39/40/8` |
| rock LOD1 | `1/51/0/12`, `2/43/12/8`, `3/43/20/8`, `6/48/28/8` |
| cliff | `1/51/0/12`, `2/43/12/8`, `3/43/20/8`, `6/48/28/8`, `5/20/36/4` |

Observed semantic evidence: ID 1 is position; ID 2 is four f16 components with near-unit xyz and w=0 (normal); ID 3 is four f16 components with near-unit xyz and w=1 (tangent). ID 6's two f32 values at byte 28 are in `[0,1]` for every vertex and match shader UV0 evidence (dec074). Cliff ID 5 is four bytes with constant white RGB and varying alpha, consistent with dec083's `VertexColor` layer mask. Item IDs 7/8 contain an index-like value 1–11 and weights `[65535,0,0,0]` (one influence); the exact joint format remains a supported interpretation, not a generalized decoder.

Implementation evidence: `crates/pla/src/assets/tr.rs` defines `TrMshLayout::parse`, `TrMbf::parse_with_layout`, and layout-backed `attribute_offset`, ID 1 position, and ID 6 UV0 access. It keeps the declared `.trmbf` payload at `0x44`, and leaves the old `0x60` start scoped to the heuristic multi-mesh fallback. `crates/pla/examples/mesh_spike.rs` attempts the sibling `.trmsh` and falls back to inferred position plus planar UV when unavailable/unsupported. Synthetic regression coverage is in `crates/pla/src/assets/tr.rs`; optional fixture coverage is in `crates/pla/tests/assets.rs`. Format widths beyond the observed codes and generalized skinning/joint decoding remain unsupported.

1. Boundary/count and position evidence: completed; the old rock count 329 and item/cliff position-float claims were explicitly corrected in the decision sheet and plan.
2. `.trmsh` IDs, codes, offsets and observed field widths across the three references: completed; semantic evidence retains uncertainty for joint fields.
3. Implemented layout-aware Rust accessors, real UV0 use in the mesh spike, and focused regression tests. Material `UVScaleOffset` is applied to real UV0 and to the planar fallback.
4. Validation completed with `./.tools/ci.sh` (exit 0): rustfmt passed; clippy passed; sheet preflight passed (27 sheets, 267,854 rows, 0 warnings or errors); workspace all-target build passed; workspace tests passed (69 passed, 2 ignored). No commit was made because no explicit commit request was given. Engram mirror `odd/pk-decompile/tasks` remains unavailable because the installed local binary predates v2.0.0-rc.11, so this task file remains authoritative until Engram is upgraded.

## Target identification (observed)

| File | Size (bytes) | SHA-256 | Identity |
|---|---|---|---|
| `/mnt/dev/decompilacion/pk1.nsz` | 2 334 586 382 | 00167d5e00bf7f5fca5311f984069497f79812cbbd03c32eb7f2fc1649af2acc | Base game `Pokemon Legends Arceus [01001F5010DFA000][v0]` |
| `/mnt/dev/decompilacion/pk2.nsz` | 52 657 467 | f68eecf0e5a207f87d4668f9e7654fa3e3eb2ceb6a2060424124d723a09e6446 | Update `Pokemon Legends Arceus [01001F5010DFA800][v262144]` |

- Evidence: `juego.torrent` (rutracker.org torrent, created 2022-03-18) lists
  both original filenames plus a Russian language mod for the base title.
- Authorization: user provided the files and explicitly requested
  `Decompila pk`. Personal-use port research; do not redistribute content.

## Route

- Mode B (binary without source). No public full decompilation of this title
  is known to exist.
- NSZ/NCZ decompression via `nsz` 5.0.0 (nicoboss/nsz). **No keys required**:
  the NCZ header embeds each section's AES-CTR key/counter, and the compressed
  payload holds pre-encryption (plaintext) section data. Skipping the tool's
  re-encryption step yields decrypted content directly.
- Static analysis: Ghidra 12.1.2 headless on an NSO→ELF64 (AArch64) conversion.
- Indexing: `gamedb` (built at `gamedb/target/release/gamedb`).
- Projection: `THE-SPREADSHEET-METHOD.json` render contract.

## Container format (observed)

NSZ container = PFS0 variant:

- `0x00` magic `PFS0`, `0x04` u32 version (6 = base, 7 = update), `0x08` u64,
  `0x10` u64.
- File records at `0x10`, 24 bytes each:
  `u64 data_offset | u64 size | u32 name_offset | u32 reserved`; names follow
  the records, offsets relative to the string table start.
- The first entry's slot includes the container header; the nsz `Nsp` reader
  maps entry positions correctly (verified: entry position 0 of the `.ncz`
  reads the NCA header, `NCZSECTN` at mapped `0x4000`).

### NCZ format (from `nsz/IndependentNczDecompressorConcise.py`, verified)

- `0x0000–0x3FFF` NCA header region copied verbatim (stored XTS-encrypted).
- `NCZSECTN` + u64 count + sections
  `{u64 offset, u64 size, u64 cryptoType, u64 pad, 16B key, 16B counter}`.
- Then a solid zstd stream (no `NCZBLOCK` table in these files). Stream =
  concatenated section payloads in ascending offset order; a synthetic "fake"
  section covers the gap `0x4000..first_offset` when present.
- Sections carry absolute NCA offsets; all payload bytes are the decrypted
  (pre-encryption) section content.

## Extraction results (verified)

Extractor: `.tools/ncz_extract.py` (writes raw entries + decrypted NCZ
sections + `manifest.json` with sizes/SHA-256/first bytes).

**Parity proof (reproducible):** the `nsz` tool's own `__decompressNcz`
reconstruction (with re-encryption) of both `.ncz` entries yields SHA-256 hashes
that match the content-addressed entry filenames exactly:

| Entry | Reconstructed SHA-256 (nsz) | Match |
|---|---|---|
| `pk2` `b38cd4c1…ncz` | `b38cd4c18831237d85312886f3a377b51b25b5db3812a374bf655ab2275d739f` | yes |
| `pk1` `c0717d7f…ncz` | `c0717d7fc3748b3226e1b1b7e0e69a20993b2e30052714608fe696c31e4af71e` | yes |

| Target | Output | Content |
|---|---|---|
| `pk2.nsz` | `/home/nico/work/pla/pk2/` | 7 entries; `.ncz` = update program NCA, 23 chunks, all cryptoType 3 |
| `pk1.nsz` | `/home/nico/work/pla/pk1/` | 6 entries; `.ncz` = base program NCA, 3 chunks (98 304 B crypto 1; 6 413 369 344 B + 45 891 584 B crypto 3) |

### pk1 base NCA content map (decrypted)

| Range (NCA offset) | Bytes | Identification |
|---|---|---|
| `0x004000–0x01C000` | 98 304 | Logo section (unencrypted): PFS0 at `0x8000` with `StartupMovie.gif` (51 019 B) and one 20 489 B entry |
| `0x01C000–0x17E460000` | 6 413 369 344 | Game data region (RomFS payload); contains BNTX textures, `.bflan` animations, `bin/` paths, custom `SFNT` file-index tables |
| `0x17E460000–0x181024000` | 45 891 584 | ExeFS section: hash table `0x8000` + PFS0 |

### ExeFS (verified, extracted)

Base (`/home/nico/work/pla/pk1/exefs/`): `main` 31 755 066 B, `main.npdm`
1 636 B, `rtld` 7 187 B, `sdk` 5 754 872 B, `subsdk0` 3 409 290 B,
`subsdk1` 4 920 073 B.

Update (`/home/nico/work/pla/pk2/`): `main` 31 882 976 B, `main.npdm` 1 636 B,
`rtld` 7 187 B, `sdk` 5 754 872 B, `subsdk0` 3 409 290 B, `subsdk1` 4 920 073 B.

### `main` NSO (update, verified)

- NSO0, flags 0x3f; text 0x32A5690 / rodata 0xD73800 / data 0x273020 +
  bss 0xC3FE0; module id
  `aee8f150dda1b5a838806e1a5ea6827ad9f3c51e000000000000000000000000`.
- Converted to ELF64 AArch64 with `.tools/nso_to_elf.py`; all three segment
  SHA-256 hashes verified OK. Output `/home/nico/work/pla/pk2/main.elf`.

## RomFS (resolved)

- The base section payload is an IVFC-wrapped RomFS: hash-table levels first
  (block size `0x4000`, level-4 table at section `0x14000`), RomFS image at
  section `0xC04000`.
- The RomFS header stores table offsets relative to the image base with the
  tables **after** the data region (non-standard order) — that is why the
  earlier ordering-based scans missed it. Locate headers by the structural
  chain instead.
- **18 370 files, 6 399 560 182 bytes** listed and extracted
  (`/home/nico/work/pla/pk1/romfs/`; index `re/exports/base-main/romfs_files.tsv`).
- Cross-check: the first romfs file
  (`/bin/appli/advresult/bin/advresult_00.arc`, 2 441 160 B) is exactly the SARC
  at section `0xC04200` that the earlier magic census found independently.
- The update's romfs section is a patch: no `BKTR`/`SARC`/romfs-header magic in
  its 46 MB payload. Its ExeFS is fully extracted and decompiled, so this only
  affects update-only assets.

## Environment / tooling

- `.tools/venv` (`uv`, Python 3.13): `nsz` 5.0.0 (git), `lz4`, `pycryptodome`,
  `zstandard`.
- `.tools/ncz_extract.py` — container + decrypted-section extractor.
- `.tools/nso_to_elf.py` — NSO→ELF64 converter with hash verification.
- `.tools/ghidra_scripts/ExportDecompiled.java` — per-function C exporter.
- No `prod.keys` anywhere on the machine (verified by filename and content
  search). Not needed for the current path.
- Disk: `/mnt/dev` ~11 GB free (project), `/home` ~421 GB free (staging).
  Large intermediates live under `/home/nico/work/pla/`.

## Work log

| # | Task | State | Evidence |
|---|---|---|---|
| 1 | Set up nsz toolchain (local venv, no sudo) | done | `.tools/venv/bin/nsz --help` |
| 2 | Create this task document | done | this file |
| 3 | Inspect NSZ containers (title info) | done | `Nsp` library listing; torrent names |
| 4 | Extract update (`pk2.nsz`) entries | done | `/home/nico/work/pla/pk2/manifest.json` |
| 5 | Extract base game (`pk1.nsz`) entries | done | `/home/nico/work/pla/pk1/manifest.json` |
| 6 | NCA content map / content types | done | section maps above (header XTS-encrypted; types inferred from payloads) |
| 7 | Decrypted ExeFS + `main` NSO | done | ExeFS dirs, `main.elf` with verified hashes |
| 7b | RomFS structure/extraction | blocked (open) | see "RomFS status"; needs FS-header descriptor (keys) or deeper RE |
| 8 | Ghidra analysis + C export | done (bounded) | capped project `PLA-update-capped`; evidence TSVs in `re/exports/update-main/` + `sheets/re/`; 1 996 top functions decompiled to `decompiled/update-main/` |
| 9 | `gamedb` index + parity check | done | `decompiled/.gamedb/index.sqlite`: files 1 996 / functions 1 949 / strings 8 365 / edges 5 302; 0 skipped, 0 failed |
| 10 | Sheets projection + preflight | done (first pass) | 14 sheets, 265 526 rows; preflight 0 errors, 1 expected warning (`port_target` unimplemented) |
| 11 | RomFS / SARC extraction | partial | 257 archives, 3 416 files, 248 MB extracted (base game); wrapper/descriptor still open |
| 12 | `sheetty` Rust machinery (parser, L0-L3 preflight, emitter) | done | `crates/sheetty` + `crates/sheetty-cli` + `crates/pla`; 13 tests; Rust preflight matches the Python prototype (0 errors, 1 warning); 15 generated modules |
| 13 | RomFS resolution | done | IVFC chain cracked; 18 370 files / 6.4 GB extracted; header located by structural chain |
| 14 | Bevy port (first subsystem) | partial | Bevy 0.19.1 minimal; PHF registry + dense index + dispatch table over `domain/subsystems`; 6 port tests green |
| 15 | Decompiled coverage extension | done | ranks 2001-4000 exported: 3 996 update + 3 997 base `.c` files; gamedb 7 993 files / 7 832 fn |
| 16 | Subsystem map (string-path clustering) | done | 17 named subsystems from `strrefs.tsv` (gRPC 1 246 fn, OpenSSL 536, Havok 430, NPLN, appli/effect/chara/...); wired into the port registry |
| 17 | Port strategy + asset formats | done | `domain/port_strategy.tsv` (19 rows: port/stub/replace/skip) and `domain/asset_formats.tsv` (17 formats from the 18 370-file romfs inventory); both wired into `crates/pla` |
| 18 | Script layer in the port | done | `mlua` 0.12 (lua53, vendored): real event scripts **load and execute** as Lua 5.3; entry point `system.EventScriptMain.ScriptCall` callable; 3 script tests green |
| 19 | Script binding contract (sol2 ctti metadata) | done | **322 bound methods over 156 C++ types** extracted from embedded `ctti_get_type_name` strings, plus 41 glue source paths revealing the game's `prog/<subsystem>` layout |
| 20 | Lua decompilation | done | **799/800 event scripts decompiled with unluac** (253 MB of Lua) via `.tools/decompile_lua.py`; index in `re/exports/base-main/lua_scripts_index.tsv` |
| 21 | Event protocol decoded | done | `ScriptCall(class, module, targetHandle)` resolves `<module>.<class>`, binds a `ScriptObjectHandle` to 10 command singletons, then `Initialize()` + `Execute()` |
| 22 | Coverage tranche 3 | done | ranks 2001-6000 exported: 5 996 update `.c` files; gamedb **9 993 files / 9 810 fn / 24 917 strings / 32 745 edges** |
| 23 | Host layer + event end-to-end | done | `crates/pla/src/host.rs`: `bit` shim, `ScriptObjectHandle`, 15 engine globals, 10 command singletons as recording stubs. A real event (`BoutiqueMainEvent`) runs end to end and emits a **42-call host trace**; 2 end-to-end tests green |
| 24 | Keys integration | done | `prod.keys` v22.5.0 (user-provided): NCA header XTS (big-endian tweak), ticket title key; keys gitignored (dec029) |
| 25 | Auxiliary NCAs | done | 6/6 decrypted: control (NACP "Pokémon Legends: Arceus" + 9 icons), manual (18 files incl. legalinfo.xml), meta (accessible-urls) |
| 26 | **Update RomFS (BKTR) reconstruction** | done | Virtual romfs rebuilt from base+patch: **19 095 files / 6.42 GB**; **parity 400/400 unpatched files byte-identical** to the base extraction; 725 files added, 465 size-changed |
| 27 | Event runner + engine target (lane A) | done | `crates/pla/src/event.rs` (coroutine driver, frame budget) + `host::make_event_target` (`event_script::EventScriptObject`: GetTargetID/IsSkip) + `_G` recording-stub fallback (799-script census). The reference event runs to its field-system wait instead of aborting at IsSkip; 4 event tests green |
| 28 | Local CI gate (lane F) | done | `.tools/ci.sh`: rustfmt --check, clippy `-D warnings`, sheet preflight, workspace build, tests; `--full` adds the gamedb build + selftest (87/87). Workspace was made fmt- and clippy-clean for it |
| 29 | Flagwork sheets (lane D) | done | All seven `bin/flagwork` AHTB tables as sheets from the **update** build: 450 event flags, 45 event works, 70 map flags, 24 map works, 8 phase works, 704 system flags, 106 system works. `.tools/ahtb_sheet.py` generates them (never hand-edited); preflight 27 sheets / 267 784 rows / 0 errors; Rust-vs-Python parity over all seven |
| 30 | Coverage tranche 4 (lane E) | done | ranks 6001-8000 exported: **2 000/2 000 ok, 0 fail, 0 missing** (7 996 update `.c` files total); gamedb re-index: **11 993 files / 11 675 fn / 26 936 strings / 43 117 edges**; 11 993 exported `.c` = 11 993 indexed (100 %) |
| 31 | BNTX pixel decode (lane G) | done | Full BRTI parse (Wexos layout: width at +0x24), GX2 format table, Tegra block-linear deswizzle (`tegra_swizzle`), BC1-7/R8/RGBA8 block decode (`bcdec_rs`). Verified visually: pokeicon = Pokemon egg icon (BC7), map frame (BC3), normal map (BC5), weather (BC4). Census 2 969 files: 99.6 % covered; 4 unit + 3 fixture tests |
| 32 | Message bindings (lane C) | done | `.dat` addressing cracked and verified on **322/322** English files (header + 8-byte entries {offset, len, flags}, 4-byte aligned); `assets/message.rs` + real `Global.GetFieldMessageWindowManager()` (WordSetPlayerName / IsClosedMessageWindow / CloseMessageWindow). The text encoding itself is still unidentified (dec036): the loader dispatches indirectly |
| 33 | Event-data bindings (lane B) | done | Real `GetEventScriptManager -> GetEventProgressManager -> GetEventListManager -> FindEventData`: the data table carries the fields the scripts read plus `GetFlag` = the event's own FnvHash64, so `SetEventFlag(GetFlag())` lands in the save store keyed by the event id (test asserts it). Found and fixed an exactness bug: u64 hashes above i64::MAX were stored as Lua doubles, losing 11 bits (dec038) |
| 38 | tr* + Havok assessment | done | tr* census (3 748 files / 50.5 MB) + verified per-type header size (trmdl 24, trskl 16, trmbf 12); Havok .hkx = tagged binary, deferred until a system needs colliders (dec043/dec044) |
| 74 | **.trmsh vertex-layout section cracked** | done (structure) | Differential analysis of the 3 validation models: `[6][stride][attr_count][offset table]` + attribute records (shared ids 3/2/1 codes 43/43/51; per-model ids 8,7,6/6/5,6). Stride + attr count now readable from the file (36/40 sample hits). Cliff layout `[6,5,1,2,3]` verified against trmbf floats; item reads as two 24-byte streams. Capstone in `.tools/venv` for code beyond Ghidra's capped analysis (dec091) |
| 71 | Coverage tranche 6 | done | triage regenerated (top 13000, top-20 identical), ranks 10001+ exported: 2 000/2 000 ok; corpus now **12 302** update `.c` files (dec086) |
| 72 | Priority-1 numeric search (offsets) | done (no hits) | Exhaustive decimal+hex search over the whole corpus: every pair/triple/stride match inspected is a coincidental struct offset; the loader reads the per-model numbers from the `.trmsh`, it never hardcodes them (dec086) |
| 73 | gfx2 module window + NVN vertex API | done | gfx2 loaders are 12–99-line functions invisible to size-ranked triage (dec087); exported gfx2_cluster (24) + trm_d1 (11) + the whole 0x0012A000–0x00151000 window (251/251): shader reflection found (`__CUS_Vec2_0`, `uProceduralShape`), mesh parser not in the window (dec090). main.elf **never calls** `nvnVertexAttribStateSetFormat`/`SetStride` — the table slots have zero readers program-wide (dec088). No `.trmsh` literals/immediates/FNV hashes anywhere (dec089). Next: export the reflection API's **callers** (> 0x00152000, material/model classes) |
| 70 | .trmsh per-model head mapped | blocked | The head carries per-model numbers: a pair differing by exactly 32, another further down, and a triple ending in 4 whose first two differ by 8 (32/24/4 item, 48/40/4 rock, 24/16/4 field). The triple looks like two offsets and a width but cannot be the record layout (48 > the rock's 36-byte record), so the schema needs the loader (dec085) |
| 69 | Input-layout schema (bounded) | blocked | The .trmsh's declaration is a Nintendo field-offset structure: four entries `[u16 id][u16 pad][u16 offsets...]` (ids 3/2/1/51, offsets below the leading value) plus head tables and a per-model pair that differs by exactly 32 in every file (380/348, 572/540, 388/356). Shape mapped, schema in the engine: needs a Ghidra tranche over the loader (dec084) |
| 68 | Layer blending + all container meshes | partial | The CPU blend composes BaseColorMap1 through LayerMaskMap when both exist; no material has both because the .trsha says LayerMaskSource is VertexColor for the layered ones, so the mask is the vertex colour — the same blocker as the UVs. `parse_all` returns every mesh of a container (dec083) |
| 67 | **Field terrain models parse** | done | The stride only has to divide evenly (field models use 40); `ground_area02_cliff01` reads 1 155 verts / 2 101 tris, detects the position at floats 3,4,5 and renders a coherent terrain patch with its albedo (dec082) |
| 66 | **Multi-mesh .trmbf container** | done | item_224 holds **eight vertex runs** (160/72/256/67/83/44/39/27) each with a descriptor and its own index buffer; the header values are offsets, not sizes. The parser walks the container, so item_224 loads (158 verts, 66 triangles) and renders, and item_228 is unchanged (dec081) |
| 65 | Emission, weather and PackedMap wired | done | Full role census over 253 materials; PackedMap (ARM) feeds metallic-roughness + occlusion directly, EmissionColorMap feeds emissive_texture, WeatherLayerMaskMap has no Bevy counterpart. Also: item_224 ships 8 158 indices and a second header variant (dec079) |
| 64 | UVs are projected (evidence closed) | done | Every UV encoding excluded at every offset in both readings (f32/u16/s16/u8/f16/10-10/11-11) + `UVScaleOffset` is a vector → the game projects the UVs and transforms them in the material (dec078) |
| 63 | Correcting the block structure | done | dec064's identical halves is **wrong** (29 203 differing bytes); the 24-byte reading alternates between real positions and records matching the model bounds; the packed region varies (dec077) |
| 62 | .trmbf header mapped | done | 68 bytes, byte-identical in shape across models: two Nintendo field-offset structures around the sizes 12/8/4, the block size at 0x28 and the vertex bytes at 0x40. The attribute byte offsets still do not fall out of it (dec076) |
| 61 | Record = two 24-byte streams | done | item_228's (0,1,5) is x,y at the start and z 20 bytes later = two interleaved streams; stream B holds the packed normal/tangent/UV data, still not decodable as any float/int pair (dec075) |
| 60 | **Attribute semantics from the shader** | done | The `.bnsh` embeds readable NVN assembly: **0 = position, 1 = normal, 3 = tangent (+w), 5 = UV0, 7 = UV1**. The UVs are vertex attributes 5/7, not a float pair in the record (dec074) |
| 59 | Shader chain found (the answer to the UV question) | done | `.trmtr` values -> `.trsha` schema (with `NumRequiredUV`) -> `.bnsh` compiled shader; 70 shader files / 22 MB under `system_resource/gfx2/shader/`, material set `ha_standard`, `ha_unlit`, ... matching the `.trmtr`'s `Standard` string (dec073) |
| 58 | **IBL works** | done | HDR cubemap (Rgba16Float, f32->f16 converter) built as (w, h x 6, 1) + `reinterpret_stacked_2d_as_array(6)` + a **Cube view descriptor** — the missing piece the render error revealed. The metal now reflects the sky instead of rendering black (dec072) |
| 57 | .trmsh tail = shader input layout | done | Three entries (index 3/2/1) + u32 43/43/51 + D3D-style field-offset tables; FNV ruled out; identical across models because it is the vertex-input signature, not geometry (dec070) |
| 56 | Layout auto-detection | done | `TrMbf::detect_position` picks the float triple maximising triangle quality; item_228 resolves to (0,1,5) with a 2.5x margin, the rock stays ambiguous because its record has near-duplicate channels (dec069) |
| 55 | .trmsh attribute table located | done | Both the rock and item .trmsh end in a **byte-identical** descriptor block (name hashes 0x2b/0x2b/0x33 + Nintendo u16 field-offset tables) = the fixed vertex-layout signature; per-model parts are the .trmbf name, sub-mesh names, index count and bounds (dec068) |
| 54 | UV search bounded | done | No UVs in the vertex record (texel-density CV bottoms at 1.32 vs ~0.3); layout varies per model (rock (0,1,2)/36, item (0,1,5)/48); the attribute table lives in the .trmsh (dec067) |
| 53 | **Textured + lit render** | done | With `bevy_light` and `tonemapping_luts` (+`zstd_rust`) the mesh renders lit and textured with the game's albedo; before, a missing LUT made everything magenta (dec065) |
| 52 | **Material flags drive the maps** | done | Value-first `True/False` + `Enable*` names; roughness+metallic packed into one MR texture; alpha test -> Mask (dec066) |
| 51 | **Materials: the .trmtr names the textures** | done | Property list with `*.bntx` + role; `TrMtr::parse`/`base_color()`; the spike decodes the albedo (256x256) as the mesh texture (dec062) |
| 50 | **Packed vertex record: position at floats (0,1,5)** | done | The only triple with volume on every axis; floats 2..4 are integers, 6..11 quantized (dec063) |
| 49 | **Quad strip indices verified** | done | (0,1,2)(2,1,3) pattern, 1 188 triangles, edges p99 0.278 (dec064) |
| 48 | Sub-meshes | done (reframed) | `item_228.trmsh` references the .trmbf and declares named sub-meshes: `chain_mesh` with u32@0x170 = 3 564 (exactly the index count) and `chain_mesh_shape`. The "shape keys" claim (dec061) is superseded; dec067 reframed the .trmsh as the carrier of the **per-model attribute table** |
| 47 | Attribute arrays 1..N | superseded | **Superseded by dec063/dec067** — the "independent vertex sets" were an artefact of slicing packed 48-byte records. Still valid: no array is a normal (per-triangle dot 0.22-0.38, no unit lengths) and none is half-float |
| 46 | Vertex layout: first (wrong) reading | superseded | The concatenated-array reading (dec058) is **superseded by dec063/dec067**: the block is a packed record and the position is at floats (0, 1, 5) for item_228. What survives: the reading bug in the spike (byte offsets passed as component indices) was real and had to be fixed |
| 45 | Mesh spike (WIP) | partial | `mesh_spike.rs` builds a Bevy Mesh from a real `.trmbf` with the attribute group configurable (0/1/2) and screenshots it; every run shows the clear colour only, while the sprite path renders fine. 3D-specific cause open (dec057): capture timing vs pipeline compilation, a missing 3D render half (like `bevy_sprite_render`), or the mesh data |
| 44 | **Render path works (window spike)** | done | `examples/window_spike.rs` decodes a real BNTX and draws it in a Vulkan window; the screenshot shows the egg icon. Bevy 0.19 needs **bevy_sprite_render** (separate from bevy_sprite) or sprites never reach the GPU (dec056) |
| 43 | Coverage tranche 5 | done | ranks 8001-10000 exported (2 000/2 000, 0 fail) — the corpus is now **10 019 `.c`** files; gamedb re-indexed (12 026 files / 13 687 fn). Still no function references the `trmbf` string, so the mesh loader is deeper than the top 10 000 or in another ExeFS module |
| 42 | Geometry chain mapped | done | `.trmsh` is the mesh descriptor: references its `.trmbf` by name, a 'mesh' tag, per-submesh names and nested count/offset tables (dec049). Chain: `.trmdl` (descriptor) -> `.trmsh` (mesh) -> `.trmbf` (buffers). Vertex count = max(index)+1 and stride = bytes/count (36 on the gimmick models, 48 on an item mesh — it varies with the attribute set; dec055). The 36-byte record splits into three 12-byte groups with near-identical statistics (0..1, 0..1, -1.6..0) and one dominant group per record — texture-space basis data, not position/normal/UV (dec052); the attribute descriptors in the game code are the next lead |
| 41 | tr* mesh buffer parsed | done | `.trmbf`: vertex block at 0x44 (size = u32@0x28) + u16 triangle indices, verified on two real files (72/24 and 1092/364 indices/triangles); `assets/tr.rs` `TrMbf` + fixture test. Vertex stride/attributes and the other types (.trmsh/.trmtr/.tranm) pending (dec048) |
| 40 | tr* container analysis | done | `.trmdl` dumped end to end: u32 header size (24), u32 offset table at 0x1C, transform floats, and length-prefixed references to its `.trmtr`/`.trskl` by name; the mesh data lives in `.trmbf`/`.trmsh`. Per-type headers differ beyond the first u32 (dec047). Parser is the next work unit |
| 39 | **Message text decoded** | done | Per-index **16-byte repeating XOR key** (the keystream repeats every 16 bytes; pooling p = j mod 16 makes the frequency attack exact). `.tools/message_cipher.py` recovers the keys (kept outside the repo); `MessageKeystream` + `__port_load_messages`/`GetMessage`/`SetMessage`/`IsEndMessage` in the port (the window state follows the message); fixture test decodes `str_pocketname_005` = **'Everyday Items'**, and the tool reads Poke/Master/Ultra/Great/Safari Ball, 'Money', 'Yes', 'n/a', full sentences with 0x10 control tags |
| 37 | Message text encoding (time-boxed) | partial | Six constraints verified (deterministic, content-keyed, not index-dependent, near-uniform, not a fixed XOR, loader behind an indirect call); algorithm still open (dec042). Follow-up: the pointer 0x04270dc8 is never written in main.elf (xref search over the whole program), so the decoder lives in another ExeFS module (dec045) |
| 36 | BNTX float formats + ASTC check | done | 0x1505 = R32_G32_FLOAT and 0x1905 = R32_G32_B32_A32_FLOAT, both 6-face HDR probe cubemaps, decoded (face 0, clamped-linear tone map); the census over both builds shows **no ASTC at all**, so that decoder is unnecessary (dec041). Every shipped format code now decodes |
| 35 | Event registry (event_list.bin) | done | Header verified across three files (version 12/16, count, descending offsets: 0/46/739) + a clean **1 821-name pool**; `assets/event_list.rs` exposes hash -> name and `__port_load_event_list` feeds `FindEventData.name`. The record fields are signed relative offsets into nested tables, so the record graph stays undecoded (dec040) |
| 34 | Field subsystem | done | `FieldProc` (initialized), `FadeManager` (IsEnd), `RideManager` (not riding), `FieldObjectForHaxe` (loaded, grounded) are real: **the reference event now runs end to end in one resume** (34 host calls, no error) instead of parking in the 2 700-frame `IsLoading` loop (dec039) |

## Complete game coverage

- Base RomFS: 18 370 files ✓
- Update (patched virtual RomFS): 19 095 files ✓
- Auxiliary NCAs (icons/NACP/manual/meta): ✓
- Code (base + update NSOs) and 800 Lua scripts: ✓
- BKTR recipe: relocation `{u64 virt, u64 phys, u32 is_patch}` at 0x14 stride,
  virtual space = whole section image (romfs at IVFFC level-5 offset), base
  offsets relative to the base *section* start, patch payload already plaintext
  (the NCZ stored the subsection ctr_vals) — decisions dec026–dec028.

## Event host trace (the port's work queue)

The captured trace for one event shows exactly what the port must implement:
`FnvHash64.new`, `Vector4.new().Set`, `Global.GetFieldMessageWindowManager()`,
`Global.FindFieldProc()`, `Global.GetEventScriptManager().GetEventProgressManager().GetEventListManager().FindEventData()`,
`Global.GetSaveSystem().EventWork().SetEventFlag()`.

After the event runner (dec033) the reference event goes further: it writes its
flag, answers the skip poll through the engine target, and then **waits for the
field system**
(`Global.GetFieldObjectForHaxe().__haxe_export_FindFieldObjectComponent().IsLoading`,
a bounded 2 700-frame loop that only ends when the field object finishes
loading). That wait is the next binding work item: a real field subsystem, not
more stubs.

A census over the 799 decompiled scripts (bare engine globals, dec032) shows
the full host surface the scripts read: `Global` (90 395 reads), `FnvHash64`
(71 048), `Vector3` (6 771), `AppConfig` (5 854), `Vector4` (3 228),
`Quaternion` (2 846), `FieldUtility` (798), `SoundController` (796), engine
classes such as `CameraParameter` (200) and `CameraInterpolationParameter`
(681), plus per-event classes. Unknown globals now resolve to recording stubs.

## Script bindings (the host contract)

The binary embeds sol2 ctti metadata with full C++ signatures for every
Lua-bound type. Extracted into `sheets/domain/script_bindings.tsv`
(322 rows), by namespace:

| namespace | methods |
|---|---|
| field | 169 |
| event_script | 41 |
| contents | 24 |
| gfl | 22 |
| savedata | 19 |
| gamesys | 16 |
| common | 16 |
| chara | 8 |
| ha_btl / poketool / demo / ui | 3 / 2 / 1 / 1 |

Game source layout from the glue paths (`re/exports/update-main/source_layout.tsv`):
`prog/common/lua`, `prog/field/{source,include}`, `prog/contents/lua`,
`prog/event_script/{source,include}`, `prog/chara/source`, `prog/demo/lua`.

## Script layer (the port's shortcut)

- `crates/pla/src/script.rs` owns the Lua 5.3 state; `crates/pla/tests/script.rs`
  proves: VM runs source, the real `event_sample.blua` loads, and execution
  fails only at runtime on the missing Haxe globals.
- The scripts call a native **host API** (e.g. `FieldObjectCommand`,
  `GetFieldObjectForHaxe`, `MessageCommand`, `GetFieldObjectForHaxe`,
  `FnvHash64`, `POKE_*`, `RegisterPosPhysics`, `GetRideManager`). Implementing
  that host surface is the port path for event logic (decision `dec016`).

## Port strategy (findings)

- **The game logic ships as Lua 5.3 bytecode.** 800 `.blua` files (797 event
  scripts) with `1bLua 5.3` chunks under `bin/haxe/release/`, produced by
  **Haxe**. `mlua` with the `lua53` feature can run them as-is, so event logic
  does not need reverse-engineering from the NSO (decision `dec014`).
- **Middleware dominates the binary**: gRPC 1 246 fn, OpenSSL 536, Havok 430,
  plus abseil/protobuf/re2/WebRTC/NPLN. The port owns gameplay and containers
  and delegates middleware (Havok → avian3d, TLS → rustls, online → skipped)
  (decision `dec015`).
- **Asset formats** (18 370 romfs files): BNTX textures 2 965 (790 MB), VFXB
  particles 1 748 (2.65 GB, embed BNTX), AHTB tables 3 067, `.dat` tables 3 179,
  GFLXPACK 678 (1.7 GB), SARC 257 (250 MB), tr* models/anims ~2 900, Havok
  `.hkx` 80, shaders 42.

## Subsystem map (domain/subsystems)

Clustering functions by the paths they reference replaced the SCC seed
(decision `dec010`). Update build:

| kind | subsystems |
|---|---|
| middleware | grpc (1 246 fn), openssl (536), havok (430), grpc_core (158), absl (106), protobuf (96), re2 (73), webrtc (17) |
| network | npln (Nintendo platform protobuf) |
| game_data | appli (38), effect (19), chara (5), event (5), message (2), archive (2), pokemon (2), pml (1), font (1) |

These drive the generated PHF registry, the dense index, and the dispatch
table consumed by `crates/pla`.

## Workspace (Rust)

```
Cargo.toml            # workspace: sheetty, sheetty-cli, pla
crates/sheetty        # canonical TSV parser, preflight L0-L3, emitter
crates/sheetty-cli    # `sheetty check <sheets-dir>`
crates/pla            # port crate; build.rs runs preflight + emission
```

- Emission: one `$OUT_DIR/sheets/<sheet>.rs` module per sheet plus
  `index.rs`; writes are gated on content so no-op rebuilds stay clean.
- Evidence sheets (`re/functions`, `re/strings`, `re/callgraph`) carry
  `# emit: false` and stay out of generated code.
- Rust 1.98.0 (already installed) — the MDD's emitter is real and tested;
  Bevy integration is the next port unit.

## Ghidra runs

- `PLA-update` (full analysis): started 21:47, still in `ApplyDataArchiveAnalyzer`
  after 2 h with no completion signal. Left running; project is only saved on
  completion. If it is killed before then, no analysis is retained.
- `PLA-update-capped` (analysis timeout 1 500 s): **used for all artifacts**.
  Timed out at 25 min, saved, then exported evidence sheets and the bounded
  decompilation. This is the reproducible path (`ghidra_capped.log`).
- Base binary `pk1/main.elf` is converted and verified but not yet analysed.

## Parity (gamedb Phase 2)

- Files: **11 993 exported `.c` -> 11 993 indexed (100 %)**, 0 failed
  (7 996 update + 3 997 base).
- Functions: 11 675 indexed. A minority of files contain a line-wrapped
  signature (`undefined8\nFUN_xxxx(...)`) that the gamedb parser does not
  recognize as a function definition. Files are indexed; only the function row
  is missing. Full list via `gamedb sql` (files LEFT JOIN functions IS NULL).
- Round-trip: `gamedb read -r decompiled FUN_0000ac90 --path update-main`
  returns the function body verbatim from the exported source.
- Tranche derivation (reproducible): `triage5k.tsv` ranks 10 000 functions;
  ranks N..M = `sed -n 'N+7,M+7p' triage5k.tsv | cut -f1`. Tranche 4 =
  ranks 6001-8000.

## Version control

- `git init` in `/mnt/dev/decompilacion` (2026-09-29); first work-unit commit
  `7f73a00` — extraction pipeline, NSO tooling, sheet book scaffold, task doc.
  Large artifacts (`pk*.nsz`, `.tools/venv`, exports) are ignored.

## Next bounded work units

1. Container/asset parsers: SARC ✓ GFLXPACK ✓ AHTB ✓ BNTX ✓ (parse + decode)
   VFXB ✓; pending: tr* models/anims, ASTC + 8 unknown-format BNTX files.
2. Real host bindings: replace the recording stubs subsystem by subsystem.
   The event trace names the order: **field system first**
   (`GetFieldObjectForHaxe` + `FieldObjectComponent.IsLoading`/`IsGrounded`),
   then `FindEventData`/`EventProgressManager`, then the command singletons.
3. Visual subsystem: render path needs a window (Bevy 0.19 headless gap).
4. Middleware follow-through: avian3d spike for Havok replacement.
