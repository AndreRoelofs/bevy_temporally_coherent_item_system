use bevy::prelude::*;

use crate::{Equipped, IdleMovement, ItemState, OnGround};

use super::ItemKey;

pub fn scene_for(key: &ItemKey, state: &ItemState) -> Option<Box<dyn Scene>> {
    let scene: Box<dyn Scene> = match key.0.as_str() {
        "core::item::gun" => Box::new(gun(state)),
        _ => return None,
    };
    Some(scene)
}

#[derive(Bundle)]
pub struct GunEquippedBundle {
    equipped: Equipped,
}

fn gun(state: &ItemState) -> impl Scene {
    let scene: Box<dyn Scene> = match state {
        ItemState::OnGround(pos) => {
            let pos = *pos;
            Box::new(bsn! {
                OnGround(pos)
                Transform::default()
                IdleMovement
            })
        }
        ItemState::EquippedBy(_entity) => Box::new(bsn! {
            Equipped
        }),
        ItemState::StoredIn(_entity) => Box::new(bsn! {
            Transform::default()
        }),
    };
    bsn! {
        #Gun
        scene
    }
}
