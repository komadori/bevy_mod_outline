use bevy::asset::load_internal_asset;
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::render::extract_component::UniformComponentPlugin;
use bevy::render::render_phase::{
    sort_phase_system, AddRenderCommand, DrawFunctions, SortedRenderPhasePlugin,
};
use bevy::{
    prelude::*,
    render::{
        camera::ExtractedCamera,
        render_graph::{RenderGraphApp, ViewNodeRunner},
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        renderer::RenderDevice,
        texture::{CachedTexture, TextureCache},
        Render, RenderApp, RenderSet,
    },
};
use compose_output::{
    prepare_compose_output_pass, prepare_compose_output_uniform, ComposeOutputPipeline,
    ComposeOutputUniform,
};
use flood_init::{prepare_flood_phases, queue_flood_meshes};
use jump_flood::JumpFloodPipeline;
use node::{FloodNode, FloodOutline};

use crate::pipeline::{OutlinePipeline, PassType, PipelineKey};
use crate::render::DrawOutline;
use crate::uniforms::{DepthMode, DrawMode};
use crate::view_uniforms::OutlineViewUniform;
use crate::NodeOutline;

mod compose_output;
mod flood_init;
mod jump_flood;
mod node;

const JUMP_FLOOD_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(195769259403206220662202295475630400955);
const COMPOSE_OUTPUT_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(264213480312212708534425965374302965944);

#[derive(Clone, Component)]
pub(crate) struct FloodTextures {
    pub flip: bool,
    pub texture_a: CachedTexture,
    pub texture_b: CachedTexture,
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
    cameras: Query<(Entity, &ExtractedCamera)>,
) {
    for (entity, camera) in cameras.iter() {
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
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        commands.entity(entity).insert(FloodTextures {
            flip: false,
            texture_a: texture_cache.get(&render_device, texture_descriptor.clone()),
            texture_b: texture_cache.get(&render_device, texture_descriptor),
        });
    }
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
        app.add_plugins((
            UniformComponentPlugin::<ComposeOutputUniform>::default(),
            SortedRenderPhasePlugin::<FloodOutline, OutlinePipeline>::default(),
        ))
        .sub_app_mut(RenderApp)
        .init_resource::<DrawFunctions<FloodOutline>>()
        .add_render_command::<FloodOutline, DrawOutline>()
        .add_systems(
            Render,
            prepare_flood_phases
                .after(RenderSet::ExtractCommands)
                .before(RenderSet::QueueMeshes),
        )
        .add_systems(
            Render,
            prepare_compose_output_uniform
                .after(RenderSet::ExtractCommands)
                .before(RenderSet::PrepareResources),
        )
        .add_systems(
            Render,
            (prepare_flood_textures, prepare_compose_output_pass).in_set(RenderSet::Prepare),
        )
        .add_systems(Render, queue_flood_meshes.in_set(RenderSet::QueueMeshes))
        .add_systems(
            Render,
            sort_phase_system::<FloodOutline>.in_set(RenderSet::PhaseSort),
        )
        .add_render_graph_node::<ViewNodeRunner<FloodNode>>(Core3d, NodeOutline::FloodPass)
        .add_render_graph_edges(
            Core3d,
            (
                NodeOutline::OutlinePass,
                NodeOutline::FloodPass,
                NodeOutline::EndOutlinePasses,
            ),
        );
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<JumpFloodPipeline>()
            .init_resource::<ComposeOutputPipeline>();
    }
}