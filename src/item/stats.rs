//! Placeholder

use std::any::{TypeId, type_name};
use std::marker::PhantomData;

use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatOp {
    Flat(f32),
    Mult(f32),
}

struct StatEntry {
    source: TypeId,
    source_name: &'static str,
    op: StatOp,
}

#[derive(Component)]
pub struct StatModifiers<S: Send + Sync + 'static> {
    entries: Vec<StatEntry>,
    _kind: PhantomData<S>,
}

impl<S: Send + Sync + 'static> Default for StatModifiers<S> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            _kind: PhantomData,
        }
    }
}

impl<S: Send + Sync + 'static> StatModifiers<S> {
    pub fn set<Source: Component>(&mut self, op: StatOp) {
        let source = TypeId::of::<Source>();
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.source == source) {
            entry.op = op;
        } else {
            self.entries.push(StatEntry {
                source,
                source_name: type_name::<Source>(),
                op,
            });
        }
    }

    pub fn remove<Source: Component>(&mut self) {
        let source = TypeId::of::<Source>();
        self.entries.retain(|entry| entry.source != source);
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn apply_to(&self, base: f32) -> f32 {
        let flat: f32 = self
            .entries
            .iter()
            .filter_map(|entry| match entry.op {
                StatOp::Flat(value) => Some(value),
                StatOp::Mult(_) => None,
            })
            .sum();
        let mult: f32 = self
            .entries
            .iter()
            .filter_map(|entry| match entry.op {
                StatOp::Mult(value) => Some(value),
                StatOp::Flat(_) => None,
            })
            .product();
        (base + flat) * mult
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cooldown(pub f32);

impl Cooldown {
    pub fn effective(self, modifiers: Option<&CooldownModifiers>) -> Cooldown {
        Cooldown(modifiers.map_or(self.0, |m| m.apply_to(self.0)).max(0.0))
    }
}

pub type CooldownModifiers = StatModifiers<Cooldown>;

pub trait StatModifierCommands {
    fn set_stat_modifier<Source: Component, S: Send + Sync + 'static>(
        &mut self,
        op: StatOp,
    ) -> &mut Self;

    fn remove_stat_modifier<Source: Component, S: Send + Sync + 'static>(&mut self) -> &mut Self;
}

impl StatModifierCommands for EntityCommands<'_> {
    fn set_stat_modifier<Source: Component, S: Send + Sync + 'static>(
        &mut self,
        op: StatOp,
    ) -> &mut Self {
        let model = self.id();
        self.commands().queue(move |world: &mut World| {
            let Ok(mut entity) = world.get_entity_mut(model) else {
                return;
            };
            if let Some(mut modifiers) = entity.get_mut::<StatModifiers<S>>() {
                modifiers.set::<Source>(op);
            } else {
                let mut modifiers = StatModifiers::<S>::default();
                modifiers.set::<Source>(op);
                entity.insert(modifiers);
            }
        });
        self
    }

    fn remove_stat_modifier<Source: Component, S: Send + Sync + 'static>(&mut self) -> &mut Self {
        let model = self.id();
        self.commands().queue(move |world: &mut World| {
            let Ok(mut entity) = world.get_entity_mut(model) else {
                return;
            };
            let Some(mut modifiers) = entity.get_mut::<StatModifiers<S>>() else {
                return;
            };
            modifiers.remove::<Source>();
            if modifiers.is_empty() {
                entity.remove::<StatModifiers<S>>();
            }
        });
        self
    }
}

#[cfg(debug_assertions)]
pub(crate) fn check_stat_source_leaks<S: Send + Sync + 'static>(
    items: Query<EntityRef, With<StatModifiers<S>>>,
) {
    for item in &items {
        let Some(modifiers) = item.get::<StatModifiers<S>>() else {
            continue;
        };
        for entry in &modifiers.entries {
            if !item.contains_type_id(entry.source) {
                error!(
                    "entity {}: leaked {} modifier from removed source `{}`",
                    item.id(),
                    type_name::<S>(),
                    entry.source_name,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component)]
    struct SourceA;
    #[derive(Component)]
    struct SourceB;

    #[test]
    fn fold_is_staged_and_order_independent() {
        let mut forward = CooldownModifiers::default();
        forward.set::<SourceA>(StatOp::Flat(0.2));
        forward.set::<SourceB>(StatOp::Mult(2.0));

        let mut reverse = CooldownModifiers::default();
        reverse.set::<SourceB>(StatOp::Mult(2.0));
        reverse.set::<SourceA>(StatOp::Flat(0.2));

        assert_eq!(forward.apply_to(0.5), 1.4, "(0.5 + 0.2) × 2.0");
        assert_eq!(forward.apply_to(0.5), reverse.apply_to(0.5));
    }

    #[test]
    fn set_replaces_by_source_tag() {
        let mut modifiers = CooldownModifiers::default();
        modifiers.set::<SourceA>(StatOp::Mult(2.0));
        modifiers.set::<SourceA>(StatOp::Mult(3.0));
        assert_eq!(
            modifiers.apply_to(1.0),
            3.0,
            "re-set replaces, never stacks"
        );

        modifiers.remove::<SourceA>();
        assert!(modifiers.is_empty());
        assert_eq!(modifiers.apply_to(1.0), 1.0);
    }
}
