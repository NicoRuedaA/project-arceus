# Player save (own port format)

Module: `crates/pla/src/game/save_game.rs`. This is the port's own save, not the original game's format (`crates/pla/src/save.rs` only emulates the original host save API for scripts and is untouched).

## Format
Strict line-based text, no new dependencies (`serde` is not a dependency; only `serde_json`). File `save.plasave`:

```
PLASAVE 2
CHECKSUM <fnv1a64 hex of every byte after this line>
zone=ha_area01          pos=x,y,z   yaw=..   time_slot=..   money=..
defeated=..  caught_count=..  rng_seed=..
seen=1,25,722  caught=722   bag=item:qty,...
member=species|form|level|hp|status|e0:e1:e2:e3:e4:e5|move:pp,move:pp|nickname-hex
END
```
Unknown keys, duplicate keys, bad numbers, missing required fields, non-finite floats, more than 6 members/4 moves are rejected. `END` is inside the checksum so truncation at any length fails. Sets are sorted, so output is deterministic. Nicknames are hex-encoded (`-` none, `~` empty).

## What is saved
Zone id, player position and yaw, time slot, party (<= 6: dex id, form, level, current HP, status, per-move PP, effort level, optional nickname), bag (item id -> qty), money, seen/caught dex sets, defeated and caught counters, RNG seed.

## API
- `PlayerSave::{encode, decode, validate}`; `SaveError` (friendly `Display`, never panics).
- `SaveFile::{in_dir, from_env, read, write, load_with_fallback, backup_path}`; write is temp file + fsync + rename, previous save copied to `save.plasave.bak` first; a damaged main file falls back to `.bak`.
- Location: `PLA_SAVE_DIR`, else `$XDG_DATA_HOME/pla-port`, else `~/.local/share/pla-port`.
- `SaveGame` Bevy resource: `save(PlayerSave) -> bool`, `load() -> Option<PlayerSave>`, `message` (text for the UI).
- Engine conversions: `PartyMember::from_combatant(&Combatant)`, `PartyMember::to_combatant(species, moves, nature_percent)` (stats recomputed from level + effort; HP, PP, status restored), `SavedStatus <-> MajorStatus`, `PlayerSave::bag_from_engine(&ItemBag, ids)`, `PlayerSave::bag_to_engine(&ItemTable)`.
- Versioning: `CURRENT_VERSION = 2`; `migrate()` chains steps. v1 (members without form/effort) -> v2 is implemented and tested.

## Wiring (done in `crates/pla/src/game/save_wire.rs`)
- `PlayerProgress` resource: party (<= 6), box list (memory only), bag map, money, seen/caught sets, defeated/caught counters, rng seed, time slot. Seeded with the demo party (Pikachu, Bidoof, Starly) and starter bag, or from the save.
- Each battle: `install_party` rebuilds the player side from the saved members (HP, PP, status restored; fainted stay fainted; first living member leads; if all are fainted the party is restored, a placeholder for a village return); `engine_bag` and the rng seed are installed. Every frame `snapshot` + `apply_sync` mirror party and bag back; when the session disappears `apply_outcome` updates seen (on encounter), caught/caught_count (party if room else box), defeated (win), then autosaves.
- Start-up (`load_startup`): zone, position (x/z), player yaw, time slot, party, bag, sets from `SaveGame::load` (message shown as a HUD toast). Explicit `--area`, `--player-pos`, `--time-slot` win over the save. Height is recomputed from terrain.
- F5 saves in the world (refused during a battle); autosave after each battle and when `ActiveZoneInfo` changes (the viewer has no runtime zone change yet, so that path is untested live).
- CLI: `--new-game` (ignore save), `--save-dir <path>` (sets `PLA_SAVE_DIR`), `--save-selftest` (evidence log).
- Tests: 9 unit tests in `save_wire.rs` (temp dirs only), 4 use the real battle engine and data.
- Evidence: one offscreen `--script` run with a temp save dir (`--new-game --save-selftest`): party before battle `#25 hp=78; #399 hp=92; #396 hp=64`, after `#25 hp=67; ...`, reloaded from the file identical. The reload was done in-process with a fresh file handle, not a second process.

## Still not done
Box persistence (format has no box), form/nickname/nature from the engine, money income, runtime time-slot changes, saving mid-battle, multiple slots, windowed interactive check of F5/toast, encryption, quest/event flags.
