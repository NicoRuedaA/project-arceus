//! Headless cross-layer battle demo: EXTERNAL national species vs VERIFIED Hisui species.
//!
//! `cargo run -p pla --example national_battle` (no window).  It loads `data/national/*.tsv`
//! through `BattleData` and proves, without any rendering, that a national species
//! (Charizard, Garchomp) fights a Hisuian species (Hisuian Decidueye) with the external
//! stats/types/learnset data and the engine's real damage formula.  All external rows are
//! EXTERNAL/PROBABLE; PLA rows keep winning for the species the game has.
//!
//! The same scenario runs as the unit test `national_species_battles_a_hisui_species_headless`
//! in `crates/pla/src/national.rs`; this binary is the human-readable companion.

use std::path::PathBuf;

use pla::battle::BattleData;
use pla::battle_engine::{Battle, Combatant, Event, XorShift32};

fn species_line(data: &BattleData, label: &str, dex: u16, form: u8) -> Combatant {
    let species = data
        .species_row_by_dex_form(dex, form)
        .expect("battle data loads")
        .unwrap_or_else(|| panic!("missing species {dex}/{form}"));
    println!(
        "{label}: #{} {} [{}/{}] stats {}/{}/{}/{}/{}/{} source={}",
        species.dex,
        species.name,
        species.types[0],
        species.types[1],
        species.hp,
        species.attack,
        species.defense,
        species.special_attack,
        species.special_defense,
        species.speed,
        if species.external {
            "EXTERNAL/PROBABLE (data/national)"
        } else {
            "VERIFIED (data/pml_personal)"
        },
    );
    Combatant::from_base(
        species,
        50,
        vec![
            data.move_row("air-slash").unwrap().unwrap(),
            data.move_row("flamethrower").unwrap().unwrap(),
        ],
    )
}

fn main() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let data = BattleData::load(&root)?;
    let national = data.national();
    println!(
        "national layer: {} species, {} form entries with pokemon_id, {} cosmetic variants, {} learnset rows, {} moves outside the engine",
        national.species.len(),
        national.forms.len(),
        national.variants.len(),
        national.learnsets.len(),
        national.missing_moves.len(),
    );

    let charizard = species_line(&data, "player (national)", 6, 0);
    let decidueye = {
        let species = data
            .species_row_by_dex_form(724, 1)
            .expect("battle data loads")
            .expect("Hisuian Decidueye");
        println!(
            "opponent (Hisui): #{} {} [{}/{}] stats {}/{}/{}/{}/{}/{} source={}",
            species.dex,
            species.name,
            species.types[0],
            species.types[1],
            species.hp,
            species.attack,
            species.defense,
            species.special_attack,
            species.special_defense,
            species.speed,
            if species.external {
                "EXTERNAL/PROBABLE (data/national)"
            } else {
                "VERIFIED (data/pml_personal)"
            },
        );
        Combatant::from_base(
            species,
            50,
            vec![data.move_row("leaf-blade").unwrap().unwrap()],
        )
    };
    let dragapult = data
        .species_row("dragapult")
        .expect("battle data loads")
        .expect("Dragapult entry");
    let total = |s: &pla::battle::BattleSpecies| {
        u32::from(s.hp)
            + u32::from(s.attack)
            + u32::from(s.defense)
            + u32::from(s.special_attack)
            + u32::from(s.special_defense)
            + u32::from(s.speed)
    };
    println!(
        "second external check: #{} {} [{}/{}] total {} (EXTERNAL/PROBABLE)",
        dragapult.dex,
        dragapult.name,
        dragapult.types[0],
        dragapult.types[1],
        total(&dragapult),
    );
    if let Some(garchomp) = data.species_row("garchomp").expect("battle data loads") {
        println!(
            "priority check: #{} {} total {} source={} (Garchomp IS in Legends: Arceus, so the game row wins)",
            garchomp.dex,
            garchomp.name,
            total(&garchomp),
            if garchomp.external {
                "EXTERNAL/PROBABLE"
            } else {
                "VERIFIED (data/pml_personal)"
            },
        );
    }

    let mut battle = Battle::new(
        vec![charizard, decidueye],
        vec![vec![0], vec![1]],
        data.type_chart().clone(),
    )?;
    battle.set_pla_damage_formula(true);
    let mut rng = XorShift32(0x4e41_54);
    let mut super_effective_hits = 0;
    let mut total_damage = 0_u32;
    for _ in 0..12 {
        if battle.ended {
            break;
        }
        let mut actions = Vec::new();
        for actor in [0_usize, 1] {
            if let Some(action) = battle.best_damage_action(actor) {
                actions.push(action);
            }
        }
        if actions.is_empty() {
            break;
        }
        let events = battle.play_turn(&actions, &mut rng);
        for event in &events {
            if let Event::UsedMove {
                actor,
                target,
                name,
                damage,
                effectiveness,
                ..
            } = event
            {
                total_damage += damage;
                if effectiveness == "super effective" {
                    super_effective_hits += 1;
                }
                println!(
                    "  {} used {name} on {}: {damage} damage ({effectiveness}); target HP {}/{}",
                    battle.combatants[*actor].species.name,
                    battle.combatants[*target].species.name,
                    battle.combatants[*target].hp,
                    battle.combatants[*target].max_hp,
                );
            }
        }
    }
    println!(
        "battle finished={} turns={} total_damage={} super_effective_hits={} winner_side={:?}",
        battle.ended,
        battle.turn,
        total_damage,
        super_effective_hits,
        battle.winner_side()
    );
    if total_damage == 0 || super_effective_hits == 0 {
        return Err("the cross-layer battle produced no effective damage".into());
    }
    println!("ok: national (EXTERNAL) vs Hisui (VERIFIED) data drives the engine without a window");
    Ok(())
}
