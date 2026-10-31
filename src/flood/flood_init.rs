use std::ops::Range;

use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::mesh::allocator::MeshAllocator;
use bevy::render::render_phase::{DrawFunctions, PhaseItemExtraIndex, ViewSortedRenderPhases};
use bevy::render::view::{ExtractedView, RetainedViewEntity};
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
    mut live_views: Local<HashSet<RetainedViewEntity>>,
) {
    live_views.clear();

    for view in query.iter() {
        live_views.insert(view.retained_view_entity);
        flood_phases.prepare_for_new_frame(view.retained_view_entity);
    }

    flood_phases.retain(|view, _| live_views.contains(view));
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

#[cfg(test)]
mod tests {
    use bevy::ecs::system::{IntoSystem, System};
    use bevy::render::render_phase::{DrawFunctionId, PhaseItemExtraIndex};
    use bevy::render::render_resource::{CachedRenderPipelineId, TextureFormat};
    use bevy::render::sync_world::MainEntity;
    use bevy::render::view::ColorGrading;

    use crate::node::OutlineSortingInfo;

    use super::*;

    fn view_with_outline_uniform(entity: Entity) -> (ExtractedView, OutlineViewUniform) {
        (
            ExtractedView {
                retained_view_entity: RetainedViewEntity::new(entity.into(), None, 0),
                clip_from_view: Mat4::IDENTITY,
                world_from_view: GlobalTransform::default(),
                clip_from_world: None,
                target_format: TextureFormat::Rgba8UnormSrgb,
                viewport: UVec4::new(0, 0, 800, 600),
                color_grading: ColorGrading::default(),
                invert_culling: false,
            },
            OutlineViewUniform {
                clip_from_world: Mat4::IDENTITY,
                world_from_view_a: [Vec4::ZERO; 2],
                world_from_view_b: 0.0,
                aspect: 1.0,
                scale_clip_from_logical: Vec2::ONE,
                scale_physical_from_logical: 1.0,
            },
        )
    }

    fn queue_a_flood_outline(world: &mut World, view: &RetainedViewEntity) {
        let mut phases = world.resource_mut::<ViewSortedRenderPhases<FloodOutline>>();
        let phase = phases
            .get_mut(view)
            .expect("view should have a flood phase");
        phase.add_retained(FloodOutline {
            sorting_info: OutlineSortingInfo {
                world_plane_origin: Vec3::ZERO,
                world_plane_offset: Vec3::ZERO,
            },
            distance: 0.0,
            entity: Entity::PLACEHOLDER,
            main_entity: MainEntity::from(Entity::PLACEHOLDER),
            pipeline: CachedRenderPipelineId::INVALID,
            draw_function: DrawFunctionId(0),
            batch_range: 0..0,
            extra_index: PhaseItemExtraIndex::None,
            indexed: false,
            volume_offset: 0.0,
            volume_colour: Vec4::ONE,
        });
        assert!(
            !phase.items.is_empty(),
            "queued outline should be in the phase"
        );
    }

    fn spawn_view(world: &mut World) -> RetainedViewEntity {
        let entity = world.spawn_empty().id();
        world
            .entity_mut(entity)
            .insert(view_with_outline_uniform(entity));
        RetainedViewEntity::new(entity.into(), None, 0)
    }

    #[test]
    fn test_switching_between_views_leaves_no_queued_flood_work_behind() {
        let mut world = World::new();
        world.init_resource::<ViewSortedRenderPhases<FloodOutline>>();
        let mut prepare = IntoSystem::into_system(prepare_flood_phases);
        prepare.initialize(&mut world);

        let mut previous: Option<RetainedViewEntity> = None;
        for switch in 0..2 {
            let view = spawn_view(&mut world);
            if let Some(previous) = &previous {
                world
                    .entity_mut(previous.main_entity.id())
                    .remove::<OutlineViewUniform>();
            }

            prepare.run((), &mut world).unwrap();

            if let Some(previous) = &previous {
                assert!(
                    world
                        .resource::<ViewSortedRenderPhases<FloodOutline>>()
                        .get(previous)
                        .is_none(),
                    "switched-away view still holds queued outlines after switch {switch}"
                );
            }
            assert_eq!(
                world
                    .resource::<ViewSortedRenderPhases<FloodOutline>>()
                    .len(),
                1,
                "only the live view should hold a flood phase after switch {switch}"
            );

            queue_a_flood_outline(&mut world, &view);
            prepare.run((), &mut world).unwrap();
            assert!(
                !world
                    .resource::<ViewSortedRenderPhases<FloodOutline>>()
                    .get(&view)
                    .expect("live view must keep its phase")
                    .items
                    .is_empty(),
                "live view lost its queued outlines at switch {switch}"
            );

            previous = Some(view);
        }
    }
}
