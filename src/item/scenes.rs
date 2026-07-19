use bevy::prelude::*;

use super::{ItemRegistry, ItemState, ItemStateKind, Rusty};

/// Where an equipped item sits relative to its holder.
const HAND_OFFSET: Vec3 = Vec3::new(-0.3, -0.3, -0.6);

const GUN_COLOR: Color = Color::srgb(0.8, 0.8, 0.85);
const RUST_COLOR: Color = Color::srgb(0.54, 0.27, 0.07);

pub fn register_builtin_items(registry: &mut ItemRegistry) {
    registry.register("core::item::gun", gun_view);
}

/// The gun's view is a pure function of its model: state picks the shape,
/// rust picks the material. Position is not baked in — a grounded view is
/// parented to the model, whose `Transform` places it.
fn gun_view(model: EntityRef) -> Option<Box<dyn Scene>> {
    let state = model.get::<ItemState>()?;
    let color = if model.contains::<Rusty>() {
        RUST_COLOR
    } else {
        GUN_COLOR
    };
    let scene: Box<dyn Scene> = match state.kind() {
        ItemStateKind::OnGround => Box::new(bsn! {
            Mesh3d(asset_value(Cuboid::new(0.1, 0.2, 1.)))
            MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial::from(color)))
        }),
        ItemStateKind::Equipped => Box::new(bsn! {
            Transform::from_translation(HAND_OFFSET)
            Mesh3d(asset_value(Sphere::new(0.1)))
            MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial::from(color)))
        }),
        // Stored items have no visual presence at all.
        ItemStateKind::Stored => return None,
    };
    Some(scene)
}
