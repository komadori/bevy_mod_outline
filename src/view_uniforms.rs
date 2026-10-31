use bevy::math::Affine3A;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::render::batching::gpu_preprocessing::GpuPreprocessingMode;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex};
use bevy::render::render_phase::{ViewBinnedRenderPhases, ViewSortedRenderPhases};
use bevy::render::render_resource::{BindGroup, BindGroupEntry};
use bevy::render::render_resource::{PipelineCache, ShaderType};
use bevy::render::renderer::RenderDevice;
use bevy::render::sync_world::RenderEntity;
use bevy::render::view::RetainedViewEntity;
use bevy::render::Extract;

use crate::culling::live_viewport;
use crate::node::{OpaqueOutline, StencilOutline, TransparentOutline};
use crate::pipeline::OutlinePipeline;

#[derive(Clone, Component, ShaderType)]
pub(crate) struct OutlineViewUniform {
    pub clip_from_world: Mat4,
    pub world_from_view_a: [Vec4; 2],
    pub world_from_view_b: f32,
    pub aspect: f32,
    pub scale_clip_from_logical: Vec2,
    pub scale_physical_from_logical: f32,
}

#[derive(Resource)]
pub(crate) struct OutlineViewBindGroup {
    pub(crate) bind_group: BindGroup,
}

#[derive(Component, Default)]
pub(crate) struct OutlineQueueStatus {
    pub(crate) has_volume: bool,
}

#[allow(clippy::type_complexity)]
pub(crate) fn extract_outline_view_uniforms(
    mut commands: Commands,
    mut stencil_phases: ResMut<ViewBinnedRenderPhases<StencilOutline>>,
    mut opaque_phases: ResMut<ViewBinnedRenderPhases<OpaqueOutline>>,
    mut transparent_phases: ResMut<ViewSortedRenderPhases<TransparentOutline>>,
    query: Extract<Query<(Entity, &RenderEntity, &Camera, &GlobalTransform), With<Camera3d>>>,
    mut live_views: Local<HashSet<RetainedViewEntity>>,
) {
    fn transpose_3x3(m: &Affine3A) -> ([Vec4; 2], f32) {
        let transpose_3x3 = m.matrix3.transpose();
        (
            [
                (transpose_3x3.x_axis, transpose_3x3.y_axis.x).into(),
                (transpose_3x3.y_axis.yz(), transpose_3x3.z_axis.xy()).into(),
            ],
            transpose_3x3.z_axis.z,
        )
    }

    live_views.clear();

    for (main_entity, entity, camera, transform) in query.iter() {
        if let Some(size) = live_viewport(camera).and(camera.logical_viewport_size()) {
            let view_from_world = transform.to_matrix().inverse();
            let (world_from_view_a, world_from_view_b) = transpose_3x3(&transform.affine());
            commands
                .entity(entity.id())
                .insert(OutlineViewUniform {
                    clip_from_world: camera.clip_from_view() * view_from_world,
                    world_from_view_a,
                    world_from_view_b,
                    aspect: size.x / size.y,
                    scale_clip_from_logical: 2.0 / size,
                    scale_physical_from_logical: camera.target_scaling_factor().unwrap_or(1.0),
                })
                .insert(OutlineQueueStatus::default());

            let retained_view_entity = RetainedViewEntity::new(main_entity.into(), None, 0);
            live_views.insert(retained_view_entity);
            stencil_phases.prepare_for_new_frame(retained_view_entity, GpuPreprocessingMode::None);
            opaque_phases.prepare_for_new_frame(retained_view_entity, GpuPreprocessingMode::None);
            transparent_phases.prepare_for_new_frame(retained_view_entity);
        } else {
            commands.entity(entity.id()).remove::<(
                OutlineViewUniform,
                OutlineQueueStatus,
                DynamicUniformIndex<OutlineViewUniform>,
            )>();
        }
    }

    stencil_phases.retain(|view, _| live_views.contains(view));
    opaque_phases.retain(|view, _| live_views.contains(view));
    transparent_phases.retain(|view, _| live_views.contains(view));
}

pub(crate) fn prepare_outline_view_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    outline_pipeline: Res<OutlinePipeline>,
    pipeline_cache: Res<PipelineCache>,
    view_uniforms: Res<ComponentUniforms<OutlineViewUniform>>,
) {
    if let Some(view_binding) = view_uniforms.binding() {
        let bind_group = render_device.create_bind_group(
            "outline_view_bind_group",
            &pipeline_cache.get_bind_group_layout(&outline_pipeline.outline_view_bind_group_layout),
            &[BindGroupEntry {
                binding: 0,
                resource: view_binding.clone(),
            }],
        );
        commands.insert_resource(OutlineViewBindGroup { bind_group });
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetId;
    use bevy::camera::RenderTargetInfo;
    use bevy::ecs::system::{IntoSystem, System};
    use bevy::render::mesh::allocator::MeshSlabId;
    use bevy::render::render_phase::{BinnedRenderPhaseType, DrawFunctionId, InputUniformIndex};
    use bevy::render::render_resource::CachedRenderPipelineId;
    use bevy::render::sync_world::MainEntity;
    use bevy::render::MainWorld;

    use crate::node::{OutlineBatchSetKey, OutlineBinKey};

    use super::*;

    fn camera_filling_a_target() -> Camera {
        let mut camera = Camera::default();
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: UVec2::new(800, 600),
            scale_factor: 1.0,
        });
        camera
    }

    fn phase_counts(render_world: &World) -> [usize; 3] {
        [
            render_world
                .resource::<ViewBinnedRenderPhases<StencilOutline>>()
                .len(),
            render_world
                .resource::<ViewBinnedRenderPhases<OpaqueOutline>>()
                .len(),
            render_world
                .resource::<ViewSortedRenderPhases<TransparentOutline>>()
                .len(),
        ]
    }

    fn queue_an_outline(render_world: &mut World, view: &RetainedViewEntity) {
        let mut phases = render_world.resource_mut::<ViewBinnedRenderPhases<StencilOutline>>();
        let phase = phases
            .get_mut(view)
            .expect("view should have a stencil phase");
        phase.add(
            OutlineBatchSetKey {
                pipeline: CachedRenderPipelineId::INVALID,
                draw_function: DrawFunctionId(0),
                vertex_slab: MeshSlabId::default(),
                index_slab: None,
            },
            OutlineBinKey {
                asset_id: AssetId::default(),
                texture_id: None,
            },
            (Entity::PLACEHOLDER, MainEntity::from(Entity::PLACEHOLDER)),
            InputUniformIndex::default(),
            BinnedRenderPhaseType::UnbatchableMesh,
        );
        assert!(!phase.is_empty(), "queued outline should be in the phase");
    }

    fn spawn_camera(render_world: &mut World, camera: Camera) -> (Entity, RetainedViewEntity) {
        let render_entity = render_world.spawn_empty().id();
        let mut main_world = render_world.resource_mut::<MainWorld>();
        let main_entity = main_world
            .spawn((
                Camera3d::default(),
                camera,
                GlobalTransform::default(),
                RenderEntity::from(render_entity),
            ))
            .id();
        (
            render_entity,
            RetainedViewEntity::new(main_entity.into(), None, 0),
        )
    }

    fn setup() -> World {
        let mut render_world = World::new();
        render_world.init_resource::<ViewBinnedRenderPhases<StencilOutline>>();
        render_world.init_resource::<ViewBinnedRenderPhases<OpaqueOutline>>();
        render_world.init_resource::<ViewSortedRenderPhases<TransparentOutline>>();
        render_world.insert_resource(MainWorld::default());
        render_world
    }

    #[test]
    fn test_switching_between_cameras_leaves_no_queued_work_behind() {
        let mut render_world = setup();
        let mut extract = IntoSystem::into_system(extract_outline_view_uniforms);
        extract.initialize(&mut render_world);

        let mut previous: Option<(Entity, RetainedViewEntity)> = None;
        for switch in 0..2 {
            let (render_entity, view) = spawn_camera(&mut render_world, camera_filling_a_target());
            if let Some((_, previous_view)) = &previous {
                let previous_main = previous_view.main_entity.id();
                render_world
                    .resource_mut::<MainWorld>()
                    .get_mut::<Camera>(previous_main)
                    .expect("previous camera should still exist")
                    .is_active = false;
            }

            extract.run((), &mut render_world).unwrap();

            assert_eq!(
                phase_counts(&render_world),
                [1; 3],
                "only the live camera should hold phases after switch {switch}"
            );
            if let Some((previous_render, previous_view)) = &previous {
                assert!(
                    render_world
                        .resource::<ViewBinnedRenderPhases<StencilOutline>>()
                        .get(previous_view)
                        .is_none(),
                    "switched-away camera still holds queued outlines after switch {switch}"
                );
                assert!(
                    render_world
                        .get::<OutlineViewUniform>(*previous_render)
                        .is_none(),
                    "switched-away camera kept its view uniform after switch {switch}"
                );
                assert!(
                    render_world
                        .get::<OutlineQueueStatus>(*previous_render)
                        .is_none(),
                    "switched-away camera kept its queue status after switch {switch}"
                );
            }

            queue_an_outline(&mut render_world, &view);
            extract.run((), &mut render_world).unwrap();
            assert!(
                !render_world
                    .resource::<ViewBinnedRenderPhases<StencilOutline>>()
                    .get(&view)
                    .expect("live camera must keep its phase")
                    .is_empty(),
                "live camera lost its queued outlines at switch {switch}"
            );

            previous = Some((render_entity, view));
        }
    }

    #[test]
    fn test_a_camera_with_a_collapsed_viewport_is_dropped() {
        let mut render_world = setup();
        let mut extract = IntoSystem::into_system(extract_outline_view_uniforms);
        extract.initialize(&mut render_world);

        let (render_entity, view) = spawn_camera(&mut render_world, camera_filling_a_target());
        extract.run((), &mut render_world).unwrap();

        render_world
            .resource_mut::<MainWorld>()
            .get_mut::<Camera>(view.main_entity.id())
            .expect("camera should still exist")
            .computed
            .target_info = Some(RenderTargetInfo {
            physical_size: UVec2::new(800, 0),
            scale_factor: 1.0,
        });
        extract.run((), &mut render_world).unwrap();

        assert_eq!(
            phase_counts(&render_world),
            [0; 3],
            "a camera whose viewport has collapsed is not drawn, so it must not keep phases"
        );
        assert!(
            render_world
                .get::<OutlineViewUniform>(render_entity)
                .is_none(),
            "a camera whose viewport has collapsed kept its view uniform"
        );
    }
}
