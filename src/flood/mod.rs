use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::{Core3d, Core3dSystems};
use bevy::pbr::{MeshInputUniform, MeshUniform};
use bevy::render::batching::gpu_preprocessing::{BatchedInstanceBuffers, GpuPreprocessingSupport};
use bevy::render::render_phase::{
    sort_phase_system, AddRenderCommand, DrawFunctions, SortedRenderPhasePlugin,
};
use bevy::render::RenderDebugFlags;
use bevy::{
    prelude::*,
    render::{
        camera::ExtractedCamera,
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        renderer::RenderDevice,
        texture::{CachedTexture, TextureCache},
        Render, RenderApp, RenderStartup, RenderSystems,
    },
};
use compose_output::{
    init_compose_output_pipeline, prepare_compose_output_pass, prepare_compose_output_uniform,
    ComposeOutputUniforms,
};
use flood_init::{prepare_flood_phases, queue_flood_meshes};
use jump_flood::init_jump_flood_pipeline;
use node::{flood_render_pass, FloodOutline};
use sobel_init::init_sobel_init_pipeline;

use crate::add_dummy_phase_buffer;
use crate::node::outline_render_pass;
use crate::pipeline::OutlinePipeline;
use crate::render::DrawOutline;
use crate::uniforms::DrawMode;
use crate::view_uniforms::OutlineViewUniform;

mod compose_output;
mod flood_init;
mod jump_flood;
mod node;
mod sobel_init;

const JUMP_FLOOD_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("66f5981f-0cc2-4e62-8221-cd495062f3ac");
const COMPOSE_OUTPUT_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("3c0c1990-4202-48ef-8aa4-bbbb3a334471");
const SOBEL_INIT_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("e011500d-544c-4a0a-85ee-e7de0b1fda3f");

#[derive(Clone)]
pub(crate) struct FloodCoverageTextures {
    pub msaa_tex: CachedTexture,
    pub resolved: CachedTexture,
}

#[derive(Clone, Component)]
pub(crate) struct FloodTextures {
    pub flip: bool,
    pub texture_a: CachedTexture,
    pub texture_b: CachedTexture,
    pub coverage: Option<FloodCoverageTextures>,
}

impl FloodTextures {
    pub fn input(&self) -> &CachedTexture {
        if self.flip {
            &self.texture_b
        } else {
            &self.texture_a
        }
    }

    pub fn output(&self) -> &CachedTexture {
        if self.flip {
            &self.texture_a
        } else {
            &self.texture_b
        }
    }

    pub fn flip(&mut self) {
        self.flip = !self.flip;
    }
}

pub fn prepare_flood_textures(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    cameras: Query<(Entity, &ExtractedCamera, &Msaa)>,
) {
    for (entity, camera, msaa) in cameras.iter() {
        let Some(target_size) = camera.physical_target_size else {
            continue;
        };

        let size = Extent3d {
            width: target_size.x,
            height: target_size.y,
            depth_or_array_layers: 1,
        };

        let texture_descriptor = TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rg16Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        let coverage = if msaa.samples() > 1 {
            let coverage_descriptor = TextureDescriptor {
                format: TextureFormat::R8Unorm,
                ..texture_descriptor.clone()
            };
            let msaa_tex = texture_cache.get(
                &render_device,
                TextureDescriptor {
                    label: Some("outline_flood_coverage_msaa"),
                    sample_count: msaa.samples(),
                    usage: TextureUsages::RENDER_ATTACHMENT,
                    ..coverage_descriptor.clone()
                },
            );
            let resolved = texture_cache.get(
                &render_device,
                TextureDescriptor {
                    label: Some("outline_flood_coverage_resolved"),
                    ..coverage_descriptor
                },
            );
            Some(FloodCoverageTextures { msaa_tex, resolved })
        } else {
            None
        };

        commands.entity(entity).insert(FloodTextures {
            flip: false,
            texture_a: texture_cache.get(&render_device, texture_descriptor.clone()),
            texture_b: texture_cache.get(&render_device, texture_descriptor),
            coverage,
        });
    }
}

fn add_dummy_phase_buffers(
    mut bibs: ResMut<BatchedInstanceBuffers<MeshUniform, MeshInputUniform>>,
) {
    add_dummy_phase_buffer::<FloodOutline>(&mut bibs);
}

#[derive(Debug)]
pub struct FloodPlugin;

impl Plugin for FloodPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            JUMP_FLOOD_SHADER_HANDLE,
            "jump_flood.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            COMPOSE_OUTPUT_SHADER_HANDLE,
            "compose_output.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            SOBEL_INIT_SHADER_HANDLE,
            "sobel_init.wgsl",
            Shader::from_wgsl
        );

        app.add_plugins(SortedRenderPhasePlugin::<FloodOutline, OutlinePipeline>::new(
            RenderDebugFlags::empty(),
        ))
        .sub_app_mut(RenderApp)
        .init_resource::<ComposeOutputUniforms>()
        .init_resource::<DrawFunctions<FloodOutline>>()
        .add_render_command::<FloodOutline, DrawOutline>()
        .add_systems(
            Render,
            prepare_flood_phases
                .after(RenderSystems::ExtractCommands)
                .before(RenderSystems::QueueMeshes),
        )
        .add_systems(
            Render,
            (prepare_flood_textures, prepare_compose_output_uniform, prepare_compose_output_pass).in_set(RenderSystems::Prepare),
        )
        .add_systems(Render, queue_flood_meshes.in_set(RenderSystems::QueueMeshes))
        .add_systems(
            Render,
            sort_phase_system::<FloodOutline>.in_set(RenderSystems::PhaseSort),
        )
        .add_systems(
            Core3d,
            flood_render_pass
                .after(outline_render_pass)
                .in_set(Core3dSystems::PostProcess),
        );
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app.add_systems(
            RenderStartup,
            (
                init_sobel_init_pipeline,
                init_jump_flood_pipeline,
                init_compose_output_pipeline,
            ),
        );

        let gpu_preprocessing_support = render_app.world().resource::<GpuPreprocessingSupport>();
        if gpu_preprocessing_support.is_available() {
            render_app.add_systems(
                Render,
                add_dummy_phase_buffers.in_set(RenderSystems::PrepareResourcesCollectPhaseBuffers),
            );
        }
    }
}
