use bevy::prelude::*;
use bevy_mod_outline::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MeshPickingPlugin, OutlinePlugin))
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut window: Query<Entity, With<Window>>,
) {
    const X: f32 = -5.0;
    const Z: f32 = 0.0;

    let cube = meshes.add(Cuboid::default());
    let material = materials.add(StandardMaterial::default());

    for i in -5..5 {
        let y = i as f32 * 1.1;
        
        //Spawn object with outline and observer
        commands
            .spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(X, y, Z),
                OutlineVolume::default(),
                OutlineMode::FloodFlat,
                Pickable::default(),
            ))
            .observe(on_click);
    }

    //Add an observer to the window
    if let Ok(entity) = window.single_mut() {
        if let Ok(mut window) = commands.get_entity(entity) {
            window.observe(on_click);
        }
    }

    //Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-18.0, 15.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn on_click(
    event: Trigger<Pointer<Click>>,
    mut query: Query<&mut OutlineVolume>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    fn deselect_all(query: &mut Query<&mut OutlineVolume>) {
        for mut outline in query.iter_mut() {
            outline.visible = false;
        }
    }

    //Deselect everything if there is no target
    if event.target == Entity::PLACEHOLDER {
        deselect_all(&mut query);
        return;
    };

    //Deselect everything if the shift key is not held down
    if !(keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)) {
        deselect_all(&mut query);
    }

    //Select the target mesh
    if let Ok(mut outline) = query.get_mut(event.target) {
        outline.visible = true;
    }
}
