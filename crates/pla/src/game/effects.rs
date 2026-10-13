//! Battle move effects: plays the game's own particle effects and timelines in battles.
//!
//! * A move's look is a timeline (`bin/effect/battle/sequence/ewNNN.wbin`, file name rule
//!   VERIFIED, see `reports/data-progress/effects.md`) that starts particle files
//!   (`bin/effect/battle/particle/**.ptcl`) on the user or the target at given frames.
//! * [`EffectsPlugin`] watches the [`ActiveBattleSession`] events (like the animations and
//!   the hit sounds) and plays: the move's timeline when a move is used (target particles left
//!   out on a miss), the critical / super-effective / not-very-effective marks when the target
//!   is hit, and the status timelines (`es001`-`es005`) when a status is applied.
//! * Other code can play any timeline or particle file with a [`PlayEffect`] message.
//! * Particles are simulated on the CPU ([`sim`]) and drawn as one quad mesh per emitter with a
//!   small custom material ([`material`]: alpha, additive and subtractive blending, no depth
//!   writes), rebuilt each frame to face the camera. Live particles are capped per effect and in
//!   total ([`sim::Quality`]); finished effects despawn themselves.
//! * `--effects off|low|high` (default low; also `PLA_EFFECTS` or `effects=` in `settings.txt`).
//!   Off plays nothing and keeps the old fixed battle pacing.
//! * With effects on, the battle pacing comes from the timelines ([`MoveTimingTable`]): a move
//!   lasts its timeline and the target reacts when the first particle lands on it; the
//!   animations and hit sounds read the same table, so all three stay in sync.
//!
//! What is decoded vs assumed is listed in [`sim`], [`timeline`] and
//! `reports/data-progress/effects_game.md`. Never compared with the real game by eye.

pub mod library;
pub mod material;
pub mod sim;
pub mod timeline;

use std::path::PathBuf;
use std::sync::Arc;

use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use self::library::{EffectLibrary, ParticleSlot, ReadyParticle};
use self::material::EffectMaterial;
use self::sim::{build_emitter_quads, EffectSim, Placement, QuadBuffers, Quality, ViewAxes, DEFAULT_EMIT_FRAMES, FPS};
use self::timeline::{effects_for_events, EffectTimeline, PlaySide};
pub use self::timeline::MoveTimingTable;
use super::battle_anim::{slot_in, MoveTiming};
use super::battle_controller::ActiveBattleSession;
use super::pokemon_anim::PokemonBattleSlot;
use crate::assets::effect_sequence::sequence_name;
use crate::battle_engine::{Battle, MajorStatus};

/// World metres per effect unit (ASSUMED: one unit = one metre, like the field and model
/// data; with it the decoded sizes give ~0.5-1 m hit flashes next to ~0.4-1 m Pokémon).
pub const WORLD_SCALE: f32 = 1.0;
/// Height above a Pokémon's feet where its effects are centred (ASSUMED: no bone locators yet).
pub const ANCHOR_HEIGHT_M: f32 = 0.5;
/// A particle play still waiting for its file after this many frames is dropped.
pub const MAX_LOAD_WAIT_FRAMES: f32 = 45.0;
/// Safety limit on a particle instance's life, in frames.
pub const MAX_INSTANCE_FRAMES: f32 = 30.0 * 20.0;

/// Pacing of a move used by `actor` (by move name) from the table, or the fixed default.
pub fn move_timing_of(table: Option<&MoveTimingTable>, battle: &Battle, actor: usize, name: &str) -> MoveTiming {
    let Some(t) = table else { return MoveTiming::default() };
    battle
        .combatants
        .get(actor)
        .and_then(|c| {
            let m = c.moves.iter().find(|m| m.data.name == name)?;
            t.timing(m.data.id, c.species.dex, 0)
        })
        .unwrap_or_default()
}

/// What to play.
#[derive(Debug, Clone, PartialEq)]
pub enum EffectRef {
    /// A timeline by file stem (`ew085`, `cmn_critical`, `es002`).
    Sequence(String),
    /// The timeline of a move for a species/form (species overrides applied).
    Move { id: u16, species: u16, form: u8 },
    /// One particle file (`ew052/ew052_hit.ptcl`), emitting for [`DEFAULT_EMIT_FRAMES`] on the user.
    Particle(String),
}

/// Where a side of a timeline is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EffectAnchor {
    /// A battle arena slot ([`PokemonBattleSlot`]), at [`ANCHOR_HEIGHT_M`] above its feet.
    Slot(usize),
    /// A fixed world point.
    Point(Vec3),
}

/// Request to play an effect.
#[derive(Message, Debug, Clone, PartialEq)]
pub struct PlayEffect {
    pub effect: EffectRef,
    pub user: EffectAnchor,
    pub target: EffectAnchor,
    /// Seconds before it starts.
    pub delay: f32,
    /// Leave out the plays on the target.
    pub skip_target: bool,
}

impl PlayEffect {
    pub fn at(effect: EffectRef, user: EffectAnchor, target: EffectAnchor) -> Self {
        PlayEffect { effect, user, target, delay: 0.0, skip_target: false }
    }
}

/// Current settings.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct EffectsSettings {
    pub quality: Quality,
    pub world_scale: f32,
    pub anchor_height: f32,
}

/// Live counters (tests, debugging).
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectStats {
    pub live_particles: usize,
    pub live_instances: usize,
    pub sequences_started: usize,
    pub particle_instances_started: usize,
    pub plays_dropped: usize,
    pub particles_dropped_by_cap: u64,
}

/// A playing timeline.
#[derive(Component, Debug, Clone)]
pub struct EffectSequenceInstance {
    pub timeline: Arc<EffectTimeline>,
    pub user: EffectAnchor,
    pub target: EffectAnchor,
    pub delay: f32,
    pub skip_target: bool,
    /// Local time in frames.
    pub frame: f32,
    /// Plays that are due but whose file is still loading: (play index, frames waited).
    pending: Vec<(usize, f32)>,
}

/// A playing particle file.
#[derive(Component, Debug, Clone)]
pub struct ParticleInstance {
    pub sim: EffectSim,
    pub rotation: Quat,
    pub scale: f32,
}

/// One emitter's mesh, child of a [`ParticleInstance`].
#[derive(Component, Debug, Clone)]
pub struct EmitterMesh {
    pub emitter: usize,
    pub mesh: Handle<Mesh>,
}

/// Plays the game's battle effects. See the module documentation.
pub struct EffectsPlugin {
    /// `None` = from `--effects`, `PLA_EFFECTS`, `settings.txt` (default low).
    pub quality: Option<Quality>,
    pub bin_root: Option<PathBuf>,
    pub cache_dir: Option<PathBuf>,
    /// Directory with `move_effect_map.tsv` (searched in the usual places when `None`).
    pub data_dir: Option<PathBuf>,
    /// Load particle files on the main thread (tests, tools).
    pub sync_loading: bool,
}

impl Default for EffectsPlugin {
    fn default() -> Self {
        EffectsPlugin { quality: None, bin_root: None, cache_dir: None, data_dir: None, sync_loading: false }
    }
}

impl EffectsPlugin {
    /// Settings from the command line, `PLA_EFFECTS` and `settings.txt`.
    pub fn from_env_and_args() -> Self {
        Self::default()
    }

    pub fn with_quality(quality: Quality) -> Self {
        EffectsPlugin { quality: Some(quality), ..Default::default() }
    }

    fn resolve_quality(&self) -> Quality {
        if let Some(q) = self.quality {
            return q;
        }
        let args: Vec<String> = std::env::args().collect();
        let env = std::env::var("PLA_EFFECTS").ok();
        let settings = std::fs::read_to_string(
            super::settings::UserSettings::default_dir().join(super::settings::UserSettings::FILE_NAME),
        )
        .ok();
        sim::quality_from(&args, env.as_deref(), settings.as_deref())
    }
}

/// `data/` directory with `move_effect_map.tsv`.
pub fn find_data_dir() -> Option<PathBuf> {
    [
        PathBuf::from("data"),
        PathBuf::from("../../data"),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        PathBuf::from("/home/nico/work/decompilacion/data"),
    ]
    .into_iter()
    .find(|p| p.join("move_effect_map.tsv").is_file())
}

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        let quality = self.resolve_quality();
        info!("effects: quality {}", quality.name());
        app.add_message::<PlayEffect>()
            .insert_resource(EffectsSettings { quality, world_scale: WORLD_SCALE, anchor_height: ANCHOR_HEIGHT_M })
            .init_resource::<EffectStats>()
            .insert_resource(EffectLibrary::new(
                self.bin_root.clone().unwrap_or_else(crate::assets::ptcl_textures::default_bin_root),
                Some(self.cache_dir.clone().unwrap_or_else(crate::assets::ptcl_textures::effects_cache_dir)),
                self.sync_loading,
            ));
        if quality == Quality::Off {
            return;
        }
        let table = self
            .data_dir
            .clone()
            .or_else(find_data_dir)
            .and_then(|d| std::fs::read_to_string(d.join("move_effect_map.tsv")).ok())
            .and_then(|t| MoveTimingTable::parse(&t).map_err(|e| warn!("effects: move_effect_map.tsv: {e}")).ok());
        match table {
            Some(t) => {
                app.insert_resource(t);
            }
            None => warn!("effects: data/move_effect_map.tsv not found; battles keep the fixed pacing"),
        }
        let render = app.is_plugin_added::<bevy::pbr::PbrPlugin>();
        if render {
            material::register_shader(app);
            app.add_plugins(MaterialPlugin::<EffectMaterial>::default());
        }
        app.add_systems(
            Update,
            (
                drive_battle_effects,
                start_effects,
                receive_loads,
                tick_sequences,
                tick_particles,
                draw_particles.run_if(move || render),
            )
                .chain(),
        );
    }
}

/// Position of an anchor (`None` when its slot has no creature).
fn anchor_pos(a: EffectAnchor, slots: &Query<(&GlobalTransform, &PokemonBattleSlot)>, height: f32) -> Option<Vec3> {
    match a {
        EffectAnchor::Point(p) => Some(p),
        EffectAnchor::Slot(s) => slots.iter().find(|(_, b)| b.0 == s).map(|(t, _)| t.translation() + Vec3::Y * height),
    }
}

/// Battle hook: timelines for the new events of the battle session.
#[allow(clippy::too_many_arguments)]
fn drive_battle_effects(
    mut commands: Commands,
    session: Option<Res<ActiveBattleSession>>,
    mut cursor: Local<usize>,
    mut in_battle: Local<bool>,
    timings: Option<Res<MoveTimingTable>>,
    mut library: ResMut<EffectLibrary>,
    mut out: MessageWriter<PlayEffect>,
    instances: Query<Entity, Or<(With<EffectSequenceInstance>, With<ParticleInstance>)>>,
) {
    let Some(sess) = session else {
        if *in_battle {
            // Battle over: clear its effects and bound the caches.
            for e in &instances {
                commands.entity(e).despawn();
            }
            if library.particle_count() > 96 {
                library.trim();
            }
        }
        *in_battle = false;
        *cursor = 0;
        return;
    };
    if !*in_battle {
        *in_battle = true;
        *cursor = sess.event_log.len();
        // Preload every move timeline of this battle and the shared marks.
        let mut names: Vec<String> = sess
            .battle
            .combatants
            .iter()
            .flat_map(|c| c.moves.iter().map(move |m| sequence_name(m.data.id, c.species.dex, 0)))
            .collect();
        names.extend(
            [timeline::CRITICAL_SEQUENCE, timeline::SUPER_EFFECTIVE_SEQUENCE, timeline::NOT_VERY_EFFECTIVE_SEQUENCE]
                .map(String::from),
        );
        names.extend(
            [MajorStatus::Drowsy, MajorStatus::Poison, MajorStatus::Burn, MajorStatus::Frostbite, MajorStatus::Paralysis]
                .map(|s| timeline::status_sequence(s).to_string()),
        );
        for n in names {
            if let Some(t) = library.timeline(&n) {
                for p in t.particle_paths() {
                    library.request(&p);
                }
            }
        }
    }
    let len = sess.event_log.len();
    if len < *cursor {
        *cursor = 0;
    }
    if len == *cursor {
        return;
    }
    let battle = &sess.battle;
    let move_of = |actor: usize, name: &str| -> Option<(u16, u16)> {
        let c = battle.combatants.get(actor)?;
        let m = c.moves.iter().find(|m| m.data.name == name)?;
        Some((m.data.id, c.species.dex))
    };
    let timing = |actor: usize, name: &str| move_timing_of(timings.as_deref(), battle, actor, name);
    for fx in effects_for_events(&sess.event_log[*cursor..], &move_of, &timing) {
        out.write(PlayEffect {
            effect: EffectRef::Sequence(fx.sequence),
            user: EffectAnchor::Slot(slot_in(battle, fx.user)),
            target: EffectAnchor::Slot(slot_in(battle, fx.target)),
            delay: fx.delay,
            skip_target: fx.skip_target,
        });
    }
    *cursor = len;
}

/// Turns [`PlayEffect`] messages into timeline instances and requests their files.
fn start_effects(
    mut commands: Commands,
    mut msgs: MessageReader<PlayEffect>,
    settings: Res<EffectsSettings>,
    mut library: ResMut<EffectLibrary>,
    mut stats: ResMut<EffectStats>,
) {
    for m in msgs.read() {
        if settings.quality == Quality::Off {
            continue;
        }
        let timeline = match &m.effect {
            EffectRef::Sequence(name) => library.timeline(name),
            EffectRef::Move { id, species, form } => library.timeline(&sequence_name(*id, *species, *form)),
            EffectRef::Particle(path) => Some(Arc::new(EffectTimeline::single(path, DEFAULT_EMIT_FRAMES))),
        };
        let Some(timeline) = timeline else { continue };
        for p in &timeline.plays {
            if !(m.skip_target && p.side == PlaySide::Target) {
                library.request(&p.path);
            }
        }
        stats.sequences_started += 1;
        commands.spawn((
            Name::new(format!("effect {}", timeline.name)),
            EffectSequenceInstance {
                timeline,
                user: m.user,
                target: m.target,
                delay: m.delay.max(0.0),
                skip_target: m.skip_target,
                frame: 0.0,
                pending: Vec::new(),
            },
        ));
    }
}

fn receive_loads(mut library: ResMut<EffectLibrary>, images: Option<ResMut<Assets<Image>>>) {
    let mut images = images;
    library.receive(images.as_deref_mut());
}

/// Advances timelines and starts their particle files.
#[allow(clippy::too_many_arguments)]
fn tick_sequences(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<EffectsSettings>,
    mut library: ResMut<EffectLibrary>,
    mut stats: ResMut<EffectStats>,
    mut seqs: Query<(Entity, &mut EffectSequenceInstance)>,
    slots: Query<(&GlobalTransform, &PokemonBattleSlot)>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<EffectMaterial>>>,
) {
    let dt = time.delta_secs();
    for (entity, mut s) in &mut seqs {
        let mut dt_left = dt;
        if s.delay > 0.0 {
            s.delay -= dt;
            if s.delay > 0.0 {
                continue;
            }
            dt_left = -s.delay;
            s.delay = 0.0;
        }
        // Plays starting in [previous frame, new frame): each exactly once.
        let from = s.frame;
        s.frame += dt_left * FPS;
        let due = s.timeline.due(from, s.frame);
        s.pending.extend(due.into_iter().map(|i| (i, 0.0)));

        let user = anchor_pos(s.user, &slots, settings.anchor_height);
        let target = anchor_pos(s.target, &slots, settings.anchor_height);
        let mut keep = Vec::new();
        for (i, waited) in std::mem::take(&mut s.pending) {
            let play = &s.timeline.plays[i];
            if s.skip_target && play.side == PlaySide::Target {
                continue;
            }
            let ready = match library.slot(&play.path) {
                Some(ParticleSlot::Ready(r)) => r.clone(),
                Some(ParticleSlot::Loading) if waited < MAX_LOAD_WAIT_FRAMES => {
                    keep.push((i, waited + dt_left * FPS));
                    continue;
                }
                None => {
                    library.request(&play.path);
                    keep.push((i, waited + dt_left * FPS));
                    continue;
                }
                _ => {
                    stats.plays_dropped += 1;
                    continue;
                }
            };
            let origin = match play.side {
                PlaySide::User => user,
                PlaySide::Target => target,
                PlaySide::Field => user.zip(target).map(|(a, b)| (a + b) * 0.5),
            };
            let Some(origin) = origin else {
                stats.plays_dropped += 1;
                continue;
            };
            // Effect local +Z points from the user to the target (ASSUMED).
            let rotation = match user.zip(target) {
                Some((a, b)) if (b - a).with_y(0.0).length_squared() > 1e-6 => {
                    let d = (b - a).with_y(0.0).normalize();
                    Quat::from_rotation_y(d.x.atan2(d.z))
                }
                _ => Quat::IDENTITY,
            };
            let seed = (entity.to_bits() << 8) ^ i as u64;
            spawn_particle_instance(
                &mut commands,
                &ready,
                play.emit_frames(),
                origin,
                rotation,
                &settings,
                seed,
                &mut library,
                meshes.as_deref_mut(),
                materials.as_deref_mut(),
            );
            stats.particle_instances_started += 1;
        }
        s.pending = keep;
        if s.frame >= s.timeline.length_frames as f32 && s.pending.is_empty() {
            commands.entity(entity).despawn();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_particle_instance(
    commands: &mut Commands,
    ready: &ReadyParticle,
    emit_frames: u32,
    origin: Vec3,
    rotation: Quat,
    settings: &EffectsSettings,
    seed: u64,
    library: &mut EffectLibrary,
    meshes: Option<&mut Assets<Mesh>>,
    materials: Option<&mut Assets<EffectMaterial>>,
) {
    if !ready.def.is_playable() {
        return;
    }
    let sim = EffectSim::new(ready.def.clone(), emit_frames, settings.quality, seed);
    let mut e = commands.spawn((
        Name::new(format!("particles {}", ready.def.path)),
        ParticleInstance { sim, rotation, scale: settings.world_scale },
        Transform::from_translation(origin),
        Visibility::default(),
    ));
    let (Some(meshes), Some(materials)) = (meshes, materials) else { return };
    e.with_children(|c| {
        for (i, em) in ready.def.emitters.iter().enumerate() {
            let image = em.texture.as_ref().and_then(|t| ready.images.get(t));
            let mat = library.material(em.texture.as_deref(), image, em.blend, em.wrap(), materials);
            let mesh = meshes.add(quad_mesh(&QuadBuffers::default()));
            c.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat),
                Transform::IDENTITY,
                Visibility::Hidden,
                NoFrustumCulling,
                EmitterMesh { emitter: i, mesh },
            ));
        }
    });
}

/// Steps every particle instance under the shared caps and despawns finished ones.
fn tick_particles(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<EffectsSettings>,
    mut stats: ResMut<EffectStats>,
    mut q: Query<(Entity, &mut ParticleInstance)>,
) {
    let dt_frames = time.delta_secs() * FPS;
    let live: usize = q.iter().map(|(_, p)| p.sim.live()).sum();
    let mut budget = settings.quality.total_cap().saturating_sub(live);
    let mut total = 0;
    let mut instances = 0;
    let mut dropped = 0;
    for (e, mut p) in &mut q {
        p.sim.step(dt_frames, settings.quality.per_effect_cap(), &mut budget);
        dropped += p.sim.dropped;
        if p.sim.finished() || p.sim.frame > MAX_INSTANCE_FRAMES {
            commands.entity(e).despawn();
        } else {
            total += p.sim.live();
            instances += 1;
        }
    }
    stats.live_particles = total;
    stats.live_instances = instances;
    stats.particles_dropped_by_cap = stats.particles_dropped_by_cap.max(dropped);
}

/// Empty-safe triangle-list mesh from quad buffers.
fn quad_mesh(b: &QuadBuffers) -> Mesh {
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    write_quads(&mut m, b);
    m
}

fn write_quads(m: &mut Mesh, b: &QuadBuffers) {
    if b.positions.is_empty() {
        // One degenerate triangle keeps the buffers non-empty; the entity is hidden anyway.
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32; 2]; 3]);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3]);
        m.insert_indices(Indices::U32(vec![0, 1, 2]));
        return;
    }
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, b.positions.clone());
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, b.uvs.clone());
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, b.colors.clone());
    m.insert_indices(Indices::U32(b.indices.clone()));
}

/// Rebuilds the emitter meshes to face the active 3D camera.
fn draw_particles(
    cameras: Query<(&GlobalTransform, &Camera), With<Camera3d>>,
    instances: Query<(&ParticleInstance, &Children)>,
    mut emitters: Query<(&EmitterMesh, &mut Visibility)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut buf: Local<QuadBuffers>,
) {
    let view = cameras
        .iter()
        .find(|(_, c)| c.is_active)
        .map(|(t, _)| ViewAxes { right: t.right().as_vec3(), up: t.up().as_vec3(), forward: t.forward().as_vec3() })
        .unwrap_or_default();
    for (inst, children) in &instances {
        let place = Placement { origin: Vec3::ZERO, rotation: inst.rotation, scale: inst.scale };
        for child in children.iter() {
            let Ok((em, mut vis)) = emitters.get_mut(child) else { continue };
            buf.clear();
            build_emitter_quads(&inst.sim, em.emitter, &place, &view, &mut buf);
            let want = if buf.positions.is_empty() { Visibility::Hidden } else { Visibility::Inherited };
            if *vis != want {
                *vis = want;
            }
            if let Some(mut m) = meshes.get_mut(&em.mesh) {
                write_quads(&mut m, &buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use bevy::time::TimeUpdateStrategy;

    fn app(quality: Quality, bin_root: Option<PathBuf>) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 30.0)));
        app.add_plugins(EffectsPlugin {
            quality: Some(quality),
            bin_root,
            cache_dir: Some(std::env::temp_dir().join(format!("pla_effects_test_{}", std::process::id()))),
            data_dir: None,
            sync_loading: true,
        });
        app
    }

    fn count<C: Component>(app: &mut App) -> usize {
        app.world_mut().query::<&C>().iter(app.world()).count()
    }

    fn real_root() -> Option<PathBuf> {
        let r = crate::assets::ptcl_textures::default_bin_root();
        r.join("effect/battle/sequence/ew052.wbin").exists().then_some(r)
    }

    #[test]
    fn off_quality_plays_nothing() {
        let mut app = app(Quality::Off, Some(PathBuf::from("/nonexistent")));
        app.world_mut().write_message(PlayEffect::at(
            EffectRef::Particle("x.ptcl".into()),
            EffectAnchor::Point(Vec3::ZERO),
            EffectAnchor::Point(Vec3::X),
        ));
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(count::<EffectSequenceInstance>(&mut app), 0);
        assert!(app.world().get_resource::<MoveTimingTable>().is_none(), "off keeps the fixed pacing");
    }

    #[test]
    fn missing_files_are_dropped_without_panicking() {
        let mut app = app(Quality::Low, Some(PathBuf::from("/nonexistent")));
        app.world_mut().write_message(PlayEffect::at(
            EffectRef::Sequence("ew999".into()),
            EffectAnchor::Point(Vec3::ZERO),
            EffectAnchor::Point(Vec3::X),
        ));
        app.world_mut().write_message(PlayEffect::at(
            EffectRef::Particle("nope/nope.ptcl".into()),
            EffectAnchor::Point(Vec3::ZERO),
            EffectAnchor::Point(Vec3::X),
        ));
        for _ in 0..(DEFAULT_EMIT_FRAMES + 10) {
            app.update();
        }
        assert_eq!(count::<ParticleInstance>(&mut app), 0);
        assert_eq!(count::<EffectSequenceInstance>(&mut app), 0, "the lone-particle timeline ends");
        assert_eq!(app.world().resource::<EffectStats>().plays_dropped, 1);
    }

    #[test]
    fn real_ember_timeline_spawns_particles_and_despawns_when_done() {
        let Some(root) = real_root() else {
            eprintln!("[skip] game effect files not present");
            return;
        };
        let mut app = app(Quality::High, Some(root));
        let user = Vec3::new(0.0, 0.5, 0.0);
        let target = Vec3::new(1.5, 0.5, -2.5);
        app.world_mut().write_message(PlayEffect {
            effect: EffectRef::Move { id: 52, species: 4, form: 0 },
            user: EffectAnchor::Point(user),
            target: EffectAnchor::Point(target),
            delay: 0.2,
            skip_target: false,
        });
        app.update();
        assert_eq!(count::<EffectSequenceInstance>(&mut app), 1);
        let mut peak = 0;
        let mut peak_instances = 0;
        let mut saw_target = false;
        let mut frames = 0;
        for _ in 0..(30 * 15) {
            app.update();
            frames += 1;
            let s = app.world().resource::<EffectStats>().clone();
            peak = peak.max(s.live_particles);
            peak_instances = peak_instances.max(s.live_instances);
            let near_target = app
                .world_mut()
                .query::<(&ParticleInstance, &Transform)>()
                .iter(app.world())
                .any(|(_, t)| t.translation.distance(target) < 1e-4);
            saw_target |= near_target;
            if count::<EffectSequenceInstance>(&mut app) == 0 && count::<ParticleInstance>(&mut app) == 0 {
                break;
            }
        }
        let stats = app.world().resource::<EffectStats>().clone();
        assert!(peak > 0, "particles were alive");
        assert!(peak <= Quality::High.total_cap());
        assert_eq!(stats.particle_instances_started, 3, "muzzle, bullet, hit");
        assert!(saw_target, "the hit plays on the target");
        assert!(frames < 30 * 15, "everything despawned on its own (frames {frames})");
        assert_eq!(count::<ParticleInstance>(&mut app), 0);
        assert!(peak_instances >= 1);
    }

    #[test]
    fn total_particles_stay_under_the_cap() {
        let Some(root) = real_root() else {
            eprintln!("[skip] game effect files not present");
            return;
        };
        let mut app = app(Quality::Low, Some(root));
        for k in 0..40 {
            app.world_mut().write_message(PlayEffect::at(
                EffectRef::Move { id: 85, species: 25, form: 0 },
                EffectAnchor::Point(Vec3::new(k as f32, 0.0, 0.0)),
                EffectAnchor::Point(Vec3::new(k as f32, 0.0, -3.0)),
            ));
        }
        let mut peak = 0;
        for _ in 0..120 {
            app.update();
            peak = peak.max(app.world().resource::<EffectStats>().live_particles);
        }
        assert!(peak > 0);
        assert!(peak <= Quality::Low.total_cap(), "peak {peak}");
    }
}
