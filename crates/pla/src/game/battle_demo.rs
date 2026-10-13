//! Scripted battle beats for the headless `--auto-demo` run (inputs, optional HP setup, frame).
//!
//! The beats were written for the once-per-round flow, so the script runs the battle in the
//! classic flow unless `--pla-turns` (or `PLA_TURNS=pla`) is given explicitly
//! ([`demo_turn_mode`]); the game itself defaults to the PLA action queue.

use super::battle_controller::{ActiveBattleSession, BattleInput, TurnMode};

/// Turn flow of the scripted demo: classic unless PLA turns were asked for explicitly.
pub fn demo_turn_mode() -> TurnMode {
    let explicit = std::env::var("PLA_TURNS").ok().as_deref() == Some("pla")
        || std::env::args().any(|a| a == "--pla-turns");
    if explicit {
        TurnMode::Pla
    } else {
        TurnMode::Classic
    }
}

/// One scripted beat: optional setup, then inputs, then (later) a screenshot.
pub struct DemoBeat {
    pub note: &'static str,
    /// Lower the active player's HP to this percent of max before the inputs (demo setup only,
    /// so the medicine beat has something to heal; it is logged as such).
    pub player_hp_percent: Option<u32>,
    pub inputs: &'static [BattleInput],
    pub frame: &'static str,
}

/// Status move, item menu, medicine use, capture attempt: four frames.
pub const BEATS: [DemoBeat; 4] = [
    DemoBeat {
        note: "Status move: Thunder Wave (move slot 3) on the wild Pokemon",
        player_hp_percent: None,
        inputs: &[BattleInput::Slot(2)],
        frame: "02_battle_status_move.png",
    },
    DemoBeat {
        note: "ITEM menu: bag with quantities from the engine bag",
        player_hp_percent: Some(55),
        inputs: &[BattleInput::OpenItem],
        frame: "03_battle_item_menu.png",
    },
    DemoBeat {
        note: "Use the first medicine (HP restore) on the active Pokemon",
        player_hp_percent: None,
        inputs: &[BattleInput::Slot(0)],
        frame: "04_battle_medicine_used.png",
    },
    DemoBeat {
        note: "CAPTURE: throw the first ball in the bag, show the shakes",
        player_hp_percent: None,
        inputs: &[BattleInput::Capture],
        frame: "05_battle_capture_attempt.png",
    },
];

/// Run the setup and inputs of `beat` on the session; returns the log lines it added.
pub fn run_beat(sess: &mut ActiveBattleSession, beat: &DemoBeat) -> Vec<String> {
    let before = sess.log_lines.len();
    sess.turn_mode = demo_turn_mode();
    if let Some(pct) = beat.player_hp_percent {
        let i = sess.active_player();
        let c = &mut sess.battle.combatants[i];
        c.hp = (c.max_hp * pct / 100).max(1);
        sess.log_lines
            .push(format!("[demo setup] player HP set to {pct}% for the medicine beat"));
    }
    for input in beat.inputs {
        sess.handle_input(*input);
    }
    sess.battle_summary = super::battle_log::tail(&sess.log_lines);
    sess.log_lines[before..].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::BattleAssets;
    use bevy::prelude::Entity;
    use std::path::Path;

    #[test]
    fn script_covers_status_item_medicine_and_capture() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let Ok(assets) = BattleAssets::load(dir) else { return };
        let mut s =
            ActiveBattleSession::start(&assets, "pikachu", 25, "pikachu", 5, 25, Entity::PLACEHOLDER)
                .unwrap();
        let mut all = Vec::new();
        for b in &BEATS {
            all.extend(run_beat(&mut s, b));
        }
        let text = all.join("\n");
        assert!(text.contains("used thunder-wave"), "{text}");
        assert!(text.contains("BAG ("), "{text}");
        assert!(text.contains("regained"), "{text}");
        assert!(text.contains("catch rate"), "{text}");
        assert!(text.contains("Shake..."), "{text}");
    }
}
