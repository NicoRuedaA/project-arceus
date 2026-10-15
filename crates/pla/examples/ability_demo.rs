//! Windowless demonstration of the two abilities that have an effect in this game (Brief D1):
//! Slow Start (112, Regigigas) and Flower Gift (122, Cherrim). It runs the real battle
//! session (`ActiveBattleSession`, no Bevy app, no window) and prints its battle log, so the
//! effect can be captured without a viewer window.
//!
//! Slow Start: the player is a Regigigas; while the counter is below 5 its physical moves
//! deal about half damage and its turn order drops. Run the same command with
//! `PLA_ABILITIES=off` for the baseline; the move, the foe and the RNG seed are the same.
//!
//! Flower Gift: the battle weather is not wired from the world yet (Brief D1 risk), so this
//! part sets `Battle::weather` directly and prints the form-change events. RUN:
//! `cargo run --offline -p pla --example ability_demo` (same command line limit rules as
//! every other heavy job).

use bevy::prelude::Entity;
use pla::battle_engine::abilities::{AbilityEvent, AbilityRules};
use pla::battle_engine::{BattleWeather, XorShift32};
use pla::game::battle_controller::{ActiveBattleSession, BattleAssets};
use std::path::PathBuf;

fn physical_slot(session: &ActiveBattleSession) -> Option<usize> {
    let active = session.active_player();
    session.battle.combatants[active]
        .moves
        .iter()
        .position(|m| m.data.category == "Physical" && m.pp > 0)
}

fn slow_start_demo(assets: &BattleAssets) -> Result<(), String> {
    // Snorlax: high HP and slow, so both sides survive the five turns. Wild dex 143.
    let mut s = ActiveBattleSession::start(assets, "regigigas", 30, "snorlax", 30, 143, Entity::PLACEHOLDER)?;
    s.rng = XorShift32(0x00d1_0001);
    println!(
        "=== Slow Start: player REGIGIGAS (L30) vs wild SNORLAX (L30), ability rules {:?} ===",
        s.battle.ability_rules()
    );
    for turn in 1..=6 {
        if s.battle.ended {
            break;
        }
        let Some(slot) = physical_slot(&s) else { break };
        let events = s.execute_player_attack(slot);
        let damaged = events.iter().find_map(|e| match e {
            pla::battle_engine::Event::UsedMove { damage, .. } if *damage > 0 => Some(*damage),
            _ => None,
        });
        println!("turn {turn}: player physical damage {:?}", damaged);
    }
    for line in &s.log_lines {
        println!("  log: {line}");
    }
    Ok(())
}

fn flower_gift_demo(assets: &BattleAssets) -> Result<(), String> {
    let mut s = ActiveBattleSession::start(assets, "cherrim", 30, "snorlax", 30, 143, Entity::PLACEHOLDER)?;
    s.rng = XorShift32(0x00d1_0002);
    // The game asks for the form change when the weather is sun; the port does not wire the
    // world weather into battles yet, so the demo sets it directly.
    s.battle.weather = BattleWeather::Sun;
    s.battle.set_abilities(AbilityRules::from_env());
    let events: Vec<AbilityEvent> = s.battle.take_ability_events();
    println!("=== Flower Gift: CHERRIM with sun, ability events {events:?} ===");
    for line in s
        .battle
        .ability_rules()
        .flower_gift
        .then(|| pla::game::battle_log::ability_lines(&|i| s.battle.combatants[i].species.name.to_uppercase(), &events))
        .unwrap_or_default()
    {
        println!("  log: {line}");
    }
    let Some(slot) = physical_slot(&s) else { return Ok(()) };
    s.execute_player_attack(slot);
    for line in &s.log_lines {
        println!("  log: {line}");
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let assets = BattleAssets::load(&root)?;
    slow_start_demo(&assets)?;
    println!();
    flower_gift_demo(&assets)
}
