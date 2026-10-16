//! The player character as a real skinned glTF model with idle / walk / run clips.
//!
//! The exporter (`player_export`) writes one glTF per outfit variant
//! (`<cache>/player/p1_default/player.gltf`) with nine clips. This module spawns that
//! scene as a child of the `PlayerAvatar` entity, builds an `AnimationGraph` with the
//! idle / walk / run / dash / swim / jump_start / jump_fall / land clips and cross-fades
//! between them from the measured planar speed of the avatar, `PlayerSwim` in water and
//! `PlayerMotionState` in the air (Brief 13). Locomotion (movement, facing, terrain
//! height) stays in the viewer / `game::player_motion`; the model only follows the avatar.
//!
//! Root motion: the walk / run / dash clips translate the `origin` bone forward
//! (0 -> 1.7 / 2.85 / 3.3 m per loop). The entity is moved by the locomotion code, so
//! that translation would make the model slide away from, and snap back to, the
//! avatar. Bevy stores curves as opaque trait objects, so the clip cannot be edited
//! per axis; instead the horizontal translation of the `origin` bone is pinned to its
//! rest value every frame after the animation system has written it.

use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle, AnimationNodeIndex};
use bevy::animation::transition::AnimationTransitions;
use bevy::animation::AnimationPlayer;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::player_motion::{MotionMode, PlayerMotionState};
use super::player_skills::{CrouchPhase, DodgeDir, PlayerSkillState};
use super::PlayerAvatar;
use super::water::{PlayerSwim, SwimState};

/// Height of the avatar root above the ground (the old capsule was 1.8 m, centred).
pub const AVATAR_ROOT_ABOVE_GROUND: f32 = 0.9;
/// Standing height of the exported model (p1 1.49 m, p2 1.46 m).
pub const MODEL_HEIGHT: f32 = 1.5;
/// Name of the bone that carries the clips' forward root motion.
pub const ROOT_MOTION_BONE: &str = "origin";

/// Cross-fade length between clips.
pub const FADE: Duration = Duration::from_millis(250);

/// Speed (m/s) tiers with hysteresis: (enter from below, leave back down).
const WALK_UP: f32 = 0.35;
const WALK_DOWN: f32 = 0.2;
const RUN_UP: f32 = 4.2;
const RUN_DOWN: f32 = 3.6;
const DASH_UP: f32 = 6.5;
const DASH_DOWN: f32 = 5.8;

/// Locomotion clip chosen from the player speed and the vertical motion state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerClip {
    Idle,
    Walk,
    Run,
    Dash,
    /// Played while the avatar is in water deep enough to swim (`game::water`). The clip exists
    /// in the exported player glTF but was unused until Brief 11.
    Swim,
    /// Take-off / rising phase of a jump (`game::player_motion`). No longer unused since Brief 13.
    JumpStart,
    /// Descending phase of a jump or a plain fall.
    JumpFall,
    /// Short clip after touchdown; played while `PlayerMotionState::land_timer` runs.
    Land,
    /// Dodge roll in the facing direction / backwards / to the left / to the right
    /// (`rolling01_{f,b,l,r}`, Brief 22). One-shots as long as the roll lasts.
    RollF,
    RollB,
    RollL,
    RollR,
    /// Crouched idle (`squat01_l_loop`).
    Crouch,
    /// Crouched walk (`sqwalk01_loop`).
    CrouchWalk,
    /// Standing to crouched (`sqwait01_l_start`).
    CrouchStart,
    /// Crouched to standing (`sqwait01_l_end`).
    CrouchEnd,
    // Brief 38 follow-up: the mounted-rider pose per ride Pokémon (seated, hands on the
    // mount's back), exported as `ride_<mount>_idle` / `ride_<mount>_move`. These are the
    // game's OWN rider clips from `pc.gfpak`, found by name in the animation controller
    // (see `player_export::CLIPS`). Old exports do not carry them: the selection falls
    // back to `Idle` exactly like the skill clips.
    RideWyrdeerIdle,
    RideWyrdeerMove,
    RideUrsalunaIdle,
    RideUrsalunaMove,
    RideBasculegionIdle,
    RideBasculegionMove,
    RideSneaslerIdle,
    RideBraviaryIdle,
    RideBraviaryMove,
}

/// Every clip with its own slot, in slot order (`clip as usize`).
pub const ALL_CLIPS: [PlayerClip; CLIP_SLOTS] = [
    PlayerClip::Idle,
    PlayerClip::Walk,
    PlayerClip::Run,
    PlayerClip::Dash,
    PlayerClip::Swim,
    PlayerClip::JumpStart,
    PlayerClip::JumpFall,
    PlayerClip::Land,
    PlayerClip::RollF,
    PlayerClip::RollB,
    PlayerClip::RollL,
    PlayerClip::RollR,
    PlayerClip::Crouch,
    PlayerClip::CrouchWalk,
    PlayerClip::CrouchStart,
    PlayerClip::CrouchEnd,
    PlayerClip::RideWyrdeerIdle,
    PlayerClip::RideWyrdeerMove,
    PlayerClip::RideUrsalunaIdle,
    PlayerClip::RideUrsalunaMove,
    PlayerClip::RideBasculegionIdle,
    PlayerClip::RideBasculegionMove,
    PlayerClip::RideSneaslerIdle,
    PlayerClip::RideBraviaryIdle,
    PlayerClip::RideBraviaryMove,
];

impl PlayerClip {
    /// Clip name inside the exported glTF.
    pub fn gltf_name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Walk => "walk",
            Self::Run => "run",
            Self::Dash => "dash",
            Self::Swim => "swim",
            Self::JumpStart => "jump_start",
            Self::JumpFall => "jump_fall",
            Self::Land => "land",
            Self::RollF => "roll_f",
            Self::RollB => "roll_b",
            Self::RollL => "roll_l",
            Self::RollR => "roll_r",
            Self::Crouch => "crouch",
            Self::CrouchWalk => "crouch_walk",
            Self::CrouchStart => "crouch_start",
            Self::CrouchEnd => "crouch_end",
            Self::RideWyrdeerIdle => "ride_wyrdeer_idle",
            Self::RideWyrdeerMove => "ride_wyrdeer_move",
            Self::RideUrsalunaIdle => "ride_ursaluna_idle",
            Self::RideUrsalunaMove => "ride_ursaluna_move",
            Self::RideBasculegionIdle => "ride_basculegion_idle",
            Self::RideBasculegionMove => "ride_basculegion_move",
            Self::RideSneaslerIdle => "ride_sneasler_idle",
            Self::RideBraviaryIdle => "ride_braviary_idle",
            Self::RideBraviaryMove => "ride_braviary_move",
        }
    }
}

/// The rider clip pair `(idle, move)` of a mount: the game's own mounted pose for that ride
/// Pokémon. `move` is `None` for a mount whose set has no gait loop (Sneasler): the rider
/// keeps the seated `idle`. Callers fall back along [`fallback_clip`] when the export does
/// not carry the clip yet.
pub fn rider_clips(kind: super::ride::RideKind) -> (PlayerClip, Option<PlayerClip>) {
    use super::ride::RideKind;
    match kind {
        RideKind::Wyrdeer => (PlayerClip::RideWyrdeerIdle, Some(PlayerClip::RideWyrdeerMove)),
        RideKind::Ursaluna => (PlayerClip::RideUrsalunaIdle, Some(PlayerClip::RideUrsalunaMove)),
        RideKind::Basculegion => {
            (PlayerClip::RideBasculegionIdle, Some(PlayerClip::RideBasculegionMove))
        }
        RideKind::Sneasler => (PlayerClip::RideSneaslerIdle, None),
        RideKind::Braviary => (PlayerClip::RideBraviaryIdle, Some(PlayerClip::RideBraviaryMove)),
    }
}

/// Picks the clip for `speed` (m/s), keeping `current` while the speed stays inside its
/// hysteresis band so a speed hovering at a threshold does not flicker between clips.
pub fn select_clip(current: PlayerClip, speed: f32) -> PlayerClip {
    use PlayerClip::*;
    let mut c = current;
    // At most three steps are ever needed (idle <-> dash).
    for _ in 0..3 {
        let next = match c {
            Idle if speed > WALK_UP => Walk,
            Walk if speed > RUN_UP => Run,
            Walk if speed < WALK_DOWN => Idle,
            Run if speed > DASH_UP => Dash,
            Run if speed < RUN_DOWN => Walk,
            Dash if speed < DASH_DOWN => Run,
            other => other,
        };
        if next == c {
            break;
        }
        c = next;
    }
    c
}

/// Clip for a frame, taking the swimming state into account: swimming always uses the `swim`
/// clip whatever the speed is, and leaving the water (or any one-shot state) re-picks the land
/// clip from a neutral state instead of staying stuck on it.
pub fn select_clip_full(current: PlayerClip, speed: f32, swimming: bool) -> PlayerClip {
    if swimming {
        return PlayerClip::Swim;
    }
    use PlayerClip::*;
    let base = match current {
        // Walk/run/dash states are the only ones the speed tiers can continue from.
        Idle | Walk | Run | Dash => current,
        _ => Idle,
    };
    select_clip(base, speed)
}

/// Clip for a frame from the vertical motion state (Brief 13) and the skill state (Brief 22).
///
/// Priority: swimming, then a running dodge roll, then the air (`jump_start` while rising,
/// `jump_fall` while descending), `land` while its timer runs, climbing (`walk` as a placeholder:
/// the export has no climb clip), the crouch cycle (enter, hold or crouched walk, leave) and
/// finally the speed tiers of [`select_clip_full`].
pub fn select_clip_motion(
    current: PlayerClip,
    speed: f32,
    swimming: bool,
    motion: Option<&PlayerMotionState>,
    skills: Option<&PlayerSkillState>,
) -> PlayerClip {
    if swimming {
        return PlayerClip::Swim;
    }
    if let Some(d) = skills.and_then(|s| s.dodge) {
        return match d.dir {
            DodgeDir::Forward => PlayerClip::RollF,
            DodgeDir::Back => PlayerClip::RollB,
            DodgeDir::Left => PlayerClip::RollL,
            DodgeDir::Right => PlayerClip::RollR,
        };
    }
    if let Some(m) = motion {
        match m.mode {
            MotionMode::Airborne => {
                return if m.vertical_speed > 0.0 {
                    PlayerClip::JumpStart
                } else {
                    PlayerClip::JumpFall
                };
            }
            MotionMode::Climbing => return PlayerClip::Walk,
            MotionMode::Grounded => {
                if m.land_timer > 0.0 {
                    return PlayerClip::Land;
                }
            }
        }
    }
    if let Some(s) = skills {
        match s.crouch {
            CrouchPhase::Entering(_) => return PlayerClip::CrouchStart,
            CrouchPhase::Leaving(_) => return PlayerClip::CrouchEnd,
            CrouchPhase::Crouched => {
                return if speed > WALK_UP {
                    PlayerClip::CrouchWalk
                } else {
                    PlayerClip::Crouch
                };
            }
            CrouchPhase::Standing => {}
        }
    }
    select_clip_full(current, speed, swimming)
}

/// The clip to use when `c` is missing from the exported file: roll variants fall back to the
/// forward roll, then to the dash; crouch variants to their standing equivalents; the speed
/// tiers to the next slower one. `Idle` is the end of every chain.
pub fn fallback_clip(c: PlayerClip) -> PlayerClip {
    use PlayerClip::*;
    match c {
        Dash => Run,
        Run => Walk,
        RollB | RollL | RollR => RollF,
        RollF => Dash,
        CrouchWalk => Crouch,
        Crouch | CrouchStart | CrouchEnd => Idle,
        // Rider move clips fall back to the mount's seated idle, then to the plain idle.
        RideWyrdeerMove => RideWyrdeerIdle,
        RideUrsalunaMove => RideUrsalunaIdle,
        RideBasculegionMove => RideBasculegionIdle,
        RideBraviaryMove => RideBraviaryIdle,
        _ => Idle,
    }
}

/// Skill clips (Brief 22) a player glTF must carry; exports made before it have only the 9
/// locomotion clips, which made the dodge play the dash clip and the crouch show nothing.
pub const SKILL_CLIPS: [PlayerClip; 4] =
    [PlayerClip::RollF, PlayerClip::Crouch, PlayerClip::CrouchWalk, PlayerClip::CrouchStart];

/// glTF names of the [`SKILL_CLIPS`] for which `has` is false.
pub fn missing_skill_clips(has: impl Fn(&str) -> bool) -> Vec<&'static str> {
    SKILL_CLIPS.iter().map(|c| c.gltf_name()).filter(|n| !has(n)).collect()
}

/// Returns `translation` with the horizontal root motion removed: x/z pinned to the rest
/// pose, the vertical component (jumps, bobbing) kept.
pub fn strip_root_motion(translation: Vec3, rest: Vec3) -> Vec3 {
    Vec3::new(rest.x, translation.y, rest.z)
}

/// One locomotion facing step: turns `current` towards the planar movement direction
/// `(x, z)` (model front is +Z) at a rate of 15 per second, as the capsule avatar did.
/// A zero direction keeps the current facing.
pub fn step_facing(current: Quat, move_xz: Vec2, dt: f32) -> Quat {
    if move_xz.length_squared() <= f32::EPSILON {
        return current;
    }
    let target = Quat::from_rotation_y(move_xz.x.atan2(move_xz.y));
    current.slerp(target, (15.0 * dt).min(1.0))
}

/// Camera heights relative to the avatar root for a model standing `height` metres tall:
/// `(focal_offset_y, shoulder_y)`. The root sits `AVATAR_ROOT_ABOVE_GROUND` above the
/// ground; the orbit focus is on the upper chest (80 %), the aim camera at the
/// shoulders (85 %).
pub fn camera_offsets(height: f32) -> (f32, f32) {
    (0.8 * height - AVATAR_ROOT_ABOVE_GROUND, 0.85 * height - AVATAR_ROOT_ABOVE_GROUND)
}

/// Location of the exported player glTF for `variant` ("p1" | "p2") under `cache_root`,
/// or `None` if the variant is unknown or the file is missing.
pub fn resolve_player_model(cache_root: &Path, variant: &str) -> Option<PathBuf> {
    if !matches!(variant, "p1" | "p2") {
        return None;
    }
    let p = cache_root.join("player").join(format!("{variant}_default")).join("player.gltf");
    p.is_file().then_some(p)
}

/// Asset-server path for `file`: relative to the asset root `work_root` when inside it
/// (as for the Pokémon models), otherwise the absolute path.
pub fn asset_path(file: &Path, work_root: &Path) -> String {
    file.strip_prefix(work_root).unwrap_or(file).to_string_lossy().replace('\\', "/")
}

/// Request: render the player from this glTF (placed on a `PlayerAvatar` entity).
/// The fallback path: one merged glTF. The dress-up path (`game::dressup`) spawns one
/// [`PlayerSceneRef`] per worn piece instead and does not use this component.
#[derive(Component)]
pub struct PlayerModelSource {
    pub gltf: Handle<Gltf>,
}

/// Marks that the scene child has been spawned.
#[derive(Component)]
pub struct PlayerSceneSpawned;

/// One spawned player scene (a worn piece, or the merged model): its glTF handle and slot tag.
/// Every scene carries the same skeleton node names and clip names, so the animation driver in
/// this module can play the same clip on all of them.
#[derive(Component, Clone)]
pub struct PlayerSceneRef {
    pub gltf: Handle<Gltf>,
    /// Slot tag for reports and debugging (`merged`, `hair`, `face`, ...).
    pub slot: String,
}

/// The glTF scene child of the avatar (feet at the scene origin). `game::ride` moves it up
/// onto the saddle while a mount is active, so the component marks exactly the entity whose
/// local Y is the seat height. Every dress-up piece scene carries it too, so a mount lifts the
/// whole dressed character.
#[derive(Component)]
pub struct PlayerModelScene;

/// Smoothed planar speed of the avatar (m/s), measured from its transform.
#[derive(Component, Default)]
pub struct PlayerMotion {
    last: Option<Vec3>,
    pub speed: f32,
}

/// Number of clip slots ([`PlayerClip`] covers every exported clip except `idle2`). Kept as a
/// literal because `ALL_CLIPS` is declared with this length; a test keeps them in sync.
pub const CLIP_SLOTS: usize = 25;

/// One wired scene of the player model: its `AnimationPlayer` and the graph node of every clip
/// it carries (indices are per graph, so each scene keeps its own table).
#[derive(Debug, Clone)]
pub struct SceneAnim {
    pub player: Entity,
    pub nodes: [Option<AnimationNodeIndex>; CLIP_SLOTS],
    /// Face-clip nodes in [`super::face_expr::FACE_CLIP_NAMES`] order (only the face piece
    /// carries them).
    pub face: [Option<AnimationNodeIndex>; super::face_expr::FACE_CLIP_SLOTS],
}

/// Runtime animation state once the model has been wired.
#[derive(Component)]
pub struct PlayerAnimState {
    /// One entry per spawned scene (one worn piece, or the single merged model).
    pub scenes: Vec<SceneAnim>,
    pub current: PlayerClip,
    /// `origin` bone of every scene with its rest translation (root motion is pinned there).
    pub roots: Vec<(Entity, Vec3)>,
    /// Face clip currently playing on the face scene, if any.
    pub face_current: Option<usize>,
}

fn slot(c: PlayerClip) -> usize {
    c as usize
}

fn spawn_scene(
    mut commands: Commands,
    gltfs: Res<Assets<Gltf>>,
    q: Query<(Entity, &PlayerModelSource), (With<PlayerAvatar>, Without<PlayerSceneSpawned>)>,
) {
    for (e, src) in &q {
        let Some(gltf) = gltfs.get(&src.gltf) else { continue };
        let Some(scene) = gltf.default_scene.clone().or_else(|| gltf.scenes.first().cloned()) else {
            continue;
        };
        commands.entity(e).insert((PlayerSceneSpawned, PlayerMotion::default())).with_children(|p| {
            // The glTF origin is at the feet; the avatar root is AVATAR_ROOT_ABOVE_GROUND higher.
            // `PlayerModelScene` is the entity `game::ride` lifts onto the saddle.
            p.spawn((
                PlayerModelScene,
                WorldAssetRoot(scene),
                PlayerSceneRef { gltf: src.gltf.clone(), slot: "merged".into() },
                Transform::from_xyz(0.0, -AVATAR_ROOT_ABOVE_GROUND, 0.0),
                Visibility::default(),
            ));
        });
    }
}

/// Build the animation graph of ONE player scene: the body clips it carries plus its face clips.
/// Returns the player entity, the per-clip node tables and the graph handle.
fn build_scene_graph(
    gltf: &Gltf,
    graphs: &mut Assets<AnimationGraph>,
) -> Option<(AnimationGraphHandle, [Option<AnimationNodeIndex>; CLIP_SLOTS], [Option<AnimationNodeIndex>; super::face_expr::FACE_CLIP_SLOTS])> {
    let clip = |c: PlayerClip| gltf.named_animations.get(c.gltf_name()).cloned();
    let idle = clip(PlayerClip::Idle)?;
    let (mut graph, idle_node) = AnimationGraph::from_clip(idle);
    let mut nodes = [None; CLIP_SLOTS];
    nodes[slot(PlayerClip::Idle)] = Some(idle_node);
    for c in ALL_CLIPS.into_iter().filter(|c| *c != PlayerClip::Idle) {
        nodes[slot(c)] = clip(c).map(|h| graph.add_clip(h, 1.0, graph.root));
    }
    let mut face = [None; super::face_expr::FACE_CLIP_SLOTS];
    for (i, name) in super::face_expr::FACE_CLIP_NAMES.iter().enumerate() {
        face[i] = gltf.named_animations.get(*name).map(|h| graph.add_clip(h.clone(), 1.0, graph.root));
    }
    Some((AnimationGraphHandle(graphs.add(graph)), nodes, face))
}

#[allow(clippy::too_many_arguments)]
fn wire_player(
    mut commands: Commands,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    q: Query<Entity, (With<PlayerSceneSpawned>, Without<PlayerAnimState>)>,
    children: Query<&Children>,
    refs: Query<&PlayerSceneRef>,
    mut players: Query<&mut AnimationPlayer>,
    names: Query<(&Name, &Transform)>,
    mut toast: Option<ResMut<super::save_wire::SaveToast>>,
) {
    for e in &q {
        // Every spawned scene (one per worn piece, or the merged model) must be ready: its glTF
        // loaded and its scene instance produced the AnimationPlayer.
        let scene_roots: Vec<(Entity, Handle<Gltf>, String)> = children
            .iter_descendants(e)
            .filter_map(|d| refs.get(d).ok().map(|r| (d, r.gltf.clone(), r.slot.clone())))
            .collect();
        if scene_roots.is_empty() {
            continue;
        }
        let mut scenes: Vec<SceneAnim> = Vec::new();
        let mut roots: Vec<(Entity, Vec3)> = Vec::new();
        let mut ready = true;
        for (root, gltf_h, _slot_tag) in scene_roots {
            let Some(gltf) = gltfs.get(&gltf_h) else {
                ready = false;
                break;
            };
            let Some(player_e) = children.iter_descendants(root).find(|d| players.get(*d).is_ok()) else {
                ready = false;
                break;
            };
            let Some((graph_h, nodes, face)) = build_scene_graph(gltf, &mut graphs) else {
                ready = false;
                break;
            };
            let mut transitions = AnimationTransitions::new();
            if let Ok(mut player) = players.get_mut(player_e) {
                if let Some(idle) = nodes[slot(PlayerClip::Idle)] {
                    transitions.play(&mut player, idle, Duration::ZERO).repeat();
                }
            }
            commands.entity(player_e).insert((graph_h, transitions));
            if let Some(bone) = children
                .iter_descendants(root)
                .find(|d| names.get(*d).is_ok_and(|(n, _)| n.as_str() == ROOT_MOTION_BONE))
            {
                let rest = names.get(bone).map_or(Vec3::ZERO, |(_, t)| t.translation);
                roots.push((bone, rest));
            }
            scenes.push(SceneAnim { player: player_e, nodes, face });
        }
        if !ready || scenes.is_empty() {
            continue;
        }
        // Outdated export warning (the first scene's glTF carries the clip set).
        if scenes.first().is_some() {
            if let Some(r) = refs.iter().next() {
                if let Some(gltf) = gltfs.get(&r.gltf) {
                    let mut missing = missing_skill_clips(|n| gltf.named_animations.contains_key(n));
                    if !gltf.named_animations.contains_key(PlayerClip::RideWyrdeerIdle.gltf_name()) {
                        missing.push("ride_wyrdeer_idle");
                    }
                    if !missing.is_empty() {
                        let msg = format!(
                            "Player model export is outdated (no {} clip): dodge, crouch and the mounted-rider pose cannot animate. Run ./install.sh to refresh it.",
                            missing.join("/")
                        );
                        eprintln!("warning: {msg}");
                        if let Some(t) = toast.as_deref_mut() {
                            t.show(msg);
                        }
                    }
                }
            }
        }
        commands.entity(e).insert(PlayerAnimState {
            scenes,
            current: PlayerClip::Idle,
            roots,
            face_current: None,
        });
    }
}

fn track_motion(time: Res<Time>, mut q: Query<(&Transform, &mut PlayerMotion), With<PlayerAvatar>>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (xf, mut m) in &mut q {
        let p = xf.translation;
        if let Some(last) = m.last {
            let planar = Vec2::new(p.x - last.x, p.z - last.z).length() / dt;
            // A jump this large is a teleport (battle arena, save load), not running.
            let planar = if planar > 25.0 { 0.0 } else { planar };
            m.speed = m.speed * 0.8 + planar * 0.2;
        }
        m.last = Some(p);
    }
}

/// Everything `switch_clips` reads to pick a clip.
type ClipInputs = (
    &'static mut PlayerAnimState,
    &'static PlayerMotion,
    Option<&'static PlayerSwim>,
    Option<&'static PlayerMotionState>,
    Option<&'static PlayerSkillState>,
);

/// Minimum measured speed (m/s) at which the mounted rider switches to the mount's gait clip
/// (`ride_*_move`). The mounts move at 5-11 m/s, so a low threshold only rejects numeric noise.
pub const RIDE_MOVE_SPEED: f32 = 1.0;

fn switch_clips(
    mut q: Query<ClipInputs>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    ride: Option<Res<super::ride::RideState>>,
) {
    // While a mount is ridden the player plays the game's own rider pose for that mount
    // (`ride_<mount>_idle` / `ride_<mount>_move`, Brief 38 follow-up); exports without the
    // clips fall back to `Idle` through the normal missing-clip chain.
    let mounted = ride.as_ref().and_then(|r| r.mounted);
    for (mut st, motion, swim, air, skills) in &mut q {
        let swimming = swim.is_some_and(|s| s.state == SwimState::Swimming);
        let mut want = if let Some(kind) = mounted {
            let (idle, moving) = rider_clips(kind);
            match moving {
                Some(m) if motion.speed > RIDE_MOVE_SPEED => m,
                _ => idle,
            }
        } else {
            select_clip_motion(st.current, motion.speed, swimming, air, skills)
        };
        // A clip missing from the file (an older cached export) falls back along `fallback_clip`.
        while want != PlayerClip::Idle
            && !st.scenes.iter().any(|s| s.nodes[slot(want)].is_some())
        {
            want = fallback_clip(want);
        }
        if want == st.current {
            continue;
        }
        let mut played = false;
        for scene in &st.scenes {
            let Some(node) = scene.nodes[slot(want)] else { continue };
            if let Ok((mut player, mut tr)) = players.get_mut(scene.player) {
                tr.play(&mut player, node, FADE).repeat();
                played = true;
            }
        }
        if played {
            st.current = want;
        }
    }
}

/// Runs after the animation system wrote the bone transforms and before they propagate.
fn pin_root_motion(q: Query<&PlayerAnimState>, mut bones: Query<&mut Transform>) {
    for st in &q {
        for (bone, rest) in &st.roots {
            if let Ok(mut t) = bones.get_mut(*bone) {
                let fixed = strip_root_motion(t.translation, *rest);
                if fixed != t.translation {
                    t.translation = fixed;
                }
            }
        }
    }
}

/// Ordering anchor: everything that must run after the player's clip switching (the facial
/// expressions, `game::face_expr`) is ordered `.after(PlayerAnimSystems)`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayerAnimSystems;

/// Adds the player model animation systems. Requires the same reflect-type registration
/// as `PokemonAnimPlugin` (idempotent), so it works with or without that plugin.
pub struct PlayerAnimPlugin;

impl Plugin for PlayerAnimPlugin {
    fn build(&self, app: &mut App) {
        app.world().resource::<AppTypeRegistry>().write().register_derived_types();
        app.add_systems(Update, (spawn_scene, wire_player, track_motion, switch_clips).chain().in_set(PlayerAnimSystems));
        app.add_systems(
            PostUpdate,
            pin_root_motion
                .after(bevy::app::AnimationSystems)
                .before(bevy::transform::TransformSystems::Propagate),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PlayerClip::*;

    #[test]
    fn tiers_from_rest() {
        assert_eq!(select_clip(Idle, 0.0), Idle);
        assert_eq!(select_clip(Idle, 0.3), Idle);
        assert_eq!(select_clip(Idle, 1.5), Walk);
        assert_eq!(select_clip(Idle, 4.5), Run);
        assert_eq!(select_clip(Idle, 8.0), Dash);
        assert_eq!(select_clip(Dash, 0.0), Idle);
    }

    #[test]
    fn hysteresis_holds_inside_band() {
        // Walk stays walk down to 0.2 even though 0.3 < WALK_UP.
        assert_eq!(select_clip(Walk, 0.25), Walk);
        assert_eq!(select_clip(Walk, 0.15), Idle);
        assert_eq!(select_clip(Idle, 0.25), Idle);
        // Run <-> walk around 4 m/s.
        assert_eq!(select_clip(Run, 3.9), Run);
        assert_eq!(select_clip(Run, 3.5), Walk);
        assert_eq!(select_clip(Walk, 4.0), Walk);
        assert_eq!(select_clip(Walk, 4.3), Run);
        // Sprint band.
        assert_eq!(select_clip(Dash, 6.0), Dash);
        assert_eq!(select_clip(Dash, 5.5), Run);
        assert_eq!(select_clip(Run, 6.2), Run);
        assert_eq!(select_clip(Run, 6.6), Dash);
    }

    #[test]
    fn swimming_forces_the_swim_clip() {
        // In water the clip is swim whatever the speed is.
        assert_eq!(select_clip_full(Idle, 0.0, true), Swim);
        assert_eq!(select_clip_full(Idle, 8.0, true), Swim);
        // Leaving the water picks a land clip from a neutral state, not from Swim.
        assert_eq!(select_clip_full(Swim, 0.0, false), Idle);
        assert_eq!(select_clip_full(Swim, 1.5, false), Walk);
        assert_eq!(select_clip_full(Swim, 4.5, false), Run);
        // On land the previous behaviour is unchanged.
        assert_eq!(select_clip_full(Walk, 4.3, false), Run);
        assert_eq!(select_clip_full(Run, 0.1, false), Idle);
        // swim is a real glTF name in the exported player model.
        assert_eq!(Swim.gltf_name(), "swim");
    }

    #[test]
    fn jump_fall_and_land_clips_come_from_the_motion_state() {
        let mut m = PlayerMotionState {
            mode: MotionMode::Airborne,
            vertical_speed: 2.0,
            ..default()
        };
        assert_eq!(select_clip_motion(Idle, 0.0, false, Some(&m), None), JumpStart);
        m.vertical_speed = 0.0;
        assert_eq!(select_clip_motion(Idle, 0.0, false, Some(&m), None), JumpFall);
        m.vertical_speed = -4.0;
        assert_eq!(select_clip_motion(Idle, 0.0, false, Some(&m), None), JumpFall);
        // Touchdown plays the land clip while its timer runs.
        m.mode = MotionMode::Grounded;
        m.vertical_speed = 0.0;
        m.land_timer = crate::game::player_motion::LAND_TIME;
        assert_eq!(select_clip_motion(Idle, 0.0, false, Some(&m), None), Land);
        m.land_timer = 0.0;
        assert_eq!(select_clip_motion(Idle, 0.0, false, Some(&m), None), Idle);
        // Climbing reuses walk (no climb clip exists in the export).
        m.mode = MotionMode::Climbing;
        assert_eq!(select_clip_motion(Idle, 0.0, false, Some(&m), None), Walk);
        // Water beats every motion state.
        m.mode = MotionMode::Airborne;
        m.vertical_speed = 1.0;
        assert_eq!(select_clip_motion(Walk, 4.0, true, Some(&m), None), Swim);
        // Without a motion state the speed rules are unchanged.
        assert_eq!(select_clip_motion(Idle, 4.5, false, None, None), Run);
        // All new clip names are real glTF animation names.
        for (clip, name) in [
            (JumpStart, "jump_start"),
            (JumpFall, "jump_fall"),
            (Land, "land"),
        ] {
            assert_eq!(clip.gltf_name(), name);
        }
    }

    #[test]
    fn clip_slots_match_the_enum_and_the_gltf_names_are_unique() {
        assert_eq!(ALL_CLIPS.len(), CLIP_SLOTS);
        for (i, c) in ALL_CLIPS.iter().enumerate() {
            assert_eq!(*c as usize, i, "{c:?} sits in slot {i}");
        }
        let mut names: Vec<_> = ALL_CLIPS.iter().map(|c| c.gltf_name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), CLIP_SLOTS);
    }

    #[test]
    fn dodge_crouch_and_priority_rules() {
        use crate::game::player_skills::{ActiveDodge, PlayerSkillState, ROLL_DURATION};
        let mut sk = PlayerSkillState::default();
        // Crouch cycle: enter, hold (idle and walking), leave.
        sk.toggle_crouch();
        assert_eq!(select_clip_motion(Idle, 0.0, false, None, Some(&sk)), CrouchStart);
        sk.tick(1.0);
        assert_eq!(select_clip_motion(Idle, 0.0, false, None, Some(&sk)), Crouch);
        assert_eq!(select_clip_motion(Crouch, 1.5, false, None, Some(&sk)), CrouchWalk);
        sk.toggle_crouch();
        assert_eq!(select_clip_motion(Crouch, 0.0, false, None, Some(&sk)), CrouchEnd);
        sk.tick(1.0);
        assert_eq!(select_clip_motion(CrouchEnd, 0.0, false, None, Some(&sk)), Idle);

        // Rolls pick their clip from the direction and beat the crouch and the air.
        for (dir, clip) in [
            (DodgeDir::Forward, RollF),
            (DodgeDir::Back, RollB),
            (DodgeDir::Left, RollL),
            (DodgeDir::Right, RollR),
        ] {
            let mut sk = PlayerSkillState::default();
            sk.toggle_crouch();
            sk.dodge = Some(ActiveDodge { dir, world_dir: Vec2::Y, elapsed: 0.0 });
            let air = PlayerMotionState { mode: MotionMode::Airborne, ..default() };
            assert_eq!(select_clip_motion(Idle, 0.0, false, Some(&air), Some(&sk)), clip);
        }
        // Water still wins over everything.
        let mut sk = PlayerSkillState {
            dodge: Some(ActiveDodge { dir: DodgeDir::Forward, world_dir: Vec2::Y, elapsed: 0.0 }),
            ..default()
        };
        assert_eq!(select_clip_motion(Idle, 0.0, true, None, Some(&sk)), Swim);
        // The roll ends after the clip length and the speed tiers take over again.
        sk.tick(ROLL_DURATION + 0.01);
        assert_eq!(select_clip_motion(RollF, 4.5, false, None, Some(&sk)), Run);
    }

    #[test]
    fn outdated_player_exports_are_detected() {
        let nine = ["idle", "idle2", "walk", "run", "dash", "jump_start", "jump_fall", "land", "swim"];
        assert_eq!(
            missing_skill_clips(|n| nine.contains(&n)),
            vec!["roll_f", "crouch", "crouch_walk", "crouch_start"]
        );
        assert!(missing_skill_clips(|_| true).is_empty());
    }

    /// The real cache export carries every clip the mapping asks for (skipped without a cache).
    #[test]
    fn real_player_export_has_every_mapped_clip() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../work/cache/player");
        for variant in ["p1_default", "p2_default"] {
            let Ok(text) = std::fs::read_to_string(root.join(variant).join("player.gltf")) else {
                eprintln!("[skip] no player export");
                return;
            };
            for c in ALL_CLIPS {
                assert!(
                    text.contains(&format!("\"{}\"", c.gltf_name())),
                    "{variant}: clip {} missing from the export",
                    c.gltf_name()
                );
            }
        }
    }

    #[test]
    fn missing_clips_fall_back_to_something_that_exists() {
        // Every chain ends at Idle and never loops.
        for c in ALL_CLIPS {
            let mut x = c;
            for _ in 0..6 {
                if x == Idle {
                    break;
                }
                x = fallback_clip(x);
            }
            assert_eq!(x, Idle, "{c:?} never reaches idle");
        }
        assert_eq!(fallback_clip(RollL), RollF);
        assert_eq!(fallback_clip(RollF), Dash);
        assert_eq!(fallback_clip(CrouchWalk), Crouch);
    }

    #[test]
    fn jittering_speed_does_not_flicker() {
        let mut c = Walk;
        let mut changes = 0;
        for i in 0..200 {
            let s = 4.2 + if i % 2 == 0 { 0.15 } else { -0.15 };
            let n = select_clip(c, s);
            if n != c {
                changes += 1;
                c = n;
            }
        }
        assert_eq!(changes, 1, "one switch to Run, then stable");
        assert_eq!(c, Run);
    }

    #[test]
    fn root_motion_is_stripped_on_a_synthetic_walk_clip() {
        // Keyframes like the exported walk clip: origin goes 0 -> 1.714 m forward (+Z).
        let rest = Vec3::ZERO;
        let keys: Vec<Vec3> = (0..=68).map(|i| Vec3::new(0.0, 0.0, 1.714 * i as f32 / 68.0)).collect();
        assert!(keys.last().unwrap().z > 1.7);
        for k in &keys {
            assert_eq!(strip_root_motion(*k, rest), Vec3::ZERO);
        }
        // Sideways drift and a non-zero rest pose are handled; vertical motion is kept.
        let rest = Vec3::new(0.1, 0.0, -0.2);
        assert_eq!(strip_root_motion(Vec3::new(0.5, 0.3, 2.0), rest), Vec3::new(0.1, 0.3, -0.2));
    }

    #[test]
    fn facing_turns_toward_movement_and_is_stable() {
        let start = Quat::IDENTITY; // facing +Z
        // Moving +X: target yaw +90 degrees.
        let mut q = start;
        for _ in 0..120 {
            q = step_facing(q, Vec2::new(1.0, 0.0), 1.0 / 60.0);
        }
        let fwd = q * Vec3::Z;
        assert!((fwd - Vec3::X).length() < 1e-3, "{fwd:?}");
        // Zero input keeps the facing.
        assert_eq!(step_facing(q, Vec2::ZERO, 0.016), q);
        // One short step only partly turns.
        let one = step_facing(start, Vec2::new(1.0, 0.0), 1.0 / 60.0);
        let angle = (one * Vec3::Z).angle_between(Vec3::Z);
        assert!(angle > 0.0 && angle < std::f32::consts::FRAC_PI_2);
        // A huge dt clamps to the target instead of overshooting.
        let snap = step_facing(start, Vec2::new(0.0, -1.0), 10.0);
        assert!(((snap * Vec3::Z) - Vec3::NEG_Z).length() < 1e-3);
    }

    #[test]
    fn camera_heights_follow_the_model() {
        let (focal, shoulder) = camera_offsets(MODEL_HEIGHT);
        // Focus ~1.2 m above the ground, shoulders ~1.28 m (root is 0.9 above ground).
        assert!((focal + AVATAR_ROOT_ABOVE_GROUND - 1.2).abs() < 1e-5);
        assert!((shoulder + AVATAR_ROOT_ABOVE_GROUND - 1.275).abs() < 1e-5);
        assert!(focal < 1.2, "lower than the capsule focus");
    }

    #[test]
    fn clip_slots_cover_every_clip_and_the_gltf_names_are_unique() {
        assert_eq!(CLIP_SLOTS, ALL_CLIPS.len(), "slot table out of sync");
        let mut names: Vec<&str> = ALL_CLIPS.iter().map(|c| c.gltf_name()).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate glTF clip name");
        // Every ride clip pair resolves and its move falls back to its idle, then to Idle.
        for kind in super::super::ride::RideKind::ALL {
            let (idle, moving) = rider_clips(kind);
            assert!(idle.gltf_name().starts_with("ride_"));
            if let Some(m) = moving {
                assert_eq!(fallback_clip(m), idle, "{kind:?}");
            }
            assert_eq!(fallback_clip(idle), PlayerClip::Idle, "{kind:?}");
        }
    }

    #[test]
    fn model_path_resolution() {
        let dir = std::env::temp_dir().join(format!("pla-player-anim-{}", std::process::id()));
        let model = dir.join("player/p2_default");
        std::fs::create_dir_all(&model).unwrap();
        std::fs::write(model.join("player.gltf"), "{}").unwrap();
        assert_eq!(resolve_player_model(&dir, "p2"), Some(model.join("player.gltf")));
        assert_eq!(resolve_player_model(&dir, "p1"), None, "missing file");
        assert_eq!(resolve_player_model(&dir, "p3"), None, "unknown variant");
        let rel = asset_path(&model.join("player.gltf"), &dir);
        assert_eq!(rel, "player/p2_default/player.gltf");
        assert_eq!(asset_path(Path::new("/x/y.gltf"), &dir), "/x/y.gltf");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
