use bevy::prelude::*;

use super::{View, ViewOf, ViewTint};
use crate::{InspectContributors, Item, Rusty};

const RUST_COLOR: Color = Color::srgb(0.54, 0.27, 0.07);

pub struct RustyViewPlugin;

impl Plugin for RustyViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(tint_on_rust)
            .add_observer(tint_new_views)
            .add_observer(untint_on_rust_removal);
        app.world_mut()
            .resource_mut::<InspectContributors>()
            .register(rust_inspect_line);
    }
}

fn rust_inspect_line(model: EntityRef) -> Option<String> {
    model
        .contains::<Rusty>()
        .then(|| "rusty — cooldown ×2".to_string())
}

fn tint_on_rust(add: On<Add, Rusty>, models: Query<&View, With<Item>>, mut commands: Commands) {
    let Ok(Some(view)) = models.get(add.event().entity).map(View::entity) else {
        return;
    };
    if let Ok(mut view) = commands.get_entity(view) {
        view.try_insert(ViewTint(RUST_COLOR));
    }
}

fn tint_new_views(
    add: On<Add, ViewOf>,
    views: Query<&ViewOf>,
    rusty_models: Query<(), With<Rusty>>,
    mut commands: Commands,
) {
    let view = add.event().entity;
    let Ok(view_of) = views.get(view) else {
        return;
    };
    if rusty_models.get(view_of.0).is_err() {
        return;
    }
    if let Ok(mut view) = commands.get_entity(view) {
        view.try_insert(ViewTint(RUST_COLOR));
    }
}

fn untint_on_rust_removal(
    remove: On<Remove, Rusty>,
    models: Query<&View, With<Item>>,
    mut commands: Commands,
) {
    let Ok(Some(view)) = models.get(remove.event().entity).map(View::entity) else {
        return;
    };
    commands.queue(move |world: &mut World| {
        if let Ok(mut view) = world.get_entity_mut(view) {
            view.remove::<ViewTint>();
        }
    });
}
