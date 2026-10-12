//! User settings and camera axis inversion configuration.
//!
//! Stores user preferences such as camera axis inversions in a simple key=value format
//! in `settings.txt` within the save directory. All parsing and serialization logic
//! is pure and unit-tested without Bevy dependencies.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Camera and control user settings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UserSettings {
    /// Invert horizontal camera movement (yaw).
    pub invert_camera_x: bool,
    /// Invert vertical camera movement (pitch).
    pub invert_camera_y: bool,
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
                    _ => {}
                }
            }
        }
        settings
    }

    /// Serialize settings into key=value text.
    pub fn serialize(&self) -> String {
        format!(
            "# pla-port user settings\ninvert_camera_x={}\ninvert_camera_y={}\n",
            if self.invert_camera_x { "true" } else { "false" },
            if self.invert_camera_y { "true" } else { "false" },
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
}
