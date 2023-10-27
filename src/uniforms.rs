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
        render_resource::{BindGroup, BindGroupDescriptor, BindGroupEntry, ShaderType},
        renderer::RenderDevice,
        Extract,
    },
};

use crate::{
    node::StencilOutline, pipeline::OutlinePipeline, ComputedOutlineDepth, OutlineStencil,
    OutlineVolume,
};

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

#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) enum DepthMode {
    #[default]
    Invalid = 0,
    Flat = 1,
    Real = 2,
}

impl DepthMode {
    pub fn is_valid(&self) -> bool {
        *self != DepthMode::Invalid
    }
}

#[derive(Component)]
pub(crate) struct ExtractedOutline {
    pub depth_mode: DepthMode,
    pub transform: Affine3A,
    pub mesh_id: AssetId<Mesh>,
}

#[derive(Resource)]
pub(crate) struct OutlineStencilBindGroup {
    pub bind_group: BindGroup,
}

#[derive(Resource)]
pub(crate) struct OutlineVolumeBindGroup {
    pub bind_group: BindGroup,
}

pub(crate) fn set_outline_visibility(
    mut query: Query<&mut ViewVisibility, With<ComputedOutlineDepth>>,
) {
    for mut visibility in query.iter_mut() {
        visibility.set();
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn extract_outline(
    mut commands: Commands,
    query: Extract<
        Query<(
            Entity,
            &ComputedOutlineDepth,
            &GlobalTransform,
            &Handle<Mesh>,
        )>,
    >,
) {
    for (entity, computed, transform, mesh) in query.iter() {
        commands.get_or_spawn(entity).insert(ExtractedOutline {
            depth_mode: computed.depth_mode,
            transform: transform.affine(),
            mesh_id: mesh.id(),
        });
    }
}

pub(crate) fn extract_outline_stencil_uniforms(
    mut commands: Commands,
    query: Extract<Query<(Entity, &OutlineStencil, &ComputedOutlineDepth)>>,
) {
    for (entity, stencil, computed) in query.iter() {
        if !stencil.enabled {
            continue;
        }
        commands.get_or_spawn(entity).insert(OutlineStencilUniform {
            origin: computed.world_origin,
            offset: stencil.offset,
        });
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn extract_outline_volume_uniforms(
    mut commands: Commands,
    query: Extract<Query<(Entity, &OutlineVolume, &ComputedOutlineDepth)>>,
) {
    for (entity, outline, computed) in query.iter() {
        if !outline.visible || outline.colour.a() == 0.0 {
            continue;
        }
        commands
            .get_or_spawn(entity)
            .insert(OutlineVolumeUniform {
                origin: computed.world_origin,
                offset: outline.width,
            })
            .insert(OutlineFragmentUniform {
                colour: outline.colour.as_linear_rgba_f32().into(),
            });
    }
}

pub(crate) fn prepare_outline_stencil_bind_group(
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

pub(crate) fn prepare_outline_volume_bind_group(
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

pub(crate) struct SetOutlineStencilBindGroup<const I: usize>();

impl<const I: usize> RenderCommand<StencilOutline> for SetOutlineStencilBindGroup<I> {
    type ViewWorldQuery = ();
    type ItemWorldQuery = Read<DynamicUniformIndex<OutlineStencilUniform>>;
    type Param = SRes<OutlineStencilBindGroup>;
    fn render<'w>(
        _item: &StencilOutline,
        _view_data: (),
        entity_data: &DynamicUniformIndex<OutlineStencilUniform>,
        bind_group: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.set_bind_group(
            I,
            &bind_group.into_inner().bind_group,
            &[entity_data.index()],
        );
        RenderCommandResult::Success
    }
}

pub(crate) struct SetOutlineVolumeBindGroup<const I: usize>();

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetOutlineVolumeBindGroup<I> {
    type ViewWorldQuery = ();
    type ItemWorldQuery = (
        Read<DynamicUniformIndex<OutlineVolumeUniform>>,
        Read<DynamicUniformIndex<OutlineFragmentUniform>>,
    );
    type Param = SRes<OutlineVolumeBindGroup>;
    fn render<'w>(
        _item: &P,
        _view_data: (),
        entity_data: (
            &DynamicUniformIndex<OutlineVolumeUniform>,
            &DynamicUniformIndex<OutlineFragmentUniform>,
        ),
        bind_group: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let (vertex, fragment) = entity_data;
        pass.set_bind_group(
            I,
            &bind_group.into_inner().bind_group,
            &[vertex.index(), fragment.index()],
        );
        RenderCommandResult::Success
    }
}
