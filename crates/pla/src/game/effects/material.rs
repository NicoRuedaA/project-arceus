//! GPU material of effect particles: one textured, vertex-coloured quad mesh per
//! emitter, drawn in the transparent pass over the scene (no depth test or writes), with three blend
//! modes (alpha, additive, subtractive). The WGSL is embedded (`effects.wgsl`).

use bevy::asset::uuid_handle;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, CompareFunction, RenderPipelineDescriptor,
    SpecializedMeshPipelineError,
};
use bevy::shader::{Shader, ShaderRef};

use super::sim::BlendMode;

/// Handle of the embedded particle shader.
pub const EFFECT_SHADER: Handle<Shader> = uuid_handle!("5b0e0c3e-7c1a-4f0e-9d7e-2f4b8a6c1d93");

/// Register the embedded shader (call once, needs `Assets<Shader>`).
pub fn register_shader(app: &mut App) {
    let shader = Shader::from_wgsl(include_str!("effects.wgsl"), "pla://game/effects/effects.wgsl");
    if let Some(mut shaders) = app.world_mut().get_resource_mut::<Assets<Shader>>() {
        let _ = shaders.insert(&EFFECT_SHADER, shader);
    }
}

/// Pipeline key: the blend mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectMaterialKey {
    pub blend: BlendMode,
}

impl From<&EffectMaterial> for EffectMaterialKey {
    fn from(m: &EffectMaterial) -> Self {
        EffectMaterialKey { blend: m.blend }
    }
}

/// Particle material (see the module doc).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(EffectMaterialKey)]
pub struct EffectMaterial {
    /// x: intensity, y: 1 = no texture (soft dot).
    #[uniform(0)]
    pub params: Vec4,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
    pub blend: BlendMode,
}

/// Fixed-function blend state of a mode, for premultiplied shader output.
pub fn blend_state(mode: BlendMode) -> BlendState {
    let keep_alpha = BlendComponent {
        src_factor: BlendFactor::Zero,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Add,
    };
    match mode {
        BlendMode::Alpha => BlendState::PREMULTIPLIED_ALPHA_BLENDING,
        BlendMode::Add => BlendState {
            color: BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
            alpha: keep_alpha,
        },
        BlendMode::Sub => BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::ReverseSubtract,
            },
            alpha: keep_alpha,
        },
    }
}

impl Material for EffectMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(EFFECT_SHADER)
    }

    fn alpha_mode(&self) -> AlphaMode {
        match self.blend {
            BlendMode::Add => AlphaMode::Add,
            _ => AlphaMode::Blend,
        }
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(blend_state(key.bind_group_data.blend));
            }
        }
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = Some(false);
            // Battle effects are centred inside the Pokémon (no bone locators yet), so a depth
            // test hides most of them behind the model; draw them over the scene instead
            // (port choice, ASSUMED to be closer to the game's look than hidden effects).
            depth.depth_compare = Some(CompareFunction::Always);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_states_match_the_modes() {
        assert_eq!(blend_state(BlendMode::Alpha), BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let add = blend_state(BlendMode::Add);
        assert_eq!(add.color.dst_factor, BlendFactor::One);
        assert_eq!(add.color.operation, BlendOperation::Add);
        let sub = blend_state(BlendMode::Sub);
        assert_eq!(sub.color.operation, BlendOperation::ReverseSubtract, "destination minus particle");
        assert_eq!(EffectMaterialKey::from(&EffectMaterial { params: Vec4::ONE, texture: None, blend: BlendMode::Sub }).blend, BlendMode::Sub);
    }
}
