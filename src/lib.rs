//! This crate provides a Bevy plugin, [`OutlinePlugin`], and associated components for
//! rendering outlines around meshes using the vertex extrusion method.
//!
//! Outlines are rendered in a seperate pass following the main 3D pass. The effect of this
//! pass is to present the outlines in depth sorted order according to the model translation
//! of each mesh. This ensures that outlines are not clipped by non-outline geometry.
//!
//! The [`OutlineVolume`] component will, by itself, cover the original object entirely with
//! the outline colour. The [`OutlineStencil`] component must also be added to prevent the body
//! of an object from being filled it. This must be added to any entity which needs to appear on
//! top of an outline.
//!
//! The [`OutlineMode`] component specifies the rendering method. Outlines may be flattened into
//! a plane in order to further avoid clipping, or left in real space.
//!
//! The [`OutlineBundle`] and [`OutlineStencilBundle`] bundles can be used to add the right
//! components, including the required [`ComputedOutline`] component. Outlines can be inherited
//! from the parent via the [`InheritOutline`] component and [`InheritOutlineBundle`].
//!
//! Vertex extrusion works best with meshes that have smooth surfaces. To avoid visual
//! artefacts when outlining meshes with hard edges, see the
//! [`OutlineMeshExt::generate_outline_normals`] function and the
//! [`AutoGenerateOutlineNormalsPlugin`].

use bevy::asset::load_internal_asset;
use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::prelude::*;
use bevy::render::batching::{batch_and_prepare_render_phase, write_batched_instance_buffer};
use bevy::render::extract_component::{
    ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin,
};
use bevy::render::mesh::MeshVertexAttribute;
use bevy::render::render_graph::{RenderGraph, RenderLabel};
use bevy::render::render_phase::{sort_phase_system, AddRenderCommand, DrawFunctions};
use bevy::render::render_resource::{SpecializedMeshPipelines, VertexFormat};
use bevy::render::view::{RenderLayers, VisibilitySystems};
use bevy::render::{Render, RenderApp, RenderSet};
use bevy::transform::TransformSystem;
use bevy::ui::graph::NodeUi;
use interpolation::Lerp;

use crate::draw::{
    queue_outline_stencil_mesh, queue_outline_volume_mesh, DrawOutline, DrawStencil,
};
use crate::node::{OpaqueOutline, OutlineNode, StencilOutline, TransparentOutline};
use crate::pipeline::{OutlinePipeline, FRAGMENT_SHADER_HANDLE, OUTLINE_SHADER_HANDLE};
use crate::uniforms::{
    extract_outline_uniforms, prepare_outline_stencil_bind_group,
    prepare_outline_volume_bind_group, set_outline_visibility, OutlineFragmentUniform,
    OutlineStencilUniform, OutlineVolumeUniform,
};
use crate::view_uniforms::{
    extract_outline_view_uniforms, prepare_outline_view_bind_group, OutlineViewUniform,
};

mod computed;
mod draw;
mod generate;
mod node;
mod pipeline;
mod uniforms;
mod view_uniforms;

pub use computed::*;
pub use generate::*;

// See https://alexanderameye.github.io/notes/rendering-outlines/

/// The direction to extrude the vertex when rendering the outline.
pub const ATTRIBUTE_OUTLINE_NORMAL: MeshVertexAttribute =
    MeshVertexAttribute::new("Outline_Normal", 1585570526, VertexFormat::Float32x3);

/// Name of the render graph node which draws the outlines.
///
/// This node runs after the main 3D passes and before the UI pass. The name can be used to
/// add additional constraints on node execution order with respect to other passes.
#[derive(Copy, Clone, Debug, RenderLabel, Hash, PartialEq, Eq)]
pub enum LabelsOutline {
    OutlinePass,
}

/// A component for stenciling meshes during outline rendering.
#[derive(Clone, Component)]
pub struct OutlineStencil {
    /// Enable rendering of the stencil
    pub enabled: bool,
    /// Offset of the stencil in logical pixels
    pub offset: f32,
}

impl Default for OutlineStencil {
    fn default() -> Self {
        OutlineStencil {
            enabled: true,
            offset: 0.0,
        }
    }
}

fn lerp_bool(this: bool, other: bool, scalar: f32) -> bool {
    if scalar <= 0.0 {
        this
    } else if scalar >= 1.0 {
        other
    } else {
        this | other
    }
}

impl Lerp for OutlineStencil {
    type Scalar = f32;

    fn lerp(&self, other: &Self, scalar: &Self::Scalar) -> Self {
        OutlineStencil {
            enabled: lerp_bool(self.enabled, other.enabled, *scalar),
            offset: self.offset.lerp(other.offset, *scalar),
        }
    }
}

#[cfg(feature = "interpolation_03")]
impl interpolation_03::Lerp for OutlineStencil {
    type Scalar = f32;

    fn lerp(&self, other: &Self, scalar: &Self::Scalar) -> Self {
        <Self as Lerp>::lerp(self, other, scalar)
    }
}

/// A component for rendering outlines around meshes.
#[derive(Clone, Component, Default)]
pub struct OutlineVolume {
    /// Enable rendering of the outline
    pub visible: bool,
    /// Width of the outline in logical pixels
    pub width: f32,
    /// Colour of the outline
    pub colour: Color,
}

impl Lerp for OutlineVolume {
    type Scalar = f32;

    fn lerp(&self, other: &Self, scalar: &Self::Scalar) -> Self {
        OutlineVolume {
            visible: lerp_bool(self.visible, other.visible, *scalar),
            width: self.width.lerp(other.width, *scalar),
            colour: {
                let [r, g, b, a] = self
                    .colour
                    .as_linear_rgba_f32()
                    .lerp(&other.colour.as_linear_rgba_f32(), scalar);
                Color::rgba_linear(r, g, b, a)
            },
        }
    }
}

#[cfg(feature = "interpolation_03")]
impl interpolation_03::Lerp for OutlineVolume {
    type Scalar = f32;

    fn lerp(&self, other: &Self, scalar: &Self::Scalar) -> Self {
        <Self as Lerp>::lerp(self, other, scalar)
    }
}

/// A component for specifying what layer(s) the outline should be rendered for.
#[derive(Component, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Deref, DerefMut, Default)]
pub struct OutlineRenderLayers(pub RenderLayers);

impl ExtractComponent for OutlineRenderLayers {
    type QueryData = (
        Option<&'static OutlineRenderLayers>,
        Option<&'static RenderLayers>,
    );
    type QueryFilter = With<ComputedOutline>;
    type Out = Self;

    fn extract_component(
        (outline_mask, object_mask): (Option<&OutlineRenderLayers>, Option<&RenderLayers>),
    ) -> Option<Self> {
        Some(
            outline_mask
                .copied()
                .or_else(|| object_mask.copied().map(OutlineRenderLayers))
                .unwrap_or_default(),
        )
    }
}

/// A component which specifies how the outline should be rendered.
#[derive(Clone, Component)]
#[non_exhaustive]
pub enum OutlineMode {
    /// Vertex extrusion flattened into a plane facing the camera and intersecting the specified
    /// point in model-space.
    FlatVertex { model_origin: Vec3 },
    /// Vertex extrusion in real model-space.
    RealVertex,
}

impl Default for OutlineMode {
    fn default() -> Self {
        OutlineMode::FlatVertex {
            model_origin: Vec3::ZERO,
        }
    }
}

/// A component for inheriting outlines from the parent entity.
#[derive(Clone, Component, Default)]
pub struct InheritOutline;

/// A bundle for rendering stenciled outlines around meshes.
#[derive(Bundle, Clone, Default)]
pub struct OutlineBundle {
    pub outline: OutlineVolume,
    pub stencil: OutlineStencil,
    pub mode: OutlineMode,
    pub computed: ComputedOutline,
}

/// A bundle for stenciling meshes in the outlining pass.
#[derive(Bundle, Clone, Default)]
pub struct OutlineStencilBundle {
    pub stencil: OutlineStencil,
    pub mode: OutlineMode,
    pub computed: ComputedOutline,
}

/// A bundle for inheriting outlines from the parent entity.
#[derive(Bundle, Clone, Default)]
pub struct InheritOutlineBundle {
    pub inherit: InheritOutline,
    pub computed: ComputedOutline,
}

/// Adds support for rendering outlines.
pub struct OutlinePlugin;

impl Plugin for OutlinePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            OUTLINE_SHADER_HANDLE,
            "outline.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            FRAGMENT_SHADER_HANDLE,
            "fragment.wgsl",
            Shader::from_wgsl
        );

        app.add_plugins((
            ExtractComponentPlugin::<OutlineRenderLayers>::default(),
            UniformComponentPlugin::<OutlineStencilUniform>::default(),
            UniformComponentPlugin::<OutlineVolumeUniform>::default(),
            UniformComponentPlugin::<OutlineFragmentUniform>::default(),
            UniformComponentPlugin::<OutlineViewUniform>::default(),
        ))
        .add_systems(
            PostUpdate,
            (
                compute_outline
                    .after(TransformSystem::TransformPropagate)
                    .after(VisibilitySystems::VisibilityPropagate),
                set_outline_visibility.in_set(VisibilitySystems::CheckVisibility),
            ),
        )
        .sub_app_mut(RenderApp)
        .init_resource::<DrawFunctions<StencilOutline>>()
        .init_resource::<DrawFunctions<OpaqueOutline>>()
        .init_resource::<DrawFunctions<TransparentOutline>>()
        .init_resource::<SpecializedMeshPipelines<OutlinePipeline>>()
        .add_render_command::<StencilOutline, DrawStencil>()
        .add_render_command::<OpaqueOutline, DrawOutline>()
        .add_render_command::<TransparentOutline, DrawOutline>()
        .add_systems(
            ExtractSchedule,
            (extract_outline_uniforms, extract_outline_view_uniforms),
        )
        .add_systems(
            Render,
            (
                prepare_outline_view_bind_group,
                prepare_outline_stencil_bind_group,
                prepare_outline_volume_bind_group,
            )
                .in_set(RenderSet::PrepareBindGroups),
        )
        .add_systems(
            Render,
            (queue_outline_stencil_mesh, queue_outline_volume_mesh).in_set(RenderSet::QueueMeshes),
        )
        .add_systems(
            Render,
            (
                sort_phase_system::<StencilOutline>,
                sort_phase_system::<OpaqueOutline>,
                sort_phase_system::<TransparentOutline>,
            )
                .in_set(RenderSet::PhaseSort),
        )
        .add_systems(
            Render,
            (
                batch_and_prepare_render_phase::<StencilOutline, OutlinePipeline>,
                batch_and_prepare_render_phase::<OpaqueOutline, OutlinePipeline>,
                batch_and_prepare_render_phase::<TransparentOutline, OutlinePipeline>,
            )
                .in_set(RenderSet::PrepareResources),
        )
        .add_systems(
            Render,
            write_batched_instance_buffer::<OutlinePipeline>
                .in_set(RenderSet::PrepareResourcesFlush),
        );

        let world = &mut app.sub_app_mut(RenderApp).world;
        let node = OutlineNode::new(world);

        let mut graph = world.resource_mut::<RenderGraph>();

        let draw_3d_graph = graph.get_sub_graph_mut(Core3d).unwrap();
        draw_3d_graph.add_node(LabelsOutline::OutlinePass, node);

        // Run after main 3D pass, but before UI psss
        draw_3d_graph.add_node_edge(Node3d::EndMainPass, LabelsOutline::OutlinePass);
        #[cfg(feature = "bevy_ui")]
        draw_3d_graph.add_node_edge(LabelsOutline::OutlinePass, NodeUi::UiPass);
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<OutlinePipeline>();
    }
}
