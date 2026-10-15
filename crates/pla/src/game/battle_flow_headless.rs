//! Headless Bevy test of a whole battle session with the real systems (no window, no
//! renderer): the save wiring installs the saved party, `battle_input_system` drives the
//! battle from logical actions, and after the win the experience, level-up, move prompt and
//! evolution run before the session closes; the autosave then writes them to a current-format (v6) save file
//! that is read back.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;

use super::battle_controller::{ActiveBattleSession, BattleAssets, BattleMenuState, TurnMode, WILD};
use super::battle_input::battle_input_system;
use super::input_map::{actions, Action, ActionState, RawInput};
use super::save_game::{PartyMember, CURRENT_VERSION, SaveFile, SaveGame, SavedMove};
use super::save_wire::{PlayerProgress, SaveWirePlugin, StartupState, WireCli};
use super::ui::ActiveZoneInfo;
use super::{GameState, PlayerAvatar};
use crate::battle_engine::progression::{exp_for_level, ProgressionTables};

fn data_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pla-battleflow-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Saved Shinx one experience point short of level 15 (its evolution level and the level
/// of thunder-fang), knowing four moves.
fn shinx(t: &ProgressionTables) -> PartyMember {
    let exp = exp_for_level(t, t.growth_group(403, 0), 15) - 1;
    PartyMember {
        species: 403,
        form: 0,
        level: 14,
        hp: 40,
        status: None,
        // quick-attack, thunder-shock, bite, tackle
        moves: vec![
            SavedMove { move_id: 98, pp: 20 },
            SavedMove { move_id: 84, pp: 20 },
            SavedMove { move_id: 44, pp: 20 },
            SavedMove { move_id: 33, pp: 20 },
        ],
        effort: [0; 6],
        nickname: Some("Sparky".into()),
        exp: Some(exp),
    }
}

fn app_with_save(assets: &BattleAssets, dir: &Path, party: Vec<PartyMember>) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin));
    app.init_state::<GameState>();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
    app.insert_resource(assets.clone());
    app.init_resource::<ActionState>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.insert_resource(ActiveZoneInfo { area: "ha_area01".into(), name: "Obsidian Fieldlands".into() });
    let mut progress = PlayerProgress::new_game();
    progress.party = party;
    progress.bag = super::save_wire::default_bag(&assets.items);
    let start = StartupState {
        progress,
        area: Some(1),
        zone: Some("ha_area01".into()),
        position: None,
        yaw: 0.0,
        time_minutes: None,
        loaded: false,
        message: String::new(),
        play_seconds: 0,
    };
    SaveWirePlugin::install(&mut app, SaveGame::new(SaveFile::in_dir(dir)), start, &WireCli::default());
    app.add_systems(Update, battle_input_system);
    app.world_mut().spawn((PlayerAvatar, Transform::from_xyz(1.0, 2.0, 3.0)));
    app
}

/// One frame with exactly these actions just pressed.
fn press(app: &mut App, pressed: &[Action]) {
    *app.world_mut().resource_mut::<ActionState>() = actions(&RawInput { key_pressed: pressed.to_vec(), ..default() });
    app.update();
    *app.world_mut().resource_mut::<ActionState>() = ActionState::default();
}

fn sess(app: &App) -> &ActiveBattleSession {
    app.world().resource::<ActiveBattleSession>()
}

#[test]
fn win_exp_level_up_move_prompt_evolution_and_save_round_trip() {
    let Ok(mut assets) = BattleAssets::load(data_root()) else { return };
    let Some(t) = assets.progression.clone() else { return };
    assets.turn_mode = TurnMode::Pla;
    let dir = tmpdir("full");
    let mut app = app_with_save(&assets, &dir, vec![shinx(&t)]);
    app.update(); // startup systems
    // The encounter: the demo session is replaced by the saved party on the next frame.
    let session = ActiveBattleSession::start(&assets, "pikachu", 25, "bidoof", 5, 399, Entity::PLACEHOLDER).unwrap();
    app.insert_resource(session);
    app.update();
    assert_eq!(sess(&app).battle.combatants[0].species.dex, 403, "saved Shinx installed");
    assert!(sess(&app).uses_queue(), "PLA turn order after install_party");
    assert_eq!(sess(&app).turn_order_lines().len(), 6);
    app.world_mut().resource_mut::<ActiveBattleSession>().battle.combatants[WILD].hp = 1;

    // Fight with move 4 (tackle) until the wild Pokemon faints.
    for _ in 0..6 {
        if sess(&app).battle.ended {
            break;
        }
        let mut s = app.world_mut().resource_mut::<ActiveBattleSession>();
        let max = s.battle.combatants[0].max_hp;
        s.battle.combatants[0].hp = max;
        press(&mut app, &[Action::Slot(3)]);
    }
    let s = sess(&app);
    assert!(s.battle.ended, "{:?}", s.log_lines);
    let log = s.log_lines.join("\n");
    assert!(log.contains("Battle over: you won!"), "{log}");
    assert!(log.contains("SHINX gained "), "{log}");
    assert!(log.contains("SHINX grew to level 15!"), "{log}");
    assert_eq!(s.menu_state, BattleMenuState::LearnMove);

    // The prompt keeps the battle open however long it waits.
    for _ in 0..30 {
        app.update();
    }
    assert!(app.world().get_resource::<ActiveBattleSession>().is_some(), "prompt open, no exit");
    press(&mut app, &[Action::Slot(3)]); // forget tackle
    let s = sess(&app);
    assert!(s.log_lines.iter().any(|l| l.contains("forgot tackle and learned thunder-fang")), "{:?}", s.log_lines);
    assert_eq!(s.menu_state, BattleMenuState::Evolving);

    // 100 ms per frame: the evolution completes after EVOLVE_SECONDS.
    for _ in 0..30 {
        app.update();
        if !sess(&app).progress.busy() {
            break;
        }
    }
    let s = sess(&app);
    assert_eq!(s.battle.combatants[0].species.dex, 404, "{:?}", s.log_lines);
    assert!(s.log_lines.iter().any(|l| l.contains("evolved into LUXIO")));

    // Leave the battle (Confirm), then the autosave writes the file.
    press(&mut app, &[Action::Confirm]);
    assert!(app.world().get_resource::<ActiveBattleSession>().is_none(), "session closed");
    app.update();
    let progress = app.world().resource::<PlayerProgress>();
    let m = &progress.party[0];
    assert_eq!((m.species, m.form, m.level), (404, 0, 15));
    assert_eq!(m.nickname.as_deref(), Some("Sparky"), "nickname kept");
    assert!(m.moves.iter().any(|mv| mv.move_id == 422), "thunder-fang saved: {:?}", m.moves);
    assert!(!m.moves.iter().any(|mv| mv.move_id == 33), "tackle forgotten");
    let exp = m.exp.expect("experience is saved");
    assert!(exp >= exp_for_level(&t, t.growth_group(404, 0), 15));
    assert_eq!(progress.defeated, 1);

    let loaded = SaveFile::in_dir(&dir).read().expect("autosave written");
    assert_eq!(loaded.version, CURRENT_VERSION);
    assert_eq!(loaded.party[0], *m, "the file holds exactly the in-memory member");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn classic_flag_keeps_the_old_round_flow_headless() {
    let Ok(mut assets) = BattleAssets::load(data_root()) else { return };
    let Some(t) = assets.progression.clone() else { return };
    assets.turn_mode = TurnMode::Classic;
    let dir = tmpdir("classic");
    let mut app = app_with_save(&assets, &dir, vec![shinx(&t)]);
    app.update();
    let session = ActiveBattleSession::start(&assets, "pikachu", 25, "bidoof", 5, 399, Entity::PLACEHOLDER).unwrap();
    app.insert_resource(session);
    app.update();
    {
        let mut s = app.world_mut().resource_mut::<ActiveBattleSession>();
        assert!(!s.uses_queue());
        s.battle.combatants[WILD].max_hp = 5_000;
        s.battle.combatants[WILD].hp = 5_000;
    }
    press(&mut app, &[Action::Slot(3)]);
    let s = sess(&app);
    assert_eq!(s.battle.turn, 1, "one round with both actions");
    assert!(s.turn_order_lines().is_empty());
    assert!(s.log_lines.iter().any(|l| l.starts_with("SHINX")), "{:?}", s.log_lines);
    assert!(s.log_lines.iter().any(|l| l.starts_with("Wild BIDOOF")), "{:?}", s.log_lines);
    let _ = std::fs::remove_dir_all(&dir);
}
