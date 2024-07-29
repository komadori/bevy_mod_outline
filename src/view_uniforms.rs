use bevy::ecs::query::ROQueryItem;
use bevy::ecs::system::lifetimeless::{Read, SRes};
use bevy::ecs::system::SystemParamItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex};
use bevy::render::render_phase::{
    PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass, ViewSortedRenderPhases,
};
use bevy::render::render_resource::ShaderType;
use bevy::render::render_resource::{BindGroup, BindGroupEntry};
use bevy::render::renderer::RenderDevice;
use bevy::render::view::RenderLayers;
use bevy::render::Extract;

use crate::node::{OpaqueOutline, StencilOutline, TransparentOutline};
use crate::pipeline::OutlinePipeline;

#[derive(Clone, Component, ShaderType)]
pub(crate) struct OutlineViewUniform {
    #[align(16)]
    clip_from_world: Mat4,
    scale: Vec2,
}

#[derive(Resource)]
pub(crate) struct OutlineViewBindGroup {
    bind_group: BindGroup,
}

#[allow(clippy::type_complexity)]
pub(crate) fn extract_outline_view_uniforms(
    mut commands: Commands,
    mut stencil_phases: ResMut<ViewSortedRenderPhases<StencilOutline>>,
    mut opaque_phases: ResMut<ViewSortedRenderPhases<OpaqueOutline>>,
    mut transparent_phases: ResMut<ViewSortedRenderPhases<TransparentOutline>>,
    query: Extract<
        Query<(Entity, &Camera, &GlobalTransform, Option<&RenderLayers>), With<Camera3d>>,
    >,
) {
    for (entity, camera, transform, view_mask) in query.iter() {
        if !camera.is_active {
            continue;
        }
        if let Some(size) = camera.logical_viewport_size() {
            let view_from_world = transform.compute_matrix().inverse();
            let mut entity_commands = commands.get_or_spawn(entity);
            entity_commands.insert(OutlineViewUniform {
                clip_from_world: camera.clip_from_view() * view_from_world,
                scale: 2.0 / size,
            });

            if let Some(view_mask) = view_mask {
                entity_commands.insert(view_mask.clone());
            }

            stencil_phases.insert_or_clear(entity);
            opaque_phases.insert_or_clear(entity);
            transparent_phases.insert_or_clear(entity);
        }
    }
}

pub(crate) fn prepare_outline_view_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    outline_pipeline: Res<OutlinePipeline>,
    view_uniforms: Res<ComponentUniforms<OutlineViewUniform>>,
) {
    if let Some(view_binding) = view_uniforms.binding() {
        let bind_group = render_device.create_bind_group(
            "outline_view_bind_group",
            &outline_pipeline.outline_view_bind_group_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: view_binding.clone(),
            }],
        );
        commands.insert_resource(OutlineViewBindGroup { bind_group });
    }
}

pub(crate) struct SetOutlineViewBindGroup<const I: usize>();

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetOutlineViewBindGroup<I> {
    type ViewQuery = Read<DynamicUniformIndex<OutlineViewUniform>>;
    type ItemQuery = ();
    type Param = SRes<OutlineViewBindGroup>;
    fn render<'w>(
        _item: &P,
        view_data: ROQueryItem<'w, Self::ViewQuery>,
        _entity_data: Option<()>,
        bind_group: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.set_bind_group(I, &bind_group.into_inner().bind_group, &[view_data.index()]);
        RenderCommandResult::Success
    }
}
