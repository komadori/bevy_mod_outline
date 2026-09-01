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
