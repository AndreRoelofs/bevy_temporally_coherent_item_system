use bevy::prelude::*;

use crate::Equipped;

use super::ItemKey;

pub fn scene_for(key: &ItemKey) -> Option<Box<dyn Scene>> {
    let scene: Box<dyn Scene> = match key.0.as_str() {
        "core::item::gun" => Box::new(gun()),
        _ => return None,
    };
    Some(scene)
}

fn gun() -> impl Scene {
    bsn! {
        Equipped
    }
}
