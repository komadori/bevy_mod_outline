use bevy::asset::load_internal_asset;
use bevy::core_pipeline::Transparent3d;
use bevy::pbr::{
    DrawMesh, MeshPipeline, MeshPipelineKey, MeshUniform, SetMeshBindGroup, SetMeshViewBindGroup,
};
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::mesh::{MeshVertexBufferLayout, PrimitiveTopology};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::{AddRenderCommand, DrawFunctions, RenderPhase, SetItemPipeline};
use bevy::render::render_resource::{
    Face, PipelineCache, RenderPipelineDescriptor, SpecializedMeshPipeline,
    SpecializedMeshPipelineError, SpecializedMeshPipelines,
};
use bevy::render::view::ExtractedView;
use bevy::render::{RenderApp, RenderStage};

pub const OUTLINE_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 2101625026478770097);

#[derive(Clone, Component)]
pub struct Outline {
    pub colour: Color,
    pub offset: f32,
}

pub struct OutlinePlugin;

fn extract_outline(
    mut commands: Commands,
    mut previous_len: Local<usize>,
    mut query: Query<(Entity, &Outline)>,
) {
    let mut values = Vec::with_capacity(*previous_len);
    for (entity, outline) in query.iter_mut() {
        values.push((entity, (outline.clone(),)));
    }
    *previous_len = values.len();
    commands.insert_or_spawn_batch(values);
}

impl Plugin for OutlinePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            OUTLINE_SHADER_HANDLE,
            "outline.wgsl",
            Shader::from_wgsl
        );
        app.sub_app_mut(RenderApp)
            .add_render_command::<Transparent3d, DrawOutline>()
            .init_resource::<OutlinePipeline>()
            .init_resource::<SpecializedMeshPipelines<OutlinePipeline>>()
            .add_system_to_stage(RenderStage::Extract, extract_outline)
            .add_system_to_stage(RenderStage::Queue, queue_outline);
    }
}

type DrawOutline = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    DrawMesh,
);

#[allow(clippy::too_many_arguments)]
fn queue_outline(
    transparent_3d_draw_functions: Res<DrawFunctions<Transparent3d>>,
    outline_pipeline: Res<OutlinePipeline>,
    msaa: Res<Msaa>,
    mut pipelines: ResMut<SpecializedMeshPipelines<OutlinePipeline>>,
    mut pipeline_cache: ResMut<PipelineCache>,
    render_meshes: Res<RenderAssets<Mesh>>,
    material_meshes: Query<(Entity, &MeshUniform, &Handle<Mesh>, &Outline)>,
    mut views: Query<(&ExtractedView, &mut RenderPhase<Transparent3d>)>,
) {
    let draw_outline = transparent_3d_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();

    let key = MeshPipelineKey::from_msaa_samples(msaa.samples)
        | MeshPipelineKey::from_primitive_topology(PrimitiveTopology::TriangleList);

    for (view, mut transparent_phase) in views.iter_mut() {
        let view_matrix = view.transform.compute_matrix();
        let view_row_2 = view_matrix.row(2);
        for (entity, mesh_uniform, mesh_handle, _) in material_meshes.iter() {
            if let Some(mesh) = render_meshes.get(mesh_handle) {
                let pipeline = pipelines
                    .specialize(&mut pipeline_cache, &outline_pipeline, key, &mesh.layout)
                    .unwrap();
                transparent_phase.add(Transparent3d {
                    entity,
                    pipeline,
                    draw_function: draw_outline,
                    distance: view_row_2.dot(mesh_uniform.transform.col(3)),
                });
            }
        }
    }
}

struct OutlinePipeline {
    mesh_pipeline: MeshPipeline,
}

impl FromWorld for OutlinePipeline {
    fn from_world(world: &mut World) -> Self {
        let world = world.cell();
        let mesh_pipeline = world.get_resource::<MeshPipeline>().unwrap().clone();
        OutlinePipeline { mesh_pipeline }
    }
}

impl SpecializedMeshPipeline for OutlinePipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayout,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;
        descriptor.primitive.cull_mode = Some(Face::Front);
        //descriptor.depth_stencil.as_mut().unwrap().depth_write_enabled = false;
        //descriptor.depth_stencil.as_mut().unwrap().depth_compare = CompareFunction::Always;
        descriptor.vertex.shader = OUTLINE_SHADER_HANDLE.typed();
        descriptor.fragment.as_mut().unwrap().shader = OUTLINE_SHADER_HANDLE.typed();
        descriptor.layout = Some(vec![
            self.mesh_pipeline.view_layout.clone(),
            self.mesh_pipeline.mesh_layout.clone(),
        ]);
        Ok(descriptor)
    }
}
