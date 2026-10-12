use pla::battle::BattleData;
use pla::battle_engine::{Action, Battle, Combatant, Event, XorShift32};
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let data = BattleData::load(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"))?;
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
        vec![get_move("thunder-shock")?, get_move("quick-attack")?],
    );
    let staraptor = Combatant::from_base(
        get_species("staraptor")?,
        12,
        vec![get_move("wing-attack")?, get_move("quick-attack")?],
    );
    let mut battle = Battle::new(vec![pikachu, staraptor], vec![vec![0], vec![1]])?;
    let mut rng = XorShift32(0x504c_4126);
    println!(
        "{} ({} HP) vs {} ({} HP)",
        battle.combatants[0].species.name,
        battle.combatants[0].hp,
        battle.combatants[1].species.name,
        battle.combatants[1].hp
    );
    while !battle.ended {
        let log = battle.play_turn(
            &[
                Action {
                    actor: 0,
                    move_slot: 0,
                },
                Action {
                    actor: 1,
                    move_slot: 0,
                },
            ],
            &mut rng,
        );
        println!("Turn {}", battle.turn);
        for event in log {
            match event {
                Event::ActedFirst(actor) => {
                    println!("  {} acts first", battle.combatants[actor].species.name)
                }
                Event::UsedMove {
                    actor,
                    target,
                    name,
                    damage,
                    critical,
                    effectiveness,
                } => println!(
                    "  {} used {} on {}: {} damage{} ({}) — HP {}/{}",
                    battle.combatants[actor].species.name,
                    name,
                    battle.combatants[target].species.name,
                    damage,
                    if critical { " (critical)" } else { "" },
                    effectiveness,
                    battle.combatants[target].hp,
                    battle.combatants[target].max_hp
                ),
                Event::Missed {
                    actor,
                    target,
                    name,
                } => println!(
                    "  {} missed {} with {}",
                    battle.combatants[actor].species.name,
                    battle.combatants[target].species.name,
                    name
                ),
                Event::KnockedOut(index) => println!(
                    "  {} was knocked out",
                    battle.combatants[index].species.name
                ),
                Event::BattleEnded { winner_side } => match winner_side {
                    Some(side) => println!(
                        "Winner: side {side} ({})",
                        battle.combatants[battle.sides[side][0]].species.name
                    ),
                    None => println!("Draw"),
                },
                _ => {}
            }
        }
    }
    Ok(())
}
