use bevy::{ecs::query::QueryItem, prelude::*, render::view::RenderLayers};

use crate::{
    uniforms::{DepthMode, DrawMode},
    InheritOutline, OutlineMode, OutlinePlaneDepth, OutlineRenderLayers, OutlineStencil,
    OutlineVolume,
};

#[derive(Clone)]
pub(crate) struct ComputedVolume {
    pub(crate) enabled: bool,
    pub(crate) offset: f32,
    pub(crate) colour: LinearRgba,
}

#[derive(Clone)]
pub(crate) struct ComputedStencil {
    pub(crate) enabled: bool,
    pub(crate) offset: f32,
}

#[derive(Clone)]
pub(crate) struct ComputedMode {
    pub(crate) depth_mode: DepthMode,
    pub(crate) draw_mode: DrawMode,
}

#[derive(Clone)]
pub(crate) struct ComputedDepth {
    pub(crate) world_plane_origin: Vec3,
    pub(crate) world_plane_offset: Vec3,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Source {
    Set,
    Inherited,
}

#[derive(Clone)]
pub(crate) struct Sourced<T: Clone> {
    pub(crate) value: T,
    pub(crate) source: Source,
}

impl<T: Clone> Sourced<T> {
    pub fn inherit(sourced: &Sourced<T>) -> Self {
        Sourced {
            value: sourced.value.clone(),
            source: Source::Inherited,
        }
    }

    pub fn set(value: T) -> Self {
        Sourced {
            value,
            source: Source::Set,
        }
    }

    pub fn is_changed<U: Component>(&self, tuple: &Option<Ref<U>>) -> bool {
        tuple.is_some() != matches!(self.source, Source::Set)
            || if let Some(r) = tuple {
                r.is_changed()
            } else {
                false
            }
    }
}

#[derive(Clone)]
pub(crate) struct ComputedInternal {
    pub(crate) inherited_from: Option<Entity>,
    pub(crate) volume: Sourced<ComputedVolume>,
    pub(crate) stencil: Sourced<ComputedStencil>,
    pub(crate) mode: Sourced<ComputedMode>,
    pub(crate) depth: Sourced<ComputedDepth>,
    pub(crate) layers: Sourced<RenderLayers>,
}

/// A component for storing the computed depth at which the outline lies.
#[derive(Clone, Component, Default)]
pub struct ComputedOutline(pub(crate) Option<ComputedInternal>);

type OutlineComponents<'a> = (
    Ref<'a, InheritedVisibility>,
    Ref<'a, GlobalTransform>,
    Option<Ref<'a, OutlineVolume>>,
    Option<Ref<'a, OutlineStencil>>,
    Option<Ref<'a, OutlineMode>>,
    Option<Ref<'a, OutlinePlaneDepth>>,
    Option<Ref<'a, OutlineRenderLayers>>,
);

#[allow(clippy::type_complexity)]
pub(crate) fn compute_outline(
    mut root_query: Query<
        (
            Entity,
            &mut ComputedOutline,
            OutlineComponents,
            Option<&Children>,
        ),
        Without<InheritOutline>,
    >,
    mut child_query_mut: Query<(&mut ComputedOutline, OutlineComponents), With<InheritOutline>>,
    child_query: Query<&Children>,
) {
    for (entity, mut computed, components, children) in root_query.iter_mut() {
        let changed = update_computed_outline(&mut computed, components, None, None, false);
        if let Some(cs) = children {
            let parent_computed = computed.0.as_ref().unwrap();
            for child in cs.iter() {
                propagate_computed_outline(
                    parent_computed,
                    changed,
                    entity,
                    *child,
                    &mut child_query_mut,
                    &child_query,
                );
            }
        }
    }
}

fn propagate_computed_outline(
    parent_computed: &ComputedInternal,
    parent_changed: bool,
    parent_entity: Entity,
    entity: Entity,
    child_query_mut: &mut Query<(&mut ComputedOutline, OutlineComponents), With<InheritOutline>>,
    child_query: &Query<&Children>,
) {
    if let Ok((mut computed, components)) = child_query_mut.get_mut(entity) {
        let changed = update_computed_outline(
            &mut computed,
            components,
            Some(parent_computed),
            Some(parent_entity),
            parent_changed,
        );
        if let Ok(cs) = child_query.get(entity) {
            let parent_computed = &computed.0.as_ref().unwrap().clone();
            for child in cs.iter() {
                propagate_computed_outline(
                    parent_computed,
                    changed,
                    entity,
                    *child,
                    child_query_mut,
                    child_query,
                );
            }
        }
    }
}

trait OptionExt<T> {
    fn only_if(self, b: bool) -> Option<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn only_if(self, b: bool) -> Self {
        if b {
            self
        } else {
            None
        }
    }
}

fn update_computed_outline(
    computed: &mut ComputedOutline,
    (visibility, transform, volume, stencil, mode, depth, layers): QueryItem<'_, OutlineComponents>,
    parent_computed: Option<&ComputedInternal>,
    parent_entity: Option<Entity>,
    force_update: bool,
) -> bool {
    let changed = force_update
        || if let ComputedOutline(Some(computed)) = computed {
            computed.inherited_from != parent_entity
                || visibility.is_changed()
                || transform.is_changed()
                || computed.volume.is_changed(&volume)
                || computed.stencil.is_changed(&stencil)
                || computed.mode.is_changed(&mode)
                || computed.depth.is_changed(&depth)
                || computed.layers.is_changed(&layers)
        } else {
            true
        };
    if changed {
        *computed = ComputedOutline(Some(ComputedInternal {
            inherited_from: parent_entity,
            volume: if let Some(parent_computed) = parent_computed.only_if(volume.is_none()) {
                Sourced::inherit(&parent_computed.volume)
            } else {
                let vol = volume.as_deref().cloned().unwrap_or_default();
                Sourced::set(ComputedVolume {
                    enabled: visibility.get() && vol.visible && !vol.colour.is_fully_transparent(),
                    offset: vol.width,
                    colour: vol.colour.into(),
                })
            },
            stencil: if let Some(parent_computed) = parent_computed.only_if(stencil.is_none()) {
                Sourced::inherit(&parent_computed.stencil)
            } else {
                let sten = stencil.as_deref().cloned().unwrap_or_default();
                Sourced::set(ComputedStencil {
                    enabled: visibility.get() && sten.enabled,
                    offset: sten.offset,
                })
            },
            mode: if let Some(parent_computed) = parent_computed.only_if(mode.is_none()) {
                Sourced::inherit(&parent_computed.mode)
            } else {
                let mode = mode.as_deref().cloned().unwrap_or_default();
                Sourced::set(match mode {
                    OutlineMode::ExtrudeFlat => ComputedMode {
                        depth_mode: DepthMode::Flat,
                        draw_mode: DrawMode::Extrude,
                    },
                    OutlineMode::ExtrudeReal => ComputedMode {
                        depth_mode: DepthMode::Real,
                        draw_mode: DrawMode::Extrude,
                    },
                    #[cfg(feature = "flood")]
                    OutlineMode::FloodFlat => ComputedMode {
                        depth_mode: DepthMode::Flat,
                        draw_mode: DrawMode::JumpFlood,
                    },
                })
            },
            depth: if let Some(parent_computed) = parent_computed.only_if(depth.is_none()) {
                Sourced::inherit(&parent_computed.depth)
            } else {
                let dep = depth.as_deref().cloned().unwrap_or_default();
                let affine = transform.affine();
                let inverse = transform.affine().matrix3.inverse();
                Sourced::set(ComputedDepth {
                    world_plane_origin: (affine.matrix3.mul_vec3a(dep.model_plane_offset.into())
                        + affine.translation)
                        .into(),
                    world_plane_offset: inverse.mul_vec3(dep.model_plane_offset),
                })
            },
            layers: if let Some(parent_computed) = parent_computed.only_if(layers.is_none()) {
                Sourced::inherit(&parent_computed.layers)
            } else {
                let layers = layers.as_deref().cloned().unwrap_or_default();
                Sourced::set(layers.0)
            },
        }));
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (App, Entity) {
        let mut app = App::new();
        app.add_systems(Update, compute_outline);
        let entity = app
            .world_mut()
            .spawn((
                ComputedOutline::default(),
                InheritedVisibility::VISIBLE,
                GlobalTransform::default(),
            ))
            .id();
        (app, entity)
    }

    #[test]
    fn test_defaults() {
        let (mut app, entity) = setup();
        app.update();

        let computed = app
            .world()
            .get::<ComputedOutline>(entity)
            .expect("Entity should have ComputedOutline");
        let internal = computed
            .0
            .as_ref()
            .expect("ComputedOutline should have Some value after update");
        assert!(internal.stencil.value.enabled);
        assert!(!internal.volume.value.enabled);
    }

    #[test]
    fn test_volume_propagation() {
        let (mut app, parent) = setup();

        // Create a child entity that inherits outline properties
        let child = app
            .world_mut()
            .spawn((
                InheritOutline,
                ComputedOutline::default(),
                InheritedVisibility::VISIBLE,
                GlobalTransform::default(),
            ))
            .set_parent(parent)
            .id();

        // Add an OutlineVolume to the parent
        let volume = OutlineVolume {
            visible: true,
            width: 2.0,
            colour: Color::WHITE,
        };
        app.world_mut().entity_mut(parent).insert(volume.clone());

        // Update the system
        app.update();

        // Check parent computed outline
        let parent_computed = app
            .world()
            .get::<ComputedOutline>(parent)
            .expect("Parent entity should have ComputedOutline component");
        let parent_internal = parent_computed
            .0
            .as_ref()
            .expect("Parent ComputedOutline should have Some value after update");
        assert!(parent_internal.volume.value.enabled);
        assert_eq!(parent_internal.volume.value.offset, 2.0);
        assert_eq!(parent_internal.volume.source, Source::Set);
        assert_eq!(parent_internal.inherited_from, None);

        // Check child computed outline
        let child_computed = app
            .world()
            .get::<ComputedOutline>(child)
            .expect("Child entity should have ComputedOutline component");
        let child_internal = child_computed
            .0
            .as_ref()
            .expect("Child ComputedOutline should have Some value after update");
        assert!(child_internal.volume.value.enabled);
        assert_eq!(child_internal.volume.value.offset, 2.0);
        assert_eq!(child_internal.volume.source, Source::Inherited);
        assert_eq!(child_internal.inherited_from, Some(parent));
    }
}
