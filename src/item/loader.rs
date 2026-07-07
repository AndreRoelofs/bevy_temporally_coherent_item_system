use bevy::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ItemKey(pub String);

#[derive(Deserialize)]
pub struct ItemLabel(pub String);

#[derive(Component)]
pub struct Item {
    pub key: ItemKey,
    pub label: ItemLabel,
}

#[derive(Deserialize)]
pub struct ItemDef {
    pub key: String,
    pub label: String,
}
