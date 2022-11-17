use bevy::prelude::*;

/// A component for storing the computed plane on which the outline lies.
#[derive(Clone, Component, Default)]
pub struct ComputedOutlinePlane {
    pub(crate) plane: Vec3,
}

/// A component which specifies that this entity lies on the same plane as its parent.
#[derive(Clone, Component, Default)]
pub struct InheritOutlinePlane;

#[allow(clippy::type_complexity)]
pub(crate) fn compute_outline_plane(
    mut root_query: Query<
        (
            &mut ComputedOutlinePlane,
            &GlobalTransform,
            Changed<GlobalTransform>,
            Option<(&Children, Changed<Children>)>,
        ),
        Without<InheritOutlinePlane>,
    >,
    mut computed_query: Query<(&mut ComputedOutlinePlane, Changed<InheritOutlinePlane>)>,
    child_query: Query<(&Children, Changed<Children>)>,
) {
    for (mut computed, transform, changed_transform, children) in root_query.iter_mut() {
        if changed_transform {
            let matrix = transform.compute_matrix();
            computed.plane = matrix.project_point3(Vec3::ZERO);
        }
        if let Some((cs, changed_children)) = children {
            let changed2 = changed_children || changed_transform;
            for child in cs.iter() {
                propagate_outline_planes(
                    &computed,
                    changed2,
                    *child,
                    &mut computed_query,
                    &child_query,
                );
            }
        }
    }
}

fn propagate_outline_planes(
    root_computed: &ComputedOutlinePlane,
    changed: bool,
    entity: Entity,
    computed_query: &mut Query<(&mut ComputedOutlinePlane, Changed<InheritOutlinePlane>)>,
    child_query: &Query<(&Children, Changed<Children>)>,
) {
    if let Ok((mut computed, changed_inherit)) = computed_query.get_mut(entity) {
        if changed_inherit || changed {
            *computed = root_computed.clone();
        }
        if let Ok((cs, changed_children)) = child_query.get(entity) {
            let changed2 = changed_children || changed_inherit || changed;
            for child in cs.iter() {
                propagate_outline_planes(
                    root_computed,
                    changed2,
                    *child,
                    computed_query,
                    child_query,
                );
            }
        }
    }
}
