use std::collections::HashMap;

use bevy::prelude::*;

use crate::{Chrome, ItemDefinition, ItemRegistry, ItemStateKind};

const GUN_COLOR: Color = Color::srgb(0.8, 0.8, 0.85);

pub struct GunViewPlugin;

impl Plugin for GunViewPlugin {
    fn build(&self, app: &mut App) {
        let world = app.world_mut();
        let ground_mesh = world
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::new(0.1, 0.2, 1.));
        let hand_mesh = world.resource_mut::<Assets<Mesh>>().add(Sphere::new(0.1));
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::from(GUN_COLOR));

        world.resource_mut::<ItemRegistry>().register(
            "core::item::gun",
            ItemDefinition {
                chrome: HashMap::from([
                    (
                        ItemStateKind::OnGround,
                        Chrome {
                            mesh: ground_mesh,
                            material: material.clone(),
                        },
                    ),
                    (
                        ItemStateKind::Equipped,
                        Chrome {
                            mesh: hand_mesh,
                            material,
                        },
                    ),
                ]),
            },
        );
    }
}
