use bevy::{
    ecs::system::{lifetimeless::SRes, SystemParamItem},
    math::Affine3,
    prelude::*,
    render::{
        batching::{no_gpu_preprocessing::BatchedInstanceBuffer, NoAutomaticBatching},
        render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass},
        render_resource::{BindGroup, BindGroupEntry, ShaderType},
        renderer::RenderDevice,
        Extract,
    },
};

use crate::{pipeline::OutlinePipeline, ComputedOutline};

#[derive(Component)]
pub(crate) struct ExtractedOutline {
    pub depth_mode: DepthMode,
    pub mesh_id: AssetId<Mesh>,
    pub automatic_batching: bool,
    pub instance_data: OutlineInstanceUniform,
}

#[derive(Clone, ShaderType)]
pub(crate) struct OutlineInstanceUniform {
    pub world_from_local: [Vec4; 3],
    pub origin_in_world: Vec3,
    pub volume_offset: f32,
    pub volume_colour: Vec4,
    pub stencil_offset: f32,
}

#[derive(Component)]
pub(crate) struct OutlineStencilEnabled;

#[derive(Component)]
pub(crate) struct OutlineVolumeEnabled;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DepthMode {
    Flat = 1,
    Real = 2,
}

#[derive(Resource)]
pub(crate) struct OutlineInstanceBindGroup {
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
    query: Extract<
        Query<(
            Entity,
            &ComputedOutline,
            &GlobalTransform,
            &Handle<Mesh>,
            Has<NoAutomaticBatching>,
        )>,
    >,
) {
    for (entity, computed, transform, mesh, no_automatic_batching) in query.iter() {
        let cmds = &mut commands.get_or_spawn(entity);
        if let ComputedOutline(Some(computed)) = computed {
            cmds.insert(ExtractedOutline {
                depth_mode: computed.mode.value.depth_mode,
                mesh_id: mesh.id(),
                automatic_batching: !no_automatic_batching,
                instance_data: OutlineInstanceUniform {
                    world_from_local: Affine3::from(&transform.affine()).to_transpose(),
                    origin_in_world: computed.mode.value.world_origin,
                    stencil_offset: computed.stencil.value.offset,
                    volume_offset: computed.volume.value.offset,
                    volume_colour: computed.volume.value.colour.to_vec4(),
                },
            });
            if computed.stencil.value.enabled {
                cmds.insert(OutlineStencilEnabled);
            }
            if computed.volume.value.enabled {
                cmds.insert(OutlineVolumeEnabled);
            }
        }
    }
}

pub(crate) fn prepare_outline_instance_bind_group(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    batched_instance_buffer: Res<BatchedInstanceBuffer<OutlineInstanceUniform>>,
    outline_pipeline: Res<OutlinePipeline>,
) {
    if let Some(instance_binding) = batched_instance_buffer.instance_data_binding() {
        let bind_group = render_device.create_bind_group(
            "outline_instance_mesh_bind_group",
            &outline_pipeline.outline_instance_bind_group_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: instance_binding,
            }],
        );
        commands.insert_resource(OutlineInstanceBindGroup { bind_group });
    };
}

pub(crate) struct SetOutlineInstanceBindGroup<const I: usize>();

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetOutlineInstanceBindGroup<I> {
    type ViewQuery = ();
    type ItemQuery = ();
    type Param = SRes<OutlineInstanceBindGroup>;
    fn render<'w>(
        item: &P,
        _view_data: (),
        _entity_data: Option<()>,
        bind_group: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let dynamic_uniform_index = item.extra_index().as_dynamic_offset().map(|x| x.get());
        pass.set_bind_group(
            I,
            &bind_group.into_inner().bind_group,
            dynamic_uniform_index.as_slice(),
        );
        RenderCommandResult::Success
    }
}
