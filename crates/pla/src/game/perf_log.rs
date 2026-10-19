//! Optional performance log of the real (windowed) viewer.
//!
//! `PLA_PERF_LOG=1 ./play.sh ...` writes one line every 3 seconds to `work/perf/windowed.log`:
//! frames per second, frame time, the window and its scale, the GPU adapter, the present mode and
//! the GPU time of the heaviest render passes. It is the only way to measure what a real window
//! costs: the automation `bench` command is offscreen and has no presentation or vsync.
//! Without the variable the plugin does nothing.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::diagnostic::RenderDiagnosticsPlugin;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::window::PrimaryWindow;

/// Seconds between two lines of the log.
pub const LOG_INTERVAL: f32 = 3.0;
const LOG_PATH: &str = "work/perf/windowed.log";

pub struct PerfLogPlugin;

impl Plugin for PerfLogPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os("PLA_PERF_LOG").is_none() {
            return;
        }
        app.add_plugins((FrameTimeDiagnosticsPlugin::default(), RenderDiagnosticsPlugin))
            .init_resource::<UpdateTimer>()
            .init_resource::<AssetChurn>()
            .add_systems(PostUpdate, count_asset_churn)
            .add_systems(First, start_update_timer)
            .add_systems(Last, (stop_update_timer, write_perf_line).chain());
        println!("[perf-log] writing {LOG_PATH} every {LOG_INTERVAL} s");
    }
}

/// Asset changes seen since the last line: a changed material or mesh is re-uploaded and, for a
/// material, gets a new bind group, so per-frame churn here is a render-thread cost.
#[derive(Resource, Default)]
struct AssetChurn {
    materials_modified: u32,
    materials_added: u32,
    meshes_modified: u32,
    images_modified: u32,
}

fn count_asset_churn(
    mut churn: ResMut<AssetChurn>,
    mut materials: MessageReader<AssetEvent<StandardMaterial>>,
    mut meshes: MessageReader<AssetEvent<Mesh>>,
    mut images: MessageReader<AssetEvent<Image>>,
) {
    for event in materials.read() {
        match event {
            AssetEvent::Modified { .. } => churn.materials_modified += 1,
            AssetEvent::Added { .. } => churn.materials_added += 1,
            _ => {}
        }
    }
    churn.meshes_modified += meshes.read().filter(|e| matches!(e, AssetEvent::Modified { .. })).count() as u32;
    churn.images_modified += images.read().filter(|e| matches!(e, AssetEvent::Modified { .. })).count() as u32;
}

/// Wall time the main world spends in its schedules (from `First` to `Last`) since the last line.
#[derive(Resource, Default)]
struct UpdateTimer {
    started: Option<Instant>,
    total_ms: f64,
    frames: u32,
}

fn start_update_timer(mut timer: ResMut<UpdateTimer>) {
    timer.started = Some(Instant::now());
}

fn stop_update_timer(mut timer: ResMut<UpdateTimer>) {
    if let Some(started) = timer.started.take() {
        timer.total_ms += started.elapsed().as_secs_f64() * 1000.0;
        timer.frames += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn write_perf_line(
    mut timer: ResMut<UpdateTimer>,
    mut churn: ResMut<AssetChurn>,
    meshes: Res<Assets<Mesh>>,
    images: Res<Assets<Image>>,
    entities: Query<Entity>,
    time: Res<Time<Real>>,
    store: Res<DiagnosticsStore>,
    window: Query<&Window, With<PrimaryWindow>>,
    adapter: Option<Res<RenderAdapterInfo>>,
    mut since: Local<f32>,
) {
    *since += time.delta_secs();
    if *since < LOG_INTERVAL {
        return;
    }
    *since = 0.0;
    let update_ms = if timer.frames > 0 { timer.total_ms / f64::from(timer.frames) } else { 0.0 };
    let frames = f64::from(timer.frames.max(1));
    timer.total_ms = 0.0;
    timer.frames = 0;
    let churn_text = format!(
        "asset changes per frame: materials modified {:.1} added {:.1}, meshes modified {:.1}, images modified {:.1}",
        f64::from(churn.materials_modified) / frames,
        f64::from(churn.materials_added) / frames,
        f64::from(churn.meshes_modified) / frames,
        f64::from(churn.images_modified) / frames,
    );
    *churn = AssetChurn::default();
    let avg = |path: &bevy::diagnostic::DiagnosticPath| store.get(path).and_then(|d| d.average());
    let fps = avg(&FrameTimeDiagnosticsPlugin::FPS);
    let frame_ms = avg(&FrameTimeDiagnosticsPlugin::FRAME_TIME);
    let mut passes: Vec<(f64, String)> = store
        .iter()
        .filter_map(|d| {
            let path = d.path().as_str();
            let name = path.strip_prefix("render/")?.strip_suffix("/elapsed_gpu")?;
            Some((d.average()?, name.to_string()))
        })
        .collect();
    passes.sort_by(|a, b| b.0.total_cmp(&a.0));
    let gpu_total: f64 = passes.iter().map(|p| p.0).sum();
    let top: Vec<String> = passes.iter().take(6).map(|(ms, n)| format!("{n}={ms:.2}")).collect();
    let win = window.single().map_or_else(
        |_| "no window".to_string(),
        |w| {
            format!(
                "{}x{} px (scale {:.2}) present {:?}",
                w.physical_width(),
                w.physical_height(),
                w.scale_factor(),
                w.present_mode
            )
        },
    );
    let gpu = adapter.map_or_else(|| "?".to_string(), |a| format!("{} ({:?}, {:?})", a.name, a.backend, a.device_type));
    let line = format!(
        "fps {} | frame {} ms | main update {update_ms:.1} ms | entities {} meshes {} images {} | {churn_text} | gpu passes total {gpu_total:.1} ms [{}] | {win} | {gpu}\n",
        fps.map_or("?".into(), |v| format!("{v:.1}")),
        frame_ms.map_or("?".into(), |v| format!("{v:.1}")),
        entities.iter().count(),
        meshes.len(),
        images.len(),
        top.join(", ")
    );
    let path = PathBuf::from(LOG_PATH);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = file.write_all(line.as_bytes());
    }
}
