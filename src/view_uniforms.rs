use bevy::math::Affine3A;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::render::batching::gpu_preprocessing::GpuPreprocessingMode;
use bevy::render::extract_component::ComponentUniforms;
use bevy::render::render_phase::{ViewBinnedRenderPhases, ViewSortedRenderPhases};
use bevy::render::render_resource::{BindGroup, BindGroupEntry};
use bevy::render::render_resource::{PipelineCache, ShaderType};
use bevy::render::renderer::RenderDevice;
use bevy::render::sync_world::RenderEntity;
use bevy::render::view::RetainedViewEntity;
use bevy::render::Extract;

use crate::node::{OpaqueOutline, StencilOutline, TransparentOutline};
use crate::pipeline::OutlinePipeline;
use crate::util::live_viewport;

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
    mut live_views: Local<HashSet<RetainedViewEntity>>,
    query: Extract<Query<(Entity, &RenderEntity, &Camera, &GlobalTransform), With<Camera3d>>>,
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
        if let Some(viewport_info) = live_viewport(camera) {
            let view_from_world = transform.to_matrix().inverse();
            let (world_from_view_a, world_from_view_b) = transpose_3x3(&transform.affine());
            commands.entity(entity.id()).insert((
                OutlineViewUniform {
                    clip_from_world: camera.clip_from_view() * view_from_world,
                    world_from_view_a,
                    world_from_view_b,
                    aspect: viewport_info.logical_size.x / viewport_info.logical_size.y,
                    scale_clip_from_logical: 2.0 / viewport_info.logical_size,
                    scale_physical_from_logical: viewport_info.scaling_factor,
                },
                OutlineQueueStatus::default(),
            ));

            let retained_view_entity = RetainedViewEntity::new(main_entity.into(), None, 0);
            stencil_phases.prepare_for_new_frame(retained_view_entity, GpuPreprocessingMode::None);
            opaque_phases.prepare_for_new_frame(retained_view_entity, GpuPreprocessingMode::None);
            transparent_phases.prepare_for_new_frame(retained_view_entity);
            live_views.insert(retained_view_entity);
        } else {
            commands
                .entity(entity.id())
                .remove::<(OutlineViewUniform, OutlineQueueStatus)>();
        }
    }
    stencil_phases.retain(|view_entity, _| live_views.contains(view_entity));
    opaque_phases.retain(|view_entity, _| live_views.contains(view_entity));
    transparent_phases.retain(|view_entity, _| live_views.contains(view_entity));
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
    use bevy::camera::RenderTargetInfo;
    use bevy::render::MainWorld;

    use super::*;

    /// Builds a render app with a single camera in the main world, returning
    /// the camera's main and render entities.
    fn setup() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.init_resource::<ViewBinnedRenderPhases<StencilOutline>>()
            .init_resource::<ViewBinnedRenderPhases<OpaqueOutline>>()
            .init_resource::<ViewSortedRenderPhases<TransparentOutline>>()
            .insert_resource(MainWorld::default())
            .add_systems(Update, extract_outline_view_uniforms);
        let render_entity = app.world_mut().spawn_empty().id();
        let main_entity = app
            .world_mut()
            .resource_mut::<MainWorld>()
            .spawn((
                Camera3d::default(),
                GlobalTransform::default(),
                RenderEntity::from(render_entity),
            ))
            .id();
        set_camera(&mut app, main_entity, true, UVec2::new(800, 600));
        (app, main_entity, render_entity)
    }

    fn set_camera(app: &mut App, main_entity: Entity, is_active: bool, size: UVec2) {
        let mut main_world = app.world_mut().resource_mut::<MainWorld>();
        let mut camera = main_world.get_mut::<Camera>(main_entity).unwrap();
        camera.is_active = is_active;
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: size,
            scale_factor: 1.0,
        });
    }

    fn is_live(app: &App, render_entity: Entity) -> bool {
        let world = app.world();
        let has_uniform = world.get::<OutlineViewUniform>(render_entity).is_some();
        let phase_counts = [
            world
                .resource::<ViewBinnedRenderPhases<StencilOutline>>()
                .len(),
            world
                .resource::<ViewBinnedRenderPhases<OpaqueOutline>>()
                .len(),
            world
                .resource::<ViewSortedRenderPhases<TransparentOutline>>()
                .len(),
        ];
        assert_eq!(
            phase_counts, [has_uniform as usize; 3],
            "Render phases should exist only while the view uniform does"
        );
        has_uniform
    }

    #[test]
    fn test_enable_and_disable_camera() {
        let (mut app, main_entity, render_entity) = setup();
        app.update();
        assert!(is_live(&app, render_entity), "Active camera should be live");

        // Deactivating the camera should release its per-view state.
        set_camera(&mut app, main_entity, false, UVec2::new(800, 600));
        app.update();
        assert!(
            !is_live(&app, render_entity),
            "Inactive camera should not be live"
        );

        // Reactivating the camera should restore it.
        set_camera(&mut app, main_entity, true, UVec2::new(800, 600));
        app.update();
        assert!(
            is_live(&app, render_entity),
            "Reactivated camera should be live"
        );

        // A camera with a collapsed viewport (e.g. a minimised window) is
        // effectively disabled even though it is still active.
        set_camera(&mut app, main_entity, true, UVec2::new(800, 0));
        app.update();
        assert!(
            !is_live(&app, render_entity),
            "Camera with a zero-size viewport should not be live"
        );
    }
}
