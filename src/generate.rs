use bevy::{
    math::FloatOrd,
    prelude::*,
    render::{
        mesh::{Indices, VertexAttributeValues},
        render_resource::{PrimitiveTopology, VertexFormat},
    },
    utils::{HashMap, HashSet},
};

use crate::ATTRIBUTE_OUTLINE_NORMAL;

enum IndexIterator<'a> {
    ExplicitU16(std::slice::Iter<'a, u16>),
    ExplicitU32(std::slice::Iter<'a, u32>),
    Implicit(std::ops::Range<usize>),
}

impl<'a> From<&'a Mesh> for IndexIterator<'a> {
    fn from(value: &'a Mesh) -> Self {
        match value.indices() {
            Some(Indices::U16(vec)) => IndexIterator::ExplicitU16(vec.iter()),
            Some(Indices::U32(vec)) => IndexIterator::ExplicitU32(vec.iter()),
            None => IndexIterator::Implicit(0..value.count_vertices()),
        }
    }
}

impl Iterator for IndexIterator<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            IndexIterator::ExplicitU16(iter) => iter.next().map(|val| *val as usize),
            IndexIterator::ExplicitU32(iter) => iter.next().map(|val| *val as usize),
            IndexIterator::Implicit(iter) => iter.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            IndexIterator::ExplicitU16(iter) => iter.size_hint(),
            IndexIterator::ExplicitU32(iter) => iter.size_hint(),
            IndexIterator::Implicit(iter) => iter.size_hint(),
        }
    }
}

impl ExactSizeIterator for IndexIterator<'_> {}

/// Failed to generate outline normals for the mesh.
#[derive(thiserror::Error, Debug)]
pub enum GenerateOutlineNormalsError {
    #[error("unsupported primitive topology '{0:?}'")]
    UnsupportedPrimitiveTopology(PrimitiveTopology),
    #[error("missing vertex attributes '{0}'")]
    MissingVertexAttribute(&'static str),
    #[error("the '{0}' vertex attribute should have {1:?} format, but had {2:?} format")]
    InvalidVertexAttributeFormat(&'static str, VertexFormat, VertexFormat),
}

/// Extension methods for [`Mesh`].
pub trait OutlineMeshExt {
    /// Generates outline normals for the mesh.
    ///
    /// Vertex extrusion only works for meshes with smooth surface normals. Hard edges cause
    /// visual artefacts. This function generates faux-smooth normals for outlining purposes
    /// by grouping vertices by their position and averaging the normals at each point. These
    /// outline normals are then inserted as a separate vertex attribute so that the regular
    /// normals remain untouched. However, insofar as the outline normals are not
    /// perpendicular to the surface of the mesh, this technique may result in non-uniform
    /// outline thickness.
    ///
    /// This function only supports meshes with TriangleList topology.
    fn generate_outline_normals(&mut self) -> Result<(), GenerateOutlineNormalsError>;
}

impl OutlineMeshExt for Mesh {
    fn generate_outline_normals(&mut self) -> Result<(), GenerateOutlineNormalsError> {
        if self.primitive_topology() != PrimitiveTopology::TriangleList {
            return Err(GenerateOutlineNormalsError::UnsupportedPrimitiveTopology(
                self.primitive_topology(),
            ));
        }
        let positions = match self.attribute(Mesh::ATTRIBUTE_POSITION).ok_or(
            GenerateOutlineNormalsError::MissingVertexAttribute(Mesh::ATTRIBUTE_POSITION.name),
        )? {
            VertexAttributeValues::Float32x3(p) => Ok(p),
            v => Err(GenerateOutlineNormalsError::InvalidVertexAttributeFormat(
                Mesh::ATTRIBUTE_POSITION.name,
                VertexFormat::Float32x3,
                v.into(),
            )),
        }?;
        let normals = match self.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(VertexAttributeValues::Float32x3(p)) => Some(p),
            _ => None,
        };
        let mut map = HashMap::<[FloatOrd; 3], Vec3>::with_capacity(positions.len());
        let mut it = IndexIterator::from(&*self);
        while let (Some(i0), Some(i1), Some(i2)) = (it.next(), it.next(), it.next()) {
            for (j0, j1, j2) in [(i0, i1, i2), (i1, i2, i0), (i2, i0, i1)] {
                let p0 = Vec3::from(positions[j0]);
                let p1 = Vec3::from(positions[j1]);
                let p2 = Vec3::from(positions[j2]);
                let angle = (p1 - p0).angle_between(p2 - p0);
                let n = map
                    .entry([FloatOrd(p0.x), FloatOrd(p0.y), FloatOrd(p0.z)])
                    .or_default();
                *n += angle
                    * if let Some(ns) = normals {
                        // Use vertex normal
                        Vec3::from(ns[j0])
                    } else {
                        // Calculate face normal
                        (p1 - p0).cross(p2 - p0).normalize_or_zero()
                    };
            }
        }
        let mut outlines = Vec::with_capacity(positions.len());
        for p in positions.iter() {
            let key = [FloatOrd(p[0]), FloatOrd(p[1]), FloatOrd(p[2])];
            outlines.push(
                map.get(&key)
                    .copied()
                    .unwrap_or(Vec3::ZERO)
                    .normalize_or_zero()
                    .to_array(),
            );
        }
        self.insert_attribute(
            ATTRIBUTE_OUTLINE_NORMAL,
            VertexAttributeValues::Float32x3(outlines),
        );
        Ok(())
    }
}

fn auto_generate_outline_normals(
    mut meshes: ResMut<Assets<Mesh>>,
    mut events: EventReader<'_, '_, AssetEvent<Mesh>>,
    mut squelch: Local<HashSet<AssetId<Mesh>>>,
) {
    for event in events.read() {
        match event {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                if squelch.contains(id) {
                    // Suppress modification events created by this system
                    squelch.remove(id);
                } else if let Some(mesh) = meshes.get_mut(*id) {
                    let _ = mesh.generate_outline_normals();
                    squelch.insert(*id);
                }
            }
            AssetEvent::Removed { id } => {
                squelch.remove(id);
            }
            _ => {}
        }
    }
}

/// Automatically runs [`generate_outline_normals`](OutlineMeshExt::generate_outline_normals)
/// on every mesh.
///
/// This is provided as a convenience for simple projects. It runs the outline normal
/// generator every time a mesh asset is created or modified without consideration for
/// whether this is necessary or appropriate.
pub struct AutoGenerateOutlineNormalsPlugin;

impl Plugin for AutoGenerateOutlineNormalsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, auto_generate_outline_normals);
    }
}
