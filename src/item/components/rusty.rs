//! Rust: the demo's proof that accumulated components survive transitions.
//! Everything about rusting lives here — the components, the system that
//! applies it, and the hook that keeps the view in sync with it.

use bevy::prelude::*;

use crate::item::view::refresh_view;
use crate::{Item, ItemRegistry, ItemState};

const RUST_AFTER_SECS: f32 = 5.0;

#[derive(Component, Clone, Default)]
pub struct Rusty;

pub struct RustyPlugin;

impl Plugin for RustyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, rust_grounded_items)
            .add_observer(view_on_rust);
    }
}

#[derive(Component, Clone, Default)]
pub struct GroundedSecs(pub f32);

type RustableItems<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static ItemState, &'static mut GroundedSecs),
    (With<Item>, Without<Rusty>),
>;

fn rust_grounded_items(time: Res<Time>, mut items: RustableItems, mut commands: Commands) {
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

pub fn view_on_rust(
    add: On<Add, Rusty>,
    models: Query<EntityRef, With<Item>>,
    registry: Res<ItemRegistry>,
    commands: Commands,
) {
    refresh_view(add.event().entity, &models, &registry, commands);
}
