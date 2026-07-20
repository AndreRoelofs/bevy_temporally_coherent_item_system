use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::prelude::*;

use crate::{Ammo, CooldownModifiers, Firearm, Item, ItemState, Player, ViewOf};

#[derive(Resource, Default)]
pub struct LookTarget(pub Option<Entity>);

pub type InspectLineFn = fn(EntityRef) -> Option<String>;

#[derive(Resource, Default)]
pub struct InspectContributors {
    lines: Vec<InspectLineFn>,
}

impl InspectContributors {
    pub fn register(&mut self, line: InspectLineFn) -> &mut Self {
        self.lines.push(line);
        self
    }
}

pub fn inspect_lines(model: EntityRef, contributors: &InspectContributors) -> Vec<String> {
    let mut lines = Vec::new();

    let label = model.get::<Item>().map_or("?", |item| item.label.as_str());
    let state = model.get::<ItemState>().map(ItemState::kind);
    lines.push(format!("{label} — {state:?}"));

    if let Some(firearm) = model.get::<Firearm>() {
        if let Some(ammo) = model.get::<Ammo>() {
            lines.push(format!("ammo {}/{}", ammo.0, firearm.magazine_size));
        }
        let cooldown = firearm.cooldown.effective(model.get::<CooldownModifiers>());
        lines.push(format!("cooldown {:.2}s", cooldown.0));
    }

    lines.extend(contributors.lines.iter().filter_map(|line| line(model)));
    lines
}

pub(crate) fn look_at_target(
    mut raycast: MeshRayCast,
    player: Query<&GlobalTransform, With<Player>>,
    views: Query<&ViewOf>,
    mut target: ResMut<LookTarget>,
) {
    let Ok(camera) = player.single() else {
        return;
    };
    let ray = Ray3d::new(camera.translation(), camera.forward());
    let new_target = raycast
        .cast_ray(ray, &MeshRayCastSettings::default())
        .first()
        .and_then(|(hit, _)| views.get(*hit).ok())
        .map(|view_of| view_of.0);
    if target.0 != new_target {
        target.0 = new_target;
    }
}
