use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use bevy_mod_outline::*;

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(Msaa::Sample4)
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((DefaultPlugins, OutlinePlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, (wobble, orbit))
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
    // Add cube with generated outline normals
    let mut cube_mesh = Cuboid::new(1.0, 1.0, 1.0).mesh().build();
    cube_mesh.generate_outline_normals().unwrap();
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(cube_mesh),
            material: materials.add(StandardMaterial::from(Color::srgb(0.1, 0.1, 0.9))),
            transform: Transform::from_xyz(0.0, 1.0, 0.0),
            ..default()
        })
        .insert(OutlineBundle {
            outline: OutlineVolume {
                visible: true,
                colour: Color::srgb(0.0, 1.0, 0.0),
                width: 25.0,
            },
            mode: OutlineMode::RealVertex,
            ..default()
        })
        .insert(Wobbles);

    // Add torus using the regular surface normals for outlining
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(
                Torus {
                    minor_radius: 0.1,
                    major_radius: 0.3,
                }
                .mesh()
                .minor_resolution(10)
                .major_resolution(20)
                .build(),
            ),
            material: materials.add(StandardMaterial::from(Color::srgb(0.9, 0.1, 0.1))),
            transform: Transform::from_xyz(0.0, 1.2, 2.0)
                .with_rotation(Quat::from_rotation_x(0.5 * PI)),
            ..default()
        })
        .insert(OutlineBundle {
            outline: OutlineVolume {
                visible: true,
                colour: Color::srgba(1.0, 0.0, 1.0, 0.3),
                width: 15.0,
            },
            ..default()
        })
        .insert(Orbits);

    // Add plane, light source, and camera
    commands.spawn(PbrBundle {
        mesh: meshes.add(Plane3d::new(Vec3::Y, Vec2::new(5.0, 5.0)).mesh().build()),
        material: materials.add(StandardMaterial::from(Color::srgb(0.3, 0.5, 0.3))),
        ..default()
    });
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });
    commands.spawn(Camera3dBundle {
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
