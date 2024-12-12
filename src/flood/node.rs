use bevy::ecs::query::QueryItem;
use bevy::render::render_phase::{
    CachedRenderPipelinePhaseItem, DrawFunctionId, PhaseItem, ViewSortedRenderPhases,
};
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

use super::compose_output::{ComposeOutputPass, ComposeOutputView};
use super::flood_init::FloodInitPass;
use super::jump_flood::JumpFloodPass;
use super::FloodTextures;

#[derive(Debug)]
pub struct FloodOutline {
    pub distance: f32,
    pub entity: Entity,
    pub main_entity: MainEntity,
    pub pipeline: CachedRenderPipelineId,
    pub draw_function: DrawFunctionId,
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
    pub volume_offset: f32,
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

        let mut flood_init_pass = FloodInitPass::new(world, view_entity, flood_phase, camera);
        let mut jump_flood_pass = JumpFloodPass::new(world);
        let compose_output_pass = ComposeOutputPass::new(world, compose_output_view, target, depth);

        for index in 0..flood_phase.items.len() {
            let item = &flood_phase.items[index];

            flood_init_pass.execute(render_context, index, flood_textures.output());
            flood_textures.flip();

            let passes = if item.volume_offset > 0.0 {
                (item.volume_offset.ceil() as u32 / 2 + 1)
                    .next_power_of_two()
                    .trailing_zeros()
                    + 1
            } else {
                0
            };
            for size in (0..passes).rev() {
                jump_flood_pass.execute(
                    render_context,
                    flood_textures.input(),
                    flood_textures.output(),
                    size,
                );
                flood_textures.flip();
            }

            compose_output_pass.execute(render_context, item.entity, flood_textures.input());
        }

        Ok(())
    }
}
