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

fn main() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
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
