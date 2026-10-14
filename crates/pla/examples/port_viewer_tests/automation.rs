//! Automation mode of the viewer (`--automation`), built exactly as `main` builds it but
//! logic-only (no GPU), so the tests run anywhere:
//!
//! 1. the app registers no window, no winit event loop and a fixed frame time;
//! 2. a real client talks to the real runner over the Unix socket (ping, step, state, quit).
//!
//! Included from `port_viewer.rs` (`cargo test -p pla --example port_viewer`).

use super::*;
use pla::game::automation::{add_default_plugins, AutomationConfig, AutomationPlugin};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pla-automation-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

/// The automation app `main` would build for `--automation --area 1`, without a GPU.
fn automation_app(dir: &std::path::Path) -> App {
    let args: Vec<String> = ["port_viewer", "--area", "1", "--automation", "--automation-no-gpu", "--automation-dir"]
        .iter()
        .map(|s| s.to_string())
        .chain([dir.to_string_lossy().into_owned()])
        .collect();
    let cfg = AutomationConfig::from_args(&args).expect("--automation");
    assert!(!cfg.gpu);
    let mut app = App::new();
    let gpu = add_default_plugins(&mut app, repo_root().join("work").to_string_lossy().into_owned(), &cfg);
    assert!(!gpu, "logic-only was requested");
    let mut setup = super::startup_schedule_guard::setup_for(&["port_viewer", "--area", "1"], &dir.join("save"));
    setup.remote = Some(AutomationPlugin { config: cfg, gpu });
    configure_viewer(&mut app, setup);
    app
}

/// Automation never registers a window (a visible window would grab the user's keyboard and
/// mouse), never adds winit and advances time by a fixed step.
#[test]
fn automation_mode_registers_no_window() {
    let dir = scratch("nowindow");
    let mut app = automation_app(&dir);
    assert!(!app.is_plugin_added::<bevy::winit::WinitPlugin>(), "winit must not be added");
    assert!(!app.is_plugin_added::<bevy::gilrs::GilrsPlugin>(), "gilrs must not be added");
    app.finish();
    app.cleanup();
    let problems = pla::game::schedule_check::initialize_all_schedules(&mut app);
    assert!(problems.is_empty(), "automation schedule problems:\n{}", problems.join("\n"));
    for _ in 0..5 {
        app.update();
    }
    let world = app.world_mut();
    let windows = world.query::<&bevy::window::Window>().iter(world).count();
    assert_eq!(windows, 0, "automation mode created {windows} window(s)");
    let primary = world.query::<&bevy::window::PrimaryWindow>().iter(world).count();
    assert_eq!(primary, 0);
    assert!(matches!(
        world.resource::<bevy::time::TimeUpdateStrategy>(),
        bevy::time::TimeUpdateStrategy::ManualDuration(d) if *d == pla::game::automation::FRAME_DT
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

fn send(stream: &mut UnixStream, reader: &mut BufReader<UnixStream>, line: &str) -> Value {
    stream.write_all(line.as_bytes()).unwrap();
    stream.write_all(b"\n").unwrap();
    let mut reply = String::new();
    reader.read_line(&mut reply).expect("reply");
    serde_json::from_str(&reply).unwrap_or_else(|e| panic!("bad reply {reply:?}: {e}"))
}

/// Headless integration: the real runner, a real client on the socket.
#[test]
fn socket_session_ping_step_state_quit() {
    let dir = scratch("session");
    let mut app = automation_app(&dir);
    let socket = dir.join(format!("{}.sock", std::process::id()));
    let client_socket = socket.clone();
    let client = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(300);
        let mut stream = loop {
            match UnixStream::connect(&client_socket) {
                Ok(s) => break s,
                Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => panic!("no socket after 300 s: {e}"),
            }
        };
        stream.set_read_timeout(Some(Duration::from_secs(300))).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut replies = Vec::new();
        for line in [
            r#"{"cmd":"ping"}"#,
            r#"{"cmd":"step","frames":3}"#,
            r#"{"cmd":"state"}"#,
            r#"{"cmd":"fly"}"#,
            r#"{"cmd":"screenshot","path":"x.png"}"#,
            r#"{"cmd":"quit"}"#,
        ] {
            replies.push(send(&mut stream, &mut reader, line));
        }
        replies
    });
    let exit = app.run();
    let replies = client.join().expect("client thread");
    assert_eq!(exit, AppExit::Success);
    let [ping, step, state, bad, shot, quit] = &replies[..] else { panic!("{replies:?}") };
    assert_eq!(ping["ok"], true, "{ping}");
    assert_eq!(ping["pong"], true);
    assert_eq!(ping["gpu"], false);
    let f0 = ping["frame"].as_u64().unwrap();
    assert_eq!(step["stepped"], 3, "{step}");
    assert_eq!(step["frame"].as_u64().unwrap(), f0 + 3, "frames advance only when asked");
    assert_eq!(state["ok"], true, "{state}");
    assert_eq!(state["game_state"], "World", "{state}");
    assert_eq!(state["frames"]["frame"].as_u64().unwrap(), f0 + 3);
    assert!(state["counts"].is_object(), "{state}");
    assert_eq!(bad["ok"], false);
    assert!(bad["error"].as_str().unwrap().contains("unknown command"), "{bad}");
    assert_eq!(shot["ok"], false);
    assert!(shot["error"].as_str().unwrap().starts_with("unavailable"), "{shot}");
    assert_eq!(quit["ok"], true);
    assert!(!socket.exists(), "the socket file is removed on quit");
    let _ = std::fs::remove_dir_all(&dir);
}
