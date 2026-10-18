# Agent quickstart: run and photograph the game

For AI agents. Full reference: [AUTOMATION.md](AUTOMATION.md). Run everything from the repository root.

## The 5 commands

```sh
.tools/scene.py list                              # prepared scenes + one-line description
.tools/scene.py run village_morning --cpu         # start, play the script, photograph, ALWAYS stop
.tools/gamectl.py start --cpu -- --area 0         # your own session (CPU rendering, no GPU needed)
.tools/gamectl.py step 60 --fast                  # simulate 60 frames, draw only the last one
.tools/gamectl.py shot work/automation/shots/a.png   # PNG: read it back with your image viewer
.tools/gamectl.py stop                            # never leave the viewer running
```

`scene.py run` prints one line per PNG (absolute path + `ok`/`BLANK`) and a summary line
(`panic`, log warnings). Exit status: 0 ok, 1 a step failed or a picture is blank, 3 the viewer died.

## Rules

- **Never open a window.** Only `--automation` (gamectl and scene.py always do that). No `play.sh`.
- **One GPU process at a time**, or use `--cpu` (or `PLA_CPU_RENDER=1`): Mesa lavapipe software
  Vulkan. If it is missing the tool names the package to install: `vulkan-swrast`.
- **Always stop the viewer** (`gamectl.py stop`); `scene.py` does it by itself, also on failure.
- Heavy builds: `systemd-run --user --scope -p MemoryMax=6G ...` and `CARGO_BUILD_JOBS=3`; never `cargo clean`.
- Use your own `--dir work/automation-<name>` when other sessions may be running.

## Where things go

- `scene.py`: PNGs in `work/scenes_out/<scene>/` (change with `--out DIR`); its private
  `work/automation-scene-<name>-<pid>/` is deleted at the end (`--keep` to inspect the log).
- `gamectl.py shot PATH`: wherever you say; `work/automation/shots/` by convention.

## Speed (1280x720, 12 cores)

| CPU (lavapipe) | time |
|---|---|
| start to first answer | about 4 s; the first 20 frames cost about 17 s more (assets load) |
| `step 60` (every frame drawn) | 25.6 s (0.43 s per frame) |
| `step 60 --fast` (`render:last`) | 1.9 s |
| `shot` | about 2 s |

A whole scene takes 1 to 4 minutes on the CPU (the trainer battle is the slowest).

Use `render:last` for every wait; only the last frame of the command is drawn (the simulation of
every frame is identical). Cameras are switched off and back on, so particle billboards and the
battle-UI anchors are rebuilt on the drawn frame, not on the skipped ones.

## Add a scene

Create `scenes/<name>.json` (small, committed). Steps are normal automation commands
([AUTOMATION.md](AUTOMATION.md#commands)); `screenshot` takes `name` (file `<out>/<name>.png`);
`"optional": true` turns a failing step into a warning.

```json
{
 "description": "Area 0 at night, raining.",
 "args": ["--area", "0"],
 "steps": [
  {"cmd": "set_weather", "weather": "rain"},
  {"cmd": "set_time", "slot": 3, "pinned": true},
  {"cmd": "step", "frames": 180, "render": "last"},
  {"cmd": "key", "key": "Escape"},
  {"cmd": "step", "frames": 20, "render": "last"},
  {"cmd": "screenshot", "name": "night_rain_pause"}
 ]
}
```

Then check it: `.tools/scene.py run <name> --cpu`, and look at the PNGs.
