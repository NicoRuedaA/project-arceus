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
- The contested windowed viewer encounter system was not modified. The existing wander path only
  receives a passive fallback component and mirrors its already-known Idle/Walk phase; no new
  perception input is fabricated.

## Implementation and verification

`game/wild_behavior.rs` contains the pure machine, perception gates, sleep-window helper,
encounter projection, and unit tests. `wild_wander.rs` attaches the passive fallback and mirrors
the existing wander phase without changing its movement rules. The focused tests pass: 9 machine
tests and 12 wander tests, including a headless Bevy regression with three synthetic creatures.
These tests verify the port policy and integration only; native behavioral parity remains UNKNOWN.
No viewer, audio, or window was opened.
