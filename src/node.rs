use std::cmp::Reverse;
use std::ops::Range;

use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::render_graph::{NodeRunError, ViewNode};
use bevy::render::render_phase::{
    CachedRenderPipelinePhaseItem, DrawFunctionId, PhaseItem, RenderPhase,
};
use bevy::render::render_resource::{
    CachedRenderPipelineId, Operations, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    StoreOp,
};
use bevy::render::view::{ViewDepthTexture, ViewTarget};
use bevy::render::{render_graph::RenderGraphContext, renderer::RenderContext};
use bevy::utils::nonmax::NonMaxU32;
use bevy::utils::FloatOrd;

pub(crate) struct StencilOutline {
    pub distance: f32,
    pub pipeline: CachedRenderPipelineId,
    pub entity: Entity,
    pub draw_function: DrawFunctionId,
    pub batch_range: Range<u32>,
    pub dynamic_offset: Option<NonMaxU32>,
}

impl PhaseItem for StencilOutline {
    type SortKey = Reverse<FloatOrd>;

    #[inline]
    fn entity(&self) -> Entity {
        self.entity
    }

    fn sort_key(&self) -> Self::SortKey {
        Reverse(FloatOrd(self.distance))
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

    fn dynamic_offset(&self) -> Option<bevy::utils::nonmax::NonMaxU32> {
        self.dynamic_offset
    }

    fn dynamic_offset_mut(&mut self) -> &mut Option<bevy::utils::nonmax::NonMaxU32> {
        &mut self.dynamic_offset
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
    pub dynamic_offset: Option<NonMaxU32>,
}

impl PhaseItem for OpaqueOutline {
    type SortKey = Reverse<FloatOrd>;

    #[inline]
    fn entity(&self) -> Entity {
        self.entity
    }

    fn sort_key(&self) -> Self::SortKey {
        Reverse(FloatOrd(self.distance))
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

    fn dynamic_offset(&self) -> Option<NonMaxU32> {
        self.dynamic_offset
    }

    fn dynamic_offset_mut(&mut self) -> &mut Option<NonMaxU32> {
        &mut self.dynamic_offset
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
    pub dynamic_offset: Option<NonMaxU32>,
}

impl PhaseItem for TransparentOutline {
    type SortKey = FloatOrd;

    #[inline]
    fn entity(&self) -> Entity {
        self.entity
    }

    fn sort_key(&self) -> Self::SortKey {
        FloatOrd(self.distance)
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

    fn dynamic_offset(&self) -> Option<NonMaxU32> {
        self.dynamic_offset
    }

    fn dynamic_offset_mut(&mut self) -> &mut Option<NonMaxU32> {
        &mut self.dynamic_offset
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
        &'static RenderPhase<StencilOutline>,
        &'static RenderPhase<OpaqueOutline>,
        &'static RenderPhase<TransparentOutline>,
        &'static Camera3d,
        &'static ViewTarget,
        &'static ViewDepthTexture,
    );

    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, stencil_phase, opaque_phase, transparent_phase, camera_3d, target, depth): QueryItem<'w, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let view_entity = graph.view_entity();

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
