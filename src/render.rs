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
    uniforms::{AlphaMaskBindGroups, ExtractedOutline, OutlineInstanceBindGroup},
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

pub(crate) struct SetOutlineAlphaMaskBindGroup<const I: usize>();

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetOutlineAlphaMaskBindGroup<I> {
    type ViewQuery = ();
    type ItemQuery = &'static ExtractedOutline;
    type Param = SRes<AlphaMaskBindGroups>;
    fn render<'w>(
        _item: &P,
        _view_data: (),
        entity_data: Option<ROQueryItem<'w, Self::ItemQuery>>,
        bind_groups: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let bind_groups = bind_groups.into_inner();

        let bind_group =
            if let Some(texture_handle) = entity_data.and_then(|e| e.alpha_mask_id.as_ref()) {
                bind_groups
                    .bind_groups
                    .get(texture_handle)
                    .unwrap_or(&bind_groups.default_bind_group)
            } else {
                &bind_groups.default_bind_group
            };

        pass.set_bind_group(I, bind_group, &[]);
        RenderCommandResult::Success
    }
}

pub(crate) type DrawOutline = (
    SetItemPipeline,
    SetOutlineViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetOutlineInstanceBindGroup<2>,
    SetOutlineAlphaMaskBindGroup<3>,
    DrawMesh,
);
