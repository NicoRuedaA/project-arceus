//! Offscreen (windowless) glTF capture tool with automatic camera framing.
//!
//! Loads exported glTF scenes through Bevy's asset system, frames each model by
//! its mesh bounds and saves one PNG per model. No window is created: the
//! camera renders into an offscreen image and the screenshot is written to
//! disk. Used to look at recovered field/Pokémon exports without opening the
//! viewer (operating rules: one GPU process at a time).
//!
//! Usage:
//!   render_gltf_headless --root <asset-root> --out <dir> [--size N] [--yaw DEG]
//!     [--dist F] [--margin M] [--capture-frame N] [--warmup-frames N]
//!     <label>=<relative gltf path>...
//!
//! `<asset-root>` is the AssetPlugin root (defaults to `work`); each positional
//! model path is relative to it. The label names the output PNG. A label of the
//! form `name@DEG` renders that model with its own camera yaw (`--yaw` is the default),
//! so a sweep can look at each model from the side its eyes face.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::camera::RenderTarget;
use bevy::camera::primitives::MeshAabb;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use bevy::world_serialization::WorldAssetRoot;

#[derive(Resource)]
struct Queue {
    models: Vec<(String, PathBuf, PathBuf)>,
    idx: usize,
    frame: u32,
    handle: Option<Handle<Gltf>>,
    root: Option<Entity>,
    target: Handle<Image>,
    dist_override: Option<f32>,
    margin: f32,
    yaw_deg: f32,
    warmup_frames: u32,
    captured: bool,
}

fn parse_args() -> Result<(PathBuf, PathBuf, u32, Option<f32>, f32, f32, u32, Vec<(String, PathBuf)>), String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut root = PathBuf::from("work");
    let mut out = PathBuf::from("/home/nico/work/capturas_modelos19");
    let mut size: u32 = 512;
    let mut dist_override: Option<f32> = None;
    let mut margin: f32 = 1.08;
    let mut yaw_deg: f32 = 34.0;
    let mut warmup_frames: u32 = 60;
    let take = |args: &mut Vec<String>, flag: &str| -> Option<String> {
        args.iter().position(|a| a == flag).map(|i| {
            args.remove(i);
            args.remove(i)
        })
    };
    if let Some(v) = take(&mut args, "--root") { root = PathBuf::from(v); }
    if let Some(v) = take(&mut args, "--out") { out = PathBuf::from(v); }
    if let Some(v) = take(&mut args, "--size") { size = v.parse().map_err(|e| format!("bad --size: {e}"))?; }
    if let Some(v) = take(&mut args, "--dist") { dist_override = Some(v.parse().map_err(|e| format!("bad --dist: {e}"))?); }
    if let Some(v) = take(&mut args, "--margin") { margin = v.parse().map_err(|e| format!("bad --margin: {e}"))?; }
    if let Some(v) = take(&mut args, "--yaw") { yaw_deg = v.parse().map_err(|e| format!("bad --yaw: {e}"))?; }
    if let Some(v) = take(&mut args, "--warmup-frames") { warmup_frames = v.parse().map_err(|e| format!("bad --warmup-frames: {e}"))?; }
    let mut models = Vec::new();
    for arg in args {
        let Some((label, rel)) = arg.split_once('=') else {
            return Err(format!("expected <label>=<relative path>, got '{arg}'"));
        };
        models.push((label.to_string(), PathBuf::from(rel)));
    }
    if models.is_empty() {
        return Err("no models given".into());
    }
    Ok((root, out, size, dist_override, margin, yaw_deg, warmup_frames, models))
}

/// `name@DEG` -> (`name`, Some(DEG)); a label without a valid `@DEG` suffix is returned whole.
fn split_label_yaw(label: &str) -> (&str, Option<f32>) {
    match label.rsplit_once('@') {
        Some((name, deg)) => match deg.parse::<f32>() {
            Ok(v) => (name, Some(v)),
            Err(_) => (label, None),
        },
        None => (label, None),
    }
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    queue_init: Res<InitArgs>,
) {
    let target = images.add(Image::new_target_texture(
        queue_init.size,
        queue_init.size,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let out_root = &queue_init.out;
    let models = queue_init
        .models
        .iter()
        .map(|(label, rel)| {
            let png = out_root.join(format!("{}.png", split_label_yaw(label).0));
            (label.clone(), rel.clone(), png)
        })
        .collect();
    commands.insert_resource(Queue {
        models,
        idx: 0,
        frame: 0,
        handle: None,
        root: None,
        target: target.clone(),
        dist_override: queue_init.dist_override,
        margin: queue_init.margin,
        yaw_deg: queue_init.yaw_deg,
        warmup_frames: queue_init.warmup_frames,
        captured: false,
    });
    commands.spawn((
        Camera3d::default(),
        RenderTarget::Image(target.into()),
        Transform::default(),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 11000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-4.0, 3.0, -3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

#[cfg(test)]
mod tests {
    use super::split_label_yaw;

    #[test]
    fn label_yaw_suffix_is_optional() {
        assert_eq!(split_label_yaw("s019_f00"), ("s019_f00", None));
        assert_eq!(split_label_yaw("s019_f00@-120.5"), ("s019_f00", Some(-120.5)));
        assert_eq!(split_label_yaw("a@b"), ("a@b", None));
    }
}

#[derive(Resource)]
struct InitArgs {
    size: u32,
    out: PathBuf,
    models: Vec<(String, PathBuf)>,
    dist_override: Option<f32>,
    margin: f32,
    yaw_deg: f32,
    warmup_frames: u32,
}

fn process_queue(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<bevy::gltf::GltfMesh>>,
    meshes: Res<Assets<Mesh>>,
    mut queue: ResMut<Queue>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
    mut exit: MessageWriter<AppExit>,
) {
    queue.frame += 1;

    if queue.frame == 1 {
        if queue.idx >= queue.models.len() {
            println!("All {} models rendered.", queue.models.len());
            exit.write(AppExit::Success);
            return;
        }
        let (label, rel, _) = queue.models[queue.idx].clone();
        println!("[{}/{}] loading '{label}' ({rel:?})", queue.idx + 1, queue.models.len());
        queue.handle = Some(asset_server.load(rel.to_string_lossy().to_string()));
    }

    if queue.root.is_none() {
        if let Some(handle) = queue.handle.clone() {
            if let Some(gltf) = gltfs.get(&handle) {
                // Union mesh bounds for auto framing.
                let mut lo = Vec3::splat(f32::MAX);
                let mut hi = Vec3::splat(f32::MIN);
                let mut any = false;
                for mesh_handle in &gltf.meshes {
                    let Some(gltf_mesh) = gltf_meshes.get(mesh_handle) else { continue };
                    for prim in &gltf_mesh.primitives {
                        if let Some(mesh) = meshes.get(&prim.mesh) {
                            if let Some(aabb) = mesh.compute_aabb() {
                                lo = lo.min(Vec3::from(aabb.min()));
                                hi = hi.max(Vec3::from(aabb.max()));
                                any = true;
                            }
                        }
                    }
                }
                if !any {
                    lo = Vec3::splat(-1.0);
                    hi = Vec3::splat(1.0);
                }
                let center = (lo + hi) * 0.5;
                let radius = (hi - lo).length() * 0.5;
                let own_yaw = split_label_yaw(&queue.models[queue.idx].0).1;
                let yaw = own_yaw.unwrap_or(queue.yaw_deg).to_radians();
                let dir = Vec3::new(yaw.sin(), 0.42, yaw.cos()).normalize();
                let dist = queue
                    .dist_override
                    .unwrap_or_else(|| radius / (22.5_f32.to_radians()).tan() * queue.margin);
                if let Ok(mut cam) = camera.single_mut() {
                    *cam = Transform::from_translation(center + dir * dist).looking_at(center, Vec3::Y);
                }
                let Some(scene) = gltf.default_scene.clone().or_else(|| gltf.scenes.first().cloned()) else {
                    eprintln!("  no default scene; skipping");
                    queue.idx += 1;
                    queue.frame = 0;
                    queue.handle = None;
                    return;
                };
                let e = commands.spawn((WorldAssetRoot(scene), Transform::default(), Visibility::default())).id();
                queue.root = Some(e);
                println!("  framed: center {center:?}, radius {radius:.3}, dist {dist:.3}");
                queue.frame = 2;
            }
        }
        return;
    }

    let warm = queue.warmup_frames + 2;
    if !queue.captured && queue.frame >= warm {
        let (label, _, png) = queue.models[queue.idx].clone();
        if let Some(parent) = png.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        println!("  capturing '{label}' -> {png:?}");
        commands.spawn(Screenshot::image(queue.target.clone())).observe(save_to_disk(png.to_string_lossy().into_owned()));
        queue.captured = true;
    }
    if queue.captured && queue.frame >= warm + 15 {
        if let Some(e) = queue.root.take() {
            commands.entity(e).despawn();
        }
        queue.captured = false;
        queue.handle = None;
        queue.idx += 1;
        queue.frame = 0;
    }
}

fn main() -> ExitCode {
    let (root, out, size, dist_override, margin, yaw_deg, warmup_frames, models) =
        match parse_args() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        };
    let root_abs = match root.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cannot canonicalize root {root:?}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let out_abs = if out.is_absolute() {
        out
    } else {
        match out.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                let _ = std::fs::create_dir_all(&out);
                out.canonicalize().unwrap_or(out)
            }
        }
    };
    let _ = std::fs::create_dir_all(&out_abs);
    println!("Asset root: {}", root_abs.display());
    println!("Out dir: {}", out_abs.display());

    let mut app = App::new();
    app.insert_resource(InitArgs {
        size,
        out: out_abs,
        models,
        dist_override,
        margin,
        yaw_deg,
        warmup_frames,
    })
    .add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                close_when_requested: false,
                ..default()
            })
            .set(AssetPlugin {
                file_path: root_abs.to_string_lossy().to_string(),
                unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                ..default()
            })
            .disable::<WinitPlugin>(),
    )
    .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_millis(16)))
    .insert_resource(ClearColor(Color::srgb(0.42, 0.45, 0.50)))
    .add_systems(Startup, setup)
    .add_systems(Update, process_queue);
    // The glTF scenes carry reflected components; register them as the animation plugins do.
    app.world().resource::<AppTypeRegistry>().write().register_derived_types();
    app.run();

    ExitCode::SUCCESS
}
