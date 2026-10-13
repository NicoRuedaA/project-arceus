# Game audio in the port: what plays when

Code: `crates/pla/src/game/audio.rs` (Bevy plugin `GameAudioPlugin`, API `GameAudio`) with the pure parts in `game/audio/{events,catalog,mix,lru}.rs`; check tool `crates/pla/examples/audio_check.rs` (headless: never opens an audio device, never plays). Data: [`data/audio_index.tsv`](../../data/audio_index.tsv) (decoded media, see [audio.md](audio.md)) plus [`data/audio_event_media.tsv`](../../data/audio_event_media.tsv) (8 music events found for this task). Nobody has listened to any of this yet: the maintainer must judge it by ear.

## How it is wired

* **Output.** `bevy_audio` 0.19.1 (features `wav` + `vorbis`; rodio 0.22, cpal 0.17, ALSA on Linux) is a separate dependency line, **not** Bevy's `bevy_audio` feature, so `DefaultPlugins` stays silent everywhere (tests, headless examples). Only `GameAudioPlugin` in `Device` mode adds `bevy_audio::AudioPlugin`, and the viewer adds it with one line. The build needs the ALSA development files (`pkg-config --exists alsa`; package `alsa-lib` on Arch, `libasound2-dev` on Debian); present on this machine.
* **Switches.** `--no-audio` on the command line or `PLA_NO_AUDIO=1` (also `true/yes/on`): mode `Off`, no device opened, nothing loaded. Without a usable device Bevy logs `No audio device found.` once and the game runs silently.
* **Volumes.** `settings.txt` keys `master_volume`, `music_volume`, `sfx_volume`, `voice_volume`: whole percent 0-100, clamped, missing keys = 100, non-numbers ignored. Output gain = master × bus × fade (linear amplitude). Applied live to playing sounds. The pause menu has a `Volume: N%` row that cycles the master volume 100/75/50/25/0 and saves.
* **Catalogue.** Event lookup is case-insensitive; copies of the same WEM in several banks count once. Missing event, no playable media (codec 0x8311, digital silence) or missing file: silent, one warning per event/file. Files are loaded on first use into a 256 MB LRU (`PLA_AUDIO_CACHE_MB`); the Ogg rewrap is used when it exists (Vorbis-source media), else the WAV (all music and jingles: Opus source, WAV only). Music files are read on a background thread.
* **Take choice** when an event lists several media: UI sounds = the take with the highest RMS; impacts, ball throw/bounce and cries = random take; music = the longest take (ties: highest RMS). Wwise random/switch/layer semantics are not evaluated (ASSUMED policy).
* **Music.** One looping track at a time, 1.5 s fade-in, 1.0 s fade-out, crossfade on change. Loop points are not applied (the whole file loops).

## What plays on which event

| Game moment (where it is observed) | Sound (game event name) | Confidence |
| --- | --- | --- |
| Field, zone `ActiveZoneInfo.area_idx` 0..6 (start, zone change, after each battle) | `PLAY_BGM_HA_FI_AREA00`, `..._AREA01_DAY` .. `..._AREA05_DAY`, `..._AREA06` | names VERIFIED (hash); area binding LIKELY (name says area NN); `_DAY` variant always (no time-of-day switch) |
| Battle session appears | `PLAY_BA_SYS_BATTLE_START`, then `PLAY_UI_BATTLE_OPEN`; music `PLAY_BGM_VS_NORA` (wild battle) | ASSUMED roles from names |
| Battle model appears (`PokemonBattleSlot` added) | `PLAY_BA_SYS_POKE_APPERANCE` + cry `PLAY_PV_<dex>_00_00` (wild first, then the player's) | cry family VERIFIED (242/242 species); variant 00 = "the" cry ASSUMED |
| Engine `UsedMove` with damage | `PLAY_BA_SYS_HIT_L/M/H` (not very effective / normal / super effective or critical), 0.45 s after the move like the hit animation | ASSUMED |
| Engine `Recoil` | `PLAY_BA_SYS_HIT_L` | ASSUMED |
| Engine `StatusApplied` | `PLAY_BA_SYS_STATE_001_SLEEP` / `002_POISON` / `003_BURN` / `004_ICE` / `005_PARALYSIS` | LIKELY (numbered like the status ids) |
| Engine `KnockedOut` | that Pokémon's cry at 0.85 speed (lower), then `Play_Ba_sys_poke_down` | slower cry is a port choice (ASSUMED) |
| Engine `Switched` | `PLAY_BA_SYS_POKE_APPERANCE` + cry of the new Pokémon | ASSUMED |
| Engine `UsedHealingItem` | `PLAY_BA_SYS_STATE_010_RECOVER` | ASSUMED |
| Battle level rises | `PLAY_ME_HA_SYS_LVUP` (jingle, music bus) | LIKELY |
| Battle ends (`battle.ended`) | `PLAY_BA_SYS_BATTLE_END`, music fades out | ASSUMED |
| Battle log `You threw a ...` / `Shake...` / `Gotcha!` / `... broke free!` | `PLAY_BA_SYS_BALL_THROW_NORMAL_MID`, `PLAY_BA_SYS_BALL_HIT`, `PLAY_SS_CP_INTO_MONSTERBALL`, `PLAY_BA_SYS_BALL_SPIN` per shake, then `PLAY_UI_COMMON_CATCH` + `PLAY_ME_HA_EV_GET_POKE` or `PLAY_BA_SYS_BALL_OPEN` | ASSUMED; matched on the controller's log phrases (tested) |
| Battle log `Got away safely!` | `PLAY_UI_BATTLE_DECIDE_RUN` | ASSUMED |
| Battle menu: move cursor / move chosen / bag / party / list cursor / back | `PLAY_UI_BATTLE_SKILLS_SELECT` / `..._SKILLS_DECIDE` / `..._DECIDE_BAG` / `PLAY_UI_COMMON_DECIDE` / `PLAY_UI_BATTLE_SELECT_P` / `PLAY_UI_BATTLE_CANCEL` | ASSUMED |
| Overworld ball (`BallBody`): thrown / bounce / hits Pokémon / each shake / caught / escaped | `..._BALL_THROW_NORMAL_MID` / `..._BALL_DROP` / `..._BALL_HIT` / `..._BALL_SPIN` / `PLAY_UI_COMMON_CATCH` + `PLAY_ME_HA_EV_GET_POKE` / `..._BALL_OPEN` | ASSUMED |
| Pause menu open / close / cursor / setting changed | `PLAY_UI_COMMON_PAUSE_MENU_OPEN` / `..._CLOSE` / `Play_UI_common_select` / `PLAY_UI_COMMON_DECIDE` | LIKELY |

The pure mapping is `events::sfx_for_event(GameEvent) -> Option<SfxId>`, `music_for_area`, `cues_for_battle_events`, `cues_for_log_line`, `ball_events`.

## Verified (tests and measurements, without listening)

* `cargo test -p pla audio` and `settings`/`pause_menu` tests: catalogue lookups on the real index (11,970 rows; 25 named species have a playable `PLAY_PV_<sp>_00_00`; at least 242 species have cries; all 36 `SfxId` events and the area/battle music events have playable media), variant policies, Ogg-before-WAV paths, LRU eviction order and byte budget, bus and fade math, crossfade, event mapping, log-line phrases, ball state transitions, settings parsing/clamping, `--no-audio`/`PLA_NO_AUDIO`; Bevy app tests with a null sink (no device): voices spawned, a file loaded once, delayed cues wait, missing media stay silent, pause and zone sounds.
* `audio_check census`: 71 of 71 port sounds (36 effects, 10 music events, 25 cries) resolve to a file on disk.
* `audio_check decode`: those 71 files decode with Bevy's own decoders (rodio: hound for WAV, lewton for Ogg Vorbis) in memory: length equal to the index within 1 % (Ogg: plus at most one 2048-sample block, see below), all samples finite, peak 0.06-0.87. The 52 Ogg files decoded by lewton equal our WAV decode on the common part (max |difference| < 0.0001); 8 Ogg files run 162-1,336 samples longer than the WAV (lewton does not trim the last block to the Ogg granule position); the extra tail is near-silent (RMS ≤ 0.0008).
* Area music names: 52 candidate names hashed with FNV-1 against the 4,934 bank event ids; 9 matched (`AREA00`, `AREA01..05_DAY`, `AREA06`, `CMN_NIGHT`, `PLAY_BGM_FIELD`); the method was first checked on `PLAY_BGM_HA_FI_CMN_WORMHOLE`. With ~2,000 tries per 4,934 ids the chance of a false match is about 0.2 %. Media come from the HIRC graph through the music child-list scan (flagged heuristic).

## Assumed

* Which game moment each sound belongs to (read from event names; the game's scripts and animation timelines that fire them are not decoded).
* Take policy, the 0.85 faint-cry speed, timings (hits at 0.45 s, shakes every 0.65 s), fade lengths.
* Battle-log matching for balls/capture/run: if `battle_controller.rs` changes those phrases, those sounds go silent (a test pins them).

## Unknown

* Area music: which of the 4 takes of each `AREAnn_DAY` event is the real loop, whether they are layers meant to play together, loop points, intro segments, the night/evening variants per area (only the shared `CMN_NIGHT` was found) and the rules in `bgm_layer.bin` / `bgm_transition_*.bin`. Area 06's name is guessed from the area number only.
* The meaning of cry variants 00-04 (which one is the faint or the attack cry).
* Victory music, trainer battles, footsteps, ambience (`ENV_*`), weather, Pokémon move sounds (`PLAY_PM_*` per species and move) and player sounds: not wired.
* Stat stage changes and low-HP warning (`PLAY_BA_SYS_PINCH`) have sounds in the API but the engine events that would fire them are not exposed to the plugin.
* How it sounds: not listened to. Volume balance between buses is not tuned.

## For the maintainer (listening checklist)

1. Field music starts within ~2 s in each area and loops; check that it is the full piece (not a stem or a short stinger).
2. Walk into a wild Pokémon: battle start sting, battle music crossfades in, cries of both Pokémon.
3. Attacks: hit sound when the target flinches; faint: low cry + fall sound.
4. Throw a ball in battle and in the field: throw, hit, shakes, click + capture jingle, or the escape pop.
5. Pause menu: open/close/cursor sounds; `Volume` row changes loudness at once and survives a restart.
6. `--no-audio` and `PLA_NO_AUDIO=1`: silent, no warnings beyond one info line.
