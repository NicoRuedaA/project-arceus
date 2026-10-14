//! GPU side of the original UI: one [`OrigUiMaterial`] per drawn quad, and the cache of the
//! exported texture PNGs (read from the player's local UI root, never from the repository).

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor,
    ShaderType,
};
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{UiMaterial, UiMaterialKey};

use super::scene::Quad;

/// Asset path of the embedded shader (see `embedded_asset!` in the plugin).
pub const SHADER_PATH: &str = "embedded://pla/game/orig_ui/orig_ui.wgsl";

/// Uniform block of `orig_ui.wgsl`.
#[derive(Debug, Clone, Copy, PartialEq, ShaderType, Default)]
pub struct OrigUiParams {
    pub uv01: Vec4,
    pub uv23: Vec4,
    pub vc0: Vec4,
    pub vc1: Vec4,
    pub vc2: Vec4,
    pub vc3: Vec4,
    pub black: Vec4,
    pub white: Vec4,
    /// translate.xy, rotate (radians), has_texture.
    pub srt: Vec4,
    /// scale.xy, wrap_s, wrap_t.
    pub srt_scale: Vec4,
    /// alpha, -, -, -.
    pub params: Vec4,
}

impl OrigUiParams {
    pub fn from_quad(q: &Quad, has_texture: bool) -> Self {
        Self {
            uv01: Vec4::new(q.uv[0].x, q.uv[0].y, q.uv[1].x, q.uv[1].y),
            uv23: Vec4::new(q.uv[2].x, q.uv[2].y, q.uv[3].x, q.uv[3].y),
            vc0: q.vcol[0],
            vc1: q.vcol[1],
            vc2: q.vcol[2],
            vc3: q.vcol[3],
            black: q.black,
            white: q.white,
            srt: Vec4::new(
                q.srt.translate[0],
                q.srt.translate[1],
                q.srt.rotate.to_radians(),
                if has_texture { 1.0 } else { 0.0 },
            ),
            srt_scale: Vec4::new(
                q.srt.scale[0],
                q.srt.scale[1],
                q.wrap[0] as f32,
                q.wrap[1] as f32,
            ),
            params: Vec4::new(q.alpha, if q.multiply { 1.0 } else { 0.0 }, 0.0, 0.0),
        }
    }
}

/// Pipeline key: additive blending for materials whose blend destination factor is "one".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct OrigUiKey {
    pub additive: bool,
    pub multiply: bool,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
#[bind_group_data(OrigUiKey)]
pub struct OrigUiMaterial {
    #[uniform(0)]
    pub params: OrigUiParams,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
    pub additive: bool,
    pub multiply: bool,
}

impl From<&OrigUiMaterial> for OrigUiKey {
    fn from(m: &OrigUiMaterial) -> Self {
        OrigUiKey {
            additive: m.additive,
            multiply: m.multiply,
        }
    }
}

impl UiMaterial for OrigUiMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, key: UiMaterialKey<Self>) {
        let color = if key.bind_group_data.additive {
            BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            }
        } else if key.bind_group_data.multiply {
            // The shader already faded the colour towards white by its alpha.
            BlendComponent {
                src_factor: BlendFactor::Zero,
                dst_factor: BlendFactor::Src,
                operation: BlendOperation::Add,
            }
        } else {
            return;
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState {
                    color,
                    alpha: BlendComponent::OVER,
                });
            }
        }
    }
}

/// Decoded PNGs by path relative to the UI root (`None` = missing or unreadable, logged once).
#[derive(Resource, Debug, Default)]
pub struct UiTextureCache {
    pub root: PathBuf,
    pub images: HashMap<String, Option<Handle<Image>>>,
    pub missing: usize,
}

impl UiTextureCache {
    pub fn new(root: PathBuf) -> Self {
        Self { root, ..default() }
    }

    /// Handle of `rel` (loaded synchronously on first use; linear-format bytes, see the shader).
    pub fn get(&mut self, images: &mut Assets<Image>, rel: &str) -> Option<Handle<Image>> {
        if let Some(h) = self.images.get(rel) {
            return h.clone();
        }
        let path = self.root.join(rel);
        let handle = match std::fs::read(&path) {
            Ok(bytes) => match Image::from_buffer(
                &bytes,
                ImageType::Extension("png"),
                CompressedImageFormats::NONE,
                false,
                ImageSampler::linear(),
                RenderAssetUsages::RENDER_WORLD,
            ) {
                Ok(img) => Some(images.add(img)),
                Err(e) => {
                    warn!("original UI: cannot decode {}: {e}", path.display());
                    None
                }
            },
            Err(e) => {
                warn!("original UI: missing texture {}: {e}", path.display());
                None
            }
        };
        if handle.is_none() {
            self.missing += 1;
        }
        self.images.insert(rel.to_string(), handle.clone());
        handle
    }
}
