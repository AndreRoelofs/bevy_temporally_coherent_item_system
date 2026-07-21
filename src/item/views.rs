//! Placeholder

use bevy::prelude::*;
use bevy::scene::ScenePatch;

use super::{ContainedBy, Item, ItemRegistry, ItemState, ItemStateKind};

mod gun;
mod inventory_ui;
mod rusty;
mod tint;

pub use gun::GunViewPlugin;
pub use inventory_ui::*;
pub use rusty::RustyViewPlugin;
pub use tint::*;

pub struct ItemViewsPlugin;

impl Plugin for ItemViewsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemRegistry>()
            .add_observer(view_on_state_change)
            .add_plugins((
                ViewTintPlugin,
                InventoryUiPlugin,
                GunViewPlugin,
                RustyViewPlugin,
            ));
    }
}

#[derive(Component, Clone, Default)]
pub struct HandSocket;

#[derive(Component)]
#[relationship(relationship_target = View)]
pub struct ViewOf(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = ViewOf, linked_spawn)]
pub struct View(Entity);

impl View {
    pub fn entity(&self) -> Option<Entity> {
        (self.0 != Entity::PLACEHOLDER).then_some(self.0)
    }
}

fn view_on_state_change(
    insert: On<Insert, ItemState>,
    models: Query<EntityRef, With<Item>>,
    registry: Res<ItemRegistry>,
    children: Query<&Children>,
    sockets: Query<(), With<HandSocket>>,
    panels: Query<&InventoryUi>,
    mut commands: Commands,
) {
    refresh_view(
        insert.event().entity,
        &models,
        &registry,
        &children,
        &sockets,
        &panels,
        commands.reborrow(),
    );
}

pub(crate) fn refresh_view(
    model: Entity,
    models: &Query<EntityRef, With<Item>>,
    registry: &ItemRegistry,
    children: &Query<&Children>,
    sockets: &Query<(), With<HandSocket>>,
    panels: &Query<&InventoryUi>,
    mut commands: Commands,
) {
    let Ok(model_ref) = models.get(model) else {
        return;
    };

    commands.entity(model).despawn_related::<View>();

    let Some(kind) = model_ref.get::<ItemState>().map(ItemState::kind) else {
        return;
    };
    let Some(chrome) = registry.chrome(model_ref, kind) else {
        return;
    };

    let parent = match kind {
        ItemStateKind::OnGround => model,
        ItemStateKind::Equipped => match model_ref.get::<ContainedBy>() {
            Some(contained) => {
                let holder = contained.container();
                socket_of(holder, children, sockets).unwrap_or_else(|| {
                    warn!("holder {holder} has no HandSocket; parenting view to its root");
                    holder
                })
            }
            None => {
                warn!("equipped item {model} has no holder; parenting view to the model");
                model
            }
        },
        ItemStateKind::Stored => {
            let Some(contained) = model_ref.get::<ContainedBy>() else {
                warn!("stored item {model} has no container; skipping its view");
                return;
            };
            match panels
                .get(contained.container())
                .ok()
                .and_then(InventoryUi::entity)
            {
                Some(panel) => panel,
                None => return,
            }
        }
    };

    let chrome = chrome.clone();
    let spawned = commands.spawn_empty().id();
    commands.queue(move |world: &mut World| {
        world.resource_scope(|world, patches: Mut<Assets<ScenePatch>>| {
            let Some(patch) = patches.get(&chrome) else {
                error!("chrome scene patch missing for view {spawned}");
                return;
            };
            let Ok(mut view) = world.get_entity_mut(spawned) else {
                return;
            };
            if let Err(err) = patch.apply(&mut view) {
                error!("failed to apply chrome to view {spawned}: {err}");
            }
        });
    });
    commands
        .entity(spawned)
        .insert((ViewOf(model), ChildOf(parent)));
}

fn socket_of(
    holder: Entity,
    children: &Query<&Children>,
    sockets: &Query<(), With<HandSocket>>,
) -> Option<Entity> {
    children
        .get(holder)
        .ok()?
        .iter()
        .find(|&child| sockets.get(child).is_ok())
}
