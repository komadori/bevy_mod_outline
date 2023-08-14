use std::cmp::Reverse;

use bevy::ecs::system::lifetimeless::Read;
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::render_graph::NodeRunError;
use bevy::render::render_phase::{
    CachedRenderPipelinePhaseItem, DrawFunctionId, PhaseItem, RenderPhase,
};
use bevy::render::render_resource::{
    CachedRenderPipelineId, LoadOp, Operations, RenderPassDepthStencilAttachment,
    RenderPassDescriptor,
};
use bevy::render::view::{ExtractedView, ViewDepthTexture, ViewTarget};
use bevy::render::{
    render_graph::{Node, RenderGraphContext},
    renderer::RenderContext,
};
use bevy::utils::FloatOrd;

pub(crate) struct StencilOutline {
    pub distance: f32,
    pub pipeline: CachedRenderPipelineId,
    pub entity: Entity,
    pub draw_function: DrawFunctionId,
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
}

impl CachedRenderPipelinePhaseItem for TransparentOutline {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.pipeline
    }
}

#[allow(clippy::type_complexity)]
pub(crate) struct OutlineNode {
    query: QueryState<
        (
            Read<ExtractedCamera>,
            Read<RenderPhase<StencilOutline>>,
            Read<RenderPhase<OpaqueOutline>>,
            Read<RenderPhase<TransparentOutline>>,
            Read<Camera3d>,
            Read<ViewTarget>,
            Read<ViewDepthTexture>,
        ),
        With<ExtractedView>,
    >,
}

impl OutlineNode {
    pub(crate) fn new(world: &mut World) -> Self {
        Self {
            query: world.query_filtered(),
        }
    }
}

impl Node for OutlineNode {
    fn update(&mut self, world: &mut World) {
        self.query.update_archetypes(world);
    }

    fn run(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let view_entity = graph.view_entity();
        let (camera, stencil_phase, opaque_phase, transparent_phase, camera_3d, target, depth) =
            match self.query.get_manual(world, view_entity) {
                Ok(query) => query,
                Err(_) => {
                    return Ok(());
                } // No window
            };

        // If drawing anything, run stencil pass to clear the depth buffer
        if !opaque_phase.items.is_empty() || !transparent_phase.items.is_empty() {
            let pass_descriptor = RenderPassDescriptor {
                label: Some("outline_stencil_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &depth.view,
                    depth_ops: Some(Operations {
                        load: camera_3d.depth_load_op.clone().into(),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
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
                color_attachments: &[Some(target.get_color_attachment(Operations {
                    load: LoadOp::Load,
                    store: true,
                }))],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &depth.view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Load,
                        store: true,
                    }),
                    stencil_ops: None,
                }),
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
                color_attachments: &[Some(target.get_color_attachment(Operations {
                    load: LoadOp::Load,
                    store: true,
                }))],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &depth.view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Load,
                        store: true,
                    }),
                    stencil_ops: None,
                }),
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
