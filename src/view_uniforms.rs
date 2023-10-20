use bevy::ecs::system::lifetimeless::{Read, SRes};
use bevy::ecs::system::SystemParamItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex};
use bevy::render::render_phase::{
    PhaseItem, RenderCommand, RenderCommandResult, RenderPhase, TrackedRenderPass,
};
use bevy::render::render_resource::ShaderType;
use bevy::render::render_resource::{BindGroup, BindGroupDescriptor, BindGroupEntry};
use bevy::render::renderer::RenderDevice;
use bevy::render::view::RenderLayers;
use bevy::render::Extract;

use crate::node::{OpaqueOutline, StencilOutline, TransparentOutline};
use crate::pipeline::OutlinePipeline;

#[derive(Clone, Component, ShaderType)]
pub(crate) struct OutlineViewUniform {
    #[align(16)]
    scale: Vec2,
}

#[derive(Resource)]
pub(crate) struct OutlineViewBindGroup {
    bind_group: BindGroup,
}

#[allow(clippy::type_complexity)]
pub(crate) fn extract_outline_view_uniforms(
    mut commands: Commands,
    query: Extract<Query<(Entity, &Camera, Option<&RenderLayers>), With<Camera3d>>>,
) {
    for (entity, camera, view_mask) in query.iter() {
        if !camera.is_active {
            continue;
        }
        if let Some(size) = camera.logical_viewport_size() {
            let mut entity_commands = commands.get_or_spawn(entity);
            entity_commands
                .insert(OutlineViewUniform { scale: 2.0 / size })
                .insert(RenderPhase::<StencilOutline>::default())
                .insert(RenderPhase::<OpaqueOutline>::default())
                .insert(RenderPhase::<TransparentOutline>::default());

            if let Some(view_mask) = view_mask {
                entity_commands.insert(*view_mask);
            }
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
        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            entries: &[BindGroupEntry {
                binding: 0,
                resource: view_binding.clone(),
            }],
            label: Some("outline_view_bind_group"),
            layout: &outline_pipeline.outline_view_bind_group_layout,
        });
        commands.insert_resource(OutlineViewBindGroup { bind_group });
    }
}

pub(crate) struct SetOutlineViewBindGroup<const I: usize>();

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetOutlineViewBindGroup<I> {
    type ViewWorldQuery = Read<DynamicUniformIndex<OutlineViewUniform>>;
    type ItemWorldQuery = ();
    type Param = SRes<OutlineViewBindGroup>;
    fn render<'w>(
        _item: &P,
        view_data: &DynamicUniformIndex<OutlineViewUniform>,
        _entity_data: (),
        bind_group: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.set_bind_group(I, &bind_group.into_inner().bind_group, &[view_data.index()]);
        RenderCommandResult::Success
    }
}
