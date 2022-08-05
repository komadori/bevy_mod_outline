use std::f32::consts::{PI, TAU};

use bevy::prelude::{shape::Torus, *};

use bevy_mod_outline::*;

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(DefaultPlugins)
        .add_plugin(OutlinePlugin)
        .add_startup_system(setup)
        .add_system(wobble)
        .add_system(orbit)
        .run();
}

#[derive(Component)]
struct Wobbles;

#[derive(Component)]
struct Orbits;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(Mesh::from(bevy::prelude::shape::Plane { size: 5.0 })),
        material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
        ..default()
    });
    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(Torus {
                radius: 0.6,
                ring_radius: 0.2,
                subdivisions_segments: 20,
                subdivisions_sides: 10,
            })),
            material: materials.add(Color::rgb(0.1, 0.1, 0.9).into()),
            transform: Transform::from_xyz(0.0, 1.0, 0.0),
            ..default()
        })
        .insert(Outline {
            colour: Color::rgba(0.0, 1.0, 0.0, 1.0),
            width: 25.0,
        })
        .insert(OutlineStencil)
        .insert(Wobbles);
    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(Torus {
                radius: 0.3,
                ring_radius: 0.1,
                subdivisions_segments: 20,
                subdivisions_sides: 10,
            })),
            material: materials.add(Color::rgb(0.9, 0.1, 0.1).into()),
            transform: Transform::from_xyz(0.0, 1.2, 2.0)
                .with_rotation(Quat::from_rotation_x(0.5 * PI)),
            ..default()
        })
        .insert(Outline {
            colour: Color::rgba(1.0, 0.0, 1.0, 0.3),
            width: 15.0,
        })
        .insert(OutlineStencil)
        .insert(Orbits);
    commands.spawn_bundle(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });
    commands.spawn_bundle(Camera3dBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}

fn wobble(mut query: Query<&mut Transform, With<Wobbles>>, timer: Res<Time>, mut t: Local<f32>) {
    let ta = *t;
    *t = (ta + 0.5 * timer.delta_seconds()) % TAU;
    let tb = *t;
    let i1 = tb.cos() - ta.cos();
    let i2 = ta.sin() - tb.sin();
    for mut transform in query.iter_mut() {
        transform.rotate(Quat::from_rotation_z(
            TAU * 20.0 * i1 * timer.delta_seconds(),
        ));
        transform.rotate(Quat::from_rotation_y(
            TAU * 20.0 * i2 * timer.delta_seconds(),
        ));
    }
}

fn orbit(mut query: Query<&mut Transform, With<Orbits>>, timer: Res<Time>) {
    for mut transform in query.iter_mut() {
        transform.translate_around(
            Vec3::ZERO,
            Quat::from_rotation_y(0.4 * timer.delta_seconds()),
        )
    }
}
