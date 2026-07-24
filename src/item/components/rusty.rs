use bevy::prelude::*;

use crate::{Cooldown, Item, OnGround, StatModifierCommands, StatOp};

const RUST_AFTER_SECS: f32 = 5.0;
const RUST_COOLDOWN_MULT: f32 = 2.0;

#[derive(Component, Clone, Default)]
pub struct Rusty;

pub struct RustyPlugin;

impl Plugin for RustyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, rust_grounded_items)
            .add_observer(attach_rust_modifier)
            .add_observer(detach_rust_modifier);
    }
}

fn attach_rust_modifier(add: On<Add, Rusty>, items: Query<(), With<Item>>, mut commands: Commands) {
    let model = add.event().entity;
    if items.get(model).is_err() {
        return;
    }
    commands
        .entity(model)
        .set_stat_modifier::<Rusty, Cooldown>(StatOp::Mult(RUST_COOLDOWN_MULT));
}

fn detach_rust_modifier(
    remove: On<Remove, Rusty>,
    items: Query<(), With<Item>>,
    mut commands: Commands,
) {
    let model = remove.event().entity;
    if items.get(model).is_err() {
        return;
    }
    commands
        .entity(model)
        .remove_stat_modifier::<Rusty, Cooldown>();
}

#[derive(Component, Clone, Default)]
pub struct GroundedSecs(pub f32);

#[expect(clippy::type_complexity)]
fn rust_grounded_items(
    time: Res<Time>,
    mut items: Query<(Entity, &mut GroundedSecs), (With<Item>, With<OnGround>, Without<Rusty>)>,
    mut commands: Commands,
) {
    for (item_e, mut grounded) in &mut items {
        grounded.0 += time.delta_secs();
        if grounded.0 >= RUST_AFTER_SECS {
            commands.entity(item_e).insert(Rusty);
        }
    }
}
