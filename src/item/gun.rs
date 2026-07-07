use bevy::prelude::*;

use super::{ItemState, OnGround};

// The basic gun component.
#[derive(SceneComponent, Clone, Default)]
#[scene(GunProps)]
pub struct Gun;

#[derive(Default)]
pub struct GunProps {
    state: ItemState,
}

impl Gun {
    fn scene(props: GunProps) -> impl Scene {
        let scene = match props.state {
            ItemState::OnGround => Box::new(Gun::ground_scene()),
            ItemState::EquippedBy(_entity) => Box::new(Gun::ground_scene()),
            ItemState::StoredIn(_entity) => Box::new(Gun::ground_scene()),
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
        }
    }
}
