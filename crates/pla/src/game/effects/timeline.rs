//! Effect timelines (`bin/effect/battle/sequence/*.wbin`) and battle timing. Pure, unit-tested.
//!
//! * [`EffectTimeline`] keeps what the playback needs from an
//!   [`EffectSequence`]: the particle plays (command `0x02`, VERIFIED paths)
//!   with their start/end frames (LIKELY) and side (`0x09`, LIKELY: user or
//!   target). Every other command type (camera `0x59`.., sounds, model motion,
//!   scale `0x0d`, ~120 unknown types) is ignored: camera shake, flashes and
//!   colour changes are not decoded, so none are played.
//! * [`MoveTimingTable`] turns a move's timeline length and first target frame
//!   (from `data/move_effect_map.tsv`) into the battle pacing used by the
//!   animations, the hit sounds and the effects ([`MoveTiming`]), so the three
//!   stay in sync.
//! * [`effects_for_events`] maps battle engine events to timed effect plays,
//!   with the same cursor logic as `battle_anim::requests_for_events_timed`.

use std::collections::HashMap;

use crate::assets::effect_sequence::{sequence_name, EffectSequence, Side, PARTICLE_PLAY};
use crate::battle_engine::{Event, MajorStatus};
use crate::game::battle_anim::{MoveTiming, HIT_DELAY_S};

use super::sim::FPS;

/// Shortest and longest time a move occupies in a turn when its timeline is known (ASSUMED bounds:
/// they keep a very short or very long timeline from stalling or rushing the turn).
pub const MIN_STEP_S: f32 = 0.9;
pub const MAX_STEP_S: f32 = 4.0;

/// Who a particle play is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaySide {
    User,
    Target,
    /// Side byte outside 0/1 (ASSUMED: midway between the two).
    Field,
}

/// One particle file started by a timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticlePlay {
    /// Path under `bin/effect/battle/particle/`.
    pub path: String,
    pub start: u32,
    pub end: u32,
    pub side: PlaySide,
}

impl ParticlePlay {
    /// Emission window in frames (at least 1).
    pub fn emit_frames(&self) -> u32 {
        self.end.saturating_sub(self.start).max(1)
    }
}

/// The playable part of a sequence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectTimeline {
    pub name: String,
    pub plays: Vec<ParticlePlay>,
    pub length_frames: u32,
    pub first_target_frame: Option<u32>,
}

impl EffectTimeline {
    pub fn from_sequence(name: &str, seq: &EffectSequence) -> EffectTimeline {
        let plays = seq
            .particles()
            .filter(|c| c.kind == PARTICLE_PLAY)
            .filter_map(|c| {
                Some(ParticlePlay {
                    path: c.particle.clone()?,
                    start: c.start,
                    end: c.end.max(c.start),
                    side: match c.side {
                        Some(Side::User) | None => PlaySide::User,
                        Some(Side::Target) => PlaySide::Target,
                        Some(Side::Unknown(_)) => PlaySide::Field,
                    },
                })
            })
            .collect();
        EffectTimeline {
            name: name.to_string(),
            plays,
            length_frames: seq.length_frames(),
            first_target_frame: seq.first_target_frame(),
        }
    }

    /// A one-play timeline for a lone particle file (emits for `frames`, on the user).
    pub fn single(path: &str, frames: u32) -> EffectTimeline {
        EffectTimeline {
            name: path.to_string(),
            plays: vec![ParticlePlay { path: path.to_string(), start: 0, end: frames, side: PlaySide::User }],
            length_frames: frames,
            first_target_frame: None,
        }
    }

    /// Indices of the plays starting in `[from, to)` (frames).
    pub fn due(&self, from: f32, to: f32) -> Vec<usize> {
        self.plays
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                let s = p.start as f32;
                s >= from && s < to
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Distinct particle files.
    pub fn particle_paths(&self) -> Vec<String> {
        let mut v: Vec<String> = self.plays.iter().map(|p| p.path.clone()).collect();
        v.sort();
        v.dedup();
        v
    }

    pub fn timing(&self) -> MoveTiming {
        move_timing(self.length_frames, self.first_target_frame)
    }
}

/// Turn pacing of a move from its timeline: the move lasts its timeline
/// (clamped to [`MIN_STEP_S`]..[`MAX_STEP_S`]) and the target reacts when the
/// first particle lands on it (LIKELY hit moment), else after the default delay.
pub fn move_timing(length_frames: u32, first_target_frame: Option<u32>) -> MoveTiming {
    let step_s = (length_frames as f32 / FPS).clamp(MIN_STEP_S, MAX_STEP_S);
    let hit_s = first_target_frame
        .map(|f| f as f32 / FPS)
        .filter(|h| *h < step_s)
        .unwrap_or(HIT_DELAY_S.min(step_s));
    MoveTiming { step_s, hit_s }
}

/// Move timings by sequence name, read from `data/move_effect_map.tsv`.
#[derive(Debug, Clone, Default, PartialEq, bevy::prelude::Resource)]
pub struct MoveTimingTable {
    pub by_sequence: HashMap<String, MoveTiming>,
}

impl MoveTimingTable {
    /// Parse the sheet (columns `sequence`, `seq_frames`, `first_target_frame`).
    pub fn parse(text: &str) -> Result<MoveTimingTable, String> {
        let mut lines = text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty());
        let header: Vec<&str> = lines.next().ok_or("empty sheet")?.split('\t').collect();
        let col = |name: &str| {
            header
                .iter()
                .position(|h| h.split(':').next() == Some(name))
                .ok_or_else(|| format!("missing column {name}"))
        };
        let (cs, cf, ct) = (col("sequence")?, col("seq_frames")?, col("first_target_frame")?);
        let mut by_sequence = HashMap::new();
        for l in lines {
            let f: Vec<&str> = l.split('\t').collect();
            let (Some(s), Some(fr), Some(ft)) = (f.get(cs), f.get(cf), f.get(ct)) else {
                continue;
            };
            let Ok(frames) = fr.parse::<u32>() else { continue };
            let first = ft.parse::<i32>().ok().filter(|v| *v >= 0).map(|v| v as u32);
            by_sequence.insert(s.to_string(), move_timing(frames, first));
        }
        Ok(MoveTimingTable { by_sequence })
    }

    /// Timing of a move used by a species/form (species overrides first), if known.
    pub fn timing(&self, move_id: u16, species: u16, form: u8) -> Option<MoveTiming> {
        self.by_sequence.get(&sequence_name(move_id, species, form)).copied()
    }
}

/// Sequence played on the Pokémon that receives a status (names LIKELY: `es001_nemuke`
/// drowsy, `es002_poison`, `es003_fire` burn, `es004_ice` frostbite, `es005_paralysis`).
pub fn status_sequence(status: MajorStatus) -> &'static str {
    match status {
        MajorStatus::Drowsy => "es001",
        MajorStatus::Poison => "es002",
        MajorStatus::Burn => "es003",
        MajorStatus::Frostbite => "es004",
        MajorStatus::Paralysis => "es005",
    }
}

/// Battle-wide hit sequences by outcome (LIKELY by name).
pub const CRITICAL_SEQUENCE: &str = "cmn_critical";
pub const SUPER_EFFECTIVE_SEQUENCE: &str = "cmn_affinity_advantage";
pub const NOT_VERY_EFFECTIVE_SEQUENCE: &str = "cmn_affinity_disadvantage";

/// A timeline to play for battle combatants.
#[derive(Debug, Clone, PartialEq)]
pub struct TimedEffect {
    pub sequence: String,
    /// Combatant on the user side of the timeline.
    pub user: usize,
    /// Combatant on the target side.
    pub target: usize,
    /// Seconds after the turn's events arrived.
    pub delay: f32,
    /// Leave out the plays on the target (missed move; ASSUMED).
    pub skip_target: bool,
}

/// Effects for a batch of engine events. `move_of(actor, name)` gives the move
/// id and the user's species; `timing(actor, name)` the move's pacing (the same
/// closure the animations and hit sounds use).
pub fn effects_for_events(
    events: &[Event],
    move_of: &dyn Fn(usize, &str) -> Option<(u16, u16)>,
    timing: &dyn Fn(usize, &str) -> MoveTiming,
) -> Vec<TimedEffect> {
    let mut out = Vec::new();
    let mut cursor = 0.0_f32;
    let mut last_action = 0.0_f32;
    let mut last_hit = HIT_DELAY_S;
    for ev in events {
        match ev {
            Event::UsedMove { actor, target, name, damage, critical, effectiveness } => {
                let t = timing(*actor, name);
                if let Some((id, species)) = move_of(*actor, name) {
                    out.push(TimedEffect {
                        sequence: sequence_name(id, species, 0),
                        user: *actor,
                        target: *target,
                        delay: cursor,
                        skip_target: false,
                    });
                }
                if *damage > 0 {
                    let extra = if *critical {
                        Some(CRITICAL_SEQUENCE)
                    } else {
                        match effectiveness.as_str() {
                            "super effective" => Some(SUPER_EFFECTIVE_SEQUENCE),
                            "not very effective" => Some(NOT_VERY_EFFECTIVE_SEQUENCE),
                            _ => None,
                        }
                    };
                    if let Some(seq) = extra {
                        out.push(TimedEffect {
                            sequence: seq.into(),
                            user: *target,
                            target: *target,
                            delay: cursor + t.hit_s,
                            skip_target: false,
                        });
                    }
                }
                last_action = cursor;
                last_hit = t.hit_s;
                cursor += t.step_s;
            }
            Event::Missed { actor, target, name } => {
                let t = timing(*actor, name);
                if let Some((id, species)) = move_of(*actor, name) {
                    out.push(TimedEffect {
                        sequence: sequence_name(id, species, 0),
                        user: *actor,
                        target: *target,
                        delay: cursor,
                        skip_target: true,
                    });
                }
                last_action = cursor;
                last_hit = t.hit_s;
                cursor += t.step_s;
            }
            Event::StatusApplied { target, status } => out.push(TimedEffect {
                sequence: status_sequence(*status).into(),
                user: *target,
                target: *target,
                delay: cursor.max(last_action + last_hit),
                skip_target: false,
            }),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::battle_anim::{requests_for_events_timed, MOVE_STEP_S};
    use crate::game::pokemon_anim::PokemonAnimRequest;

    fn used(actor: usize, target: usize, name: &str, damage: u32, critical: bool, eff: &str) -> Event {
        Event::UsedMove { actor, target, name: name.into(), damage, critical, effectiveness: eff.into() }
    }

    fn move_of(_: usize, name: &str) -> Option<(u16, u16)> {
        match name {
            "Thunderbolt" => Some((85, 25)),
            "Tackle" => Some((33, 399)),
            "Ember" => Some((52, 4)),
            _ => None,
        }
    }

    fn table() -> MoveTimingTable {
        let text = "# sheet: x\nmove:u16*\tmove_name:string\tsequence:string\tseq_frames:u16\tfirst_target_frame:i32\n\
                    85\tthunderbolt\tew085\t91\t38\n33\ttackle\tew033\t60\t30\n52\tember\tew052\t240\t24\n99\tx\tew099\t10\t-1\n";
        MoveTimingTable::parse(text).unwrap()
    }

    #[test]
    fn timing_comes_from_the_timeline_with_bounds() {
        let t = table();
        let tb = t.timing(85, 25, 0).unwrap();
        assert!((tb.step_s - 91.0 / 30.0).abs() < 1e-5);
        assert!((tb.hit_s - 38.0 / 30.0).abs() < 1e-5);
        assert_eq!(t.timing(52, 4, 0).unwrap().step_s, MAX_STEP_S, "long timelines are capped");
        let short = t.timing(99, 1, 0).unwrap();
        assert_eq!(short.step_s, MIN_STEP_S);
        assert_eq!(short.hit_s, HIT_DELAY_S, "no target frame: default hit delay");
        assert_eq!(t.timing(1, 1, 0), None);
    }

    #[test]
    fn real_move_effect_map_parses_if_present() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/move_effect_map.tsv");
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("[skip] {} missing", path.display());
            return;
        };
        let t = MoveTimingTable::parse(&text).unwrap();
        assert_eq!(t.by_sequence.len(), 192, "177 default + 15 species overrides");
        let tb = t.timing(85, 25, 0).unwrap();
        assert!((tb.hit_s - 38.0 / 30.0).abs() < 1e-5);
    }

    #[test]
    fn timeline_plays_are_scheduled_by_frame() {
        let tl = EffectTimeline {
            name: "t".into(),
            plays: vec![
                ParticlePlay { path: "a".into(), start: 0, end: 30, side: PlaySide::User },
                ParticlePlay { path: "b".into(), start: 24, end: 60, side: PlaySide::Target },
                ParticlePlay { path: "a".into(), start: 40, end: 41, side: PlaySide::Field },
            ],
            length_frames: 60,
            first_target_frame: Some(24),
        };
        assert_eq!(tl.due(0.0, 1.0), vec![0]);
        assert_eq!(tl.due(1.0, 24.0), Vec::<usize>::new());
        assert_eq!(tl.due(24.0, 50.0), vec![1, 2]);
        assert_eq!(tl.particle_paths(), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(tl.plays[2].emit_frames(), 1);
        // Covering the whole length in any step pattern schedules each play exactly once.
        let mut seen = vec![0; 3];
        let mut t = 0.0;
        while t < 61.0 {
            for i in tl.due(t, t + 0.7) {
                seen[i] += 1;
            }
            t += 0.7;
        }
        assert_eq!(seen, vec![1, 1, 1]);
    }

    #[test]
    fn battle_events_start_move_hit_and_status_effects() {
        let t = table();
        let timing = |actor: usize, name: &str| {
            move_of(actor, name).and_then(|(m, s)| t.timing(m, s, 0)).unwrap_or_default()
        };
        let events = vec![
            used(0, 1, "Thunderbolt", 20, false, "super effective"),
            Event::StatusApplied { target: 1, status: MajorStatus::Paralysis },
            used(1, 0, "Tackle", 5, true, "normal"),
            Event::Missed { actor: 0, target: 1, name: "Thunderbolt".into() },
            used(1, 0, "Splash", 0, false, "normal"),
        ];
        let fx = effects_for_events(&events, &move_of, &timing);
        let tb = t.timing(85, 25, 0).unwrap();
        let tk = t.timing(33, 399, 0).unwrap();
        let names: Vec<&str> = fx.iter().map(|e| e.sequence.as_str()).collect();
        assert_eq!(names, ["ew085", "cmn_affinity_advantage", "es005", "ew033", "cmn_critical", "ew085"]);
        assert_eq!(fx[0].delay, 0.0);
        assert!((fx[1].delay - tb.hit_s).abs() < 1e-5, "super-effective mark when the bolt lands");
        assert_eq!((fx[2].user, fx[2].target), (1, 1));
        assert!((fx[3].delay - tb.step_s).abs() < 1e-5, "second move waits for the first timeline");
        assert!((fx[4].delay - (tb.step_s + tk.hit_s)).abs() < 1e-5);
        assert!(fx[5].skip_target, "missed move: no target particles");
        assert!((fx[5].delay - (tb.step_s + tk.step_s)).abs() < 1e-5);
    }

    #[test]
    fn effect_hits_line_up_with_the_hit_animation() {
        let t = table();
        let timing = |actor: usize, name: &str| {
            move_of(actor, name).and_then(|(m, s)| t.timing(m, s, 0)).unwrap_or_default()
        };
        let events = vec![used(0, 1, "Thunderbolt", 20, true, "normal"), used(1, 0, "Ember", 7, true, "normal")];
        let fx = effects_for_events(&events, &move_of, &timing);
        let anims = requests_for_events_timed(&events, &|_, _| Some("Special".into()), &timing);
        let hits: Vec<f32> = anims.iter().filter(|r| r.request == PokemonAnimRequest::Hit).map(|r| r.delay).collect();
        let crits: Vec<f32> = fx.iter().filter(|e| e.sequence == CRITICAL_SEQUENCE).map(|e| e.delay).collect();
        assert_eq!(hits, crits, "the critical mark appears when the target flinches");
        // The timeline's first target particle lands at the same moment.
        let tb = t.timing(85, 25, 0).unwrap();
        assert!((hits[0] - 38.0 / 30.0).abs() < 1e-5 && (tb.hit_s - hits[0]).abs() < 1e-6);
        // Unknown moves keep the old fixed pacing.
        assert_eq!(MoveTiming::default().step_s, MOVE_STEP_S);
    }
}
