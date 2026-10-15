//! Windowless demonstration of the rival's item use (Brief D6): the real trainer battle
//! (`ActiveBattleSession` with the sheet trainer `trainer21_01`, Item bit on, Hyper Potion)
//! is played until the trainer uses its potion through the decoded `itemAI_001` branches, and
//! the battle log is printed. No Bevy app, no window.
//!
//! RUN: `cargo run --offline -p pla --example item_ai_demo`.

use bevy::prelude::Entity;
use pla::battle_engine::XorShift32;
use pla::game::battle_controller::{ActiveBattleSession, BattleAssets};
use pla::game::trainer_data::TrainerCatalog;
use std::path::PathBuf;

const DEMO_POTION: u16 = 17;

fn main() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let assets = BattleAssets::load(&root)?;
    let catalog = TrainerCatalog::load(&root)?;
    let trainer = catalog.get("trainer21_01").ok_or("trainer21_01 not in the catalog")?;
    println!(
        "=== Item AI: {} ({}), items {:?}, Item bit {} ===",
        trainer.id,
        trainer.display_label(),
        trainer.items,
        trainer.ai.item
    );
    for seed in 1..40 {
        let mut s = ActiveBattleSession::start_trainer(&assets, "pikachu", 25, trainer, Entity::PLACEHOLDER)?;
        s.rng = XorShift32(seed);
        let opp = s.opponent();
        // Hurt the active opponent to 15 % HP: the decoded rule turns positive (hp<20 branch).
        s.battle.combatants[opp].hp = (s.battle.combatants[opp].max_hp * 15) / 100;
        // Hurt the player's lead too, so the Potion actually heals and the turn is spent.
        let me = s.active_player();
        s.battle.combatants[me].hp = (s.battle.combatants[me].max_hp * 50) / 100;
        let before = s.battle.combatants[opp].hp;
        // Using a Potion on the player's side spends the turn; the trainer then acts.
        if !s.use_bag_item(DEMO_POTION) {
            let ids: Vec<u16> = s.bag_order.iter().map(|e| e.item_id).collect();
            return Err(format!("no Potion (17) in the bag; bag has {ids:?}").into());
        }
        if s.battle.combatants[opp].hp > before {
            println!(
                "seed {seed}: opponent {} {} -> {} of {}",
                s.battle.combatants[opp].species.name,
                before,
                s.battle.combatants[opp].hp,
                s.battle.combatants[opp].max_hp
            );
            for line in &s.log_lines {
                println!("  log: {line}");
            }
            return Ok(());
        }
    }
    Err("no seed used the potion (unexpected)".into())
}
