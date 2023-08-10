use std::f32::consts::PI;

use bevy::{
    prelude::{shape::Plane, *},
    scene::SceneInstance,
    window::close_on_esc,
};
use bevy_mod_outline::{
    AutoGenerateOutlineNormalsPlugin, OutlineBundle, OutlinePlugin, OutlineVolume,
};

#[derive(Resource)]
struct Fox(Handle<AnimationClip>);

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            OutlinePlugin,
            AutoGenerateOutlineNormalsPlugin,
        ))
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 1.0,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (setup_scene_once_loaded, close_on_esc))
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Insert a resource with the current animation
    commands.insert_resource(Fox(asset_server.load("Fox.glb#Animation0")));

    // Camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(100.0, 100.0, 150.0)
            .looking_at(Vec3::new(0.0, 20.0, 0.0), Vec3::Y),
        ..default()
    });

    // Plane
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(Plane {
            size: 500000.0,
            subdivisions: 0,
        })),
        material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
        ..default()
    });

    // Light
    commands.spawn(DirectionalLightBundle {
        transform: Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, 1.0, -PI / 4.)),
        directional_light: DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        ..default()
    });

    // Fox
    commands.spawn(SceneBundle {
        scene: asset_server.load("Fox.glb#Scene0"),
        ..default()
    });
}

// Once the scene is loaded, start the animation and add an outline
fn setup_scene_once_loaded(
    mut commands: Commands,
    scene_query: Query<&SceneInstance>,
    scene_manager: Res<SceneSpawner>,
    mut player_query: Query<&mut AnimationPlayer>,
    animation: Res<Fox>,
    mut done: Local<bool>,
) {
    if !*done {
        if let (Ok(scene), Ok(mut player)) =
            (scene_query.get_single(), player_query.get_single_mut())
        {
            if scene_manager.instance_is_ready(**scene) {
                for entity in scene_manager.iter_instance_entities(**scene) {
                    commands.entity(entity).insert(OutlineBundle {
                        outline: OutlineVolume {
                            visible: true,
                            width: 3.0,
                            colour: Color::RED,
                        },
                        ..default()
                    });
                }
                player.play(animation.0.clone_weak()).repeat();
                *done = true;
            }
        }
    }
}
