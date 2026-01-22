use std::f32::consts::PI;

use bevy::{prelude::*, scene::SceneInstanceReady};
use bevy_mod_outline::{
    AsyncSceneInheritOutline, AutoGenerateOutlineNormalsPlugin, OutlinePlugin, OutlineVolume,
};

const FOX_PATH: &str = "Fox.glb";

#[derive(Component)]
struct FoxAnimation {
    graph: Handle<AnimationGraph>,
    index: AnimationNodeIndex,
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            OutlinePlugin,
            AutoGenerateOutlineNormalsPlugin::default(),
        ))
        .insert_resource(GlobalAmbientLight::default())
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(100.0, 100.0, 150.0).looking_at(Vec3::new(0.0, 20.0, 0.0), Vec3::Y),
    ));

    let (graph, index) = AnimationGraph::from_clip(
        asset_server.load(GltfAssetLabel::Animation(0).from_asset(FOX_PATH)),
    );

    let graph_handle = graphs.add(graph);

    let fox_anim = FoxAnimation {
        graph: graph_handle,
        index: index,
    };

    let fox_scene = SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(FOX_PATH)));

    // Fox
    commands
        .spawn((
            fox_anim,
            fox_scene,
            OutlineVolume {
                visible: true,
                width: 3.0,
                colour: Color::srgb(1.0, 0.0, 0.0),
            },
            AsyncSceneInheritOutline::default(),
            Transform::default(),
        ))
        .observe(play_anim_when_ready);

    // Plane
    commands.spawn((
        Mesh3d(
            meshes.add(
                Plane3d::new(Vec3::Y, Vec2::new(500000.0, 500000.0))
                    .mesh()
                    .build(),
            ),
        ),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.3, 0.5, 0.3)))),
    ));

    // Light
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, 1.0, -PI / 4.)),
    ));
}

fn play_anim_when_ready(
    scene_ready: On<SceneInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    anims_query: Query<&FoxAnimation>,
    mut players: Query<&mut AnimationPlayer>,
) {
    if let Ok(anim) = anims_query.get(scene_ready.entity) {
        for child in children.iter_descendants(scene_ready.entity) {
            if let Ok(mut player) = players.get_mut(child) {
                player.play(anim.index).repeat();
                commands
                    .entity(child)
                    .insert(AnimationGraphHandle(anim.graph.clone()));
            }
        }
    }
}
