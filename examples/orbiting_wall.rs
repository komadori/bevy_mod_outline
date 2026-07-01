use bevy::prelude::*;
use bevy_mod_outline::*;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.15, 0.15, 0.2)))
        .add_plugins((DefaultPlugins, OutlinePlugin::JUMP_FLOOD))
        .add_systems(Startup, setup)
        .add_systems(Update, orbit_camera)
        .run();
}

// Point the camera orbits around and looks at.
const TARGET: Vec3 = Vec3::new(0.0, 1.5, -10.0);

fn orbit_camera(time: Res<Time>, mut query: Query<&mut Transform, With<Camera3d>>) {
    let angle = 0.5 * time.elapsed_secs();
    let radius = 12.0;
    for mut transform in &mut query {
        let position = TARGET + Vec3::new(radius * angle.sin(), 1.5, radius * angle.cos());
        *transform = Transform::from_translation(position).looking_at(TARGET, Vec3::Y);
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.5, 4.0, 30.0).mesh().build())),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.5, 0.1, 0.1)))),
        Transform::from_xyz(1.5, 2.0, -10.0),
        OutlineVolume {
            visible: true,
            colour: Color::srgb(0.0, 1.0, 0.0),
            width: 8.0,
        },
    ));

    // Ground
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::new(40.0, 40.0)).mesh().build())),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.6, 0.6, 0.65)))),
    ));
    commands.spawn((
        PointLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 2.0),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.0, 0.0).looking_at(TARGET, Vec3::Y),
        Msaa::Off,
    ));
}
