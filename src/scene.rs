use bevy::{prelude::*, scene::SceneInstance};

use crate::InheritOutlineBundle;

/// A component for triggering the `AsyncSceneInheritOutlinePlugin`.
#[derive(Component)]
pub struct AsyncSceneInheritOutline;

fn process_async_scene_outline(
    mut commands: Commands,
    scene_spawner: Res<SceneSpawner>,
    async_query: Query<(Entity, &SceneInstance), With<AsyncSceneInheritOutline>>,
) {
    for (entity, instance) in async_query.iter() {
        if scene_spawner.instance_is_ready(**instance) {
            for child in scene_spawner.iter_instance_entities(**instance) {
                if let Some(mut ecmds) = commands.get_entity(child) {
                    ecmds.insert(InheritOutlineBundle::default());
                }
            }
            commands.entity(entity).remove::<AsyncSceneInheritOutline>();
        }
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
            Update,
            process_async_scene_outline.run_if(any_with_component::<AsyncSceneInheritOutline>),
        );
    }
}
