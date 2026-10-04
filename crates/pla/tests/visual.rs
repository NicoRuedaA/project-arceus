//! Visual subsystem: sheet-driven image asset + sprite entity.

use bevy::image::Image;
use bevy::prelude::*;
use pla::render::{build_render_app, build_visual_app, PortSprite, SPRITE_SIZE};

#[test]
fn visual_app_builds_a_sheet_driven_image() {
    let mut app = build_visual_app();
    app.update();

    // Our sheet-driven image asset exists (ImagePlugin adds its own defaults,
    // so select by size instead of counting all assets).
    let images = app.world().resource::<Assets<Image>>();
    let ours: Vec<&Image> = images
        .iter()
        .map(|(_, img)| img)
        .filter(|img| img.width() == SPRITE_SIZE && img.height() == SPRITE_SIZE)
        .collect();
    assert_eq!(
        ours.len(),
        1,
        "expected exactly one {SPRITE_SIZE}x{SPRITE_SIZE} image"
    );
    assert_eq!(
        ours[0].data.as_ref().map(|d| d.len()),
        Some((SPRITE_SIZE * SPRITE_SIZE * 4) as usize)
    );

    // The entity carries the container tag the sheet book knows.
    let world = app.world_mut();
    let mut query = world.query::<&PortSprite>();
    let sprites: Vec<PortSprite> = query.iter(world).copied().collect();
    assert_eq!(
        sprites.len(),
        1,
        "expected one port sprite, got {sprites:?}"
    );
    assert_eq!(sprites[0].format, "bntx");
    assert_eq!(sprites[0].width, SPRITE_SIZE);
}

/// The full render path needs a window/surface (Bevy 0.19 headless skips the
/// render-app setup); keep the test for the windowed build.
#[test]
#[ignore = "requires a window/surface: Bevy 0.19 does not initialise the render app headless"]
fn render_app_initializes_with_a_window() {
    let mut app = build_render_app();
    for _ in 0..3 {
        app.update();
    }
    assert!(app
        .world()
        .get_resource::<bevy::render::renderer::RenderDevice>()
        .is_some());
}
