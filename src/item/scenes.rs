use bevy::prelude::*;

use super::ItemProps;
use crate::{EquippedBy, Gun, IdleMovement, Item, ItemKey, ItemLabel, ItemState, OnGround};

pub fn scene_for(props: &ItemProps) -> Option<Box<dyn Scene>> {
    let ItemProps { key, state } = props;
    let scene: Box<dyn Scene> = match key.0.as_str() {
        "core::item::gun" => Box::new(gun(state)),
        _ => return None,
    };
    Some(scene)
}

fn gun(state: &ItemState) -> impl Scene {
    let scene: Box<dyn Scene> = match state {
        ItemState::OnGround(pos) => {
            let pos = *pos;
            Box::new(bsn! {
                OnGround(pos)
                Transform::from_xyz(pos.x, pos.y, pos.z)
                IdleMovement
                Mesh3d(asset_value(Cuboid::new(0.1, 0.2, 1.)))
                MeshMaterial3d<StandardMaterial>
            })
        }
        ItemState::EquippedBy(entity) => {
            let entity = *entity;
            Box::new(bsn! {
                EquippedBy(entity)
                Mesh3d(asset_value(Sphere::new(0.1)))
                MeshMaterial3d<StandardMaterial>
            })
        }
        ItemState::StoredIn(_entity) => Box::new(bsn! {
            Transform::default()
        }),
    };
    let key = ItemKey("core::item::gun".to_string());
    let label = ItemLabel("Gun".to_string());
    bsn! {
        Item { key, label}
        scene
        Gun
    }
}
