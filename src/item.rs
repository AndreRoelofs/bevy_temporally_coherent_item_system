use bevy::prelude::*;
use serde::Deserialize;

mod components;
mod gun;
mod loader;
mod scenes;

pub use components::*;
pub use gun::*;
pub use loader::*;
pub use scenes::*;

#[derive(Deserialize, Default, Clone)]
pub struct ItemKey(pub String);

#[derive(Deserialize, Default, Clone)]
pub struct ItemLabel(pub String);

pub enum ItemState {
    OnGround(Vec3),
    EquippedBy(Entity),
    StoredIn(Entity),
}

impl Default for ItemState {
    fn default() -> Self {
        Self::OnGround(Vec3::ZERO)
    }
}

#[derive(Component, Default, Clone)]
pub struct Item {
    pub key: ItemKey,
    pub label: ItemLabel,
}
