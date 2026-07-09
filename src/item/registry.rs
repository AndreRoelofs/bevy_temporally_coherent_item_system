use std::collections::HashMap;

use bevy::prelude::*;

use super::Item;

/// Builds the view scene for a model entity. Reading the model through
/// `EntityRef` means a scene can react to any component on it (state, rust,
/// enchantments, ...) without the registry knowing about them.
pub type ViewSceneFn = fn(EntityRef) -> Option<Box<dyn Scene>>;

/// Maps item keys to view-scene builders. Keys are strings so entries can
/// eventually come from data (bsn files) instead of code.
#[derive(Resource, Default)]
pub struct ItemRegistry {
    scenes: HashMap<String, ViewSceneFn>,
}

impl ItemRegistry {
    pub fn register(&mut self, key: impl Into<String>, scene: ViewSceneFn) -> &mut Self {
        self.scenes.insert(key.into(), scene);
        self
    }

    pub fn view_scene(&self, model: EntityRef) -> Option<Box<dyn Scene>> {
        let item = model.get::<Item>()?;
        let Some(build) = self.scenes.get(&item.key.0) else {
            warn!("no view scene registered for item key `{}`", item.key.0);
            return None;
        };
        build(model)
    }
}
