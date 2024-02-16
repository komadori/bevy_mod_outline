use bevy::{
    ecs::system::{
        lifetimeless::{Read, SRes},
        SystemParamItem,
    },
    math::Affine3A,
    prelude::*,
    render::{
        extract_component::{ComponentUniforms, DynamicUniformIndex},
        render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass},
        render_resource::{BindGroup, BindGroupEntry, ShaderType},
        renderer::RenderDevice,
        Extract,
    },
};

use crate::{node::StencilOutline, pipeline::OutlinePipeline, ComputedOutline};

#[derive(Component)]
pub(crate) struct ExtractedOutline {
    pub depth_mode: DepthMode,
    pub transform: Affine3A,
    pub mesh_id: AssetId<Mesh>,
}

#[derive(Clone, Component, ShaderType)]
pub(crate) struct OutlineStencilUniform {
    #[align(16)]
    pub origin: Vec3,
    pub offset: f32,
}

#[derive(Clone, Component, ShaderType)]
pub(crate) struct OutlineVolumeUniform {
    #[align(16)]
    pub origin: Vec3,
    pub offset: f32,
}

#[derive(Clone, Component, ShaderType)]
pub(crate) struct OutlineFragmentUniform {
    #[align(16)]
    pub colour: Vec4,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DepthMode {
    Flat = 1,
    Real = 2,
}

#[derive(Resource)]
pub(crate) struct OutlineStencilBindGroup {
    pub bind_group: BindGroup,
}

#[derive(Resource)]
pub(crate) struct OutlineVolumeBindGroup {
    pub bind_group: BindGroup,
}

pub(crate) fn set_outline_visibility(mut query: Query<(&mut ViewVisibility, &ComputedOutline)>) {
    for (mut visibility, computed) in query.iter_mut() {
        if let ComputedOutline(Some(computed)) = computed {
            if computed.volume.value.enabled || computed.stencil.value.enabled {
                visibility.set();
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn extract_outline_uniforms(
    mut commands: Commands,
    query: Extract<Query<(Entity, &ComputedOutline, &GlobalTransform, &Handle<Mesh>)>>,
) {
    for (entity, computed, transform, mesh) in query.iter() {
        let cmds = &mut commands.get_or_spawn(entity);
        if let ComputedOutline(Some(computed)) = computed {
            cmds.insert(ExtractedOutline {
                depth_mode: computed.mode.value.depth_mode,
                transform: transform.affine(),
                mesh_id: mesh.id(),
            });
            if computed.volume.value.enabled {
                cmds.insert(OutlineVolumeUniform {
                    origin: computed.mode.value.world_origin,
                    offset: computed.volume.value.offset,
                })
                .insert(OutlineFragmentUniform {
                    colour: computed.volume.value.colour,
                });
            }
            if computed.stencil.value.enabled {
                cmds.insert(OutlineStencilUniform {
                    origin: computed.mode.value.world_origin,
                    offset: computed.stencil.value.offset,
                });
            }
        }
    }
}

pub(crate) fn prepare_outline_stencil_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    outline_pipeline: Res<OutlinePipeline>,
    vertex: Res<ComponentUniforms<OutlineStencilUniform>>,
) {
    if let Some(vertex_binding) = vertex.binding() {
        let bind_group = render_device.create_bind_group(
            Some("outline_stencil_bind_group"),
            &outline_pipeline.outline_stencil_bind_group_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: vertex_binding.clone(),
            }],
        );
        commands.insert_resource(OutlineStencilBindGroup { bind_group });
    }
}

pub(crate) fn prepare_outline_volume_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    outline_pipeline: Res<OutlinePipeline>,
    vertex: Res<ComponentUniforms<OutlineVolumeUniform>>,
    fragment: Res<ComponentUniforms<OutlineFragmentUniform>>,
) {
    if let (Some(vertex_binding), Some(fragment_binding)) = (vertex.binding(), fragment.binding()) {
        let bind_group = render_device.create_bind_group(
            "outline_volume_bind_group",
            &outline_pipeline.outline_volume_bind_group_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: vertex_binding.clone(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: fragment_binding.clone(),
                },
            ],
        );
        commands.insert_resource(OutlineVolumeBindGroup { bind_group });
    }
}

pub(crate) struct SetOutlineStencilBindGroup<const I: usize>();

impl<const I: usize> RenderCommand<StencilOutline> for SetOutlineStencilBindGroup<I> {
    type ViewQuery = ();
    type ItemQuery = Read<DynamicUniformIndex<OutlineStencilUniform>>;
    type Param = SRes<OutlineStencilBindGroup>;
    fn render<'w>(
        _item: &StencilOutline,
        _view_data: (),
        entity_data: Option<&DynamicUniformIndex<OutlineStencilUniform>>,
        bind_group: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(dyn_uniform) = entity_data else {
            return RenderCommandResult::Failure;
        };
        pass.set_bind_group(
            I,
            &bind_group.into_inner().bind_group,
            &[dyn_uniform.index()],
        );
        RenderCommandResult::Success
    }
}

pub(crate) struct SetOutlineVolumeBindGroup<const I: usize>();

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetOutlineVolumeBindGroup<I> {
    type ViewQuery = ();
    type ItemQuery = (
        Read<DynamicUniformIndex<OutlineVolumeUniform>>,
        Read<DynamicUniformIndex<OutlineFragmentUniform>>,
    );
    type Param = SRes<OutlineVolumeBindGroup>;
    fn render<'w>(
        _item: &P,
        _view_data: (),
        entity_data: Option<(
            &DynamicUniformIndex<OutlineVolumeUniform>,
            &DynamicUniformIndex<OutlineFragmentUniform>,
        )>,
        bind_group: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some((vertex, fragment)) = entity_data else {
            return RenderCommandResult::Failure;
        };
        pass.set_bind_group(
            I,
            &bind_group.into_inner().bind_group,
            &[vertex.index(), fragment.index()],
        );
        RenderCommandResult::Success
    }
}
