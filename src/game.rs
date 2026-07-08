use bevy::prelude::*;
use bevy::scene::SceneComponentInfo;

use crate::{
    CursorLocked, EYE_HEIGHT, Item, ItemKey, ItemProps, ItemState, PLATFORM_HALF,
    PLATFORM_THICKNESS, PLATFORM_TOP_Y, Player, look_around, scene_for, toggle_cursor,
    update_player,
};

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CursorLocked::default())
            .insert_resource(GunEntity::default())
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    toggle_cursor,
                    look_around,
                    update_player,
                    regenerate_item_scene,
                    pickup_gun,
                    spawn_gun.run_if(run_once),
                    update_gun_id_text,
                    update_gun_components_text,
                ),
            );
    }
}

const PICKUP_RANGE: f32 = 1.5;
const HAND_OFFSET: Vec3 = Vec3::new(-0.3, -0.3, -0.6);

#[derive(Resource, Default)]
pub struct GunEntity(pub Option<Entity>);

#[derive(Component, Clone)]
pub struct CurrentItemState(pub ItemState);

impl Default for CurrentItemState {
    fn default() -> Self {
        Self(ItemState::default())
    }
}

#[derive(Component)]
struct GunIdText;

#[derive(Component)]
struct GunComponentsText;

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

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        Text::new("gun: <none>"),
        TextFont {
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(Color::WHITE),
        GunIdText,
    ));

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(40.0),
            left: Val::Px(10.0),
            ..default()
        },
        Text::new("components:"),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::WHITE),
        GunComponentsText,
    ));
}

fn spawn_gun(world: &mut World) {
    let initial_state = ItemState::OnGround(Vec3::new(0.0, 0.0, -5.0));
    let entity = world
        .spawn_scene(scene_for(&ItemProps {
            key: ItemKey("core::item::gun".to_string()),
            state: initial_state.clone(),
        }))
        .expect("spawn gun")
        .id();
    world
        .entity_mut(entity)
        .insert(CurrentItemState(initial_state));
    world.resource_mut::<GunEntity>().0 = Some(entity);
}

fn regenerate_item_scene(world: &mut World) {
    let dirty: Vec<Entity> = world
        .query_filtered::<Entity, Changed<CurrentItemState>>()
        .iter(world)
        .collect();

    for entity in dirty {
        let Some((key, state)) = world.entity(entity).get::<Item>().and_then(|item| {
            world
                .entity(entity)
                .get::<CurrentItemState>()
                .map(|state| (item.key.clone(), state.0.clone()))
        }) else {
            continue;
        };

        let mut item = world.entity_mut(entity);

        item.retain::<(Item, SceneComponentInfo, CurrentItemState)>();

        if let Some(scene) = scene_for(&ItemProps {
            key,
            state: state.clone(),
        }) {
            let _ = item.apply_scene(scene);
        }

        match state {
            ItemState::EquippedBy(player) => {
                item.insert(Transform::from_translation(HAND_OFFSET));
                world.entity_mut(player).add_child(entity);
            }
            ItemState::OnGround(_) => {
                item.remove::<ChildOf>();
            }
            ItemState::StoredIn(_) => {}
        }
    }
}

fn pickup_gun(
    player: Query<(Entity, &Transform), (With<Player>, With<Camera3d>)>,
    mut guns: Query<&mut CurrentItemState>,
) {
    let Ok((player_e, player_t)) = player.single() else {
        return;
    };

    for mut state in &mut guns {
        let ItemState::OnGround(pos) = &state.0 else {
            continue;
        };
        let dx = player_t.translation.x - pos.x;
        let dz = player_t.translation.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist < PICKUP_RANGE {
            state.0 = ItemState::EquippedBy(player_e);
        }
    }
}

fn update_gun_id_text(gun: Res<GunEntity>, mut text: Query<&mut Text, With<GunIdText>>) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    match gun.0 {
        Some(entity) => text.0 = format!("gun: {entity:?}"),
        None => text.0 = "gun: <none>".to_string(),
    }
}

fn update_gun_components_text(world: &mut World) {
    let mut text_query = world.query_filtered::<Entity, With<GunComponentsText>>();
    let Some(text_entity) = text_query.iter(world).next() else {
        return;
    };

    let gun = world.resource::<GunEntity>();
    let Some(entity) = gun.0 else {
        world.entity_mut(text_entity).get_mut::<Text>().unwrap().0 =
            "components: <none>".to_string();
        return;
    };

    let Ok(ent) = world.get_entity(entity) else {
        world.entity_mut(text_entity).get_mut::<Text>().unwrap().0 =
            "components: <despawned>".to_string();
        return;
    };

    let components = world.components();
    let names: Vec<String> = ent
        .archetype()
        .components()
        .iter()
        .map(|&id| {
            components
                .get_name(id)
                .map(|n| n.to_string())
                .unwrap_or_default()
        })
        .collect();

    world.entity_mut(text_entity).get_mut::<Text>().unwrap().0 =
        format!("components: [{}]", names.join(", "));
}
