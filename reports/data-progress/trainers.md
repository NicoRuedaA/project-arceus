# Trainer battles, trainer AI and Noble boss battles (data research)

Scope: Pokémon Legends: Arceus update v262144. Research and data only, for Brief 30 (trainer and boss battles), Brief D5 (Noble battles) and Brief D6 (items used by the opponent). Nothing here is implemented in the game yet. Evidence comes from reading the game data files (FlatBuffers structure), the battle AI script `battle.blua` (decompiled locally, outside git), and targeted disassembly of `main.elf` (file offset = address + 0x1000). Nothing was run in the game or in an emulator. No game code, pseudocode or game text was copied; label names (message ids) are identifiers, not texts.

Confidence words: **VERIFIED** (checked against bytes or code for every row), **LIKELY** (strong indirect evidence), **ASSUMED** (a guess that fits), **UNKNOWN**.

## How to regenerate

```bash
python3 .tools/trainer_decode.py        # data/trainer_index.tsv, data/trainer_teams.tsv
python3 .tools/trainer_boss_decode.py   # data/boss_battles.tsv, data/boss_field_attacks.tsv
cargo run -p pla --example trainer_report -- trainer32_05   # windowless summary, reads only the sheets
```

Inputs: `bin/trainer/trdata_*.bin`, `bin/message/English/common/{trname,trtype,btl_loss}.tbl` (labels only), `bin/misc/app_config/event_restriction_battle.bin`, `bin/pokemon/data/{poke_nushi_battle_setting,poke_event_encount,poke_field_obstruction_waza_ns_legend}.bin`, `bin/capture/throwable_param_table.bin`, and a gfdump cache of `resident_release.gfpak` (for the character table; `PLA_RESIDENT_CACHE`, default `work/scratch-spawn/resident`). Without the game files the tools print `[skip]`.

## Results in numbers

| Item | Count |
|---|---|
| Trainer files decoded | **377 / 377** (all files, every root slot known) |
| Party members | **620** (party sizes: 1×269, 2×26, 3×52, 4×10, 5×17, 6×3) |
| Trainer → name label / class label / loss-message label | 377 / 371 / 377 (6 files of two groups use a name label missing from the table) |
| Trainer → character id + model pack (`tr00NN_00_<name>`) | **377 / 377** |
| `single_NNN` → species restriction | **242 / 242** |
| AI module flags per trainer | 377 / 377 (mapping LIKELY, see below) |
| Trainers carrying bag items | 10 (items 23, 24, 25) |
| Noble settings | 16 (8 battles + 8 rematches), all joined to their Pokémon |
| Noble / legend field attacks | 109 |

## 1. Trainer file format (`bin/trainer/trdata_<id>.bin`)

One FlatBuffers table per file; slot 22 is the party vector. Slot use was checked against the loader at `0x836564` (copies the root fields into a trainer object) and the party builder at `0x837504` (fills a Pokémon creation struct per member).

| Root slot | Content | Confidence |
|---|---|---|
| 0 | u64 FNV-1a 64 of the trainer id (`trainer01_01`, `single_001`) | VERIFIED (377/377) |
| 1 | u64 name label hash in `trname.tbl` (e.g. `CAPTAIN_GANKUTSU`, `BANDIT_FIRST`) | VERIFIED (371/377 resolve) |
| 2 | u64 class label hash in `trtype.tbl` (`TRTYPE_...`) | VERIFIED (371/377) |
| 3 | battle music event (`PLAY_BGM_VS_TRAINER_01/02`, `PLAY_BGM_VS_BOSS_03`) | VERIFIED |
| 4, 5, 6 | battle effect sequence ids (`ee3NN`, `ee5NN`, `ee62N` = `bin/effect/battle/sequence/<id>.wbin`); 4 = intro and 5 = defeat (an `ee561` particle is named `grtina_lose`) | files VERIFIED; roles LIKELY; slot 6 role UNKNOWN |
| 7 | u64 label hash in `btl_loss.tbl` (`str_btl_seq_TrainerNN_MM`, the trainer's defeat line) | VERIFIED (377/377) |
| 8 | u64 character hash; resolves in the character table of `resident_release.gfpak` (239 entries, slot 0 hash, slot 12 model pack) and in NPC placement files `bin/field/param/placement/<area>/npc/npc.bin` | VERIFIED (377/377) |
| 9 | table of 4 floats, copied to a vec3 + float of the trainer object | copy VERIFIED; "battle position x, y, z + yaw" LIKELY |
| 10 | int stored as a bool | copy VERIFIED; meaning UNKNOWN |
| 11 | int 5..80 (80 on all `single`, 10..60 on story trainers, rising with each rematch) | copy VERIFIED; "prize money base" ASSUMED, no reader found |
| 12..15 | ints; only 12 is used, values 23/24/25 | LIKELY bag items (see D6) |
| 16..21 | six bools copied side by side | LIKELY AI modules (see section 3) |
| 22 | vector of party members | VERIFIED |

| Member slot | Content | Confidence |
|---|---|---|
| 0, 1 | species, form | VERIFIED (values match species ids; e.g. Basculegion form 1) |
| 2..5 | move tables {0 move id, 1 bool} | ids VERIFIED; bool = move mastered LIKELY |
| 6 | level | LIKELY (fills the level of the creation struct; values 5..100 fit) |
| 7 | nature 0..24 (12 on most members) | LIKELY |
| 8 | sex 0/1 (female-only Basculegion form 1 has 1) | LIKELY |
| 9..14 | effort levels; slot 12 goes to the last (speed) entry of the struct, so file order is HP, Atk, Def, Spe, SpA, SpD | LIKELY |
| 15 | bool, never set; the builder turns it into a rare (shiny) mode value | LIKELY shiny flag |
| 16 | bool, set on 3 members (Kleavor, Wyrdeer, Basculegion in `trainer04_04`) | UNKNOWN |

There is **no held item** per member, which agrees with `abilities_held_items.md` (held items are vestigial). **No battle type field** (single/double) exists; the AI "double" checks in the script never apply, and no double battle trainer was found: all battles are single (LIKELY).

### Who the trainers are (internal labels only)

| Group (files) | Name label | Model pack | Notes |
|---|---|---|---|
| `single` (242) | CAPTAIN_GANKUTSU | tr0018_00_captain7 | each is the value of `restriction_btl_poke_<species>` in `event_restriction_battle.bin`; one party member, level 70, prize 80. Native report keys `shuzoku_battle_*` ("species battle"). LIKELY species-restricted challenge battles |
| trainer01/02/03 (12/11/12) | BANDIT_FIRST/SECOND/THIRD | tr0033..35 bandit | |
| trainer04 (4) | CAPTAIN_GANKUTSU | tr0018_00_captain7 | |
| trainer05, 34 | CAPTAIN_YAMA | tr0019_00_captain8 | |
| trainer06 | CAPTAIN_YASHIRO | tr0021_00_captain10 | |
| trainer07, 37 | CAPTAIN_SETSUGEN | tr0020_00_captain9 | |
| trainer08, 31 | GINGA_NINJA | tr0010_00_ninja2 | trainer08 carries item 24 |
| trainer09, 32 | GINGA_BOSS | tr0006_00_boss2 | carry items 23-25 |
| trainer10 (3) | MERCHANT ×2, POKEMONTAMER ×1 | tr0007_00_si1 / tr0008_00_si2 | trainer10_01 carries item 23, 6 members L68 |
| giratina01 | POKEMONTAMER | tr0008_00_si2 | Giratina L70, boss music `PLAY_BGM_VS_BOSS_03` |
| trainer11, 35 | DIAMOND_BOSS | tr0003_00_ouji | |
| trainer12, 38 | PEARL_BOSS | tr0004_00_hime | |
| trainer13 / 14 (11 each) | SURVEYF / SURVEYM | tr0037_00_senpai_f / tr0036_00_senpai_m | variants `04A/B/C`, `05A/B/C` differ only in one starter-line member: ASSUMED chosen by the player's starter; the two groups ASSUMED to depend on the player's gender |
| trainer15, 30 | SECURITY_CHIEF | tr0022_00_guard_leader | |
| trainer16, 21 | SECURITYM / GATEKEEPER | tr0023_00_guard_lower_m | trainer21_01 carries item 25 |
| trainer17 | SECURITYF | tr0024_00_guard_lower_f | |
| trainer18 | CAPTAIN_NOHARA | tr0012_00_captain1 | |
| trainer19, 36 | label not in the table | tr0013_00_captain2 | UNKNOWN name label |

Groups 30..38 have 5 files each (`_01`..`_05`) with higher levels: LIKELY rematch tiers. Party order is the stored order; whether it is the send-out order is UNKNOWN (the strongest member is sometimes first, sometimes last).

Join with NPC data: the character hash (slot 8) appears in 21 `npc.bin` placement files of areas 00..06 and their subareas, so a placed NPC can be matched to its trainer model. Mapping a placed NPC to a specific `trdata` file (which battle starts) is in the event scripts (`trigger_chapN.bin` reference `trainer01_01` etc.) and is out of scope here (story); the NPC placement itself belongs to the NPC data task.

## 2. Rewards and progression

- **Money formula: UNKNOWN.** The game has the messages `BTL_STRID_STD_GetMoney`, `LoseMoneyTrainer` and telemetry keys `start_money`/`end_money`/`get_money` (`0xcaba94`, `0xcacd30`, report-only code), but no code that reads slot 11 was found: the trainer object field it is copied to (`+0x458`) has no plain load anywhere else. Slot 11 as "prize base" is ASSUMED; the franchise formula (base × level of the last Pokémon) is NOT verified for this game.
- **Item rewards: none in the trainer data** (no reward slot). Rewards, if any, come from event scripts (out of scope).
- **Bag items (D6):** `trainer08_01`, `09_01` (item 24), `09_03`, `10_01`, `32_03..05` (item 23), `21_01`, `32_01`, `32_02` (item 25); one item each. In `item_effects.tsv` 23 heals and cures every status, 24 and 25 are HP-healing medicine of decreasing strength (LIKELY Full Restore / Max Potion / Hyper Potion; exact heal amounts not re-checked here). Exactly these 10 trainers have the 5th AI bool set. Item counts per trainer are not stored (UNKNOWN how many times one can be used).

## 3. Trainer AI

The script `battle.blua` exports six AI modules: `WazaBasicAI`, `WazaStrongAI`, `WazaExpertAI`, `WazaAllowanceAI`, `ItemUseAI`, `PokeChangeAI`. The native dispatcher at `0x7d7b58` runs the move modules from four adjacent bytes (+0x78 Basic, +0x79 Strong, +0x7a Expert, +0x7b Allowance) — VERIFIED from the code that calls each module by name. Trainer slots 16..21 are six adjacent bools; reading them as Basic, Strong, Expert, Allowance, ItemUse, PokeChange is **LIKELY** (order and count match; the ItemUse bit is set on exactly the 10 trainers that carry items). Coverage:

| Profile | Trainers |
|---|---|
| Basic + Strong | 167 |
| Basic + Strong + Expert | 149 |
| Basic + Strong + Expert + Change | 49 |
| Basic + Strong + Expert + Item + Change | 10 |
| Basic only | 2 |

Allowance is never used by trainers. Decision structure (per turn, from the script and `data/ai/score_rules.tsv`, `expert_sequences.tsv`):

1. **Moves:** every usable move starts from a base score; each enabled module adds deltas (Basic: affinity 0 → −10, move-group vetoes; Strong: not the max-damage move −1, high affinity +2 at 70 %; Expert: per-move sequence handlers `ExpertAI_Seq_NNN` chosen by the move's AI sequence number). The highest score wins.
2. **Switch** (`PokeChangeAI`, only with the Change bit): switch when the active Pokémon cannot hit the foe well and a bench member resists or hits harder (50–78 % gates; bench damage ≥160/200/240 adds +2/+3/+4).
3. **Items** (`ItemUseAI`, only with the Item bit): dispatch by item number — 23 → `itemAI_023`, 24/25 (and 17, 26) → `itemAI_001`. `itemAI_001`: HP under 20 % → likely use (+10, random gates 50/66 %); under 40 % → lower chance (27 % gates); otherwise −20. `itemAI_023` adds checks for status/confusion/bad poison. Exact scores are for Brief D6 to tabulate.
4. **Style (Agile/Strong):** chosen natively with the `modeChangeAi_*` config weights (normal 10; strong −10 unnecessary / +20 can knock out / −6 behind / +6 not behind / +10 danger; agile −10 cannot knock out / +15 can act again first), see `battle_styles.md`. The trainer data has no style field; every trainer uses the same selector (LIKELY).

## 4. Noble (frenzied boss) battles and other bosses

`data/boss_battles.tsv` joins three tables by name: the arena setting (`poke_nushi_battle_setting.bin`), the Pokémon (`poke_event_encount.bin`, entry with the same name: VERIFIED join 16/16) and the balm item (setting slot 26 = a `shizumedama` row of `throwable_param_table.bin`: VERIFIED 16/16).

| Setting | Script | Pokémon | Level (rematch) | Slot 16 "gauge" | Balm item | Field attacks |
|---|---|---|---|---|---|---|
| nsi_ex_01 | Ns02FieldBattle | Kleavor form 1 | 18 (70) | 1000 | 1678 | 4 |
| dredear01 | Ns04FieldBattle | Lilligant form 2 | 30 (70) | 2000 | 1639 | 4 |
| nsi_ex_06 | Ns06FieldBattle | Arcanine form 2 | 36 (70) | 3000 | 1721 | 11 |
| nsi_ex_04 | Ns05FieldBattle | Electrode form 2 | 46 (70) | 4000 | 1722 | 9 |
| nsi_ex_03 | Ns03FieldBattle | Avalugg form 2 | 56 (70) | 5000 | 1723 | 45 |
| nsi_ex_07 | Ns07FieldBattle | Dialga form 1 | 65 (85) | 10000 | 1608 | 8 |
| nsi_ex_08 | Ns08FieldBattle | Palkia form 1 | 65 (85) | 10000 | 1609 | 4 |
| nsi_ex_09 | Ns09FieldBattle | Arceus | 75 (100) | 15000 | 1610 | 14 |

- **Gauge:** event-encounter slot 16 rises with the story order and is absent on rematches: **LIKELY the frenzy gauge capacity**. Balm damage per hit is **UNKNOWN** (not in the balm rows, nor in `battle_vsns_config.bin`, which only has debug overrides `NsAp*`/`NsFallDown*` with a 0..100 range and the aura colours). Audio states `SET_STATE_VS_NUSHI_AP_0/10/…/50/MAX` show the game tracks the gauge in percent bands (VERIFIED strings).
- **Phases:** the field attack table (`data/boss_field_attacks.tsv`) holds per-noble attacks `nsNN_spMM` with variants carrying an `h` (e.g. `ns02_sp03h`): ASSUMED a second, harder phase. Kinds: bombing 80, throwing 10, charge_attack 6, barrier 6, following 4, hyper_beam 3; knock-down strength `down_strong` 74 / `down_weak` 34; status `confuse` 2, `sleepiness` 1. Numbers in slots 8/11/17/18 are UNKNOWN (sizes, damage or timing).
- **Sequence of a Noble battle** (from strings and the existing N2 notes, LIKELY): field phase (dodge attacks, throw balms that drain the gauge) → stagger window (`NUSHI_NO_SUKI` / `DEFENSELESS` audio states) → throwing the player's Pokémon during the window opens a turn-based battle (`PLAY_UI_BATTLE_OPEN_NUSHI`) → winning stuns the Noble → more balms → gauge 0 → `result_nushi`. Rematch entries (`_r`) have fixed levels 70/85/100, effort level 10 everywhere and full movesets: LIKELY plain turn-based rematches (event scripts `nsrematch*.blua`).
- **Other boss rules (battle_logic_config, VERIFIED values):** alpha (`oybn`) Pokémon: power ×1.5, `oybnQuotDamageCoef` 0.5; Giratina (`grtn`, the `giratina01` trainer): power ×1.5, damage coef 0.5 and `grtnReliveHpPer` 100 (LIKELY: revives once at 100 % HP for a second phase). Semi-legend config: `hiidoran_barrier_reduce_duration_*` 20/34/50 for three throwables (UNKNOWN units).
- **Unexplained:** `nsi_ex_03` uses sound bank `PM_N_751.bnk` while its Pokémon is 713. Slots 13..15 of the noble encounters (64/48/36; 112/96/68 on rematches) and slot 32 (3) are UNKNOWN; IV slots 26..31 are −1 (LIKELY random).

## 5. Implementation plan for Brief 30 (read-only hook list)

Current engine facts: `battle_engine.rs` `Battle` already has `sides: Vec<Vec<usize>>`, `active_combatant(side)`, `switch(side, replacement)`, `winner_side()`, `use_healing_item(side, item)`; `Combatant::with_effort_nature` takes effort levels and nature. The controller (`game/battle_controller.rs`) hard-codes the opponent as combatant `WILD = 1` (line ~26) and `resolve_turn` (~383) always asks `choose_ai_action(&battle, data, WILD, …)` (`game/battle_ai.rs` ~49).

Steps:

1. **Data loader** (new `game/trainer_data.rs`): read `data/trainer_index.tsv` + `trainer_teams.tsv` into `TrainerDef { id, label, model_pack, bgm, ai: [bool; 6], items: Vec<u16>, prize_base, party: Vec<TrainerMon> }`. Build each `Combatant` with `with_effort_nature(species, level, moves, gv, nature)`; effort-level order from the sheet (HP, Atk, Def, Spe, SpA, SpD).
2. **Session start** (`ActiveBattleSession::start` ~239): add `start_trainer(def)` that pushes N opponent combatants into side 1 instead of one wild; replace every `WILD` use with `battle.active_combatant(1)`.
3. **Forced replacement** (`resolve_turn` ~383, after `play_turn`): if the opponent's active fainted and `winner_side()` is `None`, pick the next living member in stored order (send-out order UNKNOWN; stored order is the fallback) and call `battle.switch(1, next)`; log "sent out" via `battle_log`.
4. **No capture / no flee:** guard `throw_ball` (~519), `attempt_capture_in_battle` (~624), `handle_input(BattleInput::Capture)` (~805) and `run_away` (~617) with a `is_trainer` flag (reject with a log line).
5. **AI hooks** (`game/battle_ai.rs` `choose_ai_action`): pass the trainer's six module flags; Basic/Strong already mapped; add Expert sequences from `expert_sequences.tsv` when understood; add a `choose_switch` (Change bit) and `choose_item` (Item bit, Brief D6, using `Battle::use_healing_item` on side 1).
6. **Victory / defeat:** on `winner_side() == Some(Some(0))` grant the prize (formula UNKNOWN: keep `prize_base × last level` as an ASSUMED placeholder behind a flag) into `save_game.rs` `money`; on defeat, follow the existing wild-loss flow. Hook in the session end path of `battle_controller.rs` and `game/save_wire.rs`.
7. **UI** (`game/ui.rs`): party counter for side 1 (pokéballs row) and the trainer label. **Audio:** the sheet's `bgm` names the battle music event.
8. **Bosses:** Giratina = trainer battle + `grtn` revive rule; Nobles need the separate action system of Brief D5 (gauge model from `boss_battles.tsv`, attacks from `boss_field_attacks.tsv`, balm hits from the Brief 18 throw), then a turn-based battle with the noble Pokémon of the sheet.

Tests to write then: 3-vs-3 headless battle to the end with a fixed seed, forced replacement order, capture/flee rejected, item AI with a low-HP opponent.

## Unknown (summary)

Money formula and the reader of slot 11; root slot 10 and slot 6 sequence role; member slot 16; send-out order; balm damage per hit and stagger timings; meaning of noble encounter slots 3, 13..15, 32 and attack numbers; the name label of groups 19/36; which event starts which trainer battle (story, out of scope).
