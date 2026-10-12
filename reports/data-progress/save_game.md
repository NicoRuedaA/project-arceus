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

## Wiring plan (for the viewer owner; not edited here)
1. App setup: `app.insert_resource(game::save_game::SaveGame::from_env())`.
2. Start-up: `SaveGame::load()`; if `Some`, apply zone (`--area`), player transform/yaw, time slot, spawn RNG seed, `ActiveBattleSession` bag via `bag_to_engine`, and rebuild the party with `to_combatant` (look up `BattleSpecies` by dex and `BattleMove` by id).
3. Save triggers: a key (e.g. F5) in the world state, on zone change, and after `GameState::BattleEnded`. Build `PlayerSave` from the `PlayerAvatar` transform, `ActiveZoneInfo`, lighting time slot, the party `Combatant`s (`from_combatant`) and the bag (`bag_from_engine` with the `ItemTable` ids).
4. After a successful catch/victory: update `caught`/`seen` sets and counters.
5. HUD: show `SaveGame::message` for a few seconds.

## Engine gaps (documented, not edited)
- `ItemBag` has no iterator: saving needs the candidate id list (`ItemTable` ids).
- `Combatant` has no form, nickname or nature id; the save keeps form/nickname but the engine cannot supply them yet. Nature percentages must be passed back in.
- There is no persistent party type outside a battle; `ActiveBattleSession` builds `Combatant`s per battle, so the viewer must keep a party list to save.
- Money, seen/caught sets, defeated counters and the time slot have no engine owner yet.

## NOT done
Viewer integration (no call sites edited), UI, multiple save slots, encryption, in-game quest/event flags (those belong to the script layer), live-engine round trip test (tests are pure and use temp dirs only), automatic migration of v1 files that never existed in the wild (v1 is a test fixture).
