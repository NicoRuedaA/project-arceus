//! Windowless render of the port's water material to an `Image` (no window, nothing on screen).
//!
//! It creates the real `WaterPlugin` (embedded WGSL + `MaterialPlugin`), a camera aimed at a
//! water plane with a sandy bed below it, and the decoded zone lighting of area 00 (day slot).
//! After 50 frames it screenshots the image, prints the mean colour of the water part and of the
//! sky band, and exits. Usage: `water_headless [--out PNG]`.
//!
//! This is the only check that the WGSL and the pipeline actually compile and draw: the unit tests
//! in `game::water` cover the CPU maths only. It is one process, one run, no window.

use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use pla::game::lighting::ZoneLightingPlugin;
use pla::game::water::{water_material, WaterExt, WaterPlugin};

#[derive(Resource)]
struct Out(String, Handle<Image>, u32);

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = arg(&args, "--out").unwrap_or_else(|| "work/water_headless.png".into());
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    close_when_requested: false,
                    ..default()
                })
                .disable::<WinitPlugin>(),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_millis(16)))
        .add_plugins(WaterPlugin)
        .add_plugins(ZoneLightingPlugin { area: 0, slot: 1 })
        .insert_resource(Out(out, Handle::default(), 0))
        .add_systems(Startup, setup)
        .add_systems(Update, capture)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut water_materials: ResMut<Assets<pla::game::water::WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut out: ResMut<Out>,
) {
    let target = images.add(Image::new_target_texture(1280, 720, TextureFormat::Rgba8UnormSrgb, None));
    out.1 = target.clone();

    // Water surface at y = 0 over a sand bed ~2 m below (the river depth is UNKNOWN in the game,
    // this is a deliberately visible depth for the render).
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(400.0, 400.0))),
        MeshMaterial3d(water_materials.add(water_material(WaterExt::default()))),
        Transform::IDENTITY,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(400.0, 400.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.72, 0.64, 0.45),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, -2.0, 0.0),
    ));
    // Two boxes sticking out of the water, to see the transparency and the wave shading.
    for x in [-6.0f32, 6.0] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0, 3.0, 2.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.45, 0.40, 0.33),
                perceptual_roughness: 0.9,
                ..default()
            })),
            Transform::from_xyz(x, 0.5, -8.0),
        ));
    }
    commands.spawn((
        Camera3d::default(),
        RenderTarget::Image(target.into()),
        Transform::from_xyz(0.0, 4.0, 22.0).looking_at(Vec3::new(0.0, 0.0, -6.0), Vec3::Y),
    ));
}

fn capture(mut commands: Commands, mut out: ResMut<Out>, mut exit: MessageWriter<AppExit>) {
    out.2 += 1;
    if out.2 == 50 {
        commands.spawn(Screenshot::image(out.1.clone())).observe(save_to_disk(out.0.clone()));
    }
    if out.2 == 90 {
        match stats(&out.0) {
            Ok((all, water_rows, sky_rows)) => {
                println!("[water] {path}: mean linear rgb all {all:?}", path = out.0);
                println!("[water] lower half (water) {water_rows:?}, top eighth (sky) {sky_rows:?}");
            }
            Err(e) => eprintln!("[water] cannot read back {path}: {e}", path = out.0),
        }
        exit.write(AppExit::Success);
    }
}

/// Mean linear RGB of the whole frame, of the lower half and of the top eighth.
fn stats(path: &str) -> Result<([f64; 3], [f64; 3], [f64; 3]), Box<dyn std::error::Error>> {
    use png::{BitDepth, ColorType};
    let mut dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info()?;
    if reader.info().bit_depth != BitDepth::Eight {
        return Err("only 8-bit PNGs are read back".into());
    }
    let size = reader.output_buffer_size().ok_or("image too large to buffer")?;
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf)?;
    let data = &buf[..info.buffer_size()];
    let ch = match info.color_type {
        ColorType::Rgb => 3,
        ColorType::Rgba => 4,
        other => return Err(format!("unsupported colour type {other:?}").into()),
    };
    let (w, h) = (info.width as usize, info.height as usize);
    let mut sums = ([0.0f64; 3], [0.0f64; 3], [0.0f64; 3]);
    let mut counts = (0u64, 0u64, 0u64);
    for (i, px) in data.chunks_exact(ch).enumerate() {
        let row = i / w;
        let rgb = [srgb_to_linear(px[0] as f64 / 255.0), srgb_to_linear(px[1] as f64 / 255.0), srgb_to_linear(px[2] as f64 / 255.0)];
        for k in 0..3 {
            sums.0[k] += rgb[k];
            if row >= h / 2 {
                sums.1[k] += rgb[k];
            }
            if row < h / 8 {
                sums.2[k] += rgb[k];
            }
        }
        counts.0 += 1;
        if row >= h / 2 {
            counts.1 += 1;
        }
        if row < h / 8 {
            counts.2 += 1;
        }
    }
    let mean = |s: [f64; 3], n: u64| [s[0] / n as f64, s[1] / n as f64, s[2] / n as f64];
    Ok((mean(sums.0, counts.0), mean(sums.1, counts.1.max(1)), mean(sums.2, counts.2.max(1))))
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
