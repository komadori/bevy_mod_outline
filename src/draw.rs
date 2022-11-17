use bevy::pbr::{DrawMesh, MeshPipelineKey, MeshUniform, SetMeshBindGroup, SetMeshViewBindGroup};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::{DrawFunctions, RenderPhase, SetItemPipeline};
use bevy::render::render_resource::{PipelineCache, SpecializedMeshPipelines};
use bevy::render::view::ExtractedView;

use crate::node::{OpaqueOutline, StencilOutline, TransparentOutline};
use crate::pipeline::{OutlinePipeline, PassType};
use crate::uniforms::{OutlineFragmentUniform, SetOutlineBindGroup, SetOutlineStencilBindGroup};
use crate::view_uniforms::SetOutlineViewBindGroup;
use crate::OutlineStencil;

pub type DrawStencil = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetOutlineStencilBindGroup<2>,
    DrawMesh,
);

#[allow(clippy::too_many_arguments)]
pub fn queue_outline_stencil_mesh(
    stencil_draw_functions: Res<DrawFunctions<StencilOutline>>,
    stencil_pipeline: Res<OutlinePipeline>,
    msaa: Res<Msaa>,
    mut pipelines: ResMut<SpecializedMeshPipelines<OutlinePipeline>>,
    mut pipeline_cache: ResMut<PipelineCache>,
    render_meshes: Res<RenderAssets<Mesh>>,
    material_meshes: Query<(Entity, &MeshUniform, &Handle<Mesh>), With<OutlineStencil>>,
    mut views: Query<(&ExtractedView, &mut RenderPhase<StencilOutline>)>,
) {
    let draw_stencil = stencil_draw_functions
        .read()
        .get_id::<DrawStencil>()
        .unwrap();

    let base_key = MeshPipelineKey::from_msaa_samples(msaa.samples);

    for (view, mut stencil_phase) in views.iter_mut() {
        let rangefinder = view.rangefinder3d();
        for (entity, mesh_uniform, mesh_handle) in material_meshes.iter() {
            if let Some(mesh) = render_meshes.get(mesh_handle) {
                let key =
                    base_key | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology);
                let pipeline = pipelines
                    .specialize(
                        &mut pipeline_cache,
                        &stencil_pipeline,
                        (key, PassType::Stencil),
                        &mesh.layout,
                    )
                    .unwrap();
                let distance = rangefinder.distance(&mesh_uniform.transform);
                stencil_phase.add(StencilOutline {
                    entity,
                    pipeline,
                    draw_function: draw_stencil,
                    distance,
                });
            }
        }
    }
}

pub type DrawOutline = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetOutlineViewBindGroup<2>,
    SetOutlineBindGroup<3>,
    DrawMesh,
);

#[allow(clippy::too_many_arguments)]
pub fn queue_outline_mesh(
    opaque_draw_functions: Res<DrawFunctions<OpaqueOutline>>,
    transparent_draw_functions: Res<DrawFunctions<TransparentOutline>>,
    outline_pipeline: Res<OutlinePipeline>,
    msaa: Res<Msaa>,
    mut pipelines: ResMut<SpecializedMeshPipelines<OutlinePipeline>>,
    mut pipeline_cache: ResMut<PipelineCache>,
    render_meshes: Res<RenderAssets<Mesh>>,
    material_meshes: Query<(Entity, &MeshUniform, &Handle<Mesh>, &OutlineFragmentUniform)>,
    mut views: Query<(
        &ExtractedView,
        &mut RenderPhase<OpaqueOutline>,
        &mut RenderPhase<TransparentOutline>,
    )>,
) {
    let draw_opaque_outline = opaque_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();
    let draw_transparent_outline = transparent_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();

    let base_key = MeshPipelineKey::from_msaa_samples(msaa.samples);

    for (view, mut opaque_phase, mut transparent_phase) in views.iter_mut() {
        let rangefinder = view.rangefinder3d();
        for (entity, mesh_uniform, mesh_handle, outline_fragment) in material_meshes.iter() {
            if let Some(mesh) = render_meshes.get(mesh_handle) {
                let transparent = outline_fragment.colour[3] < 1.0;
                let pass_type;
                let key = base_key
                    | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology)
                    | if transparent {
                        pass_type = PassType::Transparent;
                        MeshPipelineKey::TRANSPARENT_MAIN_PASS
                    } else {
                        pass_type = PassType::Opaque;
                        MeshPipelineKey::NONE
                    };
                let pipeline = pipelines
                    .specialize(
                        &mut pipeline_cache,
                        &outline_pipeline,
                        (key, pass_type),
                        &mesh.layout,
                    )
                    .unwrap();
                let distance = rangefinder.distance(&mesh_uniform.transform);
                if transparent {
                    transparent_phase.add(TransparentOutline {
                        entity,
                        pipeline,
                        draw_function: draw_transparent_outline,
                        distance,
                    });
                } else {
                    opaque_phase.add(OpaqueOutline {
                        entity,
                        pipeline,
                        draw_function: draw_opaque_outline,
                        distance,
                    });
                }
            }
        }
    }
}
