//! Automation / remote-control mode of the viewer (`--automation`).
//!
//! Lets a script (or an AI that reads PNG files and runs shell commands) drive and observe the
//! real viewer **without any window**:
//!
//! - **No window, ever.** [`add_default_plugins`] installs `DefaultPlugins` with
//!   `primary_window: None`, `ExitCondition::DontExit`, no `WinitPlugin` (no event loop on the
//!   user's compositor) and no `GilrsPlugin`. The viewer's camera renders into an offscreen image
//!   (wgpu / Vulkan works without a display); when no GPU adapter is usable the app runs
//!   logic-only and `screenshot` answers `unavailable`.
//! - **Manual, deterministic frames.** [`AutomationPlugin`] replaces the app runner: frames only
//!   advance when a command asks for them, and every frame advances the clock by exactly 1/60 s
//!   (`TimeUpdateStrategy::ManualDuration`).
//! - **Control channel.** JSON lines over a Unix domain socket (`work/automation/<pid>.sock`,
//!   mode 0600, directory 0700). Never TCP. One JSON object per line in, one per line out.
//! - **Panic capture.** A panic hook writes message + backtrace to
//!   `work/automation/last_panic.txt`; a panic inside a command is answered on the socket with
//!   `{"ok":false,"error":"panic",...}` before the process exits with status 101.
//!
//! Commands are parsed by [`parse_command`] (pure, unit-tested) and run by the runner between
//! frames. See `docs/AUTOMATION.md` for the full list with examples.
//!
//! Keys are injected as real `KeyboardInput` messages, so they take the same path as a physical
//! keyboard (`ButtonInput<KeyCode>` -> `ActionState`).

use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bevy::app::PluginsState;
use bevy::camera::RenderTarget;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::time::TimeUpdateStrategy;
use bevy::transform::TransformSystems;
use serde_json::{json, Map, Value};

use super::battle_controller::{ActiveBattleSession, BattleAssets};
use super::clock::GameClock;
use super::navigation::GameNavmeshResource;
use super::pause_menu::{
    BagOpenRequested, BoxOpenRequested, DexOpenRequested, PartyOpenRequested, PauseMenu,
    PauseMenuSuppressed, ShopOpenRequested,
};
use super::save_game::{PartyMember, MAX_PARTY};
use super::save_wire::{PlayerProgress, SaveRequested, SaveToast};
use super::state::{GameState, Paused};
use super::trainer_encounter::{TrainerNpc, TrainerWorld};
use super::ui::ActiveZoneInfo;
use super::weather::{weather_id_from_arg, weather_name, WeatherState, WeatherVisual};
use super::wild::WildCreature;
use super::zone::{TerrainResource, ZoneChangeRequest, ZoneChangeSource, ZoneLinks, ZoneLoading};
use super::PlayerAvatar;

/// Command-line flag that turns the mode on (off by default).
pub const AUTOMATION_FLAG: &str = "--automation";
/// Fixed frame time of the manual loop (60 frames per game second).
pub const FRAME_DT: Duration = Duration::from_nanos(16_666_667);
/// Frames per game second, for `wait_seconds`.
pub const FRAMES_PER_SECOND: f32 = 60.0;
/// Upper bound of one `step` / `wait_seconds` command (about 28 game minutes).
pub const MAX_STEP_FRAMES: u32 = 100_000;
/// Default offscreen resolution.
pub const DEFAULT_SIZE: (u32, u32) = (1280, 720);
/// Largest accepted screenshot side.
pub const MAX_SIDE: u32 = 4096;

// ---------------------------------------------------------------- configuration

/// Everything the mode reads from the command line and the environment.
#[derive(Debug, Clone, PartialEq)]
pub struct AutomationConfig {
    /// Working directory of the mode (`work/automation`): socket, panic file, scratch save.
    pub dir: PathBuf,
    /// Control socket (`<dir>/<pid>.sock` unless `--automation-socket`).
    pub socket: PathBuf,
    /// A GPU was requested (`--automation-no-gpu` / `PLA_AUTOMATION_NO_GPU=1` turn it off).
    pub gpu: bool,
    /// Offscreen render size (`--automation-size WxH`).
    pub size: (u32, u32),
    /// File that receives the viewer's stdout/stderr (`--automation-log` / `PLA_AUTOMATION_LOG`),
    /// read by `log_tail`.
    pub log_path: Option<PathBuf>,
    /// Panic report (`<dir>/last_panic.txt`).
    pub panic_file: PathBuf,
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

/// `"WxH"` -> (w, h), both in 16..=[`MAX_SIDE`].
pub fn parse_size(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once(['x', 'X'])?;
    let (w, h) = (w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?);
    ((16..=MAX_SIDE).contains(&w) && (16..=MAX_SIDE).contains(&h)).then_some((w, h))
}

impl AutomationConfig {
    /// `None` unless `--automation` is present.
    pub fn from_args(args: &[String]) -> Option<Self> {
        if !args.iter().any(|a| a == AUTOMATION_FLAG) {
            return None;
        }
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let dir = flag_value(args, "--automation-dir")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("work/automation"));
        let socket = flag_value(args, "--automation-socket")
            .map(PathBuf::from)
            .unwrap_or_else(|| dir.join(format!("{}.sock", std::process::id())));
        let no_gpu = args.iter().any(|a| a == "--automation-no-gpu")
            || env("PLA_AUTOMATION_NO_GPU").is_some_and(|v| v != "0");
        let size = flag_value(args, "--automation-size").and_then(parse_size).unwrap_or(DEFAULT_SIZE);
        let log_path = flag_value(args, "--automation-log")
            .map(PathBuf::from)
            .or_else(|| env("PLA_AUTOMATION_LOG").map(PathBuf::from));
        let panic_file = dir.join("last_panic.txt");
        Some(Self { dir, socket, gpu: !no_gpu, size, log_path, panic_file })
    }

    /// Scratch save directory used unless the caller passed `--save-dir`.
    pub fn save_dir(&self) -> PathBuf {
        self.dir.join("save")
    }

    /// Settings directory of the mode (the player's `settings.txt` is never written).
    pub fn settings_dir(&self) -> PathBuf {
        self.dir.join("settings")
    }
}

/// Arguments the viewer should see in automation mode: without `--save-dir` the run uses a
/// scratch save (`<dir>/save`) and starts a new game, so the player's own save is never touched.
pub fn effective_args(args: Vec<String>, cfg: &AutomationConfig) -> Vec<String> {
    let mut out = args;
    if !out.iter().any(|a| a == "--save-dir") {
        out.push("--save-dir".into());
        out.push(cfg.save_dir().to_string_lossy().into_owned());
        if !out.iter().any(|a| a == "--new-game") {
            out.push("--new-game".into());
        }
    }
    out
}

/// Create `dir` (and parents) and restrict it to the current user (0700).
pub fn prepare_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

// ---------------------------------------------------------------- GPU probe and plugins

/// Whether a wgpu adapter is available without any window or display handle. `Ok(description)`
/// or `Err(reason)`.
pub fn probe_gpu() -> Result<String, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = bevy::tasks::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .map_err(|e| format!("no GPU adapter: {e}"))?;
    let info = adapter.get_info();
    Ok(format!("{} ({:?}, {:?})", info.name, info.backend, info.device_type))
}

/// Add `DefaultPlugins` for automation: no primary window, no winit event loop, no gamepad, and
/// (when `gpu` is false or no adapter is found) no renderer backend. Returns whether the GPU
/// renderer is active. `asset_root` is the asset directory (the viewer's `work/`).
pub fn add_default_plugins(app: &mut App, asset_root: String, cfg: &AutomationConfig) -> bool {
    let gpu = cfg.gpu
        && match probe_gpu() {
            Ok(desc) => {
                println!("[automation] offscreen GPU renderer: {desc}");
                true
            }
            Err(e) => {
                println!("[automation] {e}: logic-only mode (screenshots unavailable)");
                false
            }
        };
    if !cfg.gpu {
        println!("[automation] GPU disabled by request: logic-only mode (screenshots unavailable)");
    }
    let plugins = DefaultPlugins
        .set(WindowPlugin {
            primary_window: None,
            exit_condition: bevy::window::ExitCondition::DontExit,
            close_when_requested: false,
            ..default()
        })
        .set(AssetPlugin {
            file_path: asset_root,
            unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
            ..default()
        })
        .disable::<bevy::winit::WinitPlugin>()
        .disable::<bevy::gilrs::GilrsPlugin>();
    if gpu {
        app.add_plugins(plugins);
    } else {
        app.add_plugins(plugins.set(bevy::render::RenderPlugin {
            render_creation: bevy::render::settings::WgpuSettings { backends: None, ..default() }.into(),
            ..default()
        }));
        // Without a GPU the render plugin skips its entity-sync bookkeeping, whose resource the
        // component hooks of renderable components still need (same stub as the start-up guard).
        if !app.is_plugin_added::<bevy::render::sync_world::SyncWorldPlugin>() {
            app.add_plugins(bevy::render::sync_world::SyncWorldPlugin);
        }
    }
    gpu
}

// ---------------------------------------------------------------- Bevy side

/// Orbit placement of the main camera for one screenshot (radians / metres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    /// Point looked at; `None` = the player (1.2 m above its root).
    pub focus: Option<Vec3>,
}

/// Live state of the mode inside the app.
#[derive(Resource, Debug)]
pub struct AutomationState {
    pub gpu: bool,
    pub size: (u32, u32),
    /// Offscreen colour target of the main camera (GPU mode).
    pub target: Option<Handle<Image>>,
    /// Camera placement forced for the next frames (screenshots with `camera`).
    pub camera_override: Option<OrbitCamera>,
}

/// Marks a camera that was redirected to the offscreen target.
#[derive(Component)]
pub struct OffscreenTarget;

/// Marks a wild creature spawned by `start_battle {"wild": ...}`.
#[derive(Component)]
pub struct AutomationSpawned;

/// Installs the automation runner, the fixed frame time and (GPU mode) the offscreen target.
pub struct AutomationPlugin {
    pub config: AutomationConfig,
    /// Result of [`add_default_plugins`].
    pub gpu: bool,
}

impl Plugin for AutomationPlugin {
    fn build(&self, app: &mut App) {
        install_panic_hook(self.config.panic_file.clone());
        app.insert_resource(TimeUpdateStrategy::ManualDuration(FRAME_DT))
            .insert_resource(AutomationState {
                gpu: self.gpu,
                size: self.config.size,
                target: None,
                camera_override: None,
            })
            .add_systems(PostUpdate, apply_camera_override.before(TransformSystems::Propagate));
        if self.gpu {
            app.add_systems(PreUpdate, attach_offscreen_target);
        }
        let config = self.config.clone();
        app.set_runner(move |app| run(app, config));
    }
}

/// Redirect every 3D camera to the offscreen image and make it the UI camera (with no window,
/// Bevy UI would otherwise pick no camera at all).
fn attach_offscreen_target(
    mut commands: Commands,
    mut state: ResMut<AutomationState>,
    mut images: ResMut<Assets<Image>>,
    cameras: Query<Entity, (With<Camera3d>, Without<OffscreenTarget>)>,
) {
    if cameras.is_empty() {
        return;
    }
    let (w, h) = state.size;
    let target = state
        .target
        .get_or_insert_with(|| images.add(Image::new_target_texture(w, h, TextureFormat::Rgba8UnormSrgb, None)))
        .clone();
    for (i, e) in cameras.iter().enumerate() {
        let mut ec = commands.entity(e);
        ec.insert((RenderTarget::Image(target.clone().into()), OffscreenTarget));
        if i == 0 {
            ec.insert(IsDefaultUiCamera);
        }
    }
}

/// Orbit position of a camera around `focus`.
pub fn orbit_transform(o: &OrbitCamera, focus: Vec3) -> Transform {
    let offset = Vec3::new(
        o.distance * o.pitch.cos() * o.yaw.sin(),
        o.distance * o.pitch.sin(),
        o.distance * o.pitch.cos() * o.yaw.cos(),
    );
    Transform::from_translation(focus + offset).looking_at(focus, Vec3::Y)
}

fn apply_camera_override(
    state: Res<AutomationState>,
    player: Query<&Transform, (With<PlayerAvatar>, Without<Camera3d>)>,
    mut cameras: Query<&mut Transform, (With<Camera3d>, Without<PlayerAvatar>)>,
) {
    let Some(o) = state.camera_override else { return };
    let focus = o
        .focus
        .or_else(|| player.single().ok().map(|t| t.translation + Vec3::Y * 1.2))
        .unwrap_or(Vec3::ZERO);
    for mut t in &mut cameras {
        *t = orbit_transform(&o, focus);
    }
}

// ---------------------------------------------------------------- commands

/// What `key` does with the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// Down, hold `frames` frames, up, one more frame.
    Press,
    /// Down only (then `frames` frames).
    Down,
    /// Up only (then `frames` frames).
    Up,
}

/// Time of day for `set_time`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeSpec {
    Minutes(u16),
    Slot(usize),
}

/// Battle requested by `start_battle`.
#[derive(Debug, Clone, PartialEq)]
pub enum BattleSpec {
    /// A wild battle against a fresh creature of this species (battle-data name) and level.
    Wild { species: String, level: u8, alpha: bool },
    /// Walk up to the nearest overworld wild creature.
    NearestWild,
    /// A trainer (or `noble:<setting>`) by id; started through the real V key.
    Trainer(String),
}

/// Screens `open_screen` can open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenName {
    Pause,
    Bag,
    Party,
    Summary,
    Box,
    Dex,
    Crafting,
    Shop,
    Graphics,
    /// Close whatever is open (Escape, repeated).
    Close,
}

/// Entity groups `entities` can list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    Wild,
    Npc,
    Trainer,
    Player,
    Camera,
    /// Every entity with a `Name`.
    Named,
}

/// One parsed control command.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Ping,
    State,
    Step { frames: u32 },
    WaitSeconds { seconds: f32 },
    Key { key: KeyCode, action: KeyAction, frames: u32 },
    Teleport { x: f32, z: f32 },
    TeleportZone { area: u8, pos: Option<(f32, f32)> },
    SetTime { time: TimeSpec, pinned: Option<bool> },
    /// `None`: back to the weather lottery.
    SetWeather { weather: Option<u8> },
    GiveItem { id: u16, count: u16 },
    AddPokemon { species: String, level: u8 },
    StartBattle(BattleSpec),
    /// Make wild creatures alpha (debug), see `game::alpha_aura`.
    ForceAlpha(super::alpha_aura::ForceAlpha),
    OpenScreen(ScreenName),
    Save,
    Load,
    Screenshot { path: PathBuf, size: Option<(u32, u32)>, camera: Option<OrbitCamera> },
    Entities { kind: EntityKind, limit: usize },
    LogTail { lines: usize },
    Quit,
}

/// Key name -> key code: letters (`"V"`), digits (`"1"`), `F1`..`F12`, named keys (`Escape`/`Esc`,
/// `Enter`, `Space`, `Tab`, `Shift`, `Ctrl`, `Alt`, `Backspace`, `Up`/`Down`/`Left`/`Right` or
/// `ArrowUp`...), and Bevy's own names (`KeyV`, `Digit1`, `ShiftLeft`). Case-insensitive.
pub fn parse_key(name: &str) -> Option<KeyCode> {
    use KeyCode as K;
    let n = name.trim();
    let lower = n.to_ascii_lowercase();
    let letter = |c: char| -> Option<KeyCode> {
        const LETTERS: [KeyCode; 26] = [
            K::KeyA, K::KeyB, K::KeyC, K::KeyD, K::KeyE, K::KeyF, K::KeyG, K::KeyH, K::KeyI, K::KeyJ, K::KeyK, K::KeyL, K::KeyM,
            K::KeyN, K::KeyO, K::KeyP, K::KeyQ, K::KeyR, K::KeyS, K::KeyT, K::KeyU, K::KeyV, K::KeyW, K::KeyX, K::KeyY, K::KeyZ,
        ];
        c.is_ascii_alphabetic().then(|| LETTERS[(c.to_ascii_lowercase() as u8 - b'a') as usize])
    };
    let digit = |c: char| -> Option<KeyCode> {
        const DIGITS: [KeyCode; 10] =
            [K::Digit0, K::Digit1, K::Digit2, K::Digit3, K::Digit4, K::Digit5, K::Digit6, K::Digit7, K::Digit8, K::Digit9];
        c.to_digit(10).map(|d| DIGITS[d as usize])
    };
    let single = |s: &str| -> Option<char> {
        let mut it = s.chars();
        let c = it.next()?;
        it.next().is_none().then_some(c)
    };
    if let Some(c) = single(n) {
        return letter(c).or_else(|| digit(c));
    }
    if let Some(rest) = lower.strip_prefix("key") {
        if let Some(c) = single(rest) {
            return letter(c);
        }
    }
    if let Some(rest) = lower.strip_prefix("digit") {
        if let Some(c) = single(rest) {
            return digit(c);
        }
    }
    if let Some(rest) = lower.strip_prefix('f') {
        if let Ok(i) = rest.parse::<usize>() {
            const FS: [KeyCode; 12] =
                [K::F1, K::F2, K::F3, K::F4, K::F5, K::F6, K::F7, K::F8, K::F9, K::F10, K::F11, K::F12];
            return (1..=12).contains(&i).then(|| FS[i - 1]);
        }
    }
    Some(match lower.as_str() {
        "escape" | "esc" => K::Escape,
        "enter" | "return" => K::Enter,
        "numpadenter" => K::NumpadEnter,
        "space" => K::Space,
        "tab" => K::Tab,
        "backspace" => K::Backspace,
        "shift" | "shiftleft" | "lshift" => K::ShiftLeft,
        "shiftright" | "rshift" => K::ShiftRight,
        "ctrl" | "control" | "controlleft" | "lctrl" => K::ControlLeft,
        "controlright" | "rctrl" => K::ControlRight,
        "alt" | "altleft" => K::AltLeft,
        "up" | "arrowup" => K::ArrowUp,
        "down" | "arrowdown" => K::ArrowDown,
        "left" | "arrowleft" => K::ArrowLeft,
        "right" | "arrowright" => K::ArrowRight,
        _ => return None,
    })
}

fn get_f32(o: &Map<String, Value>, k: &str) -> Result<Option<f32>, String> {
    match o.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|f| f.is_finite())
            .map(|f| Some(f as f32))
            .ok_or_else(|| format!("'{k}' must be a finite number")),
    }
}

fn get_u64(o: &Map<String, Value>, k: &str) -> Result<Option<u64>, String> {
    match o.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_u64().map(Some).ok_or_else(|| format!("'{k}' must be a non-negative integer")),
    }
}

fn get_str<'a>(o: &'a Map<String, Value>, k: &str) -> Result<Option<&'a str>, String> {
    match o.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_str().map(Some).ok_or_else(|| format!("'{k}' must be a string")),
    }
}

fn need<T>(v: Option<T>, k: &str) -> Result<T, String> {
    v.ok_or_else(|| format!("missing '{k}'"))
}

fn bounded<T: TryFrom<u64>>(v: u64, k: &str, max: u64) -> Result<T, String> {
    if v > max {
        return Err(format!("'{k}' must be <= {max}"));
    }
    T::try_from(v).map_err(|_| format!("'{k}' out of range"))
}

fn parse_camera(v: &Value) -> Result<OrbitCamera, String> {
    let o = v.as_object().ok_or("'camera' must be an object {yaw, pitch, distance, focus?}")?;
    let deg = o.get("degrees").and_then(Value::as_bool).unwrap_or(false);
    let ang = |x: f32| if deg { x.to_radians() } else { x };
    let focus = match o.get("focus") {
        None | Some(Value::Null) => None,
        Some(Value::Array(a)) if a.len() == 3 => {
            let f: Vec<f32> = a.iter().filter_map(Value::as_f64).map(|x| x as f32).collect();
            if f.len() != 3 {
                return Err("'camera.focus' must be [x, y, z]".into());
            }
            Some(Vec3::new(f[0], f[1], f[2]))
        }
        Some(_) => return Err("'camera.focus' must be [x, y, z]".into()),
    };
    let distance = get_f32(o, "distance")?.unwrap_or(5.0);
    if !(0.1..=2000.0).contains(&distance) {
        return Err("'camera.distance' must be in 0.1..=2000".into());
    }
    Ok(OrbitCamera {
        yaw: ang(get_f32(o, "yaw")?.unwrap_or(0.0)),
        pitch: ang(get_f32(o, "pitch")?.unwrap_or(0.35)),
        distance,
        focus,
    })
}

/// Parse one JSON line into a [`Command`]. Unknown fields are ignored; bad values are errors.
pub fn parse_command(line: &str) -> Result<Command, String> {
    let v: Value = serde_json::from_str(line.trim()).map_err(|e| format!("invalid JSON: {e}"))?;
    let o = v.as_object().ok_or("a command is a JSON object with a \"cmd\" field")?;
    let cmd = need(get_str(o, "cmd")?, "cmd")?;
    Ok(match cmd {
        "ping" => Command::Ping,
        "state" => Command::State,
        "step" => Command::Step {
            frames: bounded(get_u64(o, "frames")?.or(get_u64(o, "n")?).unwrap_or(1), "frames", MAX_STEP_FRAMES.into())?,
        },
        "wait_seconds" => {
            let s = need(get_f32(o, "seconds")?, "seconds")?;
            if !(0.0..=MAX_STEP_FRAMES as f32 / FRAMES_PER_SECOND).contains(&s) {
                return Err(format!("'seconds' must be in 0..={}", MAX_STEP_FRAMES as f32 / FRAMES_PER_SECOND));
            }
            Command::WaitSeconds { seconds: s }
        }
        "key" => {
            let name = need(get_str(o, "key")?, "key")?;
            let key = parse_key(name).ok_or_else(|| format!("unknown key '{name}'"))?;
            let action = match get_str(o, "action")?.unwrap_or("press") {
                "press" => KeyAction::Press,
                "down" => KeyAction::Down,
                "up" => KeyAction::Up,
                a => return Err(format!("unknown key action '{a}' (press|down|up)")),
            };
            let frames = bounded(get_u64(o, "frames")?.unwrap_or(1), "frames", 10_000)?;
            Command::Key { key, action, frames }
        }
        "teleport" => Command::Teleport { x: need(get_f32(o, "x")?, "x")?, z: need(get_f32(o, "z")?, "z")? },
        "teleport_zone" => {
            let area = bounded(need(get_u64(o, "area")?, "area")?, "area", 99)?;
            let pos = match (get_f32(o, "x")?, get_f32(o, "z")?) {
                (Some(x), Some(z)) => Some((x, z)),
                (None, None) => None,
                _ => return Err("give both 'x' and 'z' or neither".into()),
            };
            Command::TeleportZone { area, pos }
        }
        "set_time" => {
            let time = match (get_u64(o, "minutes")?, get_u64(o, "slot")?) {
                (Some(m), None) => TimeSpec::Minutes(bounded(m, "minutes", 1439)?),
                (None, Some(s)) => TimeSpec::Slot(bounded(s, "slot", 3)?),
                _ => return Err("give exactly one of 'minutes' (0..1439) or 'slot' (0..3)".into()),
            };
            let pinned = match o.get("pinned") {
                None | Some(Value::Null) => None,
                Some(v) => Some(v.as_bool().ok_or("'pinned' must be true or false")?),
            };
            Command::SetTime { time, pinned }
        }
        "set_weather" => {
            let w = match o.get("weather") {
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::String(s)) => s.clone(),
                _ => return Err("missing 'weather' (2..9, a name such as \"rain\", or \"auto\")".into()),
            };
            if w.eq_ignore_ascii_case("auto") {
                Command::SetWeather { weather: None }
            } else {
                Command::SetWeather {
                    weather: Some(weather_id_from_arg(&w).ok_or_else(|| format!("unknown weather '{w}'"))?),
                }
            }
        }
        "give_item" => Command::GiveItem {
            id: bounded(need(get_u64(o, "id")?, "id")?, "id", u16::MAX.into())?,
            count: bounded(get_u64(o, "count")?.unwrap_or(1), "count", 999)?,
        },
        "add_pokemon" => Command::AddPokemon {
            species: need(get_str(o, "species")?, "species")?.to_ascii_lowercase(),
            level: bounded(get_u64(o, "level")?.unwrap_or(5), "level", 100)?,
        },
        "start_battle" => match (get_str(o, "wild")?, get_str(o, "trainer")?) {
            (Some(w), None) if w.eq_ignore_ascii_case("nearest") => Command::StartBattle(BattleSpec::NearestWild),
            (Some(w), None) => Command::StartBattle(BattleSpec::Wild {
                species: w.to_ascii_lowercase(),
                level: bounded(get_u64(o, "level")?.unwrap_or(5), "level", 100)?,
                alpha: match o.get("alpha") {
                    None | Some(Value::Null) => false,
                    Some(v) => v.as_bool().ok_or("'alpha' must be true or false")?,
                },
            }),
            (None, Some(t)) => Command::StartBattle(BattleSpec::Trainer(t.to_string())),
            _ => return Err("give exactly one of 'wild' (species name or \"nearest\") or 'trainer' (id)".into()),
        },
        "force_alpha" => {
            // {"count": N} | {"all": true} | {"off": true}; nothing given = the nearest one.
            let force = match (o.get("count"), o.get("all"), o.get("off")) {
                (None, None, None) => super::alpha_aura::ForceAlpha::Nearest(1),
                (Some(_), None, None) => {
                    super::alpha_aura::ForceAlpha::Nearest(bounded::<u32>(need(get_u64(o, "count")?, "count")?, "count", 1000)?)
                }
                (None, Some(v), None) if v.as_bool() == Some(true) => super::alpha_aura::ForceAlpha::All,
                (None, None, Some(v)) if v.as_bool() == Some(true) => super::alpha_aura::ForceAlpha::Off,
                _ => return Err("give one of 'count' (N nearest), 'all': true or 'off': true".into()),
            };
            Command::ForceAlpha(force)
        }
        "open_screen" => {
            let s = need(get_str(o, "screen")?, "screen")?;
            Command::OpenScreen(match s.to_ascii_lowercase().as_str() {
                "pause" | "menu" => ScreenName::Pause,
                "bag" => ScreenName::Bag,
                "party" => ScreenName::Party,
                "summary" => ScreenName::Summary,
                "box" | "boxes" => ScreenName::Box,
                "dex" | "pokedex" => ScreenName::Dex,
                "crafting" => ScreenName::Crafting,
                "shop" => ScreenName::Shop,
                "graphics" => ScreenName::Graphics,
                "close" | "none" => ScreenName::Close,
                _ => return Err(format!("unknown screen '{s}' (pause|bag|party|summary|box|dex|crafting|shop|graphics|close)")),
            })
        }
        "save" => Command::Save,
        "load" => Command::Load,
        "screenshot" => {
            let path = PathBuf::from(need(get_str(o, "path")?, "path")?);
            if path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() != Some("png") {
                return Err("'path' must end in .png".into());
            }
            let size = match (get_u64(o, "width")?, get_u64(o, "height")?) {
                (Some(w), Some(h)) => {
                    let (w, h): (u32, u32) = (bounded(w, "width", MAX_SIDE.into())?, bounded(h, "height", MAX_SIDE.into())?);
                    if w < 16 || h < 16 {
                        return Err("'width'/'height' must be >= 16".into());
                    }
                    Some((w, h))
                }
                (None, None) => None,
                _ => return Err("give both 'width' and 'height' or neither".into()),
            };
            let camera = match o.get("camera") {
                None | Some(Value::Null) => None,
                Some(c) => Some(parse_camera(c)?),
            };
            Command::Screenshot { path, size, camera }
        }
        "entities" => {
            let f = get_str(o, "filter")?.or(get_str(o, "kind")?).unwrap_or("wild");
            Command::Entities {
                kind: match f.to_ascii_lowercase().as_str() {
                    "wild" | "pokemon" => EntityKind::Wild,
                    "npc" | "npcs" => EntityKind::Npc,
                    "trainer" | "trainers" => EntityKind::Trainer,
                    "player" => EntityKind::Player,
                    "camera" => EntityKind::Camera,
                    "named" | "name" | "all" => EntityKind::Named,
                    _ => return Err(format!("unknown filter '{f}' (wild|npc|trainer|player|camera|named)")),
                },
                limit: bounded(get_u64(o, "limit")?.unwrap_or(50), "limit", 1000)?,
            }
        }
        "log_tail" => Command::LogTail {
            lines: bounded(get_u64(o, "lines")?.or(get_u64(o, "n")?).unwrap_or(50), "lines", 5000)?,
        },
        "quit" | "exit" => Command::Quit,
        other => return Err(format!("unknown command '{other}'")),
    })
}

// ---------------------------------------------------------------- runner

/// Text of the last panic seen by the hook (also written to the panic file).
static LAST_PANIC: Mutex<Option<String>> = Mutex::new(None);

/// Panic hook: message + backtrace to `path` (and to [`LAST_PANIC`]), then the previous hook.
pub fn install_panic_hook(path: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        let thread = std::thread::current();
        let text = format!(
            "pid {}\nthread '{}'\n{}\n\nbacktrace:\n{}",
            std::process::id(),
            thread.name().unwrap_or("<unnamed>"),
            info,
            backtrace
        );
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, &text);
        if let Ok(mut p) = LAST_PANIC.lock() {
            *p = Some(text);
        }
        previous(info);
    }));
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else {
        "non-text panic".into()
    }
}

/// Removes the socket file when the runner leaves (normal exit or unwinding).
struct SocketGuard(PathBuf);

impl Drop for SocketGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// One client connection with its pending input.
struct Conn {
    stream: UnixStream,
    buf: Vec<u8>,
    closed: bool,
}

/// Non-blocking Unix socket server: one JSON line in, one JSON line out.
struct Server {
    listener: UnixListener,
    conns: Vec<Conn>,
}

impl Server {
    fn bind(path: &Path) -> std::io::Result<Self> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            prepare_private_dir(dir)?;
        }
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        let listener = UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        Ok(Self { listener, conns: Vec::new() })
    }

    /// Next complete line from any client, or `None` when nothing is pending.
    fn poll(&mut self) -> Option<(usize, String)> {
        while let Ok((stream, _)) = self.listener.accept() {
            if stream.set_nonblocking(true).is_ok() {
                self.conns.push(Conn { stream, buf: Vec::new(), closed: false });
            }
        }
        for (i, c) in self.conns.iter_mut().enumerate() {
            let mut chunk = [0u8; 4096];
            loop {
                match c.stream.read(&mut chunk) {
                    Ok(0) => {
                        c.closed = true;
                        break;
                    }
                    Ok(n) => {
                        c.buf.extend_from_slice(&chunk[..n]);
                        if c.buf.len() > 1 << 20 {
                            c.closed = true; // a single command never needs a megabyte
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => {
                        c.closed = true;
                        break;
                    }
                }
            }
            if let Some(pos) = c.buf.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = c.buf.drain(..=pos).collect();
                return Some((i, String::from_utf8_lossy(&line).trim().to_string()));
            }
        }
        self.conns.retain(|c| !c.closed || !c.buf.is_empty());
        None
    }

    fn reply(&mut self, i: usize, v: &Value) {
        if let Some(c) = self.conns.get_mut(i) {
            let _ = c.stream.set_nonblocking(false);
            let _ = c.stream.set_write_timeout(Some(Duration::from_secs(10)));
            let mut line = v.to_string();
            line.push('\n');
            let _ = c.stream.write_all(line.as_bytes());
            let _ = c.stream.flush();
            let _ = c.stream.set_nonblocking(true);
        }
    }
}

/// Book-keeping of the runner shared by the commands.
struct Ctx {
    cfg: AutomationConfig,
    frame: u64,
    started: Instant,
    /// Real milliseconds of the last updates (ring of 120).
    update_ms: std::collections::VecDeque<f32>,
}

impl Ctx {
    fn step(&mut self, app: &mut App, frames: u32) -> u32 {
        let mut done = 0;
        for _ in 0..frames {
            let t = Instant::now();
            app.update();
            self.frame += 1;
            done += 1;
            if self.update_ms.len() == 120 {
                self.update_ms.pop_front();
            }
            self.update_ms.push_back(t.elapsed().as_secs_f32() * 1000.0);
            if app.should_exit().is_some() {
                break;
            }
        }
        done
    }

    /// Step until `done(world)` or `max` frames; returns the frames run and whether it held.
    fn step_until(&mut self, app: &mut App, max: u32, done: impl Fn(&mut World) -> bool) -> (u32, bool) {
        let mut n = 0;
        while n < max {
            if done(app.world_mut()) {
                return (n, true);
            }
            n += self.step(app, 1);
            if app.should_exit().is_some() {
                break;
            }
        }
        (n, done(app.world_mut()))
    }
}

fn run(mut app: App, cfg: AutomationConfig) -> AppExit {
    if app.plugins_state() != PluginsState::Cleaned {
        while app.plugins_state() == PluginsState::Adding {
            bevy::tasks::tick_global_task_pools_on_main_thread();
        }
        app.finish();
        app.cleanup();
    }
    let mut ctx = Ctx { cfg: cfg.clone(), frame: 0, started: Instant::now(), update_ms: Default::default() };
    // First frame: Startup systems, so `state` is meaningful from the first command.
    if let Err(p) = catch_unwind(AssertUnwindSafe(|| ctx.step(&mut app, 1))) {
        eprintln!("[automation] panic in the first frame: {}", panic_text(&*p));
        std::process::exit(101);
    }
    let mut server = match Server::bind(&cfg.socket) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[automation] cannot create the control socket {}: {e}", cfg.socket.display());
            return AppExit::error();
        }
    };
    let _guard = SocketGuard(cfg.socket.clone());
    println!("[automation] listening on {} (JSON lines; see docs/AUTOMATION.md)", cfg.socket.display());
    loop {
        if let Some(exit) = app.should_exit() {
            println!("[automation] the app asked to exit");
            return exit;
        }
        let Some((client, line)) = server.poll() else {
            std::thread::sleep(Duration::from_millis(3));
            continue;
        };
        if line.is_empty() {
            continue;
        }
        let cmd = match parse_command(&line) {
            Ok(c) => c,
            Err(e) => {
                server.reply(client, &json!({"ok": false, "error": e}));
                continue;
            }
        };
        let quit = cmd == Command::Quit;
        match catch_unwind(AssertUnwindSafe(|| dispatch(&mut app, &mut ctx, &cmd))) {
            Ok(Ok(mut v)) => {
                if let Some(o) = v.as_object_mut() {
                    o.insert("ok".into(), json!(true));
                    o.insert("frame".into(), json!(ctx.frame));
                }
                server.reply(client, &v);
            }
            Ok(Err(e)) => server.reply(client, &json!({"ok": false, "error": e, "frame": ctx.frame})),
            Err(p) => {
                let text = LAST_PANIC.lock().ok().and_then(|g| g.clone()).unwrap_or_else(|| panic_text(&*p));
                server.reply(
                    client,
                    &json!({
                        "ok": false,
                        "error": "panic",
                        "panic": text,
                        "panic_file": cfg.panic_file.to_string_lossy(),
                        "frame": ctx.frame,
                    }),
                );
                eprintln!("[automation] the viewer panicked; see {}", cfg.panic_file.display());
                drop(_guard);
                std::process::exit(101);
            }
        }
        if quit {
            println!("[automation] quit requested");
            return AppExit::Success;
        }
        if app.should_exit().is_some() {
            return app.should_exit().unwrap_or(AppExit::Success);
        }
    }
}

// ---------------------------------------------------------------- dispatch

fn dispatch(app: &mut App, ctx: &mut Ctx, cmd: &Command) -> Result<Value, String> {
    match cmd {
        Command::Ping => Ok(json!({
            "pong": true,
            "pid": std::process::id(),
            "gpu": app.world().resource::<AutomationState>().gpu,
            "uptime_s": ctx.started.elapsed().as_secs_f32(),
        })),
        Command::State => Ok(state_json(app, ctx)),
        Command::Step { frames } => {
            let n = ctx.step(app, *frames);
            Ok(json!({"stepped": n}))
        }
        Command::WaitSeconds { seconds } => {
            let n = ctx.step(app, (seconds * FRAMES_PER_SECOND).round() as u32);
            Ok(json!({"stepped": n, "game_seconds": n as f32 / FRAMES_PER_SECOND}))
        }
        Command::Key { key, action, frames } => {
            let mut n = 0;
            match action {
                KeyAction::Press => {
                    send_key(app.world_mut(), *key, ButtonState::Pressed);
                    n += ctx.step(app, (*frames).max(1));
                    send_key(app.world_mut(), *key, ButtonState::Released);
                    n += ctx.step(app, 1);
                }
                KeyAction::Down => {
                    send_key(app.world_mut(), *key, ButtonState::Pressed);
                    n += ctx.step(app, *frames);
                }
                KeyAction::Up => {
                    send_key(app.world_mut(), *key, ButtonState::Released);
                    n += ctx.step(app, *frames);
                }
            }
            Ok(json!({"key": format!("{key:?}"), "stepped": n}))
        }
        Command::Teleport { x, z } => {
            let p = teleport(app.world_mut(), *x, *z)?;
            Ok(json!({"position": p}))
        }
        Command::TeleportZone { area, pos } => teleport_zone(app, ctx, *area, *pos),
        Command::SetTime { time, pinned } => {
            let world = app.world_mut();
            let mut clock = world.get_resource_mut::<GameClock>().ok_or("no game clock in this app")?;
            clock.t_hours = match time {
                TimeSpec::Minutes(m) => f32::from(*m) / 60.0,
                TimeSpec::Slot(s) => super::clock::slot_centre_hours(*s),
            };
            if let Some(p) = pinned {
                clock.pinned = *p;
            }
            Ok(json!({"time": clock.hhmm(), "slot": clock.slot(), "pinned": clock.pinned}))
        }
        Command::SetWeather { weather } => {
            let world = app.world_mut();
            let mut ws = world.get_resource_mut::<WeatherState>().ok_or("no weather in this app")?;
            ws.force = *weather;
            if let Some(id) = weather {
                ws.machine.id = *id;
            }
            let id = ws.machine.id;
            world.insert_resource(WeatherVisual::for_id(id));
            Ok(json!({"weather": weather_name(id), "id": id, "forced": weather.is_some()}))
        }
        Command::GiveItem { id, count } => {
            let world = app.world_mut();
            let mut p = world.get_resource_mut::<PlayerProgress>().ok_or("no player progress in this app")?;
            let e = p.bag.entry(*id).or_insert(0);
            *e = e.saturating_add(*count);
            Ok(json!({"item": id, "count": *e}))
        }
        Command::AddPokemon { species, level } => add_pokemon(app.world_mut(), species, *level),
        Command::StartBattle(spec) => start_battle(app, ctx, spec),
        Command::ForceAlpha(f) => {
            app.world_mut().insert_resource(*f);
            // The aura systems run on the next frames; the reply says what was asked.
            Ok(json!({"force_alpha": format!("{f:?}")}))
        }
        Command::OpenScreen(s) => open_screen(app, ctx, *s),
        Command::Save => save(app, ctx),
        Command::Load => load(app, ctx),
        Command::Screenshot { path, size, camera } => screenshot(app, ctx, path, *size, *camera),
        Command::Entities { kind, limit } => Ok(entities_json(app.world_mut(), *kind, *limit)),
        Command::LogTail { lines } => log_tail(&ctx.cfg, *lines),
        Command::Quit => Ok(json!({"bye": true})),
    }
}

/// Queue a key edge as a real keyboard message (read by `keyboard_input_system` next frame).
fn send_key(world: &mut World, key: KeyCode, state: ButtonState) {
    world.write_message(KeyboardInput {
        key_code: key,
        logical_key: Key::Unidentified(NativeKey::Unidentified),
        state,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}

fn player_transform(world: &mut World) -> Option<Transform> {
    let mut q = world.query_filtered::<&Transform, With<PlayerAvatar>>();
    q.iter(world).next().copied()
}

fn round2(v: f32) -> f64 {
    (f64::from(v) * 100.0).round() / 100.0
}

fn v3(v: Vec3) -> Value {
    json!([round2(v.x), round2(v.y), round2(v.z)])
}

fn yaw_of(t: &Transform) -> f32 {
    t.rotation.to_euler(EulerRot::YXZ).0
}

/// Ground height under (x, z): navmesh, else terrain.
fn ground_at(world: &World, x: f32, z: f32) -> Option<f32> {
    world
        .get_resource::<GameNavmeshResource>()
        .and_then(|n| n.height_at(x, z))
        .or_else(|| world.get_resource::<TerrainResource>().and_then(|t| t.area.sample_world_height(x, z)))
}

/// Snap (x, z) to the walkable navmesh (nearest face centre when off it) and move the player there.
fn teleport(world: &mut World, x: f32, z: f32) -> Result<[f64; 3], String> {
    let (mut tx, mut tz) = (x, z);
    let mut snapped = false;
    if let Some(nav) = world.get_resource::<GameNavmeshResource>() {
        if !nav.is_walkable(x, z) {
            let hit = nav.nearest(x, z, 400.0).ok_or("no walkable navmesh within 400 m")?;
            let c = super::spawn_point::face_centroid(nav.nav.primary(), hit.face);
            (tx, tz) = (c[0], c[2]);
            snapped = true;
        }
    }
    let ground = ground_at(world, tx, tz);
    let mut q = world.query_filtered::<&mut Transform, With<PlayerAvatar>>();
    let mut t = q.iter_mut(world).next().ok_or("no player in the world")?;
    t.translation.x = tx;
    t.translation.z = tz;
    if let Some(g) = ground {
        t.translation.y = g + 0.9;
    }
    if snapped {
        println!("[automation] teleport ({x:.1}, {z:.1}) was off the navmesh: snapped to ({tx:.1}, {tz:.1})");
    }
    Ok([round2(tx), round2(t.translation.y), round2(tz)])
}

fn state_name(world: &World) -> String {
    world.get_resource::<State<GameState>>().map(|s| format!("{:?}", s.get())).unwrap_or_else(|| "?".into())
}

fn teleport_zone(app: &mut App, ctx: &mut Ctx, area: u8, pos: Option<(f32, f32)>) -> Result<Value, String> {
    let zone = format!("ha_area{area:02}");
    let world = app.world_mut();
    if state_name(world) != "World" {
        return Err(format!("zone changes start only in the World state (now {})", state_name(world)));
    }
    let spawn = match pos {
        Some((x, z)) => Vec3::new(x, 0.0, z),
        None => world
            .get_resource::<ZoneLinks>()
            .and_then(|l| l.fast_travel_to(&zone))
            .map(|l| l.arrival)
            .ok_or_else(|| format!("no fast-travel point decoded for {zone}; pass x and z"))?,
    };
    world.write_message(ZoneChangeRequest {
        zone: zone.clone(),
        spawn,
        yaw_deg: 0.0,
        source: ZoneChangeSource::Debug,
        link: Some("automation".into()),
    });
    let (n, _) = ctx.step_until(app, 4, |w| !w.resource::<ZoneLoading>().is_idle());
    let (m, done) = ctx.step_until(app, 3600, |w| {
        w.resource::<ZoneLoading>().is_idle() && state_name(w) == "World"
    });
    let world = app.world_mut();
    let now = world.get_resource::<ActiveZoneInfo>().map(|z| z.area.clone()).unwrap_or_default();
    let p = player_transform(world).map(|t| v3(t.translation));
    if !done || now != zone {
        return Err(format!("zone change to {zone} did not finish after {} frames (zone now {now})", n + m));
    }
    Ok(json!({"zone": now, "stepped": n + m, "player": p}))
}

fn assets(world: &World) -> Result<&BattleAssets, String> {
    world.get_resource::<BattleAssets>().ok_or_else(|| "battle tables (data/) not loaded".to_string())
}

fn add_pokemon(world: &mut World, species: &str, level: u8) -> Result<Value, String> {
    let level = level.max(1);
    let member = {
        let a = assets(world)?;
        let s = ActiveBattleSession::start(a, species, level, "bidoof", 5, 399, Entity::PLACEHOLDER)?;
        let lead = s.battle.sides[0][0];
        PartyMember::from_combatant(&s.battle.combatants[lead])
    };
    let mut p = world.get_resource_mut::<PlayerProgress>().ok_or("no player progress in this app")?;
    let dex = member.species;
    let place = if p.party.len() < MAX_PARTY {
        p.party.push(member);
        "party"
    } else {
        p.box_list.push(member);
        "box"
    };
    Ok(json!({"species": species, "dex": dex, "level": level, "placed_in": place, "party_size": p.party.len()}))
}

fn in_battle(w: &mut World) -> bool {
    state_name(w) == "Battle" && w.contains_resource::<ActiveBattleSession>()
}

fn battle_json(world: &World) -> Value {
    let Some(s) = world.get_resource::<ActiveBattleSession>() else { return Value::Null };
    let side = |i: usize| -> Vec<Value> {
        s.battle.sides.get(i).map_or(Vec::new(), |idx| {
            idx.iter()
                .filter_map(|&c| s.battle.combatants.get(c))
                .map(|c| json!({"species": c.species.name, "level": c.level, "hp": c.hp, "max_hp": c.max_hp}))
                .collect()
        })
    };
    json!({
        "turn": s.current_turn,
        "trainer": s.trainer.as_ref().map(|t| t.id.clone()),
        "trainer_label": s.trainer_label(),
        "wild_dex": s.wild_dex,
        "ended": s.battle.ended,
        "menu": format!("{:?}", s.menu_state),
        "player_side": side(0),
        "opponent_side": side(1),
        "summary": s.battle_summary,
        "last_log": s.log_lines.iter().rev().take(4).rev().collect::<Vec<_>>(),
    })
}

fn start_battle(app: &mut App, ctx: &mut Ctx, spec: &BattleSpec) -> Result<Value, String> {
    let world = app.world_mut();
    if world.contains_resource::<ActiveBattleSession>() {
        return Err("a battle is already running".into());
    }
    if state_name(world) != "World" {
        return Err(format!("battles start only in the World state (now {})", state_name(world)));
    }
    if world.get_resource::<Paused>().is_some_and(|p| p.0) {
        return Err("the game is paused (close the open screen first)".into());
    }
    let player = player_transform(world).ok_or("no player in the world")?;
    let via: String;
    let mut pressed_e = false;
    match spec {
        BattleSpec::Trainer(id) => {
            {
                let a = assets(world)?;
                let tw = world.get_resource::<TrainerWorld>().ok_or("trainer sheets not loaded")?;
                // Validate first, so a typo is an error instead of a silent no-op.
                super::trainer_encounter::start_by_id(tw, a, id, Entity::PLACEHOLDER)?;
            }
            world.resource_mut::<TrainerWorld>().forced = Some(id.clone());
            send_key(world, KeyCode::KeyV, ButtonState::Pressed);
            via = "V key (forced trainer; a trainer marker within 3 m takes priority)".into();
        }
        BattleSpec::Wild { species, level, alpha } => {
            let dex = assets(world)?
                .data
                .species_row(species)?
                .map(|r| r.dex)
                .ok_or_else(|| format!("unknown species '{species}'"))?;
            let fwd = player.rotation * Vec3::Z;
            let pos = player.translation + Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::Z) * 1.5;
            let level = (*level).max(1);
            let mut entity_mut = world.spawn((
                    WildCreature {
                        species_name: species.clone(),
                        national_dex: dex,
                        level,
                        spawn_pos: pos,
                        aggro_range: 2.8,
                        in_battle: true,
                        defeated: false,
                        spawner_id: "automation".into(),
                    },
                    AutomationSpawned,
                    Transform::from_translation(pos),
                    Visibility::default(),
                    Name::new(format!("automation wild {species}")),
                ));
            if *alpha {
                // The arena shows the opponent with the alpha aura (see `setup_battle_arena`).
                entity_mut.insert(super::alpha_aura::AlphaAura::default());
            }
            let entity = entity_mut.id();
            // The same call the viewer's encounter system makes (its lead is the demo Pikachu; the
            // saved party replaces it when the battle starts). Started directly so a creature
            // already in aggro range cannot take the battle first.
            let session = ActiveBattleSession::start(
                assets(world)?,
                "pikachu",
                super::save_wire::DEFAULT_LEAD_LEVEL,
                species,
                level,
                dex,
                entity,
            );
            let session = match session {
                Ok(s) => s,
                Err(e) => {
                    world.despawn(entity);
                    return Err(format!("cannot start a battle against {species}: {e}"));
                }
            };
            world.insert_resource(session);
            world.resource_mut::<NextState<GameState>>().set(GameState::Battle);
            via = format!("wild {species} (#{dex}) spawned 1.5 m from the player; session started directly");
        }
        BattleSpec::NearestWild => {
            let mut q = world.query::<(&Transform, &WildCreature)>();
            let target = q
                .iter(world)
                .filter(|(_, w)| !w.defeated && !w.in_battle)
                .min_by(|a, b| {
                    a.0.translation.distance(player.translation).total_cmp(&b.0.translation.distance(player.translation))
                })
                .map(|(t, w)| (t.translation, w.species_name.clone()));
            let (pos, name) = target.ok_or("no wild creature loaded near the player")?;
            let mut pq = world.query_filtered::<&mut Transform, With<PlayerAvatar>>();
            if let Some(mut t) = pq.iter_mut(world).next() {
                t.translation.x = pos.x + 1.0;
                t.translation.z = pos.z;
                // The encounter distance is 3D: stand at the creature's height too.
                t.translation.y = pos.y;
            }
            // The creature is idle and the arrival grace would block the encounter: the command
            // ends the grace and presses E (the manual encounter), exactly like a player would.
            if let Some(mut safety) = world.get_resource_mut::<super::wild::EncounterSafety>() {
                safety.clear_grace_at(bevy::math::Vec2::new(pos.x + 1.0, pos.z));
            }
            send_key(world, KeyCode::KeyE, ButtonState::Pressed);
            pressed_e = true;
            via = format!("player moved next to the nearest wild {name}; E pressed (manual encounter)");
        }
    }
    let (n, ok) = ctx.step_until(app, 60, in_battle);
    if matches!(spec, BattleSpec::Trainer(_)) {
        send_key(app.world_mut(), KeyCode::KeyV, ButtonState::Released);
    }
    if pressed_e {
        send_key(app.world_mut(), KeyCode::KeyE, ButtonState::Released);
    }
    let n = n + ctx.step(app, 1);
    let world = app.world_mut();
    if !ok {
        return Err(format!("no battle started after {n} frames ({via}); state {}", state_name(world)));
    }
    Ok(json!({"started": true, "via": via, "stepped": n, "battle": battle_json(world)}))
}

/// Names of the screens open right now.
pub fn open_screens(world: &World) -> Vec<&'static str> {
    let mut out = Vec::new();
    if world.get_resource::<State<GameState>>().is_some_and(|s| *s.get() == GameState::MainMenu) {
        out.push("main_menu");
    }
    if let Some(b) = world.get_resource::<super::bag_screen::BagUi>() {
        if b.open {
            out.push(if b.mode == super::bag_screen::BagMode::Shop { "shop" } else { "bag" });
        }
    }
    if world.get_resource::<super::party_screen::PartyScreenState>().is_some_and(|s| s.open) {
        out.push("party");
    }
    if world.get_resource::<super::summary_screen::SummaryState>().is_some_and(|s| s.open) {
        out.push("summary");
    }
    if world.get_resource::<super::box_screen::BoxScreenState>().is_some_and(|s| s.open) {
        out.push("box");
    }
    if world.get_resource::<super::dex_screen::DexUi>().is_some_and(|s| s.open) {
        out.push("dex");
    }
    if world.get_resource::<super::crafting_ui::CraftingUi>().is_some_and(|s| s.open) {
        out.push("crafting");
    }
    if world.get_resource::<super::graphics_screen::GraphicsScreenState>().is_some_and(|s| s.open) {
        out.push("graphics");
    }
    let paused = world.get_resource::<Paused>().is_some_and(|p| p.0);
    let suppressed = world.get_resource::<PauseMenuSuppressed>().is_some_and(|p| p.0);
    if paused && !suppressed && out.is_empty() {
        out.push("pause");
    }
    out
}

fn open_screen(app: &mut App, ctx: &mut Ctx, s: ScreenName) -> Result<Value, String> {
    let world = app.world_mut();
    fn raise<R: Resource + Component<Mutability = bevy::ecs::component::Mutable>>(
        world: &mut World,
        set: impl FnOnce(&mut R),
    ) -> Result<(), String> {
        let mut r = world
            .get_resource_mut::<R>()
            .ok_or_else(|| format!("{} is not installed in this app", std::any::type_name::<R>()))?;
        set(&mut r);
        Ok(())
    }
    if s != ScreenName::Close && !open_screens(world).is_empty() {
        return Err(format!("a screen is already open: {:?} (open_screen close first)", open_screens(world)));
    }
    if s != ScreenName::Close && s != ScreenName::Pause && state_name(world) != "World" {
        return Err(format!("screens open only in the World state (now {})", state_name(world)));
    }
    let mut stepped = 0;
    match s {
        ScreenName::Pause => {
            raise::<Paused>(world, |p| p.0 = true)?;
            raise::<PauseMenu>(world, |m| m.selected = 0)?;
        }
        ScreenName::Bag => raise::<BagOpenRequested>(world, |r| r.0 = true)?,
        ScreenName::Party => raise::<PartyOpenRequested>(world, |r| r.0 = true)?,
        ScreenName::Summary => raise::<super::summary_screen::SummaryOpenRequested>(world, |r| r.0 = true)?,
        ScreenName::Box => raise::<BoxOpenRequested>(world, |r| r.0 = true)?,
        ScreenName::Dex => raise::<DexOpenRequested>(world, |r| r.0 = true)?,
        ScreenName::Crafting => raise::<super::crafting_ui::CraftingOpenRequested>(world, |r| r.0 = true)?,
        ScreenName::Shop => raise::<ShopOpenRequested>(world, |r| r.0 = true)?,
        ScreenName::Graphics => raise::<super::pause_menu::GraphicsOpenRequested>(world, |r| r.0 = true)?,
        ScreenName::Close => {
            for _ in 0..4 {
                if open_screens(app.world()).iter().all(|n| *n == "main_menu") {
                    break;
                }
                send_key(app.world_mut(), KeyCode::Escape, ButtonState::Pressed);
                stepped += ctx.step(app, 1);
                send_key(app.world_mut(), KeyCode::Escape, ButtonState::Released);
                stepped += ctx.step(app, 1);
            }
        }
    }
    stepped += ctx.step(app, 2);
    let open = open_screens(app.world());
    if s != ScreenName::Close && open.is_empty() {
        return Err(format!("{s:?} did not open after {stepped} frames"));
    }
    if s == ScreenName::Close && open.iter().any(|n| *n != "main_menu") {
        return Err(format!("could not close {open:?} with Escape"));
    }
    Ok(json!({"open_screens": open, "stepped": stepped}))
}

fn toast(world: &World) -> Option<String> {
    world.get_resource::<SaveToast>().filter(|t| t.timer > 0.0 && !t.text.is_empty()).map(|t| t.text.clone())
}

fn save(app: &mut App, ctx: &mut Ctx) -> Result<Value, String> {
    let world = app.world_mut();
    if world.contains_resource::<ActiveBattleSession>() {
        return Err("can't save during a battle (same rule as F5)".into());
    }
    world.get_resource_mut::<SaveRequested>().ok_or("save system not installed")?.0 = true;
    let n = ctx.step(app, 1);
    let world = app.world();
    let game = world.get_resource::<super::save_game::SaveGame>().ok_or("save system not installed")?;
    let message = game.message.clone().unwrap_or_default();
    if !game.file.exists() {
        return Err(format!("no save file written ({message})"));
    }
    Ok(json!({"path": game.file.path.to_string_lossy(), "message": message, "stepped": n}))
}

/// In-process reload of the save file: progress (party, bag, money, Pokédex), clock, play time and
/// the player's pose (a different zone goes through a zone change).
fn load(app: &mut App, ctx: &mut Ctx) -> Result<Value, String> {
    let world = app.world_mut();
    if world.contains_resource::<ActiveBattleSession>() {
        return Err("can't load during a battle".into());
    }
    let loaded = {
        let game = world.get_resource::<super::save_game::SaveGame>().ok_or("save system not installed")?;
        game.file.load_with_fallback().map_err(|e| format!("cannot read {}: {e:?}", game.file.path.display()))?
    };
    let save = loaded.save;
    world.insert_resource(PlayerProgress::from_save(&save));
    if let Some(mut c) = world.get_resource_mut::<GameClock>() {
        c.t_hours = f32::from(save.time_minutes) / 60.0;
    }
    if let Some(mut p) = world.get_resource_mut::<super::play_time::PlayTime>() {
        p.seconds = save.play_seconds;
    }
    let here = world.get_resource::<ActiveZoneInfo>().map(|z| z.area.clone()).unwrap_or_default();
    let mut stepped = 0;
    if save.zone == here {
        let mut q = world.query_filtered::<&mut Transform, With<PlayerAvatar>>();
        if let Some(mut t) = q.iter_mut(world).next() {
            t.translation = Vec3::from(save.position);
            t.rotation = Quat::from_rotation_y(save.yaw);
        }
    } else {
        world.write_message(ZoneChangeRequest {
            zone: save.zone.clone(),
            spawn: Vec3::from(save.position),
            yaw_deg: save.yaw.to_degrees(),
            source: ZoneChangeSource::Debug,
            link: Some("automation load".into()),
        });
        let (n, _) = ctx.step_until(app, 4, |w| !w.resource::<ZoneLoading>().is_idle());
        let (m, _) = ctx.step_until(app, 3600, |w| w.resource::<ZoneLoading>().is_idle() && state_name(w) == "World");
        stepped = n + m;
    }
    stepped += ctx.step(app, 1);
    let world = app.world_mut();
    Ok(json!({
        "zone": save.zone,
        "position": save.position,
        "party_size": save.party.len(),
        "money": save.money,
        "from_backup": loaded.from_backup,
        "stepped": stepped,
        "player": player_transform(world).map(|t| v3(t.translation)),
    }))
}

/// Mean colour and spread of an RGBA8 frame (to spot black or blank frames without looking).
fn frame_stats(data: &[u8]) -> Value {
    let n = (data.len() / 4).max(1) as f64;
    let mut sum = [0f64; 3];
    let mut luma_sq = 0f64;
    let mut luma_sum = 0f64;
    for px in data.chunks_exact(4) {
        for c in 0..3 {
            sum[c] += f64::from(px[c]);
        }
        let l = 0.2126 * f64::from(px[0]) + 0.7152 * f64::from(px[1]) + 0.0722 * f64::from(px[2]);
        luma_sum += l;
        luma_sq += l * l;
    }
    let mean_l = luma_sum / n;
    let std = (luma_sq / n - mean_l * mean_l).max(0.0).sqrt();
    json!({
        "mean_rgb": [(sum[0] / n).round(), (sum[1] / n).round(), (sum[2] / n).round()],
        "luma_mean": (mean_l * 10.0).round() / 10.0,
        "luma_std": (std * 10.0).round() / 10.0,
        "looks_blank": std < 2.0,
    })
}

fn screenshot(
    app: &mut App,
    ctx: &mut Ctx,
    path: &Path,
    size: Option<(u32, u32)>,
    camera: Option<OrbitCamera>,
) -> Result<Value, String> {
    let world = app.world_mut();
    let state = world.resource::<AutomationState>();
    if !state.gpu {
        return Err("unavailable: no GPU renderer (logic-only mode)".into());
    }
    let target = state.target.clone().ok_or("unavailable: no camera has been redirected to the offscreen target yet")?;
    let mut stepped = 0;
    if let Some((w, h)) = size.filter(|s| *s != state.size) {
        world
            .resource_mut::<Assets<Image>>()
            .insert(target.id(), Image::new_target_texture(w, h, TextureFormat::Rgba8UnormSrgb, None))
            .map_err(|e| format!("cannot resize the target: {e:?}"))?;
        world.resource_mut::<AutomationState>().size = (w, h);
    }
    world.resource_mut::<AutomationState>().camera_override = camera;
    if camera.is_some() || size.is_some() {
        stepped += ctx.step(app, 2);
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let slot: Arc<Mutex<Option<Result<Value, String>>>> = Arc::new(Mutex::new(None));
    let out = slot.clone();
    let file = path.to_path_buf();
    app.world_mut().spawn(Screenshot::image(target)).observe(move |ev: On<ScreenshotCaptured>| {
        let img: Image = ev.image.clone();
        let stats = img.data.as_deref().map(frame_stats).unwrap_or(Value::Null);
        let (w, h) = (img.width(), img.height());
        let r = img
            .try_into_dynamic()
            .map_err(|e| format!("cannot convert the frame: {e:?}"))
            .and_then(|d| d.to_rgb8().save(&file).map_err(|e| format!("cannot write {}: {e}", file.display())))
            .map(|_| json!({"width": w, "height": h, "stats": stats}));
        if let Ok(mut s) = out.lock() {
            *s = Some(r);
        }
    });
    let mut waited = 0;
    while waited < 120 {
        waited += ctx.step(app, 1);
        if slot.lock().map(|s| s.is_some()).unwrap_or(true) {
            break;
        }
    }
    app.world_mut().resource_mut::<AutomationState>().camera_override = None;
    let result = slot.lock().ok().and_then(|mut s| s.take());
    match result {
        Some(Ok(mut v)) => {
            if let Some(o) = v.as_object_mut() {
                o.insert("path".into(), json!(path.to_string_lossy()));
                o.insert("stepped".into(), json!(stepped + waited));
            }
            Ok(v)
        }
        Some(Err(e)) => Err(e),
        None => Err(format!("no frame captured after {waited} frames")),
    }
}

/// Entity counts by kind (wild with a real model vs placeholder, NPCs, trainers, pickups).
fn entity_counts(world: &mut World) -> Value {
    let mut q = world.query::<(&WildCreature, Has<super::pokemon_anim::PokemonAnimSource>)>();
    let (mut wild, mut real) = (0, 0);
    for (w, has_model) in q.iter(world) {
        if w.defeated {
            continue;
        }
        wild += 1;
        real += usize::from(has_model);
    }
    let mut npc_q = world.query::<(&super::npc::NpcInstance, Has<super::npc::NpcModelSource>)>();
    let (mut npcs, mut npc_models) = (0, 0);
    for (_, m) in npc_q.iter(world) {
        npcs += 1;
        npc_models += usize::from(m);
    }
    let mut trainer_q = world.query::<(&TrainerNpc, Has<super::npc::NpcModelSource>)>();
    let (mut trainers, mut trainer_models) = (0, 0);
    for (_, m) in trainer_q.iter(world) {
        trainers += 1;
        trainer_models += usize::from(m);
    }
    let pickups = world.query::<&super::pickups::SpawnMarker>().iter(world).count();
    let meshes = world.query::<&Mesh3d>().iter(world).count();
    json!({
        "wild_creatures": wild,
        "wild_with_real_model": real,
        "wild_placeholder": wild - real,
        "npcs": npcs,
        "npcs_with_model_source": npc_models,
        "trainer_markers": trainers,
        "trainer_markers_with_model_source": trainer_models,
        "pickups": pickups,
        "meshes": meshes,
        "entities": world.entities().len(),
    })
}

fn state_json(app: &mut App, ctx: &Ctx) -> Value {
    let world = app.world_mut();
    let player = {
        let mut q = world.query_filtered::<(
            &Transform,
            Option<&super::water::PlayerSwim>,
            Option<&super::player_motion::PlayerMotionState>,
        ), With<PlayerAvatar>>();
        q.iter(world).next().map(|(t, swim, motion)| {
            json!({
                "position": v3(t.translation),
                "yaw_deg": round2(yaw_of(t).to_degrees()),
                "swim": swim.map(|s| format!("{:?}", s.state)),
                "motion": motion.map(|m| format!("{:?}", m.mode)),
            })
        })
    };
    let zone = world.get_resource::<ActiveZoneInfo>().map(|z| json!({"area": z.area, "name": z.name}));
    let clock = world
        .get_resource::<GameClock>()
        .map(|c| json!({"time": c.hhmm(), "minutes": c.minutes(), "slot": c.slot(), "pinned": c.pinned}));
    let weather = world.get_resource::<WeatherVisual>().map(|w| {
        let forced = world.get_resource::<WeatherState>().is_some_and(|s| s.force.is_some());
        json!({"name": w.name(), "id": w.id, "forced": forced})
    });
    let progress = world.get_resource::<PlayerProgress>().map(|p| {
        json!({
            "party": p.party.iter().map(|m| json!({"dex": m.species, "level": m.level, "hp": m.hp})).collect::<Vec<_>>(),
            "box_count": p.box_list.len(),
            "bag_kinds": p.bag.values().filter(|n| **n > 0).count(),
            "bag_items": p.bag.values().map(|n| u64::from(*n)).sum::<u64>(),
            "money": p.money,
            "seen": p.seen.len(),
            "caught": p.caught.len(),
            "defeated_trainers": p.defeated_trainers.iter().collect::<Vec<_>>(),
        })
    });
    let loading = world.get_resource::<ZoneLoading>().map(|l| format!("{:?}", l.phase));
    let avg_ms = if ctx.update_ms.is_empty() {
        0.0
    } else {
        ctx.update_ms.iter().sum::<f32>() / ctx.update_ms.len() as f32
    };
    let gpu = world.resource::<AutomationState>().gpu;
    let toast_text = toast(world);
    let screens = open_screens(world);
    let paused = world.get_resource::<Paused>().is_some_and(|p| p.0);
    let game_state = state_name(world);
    // A transition requested this frame (e.g. an encounter) only shows in `game_state` next frame.
    let next_state = world.get_resource::<NextState<GameState>>().and_then(|n| match n {
        NextState::Pending(s) | NextState::PendingIfNeq(s) => Some(format!("{s:?}")),
        NextState::Unchanged => None,
    });
    let battle = battle_json(world);
    let counts = entity_counts(world);
    json!({
        "game_state": game_state,
        "next_state": next_state,
        "paused": paused,
        "zone_loading": loading,
        "zone": zone,
        "player": player,
        "clock": clock,
        "weather": weather,
        "progress": progress,
        "battle": battle,
        "toast": toast_text,
        "open_screens": screens,
        "counts": counts,
        "frames": {
            "frame": ctx.frame,
            "game_dt_ms": FRAME_DT.as_secs_f64() * 1000.0,
            "real_update_ms_avg": (f64::from(avg_ms) * 100.0).round() / 100.0,
            "real_fps": if avg_ms > 0.0 { ((1000.0 / f64::from(avg_ms)) * 10.0).round() / 10.0 } else { 0.0 },
        },
        "gpu": gpu,
    })
}

fn entities_json(world: &mut World, kind: EntityKind, limit: usize) -> Value {
    let origin = player_transform(world).map(|t| t.translation).unwrap_or(Vec3::ZERO);
    let mut rows: Vec<(f32, Value)> = Vec::new();
    let vis = |v: Option<&Visibility>, vv: Option<&ViewVisibility>| -> Value {
        json!({
            "visibility": v.map(|v| format!("{v:?}")),
            "in_view": vv.map(|v| v.get()),
        })
    };
    macro_rules! collect {
        ($filter:ty, |$e:ident, $t:ident, $extra:ident| $body:expr) => {{
            let mut q = world.query_filtered::<(Entity, &GlobalTransform, Option<&Visibility>, Option<&ViewVisibility>, $filter), ()>();
            for ($e, $t, v, vv, $extra) in q.iter(world) {
                let p = $t.translation();
                let mut row: Value = $body;
                if let Some(o) = row.as_object_mut() {
                    o.insert("entity".into(), json!(format!("{:?}", $e)));
                    o.insert("position".into(), v3(p));
                    o.insert("distance".into(), json!(round2(p.distance(origin))));
                    o.insert("visible".into(), vis(v, vv));
                }
                rows.push((p.distance(origin), row));
            }
        }};
    }
    match kind {
        EntityKind::Wild => collect!((&WildCreature, Has<super::pokemon_anim::PokemonAnimSource>, Has<super::alpha_aura::AlphaAura>), |e, t, x| json!({
            "species": x.0.species_name, "dex": x.0.national_dex, "level": x.0.level, "alpha": x.2,
            "in_battle": x.0.in_battle, "defeated": x.0.defeated, "real_model": x.1, "spawner": x.0.spawner_id,
        })),
        EntityKind::Npc => collect!((&super::npc::NpcInstance, Has<super::npc::NpcModelSource>), |e, t, x| json!({
            "key": x.0.key, "chara": x.0.chara_id, "idle_clip": x.0.idle_clip, "model_source": x.1,
        })),
        EntityKind::Trainer => collect!((&TrainerNpc, Has<super::npc::NpcModelSource>, Has<super::trainer_encounter::TrainerBeaten>), |e, t, x| json!({
            "key": x.0.key, "trainer_id": x.0.trainer_id, "model_source": x.1, "beaten_pending_grey": x.2,
        })),
        EntityKind::Player => collect!(&PlayerAvatar, |e, t, x| { let _ = x; json!({"player": true}) }),
        EntityKind::Camera => collect!(&Camera, |e, t, x| json!({"order": x.order, "active": x.is_active})),
        EntityKind::Named => collect!(&Name, |e, t, x| json!({"name": x.as_str()})),
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total = rows.len();
    json!({
        "kind": format!("{kind:?}").to_lowercase(),
        "total": total,
        "shown": total.min(limit),
        "entities": rows.into_iter().take(limit).map(|(_, v)| v).collect::<Vec<_>>(),
    })
}

/// Last `lines` lines of the viewer log (`--automation-log` / `PLA_AUTOMATION_LOG`).
fn log_tail(cfg: &AutomationConfig, lines: usize) -> Result<Value, String> {
    let path = cfg
        .log_path
        .as_ref()
        .ok_or("no log file known: start the viewer through .tools/gamectl.py or pass --automation-log <file>")?;
    let mut f = std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let window = 512 * 1024u64;
    if len > window {
        use std::io::Seek;
        let _ = f.seek(std::io::SeekFrom::Start(len - window));
    }
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&buf);
    let all: Vec<&str> = text.lines().collect();
    let tail: Vec<&str> = all[all.len().saturating_sub(lines)..].to_vec();
    let warnings = tail
        .iter()
        .filter(|l| {
            let u = l.to_ascii_uppercase();
            u.contains("WARN") || u.contains("ERROR") || u.contains("PANIC")
        })
        .count();
    Ok(json!({"path": path.to_string_lossy(), "lines": tail, "warning_lines": warnings}))
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn config_is_off_by_default_and_reads_its_flags() {
        assert!(AutomationConfig::from_args(&args(&["viewer", "--area", "1"])).is_none());
        let c = AutomationConfig::from_args(&args(&[
            "viewer",
            "--automation",
            "--automation-dir",
            "/tmp/x",
            "--automation-size",
            "640x360",
            "--automation-no-gpu",
            "--automation-log",
            "/tmp/x/v.log",
        ]))
        .unwrap();
        assert_eq!(c.dir, PathBuf::from("/tmp/x"));
        assert_eq!(c.socket, PathBuf::from(format!("/tmp/x/{}.sock", std::process::id())));
        assert_eq!(c.size, (640, 360));
        assert!(!c.gpu);
        assert_eq!(c.log_path, Some(PathBuf::from("/tmp/x/v.log")));
        assert_eq!(c.panic_file, PathBuf::from("/tmp/x/last_panic.txt"));
        assert_eq!(parse_size("10x10"), None);
        assert_eq!(parse_size("5000x100"), None);
    }

    #[test]
    fn scratch_save_unless_a_save_dir_is_given() {
        let c = AutomationConfig::from_args(&args(&["v", "--automation", "--automation-dir", "d"])).unwrap();
        let a = effective_args(args(&["v", "--automation"]), &c);
        assert!(a.windows(2).any(|w| w[0] == "--save-dir" && w[1] == "d/save"));
        assert!(a.iter().any(|x| x == "--new-game"));
        let b = effective_args(args(&["v", "--automation", "--save-dir", "mine"]), &c);
        assert_eq!(b.iter().filter(|x| *x == "--save-dir").count(), 1);
        assert!(!b.iter().any(|x| x == "--new-game"));
    }

    #[test]
    fn keys_parse_by_letter_name_and_bevy_name() {
        assert_eq!(parse_key("V"), Some(KeyCode::KeyV));
        assert_eq!(parse_key("v"), Some(KeyCode::KeyV));
        assert_eq!(parse_key("KeyV"), Some(KeyCode::KeyV));
        assert_eq!(parse_key("3"), Some(KeyCode::Digit3));
        assert_eq!(parse_key("Digit3"), Some(KeyCode::Digit3));
        assert_eq!(parse_key("Esc"), Some(KeyCode::Escape));
        assert_eq!(parse_key("escape"), Some(KeyCode::Escape));
        assert_eq!(parse_key("F5"), Some(KeyCode::F5));
        assert_eq!(parse_key("f"), Some(KeyCode::KeyF));
        assert_eq!(parse_key("Shift"), Some(KeyCode::ShiftLeft));
        assert_eq!(parse_key("up"), Some(KeyCode::ArrowUp));
        assert_eq!(parse_key("F13"), None);
        assert_eq!(parse_key("hyper"), None);
    }

    #[test]
    fn force_alpha_and_alpha_battle_parse() {
        use super::super::alpha_aura::ForceAlpha;
        assert_eq!(parse_command(r#"{"cmd":"force_alpha"}"#), Ok(Command::ForceAlpha(ForceAlpha::Nearest(1))));
        assert_eq!(parse_command(r#"{"cmd":"force_alpha","count":3}"#), Ok(Command::ForceAlpha(ForceAlpha::Nearest(3))));
        assert_eq!(parse_command(r#"{"cmd":"force_alpha","all":true}"#), Ok(Command::ForceAlpha(ForceAlpha::All)));
        assert_eq!(parse_command(r#"{"cmd":"force_alpha","off":true}"#), Ok(Command::ForceAlpha(ForceAlpha::Off)));
        assert!(parse_command(r#"{"cmd":"force_alpha","all":true,"off":true}"#).is_err());
        assert!(parse_command(r#"{"cmd":"force_alpha","all":false}"#).is_err());
        assert_eq!(
            parse_command(r#"{"cmd":"start_battle","wild":"haunter","level":30,"alpha":true}"#),
            Ok(Command::StartBattle(BattleSpec::Wild { species: "haunter".into(), level: 30, alpha: true }))
        );
        assert!(parse_command(r#"{"cmd":"start_battle","wild":"haunter","alpha":"yes"}"#).is_err());
    }

    #[test]
    fn commands_parse() {
        assert_eq!(parse_command(r#"{"cmd":"ping"}"#), Ok(Command::Ping));
        assert_eq!(parse_command(r#"{"cmd":"step","frames":300}"#), Ok(Command::Step { frames: 300 }));
        assert_eq!(parse_command(r#"{"cmd":"step"}"#), Ok(Command::Step { frames: 1 }));
        assert_eq!(
            parse_command(r#"{"cmd":"key","key":"V","frames":3}"#),
            Ok(Command::Key { key: KeyCode::KeyV, action: KeyAction::Press, frames: 3 })
        );
        assert_eq!(
            parse_command(r#"{"cmd":"key","key":"w","action":"down"}"#),
            Ok(Command::Key { key: KeyCode::KeyW, action: KeyAction::Down, frames: 1 })
        );
        assert_eq!(parse_command(r#"{"cmd":"teleport","x":10.5,"z":-3}"#), Ok(Command::Teleport { x: 10.5, z: -3.0 }));
        assert_eq!(parse_command(r#"{"cmd":"teleport_zone","area":2}"#), Ok(Command::TeleportZone { area: 2, pos: None }));
        assert_eq!(
            parse_command(r#"{"cmd":"set_time","slot":2}"#),
            Ok(Command::SetTime { time: TimeSpec::Slot(2), pinned: None })
        );
        assert_eq!(
            parse_command(r#"{"cmd":"set_time","minutes":600,"pinned":true}"#),
            Ok(Command::SetTime { time: TimeSpec::Minutes(600), pinned: Some(true) })
        );
        assert_eq!(parse_command(r#"{"cmd":"set_weather","weather":"rain"}"#), Ok(Command::SetWeather { weather: Some(4) }));
        assert_eq!(parse_command(r#"{"cmd":"set_weather","weather":4}"#), Ok(Command::SetWeather { weather: Some(4) }));
        assert_eq!(parse_command(r#"{"cmd":"set_weather","weather":"auto"}"#), Ok(Command::SetWeather { weather: None }));
        assert_eq!(
            parse_command(r#"{"cmd":"start_battle","wild":"Bidoof","level":7}"#),
            Ok(Command::StartBattle(BattleSpec::Wild { species: "bidoof".into(), level: 7, alpha: false }))
        );
        assert_eq!(
            parse_command(r#"{"cmd":"start_battle","trainer":"giratina01"}"#),
            Ok(Command::StartBattle(BattleSpec::Trainer("giratina01".into())))
        );
        assert_eq!(parse_command(r#"{"cmd":"start_battle","wild":"nearest"}"#), Ok(Command::StartBattle(BattleSpec::NearestWild)));
        assert_eq!(parse_command(r#"{"cmd":"open_screen","screen":"Pokedex"}"#), Ok(Command::OpenScreen(ScreenName::Dex)));
        assert_eq!(parse_command(r#"{"cmd":"open_screen","screen":"graphics"}"#), Ok(Command::OpenScreen(ScreenName::Graphics)));
        assert_eq!(
            parse_command(r#"{"cmd":"entities","filter":"npc","limit":5}"#),
            Ok(Command::Entities { kind: EntityKind::Npc, limit: 5 })
        );
        assert_eq!(parse_command(r#"{"cmd":"log_tail","lines":20}"#), Ok(Command::LogTail { lines: 20 }));
        assert_eq!(parse_command(r#"{"cmd":"quit"}"#), Ok(Command::Quit));
        let shot = parse_command(r#"{"cmd":"screenshot","path":"a/b.png","width":640,"height":360,"camera":{"yaw":90,"pitch":30,"distance":8,"degrees":true}}"#).unwrap();
        match shot {
            Command::Screenshot { path, size, camera: Some(c) } => {
                assert_eq!(path, PathBuf::from("a/b.png"));
                assert_eq!(size, Some((640, 360)));
                assert!((c.yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
                assert!((c.distance - 8.0).abs() < 1e-6);
                assert_eq!(c.focus, None);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bad_commands_are_errors_not_panics() {
        for bad in [
            "",
            "not json",
            "[]",
            r#"{"frames":3}"#,
            r#"{"cmd":"fly"}"#,
            r#"{"cmd":"step","frames":-1}"#,
            r#"{"cmd":"step","frames":1000001}"#,
            r#"{"cmd":"key"}"#,
            r#"{"cmd":"key","key":"V","action":"tap"}"#,
            r#"{"cmd":"teleport","x":1}"#,
            r#"{"cmd":"teleport_zone","area":1,"x":3}"#,
            r#"{"cmd":"set_time"}"#,
            r#"{"cmd":"set_time","slot":4}"#,
            r#"{"cmd":"set_time","slot":1,"minutes":3}"#,
            r#"{"cmd":"set_weather","weather":"hail"}"#,
            r#"{"cmd":"start_battle"}"#,
            r#"{"cmd":"start_battle","wild":"a","trainer":"b"}"#,
            r#"{"cmd":"open_screen","screen":"map"}"#,
            r#"{"cmd":"screenshot","path":"a.jpg"}"#,
            r#"{"cmd":"screenshot","path":"a.png","width":100}"#,
            r#"{"cmd":"screenshot","path":"a.png","width":8,"height":8}"#,
            r#"{"cmd":"screenshot","path":"a.png","camera":{"distance":0}}"#,
            r#"{"cmd":"entities","filter":"rocks"}"#,
            r#"{"cmd":"wait_seconds","seconds":-2}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn orbit_camera_looks_at_its_focus() {
        let o = OrbitCamera { yaw: 0.0, pitch: 0.0, distance: 5.0, focus: None };
        let t = orbit_transform(&o, Vec3::new(1.0, 2.0, 3.0));
        assert!((t.translation - Vec3::new(1.0, 2.0, 8.0)).length() < 1e-4);
        assert!((t.forward().as_vec3() - Vec3::NEG_Z).length() < 1e-4);
    }

    #[test]
    fn frame_stats_flags_blank_frames() {
        let black = vec![0u8; 4 * 64];
        assert_eq!(frame_stats(&black)["looks_blank"], json!(true));
        let mut mixed = Vec::new();
        for i in 0..64u8 {
            mixed.extend_from_slice(&[i * 4, i * 2, 255 - i, 255]);
        }
        assert_eq!(frame_stats(&mixed)["looks_blank"], json!(false));
    }

    /// The server answers one line per command and refuses connections from nowhere but the
    /// socket file, which is private to the user.
    #[test]
    fn socket_is_private_and_line_based() {
        let dir = std::env::temp_dir().join(format!("pla-automation-sock-{}", std::process::id()));
        let path = dir.join("t.sock");
        let mut server = Server::bind(&path).expect("bind");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let dmode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(dmode, 0o700);
        let mut client = UnixStream::connect(&path).unwrap();
        client.write_all(b"{\"cmd\":\"ping\"}\n{\"cmd\":\"st").unwrap();
        let mut got = None;
        for _ in 0..200 {
            if let Some(x) = server.poll() {
                got = Some(x);
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let (i, line) = got.expect("a line");
        assert_eq!(parse_command(&line), Ok(Command::Ping));
        server.reply(i, &json!({"ok": true}));
        let mut buf = [0u8; 64];
        let n = client.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"{\"ok\":true}\n");
        // The half line is not a command yet.
        assert!(server.poll().is_none());
        drop(SocketGuard(path.clone()));
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
