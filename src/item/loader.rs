use bevy::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ItemDef {
    pub key: String,
    pub label: String,
}
