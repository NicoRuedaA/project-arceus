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
            .add_systems(Last, write_perf_line);
        println!("[perf-log] writing {LOG_PATH} every {LOG_INTERVAL} s");
    }
}

fn write_perf_line(
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
        "fps {} | frame {} ms | gpu passes total {gpu_total:.1} ms [{}] | {win} | {gpu}\n",
        fps.map_or("?".into(), |v| format!("{v:.1}")),
        frame_ms.map_or("?".into(), |v| format!("{v:.1}")),
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
