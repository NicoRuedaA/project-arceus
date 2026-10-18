//! Temporary zone-name banner of the field HUD (the `L_zninfo_00` cartel).
//!
//! The `fieldinfo_00` panel used to show the zone name forever whenever no wild Pokémon was
//! within 60 m. Now the name is a welcome banner: it appears when a zone is entered (zone
//! change and start-up) and it disappears after [`ZONE_BANNER_SECS`] of **game** time (counted
//! with Bevy's clock and frozen while the pause menu is open) with the layout's own exit
//! animation (`fieldinfo_zninfo_00_close`: slides back and fades out in 3 frames at 60 fps),
//! falling back to a plain [`ZONE_BANNER_FADE_SECS`] alpha fade when the layout has none.
//! Entering another zone (another main area, an interior, ...) plays it again.
//!
//! The nearest-wild-Pokémon panel is NOT this banner: [`super::field::bind_field_hud`] keeps
//! binding it as before and only consults this module's [`ZoneBanner::shows_zone_name`] to
//! decide whether the zone name owns the panel during the welcome window.
//!
//! All the state machine is in [`ZoneBanner`], pure and unit-tested; the Bevy system
//! [`update_zone_banner`] only feeds it the clock and plays the layout animations.

use bevy::prelude::*;

use super::anim::PlayMode;
use super::{OrigScreen, OrigUiSystems, ScreenId};
use crate::game::state::{GameState, Paused};
use crate::game::ui::ActiveZoneInfo;

/// How long the zone name stays on screen after entering a zone, seconds (game time).
pub const ZONE_BANNER_SECS: f32 = 5.0;

/// Fallback fade length when the layout has no close animation, seconds.
pub const ZONE_BANNER_FADE_SECS: f32 = 0.4;

/// Part pane of the field HUD holding the zone / research panel.
pub const ZONE_BANNER_PART: &str = "L_zninfo_00";

/// Label of the animation tracks this module owns on the field screen.
const BANNER_LABEL: &str = "zone_banner";

/// Phase of the welcome banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BannerPhase {
    /// Nothing to show: the name is not part of the panel.
    #[default]
    Idle,
    /// The name is on screen; the countdown to the fade-out runs.
    Showing,
    /// Fading out (the layout's close animation or the alpha fallback); the name is still read
    /// by the binding so the fade shows the same string.
    Closing,
}

/// What the clock produced this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerEvent {
    /// Nothing changed.
    None,
    /// The welcome window ran out: play the close (or start the fallback fade).
    Expired,
    /// The fade-out finished.
    Closed,
}

/// The zone-name banner state machine plus its clock.
#[derive(Resource, Debug)]
pub struct ZoneBanner {
    /// Zone id the banner belongs to (`ha_areaNN` or a sub-area id); `None` before the first.
    zone: Option<String>,
    phase: BannerPhase,
    /// Seconds left of the current phase.
    remaining: f32,
    /// Length of the close (layout animation or fallback), seconds.
    close_secs: f32,
    /// True when the layout's own close animation drives the fade (no alpha fallback needed).
    layout_close: bool,
}

impl Default for ZoneBanner {
    fn default() -> Self {
        Self {
            zone: None,
            phase: BannerPhase::Idle,
            remaining: 0.0,
            close_secs: ZONE_BANNER_FADE_SECS,
            layout_close: false,
        }
    }
}

impl ZoneBanner {
    pub fn phase(&self) -> BannerPhase {
        self.phase
    }

    /// The zone the banner currently belongs to (tests and reports).
    pub fn zone(&self) -> Option<&str> {
        self.zone.as_deref()
    }

    /// True while the zone name is part of the panel (welcome window or its fade-out): the
    /// field binding shows the zone label (and keeps the panel on screen) in that window.
    pub fn shows_zone_name(&self) -> bool {
        !matches!(self.phase, BannerPhase::Idle)
    }

    /// Note the zone the player is in. A different zone than the last one starts the welcome
    /// window again (and returns true, so the caller plays the opening animation).
    pub fn note_zone(&mut self, zone: &str) -> bool {
        if self.zone.as_deref() == Some(zone) {
            return false;
        }
        self.zone = Some(zone.to_string());
        self.phase = BannerPhase::Showing;
        self.remaining = ZONE_BANNER_SECS;
        self.layout_close = false;
        true
    }

    /// Advance the clock by `dt` seconds of game time.
    pub fn tick(&mut self, dt: f32) -> BannerEvent {
        match self.phase {
            BannerPhase::Idle => BannerEvent::None,
            BannerPhase::Showing => {
                self.remaining -= dt.max(0.0);
                if self.remaining <= 0.0 {
                    BannerEvent::Expired
                } else {
                    BannerEvent::None
                }
            }
            BannerPhase::Closing => {
                self.remaining -= dt.max(0.0);
                if self.remaining <= 0.0 {
                    self.phase = BannerPhase::Idle;
                    BannerEvent::Closed
                } else {
                    BannerEvent::None
                }
            }
        }
    }

    /// Start the fade-out: `secs` is the length of the close that was really played and
    /// `layout_close` says whether the layout's own animation drives it.
    pub fn begin_closing(&mut self, secs: f32, layout_close: bool) {
        self.phase = BannerPhase::Closing;
        self.close_secs = secs.max(0.0);
        self.remaining = self.close_secs;
        self.layout_close = layout_close;
    }

    /// Alpha (0-255) the fallback fade wants on the panel nodes, or `None` when the layout's
    /// own close animation drives the fade.
    pub fn fallback_alpha(&self) -> Option<f32> {
        if self.phase != BannerPhase::Closing || self.layout_close {
            return None;
        }
        let t = if self.close_secs <= 0.0 {
            0.0
        } else {
            (self.remaining / self.close_secs).clamp(0.0, 1.0)
        };
        Some(t * 255.0)
    }
}

pub fn install(app: &mut App) {
    app.init_resource::<ZoneBanner>();
    app.add_systems(
        Update,
        update_zone_banner
            .before(super::field::bind_field_hud)
            .in_set(OrigUiSystems::Bind),
    );
}

/// Advance the banner: watch the active zone, count game time and drive the layout animations.
/// Runs before the field binding so the header it draws already sees this frame's phase.
pub fn update_zone_banner(
    time: Res<Time>,
    paused: Option<Res<Paused>>,
    state: Option<Res<State<GameState>>>,
    zone: Option<Res<ActiveZoneInfo>>,
    mut screen_q: Query<&mut OrigScreen>,
    mut banner: ResMut<ZoneBanner>,
) {
    // Game time only, and only in the world: the welcome does not tick while paused, in menus
    // or in battle (the pause menu freezes it and the field HUD is hidden there anyway).
    if paused.is_some_and(|p| p.0) {
        return;
    }
    if state.as_ref().is_some_and(|s| *s.get() != GameState::World) {
        return;
    }
    let Some(zone) = zone else {
        return;
    };
    let mut screen = screen_q
        .iter_mut()
        .find(|s| s.id == ScreenId::FieldHud);

    if banner.note_zone(&zone.area) {
        if let Some(screen) = screen.as_mut() {
            // The layout's own opening slide + fade; without it the panel simply appears.
            screen.play_part(ZONE_BANNER_PART, "wake", PlayMode::Hold, BANNER_LABEL);
        }
    }
    match banner.tick(time.delta_secs()) {
        BannerEvent::Expired => {
            let layout_close = screen.as_deref_mut().and_then(|screen| {
                let secs = close_seconds(screen)?;
                screen.play_part(ZONE_BANNER_PART, "close", PlayMode::Hold, BANNER_LABEL);
                Some(secs)
            });
            match layout_close {
                Some(secs) => banner.begin_closing(secs, true),
                None => banner.begin_closing(ZONE_BANNER_FADE_SECS, false),
            }
        }
        BannerEvent::Closed => {
            if let Some(screen) = screen.as_mut() {
                restore_panel(screen);
            }
        }
        BannerEvent::None => {
            // Fallback fade: while closing without a layout animation, fade the panel's nodes.
            if let (Some(alpha), Some(screen)) = (banner.fallback_alpha(), screen.as_mut()) {
                set_part_alpha(screen, alpha);
            }
        }
    }
}

/// Length in seconds of the part's `close` animation (`frames / 60`), when the layout has one.
fn close_seconds(screen: &OrigScreen) -> Option<f32> {
    let inst = screen.scene.part_instance(ZONE_BANNER_PART)?;
    let frames = screen.anim_len(inst, "close")?;
    Some(frames / super::anim::ANIM_FPS)
}

/// Drop this module's tracks (the panel goes back to its authored alpha and position, ready for
/// the nearest-wild-Pokémon binding) and make sure the fallback fade left nothing behind.
fn restore_panel(screen: &mut OrigScreen) {
    if let Some(inst) = screen.scene.part_instance(ZONE_BANNER_PART) {
        screen.stop(inst, BANNER_LABEL);
    }
    set_part_alpha(screen, 255.0);
}

/// Alpha override (0-255) on every node of the part pane (the fallback fade path).
fn set_part_alpha(screen: &mut OrigScreen, alpha: f32) {
    let Some(inst) = screen.scene.part_instance(ZONE_BANNER_PART) else {
        return;
    };
    let nodes: Vec<usize> = screen
        .scene
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.instance == inst)
        .map(|(i, _)| i)
        .collect();
    for n in nodes {
        screen.set_node(n, |o| o.alpha = Some(alpha));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_appears_lasts_five_seconds_and_fades_out() {
        let mut b = ZoneBanner::default();
        assert!(!b.shows_zone_name(), "nothing on screen before a zone");
        assert!(b.note_zone("ha_area00"), "the start-up zone opens the banner");
        assert!(b.shows_zone_name(), "the zone name is on screen");
        assert_eq!(b.phase(), BannerPhase::Showing);
        // It lasts the documented window of game time.
        assert_eq!(b.tick(ZONE_BANNER_SECS - 0.1), BannerEvent::None);
        assert!(b.shows_zone_name(), "still on screen just before the end");
        assert_eq!(b.tick(0.2), BannerEvent::Expired);
        assert!(b.shows_zone_name(), "the fade-out still shows the name");
        // The close it was told to play decides how long the fade lasts.
        b.begin_closing(0.05, true);
        assert_eq!(b.tick(0.04), BannerEvent::None);
        assert_eq!(b.tick(0.02), BannerEvent::Closed);
        assert!(!b.shows_zone_name(), "gone after the fade");
        assert_eq!(b.phase(), BannerPhase::Idle);
        assert_eq!(b.tick(10.0), BannerEvent::None, "idle ticks do nothing");
    }

    #[test]
    fn same_zone_does_not_reopen_and_a_new_zone_does() {
        let mut b = ZoneBanner::default();
        assert!(b.note_zone("ha_area00"));
        b.begin_closing(ZONE_BANNER_FADE_SECS, false);
        assert_eq!(b.tick(1.0), BannerEvent::Closed);
        assert!(!b.note_zone("ha_area00"), "same zone: no banner again");
        assert!(!b.shows_zone_name());
        // Another main area and an interior both open it again.
        assert!(b.note_zone("ha_area01"));
        assert!(b.shows_zone_name(), "entering another area reopens it");
        assert!(b.note_zone("ha_area01_s03_a"));
        assert_eq!(b.phase(), BannerPhase::Showing, "an interior too");
        assert_eq!(b.remaining, ZONE_BANNER_SECS, "the window restarts");
    }

    #[test]
    fn fallback_fade_runs_only_without_a_layout_close() {
        let mut b = ZoneBanner::default();
        b.note_zone("ha_area00");
        b.begin_closing(ZONE_BANNER_FADE_SECS, false);
        let a = b.fallback_alpha().expect("fallback fade");
        assert!((a - 255.0).abs() < 1e-3, "starts opaque: {a}");
        b.tick(ZONE_BANNER_FADE_SECS / 2.0);
        let a = b.fallback_alpha().expect("still fading");
        assert!(a > 100.0 && a < 160.0, "half way: {a}");
        b.tick(ZONE_BANNER_FADE_SECS);
        assert!(b.fallback_alpha().is_none(), "done: no fade left");
        // With the layout's own close animation there is no alpha fallback.
        b.note_zone("ha_area01");
        b.begin_closing(0.05, true);
        assert!(b.fallback_alpha().is_none());
    }
}
