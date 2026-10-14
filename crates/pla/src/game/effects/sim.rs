//! Pure particle simulation of one particle file (no Bevy systems, unit-tested).
//!
//! What comes from the decoded file (`crate::assets::ptcl`, confidence there):
//! particle life, particles per emission, one-time flag, colour / alpha / size
//! over life, blend type, display type, texture and atlas cells. What is
//! ASSUMED here (not proven against the game): the emission timing model
//! ([`Emitter::emission_schedule`]), a random launch in all directions at the
//! decoded `omni_speed`, gravity, the base size, the flip-book timing (pattern
//! table stepped evenly over the particle life) and 30 frames per second.
//!
//! Determinism: particles are born at their scheduled frame (not at the frame
//! the step happened to land on) and their random direction and life come from
//! a hash of (effect seed, emitter, spawn index), so the same total time gives
//! the same particles whatever the step sizes were.

use std::sync::Arc;

use bevy::math::{Quat, Vec3};

use crate::assets::ptcl::{BlendType, DisplayType, Emitter, PtclFile};

/// Game frames per second of effect data (ASSUMED: the game runs at 30 fps).
pub const FPS: f32 = 30.0;
/// Emission window for a particle file played on its own (not from a timeline).
pub const DEFAULT_EMIT_FRAMES: u32 = 60;
/// Velocity-look stretch: extra length per unit of speed per frame (ASSUMED, as the CPU preview).
pub const STREAK_STRETCH: f32 = 8.0;
pub const STREAK_MAX_STRETCH: f32 = 4.0;

/// Effect detail setting (`--effects off|low|high`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quality {
    Off,
    #[default]
    Low,
    High,
}

impl Quality {
    pub fn parse(s: &str) -> Option<Quality> {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" | "0" | "none" | "false" => Some(Quality::Off),
            "low" | "1" => Some(Quality::Low),
            "high" | "2" | "on" | "true" => Some(Quality::High),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Quality::Off => "off",
            Quality::Low => "low",
            Quality::High => "high",
        }
    }

    /// Fraction of each emission that is spawned.
    pub fn emission_scale(self) -> f32 {
        match self {
            Quality::Off => 0.0,
            Quality::Low => 0.5,
            Quality::High => 1.0,
        }
    }

    /// Live particles allowed over all effects.
    pub fn total_cap(self) -> usize {
        match self {
            Quality::Off => 0,
            Quality::Low => 1000,
            Quality::High => 2000,
        }
    }

    /// Live particles allowed in one particle-file instance.
    pub fn per_effect_cap(self) -> usize {
        match self {
            Quality::Off => 0,
            Quality::Low => 300,
            Quality::High => 600,
        }
    }

    /// Particles spawned for an emission of `n` (at least 1 when `n > 0`, except `Off`).
    pub fn scaled_count(self, n: u32) -> u32 {
        if n == 0 || self == Quality::Off {
            return 0;
        }
        ((n as f32 * self.emission_scale()).round() as u32).max(1)
    }
}

/// Quality from the command line (`--effects X` or `--effects=X`), then the
/// `PLA_EFFECTS` variable, then the `effects=` line of `settings.txt`, else Low.
pub fn quality_from(args: &[String], env_value: Option<&str>, settings_text: Option<&str>) -> Quality {
    let from_args = crate::game::cli_flags::value_of(args, "--effects").and_then(|v| {
        let q = Quality::parse(v);
        if q.is_none() {
            crate::game::cli_flags::report_bad_value("--effects", v, "off|low|high", "the default");
        }
        q
    });
    let from_settings = || {
        settings_text?.lines().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == "effects").then(|| Quality::parse(v)).flatten()
        })
    };
    from_args
        .or_else(|| env_value.and_then(Quality::parse))
        .or_else(from_settings)
        .unwrap_or_default()
}

/// How a drawable emitter's particles are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawShape {
    /// Faces the camera.
    Billboard,
    /// Faces the camera but stays upright.
    YBillboard,
    /// Stretched along its velocity on screen (velocity-look).
    Streak,
    /// Flat quad in the effect's local XY plane.
    PolygonXY,
    /// Flat quad on the effect's local XZ (ground) plane.
    PolygonXZ,
}

/// Why an emitter is not drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Skip {
    /// Mesh particles (`PRIM`): not supported yet.
    Mesh,
    /// Ribbon / stripe (`EP03`): not supported yet.
    Ribbon,
    /// Child emitter (spawned from parent particles): not supported yet.
    Child,
    /// Display byte outside the known 0-6 values.
    UnknownDisplay(u8),
    /// Zero particles per emission or zero life.
    NoParticles,
    /// No texture in sampler 0 (e.g. the invisible `cmn_collision_dummy`): not drawn.
    NoTexture,
}

impl Skip {
    pub fn label(self) -> String {
        match self {
            Skip::Mesh => "mesh".into(),
            Skip::Ribbon => "ribbon".into(),
            Skip::Child => "child".into(),
            Skip::UnknownDisplay(v) => format!("display{v}"),
            Skip::NoParticles => "no_particles".into(),
            Skip::NoTexture => "no_texture".into(),
        }
    }
}

/// Drawing shape of an emitter, or why it is skipped.
pub fn classify(e: &Emitter) -> Result<DrawShape, Skip> {
    if e.parent.is_some() {
        return Err(Skip::Child);
    }
    if e.primitive_id.is_some() {
        return Err(Skip::Mesh);
    }
    if e.is_stripe() {
        return Err(Skip::Ribbon);
    }
    let shape = match e.display {
        DisplayType::Billboard | DisplayType::ComplexBillboard => DrawShape::Billboard,
        DisplayType::YBillboard => DrawShape::YBillboard,
        DisplayType::PolygonXY => DrawShape::PolygonXY,
        DisplayType::PolygonXZ => DrawShape::PolygonXZ,
        DisplayType::VelocityLook | DisplayType::VelocityLookPolygon => DrawShape::Streak,
        DisplayType::Other(v) => return Err(Skip::UnknownDisplay(v)),
    };
    if e.emit_rate < 0.5 || e.life_frames <= 0 {
        return Err(Skip::NoParticles);
    }
    if e.texture_ids[0].is_none() {
        return Err(Skip::NoTexture);
    }
    Ok(shape)
}

/// GPU blend of an emitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BlendMode {
    /// Ordinary transparency.
    #[default]
    Alpha,
    /// Adds light (glows, sparks).
    Add,
    /// Removes light (dark rings).
    Sub,
}

/// Blend of a decoded blend byte: 0/1/2 are LIKELY normal/add/subtract; other
/// values (3-5, meaning unknown) are drawn additively (ASSUMED, as the preview).
pub fn blend_mode(b: BlendType) -> BlendMode {
    match b {
        BlendType::Normal => BlendMode::Alpha,
        BlendType::Add | BlendType::Other(_) => BlendMode::Add,
        BlendType::Sub => BlendMode::Sub,
    }
}

/// True when an emitter's name marks it additive (`glow_add`, `base_add_xz`, `add_ring`).
pub fn named_additive(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.split(|c: char| c == '_' || c.is_ascii_digit()).any(|t| t == "add")
}

/// Blend of an emitter: [`blend_mode`] of its byte, except that a byte-0 (normal) emitter
/// whose name says `add` is drawn additively. ASSUMED: half of the `_add`-named emitters
/// read 0 (`effects.md`), so their additive look must come from somewhere else in the file
/// (shader combiner, not decoded); following the artist's name is the closer guess.
pub fn emitter_blend(e: &Emitter) -> BlendMode {
    match blend_mode(e.blend) {
        BlendMode::Alpha if named_additive(&e.name) => BlendMode::Add,
        b => b,
    }
}

/// A drawable emitter with its derived settings.
#[derive(Debug, Clone)]
pub struct EmitterDef {
    pub emitter: Emitter,
    pub shape: DrawShape,
    pub blend: BlendMode,
    /// Texture drawn (see [`drawn_texture`]); `None` = soft round dot.
    pub texture: Option<String>,
    /// Sampler whose texture is drawn (0, or 1 when sampler 0 is a plain white texture).
    pub slot: usize,
}

/// Texture an emitter draws and from which sampler. A plain white sampler-0 texture
/// (`cpt_cmn_white*`) carries no shape: with a second sampler the port draws that one
/// (white x texture 1 = texture 1 under a multiply combine, ASSUMED); without one it draws a
/// soft round dot (ASSUMED: those emitters are named `light`/`shine`, and their real shape
/// comes from a shader that is not decoded). Otherwise sampler 0.
pub fn drawn_texture(e: &Emitter, file: &PtclFile) -> (Option<String>, usize) {
    let name = |i: usize| e.texture_ids[i].and_then(|id| file.texture_name(id)).map(str::to_string);
    match name(0) {
        Some(t) if t.starts_with("cpt_cmn_white") => match name(1) {
            Some(t1) => (Some(t1), 1),
            None => (None, 0),
        },
        other => (other, 0),
    }
}

impl EmitterDef {
    /// Base size (x, y) in effect units; the decoded field is ASSUMED, 1 when unset.
    pub fn base_size(&self) -> [f32; 2] {
        let s = self.emitter.base_scale;
        let fix = |v: f32| if v.is_finite() && v > 0.0 { v } else { 1.0 };
        [fix(s[0]), fix(s[1])]
    }

    /// Atlas cells (u, v), at least 1 x 1 (sampler 1's divisions are not decoded: 1 x 1).
    pub fn atlas(&self) -> [u32; 2] {
        if self.slot != 0 {
            return [1, 1];
        }
        let a = self.emitter.atlas_div;
        let fix = |v: f32| if v.is_finite() && v >= 1.0 { (v as u32).min(64) } else { 1 };
        [fix(a[0]), fix(a[1])]
    }

    /// Flip-book cell at normalised life `t` (ASSUMED: pattern list stepped evenly).
    pub fn cell_at(&self, t: f32) -> u32 {
        let p = &self.emitter.texture_pattern;
        if p.is_empty() || self.slot != 0 {
            return 0;
        }
        let i = ((t.clamp(0.0, 1.0) * p.len() as f32) as usize).min(p.len() - 1);
        let [nu, nv] = self.atlas();
        p[i] % (nu * nv)
    }

    /// Wrap mode (u, v) of the drawn sampler: 0 mirror, 1 repeat, 2 clamp (LIKELY). Clamp
    /// when no texture is drawn (the shader then draws a soft dot over 0..1).
    pub fn wrap(&self) -> [u8; 2] {
        if self.texture.is_none() {
            return [2, 2];
        }
        self.emitter.texture_wrap[self.slot.min(2)].map(|w| w.min(2))
    }

    /// UV rectangle `(u0, v0, u1, v1)` of a cell. An axis with one atlas cell and mirror
    /// wrapping spans 0..2, so the shader's mirror wrap completes a quarter texture
    /// (ASSUMED: no repetition-count field was found; see `effects_game.md`).
    pub fn cell_uv(&self, cell: u32) -> [f32; 4] {
        let [nu, nv] = self.atlas();
        let (cu, cv) = (cell % nu, (cell / nu) % nv);
        let wrap = self.wrap();
        let span = |n: u32, w: u8| if n == 1 && w == 0 { 2.0 } else { 1.0 };
        let (su, sv) = (span(nu, wrap[0]), span(nv, wrap[1]));
        [
            cu as f32 / nu as f32 * su,
            cv as f32 / nv as f32 * sv,
            (cu + 1) as f32 / nu as f32 * su,
            (cv + 1) as f32 / nv as f32 * sv,
        ]
    }
}

/// A particle file prepared for playback.
#[derive(Debug, Clone, Default)]
pub struct EffectDef {
    pub path: String,
    pub emitters: Vec<EmitterDef>,
    /// Emitters not drawn, with the reason.
    pub skipped: Vec<(String, Skip)>,
}

impl EffectDef {
    pub fn from_ptcl(path: &str, file: &PtclFile) -> EffectDef {
        let mut def = EffectDef { path: path.to_string(), ..Default::default() };
        for e in &file.emitters {
            match classify(e) {
                Ok(shape) => {
                    let (texture, slot) = drawn_texture(e, file);
                    def.emitters.push(EmitterDef { emitter: e.clone(), shape, blend: emitter_blend(e), texture, slot })
                }
                Err(skip) => def.skipped.push((e.name.clone(), skip)),
            }
        }
        def
    }

    /// Texture names the drawable emitters need.
    pub fn texture_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.emitters.iter().filter_map(|e| e.texture.clone()).collect();
        v.sort();
        v.dedup();
        v
    }

    pub fn is_playable(&self) -> bool {
        !self.emitters.is_empty()
    }
}

/// One live particle (effect-local units, frames).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    pub born: f32,
    pub life: f32,
    pub vel: [f32; 3],
}

/// Deterministic hash -> [0, 1).
fn hash01(mut z: u64) -> f32 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

/// Simulation of one particle-file instance.
#[derive(Debug, Clone)]
pub struct EffectSim {
    pub def: Arc<EffectDef>,
    /// Local time in frames since the instance started.
    pub frame: f32,
    schedules: Vec<Vec<(u32, u32)>>,
    next: Vec<usize>,
    spawn_index: Vec<u64>,
    /// Live particles per drawable emitter.
    pub particles: Vec<Vec<Particle>>,
    pub seed: u64,
    /// Particles not spawned because a cap was reached.
    pub dropped: u64,
    pub spawned: u64,
}

impl EffectSim {
    /// `emit_frames`: emission window (the timeline's play length, or [`DEFAULT_EMIT_FRAMES`]).
    pub fn new(def: Arc<EffectDef>, emit_frames: u32, quality: Quality, seed: u64) -> EffectSim {
        let schedules: Vec<Vec<(u32, u32)>> = def
            .emitters
            .iter()
            .map(|e| {
                e.emitter
                    .emission_schedule(emit_frames.max(1))
                    .into_iter()
                    .map(|(f, n)| (f, quality.scaled_count(n)))
                    .filter(|(_, n)| *n > 0)
                    .collect()
            })
            .collect();
        let n = def.emitters.len();
        EffectSim {
            def,
            frame: 0.0,
            schedules,
            next: vec![0; n],
            spawn_index: vec![0; n],
            particles: vec![Vec::new(); n],
            seed,
            dropped: 0,
            spawned: 0,
        }
    }

    pub fn live(&self) -> usize {
        self.particles.iter().map(Vec::len).sum()
    }

    /// No emission left and no live particle.
    pub fn finished(&self) -> bool {
        self.next.iter().zip(&self.schedules).all(|(n, s)| *n >= s.len())
            && self.particles.iter().all(Vec::is_empty)
    }

    /// Stop further emission (live particles finish their life).
    pub fn stop_emission(&mut self) {
        for (n, s) in self.next.iter_mut().zip(&self.schedules) {
            *n = s.len();
        }
    }

    /// Advance by `dt_frames`. Spawns every scheduled emission whose frame is
    /// before the new time, limited by `per_effect_cap` live particles and the
    /// shared `budget` (decremented per spawn), then retires dead particles.
    /// Returns the number spawned.
    pub fn step(&mut self, dt_frames: f32, per_effect_cap: usize, budget: &mut usize) -> u32 {
        let end = self.frame + dt_frames.max(0.0);
        let mut spawned = 0;
        let mut live = self.live();
        for ei in 0..self.schedules.len() {
            let e = &self.def.emitters[ei].emitter;
            while let Some(&(frame, count)) = self.schedules[ei].get(self.next[ei]) {
                if frame as f32 >= end {
                    break;
                }
                self.next[ei] += 1;
                for _ in 0..count {
                    let k = self.spawn_index[ei];
                    self.spawn_index[ei] += 1;
                    let born = frame as f32;
                    // Life jitter: life_random read as a percentage (ASSUMED).
                    let base = self.seed ^ ((ei as u64) << 40) ^ (k.wrapping_mul(0x1000_0000_01B3));
                    let jr = (e.life_random as f32 / 100.0).clamp(0.0, 1.0);
                    let life = (e.life_frames.max(1) as f32 * (1.0 + (hash01(base) - 0.5) * 2.0 * jr)).max(1.0);
                    if end - born >= life {
                        continue; // already dead by the end of this step: no cost, no draw
                    }
                    if live >= per_effect_cap || *budget == 0 {
                        self.dropped += 1;
                        continue;
                    }
                    let z = hash01(base ^ 0xA5A5) * 2.0 - 1.0;
                    let a = hash01(base ^ 0x5A5A) * std::f32::consts::TAU;
                    let r = (1.0 - z * z).max(0.0).sqrt();
                    // omni_speed read as units per SECOND (ASSUMED; per frame, the
                    // decoded sizes and speeds would spread a hit over ~8 m).
                    let s = e.omni_speed / FPS;
                    self.particles[ei].push(Particle { born, life, vel: [r * a.cos() * s, r * a.sin() * s, z * s] });
                    live += 1;
                    *budget -= 1;
                    spawned += 1;
                }
            }
        }
        for ps in &mut self.particles {
            ps.retain(|p| end - p.born < p.life);
        }
        self.frame = end;
        self.spawned += spawned as u64;
        spawned
    }

    /// Drawn state of a particle at the current time.
    pub fn view(&self, emitter: usize, p: &Particle) -> ParticleView {
        let d = &self.def.emitters[emitter];
        let e = &d.emitter;
        let age = (self.frame - p.born).max(0.0);
        let t = (age / p.life).clamp(0.0, 1.0);
        let g = [e.gravity[0] * e.gravity[3], e.gravity[1] * e.gravity[3], e.gravity[2] * e.gravity[3]];
        let fin = |v: f32| if v.is_finite() { v } else { 0.0 };
        let pos = [0, 1, 2].map(|k| fin(p.vel[k] * age + 0.5 * g[k] * age * age));
        let vel = [0, 1, 2].map(|k| fin(p.vel[k] + g[k] * age));
        let rgb = e.color_at(t);
        let alpha = e.alpha_at(t).clamp(0.0, 1.0);
        let sc = e.scale_at(t);
        let base = d.base_size();
        ParticleView {
            pos: Vec3::from(pos),
            vel: Vec3::from(vel),
            t,
            rgba: [rgb[0].max(0.0), rgb[1].max(0.0), rgb[2].max(0.0), alpha],
            size: [base[0] * sc[0].abs(), base[1] * sc[1].abs()],
            uv: d.cell_uv(d.cell_at(t)),
        }
    }
}

/// A particle as drawn (effect-local position/velocity).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleView {
    pub pos: Vec3,
    pub vel: Vec3,
    pub t: f32,
    pub rgba: [f32; 4],
    pub size: [f32; 2],
    pub uv: [f32; 4],
}

/// Camera axes in world space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewAxes {
    pub right: Vec3,
    pub up: Vec3,
    /// From the camera into the scene.
    pub forward: Vec3,
}

impl Default for ViewAxes {
    fn default() -> Self {
        ViewAxes { right: Vec3::X, up: Vec3::Y, forward: Vec3::NEG_Z }
    }
}

/// Placement of an instance in the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub origin: Vec3,
    pub rotation: Quat,
    /// World metres per effect unit (ASSUMED, see the plugin).
    pub scale: f32,
}

/// Quad vertex buffers (positions in world space).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QuadBuffers {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl QuadBuffers {
    pub fn clear(&mut self) {
        self.positions.clear();
        self.uvs.clear();
        self.colors.clear();
        self.indices.clear();
    }

    pub fn quads(&self) -> usize {
        self.positions.len() / 4
    }

    fn push(&mut self, centre: Vec3, ax: Vec3, ay: Vec3, uv: [f32; 4], rgba: [f32; 4]) {
        let base = self.positions.len() as u32;
        for (sx, sy, u, v) in [(-1.0, -1.0, uv[0], uv[3]), (1.0, -1.0, uv[2], uv[3]), (1.0, 1.0, uv[2], uv[1]), (-1.0, 1.0, uv[0], uv[1])] {
            self.positions.push((centre + ax * sx + ay * sy).to_array());
            self.uvs.push([u, v]);
            self.colors.push(rgba);
        }
        self.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

/// Append the quads of one emitter of `sim` (camera-facing billboards, streaks
/// along velocity, or flat polygons) in world space. Invisible particles
/// (alpha or size 0) are left out.
pub fn build_emitter_quads(sim: &EffectSim, emitter: usize, place: &Placement, view: &ViewAxes, out: &mut QuadBuffers) {
    let shape = sim.def.emitters[emitter].shape;
    for p in &sim.particles[emitter] {
        let v = sim.view(emitter, p);
        if v.rgba[3] <= 0.0 || v.size[0] <= 0.0 || v.size[1] <= 0.0 {
            continue;
        }
        let centre = place.origin + place.rotation * (v.pos * place.scale);
        let (hw, hh) = (v.size[0] * place.scale * 0.5, v.size[1] * place.scale * 0.5);
        let (ax, ay) = match shape {
            DrawShape::Billboard => (view.right * hw, view.up * hh),
            DrawShape::YBillboard => {
                let r = Vec3::new(view.right.x, 0.0, view.right.z).try_normalize().unwrap_or(Vec3::X);
                (r * hw, Vec3::Y * hh)
            }
            DrawShape::PolygonXY => (place.rotation * Vec3::X * hw, place.rotation * Vec3::Y * hh),
            DrawShape::PolygonXZ => (place.rotation * Vec3::X * hw, place.rotation * Vec3::Z * hh),
            DrawShape::Streak => {
                let vw = place.rotation * v.vel;
                let on_screen = vw - view.forward * vw.dot(view.forward);
                match on_screen.try_normalize() {
                    Some(dir) => {
                        let stretch = 1.0 + (v.vel.length() * STREAK_STRETCH).min(STREAK_MAX_STRETCH);
                        let side = view.forward.cross(dir).try_normalize().unwrap_or(view.up);
                        (dir * hw * stretch, side * hh)
                    }
                    None => (view.right * hw, view.up * hh),
                }
            }
        };
        out.push(centre, ax, ay, v.uv, v.rgba);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::ptcl::{Key, KeyTable};

    pub(crate) fn emitter(name: &str) -> Emitter {
        Emitter {
            name: name.into(),
            parent: None,
            display: DisplayType::Billboard,
            blend: BlendType::Add,
            shader_index: 0,
            life_frames: 10,
            life_random: 0,
            one_time: true,
            emit_start_frame: 0,
            emit_duration_frames: 0,
            emit_interval_frames: 1,
            emit_rate: 4.0,
            color0: [1.0, 0.5, 0.25, 1.0],
            color1: [1.0; 4],
            base_scale: [2.0, 1.0, 1.0],
            gravity: [0.0, -1.0, 0.0, 0.0],
            omni_speed: 0.1,
            color0_keys: KeyTable::default(),
            alpha0_keys: KeyTable {
                keys: vec![Key { value: [1.0; 3], time: 0.0 }, Key { value: [0.0; 3], time: 1.0 }],
            },
            color1_keys: KeyTable::default(),
            alpha1_keys: KeyTable::default(),
            scale_keys: KeyTable::default(),
            table6_keys: KeyTable::default(),
            texture_ids: [Some(1), None, None],
            texture_wrap: [[2, 2]; 3],
            primitive_id: None,
            attributes: vec![],
            texture_pattern: vec![],
            atlas_div: [1.0, 1.0],
        }
    }

    pub(crate) fn def_of(emitters: Vec<Emitter>) -> Arc<EffectDef> {
        let mut d = EffectDef { path: "test.ptcl".into(), ..Default::default() };
        for e in emitters {
            match classify(&e) {
                Ok(shape) => d.emitters.push(EmitterDef {
                    blend: emitter_blend(&e),
                    emitter: e,
                    shape,
                    texture: Some("tex".into()),
                    slot: 0,
                }),
                Err(s) => d.skipped.push((e.name.clone(), s)),
            }
        }
        Arc::new(d)
    }

    fn looping(rate: f32, interval: i32, duration: i32, life: i32) -> Emitter {
        let mut e = emitter("loop");
        e.one_time = false;
        e.emit_rate = rate;
        e.emit_interval_frames = interval;
        e.emit_duration_frames = duration;
        e.life_frames = life;
        e
    }

    #[test]
    fn quality_parsing_and_priority() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(quality_from(&a(&["x"]), None, None), Quality::Low);
        assert_eq!(quality_from(&a(&["x", "--effects", "off"]), Some("high"), None), Quality::Off);
        assert_eq!(quality_from(&a(&["x", "--effects=HIGH"]), None, None), Quality::High);
        assert_eq!(quality_from(&a(&["x"]), Some("off"), Some("effects=high")), Quality::Off);
        assert_eq!(quality_from(&a(&["x"]), None, Some("# c\nmaster_volume=3\neffects = high\n")), Quality::High);
        assert_eq!(quality_from(&a(&["x", "--effects", "bogus"]), None, None), Quality::Low);
        assert_eq!(Quality::Low.scaled_count(1), 1);
        assert_eq!(Quality::Low.scaled_count(50), 25);
        assert_eq!(Quality::High.scaled_count(50), 50);
        assert_eq!(Quality::Off.scaled_count(50), 0);
    }

    #[test]
    fn blend_selection() {
        assert_eq!(blend_mode(BlendType::Normal), BlendMode::Alpha);
        assert_eq!(blend_mode(BlendType::Add), BlendMode::Add);
        assert_eq!(blend_mode(BlendType::Sub), BlendMode::Sub);
        assert_eq!(blend_mode(BlendType::Other(4)), BlendMode::Add);
    }

    #[test]
    fn add_named_normal_emitters_draw_additively() {
        assert!(named_additive("base_add_xz"));
        assert!(named_additive("00_xy_grow03_add"));
        assert!(named_additive("spark02_up_side_add"));
        assert!(!named_additive("hit_mark"));
        assert!(!named_additive("ladder"));
        let mut e = emitter("glow_add");
        e.blend = BlendType::Normal;
        assert_eq!(emitter_blend(&e), BlendMode::Add);
        e.blend = BlendType::Sub;
        assert_eq!(emitter_blend(&e), BlendMode::Sub, "an explicit byte wins");
        e.name = "fire".into();
        e.blend = BlendType::Normal;
        assert_eq!(emitter_blend(&e), BlendMode::Alpha);
    }

    #[test]
    fn unsupported_emitters_are_skipped_with_a_reason() {
        let mut mesh = emitter("mesh");
        mesh.primitive_id = Some(3);
        let mut ribbon = emitter("trail");
        ribbon.attributes = vec!["EP03".into()];
        let mut child = emitter("child");
        child.parent = Some(0);
        let mut odd = emitter("odd");
        odd.display = DisplayType::Other(7);
        let mut none = emitter("none");
        none.emit_rate = 0.0;
        let mut streak = emitter("spark");
        streak.display = DisplayType::VelocityLook;
        let mut dummy = emitter("Emitter1");
        dummy.texture_ids = [None; 3];
        let d = def_of(vec![emitter("glow"), mesh, ribbon, child, odd, none, streak, dummy]);
        assert_eq!(d.emitters.len(), 2);
        assert_eq!(d.emitters[1].shape, DrawShape::Streak);
        let reasons: Vec<Skip> = d.skipped.iter().map(|s| s.1).collect();
        assert_eq!(
            reasons,
            vec![Skip::Mesh, Skip::Ribbon, Skip::Child, Skip::UnknownDisplay(7), Skip::NoParticles, Skip::NoTexture]
        );
    }

    #[test]
    fn spawn_counts_follow_rate_and_are_independent_of_step_size() {
        // 2 particles every 3 frames for 30 frames = 10 emissions = 20 particles, life 100.
        let def = def_of(vec![looping(2.0, 3, 30, 100)]);
        let total = |dt: f32| {
            let mut s = EffectSim::new(def.clone(), 90, Quality::High, 42);
            let mut budget = usize::MAX;
            let mut n = 0;
            let mut t = 0.0;
            while t < 45.0 - 1e-4 {
                n += s.step(dt, usize::MAX, &mut budget);
                t += dt;
            }
            (n, s.particles[0].clone(), s.frame)
        };
        let (a, pa, fa) = total(1.0);
        let (b, pb, fb) = total(0.25);
        let (c, pc, _) = total(2.5);
        assert_eq!((a, b, c), (20, 20, 20));
        assert!((fa - fb).abs() < 1e-3);
        // Same particles (birth frames and directions) whatever the step.
        assert_eq!(pa, pb);
        assert_eq!(pa, pc);
        // Low quality halves each emission (2 -> 1).
        let mut s = EffectSim::new(def, 90, Quality::Low, 42);
        let mut budget = usize::MAX;
        assert_eq!(s.step(45.0, usize::MAX, &mut budget), 10);
    }

    #[test]
    fn one_time_burst_dies_after_its_life_and_finishes() {
        let def = def_of(vec![emitter("burst")]);
        let mut s = EffectSim::new(def, 60, Quality::High, 1);
        let mut budget = 100;
        assert_eq!(s.step(1.0, 100, &mut budget), 4);
        assert_eq!(budget, 96);
        assert!(!s.finished());
        s.step(8.0, 100, &mut budget);
        assert_eq!(s.live(), 4, "life 10: alive at frame 9");
        s.step(1.0, 100, &mut budget);
        assert_eq!(s.live(), 0, "dead at frame 10");
        assert!(s.finished());
    }

    #[test]
    fn caps_bound_live_particles() {
        let def = def_of(vec![looping(50.0, 1, 0, 1000)]);
        let mut s = EffectSim::new(def.clone(), 60, Quality::High, 3);
        let mut budget = usize::MAX;
        s.step(10.0, 120, &mut budget);
        assert_eq!(s.live(), 120, "per-effect cap");
        assert_eq!(s.dropped, 500 - 120);
        let mut s2 = EffectSim::new(def, 60, Quality::High, 3);
        let mut budget = 30;
        s2.step(10.0, 600, &mut budget);
        assert_eq!(s2.live(), 30, "shared budget");
        assert_eq!(budget, 0);
    }

    #[test]
    fn stop_emission_lets_particles_finish() {
        let def = def_of(vec![looping(1.0, 1, 0, 5)]);
        let mut s = EffectSim::new(def, 1000, Quality::High, 3);
        let mut budget = usize::MAX;
        s.step(10.0, 600, &mut budget);
        assert!(s.live() > 0);
        s.stop_emission();
        s.step(10.0, 600, &mut budget);
        assert!(s.finished());
    }

    #[test]
    fn curves_drive_colour_alpha_size_and_atlas_cell() {
        let mut e = emitter("fire");
        e.color0_keys = KeyTable {
            keys: vec![Key { value: [1.0, 1.0, 0.0], time: 0.0 }, Key { value: [1.0, 0.0, 0.0], time: 1.0 }],
        };
        e.scale_keys = KeyTable {
            keys: vec![Key { value: [0.0; 3], time: 0.0 }, Key { value: [2.0; 3], time: 1.0 }],
        };
        e.texture_pattern = vec![0, 1, 2, 3];
        e.atlas_div = [4.0, 1.0];
        e.omni_speed = 0.0;
        let def = def_of(vec![e]);
        let mut s = EffectSim::new(def, 60, Quality::High, 9);
        let mut budget = usize::MAX;
        s.step(5.0, 100, &mut budget); // born at 0, age 5 of 10 -> t = 0.5
        let p = s.particles[0][0];
        let v = s.view(0, &p);
        assert!((v.t - 0.5).abs() < 1e-6);
        assert!((v.rgba[1] - 0.5).abs() < 1e-6, "yellow -> red halfway");
        assert!((v.rgba[3] - 0.5).abs() < 1e-6, "alpha 1 -> 0 halfway");
        assert!((v.size[0] - 2.0).abs() < 1e-6 && (v.size[1] - 1.0).abs() < 1e-6, "base (2,1) x scale 1");
        assert_eq!(v.uv, [0.5, 0.0, 0.75, 1.0], "third of four flip-book cells");
        // A single-cell mirrored texture spans 0..2 so the mirror completes it.
        let mut circle = emitter("circle");
        circle.texture_wrap = [[0, 0]; 3];
        let d = def_of(vec![circle]);
        assert_eq!(d.emitters[0].cell_uv(0), [0.0, 0.0, 2.0, 2.0]);
    }

    #[test]
    fn quads_face_the_camera_and_streaks_follow_velocity() {
        let mut spark = emitter("spark");
        spark.display = DisplayType::VelocityLook;
        spark.alpha0_keys = KeyTable::default(); // constant alpha 1
        let def = def_of(vec![emitter("glow"), spark]);
        let mut s = EffectSim::new(def, 60, Quality::High, 5);
        let mut budget = usize::MAX;
        s.step(1.0, 100, &mut budget);
        let place = Placement { origin: Vec3::new(10.0, 0.0, 0.0), rotation: Quat::IDENTITY, scale: 0.5 };
        let view = ViewAxes::default();
        let mut q = QuadBuffers::default();
        build_emitter_quads(&s, 0, &place, &view, &mut q);
        assert_eq!(q.quads(), 4);
        assert_eq!(q.indices.len(), 24);
        // Billboard in the camera plane (z constant per quad), width 2 * 0.5 = 1 m.
        let quad = &q.positions[0..4];
        assert!((quad[0][2] - quad[2][2]).abs() < 1e-5);
        assert!(((quad[1][0] - quad[0][0]) - 1.0).abs() < 1e-5);
        let mut st = QuadBuffers::default();
        build_emitter_quads(&s, 1, &place, &view, &mut st);
        assert_eq!(st.quads(), 4);
        // The long axis of a streak is parallel to its on-screen velocity.
        let p = s.particles[1][0];
        let vel = s.view(1, &p).vel;
        let on_screen = Vec3::new(vel.x, vel.y, 0.0).normalize();
        let a = Vec3::from(st.positions[0]);
        let b = Vec3::from(st.positions[1]);
        let axis = (b - a).normalize();
        assert!(axis.dot(on_screen).abs() > 0.999, "{axis:?} vs {on_screen:?}");
    }
}
