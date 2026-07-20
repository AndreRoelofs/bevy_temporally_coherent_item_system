//! Placeholder

use bevy::prelude::*;

use super::{View, ViewOf};
use crate::{InspectContributors, Item, Rusty};

const RUST_COLOR: Color = Color::srgb(0.54, 0.27, 0.07);

#[derive(Resource)]
struct RustMaterial(Handle<StandardMaterial>);

impl FromWorld for RustMaterial {
    fn from_world(world: &mut World) -> Self {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        Self(materials.add(StandardMaterial::from(RUST_COLOR)))
    }
}

pub struct RustyViewPlugin;

impl Plugin for RustyViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RustMaterial>()
            .add_observer(recolor_on_rust)
            .add_observer(recolor_new_views);
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

fn recolor_on_rust(
    add: On<Add, Rusty>,
    models: Query<&View, With<Item>>,
    rust: Res<RustMaterial>,
    mut commands: Commands,
) {
    let Ok(Some(view)) = models.get(add.event().entity).map(View::entity) else {
        return;
    };
    if let Ok(mut view) = commands.get_entity(view) {
        view.try_insert(MeshMaterial3d(rust.0.clone()));
    }
}

fn recolor_new_views(
    add: On<Add, ViewOf>,
    views: Query<&ViewOf>,
    rusty_models: Query<(), With<Rusty>>,
    rust: Res<RustMaterial>,
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
        view.try_insert(MeshMaterial3d(rust.0.clone()));
    }
}
