//! Visual subsystem: sheet-driven image asset + sprite entity.

use bevy::image::Image;
use bevy::prelude::*;
use pla::render::{
    build_render_app, build_visual_app, build_visual_app_with_bntx, PortImage, PortSprite,
    SPRITE_SIZE,
};

fn authored_linear_bntx() -> Vec<u8> {
    fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    const BRTI: usize = 0x50;
    const BRTD: usize = 0xD0;
    const MIP_TABLE: usize = 0xE0;
    const PIXELS: usize = 0xF0;
    let mut bytes = vec![0; PIXELS + 8];
    bytes[0..4].copy_from_slice(b"BNTX");
    put_u32(&mut bytes, 0x08, 1);
    put_u16(&mut bytes, 0x0C, 0xFEFF);
    let declared_size = bytes.len() as u32;
    put_u32(&mut bytes, 0x1C, declared_size);
    bytes[0x20..0x24].copy_from_slice(b"NX  ");
    put_u32(&mut bytes, 0x24, 1);
    put_u64(&mut bytes, 0x28, 0x48);
    put_u64(&mut bytes, 0x30, BRTD as u64);
    put_u64(&mut bytes, 0x48, BRTI as u64);

    bytes[BRTI..BRTI + 4].copy_from_slice(b"BRTI");
    put_u16(&mut bytes, BRTI + 0x12, 1); // Linear layout.
    put_u16(&mut bytes, BRTI + 0x16, 1);
    put_u32(&mut bytes, BRTI + 0x1C, 0x0B01); // RGBA8.
    put_u32(&mut bytes, BRTI + 0x24, 2);
    put_u32(&mut bytes, BRTI + 0x28, 1);
    put_u32(&mut bytes, BRTI + 0x2C, 1);
    put_u32(&mut bytes, BRTI + 0x30, 1);
    put_u64(&mut bytes, BRTI + 0x70, MIP_TABLE as u64);
    put_u64(&mut bytes, MIP_TABLE, PIXELS as u64);

    bytes[BRTD..BRTD + 4].copy_from_slice(b"BRTD");
    bytes[PIXELS..].copy_from_slice(&[0x11, 0x22, 0x33, 0xFF, 0x44, 0x55, 0x66, 0xFF]);
    bytes
}

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

#[test]
fn bntx_visual_app_uses_decoded_pixels_and_associates_the_sprite_image() {
    let bytes = authored_linear_bntx();
    let mut app = build_visual_app_with_bntx(&bytes, 0).expect("synthetic BNTX should decode");
    app.update();

    let world = app.world_mut();
    let mut query = world.query::<(&PortSprite, &PortImage, &Sprite)>();
    let (port_sprite, port_image, sprite) = query.single(world).expect("one decoded sprite");
    assert_eq!(port_sprite.format, "bntx");
    assert_eq!(port_sprite.width, 2);
    assert_eq!(sprite.image, port_image.0);

    let images = world.resource::<Assets<Image>>();
    let image = images.get(&port_image.0).expect("sprite image asset");
    assert_eq!((image.width(), image.height()), (2, 1));
    assert_eq!(
        image.data.as_deref(),
        Some(&[0x11, 0x22, 0x33, 0xFF, 0x44, 0x55, 0x66, 0xFF][..])
    );
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
