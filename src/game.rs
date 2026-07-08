use bevy::prelude::*;

use crate::{
    CursorLocked, EYE_HEIGHT, ItemKey, ItemProps, ItemState, PLATFORM_HALF, PLATFORM_THICKNESS,
    PLATFORM_TOP_Y, Player, look_around, scene_for, toggle_cursor, update_player,
};

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CursorLocked::default())
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    toggle_cursor,
                    look_around,
                    update_player,
                    spawn_gun.run_if(run_once),
                ),
            );
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(
            PLATFORM_HALF * 2.0,
            PLATFORM_THICKNESS,
            PLATFORM_HALF * 2.0,
        ))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.3, 0.9),
            ..default()
        })),
        Transform::from_xyz(0.0, PLATFORM_TOP_Y - PLATFORM_THICKNESS * 0.5, 0.0),
    ));

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, PLATFORM_TOP_Y + EYE_HEIGHT, 0.0),
            Player::default(),
            AmbientLight {
                brightness: 200.0,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.15, 0.15, 0.6))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.8, 0.6, 0.4),
                    unlit: true,
                    ..default()
                })),
                Transform::from_xyz(0.3, -0.3, -0.5),
            ));
        });
}

fn spawn_gun(world: &mut World) {
    let _ = world.spawn_scene(scene_for(&ItemProps {
        key: ItemKey("core::item::gun".to_string()),
        // Spawn on the ground in front of the player
        state: ItemState::OnGround(Vec3::new(0.0, 0.0, -5.0)),
    }));
}
