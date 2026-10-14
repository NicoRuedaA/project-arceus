//! Windowless Bevy render of a synthetic outdoor scene lit by the real `ZoneLightingPlugin`.
//!
//! No window is created (winit disabled, camera renders to an image), so nothing appears on the desktop.
//! Usage: `look_headless [--look classic|vivid] [--no-shadows] [--ssao] [--no-ssao] [--bloom] [--no-bloom]
//! [--ssao-quality low|medium|high|ultra] [--area 0] [--time-slot 1] [--yaw DEG] [--out PNG]`
//! `--slots` renders the four time slots (0..=3) in this single process (no window, one run) and prints
//! the mean pixel luminance of each frame, so the night dimming can be compared with figures.
//! `--clock-report` renders the day/night clock in this single process: 03:00, 09:00, 15:00, 21:00
//! plus the same scene in rain (15:00, weather 4) and snow (03:00, weather 5), with the mean pixel
//! luminance of each frame (Brief 9).
//! `--weather <2..9|name>` forces one weather in any mode (default: sunny, so renders are deterministic).
//! `--compare-post` renders the SSAO/bloom on-off matrix in this single process and prints region
//! luminance figures (mean, bottom third, four corners, share of dark pixels) per frame, so each
//! post-process effect can be quantified from one run.
//! The scene is placeholder geometry (flat colours), only meant to compare lighting/shadows/sky/fog
//! settings; it is not game content.

use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::render::render_resource::TextureFormat;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use pla::game::clock::{slot_centre_hours, slot_of_hours, GameClock};
use pla::game::lighting::{time_slot_from_args, ZoneLightingPlugin};
use pla::game::render_look::LookSettings;
use pla::game::weather::{weather_id_from_arg, WeatherPlugin};

#[derive(Resource)]
struct Out(String, Handle<Image>, u32);

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = arg(&args, "--out").unwrap_or_else(|| "work/look_headless.png".into());
    let area: u8 = arg(&args, "--area").and_then(|s| s.parse().ok()).unwrap_or(0);
    let yaw: f32 = arg(&args, "--yaw").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let settings = LookSettings::from_args(&args);
    // `--weather` forces one weather id in every mode (default: clear, so renders are deterministic).
    let forced_weather = arg(&args, "--weather").as_deref().and_then(weather_id_from_arg);
    let weather = forced_weather.unwrap_or(2);
    if args.iter().any(|a| a == "--clock-report") {
        // One process, six windowless renders: four hours of the day plus rain and snow (Brief 9).
        let stem = out.strip_suffix(".png").unwrap_or(&out).to_string();
        let cases: [(f32, u8, &str); 6] = [
            (3.0, 2, "night"),
            (9.0, 2, "morning"),
            (15.0, 2, "day"),
            (21.0, 2, "evening"),
            (15.0, 4, "day_rain"),
            (3.0, 5, "night_snow"),
        ];
        for (hours, case_weather, name) in cases {
            let path = format!("{stem}_{name}.png");
            run_one_at(area, hours, forced_weather.unwrap_or(case_weather), yaw, path.clone(), settings);
            match mean_luminance(path.as_str()) {
                Ok((srgb, lin)) => println!("[measure] {path}: mean luminance sRGB {srgb:.4}, linear {lin:.4}"),
                Err(e) => eprintln!("[measure] {path}: cannot read back ({e})"),
            }
        }
    } else if args.iter().any(|a| a == "--compare-post") {
        // One process, four windowless renders: the SSAO/bloom on-off matrix. Compare the figures of
        // each frame against `ssao_on_bloom_on` (defaults) to isolate each effect's contribution.
        let stem = out.strip_suffix(".png").unwrap_or(&out).to_string();
        let slot = time_slot_from_args(&args);
        let cases = [
            ("ssao_on_bloom_on", LookSettings { ssao: true, bloom: true, ..settings }),
            ("ssao_off_bloom_on", LookSettings { ssao: false, bloom: true, ..settings }),
            ("ssao_on_bloom_off", LookSettings { ssao: true, bloom: false, ..settings }),
            ("ssao_off_bloom_off", LookSettings { ssao: false, bloom: false, ..settings }),
        ];
        for (name, s) in cases {
            let path = format!("{stem}_{name}.png");
            run_one(area, slot, weather, yaw, path.clone(), s);
            match luma_stats(path.as_str()) {
                Ok(st) => println!(
                    "[measure] {path}: mean sRGB {:.4}, mean linear {:.4}, bottom third {:.4}, corners TL {:.4} TR {:.4} BL {:.4} BR {:.4}, dark<0.30 {:.4}%",
                    st.mean_srgb,
                    st.mean_linear,
                    st.bottom_srgb,
                    st.corner_srgb[0],
                    st.corner_srgb[1],
                    st.corner_srgb[2],
                    st.corner_srgb[3],
                    100.0 * st.dark_fraction,
                ),
                Err(e) => eprintln!("[measure] {path}: cannot read back ({e})"),
            }
        }
    } else if args.iter().any(|a| a == "--slots") {
        // One process, four windowless renders (one per time slot), then a luminance figure each.
        let stem = out.strip_suffix(".png").unwrap_or(&out).to_string();
        for slot in 0..=3usize {
            let path = format!("{stem}_s{slot}.png");
            run_one(area, slot, weather, yaw, path.clone(), settings);
            // The bottom third is ground: the night-floor check reads it (see render_look).
            match luma_stats(path.as_str()) {
                Ok(st) => println!(
                    "[measure] {path}: mean luminance sRGB {:.4}, linear {:.4}, ground (bottom third) sRGB {:.4}, dark<0.30 {:.1}%",
                    st.mean_srgb,
                    st.mean_linear,
                    st.bottom_srgb,
                    100.0 * st.dark_fraction
                ),
                Err(e) => eprintln!("[measure] {path}: cannot read back ({e})"),
            }
        }
    } else {
        let slot = time_slot_from_args(&args);
        run_one(area, slot, weather, yaw, out.clone(), settings);
    }
}

fn run_one(area: u8, slot: usize, weather: u8, yaw: f32, out: String, settings: LookSettings) {
    run_one_at(area, slot_centre_hours(slot), weather, yaw, out, settings);
}

/// One windowless render with the clock pinned at `hours` and a forced weather.
fn run_one_at(area: u8, hours: f32, weather: u8, yaw: f32, out: String, settings: LookSettings) {
    App::new()
        // ZoneLightingPlugin reads this resource at plugin-build time, so insert it first.
        .insert_resource(settings)
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
        .add_plugins(ZoneLightingPlugin { area, slot: slot_of_hours(hours) })
        .add_plugins(WeatherPlugin { area, force: Some(weather), seed: 7 })
        // Pinned clock: the scene renders exactly that time of day.
        .insert_resource(GameClock::pinned_at_hours(hours))
        .insert_resource(YawDeg(yaw))
        .insert_resource(OutPath(out))
        .add_systems(Startup, setup)
        .add_systems(Update, capture)
        .run();
}

/// Per-region luminance figures of a PNG, on the stored sRGB values.
#[derive(Debug, Clone, Copy)]
struct LumaStats {
    /// Mean Rec.709 luminance over the whole frame (sRGB and linearised).
    mean_srgb: f64,
    mean_linear: f64,
    /// Mean over the lower third of the frame (ground and object bases: contact shadows land here).
    bottom_srgb: f64,
    /// Mean over same-size patches at the four corners: TL, TR, BL, BR.
    corner_srgb: [f64; 4],
    /// Share of pixels whose sRGB luminance is below 0.30 (occluded creases get darker).
    dark_fraction: f64,
}

/// Mean Rec.709 luminance of a PNG, on the stored sRGB values (perceptual) and on their linearisation.
fn mean_luminance(path: &str) -> Result<(f64, f64), Box<dyn std::error::Error>> {
    let st = luma_stats(path)?;
    Ok((st.mean_srgb, st.mean_linear))
}

/// Region luminance statistics of a PNG (see [`LumaStats`]).
fn luma_stats(path: &str) -> Result<LumaStats, Box<dyn std::error::Error>> {
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
        ColorType::Grayscale => 1,
        ColorType::GrayscaleAlpha => 2,
        other => return Err(format!("unsupported colour type {other:?}").into()),
    };
    let (w, h) = (info.width as usize, info.height as usize);
    if w == 0 || h == 0 {
        return Err("empty image".into());
    }
    let patch = 160usize.min(w / 4).min(h / 4).max(1);
    let (mut sum_srgb, mut sum_lin, mut n) = (0.0f64, 0.0f64, 0u64);
    let (mut bottom_sum, mut bottom_n) = (0.0f64, 0u64);
    let (mut corner_sum, mut corner_n) = ([0.0f64; 4], [0u64; 4]);
    let mut dark = 0u64;
    for (i, px) in data.chunks_exact(ch).enumerate() {
        let (r, g, b) = match ch {
            1 => (px[0], px[0], px[0]),
            _ => (px[0], px[1], px[2]),
        };
        let (rf, gf, bf) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
        let lum = 0.2126 * rf + 0.7152 * gf + 0.0722 * bf;
        sum_srgb += lum;
        let (rl, gl, bl) = (srgb_to_linear(rf), srgb_to_linear(gf), srgb_to_linear(bf));
        sum_lin += 0.2126 * rl + 0.7152 * gl + 0.0722 * bl;
        let (x, y) = (i % w, i / w);
        if y * 3 >= h * 2 {
            bottom_sum += lum;
            bottom_n += 1;
        }
        if x < patch && y < patch {
            corner_sum[0] += lum;
            corner_n[0] += 1;
        }
        if x >= w - patch && y < patch {
            corner_sum[1] += lum;
            corner_n[1] += 1;
        }
        if x < patch && y >= h - patch {
            corner_sum[2] += lum;
            corner_n[2] += 1;
        }
        if x >= w - patch && y >= h - patch {
            corner_sum[3] += lum;
            corner_n[3] += 1;
        }
        if lum < 0.30 {
            dark += 1;
        }
        n += 1;
    }
    if n == 0 || bottom_n == 0 || corner_n.contains(&0) {
        return Err("empty image".into());
    }
    let mean = |s: f64, c: u64| s / c as f64;
    Ok(LumaStats {
        mean_srgb: mean(sum_srgb, n),
        mean_linear: mean(sum_lin, n),
        bottom_srgb: mean(bottom_sum, bottom_n),
        corner_srgb: [
            mean(corner_sum[0], corner_n[0]),
            mean(corner_sum[1], corner_n[1]),
            mean(corner_sum[2], corner_n[2]),
            mean(corner_sum[3], corner_n[3]),
        ],
        dark_fraction: dark as f64 / n as f64,
    })
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[derive(Resource)]
struct YawDeg(f32);
#[derive(Resource)]
struct OutPath(String);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    yaw: Res<YawDeg>,
    out: Res<OutPath>,
) {
    let target = images.add(Image::new_target_texture(1280, 720, TextureFormat::Rgba8UnormSrgb, None));
    commands.insert_resource(Out(out.0.clone(), target.clone(), 0));
    let pm = |c: Color| StandardMaterial { base_color: c, perceptual_roughness: 0.9, ..default() };
    let grass = mats.add(pm(Color::srgb(0.55, 0.68, 0.18)));
    let rock = mats.add(pm(Color::srgb(0.45, 0.40, 0.33)));
    let leaf = mats.add(pm(Color::srgb(0.12, 0.34, 0.12)));
    let bark = mats.add(pm(Color::srgb(0.75, 0.75, 0.70)));
    let cloth = mats.add(pm(Color::srgb(0.1, 0.15, 0.4)));
    commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(2000.0, 2000.0))), MeshMaterial3d(grass.clone())));
    // far hills
    for (i, (x, z, r)) in [(-200.0, -380.0, 120.0), (60.0, -450.0, 160.0), (300.0, -330.0, 100.0)].iter().enumerate() {
        let m = if i % 2 == 0 { grass.clone() } else { rock.clone() };
        commands.spawn((Mesh3d(meshes.add(Sphere::new(*r))), MeshMaterial3d(m), Transform::from_xyz(*x, -r * 0.55, *z)));
    }
    // trees on a deterministic scatter
    let (trunk, crown) = (meshes.add(Cylinder::new(0.25, 7.0)), meshes.add(Sphere::new(2.8)));
    for i in 0..70u32 {
        let a = (i as f32 * 2.399) % std::f32::consts::TAU;
        let d = 8.0 + (i as f32 * 7.13) % 140.0;
        let (x, z) = (a.cos() * d * 0.8, -a.sin().abs() * d - 6.0);
        commands.spawn((Mesh3d(trunk.clone()), MeshMaterial3d(bark.clone()), Transform::from_xyz(x, 3.5, z)));
        commands.spawn((Mesh3d(crown.clone()), MeshMaterial3d(leaf.clone()), Transform::from_xyz(x, 8.0, z).with_scale(Vec3::new(1.0, 1.3, 1.0))));
    }
    // player stand-in
    commands.spawn((Mesh3d(meshes.add(Capsule3d::new(0.35, 1.1))), MeshMaterial3d(cloth), Transform::from_xyz(0.0, 0.9, -3.0)));
    commands.spawn((Mesh3d(meshes.add(Cuboid::new(2.0, 1.4, 1.6))), MeshMaterial3d(rock), Transform::from_xyz(3.0, 0.7, -5.0)));
    let rot = Quat::from_rotation_y(yaw.0.to_radians());
    commands.spawn((
        Camera3d::default(),
        RenderTarget::Image(target.into()),
        Transform::from_xyz(0.0, 1.9, 3.0).with_rotation(rot * Quat::from_rotation_x(-0.04)),
        // placeholder viewer light that the plugin replaces (as in the real viewer)
        // (kept absent here; the plugin spawns the sun)
    ));
}

fn capture(mut commands: Commands, mut out: ResMut<Out>, mut exit: MessageWriter<AppExit>) {
    out.2 += 1;
    if out.2 == 40 {
        commands.spawn(Screenshot::image(out.1.clone())).observe(save_to_disk(out.0.clone()));
    }
    if out.2 == 80 {
        exit.write(AppExit::Success);
    }
}
