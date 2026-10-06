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

use crate::assets::bntx::Bntx;
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

#[derive(Resource)]
struct VisualImage {
    image: Option<Image>,
    format: &'static str,
}

/// Headless-safe visual app: asset + image stack only.
pub fn build_visual_app() -> App {
    build_visual_app_with_image(sprite_image(), "bntx")
}

/// Build a headless visual app that displays mip 0 from a BNTX byte buffer.
pub fn build_visual_app_with_bntx(bytes: &[u8], texture_index: usize) -> Result<App, String> {
    let image = bntx_image_from_bytes(bytes, texture_index)?;
    Ok(build_visual_app_with_image(image, "bntx"))
}

/// Build an app that owns the supplied image and exposes it through the sprite path.
pub fn build_visual_app_with_image(image: Image, format: &'static str) -> App {
    let mut app = App::new();
    app.add_plugins((
        bevy::app::TaskPoolPlugin::default(),
        bevy::asset::AssetPlugin::default(),
        bevy::image::ImagePlugin::default(),
    ));
    app.insert_resource(VisualImage {
        image: Some(image),
        format,
    });
    app.add_systems(Startup, setup_supplied_image);
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

/// Build the no-input checkerboard placeholder used by the visual app.
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

/// Parse and decode one BNTX texture as an RGBA8 Bevy image.
pub fn bntx_image_from_bytes(bytes: &[u8], texture_index: usize) -> Result<Image, String> {
    let bntx = Bntx::parse(bytes)?;
    let decoded = bntx.decode(bytes, texture_index)?;
    Ok(Image::new(
        Extent3d {
            width: decoded.width,
            height: decoded.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        decoded.rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    ))
}

/// Add one image-backed sprite and the port's sheet metadata to the world.
pub fn spawn_image_sprite(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    image: Image,
    format: &'static str,
) {
    let width = image.width();
    let handle = images.add(image);
    commands.spawn((
        Sprite::from_image(handle.clone()),
        PortImage(handle),
        PortSprite { format, width },
    ));
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
    spawn_image_sprite(&mut commands, &mut images, sprite_image(), format);
}

fn setup_supplied_image(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut supplied: ResMut<VisualImage>,
) {
    spawn_image_sprite(
        &mut commands,
        &mut images,
        supplied
            .image
            .take()
            .expect("supplied image is consumed once"),
        supplied.format,
    );
}
