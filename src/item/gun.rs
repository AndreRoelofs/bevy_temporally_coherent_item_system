use bevy::prelude::*;

use super::Item;

// The basic gun component.
#[derive(Component, Clone, Default)]
pub struct Gun(pub Item);
