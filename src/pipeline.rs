use std::borrow::Cow;

use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::render_resource::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState,
    BufferBindingType, BufferSize, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState,
    DepthStencilState, Face, FragmentState, FrontFace, MultisampleState, PolygonMode,
    PrimitiveState, ShaderSize, ShaderStages, StencilState, TextureFormat, VertexState,
};
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::BevyDefault;
use bevy::{
    pbr::{MeshPipeline, MeshPipelineKey},
    render::{
        mesh::MeshVertexBufferLayout,
        render_resource::{
            RenderPipelineDescriptor, SpecializedMeshPipeline, SpecializedMeshPipelineError,
        },
    },
};

use crate::uniforms::{OutlineFragmentUniform, OutlineStencilUniform, OutlineVolumeUniform};
use crate::view_uniforms::OutlineViewUniform;
use crate::ATTRIBUTE_OUTLINE_NORMAL;

pub const OUTLINE_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 2101625026478770097);

pub const FRAGMENT_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 12033806834125368121);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassType {
    Stencil,
    Opaque,
    Transparent,
}

pub struct OutlinePipeline {
    mesh_pipeline: MeshPipeline,
    pub outline_view_bind_group_layout: BindGroupLayout,
    pub outline_stencil_bind_group_layout: BindGroupLayout,
    pub outline_volume_bind_group_layout: BindGroupLayout,
}

impl FromWorld for OutlinePipeline {
    fn from_world(world: &mut World) -> Self {
        let world = world.cell();
        let mesh_pipeline = world.get_resource::<MeshPipeline>().unwrap().clone();
        let render_device = world.get_resource::<RenderDevice>().unwrap();
        let outline_view_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("outline_view_bind_group_layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: BufferSize::new(OutlineViewUniform::SHADER_SIZE.get()),
                    },
                    count: None,
                }],
            });
        let outline_volume_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("outline_volume_bind_group_layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: BufferSize::new(
                                OutlineVolumeUniform::SHADER_SIZE.get(),
                            ),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: BufferSize::new(
                                OutlineFragmentUniform::SHADER_SIZE.get(),
                            ),
                        },
                        count: None,
                    },
                ],
            });
        let outline_stencil_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("outline_stencil_bind_group_layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: BufferSize::new(OutlineStencilUniform::SHADER_SIZE.get()),
                    },
                    count: None,
                }],
            });
        OutlinePipeline {
            mesh_pipeline,
            outline_view_bind_group_layout,
            outline_stencil_bind_group_layout,
            outline_volume_bind_group_layout,
        }
    }
}

impl SpecializedMeshPipeline for OutlinePipeline {
    type Key = (MeshPipelineKey, PassType);

    fn specialize(
        &self,
        (key, pass_type): Self::Key,
        mesh_layout: &MeshVertexBufferLayout,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut targets = vec![];
        let mut bind_layouts = vec![self.mesh_pipeline.view_layout.clone()];
        let mut buffer_attrs = vec![Mesh::ATTRIBUTE_POSITION.at_shader_location(0)];
        let mut vertex_defs = vec![];
        let mut fragment_defs = vec![];
        bind_layouts.push(
            if mesh_layout.contains(Mesh::ATTRIBUTE_JOINT_INDEX)
                && mesh_layout.contains(Mesh::ATTRIBUTE_JOINT_WEIGHT)
            {
                vertex_defs.push("SKINNED".to_string());
                buffer_attrs.push(Mesh::ATTRIBUTE_JOINT_INDEX.at_shader_location(2));
                buffer_attrs.push(Mesh::ATTRIBUTE_JOINT_WEIGHT.at_shader_location(3));
                self.mesh_pipeline.skinned_mesh_layout.clone()
            } else {
                self.mesh_pipeline.mesh_layout.clone()
            },
        );
        bind_layouts.push(self.outline_view_bind_group_layout.clone());
        match pass_type {
            PassType::Stencil => {
                vertex_defs.push("OFFSET_ZERO".to_string());
                bind_layouts.push(self.outline_stencil_bind_group_layout.clone());
            }
            PassType::Opaque | PassType::Transparent => {
                fragment_defs.push("VOLUME".to_string());
                targets.push(Some(ColorTargetState {
                    format: TextureFormat::bevy_default(),
                    blend: Some(if pass_type == PassType::Transparent {
                        BlendState::ALPHA_BLENDING
                    } else {
                        BlendState::REPLACE
                    }),
                    write_mask: ColorWrites::ALL,
                }));

                bind_layouts.push(self.outline_volume_bind_group_layout.clone());
                buffer_attrs.push(
                    if mesh_layout.contains(ATTRIBUTE_OUTLINE_NORMAL) {
                        ATTRIBUTE_OUTLINE_NORMAL
                    } else {
                        Mesh::ATTRIBUTE_NORMAL
                    }
                    .at_shader_location(1),
                );
            }
        }
        let buffers = vec![mesh_layout.get_layout(&buffer_attrs)?];
        Ok(RenderPipelineDescriptor {
            vertex: VertexState {
                shader: OUTLINE_SHADER_HANDLE.typed::<Shader>(),
                entry_point: "vertex".into(),
                shader_defs: vertex_defs,
                buffers,
            },
            fragment: Some(FragmentState {
                shader: FRAGMENT_SHADER_HANDLE.typed::<Shader>(),
                shader_defs: fragment_defs,
                entry_point: "fragment".into(),
                targets,
            }),
            layout: Some(bind_layouts),
            primitive: PrimitiveState {
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
                topology: key.primitive_topology(),
                strip_index_format: None,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Greater,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            label: Some(Cow::Borrowed("outline_stencil_pipeline")),
        })
    }
}
