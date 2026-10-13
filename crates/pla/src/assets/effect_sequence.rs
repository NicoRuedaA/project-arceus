//! Battle effect sequences (`bin/effect/battle/sequence/*.wbin`).
//!
//! A sequence is a FlatBuffers timeline: a vector of tracks, each with a vector
//! of commands. A command is a union (u8 type + table); nearly every command
//! table starts with a start frame (field 0) and an end frame (field 1).
//! Decoded here (see `reports/data-progress/effects.md` for the evidence):
//! * type `0x02`: play a particle file; field 2 is its path relative to
//!   `bin/effect/battle/particle/` (VERIFIED: 674/676 referenced paths exist).
//! * type `0x09` right after a `0x02` in the same track: placement; field 2 is
//!   the side (0 = user, 1 = target; LIKELY, matches `_at_`/`_df_` names for
//!   384/402 named particles) and field 3 a locator id (ASSUMED).
//! * types `0x94`, `0x95`, `0x96`: sound events; field 3 is the event name.
//!
//! Every other command type is kept as raw `(type, start, end)` so later work
//! can add meanings (camera, model motion, hit timing...) without re-parsing.
//!
//! Which file a move uses: [`sequence_name`] (VERIFIED, see its doc).

/// Side a particle is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    User,
    Target,
    Unknown(u8),
}

/// One timeline command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub kind: u8,
    pub start: u32,
    pub end: u32,
    /// Particle path for `0x02`.
    pub particle: Option<String>,
    /// Placement for `0x02` (taken from the following `0x09`).
    pub side: Option<Side>,
    pub locator: Option<u8>,
    /// Sound event name for `0x94`..`0x96`.
    pub sound: Option<String>,
}

/// A parsed sequence: tracks of commands.
#[derive(Debug, Clone, Default)]
pub struct EffectSequence {
    pub tracks: Vec<Vec<Command>>,
}

pub const PARTICLE_PLAY: u8 = 0x02;
pub const PARTICLE_PLACE: u8 = 0x09;

fn u32_at(b: &[u8], o: usize) -> Result<u32, String> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| format!("read out of range at {o:#x}"))
}

fn deref(b: &[u8], p: usize) -> Result<usize, String> {
    let r = p
        .checked_add(u32_at(b, p)? as usize)
        .filter(|&r| r < b.len())
        .ok_or_else(|| format!("bad offset at {p:#x}"))?;
    Ok(r)
}

/// Absolute positions of a table's present fields (index -> position).
fn fields(b: &[u8], t: usize) -> Result<Vec<Option<usize>>, String> {
    let soff = i32::from_le_bytes(
        b.get(t..t + 4)
            .ok_or("table out of range")?
            .try_into()
            .unwrap(),
    );
    let vt = (t as i64 - soff as i64) as usize;
    let vsize = b
        .get(vt..vt + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]) as usize)
        .ok_or("vtable out of range")?;
    if !(4..=512).contains(&vsize) {
        return Err(format!("bad vtable size {vsize}"));
    }
    (0..(vsize - 4) / 2)
        .map(|i| {
            let o = vt + 4 + 2 * i;
            let f = b
                .get(o..o + 2)
                .map(|s| u16::from_le_bytes([s[0], s[1]]) as usize)
                .ok_or("vtable entry out of range")?;
            Ok((f != 0).then_some(t + f))
        })
        .collect()
}

fn get(f: &[Option<usize>], i: usize) -> Option<usize> {
    f.get(i).copied().flatten()
}

fn string(b: &[u8], p: usize) -> Result<String, String> {
    let s = deref(b, p)?;
    let n = u32_at(b, s)? as usize;
    let bytes = b.get(s + 4..s + 4 + n).ok_or("string out of range")?;
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

fn vector(b: &[u8], p: usize) -> Result<(usize, usize), String> {
    let v = deref(b, p)?;
    let n = u32_at(b, v)? as usize;
    if n > 4096 || v + 4 + 4 * n > b.len() {
        return Err("vector out of range".into());
    }
    Ok((v + 4, n))
}

impl EffectSequence {
    pub fn parse(b: &[u8]) -> Result<EffectSequence, String> {
        let root = u32_at(b, 0)? as usize;
        let rf = fields(b, root)?;
        let (tv, tn) = vector(b, get(&rf, 0).ok_or("no track vector")?)?;
        let mut tracks = Vec::with_capacity(tn);
        for k in 0..tn {
            let t = deref(b, tv + 4 * k)?;
            let tf = fields(b, t)?;
            let mut cmds: Vec<Command> = Vec::new();
            if let Some(cl) = get(&tf, 2) {
                let (cv, cn) = vector(b, cl)?;
                for j in 0..cn {
                    let c = deref(b, cv + 4 * j)?;
                    let cf = fields(b, c)?;
                    let kind = get(&cf, 0).map(|p| b[p]).unwrap_or(0);
                    let Some(val) = get(&cf, 1) else {
                        continue;
                    };
                    let vf = fields(b, deref(b, val)?)?;
                    let start = get(&vf, 0).map(|p| u32_at(b, p)).transpose()?.unwrap_or(0);
                    let end = get(&vf, 1)
                        .map(|p| u32_at(b, p))
                        .transpose()?
                        .unwrap_or(start);
                    let mut cmd = Command {
                        kind,
                        start,
                        end,
                        particle: None,
                        side: None,
                        locator: None,
                        sound: None,
                    };
                    match kind {
                        PARTICLE_PLAY => {
                            cmd.particle = get(&vf, 2).map(|p| string(b, p)).transpose()?;
                        }
                        PARTICLE_PLACE => {
                            let side = match get(&vf, 2).map(|p| b[p]).unwrap_or(0) {
                                0 => Side::User,
                                1 => Side::Target,
                                v => Side::Unknown(v),
                            };
                            let loc = get(&vf, 3).map(|p| b[p]).unwrap_or(0);
                            if let Some(play) = cmds
                                .iter_mut()
                                .rev()
                                .find(|c| c.kind == PARTICLE_PLAY)
                                .filter(|c| c.side.is_none())
                            {
                                play.side = Some(side);
                                play.locator = Some(loc);
                            }
                        }
                        0x94..=0x96 => {
                            cmd.sound = get(&vf, 3).map(|p| string(b, p)).transpose()?;
                        }
                        _ => {}
                    }
                    cmds.push(cmd);
                }
            }
            tracks.push(cmds);
        }
        Ok(EffectSequence { tracks })
    }

    /// All particle plays in track order.
    pub fn particles(&self) -> impl Iterator<Item = &Command> {
        self.tracks
            .iter()
            .flatten()
            .filter(|c| c.particle.is_some())
    }

    /// Last frame used by any command.
    pub fn length_frames(&self) -> u32 {
        self.tracks
            .iter()
            .flatten()
            .map(|c| c.end)
            .max()
            .unwrap_or(0)
    }

    /// First frame a particle is placed on the target (proxy for the hit moment, LIKELY).
    pub fn first_target_frame(&self) -> Option<u32> {
        self.particles()
            .filter(|c| c.side == Some(Side::Target))
            .map(|c| c.start)
            .min()
    }
}

/// Species/form specific sequences, `(move, species, form or any, file stem)`.
/// Read from the table the game indexes when it builds the file name
/// (function FUN_0089f36c, update main NSO; VERIFIED from the table bytes).
pub const SPECIES_OVERRIDES: [(u16, u16, Option<u8>, &str); 15] = [
    (449, 493, None, "ew449_pokeonly"),
    (459, 483, Some(0), "ew459_pokeonly"),
    (459, 483, Some(1), "ew459_pokeonly_1"),
    (460, 484, Some(0), "ew460_pokeonly"),
    (460, 484, Some(1), "ew460_pokeonly_1"),
    (828, 899, None, "ew828_pokeonly"),
    (831, 905, Some(0), "ew831_pokeonly"),
    (831, 905, Some(1), "ew831_pokeonly_1"),
    (837, 549, Some(1), "ew837_pokeonly"),
    (838, 901, None, "ew838_pokeonly"),
    (840, 628, Some(1), "ew840_pokeonly"),
    (843, 724, Some(1), "ew843_pokeonly"),
    (845, 503, Some(1), "ew845_pokeonly"),
    (467, 487, Some(1), "ew467_1"),
    (827, 903, None, "ew827_pokeonly"),
];

/// File stem of the sequence a move plays: the species override when the user
/// matches, else `ew%03d` of the move id (format string used by the game).
pub fn sequence_name(move_id: u16, species: u16, form: u8) -> String {
    SPECIES_OVERRIDES
        .iter()
        .find(|(m, s, f, _)| *m == move_id && *s == species && f.is_none_or(|f| f == form))
        .map(|(_, _, _, name)| (*name).to_string())
        .unwrap_or_else(|| format!("ew{move_id:03}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_override_names() {
        assert_eq!(sequence_name(85, 25, 0), "ew085");
        assert_eq!(sequence_name(33, 1, 0), "ew033");
        assert_eq!(sequence_name(449, 493, 7), "ew449_pokeonly");
        assert_eq!(sequence_name(459, 483, 1), "ew459_pokeonly_1");
        assert_eq!(sequence_name(459, 483, 2), "ew459");
        assert_eq!(sequence_name(845, 503, 0), "ew845");
        assert_eq!(sequence_name(845, 503, 1), "ew845_pokeonly");
    }

    #[test]
    fn rejects_garbage() {
        assert!(EffectSequence::parse(&[]).is_err());
        assert!(EffectSequence::parse(&[0xFF, 0, 0, 0, 1, 2, 3, 4]).is_err());
    }

    #[test]
    fn thunderbolt_and_ember_timelines_if_present() {
        let root = std::env::var_os("PLA_BIN_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into()
            })
            .join("effect/battle/sequence");
        if !root.join("ew085.wbin").exists() {
            eprintln!("[skip] game sequences not present");
            return;
        }
        let tb = EffectSequence::parse(&std::fs::read(root.join("ew085.wbin")).unwrap()).unwrap();
        let plays: Vec<_> = tb.particles().collect();
        assert_eq!(plays.len(), 6);
        assert_eq!(
            plays[0].particle.as_deref(),
            Some("ew085/ew085_at_start.ptcl")
        );
        assert_eq!(plays[0].side, Some(Side::User));
        assert_eq!(tb.first_target_frame(), Some(38));
        assert!(plays.iter().any(
            |c| c.particle.as_deref() == Some("ew085/ew085_df_hit01.ptcl")
                && c.side == Some(Side::Target)
        ));
        assert_eq!(tb.length_frames(), 91);

        let ember =
            EffectSequence::parse(&std::fs::read(root.join("ew052.wbin")).unwrap()).unwrap();
        let names: Vec<_> = ember
            .particles()
            .filter_map(|c| c.particle.clone())
            .collect();
        assert_eq!(
            names,
            [
                "ew052/ew052_fire_muzzle.ptcl",
                "ew052/ew052_bullet.ptcl",
                "ew052/ew052_hit.ptcl"
            ]
        );
        assert_eq!(ember.first_target_frame(), Some(24));
        let sounds: Vec<_> = ember
            .tracks
            .iter()
            .flatten()
            .filter_map(|c| c.sound.clone())
            .collect();
        assert_eq!(
            sounds,
            ["PLAY_EW0052_01", "PLAY_EW0052_02", "PLAY_EW0052_03"]
        );
    }
}
