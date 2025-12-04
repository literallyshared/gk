use bevy_ecs::prelude::*;
use uuid::Uuid;

// TODO: add npc, projectile, spell? etc? Item?
#[derive(Component)]
pub struct Player;

// NOTE: Use this to map to NetEntityIdentifier. A Player will have
// NetEntityIdentifier::Player(id)
#[derive(Component)]
pub struct Identifier {
    pub id: Uuid,
}

#[derive(Component)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

#[derive(Component, Clone, PartialEq, Eq, Hash)]
pub struct CurrentMap(pub String);

#[derive(Component)]
pub struct Collider {
    pub dynamic: bool,
    pub w: f32,
    pub h: f32,
}
