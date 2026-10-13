# Install and play (players' guide)

**English** · [Español](INSTALL.es.md)

One command turns **your own** copy of *Pokémon Legends: Arceus* into everything the
port needs. Then `./play.sh` starts the game. This repository contains no game files:
you bring the game and the keys from your own console, and everything that is extracted
or generated stays on your machine under `work/` (git ignores it).

## Quick path

1. Install the tools once (see [Requirements](#requirements)).
2. Run the installer with the paths to **your** files:

   ```bash
   ./install.sh --base /path/to/pk1.nsz --update /path/to/pk2.nsz --keys /path/to/prod.keys
   ```

   It remembers the three paths, so later runs are just `./install.sh`.
3. Play:

   ```bash
   ./play.sh --area 1
   ```

Want to see what would happen first? `./install.sh --check` changes nothing and prints
every step with the exact command it would run.

## What you need

| Item | What it is | Notes |
|---|---|---|
| `pk1.nsz` | the **base game**, v0 | 2,334,586,382 bytes; checked by SHA-256 |
| `pk2.nsz` | the **update** v1.1.1 (v262144) | 52,657,467 bytes; checked by SHA-256. The update is a patch: the base is always needed too |
| `prod.keys` | the keys dumped from your own console | only its `header_key` line is used, to read the update's patch table |

Only these exact dumps are supported: the port and its data tables target them. Other
revisions, `.nsp` or `.xci` dumps are refused with a message saying why.

### Requirements

| Tool | Why | Check |
|---|---|---|
| Python 3.10+ (3.13 recommended) | runs the installer | `python3 -V` |
| `uv` (or your own venv) | creates `.tools/venv` with the extraction packages (`nsz`, `zstandard`, `pycryptodome`, `lz4`) | `./install.sh --setup-python` does it for you (needs internet once) |
| Rust 1.98+ (`cargo`) | builds the viewer and the cache tools | `cargo -V` |
| ALSA and udev headers | the viewer's sound and gamepad support | Debian/Ubuntu: `libasound2-dev libudev-dev`; Arch: `alsa-lib systemd-libs` |
| libopus | only for the audio step | Debian/Ubuntu: `libopus-dev`; Arch: `opus` |

Not needed to play: Ghidra, a JDK or the decompiled code (they are for reverse
engineering only, see [`REPRODUCE.md`](../REPRODUCE.md)).

**Disk space:** about 21 GB for a full install, measured: data folder 6.4 GB,
decompressed base sections 6.4 GB, caches 6.2 GB (audio 4.7 GB of them) and the Rust
release build folder `target/` 2.2 GB. `--no-audio` saves ~4.7 GB and `--prune-sections`
deletes the 6.4 GB of base sections once the data folder has been verified.

## What the installer does

Each step runs only when its output is missing or out of date, so a second
`./install.sh` finishes in under a second. Times are from one measured run on the
maintainer's machine (see the [installer report](../reports/data-progress/installer.md));
yours will differ. A full first install took about 31 minutes there (12 of them compiling).

| Step | Makes | Reads | Measured time | Size |
|---|---|---|---|---|
| (build) | `target/release/examples/…` | Rust sources | 12 min first time, seconds after | 2.2 GB |
| `base-sections` | `work/pla/pk1/sections/…` | your `pk1.nsz` | 45 s | 6.4 GB |
| `update-sections` | `work/pla/pk2/sections/…` | your `pk2.nsz` | 12 s | 92 MB |
| `romfs` | `work/pla/pk2/romfs_patched/` (19,095 files, base + update) | both sections + your keys | 1.7 min (resumed run, see report) | 6.4 GB |
| `main-elf` | `work/pla/pk2/main.elf` | the update's program | 1 s | 70 MB |
| `terrain-albedo` | `work/cache/terrain_albedo/` | data folder | 51 s | 171 MB |
| `terrain-normals` | `work/cache/terrain_normals/` | data folder | 13 s | 45 MB |
| `minimap` | `work/cache/minimap/` | data folder + ground colour | 10 s | 12 MB |
| `field-models` | `work/cache/gltf_field/` | data folder | 2.7 min | 460 MB |
| `pokemon-models` | `work/cache/pokemon_batch/` | data folder | 10.7 min | 839 MB |
| `player-model` | `work/cache/player/` | data folder | 16 s | 11 MB |
| `audio` | `work/cache/audio/` | data folder + `main.elf` | 43 s | 4.7 GB |
| `terrain-layers` (off by default) | `work/cache/terrain_layers/` | data folder | not measured | only for editing improved ground textures |

Checks along the way: both game files must match their pinned SHA-256; the rebuilt data
folder must match the expected file table (19,095 files, 6,420,119,516 bytes) and a
pinned digest of every file's content; `main.elf` must match its pinned SHA-256.

## Safety

- Nothing is ever written *through* a symbolic link: every file is written under a
  temporary name and renamed into place.
- Each step works in `work/.install-staging/<step>` and moves its output into place only
  when it finished, so an interrupted run never leaves half a cache. Run `./install.sh`
  again to resume.
- Heavy commands run under `systemd-run --user --scope -p MemoryMax=6G` when available
  (`--memory-max 4G` to lower it, `--no-memory-limit` to disable it).
- Outputs that already exist but were not made by the installer are kept as they are
  (`present` in `--check`). `--force STEP` rebuilds one step; `--force all` everything.
- Two installers on the same `work/` folder are refused.

## Useful options

| Option | Effect |
|---|---|
| `--check` | change nothing; list the steps and exact commands; exit 0 = ready, 3 = steps pending, 2 = something to fix |
| `--only a,b` / `--skip a,b` | run only / never these steps |
| `--force a,b` / `--force all` | redo these steps |
| `--no-audio` | skip audio (the game plays silently) |
| `--with-terrain-layers` | also export the ground layer textures you can improve |
| `--setup-python` | create `.tools/venv` with `uv` and install the extraction packages |
| `--python PATH` | use another Python that already has those packages |
| `--prune-sections` | delete the decompressed base sections after the data folder is verified |
| `--work DIR` | install into another folder (for testing only: the viewer reads `<repo>/work`) |

The full log of every run is in `work/install-state/install.log`.

## When something goes wrong

| Message | What to do |
|---|---|
| `missing --base / --update / --keys` | pass the path to your own file |
| `expected N bytes, got M` | that is not the supported dump (or `--base`/`--update` are swapped: the message says so) |
| `right size but a different SHA-256` | the dump is damaged or a different revision: dump it again |
| `has no 'header_key' line` | pass `prod.keys`, not `title.keys` |
| `not enough free disk space` | free space or use `--no-audio` / `--prune-sections` |
| `killed, most likely by the memory limit` | close other programs or raise `--memory-max` |
| `./play.sh` says the game data is not installed | run the `./install.sh …` command it prints |

`./play.sh` checks the install before starting (without writing anything): without the
data folder it stops and prints the exact `./install.sh` command; when only some caches
are missing it names them and starts anyway (the viewer then bakes or falls back on its
own, more slowly).

## What is verified

See [`reports/data-progress/installer.md`](../reports/data-progress/installer.md) for what
was run end to end, what was only dry-run, and what is still unknown. The installer has
not been tried on another person's machine.
