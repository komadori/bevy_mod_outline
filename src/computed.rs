use bevy::{ecs::query::QueryItem, prelude::*};

use crate::{uniforms::DepthMode, InheritOutline, OutlineMode, OutlineStencil, OutlineVolume};

#[derive(Clone, Default)]
pub(crate) struct ComputedVolume {
    pub(crate) enabled: bool,
    pub(crate) offset: f32,
    pub(crate) colour: LinearRgba,
}

#[derive(Clone, Default)]
pub(crate) struct ComputedStencil {
    pub(crate) enabled: bool,
    pub(crate) offset: f32,
}

#[derive(Clone)]
pub(crate) struct ComputedMode {
    pub(crate) world_origin: Vec3,
    pub(crate) depth_mode: DepthMode,
}

impl Default for ComputedMode {
    fn default() -> Self {
        Self {
            world_origin: Vec3::NAN,
            depth_mode: DepthMode::Real,
        }
    }
}

#[derive(Copy, Clone, Default)]
pub(crate) enum Source {
    #[default]
    Set,
    Inherited,
}

#[derive(Clone, Default)]
pub(crate) struct Sourced<T: Clone + Default> {
    pub(crate) value: T,
    pub(crate) source: Source,
}

impl<T: Clone + Default> Sourced<T> {
    pub fn inherit(value: &T) -> Self {
        Sourced {
            value: value.clone(),
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

#[derive(Clone, Default)]
pub(crate) struct ComputedInternal {
    pub(crate) inherited_from: Option<Entity>,
    pub(crate) volume: Sourced<ComputedVolume>,
    pub(crate) stencil: Sourced<ComputedStencil>,
    pub(crate) mode: Sourced<ComputedMode>,
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
        let changed = update_computed_outline(&mut computed, components, &default(), None, false);
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
            parent_computed,
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

fn update_computed_outline(
    computed: &mut ComputedOutline,
    (visibility, transform, volume, stencil, mode): QueryItem<'_, OutlineComponents>,
    parent_computed: &ComputedInternal,
    parent_entity: Option<Entity>,
    force_update: bool,
) -> bool {
    let changed = force_update
        || if let ComputedOutline(Some(computed)) = computed {
            computed.inherited_from != parent_entity
                || visibility.is_changed()
                || (transform.is_changed()
                    && mode
                        .as_ref()
                        .map(|r| matches!(r.as_ref(), OutlineMode::FlatVertex { .. }))
                        .unwrap_or(false))
                || computed.volume.is_changed(&volume)
                || computed.stencil.is_changed(&stencil)
                || computed.mode.is_changed(&mode)
        } else {
            true
        };
    if changed {
        *computed = ComputedOutline(Some(ComputedInternal {
            inherited_from: parent_entity,
            volume: if let Some(vol) = volume {
                Sourced::set(ComputedVolume {
                    enabled: visibility.get() && vol.visible && !vol.colour.is_fully_transparent(),
                    offset: vol.width,
                    colour: vol.colour.into(),
                })
            } else {
                Sourced::inherit(&parent_computed.volume.value)
            },
            stencil: if let Some(sten) = stencil {
                Sourced::set(ComputedStencil {
                    enabled: visibility.get() && sten.enabled,
                    offset: sten.offset,
                })
            } else {
                Sourced::inherit(&parent_computed.stencil.value)
            },
            mode: if let Some(m) = mode {
                Sourced::set(match m.as_ref() {
                    OutlineMode::FlatVertex {
                        model_origin: origin,
                    } => ComputedMode {
                        world_origin: transform.compute_matrix().project_point3(*origin),
                        depth_mode: DepthMode::Flat,
                    },
                    OutlineMode::RealVertex => ComputedMode {
                        world_origin: Vec3::NAN,
                        depth_mode: DepthMode::Real,
                    },
                })
            } else {
                Sourced::inherit(&parent_computed.mode.value)
            },
        }));
    }
    changed
}
