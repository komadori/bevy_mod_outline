use bevy::{
    ecs::{
        query::QueryItem,
        system::{
            lifetimeless::{Read, SQuery, SRes},
            SystemParamItem,
        },
    },
    prelude::*,
    render::{
        extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent},
        render_phase::{EntityRenderCommand, RenderCommandResult, TrackedRenderPass},
        render_resource::{BindGroup, BindGroupDescriptor, BindGroupEntry, ShaderType},
        renderer::RenderDevice,
    },
};

use crate::{pipeline::OutlinePipeline, Outline};

#[derive(Clone, Component, ShaderType)]
pub struct OutlineVertexUniform {
    pub width: f32,
}

impl ExtractComponent for OutlineVertexUniform {
    type Query = Read<Outline>;
    type Filter = ();

    fn extract_component(item: QueryItem<Self::Query>) -> Self {
        OutlineVertexUniform { width: item.width }
    }
}

#[derive(Clone, Component, ShaderType)]
pub struct OutlineFragmentUniform {
    pub colour: Vec4,
}

impl ExtractComponent for OutlineFragmentUniform {
    type Query = Read<Outline>;
    type Filter = ();

    fn extract_component(item: QueryItem<Self::Query>) -> Self {
        OutlineFragmentUniform {
            colour: item.colour.as_linear_rgba_f32().into(),
        }
    }
}

pub struct OutlineBindGroup {
    pub bind_group: BindGroup,
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
