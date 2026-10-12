//! Turn-based battle controller connecting to pla::battle_engine.

use bevy::prelude::*;
use crate::battle::BattleData;
use crate::battle_engine::{Action, Battle, CaptureModifiers, Combatant, Event, XorShift32};

/// Placeholder catch rate (0-255): the real per-species value is UNKNOWN in the typed tables.
const CAPTURE_CATCH_RATE_PLACEHOLDER: u8 = 45;

/// Menu selection state inside battle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleMenuState {
    MainMenu,    // Fight, Switch, Bag, Run
    MoveSelect,  // 4 moves
    ExecutingTurn,
}

#[derive(Resource)]
pub struct ActiveBattleSession {
    pub battle: Battle,
    pub wild_entity: Entity,
    pub wild_dex: u16,
    pub rng: XorShift32,
    pub menu_state: BattleMenuState,
    pub selected_menu_idx: usize,
    pub selected_move_idx: usize,
    pub event_log: Vec<Event>,
    pub current_turn: u32,
    pub battle_summary: String,
    pub turn_timer: f32,
}

impl ActiveBattleSession {
    pub fn new(
        battle_data: &BattleData,
        player_species: &str,
        player_level: u8,
        wild_species: &str,
        wild_level: u8,
        wild_dex: u16,
        wild_entity: Entity,
    ) -> Result<Self, String> {
        let p_spec = battle_data
            .species_row(player_species)?
            .ok_or_else(|| format!("unknown player species {player_species}"))?;
        let w_spec = battle_data
            .species_row(wild_species)?
            .ok_or_else(|| format!("unknown wild species {wild_species}"))?;

        // Standard move loadouts from real PML moves
        let p_moves = match player_species {
            "pikachu" => vec![
                battle_data.move_row("thunderbolt")?.unwrap(),
                battle_data.move_row("quick-attack")?.unwrap(),
                battle_data.move_row("iron-tail")?.unwrap(),
                battle_data.move_row("spark")?.unwrap(),
            ],
            _ => vec![
                battle_data.move_row("tackle")?.unwrap(),
                battle_data.move_row("quick-attack")?.unwrap(),
            ],
        };

        let w_moves = match wild_species {
            "pikachu" => vec![
                battle_data.move_row("spark")?.unwrap(),
                battle_data.move_row("quick-attack")?.unwrap(),
            ],
            "bidoof" => vec![
                battle_data.move_row("tackle")?.unwrap(),
                battle_data.move_row("bite")?.unwrap(),
            ],
            "starly" => vec![
                battle_data.move_row("quick-attack")?.unwrap(),
                battle_data.move_row("tackle")?.unwrap(),
            ],
            _ => vec![battle_data.move_row("tackle")?.unwrap()],
        };

        let player_combatant = Combatant::from_base(p_spec, player_level, p_moves);
        let wild_combatant = Combatant::from_base(w_spec, wild_level, w_moves);

        let battle = Battle::new(
            vec![player_combatant, wild_combatant],
            vec![vec![0], vec![1]],
            battle_data.type_chart().clone(),
        )?;

        Ok(Self {
            battle,
            wild_entity,
            wild_dex,
            rng: XorShift32(0x5ca1_e001),
            menu_state: BattleMenuState::MainMenu,
            selected_menu_idx: 0,
            selected_move_idx: 0,
            event_log: Vec::new(),
            current_turn: 0,
            battle_summary: "Battle Started!".into(),
            turn_timer: 0.0,
        })
    }

    /// Select an attack move for the player and execute a turn against the wild Pokémon.
    pub fn execute_player_attack(&mut self, move_slot: usize) -> Vec<Event> {
        let player_action = Action {
            actor: 0,
            move_slot,
        };

        // Wild Pokémon selects an attack action
        let wild_moves_len = self.battle.combatants[1].moves.len();
        let wild_move_slot = if wild_moves_len > 0 {
            (self.rng.0 as usize) % wild_moves_len
        } else {
            0
        };
        let wild_action = Action {
            actor: 1,
            move_slot: wild_move_slot,
        };

        let events = self.battle.play_turn(&[player_action, wild_action], &mut self.rng);
        self.event_log.extend(events.clone());
        self.current_turn = self.battle.turn;

        // Summarize latest turn events
        let mut summary_lines = Vec::new();
        for ev in &events {
            match ev {
                Event::TurnStarted(t) => summary_lines.push(format!("Turn {t}:")),
                Event::UsedMove { actor, target: _, name, damage, critical, effectiveness } => {
                    let actor_name = &self.battle.combatants[*actor].species.name;
                    let crit_tag = if *critical { " (Critical Hit!)" } else { "" };
                    summary_lines.push(format!(
                        "{actor_name} used {name}! ({damage} dmg{crit_tag}, {effectiveness})"
                    ));
                }
                Event::Missed { actor, name, .. } => {
                    let actor_name = &self.battle.combatants[*actor].species.name;
                    summary_lines.push(format!("{actor_name}'s {name} missed!"));
                }
                Event::KnockedOut(actor) => {
                    let actor_name = &self.battle.combatants[*actor].species.name;
                    summary_lines.push(format!("{actor_name} fainted!"));
                }
                Event::BattleEnded { winner_side } => {
                    if let Some(winner) = winner_side {
                        let win_label = if *winner == 0 { "Player won!" } else { "Wild won!" };
                        summary_lines.push(format!("Battle Ended: {win_label}"));
                    } else {
                        summary_lines.push("Battle Ended: Draw".into());
                    }
                }
                _ => {}
            }
        }
        self.battle_summary = summary_lines.join(" | ");
        events
    }

    /// Attempt to catch the wild Pokémon using a Poké Ball during battle.
    pub fn attempt_capture_in_battle(&mut self, battle_data: &BattleData) -> Result<bool, String> {
        let res = self.battle.attempt_capture(
            battle_data,
            1, // target is wild Pokémon (actor 1)
            "poke_ball",
            true, // suitable range
            1,    // player galaxy rank
            1,    // pokemon rank
            // Catch rate is not in the typed species table (UNKNOWN input): fixed placeholder.
            CAPTURE_CATCH_RATE_PLACEHOLDER,
            false,// backstrike
            CaptureModifiers::default(),
            &mut self.rng,
        )?;

        if res {
            self.battle.combatants[1].caught = true;
            self.battle.ended = true;
            self.battle_summary = "Wild Pokémon caught in Poké Ball!".into();
        } else {
            self.battle_summary = "Wild Pokémon broke free!".into();
        }

        Ok(res)
    }
}
