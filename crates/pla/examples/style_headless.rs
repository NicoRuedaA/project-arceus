//! Windowless renders of the drawing style (Brief 35): the exported player and Pikachu glTFs plus
//! two plain primitives, lit by the real `ZoneLightingPlugin` (area 00, day by default), rendered
//! three times in ONE process: background only, `--style plain`, `--style original`. No window is
//! created (winit disabled, the camera renders to an image).
//!
//! It prints figures over the model pixels (pixels that differ from the background-only frame):
//! luminance mean/spread, the share of mid tones, the mean luminance of the silhouette ring (where
//! the outline lands) and a 16-bin luminance-histogram L1 distance to a crop of the one real-game
//! reference screenshot (`work/reference/real_game_target.png`, character box), when present. The
//! reference comparison is indicative only: different scene, pose, lighting and camera.
//!
//! Usage: `style_headless [--out work/style/style.png] [--time-slot 1] [--asset-root <work dir>]`.
//! The models come from `<asset-root>/cache/player/p1_default/player.gltf` and
//! `<asset-root>/cache/pokemon_batch/pm0025_00_00/pm0025_00_00_idle.gltf` (skipped if absent).

use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::camera::RenderTarget;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use bevy::world_serialization::WorldAssetRoot;
use pla::game::art_style::{ArtStyle, ArtStyleParams, ArtStylePlugin, ArtStyleTarget, OutlineParams, StyleSettings};
use pla::game::lighting::{time_slot_from_args, ZoneLightingPlugin};
use pla::game::render_look::LookSettings;

const W: u32 = 1280;
const H: u32 = 720;

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Case {
    Background,
    Plain,
    Original,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = arg(&args, "--out").unwrap_or_else(|| "work/style/style.png".into());
    let slot = time_slot_from_args(&args);
    let asset_root = arg(&args, "--asset-root")
        .or_else(|| std::env::var("PLA_CACHE_ROOT").ok().and_then(|c| {
            std::path::Path::new(&c).parent().map(|p| p.to_string_lossy().into_owned())
        }))
        .unwrap_or_else(|| "/home/nico/work/decompilacion/work".into());
    let stem = out.strip_suffix(".png").unwrap_or(&out).to_string();
    if let Some(dir) = std::path::Path::new(&out).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut paths = Vec::new();
    for case in [Case::Background, Case::Plain, Case::Original] {
        let path = format!("{stem}_{}.png", format!("{case:?}").to_lowercase());
        run_one(case, slot, &asset_root, path.clone());
        paths.push(path);
    }
    report(&paths[0], &paths[1], &paths[2]);
}

fn run_one(case: Case, slot: usize, asset_root: &str, out: String) {
    let style = StyleSettings {
        style: if case == Case::Original { ArtStyle::Original } else { ArtStyle::Plain },
        outlines: true,
    };
    let base = ArtStylePlugin::from_env_and_args();
    let mut app = App::new();
    app.insert_resource(LookSettings::default())
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    close_when_requested: false,
                    ..default()
                })
                .set(AssetPlugin { file_path: asset_root.to_string(), ..default() })
                .disable::<WinitPlugin>(),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_millis(16)));
    // The glTF scenes carry reflected components; register them as the animation plugins do.
    app.world().resource::<AppTypeRegistry>().write().register_derived_types();
    app.add_plugins(ZoneLightingPlugin { area: 0, slot })
        .add_plugins(ArtStylePlugin {
            settings: style,
            params: ArtStyleParams { toon: base.params.toon, outline: OutlineParams::default() },
        })
        .insert_resource(Run { case, out, frame: 0, ready_at: None, target: Handle::default(), gltfs: Vec::new() })
        .add_systems(Startup, setup)
        .add_systems(Update, (spawn_scenes, capture).chain());
    app.run();
}

#[derive(Resource)]
struct Run {
    case: Case,
    out: String,
    frame: u32,
    ready_at: Option<u32>,
    target: Handle<Image>,
    /// (glTF, root entity, scene spawned)
    gltfs: Vec<(Handle<Gltf>, Entity, bool)>,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    mut run: ResMut<Run>,
) {
    run.target = images.add(Image::new_target_texture(W, H, TextureFormat::Rgba8UnormSrgb, None));
    let pm = |c: Color| StandardMaterial { base_color: c, perceptual_roughness: 0.9, ..default() };
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(200.0, 200.0))),
        MeshMaterial3d(mats.add(pm(Color::srgb(0.55, 0.68, 0.18)))),
        // No cast shadows on the ground, so the model mask (frame minus background) holds models only.
        bevy::light::NotShadowReceiver,
    ));
    if run.case != Case::Background {
        // Plain primitives (non-skinned path).
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(0.35).mesh().uv(48, 24))),
            MeshMaterial3d(mats.add(pm(Color::srgb(0.85, 0.25, 0.2)))),
            Transform::from_xyz(1.15, 0.35, -0.4),
            ArtStyleTarget,
        ));
        commands.spawn((
            Mesh3d(meshes.add(Capsule3d::new(0.18, 0.5))),
            MeshMaterial3d(mats.add(pm(Color::srgb(0.15, 0.25, 0.6)))),
            Transform::from_xyz(-1.35, 0.43, -0.6),
            ArtStyleTarget,
        ));
        // Skinned glTFs (player and Pikachu) when exported.
        for (rel, pos, scale) in [
            ("cache/player/p1_default/player.gltf", Vec3::new(0.25, 0.0, 0.0), 1.0),
            ("cache/pokemon_batch/pm0025_00_00/pm0025_00_00_idle.gltf", Vec3::new(-0.6, 0.0, 0.35), 1.0),
        ] {
            if !std::path::Path::new(&asset_server_root()).join(rel).exists() {
                eprintln!("[skip] {rel} not exported");
                continue;
            }
            let h: Handle<Gltf> = asset_server.load(rel);
            let e = commands
                .spawn((
                    Transform::from_translation(pos).with_scale(Vec3::splat(scale)),
                    Visibility::default(),
                    ArtStyleTarget,
                ))
                .id();
            run.gltfs.push((h, e, false));
        }
    }
    commands.spawn((
        Camera3d::default(),
        RenderTarget::Image(run.target.clone().into()),
        Transform::from_xyz(0.0, 1.05, 3.1).looking_at(Vec3::new(0.0, 0.7, 0.0), Vec3::Y),
    ));
}

fn asset_server_root() -> String {
    let args: Vec<String> = std::env::args().collect();
    arg(&args, "--asset-root")
        .or_else(|| std::env::var("PLA_CACHE_ROOT").ok().and_then(|c| {
            std::path::Path::new(&c).parent().map(|p| p.to_string_lossy().into_owned())
        }))
        .unwrap_or_else(|| "/home/nico/work/decompilacion/work".into())
}

fn spawn_scenes(mut commands: Commands, gltfs: Res<Assets<Gltf>>, mut run: ResMut<Run>) {
    for (h, e, done) in run.gltfs.iter_mut() {
        if *done {
            continue;
        }
        let Some(g) = gltfs.get(&*h) else { continue };
        let Some(scene) = g.default_scene.clone().or_else(|| g.scenes.first().cloned()) else {
            *done = true;
            continue;
        };
        commands.entity(*e).with_children(|p| {
            p.spawn((WorldAssetRoot(scene), Transform::IDENTITY, Visibility::default()));
        });
        *done = true;
    }
}

#[allow(clippy::type_complexity)]
fn capture(
    mut commands: Commands,
    mut run: ResMut<Run>,
    meshes: Query<(), With<Mesh3d>>,
    children: Query<&Children>,
    mut exit: MessageWriter<AppExit>,
) {
    run.frame += 1;
    // Ready when every requested glTF has spawned meshes (or after a timeout).
    if run.ready_at.is_none() {
        let loaded = run.gltfs.iter().all(|(_, e, done)| {
            *done && children.iter_descendants(*e).any(|d| meshes.get(d).is_ok())
        });
        if loaded || run.frame > 1200 {
            if !loaded {
                eprintln!("[warn] models not loaded after {} frames", run.frame);
            }
            run.ready_at = Some(run.frame);
        }
    }
    let Some(r) = run.ready_at else { return };
    // Give the pipelines time to compile before the shot.
    if run.frame == r + 150 {
        commands.spawn(Screenshot::image(run.target.clone())).observe(save_to_disk(run.out.clone()));
    }
    if run.frame == r + 190 {
        exit.write(AppExit::Success);
    }
}

// --- Measurements ----------------------------------------------------------------------------------

struct Rgb {
    w: usize,
    h: usize,
    px: Vec<[f32; 3]>,
}

fn load(path: &str) -> Result<Rgb, Box<dyn std::error::Error>> {
    let mut dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info()?;
    let size = reader.output_buffer_size().ok_or("image too large")?;
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf)?;
    let ch = info.color_type.samples();
    let data = &buf[..info.buffer_size()];
    let px = data
        .chunks_exact(ch)
        .map(|p| if ch >= 3 { [p[0], p[1], p[2]] } else { [p[0], p[0], p[0]] })
        .map(|p| p.map(|v| v as f32 / 255.0))
        .collect();
    Ok(Rgb { w: info.width as usize, h: info.height as usize, px })
}

fn luma(p: [f32; 3]) -> f32 {
    0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]
}

fn saturation(p: [f32; 3]) -> f32 {
    let mx = p[0].max(p[1]).max(p[2]);
    let mn = p[0].min(p[1]).min(p[2]);
    if mx <= 1e-6 { 0.0 } else { (mx - mn) / mx }
}

fn histogram(values: impl Iterator<Item = f32>) -> [f32; 16] {
    let mut h = [0f32; 16];
    let mut n = 0f32;
    for v in values {
        h[((v.clamp(0.0, 0.9999)) * 16.0) as usize] += 1.0;
        n += 1.0;
    }
    if n > 0.0 {
        h.iter_mut().for_each(|b| *b /= n);
    }
    h
}

fn l1(a: &[f32; 16], b: &[f32; 16]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum()
}

struct Stats {
    n: usize,
    mean: f32,
    std: f32,
    mid: f32,
    dark: f32,
    sat: f32,
    hist: [f32; 16],
}

fn stats(img: &Rgb, mask: &[bool]) -> Stats {
    let vals: Vec<(f32, f32)> =
        img.px.iter().zip(mask).filter(|(_, m)| **m).map(|(p, _)| (luma(*p), saturation(*p))).collect();
    let n = vals.len().max(1) as f32;
    let mean = vals.iter().map(|v| v.0).sum::<f32>() / n;
    let std = (vals.iter().map(|v| (v.0 - mean).powi(2)).sum::<f32>() / n).sqrt();
    Stats {
        n: vals.len(),
        mean,
        std,
        mid: vals.iter().filter(|v| (0.3..0.6).contains(&v.0)).count() as f32 / n,
        dark: vals.iter().filter(|v| v.0 < 0.12).count() as f32 / n,
        sat: vals.iter().map(|v| v.1).sum::<f32>() / n,
        hist: histogram(vals.iter().map(|v| v.0)),
    }
}

fn report(bg: &str, plain: &str, original: &str) {
    let (Ok(bg), Ok(pl), Ok(or)) = (load(bg), load(plain), load(original)) else {
        eprintln!("[measure] cannot read back the frames");
        return;
    };
    if bg.w != pl.w || bg.w != or.w || bg.h != pl.h || bg.h != or.h {
        eprintln!("[measure] frame sizes differ");
        return;
    }
    let diff = |a: [f32; 3], b: [f32; 3]| (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0f32, f32::max);
    // Model pixels: differ from the background-only frame in either styled frame.
    let mask: Vec<bool> =
        (0..bg.px.len()).map(|i| diff(pl.px[i], bg.px[i]) > 0.04 || diff(or.px[i], bg.px[i]) > 0.04).collect();
    // Silhouette ring: model pixels with a non-model pixel within 2 px.
    let (w, h) = (bg.w, bg.h);
    let ring: Vec<bool> = (0..mask.len())
        .map(|i| {
            if !mask[i] {
                return false;
            }
            let (x, y) = ((i % w) as i64, (i / w) as i64);
            (-2i64..=2).any(|dy| {
                (-2i64..=2).any(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0 && ny >= 0 && nx < w as i64 && ny < h as i64 && !mask[(ny as usize) * w + nx as usize]
                })
            })
        })
        .collect();
    let changed = (0..mask.len()).filter(|i| mask[*i] && diff(pl.px[*i], or.px[*i]) > 0.02).count();
    let reference = std::path::Path::new(&asset_server_root()).join("reference/real_game_target.png");
    let ref_hist = load(&reference.to_string_lossy()).ok().filter(|r| r.w == 1920 && r.h == 1080).map(|r| {
        // Character box of the reference screenshot (approximate, includes some grass).
        let m: Vec<bool> = (0..r.px.len()).map(|i| (840..1110).contains(&(i % r.w)) && (575..1080).contains(&(i / r.w))).collect();
        stats(&r, &m)
    });
    if let Some(rs) = &ref_hist {
        println!(
            "[measure] reference character box: n {}, mean {:.4}, std {:.4}, mid-tones {:.3}, dark {:.4}, saturation {:.4}",
            rs.n, rs.mean, rs.std, rs.mid, rs.dark, rs.sat
        );
    }
    for (name, img) in [("plain", &pl), ("original", &or)] {
        let s = stats(img, &mask);
        let r = stats(img, &ring);
        let dref = ref_hist.as_ref().map(|rs| format!("{:.4}", l1(&s.hist, &rs.hist))).unwrap_or_else(|| "n/a".into());
        println!(
            "[measure] {name}: model px {}, mean {:.4}, std {:.4}, mid-tones {:.3}, dark<0.12 {:.4}, saturation {:.4}, ring mean {:.4} (n {}), hist L1 vs reference {dref}",
            s.n, s.mean, s.std, s.mid, s.dark, s.sat, r.mean, r.n
        );
    }
    let sp = stats(&pl, &mask);
    let so = stats(&or, &mask);
    println!(
        "[measure] plain vs original: changed model px {changed} of {} ({:.1}%), hist L1 {:.4}",
        sp.n,
        100.0 * changed as f32 / sp.n.max(1) as f32,
        l1(&sp.hist, &so.hist)
    );
}
