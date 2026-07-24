use std::collections::HashMap;

use bevy::prelude::*;

use crate::{
    EquippedBy, ItemDefinition, ItemIcon, ItemRegistry, ItemStateMarker, OnGround, StoredIn,
    build_chrome_patch,
};

const GUN_COLOR: Color = Color::srgb(0.8, 0.8, 0.85);
const ICON_COLOR: Color = Color::WHITE;
const ICON_BORDER_COLOR: Color = Color::BLACK;
const ICON_BORDER_PX: f32 = 2.0;

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

        let ground_material = material.clone();
        let ground = build_chrome_patch(
            world,
            bsn! {
                Mesh3d(ground_mesh)
                MeshMaterial3d<StandardMaterial>(ground_material)
            },
        );
        let equipped = build_chrome_patch(
            world,
            bsn! {
                Mesh3d(hand_mesh)
                MeshMaterial3d<StandardMaterial>(material)
            },
        );
        let icon_color = ICON_COLOR;
        let icon_border = UiRect::all(Val::Px(ICON_BORDER_PX));
        let icon_border_color = ICON_BORDER_COLOR;
        let stored = build_chrome_patch(
            world,
            bsn! {
                Node { border: icon_border }
                BackgroundColor(icon_color)
                BorderColor {
                    top: icon_border_color,
                    right: icon_border_color,
                    bottom: icon_border_color,
                    left: icon_border_color,
                }
                ItemIcon
            },
        );

        world.resource_mut::<ItemRegistry>().register(
            "core::item::gun",
            ItemDefinition {
                chrome: HashMap::from([
                    (OnGround::KEY, ground),
                    (EquippedBy::KEY, equipped),
                    (StoredIn::KEY, stored),
                ]),
            },
        );
    }
}
