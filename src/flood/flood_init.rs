use std::ops::Range;

use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::mesh::allocator::MeshAllocator;
use bevy::render::render_phase::{DrawFunctions, PhaseItemExtraIndex, ViewSortedRenderPhases};
use bevy::render::view::ExtractedView;
use bevy::render::{
    render_phase::SortedRenderPhase,
    render_resource::{
        LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
    },
    renderer::RenderContext,
    texture::CachedTexture,
};

use crate::culling::RenderOutlineEntities;
use crate::node::OutlineSortingInfo;
use crate::queue::{
    DirtyOutlineSpecialisations, OutlineCache, OutlineCacheEntry, PendingOutlineQueues,
};
use crate::uniforms::RenderOutlineInstances;
use crate::view_uniforms::OutlineQueueStatus;

use super::node::FloodOutline;
use super::{DrawMode, DrawOutline, OutlineViewUniform, FLOOD_OPS};

pub(crate) fn prepare_flood_phases(
    query: Query<&ExtractedView, With<OutlineViewUniform>>,
    mut flood_phases: ResMut<ViewSortedRenderPhases<FloodOutline>>,
) {
    for view in query.iter() {
        flood_phases.prepare_for_new_frame(view.retained_view_entity);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn queue_flood_meshes(
    flood_draw_functions: Res<DrawFunctions<FloodOutline>>,
    mesh_allocator: Res<MeshAllocator>,
    outline_cache: Res<OutlineCache>,
    render_outlines: Res<RenderOutlineInstances>,
    render_visible: Res<RenderOutlineEntities>,
    pending_queues: ResMut<PendingOutlineQueues>,
    specialisations: Res<DirtyOutlineSpecialisations>,
    mut flood_phases: ResMut<ViewSortedRenderPhases<FloodOutline>>,
    mut views: Query<(&ExtractedView, &mut OutlineQueueStatus)>,
) {
    let draw_flood = flood_draw_functions.read().get_id::<DrawOutline>().unwrap();

    for (view, mut queue_status) in views.iter_mut() {
        let Some(flood_phase) = flood_phases.get_mut(&view.retained_view_entity) else {
            continue;
        };

        let outline_view_cache = outline_cache
            .view_map
            .get(&view.retained_view_entity)
            .unwrap();

        let Some(render_view_visible) = render_visible.views.get(&view.retained_view_entity) else {
            continue;
        };

        let Some(view_pending_queues) = pending_queues.get(&view.retained_view_entity) else {
            continue;
        };

        for &main_entity in
            specialisations.iter_to_dequeue(view.retained_view_entity, &render_view_visible.0)
        {
            flood_phase.remove(Entity::PLACEHOLDER, main_entity);
        }

        for (_, &main_entity) in specialisations.iter_to_queue(
            view.retained_view_entity,
            &render_view_visible.0,
            &view_pending_queues.prev_frame,
        ) {
            let Some(outline) = render_outlines.get(&main_entity) else {
                continue;
            };

            if outline.draw_mode != DrawMode::JumpFlood {
                continue;
            }

            let mesh_slabs = mesh_allocator.mesh_slabs(&outline.mesh_id);
            let index_slab = mesh_slabs.and_then(|s| s.index_slab_id);

            let Some(OutlineCacheEntry {
                stencil_pipeline_id: _,
                volume_pipeline_id,
            }) = outline_view_cache.entity_map.get(&main_entity)
            else {
                continue;
            };

            let sorting_info = OutlineSortingInfo {
                world_plane_origin: outline.instance_data.world_plane_origin,
                world_plane_offset: outline.instance_data.world_plane_offset,
            };
            flood_phase.add_retained(FloodOutline {
                sorting_info,
                distance: 0.0,
                entity: Entity::PLACEHOLDER,
                main_entity,
                pipeline: *volume_pipeline_id,
                draw_function: draw_flood,
                batch_range: 0..0,
                extra_index: PhaseItemExtraIndex::None,
                indexed: index_slab.is_some(),
                volume_offset: outline.instance_data.volume_offset,
                volume_colour: outline.instance_data.volume_colour,
                merge_group: outline.merge_group,
            });
        }

        if !flood_phase.items.is_empty() {
            queue_status.has_volume = true;
        }
    }
}

pub(crate) struct FloodInitPass<'w> {
    world: &'w World,
    view_entity: Entity,
    flood_phase: &'w SortedRenderPhase<FloodOutline>,
    camera: &'w ExtractedCamera,
}

impl<'w> FloodInitPass<'w> {
    pub fn new(
        world: &'w World,
        view_entity: Entity,
        flood_phase: &'w SortedRenderPhase<FloodOutline>,
        camera: &'w ExtractedCamera,
    ) -> Self {
        Self {
            world,
            view_entity,
            flood_phase,
            camera,
        }
    }

    pub fn execute_direct(
        &mut self,
        render_context: &mut RenderContext<'_, '_>,
        range: Range<usize>,
        output: &CachedTexture,
    ) {
        let color_attachment = RenderPassColorAttachment {
            view: &output.default_view,
            depth_slice: None,
            resolve_target: None,
            ops: FLOOD_OPS,
        };

        self.run(render_context, range, color_attachment);
    }

    pub fn execute_coverage(
        &mut self,
        render_context: &mut RenderContext<'_, '_>,
        range: Range<usize>,
        coverage_msaa: &CachedTexture,
        resolve_target: &CachedTexture,
    ) {
        let color_attachment = RenderPassColorAttachment {
            view: &coverage_msaa.default_view,
            depth_slice: None,
            resolve_target: Some(&resolve_target.default_view),
            ops: Operations {
                load: LoadOp::Clear(wgpu_types::Color::TRANSPARENT),
                store: StoreOp::Store,
            },
        };

        self.run(render_context, range, color_attachment);
    }

    fn run(
        &mut self,
        render_context: &mut RenderContext<'_, '_>,
        range: Range<usize>,
        color_attachment: RenderPassColorAttachment<'_>,
    ) {
        let mut init_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("outline_flood_init"),
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        if let Some(viewport) = self.camera.viewport.as_ref() {
            init_pass.set_camera_viewport(viewport);
        }

        if let Err(err) =
            self.flood_phase
                .render_range(&mut init_pass, self.world, self.view_entity, range)
        {
            error!("Error encountered while rendering the outline flood init phase {err:?}");
        }
    }
}
