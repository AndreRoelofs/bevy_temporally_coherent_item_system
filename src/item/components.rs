use bevy::prelude::*;

/// For when the player lets go of the item
/// and it runs away from the player.
#[derive(Component)]
pub struct IdleMovement {
    pub speed: f32,
}

/// When the item is equipped by the player.
#[derive(Component, Clone, Default)]
pub struct Equipped;

/// When the item is on the ground.
#[derive(Component, Clone, Default)]
pub struct OnGround(pub Vec3);
