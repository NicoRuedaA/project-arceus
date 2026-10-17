# Automation mode (remote control of the viewer)

`--automation` lets a script, or an AI agent that can run shell commands and read PNG files,
drive and observe the **real** port viewer without any window. It exists so play-testing bugs can
be reproduced and inspected without asking the user for screenshots.

Status: a test tool, not game content. It adds no port coverage (see the README row
"Automation / remote control").

## Guarantees

| Topic | What the mode does |
|---|---|
| Window | **Never creates one.** `WindowPlugin { primary_window: None, exit_condition: DontExit }`, `WinitPlugin` and `GilrsPlugin` disabled: no window, no event loop on the user's compositor, no keyboard/mouse/gamepad capture. A test (`automation_mode_registers_no_window`) asserts zero `Window` entities. |
| Rendering | The 3D camera (and the UI, via `IsDefaultUiCamera`) render into an offscreen image on the GPU (wgpu/Vulkan without a display). If no adapter is found, or with `--automation-no-gpu`, the app runs **logic-only** and `screenshot` answers `unavailable`. |
| Frames | A custom runner replaces winit: frames advance **only** when a command asks for them, each by exactly 1/60 s (`TimeUpdateStrategy::ManualDuration`). Same commands from the same start give the same frames (asset loading timing aside). |
| Audio | Always silent (`AudioMode::Null`, no audio device is opened). |
| Save and settings | Without `--save-dir` the run uses a scratch save in `work/automation/save/` and starts a new game; settings live in `work/automation/settings/`. The player's own save and settings are never read or written. |
| Socket | Unix domain socket `work/automation/<pid>.sock`, mode **0600**, directory `work/automation/` mode **0700** (git-ignored). Never TCP. Removed when the viewer quits. |
| Crashes | A panic hook writes message + backtrace to `work/automation/last_panic.txt`. A panic during a command is answered on the socket (`{"ok":false,"error":"panic","panic":"..."}`) and the process exits with status 101. `gamectl.py` prints the panic and the log tail whenever the viewer is gone. |

Security: off by default; reachable only through a file socket owned by the current user; it can
only act on the game (no shell, no file reads besides its own log; `screenshot` writes the PNG
path it is given).

## Quick start

From the repository root (`PLA_BIN_ROOT` / `PLA_CACHE_ROOT` as for `play.sh` when the defaults do
not fit):

```sh
.tools/gamectl.py start -- --area 1 --trainer giratina01   # builds, starts, waits for the socket
.tools/gamectl.py step 300                                 # 5 game seconds
.tools/gamectl.py state                                    # zone, player, clock, battle, counts...
.tools/gamectl.py shot work/automation/shots/area1.png     # offscreen PNG (read it back)
.tools/gamectl.py cmd '{"cmd":"key","key":"V","frames":3}'
.tools/gamectl.py log 40                                   # last viewer log lines
.tools/gamectl.py stop                                     # quit (SIGTERM if it does not answer)
```

Smoke scenario (area 01, time slots, every Esc-menu screen, save/load, wild battle to the end,
trainer `giratina01` via V, PASS/FAIL per step, panic text, PNG list in `work/automation/shots/`):

```sh
.tools/smoke_viewer.py              # GPU offscreen
.tools/smoke_viewer.py --no-gpu     # logic only; screenshots must answer "unavailable"
.tools/smoke_viewer.py --no-build   # reuse the current build
```

Exit status of both tools: `0` ok, `1` a command answered `ok:false` / a step failed, `3` the
viewer is dead (crash report printed).

## Viewer flags

| Flag | Meaning |
|---|---|
| `--automation` | Turn the mode on (required; off by default). |
| `--automation-dir DIR` | Working directory (default `work/automation`). |
| `--automation-socket PATH` | Socket path (default `<dir>/<pid>.sock`). |
| `--automation-no-gpu` | Logic-only (also `PLA_AUTOMATION_NO_GPU=1`). |
| `--automation-size WxH` | Offscreen size (default `1280x720`, 16..4096). |
| `--automation-log FILE` | File holding the viewer's stdout/stderr, read by `log_tail` (also `PLA_AUTOMATION_LOG`; `gamectl.py` sets it). |

Every other viewer flag works as usual (`--area`, `--player-pos`, `--trainer`, `--time-slot`, `--ui debug` (the original UI is the default),
`--weather`, `--style`, `--wild-behavior`, `--save-dir`, ...).

## gamectl.py

| Subcommand | Sends / does |
|---|---|
| `start [--no-gpu] [--release] [--no-build] [--bin PATH] [--size WxH] [--timeout S] -- ARGS` | `cargo build` (in a `systemd-run` scope, 6 GB cap), starts `port_viewer --automation ARGS` detached with its output in `work/automation/viewer-<time>.log`, waits for the socket, records `work/automation/current.json`. |
| `cmd JSON` | Any command below, raw. |
| `ping`, `state`, `save`, `load`, `quit` | Same-named command. |
| `step N` / `wait S` | `step` / `wait_seconds`. |
| `key K [--frames N] [--action press\|down\|up]` | `key`. |
| `mouse [X Y] [--wheel N] [--button left\|right\|middle] [--action move\|press\|release\|click]` | `mouse` (virtual pointer: there is no window to move a cursor in; see below). |
| `shot PATH [--width W --height H] [--yaw D --pitch D --distance M]` | `screenshot` (angles in degrees). |
| `entities [FILTER] [--limit N]` | `entities`. |
| `log [N]` | `log_tail`. |
| `status` | Session file + whether the process is alive (crash report when dead). |
| `stop` | `quit`, waits, SIGTERM if needed. |

## Protocol

One JSON object per line in, one per line out. Every reply has `ok` and, on success, `frame` (the
frame counter after the command). Errors: `{"ok":false,"error":"..."}`; nothing is changed by a
command that answers an error, unless the error says otherwise. Bad JSON or unknown fields values
are errors, never crashes.

### Commands

| Command | Fields | Effect / reply |
|---|---|---|
| `ping` | | `pong`, `pid`, `gpu`, `uptime_s`. |
| `state` | | `game_state` (World/Battle/Loading/MainMenu/BattleEnded), `next_state` (a transition requested this frame), `paused`, `zone_loading`, `zone` {area, name}, `player` {position, yaw_deg, swim, motion}, `ride` {mounted, selected, stamina, transition, camera_bonus}, `clock` {time, minutes, slot, pinned}, `weather` {name, id, forced}, `progress` {party [dex, level, hp], box_count, bag_kinds, bag_items, money, seen, caught}, `battle` (null or turn, trainer, sides with HP, menu, last log lines), `toast`, `open_screens`, `counts` {wild_creatures, wild_with_real_model, wild_placeholder, npcs, npcs_with_model_source, trainer_markers, pickups, meshes, entities}, `frames` {frame, game_dt_ms, real_update_ms_avg, real_fps}, `gpu`. |
| `step` | `frames` (default 1, max 100000) | Runs that many frames. |
| `wait_seconds` | `seconds` | `round(seconds*60)` frames. |
| `key` | `key`, `action` = `press` (default) / `down` / `up`, `frames` (default 1) | Injected as a real `KeyboardInput` message. `press`: down, `frames` frames, up, 1 frame. Keys: letters, digits, `F1`-`F12`, `Escape`/`Esc`, `Enter`, `Space`, `Tab`, `Backspace`, `Shift`, `Ctrl`, `Alt`, `Up`/`Down`/`Left`/`Right`, `[`/`]` (bag quantity), or Bevy names (`KeyV`, `Digit1`, `ShiftLeft`). |
| `mouse` | optional `x` + `y` (window pixels of the offscreen size), `wheel` (-50..=50 notches, positive = down), `button` = `left`/`right`/`middle`, `action` = `move` (default) / `press` / `release` / `click`, `frames` (default 1) | Drives the port's scrollbars (`orig_ui::scrollbar::VirtualPointer`): there is no window in automation mode, so the pointer is virtual (the middle of the viewport until the first `x`/`y`). `x`/`y` move it (missing coordinate keeps its value), `wheel` scrolls the list under it, `click` presses and releases with `frames` frames between, then `frames` more run. Reply: `pointer`, `wheel`, `button`, `action`, `stepped`. |
| `teleport` | `x`, `z` | Moves the player; off the navmesh it snaps to the nearest walkable face (400 m); Y from navmesh/terrain + 0.9. No frame is run. |
| `teleport_zone` | `area` (0-99), optional `x`,`z` | Real zone change (`ZoneChangeRequest`) to `ha_areaNN` at its fast-travel arrival (or x,z); runs frames until the load finishes (max 3600). World state only. |
| `set_time` | `minutes` (0-1439) **or** `slot` (0-3), optional `pinned` | Sets the game clock; `pinned:true` stops it. |
| `set_weather` | `weather` = 2..9, a name (`sunny`, `cloudy`, `rain`, `snow`, `drought`, `fog`, `rainstorm`, `snowstorm`) or `auto` | Forces the weather (`auto` returns to the lottery). |
| `give_item` | `id`, `count` (default 1) | Adds to the bag (debug). |
| `add_pokemon` | `species` (battle-data name, e.g. `bidoof`), `level` | Adds a member to the party (the box when full) (debug). |
| `open_screen` | `screen` = `pause`, `bag`, `party`, `box`, `dex`, `crafting`, `shop`, `graphics`, `close` | Raises the same request the Esc menu raises (pause: opens the Esc menu); `close` presses Escape until nothing is open. Replies `open_screens`. |
| `start_battle` | `wild` = species name + `level` (+ optional `alpha`: true, the arena opponent gets the alpha aura), **or** `wild` = `"nearest"`, **or** `trainer` = id (`giratina01`, `noble:<setting>`) | Wild species: spawns the creature 1.5 m in front of the player and starts its session directly (same call as the viewer's encounter). `nearest`: moves the player next to the nearest loaded wild creature and lets the viewer's encounter system start it. Trainer: validates the id, sets it as the forced trainer and presses **V** (a trainer marker within 3 m takes priority, as in the game). Runs up to 60 frames until `GameState::Battle`. World state, not paused. |
| `force_alpha` | one of `count` (the N nearest wild creatures, default 1 when nothing is given), `all`: true, `off`: true | Debug: makes wild creatures alpha (2.5x scale and the red aura of `game::alpha_aura`) or undoes it. The aura builds a few frames later, once the model is loaded. `entities wild` lists `alpha` per creature. Same as the viewer flag `--force-alpha [N\|all]`. |
| `save` | | Same as F5 (refused in battle). Replies the file path and message. |
| `load` | | In-process reload of the save file: party, bag, money, Pokédex, clock, play time and the player's pose (another zone goes through a zone change). Not a process restart. |
| `screenshot` | `path` (`.png`), optional `width`+`height`, optional `camera` {`yaw`, `pitch`, `distance`, `focus`:[x,y,z], `degrees`:bool} | Captures the main camera's offscreen target (the UI included). `camera` places an orbit camera around the player (or `focus`) for this shot only. Reply: `width`, `height`, `stats` {mean_rgb, luma_mean, luma_std, `looks_blank`}, frames run (2-5). Logic-only: `unavailable`. |
| `entities` | `filter` = `wild`, `npc`, `trainer`, `player`, `camera`, `follower`, `named`; `limit` (default 50) | Nearest first: entity id, position, distance, visibility (`Visibility` and `in_view`), plus kind fields (species, level, real_model, trainer id, NPC key...). `npc` adds the look-at state (`look_yaw_deg`, `gaze_deg`, `head_yaw_deg`, `trunk_yaw_deg`, `look_bones`); `follower` adds the follower state (`state_pos`, `speed`, `yaw_deg`, `walked`, `teleports`, `trail`, `anim_source`). |
| `log_tail` | `lines` (default 50) | Last lines of the viewer log and how many look like warnings/errors. |
| `graphics` | `preset` (`default`, `switch`, `low`, `medium`, `high`, `ultra`) and/or `set` {`option_id`: value} (the `graphics.toml` keys and values, e.g. `{"anti_aliasing":"smaa","render_scale":150}`) | Changes Graphics screen options live, exactly as the screen does (the appliers run; restart-only options are stored but act on the next start). The file is not written. Reply: `preset` (or null = custom) and the full settings text. |
| `bench` | `frames` (default 300), `warmup` (default 60) | Frame timing: runs `warmup` frames, then `frames` frames, each followed by a wait until the GPU has finished it (`RenderDevice::poll(Wait)`). Reply: `frame_ms_avg`/`p50`/`p95`/`p99`/`max`, `cpu_ms_avg` (the update alone), `gpu_wait_ms_avg` (time the GPU still needed after the update), `fps_avg`. Without overlap between CPU and GPU, so it is an upper bound of a pipelined windowed frame. The FPS cap option sleeps inside the update, so a cap of 120 shows as about 8.33 ms here (that is how the cap is measured offscreen). |
| `quit` | | Replies, then the viewer exits (status 0) and removes its socket. |

### Examples

```sh
G=.tools/gamectl.py
$G cmd '{"cmd":"set_time","slot":3,"pinned":true}'
$G cmd '{"cmd":"set_weather","weather":"rain"}'
$G cmd '{"cmd":"teleport","x":560,"z":420}'
$G cmd '{"cmd":"teleport_zone","area":2}'
$G cmd '{"cmd":"start_battle","wild":"bidoof","level":5}'
$G key F; $G key 1; $G step 150            # Fight, first move, let the turn play
$G cmd '{"cmd":"start_battle","wild":"haunter","level":30,"alpha":true}'   # alpha aura in the arena
$G cmd '{"cmd":"force_alpha","all":true}'                                    # every wild creature alpha
$G cmd '{"cmd":"start_battle","trainer":"giratina01"}'
$G cmd '{"cmd":"open_screen","screen":"party"}'; $G shot work/automation/shots/party.png
$G cmd '{"cmd":"open_screen","screen":"close"}'
$G mouse 640 360 --wheel -3                # pointer over the list, wheel up three notches
$G mouse 1181 320 --button left --action press
$G mouse 1181 700 --button left --action release   # drag a scrollbar knob in two steps
$G mouse 1181 500 --button left            # --button without --action = click
$G shot work/automation/shots/orbit.png --yaw 45 --pitch 30 --distance 7
$G entities npc --limit 5
$G cmd '{"cmd":"give_item","id":17,"count":5}'
$G cmd '{"cmd":"add_pokemon","species":"shinx","level":12}'
$G cmd '{"cmd":"graphics","preset":"high"}'
$G cmd '{"cmd":"graphics","set":{"vsync":"off","fps_cap":"120"}}'
$G cmd '{"cmd":"bench","frames":300,"warmup":120}'
```

Raw socket without gamectl (Python one-liner):

```sh
python3 -c 'import socket,sys; s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); s.sendall(b"{\"cmd\":\"ping\"}\n"); print(s.recv(4096).decode())' work/automation/<pid>.sock
```

## Limits (honest)

- **Not a pixel-exact replay.** Frames and game time are deterministic; asset loading still
  happens on its own schedule, so a model can appear one frame earlier or later between runs.
- **`load` is in-process**, not a restart: systems that only read the save at start-up (for
  example world events seeded at start) keep their state.
- **`start_battle` with a species** starts the session directly instead of walking up to a
  creature; the battle itself is the viewer's own code.
- **Mouse and gamepad are not injected** (keys only). The camera can be placed for a screenshot
  with `camera`, not moved by the mouse.
- **One frame target.** Screenshots capture the main 3D camera and the UI on it; the full-screen
  map (`M`) and other cameras are not covered.
- **Linux/Unix only** (Unix domain socket).
- The automation tests run logic-only (no GPU) so they work anywhere; the GPU path is checked by
  running the smoke scenario.

## Tests

- `cargo test -p pla --lib automation`: command parsing (every command and many bad inputs), key
  names, config flags, scratch-save arguments, orbit camera, blank-frame stats, private 0600 socket
  with line framing.
- `cargo test -p pla --example port_viewer`: `automation_mode_registers_no_window` (the app as
  `main` builds it: no winit, no gilrs, zero `Window` entities, fixed frame time, every schedule
  initialises), `socket_session_ping_step_state_quit` (the real runner with a real client: ping,
  step, state, an unknown command, an unavailable screenshot, quit; the socket file is removed),
  plus the existing start-up schedule guard.
