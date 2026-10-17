//! Windowless test scenario for the main-series battle gimmicks (port extension, see
//! `battle_engine::gimmicks`). Each scene runs the real battle session (`ActiveBattleSession`,
//! no Bevy app, no window) through the same inputs the player uses (`Y` arms the gimmick, a
//! digit uses the move) and prints the battle log, so every mechanic can be checked by eye.
//! Only the checked-in `data/` sheets are needed: no game dump, no keys.
//!
//! Scenes: `mega`, `z`, `dynamax`, `tera`, `protect`, `rule` (one gimmick per battle), or
//! `all` (default). RUN: `cargo run --offline -p pla --example gimmick_demo -- [scene]`.

use bevy::prelude::Entity;
use pla::battle::BattleData;
use pla::battle_engine::effects::MoveEffectsTable;
use pla::battle_engine::gimmicks::{Gimmick, GimmickRequest};
use pla::battle_engine::statuses::StatusRules;
use pla::battle_engine::{Action, Battle, Combatant, Event, XorShift32};
use pla::game::battle_controller::{ActiveBattleSession, BattleAssets, BattleInput, TurnMode};
use std::path::{Path, PathBuf};

const SEED: u32 = 0x0617_0001;

fn session(assets: &BattleAssets, gimmick: Gimmick, player: &str, level: u8) -> Result<ActiveBattleSession, String> {
    let mut a = assets.clone();
    a.gimmick = Some(gimmick);
    // Classic rounds: both sides act once per input, so each beat reads as one turn.
    a.turn_mode = TurnMode::Classic;
    // Snorlax: bulky, so the foe survives every scene. Wild dex 143.
    let mut s = ActiveBattleSession::start(&a, player, level, "snorlax", level, 143, Entity::PLACEHOLDER)?;
    s.rng = XorShift32(SEED);
    Ok(s)
}

fn slot_of(s: &ActiveBattleSession, name: &str) -> usize {
    let me = s.active_player();
    s.battle.combatants[me].moves.iter().position(|m| m.data.name == name).unwrap_or(0)
}

/// Highlight `slot`, press Y, then use the move.
fn gimmick_move(s: &mut ActiveBattleSession, slot: usize) {
    s.selected_move_idx = slot;
    s.handle_input(BattleInput::Gimmick);
    s.handle_input(BattleInput::Slot(slot));
}

fn stats(s: &ActiveBattleSession) -> String {
    stats_of(s, s.active_player())
}

fn stats_of(s: &ActiveBattleSession, i: usize) -> String {
    let c = &s.battle.combatants[i];
    format!(
        "HP {}/{} Atk {} Def {} SpA {} SpD {} Spe {} types {:?}",
        c.hp, c.max_hp, c.attack, c.defense, c.special_attack, c.special_defense, c.speed, c.species.types
    )
}

fn print_log(s: &ActiveBattleSession) {
    for line in &s.log_lines {
        println!("  log: {line}");
    }
}

fn mega(assets: &BattleAssets) -> Result<(), String> {
    println!("=== MEGA EVOLUTION: player LUCARIO (L40) vs wild SNORLAX ===");
    let mut s = session(assets, Gimmick::Mega, "lucario", 40)?;
    println!("before: {}", stats(&s));
    gimmick_move(&mut s, 0);
    println!("after:  {}  (mega: {})", stats(&s), s.battle.is_mega(s.active_player()));
    s.handle_input(BattleInput::Slot(0));
    println!("turn 2: still mega: {}", s.battle.is_mega(s.active_player()));
    print_log(&s);
    Ok(())
}

fn z_move(assets: &BattleAssets) -> Result<(), String> {
    println!("=== Z-MOVE: player PIKACHU (L30) Thunderbolt vs Gigavolt Havoc on wild SNORLAX ===");
    let mut plain = session(assets, Gimmick::ZMove, "pikachu", 30)?;
    let mut z = session(assets, Gimmick::ZMove, "pikachu", 30)?;
    let bolt = slot_of(&plain, "thunderbolt");
    plain.handle_input(BattleInput::Slot(bolt));
    gimmick_move(&mut z, bolt);
    let damage = |s: &ActiveBattleSession| {
        s.event_log.iter().find_map(|e| match e {
            Event::UsedMove { actor, damage, .. } if *actor == s.active_player() => Some(*damage),
            _ => None,
        })
    };
    println!("damage: plain {:?}, Z-Move {:?} (same seed)", damage(&plain), damage(&z));
    println!("second Z attempt (once per battle):");
    gimmick_move(&mut z, bolt);
    print_log(&z);
    Ok(())
}

fn dynamax(assets: &BattleAssets) -> Result<(), String> {
    println!("=== DYNAMAX: player PIKACHU (L30) vs wild SNORLAX, 4 turns ===");
    let mut s = session(assets, Gimmick::Dynamax, "pikachu", 30)?;
    let me = s.active_player();
    println!("before: {}", stats(&s));
    let bolt = slot_of(&s, "thunderbolt");
    gimmick_move(&mut s, bolt);
    println!("turn 1 (Max Lightning): {} turns left {:?}", stats(&s), s.battle.dynamax_turns(me));
    s.handle_input(BattleInput::Slot(slot_of(&s, "thunder-wave")));
    println!("turn 2 (status move -> Max Guard): turns left {:?}", s.battle.dynamax_turns(me));
    s.handle_input(BattleInput::Slot(slot_of(&s, "iron-tail")));
    println!("turn 3 (Max Steelspike, +Def stage {:?}): turns left {:?}", s.stages_of(me), s.battle.dynamax_turns(me));
    s.handle_input(BattleInput::Slot(slot_of(&s, "quick-attack")));
    println!("turn 4 (back to normal, PIKACHU): {} dynamaxed: {}", stats_of(&s, me), s.battle.dynamax_turns(me).is_some());
    print_log(&s);
    Ok(())
}

fn tera(assets: &BattleAssets) -> Result<(), String> {
    println!("=== TERASTALLIZATION: player PIKACHU (L30), Tera Electric (matches its type) ===");
    let mut plain = session(assets, Gimmick::Terastal, "pikachu", 30)?;
    let mut t = session(assets, Gimmick::Terastal, "pikachu", 30)?;
    let bolt = slot_of(&plain, "thunderbolt");
    plain.handle_input(BattleInput::Slot(bolt));
    gimmick_move(&mut t, bolt);
    let dealt = |s: &ActiveBattleSession| {
        s.event_log.iter().find_map(|e| match e {
            Event::UsedMove { actor, damage, .. } if *actor == s.active_player() => Some(*damage),
            _ => None,
        })
    };
    println!("thunderbolt damage: plain {:?}, Tera {:?} (STAB x1.25 -> x5/3)", dealt(&plain), dealt(&t));
    println!("after: {}", stats(&t));
    print_log(&t);

    println!("--- Tera Fire (new type): PIKACHU's own types become Fire (new weaknesses), Electric keeps its STAB ---");
    let mut f = session(assets, Gimmick::Terastal, "pikachu", 30)?;
    let me = f.active_player();
    f.battle.set_tera_type(me, "Fire");
    let quick = slot_of(&f, "quick-attack");
    gimmick_move(&mut f, quick);
    println!("after: {}", stats(&f));
    print_log(&f);
    Ok(())
}

/// Engine-level: the foe protects; the Z-Move still lands for 1/4.
fn protect(data_root: &Path) -> Result<(), String> {
    println!("=== PROTECT vs Z-MOVE (engine): PIKACHU Gigavolt Havoc into GENGAR's Protect ===");
    let d = BattleData::load(data_root)?;
    let mk = |d: &BattleData, sp: &str, mv: &str| -> Result<Combatant, String> {
        let s = d.species_row(sp)?.ok_or("species")?;
        Ok(Combatant::from_base(s, 40, vec![d.move_row(mv)?.ok_or("move")?]))
    };
    for protected in [false, true] {
        let mut gengar = mk(&d, "gengar", "protect")?;
        gengar.speed = 999;
        let mut b = Battle::new(
            vec![mk(&d, "pikachu", "thunderbolt")?, gengar],
            vec![vec![0], vec![1]],
            d.type_chart().clone(),
        )?;
        b.set_move_effects(MoveEffectsTable::load(data_root)?);
        b.set_status_rules(StatusRules::game_rules());
        b.enable_gimmicks(Some(Gimmick::ZMove));
        b.give_z_crystal(0, "Electric");
        let mut actions = vec![Action { actor: 0, move_slot: 0 }];
        if protected {
            actions.push(Action { actor: 1, move_slot: 0 });
        }
        let req = [GimmickRequest { actor: 0, gimmick: Gimmick::ZMove }];
        let events = b.play_turn_with_gimmicks(&actions, &req, &mut XorShift32(SEED));
        let dmg = events.iter().find_map(|e| match e {
            Event::UsedMove { actor: 0, damage, .. } => Some(*damage),
            _ => None,
        });
        println!("protect {protected}: damage {dmg:?}, gimmick events {:?}", b.take_gimmick_events());
    }
    Ok(())
}

fn rule(assets: &BattleAssets, data_root: &Path) -> Result<(), String> {
    println!("=== RULE: one gimmick per battle ===");
    let d = BattleData::load(data_root)?;
    let mk = |sp: &str| -> Result<Combatant, String> {
        let s = d.species_row(sp)?.ok_or("species")?;
        Ok(Combatant::from_base(s, 30, vec![d.move_row("thunderbolt")?.ok_or("move")?]))
    };
    let mut b = Battle::new(vec![mk("pikachu")?, mk("gengar")?], vec![vec![0], vec![1]], d.type_chart().clone())?;
    b.enable_gimmicks(None);
    println!("free choice; before any use the battle gimmick is {:?}", b.battle_gimmick());
    let tera = [GimmickRequest { actor: 0, gimmick: Gimmick::Terastal }];
    b.play_turn_with_gimmicks(&[Action { actor: 0, move_slot: 0 }], &tera, &mut XorShift32(SEED));
    println!("side 0 Terastallized; the battle is now locked to {:?}", b.battle_gimmick());
    for g in Gimmick::ALL {
        println!("  foe asks for {:<16} -> {:?}", g.label(), b.gimmick_refusal(1, g, Some(0)));
    }
    println!("  side 0 asks again for Tera   -> {:?}", b.gimmick_refusal(0, Gimmick::Terastal, Some(0)));

    println!("--- in the battle screen: a Mega battle, but PIKACHU has no Mega form ---");
    let mut s = session(assets, Gimmick::Mega, "pikachu", 30)?;
    gimmick_move(&mut s, 0);
    print_log(&s);
    Ok(())
}

fn main() -> Result<(), String> {
    let data_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let assets = BattleAssets::load(&data_root)?;
    let scene = std::env::args().nth(1).unwrap_or_else(|| "all".into());
    let all = scene == "all";
    let mut ran = false;
    let mut run = |name: &str, f: &dyn Fn() -> Result<(), String>| -> Result<(), String> {
        if all || scene == name {
            ran = true;
            f()?;
            println!();
        }
        Ok(())
    };
    run("mega", &|| mega(&assets))?;
    run("z", &|| z_move(&assets))?;
    run("dynamax", &|| dynamax(&assets))?;
    run("tera", &|| tera(&assets))?;
    run("protect", &|| protect(&data_root))?;
    run("rule", &|| rule(&assets, &data_root))?;
    if !ran {
        return Err(format!("unknown scene {scene}: mega, z, dynamax, tera, protect, rule or all"));
    }
    Ok(())
}
