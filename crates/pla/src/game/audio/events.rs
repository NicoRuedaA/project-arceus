//! Pure mapping from game events to sounds (no Bevy systems, no files).
//!
//! Every sound is a named Wwise event of the game (`data/audio_index.tsv`, names recovered by
//! FNV-1 hash). The *name* of each event is VERIFIED (its hash matches the game's event id); the
//! *role* given to it here (which game moment plays it) is read from the name and is ASSUMED
//! unless stated otherwise. The original game drives these events from scripts and animation
//! timelines that are not decoded, so timing and choice are ours.

use crate::battle_engine::{Event, MajorStatus};

use super::super::ball_lifecycle::BallLifecycleState;

/// Sound bus a sound plays on (each has its own volume in `settings.txt`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bus {
    Music,
    Sfx,
    Voice,
}

/// How to pick one media file when an event lists several (random takes, layers or states the
/// port cannot evaluate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantPolicy {
    /// A random take each time (cries, impacts).
    Random,
    /// Always the take with the highest RMS (UI: one stable, audible click).
    Loudest,
    /// Always the longest take, ties broken by the highest RMS (music: the full loop rather
    /// than an intro stinger or a sparse layer).
    Longest,
}

/// Sound effects the port plays. Each maps to one game event name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SfxId {
    UiCursor,
    UiDecide,
    UiCancel,
    PauseOpen,
    PauseClose,
    BattleUiOpen,
    BattleMoveCursor,
    BattleMoveDecide,
    BattleListCursor,
    BattleBag,
    BattleRun,
    BattleCancel,
    BattleStart,
    BattleEnd,
    PokemonAppear,
    HitWeak,
    HitNormal,
    HitStrong,
    Faint,
    StatusSleep,
    StatusPoison,
    StatusBurn,
    StatusFrostbite,
    StatusParalysis,
    StatUp,
    StatDown,
    Recover,
    BallThrow,
    BallBounce,
    BallHit,
    BallIntoBall,
    BallShake,
    BallCaught,
    BallBreakFree,
    CaptureJingle,
    LevelUpJingle,
}

impl SfxId {
    /// Every id, for coverage tests and the census example.
    pub const ALL: [SfxId; 36] = [
        SfxId::UiCursor,
        SfxId::UiDecide,
        SfxId::UiCancel,
        SfxId::PauseOpen,
        SfxId::PauseClose,
        SfxId::BattleUiOpen,
        SfxId::BattleMoveCursor,
        SfxId::BattleMoveDecide,
        SfxId::BattleListCursor,
        SfxId::BattleBag,
        SfxId::BattleRun,
        SfxId::BattleCancel,
        SfxId::BattleStart,
        SfxId::BattleEnd,
        SfxId::PokemonAppear,
        SfxId::HitWeak,
        SfxId::HitNormal,
        SfxId::HitStrong,
        SfxId::Faint,
        SfxId::StatusSleep,
        SfxId::StatusPoison,
        SfxId::StatusBurn,
        SfxId::StatusFrostbite,
        SfxId::StatusParalysis,
        SfxId::StatUp,
        SfxId::StatDown,
        SfxId::Recover,
        SfxId::BallThrow,
        SfxId::BallBounce,
        SfxId::BallHit,
        SfxId::BallIntoBall,
        SfxId::BallShake,
        SfxId::BallCaught,
        SfxId::BallBreakFree,
        SfxId::CaptureJingle,
        SfxId::LevelUpJingle,
    ];

    /// Game event name (case as found in the game; lookups are case-insensitive).
    pub fn event_name(self) -> &'static str {
        match self {
            SfxId::UiCursor => "Play_UI_common_select",
            SfxId::UiDecide => "PLAY_UI_COMMON_DECIDE",
            SfxId::UiCancel => "Play_UI_common_cancel",
            SfxId::PauseOpen => "PLAY_UI_COMMON_PAUSE_MENU_OPEN",
            SfxId::PauseClose => "PLAY_UI_COMMON_PAUSE_MENU_CLOSE",
            SfxId::BattleUiOpen => "PLAY_UI_BATTLE_OPEN",
            SfxId::BattleMoveCursor => "PLAY_UI_BATTLE_SKILLS_SELECT",
            SfxId::BattleMoveDecide => "PLAY_UI_BATTLE_SKILLS_DECIDE",
            SfxId::BattleListCursor => "PLAY_UI_BATTLE_SELECT_P",
            SfxId::BattleBag => "PLAY_UI_BATTLE_DECIDE_BAG",
            SfxId::BattleRun => "PLAY_UI_BATTLE_DECIDE_RUN",
            SfxId::BattleCancel => "PLAY_UI_BATTLE_CANCEL",
            SfxId::BattleStart => "PLAY_BA_SYS_BATTLE_START",
            SfxId::BattleEnd => "PLAY_BA_SYS_BATTLE_END",
            SfxId::PokemonAppear => "PLAY_BA_SYS_POKE_APPERANCE",
            SfxId::HitWeak => "PLAY_BA_SYS_HIT_L",
            SfxId::HitNormal => "PLAY_BA_SYS_HIT_M",
            SfxId::HitStrong => "PLAY_BA_SYS_HIT_H",
            SfxId::Faint => "Play_Ba_sys_poke_down",
            SfxId::StatusSleep => "PLAY_BA_SYS_STATE_001_SLEEP",
            SfxId::StatusPoison => "PLAY_BA_SYS_STATE_002_POISON",
            SfxId::StatusBurn => "PLAY_BA_SYS_STATE_003_BURN",
            SfxId::StatusFrostbite => "PLAY_BA_SYS_STATE_004_ICE",
            SfxId::StatusParalysis => "PLAY_BA_SYS_STATE_005_PARALYSIS",
            SfxId::StatUp => "PLAY_BA_SYS_STATE_008_P_UP",
            SfxId::StatDown => "PLAY_BA_SYS_STATE_009_P_DOWN",
            SfxId::Recover => "PLAY_BA_SYS_STATE_010_RECOVER",
            SfxId::BallThrow => "PLAY_BA_SYS_BALL_THROW_NORMAL_MID",
            SfxId::BallBounce => "PLAY_BA_SYS_BALL_DROP",
            SfxId::BallHit => "PLAY_BA_SYS_BALL_HIT",
            SfxId::BallIntoBall => "PLAY_SS_CP_INTO_MONSTERBALL",
            SfxId::BallShake => "PLAY_BA_SYS_BALL_SPIN",
            SfxId::BallCaught => "PLAY_UI_COMMON_CATCH",
            SfxId::BallBreakFree => "PLAY_BA_SYS_BALL_OPEN",
            SfxId::CaptureJingle => "PLAY_ME_HA_EV_GET_POKE",
            SfxId::LevelUpJingle => "PLAY_ME_HA_SYS_LVUP",
        }
    }

    /// Bus the sound plays on: jingles (`PLAY_ME_*`) share the music volume.
    pub fn bus(self) -> Bus {
        match self {
            SfxId::CaptureJingle | SfxId::LevelUpJingle => Bus::Music,
            _ => Bus::Sfx,
        }
    }

    /// Which take to play.
    pub fn policy(self) -> VariantPolicy {
        match self {
            SfxId::BallBounce
            | SfxId::BallThrow
            | SfxId::HitWeak
            | SfxId::HitNormal
            | SfxId::HitStrong => VariantPolicy::Random,
            _ => VariantPolicy::Loudest,
        }
    }
}

/// Background music the port plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MusicId {
    /// Field music of terrain area `n` (0..=6).
    Area(u8),
    /// Shared night field music (not wired: the port has no day/night music switch yet).
    FieldNight,
    /// Battle against a wild Pokémon.
    WildBattle,
    /// Trainer battle (not wired: the port has no trainer battles).
    TrainerBattle,
}

impl MusicId {
    /// Game event name, `None` when no music is identified for it.
    ///
    /// `PLAY_BGM_HA_FI_AREA00` is named in the index (`hash_string`). `AREA01..05_DAY`, `AREA06`
    /// and `CMN_NIGHT` were found by hashing candidate names against the event ids of `BGM.bnk`
    /// (see `data/audio_event_media.tsv`, generated by `examples/audio_check.rs events`).
    pub fn event_name(self) -> Option<&'static str> {
        Some(match self {
            MusicId::Area(0) => "PLAY_BGM_HA_FI_AREA00",
            MusicId::Area(1) => "PLAY_BGM_HA_FI_AREA01_DAY",
            MusicId::Area(2) => "PLAY_BGM_HA_FI_AREA02_DAY",
            MusicId::Area(3) => "PLAY_BGM_HA_FI_AREA03_DAY",
            MusicId::Area(4) => "PLAY_BGM_HA_FI_AREA04_DAY",
            MusicId::Area(5) => "PLAY_BGM_HA_FI_AREA05_DAY",
            MusicId::Area(6) => "PLAY_BGM_HA_FI_AREA06",
            MusicId::Area(_) => return None,
            MusicId::FieldNight => "PLAY_BGM_HA_FI_CMN_NIGHT",
            MusicId::WildBattle => "PLAY_BGM_VS_NORA",
            MusicId::TrainerBattle => "PLAY_BGM_VS_TRAINER_01",
        })
    }
}

/// Field music for the terrain area shown by the viewer.
pub fn music_for_area(area: u8) -> Option<MusicId> {
    let m = MusicId::Area(area);
    m.event_name().map(|_| m)
}

/// How hard a hit landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitStrength {
    Weak,
    Normal,
    Strong,
}

/// Game moments that make a sound. The port's systems translate engine events, UI state
/// changes and ball states into these; [`sfx_for_event`] picks the sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameEvent {
    UiCursor,
    UiDecide,
    UiCancel,
    PauseOpened,
    PauseClosed,
    BattleStarted,
    BattleEnded,
    BattleMoveCursor,
    BattleMoveChosen,
    BattleListCursor,
    BattleBagOpened,
    BattleRan,
    BattleBack,
    PokemonAppeared,
    Damage(HitStrength),
    Fainted,
    StatusApplied(MajorStatus),
    StatRaised,
    StatLowered,
    Healed,
    BallThrown,
    BallBounced,
    BallHitPokemon,
    BallShook,
    CaptureSucceeded,
    CaptureFailed,
    LevelUp,
}

/// The sound effect of a game event (`None`: the event is silent in the port).
pub fn sfx_for_event(e: &GameEvent) -> Option<SfxId> {
    Some(match e {
        GameEvent::UiCursor => SfxId::UiCursor,
        GameEvent::UiDecide => SfxId::UiDecide,
        GameEvent::UiCancel => SfxId::UiCancel,
        GameEvent::PauseOpened => SfxId::PauseOpen,
        GameEvent::PauseClosed => SfxId::PauseClose,
        GameEvent::BattleStarted => SfxId::BattleStart,
        GameEvent::BattleEnded => SfxId::BattleEnd,
        GameEvent::BattleMoveCursor => SfxId::BattleMoveCursor,
        GameEvent::BattleMoveChosen => SfxId::BattleMoveDecide,
        GameEvent::BattleListCursor => SfxId::BattleListCursor,
        GameEvent::BattleBagOpened => SfxId::BattleBag,
        GameEvent::BattleRan => SfxId::BattleRun,
        GameEvent::BattleBack => SfxId::BattleCancel,
        GameEvent::PokemonAppeared => SfxId::PokemonAppear,
        GameEvent::Damage(HitStrength::Weak) => SfxId::HitWeak,
        GameEvent::Damage(HitStrength::Normal) => SfxId::HitNormal,
        GameEvent::Damage(HitStrength::Strong) => SfxId::HitStrong,
        GameEvent::Fainted => SfxId::Faint,
        GameEvent::StatusApplied(s) => match s {
            MajorStatus::Drowsy => SfxId::StatusSleep,
            MajorStatus::Poison => SfxId::StatusPoison,
            MajorStatus::Burn => SfxId::StatusBurn,
            MajorStatus::Frostbite => SfxId::StatusFrostbite,
            MajorStatus::Paralysis => SfxId::StatusParalysis,
        },
        GameEvent::StatRaised => SfxId::StatUp,
        GameEvent::StatLowered => SfxId::StatDown,
        GameEvent::Healed => SfxId::Recover,
        GameEvent::BallThrown => SfxId::BallThrow,
        GameEvent::BallBounced => SfxId::BallBounce,
        GameEvent::BallHitPokemon => SfxId::BallHit,
        GameEvent::BallShook => SfxId::BallShake,
        GameEvent::CaptureSucceeded => SfxId::BallCaught,
        GameEvent::CaptureFailed => SfxId::BallBreakFree,
        GameEvent::LevelUp => SfxId::LevelUpJingle,
    })
}

/// One thing to play.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cue {
    Sfx(SfxId),
    /// Pokémon cry; `faint` plays it slower (lower), a port choice (ASSUMED).
    Cry { species: u16, form: u8, faint: bool },
    Music(MusicId),
    StopMusic,
}

/// A cue `delay` seconds after the moment it was produced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimedCue {
    pub cue: Cue,
    pub delay: f32,
}

fn at(cue: Cue, delay: f32) -> TimedCue {
    TimedCue { cue, delay }
}

fn sfx_at(e: GameEvent, delay: f32) -> Option<TimedCue> {
    sfx_for_event(&e).map(|s| at(Cue::Sfx(s), delay))
}

/// Hit strength of a damaging move from the engine's effectiveness label and critical flag:
/// critical or super effective = strong, not very effective = weak (ASSUMED reading of the
/// `HIT_L/M/H` names).
pub fn hit_strength(effectiveness: &str, critical: bool) -> HitStrength {
    if critical || effectiveness == "super effective" {
        HitStrength::Strong
    } else if effectiveness == "not very effective" {
        HitStrength::Weak
    } else {
        HitStrength::Normal
    }
}

/// Cues for a batch of engine events, timed like the battle animations of
/// [`super::super::battle_anim::requests_for_events`] (move step, hit and faint delays) so a hit
/// sound lands when the target flinches. `species(i)` returns the national dex number of
/// combatant `i`.
pub fn cues_for_battle_events(events: &[Event], species: &dyn Fn(usize) -> Option<u16>) -> Vec<TimedCue> {
    cues_for_battle_events_timed(events, species, &|_, _| super::super::battle_anim::MoveTiming::default())
}

/// [`cues_for_battle_events`] with the per-move pacing `timing(actor, move_name)` the
/// animations use (`battle_anim::requests_for_events_timed`), so hits stay in sync.
pub fn cues_for_battle_events_timed(
    events: &[Event],
    species: &dyn Fn(usize) -> Option<u16>,
    timing: &dyn Fn(usize, &str) -> super::super::battle_anim::MoveTiming,
) -> Vec<TimedCue> {
    use super::super::battle_anim::{FAINT_DELAY_S, HIT_DELAY_S};
    let mut out = Vec::new();
    let mut cursor = 0.0_f32;
    let mut last_action = 0.0_f32;
    let mut last_hit = HIT_DELAY_S;
    for ev in events {
        match ev {
            Event::UsedMove { actor, name, damage, critical, effectiveness, .. } => {
                let t = timing(*actor, name);
                if *damage > 0 {
                    let s = hit_strength(effectiveness, *critical);
                    out.extend(sfx_at(GameEvent::Damage(s), cursor + t.hit_s));
                }
                last_action = cursor;
                last_hit = t.hit_s;
                cursor += t.step_s;
            }
            Event::Missed { actor, name, .. } => {
                let t = timing(*actor, name);
                last_action = cursor;
                last_hit = t.hit_s;
                cursor += t.step_s;
            }
            Event::Recoil { .. } => out.extend(sfx_at(GameEvent::Damage(HitStrength::Weak), cursor)),
            Event::StatusApplied { status, .. } => {
                out.extend(sfx_at(GameEvent::StatusApplied(*status), cursor.max(last_action + last_hit)))
            }
            Event::KnockedOut(i) => {
                let t = last_action + FAINT_DELAY_S + (last_hit - HIT_DELAY_S);
                if let Some(sp) = species(*i) {
                    out.push(at(Cue::Cry { species: sp, form: 0, faint: true }, t));
                }
                out.extend(sfx_at(GameEvent::Fainted, t + 0.4));
            }
            Event::Switched { to, .. } => {
                out.extend(sfx_at(GameEvent::PokemonAppeared, cursor));
                if let Some(sp) = species(*to) {
                    out.push(at(Cue::Cry { species: sp, form: 0, faint: false }, cursor + 0.3));
                }
            }
            Event::UsedHealingItem { .. } => out.extend(sfx_at(GameEvent::Healed, cursor)),
            _ => {}
        }
    }
    out
}

/// Seconds between two shake sounds of a ball in battle (the overworld ball uses its own timer).
pub const BATTLE_SHAKE_STEP_S: f32 = 0.65;

/// Cues for one new battle log line. The battle controller reports ball throws, capture
/// results and running away only as log text, so these are matched on the exact phrases
/// `battle_controller.rs` writes (`You threw a`, `Shake...`, `Gotcha!`, `broke free!`,
/// `Got away safely!`). If those phrases change, these sounds go silent (tested).
pub fn cues_for_log_line(line: &str) -> Vec<TimedCue> {
    let mut out = Vec::new();
    if line.starts_with("You threw a ") {
        out.extend(sfx_at(GameEvent::BallThrown, 0.0));
        out.extend(sfx_at(GameEvent::BallHitPokemon, 0.5));
        out.push(at(Cue::Sfx(SfxId::BallIntoBall), 0.8));
    } else if line.starts_with("Shake...") {
        let n = line.matches("Shake...").count();
        for k in 0..n {
            out.extend(sfx_at(GameEvent::BallShook, 1.2 + k as f32 * BATTLE_SHAKE_STEP_S));
        }
    } else if line.starts_with("Gotcha! ") {
        out.extend(sfx_at(GameEvent::CaptureSucceeded, 2.6));
        out.push(at(Cue::Sfx(SfxId::CaptureJingle), 3.0));
    } else if line.ends_with(" broke free!") {
        out.extend(sfx_at(GameEvent::CaptureFailed, 2.6));
    } else if line == "Got away safely!" {
        out.extend(sfx_at(GameEvent::BattleRan, 0.0));
    }
    out
}

/// Snapshot of an overworld ball between two frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BallSnapshot {
    pub state: BallLifecycleState,
    pub bounces: u8,
}

/// Game events of an overworld ball that changed from `prev` (None: just thrown) to `now`.
pub fn ball_events(prev: Option<BallSnapshot>, now: BallSnapshot) -> Vec<GameEvent> {
    use BallLifecycleState as S;
    let mut out = Vec::new();
    let Some(p) = prev else {
        if matches!(now.state, S::InFlight) {
            out.push(GameEvent::BallThrown);
        }
        return out;
    };
    if now.bounces > p.bounces {
        out.push(GameEvent::BallBounced);
    }
    match (p.state, now.state) {
        (S::InFlight, S::RollingOnGround { .. }) => out.push(GameEvent::BallBounced),
        (S::InFlight, S::Shaking { .. }) => out.push(GameEvent::BallHitPokemon),
        (S::Shaking { shake_count: a, .. }, S::Shaking { shake_count: b, .. }) if b > a => {
            out.push(GameEvent::BallShook)
        }
        (S::Shaking { .. }, S::Captured { .. }) => {
            out.push(GameEvent::BallShook);
            out.push(GameEvent::CaptureSucceeded);
        }
        (S::Shaking { .. }, S::Breakout) => out.push(GameEvent::CaptureFailed),
        _ => {}
    }
    out
}

/// Events of an overworld ball whose entity disappeared while last seen as `last`: a ball that
/// was on its final shake without a capture broke free (the breakout tick despawns it at once).
pub fn ball_vanished_events(last: BallSnapshot) -> Vec<GameEvent> {
    match last.state {
        BallLifecycleState::Shaking { shake_count, max_shakes, will_capture: false, .. }
            if shake_count + 1 >= max_shakes =>
        {
            vec![GameEvent::CaptureFailed]
        }
        _ => Vec::new(),
    }
}

/// Wwise event name of a Pokémon cry: `PLAY_PV_<species>_<form>_<variant>` (format string found
/// in `main.elf`; variant meaning unknown, 00 is used for every cry).
pub fn cry_event_name(species: u16, form: u8, variant: u8) -> String {
    format!("PLAY_PV_{species:03}_{form:02}_{variant:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_sfx_has_a_distinct_event_name() {
        let names: HashSet<_> = SfxId::ALL.iter().map(|s| s.event_name().to_ascii_lowercase()).collect();
        assert_eq!(names.len(), SfxId::ALL.len());
    }

    #[test]
    fn sfx_for_event_maps_key_moments() {
        assert_eq!(sfx_for_event(&GameEvent::UiCursor), Some(SfxId::UiCursor));
        assert_eq!(sfx_for_event(&GameEvent::BattleStarted), Some(SfxId::BattleStart));
        assert_eq!(sfx_for_event(&GameEvent::Fainted), Some(SfxId::Faint));
        assert_eq!(sfx_for_event(&GameEvent::CaptureSucceeded), Some(SfxId::BallCaught));
        assert_eq!(sfx_for_event(&GameEvent::LevelUp), Some(SfxId::LevelUpJingle));
        assert_eq!(
            sfx_for_event(&GameEvent::StatusApplied(MajorStatus::Burn)),
            Some(SfxId::StatusBurn)
        );
        assert_eq!(SfxId::LevelUpJingle.bus(), Bus::Music);
        assert_eq!(SfxId::UiCursor.bus(), Bus::Sfx);
    }

    #[test]
    fn hit_strength_reads_effectiveness_and_critical() {
        assert_eq!(hit_strength("normal", false), HitStrength::Normal);
        assert_eq!(hit_strength("super effective", false), HitStrength::Strong);
        assert_eq!(hit_strength("not very effective", false), HitStrength::Weak);
        assert_eq!(hit_strength("not very effective", true), HitStrength::Strong);
    }

    #[test]
    fn area_music_names() {
        assert_eq!(music_for_area(0).and_then(|m| m.event_name()), Some("PLAY_BGM_HA_FI_AREA00"));
        assert_eq!(music_for_area(3).and_then(|m| m.event_name()), Some("PLAY_BGM_HA_FI_AREA03_DAY"));
        assert_eq!(music_for_area(9), None);
    }

    #[test]
    fn battle_events_time_hits_and_faint_cries() {
        use crate::game::battle_anim::{FAINT_DELAY_S, HIT_DELAY_S, MOVE_STEP_S};
        let events = vec![
            Event::TurnStarted(1),
            Event::UsedMove { actor: 0, target: 1, name: "Tackle".into(), damage: 9, critical: false, effectiveness: "normal".into() },
            Event::UsedMove { actor: 1, target: 0, name: "Growl".into(), damage: 0, critical: false, effectiveness: "normal".into() },
            Event::UsedMove { actor: 0, target: 1, name: "Spark".into(), damage: 30, critical: true, effectiveness: "normal".into() },
            Event::KnockedOut(1),
        ];
        let sp = |i: usize| Some(if i == 1 { 399 } else { 25 });
        let c = cues_for_battle_events(&events, &sp);
        assert_eq!(c[0], at(Cue::Sfx(SfxId::HitNormal), HIT_DELAY_S));
        assert_eq!(c[1], at(Cue::Sfx(SfxId::HitStrong), 2.0 * MOVE_STEP_S + HIT_DELAY_S));
        assert_eq!(c[2], at(Cue::Cry { species: 399, form: 0, faint: true }, 2.0 * MOVE_STEP_S + FAINT_DELAY_S));
        assert_eq!(c[3].cue, Cue::Sfx(SfxId::Faint));
        assert_eq!(c.len(), 4);
    }

    #[test]
    fn switch_plays_appear_and_cry() {
        let c = cues_for_battle_events(&[Event::Switched { side: 0, from: 0, to: 2 }], &|_| Some(396));
        assert_eq!(c[0].cue, Cue::Sfx(SfxId::PokemonAppear));
        assert_eq!(c[1].cue, Cue::Cry { species: 396, form: 0, faint: false });
    }

    #[test]
    fn log_lines_of_the_battle_controller() {
        assert_eq!(cues_for_log_line("You threw a Regular Ball at Wild BIDOOF!")[0].cue, Cue::Sfx(SfxId::BallThrow));
        assert_eq!(cues_for_log_line("Shake... Shake...").len(), 2);
        let g = cues_for_log_line("Gotcha! Wild BIDOOF was caught!");
        assert_eq!(g[0].cue, Cue::Sfx(SfxId::BallCaught));
        assert_eq!(g[1].cue, Cue::Sfx(SfxId::CaptureJingle));
        assert_eq!(cues_for_log_line("Wild BIDOOF broke free!")[0].cue, Cue::Sfx(SfxId::BallBreakFree));
        assert_eq!(cues_for_log_line("Got away safely!")[0].cue, Cue::Sfx(SfxId::BattleRun));
        assert!(cues_for_log_line("Battle Started!").is_empty());
    }

    #[test]
    fn overworld_ball_lifecycle_sounds() {
        use BallLifecycleState as S;
        let fly = BallSnapshot { state: S::InFlight, bounces: 0 };
        assert_eq!(ball_events(None, fly), vec![GameEvent::BallThrown]);
        let bounced = BallSnapshot { state: S::InFlight, bounces: 1 };
        assert_eq!(ball_events(Some(fly), bounced), vec![GameEvent::BallBounced]);
        let shake = |n, cap| BallSnapshot {
            state: S::Shaking { shake_count: n, timer: 0.0, max_shakes: 3, will_capture: cap },
            bounces: 0,
        };
        assert_eq!(ball_events(Some(fly), shake(0, true)), vec![GameEvent::BallHitPokemon]);
        assert_eq!(ball_events(Some(shake(0, true)), shake(1, true)), vec![GameEvent::BallShook]);
        let caught = BallSnapshot { state: S::Captured { despawn_timer: 0.0 }, bounces: 0 };
        assert_eq!(
            ball_events(Some(shake(2, true)), caught),
            vec![GameEvent::BallShook, GameEvent::CaptureSucceeded]
        );
        assert_eq!(ball_vanished_events(shake(2, false)), vec![GameEvent::CaptureFailed]);
        assert!(ball_vanished_events(shake(0, false)).is_empty());
        assert!(ball_vanished_events(fly).is_empty());
    }

    #[test]
    fn cry_names_follow_the_game_format() {
        assert_eq!(cry_event_name(25, 0, 0), "PLAY_PV_025_00_00");
        assert_eq!(cry_event_name(493, 12, 3), "PLAY_PV_493_12_03");
    }
}
