//! Graphics quality machinery behind the Graphics screen (`graphics_screen.rs`): texture mipmaps,
//! anisotropic filtering and texture resolution, render scale (super/under-sampling), the frame
//! limiter and v-sync, and the streaming-distance multipliers.
//!
//! Every default reproduces the game as it was before these options existed: no mipmaps, no
//! anisotropic filtering, full texture resolution, render scale 100 %, v-sync as Bevy's default,
//! no frame cap and every streaming distance at 100 %. With those defaults nothing here touches an
//! image, a camera or a window.
//!
//! Textures: only images used by 3D materials (`StandardMaterial` and the materials built from it)
//! are processed, never UI images, fonts or render targets. Mipmaps are generated on the CPU in a
//! background task (box filter; colour textures are averaged in linear light; cut-out alpha keeps
//! its coverage so grass cards do not thin out in the distance), then the material is re-bound.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use bevy::asset::AssetId;
use bevy::camera::RenderTarget;
use bevy::image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::tasks::{futures_lite::future, AsyncComputeTaskPool, Task};
use bevy::window::{PresentMode, PrimaryWindow};

use crate::assets::bntx::TextureData;

// ------------------------------------------------------------------ CPU mip generation

/// Images whose longest side is at least this many pixels are left out of the "Texture quality"
/// reduction (the baked terrain has its own "Terrain texture" option).
pub const REDUCE_MAX_SIDE: u32 = 2048;

fn srgb_to_linear_lut() -> &'static [f32; 256] {
    static LUT: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    LUT.get_or_init(|| {
        std::array::from_fn(|i| {
            let c = i as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        })
    })
}

const LIN_STEPS: usize = 4096;

fn linear_to_srgb_lut() -> &'static [u8; LIN_STEPS + 1] {
    static LUT: std::sync::OnceLock<[u8; LIN_STEPS + 1]> = std::sync::OnceLock::new();
    LUT.get_or_init(|| {
        std::array::from_fn(|i| {
            let l = i as f32 / LIN_STEPS as f32;
            let c = if l <= 0.003_130_8 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
            (c * 255.0).round().clamp(0.0, 255.0) as u8
        })
    })
}

/// Size of the next mip level.
pub fn next_size(w: u32, h: u32) -> (u32, u32) {
    ((w / 2).max(1), (h / 2).max(1))
}

/// Number of levels of a full mip chain for `w` x `h`.
pub fn mip_count(w: u32, h: u32) -> u32 {
    32 - w.max(h).max(1).leading_zeros()
}

/// One 2x2 box-filter step of an RGBA8 image. Colour channels of sRGB images are averaged in
/// linear light; alpha is averaged as is. Odd sizes clamp the last row/column.
pub fn downsample_rgba8(src: &[u8], w: u32, h: u32, srgb: bool) -> (Vec<u8>, u32, u32) {
    let (nw, nh) = next_size(w, h);
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    let to_lin = srgb_to_linear_lut();
    let to_srgb = linear_to_srgb_lut();
    for y in 0..nh {
        let y0 = (2 * y).min(h - 1);
        let y1 = (2 * y + 1).min(h - 1);
        for x in 0..nw {
            let x0 = (2 * x).min(w - 1);
            let x1 = (2 * x + 1).min(w - 1);
            let px = [
                ((y0 * w + x0) * 4) as usize,
                ((y0 * w + x1) * 4) as usize,
                ((y1 * w + x0) * 4) as usize,
                ((y1 * w + x1) * 4) as usize,
            ];
            let o = ((y * nw + x) * 4) as usize;
            for c in 0..3 {
                if srgb {
                    let s: f32 = px.iter().map(|p| to_lin[src[p + c] as usize]).sum::<f32>() * 0.25;
                    out[o + c] = to_srgb[(s.clamp(0.0, 1.0) * LIN_STEPS as f32).round() as usize];
                } else {
                    let s: u32 = px.iter().map(|p| u32::from(src[p + c])).sum();
                    out[o + c] = ((s + 2) / 4) as u8;
                }
            }
            let a: u32 = px.iter().map(|p| u32::from(src[p + 3])).sum();
            out[o + 3] = ((a + 2) / 4) as u8;
        }
    }
    (out, nw, nh)
}

/// Fraction of texels with alpha >= 128 (the alpha-mask cut-off) after scaling alpha by `s`.
fn coverage(data: &[u8], s: f32) -> f32 {
    let n = (data.len() / 4).max(1);
    let hit = data.chunks_exact(4).filter(|p| f32::from(p[3]) * s >= 127.5).count();
    hit as f32 / n as f32
}

/// Scale the alpha of a mip level so its cut-off coverage matches `target` (keeps alpha-tested
/// cards from thinning out at a distance).
fn preserve_coverage(data: &mut [u8], target: f32) {
    if coverage(data, 1.0) >= target {
        return;
    }
    let (mut lo, mut hi) = (1.0f32, 8.0f32);
    for _ in 0..14 {
        let mid = 0.5 * (lo + hi);
        if coverage(data, mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    for p in data.chunks_exact_mut(4) {
        p[3] = (f32::from(p[3]) * hi).round().min(255.0) as u8;
    }
}

/// The result of processing one image on the CPU.
#[derive(Debug, Clone, PartialEq)]
pub struct MipChain {
    /// Level 0 first, then every smaller level (wgpu layer-major order for one layer).
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub levels: u32,
}

/// Halve `level0` `reduce` times, then (with `mips`) append the whole mip chain.
pub fn build_mip_chain(level0: &[u8], w: u32, h: u32, srgb: bool, reduce: u8, mips: bool) -> MipChain {
    let mut cur = level0.to_vec();
    let (mut cw, mut ch) = (w, h);
    for _ in 0..reduce {
        if cw <= 4 && ch <= 4 {
            break;
        }
        let (d, nw, nh) = downsample_rgba8(&cur, cw, ch, srgb);
        cur = d;
        cw = nw;
        ch = nh;
    }
    let (bw, bh) = (cw, ch);
    let mut data = cur.clone();
    let mut levels = 1;
    if mips {
        let cutout = cur.chunks_exact(4).any(|p| p[3] < 128);
        let target = if cutout { coverage(&cur, 1.0) } else { 0.0 };
        while cw > 1 || ch > 1 {
            let (mut d, nw, nh) = downsample_rgba8(&cur, cw, ch, srgb);
            if cutout {
                preserve_coverage(&mut d, target);
            }
            data.extend_from_slice(&d);
            cur = d;
            cw = nw;
            ch = nh;
            levels += 1;
        }
    }
    MipChain { data, width: bw, height: bh, levels }
}

/// Downscale a CPU texture (the baked terrain albedo / normals) until its longest side is at most
/// `max_side` (0 = no limit).
pub fn limit_texture(tex: TextureData, max_side: u32, srgb: bool) -> TextureData {
    if max_side == 0 || tex.width.max(tex.height) <= max_side || tex.rgba.len() != (tex.width * tex.height * 4) as usize {
        return tex;
    }
    let (mut d, mut w, mut h) = (tex.rgba, tex.width, tex.height);
    while w.max(h) > max_side {
        let (nd, nw, nh) = downsample_rgba8(&d, w, h, srgb);
        d = nd;
        w = nw;
        h = nh;
    }
    TextureData { width: w, height: h, rgba: d }
}

static TERRAIN_TEXTURE_MAX: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// "Terrain texture" option (restart): the baked terrain albedo and normals are reduced to this
/// side at load (0 = as baked).
pub fn set_terrain_texture_max(side: u32) {
    TERRAIN_TEXTURE_MAX.store(side, std::sync::atomic::Ordering::Relaxed);
}

pub fn terrain_texture_max() -> u32 {
    TERRAIN_TEXTURE_MAX.load(std::sync::atomic::Ordering::Relaxed)
}

// ------------------------------------------------------------------ texture settings

/// Texture options of the Graphics screen.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureQuality {
    pub mipmaps: bool,
    /// 1 = off, else 2/4/8/16.
    pub anisotropy: u16,
    /// Halvings of every material texture below [`REDUCE_MAX_SIDE`] (0 full, 1 half, 2 quarter).
    /// Read once at start-up (restart option).
    pub reduce: u8,
}

impl Default for TextureQuality {
    fn default() -> Self {
        Self { mipmaps: false, anisotropy: 1, reduce: 0 }
    }
}

impl TextureQuality {
    fn touches_images(&self) -> bool {
        self.mipmaps || self.anisotropy > 1 || self.reduce > 0
    }
}

/// Sampler for an image with/without mips and with the given anisotropy (address modes kept).
pub fn sampler_for(current: &ImageSampler, mips: bool, anisotropy: u16) -> ImageSampler {
    let mut d = match current {
        ImageSampler::Default => ImageSamplerDescriptor::linear(),
        ImageSampler::Descriptor(d) => d.clone(),
    };
    if mips {
        d.min_filter = ImageFilterMode::Linear;
        d.mipmap_filter = ImageFilterMode::Linear;
    }
    if anisotropy > 1 {
        d.set_anisotropic_filter(anisotropy);
    } else {
        d.anisotropy_clamp = 1;
    }
    ImageSampler::Descriptor(d)
}

fn eligible(img: &Image) -> Option<bool> {
    let desc = &img.texture_descriptor;
    if desc.dimension != TextureDimension::D2
        || desc.size.depth_or_array_layers != 1
        || desc.usage.contains(TextureUsages::RENDER_ATTACHMENT)
        || desc.size.width < 4
        || desc.size.height < 4
    {
        return None;
    }
    let srgb = match desc.format {
        TextureFormat::Rgba8UnormSrgb => true,
        TextureFormat::Rgba8Unorm => false,
        _ => return None,
    };
    let need = (desc.size.width * desc.size.height * 4) as usize;
    (img.data.as_ref()?.len() >= need).then_some(srgb)
}

/// What the registry did to one image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TexState {
    width: u32,
    height: u32,
    levels: u32,
    srgb: bool,
}

/// Material textures seen so far and what was done to them.
#[derive(Resource, Default)]
pub struct TextureRegistry {
    known: HashSet<AssetId<Image>>,
    pending: HashSet<AssetId<Image>>,
    state: HashMap<AssetId<Image>, TexState>,
    tasks: Vec<(AssetId<Image>, Task<MipChain>)>,
    applied: TextureQuality,
    /// Images processed so far (tests, logs).
    pub processed: usize,
}

impl TextureRegistry {
    pub fn known_len(&self) -> usize {
        self.known.len()
    }
    pub fn busy(&self) -> bool {
        !self.tasks.is_empty() || !self.pending.is_empty()
    }
}

fn material_images(m: &StandardMaterial) -> impl Iterator<Item = AssetId<Image>> + '_ {
    [
        &m.base_color_texture,
        &m.normal_map_texture,
        &m.emissive_texture,
        &m.metallic_roughness_texture,
        &m.occlusion_texture,
    ]
    .into_iter()
    .flatten()
    .map(|h| h.id())
}

/// Collect the images of new/changed materials and queue those that need work.
fn collect_material_textures(
    mut mat_events: MessageReader<AssetEvent<StandardMaterial>>,
    mut img_events: MessageReader<AssetEvent<Image>>,
    materials: Res<Assets<StandardMaterial>>,
    images: Res<Assets<Image>>,
    mut reg: ResMut<TextureRegistry>,
) {
    for ev in mat_events.read() {
        let (AssetEvent::Added { id } | AssetEvent::Modified { id }) = ev else { continue };
        let Some(m) = materials.get(*id) else { continue };
        let ids: Vec<_> = material_images(m).collect();
        for img in ids {
            if reg.known.insert(img) {
                reg.pending.insert(img);
            }
        }
    }
    for ev in img_events.read() {
        let (AssetEvent::Added { id } | AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id }) = ev
        else {
            continue;
        };
        if !reg.known.contains(id) {
            continue;
        }
        // Our own write-back shows up as `Modified`: ignore it when the image is what we left.
        if let (Some(st), Some(img)) = (reg.state.get(id), images.get(*id)) {
            let d = &img.texture_descriptor;
            if d.size.width == st.width && d.size.height == st.height && d.mip_level_count == st.levels {
                continue;
            }
        }
        reg.state.remove(id);
        reg.pending.insert(*id);
    }
}

/// Start background jobs for queued images (mips / reduction) or set their sampler directly.
fn process_texture_queue(
    cfg: Res<TextureQuality>,
    mut images: ResMut<Assets<Image>>,
    mut reg: ResMut<TextureRegistry>,
    mut touched: ResMut<TouchedTextures>,
    startup: Res<StartupTextureReduce>,
) {
    if reg.pending.is_empty() {
        return;
    }
    if !cfg.touches_images() && reg.state.is_empty() {
        // Defaults and nothing processed yet: leave every image exactly as loaded.
        reg.pending.clear();
        return;
    }
    let ids: Vec<_> = reg.pending.iter().copied().take(256).collect();
    for id in ids {
        let Some(img) = images.get(id) else {
            // Not loaded yet: the image event queues it again.
            reg.pending.remove(&id);
            continue;
        };
        reg.pending.remove(&id);
        let Some(srgb) = eligible(img) else { continue };
        let desc = &img.texture_descriptor;
        let (w, h, levels) = (desc.size.width, desc.size.height, desc.mip_level_count);
        let already_reduced = reg.state.contains_key(&id);
        let reduce = if already_reduced || w.max(h) >= REDUCE_MAX_SIDE { 0 } else { startup.0 };
        let want_levels = if cfg.mipmaps { mip_count(w, h) } else { 1 };
        if reduce == 0 && levels == want_levels {
            // Sampler-only change.
            let s = sampler_for(&img.sampler, cfg.mipmaps, cfg.anisotropy);
            if let Some(mut m) = images.get_mut(id) {
                if m.sampler != s {
                    m.sampler = s;
                    touched.0.insert(id);
                }
            }
            reg.state.insert(id, TexState { width: w, height: h, levels, srgb });
            continue;
        }
        if reduce == 0 && !cfg.mipmaps && levels > 1 {
            // Mipmaps switched off: keep level 0 only.
            if let Some(mut m) = images.get_mut(id) {
                if let Some(d) = m.data.as_mut() {
                    d.truncate((w * h * 4) as usize);
                }
                m.texture_descriptor.mip_level_count = 1;
                m.sampler = sampler_for(&m.sampler, false, cfg.anisotropy);
            }
            reg.state.insert(id, TexState { width: w, height: h, levels: 1, srgb });
            touched.0.insert(id);
            continue;
        }
        let level0 = img.data.as_ref().map(|d| d[..(w * h * 4) as usize].to_vec()).unwrap_or_default();
        let mips = cfg.mipmaps;
        let task = AsyncComputeTaskPool::get().spawn(async move { build_mip_chain(&level0, w, h, srgb, reduce, mips) });
        // Mark as reduced at once so a re-queue never halves it twice.
        reg.state.insert(id, TexState { width: w, height: h, levels, srgb });
        reg.tasks.push((id, task));
    }
}

/// Write finished background jobs back into their images.
fn poll_texture_tasks(
    cfg: Res<TextureQuality>,
    mut images: ResMut<Assets<Image>>,
    mut reg: ResMut<TextureRegistry>,
    mut touched: ResMut<TouchedTextures>,
) {
    let mut i = 0;
    while i < reg.tasks.len() {
        let done = future::block_on(future::poll_once(&mut reg.tasks[i].1));
        let Some(chain) = done else {
            i += 1;
            continue;
        };
        let (id, _) = reg.tasks.swap_remove(i);
        let srgb = reg.state.get(&id).is_some_and(|s| s.srgb);
        if let Some(mut img) = images.get_mut(id) {
            img.texture_descriptor.size = Extent3d { width: chain.width, height: chain.height, depth_or_array_layers: 1 };
            img.texture_descriptor.mip_level_count = chain.levels;
            img.data = Some(chain.data);
            img.sampler = sampler_for(&img.sampler, chain.levels > 1, cfg.anisotropy);
            reg.state.insert(id, TexState { width: chain.width, height: chain.height, levels: chain.levels, srgb });
            reg.processed += 1;
            touched.0.insert(id);
        }
    }
}

/// Live change of the texture options: queue every known image again.
fn texture_settings_changed(cfg: Res<TextureQuality>, mut reg: ResMut<TextureRegistry>) {
    if reg.applied == *cfg {
        return;
    }
    reg.applied = *cfg;
    let all: Vec<_> = reg.known.iter().copied().collect();
    reg.pending.extend(all);
}

/// Images whose data or sampler changed this frame (their materials must be re-bound).
#[derive(Resource, Default)]
pub struct TouchedTextures(pub HashSet<AssetId<Image>>);

/// "Texture quality" captured at start-up (restart option).
#[derive(Resource, Default, Clone, Copy)]
pub struct StartupTextureReduce(pub u8);

/// Re-bind every material that uses a touched image (Bevy keeps the old texture view in a material
/// bind group until the material itself changes).
fn rebind_materials(world: &mut World) {
    let ids = {
        let Some(mut t) = world.get_resource_mut::<TouchedTextures>() else { return };
        if t.0.is_empty() {
            return;
        }
        std::mem::take(&mut t.0)
    };
    touch::<StandardMaterial>(world, &ids, |m| m);
    touch::<super::art_style::ToonMaterial>(world, &ids, |m| &m.base);
    touch::<super::art_style::OutlineMaterial>(world, &ids, |m| &m.base);
    touch::<super::water::WaterMaterial>(world, &ids, |m| &m.base);
}

fn touch<M: Asset>(world: &mut World, ids: &HashSet<AssetId<Image>>, base: fn(&M) -> &StandardMaterial) {
    let Some(mut assets) = world.get_resource_mut::<Assets<M>>() else { return };
    let hit: Vec<AssetId<M>> =
        assets.iter().filter(|(_, m)| material_images(base(m)).any(|i| ids.contains(&i))).map(|(id, _)| id).collect();
    for id in hit {
        let _ = assets.get_mut(id);
    }
}

pub struct TextureQualityPlugin {
    pub settings: TextureQuality,
}

impl Plugin for TextureQualityPlugin {
    fn build(&self, app: &mut App) {
        if self.settings.touches_images() {
            println!(
                "Textures: mipmaps {}, anisotropy {}x, texture reduction {} (material textures below {REDUCE_MAX_SIDE} px).",
                if self.settings.mipmaps { "on" } else { "off" },
                self.settings.anisotropy,
                ["full", "half", "quarter"][self.settings.reduce.min(2) as usize]
            );
        }
        app.insert_resource(self.settings)
            .insert_resource(StartupTextureReduce(self.settings.reduce))
            .insert_resource(TextureRegistry { applied: self.settings, ..default() })
            .init_resource::<TouchedTextures>()
            .add_systems(
                PostUpdate,
                (
                    texture_settings_changed,
                    collect_material_textures,
                    process_texture_queue,
                    poll_texture_tasks,
                    rebind_materials,
                )
                    .chain(),
            );
    }
}

// ------------------------------------------------------------------ streaming distances

/// Streaming-distance multipliers (Graphics screen, page "Distances"). 1.0 = the distances the
/// game used before the options (objects 200/230 m, pickups 120/150 m, NPCs 200/230 m, wild
/// creatures 120 m, collision 200/230 m).
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct StreamDistances {
    pub objects: f32,
    pub collision: f32,
    pub pickups: f32,
    pub npcs: f32,
    pub wild: f32,
}

impl Default for StreamDistances {
    fn default() -> Self {
        Self { objects: 1.0, collision: 1.0, pickups: 1.0, npcs: 1.0, wild: 1.0 }
    }
}

/// Multiplier of an optional [`StreamDistances`] (1.0 without the resource).
pub fn scale_of(d: Option<&StreamDistances>, pick: fn(&StreamDistances) -> f32) -> f32 {
    d.map_or(1.0, |d| pick(d).clamp(0.25, 4.0))
}

// ------------------------------------------------------------------ frame pacing

/// V-sync and frame cap (Graphics screen, page "Display").
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramePacing {
    pub vsync: bool,
    /// Frames per second, 0 = no cap.
    pub fps_cap: u32,
}

impl Default for FramePacing {
    fn default() -> Self {
        Self { vsync: true, fps_cap: 0 }
    }
}

impl FramePacing {
    pub fn period(&self) -> Option<Duration> {
        (self.fps_cap > 0).then(|| Duration::from_secs_f64(1.0 / f64::from(self.fps_cap)))
    }
}

/// Next frame's deadline from the previous one: `prev + period`, or `now + period` when the frame
/// ran late by more than one period (no catch-up burst).
pub fn next_deadline(prev: Option<Instant>, now: Instant, period: Duration) -> Instant {
    match prev {
        Some(p) if p + period >= now => p + period,
        _ => now + period,
    }
}

/// Sleeps at the end of the frame until the cap's deadline (sleep, then a short spin for the last
/// millisecond). Runs in `Last`, so the spacing covers the whole frame: logic and rendering. In
/// the automation mode it delays the real time of every stepped frame the same way, which is how
/// the cap is measured offscreen (`bench` command).
fn frame_limiter(pacing: Res<FramePacing>, mut deadline: Local<Option<Instant>>) {
    let Some(period) = pacing.period() else {
        *deadline = None;
        return;
    };
    let now = Instant::now();
    let Some(target) = *deadline else {
        *deadline = Some(now + period);
        return;
    };
    if target > now {
        let left = target - now;
        if left > Duration::from_micros(1500) {
            std::thread::sleep(left - Duration::from_micros(1000));
        }
        while Instant::now() < target {
            std::hint::spin_loop();
        }
    }
    *deadline = Some(next_deadline(Some(target), Instant::now(), period));
}

/// V-sync on = the window's default present mode (what the game used before), off = no v-sync.
fn apply_vsync(pacing: Res<FramePacing>, mut windows: Query<&mut Window, With<PrimaryWindow>>, mut last: Local<Option<bool>>) {
    if *last == Some(pacing.vsync) {
        return;
    }
    let first = last.is_none();
    *last = Some(pacing.vsync);
    if first && pacing.vsync {
        return; // start-up with v-sync on: leave the window exactly as created
    }
    let mode = if pacing.vsync { PresentMode::default() } else { PresentMode::AutoNoVsync };
    for mut w in &mut windows {
        if w.present_mode != mode {
            w.present_mode = mode;
        }
    }
}

pub struct FramePacingPlugin {
    pub pacing: FramePacing,
}

impl Plugin for FramePacingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.pacing).add_systems(Update, apply_vsync).add_systems(Last, frame_limiter);
    }
}

// ------------------------------------------------------------------ render scale

/// Render scale of the 3D view in percent of the output size (window or automation target).
/// 100 = render straight to the output (as before the option). Other values render the 3D camera
/// into an offscreen image of the scaled size, which a second camera draws stretched over the
/// whole output (supersampling above 100, cheaper rendering below); the UI is drawn by that
/// second camera at the output size.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderScale {
    pub percent: u16,
}

impl Default for RenderScale {
    fn default() -> Self {
        Self { percent: 100 }
    }
}

/// Size of the scaled 3D image for an output of `w` x `h`.
pub fn scaled_size(w: u32, h: u32, percent: u16) -> (u32, u32) {
    let s = f64::from(percent.clamp(25, 400)) / 100.0;
    let f = |v: u32| ((f64::from(v) * s).round() as u32).clamp(16, 8192);
    (f(w), f(h))
}

/// Whether two render targets are the same output.
pub fn same_target(a: &RenderTarget, b: &RenderTarget) -> bool {
    match (a, b) {
        (RenderTarget::Image(x), RenderTarget::Image(y)) => x.handle == y.handle,
        (RenderTarget::Window(x), RenderTarget::Window(y)) => match (x, y) {
            (bevy::window::WindowRef::Primary, bevy::window::WindowRef::Primary) => true,
            (bevy::window::WindowRef::Entity(a), bevy::window::WindowRef::Entity(b)) => a == b,
            _ => false,
        },
        _ => format!("{a:?}") == format!("{b:?}"),
    }
}

#[derive(Resource, Default)]
struct RenderScaleState {
    image: Option<Handle<Image>>,
    compositor: Option<Entity>,
    view: Option<Entity>,
    camera: Option<Entity>,
    output: Option<RenderTarget>,
    camera_was_ui: bool,
}

/// Marks the camera that draws the scaled 3D image (and the UI) to the output.
#[derive(Component)]
pub struct RenderScaleCompositor;

/// The full-screen image node showing the scaled 3D view.
#[derive(Component)]
pub struct RenderScaleView;

fn output_size(target: &RenderTarget, windows: &Query<&Window>, primary: &Query<&Window, With<PrimaryWindow>>, images: &Assets<Image>) -> Option<(u32, u32)> {
    match target {
        RenderTarget::Window(bevy::window::WindowRef::Primary) => {
            primary.iter().next().map(|w| (w.physical_width(), w.physical_height()))
        }
        RenderTarget::Window(bevy::window::WindowRef::Entity(e)) => {
            windows.get(*e).ok().map(|w| (w.physical_width(), w.physical_height()))
        }
        RenderTarget::Image(t) => images.get(&t.handle).map(|i| (i.width(), i.height())),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_render_scale(
    mut commands: Commands,
    scale: Res<RenderScale>,
    mut st: ResMut<RenderScaleState>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window>,
    primary: Query<&Window, With<PrimaryWindow>>,
    mut cams: Query<
        (Entity, &mut RenderTarget, &mut Projection, Has<IsDefaultUiCamera>),
        (With<Camera3d>, Without<RenderScaleCompositor>),
    >,
    mut comp: Query<&mut RenderTarget, (With<RenderScaleCompositor>, Without<Camera3d>)>,
) {
    let want = scale.percent != 100;
    if !want {
        if let (Some(cam), Some(output)) = (st.camera, st.output.clone()) {
            if let Ok((_, mut t, mut proj, _)) = cams.get_mut(cam) {
                *t = output;
                // Bevy recomputes a camera's target size only when the camera, its projection or
                // the target image/window changed: touch the projection so the size follows.
                proj.set_changed();
                if st.camera_was_ui {
                    commands.entity(cam).insert(IsDefaultUiCamera);
                }
            }
            for e in [st.compositor, st.view].into_iter().flatten() {
                commands.entity(e).despawn();
            }
            if let Some(h) = st.image.take() {
                images.remove(&h);
            }
            *st = RenderScaleState::default();
        }
        return;
    }
    // The main 3D camera (the viewer has one).
    let Some((cam, mut target, mut proj, is_ui)) = cams.iter_mut().next() else { return };
    if st.camera != Some(cam) {
        // First activation (or the camera was replaced): remember where it drew.
        let output = target.clone();
        let Some((w, h)) = output_size(&output, &windows, &primary, &images) else { return };
        let (sw, sh) = scaled_size(w, h, scale.percent);
        let image = images.add(Image::new_target_texture(sw, sh, TextureFormat::Rgba8UnormSrgb, None));
        *target = RenderTarget::Image(image.clone().into());
        proj.set_changed();
        commands.entity(cam).remove::<IsDefaultUiCamera>();
        let compositor = commands
            .spawn((
                RenderScaleCompositor,
                Camera2d,
                Camera { order: 10, clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
                output.clone(),
                IsDefaultUiCamera,
                Msaa::Off,
            ))
            .id();
        let view = commands
            .spawn((
                RenderScaleView,
                ImageNode::new(image.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                GlobalZIndex(i32::MIN),
                UiTargetCamera(compositor),
            ))
            .id();
        *st = RenderScaleState {
            image: Some(image),
            compositor: Some(compositor),
            view: Some(view),
            camera: Some(cam),
            output: Some(output),
            camera_was_ui: is_ui,
        };
        return;
    }
    // Someone else retargeted the camera (e.g. the automation mode's offscreen target): that is
    // the new output; the camera goes back to the scaled image.
    if let Some(img) = st.image.clone() {
        let ours = matches!(&*target, RenderTarget::Image(i) if i.handle == img);
        if !ours {
            st.output = Some(target.clone());
            *target = RenderTarget::Image(img.into());
            proj.set_changed();
            commands.entity(cam).remove::<IsDefaultUiCamera>();
        }
    }
    // Keep the compositor on the output the camera had, and the image at the scaled output size.
    if let (Some(c), Some(output)) = (st.compositor, st.output.clone()) {
        if let Ok(mut t) = comp.get_mut(c) {
            if !same_target(&t, &output) {
                *t = output.clone();
            }
        }
        let Some((w, h)) = output_size(&output, &windows, &primary, &images) else { return };
        let (sw, sh) = scaled_size(w, h, scale.percent);
        if let Some(handle) = st.image.clone() {
            let cur = images.get(&handle).map(|i| (i.width(), i.height()));
            if cur != Some((sw, sh)) {
                if let Some(mut img) = images.get_mut(&handle) {
                    img.resize(Extent3d { width: sw, height: sh, depth_or_array_layers: 1 });
                }
                proj.set_changed();
            }
        }
    }
}

pub struct RenderScalePlugin {
    pub scale: RenderScale,
}

impl Plugin for RenderScalePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.scale)
            .init_resource::<RenderScaleState>()
            .add_systems(PostUpdate, sync_render_scale);
    }
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, px: [u8; 4]) -> Vec<u8> {
        (0..w * h).flat_map(|_| px).collect()
    }

    #[test]
    fn mip_chain_sizes_and_layout() {
        assert_eq!(mip_count(256, 256), 9);
        assert_eq!(mip_count(4096, 4096), 13);
        assert_eq!(mip_count(5, 3), 3);
        let src = solid(8, 4, [255, 0, 0, 255]);
        let c = build_mip_chain(&src, 8, 4, true, 0, true);
        assert_eq!((c.width, c.height, c.levels), (8, 4, 4));
        // 8x4 + 4x2 + 2x1 + 1x1 texels.
        assert_eq!(c.data.len(), (32 + 8 + 2 + 1) * 4);
        assert!(c.data.chunks_exact(4).all(|p| p == [255, 0, 0, 255]), "a solid colour stays solid");
    }

    #[test]
    fn srgb_average_is_done_in_linear_light() {
        // Black and white texels: linear mean 0.5 -> sRGB ~188, not 128.
        let src = [0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255];
        let (d, w, h) = downsample_rgba8(&src, 2, 2, true);
        assert_eq!((w, h), (1, 1));
        assert!((186..=190).contains(&d[0]), "{}", d[0]);
        let (d, _, _) = downsample_rgba8(&src, 2, 2, false);
        assert_eq!(d[0], 128);
    }

    #[test]
    fn reduction_halves_and_skips_mips_when_asked() {
        let src = solid(64, 32, [10, 20, 30, 255]);
        let c = build_mip_chain(&src, 64, 32, false, 2, false);
        assert_eq!((c.width, c.height, c.levels), (16, 8, 1));
        assert_eq!(c.data.len(), 16 * 8 * 4);
    }

    #[test]
    fn cutout_alpha_keeps_its_coverage() {
        // A 1-texel wide alpha line on 4x4: plain averaging would drop it under the cut-off.
        let mut src = solid(4, 4, [0, 200, 0, 0]);
        for y in 0..4 {
            src[(y * 4 + 1) * 4 + 3] = 255;
        }
        let c = build_mip_chain(&src, 4, 4, true, 0, true);
        let level1 = &c.data[64..64 + 16];
        assert!(level1.chunks_exact(4).any(|p| p[3] >= 128), "{level1:?}");
    }

    #[test]
    fn limit_texture_downscales_to_the_cap() {
        let t = TextureData { width: 64, height: 64, rgba: solid(64, 64, [1, 2, 3, 255]) };
        let r = limit_texture(t, 16, true);
        assert_eq!((r.width, r.height, r.rgba.len()), (16, 16, 16 * 16 * 4));
        let t = TextureData { width: 64, height: 64, rgba: solid(64, 64, [1, 2, 3, 255]) };
        assert_eq!(limit_texture(t, 0, true).width, 64);
    }

    #[test]
    fn sampler_keeps_address_modes_and_sets_anisotropy() {
        let base = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: bevy::image::ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::nearest()
        });
        let ImageSampler::Descriptor(d) = sampler_for(&base, true, 8) else { panic!() };
        assert_eq!(d.address_mode_u, bevy::image::ImageAddressMode::Repeat);
        assert_eq!(d.anisotropy_clamp, 8);
        assert_eq!(d.mipmap_filter, ImageFilterMode::Linear);
        let ImageSampler::Descriptor(d) = sampler_for(&ImageSampler::Default, false, 1) else { panic!() };
        assert_eq!(d.anisotropy_clamp, 1);
    }

    #[test]
    fn scaled_sizes_and_deadlines() {
        assert_eq!(scaled_size(1920, 1080, 100), (1920, 1080));
        assert_eq!(scaled_size(1920, 1080, 150), (2880, 1620));
        assert_eq!(scaled_size(1920, 1080, 50), (960, 540));
        let now = Instant::now();
        let p = Duration::from_millis(8);
        assert_eq!(next_deadline(None, now, p), now + p);
        assert_eq!(next_deadline(Some(now), now, p), now + p);
        // Late by more than a period: no catch-up.
        assert_eq!(next_deadline(Some(now), now + Duration::from_millis(20), p), now + Duration::from_millis(28));
        assert_eq!(FramePacing { vsync: false, fps_cap: 120 }.period(), Some(Duration::from_secs_f64(1.0 / 120.0)));
        assert_eq!(FramePacing::default().period(), None);
    }

    fn texture_app(cfg: TextureQuality) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::image::ImagePlugin::default()));
        app.init_asset::<StandardMaterial>();
        app.add_plugins(TextureQualityPlugin { settings: cfg });
        app
    }

    fn add_textured(app: &mut App, w: u32, h: u32) -> (Handle<Image>, Handle<StandardMaterial>) {
        let img = Image::new(
            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            TextureDimension::D2,
            solid(w, h, [100, 150, 200, 255]),
            TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::default(),
        );
        let ih = app.world_mut().resource_mut::<Assets<Image>>().add(img);
        let mh = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial { base_color_texture: Some(ih.clone()), ..default() });
        (ih, mh)
    }

    /// Run frames until the asset events were read and every background job was written back.
    fn settle(app: &mut App) {
        for i in 0..400 {
            app.update();
            if i >= 6 && !app.world().resource::<TextureRegistry>().busy() {
                app.update();
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("texture jobs did not finish");
    }

    #[test]
    fn headless_defaults_leave_images_untouched() {
        let mut app = texture_app(TextureQuality::default());
        let (ih, _mh) = add_textured(&mut app, 64, 64);
        settle(&mut app);
        let img = app.world().resource::<Assets<Image>>().get(&ih).unwrap().clone();
        assert_eq!(img.texture_descriptor.mip_level_count, 1);
        assert_eq!(img.sampler, ImageSampler::Default);
        assert_eq!(app.world().resource::<TextureRegistry>().processed, 0);
    }

    #[test]
    fn headless_mipmaps_anisotropy_and_live_toggle() {
        let mut app = texture_app(TextureQuality { mipmaps: true, anisotropy: 8, reduce: 1 });
        let (ih, _mh) = add_textured(&mut app, 64, 32);
        settle(&mut app);
        {
            let img = app.world().resource::<Assets<Image>>().get(&ih).unwrap();
            assert_eq!((img.width(), img.height()), (32, 16), "half texture quality");
            assert_eq!(img.texture_descriptor.mip_level_count, 6);
            let ImageSampler::Descriptor(d) = &img.sampler else { panic!("descriptor expected") };
            assert_eq!(d.anisotropy_clamp, 8);
        }
        // Live: mipmaps off, anisotropy 2x; the reduction is not applied twice.
        *app.world_mut().resource_mut::<TextureQuality>() = TextureQuality { mipmaps: false, anisotropy: 2, reduce: 1 };
        settle(&mut app);
        let img = app.world().resource::<Assets<Image>>().get(&ih).unwrap();
        assert_eq!((img.width(), img.height(), img.texture_descriptor.mip_level_count), (32, 16, 1));
        assert_eq!(img.data.as_ref().unwrap().len(), 32 * 16 * 4);
        let ImageSampler::Descriptor(d) = &img.sampler else { panic!() };
        assert_eq!(d.anisotropy_clamp, 2);
        // Mipmaps back on.
        app.world_mut().resource_mut::<TextureQuality>().mipmaps = true;
        settle(&mut app);
        let img = app.world().resource::<Assets<Image>>().get(&ih).unwrap();
        assert_eq!(img.texture_descriptor.mip_level_count, 6);
    }

    #[test]
    fn headless_render_scale_retargets_and_restores_the_camera() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::image::ImagePlugin::default()));
        app.add_plugins(RenderScalePlugin { scale: RenderScale { percent: 150 } });
        let out = app.world_mut().resource_mut::<Assets<Image>>().add(Image::new_target_texture(
            200,
            100,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        let cam = app.world_mut().spawn((Camera3d::default(), RenderTarget::Image(out.clone().into()), IsDefaultUiCamera)).id();
        app.update();
        app.update();
        let t = app.world().get::<RenderTarget>(cam).unwrap().clone();
        let RenderTarget::Image(scaled) = t else { panic!("camera must draw into the scaled image") };
        assert_ne!(scaled.handle, out);
        let img = app.world().resource::<Assets<Image>>().get(&scaled.handle).unwrap();
        assert_eq!((img.width(), img.height()), (300, 150));
        assert!(app.world().get::<IsDefaultUiCamera>(cam).is_none(), "the compositor draws the UI");
        let mut q = app.world_mut().query_filtered::<&RenderTarget, With<RenderScaleCompositor>>();
        assert_eq!(q.iter(app.world()).count(), 1);
        // Back to 100 %.
        app.world_mut().resource_mut::<RenderScale>().percent = 100;
        app.update();
        app.update();
        assert!(same_target(app.world().get::<RenderTarget>(cam).unwrap(), &RenderTarget::Image(out.into())));
        assert!(app.world().get::<IsDefaultUiCamera>(cam).is_some());
        let mut q = app.world_mut().query_filtered::<Entity, With<RenderScaleCompositor>>();
        assert_eq!(q.iter(app.world()).count(), 0);
    }
}
