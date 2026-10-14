# Trainer and boss battles in the port (Brief 30)

Scope: the Bevy port of Pokémon Legends: Arceus (base + update v262144 overlay). This report covers the **implementation** of trainer battles (several Pokémon per side) and the turn-based part of boss battles, built on the data research in [`trainers.md`](trainers.md) (sheets `data/trainer_index.tsv`, `data/trainer_teams.tsv`, `data/boss_battles.tsv`, `data/boss_field_attacks.tsv`). Nothing was run in the original game; every check below is a unit test, a headless Bevy test or a windowless example.

Confidence words: **VERIFIED** (checked against the bytes or code for every row, or measured here), **PROBABLE / LIKELY** (strong indirect evidence), **SUPUESTO / ASSUMED** (a guess that fits, kept visible), **UNKNOWN**.

## Results in numbers

| Item | Count | How |
|---|---|---|
| Trainers loaded from the sheets | **377 / 377** | `trainer_data::TrainerCatalog::load`, test `real_sheets_load_every_trainer_and_member` |
| Party members built as engine combatants | **620 / 620** | test `every_real_party_member_builds_an_engine_combatant` (form-aware species rows: Hisuian forms resolve to their own row) |
| Trainers that start a battle with their whole party | **377 / 377** | test `every_trainer_starts_a_battle` |
| Noble settings loaded and built | **16 / 16** (8 battles + 8 rematches) | test `real_noble_sheet_has_sixteen_settings` |
| Windowless sweep: every trainer played to the end | **377 / 377** end (no panic, no stall) | `battle_demo --trainer all` (player demo party Pikachu + 2 bench, best damaging move each turn, seed 1) |
| Sweep outcomes, PLA turn order | L30: 31 won / 345 lost / 1 draw; L50: 75 / 301 / 1; L70: 160 / 216 / 1; L100: 294 / 82 / 1 | same command with `--level N` |
| Sweep outcomes, classic turns, L50 | 60 won / 317 lost / 0 draw | `--classic-turns` |
| Placed NPCs that stand for a trainer | **51** NPC records in **12** areas (ASSUMED link, see below) | test `placements_join_trainer_model_packs` |

The one draw (`single_180`, Rapidash with Double-Edge) is both sides fainting on the same turn from recoil; the engine reports it as a draw.

## What is implemented

| Part | Where | Confidence |
|---|---|---|
| Sheet loader (trainer, party, AI bits, items, prize field, Noble settings) | `crates/pla/src/game/trainer_data.rs` (`TrainerCatalog`, `NobleCatalog`) | fields as in `trainers.md` (VERIFIED/LIKELY) |
| Member -> combatant (species + form, level, 4 moves, effort levels, nature) | `TrainerMon::to_combatant` | effort order HP, Atk, Def, Spe, SpA, SpD LIKELY; nature table **ASSUMED** (main-series order; 12 = neutral fits the data) |
| Trainer battle start: whole party on side 1, first member at combatant 1, the rest after the player's party | `ActiveBattleSession::start_trainer` (`trainer_battle.rs`) | — |
| Forced replacement when the active opponent faints, next living member in **stored order**; victory when all faint | engine `advance_active_for` + log line "X sent out Y!" | send-out order **UNKNOWN** (stored order used) |
| No capture (balls refused, nothing consumed, no turn spent) and no running | `capture_refusal`, `run_refusal` | **PROBABLE** (brief rule, main-series behaviour; not read in this game's code) |
| Move AI from the module bits: Basic -> `basic_affinity_ineffective`; Strong -> `strong_non_max_damage` + `strong_high_affinity_bonus` (70 % gate) | `trainer_rules`, `battle_ai::choose_ai_action_rules` | rules VERIFIED (observed Lua); bit mapping LIKELY; affinity scale **ASSUMED** (`0` = no effect, else `7 + log2(multiplier)`; it makes every recovered threshold coherent) |
| Expert module | not used | per-score predicates of the 49 `ExpertAI_Seq_*` handlers are only partly mapped: **UNKNOWN** |
| Item AI (Item bit; items 23/24/25): under 20 % HP 66 % chance, under 40 % HP 27 % chance, never above | `choose_item` | thresholds LIKELY; merging the two observed gates of each tier into one chance **ASSUMED**; each item usable once **ASSUMED** (count not stored) |
| Switch AI (Change bit): "0x break" (78 % / 50 %) and "make better" (90 % stay, 4+ stages stay, 50 % immune / 33 % resist to the player's last move type) | `choose_switch` | gates VERIFIED values (`score_rules.tsv`); bench choice by `power x effectiveness x STAB` and the rank-up reading **ASSUMED** |
| Prize money on a win: `prize_base x level of the last opponent Pokémon` | `trainer_prize_assumed`; paid through `save_wire::apply_outcome` instead of the wild prize | **SUPUESTO / ASSUMED**: the game's formula and the reader of root slot 11 are UNKNOWN |
| Experience: every fainted opponent member gives `exp_yield(..., trainer = true)` to the active Pokémon at the end | `battle_progress::process_battle_end` | trainer bonus from the existing progression code; "active Pokémon is the only participant" **ASSUMED** |
| Giratina (`giratina01`): power x1.5 on Atk/Def/SpA/SpD/Spe and a second phase (rises again once at 100 % HP) | `TrainerBattleState::for_trainer`, `trainer_after_turn` | `grtnPowerCoef` 1.5 and `grtnReliveHpPer` 100 VERIFIED values; that they apply to this trainer and mean "revive once" **LIKELY** |
| Noble turn-based phase (one Pokémon from `boss_battles.tsv`; no capture, no running; `nsPowerCoef` = 1) | `ActiveBattleSession::start_noble` | Noble AI profile **ASSUMED** Basic + Strong; field phase (frenzy gauge, balms, field attacks) **not implemented** (balm damage and attack numbers UNKNOWN) |
| Battle UI: opponent card "LABEL [##-] 2/3  SPECIES Lv. N", player card with its own counter | `opponent_card_title`, `player_card_title`, `game/ui.rs` | — |
| Arena: the opponent's 3D model and cry follow the Pokémon that is out | `port_viewer.rs` `sync_arena_player_model` (slot 1), `battle_anim::slot_in`, `audio.rs` | — |
| Saved party installed into a trainer battle keeps the trainer's whole party | `save_wire::install_party` | — |
| Trainers in the world: cached real models at NPC placements whose model pack belongs to a trainer, with a red capsule fallback; V within 3 m starts the battle; `--trainer <id>` (or `noble:<setting>`) lets V start any battle; `--no-trainers` hides them; markers are hidden during a battle and a beaten model or fallback turns grey | `trainer_encounter.rs` (`TrainerEncounterPlugin`) | NPC -> trainer link **ASSUMED** (the real link is in the story event scripts, out of scope); coordinates used as stored like the object placements (PROBABLE same frame); model colours and pixel correctness remain **ASSUMED** |

Native functions touched by this work (for the evidence ledger): the trainer loader at `0x836564` (copies the trainer root fields into the trainer object) is partially mirrored by `pla::game::trainer_data::TrainerCatalog::load`; the party builder at `0x837504` (fills one Pokémon creation struct per member) is partially mirrored by `pla::game::trainer_data::TrainerMon::to_combatant`. Both read the decoded sheets, not the binary files, and neither is behaviour-verified.

## How to reproduce

```bash
cargo test -p pla --lib trainer                                     # 21 trainer tests (+1 affinity test: `battle_ai`)
cargo run -p pla --example battle_demo -- --trainer trainer09_01 --seed 3   # one battle, full log
cargo run -p pla --example battle_demo -- --trainer all --level 50          # every trainer, totals
cargo run -p pla --example battle_demo -- --trainer noble:nsi_ex_01_r       # a Noble rematch
```

All of them are windowless. In the viewer: walk to a cached trainer model, or its red capsule fallback, and press **V**; alternatively start with `--trainer trainer04_01` and press V anywhere.

## Defeated trainers (save v8)
Beaten trainers are saved (`PlayerProgress.defeated_trainers`, save format v8, see [save_game.md](save_game.md)). Decision: after a restart or Load the marker of a beaten trainer is grey, V next to it starts no battle and only shows the toast "<label> (<id>) was already defeated."; the prize is paid on the first win only. `--trainer <id>` (debug) still allows V to replay that trainer anywhere, but the rematch pays no prize. Headless test: `defeated_trainer_survives_save_and_load`.

## Not done / unknown

- Which event starts which trainer battle (story scripts, out of scope); trainer model visual/pixel fidelity and palette (**ASSUMED**); trainer intro/defeat texts (game texts out of scope; labels only).
- Money formula and the reader of slot 11; money lost on defeat (none is taken).
- Expert AI sequences; style choice of trainers beyond the shared selector.
- Noble field phase: gauge, balm hits, stagger windows, field attacks (`boss_field_attacks.tsv` is loaded nowhere yet).
- Send-out order, item counts, double battles (none found in the data).
- The trainer's battle music event (`bgm` column, e.g. `PLAY_BGM_VS_TRAINER_01`) is kept in the session but not wired to the audio plugin.

## What to check by eye

- The two battle cards show the trainer label and the `[###]` counters, and the counters drop as members faint.
- The opponent's 3D model and cry change when the trainer sends out the next Pokémon.
- A cached trainer model stands where the NPC belongs; if its cache entry is absent, the red capsule fallback stands there (on the ground, not floating or buried) in `ha_area00`..`ha_area05`. After victory, inspect the grey model tint or grey capsule fallback.
- The toast "V: battle ..." appears near a capsule; balls and Run are refused with a message.
