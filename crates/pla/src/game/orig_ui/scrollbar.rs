//! Functional scrollbars for the game's own scrollbar parts (`common_scrollbar_00`,
//! `inventory_scrollbar_00`, ...).
//!
//! The game drives its scrollbars from run-time list data; the decoded layouts only hold the
//! resting state (track `W_area_00` + knob `W_box_00` centered on the track), which is why the
//! bar used to be drawn but never moved (brief ui-menu). This module:
//!
//! - [`set_bar`] places the game's own knob for a list window (`visible` of `total` rows at
//!   `scroll`), hiding the part while the whole list fits;
//! - [`ScrollBars`] publishes the shown bars (track rectangle in layout pixels) so the mouse can
//!   drive them: wheel (`scroll_wheel`), a drag of the knob ([`scroll_for_offset`], with the
//!   pointer's grab offset) and a click on the track (one page towards the click,
//!   [`scroll_page_click`]);
//! - [`scrollbar_input`] writes the new scroll window back into the screen's own state
//!   (`GraphicsScreenState`, `DexUi`, `BagUi`, `BoxScreenState`) so the keyboard selection and
//!   the bar stay in sync.
//!
//! PROBABLE: the exact game mapping (knob travel, page step) is not in the decode; the port's
//! choice is proportional (knob length = visible / total, position = scroll / (total - visible))
//! and one page per track click.

use std::collections::HashMap;

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::game::bag_screen::BagUi;
use super::menus::BAG_PAGE;
use super::{OrigScreen, ScreenId};
use crate::game::box_screen::BoxScreenState;
use crate::game::dex_screen::DexUi;
use crate::game::graphics_screen::GraphicsScreenState;
use crate::game::pause_menu::PauseMenu;

/// Vertical track geometry in layout pixels (1920x1080 space, origin top-left, y down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Track {
    /// Centre of the track.
    pub center: Vec2,
    /// Length of the track along its long side (the screen's y for every decoded bar).
    pub len: f32,
    /// Thickness of the track.
    pub thickness: f32,
}

impl Track {
    /// Whether layout point `p` is on the track (with a small grab margin on both sides).
    pub fn contains(&self, p: Vec2) -> bool {
        (p.x - self.center.x).abs() <= self.thickness / 2.0 + 6.0
            && (p.y - self.center.y).abs() <= self.len / 2.0 + 6.0
    }

    /// Offset of layout point `p` from the track centre, along the bar (positive = down).
    pub fn offset(&self, p: Vec2) -> f32 {
        p.y - self.center.y
    }
}

/// Knob length and centre offset from the track centre (positive = down) for a window of
/// `visible` out of `total` rows starting at `scroll`. At least 12 % of the track so the knob
/// stays grabbable on very long lists.
pub fn knob_geometry(track_len: f32, visible: usize, total: usize, scroll: usize) -> (f32, f32) {
    let visible = visible.max(1);
    let total = total.max(1);
    let scroll = scroll.min(total.saturating_sub(visible));
    let len = (track_len * visible as f32 / total as f32).clamp(track_len * 0.12, track_len.max(1.0));
    let span = (track_len - len).max(0.0);
    let max_scroll = total.saturating_sub(visible);
    let t = if max_scroll == 0 {
        0.0
    } else {
        scroll as f32 / max_scroll as f32
    };
    (len, -track_len / 2.0 + len / 2.0 + t * span)
}

/// Scroll window for a knob whose centre sits `offset` from the track centre (a drag).
pub fn scroll_for_offset(
    track_len: f32,
    knob_len: f32,
    visible: usize,
    total: usize,
    offset: f32,
) -> usize {
    let visible = visible.max(1);
    let max_scroll = total.saturating_sub(visible);
    if max_scroll == 0 {
        return 0;
    }
    let span = (track_len - knob_len).max(1.0);
    let t = ((offset + track_len / 2.0 - knob_len / 2.0) / span).clamp(0.0, 1.0);
    ((t * max_scroll as f32).round() as usize).min(max_scroll)
}

/// Scroll window after a wheel of `notches` notches (positive = down): one row per notch.
pub fn scroll_wheel(scroll: usize, notches: f32, visible: usize, total: usize) -> usize {
    let max_scroll = total.saturating_sub(visible.max(1));
    let step = notches.signum() * notches.abs().min(5.0).ceil();
    let next = scroll as i64 + step as i64;
    next.clamp(0, max_scroll as i64) as usize
}

/// Scroll window after a click on the track: one page towards the click side (`click_offset` and
/// `knob_offset` are measured from the track centre).
pub fn scroll_page_click(
    scroll: usize,
    click_offset: f32,
    knob_offset: f32,
    visible: usize,
    total: usize,
) -> usize {
    let visible = visible.max(1);
    let max_scroll = total.saturating_sub(visible);
    let page = visible as i64;
    let next = if click_offset >= knob_offset {
        scroll as i64 + page
    } else {
        scroll as i64 - page
    };
    next.clamp(0, max_scroll as i64) as usize
}

/// One bar a binding drew: enough for the mouse to drive its list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    pub screen: ScreenId,
    pub track: Track,
    pub visible: usize,
    pub total: usize,
    pub scroll: usize,
}

/// The bars of the shown screens, republished by their bindings every frame.
#[derive(Resource, Debug, Default)]
pub struct ScrollBars {
    pub bars: HashMap<ScreenId, Bar>,
}

impl ScrollBars {
    /// Publish the bar of `screen` (`None` clears a bar that is no longer drawn: the list fits
    /// or the screen stopped showing one).
    pub fn publish(&mut self, screen: ScreenId, bar: Option<Bar>) {
        match bar {
            Some(bar) => {
                self.bars.insert(screen, bar);
            }
            None => {
                self.bars.remove(&screen);
            }
        }
    }

    pub fn get(&self, screen: ScreenId) -> Option<&Bar> {
        self.bars.get(&screen)
    }
}

/// Pointer override for runs without a window (automation mode): the position is in window
/// pixels, exactly as the real cursor would be, and takes precedence while `position` is `Some`.
/// The size is the virtual viewport the position refers to (the offscreen screenshot size).
/// `pressed` / `just_pressed` / `just_released` simulate the left button there (the real
/// `ButtonInput<MouseButton>` never changes with no primary window); the edges are consumed by
/// [`scrollbar_input`] on the frame they are read.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct VirtualPointer {
    pub position: Option<Vec2>,
    pub size: Vec2,
    pub pressed: bool,
    pub just_pressed: bool,
    pub just_released: bool,
}

/// Active drag of a knob (kept between frames).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct ScrollDrag {
    pub screen: Option<ScreenId>,
    /// Pointer offset from the knob centre when the drag started (layout pixels).
    pub grab: f32,
}

/// Place the knob of the scrollbar part `part` for a window of `visible` out of `total` rows at
/// `scroll`, hiding the part while everything fits. Returns the bar the mouse can drive (once
/// the screen has been evaluated at least once).
pub fn set_bar(
    s: &mut OrigScreen,
    part: &str,
    visible: usize,
    total: usize,
    scroll: usize,
) -> Option<Bar> {
    let inst = s.scene.part_instance(part)?;
    let track_node = s.scene.find_in(inst, "W_area_00")?;
    let knob_node = s.scene.find_in(inst, "W_box_00")?;
    let overflow = total > visible.max(1) && total > 1;
    s.set(part, |o| o.visible = Some(overflow));
    if !overflow {
        return None;
    }
    let frame = s.last.as_ref()?;
    let g = *frame.geoms.get(track_node)?;
    if !g.visible {
        return None;
    }
    let (center, len, thickness) = if g.size.x >= g.size.y {
        (g.center, g.size.x, g.size.y)
    } else {
        (g.center, g.size.y, g.size.x)
    };
    let (knob_len, offset) = knob_geometry(len, visible, total, scroll);
    let knob_h = s.scene.nodes[knob_node].pane.size[1].abs().max(1.0);
    s.set_node(knob_node, |o| {
        o.size = Some([knob_len, knob_h]);
        o.translate = Some([offset, 0.0]);
    });
    Some(Bar {
        screen: s.id,
        track: Track {
            center,
            len,
            thickness,
        },
        visible,
        total,
        scroll,
    })
}

/// Pointer position in layout pixels (the 1920x1080 space every screen here uses): the virtual
/// pointer of the automation mode first, else the real window's cursor.
fn pointer_layout(
    windows: &Query<&Window, With<PrimaryWindow>>,
    virtual_position: Option<Vec2>,
    virtual_size: Vec2,
) -> Option<Vec2> {
    const LAYOUT: Vec2 = Vec2::new(1920.0, 1080.0);
    if let Some(p) = virtual_position {
        let size = if virtual_size.x > 1.0 && virtual_size.y > 1.0 {
            virtual_size
        } else {
            LAYOUT
        };
        return Some(p / size * LAYOUT);
    }
    let w = windows.iter().next()?;
    let p = w.cursor_position()?;
    Some(p / Vec2::new(w.width().max(1.0), w.height().max(1.0)) * LAYOUT)
}

/// Mouse input for every published bar: wheel over the list, drag of the knob, click on the
/// track (one page). Writes the new window back into the owning screen's state, keeping the
/// keyboard selection inside the window.
#[allow(clippy::too_many_arguments)]
pub fn scrollbar_input(
    config: Res<super::OrigUiConfig>,
    bars: Res<ScrollBars>,
    screens: Query<&OrigScreen>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut virtual_pointer: Option<ResMut<VirtualPointer>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut wheel: Option<MessageReader<MouseWheel>>,
    mut drag: ResMut<ScrollDrag>,
    mut options: Option<ResMut<GraphicsScreenState>>,
    mut dex: Option<ResMut<DexUi>>,
    mut bag: Option<ResMut<BagUi>>,
    mut boxes: Option<ResMut<BoxScreenState>>,
    mut pause: Option<ResMut<PauseMenu>>,
) {
    if !config.enabled() {
        return;
    }
    let notches: f32 = wheel
        .as_mut()
        .map(|r| r.read().map(|e| e.y).sum())
        .unwrap_or(0.0);
    // The simulated pointer (automation) owns the mouse when it has a position; its button
    // edges are consumed here once.
    let simulated = virtual_pointer
        .as_deref()
        .filter(|v| v.position.is_some())
        .map(|v| (v.position, v.pressed, v.just_pressed, v.just_released));
    if let Some(vp) = virtual_pointer.as_deref_mut() {
        vp.just_pressed = false;
        vp.just_released = false;
    }
    let (held, pressed_now) = match simulated {
        Some((_, held, pressed, _)) => (held, pressed),
        None => (
            mouse_buttons.pressed(MouseButton::Left),
            mouse_buttons.just_pressed(MouseButton::Left),
        ),
    };
    let Some(pointer) = pointer_layout(
        &windows,
        simulated.and_then(|(p, ..)| p),
        virtual_pointer.as_deref().map_or(Vec2::ZERO, |v| v.size),
    ) else {
        drag.screen = None;
        return;
    };
    let shown: Vec<Bar> = bars
        .bars
        .values()
        .filter(|b| screens.iter().any(|s| s.id == b.screen && s.shown))
        .copied()
        .collect();
    if shown.is_empty() {
        drag.screen = None;
        return;
    }

    // A drag in progress keeps the knob under the pointer.
    if let Some(screen) = drag.screen {
        if held {
            if let Some(bar) = shown.iter().find(|b| b.screen == screen) {
                let (knob_len, _) =
                    knob_geometry(bar.track.len, bar.visible, bar.total, bar.scroll);
                let target = bar.track.offset(pointer) - drag.grab;
                let scroll =
                    scroll_for_offset(bar.track.len, knob_len, bar.visible, bar.total, target);
                write_scroll(screen, scroll, bar, &mut options, &mut dex, &mut bag, &mut boxes, &mut pause);
                return;
            }
        }
        drag.screen = None;
    }

    // A press on the knob grabs it; a press on the track jumps one page.
    if pressed_now {
        if let Some(bar) = shown.iter().rev().find(|b| b.track.contains(pointer)) {
            let (knob_len, knob_offset) =
                knob_geometry(bar.track.len, bar.visible, bar.total, bar.scroll);
            let offset = bar.track.offset(pointer);
            if (offset - knob_offset).abs() <= knob_len / 2.0 + 6.0 {
                drag.screen = Some(bar.screen);
                drag.grab = offset - knob_offset;
            } else {
                let scroll = scroll_page_click(
                    bar.scroll,
                    offset,
                    knob_offset,
                    bar.visible,
                    bar.total,
                );
                write_scroll(bar.screen, scroll, bar, &mut options, &mut dex, &mut bag, &mut boxes, &mut pause);
            }
        }
    }

    // The wheel: on the bar under the pointer, or on the only shown bar.
    if notches != 0.0 {
        let target = shown
            .iter()
            .rev()
            .find(|b| b.track.contains(pointer))
            .or_else(|| (shown.len() == 1).then(|| &shown[0]));
        if let Some(bar) = target {
            let scroll = scroll_wheel(bar.scroll, notches, bar.visible, bar.total);
            write_scroll(bar.screen, scroll, bar, &mut options, &mut dex, &mut bag, &mut boxes, &mut pause);
        }
    }
}

/// Write a new scroll window into the state of `screen`, keeping its selection inside it.
fn write_scroll(
    screen: ScreenId,
    scroll: usize,
    bar: &Bar,
    options: &mut Option<ResMut<GraphicsScreenState>>,
    dex: &mut Option<ResMut<DexUi>>,
    bag: &mut Option<ResMut<BagUi>>,
    boxes: &mut Option<ResMut<BoxScreenState>>,
    pause: &mut Option<ResMut<PauseMenu>>,
) {
    match screen {
        ScreenId::PauseMenu => {
            if let Some(menu) = pause.as_deref_mut() {
                let visible = bar.visible.max(1);
                menu.scroll = scroll;
                if menu.selected < scroll {
                    menu.selected = scroll;
                } else if menu.selected >= scroll + visible {
                    menu.selected = scroll + visible - 1;
                }
            }
        }
        ScreenId::Options => {
            if let Some(st) = options.as_deref_mut() {
                let visible = bar.visible.max(1);
                st.scroll = scroll;
                if st.selected < scroll {
                    st.selected = scroll;
                } else if st.selected >= scroll + visible {
                    st.selected = scroll + visible - 1;
                }
            }
        }
        ScreenId::Dex => {
            if let Some(ui) = dex.as_deref_mut() {
                // The bar carries the window start; the selection follows at its centre.
                let sel = scroll + bar.visible / 2;
                ui.selected = sel.min(bar.total.saturating_sub(1));
            }
        }
        ScreenId::Bag => {
            if let Some(ui) = bag.as_deref_mut() {
                // Pages: the first item of the page becomes the selection.
                let page = scroll.min(bar.total.saturating_sub(1));
                ui.selected = page * BAG_PAGE;
            }
        }
        ScreenId::Box => {
            if let Some(st) = boxes.as_deref_mut() {
                st.page = scroll.min(bar.total.saturating_sub(1));
                st.selected_slot = 0;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knob_grows_with_the_visible_share_and_travels_the_track() {
        // 9 of 11 rows: most of the track, at the top for scroll 0 and at the bottom for 2.
        let (len, top) = knob_geometry(600.0, 9, 11, 0);
        assert!((len - 600.0 * 9.0 / 11.0).abs() < 1e-3, "{len}");
        assert!((top + 600.0 / 2.0 - len / 2.0).abs() < 1e-3, "top: {top}");
        let (len2, bottom) = knob_geometry(600.0, 9, 11, 2);
        assert_eq!(len2, len);
        assert!((bottom - (600.0 / 2.0 - len / 2.0)).abs() < 1e-3, "bottom: {bottom}");
        let (_, mid) = knob_geometry(600.0, 9, 11, 1);
        assert!((mid - 0.0).abs() < 1e-3, "middle: {mid}");
        // Everything fits: the knob fills the track and stays on top.
        let (full, top0) = knob_geometry(600.0, 11, 11, 0);
        assert!((full - 600.0).abs() < 1e-3);
        assert!(top0 <= 0.0);
        // Very long lists keep a grabbable minimum.
        let (tiny, _) = knob_geometry(600.0, 1, 1000, 500);
        assert!((tiny - 72.0).abs() < 1e-3, "{tiny}");
    }

    #[test]
    fn drag_offset_round_trips_through_the_geometry() {
        for scroll in [0, 1, 2] {
            let (len, offset) = knob_geometry(686.0, 9, 11, scroll);
            let back = scroll_for_offset(686.0, len, 9, 11, offset);
            assert_eq!(back, scroll, "scroll {scroll} made {back}");
        }
        // Dragging past both ends clamps.
        let (len, _) = knob_geometry(686.0, 7, 242, 10);
        assert_eq!(scroll_for_offset(686.0, len, 7, 242, -10_000.0), 0);
        assert_eq!(scroll_for_offset(686.0, len, 7, 242, 10_000.0), 242 - 7);
    }

    #[test]
    fn wheel_steps_one_row_per_notch_and_clamps() {
        assert_eq!(scroll_wheel(0, -1.0, 9, 11), 0);
        assert_eq!(scroll_wheel(0, 1.0, 9, 11), 1);
        assert_eq!(scroll_wheel(1, -1.0, 9, 11), 0);
        assert_eq!(scroll_wheel(2, 3.0, 9, 11), 2, "already at the end");
        assert_eq!(scroll_wheel(0, 100.0, 9, 200), 5, "one command is bounded");
        assert_eq!(scroll_wheel(4, -1.0, 9, 11), 2);
    }

    #[test]
    fn track_click_jumps_one_page_towards_the_click() {
        // Click below the knob: one page down, clamped at the end.
        assert_eq!(scroll_page_click(0, 200.0, -200.0, 9, 30), 9);
        assert_eq!(scroll_page_click(0, 200.0, -200.0, 9, 11), 2);
        // Click above the knob: one page up.
        assert_eq!(scroll_page_click(9, -200.0, 200.0, 9, 30), 0);
        assert_eq!(scroll_page_click(2, -10.0, 200.0, 9, 11), 0);
        // A click exactly at the knob centre does not move (the knob arm handles those before).
        assert_eq!(scroll_page_click(9, 300.0, 300.0, 9, 30), 18, "towards the click");
    }

    #[test]
    fn track_hit_test_uses_the_track_rectangle() {
        let t = Track {
            center: Vec2::new(910.0, 500.0),
            len: 600.0,
            thickness: 16.0,
        };
        assert!(t.contains(Vec2::new(910.0, 500.0)));
        assert!(t.contains(Vec2::new(910.0 + 14.0, 500.0 + 300.0)));
        assert!(!t.contains(Vec2::new(910.0 + 30.0, 500.0)));
        assert!(!t.contains(Vec2::new(910.0, 500.0 + 400.0)));
        assert!((t.offset(Vec2::new(910.0, 600.0)) - 100.0).abs() < 1e-4);
    }

    /// The pointer drives the Options window end to end: press on the knob, drag it down, and
    /// the state's window (and selection) follow.
    #[test]
    fn input_drags_the_knob_into_the_screen_state() {
        use super::super::scene::Scene;
        use super::super::{OrigScreen, ScreenId};
        use crate::game::graphics_screen::GraphicsScreenState;
        use crate::game::orig_ui::layout::ScreenDoc;
        use std::path::PathBuf;

        let json = r#"{"screen":"t","archives":[{"name":"a","textures":{},
          "animations":{},"layouts":{"main":{"layout":{"width":1920,"height":1080,"name":"main"},
          "materials":[],"root":{"name":"RootPane","type":"null",
          "origin":{"x":"center","y":"center"},"parent_origin":{"x":"center","y":"center"},
          "translate":[0,0,0],"rotate":[0,0,0],"scale":[1,1],"size":[1920,1080],"children":[]}}}}]}"#;
        let doc = std::sync::Arc::new(ScreenDoc::from_json(json).unwrap());
        let mut screen = OrigScreen::new(
            ScreenId::Options,
            std::sync::Arc::new(Scene::build(doc, "main").unwrap()),
        );
        screen.shown = true;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<ButtonInput<MouseButton>>();
        app.init_resource::<ScrollBars>();
        app.init_resource::<ScrollDrag>();
        app.insert_resource(super::super::OrigUiConfig {
            mode: super::super::UiMode::Original,
            root: Some(PathBuf::from("/nonexistent")),
        });
        // 1280x720 viewport: layout point (910, 290) is window point (606.7, 193.3).
        // With 9 of 30 rows visible the knob sits at the top of the track (offset -210).
        app.insert_resource(VirtualPointer {
            position: Some(Vec2::new(606.7, 193.3)),
            size: Vec2::new(1280.0, 720.0),
            ..Default::default()
        });
        app.insert_resource(GraphicsScreenState {
            open: true,
            selected: 0,
            page: 0,
            scroll: 0,
        });
        app.world_mut().spawn(screen);
        app.world_mut().resource_mut::<ScrollBars>().publish(
            ScreenId::Options,
            Some(Bar {
                screen: ScreenId::Options,
                track: Track {
                    center: Vec2::new(910.0, 500.0),
                    len: 600.0,
                    thickness: 16.0,
                },
                visible: 9,
                total: 30,
                scroll: 0,
            }),
        );
        app.add_systems(Update, scrollbar_input);

        // Press on the knob at the top and drag it to the bottom: scroll follows to the end.
        let press = |app: &mut App, y: f32| {
            {
                let mut vp = app.world_mut().resource_mut::<VirtualPointer>();
                vp.position = Some(Vec2::new(606.7, y));
                vp.pressed = true;
                vp.just_pressed = true;
            }
            app.update();
        };
        press(&mut app, 193.3);
        assert_eq!(app.world().resource::<ScrollDrag>().screen, Some(ScreenId::Options));
        {
            let mut vp = app.world_mut().resource_mut::<VirtualPointer>();
            vp.just_pressed = false;
            vp.position = Some(Vec2::new(606.7, 720.0 - 10.0));
        }
        app.update();
        let st = app.world().resource::<GraphicsScreenState>();
        assert_eq!(st.scroll, 30 - 9, "dragged to the end: {}", st.scroll);
        assert_eq!(st.selected, 30 - 9, "the selection follows the window");
        // Release: a later move no longer scrolls.
        {
            let mut vp = app.world_mut().resource_mut::<VirtualPointer>();
            vp.pressed = false;
            vp.just_released = true;
            vp.position = Some(Vec2::new(606.7, 10.0));
        }
        app.update();
        assert_eq!(app.world().resource::<ScrollDrag>().screen, None);
        assert_eq!(app.world().resource::<GraphicsScreenState>().scroll, 30 - 9);
    }
}
