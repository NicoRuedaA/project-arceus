# REPRODUCE — rebuild the whole workspace from your own copy

Everything heavy is **regenerable** from your own game files + keys + the
committed tooling. This document is the exact pipeline, with the parity check
that proves each step.

## 0. Prerequisites

- Your own copy of the game: `pk1.nsz` (base) and `pk2.nsz` (update).
- Your own `prod.keys` (from your console; **never** commit it).
- Rust (1.98+), Python 3.13, `uv`, a JDK (for `unluac`), Ghidra 12.x.

```bash
uv venv .tools/venv
uv pip install --python .tools/venv/bin/python "git+https://github.com/nicoboss/nsz.git" lz4
mkdir -p .tools/lua && curl -sL -o .tools/lua/unluac.jar \
  https://sourceforge.net/projects/unluac/files/latest/download
```

## 1. Containers → decrypted entries (`ncz_extract.py`)

NSZ/NCZ embeds the section keys, so this step needs **no keys**:

```bash
.tools/venv/bin/python .tools/ncz_extract.py pk1.nsz /home/nico/work/pla/pk1
.tools/venv/bin/python .tools/ncz_extract.py pk2.nsz /home/nico/work/pla/pk2
```

**Parity:** the `nsz` tool's own reconstruction reproduces the content-addressed
NCZ filenames (SHA-256 of the reconstructed NCA == the entry name prefix).

## 2. Executables (`nso_to_elf.py`)

```bash
.tools/venv/bin/python .tools/nso_to_elf.py <exefs>/main <work>/main.elf
```

**Parity:** the converter verifies the NSO's embedded SHA-256 for every segment.

## 3. Static analysis (Ghidra headless)

```bash
/opt/ghidra/support/analyzeHeadless <projects> PLA-update-capped \
  -import main.elf -processor AARCH64:LE:64:v8A -analysisTimeoutPerFile 1500 \
  -scriptPath .tools/ghidra_scripts -postScript ExportTsv.java <exports-dir>
```

Then, for a bounded decompilation (recommended — a 53 MB `.text` never finishes
a full auto-analysis):

```bash
.tools/venv/bin/python .tools/sheetty.py triage <exports>/functions.tsv <exports>/triage.tsv --top 6000
# take the addresses you want, then:
analyzeHeadless ... -process main.elf -noanalysis -readOnly \
  -postScript ExportTop.java <decompiled-dir> <addresses.txt> [timeout-seconds]
```

`ExportTop.java` takes an optional per-function timeout (default 120 s) and logs
each failure as `EXPORT-TOP-FAIL <address> <decompiler error>`. The update-main
export covers every inventory address: 68,327 of 68,330 functions decompile.
For the functions the decompiler rejects, export the disassembly instead, into a
directory outside the gameDB root:

```bash
analyzeHeadless ... -process main.elf -noanalysis -readOnly \
  -postScript ExportAsm.java <asm-dir> <failed-addresses.txt>
```

See [`reports/function-progress/update-main-full-pseudocode-export.md`](reports/function-progress/update-main-full-pseudocode-export.md).

## 4. Base RomFS (`romfs_extract.py`)

```bash
.tools/venv/bin/python .tools/romfs_extract.py <base-section>.bin \
  --list <work>/romfs_files.tsv
.tools/venv/bin/python .tools/romfs_extract.py <base-section>.bin \
  --extract <work>/romfs
```

**Result:** 18 370 files / 6.4 GB. The section is an IVFC-wrapped RomFS whose
tables sit *after* the data region; the tool locates the header by its
structural chain.

## 5. Update RomFS (`bktr_extract.py`) — needs keys

Game updates ship an AesCtrEx/BKTR patch. The tool rebuilds the **virtual**
(patched) RomFS from base + patch:

```bash
.tools/venv/bin/python .tools/bktr_extract.py pk2.nsz <pk2-sections-dir> \
  <base-section>.bin 0xC04000 <work>/romfs_patched --list <work>/romfs_patched.tsv
```

**Parity:** every file the patch does *not* touch is byte-identical to the base
extraction (400/400 sampled). The update adds 725 files and changes 465.

## 6. Auxiliary NCAs (keys + ticket)

The control/meta/manual NCAs (icons, NACP, manual) decrypt with `prod.keys`; a
rights-managed NCA additionally needs the title key from the shipped ticket
(`titlekek` + `Keys.decryptTitleKey`). See the session log in `odd/tasks/` for
the exact recipe.

## 7. Lua decompilation (`decompile_lua.py`)

```bash
.tools/venv/bin/python .tools/decompile_lua.py <work>/romfs <work>/lua --jobs 8
```

**Result:** 799/800 event scripts decompiled to readable Lua (253 MB). The
**shipped bytecode executes**; the decompiled Lua is for reading (unluac output
diverges in at least one runtime edge).

## 8. Index + evidence sheets

```bash
cargo build --manifest-path gamedb/Cargo.toml --release   # gamedb submodule
./gamedb/target/release/gamedb index -r decompiled -v 2000
./gamedb/target/release/gamedb stats  -r decompiled
```

The evidence sheets (`sheets/re/functions.tsv`, `callgraph.tsv`, `triage.tsv`,
`structs.tsv`, `fields.tsv`) are committed; `sheets/re/strings.tsv` is **not**
(it contains game text).

## 9. Sheet book

```bash
cargo run -p sheetty-cli -- check sheets     # 0 errors, 0 warnings
```

The `domain/*` sheets that come from Game Freak AHTB tables are generated
(never hand-edited):

```bash
U=<work>/romfs_patched/bin/flagwork
.tools/venv/bin/python .tools/ahtb_sheet.py $U/event_flags.tbl domain/event_flags \
  sheets/domain/event_flags.tsv --requires domain/asset_formats
for t in event_works map_flags map_works phase_works system_flags system_works; do
  .tools/venv/bin/python .tools/ahtb_sheet.py $U/$t.tbl domain/$t sheets/domain/$t.tsv
done
```

The flagwork tables come from the **update** RomFS (the port's target build):
450 event flags, 45 event works, 70 map flags, 24 map works, 8 phase works,
704 system flags, 106 system works.

## Test fixtures

To run the fixture-based tests, provide these files (from your extraction) via
`PLA_FIXTURES=<dir>` or `crates/pla/tests/fixtures/`:

| Fixture | Source in your extraction |
|---|---|
| `boot_bg.arc`, `opapp_01.arc`, `titlemenu_00.arc` | any small SARC from the base RomFS (`/bin/appli/...`) |
| `locators.gfpak`, `ha_area00_s06.gfpak` | small GFLXPACK from the base RomFS |
| `event_flags.tbl`, `event_works.tbl`, `map_flags.tbl`, `map_works.tbl`, `phase_works.tbl`, `system_flags.tbl`, `system_works.tbl` | `bin/flagwork/*.tbl` from the **update** RomFS |
| `*.tbl.expected.tsv` | regenerate with `.tools/ahtb_sheet.py --expected` (name<TAB>id) |
| `icon_ball.bntx`, `pokeicon.bntx` | small BNTX from the base RomFS |
| `particle.ptcl` | a `.ptcl` that embeds a BNTX (1742 of 1748 do) |
| `event_sample.blua` | any event script (`bin/haxe/release/event/*.blua`) |
| `bag_pocket.tbl`, `bag_pocket.dat` | `bin/message/English/common/` (message key table + payload) |
| `event_list.bin` | `bin/event/event_progress/event_list.bin` (event name pool) |
| `probe_rg32f.bntx` | `system_resource/gfx2/texture/default_diffuse_prb.bntx` (HDR float cubemap) |
| `*.expected.tsv` | regenerate with the Python parsers (name/size/SHA-256) |
| flagwork `*.tbl.expected.tsv` | `.tools/ahtb_sheet.py <table> <sheet> <out> --expected <fixture>` |

## Parity summary

| Check | Result |
|---|---|
| NCZ reconstruction vs content-addressed filename | match |
| NSO segment SHA-256 | match |
| SARC/AHTB/BNTX fixtures vs Python parsers | match |
| BKTR unpatched files vs base extraction | 400/400 identical |
| gamedb indexed files vs exported `.c` files | 100% |
