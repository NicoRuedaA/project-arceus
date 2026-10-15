# Money from trainer battles (external-note check, 2026-10-15)

An external, unverified note (kept outside git, in `work/notas_externas/`) claims that in
Pokémon Legends: Arceus **trainer battles pay no money** (experience only), except the Miss
Fortune Sisters, whose rematches give sellable Nuggets. This report contrasts that hint with the
game data and the public record, and records the port rule that followed.

## What the game data shows

- `data/trainer_index.tsv` exports root slot 11 (10..80 per trainer; 80 on all `single`
  challenge battles, rising with each story rematch). The field is **copied but never read**:
  `reports/data-progress/trainers.md` §Money found no reader, and a re-check of the corpus
  confirms it (no read of the trainer's `+0x458` field; the money API `FUN_010171a4` get,
  `FUN_010171c8` add, `FUN_010171f4` subtract has no battle-range caller: its only visible add
  callers are the event-command dispatcher `FUN_01297b20` (case 0x88 = "add money", an event
  script command) and a UI panel; the subtract callers are UI/shop paths).
- The battle-standard message table `btl_std.tbl` (542 entries) contains
  `BTL_STRID_STD_GetMoney`, `PickMoney`, `LoseMoneyWild` and `LoseMoneyTrainer`, so a money
  flow exists somewhere in the game, but no link from a trainer win to a payment was found.

## What the public record says

- Trainer battles in PLA are described as "all but removed" compared with older games, with
  money coming from reporting captures, selling items and requests (Digital Trends, "How to make
  money fast in Pokémon Legends: Arceus").
- Ingo's Training Ground battles (the dojo): "Your Pokémon receive full experience from these
  battles, but only when the battle ends" — no money mentioned (Serebii, Ingo's Battles).
- The Miss Fortune Sisters give "up to four nuggets ... 40,000 Pokédollars" on a rematch win
  (Digital Trends), matching the external note's 4 Nuggets (wins 1-10) / 5 Nuggets (11+).
- Requests "won't give you money as a reward, but will often give you items" (Gamer Guides).

## Decision (port rule)

- **Trainer battles pay no money.** `save_wire::apply_outcome` records the win in
  `defeated_trainers` and adds no money; the assumed `prize_base x level` formula and the
  `TrainerBattleState.prize` field were removed, and the end line no longer reports a prize.
  The `prize_base` column stays in the sheet as raw exported data with no known reader.
- Wild battles keep the existing provisional prize (`victory_reward`, 50 + 10 x level,
  ASSUMED): the note says nothing about wild battles and no game data contradicts it; it stays
  a port placeholder, flagged here as an open question.
- The Miss Fortune Sisters' Nugget reward is **not modelled**: their battles exist in the data
  (`trainer01`-`trainer03`, variants 11-20 = the 10 rematch variants), but the port has no
  random-rematch trigger or win counter to hang the 4/5 Nuggets on. PROBABLE, documented only.

## Confidence

- No reader of the trainer money field: **VERIFIED** (corpus reading).
- "Trainer battles pay no money" (the port rule): **PROBABLE** (external note + public
  sources, consistent with the game data; a negative cannot be proven from the data alone).
- The field's real meaning: **UNKNOWN**.
- Sisters' 4/5 Nuggets: **PROBABLE** (external note + one public source for "up to four").

## Professor Laventon's reports (note section 5.2)

The report payment **exists in the game** (you show captures to the professor at the Galaxy
Hall; the public record lists total caught, 10/20/30+ batches, species variety, first-time
catches, back strikes, space-time distortions and Alphas as the criteria, with the Galaxy rank
as a multiplier). Because it exists in the game, this section is **not** a new port mechanic:
per the check instruction it is documented here only. The port feeds the equivalent counters
already (catches, moves used, research tasks, `Progress.note_research`); a "report" screen and
its money payment are not implemented and are out of this task's scope.

## Tests

- `save_wire::trainer_wins_pay_no_money_and_are_remembered_once`, plus the updated
  `win_pays_the_prize_through_the_real_engine_snapshot` (wild) show the split rule.
- `trainer_encounter::walk_up_press_v_battle_and_remember_the_win_without_paying` and
  `defeated_trainer_survives_save_and_load` assert the purse never moves across a trainer win,
  a reload and a restart.
- `trainer_battle::three_vs_three_runs_to_the_end_deterministically` asserts the battle log
  carries no money line.
