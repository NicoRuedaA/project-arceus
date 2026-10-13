# Gamepad input support (`feat/gamepad`)

Brief 3 of `docs/briefs-agentes.md` (worktree `gamepad`, branch `feat/gamepad`, base `main@db40a03`).
Default Switch-style mapping for the Bevy port, pure `ActionState` input layer, and the real
`bevy_gilrs` backend (maintainer decision: crates.io is reachable, no evdev reader was written).

## What was built

1. **`crates/pla/src/game/input_map.rs` (new).** `enum Action`, the pure mapping
   `actions(&RawInput) -> ActionState` (radial stick deadzone `0.15` + smoothstep response curve,
   keyboard-wins-per-axis priority), the `ActionState` resource with `move_axis()`/`look_axis()`/
   `zoom()`/`pressed()`/`just_pressed()`/`consume()`, and `InputMapPlugin`: a `PreUpdate` system
   (`fill_action_state`, after `bevy::input::InputSystems`) that snapshots `ButtonInput<KeyCode>`
   and every `Gamepad` component into `ActionState`.
2. **Consumers replaced.** `player_locomotion` and `camera_controls` (`port_viewer.rs`),
   `battle_input_system` (`game/battle_input.rs`, `map_keys` became `map_actions`) and the pause
   menu (`game/pause_menu.rs`) read `Res<ActionState>`. Exception (merge fix): the pause menu
   still reads W/S directly so they keep navigating the vertical menu, as before the action layer.
   `app.add_plugins(pla::game::input_map::InputMapPlugin)` is one line in the viewer.
3. **Backend.** `bevy_gilrs` added to the `bevy` features in `crates/pla/Cargo.toml` (existing
   features untouched); `Cargo.lock` committed (`gilrs 0.11.2`, `gilrs-core 0.6.8`,
   `libudev-sys 0.1.4`, `inotify 0.11.5`). `GilrsPlugin` joins `DefaultPlugins` automatically
   with that feature. Linux build needs `libudev` dev files: verified present here
   (`pkg-config --exists libudev` exit 0, `/usr/include/libudev.h` exists).
4. **Pause-menu edge consumption.** The menu consumes `Menu`/`Cancel` plus its navigation and
   confirm edges from `ActionState`, and still clears the raw `Enter`/`Space` keys so the systems
   that keep raw keyboard readers (creature encounter, ball throw) do not fire on an unpause
   frame. This preserves the old "Esc opens the menu and the battle does not see Back" contract.

## Default mapping (Switch-style)

`KEY` rows reproduce the old viewer bindings; `_` means not mapped. Confidence per row in the
sections below.

| Action | Keyboard | Gamepad | Notes |
|---|---|---|---|
| Move | WASD / arrows | Left stick | stick keeps deflection → analog walk speed |
| Look | Q / E (yaw) | Right stick (yaw + pitch) | mouse stays raw in the camera system |
| Zoom | R (closer) / F (farther) | _ | |
| Sprint (held) | Shift | L3 (left stick click) | original: L3 sprint toggle (used as hold here) |
| Aim toggle | Z / Tab | ZL | viewer toggles orbit/aim mode; original ZL is aim-hold |
| Dodge | _ | Y | no dodge system exists in the port yet (mapped, unconsumed) |
| Throw | _ | ZR | ball throw still uses mouse / Space (unchanged) |
| Confirm | Enter / NumpadEnter / Space | A (south) | Space now also confirms in battle menus |
| Cancel | Escape / Backspace | B (east) | |
| Menu (pause) | Escape | Plus (start) | |
| Prev / Next | Left, Up / Right, Down / A / D | D-pad left, up / right, down | |
| Battle move slots 1–4 | Digit1..Digit4 | _ | |
| Battle Fight / Switch / Item | F / S / I | Y / X / L | X = swap item/Pokémon matches the original |
| Battle Capture / Run | C / B / R | ZR / R | ZR = throw Poké Ball matches the original |

Original-button evidence (design reference only): `reports/function-progress/n2-input-and-rumble.md`
§2 maps A=confirm, B=cancel, X=swap item/Pokémon, Y=dodge roll, L3=sprint, ZL=lock-on/aim,
ZR=throw Poké Ball, dpad = 4 directions.

## Verification

- `cargo test -p pla input_map` → **12 passed, 0 failed** (`game::input_map::tests::*`).
- `cargo test -p pla battle_input` → **5 passed, 0 failed**.
- `cargo test -p pla pause_menu` → **8 passed, 0 failed** (2 new headless Bevy tests; on the
  branch, before the merge into `main`).
- `cargo test -p pla --lib` → **389 passed, 0 failed**.
- `cargo test -p pla` (all 15 targets) → **478 passed, 0 failed, 2 ignored**.
- `cargo build -p pla --example port_viewer` → builds (the viewer was **not** run: never opened).
- `cargo clippy -p pla --lib --tests --example port_viewer` → **no warning points to `input_map.rs`
  or `pause_menu.rs`**; the crate's pre-existing warnings (320 lib, 24 viewer) are unchanged.
  The 2 warnings in `battle_input.rs` (`too_many_arguments 8/7`, redundant `as u32`) predate this
  branch; both functions keep their previous argument counts.
- Measured curve numbers (unit tests): magnitude 0.10 and 0.15 → **0**; 0.16 → **0.00041**;
  full deflection → **1.0**; diagonal (1,1) clamped to unit length → **(0.7071, 0.7071)**;
  monotonic over 0.16..1.00.
- Headless Bevy tests spawn a synthetic `Gamepad` component (not gilrs events) and assert
  `ActionState` after `app.update()`; the pause tests step a real schedule and prove the consumed
  edges are gone for later systems in the same frame.

## Verified

- All unit and headless results above, on this machine, from the commands listed.
- Keyboard behaviour is preserved for movement, look, zoom, sprint and every battle key, except:
  Space now also confirms in battle menus, and A/D now also navigate the pause menu (W/S, arrows
  and the d-pad do too; W/S were restored at merge time and covered by a headless test).
- Deadzone, curve, keyboard-over-stick priority and multi-source edge dedupe behave as tested.
- The project compiles and links with `bevy_gilrs` + `gilrs`/`libudev-sys` (build, not runtime).

## Assumed

- Every gamepad button/axis assignment above not backed by the original report (Fight=Y,
  Item=L, Run=R, Menu=Plus, zoom unmapped, ZL as a toggle) is a design choice, not decoded data.
- Right-stick signs: up = look up (negative mouse-delta-y); left-stick up = forward. Chosen to
  match the keyboard/mouse conventions; only a real pad can confirm.
- Analog feel: 0.15 deadzone and smoothstep curve are plausible, not the original's numbers
  (the original's analog handling was not decoded here).
- `Gamepad::just_pressed` edges from the gilrs path arrive as expected once per frame;
  verified only against synthetic components.

## Unknown

- Real-gamepad behaviour on the maintainer's machine: device recognition and button mapping done
  by gilrs/SDL, and whether the assumed signs/feel are right. This is the maintainer's test.
- The original's exact stick curve/sensitivity and its hold-vs-toggle semantics (L3 sprint, ZL aim).
- HD rumble / vibration is **not implemented** (out of scope here); `n2-input-and-rumble.md`
  documents the original patterns if it is picked up later (`bevy_gilrs` exposes
  `GamepadRumbleRequest`).

## Maintainer checks (not done here)

1. Plug a pad and run the viewer: move/look with both sticks, L3 sprint, ZL aim toggle, Y/ZR
   (mapped but unused by gameplay), D-pad + A in the pause menu, + to pause, A/B/X/Y/L/ZR/R in
   battle (Fight/Switch/Item/Capture/Run).
2. Confirm stick direction signs (look up/down, walk direction) and that a half-pushed stick
   walks slower.
3. Confirm A/D also moving through the (vertical) pause menu is acceptable.

## Files

- `crates/pla/Cargo.toml`, `Cargo.lock` — `bevy_gilrs` feature + locked dependency tree.
- `crates/pla/src/game/input_map.rs` — new pure mapping + plugin + tests.
- `crates/pla/src/game/mod.rs` — `pub mod input_map;`.
- `crates/pla/src/game/battle_input.rs` — `map_actions`, system reads `ActionState`, 5 tests.
- `crates/pla/src/game/pause_menu.rs` — menu reads/consumes `ActionState`, 2 headless tests.
- `crates/pla/examples/port_viewer.rs` — plugin line + the two systems read `ActionState`.
