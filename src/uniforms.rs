use bevy::{
    ecs::system::{
        lifetimeless::{Read, SQuery, SRes},
        SystemParamItem,
    },
    prelude::*,
    render::{
        extract_component::{ComponentUniforms, DynamicUniformIndex},
        render_phase::{EntityRenderCommand, RenderCommandResult, TrackedRenderPass},
        render_resource::{BindGroup, BindGroupDescriptor, BindGroupEntry, ShaderType},
        renderer::RenderDevice,
        Extract,
    },
};

use crate::{pipeline::OutlinePipeline, Outline};

#[derive(Clone, Component, ShaderType)]
pub struct OutlineVertexUniform {
    #[align(16)]
    pub width: f32,
}

#[derive(Clone, Component, ShaderType)]
pub struct OutlineFragmentUniform {
    #[align(16)]
    pub colour: Vec4,
}

pub struct OutlineBindGroup {
    pub bind_group: BindGroup,
}

pub fn extract_outline_uniforms(mut commands: Commands, query: Extract<Query<(Entity, &Outline)>>) {
    for (entity, outline) in query.iter() {
        if !outline.visible || outline.colour.a() == 0.0 {
            continue;
        }
        commands
            .get_or_spawn(entity)
            .insert(OutlineVertexUniform {
                width: outline.width,
            })
            .insert(OutlineFragmentUniform {
                colour: outline.colour.as_linear_rgba_f32().into(),
            });
    }
}

pub fn queue_outline_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    outline_pipeline: Res<OutlinePipeline>,
    vertex: Res<ComponentUniforms<OutlineVertexUniform>>,
    fragment: Res<ComponentUniforms<OutlineFragmentUniform>>,
) {
    if let (Some(vertex_binding), Some(fragment_binding)) = (vertex.binding(), fragment.binding()) {
        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: vertex_binding.clone(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: fragment_binding.clone(),
                },
            ],
            label: Some("outline_bind_group"),
            layout: &outline_pipeline.outline_bind_group_layout,
        });
        commands.insert_resource(OutlineBindGroup { bind_group });
    }
}

pub struct SetOutlineBindGroup<const I: usize>();

impl<const I: usize> EntityRenderCommand for SetOutlineBindGroup<I> {
    type Param = (
        SRes<OutlineBindGroup>,
        SQuery<(
            Read<DynamicUniformIndex<OutlineVertexUniform>>,
            Read<DynamicUniformIndex<OutlineFragmentUniform>>,
        )>,
    );
    fn render<'w>(
        _view: Entity,
        item: Entity,
        (bind_group, query): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let (vertex, fragment) = query.get(item).unwrap();
        pass.set_bind_group(
            I,
            &bind_group.into_inner().bind_group,
            &[vertex.index(), fragment.index()],
        );
        RenderCommandResult::Success
    }
}
