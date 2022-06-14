use bevy::asset::load_internal_asset;
use bevy::core_pipeline::{Opaque3d, Transparent3d};
use bevy::ecs::system::lifetimeless::{Read, SQuery, SRes};
use bevy::ecs::system::SystemParamItem;
use bevy::pbr::{
    DrawMesh, MeshPipeline, MeshPipelineKey, MeshUniform, SetMeshBindGroup, SetMeshViewBindGroup,
};
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::camera::{ActiveCamera, Camera3d};
use bevy::render::mesh::{MeshVertexBufferLayout, PrimitiveTopology};
use bevy::render::render_asset::{RenderAsset, RenderAssetPlugin, RenderAssets};
use bevy::render::render_component::ExtractComponentPlugin;
use bevy::render::render_phase::{
    AddRenderCommand, DrawFunctions, EntityRenderCommand, RenderCommandResult, RenderPhase,
    SetItemPipeline, TrackedRenderPass,
};
use bevy::render::render_resource::std140::{AsStd140, Std140};
use bevy::render::render_resource::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferInitDescriptor, BufferSize,
    BufferUsages, DynamicUniformVec, Face, PipelineCache, RenderPipelineDescriptor, ShaderStages,
    SpecializedMeshPipeline, SpecializedMeshPipelineError, SpecializedMeshPipelines,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::view::ExtractedView;
use bevy::render::{RenderApp, RenderStage};
use libm::nextafterf;

// See https://alexanderameye.github.io/notes/rendering-outlines/

const OUTLINE_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 2101625026478770097);

/// An asset for rendering outlines around meshes.
#[derive(Clone, TypeUuid)]
#[uuid = "552e416b-2766-4e6a-9ee5-9ebd0e8c0230"]
pub struct Outline {
    /// Colour of the outline
    pub colour: Color,
    /// Width of the outline in logical pixels
    pub width: f32,
}

impl RenderAsset for Outline {
    type ExtractedAsset = Outline;

    type PreparedAsset = GpuOutline;

    type Param = (SRes<RenderDevice>, SRes<OutlinePipeline>);

    fn extract_asset(&self) -> Self::ExtractedAsset {
        self.clone()
    }

    fn prepare_asset(
        outline: Self::ExtractedAsset,
        (render_device, outline_pipeline): &mut bevy::ecs::system::SystemParamItem<Self::Param>,
    ) -> Result<
        Self::PreparedAsset,
        bevy::render::render_asset::PrepareAssetError<Self::ExtractedAsset>,
    > {
        let colour = outline.colour.as_linear_rgba_f32().into();
        let vbuffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("outline_vertex_stage_uniform_buffer"),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            contents: VertexStageData {
                width: outline.width,
            }
            .as_std140()
            .as_bytes(),
        });
        let fbuffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("outline_fragment_stage_uniform_buffer"),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            contents: FragmentStageData { colour }.as_std140().as_bytes(),
        });
        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: Some("outline_bind_group"),
            layout: &outline_pipeline.outline_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: vbuffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: fbuffer.as_entire_binding(),
                },
            ],
        });
        Ok(GpuOutline {
            _vertex_stage_buffer: vbuffer,
            _fragment_stage_buffer: fbuffer,
            bind_group,
            transparent: colour.w < 1.0,
        })
    }
}

#[derive(Clone, Component, AsStd140)]
struct ViewSizeUniform {
    logical_size: Vec2,
}

#[derive(Default)]
struct ViewSizeUniforms {
    pub uniforms: DynamicUniformVec<ViewSizeUniform>,
}

#[derive(Component)]
struct ViewSizeUniformOffset {
    pub offset: u32,
}

#[derive(Component)]
struct GpuViewSize {
    bind_group: BindGroup,
}

#[derive(Clone, AsStd140)]
struct VertexStageData {
    width: f32,
}

#[derive(Clone, AsStd140)]
struct FragmentStageData {
    colour: Vec4,
}

pub struct GpuOutline {
    _vertex_stage_buffer: Buffer,
    _fragment_stage_buffer: Buffer,
    bind_group: BindGroup,
    transparent: bool,
}

/// Adds support for the [Outline] asset type.
pub struct OutlinePlugin;

impl Plugin for OutlinePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            OUTLINE_SHADER_HANDLE,
            "outline.wgsl",
            Shader::from_wgsl
        );
        app.add_asset::<Outline>()
            .add_plugin(ExtractComponentPlugin::<Handle<Outline>>::default())
            .add_plugin(RenderAssetPlugin::<Outline>::default())
            .sub_app_mut(RenderApp)
            .add_render_command::<Opaque3d, DrawOutline>()
            .add_render_command::<Transparent3d, DrawOutline>()
            .init_resource::<OutlinePipeline>()
            .init_resource::<SpecializedMeshPipelines<OutlinePipeline>>()
            .init_resource::<ViewSizeUniforms>()
            .add_system_to_stage(RenderStage::Extract, extract_view_size_uniforms)
            .add_system_to_stage(RenderStage::Prepare, prepare_view_size_uniforms)
            .add_system_to_stage(RenderStage::Queue, queue_outline);
    }
}

type DrawOutline = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetOutlineViewBindGroup<2>,
    SetOutlineBindGroup<3>,
    DrawMesh,
);

struct SetOutlineViewBindGroup<const I: usize>();

impl<const I: usize> EntityRenderCommand for SetOutlineViewBindGroup<I> {
    type Param = SQuery<(Read<ViewSizeUniformOffset>, Read<GpuViewSize>)>;
    #[inline]
    fn render<'w>(
        view: Entity,
        _item: Entity,
        view_query: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let (view_size_uniform, gpu_view_size) = view_query.get_inner(view).unwrap();
        pass.set_bind_group(I, &gpu_view_size.bind_group, &[view_size_uniform.offset]);

        RenderCommandResult::Success
    }
}

struct SetOutlineBindGroup<const I: usize>();

impl<const I: usize> EntityRenderCommand for SetOutlineBindGroup<I> {
    type Param = (SRes<RenderAssets<Outline>>, SQuery<Read<Handle<Outline>>>);
    fn render<'w>(
        _view: Entity,
        item: Entity,
        (outlines, query): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let outline_handle = query.get(item).unwrap();
        let outline = outlines.into_inner().get(outline_handle).unwrap();
        pass.set_bind_group(I, &outline.bind_group, &[]);
        RenderCommandResult::Success
    }
}

fn extract_view_size_uniforms(
    mut commands: Commands,
    windows: Res<Windows>,
    images: Res<Assets<Image>>,
    active_camera: Res<ActiveCamera<Camera3d>>,
    query: Query<&Camera, With<Camera3d>>,
) {
    if let Some(entity) = active_camera.get() {
        if let Ok(camera) = query.get(entity) {
            if let Some(size) = camera.target.get_logical_size(&windows, &images) {
                commands
                    .get_or_spawn(entity)
                    .insert(ViewSizeUniform { logical_size: size });
            }
        }
    }
}

fn prepare_view_size_uniforms(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    outline_pipeline: Res<OutlinePipeline>,
    mut view_size_uniforms: ResMut<ViewSizeUniforms>,
    views: Query<(Entity, &ViewSizeUniform)>,
) {
    view_size_uniforms.uniforms.clear();
    for (entity, view_size_uniform) in views.iter() {
        let view_size_uniforms = ViewSizeUniformOffset {
            offset: view_size_uniforms.uniforms.push(view_size_uniform.clone()),
        };

        commands.entity(entity).insert(view_size_uniforms);
    }

    view_size_uniforms
        .uniforms
        .write_buffer(&render_device, &render_queue);

    if let Some(view_binding) = view_size_uniforms.uniforms.binding() {
        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            entries: &[BindGroupEntry {
                binding: 0,
                resource: view_binding.clone(),
            }],
            label: Some("outline_view_size_bind_group"),
            layout: &outline_pipeline.view_size_bind_group_layout,
        });
        for (entity, _) in views.iter() {
            commands.entity(entity).insert(GpuViewSize {
                bind_group: bind_group.clone(),
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn queue_outline(
    opaque_3d_draw_functions: Res<DrawFunctions<Opaque3d>>,
    transparent_3d_draw_functions: Res<DrawFunctions<Transparent3d>>,
    outline_pipeline: Res<OutlinePipeline>,
    msaa: Res<Msaa>,
    mut pipelines: ResMut<SpecializedMeshPipelines<OutlinePipeline>>,
    mut pipeline_cache: ResMut<PipelineCache>,
    render_meshes: Res<RenderAssets<Mesh>>,
    render_outlines: Res<RenderAssets<Outline>>,
    material_meshes: Query<(Entity, &MeshUniform, &Handle<Mesh>, &Handle<Outline>)>,
    mut views: Query<(
        &ExtractedView,
        &mut RenderPhase<Opaque3d>,
        &mut RenderPhase<Transparent3d>,
    )>,
) {
    let draw_opaque_outline = opaque_3d_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();
    let draw_transparent_outline = transparent_3d_draw_functions
        .read()
        .get_id::<DrawOutline>()
        .unwrap();

    let base_key = MeshPipelineKey::from_msaa_samples(msaa.samples)
        | MeshPipelineKey::from_primitive_topology(PrimitiveTopology::TriangleList);

    for (view, mut opaque_phase, mut transparent_phase) in views.iter_mut() {
        let inverse_view_matrix = view.transform.compute_matrix().inverse();
        let inverse_view_row_2 = inverse_view_matrix.row(2);
        for (entity, mesh_uniform, mesh_handle, outline_handle) in material_meshes.iter() {
            if let Some(mesh) = render_meshes.get(mesh_handle) {
                if let Some(outline) = render_outlines.get(outline_handle) {
                    let key = if outline.transparent {
                        base_key | MeshPipelineKey::TRANSPARENT_MAIN_PASS
                    } else {
                        base_key
                    };
                    let pipeline = pipelines
                        .specialize(&mut pipeline_cache, &outline_pipeline, key, &mesh.layout)
                        .unwrap();
                    let distance = nextafterf(
                        inverse_view_row_2.dot(mesh_uniform.transform.col(3)),
                        f32::NEG_INFINITY,
                    );
                    if outline.transparent {
                        transparent_phase.add(Transparent3d {
                            entity,
                            pipeline,
                            draw_function: draw_transparent_outline,
                            distance,
                        });
                    } else {
                        opaque_phase.add(Opaque3d {
                            entity,
                            pipeline,
                            draw_function: draw_opaque_outline,
                            distance: -distance,
                        });
                    }
                }
            }
        }
    }
}

pub struct OutlinePipeline {
    mesh_pipeline: MeshPipeline,
    view_size_bind_group_layout: BindGroupLayout,
    outline_bind_group_layout: BindGroupLayout,
}

impl FromWorld for OutlinePipeline {
    fn from_world(world: &mut World) -> Self {
        let world = world.cell();
        let mesh_pipeline = world.get_resource::<MeshPipeline>().unwrap().clone();
        let render_device = world.get_resource::<RenderDevice>().unwrap();
        let view_size_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("outline_view_size_bind_group_layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: BufferSize::new(
                            ViewSizeUniform::std140_size_static() as u64
                        ),
                    },
                    count: None,
                }],
            });
        let outline_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("outline_bind_group_layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: BufferSize::new(
                                VertexStageData::std140_size_static() as u64,
                            ),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: BufferSize::new(
                                FragmentStageData::std140_size_static() as u64,
                            ),
                        },
                        count: None,
                    },
                ],
            });
        OutlinePipeline {
            mesh_pipeline,
            view_size_bind_group_layout,
            outline_bind_group_layout,
        }
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
        descriptor.vertex.shader = OUTLINE_SHADER_HANDLE.typed();
        descriptor.fragment.as_mut().unwrap().shader = OUTLINE_SHADER_HANDLE.typed();
        descriptor.layout = Some(vec![
            self.mesh_pipeline.view_layout.clone(),
            self.mesh_pipeline.mesh_layout.clone(),
            self.view_size_bind_group_layout.clone(),
            self.outline_bind_group_layout.clone(),
        ]);
        Ok(descriptor)
    }
}
