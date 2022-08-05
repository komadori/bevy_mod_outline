use bevy::asset::load_internal_asset;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{
    ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin,
};
use bevy::render::render_graph::RenderGraph;
use bevy::render::render_phase::{sort_phase_system, AddRenderCommand, DrawFunctions};
use bevy::render::render_resource::SpecializedMeshPipelines;
use bevy::render::{RenderApp, RenderStage};

use crate::draw::{queue_outline_mesh, queue_outline_stencil_mesh, DrawOutline, DrawStencil};
use crate::node::{OpaqueOutline, OutlineNode, StencilOutline, TransparentOutline};
use crate::pipeline::{
    OutlinePipeline, COMMON_SHADER_HANDLE, OUTLINE_SHADER_HANDLE, STENCIL_SHADER_HANDLE,
};
use crate::uniforms::{queue_outline_bind_group, OutlineFragmentUniform, OutlineVertexUniform};
use crate::view_uniforms::{
    extract_outline_view_uniforms, queue_outline_view_bind_group, OutlineViewUniform,
};

mod draw;
mod node;
mod pipeline;
mod uniforms;
mod view_uniforms;

// See https://alexanderameye.github.io/notes/rendering-outlines/

/// A component for stenciling meshes during outline rendering
#[derive(Component, Default)]
pub struct OutlineStencil;

impl ExtractComponent for OutlineStencil {
    type Query = ();
    type Filter = With<OutlineStencil>;

    fn extract_component(_item: QueryItem<Self::Query>) -> Self {
        OutlineStencil
    }
}

/// A component for rendering outlines around meshes.
#[derive(Clone, Component)]
pub struct Outline {
    /// Width of the outline in logical pixels
    pub width: f32,
    /// Colour of the outline
    pub colour: Color,
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
            .add_plugin(ExtractComponentPlugin::<OutlineVertexUniform>::default())
            .add_plugin(ExtractComponentPlugin::<OutlineFragmentUniform>::default())
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
        draw_3d_graph.add_node(OutlineNode::NAME, node);
        draw_3d_graph
            .add_node_edge(
                bevy::core_pipeline::core_3d::graph::node::MAIN_PASS,
                OutlineNode::NAME,
            )
            .unwrap();
        draw_3d_graph
            .add_slot_edge(
                draw_3d_graph.input_node().unwrap().id,
                bevy::core_pipeline::core_3d::graph::input::VIEW_ENTITY,
                OutlineNode::NAME,
                OutlineNode::IN_VIEW,
            )
            .unwrap();
    }
}
