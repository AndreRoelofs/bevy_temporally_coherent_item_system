use std::collections::HashMap;

use bevy::prelude::*;
use bevy::scene::ScenePatch;

use super::{Item, ItemState};

#[derive(Default)]
pub struct ItemDefinition {
    pub chrome: HashMap<ItemState, Handle<ScenePatch>>,
}

#[derive(Resource, Default)]
pub struct ItemRegistry {
    definitions: HashMap<String, ItemDefinition>,
}

impl ItemRegistry {
    pub fn register(&mut self, key: impl Into<String>, definition: ItemDefinition) -> &mut Self {
        self.definitions.insert(key.into(), definition);
        self
    }

    pub fn chrome(&self, model: EntityRef, state: ItemState) -> Option<&Handle<ScenePatch>> {
        let item = model.get::<Item>()?;
        let Some(definition) = self.definitions.get(&item.key.0) else {
            warn!("no item definition registered for key `{}`", item.key.0);
            return None;
        };
        definition.chrome.get(&state)
    }
}

pub fn build_chrome_patch(world: &mut World, scene: impl Scene) -> Handle<ScenePatch> {
    let asset_server = world.resource::<AssetServer>().clone();
    let mut patch = ScenePatch::load(&asset_server, scene);
    let mut patches = world.resource_mut::<Assets<ScenePatch>>();
    patch
        .resolve(&asset_server, &patches)
        .expect("chrome scenes have no asset-path dependencies");
    patches.add(patch)
}
