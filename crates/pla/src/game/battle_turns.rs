//! Turn flow switch (PLA action-order queue vs the old once-per-round flow) and the text of
//! the HUD turn-order list. Pure, no Bevy systems.
//!
//! - [`TurnMode::Pla`] (default): the engine's action-order queue decides who acts
//!   (`Battle::next_actor`, brief 29). The lowest wait acts alone, so a fast Pokemon or an
//!   agile move can act twice in a row. The player picks a style per move (Q agile, E strong).
//! - [`TurnMode::Classic`]: the old flow, both sides act once per round (`Battle::play_turn`),
//!   no styles. Selected with `--classic-turns` or `PLA_TURNS=classic`, kept for comparison.

use crate::battle_engine::styles::{Style, TurnOrderEntry};

/// How a battle decides who acts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TurnMode {
    /// PLA action-order queue with agile / strong styles.
    #[default]
    Pla,
    /// Old once-per-round flow (both sides act every turn), no styles.
    Classic,
}

impl TurnMode {
    /// `--classic-turns` selects [`TurnMode::Classic`], `--pla-turns` [`TurnMode::Pla`]; the
    /// last flag wins; without either the default ([`TurnMode::Pla`]) is used.
    pub fn from_args(args: &[String]) -> Self {
        let mut mode = Self::default();
        for a in args {
            match a.as_str() {
                "--classic-turns" => mode = Self::Classic,
                "--pla-turns" => mode = Self::Pla,
                _ => {}
            }
        }
        mode
    }

    /// `PLA_TURNS=classic|pla` wins over the command line; otherwise [`Self::from_args`] on the
    /// process arguments.
    pub fn from_env_and_args() -> Self {
        match std::env::var("PLA_TURNS").ok().as_deref() {
            Some("classic") => Self::Classic,
            Some("pla") => Self::Pla,
            _ => Self::from_args(&std::env::args().collect::<Vec<_>>()),
        }
    }

    pub fn uses_queue(self) -> bool {
        self == Self::Pla
    }
}

/// Display word of a style (log and move grid).
pub fn style_word(style: Style) -> &'static str {
    match style {
        Style::Normal => "Normal",
        Style::Agile => "Agile",
        Style::Strong => "Strong",
    }
}

/// Lines of the HUD turn-order list: `"1. PIKACHU  wait 11"`, numbered from the next turn.
/// `name(i)` gives the display name of combatant `i`; `player_side` marks the player's turns
/// with `>` so they stand out.
pub fn turn_order_lines(
    entries: &[TurnOrderEntry],
    name: &dyn Fn(usize) -> String,
    player_side: usize,
) -> Vec<String> {
    entries
        .iter()
        .enumerate()
        .map(|(k, e)| {
            let mark = if e.side == player_side { '>' } else { ' ' };
            format!("{mark}{}. {}  wait {}", k + 1, name(e.combatant), e.wait.max(0))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn flags_select_the_mode_and_the_default_is_pla() {
        assert_eq!(TurnMode::from_args(&args(&[])), TurnMode::Pla);
        assert_eq!(TurnMode::from_args(&args(&["--classic-turns"])), TurnMode::Classic);
        assert_eq!(TurnMode::from_args(&args(&["--classic-turns", "--pla-turns"])), TurnMode::Pla);
        assert_eq!(TurnMode::from_args(&args(&["--area", "3", "--classic-turns"])), TurnMode::Classic);
        assert!(TurnMode::Pla.uses_queue() && !TurnMode::Classic.uses_queue());
    }

    #[test]
    fn turn_order_lines_number_mark_and_clamp() {
        // Hand example of `queue_starts_with_base_waits_and_picks_the_fastest` (styles.rs):
        // agility 100 (W 10) vs 50 (W 12) -> A 10, B 2, A 8, B 4, A 6, B 6.
        let e = |side: usize, wait: i32| TurnOrderEntry { side, combatant: side, wait };
        let list = [e(0, 10), e(1, 2), e(0, 8), e(1, 4), e(0, 6), e(1, 6)];
        let names = |i: usize| if i == 0 { "PIKACHU".to_string() } else { "Wild BIDOOF".to_string() };
        let lines = turn_order_lines(&list, &names, 0);
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0], ">1. PIKACHU  wait 10");
        assert_eq!(lines[1], " 2. Wild BIDOOF  wait 2");
        assert_eq!(lines[5], " 6. Wild BIDOOF  wait 6");
        let neg = turn_order_lines(&[e(1, -3)], &names, 0);
        assert_eq!(neg[0], " 1. Wild BIDOOF  wait 0", "a negative wait is shown as 0");
    }

}
