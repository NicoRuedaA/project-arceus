//! User settings: camera axis inversion and audio volumes.
//!
//! Stores user preferences such as camera axis inversions and the four volume buses
//! (`master_volume`, `music_volume`, `sfx_volume`, `voice_volume`, whole percent 0-100,
//! clamped; missing keys keep the default 100) in a simple key=value format
//! in `settings.txt` within the save directory. All parsing and serialization logic
//! is pure and unit-tested without Bevy dependencies.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use bevy::prelude::Resource;

/// Camera, control and audio user settings.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct UserSettings {
    /// Invert horizontal camera movement (yaw).
    pub invert_camera_x: bool,
    /// Invert vertical camera movement (pitch).
    pub invert_camera_y: bool,
    /// Volume of everything, percent 0-100.
    pub master_volume: u8,
    /// Background music and jingles, percent 0-100.
    pub music_volume: u8,
    /// Sound effects (UI, battle, balls), percent 0-100.
    pub sfx_volume: u8,
    /// Pokémon cries, percent 0-100.
    pub voice_volume: u8,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            invert_camera_x: false,
            invert_camera_y: false,
            master_volume: 100,
            music_volume: 100,
            sfx_volume: 100,
            voice_volume: 100,
        }
    }
}

/// Parse a volume value: a number (optionally followed by `%`), rounded and clamped to 0-100.
/// Returns `None` for text that is not a number (the key then keeps its previous value).
pub fn parse_volume(v: &str) -> Option<u8> {
    let n: f32 = v.trim().trim_end_matches('%').trim().parse().ok()?;
    if !n.is_finite() {
        return None;
    }
    Some(n.round().clamp(0.0, 100.0) as u8)
}

impl UserSettings {
    pub const FILE_NAME: &'static str = "settings.txt";

    /// Parse key=value text lines into `UserSettings`.
    pub fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                let v = v.trim();
                let val = matches!(v, "1" | "true" | "TRUE" | "on" | "ON" | "yes" | "YES");
                match k {
                    "invert_camera_x" => settings.invert_camera_x = val,
                    "invert_camera_y" => settings.invert_camera_y = val,
                    "master_volume" | "music_volume" | "sfx_volume" | "voice_volume" => {
                        if let Some(p) = parse_volume(v) {
                            match k {
                                "master_volume" => settings.master_volume = p,
                                "music_volume" => settings.music_volume = p,
                                "sfx_volume" => settings.sfx_volume = p,
                                _ => settings.voice_volume = p,
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        settings
    }

    /// Serialize settings into key=value text.
    pub fn serialize(&self) -> String {
        format!(
            "# pla-port user settings\ninvert_camera_x={}\ninvert_camera_y={}\n\
             # volumes in percent (0-100)\nmaster_volume={}\nmusic_volume={}\nsfx_volume={}\nvoice_volume={}\n",
            if self.invert_camera_x { "true" } else { "false" },
            if self.invert_camera_y { "true" } else { "false" },
            self.master_volume,
            self.music_volume,
            self.sfx_volume,
            self.voice_volume,
        )
    }

    /// Load settings from the given directory (or default if missing or unreadable).
    pub fn load_from_dir(dir: &Path) -> Self {
        let path = dir.join(Self::FILE_NAME);
        match fs::read_to_string(&path) {
            Ok(content) => Self::parse(&content),
            Err(_) => Self::default(),
        }
    }

    /// Save settings into the given directory atomically or via standard write.
    pub fn save_to_dir(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)?;
        let path = dir.join(Self::FILE_NAME);
        let tmp = dir.join(format!("{}.tmp", Self::FILE_NAME));
        let content = self.serialize();
        fs::write(&tmp, content.as_bytes())?;
        fs::rename(&tmp, &path)
    }

    /// Directory holding `settings.txt`: the same one as the saves
    /// (`PLA_SAVE_DIR`, else `$XDG_DATA_HOME/pla-port`, else `~/.local/share/pla-port`).
    pub fn default_dir() -> PathBuf {
        super::save_game::default_save_dir()
    }

    /// Apply the axis inversions to a camera input delta `(yaw_input, pitch_input)`.
    pub fn apply_invert(&self, yaw_input: f32, pitch_input: f32) -> (f32, f32) {
        (yaw_input * self.yaw_sign(), pitch_input * self.pitch_sign())
    }

    /// Multiplier for horizontal mouse movement given the settings.
    pub fn yaw_sign(&self) -> f32 {
        if self.invert_camera_x { -1.0 } else { 1.0 }
    }

    /// Multiplier for vertical mouse movement given the settings.
    pub fn pitch_sign(&self) -> f32 {
        if self.invert_camera_y { -1.0 } else { 1.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_default_and_comments() {
        let text = "# comment\n\ninvert_camera_x=false\ninvert_camera_y=true\n";
        let s = UserSettings::parse(text);
        assert!(!s.invert_camera_x);
        assert!(s.invert_camera_y);
        assert_eq!(s.yaw_sign(), 1.0);
        assert_eq!(s.pitch_sign(), -1.0);
    }

    #[test]
    fn test_parse_boolean_variants() {
        let text = "invert_camera_x=1\ninvert_camera_y=ON";
        let s = UserSettings::parse(text);
        assert!(s.invert_camera_x);
        assert!(s.invert_camera_y);

        let text2 = "invert_camera_x=0\ninvert_camera_y=false";
        let s2 = UserSettings::parse(text2);
        assert!(!s2.invert_camera_x);
        assert!(!s2.invert_camera_y);
    }

    #[test]
    fn test_roundtrip_serialization() {
        let mut s = UserSettings::default();
        s.invert_camera_x = true;
        s.invert_camera_y = false;
        let serialized = s.serialize();
        let parsed = UserSettings::parse(&serialized);
        assert_eq!(s, parsed);
    }

    #[test]
    fn test_save_and_load_in_temp_dir() {
        let temp = std::env::temp_dir().join(format!("pla_test_settings_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);

        let mut s = UserSettings::default();
        s.invert_camera_x = true;
        s.invert_camera_y = true;

        assert!(s.save_to_dir(&temp).is_ok());
        let loaded = UserSettings::load_from_dir(&temp);
        assert_eq!(s, loaded);

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_load_from_missing_dir_returns_default() {
        let missing = PathBuf::from("/non/existent/path/for/pla/test");
        let s = UserSettings::load_from_dir(&missing);
        assert_eq!(s, UserSettings::default());
    }

    #[test]
    fn test_volumes_default_to_full_when_missing() {
        let s = UserSettings::parse("invert_camera_x=true\n");
        assert_eq!((s.master_volume, s.music_volume, s.sfx_volume, s.voice_volume), (100, 100, 100, 100));
    }

    #[test]
    fn test_volumes_parse_clamp_and_ignore_garbage() {
        let s = UserSettings::parse(
            "master_volume=80\nmusic_volume=150\nsfx_volume=-5\nvoice_volume=42.6%\n",
        );
        assert_eq!(s.master_volume, 80);
        assert_eq!(s.music_volume, 100);
        assert_eq!(s.sfx_volume, 0);
        assert_eq!(s.voice_volume, 43);
        let g = UserSettings::parse("music_volume=loud\nsfx_volume=nan\n");
        assert_eq!(g.music_volume, 100);
        assert_eq!(g.sfx_volume, 100);
        assert_eq!(parse_volume(" 55 % "), Some(55));
        assert_eq!(parse_volume("inf"), None);
    }

    #[test]
    fn test_volumes_roundtrip() {
        let s = UserSettings { master_volume: 70, music_volume: 25, sfx_volume: 0, voice_volume: 100, ..Default::default() };
        assert_eq!(UserSettings::parse(&s.serialize()), s);
    }

    #[test]
    fn test_apply_invert_axes_are_independent() {
        let mut s = UserSettings::default();
        assert_eq!(s.apply_invert(3.0, 2.0), (3.0, 2.0));
        s.invert_camera_x = true;
        assert_eq!(s.apply_invert(3.0, 2.0), (-3.0, 2.0));
        s.invert_camera_y = true;
        assert_eq!(s.apply_invert(3.0, 2.0), (-3.0, -2.0));
        s.invert_camera_x = false;
        assert_eq!(s.apply_invert(3.0, 2.0), (3.0, -2.0));
    }
}
