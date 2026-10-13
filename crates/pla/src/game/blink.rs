//! Eye blinking (Brief 22).
//!
//! # Data
//!
//! `bin/misc/app_config/character_blink_config.bin` (decoded into `data/character_face.tsv`) has
//! ten parameters: a minimum and maximum interval between blinks for the player (30 and 150),
//! the player when hurt (30 and 90), Pokemon (30 and 150), NPCs (30 and 150) and ridden Pokemon
//! (30 and 150). The values are VERIFIED as read; that they are frames and that a frame is 1/30 s
//! is ASSUMED (the other `*_frame` values in the face config read naturally at 30 per second).
//!
//! The blink itself: Pokemon packs carry a 0.35 s one-shot clip `eye01` (328 of the clips in the
//! 356 packs) that animates only eyelid bones (`left_upper_eyelid*`, `right_upper_eyelid*`,
//! `left_lower_eyelid`, `right_lower_eyelid`), plus a `mouth01` clip. That is the blink animation
//! (PROBABLE from the name, the length and the bones; no code was read). The port does NOT play it
//! yet: the exported Pokemon glTF files only carry walk, idle and the eight battle clips, and
//! playing a bone-masked clip on top of the body animation needs an exporter and graph change
//! that could not be checked without a window.
//!
//! The player has no blink clip among its 423 clips. The player's eyes are separate meshes
//! (`eye_mesh_shape#eye` for the iris disc, `eyehilight_mesh_shape#eye` for the highlight), so the
//! port blinks by hiding those two meshes for [`BLINK_SECONDS`] (SUPUESTO: the original swaps the
//! eye texture layer or UV, which is not decoded). `PLA_PLAYER_BLINK=0` turns it off.

use bevy::prelude::*;

use super::wild::splitmix64;
use super::PlayerAvatar;

/// Seconds per frame of the config values (ASSUMED 30 per second).
pub const FRAME_SECONDS: f32 = 1.0 / 30.0;
/// Length of the Pokemon `eye01` blink clip (VERIFIED: 0.35 s). The player's blink reuses it.
pub const BLINK_SECONDS: f32 = 0.35;
/// How long the player's eye meshes stay hidden within one blink: the middle of the clip length
/// (ASSUMED; a real eyelid is closed for a fraction of the clip).
pub const PLAYER_EYES_HIDDEN_SECONDS: f32 = 0.18;

/// Who blinks; each kind has its own decoded interval range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlinkKind {
    Player,
    /// Player with low HP (the port has no player HP; unused).
    PlayerPinch,
    Pokemon,
    Npc,
    Ride,
}

/// Decoded interval range in frames: `(min, max)` from `character_blink_config.bin`.
pub fn interval_frames(kind: BlinkKind) -> (u32, u32) {
    match kind {
        BlinkKind::Player => (30, 150),
        BlinkKind::PlayerPinch => (30, 90),
        BlinkKind::Pokemon => (30, 150),
        BlinkKind::Npc => (30, 150),
        BlinkKind::Ride => (30, 150),
    }
}

/// Seconds until the next blink, drawn uniformly from the decoded range.
pub fn next_interval(kind: BlinkKind, rng: &mut u64) -> f32 {
    let (lo, hi) = interval_frames(kind);
    let span = u64::from(hi.saturating_sub(lo)) + 1;
    let frames = u64::from(lo) + splitmix64(rng) % span;
    frames as f32 * FRAME_SECONDS
}

/// One entity's blink schedule.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Blink {
    pub kind: BlinkKind,
    /// Seconds until the next blink starts (while not blinking).
    pub wait: f32,
    /// Seconds since the current blink started; `None` between blinks.
    pub blinking: Option<f32>,
    rng: u64,
}

impl Blink {
    pub fn new(kind: BlinkKind, seed: u64) -> Self {
        let mut rng = seed ^ 0x6b6c_696e_6b21_u64;
        let wait = next_interval(kind, &mut rng);
        Self {
            kind,
            wait,
            blinking: None,
            rng,
        }
    }

    /// Advance by `dt`; returns true on the frame a blink starts.
    pub fn tick(&mut self, dt: f32) -> bool {
        match &mut self.blinking {
            Some(t) => {
                *t += dt;
                if *t >= BLINK_SECONDS {
                    self.blinking = None;
                    self.wait = next_interval(self.kind, &mut self.rng);
                }
                false
            }
            None => {
                self.wait -= dt;
                if self.wait <= 0.0 {
                    self.blinking = Some(0.0);
                    true
                } else {
                    false
                }
            }
        }
    }

    /// True while the player's eye meshes should be hidden (the closed part of the blink).
    pub fn eyes_closed(&self) -> bool {
        self.blinking.is_some_and(|t| {
            let mid = BLINK_SECONDS * 0.5;
            (t - mid).abs() <= PLAYER_EYES_HIDDEN_SECONDS * 0.5
        })
    }
}

/// `PLA_PLAYER_BLINK=0` disables the player's blink.
pub fn player_blink_enabled() -> bool {
    std::env::var_os("PLA_PLAYER_BLINK").is_none_or(|v| v != "0")
}

/// Mesh names (glTF node names) whose visibility is toggled for the player's blink.
pub fn is_player_eye_mesh(name: &str) -> bool {
    name.starts_with("eye_mesh_shape") || name.starts_with("eyehilight_mesh_shape")
}

pub struct BlinkPlugin;

impl Plugin for BlinkPlugin {
    fn build(&self, app: &mut App) {
        if player_blink_enabled() {
            app.add_systems(
                Update,
                (attach_player_blink, tick_blinks, apply_player_blink).chain(),
            );
        }
    }
}

fn attach_player_blink(
    mut commands: Commands,
    avatars: Query<Entity, (With<PlayerAvatar>, Without<Blink>)>,
) {
    for e in &avatars {
        commands
            .entity(e)
            .insert(Blink::new(BlinkKind::Player, 0x504c_4159));
    }
}

fn tick_blinks(time: Res<Time>, mut q: Query<&mut Blink>) {
    let dt = time.delta_secs();
    for mut b in &mut q {
        b.tick(dt);
    }
}

fn apply_player_blink(
    avatar: Query<(Entity, &Blink), With<PlayerAvatar>>,
    children: Query<&Children>,
    mut eyes: Query<(&Name, &mut Visibility)>,
) {
    let Ok((root, blink)) = avatar.single() else {
        return;
    };
    let want = if blink.eyes_closed() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for d in children.iter_descendants(root) {
        if let Ok((name, mut vis)) = eyes.get_mut(d) {
            if is_player_eye_mesh(name.as_str()) && *vis != want {
                *vis = want;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_match_the_decoded_sheet() {
        // The constants are checked against data/character_face.tsv when the repo data is there.
        let Some(path) = crate::game::world_objects::find_data_file("character_face.tsv") else {
            println!("[skip] data/character_face.tsv not found");
            return;
        };
        let text = std::fs::read_to_string(path).unwrap();
        let get = |name: &str| -> u32 {
            text.lines()
                .filter(|l| l.starts_with("character_blink_config\t"))
                .find_map(|l| {
                    let c: Vec<&str> = l.split('\t').collect();
                    (c[1] == name).then(|| c[2].parse().unwrap())
                })
                .unwrap_or_else(|| panic!("{name} missing"))
        };
        for (kind, prefix) in [
            (BlinkKind::Player, "player"),
            (BlinkKind::PlayerPinch, "player_pinch"),
            (BlinkKind::Pokemon, "pokemon"),
            (BlinkKind::Npc, "npc"),
            (BlinkKind::Ride, "ride"),
        ] {
            assert_eq!(
                interval_frames(kind),
                (
                    get(&format!("{prefix}_blink_interval_min")),
                    get(&format!("{prefix}_blink_interval_max"))
                ),
                "{prefix}"
            );
        }
    }

    #[test]
    fn drawn_intervals_stay_inside_the_range_and_vary() {
        let mut rng = 7u64;
        let (lo, hi) = interval_frames(BlinkKind::Player);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..500 {
            let s = next_interval(BlinkKind::Player, &mut rng);
            assert!(
                s >= lo as f32 * FRAME_SECONDS - 1e-4 && s <= hi as f32 * FRAME_SECONDS + 1e-4,
                "{s}"
            );
            seen.insert((s * 1000.0) as i64);
        }
        assert!(
            seen.len() > 50,
            "intervals are not constant: {}",
            seen.len()
        );
        // The pinch range is shorter than the calm one.
        let (_, pinch_hi) = interval_frames(BlinkKind::PlayerPinch);
        assert!(pinch_hi < hi);
    }

    #[test]
    fn a_blink_cycle_waits_blinks_and_reschedules() {
        let mut b = Blink::new(BlinkKind::Pokemon, 1);
        let dt = 1.0 / 60.0;
        let mut starts = 0;
        let mut blink_frames = 0;
        let mut min_gap = f32::MAX;
        let mut since_end = 0.0_f32;
        let mut was_blinking = false;
        for _ in 0..(60 * 60) {
            if b.tick(dt) {
                starts += 1;
                if starts > 1 {
                    min_gap = min_gap.min(since_end);
                }
            }
            if b.blinking.is_some() {
                blink_frames += 1;
                was_blinking = true;
            } else {
                if was_blinking {
                    since_end = 0.0;
                    was_blinking = false;
                }
                since_end += dt;
            }
        }
        // One minute at 1-5 s between blinks plus 0.35 s each: roughly 10-45 blinks.
        assert!((10..=45).contains(&starts), "{starts} blinks in a minute");
        let blink_len = blink_frames as f32 * dt / starts as f32;
        assert!((blink_len - BLINK_SECONDS).abs() < 0.03, "{blink_len}");
        assert!(
            min_gap >= 1.0 - 0.05,
            "blinks never closer than the minimum interval: {min_gap}"
        );
    }

    #[test]
    fn the_eyes_are_only_closed_in_the_middle_of_a_blink() {
        let mut b = Blink::new(BlinkKind::Player, 3);
        assert!(!b.eyes_closed());
        b.blinking = Some(0.0);
        assert!(!b.eyes_closed(), "starting to close");
        b.blinking = Some(BLINK_SECONDS * 0.5);
        assert!(b.eyes_closed());
        b.blinking = Some(BLINK_SECONDS - 0.01);
        assert!(!b.eyes_closed(), "opening again");
    }

    #[test]
    fn only_the_two_eye_meshes_are_toggled() {
        assert!(is_player_eye_mesh("eye_mesh_shape#eye"));
        assert!(is_player_eye_mesh("eyehilight_mesh_shape#eye"));
        assert!(!is_player_eye_mesh("eyelash_mesh_shape#face"));
        assert!(!is_player_eye_mesh("eyebrow_mesh_shape#eyebrow"));
        assert!(!is_player_eye_mesh("face_mesh_shape#eye"));
    }

    #[test]
    fn headless_blink_hides_and_restores_the_eye_meshes() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(BlinkPlugin);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(16),
        ));
        let eye = app
            .world_mut()
            .spawn((Name::new("eye_mesh_shape#eye"), Visibility::Inherited))
            .id();
        let brow = app
            .world_mut()
            .spawn((
                Name::new("eyebrow_mesh_shape#eyebrow"),
                Visibility::Inherited,
            ))
            .id();
        let root = app
            .world_mut()
            .spawn(PlayerAvatar)
            .add_children(&[eye, brow])
            .id();
        let mut hidden_frames = 0;
        let mut brow_hidden = false;
        for _ in 0..(60 * 20) {
            app.update();
            if *app.world().get::<Visibility>(eye).unwrap() == Visibility::Hidden {
                hidden_frames += 1;
            }
            brow_hidden |= *app.world().get::<Visibility>(brow).unwrap() == Visibility::Hidden;
        }
        assert!(app.world().get::<Blink>(root).is_some());
        assert!(hidden_frames > 0, "the eyes closed at least once in 19 s");
        assert!(!brow_hidden, "the eyebrow is never touched");
        // 18 ms steps hide for ~0.18 s: far less than the whole run.
        assert!(hidden_frames < 60 * 20 / 4);
    }
}
