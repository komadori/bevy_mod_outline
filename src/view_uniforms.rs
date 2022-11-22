use bevy::ecs::system::lifetimeless::{Read, SQuery, SRes};
use bevy::ecs::system::SystemParamItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex};
use bevy::render::render_phase::{
    EntityRenderCommand, RenderCommandResult, RenderPhase, TrackedRenderPass,
};
use bevy::render::render_resource::ShaderType;
use bevy::render::render_resource::{BindGroup, BindGroupDescriptor, BindGroupEntry};
use bevy::render::renderer::RenderDevice;
use bevy::render::Extract;

use crate::node::{OpaqueOutline, StencilOutline, TransparentOutline};
use crate::pipeline::OutlinePipeline;

#[derive(Clone, Component, ShaderType)]
pub struct OutlineViewUniform {
    #[align(16)]
    scale: Vec2,
}

#[derive(Resource)]
pub struct OutlineViewBindGroup {
    bind_group: BindGroup,
}

pub fn extract_outline_view_uniforms(
    mut commands: Commands,
    query: Extract<Query<(Entity, &Camera), With<Camera3d>>>,
) {
    for (entity, camera) in query.iter() {
        if !camera.is_active {
            continue;
        }
        if let Some(size) = camera.logical_viewport_size() {
            commands
                .get_or_spawn(entity)
                .insert(OutlineViewUniform { scale: 2.0 / size })
                .insert(RenderPhase::<StencilOutline>::default())
                .insert(RenderPhase::<OpaqueOutline>::default())
                .insert(RenderPhase::<TransparentOutline>::default());
        }
    }
}

pub fn queue_outline_view_bind_group(
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

pub struct SetOutlineViewBindGroup<const I: usize>();

impl<const I: usize> EntityRenderCommand for SetOutlineViewBindGroup<I> {
    type Param = (
        SRes<OutlineViewBindGroup>,
        SQuery<Read<DynamicUniformIndex<OutlineViewUniform>>>,
    );
    fn render<'w>(
        view: Entity,
        _item: Entity,
        (bind_group, query): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let view_index = query.get(view).unwrap();
        pass.set_bind_group(
            I,
            &bind_group.into_inner().bind_group,
            &[view_index.index()],
        );
        RenderCommandResult::Success
    }
}
