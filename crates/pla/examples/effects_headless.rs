//! Windowless GPU render of battle move effects through the real `EffectsPlugin`
//! (custom WGSL material, blend modes, camera-facing quads), as in `look_headless`:
//! no window is created (winit disabled, the camera renders to an image).
//!
//! The arena copies the viewer's battle layout (`setup_battle_arena` slot positions and the
//! battle camera framing); grey spheres stand in for the Pokémon. Each move is played from
//! slot 0 (user) to slot 1 (target) and three frames of its timeline are saved.
//! Usage: `effects_headless [--moves 84,52,33] [--frames 20,40,60] [--out work/effects/gpu]`
//! Writes `<out>_<move>_<frame>.png` and prints the live particle counts.

use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::time::TimeUpdateStrategy;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use pla::game::effects::sim::Quality;
use pla::game::effects::{EffectAnchor, EffectRef, EffectStats, EffectsPlugin, PlayEffect};
use pla::game::pokemon_anim::PokemonBattleSlot;

const LEAD: u32 = 5;
const GAP: u32 = 110;

#[derive(Resource)]
struct Script {
    moves: Vec<u16>,
    frames: Vec<u32>,
    out: String,
    target: Handle<Image>,
    tick: u32,
}

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let list = |k: &str, d: &str| -> Vec<u32> {
        arg(&args, k).unwrap_or_else(|| d.into()).split(',').filter_map(|s| s.parse().ok()).collect()
    };
    let moves: Vec<u16> = list("--moves", "84,52,33").into_iter().map(|m| m as u16).collect();
    let frames = list("--frames", "20,40,60");
    let out = arg(&args, "--out").unwrap_or_else(|| "work/effects/gpu".into());
    if !pla::assets::ptcl_textures::default_bin_root().join("effect/battle/sequence").is_dir() {
        println!("[skip] game effect data not present");
        return;
    }
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
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_millis(5)))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 30.0)))
        .add_plugins(EffectsPlugin { quality: Some(Quality::High), sync_loading: true, ..Default::default() })
        .insert_resource(Script { moves, frames, out, target: Handle::default(), tick: 0 })
        .add_systems(Startup, setup)
        .add_systems(Update, drive)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut script: ResMut<Script>,
) {
    let target = images.add(Image::new_target_texture(960, 540, TextureFormat::Rgba8UnormSrgb, None));
    script.target = target.clone();
    let pm = |c: Color| StandardMaterial { base_color: c, perceptual_roughness: 0.9, ..default() };
    commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(60.0, 60.0))), MeshMaterial3d(mats.add(pm(Color::srgb(0.32, 0.42, 0.2))))));
    // Viewer arena, relative to the player: Pikachu slot 0, wild slot 1.
    let (p, w) = (Vec3::new(-0.7, 0.0, -1.8), Vec3::new(0.9, 0.0, -4.4));
    for (slot, pos, r) in [(0usize, p, 0.35f32), (1, w, 0.4)] {
        commands
            .spawn((PokemonBattleSlot(slot), Transform::from_translation(pos), Visibility::default()))
            .with_children(|c| {
                c.spawn((
                    Mesh3d(meshes.add(Sphere::new(r))),
                    MeshMaterial3d(mats.add(pm(Color::srgb(0.5, 0.5, 0.55)))),
                    Transform::from_xyz(0.0, r, 0.0),
                ));
            });
    }
    commands.spawn((DirectionalLight { illuminance: 8000.0, ..default() }, Transform::from_xyz(3.0, 8.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y)));
    let to_wild = (w - p).normalize();
    let right_side = Vec3::new(to_wild.z, 0.0, -to_wild.x).normalize();
    let eye = p - to_wild * 2.3 - right_side * 1.0 + Vec3::new(0.0, 1.35, 0.0);
    let look = (p + w) * 0.5 + Vec3::new(0.0, 0.35, 0.0) + right_side * 0.2;
    commands.spawn((
        Camera3d::default(),
        RenderTarget::Image(target.into()),
        Transform::from_translation(eye).looking_at(look, Vec3::Y),
    ));
}

fn drive(
    mut commands: Commands,
    mut script: ResMut<Script>,
    stats: Res<EffectStats>,
    mut play: MessageWriter<PlayEffect>,
    mut exit: MessageWriter<AppExit>,
) {
    script.tick += 1;
    let t = script.tick;
    let total = script.moves.len() as u32 * GAP;
    if t >= LEAD + total + 20 {
        exit.write(AppExit::Success);
        return;
    }
    if t < LEAD {
        return;
    }
    let k = (t - LEAD) / GAP;
    let local = (t - LEAD) % GAP;
    let Some(&mv) = script.moves.get(k as usize) else { return };
    if local == 0 {
        play.write(PlayEffect::at(
            EffectRef::Move { id: mv, species: 25, form: 0 },
            EffectAnchor::Slot(0),
            EffectAnchor::Slot(1),
        ));
    }
    // The message is read on the next update, so timeline frame f is reached at local f + 1.
    if script.frames.iter().any(|f| f + 1 == local) {
        let path = format!("{}_{mv:03}_{:02}.png", script.out, local - 1);
        println!("move {mv} frame {}: {} live particles in {} instances -> {path}", local - 1, stats.live_particles, stats.live_instances);
        commands.spawn(Screenshot::image(script.target.clone())).observe(save_to_disk(path));
    }
}
