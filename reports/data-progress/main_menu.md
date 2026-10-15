# Main menu and basic options (brief 7)

English, 2026-10-13. Status of the opt-in main menu added to the Bevy viewer
(`crates/pla/src/game/main_menu.rs`) by the `feat/main-menu` branch.

## What it is

- `GameState::MainMenu` is a new state, **off by default**: the viewer enters it only with
  the new `--menu` flag, so `--auto-demo`, `--script` and the direct start keep booting
  into `GameState::World` exactly as before.
- The menu overlay (Bevy UI) has four rows: **Continue** (disabled without a valid
  `SaveGame` loaded at start-up), **New game**, **Options**, **Exit**. Navigation is
  keyboard/gamepad through the shared `ActionState` of brief 3 (`game/input_map.rs`,
  cherry-picked into this branch because `main` does not contain it yet) plus mouse
  hover/click. Menu text is English (localisation is out of scope).
- **Options page**: master volume cycle (100/75/50/25/0 %), field of view
  (45/60/75/90/110 deg cycle), shadows ON/OFF and quality (Low/Medium/High). Every change
  is written to `settings.txt` immediately. `UserSettings` gained `fov_degrees`,
  `shadows` and `quality`; the volume buses already existed (audio task). Old files
  without the new keys keep their defaults, and unknown keys keep being ignored, so the
  format is backward compatible in both directions.
- **Continue** simply closes the menu: the world was already placed by the save loaded at
  start-up. **New game** resets `PlayerProgress` to a fresh game (re-seeding the same
  starter party/bag the direct start seeds when `BattleAssets` is present), forgets the
  loaded save in memory (`SaveGame::current = None`, the file stays on disk until the next
  save, exactly like `--new-game`) and requests the safe spawn.
- While the menu is open the world systems stay frozen (they are already gated on
  `in_state(GameState::World)`), the world/battle HUD is hidden, and the pause overlay is
  blocked from opening on top of it (`pause_menu::pause_input` returns early in
  `MainMenu`). The menu consumes its own `ActionState` edges and clears the raw
  Enter/Space/click edges so the world never sees the confirming press on the frame the
  state changes.

## Verified

- Unit tests (8 in `main_menu.rs`, 4 new in `settings.rs`): selection wrapping/skipping of
  the disabled Continue row, row activation, options cycles and labels, FOV cycling,
  settings round-trip with the new keys, old-file compatibility, unknown keys ignored,
  clamps, quality cycle.
- Headless Bevy tests (synthetic key input, no window): `MainMenu -> World` with New game
  (Continue missing from the walkable rows), `MainMenu -> World` with Continue when the
  flag `has_save` is set, options page edit + persist + Esc-back, and the pause overlay
  staying closed while Esc is pressed in the menu.
- Full suite: `cargo test -p pla` green — 440 lib tests + 89 integration tests (23
  assets, 6 event, 7 pokemon_anim, 3 script, 5+12+5 tr_animation*, 5
  tr_matrices, 6 tr_skin, 4 trskl, 2 visual [1 ignored], 5 wwise_audio) + doc-tests, 0
  failures. `cargo build -p pla --example port_viewer` succeeds.
- `--menu` is listed in `--help`. The viewer was NOT run (rule: never open a window).

## Assumed

- The exact game behaviour of a "new game" from a main menu is not decoded: the reset
  reuses the demo direct-start content (starter party/bag) and the safe-spawn request.
  The old save file is not deleted (same as the existing `--new-game` flag).
- The default field of view (60 deg), the FOV cycle steps and the default quality
  (Medium) are picked for the UI; no game value was decoded.
- Master volume was already wired to the audio system by the audio task; the other
  volumes (music/sfx/voice) are not editable from this options page yet.

## Unknown / not applied yet

- `fov_degrees`, `shadows` and `quality` are persisted and editable but **nothing in the
  renderer reads them yet** (wiring them would touch `lighting.rs` / `render_look.rs`,
  which conflicts with briefs 4, 6, 9 and 35). They exist so the settings format and the
  options UI are in place; applying them is a follow-up.
- `--menu` combined with `--auto-demo`/`--script` is not a supported combination (the
  scripted drivers assume `GameState::World`); the menu is off unless asked for.
- Visual quality of the overlay (layout, colours) was only checked by compilation and
  headless tests: the maintainer must look at it.

## Integration points

- `crates/pla/src/game/main_menu.rs` — new module (menu logic + UI + plugin).
- `crates/pla/src/game/state.rs` — `GameState::MainMenu`.
- `crates/pla/src/game/settings.rs` — `Quality`, `fov_degrees`, `shadows`, `quality`,
  `parse_fov`.
- `crates/pla/src/game/pause_menu.rs` — pause input ignores the menu state.
- `crates/pla/src/game/ui.rs` — HUD hidden in `MainMenu`.
- `crates/pla/src/game/mod.rs` — `pub mod main_menu;`.
- `crates/pla/examples/port_viewer.rs` — `--menu` flag, `insert_state(MainMenu)`,
  `MainMenuPlugin::install`, telemetry label `MAIN MENU`.
- `crates/pla/src/game/input_map.rs` — cherry-picked from `feat/gamepad`
  (commit `aeb7482`): `Action`, `ActionState`, `InputMapPlugin`.

## Merge into `main` (2026-10-13)

- The branch carried its own copy of the input map commit (`a4a07a3`, same patch as
  `aeb7482`, which reached `main` through `feat/gamepad`). The two `input_map.rs` files
  are byte-identical, so `main` keeps ONE implementation and the menu uses `main`'s
  `ActionState` API unchanged.
- `GameState` now has both `Loading` (zone change) and `MainMenu`; the pause input keeps
  `main`'s action-based toggle and W/S navigation and only gained the early return in
  `MainMenu`.
- Defect found at merge: the viewer installed `MainMenuPlugin` (which adds
  `InputMapPlugin` when missing) before its own `add_plugins(InputMapPlugin)`, so
  `--menu` would have panicked at start-up (Bevy refuses a plugin added twice). The
  viewer now installs the menu after `InputMapPlugin`. Checked by compilation and by
  reading the code only; the viewer was not run.
- Known gap (shared with the pause menu): `UserSettings::serialize` does not write the
  `effects=` key, so saving from the options page drops it.

## How to try it

```bash
cargo run -p pla --example port_viewer -- --menu
```

`Esc` backs out of the options page; `Exit` closes the program. Same command with
`--save-dir <dir>` to point `settings.txt` and the save somewhere else.
