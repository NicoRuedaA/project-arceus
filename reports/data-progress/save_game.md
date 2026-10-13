# Player save (own port format)

Module: `crates/pla/src/game/save_game.rs`. This is the port's own save, not the original game's format (`crates/pla/src/save.rs` only emulates the original host save API for scripts and is untouched).

## Format
Strict line-based text, no new dependencies (`serde` is not a dependency; only `serde_json`). File `save.plasave`:

```
PLASAVE 3
CHECKSUM <fnv1a64 hex of every byte after this line>
zone=ha_area01          pos=x,y,z   yaw=..   time_slot=..   money=..
defeated=..  caught_count=..  rng_seed=..
seen=1,25,722  caught=722   bag=item:qty,...
member=species|form|level|hp|status|e0:e1:e2:e3:e4:e5|move:pp,move:pp|nickname-hex
box=species|form|level|hp|status|e0:e1:e2:e3:e4:e5|move:pp,move:pp|nickname-hex
END
```
Unknown keys, duplicate keys, bad numbers, missing required fields, non-finite floats, more than 6 party members / 4 moves / 960 box slots are rejected. `member` and `box` are the only keys allowed to repeat. `END` is inside the checksum so truncation at any length fails. Sets are sorted, so output is deterministic. Nicknames are hex-encoded (`-` none, `~` empty).

## What is saved
Zone id, player position and yaw, time slot, party (<= 6: dex id, form, level, current HP, status, per-move PP, effort level, optional nickname), box (`box_pokemon`, <= [`MAX_BOX`] = 960 stored Pokemon, same record as a party member), bag (item id -> qty), money, seen/caught dex sets, defeated and caught counters, RNG seed.

## API
- `PlayerSave::{encode, decode, validate}`; `SaveError` (friendly `Display`, never panics).
- `SaveFile::{in_dir, from_env, read, write, load_with_fallback, backup_path}`; write is temp file + fsync + rename, previous save copied to `save.plasave.bak` first; a damaged main file falls back to `.bak`.
- Location: `PLA_SAVE_DIR`, else `$XDG_DATA_HOME/pla-port`, else `~/.local/share/pla-port`.
- `SaveGame` Bevy resource: `save(PlayerSave) -> bool`, `load() -> Option<PlayerSave>`, `message` (text for the UI).
- Engine conversions: `PartyMember::from_combatant(&Combatant)`, `PartyMember::to_combatant(species, moves, nature_percent)` (stats recomputed from level + effort; HP, PP, status restored), `SavedStatus <-> MajorStatus`, `PlayerSave::bag_from_engine(&ItemBag, ids)`, `PlayerSave::bag_to_engine(&ItemTable)`.
- Versioning: `CURRENT_VERSION = 3`; `migrate()` chains steps. v1 (members without form/effort) -> v2 and v2 (no box) -> v3 are implemented and tested. v2->v3 is a documented no-op: a v2 file has no `box` lines, so the box starts empty.

## Wiring (done in `crates/pla/src/game/save_wire.rs`)
- `PlayerProgress` resource: party (<= 6), box list (persisted as `box_pokemon`), bag map, money, seen/caught sets, defeated/caught counters, rng seed, time slot. Seeded with the demo party (Pikachu, Bidoof, Starly) and starter bag, or from the save.
- Box operations (pure, no Bevy): `PlayerProgress::deposit(PartyMember)`, `withdraw(box_index)`, `swap(box_index, party_index)`, each returning `Result<_, BoxError>` (`BoxFull`, `PartyFull`, `BadBoxSlot`, `BadPartySlot`). No UI uses them yet; they exist for the box screen (task 32) and are unit tested.
- Each battle: `install_party` rebuilds the player side from the saved members (HP, PP, status restored; fainted stay fainted; first living member leads; if all are fainted the party is restored, a placeholder for a village return); `engine_bag` and the rng seed are installed. Every frame `snapshot` + `apply_sync` mirror party and bag back; when the session disappears `apply_outcome` updates seen (on encounter), caught/caught_count, defeated (win), then autosaves. A catch goes to the party while it has room, otherwise it is `deposit`ed into the box; if the party AND the box are full the catch is still counted (caught set + `caught_count`) but not stored.
- Start-up (`load_startup`): zone, position (x/z), player yaw, time slot, party, bag, sets from `SaveGame::load` (message shown as a HUD toast). Explicit `--area`, `--player-pos`, `--time-slot` win over the save. Height is recomputed from terrain.
- F5 saves in the world (refused during a battle); autosave after each battle and when `ActiveZoneInfo` changes (the viewer has no runtime zone change yet, so that path is untested live).
- CLI: `--new-game` (ignore save), `--save-dir <path>` (sets `PLA_SAVE_DIR`), `--save-selftest` (evidence log).
- Tests: 18 unit tests in `save_game.rs` (temp dirs only) and 14 in `save_wire.rs` (temp dirs only); 4 of the `save_wire` ones use the real battle engine and skip when the data tables are absent. `cargo test --offline -p pla save_` -> 34 passed, 0 failed (2026-10-13).
- Box evidence: round trip with a non-empty box, v2->v3 migration of a literal v2 body (empty box), the `MAX_BOX` boundary refused on write and on read, a malformed `box` member rejected, a full party routing a catch to the box and reloading it from the file, `deposit`/`withdraw`/`swap` (including limits) and a full-box catch counted but not stored. The full-party catch test also runs the real `battle_engine` and reloads the file.
- Evidence: one offscreen `--script` run with a temp save dir (`--new-game --save-selftest`): party before battle `#25 hp=78; #399 hp=92; #396 hp=64`, after `#25 hp=67; ...`, reloaded from the file identical. The reload was done in-process with a fresh file handle, not a second process.

## Still not done
Form/nickname/nature from the engine, money income, runtime time-slot changes, saving mid-battle, multiple slots, windowed interactive check of F5/toast, encryption, quest/event flags. The box is persisted but has no screen (task 32) and no reordering/sorting; the party is auto-filled from the box only through the pure `withdraw`/`swap` helpers, which no system calls yet.

## Verified / Assumed / Unknown
- **Verified** (by the tests above, `cargo test --offline -p pla save_`, 34 passed): the box encodes/decodes and survives a file round trip; a v2 save migrates to v3 with an empty box; the `MAX_BOX` limit is enforced on write and on read; a catch with a full party is deposited in the box; `deposit`/`withdraw`/`swap` respect `MAX_BOX`/`MAX_PARTY` and slot bounds.
- **Assumed**: sending a full-party catch to the box is the intended behaviour (the brief asks for it). The 960-slot ceiling (30 boxes x 32) is a reasonable port choice, not decoded game data.
- **Unknown**: the real game's box capacity, its slot/box ordering and the behaviour when the party **and** the box are both full. Nothing here was checked in a windowed session (the box has no UI yet).
