use std::collections::HashMap;

use bevy::prelude::*;

use super::{Item, ItemStateKind};

/// What an item looks like in one state: pure appearance data. Placement is
/// structural (ground views hang off the model, equipped views off the
/// holder's socket) and behavior lives in components, so neither is here.
pub struct Chrome {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

/// Everything data-driven about an item kind. This is the content a future
/// `.bsn` asset file provides; for now view plugins build it in code, once,
/// so view rebuilds reuse the same handles instead of minting new assets.
#[derive(Default)]
pub struct ItemDefinition {
    /// Chrome per state. No entry means no view (e.g. `Stored`).
    pub chrome: HashMap<ItemStateKind, Chrome>,
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
    pub fn chrome(&self, model: EntityRef, kind: ItemStateKind) -> Option<&Chrome> {
        let item = model.get::<Item>()?;
        let Some(definition) = self.definitions.get(&item.key.0) else {
            warn!("no item definition registered for key `{}`", item.key.0);
            return None;
        };
        definition.chrome.get(&kind)
    }
}
