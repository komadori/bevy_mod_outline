use bevy::{
    ecs::{
        query::ROQueryItem,
        system::{
            lifetimeless::{Read, SRes},
            SystemParamItem,
        },
    },
    pbr::{DrawMesh, SetMeshBindGroup},
    render::{
        extract_component::DynamicUniformIndex,
        render_phase::{
            PhaseItem, RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass,
        },
    },
};

use crate::{
    uniforms::OutlineInstanceBindGroup,
    view_uniforms::{OutlineViewBindGroup, OutlineViewUniform},
};

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
pub(crate) type DrawStencil = (
    SetItemPipeline,
    SetOutlineViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetOutlineInstanceBindGroup<2>,
    DrawMesh,
);

pub(crate) type DrawOutline = (
    SetItemPipeline,
    SetOutlineViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetOutlineInstanceBindGroup<2>,
    DrawMesh,
);
