//! Rare palette ("shiny") rolls — Brief 17 remainder: the probability of the rare palette.
//!
//! **What is VERIFIED (local files + executable):**
//! - `bin/misc/app_config/pokemon_rare.bin` (update v262144) stores five parameters; the three
//!   try counts are `TryCount_Item` = 3, `TryCount_Research100` = 1 and
//!   `TryCount_ResearchPerfect` = 2 (raw values reproduced in `data/pokemon_variants.tsv`, test
//!   below); the field effect parameters are `Field_StartEffectDistance` = 35 (m) and
//!   `Field_SoundFrame` = 150 (frames).
//! - The update executable looks those parameters up by the FNV-1a 64 hash of their names:
//!   `FUN_00f94470` (pokemon creation path) contains the immediates
//!   0x95ccdc647c831655 = fnv1a64("TryCount_Item"), 0x79efd2242bab9e76 =
//!   fnv1a64("TryCount_Research100") and 0x09b7e367dc52b434 = fnv1a64("TryCount_ResearchPerfect");
//!   `FUN_01b1aa88` (field sparkle trigger) reads `Field_SoundFrame` for the `pokemon_rare`
//!   config. The "extra tries" reading of those parameters is therefore in the shipped code, not
//!   only in the file (method: Ghidra constant scan over the 9.5 M instructions of `main.elf`,
//!   see `reports/data-progress/alpha_shiny_outbreaks.md`).
//!
//! **What is PROBABLE (external player notes, not shipped with the repository):** one try is a
//! 1/4096 roll and the bonuses add tries on top of the base try, so the chance is N/4096 with
//! N = 1 + 1 (research 10) + 2 (perfect) + 3 (charm) + 25 (mass outbreak) + 12 (massive mass
//! outbreak). A standard mass-outbreak file really stores 25 (`huge_outbreak.bin`, column
//! `f7_raw`) and the massive-outbreak settings really store 12 (`new_huge_outbreak_lottery.bin`,
//! `f14` and `f17`), so the note's two event bonuses agree with the decoded files; the 4096
//! denominator itself exists only in the note. The port keeps it behind
//! [`RARE_BASE_DENOMINATOR`] with that label.

use super::wild::splitmix64;

/// Extra tries of the shiny charm item (`TryCount_Item`). VERIFIED value (3); the
/// "charm item" meaning is PROBABLE (external notes).
pub const RARE_TRY_BONUS_ITEM: u32 = 3;
/// Extra tries at Pokédex research level 10 for the species (`TryCount_Research100`).
/// VERIFIED value (1), PROBABLE meaning.
pub const RARE_TRY_BONUS_RESEARCH_100: u32 = 1;
/// Extra tries when the species research entry is perfect (`TryCount_ResearchPerfect`).
/// VERIFIED value (2), PROBABLE meaning.
pub const RARE_TRY_BONUS_RESEARCH_PERFECT: u32 = 2;
/// Extra tries of a standard mass outbreak. VERIFIED value (`huge_outbreak.bin` `f7_raw` = 25);
/// the meaning is PROBABLE (external notes).
pub const RARE_TRY_BONUS_OUTBREAK: u32 = 25;
/// Extra tries of a massive mass outbreak. VERIFIED value (`new_huge_outbreak_lottery.bin`
/// `f14`/`f17` = 12); the meaning is PROBABLE (external notes).
pub const RARE_TRY_BONUS_MMO: u32 = 12;
/// The single base try every wild creature gets.
pub const RARE_BASE_TRIES: u32 = 1;
/// Denominator of one try (**PROBABLE**: external player notes only; no local file or code
/// constant carries it).
pub const RARE_BASE_DENOMINATOR: u32 = 4096;
/// `Field_StartEffectDistance` (VERIFIED raw value): distance in metres at which the field
/// sparkle effect starts.
pub const RARE_EFFECT_START_DISTANCE_M: f32 = 35.0;
/// `Field_SoundFrame` (VERIFIED raw value): frame the sparkle sound fires at.
pub const RARE_SOUND_FRAME: u32 = 150;

/// Everything that adds tries to the rare roll of one spawned creature.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RareContext {
    /// Pokédex research level 10 for this species (`TryCount_Research100`).
    pub research_level_10: bool,
    /// Perfect Pokédex research entry for this species (`TryCount_ResearchPerfect`).
    pub research_perfect: bool,
    /// Shiny charm item (`TryCount_Item`). The port has no charm item flow yet, so this stays
    /// `false` and the constant is wired for when it lands.
    pub charm: bool,
    /// Extra tries of the active world event: [`RARE_TRY_BONUS_OUTBREAK`] for a standard mass
    /// outbreak, [`RARE_TRY_BONUS_MMO`] for a massive mass outbreak, 0 for a plain wild spawn
    /// or a distortion spawn.
    pub event_bonus: u32,
}

impl RareContext {
    /// Total tries of the rare roll (base + every bonus).
    pub fn tries(self) -> u32 {
        RARE_BASE_TRIES
            + u32::from(self.research_level_10) * RARE_TRY_BONUS_RESEARCH_100
            + u32::from(self.research_perfect) * RARE_TRY_BONUS_RESEARCH_PERFECT
            + u32::from(self.charm) * RARE_TRY_BONUS_ITEM
            + self.event_bonus
    }

    /// Chance of one creature being rare, as the note's `tries / 4096` fraction.
    pub fn odds(self) -> f32 {
        self.tries() as f32 / RARE_BASE_DENOMINATOR as f32
    }
}

/// Deterministic rare roll: `tries` attempts of one `1/4096` chance.
///
/// Same seed and context always answer the same way (the port never stores the outcome, so a
/// respawn re-rolls from its own seed).
pub fn is_rare(seed: u64, ctx: RareContext) -> bool {
    let mut state = seed ^ 0x5a17_ea11_5eed_c0de;
    let roll = splitmix64(&mut state) % u64::from(RARE_BASE_DENOMINATOR);
    roll < u64::from(ctx.tries())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_variants() -> Option<String> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/pokemon_variants.tsv");
        std::fs::read_to_string(p).ok()
    }

    /// The constants above must equal the raw values in `data/pokemon_variants.tsv`.
    #[test]
    fn try_counts_and_effect_parameters_match_the_decoded_sheet() {
        let Some(text) = read_variants() else {
            eprintln!("[skip] data/pokemon_variants.tsv not found");
            return;
        };
        let value = |key: &str| -> Option<String> {
            text.lines()
                .filter(|l| !l.starts_with('#'))
                .find_map(|l| {
                    let f: Vec<&str> = l.split('\t').collect();
                    (f.len() > 3 && f[2] == key).then(|| f[3].to_string())
                })
        };
        assert_eq!(value("TryCount_Item").as_deref(), Some(&RARE_TRY_BONUS_ITEM.to_string()[..]));
        assert_eq!(
            value("TryCount_Research100").as_deref(),
            Some(&RARE_TRY_BONUS_RESEARCH_100.to_string()[..])
        );
        assert_eq!(
            value("TryCount_ResearchPerfect").as_deref(),
            Some(&RARE_TRY_BONUS_RESEARCH_PERFECT.to_string()[..])
        );
        assert_eq!(
            value("Field_StartEffectDistance").as_deref(),
            Some(&(RARE_EFFECT_START_DISTANCE_M as u32).to_string()[..])
        );
        assert_eq!(
            value("Field_SoundFrame").as_deref(),
            Some(&RARE_SOUND_FRAME.to_string()[..])
        );
    }

    #[test]
    fn tries_add_up_like_the_note_table() {
        let base = RareContext::default();
        assert_eq!(base.tries(), 1);
        assert_eq!(
            RareContext { research_level_10: true, ..base }.tries(),
            2
        );
        // Perfect implies level 10 in the game; the note's "perfect" row is 3 tries without charm.
        assert_eq!(
            RareContext { research_level_10: true, research_perfect: true, ..base }.tries(),
            4
        );
        assert_eq!(RareContext { charm: true, ..base }.tries(), 4);
        assert_eq!(
            RareContext {
                research_level_10: true,
                research_perfect: true,
                charm: true,
                ..base
            }
            .tries(),
            7
        );
        assert_eq!(
            RareContext { event_bonus: RARE_TRY_BONUS_OUTBREAK, ..base }.tries(),
            26
        );
        assert_eq!(
            RareContext { event_bonus: RARE_TRY_BONUS_MMO, ..base }.tries(),
            13
        );
    }

    #[test]
    fn roll_is_deterministic_and_bounded() {
        let ctx = RareContext::default();
        for seed in [0u64, 1, 42, u64::MAX] {
            assert_eq!(is_rare(seed, ctx), is_rare(seed, ctx));
        }
        // With all bonuses the roll must pass on some seeds; base alone must not pass everywhere.
        let full = RareContext {
            research_level_10: true,
            research_perfect: true,
            charm: true,
            event_bonus: RARE_TRY_BONUS_OUTBREAK,
        };
        assert!((0..10_000).any(|s| is_rare(s, full)));
        assert!((0..10_000).any(|s| !is_rare(s, ctx)));
    }

    /// The empirical rate must follow `tries / 4096` within a wide tolerance (xorshift noise).
    #[test]
    fn empirical_rate_matches_tries_over_denominator() {
        let cases = [
            (RareContext::default(), 1.0),
            (
                RareContext { research_level_10: true, research_perfect: true, ..Default::default() },
                4.0,
            ),
            (
                RareContext { event_bonus: RARE_TRY_BONUS_OUTBREAK, ..Default::default() },
                26.0,
            ),
        ];
        for (ctx, tries) in cases {
            let n = 200_000u64;
            let got = (0..n).filter(|s| is_rare(*s, ctx)).count() as f32 / n as f32;
            let want = tries / RARE_BASE_DENOMINATOR as f32;
            assert!(
                (got - want).abs() < want * 0.25 + 0.00002,
                "ctx {ctx:?}: got {got}, want {want}"
            );
        }
        assert!((RareContext::default().odds() - 1.0 / 4096.0).abs() < 1e-9);
    }
}
