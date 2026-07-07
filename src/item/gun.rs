use bevy::prelude::*;

use crate::IdleMovement;

use super::{Equipped, Item, ItemState, OnGround};

// The basic gun component.
#[derive(SceneComponent, Clone, Default)]
#[scene(GunProps)]
pub struct Gun(pub Item);

#[derive(Default)]
pub struct GunProps {
    state: ItemState,
}

impl Gun {
    fn scene(props: GunProps) -> impl Scene {
        let scene: Box<dyn Scene> = match props.state {
            ItemState::OnGround => Box::new(Gun::ground_scene()),
            ItemState::EquippedBy(_entity) => Box::new(Gun::equipped_scene()),
            ItemState::StoredIn(_entity) => Box::new(Gun::stored_scene()),
        };
        bsn! {
            #Gun
            scene
        }
    }

    fn ground_scene() -> impl Scene {
        bsn! {
            OnGround(Vec3::ZERO)
            Transform::default()
            IdleMovement
        }
    }

    fn equipped_scene() -> impl Scene {
        bsn! {
            Equipped
        }
    }

    fn stored_scene() -> impl Scene {
        bsn! {
            Transform::default()
        }
    }
}
