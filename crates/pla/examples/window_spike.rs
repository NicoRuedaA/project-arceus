//! Window spike: draw a decoded game texture in a real Bevy window.
//!
//! Usage: cargo run -p pla --example window_spike -- <file.bntx> [out.png]
//!
//! Decodes mip 0 of the texture with the port's BNTX decoder, uploads it as a
//! Bevy `Image`, draws it centred on a 2D camera and — when an output path is
//! given — screenshots the primary window and exits, which is how the port
//! proves the render path works without a human watching.

use std::process::ExitCode;

use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::WindowResolution;

use pla::render::{bntx_image_from_bytes, spawn_image_sprite};

#[derive(Resource)]
struct Shot(Option<String>);

#[derive(Resource)]
struct Frames(u32);

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);
    let Some(path) = std::env::args().nth(1) else {
        return;
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("read {path}: {e}");
            return;
        }
    };
    let image = match bntx_image_from_bytes(&bytes, 0) {
        Ok(image) => image,
        Err(error) => {
            eprintln!("decode: {error}");
            return;
        }
    };
    println!(
        "texture {}x{} ({} bytes of RGBA)",
        image.width(),
        image.height(),
        image.data.as_ref().map_or(0, Vec::len)
    );
    spawn_image_sprite(&mut commands, &mut images, image, "bntx");
}

/// Waits a few frames so the sprite is in the render world, then captures.
fn capture_and_quit(
    mut commands: Commands,
    shot: Res<Shot>,
    mut frames: ResMut<Frames>,
    mut exit: MessageWriter<AppExit>,
) {
    frames.0 += 1;
    if frames.0 == 20 {
        if let Some(out) = &shot.0 {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(out.clone()));
        }
    }
    if frames.0 > 60 {
        exit.write(AppExit::Success);
    }
}

fn main() -> ExitCode {
    let out = std::env::args().nth(2);
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "PLA port — window spike".into(),
                resolution: WindowResolution::new(640, 480),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(Shot(out))
        .insert_resource(Frames(0))
        .add_systems(Startup, setup)
        .add_systems(Update, capture_and_quit)
        .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.3)))
        .run();
    ExitCode::SUCCESS
}
