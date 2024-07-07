use std::{f32::consts::TAU, num::Wrapping, time::Duration};

use bevy::prelude::*;

use bevy_mod_outline::*;

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(Msaa::Sample4)
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((DefaultPlugins, OutlinePlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, (spawn_objects, move_objects, despawn_objects))
        .run();
}

#[derive(Resource)]
struct MyAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

#[derive(Component)]
struct FlyingObject;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(MyAssets {
        mesh: meshes.add(
            Capsule3d::new(1.0, 2.0)
                .mesh()
                .rings(10)
                .latitudes(15)
                .longitudes(15)
                .build(),
        ),
        material: materials.add(StandardMaterial::from(Color::srgb(0.5, 0.5, 0.5))),
    });

    // Add light source and camera
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 750.0,
            ..default()
        },
        ..default()
    });
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 0.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}

struct SpawnState(Timer, Wrapping<u64>);

impl Default for SpawnState {
    fn default() -> Self {
        let mut timer = Timer::from_seconds(0.75, TimerMode::Repeating);
        timer.tick(timer.duration() - Duration::from_nanos(1));
        Self(timer, Wrapping(0))
    }
}

fn spawn_objects(
    mut commands: Commands,
    mut timer: Local<SpawnState>,
    time: Res<Time>,
    assets: Res<MyAssets>,
) {
    timer.0.tick(time.delta());
    if timer.0.just_finished() {
        timer.1 *= 6364136223846793005;
        timer.1 += 1442695040888963407;
        let x = ((timer.1 .0 >> 40) as i8 as f32) / 128.0;
        let y = ((timer.1 .0 >> 32) as i8 as f32) / 128.0;
        let b = (timer.1 .0 >> 48) & 1 == 1;
        commands
            .spawn(PbrBundle {
                mesh: assets.mesh.clone(),
                material: assets.material.clone(),
                transform: Transform::from_rotation(Quat::from_axis_angle(
                    Vec3::new(1.0, 0.0, 0.0),
                    0.25 * TAU,
                ))
                .with_translation(Vec3::new(15.0 * x, 15.0 * y, 0.0)),
                ..default()
            })
            .insert(FlyingObject)
            .insert(OutlineBundle {
                outline: OutlineVolume {
                    visible: true,
                    width: if b { 10.0 } else { 5.0 },
                    colour: if b {
                        Color::srgb(0.0, 1.0, 0.0)
                    } else {
                        Color::srgb(1.0, 0.0, 0.0)
                    },
                },
                ..default()
            });
    }
}

fn move_objects(time: Res<Time>, mut query: Query<&mut Transform, With<FlyingObject>>) {
    for mut t in query.iter_mut() {
        t.translation += Vec3::new(0.0, 0.0, 5.0 * time.delta_seconds());
    }
}

fn despawn_objects(mut commands: Commands, query: Query<(Entity, &Transform), With<FlyingObject>>) {
    for (e, t) in query.iter() {
        if t.translation.z > 51.0 {
            commands.entity(e).despawn();
        }
    }
}
