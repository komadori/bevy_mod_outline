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

use crate::{pipeline::OutlinePipeline, ComputedOutlineDepth, Outline, OutlineStencil};

macro_rules! outline_vertex_uniform {
    ($x:ident) => {
        #[derive(Clone, Component, ShaderType)]
        pub struct $x {
            #[align(16)]
            pub origin: Vec3,
            pub offset: f32,
        }
    };
}

outline_vertex_uniform!(OutlineStencilUniform);
outline_vertex_uniform!(OutlineVolumeUniform);

#[derive(Clone, Component, ShaderType)]
pub struct OutlineFragmentUniform {
    #[align(16)]
    pub colour: Vec4,
}

pub struct OutlineStencilBindGroup {
    pub bind_group: BindGroup,
}

pub struct OutlineVolumeBindGroup {
    pub bind_group: BindGroup,
}

pub fn extract_outline_stencil_uniforms(
    mut commands: Commands,
    query: Extract<Query<(Entity, &OutlineStencil, &ComputedOutlineDepth)>>,
) {
    for (entity, stencil, computed) in query.iter() {
        commands.get_or_spawn(entity).insert(OutlineStencilUniform {
            origin: computed.origin,
            offset: stencil.offset,
        });
    }
}

pub fn extract_outline_volume_uniforms(
    mut commands: Commands,
    query: Extract<Query<(Entity, &Outline, &ComputedOutlineDepth)>>,
) {
    for (entity, outline, computed) in query.iter() {
        if !outline.visible || outline.colour.a() == 0.0 {
            continue;
        }
        commands
            .get_or_spawn(entity)
            .insert(OutlineVolumeUniform {
                origin: computed.origin,
                offset: outline.width,
            })
            .insert(OutlineFragmentUniform {
                colour: outline.colour.as_linear_rgba_f32().into(),
            });
    }
}

pub fn queue_outline_stencil_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    outline_pipeline: Res<OutlinePipeline>,
    vertex: Res<ComponentUniforms<OutlineStencilUniform>>,
) {
    if let Some(vertex_binding) = vertex.binding() {
        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            entries: &[BindGroupEntry {
                binding: 0,
                resource: vertex_binding.clone(),
            }],
            label: Some("outline_stencil_bind_group"),
            layout: &outline_pipeline.outline_stencil_bind_group_layout,
        });
        commands.insert_resource(OutlineStencilBindGroup { bind_group });
    }
}

pub fn queue_outline_volume_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    outline_pipeline: Res<OutlinePipeline>,
    vertex: Res<ComponentUniforms<OutlineVolumeUniform>>,
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
            label: Some("outline_volume_bind_group"),
            layout: &outline_pipeline.outline_volume_bind_group_layout,
        });
        commands.insert_resource(OutlineVolumeBindGroup { bind_group });
    }
}

pub struct SetOutlineStencilBindGroup<const I: usize>();

impl<const I: usize> EntityRenderCommand for SetOutlineStencilBindGroup<I> {
    type Param = (
        SRes<OutlineStencilBindGroup>,
        SQuery<Read<DynamicUniformIndex<OutlineStencilUniform>>>,
    );
    fn render<'w>(
        _view: Entity,
        item: Entity,
        (bind_group, query): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let vertex = query.get(item).unwrap();
        pass.set_bind_group(I, &bind_group.into_inner().bind_group, &[vertex.index()]);
        RenderCommandResult::Success
    }
}

pub struct SetOutlineVolumeBindGroup<const I: usize>();

impl<const I: usize> EntityRenderCommand for SetOutlineVolumeBindGroup<I> {
    type Param = (
        SRes<OutlineVolumeBindGroup>,
        SQuery<(
            Read<DynamicUniformIndex<OutlineVolumeUniform>>,
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
