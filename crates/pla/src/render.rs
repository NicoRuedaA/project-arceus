//! First visual subsystem: sheet-driven sprite from a game texture.
//!
//! What works headless: the asset + image stack builds an `Image` from the
//! container metadata the sheet book knows, and entities carry the sheet-driven
//! tag. What does NOT work headless in Bevy 0.19: the render-app initialization
//! path — `PipelinedRenderingPlugin::cleanup` skips setup when headless
//! (`pipelined_rendering.rs:126`) and the non-pipelined path polls
//! `DeviceErrorHandler` before it is inserted (`error_handler.rs:186`), so the
//! full `RenderPlugin` needs a window/surface. `build_render_app()` is kept for
//! the port's windowed build and its test is `#[ignore]`d here.

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::generated;

/// The sprite's image handle (wrapped: `Handle<Image>` is not a Bundle member
/// in this feature configuration).
#[derive(Component, Debug, Clone)]
pub struct PortImage(pub Handle<Image>);

/// Marks an entity the port spawned from sheet data.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortSprite {
    /// Texture container the sheet book knows for this sprite.
    pub format: &'static str,
    /// Sprite width in pixels.
    pub width: u32,
}

/// Sheet-driven sprite size for the first visual unit.
pub const SPRITE_SIZE: u32 = 32;

/// Headless-safe visual app: asset + image stack only.
pub fn build_visual_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        bevy::app::TaskPoolPlugin::default(),
        bevy::asset::AssetPlugin::default(),
        bevy::image::ImagePlugin::default(),
    ));
    app.add_systems(Startup, setup_scene);
    app
}

/// Full render app (needs a window/surface; see the module docs).
pub fn build_render_app() -> App {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(bevy::window::WindowPlugin {
        primary_window: None,
        exit_condition: bevy::window::ExitCondition::DontExit,
        close_when_requested: false,
        ..Default::default()
    }));
    app.add_systems(Startup, setup_scene);
    app
}

/// Build the sprite image from container metadata (checkerboard placeholder
/// until the BNTX pixel decoder lands).
pub fn sprite_image() -> Image {
    let size = Extent3d {
        width: SPRITE_SIZE,
        height: SPRITE_SIZE,
        depth_or_array_layers: 1,
    };
    let mut data = vec![0u8; (SPRITE_SIZE * SPRITE_SIZE * 4) as usize];
    for y in 0..SPRITE_SIZE {
        for x in 0..SPRITE_SIZE {
            let v = if (x / 4 + y / 4) % 2 == 0 { 0x40 } else { 0xC0 };
            let i = ((y * SPRITE_SIZE + x) * 4) as usize;
            data[i..i + 3].copy_from_slice(&[v, v, v]);
            data[i + 3] = 0xFF;
        }
    }
    Image::new(
        size,
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    )
}

fn setup_scene(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // The sheet book is the source of truth: which texture container do we know?
    let format = if generated::domain_asset_formats::REGISTRY
        .get("bntx")
        .is_some()
    {
        "bntx"
    } else {
        "unknown"
    };
    let handle = images.add(sprite_image());
    commands.spawn((
        PortImage(handle),
        PortSprite {
            format,
            width: SPRITE_SIZE,
        },
    ));
}
