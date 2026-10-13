//! Volume buses and the music fader (pure math, no Bevy).
//!
//! Output gain of a sound = master × bus × per-sound gain × fade, clamped to 0..1 (linear
//! amplitude; a 50 % setting is about -6 dB). The music fader keeps several voices so a new
//! track fades in while the old one fades out (crossfade).

use super::super::settings::UserSettings;
use super::events::{Bus, MusicId};

/// Linear gains of the four volume settings (0..1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BusVolumes {
    pub master: f32,
    pub music: f32,
    pub sfx: f32,
    pub voice: f32,
}

impl Default for BusVolumes {
    fn default() -> Self {
        Self { master: 1.0, music: 1.0, sfx: 1.0, voice: 1.0 }
    }
}

fn pct(p: u8) -> f32 {
    f32::from(p.min(100)) / 100.0
}

impl BusVolumes {
    pub fn from_settings(s: &UserSettings) -> Self {
        Self {
            master: pct(s.master_volume),
            music: pct(s.music_volume),
            sfx: pct(s.sfx_volume),
            voice: pct(s.voice_volume),
        }
    }

    /// Gain of a bus including the master volume.
    pub fn bus(&self, bus: Bus) -> f32 {
        let b = match bus {
            Bus::Music => self.music,
            Bus::Sfx => self.sfx,
            Bus::Voice => self.voice,
        };
        (self.master * b).clamp(0.0, 1.0)
    }

    /// Final linear gain of a sound on `bus` with its own `gain` and fade level.
    pub fn output(&self, bus: Bus, gain: f32, fade: f32) -> f32 {
        let g = self.bus(bus) * gain * fade;
        if g.is_finite() {
            g.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// Linear ramp from `from` to `to` over `duration` seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fade {
    pub from: f32,
    pub to: f32,
    pub duration: f32,
    pub elapsed: f32,
}

impl Fade {
    pub fn new(from: f32, to: f32, duration: f32) -> Self {
        Self { from, to, duration: duration.max(0.0), elapsed: 0.0 }
    }

    pub fn value(&self) -> f32 {
        if self.duration <= 0.0 || self.elapsed >= self.duration {
            return self.to;
        }
        self.from + (self.to - self.from) * (self.elapsed / self.duration)
    }

    pub fn advance(&mut self, dt: f32) {
        self.elapsed = (self.elapsed + dt.max(0.0)).min(self.duration.max(0.0));
    }

    pub fn done(&self) -> bool {
        self.elapsed >= self.duration
    }
}

/// One music voice (a looping track) under the fader.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MusicVoice {
    pub id: u64,
    pub music: MusicId,
    pub fade: Fade,
    /// Fading out; removed when the fade ends.
    pub stopping: bool,
    /// The file is loaded and the voice is audible; the fade-in waits until then.
    pub ready: bool,
}

/// Music fader: at most one current voice plus voices fading out.
#[derive(Debug, Clone, Default)]
pub struct MusicFader {
    pub voices: Vec<MusicVoice>,
    next_id: u64,
}

/// What the caller must do after [`MusicFader::play`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicStart {
    /// The same music is already current: nothing to do.
    AlreadyPlaying,
    /// Start loading/playing a new voice with this id.
    Start(u64),
}

impl MusicFader {
    pub const DEFAULT_FADE_IN_S: f32 = 1.5;
    pub const DEFAULT_FADE_OUT_S: f32 = 1.0;

    /// The voice that is not fading out, if any.
    pub fn current(&self) -> Option<&MusicVoice> {
        self.voices.iter().find(|v| !v.stopping)
    }

    /// Switch to `music`: the current voice fades out over `fade_out`, the new one fades in
    /// over `fade_in` once it is ready (crossfade when both are > 0).
    pub fn play(&mut self, music: MusicId, fade_in: f32, fade_out: f32) -> MusicStart {
        if self.current().is_some_and(|v| v.music == music) {
            return MusicStart::AlreadyPlaying;
        }
        self.stop(fade_out);
        self.next_id += 1;
        let id = self.next_id;
        self.voices.push(MusicVoice {
            id,
            music,
            fade: Fade::new(0.0, 1.0, fade_in),
            stopping: false,
            ready: false,
        });
        MusicStart::Start(id)
    }

    /// Fade every voice out over `fade_out` seconds.
    pub fn stop(&mut self, fade_out: f32) {
        for v in self.voices.iter_mut().filter(|v| !v.stopping) {
            v.stopping = true;
            let from = if v.ready { v.fade.value() } else { 0.0 };
            v.fade = Fade::new(from, 0.0, fade_out);
        }
    }

    /// Mark a voice as audible (its fade-in starts now).
    pub fn mark_ready(&mut self, id: u64) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
            v.ready = true;
        }
    }

    /// Drop a voice whose file could not be loaded.
    pub fn forget(&mut self, id: u64) {
        self.voices.retain(|v| v.id != id);
    }

    /// Advance the fades; returns the ids of voices that finished fading out (to despawn).
    pub fn update(&mut self, dt: f32) -> Vec<u64> {
        for v in self.voices.iter_mut().filter(|v| v.ready || v.stopping) {
            v.fade.advance(dt);
        }
        let (gone, keep): (Vec<_>, Vec<_>) =
            self.voices.drain(..).partition(|v| v.stopping && v.fade.done());
        self.voices = keep;
        gone.into_iter().map(|v| v.id).collect()
    }

    /// Fade level of a voice (0 when unknown).
    pub fn level(&self, id: u64) -> f32 {
        self.voices.iter().find(|v| v.id == id).map_or(0.0, |v| v.fade.value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_math_multiplies_and_clamps() {
        let s = UserSettings { master_volume: 50, music_volume: 80, sfx_volume: 100, voice_volume: 0, ..Default::default() };
        let v = BusVolumes::from_settings(&s);
        assert!((v.bus(Bus::Music) - 0.4).abs() < 1e-6);
        assert!((v.bus(Bus::Sfx) - 0.5).abs() < 1e-6);
        assert_eq!(v.bus(Bus::Voice), 0.0);
        assert!((v.output(Bus::Sfx, 0.5, 0.5) - 0.125).abs() < 1e-6);
        assert_eq!(v.output(Bus::Sfx, 3.0, 1.0), 1.0);
        assert_eq!(v.output(Bus::Sfx, f32::NAN, 1.0), 0.0);
        assert_eq!(BusVolumes::from_settings(&UserSettings::default()), BusVolumes::default());
    }

    #[test]
    fn fade_is_linear_and_saturates() {
        let mut f = Fade::new(0.0, 1.0, 2.0);
        assert_eq!(f.value(), 0.0);
        f.advance(0.5);
        assert!((f.value() - 0.25).abs() < 1e-6);
        f.advance(10.0);
        assert_eq!(f.value(), 1.0);
        assert!(f.done());
        assert_eq!(Fade::new(0.3, 0.0, 0.0).value(), 0.0);
    }

    #[test]
    fn crossfade_old_out_new_in() {
        let mut m = MusicFader::default();
        let MusicStart::Start(a) = m.play(MusicId::Area(0), 1.0, 1.0) else { panic!() };
        assert_eq!(m.level(a), 0.0);
        m.update(0.5);
        assert_eq!(m.level(a), 0.0, "fade-in waits for the file");
        m.mark_ready(a);
        m.update(1.0);
        assert_eq!(m.level(a), 1.0);
        assert_eq!(m.play(MusicId::Area(0), 1.0, 1.0), MusicStart::AlreadyPlaying);
        let MusicStart::Start(b) = m.play(MusicId::WildBattle, 2.0, 1.0) else { panic!() };
        m.mark_ready(b);
        assert!(m.update(0.5).is_empty());
        assert!((m.level(a) - 0.5).abs() < 1e-6);
        assert!((m.level(b) - 0.25).abs() < 1e-6);
        assert_eq!(m.update(0.5), vec![a]);
        assert_eq!(m.current().map(|v| v.id), Some(b));
        assert_eq!(m.voices.len(), 1);
    }

    #[test]
    fn stop_fades_everything_and_unready_voices_end_silently() {
        let mut m = MusicFader::default();
        let MusicStart::Start(a) = m.play(MusicId::Area(1), 1.0, 1.0) else { panic!() };
        m.stop(0.5);
        assert_eq!(m.level(a), 0.0);
        assert_eq!(m.update(0.6), vec![a]);
        assert!(m.current().is_none());
        let MusicStart::Start(b) = m.play(MusicId::Area(2), 1.0, 1.0) else { panic!() };
        m.forget(b);
        assert!(m.voices.is_empty());
    }
}
