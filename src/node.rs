use std::cmp::Reverse;
use std::ops::Range;

use bevy::ecs::query::QueryItem;
use bevy::math::FloatOrd;
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::render_graph::{NodeRunError, ViewNode};
use bevy::render::render_phase::{
    CachedRenderPipelinePhaseItem, DrawFunctionId, PhaseItem, PhaseItemExtraIndex, SortedPhaseItem,
    ViewSortedRenderPhases,
};
use bevy::render::render_resource::{
    CachedRenderPipelineId, Operations, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    StoreOp,
};
use bevy::render::view::{ViewDepthTexture, ViewTarget};
use bevy::render::{render_graph::RenderGraphContext, renderer::RenderContext};

pub(crate) struct StencilOutline {
    pub distance: f32,
    pub pipeline: CachedRenderPipelineId,
    pub entity: Entity,
    pub draw_function: DrawFunctionId,
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
}

impl PhaseItem for StencilOutline {
    #[inline]
    fn entity(&self) -> Entity {
        self.entity
    }

    fn draw_function(&self) -> bevy::render::render_phase::DrawFunctionId {
        self.draw_function
    }

    fn batch_range(&self) -> &std::ops::Range<u32> {
        &self.batch_range
    }

    fn batch_range_mut(&mut self) -> &mut std::ops::Range<u32> {
        &mut self.batch_range
    }

    fn extra_index(&self) -> bevy::render::render_phase::PhaseItemExtraIndex {
        self.extra_index
    }

    fn batch_range_and_extra_index_mut(
        &mut self,
    ) -> (
        &mut Range<u32>,
        &mut bevy::render::render_phase::PhaseItemExtraIndex,
    ) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl SortedPhaseItem for StencilOutline {
    type SortKey = Reverse<FloatOrd>;

    fn sort_key(&self) -> Self::SortKey {
        Reverse(FloatOrd(self.distance))
    }
}

impl CachedRenderPipelinePhaseItem for StencilOutline {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.pipeline
    }
}

pub(crate) struct OpaqueOutline {
    pub distance: f32,
    pub pipeline: CachedRenderPipelineId,
    pub entity: Entity,
    pub draw_function: DrawFunctionId,
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
}

impl PhaseItem for OpaqueOutline {
    #[inline]
    fn entity(&self) -> Entity {
        self.entity
    }

    fn draw_function(&self) -> bevy::render::render_phase::DrawFunctionId {
        self.draw_function
    }

    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    fn extra_index(&self) -> bevy::render::render_phase::PhaseItemExtraIndex {
        self.extra_index
    }

    fn batch_range_and_extra_index_mut(
        &mut self,
    ) -> (
        &mut Range<u32>,
        &mut bevy::render::render_phase::PhaseItemExtraIndex,
    ) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl SortedPhaseItem for OpaqueOutline {
    type SortKey = Reverse<FloatOrd>;

    fn sort_key(&self) -> Self::SortKey {
        Reverse(FloatOrd(self.distance))
    }
}

impl CachedRenderPipelinePhaseItem for OpaqueOutline {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.pipeline
    }
}

pub(crate) struct TransparentOutline {
    pub distance: f32,
    pub pipeline: CachedRenderPipelineId,
    pub entity: Entity,
    pub draw_function: DrawFunctionId,
    pub batch_range: Range<u32>,
    pub extra_index: PhaseItemExtraIndex,
}

impl PhaseItem for TransparentOutline {
    #[inline]
    fn entity(&self) -> Entity {
        self.entity
    }

    fn draw_function(&self) -> bevy::render::render_phase::DrawFunctionId {
        self.draw_function
    }

    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    fn extra_index(&self) -> PhaseItemExtraIndex {
        self.extra_index
    }

    fn batch_range_and_extra_index_mut(&mut self) -> (&mut Range<u32>, &mut PhaseItemExtraIndex) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl SortedPhaseItem for TransparentOutline {
    type SortKey = Reverse<FloatOrd>;

    fn sort_key(&self) -> Self::SortKey {
        Reverse(FloatOrd(self.distance))
    }
}

impl CachedRenderPipelinePhaseItem for TransparentOutline {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.pipeline
    }
}

pub(crate) struct OutlineNode;

impl FromWorld for OutlineNode {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

impl ViewNode for OutlineNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static Camera3d,
        &'static ViewTarget,
        &'static ViewDepthTexture,
    );

    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, camera_3d, target, depth): QueryItem<'w, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let view_entity = graph.view_entity();
        let (Some(stencil_phase), Some(opaque_phase), Some(transparent_phase)) = (
            world
                .get_resource::<ViewSortedRenderPhases<StencilOutline>>()
                .and_then(|ps| ps.get(&view_entity)),
            world
                .get_resource::<ViewSortedRenderPhases<OpaqueOutline>>()
                .and_then(|ps| ps.get(&view_entity)),
            world
                .get_resource::<ViewSortedRenderPhases<TransparentOutline>>()
                .and_then(|ps| ps.get(&view_entity)),
        ) else {
            return Ok(());
        };

        // If drawing anything, run stencil pass to clear the depth buffer
        if !opaque_phase.items.is_empty() || !transparent_phase.items.is_empty() {
            let pass_descriptor = RenderPassDescriptor {
                label: Some("outline_stencil_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: depth.view(),
                    depth_ops: Some(Operations {
                        load: camera_3d.depth_load_op.clone().into(),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            };
            let mut tracked_pass = render_context.begin_tracked_render_pass(pass_descriptor);
            if let Some(viewport) = camera.viewport.as_ref() {
                tracked_pass.set_camera_viewport(viewport);
            }
            stencil_phase.render(&mut tracked_pass, world, view_entity);
        }

        if !opaque_phase.items.is_empty() {
            let pass_descriptor = RenderPassDescriptor {
                label: Some("outline_opaque_pass"),
                color_attachments: &[Some(target.get_color_attachment())],
                depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
                timestamp_writes: None,
                occlusion_query_set: None,
            };
            let mut tracked_pass = render_context.begin_tracked_render_pass(pass_descriptor);
            if let Some(viewport) = camera.viewport.as_ref() {
                tracked_pass.set_camera_viewport(viewport);
            }
            opaque_phase.render(&mut tracked_pass, world, view_entity);
        }

        if !transparent_phase.items.is_empty() {
            let pass_descriptor = RenderPassDescriptor {
                label: Some("outline_transparent_pass"),
                color_attachments: &[Some(target.get_color_attachment())],
                depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
                timestamp_writes: None,
                occlusion_query_set: None,
            };
            let mut tracked_pass = render_context.begin_tracked_render_pass(pass_descriptor);
            if let Some(viewport) = camera.viewport.as_ref() {
                tracked_pass.set_camera_viewport(viewport);
            }
            transparent_phase.render(&mut tracked_pass, world, view_entity);
        }

        Ok(())
    }
}
