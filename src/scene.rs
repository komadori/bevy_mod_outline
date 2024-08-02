use bevy::{
    prelude::*,
    scene::{SceneInstance, SceneInstanceReady},
};

use crate::{compute_outline, InheritOutlineBundle};

/// A component for triggering the `AsyncSceneInheritOutlinePlugin`.
#[derive(Component)]
pub struct AsyncSceneInheritOutline;

/// A component marking that `AsyncSceneInheritOutlinePlugin` has processed a scene.
#[derive(Component)]
pub struct AsyncSceneInheritOutlineProcessed;

fn instance_maybe_ready(
    mut commands: Commands,
    scene_spawner: &SceneSpawner,
    entity: Entity,
    instance: &SceneInstance,
) {
    if scene_spawner.instance_is_ready(**instance) {
        for child in scene_spawner.iter_instance_entities(**instance) {
            commands
                .entity(child)
                .insert(InheritOutlineBundle::default());
        }
        commands
            .entity(entity)
            .insert(AsyncSceneInheritOutlineProcessed);
    }
}

// Handles scenes which are already ready when `AsyncSceneInheritOutline` is added.
#[allow(clippy::type_complexity)]
fn async_added(
    mut commands: Commands,
    scene_spawner: Res<SceneSpawner>,
    async_query: Query<
        (Entity, &SceneInstance),
        (
            Added<AsyncSceneInheritOutline>,
            Without<AsyncSceneInheritOutlineProcessed>,
        ),
    >,
) {
    for (entity, instance) in async_query.iter() {
        instance_maybe_ready(commands.reborrow(), &scene_spawner, entity, instance);
    }
}

// Handles scenes which become ready after `AsyncSceneInheritOutline` is added.
#[allow(clippy::type_complexity)]
fn async_ready_event(
    mut commands: Commands,
    mut ready_events: EventReader<SceneInstanceReady>,
    scene_spawner: Res<SceneSpawner>,
    async_query: Query<
        &SceneInstance,
        (
            With<AsyncSceneInheritOutline>,
            Without<AsyncSceneInheritOutlineProcessed>,
        ),
    >,
) {
    for event in ready_events.read() {
        if let Ok(instance) = async_query.get(event.parent) {
            instance_maybe_ready(commands.reborrow(), &scene_spawner, event.parent, instance);
        }
    }
}

// Handles cleaning when `AsyncSceneInheritOutline` is removed.
#[allow(clippy::type_complexity)]
fn async_removed(
    mut commands: Commands,
    scene_spawner: Res<SceneSpawner>,
    async_query: Query<
        (Entity, &SceneInstance),
        (
            Without<AsyncSceneInheritOutline>,
            With<AsyncSceneInheritOutlineProcessed>,
        ),
    >,
) {
    for (entity, instance) in async_query.iter() {
        for child in scene_spawner.iter_instance_entities(**instance) {
            commands.entity(child).remove::<InheritOutlineBundle>();
        }
        commands
            .entity(entity)
            .remove::<AsyncSceneInheritOutlineProcessed>();
    }
}

/// Automatically inherits outlines for the entities in a scene.
///
/// Once a `SceneInstance` marked with `AsyncSceneInheritOutline` is ready, this plugin will add
/// `InheritOutlineBundle` to all of its entities and then remove the marker component.
pub struct AsyncSceneInheritOutlinePlugin;

impl Plugin for AsyncSceneInheritOutlinePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (
                (async_added, async_ready_event).run_if(
                    |query: Query<
                        (),
                        (
                            With<AsyncSceneInheritOutline>,
                            Without<AsyncSceneInheritOutlineProcessed>,
                        ),
                    >| !query.is_empty(),
                ),
                async_removed.run_if(
                    |query: Query<
                        (),
                        (
                            Without<AsyncSceneInheritOutline>,
                            With<AsyncSceneInheritOutlineProcessed>,
                        ),
                    >| !query.is_empty(),
                ),
            )
                .before(compute_outline),
        );
    }
}
