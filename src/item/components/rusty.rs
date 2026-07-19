use bevy::prelude::*;

use crate::{Item, ItemState, StatFold, StatModifierRegistry, StatsDirty};

const RUST_AFTER_SECS: f32 = 5.0;
const RUST_COOLDOWN_MULT: f32 = 2.0;

#[derive(Component, Clone, Default)]
pub struct Rusty;

pub struct RustyPlugin;

impl Plugin for RustyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, rust_grounded_items)
            .add_observer(dirty_on_rust_added)
            .add_observer(dirty_on_rust_removed);
        app.world_mut()
            .resource_mut::<StatModifierRegistry>()
            .register(rust_modifier);
    }
}

fn rust_modifier(model: EntityRef, fold: &mut StatFold) {
    if model.contains::<Rusty>() {
        fold.cooldown_mult *= RUST_COOLDOWN_MULT;
    }
}

fn dirty_on_rust_added(add: On<Add, Rusty>, items: Query<(), With<Item>>, commands: Commands) {
    mark_stats_dirty(add.event().entity, &items, commands);
}

fn dirty_on_rust_removed(
    remove: On<Remove, Rusty>,
    items: Query<(), With<Item>>,
    commands: Commands,
) {
    mark_stats_dirty(remove.event().entity, &items, commands);
}

fn mark_stats_dirty(model: Entity, items: &Query<(), With<Item>>, mut commands: Commands) {
    if items.get(model).is_err() {
        return;
    }
    if let Ok(mut model) = commands.get_entity(model) {
        model.try_insert(StatsDirty);
    }
}

#[derive(Component, Clone, Default)]
pub struct GroundedSecs(pub f32);

#[expect(clippy::type_complexity)]
fn rust_grounded_items(
    time: Res<Time>,
    mut items: Query<(Entity, &ItemState, &mut GroundedSecs), (With<Item>, Without<Rusty>)>,
    mut commands: Commands,
) {
    for (item_e, state, mut grounded) in &mut items {
        if !state.is_on_ground() {
            continue;
        }
        grounded.0 += time.delta_secs();
        if grounded.0 >= RUST_AFTER_SECS {
            commands.entity(item_e).insert(Rusty);
        }
    }
}
