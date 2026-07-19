use bevy::prelude::*;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct EffectiveStats {
    pub cooldown_secs: f32,
}

pub struct StatFold {
    pub cooldown_mult: f32,
}

impl Default for StatFold {
    fn default() -> Self {
        Self { cooldown_mult: 1.0 }
    }
}

pub type StatModifierFn = fn(EntityRef, &mut StatFold);

#[derive(Resource, Default)]
pub struct StatModifierRegistry {
    modifiers: Vec<StatModifierFn>,
}

impl StatModifierRegistry {
    pub fn register(&mut self, modifier: StatModifierFn) -> &mut Self {
        self.modifiers.push(modifier);
        self
    }

    pub fn fold(&self, model: EntityRef) -> StatFold {
        let mut fold = StatFold::default();
        for modifier in &self.modifiers {
            modifier(model, &mut fold);
        }
        fold
    }
}

#[derive(Component, Default)]
pub struct StatsDirty;
