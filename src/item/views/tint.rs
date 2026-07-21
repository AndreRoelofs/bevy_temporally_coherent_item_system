use std::collections::HashMap;

use bevy::color::ColorToPacked;
use bevy::prelude::*;

use super::ViewOf;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[component(immutable)]
pub struct ViewTint(pub Color);

#[derive(Resource, Default)]
struct TintMaterials(HashMap<[u8; 4], Handle<StandardMaterial>>);

impl TintMaterials {
    fn handle(
        &mut self,
        tint: Color,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.0
            .entry(tint.to_linear().to_u8_array())
            .or_insert_with(|| materials.add(StandardMaterial::from(tint)))
            .clone()
    }
}

#[derive(Component)]
struct UntintedMaterial(Handle<StandardMaterial>);

#[derive(Component)]
struct UntintedColor(Color);

pub struct ViewTintPlugin;

impl Plugin for ViewTintPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TintMaterials>()
            .add_observer(apply_tint)
            .add_observer(restore_untinted);
    }
}

#[expect(clippy::type_complexity)]
fn apply_tint(
    insert: On<Insert, ViewTint>,
    views: Query<
        (
            &ViewTint,
            Option<&MeshMaterial3d<StandardMaterial>>,
            Option<&BackgroundColor>,
            Has<UntintedMaterial>,
            Has<UntintedColor>,
        ),
        With<ViewOf>,
    >,
    mut cache: ResMut<TintMaterials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let view = insert.event().entity;
    let Ok((&ViewTint(tint), material, background, stashed_material, stashed_color)) =
        views.get(view)
    else {
        return;
    };
    let Ok(mut entity) = commands.get_entity(view) else {
        return;
    };
    if let Some(material) = material {
        if !stashed_material {
            entity.try_insert(UntintedMaterial(material.0.clone()));
        }
        let handle = cache.handle(tint, &mut materials);
        entity.try_insert(MeshMaterial3d(handle));
    } else if let Some(&BackgroundColor(base)) = background {
        if !stashed_color {
            entity.try_insert(UntintedColor(base));
        }
        entity.try_insert(BackgroundColor(tint));
    }
}

fn restore_untinted(
    remove: On<Remove, ViewTint>,
    views: Query<EntityRef, With<ViewOf>>,
    mut commands: Commands,
) {
    let Ok(view) = views.get(remove.event().entity) else {
        return;
    };
    let material = view.get::<UntintedMaterial>().map(|stash| stash.0.clone());
    let color = view.get::<UntintedColor>().map(|stash| stash.0);
    if material.is_none() && color.is_none() {
        return;
    }
    let entity = view.id();
    commands.queue(move |world: &mut World| {
        let Ok(mut view) = world.get_entity_mut(entity) else {
            return;
        };
        if let Some(handle) = material {
            view.insert(MeshMaterial3d(handle));
            view.remove::<UntintedMaterial>();
        }
        if let Some(color) = color {
            view.insert(BackgroundColor(color));
            view.remove::<UntintedColor>();
        }
    });
}
