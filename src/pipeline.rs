use std::borrow::Cow;

use bevy::pbr::{setup_morph_and_skinning_defs, MeshPipelineKey};
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::render_resource::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState,
    BufferBindingType, BufferSize, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState,
    DepthStencilState, Face, FragmentState, FrontFace, MultisampleState, PolygonMode,
    PrimitiveState, PrimitiveTopology, ShaderDefVal, ShaderSize, ShaderStages, StencilState,
    TextureFormat, VertexState,
};
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::BevyDefault;
use bevy::render::view::ViewTarget;
use bevy::{
    pbr::MeshPipeline,
    render::{
        mesh::MeshVertexBufferLayout,
        render_resource::{
            RenderPipelineDescriptor, SpecializedMeshPipeline, SpecializedMeshPipelineError,
        },
    },
};
use bitfield::{bitfield_bitrange, bitfield_fields};

use crate::uniforms::{
    DepthMode, OutlineFragmentUniform, OutlineStencilUniform, OutlineVolumeUniform,
};
use crate::view_uniforms::OutlineViewUniform;
use crate::ATTRIBUTE_OUTLINE_NORMAL;

pub(crate) const OUTLINE_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 2101625026478770097);

pub(crate) const FRAGMENT_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 12033806834125368121);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PassType {
    Stencil = 1,
    Opaque = 2,
    Transparent = 3,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PipelineKey(u32);
bitfield_bitrange! {struct PipelineKey(u32)}

impl PipelineKey {
    bitfield_fields! {
        u32;
        msaa_samples_minus_one, set_msaa_samples_minus_one: 5, 0;
        primitive_topology_int, set_primitive_topology_int: 8, 6;
        pass_type_int, set_pass_type_int: 10, 9;
        depth_mode_int, set_depth_mode_int: 12, 11;
        pub offset_zero, set_offset_zero: 13;
        pub hdr_format, set_hdr_format: 14;
        pub opengl_workaround, set_opengl_workaround: 15;
        pub morph_targets, set_morph_targets: 16;
    }

    pub(crate) fn new() -> Self {
        PipelineKey(0)
    }

    pub(crate) fn with_msaa(mut self, msaa: Msaa) -> Self {
        self.set_msaa_samples_minus_one(msaa as u32 - 1);
        self
    }

    pub(crate) fn msaa(&self) -> Msaa {
        match self.msaa_samples_minus_one() + 1 {
            x if x == Msaa::Off as u32 => Msaa::Off,
            x if x == Msaa::Sample2 as u32 => Msaa::Sample2,
            x if x == Msaa::Sample4 as u32 => Msaa::Sample4,
            x if x == Msaa::Sample8 as u32 => Msaa::Sample8,
            x => panic!("Invalid value for Msaa: {}", x),
        }
    }

    pub(crate) fn with_primitive_topology(mut self, primitive_topology: PrimitiveTopology) -> Self {
        self.set_primitive_topology_int(primitive_topology as u32);
        self
    }

    pub(crate) fn primitive_topology(&self) -> PrimitiveTopology {
        match self.primitive_topology_int() {
            x if x == PrimitiveTopology::PointList as u32 => PrimitiveTopology::PointList,
            x if x == PrimitiveTopology::LineList as u32 => PrimitiveTopology::LineList,
            x if x == PrimitiveTopology::LineStrip as u32 => PrimitiveTopology::LineStrip,
            x if x == PrimitiveTopology::TriangleList as u32 => PrimitiveTopology::TriangleList,
            x if x == PrimitiveTopology::TriangleStrip as u32 => PrimitiveTopology::TriangleStrip,
            x => panic!("Invalid value for PrimitiveTopology: {}", x),
        }
    }

    pub(crate) fn with_pass_type(mut self, pass_type: PassType) -> Self {
        self.set_pass_type_int(pass_type as u32);
        self
    }

    pub(crate) fn pass_type(&self) -> PassType {
        match self.pass_type_int() {
            x if x == PassType::Stencil as u32 => PassType::Stencil,
            x if x == PassType::Opaque as u32 => PassType::Opaque,
            x if x == PassType::Transparent as u32 => PassType::Transparent,
            x => panic!("Invalid value for PassType: {}", x),
        }
    }

    pub(crate) fn with_depth_mode(mut self, depth_mode: DepthMode) -> Self {
        self.set_depth_mode_int(depth_mode as u32);
        self
    }

    pub(crate) fn depth_mode(&self) -> DepthMode {
        match self.depth_mode_int() {
            x if x == DepthMode::Flat as u32 => DepthMode::Flat,
            x if x == DepthMode::Real as u32 => DepthMode::Real,
            x => panic!("Invalid value for DepthMode: {}", x),
        }
    }

    pub(crate) fn with_offset_zero(mut self, offset_zero: bool) -> Self {
        self.set_offset_zero(offset_zero);
        self
    }

    pub(crate) fn with_hdr_format(mut self, hdr_format: bool) -> Self {
        self.set_hdr_format(hdr_format);
        self
    }

    pub(crate) fn with_opengl_workaround(mut self, opengl_workaround: bool) -> Self {
        self.set_opengl_workaround(opengl_workaround);
        self
    }

    pub(crate) fn with_morph_targets(mut self, morph_targets: bool) -> Self {
        self.set_morph_targets(morph_targets);
        self
    }
}

impl From<PipelineKey> for MeshPipelineKey {
    fn from(key: PipelineKey) -> Self {
        if key.morph_targets() {
            MeshPipelineKey::empty() | MeshPipelineKey::MORPH_TARGETS
        } else {
            MeshPipelineKey::empty()
        }
    }
}

#[derive(Resource)]
pub(crate) struct OutlinePipeline {
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
    type Key = PipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayout,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut targets = vec![];
        let mut vertex_defs = vec!["MESH_BINDGROUP_1".into()];
        let mut fragment_defs = vec![];
        let mut buffer_attrs = Vec::new();

        if layout.contains(Mesh::ATTRIBUTE_POSITION) {
            vertex_defs.push("VERTEX_POSITIONS".into());
            buffer_attrs.push(Mesh::ATTRIBUTE_POSITION.at_shader_location(0));
        }

        if layout.contains(Mesh::ATTRIBUTE_NORMAL) {
            vertex_defs.push("VERTEX_NORMALS".into());
            buffer_attrs.push(Mesh::ATTRIBUTE_NORMAL.at_shader_location(2));
        }

        if layout.contains(Mesh::ATTRIBUTE_TANGENT) {
            vertex_defs.push("VERTEX_TANGENTS".into());
            buffer_attrs.push(Mesh::ATTRIBUTE_TANGENT.at_shader_location(3));
        }

        let mut bind_layouts = vec![if key.msaa() == Msaa::Off {
            self.mesh_pipeline.view_layout.clone()
        } else {
            self.mesh_pipeline.view_layout_multisampled.clone()
        }];

        bind_layouts.push(setup_morph_and_skinning_defs(
            &self.mesh_pipeline.mesh_layouts,
            layout,
            5,
            &key.into(),
            &mut vertex_defs,
            &mut buffer_attrs,
        ));

        bind_layouts.push(self.outline_view_bind_group_layout.clone());
        let cull_mode;
        if key.depth_mode() == DepthMode::Flat {
            vertex_defs.push(ShaderDefVal::from("FLAT_DEPTH"));
            cull_mode = Some(Face::Back);
        } else if key.pass_type() == PassType::Stencil {
            cull_mode = Some(Face::Back);
        } else {
            cull_mode = Some(Face::Front);
        }
        if key.offset_zero() {
            vertex_defs.push(ShaderDefVal::from("OFFSET_ZERO"));
        } else {
            buffer_attrs.push(
                if layout.contains(ATTRIBUTE_OUTLINE_NORMAL) {
                    ATTRIBUTE_OUTLINE_NORMAL
                } else {
                    Mesh::ATTRIBUTE_NORMAL
                }
                .at_shader_location(1),
            );
        }
        match key.pass_type() {
            PassType::Stencil => {
                bind_layouts.push(self.outline_stencil_bind_group_layout.clone());
            }
            PassType::Opaque | PassType::Transparent => {
                fragment_defs.push(ShaderDefVal::from("VOLUME"));
                targets.push(Some(ColorTargetState {
                    format: if key.hdr_format() {
                        ViewTarget::TEXTURE_FORMAT_HDR
                    } else {
                        TextureFormat::bevy_default()
                    },
                    blend: Some(if key.pass_type() == PassType::Transparent {
                        BlendState::ALPHA_BLENDING
                    } else {
                        BlendState::REPLACE
                    }),
                    write_mask: ColorWrites::ALL,
                }));

                bind_layouts.push(self.outline_volume_bind_group_layout.clone());
            }
        }
        if key.opengl_workaround() {
            let val = ShaderDefVal::from("OPENGL_WORKAROUND");
            vertex_defs.push(val.clone());
            fragment_defs.push(val);
        }
        let buffers = vec![layout.get_layout(&buffer_attrs)?];
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
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Greater,
                stencil: StencilState::default(),
                bias: if key.depth_mode() == DepthMode::Flat && key.pass_type() == PassType::Stencil
                {
                    DepthBiasState {
                        // Values determined empirically
                        constant: 3,
                        slope_scale: 1.0,
                        ..default()
                    }
                } else {
                    default()
                },
            }),
            multisample: MultisampleState {
                count: key.msaa().samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            push_constant_ranges: default(),
            label: Some(Cow::Borrowed("outline_pipeline")),
        })
    }
}
