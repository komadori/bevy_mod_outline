use std::borrow::Cow;

use bevy::asset::uuid_handle;
use bevy::ecs::system::lifetimeless::SRes;
use bevy::ecs::system::SystemParamItem;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{
    setup_morph_and_skinning_defs, skins_use_uniform_buffers, MorphIndices, SkinUniforms,
};
use bevy::prelude::*;
use bevy::render::batching::{gpu_preprocessing, GetBatchData, GetFullBatchData};
use bevy::render::mesh::allocator::{MeshAllocator, MeshSlabs};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer_sized};
use bevy::render::render_resource::{
    BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendState, ColorTargetState, ColorWrites,
    CompareFunction, DepthBiasState, DepthStencilState, Face, FragmentState, FrontFace,
    GpuArrayBuffer, MultisampleState, PolygonMode, PrimitiveState, SamplerBindingType,
    ShaderStages, ShaderType, StencilState, TextureFormat, TextureSampleType, VertexState,
};
use bevy::render::renderer::RenderDevice;
use bevy::render::settings::{Backends, WgpuSettings};
use bevy::render::sync_world::MainEntity;
use bevy::shader::ShaderDefVal;
use bevy::{
    pbr::MeshPipeline,
    render::render_resource::{
        RenderPipelineDescriptor, SpecializedMeshPipeline, SpecializedMeshPipelineError,
    },
};
use nonmax::NonMaxU32;

use crate::pipeline_key::{DerivedPipelineKey, PassType};
use crate::uniforms::{DepthMode, OutlineInstanceUniform, RenderOutlineInstances};
use crate::view_uniforms::OutlineViewUniform;
use crate::ATTRIBUTE_OUTLINE_NORMAL;

pub(crate) const COMMON_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("aee41cd9-fc8f-4788-9ea4-f85bd8070c65");

pub(crate) const OUTLINE_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("910c269f-f115-47ba-b757-6ae51bf0c79f");

pub(crate) const FRAGMENT_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("1f5b5967-7cbb-4392-8f34-421587938a12");

#[derive(Resource)]
pub(crate) struct OutlinePipeline {
    mesh_pipeline: MeshPipeline,
    pub outline_view_bind_group_layout: BindGroupLayoutDescriptor,
    pub outline_instance_bind_group_layout: BindGroupLayoutDescriptor,
    pub alpha_mask_bind_group_layout: BindGroupLayoutDescriptor,
    pub instance_batch_size: Option<u32>,
    pub skins_use_uniform_buffers: bool,
}

pub(crate) fn init_outline_pipeline(
    mut commands: Commands,
    mesh_pipeline: Res<MeshPipeline>,
    render_device: Res<RenderDevice>,
) {
    let limits = render_device.limits();
    let outline_view_bind_group_layout = BindGroupLayoutDescriptor::new(
        "outline_view_bind_group_layout",
        &BindGroupLayoutEntries::single(
            ShaderStages::VERTEX,
            uniform_buffer_sized(true, Some(OutlineViewUniform::min_size())),
        ),
    );
    let outline_instance_bind_group_layout = BindGroupLayoutDescriptor::new(
        "outline_instance_bind_group_layout",
        &BindGroupLayoutEntries::single(
            ShaderStages::VERTEX,
            GpuArrayBuffer::<OutlineInstanceUniform>::binding_layout(&limits),
        ),
    );
    let alpha_mask_bind_group_layout = BindGroupLayoutDescriptor::new(
        "alpha_mask_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );

    let instance_batch_size = GpuArrayBuffer::<OutlineInstanceUniform>::batch_size(&limits);
    let skins_use_uniform_buffers = skins_use_uniform_buffers(&limits);
    commands.insert_resource(OutlinePipeline {
        mesh_pipeline: mesh_pipeline.clone(),
        outline_view_bind_group_layout,
        outline_instance_bind_group_layout,
        alpha_mask_bind_group_layout,
        instance_batch_size,
        skins_use_uniform_buffers,
    });
}

impl SpecializedMeshPipeline for OutlinePipeline {
    type Key = DerivedPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut targets = vec![];
        let mut vertex_defs = vec![];
        let mut fragment_defs = vec![];
        let mut buffer_attrs = vec![Mesh::ATTRIBUTE_POSITION.at_shader_location(0)];

        let bind_layouts = vec![
            self.outline_view_bind_group_layout.clone(),
            self.outline_instance_bind_group_layout.clone(),
            setup_morph_and_skinning_defs(
                &self.mesh_pipeline.mesh_layouts,
                layout,
                5,
                &key.into(),
                &mut vertex_defs,
                &mut buffer_attrs,
                self.skins_use_uniform_buffers,
            ),
            self.alpha_mask_bind_group_layout.clone(),
        ];

        if key.alpha_mask_texture() {
            let val = ShaderDefVal::from("ALPHA_MASK_TEXTURE");
            vertex_defs.push(val.clone());
            fragment_defs.push(val);

            let channel_def = ShaderDefVal::UInt(
                "ALPHA_MASK_CHANNEL".to_string(),
                key.alpha_mask_channel_int(),
            );
            fragment_defs.push(channel_def);

            buffer_attrs.push(Mesh::ATTRIBUTE_UV_0.at_shader_location(2));
        }

        if let Some(sz) = self.instance_batch_size {
            vertex_defs.push(ShaderDefVal::Int(
                "INSTANCE_BATCH_SIZE".to_string(),
                sz as i32,
            ));
        }

        if key.msaa() != Msaa::Off {
            fragment_defs.push(ShaderDefVal::from("MSAA"));
        }
        if key.pass_type() != PassType::FloodInit && key.depth_mode() == DepthMode::Flat {
            let val = ShaderDefVal::from("FLAT_DEPTH");
            vertex_defs.push(val.clone());
            fragment_defs.push(val);
        }
        let cull_mode =
            if key.pass_type() == PassType::FloodInit || key.depth_mode() == DepthMode::Flat {
                if key.double_sided() {
                    None
                } else {
                    Some(Face::Back)
                }
            } else if key.pass_type() == PassType::Stencil {
                Some(Face::Back)
            } else {
                Some(Face::Front)
            };
        if key.vertex_offset_zero() {
            vertex_defs.push(ShaderDefVal::from("VERTEX_OFFSET_ZERO"));
        } else {
            buffer_attrs.push(
                if layout.0.contains(ATTRIBUTE_OUTLINE_NORMAL) {
                    ATTRIBUTE_OUTLINE_NORMAL
                } else {
                    Mesh::ATTRIBUTE_NORMAL
                }
                .at_shader_location(1),
            );
        }
        if key.plane_offset_zero() {
            vertex_defs.push(ShaderDefVal::from("PLANE_OFFSET_ZERO"));
        }
        match key.pass_type() {
            PassType::Stencil => {}
            PassType::Volume => {
                let val = ShaderDefVal::from("VOLUME");
                vertex_defs.push(val.clone());
                fragment_defs.push(val);
                targets.push(Some(ColorTargetState {
                    format: key.target_format(),
                    blend: Some(if key.transparent() {
                        BlendState::ALPHA_BLENDING
                    } else {
                        BlendState::REPLACE
                    }),
                    write_mask: ColorWrites::ALL,
                }));
            }
            #[cfg(feature = "flood")]
            PassType::FloodInit => {
                let val = ShaderDefVal::from("FLOOD_INIT");
                vertex_defs.push(val.clone());
                fragment_defs.push(val);
                targets.push(Some(ColorTargetState {
                    format: if key.msaa() != Msaa::Off {
                        TextureFormat::R8Unorm
                    } else {
                        TextureFormat::Rg16Float
                    },
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                }));
            }
        }
        let depth_stencil = match key.pass_type() {
            PassType::Stencil | PassType::Volume => Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::Greater),
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            #[cfg(feature = "flood")]
            PassType::FloodInit => None,
        };
        let buffers = vec![layout.0.get_layout(&buffer_attrs)?];
        // Proxy for webgl feature flag in bevy
        let immediate_size = if WgpuSettings::default().backends == Some(Backends::GL) {
            4
        } else {
            0
        };
        Ok(RenderPipelineDescriptor {
            vertex: VertexState {
                shader: OUTLINE_SHADER_HANDLE,
                entry_point: None,
                shader_defs: vertex_defs,
                buffers,
            },
            fragment: Some(FragmentState {
                shader: FRAGMENT_SHADER_HANDLE,
                shader_defs: fragment_defs,
                entry_point: None,
                targets,
            }),
            layout: bind_layouts,
            primitive: PrimitiveState {
                front_face: FrontFace::Ccw,
                cull_mode,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
                topology: key.primitive_topology(),
                strip_index_format: None,
            },
            depth_stencil,
            multisample: MultisampleState {
                count: key.msaa().samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            immediate_size,
            label: Some(Cow::Borrowed("outline_pipeline")),
            zero_initialize_workgroup_memory: false,
        })
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct OutlineBatchSetCompareData {
    mesh_slabs: Option<MeshSlabs>,
    alpha_mask_id: Option<AssetId<Image>>,
}

impl GetBatchData for OutlinePipeline {
    type Param = (
        SRes<RenderOutlineInstances>,
        SRes<MeshAllocator>,
        SRes<SkinUniforms>,
        SRes<MorphIndices>,
    );
    type BatchSetCompareData = OutlineBatchSetCompareData;
    type BatchCompareData = AssetId<Mesh>;
    type BufferData = OutlineInstanceUniform;

    fn get_batch_data(
        (render_outlines, mesh_allocator, skin_uniforms, morph_indices): &SystemParamItem<
            Self::Param,
        >,
        (_entity, main_entity): (Entity, MainEntity),
    ) -> Option<(
        Self::BufferData,
        Option<(Self::BatchSetCompareData, Self::BatchCompareData)>,
    )> {
        let outline = render_outlines.get(&main_entity)?;
        let instance_data = outline.instance_data.prepare_instance(
            &outline.mesh_id,
            main_entity,
            mesh_allocator,
            skin_uniforms,
            morph_indices,
        );

        // Only batch entities with the same mesh and alpha mask texture
        let batch_data = if outline.automatic_batching {
            Some((
                OutlineBatchSetCompareData {
                    mesh_slabs: mesh_allocator.mesh_slabs(&outline.mesh_id),
                    alpha_mask_id: outline.alpha_mask_id,
                },
                outline.mesh_id,
            ))
        } else {
            None
        };

        Some((instance_data, batch_data))
    }
}

impl GetFullBatchData for OutlinePipeline {
    type BufferInputData = ();

    fn get_binned_batch_data(
        (render_outlines, mesh_allocator, skin_uniforms, morph_indices): &SystemParamItem<
            Self::Param,
        >,
        main_entity: MainEntity,
    ) -> Option<Self::BufferData> {
        let outline = render_outlines.get(&main_entity)?;
        Some(outline.instance_data.prepare_instance(
            &outline.mesh_id,
            main_entity,
            mesh_allocator,
            skin_uniforms,
            morph_indices,
        ))
    }

    fn get_index_and_compare_data(
        _param: &SystemParamItem<Self::Param>,
        _main_entity: MainEntity,
    ) -> Option<(
        NonMaxU32,
        Option<(Self::BatchSetCompareData, Self::BatchCompareData)>,
    )> {
        unimplemented!("GPU batching is not used.");
    }

    fn get_binned_index(
        _param: &SystemParamItem<Self::Param>,
        _main_entity: MainEntity,
    ) -> Option<NonMaxU32> {
        unimplemented!("GPU batching is not used.");
    }

    fn write_batch_indirect_parameters_metadata(
        _indexed: bool,
        _base_output_index: u32,
        _batch_set_index: Option<NonMaxU32>,
        _phase_indirect_parameters_buffers: &mut gpu_preprocessing::UntypedPhaseIndirectParametersBuffers,
        _indirect_parameters_offset: u32,
    ) {
        unimplemented!("GPU batching is not used.");
    }
}
