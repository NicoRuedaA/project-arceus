//! Windowless render of the real terrain (baked albedo) with streamed vegetation, camera at ground level.
//! No window (winit disabled; the camera renders to an image), so nothing appears on the desktop.
//! Usage: `vegetation_headless [--area 1] [--pos X,Z] [--yaw DEG] [--grass off|low|high] [--out PNG]`
//! Prints the plugin's live stats (chunks, draw calls, instances, triangles) at capture time.

use std::path::PathBuf;
use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use pla::assets::terrain::{build_mesh_chunks, TerrainArea, TerrainScaleOverride};
use pla::assets::terrain_albedo::load_baked_albedo;
use pla::assets::vegetation::GrassQuality;
use pla::game::lighting::ZoneLightingPlugin;
use pla::game::vegetation::{VegetationPlugin, VegetationStats};
use pla::game::PlayerAvatar;

#[derive(Resource)]
struct Cfg {
    out: String,
    area: u8,
    pos: Vec2,
    yaw: f32,
    bin_root: PathBuf,
    cache: PathBuf,
    frame: u32,
    target: Handle<Image>,
}

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let area: u8 = arg(&args, "--area").and_then(|s| s.parse().ok()).unwrap_or(1);
    let pos = arg(&args, "--pos")
        .and_then(|s| {
            let (a, b) = s.split_once(',')?;
            Some(Vec2::new(a.trim().parse().ok()?, b.trim().parse().ok()?))
        })
        .unwrap_or(Vec2::new(442.0, 246.0));
    let cache = PathBuf::from(std::env::var("PLA_CACHE_ROOT").unwrap_or_else(|_| "/home/nico/work/decompilacion/work/cache".into()));
    let work = cache.parent().unwrap().to_path_buf();
    let bin_root = PathBuf::from(
        arg(&args, "--bin-root").unwrap_or_else(|| "/home/nico/work/decompilacion/work/pla/pk2/romfs_patched/bin".into()),
    );
    let quality = GrassQuality::from_args(&args);
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin { primary_window: None, exit_condition: ExitCondition::DontExit, close_when_requested: false, ..default() })
                .set(AssetPlugin { file_path: work.to_string_lossy().to_string(), unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow, ..default() })
                .disable::<WinitPlugin>(),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_millis(16)))
        .add_plugins(ZoneLightingPlugin { area, slot: 1 })
        .add_plugins(VegetationPlugin { area, bin_root: bin_root.clone(), cache_root: cache.clone(), quality })
        .insert_resource(Cfg {
            out: arg(&args, "--out").unwrap_or_else(|| "work/veg/headless.png".into()),
            area,
            pos,
            yaw: arg(&args, "--yaw").and_then(|s| s.parse().ok()).unwrap_or(0.0),
            bin_root,
            cache,
            frame: 0,
            target: Handle::default(),
        })
        .add_systems(Startup, setup)
        .add_systems(Update, capture)
        .run();
}

fn setup(mut commands: Commands, mut cfg: ResMut<Cfg>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    let area = TerrainArea::load(&cfg.bin_root, cfg.area).expect("terrain");
    let settings = area.mesh_settings(TerrainScaleOverride::default(), 64).unwrap();
    let chunks = build_mesh_chunks(&area.heightmap, settings).unwrap();
    let albedo = load_baked_albedo(pla::assets::terrain_albedo::albedo_cache_path(cfg.cache.parent().unwrap(), cfg.area)).ok().map(|t| {
        images.add(Image::new(Extent3d { width: t.width, height: t.height, depth_or_array_layers: 1 }, TextureDimension::D2, t.rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default()))
    });
    let mat = mats.add(StandardMaterial { base_color: Color::WHITE, base_color_texture: albedo, perceptual_roughness: 0.95, ..default() });
    for c in &chunks {
        commands.spawn((Mesh3d(meshes.add(c.mesh.clone())), MeshMaterial3d(mat.clone())));
    }
    let y = area.sample_world_height(cfg.pos.x, cfg.pos.y).unwrap_or(0.0);
    commands.spawn((PlayerAvatar, Transform::from_xyz(cfg.pos.x, y + 0.9, cfg.pos.y)));
    cfg.target = images.add(Image::new_target_texture(1280, 720, TextureFormat::Rgba8UnormSrgb, None));
    let rot = Quat::from_rotation_y(cfg.yaw.to_radians()) * Quat::from_rotation_x(-0.12);
    commands.spawn((Camera3d::default(), RenderTarget::Image(cfg.target.clone().into()), Transform::from_xyz(cfg.pos.x, y + 1.8, cfg.pos.y).with_rotation(rot)));
}

fn capture(mut commands: Commands, mut cfg: ResMut<Cfg>, stats: Option<Res<VegetationStats>>, mut exit: MessageWriter<AppExit>) {
    cfg.frame += 1;
    if cfg.frame == 600 {
        if let Some(s) = &stats {
            println!("stats at capture: {s:?}");
        }
        commands.spawn(Screenshot::image(cfg.target.clone())).observe(save_to_disk(cfg.out.clone()));
    }
    if cfg.frame == 660 {
        exit.write(AppExit::Success);
    }
}
