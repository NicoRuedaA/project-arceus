# Wild behavior evidence — update v262144

## Scope

Brief 27 is implemented as a deterministic, Bevy-independent reaction machine. No native
numeric value is presented as decoded unless the cited source directly supports that claim.

## Verified

- Static evidence identifies a native wild state field with IDs in `1..=7`, a distance field at
  `+0x10`, and reaction flags at `+0x153` (`n2-entities-and-propagation.md` §2.2).
- The native loop visibly looks up reaction keys for ordinary distance, sleep, random reaction,
  warning, mild/coward reaction, and escaping duration. The source does not provide decoded values
  in this worktree.
- The player stealth report directly describes tall-grass / crouch concealment reaching wild
  perception. This is static source evidence, not a runtime replay.
- `data/field/wild_encounter_modifiers.tsv` contains 322 raw records with unresolved semantics.

## Probable

- The native `+0x153` labels alert/flee/combat are useful behavioral context, but the individual
  bit meanings and all seven state names remain unresolved.

## Supposed

- `Archetype::{Passive, Timid, Aggressive}` and their default vision/hearing ranges are port
  policy only. They are not species decodes.
- `EncounterIntent::Engage { surprise: true }` is an isolated port-facing projection. It does not
  assert that the native battle opener uses the same label or threshold.

## Unknown and skipped investigations

- `[skip]` No schema-backed decoder was available for `field_wild_pokemon_config.bin` (736 bytes),
  `field_pokemon_slope_config.bin`, `pokemon_controller_config.bin`, `npc_ai_config.bin`, the
  `ai_*_config.bin` files, or `field_vigilance_bgm_config.bin`; their numeric contents were not
  interpreted.
- Species-to-archetype mappings, exact vision/hearing/noise thresholds, sleep clock boundaries,
  flee/chase/attack transitions, and native encounter outcomes remain UNKNOWN.
- The viewer encounter system was not modified: battles still start from the existing proximity
  rule (2.8 m) or the interact key; `encounter_intent` stays unwired.

## Implementation and verification

`game/wild_behavior.rs` contains the pure machine, perception gates, sleep-window helper,
encounter projection, and unit tests. `wild_wander.rs` attaches the passive fallback and mirrors
the existing wander phase without changing its movement rules. The focused tests pass: 9 machine
tests and 12 wander tests, including a headless Bevy regression with three synthetic creatures.
These tests verify the port policy and integration only; native behavioral parity remains UNKNOWN.
No viewer, audio, or window was opened.

## Viewer wiring (2026-10-13)

`wild_wander.rs` now steps the machine every frame in the viewer (`--wild-behavior on`, the
default). Inputs: planar distance to the player, the angle between the creature's facing and the
player, and a noise class from the player's planar speed (silent ≤ 0.5 m/s, walk ≤ 6 m/s, run
above; SUPPOSED thresholds). A hidden player (`PlayerStealth`) is not perceived. Outputs: Idle and
Wander run the old wander loop (without its own 6 m stop-and-face rule); Alert and Attack stop and
face the player; Flee walks away at 3.5 m/s; Chase walks towards the player at 3.0 m/s and stops
just inside the 2.5 m attack distance, where the viewer's existing 2.8 m proximity encounter starts
the battle; Sleep stands still. Flee/chase never leave the spawner radius or the navmesh. Speeds
are scaled by the creature size hint. All these numbers are SUPPOSED port policy.

Archetypes: no species mapping is decoded, so each creature gets passive, timid or aggressive
deterministically from its wander seed (about one third each, SUPPOSED). `--wild-behavior
passive|timid|aggressive` forces one archetype for testing; `--wild-behavior off` restores the
previous wander-only loop (passive fallback component, phase mirror only). Sleep stays inactive:
the machine supports a sleep window, but no window or clock boundary is decoded, so none is set.

Verification: deterministic unit tests (flag parsing, archetype spread, noise classes, passive
alert, timid flee inside the radius, aggressive chase to the attack distance, walkability,
hidden player) and a headless Bevy test of the attached archetype per flag value. Never checked
in a window.
