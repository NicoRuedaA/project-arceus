use pla::battle::BattleData;
use pla::battle_engine::effects::MoveEffectsTable;
use pla::battle_engine::statuses::StatusRules;
use pla::battle_engine::styles::{MoveStyleTable, Style, StyledAction, TURN_ORDER_PREVIEW_LEN};
use pla::battle_engine::{Action, Battle, Combatant, Event, HealingItem, XorShift32};
use pla::game::battle_log;
use pla::game::battle_style_ai::choose_ai_style;
use pla::game::battle_turns::turn_order_lines;
use std::io::{self, Write};
use std::path::PathBuf;

fn arg_value(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

/// `--trainer <id> [--seed N] [--level L] [--classic-turns]`: a windowless, non-interactive
/// trainer battle (Brief 30). The player's demo party (Pikachu + two bench members at level L)
/// always uses its best damaging move; the trainer uses its AI module bits. `--trainer
/// noble:<setting>` runs a Noble's turn-based phase. `--trainer all` plays every trainer once
/// and prints the totals.
fn trainer_mode(root: &PathBuf, id: &str) -> Result<(), String> {
    use pla::game::battle_controller::{ActiveBattleSession, BattleAssets};
    use pla::game::trainer_data::{NobleCatalog, TrainerCatalog};
    let assets = BattleAssets::load(root)?;
    let catalog = TrainerCatalog::load(root)?;
    let nobles = NobleCatalog::load(root)?;
    let seed: u32 = arg_value("--seed").and_then(|v| v.parse().ok()).unwrap_or(1);
    let level: u8 = arg_value("--level").and_then(|v| v.parse().ok()).unwrap_or(50);
    let entity = bevy::prelude::Entity::PLACEHOLDER;
    let start = |id: &str| -> Result<ActiveBattleSession, String> {
        match id.strip_prefix("noble:") {
            Some(n) => ActiveBattleSession::start_noble(&assets, "pikachu", level, nobles.get(n).ok_or("unknown noble")?, entity),
            None => ActiveBattleSession::start_trainer(&assets, "pikachu", level, catalog.get(id).ok_or("unknown trainer")?, entity),
        }
    };
    let play = |s: &mut ActiveBattleSession| -> usize {
        s.rng = XorShift32(seed.max(1));
        let mut inputs = 0;
        while !s.battle.ended && inputs < 500 {
            inputs += 1;
            let p = s.active_player();
            let slot = s.battle.best_damage_action(p).map_or(0, |a| a.move_slot);
            if s.execute_player_attack(slot).is_empty() && !s.battle.ended {
                let next = s.battle.sides[0].iter().copied().find(|&i| i != p && s.battle.combatants[i].is_alive());
                match next {
                    Some(n) => {
                        s.switch_to(n);
                    }
                    None => break,
                }
            }
        }
        inputs
    };
    if id == "all" {
        let (mut won, mut lost, mut draw, mut open, mut prize) = (0, 0, 0, 0, 0u64);
        for t in catalog.trainers.keys() {
            let mut s = start(t)?;
            play(&mut s);
            match s.battle.winner_side() {
                Some(Some(0)) => won += 1,
                Some(Some(_)) => lost += 1,
                Some(None) => {
                    draw += 1;
                    println!("draw: {t} (both sides fainted on the same turn)");
                }
                None => {
                    open += 1;
                    println!("unfinished: {t} after {} turns", s.battle.turn);
                }
            }
            prize += u64::from(s.trainer.as_ref().and_then(|x| x.prize).unwrap_or(0));
        }
        println!(
            "trainers played: {}  player won: {won}  player lost: {lost}  draw: {draw}  unfinished: {open}  (player party L{level}, seed {seed})",
            catalog.len()
        );
        println!("total prize money paid (ASSUMED formula prize_base x last level): {prize}");
        return Ok(());
    }
    let mut s = start(id)?;
    let inputs = play(&mut s);
    for l in &s.log_lines {
        println!("{l}");
    }
    let t = s.trainer.as_ref().ok_or("not a trainer battle")?;
    println!("---");
    println!(
        "trainer {} ({}), AI {}, team {}, player inputs {inputs}, winner side {:?}, prize {:?} (ASSUMED formula)",
        t.id,
        t.label,
        t.ai.profile(),
        s.battle.sides[1].len(),
        s.battle.winner_side(),
        t.prize
    );
    Ok(())
}

fn main() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    if let Some(id) = arg_value("--trainer") {
        return trainer_mode(&root, &id);
    }
    let data = BattleData::load(&root)?;
    let get_species = |name: &str| -> Result<_, String> {
        data.species_row(name)?
            .ok_or_else(|| format!("missing species {name}"))
    };
    let get_move = |name: &str| -> Result<_, String> {
        data.move_row(name)?
            .ok_or_else(|| format!("missing move {name}"))
    };
    let pikachu = Combatant::from_base(
        get_species("pikachu")?,
        12,
        vec![
            get_move("thunder-shock")?,
            get_move("quick-attack")?,
            get_move("confuse-ray")?,
            get_move("solar-beam")?,
        ],
    );
    let eevee = Combatant::from_base(
        get_species("eevee")?,
        12,
        vec![get_move("quick-attack")?, get_move("tackle")?],
    );
    let staraptor = Combatant::from_base(
        get_species("staraptor")?,
        12,
        vec![get_move("wing-attack")?, get_move("quick-attack")?],
    );
    let mut battle = Battle::new(
        vec![pikachu, eevee, staraptor],
        vec![vec![0, 1], vec![2]],
        data.type_chart().clone(),
    )?;
    battle.set_move_effects(MoveEffectsTable::load(&root)?);
    battle.set_status_rules(StatusRules::game_rules());
    // `--pla-turns`: the PLA action-order queue (one combatant acts per step) with agile /
    // strong styles. Without it the demo keeps the old once-per-round flow.
    let pla_turns = std::env::args().any(|a| a == "--pla-turns");
    if pla_turns {
        battle.set_move_styles(MoveStyleTable::load(&root)?);
        battle.enable_action_queue();
        println!("PLA turns ON: `move <slot> [agile|strong]`; the opponent acts when the queue picks it.");
    }
    let mut rng = XorShift32(0x504c_4126);
    let potion = HealingItem {
        name: "Potion (demo)".into(),
        healing: 20,
    };
    println!(
        "Partial PLA battle-rules demo (not complete). Commands: move <slot>, switch <party slot>, heal, quit."
    );
    println!(
        "Verified (see reports/data-progress/battle_rules_verified.md): effort stats, critical probabilities, type categories. Damage formula: this demo enables the game rules, which select the real game formula (the engine default is still the old series formula, which the game does not use). Rules checked against the game code: 18 verified, 7 corrected, 4 unknown."
    );
    println!(
        "Nature mapping, native action classes/AI, and catch rates are unknown inputs; defaults are neutral. Healing amount and party controls are demo-only."
    );
    println!(
        "Game rules are ON (confusion, trapping, charge turns, Protect, drowsiness, status rates, rank multipliers, real damage formula). Pikachu slots 3/4 are confuse-ray and solar-beam."
    );

    while !battle.ended {
        if pla_turns {
            let names = |i: usize| battle.combatants[i].species.name.clone();
            println!("Turn order: {}", turn_order_lines(&battle.turn_order_preview(TURN_ORDER_PREVIEW_LEN), &names, 0).join(" | "));
            let enemy = battle.active_combatant(1).ok_or("missing opponent combatant")?;
            if battle.next_actor() == Some(enemy) {
                let player = battle.active_combatant(0).ok_or("missing player combatant")?;
                if let Some(ai) = battle.best_damage_action(enemy) {
                    let style = choose_ai_style(&battle, data.type_chart(), enemy, ai.move_slot, player);
                    let events = battle.play_turn_styled(&[StyledAction { action: ai, style }], &mut rng);
                    print_events(&mut battle, &events);
                } else {
                    battle.advance_action_queue(enemy);
                }
                continue;
            }
        }
        let active = battle
            .active_combatant(0)
            .ok_or("missing player combatant")?;
        let enemy = battle
            .active_combatant(1)
            .ok_or("missing opponent combatant")?;
        println!(
            "\n{} {}/{} HP vs {} {}/{} HP",
            battle.combatants[active].species.name,
            battle.combatants[active].hp,
            battle.combatants[active].max_hp,
            battle.combatants[enemy].species.name,
            battle.combatants[enemy].hp,
            battle.combatants[enemy].max_hp
        );
        print!(
            "Your action [{}]: ",
            battle.combatants[active]
                .moves
                .iter()
                .enumerate()
                .map(|(i, slot)| format!(
                    "{}:{} ({}/{})",
                    i + 1,
                    slot.data.name,
                    slot.pp,
                    slot.data.pp
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
        io::stdout().flush().map_err(|error| error.to_string())?;
        let mut command = String::new();
        if io::stdin()
            .read_line(&mut command)
            .map_err(|error| error.to_string())?
            == 0
        {
            break;
        }
        let words: Vec<_> = command.split_whitespace().collect();
        let mut style = Style::Normal;
        let player_action: Option<Action> = match words.first().copied() {
            Some("quit") => break,
            Some("move") => {
                let slot = words
                    .get(1)
                    .and_then(|n| n.parse::<usize>().ok())
                    .unwrap_or(1)
                    .saturating_sub(1);
                if slot >= battle.combatants[active].moves.len() {
                    println!("No such move slot.");
                    continue;
                }
                style = match words.get(2).copied() {
                    Some("agile") => Style::Agile,
                    Some("strong") => Style::Strong,
                    _ => Style::Normal,
                };
                Some(Action {
                    actor: active,
                    move_slot: slot,
                })
            }
            Some("switch") => {
                let party_slot = words
                    .get(1)
                    .and_then(|n| n.parse::<usize>().ok())
                    .unwrap_or(0)
                    .saturating_sub(1);
                let replacement = *battle.sides[0]
                    .get(party_slot)
                    .ok_or("no such party slot")?;
                println!("{:?}", battle.switch(0, replacement)?);
                None
            }
            Some("heal") => {
                println!("{:?}", battle.use_healing_item(0, &potion)?);
                None
            }
            _ => {
                println!("Use move <slot>, switch <party slot>, heal, or quit.");
                continue;
            }
        };
        if pla_turns {
            match player_action {
                Some(action) => {
                    let events = battle.play_turn_styled(&[StyledAction { action, style }], &mut rng);
                    print_events(&mut battle, &events);
                }
                None => battle.advance_action_queue(active),
            }
            continue;
        }
        let mut actions = Vec::new();
        if let Some(action) = player_action {
            actions.push(action);
        }
        let enemy_actor = battle
            .active_combatant(1)
            .ok_or("missing opponent combatant")?;
        if let Some(ai_action) = battle.best_damage_action(enemy_actor) {
            actions.push(ai_action);
        }
        let events = battle.play_turn(&actions, &mut rng);
        print_events(&mut battle, &events);
    }
    Ok(())
}

fn print_events(battle: &mut Battle, events: &[Event]) {
    for event in events {
        match event {
            Event::ActedFirst(actor) => {
                println!("{} acts first", battle.combatants[*actor].species.name)
            }
            Event::UsedMove {
                actor,
                target,
                name,
                damage,
                critical,
                effectiveness,
            } => println!(
                "{} used {} on {}: {} damage{} ({}) — HP {}/{}",
                battle.combatants[*actor].species.name,
                name,
                battle.combatants[*target].species.name,
                damage,
                if *critical { " (critical)" } else { "" },
                effectiveness,
                battle.combatants[*target].hp,
                battle.combatants[*target].max_hp
            ),
            Event::Missed {
                actor,
                target,
                name,
            } => println!(
                "{} missed {} with {}",
                battle.combatants[*actor].species.name,
                battle.combatants[*target].species.name,
                name
            ),
            Event::KnockedOut(index) => {
                println!("{} was knocked out", battle.combatants[*index].species.name)
            }
            Event::BattleEnded { winner_side } => match winner_side {
                Some(side) => println!("Winner: side {side}"),
                None => println!("Draw"),
            },
            _ => {}
        }
    }
    let statuses = battle.take_status_events();
    let rules = battle.take_rule_events();
    let styles = battle.take_style_events();
    let names = |i: usize| battle.combatants[i].species.name.clone();
    for event in &statuses {
        if let Some(line) = battle_log::format_status(&names, event) {
            println!("{line}");
        }
    }
    for event in &rules {
        println!("{}", battle_log::format_rule(&names, event));
    }
    for event in &styles {
        if let Some(line) = battle_log::format_style(&names, event) {
            println!("{line}");
        }
    }
}
