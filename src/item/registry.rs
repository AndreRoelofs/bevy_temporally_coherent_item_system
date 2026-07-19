use std::collections::HashMap;

use bevy::prelude::*;
use bevy::scene::ScenePatch;

use super::{Item, ItemStateKind};

/// Everything data-driven about an item kind. Chrome is a `bsn!` scene per
/// state, stored as a resolved [`ScenePatch`] asset so every view rebuild
/// reapplies the same patch instead of minting new assets. Built once in
/// code today; when the official `.bsn` file loader ships, each entry
/// becomes `asset_server.load("items/gun.bsn")`.
#[derive(Default)]
pub struct ItemDefinition {
    /// Chrome per state. No entry means no view (e.g. `Stored`).
    pub chrome: HashMap<ItemStateKind, Handle<ScenePatch>>,
}

/// Maps item keys to their definitions. Keys are strings so entries can
/// eventually come from data instead of code.
#[derive(Resource, Default)]
pub struct ItemRegistry {
    definitions: HashMap<String, ItemDefinition>,
}

impl ItemRegistry {
    pub fn register(&mut self, key: impl Into<String>, definition: ItemDefinition) -> &mut Self {
        self.definitions.insert(key.into(), definition);
        self
    }

    /// The chrome for a model in a given state, or `None` when the state
    /// has no visual presence or the key is unregistered (logged: an
    /// unregistered key is a wiring error, an absent state entry is not).
    pub fn chrome(&self, model: EntityRef, kind: ItemStateKind) -> Option<&Handle<ScenePatch>> {
        let item = model.get::<Item>()?;
        let Some(definition) = self.definitions.get(&item.key.0) else {
            warn!("no item definition registered for key `{}`", item.key.0);
            return None;
        };
        definition.chrome.get(&kind)
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
