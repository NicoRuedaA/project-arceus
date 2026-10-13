# Player skills and expressions: dodge, crouch, hide, blink (Brief 22)

**In plain words.** The protagonist can now roll (dodge) in the direction you push, crouch and
walk crouched, and a hidden player is ignored by wild creatures that wander near. The roll clips
and the crouch clips are the game's own animations, found by name among the 423 player clips; the
roll distance (5.25 m) is measured from the clip itself. The game has no clip for "hide": hiding
is the crouch plus a rule. The protagonist also blinks now, using a stand-in effect, and the
Pokemon blink animation was found in the game files but is not played yet. Nobody has seen any of
this in a window.

Sheets: [`data/character_face.tsv`](../../data/character_face.tsv) (158 rows: blink intervals,
player facial expressions, look-at speeds) and [`data/player_controller.tsv`](../../data/player_controller.tsv)
(Brief 13). Decoder: `.tools/player_config_decode.py`. Code: `game/player_skills.rs` (state and
rules), `game/player_anim.rs` (clip choice), `game/blink.rs`, one hook in `game/wild_wander.rs`,
movement in `port_viewer.rs::player_locomotion`.

## Clip inventory (the 423 `pc0002_00_*.tranm` names)

Names come from the player animation controller file (`pc/0006.bin` of the pack dump); each name
resolves to a pack entry through the recovered directory hash state (`ANIM_DIR_STATE` in
`player_export`). Groups by number prefix: 01xxx battle 59, 02xxx field 106, 03xxx ride 131, 05xxx
actions 53, 06xxx throw/item 43, 07xxx gestures 16, 08xxx demo 1, 09xxx photo poses 14.

| Wanted | Clips found (exact names) | Use |
|---|---|---|
| dodge | `05011_rolling01_f`, `05012_rolling01_b`, `05013_rolling01_l`, `05014_rolling01_r` (and `rolling02_*` 05016-05019, `rollingthrow01_f` 05015, battle `avoid01` 01155) | exported as `roll_f/b/l/r` |
| crouch | `02031_squat01_l_loop` (idle), `02105_sqwalk01_loop` (walk), `02033_sqwait01_l_start`, `02034_sqwait01_l_end` (enter, exit); also `sqwalk01_{l,r}_{f,b,l,r}` directional (02108-02115), `sqrun01` 02134, `sqwalkstart/stop`, `sqrunstart/stop`, `sqthrow01_*` | idle, walk, enter, exit exported as `crouch`, `crouch_walk`, `crouch_start`, `crouch_end`; the rest unused |
| hide | **none** (no name contains hide, stealth or conceal) | hiding = crouch + rule (below) |
| blink | **none** for the player | see blink |
| climb | **none** (`slipdown`, `down`, `standup` are not climbing) | Brief 13 uses `walk` |

Exported total: 17 clips (was 9). `player_export` measured, per clip, frames at 60 fps and the
`origin` bone's root travel: every `rolling01_*` is 91 frames (1.517 s) and exactly 5.250 m
(`_f` dz +5.25, `_b` dz -5.25, `_l` dx +5.25, `_r` dx -5.25; +X is the character's left);
`sqwalk01_loop` 85 frames, 1.540 m; `squat01_l_loop` 81 frames; `sqwait01_l_start` 25 frames,
`_end` 33 frames. `data/player_model_export_index.tsv` now says 17 clips for both variants.
The cached export is rebuilt by `./play.sh` (the setup step watches `player_export`).

## Rules and values

| Item | Value | Status |
|---|---|---|
| roll distance / duration | 5.25 m in 1.517 s (constant speed 3.46 m/s) | distance and length VERIFIED from the clip data; constant-speed profile ASSUMED |
| roll clip by direction | within 55 deg of the facing: forward; up to 135 deg: left or right; beyond: back (`rolling_forward_angle` 55, `rolling_side_angle` 135, `rolling_backward_angle` 180) | values VERIFIED, use PROBABLE |
| roll start | Dodge key/button; only on the ground and out of water; cancels a crouch | ASSUMED rules |
| input buffer | `buffered_input_frame_rolling` 5 frames, read as 5/30 s | value VERIFIED, 30 Hz ASSUMED |
| i-frames | the roll asserts invulnerability (native report `n2-avatar-locomotion-and-state-machine.md`); exposed as `PlayerSkillState::invulnerable()` for the whole roll | PROBABLE; window length ASSUMED; nothing consumes it (wild attacks do not hurt the player in the port) |
| crouch speed | `control_maxCrouchingSpeed` 2.1 m/s caps the walk | value VERIFIED (it exceeds the parameter's own bound 0), role PROBABLE |
| crouch cancel | sprint, jump, falling or water stand the avatar up | ASSUMED |
| hide | tall grass hides; crouching hides only inside a stealth volume (native `IsHideEnable`); a hidden player is not noticed | PROBABLE |
| tall grass / stealth volumes | **no data in the port**: `StealthContext` is false everywhere. `PLA_STEALTH_VOLUME_EVERYWHERE=1` makes the whole world a stealth volume (debug aid) | UNKNOWN |
| controls | roll: `H` / gamepad north (Y); crouch toggle: Left Ctrl / right stick click (R3) | port choices (the original bindings for crouch are unknown) |

What hiding changes: `PlayerStealth.hidden` is read by `wild_wander`, which then treats the player
as absent, so creatures keep wandering instead of stopping to look at you. The wild behaviour
state machine (`wild_behavior::Perception::player_hidden`) already had the flag but nothing feeds
it yet; the encounter system (`creature_encounter_system`) does not read the stealth state.

## Faces

`character_blink_config.bin`: player blink every 30-150 frames, 30-90 when hurt, Pokemon, NPC and
ridden Pokemon 30-150 (VERIFIED values; frame = 1/30 s ASSUMED). `player_face_config.bin` (93
rows) maps situations to named expressions: `face_default normal`, `face_damage damage`,
`face_hide serious` (the expression used while hiding), `face_vigilance serious`,
`face_battle_result_win rejoice`, `..._lose trouble`, `face_look_warlike serious`, smile, sad,
relief, anger, unique1/unique2, plus keep-frame counts. These names are decoded; how the original
renders an expression (texture layer, UV or blendshape) is UNKNOWN, so none is rendered.

- **Player blink (implemented, SUPUESTO look)**: the two eye meshes (`eye_mesh_shape#eye`,
  `eyehilight_mesh_shape#eye`) are hidden for 0.18 s in the middle of a 0.35 s blink at the decoded
  interval; `PLA_PLAYER_BLINK=0` disables it. How the original closes the eyes is not decoded.
- **Pokemon blink (found, not played)**: packs carry a 0.35 s one-shot `eye01` clip (328 clips
  across 356 packs) and a `mouth01` clip (284), parsed as plain bone clips. Pikachu's `eye01`
  animates only `left/right_upper_eyelid_01/_02` and `left/right_lower_eyelid` (21 rotation keys
  on the upper lids, 15 on the lower), plus constant `origin`/root tracks; Starly's has the same
  eyelid set. That is the blink (PROBABLE). Playing it needs (1) adding `eye01` to the exported
  Pokemon glTF (today only walk, idle and 8 battle clips are exported, and changing the exporter
  rebuilds every cached Pokemon), and (2) an animation-graph node masked to the eyelid bones so it
  does not fight the body clip's root tracks. Neither was done: it could not be checked without a
  window. `game::blink::Blink` already schedules Pokemon blinks from the decoded interval.
- Look-at: `character_look_at_config.bin` (55 rows) gives head and eye turn speeds (60 and 140
  deg/s head, 60 and 180 deg/s eye, 300 deg/s for Pokemon eye UV); decoded, not used.

## Verification

Unit tests: signed angle and direction thresholds, roll timing, crouch cycle, buffers, clip
priority (water, roll, air, land, crouch, speed tiers), fallback chains, hidden-player rule and
the wander filter, blink schedule (range, minimum gap, length) and its match with the sheet.
Headless Bevy tests with synthetic keys: crouch toggle, stealth flag, stand-up on jump and sprint,
dodge buffer, blink toggling only the two eye meshes. The exporter ran offscreen once and printed
the numbers above. Not verified: the animations on screen, the roll direction against the camera,
the feel of the speeds, and everything that needs the cached export rebuilt.
