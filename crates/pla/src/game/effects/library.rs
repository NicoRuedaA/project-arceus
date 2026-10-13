//! Loading of effect timelines and particle files, with GPU textures and materials.
//!
//! Timelines (`.wbin`, a few KB) load synchronously on first use. Particle files
//! (`.ptcl`, up to several MB with their textures) load on one background thread
//! (or synchronously when `sync` is set, for tests and tools); the main thread
//! turns their decoded textures into Bevy images once ready. Textures are shared
//! by name (the same `cpt_*` texture appears in many move effects).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::assets::bntx::TextureData;
use crate::assets::effect_sequence::EffectSequence;
use crate::assets::ptcl::PtclFile;
use crate::assets::ptcl_textures::load_textures;

use super::material::EffectMaterial;
use super::sim::{BlendMode, EffectDef};
use super::timeline::EffectTimeline;

/// A particle file decoded off the main thread.
#[derive(Debug, Clone)]
pub struct LoadedParticle {
    pub def: Arc<EffectDef>,
    pub textures: HashMap<String, TextureData>,
    pub texture_errors: Vec<(String, String)>,
}

/// Read and decode a particle file (`path` under `bin/effect/battle/particle/`).
pub fn load_particle(bin_root: &Path, cache_dir: Option<&Path>, path: &str) -> Result<LoadedParticle, String> {
    let full = bin_root.join("effect/battle/particle").join(path);
    let bytes = std::fs::read(&full).map_err(|e| format!("{}: {e}", full.display()))?;
    let file = PtclFile::parse(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let def = EffectDef::from_ptcl(path, &file);
    let wanted = def.texture_names();
    let (tex, texture_errors) = load_textures(&bytes, &file, Some(&wanted), cache_dir);
    Ok(LoadedParticle {
        def: Arc::new(def),
        textures: tex.into_iter().map(|(k, v)| (k, v.data)).collect(),
        texture_errors,
    })
}

/// Read a timeline (`name` without extension, under `bin/effect/battle/sequence/`).
pub fn load_timeline(bin_root: &Path, name: &str) -> Result<EffectTimeline, String> {
    let full = bin_root.join("effect/battle/sequence").join(format!("{name}.wbin"));
    let bytes = std::fs::read(&full).map_err(|e| format!("{}: {e}", full.display()))?;
    let seq = EffectSequence::parse(&bytes).map_err(|e| format!("{name}: {e}"))?;
    Ok(EffectTimeline::from_sequence(name, &seq))
}

/// A particle file ready to play.
#[derive(Debug, Clone)]
pub struct ReadyParticle {
    pub def: Arc<EffectDef>,
    /// Texture images by name (empty when nothing is rendered).
    pub images: HashMap<String, Handle<Image>>,
}

#[derive(Debug, Clone)]
pub enum ParticleSlot {
    Loading,
    Ready(Arc<ReadyParticle>),
    Failed,
}

/// Counters for reports and tests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibraryStats {
    pub particles_loaded: usize,
    pub particles_failed: usize,
    pub timelines_loaded: usize,
    pub timelines_missing: usize,
    pub textures_failed: usize,
}

type LoadResult = (String, Result<LoadedParticle, String>);

/// Cache of timelines, particle files, images and materials.
#[derive(Resource)]
pub struct EffectLibrary {
    pub bin_root: PathBuf,
    pub cache_dir: Option<PathBuf>,
    /// Load particle files on the calling thread (tests, tools).
    pub sync: bool,
    timelines: HashMap<String, Option<Arc<EffectTimeline>>>,
    particles: HashMap<String, ParticleSlot>,
    images: HashMap<String, Handle<Image>>,
    materials: HashMap<(Option<String>, BlendMode, [u8; 2]), Handle<EffectMaterial>>,
    jobs: Option<Sender<String>>,
    results_tx: Sender<LoadResult>,
    results: Mutex<Receiver<LoadResult>>,
    /// Decoded but not yet converted to images (main thread).
    arrived: Vec<(String, LoadedParticle)>,
    pub stats: LibraryStats,
}

impl EffectLibrary {
    pub fn new(bin_root: PathBuf, cache_dir: Option<PathBuf>, sync: bool) -> Self {
        let (results_tx, rx) = channel();
        EffectLibrary {
            bin_root,
            cache_dir,
            sync,
            timelines: HashMap::new(),
            particles: HashMap::new(),
            images: HashMap::new(),
            materials: HashMap::new(),
            jobs: None,
            results_tx,
            results: Mutex::new(rx),
            arrived: Vec::new(),
            stats: LibraryStats::default(),
        }
    }

    /// Timeline by name (loaded on first use; `None` when missing or unreadable).
    pub fn timeline(&mut self, name: &str) -> Option<Arc<EffectTimeline>> {
        if let Some(t) = self.timelines.get(name) {
            return t.clone();
        }
        let t = match load_timeline(&self.bin_root, name) {
            Ok(t) => {
                self.stats.timelines_loaded += 1;
                Some(Arc::new(t))
            }
            Err(e) => {
                self.stats.timelines_missing += 1;
                warn!("effects: no timeline {name} ({e})");
                None
            }
        };
        self.timelines.insert(name.to_string(), t.clone());
        t
    }

    /// Start loading a particle file if it is not known yet.
    pub fn request(&mut self, path: &str) {
        if self.particles.contains_key(path) {
            return;
        }
        self.particles.insert(path.to_string(), ParticleSlot::Loading);
        if self.sync {
            let r = load_particle(&self.bin_root, self.cache_dir.as_deref(), path);
            let _ = self.results_tx.send((path.to_string(), r));
            return;
        }
        if self.jobs.is_none() {
            let (jtx, jrx) = channel::<String>();
            let out = self.results_tx.clone();
            let root = self.bin_root.clone();
            let cache = self.cache_dir.clone();
            let spawned = std::thread::Builder::new().name("pla-effects-loader".into()).spawn(move || {
                while let Ok(p) = jrx.recv() {
                    let r = load_particle(&root, cache.as_deref(), &p);
                    if out.send((p, r)).is_err() {
                        break;
                    }
                }
            });
            if spawned.is_err() {
                self.sync = true;
                self.particles.remove(path);
                self.request(path);
                return;
            }
            self.jobs = Some(jtx);
        }
        if let Some(j) = &self.jobs {
            let _ = j.send(path.to_string());
        }
    }

    pub fn slot(&self, path: &str) -> Option<&ParticleSlot> {
        self.particles.get(path)
    }

    /// Move finished loads into the library; textures become images when `images` is given.
    pub fn receive(&mut self, mut images: Option<&mut Assets<Image>>) {
        let incoming: Vec<LoadResult> = match self.results.lock() {
            Ok(rx) => rx.try_iter().collect(),
            Err(_) => Vec::new(),
        };
        for (path, r) in incoming {
            match r {
                Ok(p) => self.arrived.push((path, p)),
                Err(e) => {
                    warn!("effects: cannot load {path}: {e}");
                    self.stats.particles_failed += 1;
                    self.particles.insert(path, ParticleSlot::Failed);
                }
            }
        }
        for (path, loaded) in std::mem::take(&mut self.arrived) {
            self.stats.textures_failed += loaded.texture_errors.len();
            let mut handles = HashMap::new();
            if let Some(assets) = images.as_deref_mut() {
                for (name, tex) in loaded.textures {
                    let h = self.images.entry(name.clone()).or_insert_with(|| assets.add(to_image(tex))).clone();
                    handles.insert(name, h);
                }
            }
            self.stats.particles_loaded += 1;
            self.particles.insert(path, ParticleSlot::Ready(Arc::new(ReadyParticle { def: loaded.def, images: handles })));
        }
    }

    /// Material for a texture, blend mode and wrap mode (shared).
    pub fn material(
        &mut self,
        texture: Option<&str>,
        image: Option<&Handle<Image>>,
        blend: BlendMode,
        wrap: [u8; 2],
        materials: &mut Assets<EffectMaterial>,
    ) -> Handle<EffectMaterial> {
        let key = (image.and(texture.map(str::to_string)), blend, wrap);
        self.materials
            .entry(key)
            .or_insert_with(|| {
                materials.add(EffectMaterial {
                    params: Vec4::new(
                        1.0,
                        if image.is_some() { 0.0 } else { 1.0 },
                        wrap[0].min(2) as f32,
                        wrap[1].min(2) as f32,
                    ),
                    texture: image.cloned(),
                    blend,
                })
            })
            .clone()
    }

    /// Number of known particle files.
    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }

    /// Forget everything that is not loading (bounded memory between battles).
    pub fn trim(&mut self) {
        self.particles.retain(|_, s| matches!(s, ParticleSlot::Loading));
        self.images.clear();
        self.materials.clear();
        self.timelines.clear();
    }
}

/// RGBA8 sRGB image with linear filtering, kept only on the GPU.
pub fn to_image(t: TextureData) -> Image {
    let mut img = Image::new(
        Extent3d { width: t.width.max(1), height: t.height.max(1), depth_or_array_layers: 1 },
        TextureDimension::D2,
        t.rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::linear();
    img
}
