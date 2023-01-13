//! This crate provides a Bevy plugin, [`OutlinePlugin`], and associated components for
//! rendering outlines around meshes using the vertex extrusion method.
//!
//! Outlines are rendered in a seperate pass following the main 3D pass. The effect of this
//! pass is to present the outlines in depth sorted order according to the model translation
//! of each mesh. This ensures that outlines are not clipped by other geometry.
//!
//! The [`Outline`] component will, by itself, cover the original object entirely with the
//! outline colour. The [`OutlineStencil`] component must also be added to prevent the body of
//! an object from being filled it. This must be added to any entity which needs to appear on
//! top of an outline.
//!
//! Vertex extrusion works best with meshes that have smooth surfaces. To avoid visual
//! artefacts when outlining meshes with hard edges, see the
//! [`OutlineMeshExt::generate_outline_normals`] function and the
//! [`AutoGenerateOutlineNormalsPlugin`].

use bevy::asset::load_internal_asset;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{
    ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin,
};
use bevy::render::mesh::MeshVertexAttribute;
use bevy::render::render_graph::RenderGraph;
use bevy::render::render_phase::{sort_phase_system, AddRenderCommand, DrawFunctions};
use bevy::render::render_resource::{SpecializedMeshPipelines, VertexFormat};
use bevy::render::view::RenderLayers;
use bevy::render::{RenderApp, RenderStage};

use crate::draw::{queue_outline_mesh, queue_outline_stencil_mesh, DrawOutline, DrawStencil};
use crate::node::{OpaqueOutline, OutlineNode, StencilOutline, TransparentOutline};
use crate::pipeline::{
    OutlinePipeline, COMMON_SHADER_HANDLE, OUTLINE_SHADER_HANDLE, STENCIL_SHADER_HANDLE,
};
use crate::uniforms::{
    extract_outline_uniforms, queue_outline_bind_group, OutlineFragmentUniform,
    OutlineVertexUniform,
};
use crate::view_uniforms::{
    extract_outline_view_uniforms, queue_outline_view_bind_group, OutlineViewUniform,
};

mod draw;
mod generate;
mod node;
mod pipeline;
mod uniforms;
mod view_uniforms;

pub use generate::*;

// See https://alexanderameye.github.io/notes/rendering-outlines/

/// The direction to extrude the vertex when rendering the outline.
pub const ATTRIBUTE_OUTLINE_NORMAL: MeshVertexAttribute =
    MeshVertexAttribute::new("Outline_Normal", 1585570526, VertexFormat::Float32x3);

/// Name of the render graph node which draws the outlines.
///
/// This node runs after the main 3D passes and before the UI pass. The name can be used to
/// add additional constraints on node execution order with respect to other passes.
pub const OUTLINE_PASS_NODE_NAME: &str = "bevy_mod_outline_node";

/// A component for stenciling meshes during outline rendering.
#[derive(Clone, Component, Default)]
pub struct OutlineStencil;

impl ExtractComponent for OutlineStencil {
    type Query = ();
    type Filter = With<OutlineStencil>;

    fn extract_component(_item: QueryItem<Self::Query>) -> Self {
        OutlineStencil
    }
}

/// A component for rendering outlines around meshes.
#[derive(Clone, Component, Default)]
pub struct Outline {
    /// Enable rendering of the outline
    pub visible: bool,
    /// Width of the outline in logical pixels
    pub width: f32,
    /// Colour of the outline
    pub colour: Color,
}

/// A component for specifying what layer(s) the outline should be rendered for
#[derive(Component, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Deref, DerefMut, Default)]
pub struct OutlineRenderLayers(pub RenderLayers);

impl ExtractComponent for OutlineRenderLayers {
    type Query = &'static OutlineRenderLayers;
    type Filter = ();

    fn extract_component(item: &OutlineRenderLayers) -> Self {
        *item
    }
}

/// A bundle for rendering stenciled outlines around meshes.
#[derive(Bundle, Clone, Default)]
pub struct OutlineBundle {
    pub outline: Outline,
    pub stencil: OutlineStencil,
}

/// Adds support for rendering outlines.
pub struct OutlinePlugin;

impl Plugin for OutlinePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, COMMON_SHADER_HANDLE, "common.wgsl", Shader::from_wgsl);
        load_internal_asset!(
            app,
            STENCIL_SHADER_HANDLE,
            "stencil.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            OUTLINE_SHADER_HANDLE,
            "outline.wgsl",
            Shader::from_wgsl
        );

        app.add_plugin(ExtractComponentPlugin::<OutlineStencil>::extract_visible())
            .add_plugin(ExtractComponentPlugin::<OutlineRenderLayers>::default())
            .add_plugin(UniformComponentPlugin::<OutlineVertexUniform>::default())
            .add_plugin(UniformComponentPlugin::<OutlineFragmentUniform>::default())
            .add_plugin(UniformComponentPlugin::<OutlineViewUniform>::default())
            .sub_app_mut(RenderApp)
            .init_resource::<DrawFunctions<StencilOutline>>()
            .init_resource::<DrawFunctions<OpaqueOutline>>()
            .init_resource::<DrawFunctions<TransparentOutline>>()
            .init_resource::<OutlinePipeline>()
            .init_resource::<SpecializedMeshPipelines<OutlinePipeline>>()
            .add_render_command::<StencilOutline, DrawStencil>()
            .add_render_command::<OpaqueOutline, DrawOutline>()
            .add_render_command::<TransparentOutline, DrawOutline>()
            .add_system_to_stage(RenderStage::Extract, extract_outline_view_uniforms)
            .add_system_to_stage(RenderStage::Extract, extract_outline_uniforms)
            .add_system_to_stage(RenderStage::PhaseSort, sort_phase_system::<StencilOutline>)
            .add_system_to_stage(RenderStage::PhaseSort, sort_phase_system::<OpaqueOutline>)
            .add_system_to_stage(
                RenderStage::PhaseSort,
                sort_phase_system::<TransparentOutline>,
            )
            .add_system_to_stage(RenderStage::Queue, queue_outline_view_bind_group)
            .add_system_to_stage(RenderStage::Queue, queue_outline_bind_group)
            .add_system_to_stage(RenderStage::Queue, queue_outline_stencil_mesh)
            .add_system_to_stage(RenderStage::Queue, queue_outline_mesh);

        let world = &mut app.sub_app_mut(RenderApp).world;
        let node = OutlineNode::new(world);

        let mut graph = world.resource_mut::<RenderGraph>();

        let draw_3d_graph = graph
            .get_sub_graph_mut(bevy::core_pipeline::core_3d::graph::NAME)
            .unwrap();
        draw_3d_graph.add_node(OUTLINE_PASS_NODE_NAME, node);
        draw_3d_graph
            .add_slot_edge(
                draw_3d_graph.input_node().unwrap().id,
                bevy::core_pipeline::core_3d::graph::input::VIEW_ENTITY,
                OUTLINE_PASS_NODE_NAME,
                OutlineNode::IN_VIEW,
            )
            .unwrap();

        // Run after main 3D pass, but before UI psss
        draw_3d_graph
            .add_node_edge(
                bevy::core_pipeline::core_3d::graph::node::MAIN_PASS,
                OUTLINE_PASS_NODE_NAME,
            )
            .unwrap();
        #[cfg(feature = "bevy_ui")]
        draw_3d_graph
            .add_node_edge(
                OUTLINE_PASS_NODE_NAME,
                bevy::ui::draw_ui_graph::node::UI_PASS,
            )
            .unwrap();
    }
}
