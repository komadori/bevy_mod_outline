use bevy::{
    pbr::wireframe::{Wireframe, WireframePlugin},
    prelude::*,
    state::state::FreelyMutableState,
};

use bevy_mod_outline::*;

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((DefaultPlugins, OutlinePlugin, WireframePlugin))
        .insert_state(DrawMethod::Extrude)
        .insert_state(DrawShape::Triangle)
        .init_resource::<Shapes>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                highlight::<DrawMethod>,
                highlight::<DrawShape>,
                interaction::<DrawMethod>,
                interaction::<DrawShape>,
                change_shape,
            ),
        )
        .add_systems(
            OnEnter(DrawMethod::JumpFlood),
            |mut query: Query<Entity, With<TheObject>>, mut commands: Commands| {
                commands
                    .entity(query.get_single().unwrap())
                    .insert(OutlineMode::FloodFlat);
            },
        )
        .add_systems(
            OnExit(DrawMethod::JumpFlood),
            |mut query: Query<Entity, With<TheObject>>, mut commands: Commands| {
                commands
                    .entity(query.get_single().unwrap())
                    .remove::<OutlineMode>();
            },
        )
        .run();
}

#[derive(Component)]
struct TheObject;

#[derive(Copy, Clone, States, Component, Debug, PartialEq, Eq, Hash)]
enum DrawMethod {
    Extrude,
    JumpFlood,
}

#[derive(Copy, Clone, States, Component, Debug, PartialEq, Eq, Hash)]
enum DrawShape {
    Triangle,
    Rectangle,
    Circle,
}

#[derive(Resource)]
struct Shapes {
    triangle: Handle<Mesh>,
    rectangle: Handle<Mesh>,
    circle: Handle<Mesh>,
}

impl Shapes {
    fn get(&self, shape: DrawShape) -> Handle<Mesh> {
        match shape {
            DrawShape::Triangle => self.triangle.clone(),
            DrawShape::Rectangle => self.rectangle.clone(),
            DrawShape::Circle => self.circle.clone(),
        }
    }
}

impl FromWorld for Shapes {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.get_resource_mut::<Assets<Mesh>>().unwrap();
        let settings = GenerateOutlineNormalsSettings::default().with_stretch_edges(true);
        Self {
            triangle: meshes.add(
                Triangle2d::new(
                    Vec2::new(0.0, 1.0),
                    Vec2::new(1.0, -1.0),
                    Vec2::new(-1.0, -1.0),
                )
                .mesh()
                .build()
                .with_generated_outline_normals(&settings)
                .unwrap(),
            ),
            rectangle: meshes.add(
                Rectangle::new(2.0, 2.0)
                    .mesh()
                    .build()
                    .with_generated_outline_normals(&settings)
                    .unwrap(),
            ),
            circle: meshes.add(
                Circle::new(1.0)
                    .mesh()
                    .build()
                    .with_generated_outline_normals(&settings)
                    .unwrap(),
            ),
        }
    }
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shapes: Res<Shapes>,
) {
    // Add shape
    commands.spawn((
        Mesh3d(shapes.triangle.clone()),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.5, 0.5, 0.5)))),
        OutlineVolume {
            visible: true,
            width: 25.0,
            colour: Color::srgb(1.0, 1.0, 0.0),
        },
        Wireframe,
        TheObject,
    ));

    // Add buttons
    commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|parent| {
                    for method in [DrawMethod::Extrude, DrawMethod::JumpFlood] {
                        parent
                            .spawn((
                                Button,
                                Node {
                                    margin: UiRect::all(Val::Px(5.0)),
                                    padding: UiRect::all(Val::Px(10.0)),
                                    border: UiRect::all(Val::Px(5.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BorderColor(Color::BLACK),
                                BorderRadius::MAX,
                                BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
                                method,
                            ))
                            .with_children(|parent| {
                                parent
                                    .spawn(Text::new(format!("{:?}", method)))
                                    .insert(TextColor(Color::WHITE));
                            });
                    }
                });
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|parent| {
                    for shape in [DrawShape::Triangle, DrawShape::Rectangle, DrawShape::Circle] {
                        parent
                            .spawn((
                                Button,
                                Node {
                                    margin: UiRect::all(Val::Px(5.0)),
                                    padding: UiRect::all(Val::Px(10.0)),
                                    border: UiRect::all(Val::Px(5.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BorderColor(Color::BLACK),
                                BorderRadius::MAX,
                                BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
                                shape,
                            ))
                            .with_children(|parent| {
                                parent
                                    .spawn(Text::new(format!("{:?}", shape)))
                                    .insert(TextColor(Color::WHITE));
                            });
                    }
                });
        });

    // Add light source and camera
    commands.spawn((PointLight::default(), Transform::from_xyz(4.0, 8.0, 4.0)));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        Msaa::Off,
    ));
}

fn highlight<T: Component + States>(
    mut query: Query<(&mut BorderColor, &T)>,
    state: Res<State<T>>,
) {
    for (mut border, m) in query.iter_mut() {
        *border = if m == state.get() {
            BorderColor(Color::srgb(0.0, 0.0, 1.0))
        } else {
            BorderColor(Color::BLACK)
        };
    }
}

fn interaction<T: Component + FreelyMutableState>(
    query: Query<(&Interaction, &T)>,
    mut armed: Local<bool>,
    mut state: ResMut<NextState<T>>,
) {
    let mut any_pressed = false;
    for (i, m) in query.iter() {
        if *i == Interaction::Pressed {
            any_pressed = true;
            if *armed {
                state.set(m.clone());
            }
        }
    }
    *armed = !any_pressed;
}

fn change_shape(
    mut reader: EventReader<StateTransitionEvent<DrawShape>>,
    mut query: Query<&mut Mesh3d, With<TheObject>>,
    shapes: Res<Shapes>,
) {
    for event in reader.read() {
        if let Ok(mut mesh) = query.get_single_mut() {
            mesh.0 = shapes.get(event.entered.unwrap());
        }
    }
}
