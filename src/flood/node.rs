use bevy::ecs::query::QueryItem;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex};
use bevy::render::render_phase::{
    CachedRenderPipelinePhaseItem, DrawFunctionId, PhaseItem, ViewSortedRenderPhases,
};
use bevy::render::render_resource::PipelineCache;
use bevy::render::renderer::RenderQueue;
use bevy::render::view::ViewDepthTexture;
use bevy::{
    math::FloatOrd,
    prelude::*,
    render::{
        camera::ExtractedCamera,
        render_graph::{NodeRunError, RenderGraphContext, ViewNode},
        render_phase::{PhaseItemExtraIndex, SortedPhaseItem},
        render_resource::CachedRenderPipelineId,
        renderer::RenderContext,
        sync_world::MainEntity,
        view::ViewTarget,
    },
};
use std::ops::Range;

use super::compose_output::ComposeOutputView;
use super::{
    compose_output_pass, flood_init_pass, jump_flood_pass, ComposeOutputPipeline,
    ComposeOutputUniform, FloodTextures, JumpFloodPipeline,
};

#[derive(Debug)]
pub struct FloodOutline {
    pub distance: f32,
    pub entity: Entity,
    pub main_entity: MainEntity,
    pub pipeline: CachedRenderPipelineId,
    pub draw_function: DrawFunctionId,
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
}

impl PhaseItem for FloodOutline {
    #[inline]
    fn entity(&self) -> Entity {
        self.entity
    }

    #[inline]
    fn main_entity(&self) -> MainEntity {
        self.main_entity
    }

    #[inline]
    fn draw_function(&self) -> DrawFunctionId {
        self.draw_function
    }

    #[inline]
    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    #[inline]
    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    #[inline]
    fn extra_index(&self) -> PhaseItemExtraIndex {
        self.extra_index
    }

    #[inline]
    fn batch_range_and_extra_index_mut(&mut self) -> (&mut Range<u32>, &mut PhaseItemExtraIndex) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl CachedRenderPipelinePhaseItem for FloodOutline {
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.pipeline
    }
}

impl SortedPhaseItem for FloodOutline {
    type SortKey = FloatOrd;

    fn sort_key(&self) -> Self::SortKey {
        FloatOrd(self.distance)
    }
}

pub(crate) struct FloodNode;

impl FromWorld for FloodNode {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

impl ViewNode for FloodNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ViewTarget,
        &'static ViewDepthTexture,
        &'static FloodTextures,
        &'static ComposeOutputView,
    );

    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, target, depth, flood_textures, compose_output_view): QueryItem<
            'w,
            Self::ViewQuery,
        >,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let view_entity = graph.view_entity();
        let Some(flood_phase) = world
            .get_resource::<ViewSortedRenderPhases<FloodOutline>>()
            .and_then(|ps| ps.get(&view_entity))
        else {
            return Ok(());
        };

        let mut flood_textures = flood_textures.clone();

        let pipeline_cache = world.resource::<PipelineCache>();
        let jump_flood_pipeline = world.resource::<JumpFloodPipeline>();
        let Some(jump_flood_render) =
            pipeline_cache.get_render_pipeline(jump_flood_pipeline.pipeline_id)
        else {
            return Ok(());
        };
        let compose_output_pipeline = world.resource::<ComposeOutputPipeline>();
        let Some(compose_output_render) =
            pipeline_cache.get_render_pipeline(compose_output_view.pipeline_id)
        else {
            return Ok(());
        };

        let compose_output_uniforms = world.resource::<ComponentUniforms<ComposeOutputUniform>>();
        let render_queue = world.resource::<RenderQueue>();

        for i in 0..flood_phase.items.len() {
            let render_entity = flood_phase.items[i].entity;
            let component_output_uniform = world
                .entity(render_entity)
                .get::<ComposeOutputUniform>()
                .unwrap();
            let dynamic_index = world
                .entity(render_entity)
                .get::<DynamicUniformIndex<ComposeOutputUniform>>()
                .unwrap()
                .index();

            flood_init_pass(
                world,
                view_entity,
                flood_phase,
                i,
                camera,
                render_context,
                flood_textures.output(),
            );

            flood_textures.flip();

            let mut size = if component_output_uniform.volume_offset > 0.0 {
                (component_output_uniform.volume_offset as u32).next_power_of_two()
            } else {
                0
            };
            while size != 0 {
                jump_flood_pass(
                    jump_flood_pipeline,
                    render_queue,
                    jump_flood_render,
                    render_context,
                    flood_textures.input(),
                    flood_textures.output(),
                    size,
                );

                size >>= 1;
                flood_textures.flip();
            }

            compose_output_pass(
                compose_output_pipeline,
                compose_output_render,
                render_context,
                compose_output_uniforms,
                dynamic_index,
                flood_textures.input(),
                target,
                depth,
            );
        }

        Ok(())
    }
}
