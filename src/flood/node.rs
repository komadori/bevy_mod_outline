use bevy::ecs::entity::EntityHash;
use bevy::render::render_phase::{
    CachedRenderPipelinePhaseItem, DrawFunctionId, PhaseItem, ViewSortedRenderPhases,
};
use bevy::render::view::{ExtractedView, ViewDepthTexture};
use bevy::{
    math::FloatOrd,
    prelude::*,
    render::{
        camera::ExtractedCamera,
        render_phase::{PhaseItemExtraIndex, SortedPhaseItem},
        render_resource::{CachedRenderPipelineId, PipelineCache},
        renderer::{RenderContext, ViewQuery},
        sync_world::MainEntity,
        view::ViewTarget,
    },
};
use indexmap::IndexMap;
use itertools::*;
use std::ops::Range;

use crate::node::{OutlineRangefinder, OutlineSortingInfo};
use crate::OutlineViewUniform;

use super::compose_output::{ComposeOutputPass, ComposeOutputView};
use super::flood_init::FloodInitPass;
use super::jump_flood::JumpFloodPass;
use super::sobel_init::SobelInitPass;
use super::FloodTextures;

#[derive(Debug)]
pub struct FloodOutline {
    pub sorting_info: OutlineSortingInfo,
    pub distance: f32,
    pub entity: Entity,
    pub main_entity: MainEntity,
    pub pipeline: CachedRenderPipelineId,
    pub draw_function: DrawFunctionId,
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
    pub indexed: bool,
    pub volume_offset: f32,
    pub volume_colour: Vec4,
    pub screen_space_bounds: URect,
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
        self.extra_index.clone()
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

    fn recalculate_sort_keys(
        items: &mut IndexMap<(Entity, MainEntity), Self, EntityHash>,
        view: &ExtractedView,
    ) {
        let rangefinder = OutlineRangefinder::new(view);
        for item in items.values_mut() {
            item.distance = rangefinder.distance_of(&item.sorting_info);
        }
    }

    fn indexed(&self) -> bool {
        self.indexed
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn flood_render_pass(
    world: &World,
    view: ViewQuery<(
        &ExtractedView,
        &ExtractedCamera,
        &ViewTarget,
        &ViewDepthTexture,
        &OutlineViewUniform,
        &FloodTextures,
        &ComposeOutputView,
        &Msaa,
    )>,
    flood_phases: Res<ViewSortedRenderPhases<FloodOutline>>,
    pipeline_cache: Res<PipelineCache>,
    mut render_context: RenderContext,
) {
    let view_entity = view.entity();
    let (
        view_extracted,
        camera,
        target,
        depth,
        view_uniform,
        flood_textures,
        compose_output_view,
        msaa,
    ) = view.into_inner();

    let Some(flood_phase) = flood_phases.get(&view_extracted.retained_view_entity) else {
        return;
    };

    let mut flood_textures = flood_textures.clone();
    let coverage_init = msaa.samples() > 1;

    let mut flood_init_pass = FloodInitPass::new(world, view_entity, flood_phase, camera);
    let Some(mut jump_flood_pass) = JumpFloodPass::new(world) else {
        return;
    };
    let sobel_init_pass = if coverage_init {
        let Some(pass) = SobelInitPass::new(world) else {
            return;
        };
        Some(pass)
    } else {
        None
    };
    let Some(compose_output_pass) =
        ComposeOutputPass::new(world, compose_output_view, target, depth)
    else {
        return;
    };

    for ((_, volume_offset, _), group) in &flood_phase
        .items
        .values()
        .enumerate()
        .chunk_by(|(_, item)| (item.distance, item.volume_offset, item.volume_colour))
    {
        let mut group_iter = group.into_iter();
        let Some((first_index, first_item)) = group_iter.next() else {
            continue;
        };

        // Sum item range and screen-space bounds
        let mut last_index = first_index;
        let mut screen_space_bounds = first_item.screen_space_bounds;
        for (index, item) in group_iter {
            last_index = index;
            screen_space_bounds = screen_space_bounds.union(item.screen_space_bounds);
        }

        if let (Some(sobel_init_pass), Some(coverage)) =
            (sobel_init_pass.as_ref(), flood_textures.coverage.as_ref())
        {
            flood_init_pass.execute_coverage(
                &mut render_context,
                first_index..last_index + 1,
                &coverage.msaa_tex,
                &coverage.resolved,
            );
            sobel_init_pass.execute(
                &mut render_context,
                &coverage.resolved,
                flood_textures.output(),
                &pipeline_cache,
                &screen_space_bounds,
            );
        } else {
            flood_init_pass.execute_direct(
                &mut render_context,
                first_index..last_index + 1,
                flood_textures.output(),
            );
        }
        flood_textures.flip();

        let scaled_offset = view_uniform.scale_physical_from_logical * volume_offset;
        let passes = if scaled_offset > 0.0 {
            (scaled_offset.ceil() as u32 / 2 + 1)
                .next_power_of_two()
                .trailing_zeros()
                + 1
        } else {
            0
        };

        for size in (0..passes).rev() {
            jump_flood_pass.execute(
                &mut render_context,
                flood_textures.input(),
                flood_textures.output(),
                &pipeline_cache,
                size,
                &screen_space_bounds,
            );
            flood_textures.flip();
        }

        compose_output_pass.execute(
            &mut render_context,
            view_entity,
            first_item.main_entity,
            flood_textures.input(),
            &pipeline_cache,
            &screen_space_bounds,
        );
    }
}
